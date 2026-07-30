//! Where the pages in a vault are served from.
//!
//! Two steps, one per file: resolve a host to an address
//! ([`dns_resolution`]), then look that address up in an offline database
//! ([`ip_geolocation`]). Both are per **host**, never per save — a 10k-save
//! vault is a few hundred hosts, which is the same observation that makes the
//! link sweep affordable.
//!
//! ## What this actually measures
//!
//! Geolocation of a web host tells you where the *bytes* come from, not where
//! the site or its authors are. Most of a modern bookmark collection sits
//! behind Cloudflare, Fastly, AWS or Akamai, so expect dots to pile up on a
//! handful of datacenter towns. Anycast makes it fuzzier still: one address
//! is answered from dozens of cities at once, and the database can only name
//! one of them.
//!
//! That is worth surfacing rather than hiding — "how much of my reading is
//! served by three companies" is a real answer — but the UI should say
//! *served from*, never *located in*.

mod dns_resolution;
mod ip_geolocation;

pub use dns_resolution::resolve_host;
pub use ip_geolocation::{IpGeolocator, IpLocation};

use std::path::Path;

/// The filename the app looks for inside the vault's assets directory.
///
/// Kept in the vault rather than beside the binary so it travels with the
/// user's data, and so an app update never silently discards a 150 MB
/// download.
pub const GEO_DB_FILE: &str = "geoip.mmdb";

/// A resolved host: address plus place.
#[derive(Debug, Clone, PartialEq)]
pub struct HostLocation {
    /// The [`crate::monitor`] host key this describes.
    pub host: String,
    pub ip: String,
    pub country: String,
    pub city: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    /// Unix seconds this was looked up.
    pub resolved_at: i64,
    /// How many saves live on this host. Only populated by queries that
    /// aggregate for the map; zero when a single host is resolved.
    pub save_count: i64,
}

/// Resolves hosts to places. Holds the geolocation database open.
pub struct GeoLocator {
    geolocator: IpGeolocator,
}

impl GeoLocator {
    /// Open the database stored in a vault's assets directory.
    ///
    /// Returns `Err` when no database has been downloaded yet, which is the
    /// normal state on a fresh install — the feature is opt-in because the
    /// database is a large third-party file the user chooses to fetch.
    pub fn open_in(assets_dir: impl AsRef<Path>) -> Result<GeoLocator, String> {
        GeoLocator::open(assets_dir.as_ref().join(GEO_DB_FILE))
    }

    pub fn open(path: impl AsRef<Path>) -> Result<GeoLocator, String> {
        Ok(GeoLocator {
            geolocator: IpGeolocator::open(path)?,
        })
    }

    pub fn describe(&self) -> String {
        self.geolocator.describe()
    }

    /// Resolve one host and place it. `None` when DNS fails, the address is
    /// local, or the database has no entry.
    ///
    /// Blocking; call it off the UI thread.
    pub fn locate_host(&self, host: &str, now: i64) -> Option<HostLocation> {
        let ip = resolve_host(host)?;
        let place = self.geolocator.locate(ip)?;
        Some(HostLocation {
            host: host.to_string(),
            ip: ip.to_string(),
            country: place.country,
            city: place.city,
            latitude: place.latitude,
            longitude: place.longitude,
            resolved_at: now,
            save_count: 0,
        })
    }
}

/// How long a host's location is trusted before it is looked up again.
///
/// Hosting moves, but slowly, and a re-resolve costs a DNS round trip on a
/// host the sweep is about to contact anyway. A month keeps the map honest
/// without adding meaningful work.
pub const LOCATION_MAX_AGE_SECS: i64 = 30 * 24 * 60 * 60;

/// Whether a stored location is old enough to redo.
pub fn is_stale(resolved_at: i64, now: i64) -> bool {
    now - resolved_at >= LOCATION_MAX_AGE_SECS
}

#[cfg(test)]
mod tests {
    use super::{is_stale, LOCATION_MAX_AGE_SECS};

    #[test]
    fn locations_expire_after_a_month() {
        let now = 1_800_000_000;
        assert!(!is_stale(now, now));
        assert!(!is_stale(now - LOCATION_MAX_AGE_SECS + 60, now));
        assert!(is_stale(now - LOCATION_MAX_AGE_SECS, now));
        // A never-resolved host (0) is always stale.
        assert!(is_stale(0, now));
    }
}
