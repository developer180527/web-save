//! Probe a URL exactly like the link monitor does:
//! `cargo run -p websave-core --example check_url -- https://crates.io/`

fn main() {
    let url = std::env::args()
        .nth(1)
        .expect("usage: check_url <url>");
    let outcome = websave_core::check_url(&url, "");
    println!(
        "{url}\n  status: {:?}\n  http:   {:?}\n  redirect: {:?}",
        outcome.status, outcome.http_status, outcome.redirect_url
    );
}
