//! The rule that stops arbitrary web pages from reaching the vault.
//!
//! The server binds loopback only, never sends CORS headers to a web page,
//! and requires a custom `x-websave-client` header on every request that
//! touches vault data.
//!
//! That combination is the whole security model, and it works because of how
//! browsers treat custom headers: adding one makes the request non-simple, so
//! the browser must send a CORS preflight first. A web page's preflight is
//! refused and its responses carry no CORS headers, so the browser refuses
//! the real request. A page on `evil.example` therefore cannot read or write
//! the vault even though the port is open on the user's own machine.
//!
//! Browser extensions are the one exception, and only by their unforgeable
//! extension origin: see [`super::extension_cors`]. Chrome would let them
//! through regardless; Firefox needs the CORS headers.
//!
//! `/ping` is deliberately ungated: it carries no vault data, and the
//! extension needs it to detect whether the app is running.

use tiny_http::{Header, Request};

pub(super) const CLIENT_HEADER: &str = "x-websave-client";
pub(super) const EXT_VERSION_HEADER: &str = "x-websave-ext-version";

/// Which client sent this request, if it identified itself at all.
///
/// `None` means the request must be refused — see the module docs.
pub(super) fn identify(request: &Request) -> Option<String> {
    header_value(request, CLIENT_HEADER)
}

/// The extension's version, when it sent one.
pub(super) fn extension_version(request: &Request) -> Option<String> {
    header_value(request, EXT_VERSION_HEADER)
}

/// Case-insensitive header lookup. (`Header::equiv` wants a `&'static str`,
/// so field names are compared directly instead.)
pub(super) fn header_value(request: &Request, name: &str) -> Option<String> {
    request
        .headers()
        .iter()
        .find(|h: &&Header| h.field.as_str().as_str().eq_ignore_ascii_case(name))
        .map(|h| h.value.as_str().to_string())
}
