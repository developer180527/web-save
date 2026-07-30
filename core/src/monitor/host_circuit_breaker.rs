//! Stops a sweep from paying the timeout price for a host that is gone.
//!
//! A domain that has lapsed takes the full request timeout to fail, and a
//! vault often holds dozens or hundreds of links from one domain. Probing
//! each of them individually means the sweep spends most of its wall clock
//! waiting on a machine it has already established is not there — 200 saved
//! pages from a dead domain is over an hour of nothing.
//!
//! After [`MonitorConfig::host_failure_threshold`] consecutive *transport*
//! failures on one host, the breaker opens and the sweep abandons that host's
//! remaining links.
//!
//! Two decisions worth spelling out:
//!
//! - **Only transport failures count.** A 404 is a healthy server answering a
//!   question about one page; it predicts nothing about the next link on that
//!   host. DNS failure, connection refused and timeouts are the host itself.
//!   See [`crate::probe::CheckOutcome::is_transport_failure`].
//!
//! - **Skipped links are not judged.** They keep whatever status they had and
//!   stay due. Inheriting a `dead` verdict from their neighbours would be
//!   cheap and wrong: the same signal is produced by the user's wifi being
//!   off, and it would march an entire vault into the Trash's 30-day deletion
//!   countdown during a train journey. The breaker is an optimisation, not a
//!   verdict — the links it skips get checked on a later sweep.
//!
//! State lives for one sweep. Every sweep gives an unreachable host a fresh
//! chance, which is how recovery is noticed.
//!
//! [`MonitorConfig::host_failure_threshold`]: super::MonitorConfig

use std::collections::HashMap;
use std::sync::Mutex;

/// Per-host reachability tally for the duration of one sweep.
pub(super) struct HostCircuitBreaker {
    /// Consecutive transport failures that open a host. `0` disables.
    threshold: usize,
    hosts: Mutex<HashMap<String, usize>>,
}

impl HostCircuitBreaker {
    pub(super) fn new(threshold: usize) -> HostCircuitBreaker {
        HostCircuitBreaker {
            threshold,
            hosts: Mutex::new(HashMap::new()),
        }
    }

    /// Whether this host has been written off for the rest of the sweep.
    pub(super) fn is_open(&self, host: &str) -> bool {
        if self.threshold == 0 {
            return false;
        }
        self.hosts
            .lock()
            .unwrap()
            .get(host)
            .is_some_and(|failures| *failures >= self.threshold)
    }

    /// Record a link that never reached the server. Returns `true` if this
    /// was the failure that opened the breaker, so the caller can report it
    /// exactly once.
    pub(super) fn record_transport_failure(&self, host: &str) -> bool {
        if self.threshold == 0 {
            return false;
        }
        let mut hosts = self.hosts.lock().unwrap();
        let failures = hosts.entry(host.to_string()).or_insert(0);
        *failures += 1;
        *failures == self.threshold
    }

    /// Record a link that got an answer — any answer, including a 404. The
    /// host is up, so the run of failures is broken.
    pub(super) fn record_reachable(&self, host: &str) {
        if self.threshold == 0 {
            return;
        }
        self.hosts.lock().unwrap().remove(host);
    }
}

#[cfg(test)]
mod tests {
    use super::HostCircuitBreaker;

    #[test]
    fn opens_on_the_nth_consecutive_failure_and_says_so_once() {
        let breaker = HostCircuitBreaker::new(3);
        assert!(!breaker.is_open("gone.example"));

        assert!(!breaker.record_transport_failure("gone.example"));
        assert!(!breaker.record_transport_failure("gone.example"));
        assert!(!breaker.is_open("gone.example"), "not yet at the threshold");

        assert!(
            breaker.record_transport_failure("gone.example"),
            "the tripping failure reports itself"
        );
        assert!(breaker.is_open("gone.example"));

        assert!(
            !breaker.record_transport_failure("gone.example"),
            "already open — must not announce a second time"
        );
    }

    /// The run has to be *consecutive*. A host that fails intermittently is
    /// flaky, not gone, and its links deserve individual verdicts.
    #[test]
    fn a_reachable_link_resets_the_run() {
        let breaker = HostCircuitBreaker::new(3);
        breaker.record_transport_failure("flaky.example");
        breaker.record_transport_failure("flaky.example");
        breaker.record_reachable("flaky.example");

        assert!(!breaker.record_transport_failure("flaky.example"));
        assert!(!breaker.record_transport_failure("flaky.example"));
        assert!(!breaker.is_open("flaky.example"));
    }

    #[test]
    fn hosts_are_tracked_independently() {
        let breaker = HostCircuitBreaker::new(2);
        breaker.record_transport_failure("a.example");
        breaker.record_transport_failure("a.example");

        assert!(breaker.is_open("a.example"));
        assert!(!breaker.is_open("b.example"));
    }

    #[test]
    fn a_zero_threshold_disables_the_breaker() {
        let breaker = HostCircuitBreaker::new(0);
        for _ in 0..100 {
            assert!(!breaker.record_transport_failure("gone.example"));
        }
        assert!(!breaker.is_open("gone.example"));
    }
}
