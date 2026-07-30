//! Which network actually serves an address.
//!
//! This is what makes the map honest. Geolocating a CDN address produces a
//! coordinate, but for an anycast range that coordinate is close to
//! arbitrary — one address answers from dozens of cities and the database can
//! only name one. On a real 40-host vault, 17 hosts plotted onto "Toronto"
//! for no reason other than being behind Cloudflare.
//!
//! The autonomous system is a fact rather than an estimate: that address
//! genuinely is announced by AS13335, and AS13335 genuinely is Cloudflare. So
//! "Cloudflare · 17 sites" is true where "Toronto · 17 sites" is not, and it
//! is also the more interesting statement — how much of someone's reading
//! passes through a handful of companies.
//!
//! Uses a second, separate database (~5 MB) because ASN and city data ship as
//! different files.

use std::net::IpAddr;
use std::path::Path;

use maxminddb::{geoip2, Reader};

/// The network announcing an address.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Network {
    /// Autonomous system number, e.g. 13335.
    pub asn: Option<u32>,
    /// Registered organisation, e.g. `CLOUDFLARENET`. Empty when unknown.
    pub organization: String,
}

impl Network {
    pub fn is_known(&self) -> bool {
        self.asn.is_some() || !self.organization.is_empty()
    }
}

/// An opened ASN database.
pub struct AsnLookup {
    reader: Reader<Vec<u8>>,
}

impl AsnLookup {
    pub fn open(path: impl AsRef<Path>) -> Result<AsnLookup, String> {
        let path = path.as_ref();
        Reader::open_readfile(path)
            .map(|reader| AsnLookup { reader })
            .map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn describe(&self) -> String {
        let meta = self.reader.metadata();
        format!("{} · {} entries", meta.database_type, meta.node_count)
    }

    pub fn lookup(&self, ip: IpAddr) -> Option<Network> {
        let record = self
            .reader
            .lookup(ip)
            .ok()?
            .decode::<geoip2::Asn>()
            .ok()??;
        let network = Network {
            asn: record.autonomous_system_number,
            organization: record
                .autonomous_system_organization
                .unwrap_or_default()
                .to_string(),
        };
        network.is_known().then_some(network)
    }
}

/// Networks whose addresses are anycast, so their coordinates mean little.
///
/// Not an exhaustive registry and not meant to be — it covers the providers
/// that actually dominate a bookmark collection. A host on one of these is
/// still plotted (the dot is where *this* machine reaches it, which is a real
/// thing) but the UI can say so instead of implying the site lives there.
///
/// Matched on the organisation string because ASNs are numerous per provider:
/// Amazon alone announces dozens.
const ANYCAST_HINTS: &[&str] = &[
    "cloudflare",
    "fastly",
    "akamai",
    "amazon",
    "google",
    "microsoft",
    "azure",
    "vercel",
    "netlify",
    "bunny",
    "stackpath",
    "cdn77",
    "incapsula",
    "imperva",
    "edgecast",
    "limelight",
    "sucuri",
    "ovh",
];

/// Whether this organisation is likely serving from an anycast network.
pub fn is_likely_anycast(organization: &str) -> bool {
    let lower = organization.to_ascii_lowercase();
    ANYCAST_HINTS.iter().any(|hint| lower.contains(hint))
}

#[cfg(test)]
mod tests {
    use super::{is_likely_anycast, Network};

    #[test]
    fn recognises_the_big_edge_networks() {
        for org in [
            "CLOUDFLARENET",
            "Fastly, Inc.",
            "AKAMAI-AS",
            "AMAZON-02",
            "GOOGLE",
            "MICROSOFT-CORP-MSN-AS-BLOCK",
        ] {
            assert!(is_likely_anycast(org), "{org} should be flagged");
        }
    }

    #[test]
    fn leaves_ordinary_hosting_alone() {
        for org in ["Hetzner Online GmbH", "DigitalOcean, LLC", "Linode", ""] {
            assert!(!is_likely_anycast(org), "{org} should not be flagged");
        }
    }

    #[test]
    fn a_network_needs_a_number_or_a_name() {
        assert!(!Network::default().is_known());
        assert!(Network {
            asn: Some(13335),
            organization: String::new()
        }
        .is_known());
        assert!(Network {
            asn: None,
            organization: "CLOUDFLARENET".into()
        }
        .is_known());
    }
}
