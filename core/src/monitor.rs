use std::io::Read;
use std::time::Duration;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::models::LinkStatus;

const MAX_BODY_BYTES: u64 = 2 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(20);
const USER_AGENT: &str = "websave-link-checker/0.1";
/// Some CDNs/routers classify requests without an HTML `Accept` header as
/// API traffic and 404 perfectly healthy pages (crates.io does this).
const ACCEPT: &str = "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8";
/// Fallback identity for sites that reject unknown bots outright.
const BROWSER_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
     AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36";

/// Version of the content-hash basis.
///
/// - `0`: raw response bytes. Every counter, nonce, analytics payload and
///   asset build hash flipped it, so `Changed` fired on most checks.
/// - `1`: normalized readable text (see [`content_hash`]).
///
/// Bump this whenever the bytes fed to the hash change meaning. A stored hash
/// carrying an older version is not comparable to a fresh one, so it is
/// re-baselined instead of reported as `Changed`. See
/// `docs/content-hashing.md`.
pub const HASH_ALGO: u32 = 1;

/// The minimal data needed to check one save without holding the vault lock.
#[derive(Debug, Clone)]
pub struct CheckTarget {
    pub id: i64,
    pub url: String,
    pub content_hash: String,
    /// [`HASH_ALGO`] version `content_hash` was computed under.
    pub hash_algo: u32,
}

/// Result of probing a URL. `None` fields mean "leave the stored value alone".
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckOutcome {
    pub status: LinkStatus,
    pub http_status: Option<u16>,
    pub redirect_url: Option<String>,
    pub content_hash: Option<String>,
    /// Cover image URL (og:image and friends) found in the page, absolute.
    pub og_image: Option<String>,
    /// Readable text extracted from the page, for the archive snapshot.
    pub archive_text: Option<String>,
}

/// Probe a URL and classify it.
///
/// - `redirected`: the request ended at a meaningfully different location
///   (http→https upgrades and `www.` changes are treated as the same place).
/// - `changed`: reachable, but the content hash differs from `previous_hash`.
/// - `dead`: transport/DNS failure, 404/410, other client errors, or 5xx.
///   401/403/405/406/429 count as `active` — the page exists, it just
///   doesn't want to talk to a bot.
///
/// `previous_algo` is the [`HASH_ALGO`] version `previous_hash` was computed
/// under. A stale version means the two hashes aren't comparable: the check
/// re-baselines (reports `active`, stores the fresh hash) rather than
/// reporting a change it can't actually see.
///
/// This performs a blocking network call; run it off the UI thread.
pub fn check_url(url: &str, previous_hash: &str, previous_algo: u32) -> CheckOutcome {
    log::debug!("checking {url}");
    let previous_hash = if previous_algo == HASH_ALGO {
        previous_hash
    } else {
        if !previous_hash.is_empty() {
            log::debug!("{url}: hash algo {previous_algo} → {HASH_ALGO}, re-baselining");
        }
        ""
    };
    let agent = ureq::AgentBuilder::new()
        .timeout(TIMEOUT)
        .redirects(8)
        .user_agent(USER_AGENT)
        .build();

    let first = probe(&agent, url, previous_hash, None);
    // A dead verdict from an HTTP status (not a transport failure) may just
    // be bot filtering — confirm with a browser identity before believing it.
    if first.status == LinkStatus::Dead && first.http_status.is_some() {
        log::debug!(
            "{url} looked dead (http {:?}), retrying as browser",
            first.http_status
        );
        let second = probe(&agent, url, previous_hash, Some(BROWSER_UA));
        if second.status != LinkStatus::Dead {
            return second;
        }
    }
    first
}

