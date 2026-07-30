//! Splits a batch of targets into one work queue per host.
//!
//! This is what lets a sweep be parallel and polite at the same time. The
//! unit of work handed to a worker thread is a whole host, so everything
//! queued for one server is checked by one thread in sequence while different
//! servers proceed at once.
//!
//! The alternative — one shared queue of links plus a lock per host — parks
//! every worker on the same lock the moment a vault holds 300 pages from one
//! domain. Grouping removes the contention instead of managing it.

use std::collections::HashMap;

use crate::probe::CheckTarget;

/// Everything a sweep has to check on one host, in due order.
pub(super) struct HostQueue {
    /// The grouping key: hostname, plus port when the URL names one.
    pub host: String,
    pub targets: Vec<CheckTarget>,
}

/// Group targets by host, preserving each host's relative order.
pub(super) fn group_by_host(targets: Vec<CheckTarget>) -> Vec<HostQueue> {
    let mut groups: HashMap<String, Vec<CheckTarget>> = HashMap::new();
    for target in targets {
        groups
            .entry(host_key(&target.url))
            .or_default()
            .push(target);
    }
    groups
        .into_iter()
        .map(|(host, targets)| HostQueue { host, targets })
        .collect()
}

/// The identity a host is paced and circuit-broken under.
///
/// Port is included because a different port is a different server. `www.` is
/// stripped and the host lower-cased because `www.example.com` and
/// `example.com` are, virtually always, the one machine — pacing them
/// separately would double the rate we hit it.
pub(super) fn host_key(url: &str) -> String {
    let Ok(parsed) = url::Url::parse(url) else {
        // Unparseable URLs each get their own key rather than sharing one
        // bucket, so a handful of malformed saves aren't forced into a single
        // serialized queue.
        return url.to_string();
    };
    match parsed.host_str() {
        Some(host) => {
            let host = host.trim_start_matches("www.").to_ascii_lowercase();
            match parsed.port() {
                Some(port) => format!("{host}:{port}"),
                None => host,
            }
        }
        None => url.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{group_by_host, host_key};
    use crate::probe::CheckTarget;

    fn targets(urls: &[&str]) -> Vec<CheckTarget> {
        urls.iter()
            .enumerate()
            .map(|(i, url)| CheckTarget {
                id: i as i64,
                url: (*url).to_string(),
                ..Default::default()
            })
            .collect()
    }

    #[test]
    fn one_queue_per_host_preserving_order() {
        let groups = group_by_host(targets(&[
            "https://example.com/a",
            "https://other.org/x",
            "https://example.com/b",
            "https://example.com/c",
        ]));
        assert_eq!(groups.len(), 2);

        let big = groups
            .iter()
            .find(|g| g.targets.len() == 3)
            .expect("example.com");
        assert_eq!(big.host, "example.com");
        assert_eq!(
            big.targets.iter().map(|t| t.id).collect::<Vec<_>>(),
            vec![0, 2, 3],
            "a host's links keep their due order"
        );
    }

    #[test]
    fn www_and_case_fold_onto_one_host() {
        assert_eq!(host_key("https://WWW.Example.COM/a"), "example.com");
        assert_eq!(host_key("https://example.com/b"), "example.com");
        assert_eq!(
            group_by_host(targets(&[
                "https://www.example.com/a",
                "https://example.com/b"
            ]))
            .len(),
            1
        );
    }

    #[test]
    fn ports_are_distinct_hosts() {
        assert_eq!(host_key("http://127.0.0.1:8080/a"), "127.0.0.1:8080");
        assert_eq!(
            group_by_host(targets(&["http://127.0.0.1:1/a", "http://127.0.0.1:2/a"])).len(),
            2
        );
    }

    #[test]
    fn unparseable_urls_get_their_own_queues() {
        assert_eq!(
            group_by_host(targets(&["not a url", "also not a url"])).len(),
            2
        );
    }
}
