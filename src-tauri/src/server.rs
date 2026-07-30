//! Localhost capture server.
//!
//! The browser extension (and any other local capture client) talks to the
//! app through this endpoint instead of touching the database directly, so
//! all validation and storage logic stays in `websave-core`.
//!
//! Security model: we bind to 127.0.0.1 only, never send CORS headers, and
//! require a custom `x-websave-client` header. Custom headers force browsers
//! to send a CORS preflight, which fails without CORS headers — so arbitrary
//! web pages cannot post into the vault. The extension is exempt because
//! Chrome lets it call hosts listed in `host_permissions` directly.

use std::io::Read;
use std::sync::Arc;

use tauri::{AppHandle, Emitter};
use tiny_http::{Header, Method, Response, Server};
use websave_core::{NewSave, Vault};

pub const CAPTURE_ADDR: &str = "127.0.0.1:38917";
const CLIENT_HEADER: &str = "x-websave-client";
const EXT_VERSION_HEADER: &str = "x-websave-ext-version";
// Generous cap: payloads may carry a base64 viewport screenshot.
const MAX_BODY_BYTES: u64 = 12 * 1024 * 1024;
const MAX_SCREENSHOT_BYTES: usize = 6 * 1024 * 1024;

/// What the extension POSTs: a NewSave plus an optional viewport screenshot
/// (data URL), sent only when the page declares no cover image of its own.
#[derive(serde::Deserialize)]
struct CapturePayload {
    #[serde(flatten)]
    save: NewSave,
    #[serde(default)]
    screenshot: Option<String>,
}

pub fn spawn(app: AppHandle, vault: Arc<Vault>) {
    std::thread::spawn(move || {
        let server = match Server::http(CAPTURE_ADDR) {
            Ok(s) => s,
            Err(e) => {
                log::error!("capture server: failed to bind {CAPTURE_ADDR}: {e}");
                return;
            }
        };
        log::info!("capture server: listening on http://{CAPTURE_ADDR}");
        for mut request in server.incoming_requests() {
            let response = handle(&mut request, &vault, &app);
            if let Err(e) = request.respond(response) {
                log::warn!("capture server: failed to respond: {e}");
            }
        }
    });
}

