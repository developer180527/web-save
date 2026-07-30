//! The link monitor: the thing that decides *which* saves get checked, in
//! what order, and reports what happened.
//!
//! This used to live in the Tauri host, with a second copy in the FFI layer
//! for iOS — so a Qt shell would have needed a third, and any change to the
//! sweep (batching, budgets, backoff, cancellation) had to be made in every
//! one of them. The loop belongs to the engine, not to whoever is drawing
//! the window.
//!
//! What stays with the host is genuinely host-specific: when to run
//! ([`Monitor`] never sleeps or owns a timer) and how to surface progress
//! (via the [`MonitorEvent`] sink — Tauri emits IPC events, iOS updates an
//! observable, Qt emits a signal).
//!
//! This module is the orchestrator; each decision it coordinates lives in its
//! own file:
//!
//! - [`host_grouping`] — carving a batch into per-host work queues.
//! - [`host_circuit_breaker`] — writing off a host that has stopped answering.
//! - [`target_cursor`] — which saves the next batch contains.
//! - [`sweep_state`] — live counters and the decision to stop.
//! - [`cancel_handle`] / [`config`] / [`events`] — the public surface.
//!
//! [`crate::probe`] does the actual HTTP work; [`crate::schedule`] decides
//! when each save comes due next.

mod cancel_handle;
mod config;
mod events;
mod host_circuit_breaker;
mod host_grouping;
mod sweep_state;
mod target_cursor;

pub use cancel_handle::MonitorHandle;
pub use config::{
    MonitorConfig, DEFAULT_BATCH, DEFAULT_CONCURRENCY, DEFAULT_HOST_FAILURE_THRESHOLD,
    DEFAULT_HOST_GAP,
};
pub use events::{MonitorEvent, SweepSummary};

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::models::LinkStatus;
use crate::probe::{CheckOutcome, CheckTarget, Prober};
use crate::vault::Vault;
use host_circuit_breaker::HostCircuitBreaker;
use host_grouping::{group_by_host, HostQueue};
use sweep_state::SweepState;
use target_cursor::TargetCursor;

type Sink = Arc<dyn Fn(MonitorEvent) + Send + Sync>;

/// Owns link checking for a vault.
///
/// Construct one per vault and keep it for the app's lifetime; sweeps are
/// blocking, so run them on a worker thread.
pub struct Monitor {
    vault: Arc<Vault>,
    config: MonitorConfig,
    sink: Sink,
    handle: MonitorHandle,
}

impl Monitor {
    /// A monitor that reports nothing. Fine for headless callers.
    pub fn new(vault: Arc<Vault>, config: MonitorConfig) -> Monitor {
        Monitor::with_sink(vault, config, |_| {})
    }

    pub fn with_sink(
        vault: Arc<Vault>,
        config: MonitorConfig,
        sink: impl Fn(MonitorEvent) + Send + Sync + 'static,
    ) -> Monitor {
        Monitor {
            vault,
            config,
            sink: Arc::new(sink),
            handle: MonitorHandle::default(),
        }
    }

    /// A handle that can cancel whichever sweep is currently running.
    pub fn handle(&self) -> MonitorHandle {
        self.handle.clone()
    }

    pub fn config(&self) -> &MonitorConfig {
        &self.config
    }

    /// Check every save whose scheduled time has arrived.
    ///
    /// This is the normal path, and the one that makes a large vault
    /// affordable: most saves will not be due on most runs, and most of those
    /// that are will settle with a 304.
    pub fn run_due(&self) -> SweepSummary {
        let total = self.vault.count_due_for_check().unwrap_or(0) as usize;
        self.sweep(total, TargetCursor::Due { skipped: 0 })
    }

    /// Check every save regardless of schedule — the user pressed "Re-check
    /// all links". Also re-baselines the schedule for the whole vault.
    pub fn run_all(&self) -> SweepSummary {
        let total = self.vault.count_saves().unwrap_or(0) as usize;
        self.sweep(total, TargetCursor::All { after_id: 0 })
    }

