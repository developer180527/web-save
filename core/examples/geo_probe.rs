//! Check a geolocation database against real hosts:
//! `cargo run -p websave-core --example geo_probe -- /path/to/geoip.mmdb`
fn main() {
    let path = std::env::args().nth(1).expect("usage: geo_probe <db.mmdb>");
    let locator = websave_core::GeoLocator::open(&path).expect("open db");
    println!("{}\n", locator.describe());
    let now = 1_800_000_000;
    for host in [
        "github.com",
        "rust-lang.org",
        "bbc.co.uk",
        "lemonde.fr",
        "abc.net.au",
        "yandex.ru",
        "asahi.com",
        "localhost",
    ] {
        match locator.locate_host(host, now) {
            Some(l) => println!(
                "{host:<16} {:<16} {:<3} {:<14} {:?}",
                l.ip,
                l.country,
                if l.city.is_empty() {
                    "-".into()
                } else {
                    l.city
                },
                l.latitude.zip(l.longitude)
            ),
            None => println!("{host:<16} (no location)"),
        }
    }
}
