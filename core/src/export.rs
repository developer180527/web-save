//! Vault export.
//!
//! Two formats, on purpose:
//! * **JSON** — full fidelity (every field, including notes, status and
//!   timestamps). This is the backup format.
//! * **Netscape HTML** — the universal bookmark format, so the export can be
//!   read by any browser *and* by our own importer (round-trip).

use crate::models::Save;

/// Full-fidelity JSON backup: `{ "app", "version", "exportedAt", "saves": [...] }`.
pub fn to_json(saves: &[Save], exported_at: i64) -> String {
    let payload = serde_json::json!({
        "app": "websave",
        "format": "websave-export-v1",
        "exportedAt": exported_at,
        "count": saves.len(),
        "saves": saves,
    });
    serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string())
}

/// Netscape bookmark HTML — importable by every browser and by our own
/// importer. Tags become the `TAGS` attribute (Firefox convention).
pub fn to_netscape_html(saves: &[Save]) -> String {
    let mut out = String::with_capacity(saves.len() * 160 + 256);
    out.push_str(
        "<!DOCTYPE NETSCAPE-Bookmark-file-1>\n\
         <META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">\n\
         <TITLE>Bookmarks</TITLE>\n\
         <H1>WebSave</H1>\n\
         <DL><p>\n",
    );
    for save in saves {
        out.push_str("    <DT><A HREF=\"");
        out.push_str(&escape(&save.url));
        out.push_str("\" ADD_DATE=\"");
        out.push_str(&save.created_at.to_string());
        if !save.tags.is_empty() {
            out.push_str("\" TAGS=\"");
            out.push_str(&escape(&save.tags.join(",")));
        }
        out.push_str("\">");
        let label = if save.title.trim().is_empty() {
            &save.url
        } else {
            &save.title
        };
        out.push_str(&escape(label));
        out.push_str("</A>\n");
        // Descriptions live in <DD>, which the Netscape format allows.
        if !save.description.trim().is_empty() {
            out.push_str("    <DD>");
            out.push_str(&escape(&save.description));
            out.push('\n');
        }
    }
    out.push_str("</DL><p>\n");
    out
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::LinkStatus;

    fn sample() -> Save {
        Save {
            id: 1,
            url: "https://example.com/a?x=1&y=2".into(),
            title: "A <b>bold</b> title".into(),
            description: "Desc".into(),
            notes: "note".into(),
            favicon_url: String::new(),
            thumbnail: String::new(),
            favorite: true,
            is_read: false,
            status: LinkStatus::Active,
            redirect_url: String::new(),
            http_status: Some(200),
            tags: vec!["rust".into(), "web".into()],
            created_at: 1_700_000_000,
            updated_at: 1_700_000_000,
            last_checked_at: None,
            archived_at: None,
            dead_since: None,
        }
    }

    #[test]
    fn json_export_round_trips_fields() {
        let json = to_json(&[sample()], 123);
        assert!(json.contains("\"websave-export-v1\""));
        assert!(json.contains("\"exportedAt\": 123"));
        // camelCase field names, matching the app's own serialization.
        assert!(json.contains("\"createdAt\""));
        assert!(json.contains("\"rust\""));
    }

    #[test]
    fn html_export_escapes_and_carries_metadata() {
        let html = to_netscape_html(&[sample()]);
        assert!(html.starts_with("<!DOCTYPE NETSCAPE-Bookmark-file-1>"));
        assert!(html.contains("HREF=\"https://example.com/a?x=1&amp;y=2\""));
        assert!(html.contains("ADD_DATE=\"1700000000\""));
        assert!(html.contains("TAGS=\"rust,web\""));
        assert!(html.contains("A &lt;b&gt;bold&lt;/b&gt; title"));
        assert!(html.trim_end().ends_with("</DL><p>"));
    }

    #[test]
    fn html_export_is_readable_by_our_own_importer() {
        let html = to_netscape_html(&[sample()]);
        let (format, items) = crate::import::parse(&html);
        assert_eq!(format, crate::import::ImportFormat::NetscapeHtml);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].url, "https://example.com/a?x=1&y=2");
        assert_eq!(items[0].tags, vec!["rust", "web"]);
        assert_eq!(items[0].created_at, Some(1_700_000_000));
    }
}
