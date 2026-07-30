//! What the capture server tells its host.
//!
//! The server itself only knows how to store things. Raising a window or
//! refreshing a list is the host's job, so those arrive as events rather than
//! being wired into the request handler — which is what lets the same server
//! run under Tauri, Flutter or a headless daemon.

use crate::models::Save;

#[derive(Debug, Clone)]
pub enum CaptureEvent {
    /// A capture client stored a page. The host should refresh its views and
    /// nudge the link monitor so the new save is checked promptly.
    Saved(Box<Save>),
    /// `GET /show` — a companion app (the macOS menubar) asks for the main
    /// window.
    ShowRequested,
    /// `GET /reload` — a companion app wrote to the vault directly and wants
    /// the host's views refreshed.
    ReloadRequested,
}
