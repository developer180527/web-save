//! Readable-text extraction for archive snapshots.
//!
//! Deliberately simple and dependency-free: the goal is *searchable* text,
//! not a pixel-perfect reading mode. Scripts, styles and chrome are
//! stripped, the `<article>`/`<main>` subtree is preferred when present,
//! and the result is plain whitespace-normalized text.

const MAX_ARCHIVE_CHARS: usize = 100_000;

/// Blocks whose contents are never prose.
const NOISE_BLOCKS: &[&str] = &["script", "style", "noscript", "svg", "template", "head"];

/// Extract the readable text of an HTML page. Returns an empty string when
/// nothing useful remains.
pub fn extract_readable_text(html: &str) -> String {
    let mut cleaned = html.to_string();
    for tag in NOISE_BLOCKS {
        cleaned = strip_blocks(&cleaned, tag);
    }

    // Prefer the semantic content container when the page declares one, but
    // only if it actually holds prose. Sites increasingly emit decorative
    // empty `<article>`/`<main>` wrappers (GitHub does); picking one blindly
    // yielded an empty extraction for the whole page.
    for scope in [
        subtree(&cleaned, "article"),
        subtree(&cleaned, "main"),
        subtree(&cleaned, "body"),
        Some(cleaned.as_str()),
    ]
    .into_iter()
    .flatten()
    {
        let text = flatten(scope);
        if !text.is_empty() {
            return text;
        }
    }
    String::new()
}

/// Tags to text: strip markup, decode entities, collapse whitespace, cap.
fn flatten(html: &str) -> String {
    let text = strip_tags(html);
    let decoded = crate::import::decode_entities(&text);
    let mut out = String::with_capacity(decoded.len().min(MAX_ARCHIVE_CHARS));
    for (i, word) in decoded.split_whitespace().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(word);
        if out.len() >= MAX_ARCHIVE_CHARS {
            break;
        }
    }
    out
}

/// Sentinel replacing any numeric token when normalizing for the content hash.
const NUM: &str = "\u{2039}n\u{203a}";
/// Sentinel replacing a relative-time phrase ("3 hours ago", "last week").
const TIME: &str = "\u{2039}t\u{203a}";

/// Time units, as they appear both spelled out and abbreviated. Used to
/// recognize the `<number> <unit> ago` shape.
const TIME_UNITS: &[&str] = &[
    "s", "sec", "secs", "second", "seconds", "m", "min", "mins", "minute", "minutes", "h", "hr",
    "hrs", "hour", "hours", "d", "day", "days", "w", "wk", "wks", "week", "weeks", "mo", "mon",
    "mos", "month", "months", "y", "yr", "yrs", "year", "years",
];

/// Standalone words that are themselves relative times.
const TIME_WORDS: &[&str] = &["now", "today", "yesterday", "tomorrow"];

/// Canonicalize extracted text for change detection.
///
/// The archive keeps the verbatim text; this is only the hash basis. It
/// absorbs the churn that would otherwise make every check report `Changed`:
/// counters that tick (`169k` stars), and relative timestamps that move on
/// their own (`3 hours ago`). Case is folded so a styling change that swaps
/// text-transform doesn't count either.
///
/// See `docs/content-hashing.md`.
pub fn normalize_for_hash(text: &str) -> String {
    let mut out: Vec<String> = Vec::new();

    for raw in text.split_whitespace() {
        // Trim surrounding punctuation so `169k,` classifies as a number and
        // `(today)` as a time word.
        let word = raw.trim_matches(|c: char| !c.is_alphanumeric());
        if word.is_empty() {
            continue;
        }
        let lower: String = word.chars().flat_map(|c| c.to_lowercase()).collect();

        // `3 hours ago` / `7h ago` collapse to a single sentinel.
        if lower == "ago" {
            collapse_time_phrase(&mut out);
        } else if TIME_WORDS.contains(&lower.as_str()) {
            out.push(TIME.to_string());
        } else if is_numeric_token(&lower) {
            out.push(NUM.to_string());
        } else {
            out.push(lower);
        }
    }

    out.join(" ")
}

/// On seeing `ago`, drop the quantity that preceded it and emit one sentinel.
/// Handles both `3 hours ago` (two tokens) and `7h ago` (one).
fn collapse_time_phrase(out: &mut Vec<String>) {
    // `hours` in `3 hours ago`
    if out.last().is_some_and(|t| TIME_UNITS.contains(&t.as_str())) {
        out.pop();
    }
    // the number in `3 hours ago`, or the fused `7h`
    if out.last().is_some_and(|t| t == NUM) {
        out.pop();
    }
    out.push(TIME.to_string());
}

/// Whether a token is a count rather than a word: `42`, `3.2k`, `12,481`,
/// `169k`, `1.5m`, `7h`. Trailing magnitude and unit suffixes are ignored so
/// a counter rolling over from `169k` to `170k` normalizes identically.
fn is_numeric_token(lower: &str) -> bool {
    let digits = lower.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    if digits.is_empty() || !digits.starts_with(|c: char| c.is_ascii_digit()) {
        return false;
    }
    digits
        .chars()
        .all(|c| c.is_ascii_digit() || c == ',' || c == '.' || c == ':' || c == '-' || c == '/')
}

/// Remove every `<tag ...>...</tag>` block, case-insensitively.
fn strip_blocks(html: &str, tag: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let open = format!("<{tag}");
    let close = format!("</{tag}");
    let mut out = String::with_capacity(html.len());
    let mut pos = 0;
    while let Some(i) = find_tag(&lower, &open, pos) {
        out.push_str(&html[pos..i]);
        match lower.get(i..).and_then(|s| s.find(&close)) {
            Some(rel) => {
                let after = i + rel;
                pos = match lower.get(after..).and_then(|s| s.find('>')) {
                    Some(gt) => after + gt + 1,
                    None => lower.len(),
                };
            }
            None => {
                // Unclosed: drop the rest.
                pos = lower.len();
            }
        }
    }
    out.push_str(&html[pos..]);
    out
}

