//! Hosting `websave_core`'s capture server inside the Flutter app.
//!
//! The browser extension saves pages by POSTing to `127.0.0.1:38917`. Until
//! now only the legacy Tauri app ran that server, so with just this app open
//! the extension had nowhere to send and queued every save as "waiting to
//! sync". The server itself (binding, routing, the client-header gate) is
//! core's, shared with the Tauri host; all that lives here is starting it
//! against the bridge's vault and passing its events up to Dart.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use websave_core::{CaptureConfig, CaptureEvent, CaptureServer, CAPTURE_ADDR};

use crate::frb_generated::StreamSink;

/// Kept for the life of the process; dropping it would stop the server.
static SERVER: OnceLock<CaptureServer> = OnceLock::new();

/// Where capture events go once Dart is listening. Events that arrive
/// before then are dropped, which is harmless: the UI loads from the vault
/// on start anyway.
static EVENTS: Mutex<Option<StreamSink<CaptureNotice>>> = Mutex::new(None);

// Plain enums plus a struct rather than enums carrying data: the latter make
// flutter_rust_bridge generate `freezed` classes, a code-generation
// dependency nothing else in the app needs.

/// Something the capture server did that the UI should react to.
pub enum CaptureNoticeKind {
    /// A page arrived from the extension; refresh the views.
    Saved,
    /// The macOS menubar asked for the main window.
    ShowRequested,
    /// A companion app wrote to the vault directly; refresh the views.
    ReloadRequested,
}

pub struct CaptureNotice {
    pub kind: CaptureNoticeKind,
    /// The saved page, for [`CaptureNoticeKind::Saved`].
    pub url: Option<String>,
}

/// How starting the capture server went.
pub enum CaptureServerState {
    /// This app is serving the extension.
    Serving,
    /// Another WebSave (usually the legacy Tauri app) already serves the
    /// extension on this port. Saves still reach the shared vault.
    OtherInstance,
    /// The port is held by something that is not WebSave, or binding failed
    /// outright. The extension cannot save until that is resolved.
    Unavailable,
}

pub struct CaptureServerStart {
    pub state: CaptureServerState,
    /// Why, for [`CaptureServerState::Unavailable`].
    pub reason: Option<String>,
}

impl CaptureServerStart {
    fn of(state: CaptureServerState) -> CaptureServerStart {
        CaptureServerStart { state, reason: None }
    }
}

/// Start the extension's capture server. Desktop only: there is no browser
/// extension on iOS or Android. Idempotent, so a hot restart is harmless.
pub fn start_capture_server(app_version: String) -> Result<CaptureServerStart, String> {
    if SERVER.get().is_some() {
        return Ok(CaptureServerStart::of(CaptureServerState::Serving));
    }
    let vault = super::vault_api::shared_vault()?;
    let config = CaptureConfig {
        app_version,
        ..Default::default()
    };
    match CaptureServer::start(vault, config, forward) {
        Ok(server) => {
            let _ = SERVER.set(server);
            Ok(CaptureServerStart::of(CaptureServerState::Serving))
        }
        // Binding fails when the port is taken. Say whose it is: another
        // WebSave is fine, anything else means the extension is cut off.
        Err(_) if is_websave_listening() => {
            Ok(CaptureServerStart::of(CaptureServerState::OtherInstance))
        }
        Err(e) => Ok(CaptureServerStart {
            state: CaptureServerState::Unavailable,
            reason: Some(e.to_string()),
        }),
    }
}

/// Events from the capture server, for as long as Dart listens.
pub fn capture_events(sink: StreamSink<CaptureNotice>) {
    *EVENTS.lock().unwrap() = Some(sink);
}

fn forward(event: CaptureEvent) {
    let (kind, url) = match event {
        CaptureEvent::Saved(save) => (CaptureNoticeKind::Saved, Some(save.url)),
        CaptureEvent::ShowRequested => (CaptureNoticeKind::ShowRequested, None),
        CaptureEvent::ReloadRequested => (CaptureNoticeKind::ReloadRequested, None),
    };
    let notice = CaptureNotice { kind, url };
    if let Some(sink) = EVENTS.lock().unwrap().as_ref() {
        let _ = sink.add(notice);
    }
}

/// Whether whatever holds the capture port answers `/ping` as WebSave.
///
/// A raw request rather than an HTTP client: it is one fixed GET to
/// loopback, and the bridge has no other use for a client.
fn is_websave_listening() -> bool {
    let Ok(mut stream) = TcpStream::connect(CAPTURE_ADDR) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let request = format!("GET /ping HTTP/1.1\r\nHost: {CAPTURE_ADDR}\r\nConnection: close\r\n\r\n");
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }
    let mut response = String::new();
    let _ = stream.read_to_string(&mut response);
    response.contains(r#""app":"websave""#)
}
