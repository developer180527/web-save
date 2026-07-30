//! Concurrency behaviour of a sweep, against real HTTP servers on loopback.
//!
//! The two rules under test pull in opposite directions and have to hold at
//! the same time: distinct hosts are checked in parallel, and one host is
//! never checked concurrently. Asserting that needs a server that can
//! actually observe overlapping connections, so these spin up real listeners
//! rather than mocking the prober.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use websave_core::{Monitor, MonitorConfig, NewSave, Vault};

/// A loopback HTTP server that records how it was used.
struct TestServer {
    port: u16,
    /// Requests served.
    hits: Arc<AtomicUsize>,
    /// High-water mark of simultaneously open requests.
    peak_concurrent: Arc<AtomicUsize>,
    /// Milliseconds since the server started, one per request accepted.
    arrivals: Arc<std::sync::Mutex<Vec<u128>>>,
}

impl TestServer {
    /// Start a server that waits `delay` before answering each request.
    fn start(delay: Duration) -> TestServer {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let hits = Arc::new(AtomicUsize::new(0));
        let peak_concurrent = Arc::new(AtomicUsize::new(0));
        let arrivals = Arc::new(std::sync::Mutex::new(Vec::new()));
        let in_flight = Arc::new(AtomicUsize::new(0));
        let epoch = Instant::now();

        let server = TestServer {
            port,
            hits: hits.clone(),
            peak_concurrent: peak_concurrent.clone(),
            arrivals: arrivals.clone(),
        };

        // Detached: the listener lives until the test process exits.
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let (hits, peak, in_flight, arrivals) = (
                    hits.clone(),
                    peak_concurrent.clone(),
                    in_flight.clone(),
                    arrivals.clone(),
                );
                std::thread::spawn(move || {
                    let now = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    hits.fetch_add(1, Ordering::SeqCst);
                    arrivals.lock().unwrap().push(epoch.elapsed().as_millis());

                    // Read the request head; we don't care what it says.
                    let mut buf = [0u8; 2048];
                    let _ = stream.read(&mut buf);
                    std::thread::sleep(delay);

                    let body = "<html><body><article><p>Fixture page.</p></article></body></html>";
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\
                         Connection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(response.as_bytes());
                    let _ = stream.flush();
                    in_flight.fetch_sub(1, Ordering::SeqCst);
                });
            }
        });

        server
    }

    fn url(&self, path: usize) -> String {
        format!("http://127.0.0.1:{}/{path}", self.port)
    }
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

fn config(max_concurrency: usize, min_host_gap: Duration) -> MonitorConfig {
    MonitorConfig {
        batch: 128,
        max_concurrency,
        min_host_gap,
        pause_between_batches: Duration::ZERO,
        fetch_thumbnails: false,
        ..Default::default()
    }
}

/// Eight hosts, each answering in 200ms. Serially that is 1.6s; in parallel
/// it is one round trip. This is the whole point of the change.
#[test]
fn distinct_hosts_are_checked_in_parallel() {
    let servers: Vec<TestServer> = (0..8)
        .map(|_| TestServer::start(Duration::from_millis(200)))
        .collect();
    let (_dir, vault) = vault_with(servers.iter().map(|s| s.url(0)).collect());
    let monitor = Monitor::new(vault, config(8, Duration::ZERO));

    let started = Instant::now();
    let summary = monitor.run_due();
    let elapsed = started.elapsed();

    assert_eq!(summary.checked, 8);
    assert_eq!(summary.dead, 0, "the fixtures all answered 200");
    assert!(
        elapsed < Duration::from_millis(900),
        "8 × 200ms hosts took {elapsed:?}; that is serial, not parallel"
    );
    for server in &servers {
        assert_eq!(server.hits.load(Ordering::SeqCst), 1);
    }
}

/// The counterpart: one host must never see two of our requests at once,
/// however many workers are free. A shared queue of individual links guarded
/// by a per-host lock would pass the test above and fail this one.
#[test]
fn one_host_is_never_hit_concurrently() {
    let server = TestServer::start(Duration::from_millis(80));
    let (_dir, vault) = vault_with((0..10).map(|i| server.url(i)).collect());
    // Far more workers than hosts: if serialization came from worker scarcity
    // rather than from grouping, this is where it would show.
    let monitor = Monitor::new(vault, config(16, Duration::ZERO));

    let summary = monitor.run_due();

    assert_eq!(summary.checked, 10);
    assert_eq!(server.hits.load(Ordering::SeqCst), 10);
    assert_eq!(
        server.peak_concurrent.load(Ordering::SeqCst),
        1,
        "the host saw overlapping requests"
    );
}

/// Mixed load: two busy hosts should still overlap each other while each
/// stays internally sequential.
#[test]
fn hosts_overlap_each_other_but_not_themselves() {
    let a = TestServer::start(Duration::from_millis(100));
    let b = TestServer::start(Duration::from_millis(100));
    let urls = (0..4)
        .flat_map(|i| [a.url(i), b.url(i)])
        .collect::<Vec<_>>();
    let (_dir, vault) = vault_with(urls);
    let monitor = Monitor::new(vault, config(8, Duration::ZERO));

    let started = Instant::now();
    monitor.run_due();
    let elapsed = started.elapsed();

    assert_eq!(a.peak_concurrent.load(Ordering::SeqCst), 1);
    assert_eq!(b.peak_concurrent.load(Ordering::SeqCst), 1);
    // 8 requests: serial would be ~800ms, the two queues in parallel ~400ms.
    assert!(
        elapsed < Duration::from_millis(700),
        "two host queues did not overlap: {elapsed:?}"
    );
}

/// Politeness is per-host pacing now, so it has to actually pace.
#[test]
fn min_host_gap_spaces_requests_to_one_host() {
    let server = TestServer::start(Duration::ZERO);
    let (_dir, vault) = vault_with((0..4).map(|i| server.url(i)).collect());
    let gap = Duration::from_millis(150);
    let monitor = Monitor::new(vault, config(8, gap));

    monitor.run_due();

    let arrivals = server.arrivals.lock().unwrap().clone();
    assert_eq!(arrivals.len(), 4);
    let mut sorted = arrivals;
    sorted.sort_unstable();
    for pair in sorted.windows(2) {
        let delta = pair[1] - pair[0];
        assert!(
            delta >= 120,
            "requests {}ms apart, expected at least ~{}ms",
            delta,
            gap.as_millis()
        );
    }
}

/// A slow host should not have the gap stacked on top of its own latency —
/// the gap paces request *starts*, so a server slower than the gap is
/// already polite enough.
#[test]
fn a_slow_host_does_not_pay_the_gap_twice() {
    let server = TestServer::start(Duration::from_millis(200));
    let (_dir, vault) = vault_with((0..3).map(|i| server.url(i)).collect());
    let monitor = Monitor::new(vault, config(4, Duration::from_millis(100)));

    let started = Instant::now();
    monitor.run_due();
    let elapsed = started.elapsed();

    // 3 × 200ms of real latency. Adding the gap on top would be 900ms.
    assert!(
        elapsed < Duration::from_millis(800),
        "gap was added to latency instead of absorbed by it: {elapsed:?}"
    );
}
