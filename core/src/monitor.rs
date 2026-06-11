use std::io::Read;
use std::time::Duration;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::models::LinkStatus;

const MAX_BODY_BYTES: u64 = 2 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(20);
const USER_AGENT: &str = "websave-link-checker/0.1";

/// The minimal data needed to check one save without holding the vault lock.
#[derive(Debug, Clone)]
pub struct CheckTarget {
    pub id: i64,
    pub url: String,
    pub content_hash: String,
}

/// Result of probing a URL. `None` fields mean "leave the stored value alone".
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckOutcome {
    pub status: LinkStatus,
    pub http_status: Option<u16>,
    pub redirect_url: Option<String>,
    pub content_hash: Option<String>,
}

/// Probe a URL and classify it.
///
/// - `redirected`: the request ended at a meaningfully different location
///   (http→https upgrades and `www.` changes are treated as the same place).
/// - `changed`: reachable, but the body hash differs from `previous_hash`.
/// - `dead`: transport/DNS failure, 404/410, other client errors, or 5xx.
///   401/403/405/406/429 count as `active` — the page exists, it just
///   doesn't want to talk to a bot.
///
/// This performs a blocking network call; run it off the UI thread.
pub fn check_url(url: &str, previous_hash: &str) -> CheckOutcome {
    log::debug!("checking {url}");
    let agent = ureq::AgentBuilder::new()
        .timeout(TIMEOUT)
        .redirects(8)
        .user_agent(USER_AGENT)
        .build();

    match agent.get(url).call() {
        Ok(resp) => {
            let code = resp.status();
            let final_url = resp.get_url().to_string();
            let redirected = !same_destination(url, &final_url);
            let mut body = Vec::new();
            let _ = resp
                .into_reader()
                .take(MAX_BODY_BYTES)
                .read_to_end(&mut body);
            let hash = hex_sha256(&body);
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
            }
        }
        Err(_) => CheckOutcome {
            status: LinkStatus::Dead,
            http_status: None,
            redirect_url: None,
            content_hash: None,
        },
    }
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
    use super::same_destination;

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
