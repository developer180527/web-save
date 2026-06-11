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

#[tauri::command]
pub fn add_save(vault: VaultState, save: NewSave) -> Result<Save, String> {
    vault.add_save(save).map_err(err)
}

#[tauri::command]
pub fn update_save(vault: VaultState, id: i64, patch: SavePatch) -> Result<Save, String> {
    vault.update_save(id, patch).map_err(err)
}

#[tauri::command]
pub fn set_favorite(vault: VaultState, id: i64, favorite: bool) -> Result<Save, String> {
    vault.set_favorite(id, favorite).map_err(err)
}

#[tauri::command]
pub fn set_tags(vault: VaultState, id: i64, tags: Vec<String>) -> Result<Save, String> {
    vault.set_tags(id, &tags).map_err(err)
}

#[tauri::command]
pub fn delete_save(vault: VaultState, id: i64) -> Result<(), String> {
    vault.delete_save(id).map_err(err)
}

#[tauri::command]
pub fn list_tags(vault: VaultState) -> Result<Vec<TagCount>, String> {
    vault.list_tags().map_err(err)
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
pub async fn check_save_now(vault: VaultState<'_>, id: i64) -> Result<Save, String> {
    let vault = vault.inner().clone();
    tauri::async_runtime::spawn_blocking(move || vault.check_save(id).map_err(err))
        .await
        .map_err(|e| e.to_string())?
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