fn handle(
    request: &mut tiny_http::Request,
    vault: &Vault,
    app: &AppHandle,
) -> Response<std::io::Cursor<Vec<u8>>> {
    match (request.method(), request.url()) {
        (Method::Get, "/ping") => json(
            200,
            format!(
                r#"{{"app":"websave","version":"{}"}}"#,
                env!("CARGO_PKG_VERSION")
            ),
        ),
        // Recent saves for the extension popup's search. Read access is
        // gated by the same custom header as /save, so ordinary web pages
        // (which would need a CORS preflight we never answer) can't read the
        // vault — only the extension, which Chrome exempts via
        // host_permissions.
        (Method::Get, url) if url.starts_with("/recent") => {
            if header_value(request, CLIENT_HEADER).is_none() {
                log::warn!("capture server: rejected /recent without {CLIENT_HEADER} header");
                return json(403, r#"{"error":"missing client header"}"#.into());
            }
            let limit = query_param(url, "limit")
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(200)
                .clamp(1, 500);
            match vault.list_saves(&websave_core::ListQuery {
                limit: Some(limit),
                ..Default::default()
            }) {
                Ok(saves) => {
                    // Only what the popup renders — keep the payload small.
                    let slim: Vec<_> = saves
                        .iter()
                        .map(|s| {
                            serde_json::json!({
                                "id": s.id,
                                "url": s.url,
                                "title": s.title,
                                "faviconUrl": s.favicon_url,
                                "status": s.status.as_str(),
                            })
                        })
                        .collect();
                    match serde_json::to_string(&slim) {
                        Ok(body) => json(200, body),
                        Err(e) => json(500, format!(r#"{{"error":"{e}"}}"#)),
                    }
                }
                Err(e) => json(500, format!(r#"{{"error":"{e}"}}"#)),
            }
        }
        // Used by the native menubar app: raise the main window.
        (Method::Get, "/show") => {
            crate::tray::show_main(app);
            json(200, r#"{"ok":true}"#.into())
        }
        // Used by the native menubar app after it writes to the vault, so
        // the engine's windows refresh immediately.
        (Method::Get, "/reload") => {
            let _ = app.emit("saves-updated", ());
            json(200, r#"{"ok":true}"#.into())
        }
        (Method::Post, "/save") => {
            let client = header_value(request, CLIENT_HEADER);
            if client.is_none() {
                log::warn!("capture server: rejected /save without {CLIENT_HEADER} header");
                return json(403, r#"{"error":"missing client header"}"#.into());
            }
            let ext_version = header_value(request, EXT_VERSION_HEADER);

            let mut body = String::new();
            if request
                .as_reader()
                .take(MAX_BODY_BYTES)
                .read_to_string(&mut body)
                .is_err()
            {
                return json(400, r#"{"error":"unreadable body"}"#.into());
            }
            let payload: CapturePayload = match serde_json::from_str(&body) {
                Ok(n) => n,
                Err(e) => return json(400, format!(r#"{{"error":"invalid json: {e}"}}"#)),
            };

            match vault.add_save(payload.save) {
                Ok(mut save) => {
                    // Screenshot fallback: only fills the gap, never replaces
                    // an existing cover.
                    if save.thumbnail.is_empty() {
                        if let Some((bytes, ext)) =
                            payload.screenshot.as_deref().and_then(decode_data_url)
                        {
                            if let Ok(path) = vault.set_thumbnail(save.id, &bytes, ext) {
                                save.thumbnail = path;
                            }
                        }
                    }
                    // Remember that a capture client checked in, so the app
                    // can show "extension connected" and drop onboarding.
                    // Best-effort: a metadata failure must never fail a save.
                    if client.as_deref() == Some("extension") {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        let _ = vault.set_meta("ext.last_seen", &now.to_string());
                        if let Some(v) = &ext_version {
                            let _ = vault.set_meta("ext.version", v);
                        }
                    }
                    let _ = app.emit("saves-updated", ());
                    crate::wake_monitor(app);
                    match serde_json::to_string(&save) {
                        Ok(s) => json(200, s),
                        Err(e) => json(500, format!(r#"{{"error":"{e}"}}"#)),
                    }
                }
                Err(e) => {
                    log::warn!("capture server: save rejected: {e}");
                    json(400, format!(r#"{{"error":"{e}"}}"#))
                }
            }
        }
        _ => json(404, r#"{"error":"not found"}"#.into()),
    }
}

/// Read a query-string parameter out of a request path.
fn query_param(url: &str, name: &str) -> Option<String> {
    let (_, query) = url.split_once('?')?;
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == name).then(|| v.to_string())
    })
}

/// Case-insensitive lookup of a request header value. (`Header::equiv`
/// requires a `&'static str`, so we compare field names directly.)
fn header_value(request: &tiny_http::Request, name: &str) -> Option<String> {
    request
        .headers()
        .iter()
        .find(|h: &&Header| h.field.as_str().as_str().eq_ignore_ascii_case(name))
        .map(|h| h.value.as_str().to_string())
}

/// `data:image/jpeg;base64,...` → (bytes, extension), with mime and size
/// validation.
fn decode_data_url(data_url: &str) -> Option<(Vec<u8>, &'static str)> {
    use base64::Engine;
    let rest = data_url.strip_prefix("data:")?;
    let (mime, b64) = rest.split_once(";base64,")?;
    let ext = match mime.to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        _ => return None,
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .ok()?;
    (!bytes.is_empty() && bytes.len() <= MAX_SCREENSHOT_BYTES).then_some((bytes, ext))
}

fn json(status: u16, body: String) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(body)
        .with_status_code(status)
        .with_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap())
}
