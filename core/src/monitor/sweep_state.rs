//! Live counters for a sweep in progress, and the decision to stop.
//!
//! Shared across every worker thread, so all of it is atomic and the summary
//! is assembled once the threads have joined.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;

use super::cancel_handle::MonitorHandle;
use super::config::MonitorConfig;
use super::events::SweepSummary;

pub(super) struct SweepState {
    pub started: Instant,
    pub checked: AtomicUsize,
    pub not_modified: AtomicUsize,
    pub changed: AtomicUsize,
    pub dead: AtomicUsize,
    pub skipped: AtomicUsize,
    pub hosts_unreachable: AtomicUsize,
    pub located: AtomicUsize,
    cancelled: AtomicBool,
    budget_exhausted: AtomicBool,
}

impl SweepState {
    pub(super) fn new() -> SweepState {
        SweepState {
            started: Instant::now(),
            checked: AtomicUsize::new(0),
            not_modified: AtomicUsize::new(0),
            changed: AtomicUsize::new(0),
            dead: AtomicUsize::new(0),
            skipped: AtomicUsize::new(0),
            hosts_unreachable: AtomicUsize::new(0),
            located: AtomicUsize::new(0),
            cancelled: AtomicBool::new(false),
            budget_exhausted: AtomicBool::new(false),
        }
    }

    /// Whether this worker should stop, recording *why* so the summary can
    /// report it. Checked before each request rather than mid-request, so a
    /// stop costs at most one in-flight check per worker.
    pub(super) fn should_stop(&self, handle: &MonitorHandle, config: &MonitorConfig) -> bool {
        if handle.is_cancelled() {
            self.cancelled.store(true, Ordering::Relaxed);
            return true;
        }
        let over_count = config
            .max_per_sweep
            .is_some_and(|max| self.checked.load(Ordering::Relaxed) >= max);
        let over_time = config
            .time_budget
            .is_some_and(|budget| self.started.elapsed() >= budget);
        if over_count || over_time {
            self.budget_exhausted.store(true, Ordering::Relaxed);
            return true;
        }
        false
    }

    pub(super) fn stopped(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed) || self.budget_exhausted.load(Ordering::Relaxed)
    }

    pub(super) fn summarize(&self) -> SweepSummary {
        SweepSummary {
            checked: self.checked.load(Ordering::Relaxed),
            not_modified: self.not_modified.load(Ordering::Relaxed),
            changed: self.changed.load(Ordering::Relaxed),
            dead: self.dead.load(Ordering::Relaxed),
            skipped: self.skipped.load(Ordering::Relaxed),
            hosts_unreachable: self.hosts_unreachable.load(Ordering::Relaxed),
            located: self.located.load(Ordering::Relaxed),
            cancelled: self.cancelled.load(Ordering::Relaxed),
            budget_exhausted: self.budget_exhausted.load(Ordering::Relaxed),
        }
    }
}
