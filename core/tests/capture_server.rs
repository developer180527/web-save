//! End-to-end tests for the localhost capture server.
//!
//! Mostly about the security gate, because that gate is the only thing
//! standing between an open port on the user's machine and their whole vault.
//! Each test binds port 0 so runs never collide.

use std::sync::{Arc, Mutex};

use websave_core::{CaptureConfig, CaptureEvent, CaptureServer, ListQuery, Vault};

const CLIENT_HEADER: &str = "x-websave-client";

struct Harness {
    base: String,
    vault: Arc<Vault>,
    events: Arc<Mutex<Vec<String>>>,
    server: CaptureServer,
    _dir: tempfile::TempDir,
}

fn start() -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let vault = Arc::new(Vault::open(dir.path().join("vault")).unwrap());
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();

    let server = CaptureServer::start(
        vault.clone(),
        CaptureConfig {
            addr: "127.0.0.1:0".into(),
            app_version: "9.9.9".into(),
        },
        move |event| {
            let label = match event {
                CaptureEvent::Saved(save) => format!("saved:{}", save.url),
                CaptureEvent::ShowRequested => "show".into(),
                CaptureEvent::ReloadRequested => "reload".into(),
            };
            sink.lock().unwrap().push(label);
        },
    )
    .expect("server binds");

    let base = format!("http://{}", server.addr().expect("bound to an ip"));
    Harness {
        base,
        vault,
        events,
        server,
        _dir: dir,
    }
}

impl Harness {
    fn events(&self) -> Vec<String> {
        self.events.lock().unwrap().clone()
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.server.shutdown();
    }
}

/// `ureq` treats 4xx as an error; unwrap either arm into (status, body).
fn send(request: ureq::Request, body: Option<&str>) -> (u16, String) {
    let result = match body {
        Some(b) => request.send_string(b),
        None => request.call(),
    };
    match result {
        Ok(r) => (r.status(), r.into_string().unwrap_or_default()),
        Err(ureq::Error::Status(code, r)) => (code, r.into_string().unwrap_or_default()),
        Err(e) => panic!("transport error: {e}"),
    }
}

