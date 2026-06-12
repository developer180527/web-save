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

    // Prefer the semantic content container when the page declares one.
    let scope = subtree(&cleaned, "article")
        .or_else(|| subtree(&cleaned, "main"))
        .or_else(|| subtree(&cleaned, "body"))
        .unwrap_or(cleaned.as_str());

    let text = strip_tags(scope);
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

    #[test]
    fn handles_tagless_and_broken_input() {
        assert_eq!(extract_readable_text("plain words"), "plain words");
        assert_eq!(extract_readable_text("<script>only noise"), "");
        assert_eq!(extract_readable_text(""), "");
    }
}
