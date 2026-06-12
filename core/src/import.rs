//! Bookmark import: Netscape bookmark HTML (every browser's export format,
//! plus Pocket's HTML variant), Raindrop.io / Pocket / generic CSV, and
//! plain URL lists.
//!
//! Parsers are lenient by design: imports come from a zoo of exporters, so
//! anything unparseable is skipped rather than failing the whole file.

use serde::Serialize;

/// One parsed bookmark, ready to be merged into the vault.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImportItem {
    pub url: String,
    pub title: String,
    pub description: String,
    pub notes: String,
    pub tags: Vec<String>,
    pub favorite: bool,
    /// Original save time (unix seconds), when the source provides one.
    pub created_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImportFormat {
    NetscapeHtml,
    RaindropCsv,
    PocketCsv,
    GenericCsv,
    UrlList,
}

impl ImportFormat {
    pub fn label(self) -> &'static str {
        match self {
            ImportFormat::NetscapeHtml => "Browser bookmarks (HTML)",
            ImportFormat::RaindropCsv => "Raindrop.io CSV",
            ImportFormat::PocketCsv => "Pocket CSV",
            ImportFormat::GenericCsv => "CSV",
            ImportFormat::UrlList => "URL list",
        }
    }
}

/// Outcome of an import (or a dry-run preview).
#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub total: u32,
    /// Saves that did not exist in the vault yet.
    pub new: u32,
    /// Saves whose URL was already in the vault (merged, never overwritten).
    pub existing: u32,
    /// Entries with no usable http(s) URL.
    pub invalid: u32,
}

/// Sniff the format and parse. Never fails; unparseable content simply
/// yields zero items.
pub fn parse(content: &str) -> (ImportFormat, Vec<ImportItem>) {
    let lower = content.trim_start().to_ascii_lowercase();
    let looks_like_html = lower.starts_with("<!doctype netscape")
        || lower.starts_with("<!doctype html")
        || lower.contains("<dl")
        || (lower.contains("<a ") && lower.contains("href="));
    if looks_like_html {
        return (ImportFormat::NetscapeHtml, parse_netscape_html(content));
    }
    if let Some((format, items)) = parse_csv(content) {
        return (format, items);
    }
    (ImportFormat::UrlList, parse_url_list(content))
}

// ---------------------------------------------------------------- HTML --

/// Folder names that carry no meaning as tags.
const GENERIC_FOLDERS: &[&str] = &[
    "bookmarks",
    "bookmarks bar",
    "bookmarks menu",
    "bookmarks toolbar",
    "other bookmarks",
    "mobile bookmarks",
    "unsorted bookmarks",
    "favorites bar",
    "reading list",
    "imported",
    "unfiled",
];

/// Parse the Netscape bookmark format. The structure is:
/// `<H3>folder</H3> <DL> ...entries... </DL>` nested arbitrarily, with
/// entries as `<DT><A HREF="..." ADD_DATE="..." TAGS="...">title</A>`.
/// Pocket's HTML export is a flat `<ul>` of the same `<a>` shape (with
/// `time_added`), which this parser handles for free.
pub fn parse_netscape_html(content: &str) -> Vec<ImportItem> {
    let lower = content.to_ascii_lowercase();
    let mut items = Vec::new();
    let mut folders: Vec<String> = Vec::new();
    let mut pending_folder: Option<String> = None;
    let mut pos = 0;

    while pos < lower.len() {
        let h3 = find_from(&lower, "<h3", pos);
        let a = find_anchor(&lower, pos);
        let dl_open = find_from(&lower, "<dl", pos);
        let dl_close = find_from(&lower, "</dl", pos);

        let next = [h3, a, dl_open, dl_close]
            .into_iter()
            .flatten()
            .min();
        let Some(next) = next else { break };

        if Some(next) == dl_close {
            folders.pop();
            pos = next + 4;
        } else if Some(next) == dl_open {
            folders.push(pending_folder.take().unwrap_or_default());
            pos = next + 3;
        } else if Some(next) == h3 {
            let (text, end) = element_text(content, &lower, next, "</h3");
            pending_folder = Some(decode_entities(text.trim()));
            pos = end;
        } else {
            // <a ...>title</a>
            let Some(tag_end) = find_from(&lower, ">", next) else { break };
            let tag = &content[next..tag_end];
            let (text, end) = element_text(content, &lower, next, "</a");
            pos = end;

            let Some(url) = attr(tag, "href") else { continue };
            if !url.starts_with("http://") && !url.starts_with("https://") {
                continue;
            }
            let mut tags: Vec<String> = folders
                .iter()
                .filter(|f| !f.is_empty())
                .filter(|f| !GENERIC_FOLDERS.contains(&f.to_ascii_lowercase().as_str()))
                .cloned()
                .collect();
            if let Some(attr_tags) = attr(tag, "tags") {
                tags.extend(split_tags(&attr_tags));
            }
            let created_at = attr(tag, "add_date")
                .or_else(|| attr(tag, "time_added"))
                .and_then(|v| parse_epoch(&v));

            items.push(ImportItem {
                url,
                title: decode_entities(text.trim()),
                tags,
                created_at,
                ..Default::default()
            });
        }
    }
    items
}

