//! The Dart-facing vault API.
//!
//! Phase 0 of the Flutter migration: deliberately just enough to prove the
//! pipe end to end — open a real vault, read real saves, render them. The
//! remaining ~38 operations follow the same shape once this is trusted.
//!
//! Note what *isn't* here. Under Tauri every one of these round-trips is a
//! JSON string across the webview bridge; here Dart calls Rust directly, so
//! there is no serialization layer to write and none to keep in sync.

use std::sync::{Arc, OnceLock};

use flutter_rust_bridge::frb;
use websave_core::{ListQuery, Vault};

/// The open vault, shared by every call.
///
/// A `OnceLock` rather than a Dart-held handle because the vault is a
/// process-wide singleton — SQLite in WAL mode, one connection behind a
/// mutex — and handing Dart an opaque pointer to it would only add a way to
/// get it wrong.
static VAULT: OnceLock<Arc<Vault>> = OnceLock::new();

/// A save, flattened for Dart.
///
/// Mirrors [`websave_core::Save`] rather than reusing it: the Dart side wants
/// `status` as a plain string it can switch on, and keeping a separate type
/// means core's model can change shape without silently altering the Dart
/// API.
pub struct SaveView {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub description: String,
    pub favicon_url: String,
    /// Path relative to the vault's assets dir; empty when there is none.
    pub thumbnail: String,
    pub favorite: bool,
    pub is_read: bool,
    /// One of: unchecked, active, changed, redirected, dead.
    pub status: String,
    pub http_status: Option<i64>,
    pub tags: Vec<String>,
    pub created_at: i64,
    pub last_checked_at: Option<i64>,
}

impl From<websave_core::Save> for SaveView {
    fn from(s: websave_core::Save) -> SaveView {
        SaveView {
            id: s.id,
            url: s.url,
            title: s.title,
            description: s.description,
            favicon_url: s.favicon_url,
            thumbnail: s.thumbnail,
            favorite: s.favorite,
            is_read: s.is_read,
            status: s.status.as_str().to_string(),
            http_status: s.http_status,
            tags: s.tags,
            created_at: s.created_at,
            last_checked_at: s.last_checked_at,
        }
    }
}

/// Vault-wide counts, for the sidebar.
pub struct StatsView {
    pub total: i64,
    pub favorites: i64,
    pub unread: i64,
    pub active: i64,
    pub changed: i64,
    pub redirected: i64,
    pub dead: i64,
}

/// Open (or create) the vault at `path`. Call once at startup.
///
/// Safe to call against a directory the Tauri app also has open — SQLite runs
/// in WAL mode and the existing macOS menubar app already relies on exactly
/// that, which is what lets the two frontends run side by side during the
/// migration.
pub fn open_vault(path: String) -> Result<String, String> {
    let vault = Vault::open(&path).map_err(|e| e.to_string())?;
    let root = vault.root().display().to_string();
    // A second call is a no-op rather than an error: hot restart in Flutter
    // re-runs main() without tearing down the Rust side.
    let _ = VAULT.set(Arc::new(vault));
    Ok(root)
}

fn vault() -> Result<&'static Arc<Vault>, String> {
    VAULT.get().ok_or_else(|| "vault not opened".to_string())
}

/// Saves matching an optional full-text query, newest first.
pub fn list_saves(query: Option<String>, limit: Option<i64>) -> Result<Vec<SaveView>, String> {
    let saves = vault()?
        .list_saves(&ListQuery {
            query: query.filter(|q| !q.trim().is_empty()),
            limit,
            ..Default::default()
        })
        .map_err(|e| e.to_string())?;
    Ok(saves.into_iter().map(SaveView::from).collect())
}

pub fn vault_stats() -> Result<StatsView, String> {
    let s = vault()?.stats().map_err(|e| e.to_string())?;
    Ok(StatsView {
        total: s.total,
        favorites: s.favorites,
        unread: s.unread,
        active: s.active,
        changed: s.changed,
        redirected: s.redirected,
        dead: s.dead,
    })
}

/// Absolute path of the assets directory, so Dart can resolve the relative
/// `thumbnail` paths into files it can display.
pub fn assets_dir() -> Result<String, String> {
    Ok(vault()?.assets_dir().display().to_string())
}

#[frb(init)]
pub fn init_app() {
    flutter_rust_bridge::setup_default_user_utils();
}
