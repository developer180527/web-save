//! Resolve every host in a vault and print the map it would draw:
//! `cargo run -p websave-core --example geo_map -- /path/to/vault`
use std::collections::BTreeSet;
use std::sync::Arc;
use websave_core::{monitor::host_key, GeoLocator, ListQuery, Vault};

fn main() {
    let path = std::env::args().nth(1).expect("usage: geo_map <vault dir>");
    let vault = Arc::new(Vault::open(&path).expect("open vault"));
    let locator = GeoLocator::open_in(vault.assets_dir()).expect("geoip.mmdb in assets/");
    println!("{}\n", locator.describe());

    let hosts: BTreeSet<String> = vault
        .list_saves(&ListQuery::default())
        .unwrap()
        .iter()
        .map(|s| host_key(&s.url))
        .collect();
    let now = 1_800_000_000;
    let mut missed = 0;
    for host in &hosts {
        match locator.locate_host(host, now) {
            Some(l) => vault.set_host_location(&l).unwrap(),
            None => missed += 1,
        }
    }
    println!("{} distinct hosts, {missed} unresolved\n", hosts.len());

    println!("{:<34} {:>5}  {:<22} IP", "HOST", "SAVES", "SERVED FROM");
    for p in vault.map_points().unwrap().iter().take(20) {
        println!(
            "{:<34} {:>5}  {:<22} {}",
            p.host,
            p.save_count,
            format!(
                "{}, {}",
                if p.city.is_empty() { "?" } else { &p.city },
                p.country
            ),
            p.ip
        );
    }
    println!("\nBy country:");
    for (country, n) in vault.hosts_by_country().unwrap() {
        println!("  {country}  {n}");
    }
}
