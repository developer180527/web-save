//! End-to-end behaviour of the dead-host circuit breaker.
//!
//! Unreachable hosts are `127.0.0.1` on ports nothing is listening on, which
//! refuse the connection instantly. In production those are 20-second
//! timeouts — the reason the breaker exists — but the *count* of probes is
//! what these assert, and that is identical either way.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use websave_core::{
    LinkStatus, Monitor, MonitorConfig, MonitorEvent, NewSave, SweepSummary, Vault,
};

/// A port on loopback with nothing behind it: every connection is refused.
const DEAD_PORT: u16 = 1;
const OTHER_DEAD_PORT: u16 = 2;

fn dead_url(port: u16, i: usize) -> String {
    format!("http://127.0.0.1:{port}/{i}")
}

fn vault_with(urls: Vec<String>) -> (tempfile::TempDir, Arc<Vault>) {
    let dir = tempfile::tempdir().unwrap();
    let vault = Vault::open(dir.path().join("vault")).unwrap();
    for url in urls {
        vault
            .add_save(NewSave {
                url,
                ..Default::default()
            })
            .unwrap();
    }
    (dir, Arc::new(vault))
}

fn config(threshold: usize, batch: i64) -> MonitorConfig {
    MonitorConfig {
        batch,
        max_concurrency: 8,
        min_host_gap: Duration::ZERO,
        host_failure_threshold: threshold,
        pause_between_batches: Duration::ZERO,
        fetch_thumbnails: false,
        ..Default::default()
    }
}

/// A server that cycles: drop `drop_count` connections unanswered (a
/// transport failure, same class as a refused connection), then answer
/// `answer_count` of them with a 404, and repeat.
fn spawn_flaky_server(drop_count: usize, answer_count: usize) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(0usize));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let position = {
                let mut seen = seen.lock().unwrap();
                let cycle = drop_count.saturating_add(answer_count).max(1);
                let position = *seen % cycle;
                *seen += 1;
                position
            };
            std::thread::spawn(move || {
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf);
                if position < drop_count {
                    // Close without a byte of response: ureq reports a
                    // transport error, exactly like an unreachable host.
                    return;
                }
                let _ = stream.write_all(
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
            });
        }
    });
    port
}

/// A server that answers 404 to everything. A dead *page* on a live host
/// must not be mistaken for a dead host.
fn spawn_404_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
            });
        }
    });
    port
}

/// The headline: 20 links on a host that isn't there cost 3 probes, not 20.
#[test]
fn an_unreachable_host_is_probed_to_the_threshold_and_no_further() {
    let (_dir, vault) = vault_with((0..20).map(|i| dead_url(DEAD_PORT, i)).collect());
    let monitor = Monitor::new(vault.clone(), config(3, 128));

    let summary = monitor.run_due();

    assert_eq!(summary.checked, 3, "probed past the threshold");
    assert_eq!(summary.dead, 3);
    assert_eq!(summary.skipped, 17);
    assert_eq!(summary.hosts_unreachable, 1);
    assert!(!summary.cancelled && !summary.budget_exhausted);
}

/// Skipped links are not judged. Inheriting a `dead` verdict from their
/// neighbours would march the vault into the Trash countdown the first time
/// the user's wifi drops.
#[test]
fn skipped_links_keep_their_status_and_stay_due() {
    let (_dir, vault) = vault_with((0..10).map(|i| dead_url(DEAD_PORT, i)).collect());
    let monitor = Monitor::new(vault.clone(), config(3, 128));

    monitor.run_due();

    let unchecked = vault
        .list_saves(&Default::default())
        .unwrap()
        .into_iter()
        .filter(|s| s.status == LinkStatus::Unchecked)
        .count();
    assert_eq!(unchecked, 7, "the skipped links were given a verdict");
    assert_eq!(
        vault.count_due_for_check().unwrap(),
        7,
        "skipped links must stay due so a later sweep can judge them"
    );
    for save in vault.list_saves(&Default::default()).unwrap() {
        if save.status == LinkStatus::Unchecked {
            assert_eq!(save.dead_since, None, "no deletion countdown was started");
        }
    }
}

/// The bug this pairing creates if you're not careful: skipped links stay due
/// and sort to the front of the due query, so a naive sweep is handed the
/// same batch forever. The cursor has to step over them.
#[test]
fn a_sweep_terminates_even_though_skipped_links_remain_due() {
    let (_dir, vault) = vault_with((0..30).map(|i| dead_url(DEAD_PORT, i)).collect());
    // A batch smaller than the vault forces several rounds of the loop.
    let monitor = Monitor::new(vault.clone(), config(3, 8));

    // If the cursor didn't advance past skipped rows this would never return.
    let summary = monitor.run_due();

    assert_eq!(summary.checked, 3);
    assert_eq!(summary.skipped, 27);
}

/// The breaker's state must outlive a single batch, or a dead host's links
/// spread across batches would each pay the threshold again.
#[test]
fn the_breaker_persists_across_batches() {
    let (_dir, vault) = vault_with((0..40).map(|i| dead_url(DEAD_PORT, i)).collect());
    let monitor = Monitor::new(vault.clone(), config(3, 5));

    let summary = monitor.run_due();

    assert_eq!(
        summary.checked, 3,
        "the host was re-probed in a later batch"
    );
}

