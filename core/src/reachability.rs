//! Which hosts are unreachable *by design* from wherever we happen to be.
//!
//! A link checker that can't tell "this page is gone" from "this page lives on
//! a network I'm not currently attached to" will condemn a perfectly healthy
//! Tailscale, LAN or VPN bookmark the moment you step off that network. Those
//! saves must never be treated as rot, because the failure says nothing about
//! the resource — only about our vantage point.
//!
//! This module answers the narrow question *"is this host private?"* purely
//! from the URL, with no DNS lookups: the check has to be cheap, offline-safe
//! and identical on every platform.

/// Suffixes that are private or non-public by definition.
///
/// `.ts.net` is Tailscale's MagicDNS domain — the case that motivated this
/// module. The rest are the standard private-use and mDNS namespaces, plus
/// the conventional homelab TLDs people actually type.
const PRIVATE_SUFFIXES: &[&str] = &[
    ".ts.net",     // Tailscale MagicDNS
    ".local",      // mDNS / Bonjour
    ".localdomain",
    ".internal",   // RFC 8375-adjacent convention, also cloud-internal DNS
    ".intranet",
    ".private",
    ".corp",
    ".lan",
    ".home",
    ".home.arpa", // RFC 8375
    ".in-addr.arpa",
    ".ip6.arpa",
    ".onion",     // Tor: only resolvable through a Tor proxy
    ".i2p",
];

/// Why a host is considered private. Surfaced to the UI so the app can
/// explain itself instead of just showing a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivateHost {
    /// A Tailscale MagicDNS name (`*.ts.net`) or CGNAT-range address.
    Tailscale,
    /// mDNS/Bonjour (`*.local`) or another private-use suffix.
    LocalDomain,
    /// This machine (`localhost`, `127.0.0.0/8`, `::1`).
    Loopback,
    /// An RFC 1918 / ULA / link-local address literal.
    PrivateAddress,
    /// A bare hostname with no dot (`http://nas/`) — only resolvable via
    /// local DNS or a hosts file.
    SingleLabel,
}

impl PrivateHost {
    /// A short human explanation, for status text and tooltips.
    pub fn reason(self) -> &'static str {
        match self {
            PrivateHost::Tailscale => "on your Tailscale network",
            PrivateHost::LocalDomain => "on a local network domain",
            PrivateHost::Loopback => "on this machine",
            PrivateHost::PrivateAddress => "on a private network address",
            PrivateHost::SingleLabel => "a local hostname",
        }
    }
}

/// Classify a URL's host as private, or `None` if it looks publicly routable.
///
/// Errs toward "public": misjudging a public host as private only costs us
/// the ability to auto-purge it, while the reverse deletes someone's data.
pub fn private_host(url: &str) -> Option<PrivateHost> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?.trim_end_matches('.').to_ascii_lowercase();
    classify_host(&host)
}

/// The host-string half of [`private_host`], split out for testing and for
/// callers that already parsed the URL.
pub fn classify_host(host: &str) -> Option<PrivateHost> {
    if host.is_empty() {
        return None;
    }
    if host == "localhost" || host.ends_with(".localhost") {
        return Some(PrivateHost::Loopback);
    }
    if let Some(kind) = private_ip(host) {
        return Some(kind);
    }
    for suffix in PRIVATE_SUFFIXES {
        if host.ends_with(suffix) {
            return Some(if *suffix == ".ts.net" {
                PrivateHost::Tailscale
            } else {
                PrivateHost::LocalDomain
            });
        }
    }
    // No dot at all: `http://nas/`, `http://router/`. Resolvable only through
    // local DNS, a hosts file or NetBIOS.
    if !host.contains('.') && !host.contains(':') {
        return Some(PrivateHost::SingleLabel);
    }
    None
}

/// Classify an address literal. Handles IPv4, and IPv6 in either bare or
/// bracketed form.
fn private_ip(host: &str) -> Option<PrivateHost> {
    let bare = host.trim_start_matches('[').trim_end_matches(']');

    if let Ok(v4) = bare.parse::<std::net::Ipv4Addr>() {
        let [a, b, ..] = v4.octets();
        return if v4.is_loopback() {
            Some(PrivateHost::Loopback)
        } else if v4.is_private() || v4.is_link_local() {
            // 10/8, 172.16/12, 192.168/16, 169.254/16
            Some(PrivateHost::PrivateAddress)
        } else if a == 100 && (64..=127).contains(&b) {
            // 100.64.0.0/10 — CGNAT, which is also Tailscale's address range.
            Some(PrivateHost::Tailscale)
        } else {
            None
        };
    }

    if let Ok(v6) = bare.parse::<std::net::Ipv6Addr>() {
        let first = v6.segments()[0];
        return if v6.is_loopback() {
            Some(PrivateHost::Loopback)
        } else if (first & 0xfe00) == 0xfc00 || (first & 0xffc0) == 0xfe80 {
            // fc00::/7 unique-local, fe80::/10 link-local
            Some(PrivateHost::PrivateAddress)
        } else {
            None
        };
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tailscale_hosts_are_private() {
        // The bug that motivated this module: a healthy Tailscale service was
        // being marked dead (and put on a 30-day deletion timer) whenever the
        // checker ran off the tailnet.
        assert_eq!(
            private_host("https://vault-server.taild29644.ts.net:9443/"),
            Some(PrivateHost::Tailscale)
        );
        assert_eq!(
            private_host("http://100.101.102.103/admin"),
            Some(PrivateHost::Tailscale),
            "CGNAT range is Tailscale's address space"
        );
    }

    #[test]
    fn local_and_private_networks_are_private() {
        for (url, expected) in [
            ("http://nas.local/", PrivateHost::LocalDomain),
            ("https://git.internal/repo", PrivateHost::LocalDomain),
            ("http://printer.lan", PrivateHost::LocalDomain),
            ("http://box.home.arpa", PrivateHost::LocalDomain),
            ("http://localhost:3000/", PrivateHost::Loopback),
            ("http://127.0.0.1:8080", PrivateHost::Loopback),
            ("http://[::1]:8080", PrivateHost::Loopback),
            ("http://192.168.1.10/", PrivateHost::PrivateAddress),
            ("http://10.0.0.5/", PrivateHost::PrivateAddress),
            ("http://172.16.4.4/", PrivateHost::PrivateAddress),
            ("http://169.254.1.1/", PrivateHost::PrivateAddress),
            ("http://[fd00::1]/", PrivateHost::PrivateAddress),
            ("http://nas/", PrivateHost::SingleLabel),
        ] {
            assert_eq!(private_host(url), Some(expected), "{url}");
        }
    }

    #[test]
    fn public_hosts_are_not_private() {
        for url in [
            "https://example.com/page",
            "https://news.ycombinator.com/",
            "https://sub.domain.co.uk/x",
            "http://8.8.8.8/",
            "https://172.32.0.1/", // just outside 172.16/12
            "https://100.128.0.1/", // just outside the CGNAT range
        ] {
            assert_eq!(private_host(url), None, "{url}");
        }
    }

    #[test]
    fn trailing_dot_and_case_are_normalized() {
        assert_eq!(
            private_host("https://Server.TS.NET./"),
            Some(PrivateHost::Tailscale)
        );
    }
}