/// Inner HTML of the first `<tag ...>...</tag>` occurrence.
fn subtree<'a>(html: &'a str, tag: &str) -> Option<&'a str> {
    let lower = html.to_ascii_lowercase();
    let start = find_tag(&lower, &format!("<{tag}"), 0)?;
    let open_end = start + lower.get(start..)?.find('>')?;
    // Last close tag, so nested same-name elements stay included.
    let close = lower.rfind(&format!("</{tag}"))?;
    if close <= open_end {
        return None;
    }
    Some(&html[open_end + 1..close])
}

/// Find `<tag` followed by whitespace or `>`, so `<art` doesn't match
/// `<artifact>`.
fn find_tag(lower: &str, open: &str, from: usize) -> Option<usize> {
    let mut pos = from;
    while let Some(i) = lower.get(pos..).map(|s| s.find(open)).flatten() {
        let abs = pos + i;
        match lower.as_bytes().get(abs + open.len()) {
            Some(b'>') | Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r') | Some(b'/') => {
                return Some(abs)
            }
            None => return None,
            _ => pos = abs + open.len(),
        }
    }
    None
}

/// Replace every tag with a space (block boundaries become word breaks).
fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => {
                in_tag = true;
                out.push(' ');
            }
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_noise_and_keeps_prose() {
        let html = r#"<html><head><title>T</title><style>.x{color:red}</style></head>
        <body><script>var x = "ignore me";</script>
        <nav><a href="/">Home</a></nav>
        <p>Hello &amp; welcome to the <b>article</b> body.</p>
        </body></html>"#;
        let text = extract_readable_text(html);
        assert!(text.contains("Hello & welcome to the article body."));
        assert!(!text.contains("ignore me"));
        assert!(!text.contains("color:red"));
        assert!(!text.contains('<'));
    }

    #[test]
    fn prefers_article_over_page_chrome() {
        let html = r#"<body>
          <header>Site chrome everywhere</header>
          <article><h1>Real Title</h1><p>The actual content lives here.</p></article>
          <footer>Copyright chrome</footer>
        </body>"#;
        let text = extract_readable_text(html);
        assert!(text.contains("The actual content lives here."));
        assert!(!text.contains("Site chrome"));
        assert!(!text.contains("Copyright"));
    }

    #[test]
    fn caps_length_and_collapses_whitespace() {
        let big = format!("<body><p>{}</p></body>", "word ".repeat(40_000));
        let text = extract_readable_text(&big);
        assert!(text.len() <= MAX_ARCHIVE_CHARS + 10);
        assert!(!text.contains("  "), "whitespace collapsed");
    }

    /// The whole point of the normalized hash: a repo page whose counters and
    /// relative timestamps tick on their own must fingerprint identically.
    /// A decorative empty `<article>` must not swallow the page. GitHub emits
    /// one before the README, which made every such page archive as "".
    #[test]
    fn empty_semantic_wrapper_falls_through_to_real_content() {
        let html = r#"<body>
            <article class="decorative"><span></span></article>
            <main><p>The README body that should be archived.</p></main>
        </body>"#;
        let text = extract_readable_text(html);
        assert!(text.contains("The README body that should be archived."));

        // ...and when nothing has prose, the result is still empty.
        assert_eq!(
            extract_readable_text("<body><article> </article></body>"),
            ""
        );
    }

    #[test]
    fn counters_and_relative_times_normalize_away() {
        let before = "MuseScore 169k stars 12,481 forks updated 3 hours ago";
        let after = "MuseScore 170k stars 12,492 forks updated 5 minutes ago";
        assert_eq!(normalize_for_hash(before), normalize_for_hash(after));

        // Abbreviated and fused forms collapse the same way.
        assert_eq!(
            normalize_for_hash("released 7h ago"),
            normalize_for_hash("released 3 days ago")
        );
        assert_eq!(
            normalize_for_hash("commented today"),
            normalize_for_hash("commented yesterday")
        );
    }

    #[test]
    fn real_prose_changes_survive_normalization() {
        assert_ne!(
            normalize_for_hash("The API now requires an auth token."),
            normalize_for_hash("The API no longer requires an auth token.")
        );
        // A number changing to a word is a real change, not a counter tick.
        assert_ne!(
            normalize_for_hash("supports 3 formats"),
            normalize_for_hash("supports many formats")
        );
    }

    #[test]
    fn normalization_folds_case_and_punctuation() {
        assert_eq!(
            normalize_for_hash("Hello, World!"),
            normalize_for_hash("hello world")
        );
        // Words that merely contain digits are not counters.
        assert_eq!(normalize_for_hash("utf8"), "utf8");
        assert_ne!(normalize_for_hash("h264"), normalize_for_hash("h265"));
    }

    #[test]
    fn analytics_and_scripts_never_reach_the_hash() {
        let with_beacon = r#"<body><article><p>Stable prose.</p></article>
            <script>window.__ID__="req-8f2a1c";</script></body>"#;
        let other_beacon = r#"<body><article><p>Stable prose.</p></article>
            <script>window.__ID__="req-0000ff";</script></body>"#;
        assert_eq!(
            normalize_for_hash(&extract_readable_text(with_beacon)),
            normalize_for_hash(&extract_readable_text(other_beacon))
        );
    }

    #[test]
    fn handles_tagless_and_broken_input() {
        assert_eq!(extract_readable_text("plain words"), "plain words");
        assert_eq!(extract_readable_text("<script>only noise"), "");
        assert_eq!(extract_readable_text(""), "");
    }
}