/// A live server returning 404 is answering. Its links each deserve their own
/// verdict, however many of them are broken.
#[test]
fn a_live_host_serving_404s_never_trips_the_breaker() {
    let port = spawn_404_server();
    let (_dir, vault) = vault_with((0..8).map(|i| dead_url(port, i)).collect());
    let monitor = Monitor::new(vault.clone(), config(3, 128));

    let summary = monitor.run_due();

    assert_eq!(summary.checked, 8, "a reachable host must be fully checked");
    assert_eq!(summary.dead, 8, "and each of its 404s judged individually");
    assert_eq!(summary.skipped, 0);
    assert_eq!(summary.hosts_unreachable, 0);
}

/// One host going down must not take its neighbours with it.
#[test]
fn other_hosts_are_unaffected() {
    let live = spawn_404_server();
    let urls = (0..6)
        .flat_map(|i| {
            [
                dead_url(DEAD_PORT, i),
                dead_url(OTHER_DEAD_PORT, i),
                dead_url(live, i),
            ]
        })
        .collect();
    let (_dir, vault) = vault_with(urls);
    let monitor = Monitor::new(vault.clone(), config(3, 128));

    let summary = monitor.run_due();

    // 6 from the live host + 3 each from the two dead ones.
    assert_eq!(summary.checked, 12);
    assert_eq!(summary.skipped, 6, "3 left on each dead host");
    assert_eq!(summary.hosts_unreachable, 2);
}

/// Hosts recover. Each sweep starts with a clean slate, so a host that came
/// back is checked normally rather than staying written off.
#[test]
fn a_later_sweep_gives_the_host_a_fresh_chance() {
    let (_dir, vault) = vault_with((0..10).map(|i| dead_url(DEAD_PORT, i)).collect());
    let monitor = Monitor::new(vault.clone(), config(3, 128));

    let first = monitor.run_due();
    assert_eq!(first.checked, 3);

    // The three already checked are now on the dead cadence, so the next
    // sweep starts on the ones it skipped rather than repeating itself.
    let second = monitor.run_due();
    assert_eq!(second.checked, 3);
    assert_eq!(second.skipped, 4);

    assert_eq!(
        vault.count_due_for_check().unwrap(),
        4,
        "the vault converges instead of stalling"
    );
}

#[test]
fn the_breaker_announces_each_host_once() {
    let (_dir, vault) = vault_with((0..15).map(|i| dead_url(DEAD_PORT, i)).collect());
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    let monitor = Monitor::with_sink(vault, config(3, 4), move |event| {
        if let MonitorEvent::HostUnreachable { host } = event {
            sink.lock().unwrap().push(host);
        }
    });

    let summary = monitor.run_due();

    let events = events.lock().unwrap();
    assert_eq!(
        events.len(),
        1,
        "one event per unreachable host, not per skipped batch: {events:?}"
    );
    assert_eq!(events[0], "127.0.0.1:1");
    // The running total belongs to the summary, which sees every batch.
    assert_eq!(summary.skipped, 12);
}

/// Escape hatch for anyone who would rather pay the timeouts.
#[test]
fn a_zero_threshold_checks_everything() {
    let (_dir, vault) = vault_with((0..12).map(|i| dead_url(DEAD_PORT, i)).collect());
    let monitor = Monitor::new(vault, config(0, 128));

    let summary = monitor.run_due();

    assert_eq!(
        summary,
        SweepSummary {
            checked: 12,
            dead: 12,
            ..Default::default()
        }
    );
}

/// A run of failures has to be *consecutive*. A host that drops two
/// connections then answers is flaky, not gone, and its links still deserve
/// individual verdicts — so a sweep must keep working through them.
#[test]
fn a_flaky_host_is_not_written_off_as_a_dead_one() {
    // Drops two connections, answers the third, repeat. With a threshold of
    // 3 the run never gets there.
    let port = spawn_flaky_server(2, 1);
    let (_dir, vault) = vault_with((0..15).map(|i| dead_url(port, i)).collect());
    let monitor = Monitor::new(vault, config(3, 128));

    let summary = monitor.run_due();

    assert_eq!(summary.checked, 15, "a flaky host was abandoned");
    assert_eq!(summary.skipped, 0);
    assert_eq!(summary.hosts_unreachable, 0);
}

/// And the other side of the same rule: once the run *does* reach the
/// threshold, the host goes.
#[test]
fn a_host_that_only_drops_connections_still_trips() {
    // Never answers: every connection is accepted and closed unanswered,
    // which is a transport failure just like a refused connection.
    let port = spawn_flaky_server(usize::MAX, 0);
    let (_dir, vault) = vault_with((0..15).map(|i| dead_url(port, i)).collect());
    let monitor = Monitor::new(vault, config(3, 128));

    let summary = monitor.run_due();

    assert_eq!(summary.checked, 3);
    assert_eq!(summary.skipped, 12);
    assert_eq!(summary.hosts_unreachable, 1);
}
