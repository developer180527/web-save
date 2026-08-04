//! Vault export.
//!
//! Two formats, on purpose:
//! * **JSON** — full fidelity (every field, including notes, status and
//!   timestamps). This is the backup format.
//! * **Netscape HTML** — the universal bookmark format, so the export can be
//!   read by any browser *and* by our own importer (round-trip).

use crate::models::{LinkStatus, Save};

/// What an export contains, and what it deliberately leaves behind.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExportSummary {
    pub exported: usize,
    /// Links a working server said are gone (404/410), which are omitted.
    pub skipped_dead: usize,
}

/// The saves that belong in an export, and a count of what was dropped.
///
/// Dead links are excluded. An export is either a backup or a hand-off to
/// another tool, and neither is served by carrying addresses that are known
/// not to resolve — re-importing a backup would resurrect rot the vault had
/// already identified.
///
/// Only [`LinkStatus::Dead`] is dropped, and that status now requires a
/// working server to have answered 404/410. Anything merely
/// [`LinkStatus::Unreachable`] — a site that was down, or a Tailscale/VPN/LAN
/// address unreachable from wherever the check ran — is kept, because that
/// verdict describes our vantage point rather than the resource. Excluding
/// those would quietly drop a user's private bookmarks from their own backup.
pub fn exportable(saves: &[Save]) -> (Vec<Save>, ExportSummary) {
    let (keep, dropped): (Vec<Save>, Vec<Save>) = saves
        .iter()
        .cloned()
        .partition(|s| s.status != LinkStatus::Dead);
    let summary = ExportSummary {
        exported: keep.len(),
        skipped_dead: dropped.len(),
    };
    (keep, summary)
}

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

#[cfg(test)]
mod export_filter_tests {
    use super::*;
    use crate::models::LinkStatus;

    fn save_with(id: i64, status: LinkStatus) -> Save {
        Save {
            id,
            url: format!("https://example.com/{id}"),
            status,
            ..Default::default()
        }
    }

    #[test]
    fn dead_links_are_excluded_but_unreachable_ones_are_kept() {
        let saves = vec![
            save_with(1, LinkStatus::Active),
            save_with(2, LinkStatus::Dead),
            // A private host we couldn't route to must stay in the backup:
            // dropping it would lose the user's own data over a verdict that
            // describes the network, not the link.
            save_with(3, LinkStatus::Unreachable),
            save_with(4, LinkStatus::Changed),
            save_with(5, LinkStatus::Unchecked),
        ];

        let (kept, summary) = exportable(&saves);

        assert_eq!(summary.exported, 4);
        assert_eq!(summary.skipped_dead, 1);
        assert!(
            !kept.iter().any(|s| s.status == LinkStatus::Dead),
            "no dead link reaches the file"
        );
        assert!(kept.iter().any(|s| s.status == LinkStatus::Unreachable));
    }

    #[test]
    fn an_empty_vault_exports_cleanly() {
        let (kept, summary) = exportable(&[]);
        assert!(kept.is_empty());
        assert_eq!(summary, ExportSummary::default());
        // And the serializers must still produce valid, parseable output.
        assert!(to_json(&kept, 0).contains("\"saves\""));
        assert!(to_netscape_html(&kept).contains("</DL>"));
    }
}
