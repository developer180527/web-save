//! Turning an IP address into a place, offline.
//!
//! Reads a MaxMind-format database (`.mmdb`) from disk. Offline is the point:
//! an online geo-IP API would mean shipping every host the user has ever
//! bookmarked to a third party, one request at a time, which is precisely the
//! browsing history this app exists to keep local. It would also need a key
//! and a rate limit budget.
//!
//! Two shapes of database are understood, and they give different answers:
//!
//! - **City** databases carry latitude and longitude, so saves can be plotted.
//! - **Country** databases carry only an ISO code. Those still produce a
//!   per-country breakdown, but no dots — rather than invent a centroid and
//!   present a guess as a measurement.
//!
//! The format is the same either way, so a user can start on the small
//! country database and drop in a city one later without any migration.

use std::net::IpAddr;
use std::path::Path;

use maxminddb::{geoip2, Reader};

/// Where an address appears to be.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct IpLocation {
    /// ISO 3166-1 alpha-2, e.g. `US`. Empty when the database has no answer.
    pub country: String,
    /// Empty for country-level databases.
    pub city: String,
    /// Only present in city-level databases.
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

impl IpLocation {
    /// Whether this can be drawn on a map.
    pub fn is_plottable(&self) -> bool {
        self.latitude.is_some() && self.longitude.is_some()
    }
}

/// An opened geolocation database.
pub struct IpGeolocator {
    reader: Reader<Vec<u8>>,
}

impl IpGeolocator {
    /// Open a `.mmdb` file. The whole database is read into memory — 4 MB for
    /// a country database, ~150 MB for a city one — so open it once and keep
    /// it.
    pub fn open(path: impl AsRef<Path>) -> Result<IpGeolocator, String> {
        let path = path.as_ref();
        Reader::open_readfile(path)
            .map(|reader| IpGeolocator { reader })
            .map_err(|e| format!("{}: {e}", path.display()))
    }

    /// A one-line description of the loaded database, for Settings.
    pub fn describe(&self) -> String {
        let meta = self.reader.metadata();
        format!(
            "{} · {} entries · built {}",
            meta.database_type, meta.node_count, meta.build_epoch
        )
    }

    /// Whether this database can produce coordinates, or only country codes.
    /// Lets Settings explain why the map has no dots.
    pub fn has_coordinates(&self) -> bool {
        self.reader
            .metadata()
            .database_type
            .to_ascii_lowercase()
            .contains("city")
    }

    /// Look an address up.
    ///
    /// Private and reserved ranges return `None` rather than an empty record:
    /// a host on the LAN has no meaningful position on a world map, and
    /// plotting it at (0, 0) would put a dot in the Gulf of Guinea.
    pub fn locate(&self, ip: IpAddr) -> Option<IpLocation> {
        if !is_globally_routable(ip) {
            return None;
        }
        // One decode path covers both database shapes: every field of the
        // City record defaults, so a country database simply yields a record
        // whose city and coordinates are absent.
        let record = self
            .reader
            .lookup(ip)
            .ok()?
            .decode::<geoip2::City>()
            .ok()??;
        let location = IpLocation {
            country: record.country.iso_code.unwrap_or_default().to_string(),
            city: record.city.names.english.unwrap_or_default().to_string(),
            latitude: record.location.latitude,
            longitude: record.location.longitude,
        };
        // An address the database simply doesn't know is not a location.
        (!location.country.is_empty() || location.is_plottable()).then_some(location)
    }
}

/// Whether an address is one the public internet could route to.
///
/// `IpAddr::is_global` is still unstable, so the cases that actually turn up
/// in a bookmark vault are checked directly: loopback for local dev servers,
/// private ranges for intranet pages, link-local, and the unspecified address.
fn is_globally_routable(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            !(v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_unspecified()
                // 100.64.0.0/10, carrier-grade NAT.
                || (v4.octets()[0] == 100 && (64..128).contains(&v4.octets()[1])))
        }
        IpAddr::V6(v6) => {
            !(v6.is_loopback()
                || v6.is_unspecified()
                // fc00::/7 unique-local, fe80::/10 link-local.
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || (v6.segments()[0] & 0xffc0) == 0xfe80)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{is_globally_routable, IpLocation};

    #[test]
    fn local_addresses_are_never_plotted() {
        for local in [
            "127.0.0.1",
            "10.0.0.5",
            "192.168.1.20",
            "172.16.4.1",
            "169.254.1.1",
            "0.0.0.0",
            "100.64.0.1",
            "::1",
            "fe80::1",
            "fd00::1",
        ] {
            assert!(
                !is_globally_routable(local.parse().unwrap()),
                "{local} should not be geolocated"
            );
        }
    }

    #[test]
    fn public_addresses_are_routable() {
        for public in ["1.1.1.1", "8.8.8.8", "93.184.216.34", "2606:4700::1111"] {
            assert!(
                is_globally_routable(public.parse().unwrap()),
                "{public} should be geolocated"
            );
        }
    }

    #[test]
    fn only_records_with_coordinates_are_plottable() {
        assert!(!IpLocation::default().is_plottable());
        assert!(!IpLocation {
            country: "US".into(),
            ..Default::default()
        }
        .is_plottable());
        assert!(IpLocation {
            country: "US".into(),
            latitude: Some(39.0),
            longitude: Some(-77.4),
            ..Default::default()
        }
        .is_plottable());
    }
}