/// Inner text of the element starting at `start`; returns (text, next_pos).
fn element_text<'a>(
    content: &'a str,
    lower: &str,
    start: usize,
    close_tag: &str,
) -> (&'a str, usize) {
    let Some(open_end) = find_from(lower, ">", start) else {
        return ("", lower.len());
    };
    match find_from(lower, close_tag, open_end) {
        Some(close) => (&content[open_end + 1..close], close + close_tag.len()),
        // Some exporters omit closing tags; take until the next element.
        None => {
            let text_end = find_from(lower, "<", open_end + 1).unwrap_or(lower.len());
            (&content[open_end + 1..text_end], text_end)
        }
    }
}

fn find_from(haystack: &str, needle: &str, from: usize) -> Option<usize> {
    haystack.get(from..)?.find(needle).map(|i| i + from)
}

/// Find `<a` followed by whitespace (so `<area>` etc. don't match).
fn find_anchor(lower: &str, from: usize) -> Option<usize> {
    let mut pos = from;
    while let Some(i) = find_from(lower, "<a", pos) {
        match lower.as_bytes().get(i + 2) {
            Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r') => return Some(i),
            _ => pos = i + 2,
        }
    }
    None
}

/// Extract an attribute value from a raw tag string, case-insensitively.
/// Also used by the monitor to pull og:image metadata from checked pages.
pub(crate) fn attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let pat = format!("{name}=");
    let mut from = 0;
    while let Some(i) = find_from(&lower, &pat, from) {
        // Must be a standalone attribute name (preceded by whitespace).
        let ok = i > 0 && lower.as_bytes()[i - 1].is_ascii_whitespace();
        if !ok {
            from = i + pat.len();
            continue;
        }
        let rest = &tag[i + pat.len()..];
        let value = match rest.as_bytes().first() {
            Some(&q @ (b'"' | b'\'')) => rest[1..].split(q as char).next().unwrap_or(""),
            _ => rest
                .split(|c: char| c.is_ascii_whitespace() || c == '>')
                .next()
                .unwrap_or(""),
        };
        return Some(decode_entities(value));
    }
    None
}

pub(crate) fn decode_entities(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#039;", "'")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
}

// ----------------------------------------------------------------- CSV --

fn parse_csv(content: &str) -> Option<(ImportFormat, Vec<ImportItem>)> {
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(content.as_bytes());
    let headers: Vec<String> = reader
        .headers()
        .ok()?
        .iter()
        .map(|h| h.trim().to_ascii_lowercase())
        .collect();
    let col = |name: &str| headers.iter().position(|h| h == name);

    let url_col = col("url")?;
    let format = if col("time_added").is_some() {
        ImportFormat::PocketCsv
    } else if col("folder").is_some() || col("excerpt").is_some() || col("cover").is_some() {
        ImportFormat::RaindropCsv
    } else {
        ImportFormat::GenericCsv
    };

    let title_col = col("title");
    let tags_col = col("tags");
    let note_col = col("note").or_else(|| col("notes"));
    let excerpt_col = col("excerpt").or_else(|| col("description"));
    let folder_col = col("folder");
    let favorite_col = col("favorite");
    let created_col = col("created")
        .or_else(|| col("created_at"))
        .or_else(|| col("time_added"))
        .or_else(|| col("add_date"));

    let mut items = Vec::new();
    for record in reader.records().flatten() {
        let field = |i: Option<usize>| {
            i.and_then(|i| record.get(i)).unwrap_or("").trim().to_string()
        };
        let url = field(Some(url_col));
        if url.is_empty() {
            continue;
        }
        let mut tags = split_tags(&field(tags_col));
        let folder = field(folder_col);
        if !folder.is_empty()
            && !GENERIC_FOLDERS.contains(&folder.to_ascii_lowercase().as_str())
        {
            tags.push(folder);
        }
        items.push(ImportItem {
            url,
            title: field(title_col),
            description: field(excerpt_col),
            notes: field(note_col),
            tags,
            favorite: field(favorite_col).eq_ignore_ascii_case("true"),
            created_at: parse_timestamp(&field(created_col)),
        });
    }
    Some((format, items))
}