#[test]
fn ping_is_open_and_reports_the_host_version() {
    let h = start();
    let (status, body) = send(ureq::get(&format!("{}/ping", h.base)), None);
    assert_eq!(status, 200);
    // Ungated on purpose: the extension needs it to detect a running app,
    // and it exposes nothing about the vault.
    assert!(body.contains(r#""app":"websave""#), "{body}");
    assert!(
        body.contains(r#""version":"9.9.9""#),
        "reports the host app's version, not core's: {body}"
    );
}

/// The security model in one test: a web page can't read the vault, because
/// it can't set a custom header without a CORS preflight we never answer.
#[test]
fn reads_and_writes_are_refused_without_the_client_header() {
    let h = start();

    let (status, _) = send(ureq::get(&format!("{}/recent", h.base)), None);
    assert_eq!(status, 403, "/recent must not serve an unidentified caller");

    let (status, _) = send(
        ureq::post(&format!("{}/save", h.base)),
        Some(r#"{"url":"https://evil.example/x"}"#),
    );
    assert_eq!(status, 403, "/save must not accept an unidentified caller");

    assert_eq!(
        h.vault.stats().unwrap().total,
        0,
        "a refused request must not have written anything"
    );
    assert!(h.events().is_empty());
}

/// The other half of the model: we must never send CORS headers, or the
/// preflight would start succeeding and the header gate would mean nothing.
#[test]
fn no_response_ever_carries_cors_headers() {
    let h = start();
    for path in ["/ping", "/recent", "/nope"] {
        let url = format!("{}{path}", h.base);
        let response = match ureq::get(&url).set(CLIENT_HEADER, "extension").call() {
            Ok(r) => r,
            Err(ureq::Error::Status(_, r)) => r,
            Err(e) => panic!("{e}"),
        };
        for header in response.headers_names() {
            assert!(
                !header.to_ascii_lowercase().starts_with("access-control-"),
                "{path} leaked a CORS header: {header}"
            );
        }
    }
}

#[test]
fn an_identified_client_can_capture_a_page() {
    let h = start();
    let (status, body) = send(
        ureq::post(&format!("{}/save", h.base))
            .set(CLIENT_HEADER, "extension")
            .set("x-websave-ext-version", "1.4.0"),
        Some(r#"{"url":"https://example.com/article","title":"An Article"}"#),
    );

    assert_eq!(status, 200, "{body}");
    assert!(body.contains("An Article"), "{body}");

    let saves = h.vault.list_saves(&ListQuery::default()).unwrap();
    assert_eq!(saves.len(), 1);
    assert_eq!(saves[0].url, "https://example.com/article");

    assert_eq!(
        h.events(),
        vec!["saved:https://example.com/article".to_string()],
        "the host is told, so it can refresh and wake the monitor"
    );

    // The app shows "extension connected" off these.
    assert!(h.vault.get_meta("ext.last_seen").unwrap().is_some());
    assert_eq!(
        h.vault.get_meta("ext.version").unwrap().as_deref(),
        Some("1.4.0")
    );
}

#[test]
fn recent_serves_the_popup_a_slim_payload() {
    let h = start();
    for i in 0..3 {
        h.vault
            .add_save(websave_core::NewSave {
                url: format!("https://example.com/{i}"),
                title: format!("Page {i}"),
                ..Default::default()
            })
            .unwrap();
    }

    let (status, body) = send(
        ureq::get(&format!("{}/recent?limit=2", h.base)).set(CLIENT_HEADER, "extension"),
        None,
    );
    assert_eq!(status, 200);

    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    let items = parsed.as_array().expect("an array");
    assert_eq!(items.len(), 2, "limit is honoured");
    // Only what the popup renders — no notes, no archive text.
    let keys: Vec<&str> = items[0]
        .as_object()
        .unwrap()
        .keys()
        .map(|k| k.as_str())
        .collect();
    assert_eq!(keys, vec!["faviconUrl", "id", "status", "title", "url"]);
}

#[test]
fn a_malformed_body_is_rejected_without_disturbing_the_vault() {
    let h = start();
    let (status, body) = send(
        ureq::post(&format!("{}/save", h.base)).set(CLIENT_HEADER, "extension"),
        Some("{ not json"),
    );
    assert_eq!(status, 400);
    // Must still be parseable JSON — the frontend shows this message.
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(parsed["error"].is_string(), "{body}");
    assert_eq!(h.vault.stats().unwrap().total, 0);
}

#[test]
fn an_invalid_url_is_rejected_with_a_readable_error() {
    let h = start();
    let (status, body) = send(
        ureq::post(&format!("{}/save", h.base)).set(CLIENT_HEADER, "extension"),
        Some(r#"{"url":"javascript:alert(1)"}"#),
    );
    assert_eq!(status, 400);
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(
        parsed["error"].as_str().unwrap().contains("javascript"),
        "{body}"
    );
    assert_eq!(h.vault.stats().unwrap().total, 0);
}

/// These two exist for the macOS menubar app, which writes to the vault
/// directly and then asks the running engine to catch up.
#[test]
fn companion_endpoints_report_to_the_host() {
    let h = start();
    assert_eq!(send(ureq::get(&format!("{}/show", h.base)), None).0, 200);
    assert_eq!(send(ureq::get(&format!("{}/reload", h.base)), None).0, 200);
    assert_eq!(h.events(), vec!["show".to_string(), "reload".to_string()]);
}

#[test]
fn unknown_paths_are_404() {
    let h = start();
    let (status, _) = send(
        ureq::get(&format!("{}/../../etc/passwd", h.base)).set(CLIENT_HEADER, "extension"),
        None,
    );
    assert_eq!(status, 404);
}