fn probe(
    agent: &ureq::Agent,
    url: &str,
    previous_hash: &str,
    user_agent: Option<&str>,
) -> CheckOutcome {
    let mut request = agent
        .get(url)
        .set("Accept", ACCEPT)
        .set("Accept-Language", "en;q=0.9, *;q=0.5");
    if let Some(ua) = user_agent {
        request = request.set("User-Agent", ua);
    }

    match request.call() {
        Ok(resp) => {
            let code = resp.status();
            let final_url = resp.get_url().to_string();
            let is_html = resp.content_type().to_ascii_lowercase().contains("html");
            let redirected = !same_destination(url, &final_url);
            let mut body = Vec::new();
            let _ = resp
                .into_reader()
                .take(MAX_BODY_BYTES)
                .read_to_end(&mut body);
            let (og_image, archive_text) = if is_html {
                let html = String::from_utf8_lossy(&body);
                (
                    extract_og_image(&html, &final_url),
                    Some(crate::extract::extract_readable_text(&html)),
                )
            } else {
                (None, None)
            };
            let hash = content_hash(&body, archive_text.as_deref());
            let status = if (300..400).contains(&code) || redirected {
                LinkStatus::Redirected
            } else if !previous_hash.is_empty() && previous_hash != hash {
                LinkStatus::Changed
            } else {
                LinkStatus::Active
            };
            CheckOutcome {
                status,
                http_status: Some(code),
                redirect_url: Some(if redirected { final_url } else { String::new() }),
                content_hash: Some(hash),
                og_image,
                archive_text,
            }
        }
        Err(ureq::Error::Status(code, resp)) => {
            let final_url = resp.get_url().to_string();
            let redirected = !same_destination(url, &final_url);
            let status = match code {
                401 | 403 | 405 | 406 | 429 => {
                    if redirected {
                        LinkStatus::Redirected
                    } else {
                        LinkStatus::Active
                    }
                }
                _ => LinkStatus::Dead,
            };
            CheckOutcome {
                status,
                http_status: Some(code),
                redirect_url: Some(if redirected { final_url } else { String::new() }),
                content_hash: None,
                og_image: None,
                archive_text: None,
            }
        }
        Err(_) => CheckOutcome {
            status: LinkStatus::Dead,
            http_status: None,
            redirect_url: None,
            content_hash: None,
            og_image: None,
            archive_text: None,
        },
    }
}

/// Pull the page's cover image URL out of its HTML head metadata:
/// `og:image`, `og:image:url`, `twitter:image`, or `link rel="image_src"`.
/// Relative URLs are resolved against the page's final URL.
pub fn extract_og_image(html: &str, base_url: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let keys = [
        "og:image",
        "og:image:url",
        "twitter:image",
        "twitter:image:src",
    ];
    let mut pos = 0;
    let mut found: Option<String> = None;

    while let Some(i) = lower.get(pos..).and_then(|s| s.find("<meta")) {
        let start = pos + i;
        let end = lower
            .get(start..)
            .and_then(|s| s.find('>'))
            .map(|e| start + e);
        let Some(end) = end else { break };
        let tag = &html[start..end];
        let key = crate::import::attr(tag, "property")
            .or_else(|| crate::import::attr(tag, "name"))
            .unwrap_or_default()
            .to_ascii_lowercase();
        if keys.contains(&key.as_str()) {
            if let Some(content) = crate::import::attr(tag, "content") {
                if !content.trim().is_empty() {
                    found = Some(content.trim().to_string());
                    break;
                }
            }
        }
        pos = end;
    }

    if found.is_none() {
        // <link rel="image_src" href="...">
        if let Some(i) = lower
            .find("rel=\"image_src\"")
            .or_else(|| lower.find("rel='image_src'"))
        {
            let start = lower[..i].rfind("<link").unwrap_or(i);
            let end = lower
                .get(start..)
                .and_then(|s| s.find('>'))
                .map(|e| start + e);
            if let Some(end) = end {
                found = crate::import::attr(&html[start..end], "href");
            }
        }
    }

    let raw = found?;
    let base = url::Url::parse(base_url).ok()?;
    let absolute = base.join(&raw).ok()?;
    if absolute.scheme() == "http" || absolute.scheme() == "https" {
        Some(absolute.to_string())
    } else {
        None
    }
}

const MAX_IMAGE_BYTES: u64 = 5 * 1024 * 1024;

/// Download a cover image; returns the bytes and a file extension derived
/// from the content type. Non-images and oversized files are rejected.
pub fn download_image(url: &str) -> Option<(Vec<u8>, &'static str)> {
    let resp = ureq::AgentBuilder::new()
        .timeout(TIMEOUT)
        .redirects(8)
        .user_agent(BROWSER_UA)
        .build()
        .get(url)
        .set("Accept", "image/*")
        .call()
        .ok()?;
    let ext = match resp.content_type().to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/avif" => "avif",
        other => {
            log::debug!("thumbnail: skipping {url}: content-type {other}");
            return None;
        }
    };
    let mut bytes = Vec::new();
    resp.into_reader()
        .take(MAX_IMAGE_BYTES)
        .read_to_end(&mut bytes)
        .ok()?;
    (!bytes.is_empty()).then_some((bytes, ext))
}

/// Whether two URLs point at the same place for monitoring purposes:
/// scheme upgrades and `www.` prefixes are ignored, trailing slashes trimmed.
fn same_destination(a: &str, b: &str) -> bool {
    fn key(raw: &str) -> Option<(String, String, String)> {
        let u = url::Url::parse(raw).ok()?;
        let host = u.host_str()?.trim_start_matches("www.").to_string();
        let path = u.path().trim_end_matches('/').to_string();
        let query = u.query().unwrap_or("").to_string();
        Some((host, path, query))
    }
    match (key(a), key(b)) {
        (Some(ka), Some(kb)) => ka == kb,
        _ => a == b,
    }
}

