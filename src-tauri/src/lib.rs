mod commands;
mod server;
mod tray;

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use websave_core::Vault;

/// How often the background monitor wakes up on its own.
const CHECK_INTERVAL: Duration = Duration::from_secs(30 * 60);
/// A link is due for a re-check once its last check is older than this.
const CHECK_MAX_AGE_SECS: i64 = 24 * 60 * 60;
/// Checks per batch, to keep network usage polite.
const CHECK_BATCH: i64 = 20;

/// Pokes the monitor thread so freshly added saves get their first link
/// check within seconds instead of waiting for the next 30-minute cycle.
pub(crate) struct MonitorWaker(Mutex<mpsc::Sender<()>>);

impl MonitorWaker {
    pub fn wake(&self) {
        let _ = self.0.lock().unwrap().send(());
    }
}

/// Wake the monitor from anywhere that just wrote new URLs to the vault.
pub(crate) fn wake_monitor(app: &AppHandle) {
    if let Some(waker) = app.try_state::<MonitorWaker>() {
        waker.wake();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
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
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let vault_dir = app.path().app_data_dir()?.join("vault");
            let vault = Arc::new(Vault::open(vault_dir)?);
            app.manage(vault.clone());
            app.manage(tray::QuickPanelGuard::new());

            // On macOS the menubar belongs to the native SwiftUI app
            // (macos-menubar/), which links websave-core directly — so the
            // engine creates no tray icon and no popover webview there.
            #[cfg(not(target_os = "macos"))]
            {
                use tauri::Listener;

                tray::setup(app.handle(), &vault)?;

                tauri::WebviewWindowBuilder::new(
                    app,
                    tray::QUICK_WINDOW,
                    tauri::WebviewUrl::default(),
                )
                .title("WebSave Quick Access")
                .inner_size(380.0, 520.0)
                .visible(false)
                .decorations(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .minimizable(false)
                .maximizable(false)
                .closable(false)
                .build()?;

                // Keep the tray dropdown in sync with the vault.
                let handle = app.handle().clone();
                let menu_vault = vault.clone();
                app.listen("saves-updated", move |_| {
                    let handle_inner = handle.clone();
                    let vault_inner = menu_vault.clone();
                    let _ = handle.run_on_main_thread(move || {
                        tray::rebuild_menu(&handle_inner, &vault_inner);
                    });
                });
            }

            server::spawn(app.handle().clone(), vault.clone());

            let (wake_tx, wake_rx) = mpsc::channel();
            app.manage(MonitorWaker(Mutex::new(wake_tx)));
            spawn_monitor(app.handle().clone(), vault, wake_rx);
            Ok(())
        })
        .on_window_event(|window, event| match event {
            // Closing the main window hides it so capture and link
            // monitoring keep running; quit lives in the tray menu.
            WindowEvent::CloseRequested { api, .. } if window.label() == "main" => {
                api.prevent_close();
                let _ = window.hide();
            }
            WindowEvent::Focused(false) if window.label() == tray::QUICK_WINDOW => {
                let _ = window.hide();
                window
                    .app_handle()
                    .state::<tray::QuickPanelGuard>()
                    .mark_hidden();
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_saves,
            commands::get_save,
            commands::add_save,
            commands::update_save,
            commands::set_favorite,
            commands::set_tags,
            commands::set_read,
            commands::set_url,
            commands::delete_save,
            commands::bulk_set_favorite,
            commands::bulk_set_read,
            commands::bulk_delete,
            commands::bulk_add_tag,
            commands::list_saved_searches,
            commands::add_saved_search,
            commands::delete_saved_search,
            commands::list_tags,
            commands::get_archive,
            commands::vault_stats,
            commands::vault_path,
            commands::check_save_now,
            commands::recheck_all,
            commands::logs_path,
            commands::open_logs_dir,
            commands::open_vault_dir,
            commands::capture_endpoint,
            commands::preview_import,
            commands::run_import,
            commands::launch_menubar_app,
            commands::show_main_window,
            commands::hide_quick_window,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        // macOS: clicking the dock icon while the window is hidden.
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Reopen { .. } = event {
            tray::show_main(app_handle);
        }
        let _ = (app_handle, &event);
    });
}

/// Background link monitor. Runs whenever it is woken (new saves, imports,
/// captures) and at least every `CHECK_INTERVAL`; notifies the UI as
/// results land.
fn spawn_monitor(app: AppHandle, vault: Arc<Vault>, wake: mpsc::Receiver<()>) {
    std::thread::spawn(move || {
        // Let the app finish starting before hitting the network.
        std::thread::sleep(Duration::from_secs(5));
        loop {
            run_due_checks(&app, &vault);
            match wake.recv_timeout(CHECK_INTERVAL) {
                // Brief debounce so a burst of adds is checked as one batch.
                Ok(()) => std::thread::sleep(Duration::from_secs(1)),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });
}

/// Check everything currently due, in polite batches, emitting progress
/// after each batch so the UI updates as statuses come in.
fn run_due_checks(app: &AppHandle, vault: &Arc<Vault>) {
    loop {
        let Ok(targets) = vault.saves_due_for_check(CHECK_MAX_AGE_SECS, CHECK_BATCH) else {
            return;
        };
        if targets.is_empty() {
            return;
        }
        log::info!("monitor: checking {} link(s)", targets.len());
        for t in &targets {
            let outcome = websave_core::check_url(&t.url, &t.content_hash);
            let _ = vault.apply_check(t.id, &outcome);
            vault.maybe_fetch_thumbnail(t.id, outcome.og_image.as_deref());
        }
        let _ = app.emit("saves-updated", ());
        if (targets.len() as i64) < CHECK_BATCH {
            return;
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}
