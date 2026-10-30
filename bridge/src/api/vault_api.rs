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
    /// One of: unchecked, active, changed, redirected, unreachable, dead.
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
    /// Couldn't be reached from here — a site that's down, or a
    /// Tailscale/VPN/LAN address we're not currently on the network for.
    /// Never auto-deleted.
    pub unreachable: i64,
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

/// The open vault for other bridge modules that need to own a handle, like
/// the capture server's worker thread.
pub(crate) fn shared_vault() -> Result<Arc<Vault>, String> {
    vault().cloned()
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
        unreachable: s.unreachable,
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
    let vault = vault()?;
    // Recorded because this is the one action in the app with no undo. The
    // URL is captured before the row goes, since afterwards there is nothing
    // left to name it by.
    let url = vault.get_save(id).ok().map(|s| s.url);
    vault.delete_save(id).map_err(|e| e.to_string())?;
    vault.log_activity(
        "warn",
        "app",
        &format!("Deleted {}", url.unwrap_or_else(|| format!("save {id}"))),
    );
    Ok(())
}

/// Check one link right now and return its fresh state.
///
/// Network-bound and can take seconds — flutter_rust_bridge runs it on a
/// worker thread, so the Dart isolate keeps painting, but the caller still
/// wants a pending state on the button.
pub fn check_save_now(id: i64) -> Result<SaveView, String> {
    let vault = vault()?;
    let before = vault.get_save(id).ok().map(|s| s.status);
    let after = vault.check_save(id).map_err(|e| e.to_string())?;

    // Only when the verdict moved. Logging every manual check would bury the
    // events worth reading under a line per button press.
    if before != Some(after.status) {
        vault.log_activity(
            if matches!(after.status, websave_core::LinkStatus::Dead) {
                "warn"
            } else {
                "info"
            },
            "monitor",
            &format!("{} is now {:?}", after.url, after.status),
        );
    }
    Ok(SaveView::from(after))
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
    let vault = vault()?;
    let report = vault.import_items(&items).map_err(|e| e.to_string())?;
    vault.log_activity(
        "info",
        "import",
        &format!(
            "Imported {} new save(s) from {} ({} already present, {} unusable)",
            report.new,
            describe_format(format),
            report.existing,
            report.invalid
        ),
    );
    Ok(ImportPreview {
        format: describe_format(format),
        total: report.total,
        new_items: report.new,
        existing: report.existing,
        invalid: report.invalid,
    })
}

/// An export, plus what it left out — so the UI can say so rather than
/// silently writing a smaller file than the user expected.
pub struct ExportResult {
    /// The serialized vault, ready to write to disk.
    pub content: String,
    /// Saves written to the file.
    pub exported: u32,
    /// Dead links omitted (see [`websave_core::export::exportable`]).
    pub skipped_dead: u32,
}

/// What an export would contain right now, without serializing anything.
///
/// Lets the transfer screen tell the user that dead links will be left out
/// *before* they pick a destination, instead of explaining it afterwards.
pub fn export_preview() -> Result<ExportResult, String> {
    let vault = vault()?;
    let saves = vault.all_saves().map_err(|e| e.to_string())?;
    let (_, summary) = websave_core::export::exportable(&saves);
    Ok(ExportResult {
        content: String::new(),
        exported: summary.exported as u32,
        skipped_dead: summary.skipped_dead as u32,
    })
}

/// Serialize the vault.
///
/// `"json"` keeps the most detail and imports back in, so it is the format
/// for moving a vault between devices. `"html"` is Netscape bookmarks, which
/// any browser reads as well as this app's own importer, and is the one to
/// hand to something that is not WebSave. Anything not "json" gives HTML.
///
/// Links a server confirmed gone are excluded; the count comes back in the
/// result so the UI can report it.
pub fn export_vault(format: String) -> Result<ExportResult, String> {
    let all = vault()?.all_saves().map_err(|e| e.to_string())?;
    let (saves, summary) = websave_core::export::exportable(&all);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let content = match format.as_str() {
        "json" => websave_core::export::to_json(&saves, now),
        _ => websave_core::export::to_netscape_html(&saves),
    };
    Ok(ExportResult {
        content,
        exported: summary.exported as u32,
        skipped_dead: summary.skipped_dead as u32,
    })
}

// ---- launch at login ----
//
// Deliberately in Rust rather than Dart. The Tauri frontend already registers
// a login item through tauri-plugin-autostart, which wraps this same
// auto-launch crate, so sharing the crate means both frontends write the same
// kind of entry in the same place instead of two mechanisms disagreeing about
// what "enabled" means. On macOS that is a LaunchAgent plist, on Windows a Run
// key, on Linux a .desktop file in ~/.config/autostart.

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn launcher() -> Result<auto_launch::AutoLaunch, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;

    // On macOS the executable lives inside the bundle, but a login item has to
    // point at the .app — registering the inner binary launches a process with
    // no dock entry, no menu bar and no window server connection.
    #[cfg(target_os = "macos")]
    let target = exe
        .ancestors()
        .find(|p| p.extension().is_some_and(|e| e == "app"))
        .map(|p| p.to_path_buf())
        .unwrap_or(exe);
    #[cfg(not(target_os = "macos"))]
    let target = exe;

    let mut builder = auto_launch::AutoLaunchBuilder::new();
    builder
        .set_app_name("WebSave")
        .set_app_path(&target.to_string_lossy());

    // SMAppService, not the crate's LaunchAgent default.
    //
    // macOS 13 split these in two. A LaunchAgent plist registers a *background
    // item*: it lands under Settings > General > Login Items > "Allow in the
    // Background" and triggers the "added items that can run in the
    // background" notification. That is right for the Tauri build, whose
    // menubar companion is a daemon, and wrong here — this is a windowed app
    // the user wants reopened at login, which is SMAppService's MainApp
    // registration and shows under "Open at Login" instead.
    #[cfg(target_os = "macos")]
    builder.set_macos_launch_mode(auto_launch::MacOSLaunchMode::SMAppService);

    builder.build().map_err(|e| e.to_string())
}

