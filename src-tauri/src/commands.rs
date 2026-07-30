use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;
use websave_core::{
    ListQuery, Monitor, MonitorConfig, MonitorEvent, NewSave, Save, SavePatch, SavedSearch,
    TagCount, Vault, VaultStats,
};

type VaultState<'a> = State<'a, Arc<Vault>>;

fn err(e: websave_core::Error) -> String {
    e.to_string()
}

#[tauri::command]
pub fn list_saves(vault: VaultState, query: ListQuery) -> Result<Vec<Save>, String> {
    vault.list_saves(&query).map_err(err)
}

#[tauri::command]
pub fn get_save(vault: VaultState, id: i64) -> Result<Save, String> {
    vault.get_save(id).map_err(err)
}

/// Mutations announce themselves so every surface (main window, quick
/// panel, tray menu) stays in sync regardless of where the change happened.
fn announce(app: &AppHandle) {
    let _ = app.emit("saves-updated", ());
}

#[tauri::command]
pub fn add_save(app: AppHandle, vault: VaultState, save: NewSave) -> Result<Save, String> {
    let save = vault.add_save(save).map_err(err)?;
    announce(&app);
    crate::wake_monitor(&app);
    Ok(save)
}

#[tauri::command]
pub fn update_save(
    app: AppHandle,
    vault: VaultState,
    id: i64,
    patch: SavePatch,
) -> Result<Save, String> {
    let save = vault.update_save(id, patch).map_err(err)?;
    announce(&app);
    Ok(save)
}

#[tauri::command]
pub fn set_favorite(
    app: AppHandle,
    vault: VaultState,
    id: i64,
    favorite: bool,
) -> Result<Save, String> {
    let save = vault.set_favorite(id, favorite).map_err(err)?;
    announce(&app);
    Ok(save)
}

#[tauri::command]
pub fn set_tags(
    app: AppHandle,
    vault: VaultState,
    id: i64,
    tags: Vec<String>,
) -> Result<Save, String> {
    let save = vault.set_tags(id, &tags).map_err(err)?;
    announce(&app);
    Ok(save)
}

#[tauri::command]
pub fn delete_save(app: AppHandle, vault: VaultState, id: i64) -> Result<(), String> {
    vault.delete_save(id).map_err(err)?;
    announce(&app);
    Ok(())
}

#[tauri::command]
pub fn set_read(app: AppHandle, vault: VaultState, id: i64, is_read: bool) -> Result<Save, String> {
    let save = vault.set_read(id, is_read).map_err(err)?;
    announce(&app);
    Ok(save)
}

#[tauri::command]
pub fn set_url(app: AppHandle, vault: VaultState, id: i64, url: String) -> Result<Save, String> {
    let save = vault.set_url(id, &url).map_err(err)?;
    announce(&app);
    crate::wake_monitor(&app);
    Ok(save)
}

// ---- bulk operations ----

#[tauri::command]
pub fn bulk_set_favorite(
    app: AppHandle,
    vault: VaultState,
    ids: Vec<i64>,
    favorite: bool,
) -> Result<(), String> {
    vault.set_favorite_many(&ids, favorite).map_err(err)?;
    announce(&app);
    Ok(())
}

#[tauri::command]
pub fn bulk_set_read(
    app: AppHandle,
    vault: VaultState,
    ids: Vec<i64>,
    is_read: bool,
) -> Result<(), String> {
    vault.set_read_many(&ids, is_read).map_err(err)?;
    announce(&app);
    Ok(())
}

#[tauri::command]
pub fn bulk_delete(app: AppHandle, vault: VaultState, ids: Vec<i64>) -> Result<(), String> {
    vault.delete_many(&ids).map_err(err)?;
    announce(&app);
    Ok(())
}

#[tauri::command]
pub fn bulk_add_tag(
    app: AppHandle,
    vault: VaultState,
    ids: Vec<i64>,
    tag: String,
) -> Result<(), String> {
    vault.add_tag_many(&ids, &tag).map_err(err)?;
    announce(&app);
    Ok(())
}

// ---- saved searches ----

#[tauri::command]
pub fn list_saved_searches(vault: VaultState) -> Result<Vec<SavedSearch>, String> {
    vault.list_saved_searches().map_err(err)
}

