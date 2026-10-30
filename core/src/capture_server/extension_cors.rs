//! CORS for browser extensions, and only for browser extensions.
//!
//! Chrome lets an extension reach any host in its `host_permissions` without
//! CORS. Firefox does not extend that to loopback: an extension's request to
//! 127.0.0.1 is held to CORS like a web page's, so with no CORS headers every
//! capture from Firefox failed with a bare "NetworkError" even though the
//! request reached this server and was answered. (Measured against Firefox
//! 159: a loopback response without `Access-Control-Allow-Origin` was
//! discarded every time, and the same response with it was accepted every
//! time.)
//!
//! So responses to an extension carry CORS headers naming that extension's
//! origin, and its preflights are answered. This does not weaken the gate in
//! [`super::client_gate`] for web pages: the browser sets `Origin` itself and
//! a page cannot claim an extension scheme, so a page still gets no CORS
//! headers, its preflight is still refused, and it still cannot send the
//! client header. Anything outside a browser could already call the server
//! directly; CORS never constrained it.

use tiny_http::{Header, Request, Response};

use super::client_gate::{header_value, CLIENT_HEADER, EXT_VERSION_HEADER};
use super::endpoints::Reply;

/// Origins only a browser extension can have.
const EXTENSION_SCHEMES: [&str; 3] = [
    "moz-extension://",
    "chrome-extension://",
    "safari-web-extension://",
];

/// The request's origin, if it is a browser extension's.
pub(super) fn extension_origin(request: &Request) -> Option<String> {
    let origin = header_value(request, "Origin")?;
    let is_extension = EXTENSION_SCHEMES
        .iter()
        .any(|scheme| origin.len() > scheme.len() && origin.starts_with(scheme));
    // Echoed back into a header below, so anything that is not a plain
    // `scheme://id` is refused rather than reflected.
    let well_formed = origin
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"-:/._".contains(&b));
    (is_extension && well_formed).then_some(origin)
}

/// Allow `origin` to read this response.
pub(super) fn allow(reply: Reply, origin: &str) -> Reply {
    reply
        .with_header(header("Access-Control-Allow-Origin", origin))
        // The answer differs by Origin, so no cache may reuse it for another.
        .with_header(header("Vary", "Origin"))
}

/// Answer an extension's preflight. A capture is a JSON POST carrying the
/// client header, which makes the browser ask first.
///
/// Leaves out the allowed origin: the server adds [`allow`] to every reply
/// to an extension, this one included, and a browser rejects the header if
/// it appears twice.
pub(super) fn preflight() -> Reply {
    let allowed_headers = format!("content-type, {CLIENT_HEADER}, {EXT_VERSION_HEADER}");
    Response::from_data(Vec::new())
        .with_status_code(204)
        .with_header(header("Access-Control-Allow-Methods", "GET, POST"))
        .with_header(header("Access-Control-Allow-Headers", &allowed_headers))
        .with_header(header("Access-Control-Max-Age", "600"))
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("validated ASCII header")
}