/// Fingerprint a response for change detection, at [`HASH_ALGO`] version 1.
///
/// Hashes the page's normalized readable text when there is any, so the
/// question `Changed` answers is "did the words change" rather than "did the
/// server emit different bytes" — which is almost always yes and almost never
/// interesting.
///
/// Falls back to the raw body for non-HTML responses (PDFs, images, JSON) and
/// for HTML that extracts to nothing. That fallback is load-bearing: hashing
/// an empty extraction would collide every content-free page onto one hash,
/// and no change on any of them would ever be detected.
fn content_hash(body: &[u8], archive_text: Option<&str>) -> String {
    match archive_text {
        Some(text) if !text.trim().is_empty() => {
            hex_sha256(crate::extract::normalize_for_hash(text).as_bytes())
        }
        _ => hex_sha256(body),
    }
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for b in digest {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{content_hash, extract_og_image, same_destination};

    /// Simulates two fetches of the same page: `extract_readable_text` has
    /// already run, so this is exactly what `probe()` feeds the hash.
    fn hash_page(html: &str) -> String {
        let text = crate::extract::extract_readable_text(html);
        content_hash(html.as_bytes(), Some(&text))
    }

    #[test]
    fn chrome_churn_does_not_change_the_hash() {
        let first = r#"<body>
            <article><h1>Guide</h1><p>The steps have not changed.</p></article>
            <aside>1,204 stars · updated 3 hours ago</aside>
            <script src="/assets/index-a3f9c2.js"></script>
            <meta name="csrf-token" content="tok-11111111">
        </body>"#;
        let second = r#"<body>
            <article><h1>Guide</h1><p>The steps have not changed.</p></article>
            <aside>1,209 stars · updated 12 minutes ago</aside>
            <script src="/assets/index-77b0de.js"></script>
            <meta name="csrf-token" content="tok-99999999">
        </body>"#;
        assert_ne!(
            super::hex_sha256(first.as_bytes()),
            super::hex_sha256(second.as_bytes()),
            "raw bodies differ — this is what used to flag Changed"
        );
        assert_eq!(hash_page(first), hash_page(second));
    }

    #[test]
    fn rewritten_prose_does_change_the_hash() {
        let before = "<body><article><p>Ships with batteries included.</p></article></body>";
        let after = "<body><article><p>Batteries are sold separately.</p></article></body>";
        assert_ne!(hash_page(before), hash_page(after));
    }

    /// Without the empty-extraction fallback every content-free page would
    /// collide on one hash and no change would ever be detected.
    #[test]
    fn empty_extraction_falls_back_to_the_body() {
        let a = content_hash(b"%PDF-1.7 first revision", Some(""));
        let b = content_hash(b"%PDF-1.7 second revision", Some(""));
        assert_ne!(a, b);
        assert_eq!(a, super::hex_sha256(b"%PDF-1.7 first revision"));

        // Non-HTML responses carry no extraction at all.
        let pdf_a = content_hash(b"binary-v1", None);
        let pdf_b = content_hash(b"binary-v2", None);
        assert_ne!(pdf_a, pdf_b);
    }

    #[test]
    fn hash_algo_is_pinned() {
        // A change to the hashing rule must come with a HASH_ALGO bump, or
        // stored hashes silently stop being comparable.
        assert_eq!(super::HASH_ALGO, 1);
        assert_eq!(
            hash_page("<body><article><p>Fixed input.</p></article></body>"),
            super::hex_sha256(crate::extract::normalize_for_hash("Fixed input.").as_bytes())
        );
    }

    #[test]
    fn og_image_extraction_and_resolution() {
        let html = r#"<html><head>
            <meta charset="utf-8">
            <meta name="description" content="not an image">
            <meta property="og:image" content="/static/cover.png?v=2">
        </head></html>"#;
        assert_eq!(
            extract_og_image(html, "https://example.com/post/1"),
            Some("https://example.com/static/cover.png?v=2".into())
        );

        let twitter = r#"<META NAME="twitter:image" CONTENT="https://cdn.example.com/t.jpg">"#;
        assert_eq!(
            extract_og_image(twitter, "https://example.com"),
            Some("https://cdn.example.com/t.jpg".into())
        );

        assert_eq!(
            extract_og_image("<html>no meta</html>", "https://e.com"),
            None
        );
        assert_eq!(
            extract_og_image(
                r#"<meta property="og:image" content="data:image/png;base64,xx">"#,
                "https://e.com"
            ),
            None,
            "non-http schemes rejected"
        );
    }

    #[test]
    fn scheme_and_www_changes_are_same_destination() {
        assert!(same_destination(
            "http://example.com/docs",
            "https://www.example.com/docs/"
        ));
        assert!(!same_destination(
            "https://example.com/docs",
            "https://example.com/other"
        ));
        assert!(!same_destination(
            "https://example.com/docs",
            "https://elsewhere.org/docs"
        ));
    }
}