#[tauri::command]
pub fn add_saved_search(
    vault: VaultState,
    name: String,
    query: ListQuery,
) -> Result<SavedSearch, String> {
    vault.add_saved_search(&name, &query).map_err(err)
}

#[tauri::command]
pub fn delete_saved_search(vault: VaultState, id: i64) -> Result<(), String> {
    vault.delete_saved_search(id).map_err(err)
}

#[tauri::command]
pub fn list_tags(vault: VaultState) -> Result<Vec<TagCount>, String> {
    vault.list_tags().map_err(err)
}

/// The archived readable text of a save, if a snapshot exists.
#[tauri::command]
pub fn get_archive(vault: VaultState, id: i64) -> Result<Option<String>, String> {
    vault.archive_text(id).map_err(err)
}

#[tauri::command]
pub fn vault_stats(vault: VaultState) -> Result<VaultStats, String> {
    vault.stats().map_err(err)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionStatus {
    /// Unix seconds of the last capture received from the browser extension,
    /// or null if it has never checked in.
    last_seen: Option<i64>,
    version: Option<String>,
}

/// Whether the browser extension has ever delivered a capture (the only
/// signal the app has about the extension — it cannot inspect the browser).
#[tauri::command]
pub fn extension_status(vault: VaultState) -> Result<ExtensionStatus, String> {
    Ok(ExtensionStatus {
        last_seen: vault
            .get_meta("ext.last_seen")
            .map_err(err)?
            .and_then(|s| s.parse().ok()),
        version: vault.get_meta("ext.version").map_err(err)?,
    })
}

#[tauri::command]
pub fn vault_path(vault: VaultState) -> String {
    vault.root().display().to_string()
}

#[tauri::command]
pub fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

// ---- preferences (host-defined keys in the vault's meta store) ----

#[tauri::command]
pub fn get_pref(vault: VaultState, key: String) -> Result<Option<String>, String> {
    vault.get_meta(&key).map_err(err)
}

#[tauri::command]
pub fn set_pref(vault: VaultState, key: String, value: String) -> Result<(), String> {
    vault.set_meta(&key, &value).map_err(err)
}

// ---- trash ----

/// How long a dead link is kept before permanent deletion.
pub const TRASH_GRACE_SECS: i64 = 30 * 24 * 60 * 60;

/// Permanently delete links that stayed dead for the whole grace period.
#[tauri::command]
pub fn purge_trash(app: AppHandle, vault: VaultState) -> Result<u32, String> {
    let removed = vault.purge_expired_dead(TRASH_GRACE_SECS).map_err(err)?;
    if removed > 0 {
        announce(&app);
    }
    Ok(removed)
}

/// Empty the trash now, regardless of how long links have been dead.
#[tauri::command]
pub fn empty_trash(app: AppHandle, vault: VaultState) -> Result<u32, String> {
    let removed = vault.purge_expired_dead(-1).map_err(err)?;
    if removed > 0 {
        announce(&app);
    }
    Ok(removed)
}

// ---- export ----

/// Serialize the whole vault. `format` is "json" (full fidelity) or "html"
/// (Netscape bookmarks, readable by browsers and by our own importer).
#[tauri::command]
pub fn export_data(vault: VaultState, format: String) -> Result<String, String> {
    let saves = vault.all_saves().map_err(err)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    Ok(match format.as_str() {
        "html" => websave_core::export::to_netscape_html(&saves),
        _ => websave_core::export::to_json(&saves, now),
    })
}

// ---- in-app preview ----

/// Open a saved page in an isolated preview window.
///
/// The window label is deliberately NOT listed in any capability, so the
/// remote page gets zero access to Tauri commands or the vault — it is just
/// a sandboxed browser view.
#[tauri::command]
/// NOTE: this MUST stay `async`. Building a window needs the main thread, and
/// a synchronous command holds it — which deadlocks instead of opening
/// anything.
pub async fn preview_save(app: AppHandle, url: String, title: String) -> Result<(), String> {
    let parsed = tauri::Url::parse(&url).map_err(|e| format!("invalid url: {e}"))?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err("only http(s) pages can be previewed".into());
    }
    let window_title = if title.trim().is_empty() {
        "Preview".to_string()
    } else {
        format!("Preview — {title}")
    };

    // Reuse one preview window: navigating avoids the "label already exists"
    // race you get from closing and instantly rebuilding.
    if let Some(existing) = app.get_webview_window("preview") {
        existing.navigate(parsed).map_err(|e| e.to_string())?;
        let _ = existing.set_title(&window_title);
        let _ = existing.show();
        let _ = existing.set_focus();
        log::info!("preview: reused window for {url}");
        return Ok(());
    }

    let win =
        tauri::WebviewWindowBuilder::new(&app, "preview", tauri::WebviewUrl::External(parsed))
            .title(&window_title)
            .inner_size(1000.0, 720.0)
            .min_inner_size(420.0, 320.0)
            .build()
            .map_err(|e| {
                log::error!("preview: failed to open window for {url}: {e}");
                e.to_string()
            })?;
    let _ = win.set_focus();
    log::info!("preview: opened window for {url}");
    Ok(())
}

