mod commands;
mod server;

use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use websave_core::Vault;

/// How often the background monitor wakes up.
const CHECK_INTERVAL: Duration = Duration::from_secs(30 * 60);
/// A link is due for a re-check once its last check is older than this.
const CHECK_MAX_AGE_SECS: i64 = 24 * 60 * 60;
/// Checks per wake-up, to keep network usage polite.
const CHECK_BATCH: i64 = 20;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("websave".into()),
                    }),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Webview),
                ])
                .level(log::LevelFilter::Info)
                .level_for("websave_core", log::LevelFilter::Debug)
                .level_for("web_save_lib", log::LevelFilter::Debug)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let vault_dir = app.path().app_data_dir()?.join("vault");
            let vault = Arc::new(Vault::open(vault_dir)?);
            app.manage(vault.clone());
            server::spawn(app.handle().clone(), vault.clone());
            spawn_monitor(app.handle().clone(), vault);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_saves,
            commands::get_save,
            commands::add_save,
            commands::update_save,
            commands::set_favorite,
            commands::set_tags,
            commands::delete_save,
            commands::list_tags,
            commands::vault_stats,
            commands::vault_path,
            commands::check_save_now,
            commands::recheck_all,
            commands::logs_path,
            commands::open_logs_dir,
            commands::open_vault_dir,
            commands::capture_endpoint,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Background link monitor: periodically re-checks stale links and notifies
/// the UI when statuses change.
fn spawn_monitor(app: AppHandle, vault: Arc<Vault>) {
    std::thread::spawn(move || {
        // Let the app finish starting before hitting the network.
        std::thread::sleep(Duration::from_secs(90));
        loop {
            if let Ok(targets) = vault.saves_due_for_check(CHECK_MAX_AGE_SECS, CHECK_BATCH) {
                if !targets.is_empty() {
                    log::info!("monitor: checking {} stale link(s)", targets.len());
                    for t in &targets {
                        let outcome = websave_core::check_url(&t.url, &t.content_hash);
                        let _ = vault.apply_check(t.id, &outcome);
                    }
                    let _ = app.emit("saves-updated", ());
                }
            }
            std::thread::sleep(CHECK_INTERVAL);
        }
    });
}
