//! Storing and aggregating where hosts are served from.
//!
//! No geolocation database is needed here: these test the storage and
//! counting, with locations supplied directly. The database-backed lookup is
//! exercised by `cargo run --example geo_probe -- <db.mmdb>`, since it needs
//! a ~130 MB third-party file no test should depend on.

use websave_core::{GeoLocator, HostLocation, NewSave, Vault};

fn vault() -> (tempfile::TempDir, Vault) {
    let dir = tempfile::tempdir().unwrap();
    let vault = Vault::open(dir.path().join("vault")).unwrap();
    (dir, vault)
}

fn save(vault: &Vault, url: &str) {
    vault
        .add_save(NewSave {
            url: url.into(),
            ..Default::default()
        })
        .unwrap();
}

fn located(host: &str, city: &str, lat: f64, lon: f64) -> HostLocation {
    HostLocation {
        host: host.into(),
        ip: "203.0.113.7".into(),
        country: "US".into(),
        city: city.into(),
        latitude: Some(lat),
        longitude: Some(lon),
        asn: Some(13335),
        asn_org: "CLOUDFLARENET".into(),
        resolved_at: 1_800_000_000,
        save_count: 0,
    }
}

#[test]
fn a_location_round_trips_and_refreshes_in_place() {
    let (_dir, vault) = vault();
    vault
        .set_host_location(&located("example.com", "Ashburn", 39.04, -77.48))
        .unwrap();
    assert_eq!(vault.located_host_count().unwrap(), 1);
    assert_eq!(
        vault.host_location_age("example.com").unwrap(),
        Some(1_800_000_000)
    );

    // Re-resolving the same host updates rather than duplicating.
    let mut moved = located("example.com", "Frankfurt", 50.11, 8.68);
    moved.resolved_at = 1_900_000_000;
    vault.set_host_location(&moved).unwrap();

    assert_eq!(vault.located_host_count().unwrap(), 1);
    assert_eq!(
        vault.host_location_age("example.com").unwrap(),
        Some(1_900_000_000)
    );
    assert_eq!(vault.map_points().unwrap()[0].city, "Frankfurt");
}

#[test]
fn unknown_hosts_have_no_age() {
    let (_dir, vault) = vault();
    assert_eq!(vault.host_location_age("never-seen.example").unwrap(), None);
}

/// The map counts saves per host, so a busy host draws a bigger dot.
#[test]
fn map_points_count_saves_and_order_by_busiest() {
    let (_dir, vault) = vault();
    for path in ["a", "b", "c"] {
        save(&vault, &format!("https://busy.example/{path}"));
    }
    save(&vault, "https://quiet.example/only");

    vault
        .set_host_location(&located("busy.example", "Ashburn", 39.04, -77.48))
        .unwrap();
    vault
        .set_host_location(&located("quiet.example", "Paris", 48.85, 2.35))
        .unwrap();

    let points = vault.map_points().unwrap();
    assert_eq!(points.len(), 2);
    assert_eq!(
        (points[0].host.as_str(), points[0].save_count),
        ("busy.example", 3)
    );
    assert_eq!(
        (points[1].host.as_str(), points[1].save_count),
        ("quiet.example", 1)
    );
}

/// Regression: the save/host join must use `monitor::host_key`, the same rule
/// the sweep groups by. An earlier SQL version used `LTRIM(host, 'wW.')` to
/// strip `www.`, which strips a character *set* — so every domain starting
/// with w lost letters (`wikipedia.org` → `ikipedia.org`) and silently
/// counted zero saves.
#[test]
fn hosts_beginning_with_w_are_counted_correctly() {
    let (_dir, vault) = vault();
    save(&vault, "https://wikipedia.org/wiki/Rust");
    save(&vault, "https://wikipedia.org/wiki/SQLite");
    save(&vault, "https://west.io/blog");
    // `www.` really is folded away, and folds onto the bare domain.
    save(&vault, "https://www.wired.com/story/one");
    save(&vault, "https://wired.com/story/two");

    for host in ["wikipedia.org", "west.io", "wired.com"] {
        vault
            .set_host_location(&located(host, "Somewhere", 1.0, 2.0))
            .unwrap();
    }

    let counts: std::collections::HashMap<_, _> = vault
        .map_points()
        .unwrap()
        .into_iter()
        .map(|p| (p.host, p.save_count))
        .collect();
    assert_eq!(counts["wikipedia.org"], 2, "leading 'w' was eaten");
    assert_eq!(counts["west.io"], 1, "leading 'w' was eaten");
    assert_eq!(
        counts["wired.com"], 2,
        "www. should fold onto the bare host"
    );
}