/// Check one link immediately. Network-bound, so it runs on a blocking thread.
#[tauri::command]
pub async fn check_save_now(
    app: AppHandle,
    vault: VaultState<'_>,
    id: i64,
) -> Result<Save, String> {
    let vault = vault.inner().clone();
    let save = tauri::async_runtime::spawn_blocking(move || vault.check_save(id).map_err(err))
        .await
        .map_err(|e| e.to_string())??;
    announce(&app);
    Ok(save)
}

/// Progress of a full re-check sweep, streamed to the UI so the button can
/// show a real loading state instead of returning instantly.
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RecheckProgress {
    done: usize,
    total: usize,
    finished: bool,
}

/// Shortest gap between progress events.
///
/// A 10k-save sweep would otherwise push 10k messages across the IPC bridge
/// — far more than a progress label can use, and enough to compete with the
/// checks themselves for the frontend's attention.
const PROGRESS_THROTTLE: Duration = Duration::from_millis(120);

/// Build a [`Monitor`] that reports into the Tauri event bus.
///
/// `progress` distinguishes the two callers: a user-initiated sweep drives
/// the re-check button's loading state, while the background daemon must
/// stay silent on that channel or the button would spin on its own every
/// half hour.
pub fn build_monitor(
    app: AppHandle,
    vault: Arc<Vault>,
    config: MonitorConfig,
    progress: bool,
) -> Monitor {
    let last_emit = Mutex::new(Instant::now() - PROGRESS_THROTTLE);
    Monitor::with_sink(vault, config, move |event| match event {
        MonitorEvent::Started { total } if progress => {
            let _ = app.emit(
                "recheck-progress",
                RecheckProgress {
                    done: 0,
                    total,
                    finished: false,
                },
            );
        }
        MonitorEvent::Checked { done, total, .. } if progress => {
            let mut last = last_emit.lock().unwrap();
            if last.elapsed() < PROGRESS_THROTTLE {
                return;
            }
            *last = Instant::now();
            let _ = app.emit(
                "recheck-progress",
                RecheckProgress {
                    done,
                    total,
                    finished: false,
                },
            );
        }
        // One list refresh per batch rather than per link: the frontend
        // refetches the whole list on this event.
        MonitorEvent::BatchDone { .. } => {
            let _ = app.emit("saves-updated", ());
        }
        MonitorEvent::Finished(summary) => {
            let _ = app.emit("saves-updated", ());
            if progress {
                let _ = app.emit(
                    "recheck-progress",
                    RecheckProgress {
                        done: summary.checked,
                        total: summary.checked,
                        finished: true,
                    },
                );
            }
        }
        _ => {}
    })
}

/// Queue a re-check of every save in the background. Emits "saves-updated"
/// as results land and "recheck-progress" so the UI can show progress.
/// Returns the number of saves queued.
#[tauri::command]
pub fn recheck_all(app: AppHandle, vault: VaultState) -> Result<i64, String> {
    let total = vault.count_saves().map_err(err)?;
    log::info!("recheck_all: queueing {total} link check(s)");

    // Nothing to do — tell the UI immediately so it doesn't spin forever.
    if total == 0 {
        let _ = app.emit(
            "recheck-progress",
            RecheckProgress {
                done: 0,
                total: 0,
                finished: true,
            },
        );
        return Ok(0);
    }

    // More hosts in flight than the background daemon: the user is watching
    // a progress bar and asked for this explicitly. Per-host pacing is
    // unchanged, so no individual server sees a heavier load.
    let config = MonitorConfig {
        max_concurrency: 24,
        ..Default::default()
    };
    let monitor = build_monitor(app, vault.inner().clone(), config, true);
    std::thread::spawn(move || {
        monitor.run_all();
    });
    Ok(total)
}

