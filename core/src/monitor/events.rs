//! What a sweep tells the outside world: progress events and the final tally.
//!
//! Deliberately UI-free. A host turns these into Tauri IPC events, a SwiftUI
//! observable, or a Qt signal; core has no opinion about which.

use crate::models::LinkStatus;

/// Progress reported as a sweep runs.
#[derive(Debug, Clone)]
pub enum MonitorEvent {
    /// A sweep began. `total` is how many saves it expects to check.
    Started { total: usize },
    /// One save finished checking.
    Checked {
        id: i64,
        url: String,
        status: LinkStatus,
        http_status: Option<u16>,
        /// Settled by a 304 — nothing was downloaded.
        not_modified: bool,
        done: usize,
        total: usize,
    },
    /// A host stopped answering, so the sweep abandoned its remaining links.
    /// Fired exactly once per host per sweep.
    ///
    /// Deliberately carries no count: the sweep keeps meeting more links on
    /// that host in later batches, so the real number isn't known until it
    /// ends. [`SweepSummary::skipped`] has the total. See
    /// [`super::host_circuit_breaker`].
    HostUnreachable {
        /// The grouping key — hostname, plus port when the URL names one.
        host: String,
    },
    /// A batch was persisted. A good moment to refresh a list view, where
    /// reacting to every [`MonitorEvent::Checked`] would be too chatty.
    BatchDone { done: usize, total: usize },
    /// The sweep ended — completed, out of budget, or cancelled.
    Finished(SweepSummary),
}

/// What a finished sweep did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SweepSummary {
    pub checked: usize,
    /// Of those, how many the server settled with a 304.
    pub not_modified: usize,
    pub changed: usize,
    pub dead: usize,
    /// Links skipped because their host had stopped answering. These were
    /// *not* judged — they keep their previous status and stay due.
    pub skipped: usize,
    /// Hosts whose breaker tripped.
    pub hosts_unreachable: usize,
    /// Hosts whose serving location was resolved or refreshed this sweep.
    pub located: usize,
    /// Stopped early by [`super::MonitorHandle::cancel`].
    pub cancelled: bool,
    /// Stopped early by [`super::MonitorConfig::max_per_sweep`] or
    /// [`super::MonitorConfig::time_budget`]; more work remains due.
    pub budget_exhausted: bool,
}
