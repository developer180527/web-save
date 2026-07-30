//! End-to-end sweeps against a vault of unroutable URLs.
//!
//! Every target points at `127.0.0.1:1`, where the connection is refused
//! immediately: no DNS, no network, no waiting on a timeout — so these
//! exercise the real [`Monitor`] loop (batching, budgets, cancellation,
//! persistence) without depending on anything outside the machine.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use websave_core::{
    schedule, LinkStatus, ListQuery, Monitor, MonitorConfig, MonitorEvent, NewSave, Vault,
};

fn vault(count: usize) -> (tempfile::TempDir, Arc<Vault>) {
    let dir = tempfile::tempdir().unwrap();
    let vault = Vault::open(dir.path().join("vault")).unwrap();
    for i in 0..count {
        vault
            .add_save(NewSave {
                url: format!("http://127.0.0.1:1/{i}"),
                title: format!("Page {i}"),
                ..Default::default()
            })
            .unwrap();
    }
    (dir, Arc::new(vault))
}

fn config() -> MonitorConfig {
    MonitorConfig {
        batch: 4,
        pause_between_batches: Duration::ZERO,
        // These fixtures put every save on one host, so the default 1s
        // per-host gap would pace them one per second. Pacing has its own
        // tests in `concurrency.rs`; these are about the loop.
        min_host_gap: Duration::ZERO,
        // Likewise the breaker: every fixture URL is a refused connection, so
        // the default threshold would abandon the host after three and these
        // tests would be measuring the breaker instead of the sweep. It has
        // its own tests in `host_circuit_breaker.rs`.
        host_failure_threshold: 0,
        fetch_thumbnails: false,
        ..Default::default()
    }
}

#[test]
fn a_sweep_drains_the_due_queue_and_persists_every_result() {
    let (_dir, vault) = vault(10);
    let monitor = Monitor::new(vault.clone(), config());

    let summary = monitor.run_due();

    assert_eq!(summary.checked, 10);
    assert_eq!(summary.dead, 10);
    assert!(!summary.cancelled && !summary.budget_exhausted);
    assert_eq!(
        vault
            .list_saves(&ListQuery {
                status: Some(LinkStatus::Dead),
                ..Default::default()
            })
            .unwrap()
            .len(),
        10
    );

    // The whole point: a second sweep right afterwards has nothing to do,
    // because everything just checked is scheduled into the future.
    assert_eq!(vault.count_due_for_check().unwrap(), 0);
    assert_eq!(monitor.run_due().checked, 0);
    assert_eq!(
        vault.check_interval_secs(1).unwrap(),
        schedule::DEAD_INTERVAL_SECS
    );
}

/// A forced re-check ignores the schedule; that is the difference between it
/// and the background sweep.
#[test]
fn run_all_rechecks_saves_that_are_not_due() {
    let (_dir, vault) = vault(6);
    let monitor = Monitor::new(vault.clone(), config());

    assert_eq!(monitor.run_due().checked, 6);
    assert_eq!(vault.count_due_for_check().unwrap(), 0);

    // Paged by id, so every save is visited exactly once even though each
    // result rewrites the scheduling columns as the sweep walks the table.
    assert_eq!(monitor.run_all().checked, 6);
}

#[test]
fn events_report_progress_and_finish_once() {
    let (_dir, vault) = vault(9);
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    let monitor = Monitor::with_sink(vault, config(), move |event| {
        sink.lock().unwrap().push(event);
    });

    monitor.run_due();

    let events = events.lock().unwrap();
    assert!(matches!(
        events.first(),
        Some(MonitorEvent::Started { total: 9 })
    ));
    assert!(matches!(events.last(), Some(MonitorEvent::Finished(s)) if s.checked == 9));
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, MonitorEvent::Finished(_)))
            .count(),
        1,
        "hosts drive a loading state off Finished; a second one would strand it"
    );

    let progress: Vec<usize> = events
        .iter()
        .filter_map(|e| match e {
            MonitorEvent::Checked { done, .. } => Some(*done),
            _ => None,
        })
        .collect();
    assert_eq!(progress, (1..=9).collect::<Vec<_>>());
}

#[test]
fn max_per_sweep_stops_early_and_leaves_the_rest_due() {
    let (_dir, vault) = vault(10);
    let monitor = Monitor::new(
        vault.clone(),
        MonitorConfig {
            max_per_sweep: Some(4),
            ..config()
        },
    );

    let summary = monitor.run_due();
    assert_eq!(summary.checked, 4);
    assert!(summary.budget_exhausted);
    assert_eq!(vault.count_due_for_check().unwrap(), 6);
}

#[test]
fn cancelling_stops_the_sweep_and_says_so() {
    let (_dir, vault) = vault(20);
    let handle: Arc<Mutex<Option<websave_core::MonitorHandle>>> = Arc::new(Mutex::new(None));
    let seen = Arc::new(AtomicUsize::new(0));

    let cancel_after = handle.clone();
    let counter = seen.clone();
    let monitor = Monitor::with_sink(vault.clone(), config(), move |event| {
        if let MonitorEvent::Checked { .. } = event {
            if counter.fetch_add(1, Ordering::Relaxed) == 2 {
                if let Some(h) = cancel_after.lock().unwrap().as_ref() {
                    h.cancel();
                }
            }
        }
    });
    *handle.lock().unwrap() = Some(monitor.handle());

    let summary = monitor.run_due();
    assert!(summary.cancelled);
    assert!(summary.checked < 20, "checked {}", summary.checked);
    // Results gathered before the stop are still persisted, not discarded.
    assert_eq!(
        vault.count_due_for_check().unwrap(),
        20 - summary.checked as i64
    );

    // The handle resets, so the next sweep is not born cancelled.
    assert!(monitor.run_due().checked > 0);
}