/// Tags from CSV/HTML attributes: comma-separated (Raindrop, Firefox) or
/// pipe-separated (Pocket).
fn split_tags(raw: &str) -> Vec<String> {
    raw.split([',', '|'])
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(String::from)
        .collect()
}

// ------------------------------------------------------------ URL list --

/// One URL per line; markdown links and list bullets allowed.
pub fn parse_url_list(content: &str) -> Vec<ImportItem> {
    let mut items = Vec::new();
    for line in content.lines() {
        let line = line
            .trim()
            .trim_start_matches(['-', '*', '>'])
            .trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Markdown link: [title](url)
        if let Some(rest) = line.strip_prefix('[') {
            if let Some((title, after)) = rest.split_once("](") {
                if let Some(url) = after.split([')', ' ']).next() {
                    if url.starts_with("http") {
                        items.push(ImportItem {
                            url: url.to_string(),
                            title: title.trim().to_string(),
                            ..Default::default()
                        });
                        continue;
                    }
                }
            }
        }
        if let Some(url) = line
            .split_whitespace()
            .find(|t| t.starts_with("http://") || t.starts_with("https://"))
        {
            items.push(ImportItem {
                url: url.to_string(),
                ..Default::default()
            });
        }
    }
    items
}

// --------------------------------------------------------- timestamps --

/// Epoch seconds from a numeric string, tolerating ms/µs precision.
fn parse_epoch(raw: &str) -> Option<i64> {
    let n: i64 = raw.trim().parse().ok()?;
    let secs = match n {
        n if n > 20_000_000_000_000 => n / 1_000_000, // microseconds
        n if n > 20_000_000_000 => n / 1_000,         // milliseconds
        n => n,
    };
    // Sanity window: 1990..2100.
    (631_152_000..4_102_444_800).contains(&secs).then_some(secs)
}

/// Epoch seconds from either a numeric epoch or an ISO-8601 date
/// (`2020-05-01T10:30:00.000Z` / `2020-05-01`). UTC assumed; good enough
/// for "when did I save this".
fn parse_timestamp(raw: &str) -> Option<i64> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if raw.chars().all(|c| c.is_ascii_digit()) {
        return parse_epoch(raw);
    }
    let date = &raw[..raw.len().min(10)];
    let mut parts = date.split('-');
    let y: i64 = parts.next()?.parse().ok()?;
    let m: u32 = parts.next()?.parse().ok()?;
    let d: u32 = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let mut secs = days_from_civil(y, m, d) * 86_400;
    if let Some(time) = raw.get(11..19) {
        let mut t = time.split(':');
        let h: i64 = t.next().and_then(|v| v.parse().ok()).unwrap_or(0);
        let min: i64 = t.next().and_then(|v| v.parse().ok()).unwrap_or(0);
        let s: i64 = t.next().and_then(|v| v.parse().ok()).unwrap_or(0);
        secs += h * 3600 + min * 60 + s;
    }
    (631_152_000..4_102_444_800).contains(&secs).then_some(secs)
}

/// Days since 1970-01-01 (Howard Hinnant's civil-days algorithm).
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps() {
        assert_eq!(parse_epoch("1588327800"), Some(1_588_327_800));
        assert_eq!(parse_epoch("1588327800000"), Some(1_588_327_800)); // ms
        assert_eq!(parse_epoch("99"), None);
        assert_eq!(
            parse_timestamp("2020-05-01T10:10:00.000Z"),
            Some(1_588_327_800)
        );
        assert_eq!(parse_timestamp("2020-05-01"), Some(1_588_291_200));
        assert_eq!(parse_timestamp("not a date"), None);
    }

    #[test]
    fn attrs_and_entities() {
        let tag = r#"<A HREF="https://e.com/?a=1&amp;b=2" ADD_DATE="1588327800" TAGS="rust,web">"#;
        assert_eq!(attr(tag, "href").as_deref(), Some("https://e.com/?a=1&b=2"));
        assert_eq!(attr(tag, "add_date").as_deref(), Some("1588327800"));
        assert_eq!(attr(tag, "missing"), None);
    }
}