    /// Check one save now, outside any sweep, and return its fresh state.
    pub fn check_one(&self, id: i64) -> crate::Result<crate::Save> {
        self.vault.check_save(id)
    }

    fn sweep(&self, total: usize, mut cursor: TargetCursor) -> SweepSummary {
        self.handle.reset();
        let state = SweepState::new();
        let prober = Prober::new();
        // One breaker for the whole sweep, not per batch: a dead host's links
        // are spread across many batches, and re-learning it is unreachable
        // in each one would defeat the point.
        let breaker = HostCircuitBreaker::new(self.config.host_failure_threshold);

        (self.sink)(MonitorEvent::Started { total });
        log::info!(
            "monitor: sweep starting, {total} link(s) to check, up to {} host(s) at a time",
            self.workers()
        );

        loop {
            let targets = match cursor.next_batch(&self.vault, self.config.batch) {
                Ok(targets) if targets.is_empty() => break,
                Ok(targets) => targets,
                Err(e) => {
                    log::warn!("monitor: could not read check targets: {e}");
                    break;
                }
            };
            let batch_len = targets.len();
            let skipped_before = state.skipped.load(Ordering::Relaxed);

            self.run_batch(targets, &state, &prober, &breaker, total);

            // Anything the breaker skipped is still due, so the cursor has to
            // step over it or the next query returns the same rows.
            cursor.note_skipped(state.skipped.load(Ordering::Relaxed) - skipped_before);

            (self.sink)(MonitorEvent::BatchDone {
                done: state.checked.load(Ordering::Relaxed),
                total,
            });

            if state.stopped() {
                break;
            }
            // A short batch means the queue is drained; skip the pause so an
            // idle app isn't left sleeping for nothing.
            if (batch_len as i64) < self.config.batch {
                break;
            }
            if !self.config.pause_between_batches.is_zero() {
                std::thread::sleep(self.config.pause_between_batches);
            }
        }

        let summary = state.summarize();
        log::info!(
            "monitor: sweep done in {:?} — {} checked ({} via 304), {} changed, {} dead{}{}",
            state.started.elapsed(),
            summary.checked,
            summary.not_modified,
            summary.changed,
            summary.dead,
            if summary.skipped > 0 {
                format!(
                    ", {} skipped across {} unreachable host(s)",
                    summary.skipped, summary.hosts_unreachable
                )
            } else {
                String::new()
            },
            if summary.cancelled {
                ", cancelled"
            } else if summary.budget_exhausted {
                ", budget reached"
            } else {
                ""
            }
        );
        (self.sink)(MonitorEvent::Finished(summary));
        summary
    }

