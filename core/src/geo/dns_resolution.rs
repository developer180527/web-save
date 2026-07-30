//! Turning a host name into the address that actually served the page.
//!
//! Deliberately uses the platform resolver (`ToSocketAddrs`) rather than
//! speaking DNS ourselves: it honours the machine's own configuration —
//! `/etc/hosts`, split-horizon VPN, corporate resolvers, MDNS — so the
//! address recorded is the one this machine really reaches, which is the only
//! honest answer to "where is this hosted for me".
//!
//! No new information leaves the machine. A link check already resolves and
//! connects to every one of these hosts; this just asks for the answer.

use std::net::{IpAddr, SocketAddr, ToSocketAddrs};

/// Resolve a [`crate::monitor`] host key to a single address.
///
/// The key may carry a port (`example.com:8443`); the port is irrelevant to
/// geolocation but has to be parsed off before the lookup.
///
/// Prefers IPv4 when a host offers both. Not because IPv6 is worse, but
/// because geolocation databases have far better IPv4 coverage — an IPv6
/// answer would frequently geolocate to nothing at all.
///
/// This blocks on the platform resolver; call it off the UI thread.
pub fn resolve_host(host_key: &str) -> Option<IpAddr> {
    let (host, port) = split_port(host_key);
    let addresses: Vec<SocketAddr> = (host, port).to_socket_addrs().ok()?.collect();
    addresses
        .iter()
        .find(|a| a.is_ipv4())
        .or_else(|| addresses.first())
        .map(|a| a.ip())
}

/// `example.com:8443` → `("example.com", 8443)`, `example.com` → 443.
///
/// Care with IPv6 literals: `[::1]:8080` has colons in the host part, so a
/// naive `split_once(':')` would mangle it.
fn split_port(host_key: &str) -> (&str, u16) {
    if let Some(rest) = host_key.strip_prefix('[') {
        if let Some((host, tail)) = rest.split_once(']') {
            let port = tail.strip_prefix(':').and_then(|p| p.parse().ok());
            return (host, port.unwrap_or(443));
        }
    }
    match host_key.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') => (host, port.parse().unwrap_or(443)),
        _ => (host_key, 443),
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_host, split_port};

    #[test]
    fn splits_ports_without_mangling_ipv6() {
        assert_eq!(split_port("example.com"), ("example.com", 443));
        assert_eq!(split_port("example.com:8443"), ("example.com", 8443));
        assert_eq!(split_port("[::1]:8080"), ("::1", 8080));
        assert_eq!(split_port("[::1]"), ("::1", 443));
        // A bare IPv6 literal has no port to find.
        assert_eq!(split_port("::1").0, "::1");
    }

    /// Uses localhost so the test needs no network and no DNS server.
    #[test]
    fn resolves_a_local_name() {
        let ip = resolve_host("localhost").expect("localhost resolves");
        assert!(ip.is_loopback(), "got {ip}");
        assert_eq!(
            resolve_host("127.0.0.1:38917"),
            Some("127.0.0.1".parse().unwrap())
        );
    }

    #[test]
    fn unresolvable_hosts_are_none() {
        assert_eq!(resolve_host("no-such-host.invalid"), None);
    }
}
