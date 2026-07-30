//! Hosting `websave_core`'s capture server inside the Tauri app.
//!
//! The server itself — binding, routing, the client-header gate, screenshot
//! decoding — lives in core, because it is how the outside world writes to a
//! vault and has nothing to do with any particular UI. All that belongs here
//! is the translation of [`CaptureEvent`] into things a Tauri app can do:
//! refresh its windows, raise the main one, wake the link monitor.

use std::sync::Arc;

use tauri::{AppHandle, Emitter};
use websave_core::{CaptureConfig, CaptureEvent, CaptureServer, Vault};

pub fn spawn(app: AppHandle, vault: Arc<Vault>) {
    let config = CaptureConfig {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        ..Default::default()
    };
    let server = CaptureServer::start(vault, config, move |event| match event {
        CaptureEvent::Saved(_) => {
            let _ = app.emit("saves-updated", ());
            // A freshly captured page should get its health check, thumbnail
            // and archive within seconds, not at the next half-hour tick.
            crate::wake_monitor(&app);
        }
        CaptureEvent::ShowRequested => crate::tray::show_main(&app),
        CaptureEvent::ReloadRequested => {
            let _ = app.emit("saves-updated", ());
        }
    });

    if let Err(e) = server {
        // Almost always "address in use": another instance already owns the
        // port and is serving the extension, so this one just goes without.
        log::error!("capture server: failed to start: {e}");
        return;
    }
    // Deliberately leaked: the server lives as long as the app, and Tauri's
    // setup hook has nowhere to park it.
    std::mem::forget(server);
}
