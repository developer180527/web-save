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
    pub unchecked: i64,
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

/// A tag and how many saves carry it, for the sidebar.
pub struct TagCountView {
    pub name: String,
    pub count: i64,
}

/// Saves matching the sidebar's filters, newest first.
///
/// Every filter combines with AND, and all of them are applied in SQL rather
/// than by filtering a full list in Dart — on a 10k-save vault the difference
/// is a windowed index scan versus shipping every row across the bridge.
pub fn list_saves(
    query: Option<String>,
    tag: Option<String>,
    status: Option<String>,
    favorites_only: bool,
    unread_only: bool,
    limit: Option<i64>,
) -> Result<Vec<SaveView>, String> {
    let saves = vault()?
        .list_saves(&ListQuery {
            query: query.filter(|q| !q.trim().is_empty()),
            tag: tag.filter(|t| !t.is_empty()),
            status: status
                .filter(|s| !s.is_empty())
                .map(|s| websave_core::LinkStatus::parse(&s)),
            favorites_only,
            unread_only,
            limit,
            offset: None,
        })
        .map_err(|e| e.to_string())?;
    Ok(saves.into_iter().map(SaveView::from).collect())
}

/// Every tag in use, with counts, most-used first.
pub fn list_tags() -> Result<Vec<TagCountView>, String> {
    Ok(vault()?
        .list_tags()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|t| TagCountView {
            name: t.name,
            count: t.count,
        })
        .collect())
}

pub fn vault_stats() -> Result<StatsView, String> {
    let s = vault()?.stats().map_err(|e| e.to_string())?;
    Ok(StatsView {
        total: s.total,
        favorites: s.favorites,
        unread: s.unread,
        unchecked: s.unchecked,
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

// ---- writes ----
//
// Every mutation returns the save as core now holds it, so the UI can replace
// one row in place instead of refetching the list. Core stays the single
// source of truth and Dart never has to guess what a write did.

pub fn set_favorite(id: i64, favorite: bool) -> Result<SaveView, String> {
    vault()?
        .set_favorite(id, favorite)
        .map(SaveView::from)
        .map_err(|e| e.to_string())
}

pub fn set_read(id: i64, is_read: bool) -> Result<SaveView, String> {
    vault()?
        .set_read(id, is_read)
        .map(SaveView::from)
        .map_err(|e| e.to_string())
}

pub fn set_tags(id: i64, tags: Vec<String>) -> Result<SaveView, String> {
    vault()?
        .set_tags(id, &tags)
        .map(SaveView::from)
        .map_err(|e| e.to_string())
}

/// Update the user-editable metadata. `None` leaves a field alone.
pub fn update_save(
    id: i64,
    title: Option<String>,
    description: Option<String>,
    notes: Option<String>,
) -> Result<SaveView, String> {
    vault()?
        .update_save(
            id,
            websave_core::SavePatch {
                title,
                description,
                notes,
                favicon_url: None,
            },
        )
        .map(SaveView::from)
        .map_err(|e| e.to_string())
}

pub fn delete_save(id: i64) -> Result<(), String> {
    vault()?.delete_save(id).map_err(|e| e.to_string())
}

/// Check one link right now and return its fresh state.
///
/// Network-bound and can take seconds — flutter_rust_bridge runs it on a
/// worker thread, so the Dart isolate keeps painting, but the caller still
/// wants a pending state on the button.
pub fn check_save_now(id: i64) -> Result<SaveView, String> {
    vault()?
        .check_save(id)
        .map(SaveView::from)
        .map_err(|e| e.to_string())
}

/// The readable-text snapshot the monitor archived, if there is one.
pub fn archive_text(id: i64) -> Result<Option<String>, String> {
    vault()?.archive_text(id).map_err(|e| e.to_string())
}

// ---- import / export ----
//
// The whole point of a portable vault: get your data out on one machine and
// back in on another. On iOS this is the *only* way in until a Share
// Extension exists, since nothing on a phone can reach the capture server.

pub struct ImportPreview {
    /// Human-readable detected format, e.g. "Browser bookmarks (HTML)".
    pub format: String,
    pub total: u32,
    /// URLs not already in the vault.
    pub new_items: u32,
    /// Already present. Importing merges metadata, never overwrites.
    pub existing: u32,
    /// Entries with no usable http(s) URL.
    pub invalid: u32,
}

fn describe_format(format: websave_core::ImportFormat) -> String {
    format.label().to_string()
}

/// What importing this file *would* do, without touching the vault.
///
/// Separate from [`run_import`] so the UI can show a real diff — new versus
/// already-here versus unusable — before anyone commits to it. Core's parser
/// never fails; unreadable content simply yields zero items.
pub fn preview_import(content: String) -> Result<ImportPreview, String> {
    let (format, items) = websave_core::import::parse(&content);
    let report = vault()?.preview_import(&items).map_err(|e| e.to_string())?;
    Ok(ImportPreview {
        format: describe_format(format),
        total: report.total,
        new_items: report.new,
        existing: report.existing,
        invalid: report.invalid,
    })
}

/// Actually import. Existing URLs are merged, never overwritten, so importing
/// the same file twice is safe.
pub fn run_import(content: String) -> Result<ImportPreview, String> {
    let (format, items) = websave_core::import::parse(&content);
    let report = vault()?.import_items(&items).map_err(|e| e.to_string())?;
    Ok(ImportPreview {
        format: describe_format(format),
        total: report.total,
        new_items: report.new,
        existing: report.existing,
        invalid: report.invalid,
    })
}

/// Serialize the whole vault.
///
/// `"html"` is the round-trip format: Netscape bookmarks, which both any
/// browser *and* this app's own importer read. `"json"` carries more fields
/// (status, timestamps, archive stamps) but the importer does not parse it —
/// it is for backup and for other tools, not for moving a vault between
/// devices. Anything that is not "json" gives HTML.
pub fn export_vault(format: String) -> Result<String, String> {
    let saves = vault()?.all_saves().map_err(|e| e.to_string())?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    Ok(match format.as_str() {
        "json" => websave_core::export::to_json(&saves, now),
        _ => websave_core::export::to_netscape_html(&saves),
    })
}
