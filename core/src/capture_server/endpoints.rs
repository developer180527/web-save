//! Routing and request handling.

use std::io::{Cursor, Read};

use tiny_http::{Header, Method, Request, Response};

use super::client_gate;
use super::events::CaptureEvent;
use super::screenshot_data_url;
use super::{CaptureConfig, EventSink};
use crate::models::{ListQuery, NewSave};
use crate::vault::Vault;

/// Generous cap: a payload may carry a base64 viewport screenshot.
const MAX_BODY_BYTES: u64 = 12 * 1024 * 1024;

/// Default page size for `/recent`, and the ceiling a caller can ask for.
const RECENT_DEFAULT: i64 = 200;
const RECENT_MAX: i64 = 500;

pub(super) type Reply = Response<Cursor<Vec<u8>>>;

/// What the extension POSTs: a [`NewSave`] plus an optional viewport
/// screenshot, sent only when the page declares no cover image of its own.
#[derive(serde::Deserialize)]
struct CapturePayload {
    #[serde(flatten)]
    save: NewSave,
    #[serde(default)]
    screenshot: Option<String>,
}

pub(super) fn route(
    request: &mut Request,
    vault: &Vault,
    config: &CaptureConfig,
    sink: &EventSink,
) -> Reply {
    match (request.method(), request.url()) {
        (Method::Get, "/ping") => ping(config),
        (Method::Get, url) if url.starts_with("/recent") => recent(request, vault, url),
        (Method::Get, "/show") => {
            sink(CaptureEvent::ShowRequested);
            json(200, r#"{"ok":true}"#.into())
        }
        (Method::Get, "/reload") => {
            sink(CaptureEvent::ReloadRequested);
            json(200, r#"{"ok":true}"#.into())
        }
        (Method::Post, "/save") => save(request, vault, sink),
        _ => json(404, r#"{"error":"not found"}"#.into()),
    }
}

/// Ungated on purpose: carries no vault data, and the extension needs it to
/// discover whether the app is running.
fn ping(config: &CaptureConfig) -> Reply {
    json(
        200,
        format!(
            r#"{{"app":"websave","version":"{}"}}"#,
            config.app_version.replace('"', "")
        ),
    )
}

/// Recent saves for the extension popup's search.
///
/// Read access is gated by the same client header as `/save` — see
/// [`super::client_gate`] — so ordinary web pages can't read the vault.
fn recent(request: &Request, vault: &Vault, url: &str) -> Reply {
    if client_gate::identify(request).is_none() {
        log::warn!("capture server: rejected /recent without a client header");
        return json(403, r#"{"error":"missing client header"}"#.into());
    }
    let limit = query_param(url, "limit")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(RECENT_DEFAULT)
        .clamp(1, RECENT_MAX);

    let saves = match vault.list_saves(&ListQuery {
        limit: Some(limit),
        ..Default::default()
    }) {
        Ok(saves) => saves,
        Err(e) => return json(500, error_body(&e.to_string())),
    };
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
        Err(e) => json(500, error_body(&e.to_string())),
    }
}

fn save(request: &mut Request, vault: &Vault, sink: &EventSink) -> Reply {
    let Some(client) = client_gate::identify(request) else {
        log::warn!("capture server: rejected /save without a client header");
        return json(403, r#"{"error":"missing client header"}"#.into());
    };
    let ext_version = client_gate::extension_version(request);

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
        Ok(p) => p,
        Err(e) => return json(400, error_body(&format!("invalid json: {e}"))),
    };

    let mut save = match vault.add_save(payload.save) {
        Ok(save) => save,
        Err(e) => {
            log::warn!("capture server: save rejected: {e}");
            return json(400, error_body(&e.to_string()));
        }
    };

    // Screenshot fallback: fills the gap, never replaces an existing cover.
    if save.thumbnail.is_empty() {
        if let Some((bytes, ext)) = payload
            .screenshot
            .as_deref()
            .and_then(screenshot_data_url::decode)
        {
            if let Ok(path) = vault.set_thumbnail(save.id, &bytes, ext) {
                save.thumbnail = path;
            }
        }
    }

    // Remember that a capture client checked in, so the app can show
    // "extension connected" and drop its onboarding. Best-effort: a metadata
    // failure must never fail a save that already succeeded.
    if client == "extension" {
        let _ = vault.set_meta("ext.last_seen", &crate::vault::now().to_string());
        if let Some(version) = &ext_version {
            let _ = vault.set_meta("ext.version", version);
        }
    }

    sink(CaptureEvent::Saved(Box::new(save.clone())));
    match serde_json::to_string(&save) {
        Ok(body) => json(200, body),
        Err(e) => json(500, error_body(&e.to_string())),
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

/// A JSON error body with the message escaped, so a quote in an error can't
/// produce malformed JSON.
fn error_body(message: &str) -> String {
    serde_json::json!({ "error": message }).to_string()
}

fn json(status: u16, body: String) -> Reply {
    Response::from_string(body)
        .with_status_code(status)
        .with_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap())
}

#[cfg(test)]
mod tests {
    use super::client_gate::{CLIENT_HEADER, EXT_VERSION_HEADER};
    use super::{error_body, query_param};

    #[test]
    fn reads_query_parameters() {
        assert_eq!(
            query_param("/recent?limit=50", "limit").as_deref(),
            Some("50")
        );
        assert_eq!(
            query_param("/recent?a=1&limit=7", "limit").as_deref(),
            Some("7")
        );
        assert_eq!(query_param("/recent", "limit"), None);
        assert_eq!(query_param("/recent?limit=5", "other"), None);
    }

    /// The old handler interpolated error text straight into a JSON string
    /// literal, so a quote in a message produced a broken body.
    #[test]
    fn error_bodies_escape_their_message() {
        let body = error_body(r#"bad "url" here"#);
        let parsed: serde_json::Value = serde_json::from_str(&body).expect("valid JSON");
        assert_eq!(parsed["error"], r#"bad "url" here"#);
    }

    #[test]
    fn header_names_are_stable() {
        // The extension hardcodes these; renaming one silently breaks capture.
        assert_eq!(CLIENT_HEADER, "x-websave-client");
        assert_eq!(EXT_VERSION_HEADER, "x-websave-ext-version");
    }
}