/// A country-level database gives no coordinates. Those hosts must not be
/// plotted at (0, 0) — a dot in the Gulf of Guinea is worse than no dot — but
/// they still count towards the country breakdown.
#[test]
fn hosts_without_coordinates_are_counted_but_not_plotted() {
    let (_dir, vault) = vault();
    save(&vault, "https://plotted.example/x");
    save(&vault, "https://unplotted.example/y");

    vault
        .set_host_location(&located("plotted.example", "Paris", 48.85, 2.35))
        .unwrap();
    vault
        .set_host_location(&HostLocation {
            host: "unplotted.example".into(),
            country: "JP".into(),
            latitude: None,
            longitude: None,
            ..located("unplotted.example", "", 0.0, 0.0)
        })
        .unwrap();

    let points = vault.map_points().unwrap();
    assert_eq!(points.len(), 1, "only the host with coordinates is drawn");
    assert_eq!(points[0].host, "plotted.example");

    // Both still show up in the country list.
    assert_eq!(vault.located_host_count().unwrap(), 2);
    let countries = vault.hosts_by_country().unwrap();
    assert_eq!(countries.len(), 2);
    assert!(countries.iter().any(|(c, n)| c == "JP" && *n == 1));
}

#[test]
fn countries_are_ranked_by_host_count() {
    let (_dir, vault) = vault();
    for (i, country) in ["US", "US", "US", "DE", "DE", "FR"].iter().enumerate() {
        vault
            .set_host_location(&HostLocation {
                host: format!("h{i}.example"),
                country: (*country).into(),
                ..located("x", "", 1.0, 2.0)
            })
            .unwrap();
    }
    let countries = vault.hosts_by_country().unwrap();
    assert_eq!(
        countries,
        vec![("US".into(), 3), ("DE".into(), 2), ("FR".into(), 1)]
    );
}

/// Swapping a country database for a city one should refill coordinates,
/// which means dropping what was learned under the old one.
#[test]
fn locations_can_be_cleared_wholesale() {
    let (_dir, vault) = vault();
    vault
        .set_host_location(&located("example.com", "Ashburn", 39.04, -77.48))
        .unwrap();
    vault.clear_host_locations().unwrap();
    assert_eq!(vault.located_host_count().unwrap(), 0);
    assert!(vault.map_points().unwrap().is_empty());
}

/// The feature is opt-in: with no database downloaded, opening the locator
/// fails cleanly rather than panicking or silently doing nothing.
#[test]
fn a_missing_database_is_a_clean_error() {
    let (dir, vault) = vault();
    let Err(error) = GeoLocator::open_in(vault.assets_dir()) else {
        panic!("expected opening a non-existent database to fail");
    };
    assert!(error.contains("geoip.mmdb"), "{error}");
    drop(dir);
}

/// The point of the ASN database: a truthful grouping to sit beside the dots.
#[test]
fn hosts_are_grouped_by_serving_network() {
    let (_dir, vault) = vault();
    for (i, org) in ["CLOUDFLARENET", "CLOUDFLARENET", "Fastly, Inc.", ""]
        .iter()
        .enumerate()
    {
        vault
            .set_host_location(&HostLocation {
                host: format!("h{i}.example"),
                asn_org: (*org).into(),
                ..located("x", "Somewhere", 1.0, 2.0)
            })
            .unwrap();
    }

    let providers = vault.hosts_by_provider().unwrap();
    assert_eq!(
        providers,
        vec![("CLOUDFLARENET".into(), 2), ("Fastly, Inc.".into(), 1)],
        "hosts with no known network are left out rather than bucketed as ''"
    );
}

/// Without an ASN database installed the map still works; it just cannot name
/// the provider.
#[test]
fn a_missing_asn_database_leaves_locations_usable() {
    let (_dir, vault) = vault();
    save(&vault, "https://example.com/x");
    vault
        .set_host_location(&HostLocation {
            asn: None,
            asn_org: String::new(),
            ..located("example.com", "Paris", 48.85, 2.35)
        })
        .unwrap();

    let points = vault.map_points().unwrap();
    assert_eq!(points.len(), 1);
    assert_eq!(points[0].save_count, 1);
    assert!(points[0].asn_org.is_empty());
    assert!(vault.hosts_by_provider().unwrap().is_empty());
}
