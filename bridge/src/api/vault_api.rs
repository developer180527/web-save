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

// ---- map ----

/// One dot on the map: a host, where it is served from, and how much of the
/// vault lives there.
pub struct MapPoint {
    pub host: String,
    pub ip: String,
    pub country: String,
    pub city: String,
    pub latitude: f64,
    pub longitude: f64,
    pub save_count: i64,
    /// The network announcing this address, e.g. `CLOUDFLARENET`. Empty when
    /// no ASN database is installed.
    pub provider: String,
    /// Whether that network is one whose addresses are anycast, meaning the
    /// coordinates say much less than the provider name does.
    pub anycast: bool,
}

pub struct MapData {
    pub points: Vec<MapPoint>,
    /// `(ISO country code, host count)`, most first. Populated even by a
    /// country-level database, which yields no coordinates to plot.
    pub countries: Vec<(String, i64)>,
    /// Hosts located at all, including ones with no coordinates. Lets the UI
    /// distinguish "no database yet" from "nothing resolved yet".
    pub located_hosts: i64,
    /// `(network, host count)`, busiest first. The truthful counterpart to
    /// the dots — see `websave_core::geo::asn_lookup`.
    pub providers: Vec<(String, i64)>,
    /// A description of the loaded database, or null when none is installed.
    pub database: Option<String>,
}

/// Everything the map view needs, in one call.
pub fn map_data() -> Result<MapData, String> {
    let vault = vault()?;
    let points = vault
        .map_points()
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter_map(|p| {
            Some(MapPoint {
                latitude: p.latitude?,
                longitude: p.longitude?,
                host: p.host,
                ip: p.ip,
                country: p.country,
                city: p.city,
                save_count: p.save_count,
                anycast: websave_core::geo::is_likely_anycast(&p.asn_org),
                provider: p.asn_org,
            })
        })
        .collect();
    Ok(MapData {
        points,
        countries: vault.hosts_by_country().map_err(|e| e.to_string())?,
        providers: vault.hosts_by_provider().map_err(|e| e.to_string())?,
        located_hosts: vault.located_host_count().map_err(|e| e.to_string())?,
        database: websave_core::GeoLocator::open_in(vault.assets_dir())
            .ok()
            .map(|l| l.describe()),
    })
}

// ---- preferences ----
//
// Host-defined keys in the vault's `meta` table. Core attaches no meaning to
// any of them, which is what lets the theme engine store its selection beside
// the user's data instead of in platform preferences.

pub fn get_pref(key: String) -> Result<Option<String>, String> {
    vault()?.get_meta(&key).map_err(|e| e.to_string())
}

pub fn set_pref(key: String, value: String) -> Result<(), String> {
    vault()?.set_meta(&key, &value).map_err(|e| e.to_string())
}