/// Whether this app is registered to start at login.
///
/// Reports what is actually on disk rather than a stored preference, so an
/// entry removed in System Settings shows here as off.
pub fn launch_at_login() -> Result<bool, String> {
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        launcher()?.is_enabled().map_err(|e| e.to_string())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Ok(false)
    }
}

/// Register or remove the login item.
///
/// Returns the state read back afterwards rather than the state asked for:
/// registration can fail quietly under sandboxing or a managed profile, and a
/// switch that flips regardless is worse than one that refuses to move.
pub fn set_launch_at_login(enabled: bool) -> Result<bool, String> {
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        let launcher = launcher()?;
        if enabled {
            launcher.enable().map_err(|e| e.to_string())?;
        } else {
            launcher.disable().map_err(|e| e.to_string())?;
        }
        launcher.is_enabled().map_err(|e| e.to_string())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = enabled;
        Ok(false)
    }
}

/// Whether a login item can be offered at all. False on iOS and Android, where
/// the concept does not exist.
pub fn supports_launch_at_login() -> bool {
    cfg!(any(target_os = "macos", target_os = "windows", target_os = "linux"))
}

// ---- browser extension ----

pub struct ExtensionStatus {
    /// Last time the extension called the capture server, as a unix
    /// timestamp. None if it has never connected.
    pub last_seen: Option<i64>,
    /// Version it reported on that call.
    pub version: Option<String>,
    /// Whether that was recent enough to call it connected.
    pub connected: bool,
}

/// What the vault knows about the browser extension.
///
/// Read from vault meta rather than from a live socket: whichever app is
/// serving the extension (this one, or the legacy Tauri app when it holds
/// the port) records `ext.last_seen` and `ext.version` on every call. That
/// makes this honest about what it is, a record of the last contact rather
/// than a live connection check, and right whichever app took the call.
pub fn extension_status() -> Result<ExtensionStatus, String> {
    let vault = vault()?;
    let last_seen = vault
        .get_meta("ext.last_seen")
        .map_err(|e| e.to_string())?
        .and_then(|v| v.parse::<i64>().ok());
    let version = vault.get_meta("ext.version").map_err(|e| e.to_string())?;

    // Twelve hours. The extension only calls in when the user saves something,
    // so anything shorter would report a working setup as disconnected over a
    // quiet afternoon.
    const RECENT: i64 = 12 * 60 * 60;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    Ok(ExtensionStatus {
        connected: last_seen.is_some_and(|seen| now - seen < RECENT),
        last_seen,
        version,
    })
}

// ---- adding by hand ----

/// Add a save from a URL typed or pasted into the app.
///
/// Until now every save arrived through the browser extension, which means
/// iOS — where there is no extension and no capture server — could only ever
/// look at a vault it had no way to add to.
///
/// The title is left empty when not given rather than guessed at: the link
/// checker fills in title, description and favicon on its first pass, and a
/// placeholder written here would be mistaken for real metadata and never
/// replaced.
pub fn add_save_by_url(url: String, title: Option<String>) -> Result<SaveView, String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err("Enter a link to save.".into());
    }

    // Accept a bare host the way a browser bar does. Typing "example.com" and
    // being told it is not a link is a worse answer than assuming https.
    let normalized = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };

    let parsed = url::Url::parse(&normalized).map_err(|_| "That is not a link.".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("Only http and https links can be saved.".into());
    }

    let vault = vault()?;
    let save = vault
        .add_save(websave_core::NewSave {
            url: parsed.to_string(),
            title: title.unwrap_or_default(),
            description: String::new(),
            favicon_url: String::new(),
            tags: Vec::new(),
        })
        .map_err(|e| e.to_string())?;
    vault.log_activity("info", "app", &format!("Added {}", save.url));
    Ok(SaveView::from(save))
}

/// Whether a string looks like something worth offering to save.
///
/// Used against the clipboard on resume. Deliberately strict — a bare host is
/// accepted when typed, because typing is an explicit act, but suggesting a
/// save for every "example.com" that passes through the clipboard would be
/// noise.
pub fn looks_like_url(text: String) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.len() > 2048 || trimmed.split_whitespace().count() > 1 {
        return false;
    }
    url::Url::parse(trimmed).is_ok_and(|u| matches!(u.scheme(), "http" | "https"))
}

// ---- activity log ----

pub struct ActivityEntryView {
    pub at: i64,
    pub level: String,
    pub source: String,
    pub message: String,
}

/// Recent activity, newest first.
///
/// Read from the vault rather than from an in-memory buffer so it survives a
/// restart and — the point of it — so work done by a process with no Flutter
/// engine, like the iOS background sweep, is visible here at all.
pub fn recent_activity(limit: i64) -> Result<Vec<ActivityEntryView>, String> {
    Ok(vault()?
        .recent_activity(limit)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|e| ActivityEntryView {
            at: e.at,
            level: e.level,
            source: e.source,
            message: e.message,
        })
        .collect())
}

/// Append an entry from the Dart side.
pub fn log_activity(level: String, source: String, message: String) -> Result<(), String> {
    vault()?.log_activity(&level, &source, &message);
    Ok(())
}

pub fn clear_activity() -> Result<(), String> {
    vault()?.clear_activity().map_err(|e| e.to_string())
}
