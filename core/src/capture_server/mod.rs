//! Localhost capture server.
//!
//! The browser extension — and any other local capture client, like the macOS
//! menubar app — talks to the vault through this endpoint instead of opening
//! the database itself, so all validation and storage logic stays in one
//! place.
//!
//! This lives in core rather than in a UI host because nothing about it is
//! UI-specific: it is how the outside world writes to a vault. The two things
//! that *were* host-specific — raising a window, refreshing a list — are
//! reported as [`CaptureEvent`]s instead of being wired into the handler.
//!
//! - [`client_gate`] — the rule that keeps arbitrary web pages out.
//! - [`endpoints`] — routing and request handling.
//! - [`screenshot_data_url`] — decoding the extension's fallback cover image.
//! - [`events`] — what the server tells its host.

mod client_gate;
mod endpoints;
mod events;
mod screenshot_data_url;

pub use events::CaptureEvent;

use std::sync::Arc;

use tiny_http::Server;

use crate::vault::Vault;

/// Where capture clients look for a running app. The extension and the
/// menubar app both hardcode this, so it is part of the contract.
pub const CAPTURE_ADDR: &str = "127.0.0.1:38917";

type EventSink = Arc<dyn Fn(CaptureEvent) + Send + Sync>;

#[derive(Debug, Clone)]
pub struct CaptureConfig {
    /// Address to bind. Loopback only — see [`client_gate`].
    pub addr: String,
    /// Reported by `/ping`. The *host application's* version, which is what
    /// the extension checks compatibility against, not core's.
    pub app_version: String,
}

impl Default for CaptureConfig {
    fn default() -> CaptureConfig {
        CaptureConfig {
            addr: CAPTURE_ADDR.to_string(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// A running capture server. Dropping this does not stop it; call
/// [`CaptureServer::shutdown`].
pub struct CaptureServer {
    server: Arc<Server>,
}

impl CaptureServer {
    /// Bind and start serving on a background thread.
    ///
    /// Fails if the address is already in use, which is also how a second
    /// instance of the app discovers it is the second instance.
    pub fn start(
        vault: Arc<Vault>,
        config: CaptureConfig,
        sink: impl Fn(CaptureEvent) + Send + Sync + 'static,
    ) -> std::io::Result<CaptureServer> {
        let server =
            Server::http(config.addr.as_str()).map_err(|e| std::io::Error::other(e.to_string()))?;
        let server = Arc::new(server);
        log::info!("capture server: listening on http://{}", config.addr);

        let sink: EventSink = Arc::new(sink);
        let worker = server.clone();
        std::thread::spawn(move || {
            for mut request in worker.incoming_requests() {
                let reply = endpoints::route(&mut request, &vault, &config, &sink);
                if let Err(e) = request.respond(reply) {
                    log::warn!("capture server: failed to respond: {e}");
                }
            }
            log::info!("capture server: stopped");
        });

        Ok(CaptureServer { server })
    }

    /// The address actually bound. Differs from the configured one when the
    /// port was `0`, which tests use to avoid fighting over 38917.
    pub fn addr(&self) -> Option<std::net::SocketAddr> {
        self.server.server_addr().to_ip()
    }

    /// Stop accepting requests. The worker thread exits once the in-flight
    /// request finishes.
    pub fn shutdown(&self) {
        self.server.unblock();
    }
}