#[tauri::command]
pub fn logs_path(app: AppHandle) -> Result<String, String> {
    app.path()
        .app_log_dir()
        .map(|p| p.join("websave.log").display().to_string())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_logs_dir(app: AppHandle) -> Result<(), String> {
    let dir = app.path().app_log_dir().map_err(|e| e.to_string())?;
    app.opener()
        .open_path(dir.display().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_vault_dir(app: AppHandle, vault: VaultState) -> Result<(), String> {
    app.opener()
        .open_path(vault.root().display().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn capture_endpoint() -> String {
    format!("http://{}", websave_core::CAPTURE_ADDR)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub format: String,
    #[serde(flatten)]
    pub report: websave_core::ImportReport,
}

fn read_import_file(path: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("could not read file: {e}"))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Parse a bookmarks file and report what an import would do, without
/// writing anything.
#[tauri::command]
pub async fn preview_import(vault: VaultState<'_>, path: String) -> Result<ImportPreview, String> {
    let vault = vault.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let content = read_import_file(&path)?;
        let (format, items) = websave_core::import::parse(&content);
        let report = vault.preview_import(&items).map_err(err)?;
        Ok(ImportPreview {
            format: format.label().to_string(),
            report,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Import a bookmarks file into the vault.
#[tauri::command]
pub async fn run_import(
    app: AppHandle,
    vault: VaultState<'_>,
    path: String,
) -> Result<websave_core::ImportReport, String> {
    let vault = vault.inner().clone();
    let report = tauri::async_runtime::spawn_blocking(move || {
        let content = read_import_file(&path)?;
        let (_, items) = websave_core::import::parse(&content);
        vault.import_items(&items).map_err(err)
    })
    .await
    .map_err(|e| e.to_string())??;
    announce(&app);
    crate::wake_monitor(&app);
    Ok(report)
}

/// Launch the native menubar companion app (macOS only).
///
/// The menubar app ships embedded inside the engine's own bundle
/// (`Contents/Library/LoginItems/`) so the two share a lifecycle — deleting
/// the engine app removes the menubar with it. That embedded copy is
/// preferred; the other locations cover loose installs and the in-repo dev
/// build, with LaunchServices as a last resort.
#[tauri::command]
pub fn launch_menubar_app() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        const BUNDLE_ID: &str = "com.venugopal.web-save.menubar";
        const APP_NAME: &str = "WebSave Menubar.app";

        let mut candidates: Vec<std::path::PathBuf> = Vec::new();
        if let Ok(exe) = std::env::current_exe() {
            // Embedded helper inside our own bundle:
            // <engine>.app/Contents/MacOS/web-save → ../Library/LoginItems/<helper>
            if let Some(contents) = exe.parent().and_then(|p| p.parent()) {
                candidates.push(contents.join("Library/LoginItems").join(APP_NAME));
            }
            // Alongside the engine binary (loose install).
            if let Some(dir) = exe.parent() {
                candidates.push(dir.join(APP_NAME));
            }
            // In-repo dev build (running via `tauri dev`).
            for ancestor in exe.ancestors().skip(1) {
                let dev = ancestor.join("macos-menubar/dist").join(APP_NAME);
                if dev.exists() {
                    candidates.push(dev);
                    break;
                }
            }
        }
        candidates.push(std::path::PathBuf::from("/Applications").join(APP_NAME));

        for candidate in candidates {
            if candidate.exists() {
                let ok = std::process::Command::new("open")
                    .arg(&candidate)
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);
                if ok {
                    log::info!("launched menubar app from {}", candidate.display());
                    return Ok(());
                }
            }
        }
        // Last resort: LaunchServices may know an already-registered copy.
        if std::process::Command::new("open")
            .args(["-b", BUNDLE_ID])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return Ok(());
        }
        Err("Menubar app not found — build it with `npm run menubar`".into())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("The menubar companion app is macOS-only".into())
    }
}

#[tauri::command]
pub fn show_main_window(app: AppHandle) {
    crate::tray::show_main(&app);
}

#[tauri::command]
pub fn hide_quick_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window(crate::tray::QUICK_WINDOW) {
        let _ = window.hide();
        app.state::<crate::tray::QuickPanelGuard>().mark_hidden();
    }
}
