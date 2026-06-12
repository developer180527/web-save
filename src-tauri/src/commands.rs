use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;
use websave_core::{ListQuery, NewSave, Save, SavePatch, TagCount, Vault, VaultStats};

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

#[tauri::command]
pub fn vault_path(vault: VaultState) -> String {
    vault.root().display().to_string()
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

/// Queue a re-check of every save in the background; emits "saves-updated"
/// as results land. Returns the number of saves queued.
#[tauri::command]
pub fn recheck_all(app: AppHandle, vault: VaultState) -> Result<i64, String> {
    let targets = vault.saves_due_for_check(-1, 10_000).map_err(err)?;
    let count = targets.len() as i64;
    log::info!("recheck_all: queueing {count} link check(s)");
    let vault = vault.inner().clone();
    std::thread::spawn(move || {
        for (i, t) in targets.iter().enumerate() {
            let outcome = websave_core::check_url(&t.url, &t.content_hash);
            let _ = vault.apply_check(t.id, &outcome);
            vault.maybe_fetch_thumbnail(t.id, outcome.og_image.as_deref());
            // Stream progress to the UI every few results instead of one
            // event per link.
            if i % 5 == 4 {
                let _ = app.emit("saves-updated", ());
            }
        }
        let _ = app.emit("saves-updated", ());
        log::info!("recheck_all: finished");
    });
    Ok(count)
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
    format!("http://{}", crate::server::CAPTURE_ADDR)
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
/// Resolution order: LaunchServices by bundle id (covers an installed copy
/// and activates an already-running instance instead of duplicating), then
/// known locations — /Applications, next to the engine binary, and the
/// in-repo dev build.
#[tauri::command]
pub fn launch_menubar_app() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        const BUNDLE_ID: &str = "com.venugopal.web-save.menubar";
        const APP_NAME: &str = "WebSave Menubar.app";

        let by_id = std::process::Command::new("open")
            .args(["-b", BUNDLE_ID])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if by_id {
            return Ok(());
        }

        let mut candidates = vec![std::path::PathBuf::from("/Applications").join(APP_NAME)];
        if let Ok(exe) = std::env::current_exe() {
            // Alongside the installed engine bundle.
            if let Some(dir) = exe.parent() {
                candidates.push(dir.join(APP_NAME));
            }
            // Dev build: target/debug/web-save → repo/macos-menubar/dist/.
            for ancestor in exe.ancestors().skip(1) {
                let dev = ancestor.join("macos-menubar/dist").join(APP_NAME);
                if dev.exists() {
                    candidates.push(dev);
                    break;
                }
            }
        }

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