    /// Check one batch, several hosts at a time.
    ///
    /// The unit of work handed to a thread is a *host*, not a link — see
    /// [`host_grouping`] for why.
    fn run_batch(
        &self,
        targets: Vec<CheckTarget>,
        state: &SweepState,
        prober: &Prober,
        breaker: &HostCircuitBreaker,
        total: usize,
    ) {
        let groups = group_by_host(targets);
        let workers = self.workers().min(groups.len());
        let queue = Mutex::new(groups);

        // Scoped threads so the workers can borrow the monitor, the prober
        // and the sweep state directly. They are all joined before this
        // returns, so a sweep never outlives the call that started it.
        std::thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| loop {
                    // Bind the pop to its own statement. In `while let Some(g)
                    // = queue.lock().unwrap().pop()` the temporary guard lives
                    // until the end of the loop body, so every worker would
                    // hold the queue lock for the duration of its host —
                    // serializing the entire sweep.
                    let next = queue.lock().unwrap().pop();
                    let Some(group) = next else { break };
                    self.check_host(group, state, prober, breaker, total);
                });
            }
        });
    }

    /// Check every link queued for one host, in sequence, no faster than
    /// [`MonitorConfig::min_host_gap`] — and stop early if the host turns out
    /// to be unreachable.
    fn check_host(
        &self,
        group: HostQueue,
        state: &SweepState,
        prober: &Prober,
        breaker: &HostCircuitBreaker,
        total: usize,
    ) {
        // Already written off in an earlier batch.
        if breaker.is_open(&group.host) {
            self.abandon(&group.host, group.targets.len(), state, false);
            return;
        }

        let mut last_start: Option<Instant> = None;
        for (index, target) in group.targets.iter().enumerate() {
            if state.should_stop(&self.handle, &self.config) {
                return;
            }
            // Rate-limit on request *starts*, so a slow server doesn't get
            // the gap added on top of its own latency — a host that takes
            // longer than the gap to answer is already politely paced.
            if let Some(previous) = last_start {
                let waited = previous.elapsed();
                if waited < self.config.min_host_gap {
                    std::thread::sleep(self.config.min_host_gap - waited);
                }
            }
            last_start = Some(Instant::now());

            let Some(outcome) = self.check(target, state, prober, total) else {
                continue;
            };

            if outcome.is_transport_failure() {
                if breaker.record_transport_failure(&group.host) {
                    let remaining = group.targets.len() - index - 1;
                    self.abandon(&group.host, remaining, state, true);
                    return;
                }
            } else {
                // Any answer at all — including a 404 — means the host is up.
                breaker.record_reachable(&group.host);
            }
        }
    }

    /// Give up on a host's remaining links for this sweep.
    ///
    /// They are counted, not judged: their stored status is left alone and
    /// they stay due, so a later sweep decides their fate. See
    /// [`host_circuit_breaker`].
    fn abandon(&self, host: &str, remaining: usize, state: &SweepState, first_time: bool) {
        state.skipped.fetch_add(remaining, Ordering::Relaxed);
        // Announced once, on the trip. Later batches keep finding more links
        // on the same host and skipping them silently — they're in the
        // summary's total, but they aren't news.
        if first_time {
            state.hosts_unreachable.fetch_add(1, Ordering::Relaxed);
            log::info!("monitor: {host} is unreachable, skipping its links for this sweep");
            (self.sink)(MonitorEvent::HostUnreachable {
                host: host.to_string(),
            });
        }
    }

    /// Probe one link and persist the result. `None` means the result could
    /// not be recorded, so it should not inform the breaker either.
    fn check(
        &self,
        target: &CheckTarget,
        state: &SweepState,
        prober: &Prober,
        total: usize,
    ) -> Option<CheckOutcome> {
        let outcome = prober.check(target);
        if let Err(e) = self.vault.apply_check(target.id, &outcome) {
            // A save deleted mid-sweep is ordinary, not an error worth
            // aborting the whole run over.
            log::debug!("monitor: could not record check for #{}: {e}", target.id);
            return None;
        }
        if self.config.fetch_thumbnails {
            self.vault
                .maybe_fetch_thumbnail(target.id, outcome.og_image.as_deref());
        }

        state
            .not_modified
            .fetch_add(usize::from(outcome.not_modified), Ordering::Relaxed);
        state.changed.fetch_add(
            usize::from(outcome.status == LinkStatus::Changed),
            Ordering::Relaxed,
        );
        state.dead.fetch_add(
            usize::from(outcome.status == LinkStatus::Dead),
            Ordering::Relaxed,
        );
        let done = state.checked.fetch_add(1, Ordering::Relaxed) + 1;

        (self.sink)(MonitorEvent::Checked {
            id: target.id,
            url: target.url.clone(),
            status: outcome.status,
            http_status: outcome.http_status,
            not_modified: outcome.not_modified,
            done,
            total,
        });
        Some(outcome)
    }

    fn workers(&self) -> usize {
        self.config.max_concurrency.max(1)
    }
}
