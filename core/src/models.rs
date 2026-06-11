use serde::{Deserialize, Serialize};

/// Health of a saved link, maintained by the monitoring service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkStatus {
    /// Never checked yet.
    Unchecked,
    /// Reachable and unchanged since the last check.
    Active,
    /// Reachable, but the content hash differs from the previous check.
    Changed,
    /// The URL now resolves to a meaningfully different location.
    Redirected,
    /// Unreachable: DNS/transport failure, 404/410, or server error.
    Dead,
}

impl LinkStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            LinkStatus::Unchecked => "unchecked",
            LinkStatus::Active => "active",
            LinkStatus::Changed => "changed",
            LinkStatus::Redirected => "redirected",
            LinkStatus::Dead => "dead",
        }
    }

    pub fn parse(s: &str) -> LinkStatus {
        match s {
            "active" => LinkStatus::Active,
            "changed" => LinkStatus::Changed,
            "redirected" => LinkStatus::Redirected,
            "dead" => LinkStatus::Dead,
            _ => LinkStatus::Unchecked,
        }
    }
}

/// A saved web reference with its full metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Save {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub description: String,
    pub notes: String,
    pub favicon_url: String,
    pub favorite: bool,
    pub status: LinkStatus,
    /// Final URL after redirects, when it differs from `url`.
    pub redirect_url: String,
    /// Last observed HTTP status code, if any.
    pub http_status: Option<i64>,
    pub tags: Vec<String>,
    /// Unix epoch seconds.
    pub created_at: i64,
    pub updated_at: i64,
    pub last_checked_at: Option<i64>,
}

/// Input for capturing a page (from the extension, UI, or any other client).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NewSave {
    pub url: String,
    pub title: String,
    pub description: String,
    pub favicon_url: String,
    pub tags: Vec<String>,
}

/// Partial update of user-editable metadata. `None` fields are left untouched.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SavePatch {
    pub title: Option<String>,
    pub description: Option<String>,
    pub notes: Option<String>,
    pub favicon_url: Option<String>,
}

/// Filters for listing/searching saves. All filters combine with AND.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ListQuery {
    /// Full-text query over title, url, description, notes and tags.
    pub query: Option<String>,
    pub tag: Option<String>,
    pub favorites_only: bool,
    pub status: Option<LinkStatus>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagCount {
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultStats {
    pub total: i64,
    pub favorites: i64,
    pub unchecked: i64,
    pub active: i64,
    pub changed: i64,
    pub redirected: i64,
    pub dead: i64,
}
