//! Concurrency behaviour of a sweep, against real HTTP servers on loopback.
//!
//! The two rules under test pull in opposite directions and have to hold at
//! the same time: distinct hosts are checked in parallel, and one host is
//! never checked concurrently. Asserting that needs a server that can
//! actually observe overlapping connections, so these spin up real listeners
//! rather than mocking the prober.
//!
//! Every assertion is made from when requests started and ended *at the
//! servers*, never from how long the sweep took overall. A sweep also writes
//! each result to SQLite, and on Windows CI runners those writes are slow and
//! erratic (antivirus scanning of fresh temp files): a total time measured
//! with a stopwatch failed there while the scheduling was demonstrably
//! correct. Overlap and idle time between requests are properties of the
//! schedule alone, so disk speed cannot move them.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use websave_core::{Monitor, MonitorConfig, NewSave, Vault};

/// One clock for every server in the process, so intervals recorded by
/// different servers can be compared with each other.
fn now() -> Duration {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    EPOCH.get_or_init(Instant::now).elapsed()
}

/// When one request was accepted and when its response had been written.
#[derive(Clone, Copy, Debug)]
struct Span {
    start: Duration,
    end: Duration,
}

/// A loopback HTTP server that records how it was used.
struct TestServer {
    port: u16,
    /// Requests served.
    hits: Arc<AtomicUsize>,
    /// High-water mark of simultaneously open requests.
    peak_concurrent: Arc<AtomicUsize>,
    /// Every request served, in the order they finished.
    spans: Arc<Mutex<Vec<Span>>>,
}

impl TestServer {
    /// Start a server that waits `delay` before answering each request.
    fn start(delay: Duration) -> TestServer {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let hits = Arc::new(AtomicUsize::new(0));
        let peak_concurrent = Arc::new(AtomicUsize::new(0));
        let spans = Arc::new(Mutex::new(Vec::new()));
        let in_flight = Arc::new(AtomicUsize::new(0));

        let server = TestServer {
            port,
            hits: hits.clone(),
            peak_concurrent: peak_concurrent.clone(),
            spans: spans.clone(),
        };

        // Detached: the listener lives until the test process exits.
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let (hits, peak, in_flight, spans) = (
                    hits.clone(),
                    peak_concurrent.clone(),
                    in_flight.clone(),
                    spans.clone(),
                );
                std::thread::spawn(move || {
                    let start = now();
                    let open = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(open, Ordering::SeqCst);
                    hits.fetch_add(1, Ordering::SeqCst);

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
                    spans.lock().unwrap().push(Span { start, end: now() });
                });
            }
        });

        server
    }

    fn url(&self, path: usize) -> String {
        format!("http://127.0.0.1:{}/{path}", self.port)
    }

    /// The requests this server saw, earliest first.
    fn spans(&self) -> Vec<Span> {
        let mut spans = self.spans.lock().unwrap().clone();
        spans.sort_by_key(|s| s.start);
        spans
    }
}

/// The most requests that were open at the same instant, across any set of
/// servers.
fn peak_overlap(spans: &[Span]) -> usize {
    spans
        .iter()
        .map(|probe| {
            spans
                .iter()
                .filter(|s| s.start <= probe.start && probe.start < s.end)
                .count()
        })
        .max()
        .unwrap_or(0)
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

/// Eight hosts, each holding its request open for 200ms, and eight workers.
/// Checked in parallel, all eight are in flight at once; serially, never
/// more than one. This is the whole point of grouping by host.
#[test]
fn distinct_hosts_are_checked_in_parallel() {
    let servers: Vec<TestServer> = (0..8)
        .map(|_| TestServer::start(Duration::from_millis(200)))
        .collect();
    let (_dir, vault) = vault_with(servers.iter().map(|s| s.url(0)).collect());
    let monitor = Monitor::new(vault, config(8, Duration::ZERO));

    let summary = monitor.run_due();

    assert_eq!(summary.checked, 8);
    assert_eq!(summary.dead, 0, "the fixtures all answered 200");
    for server in &servers {
        assert_eq!(server.hits.load(Ordering::SeqCst), 1);
    }
    // A worker only reaches its host after the previous step, so a worker
    // stalled for the whole 200ms window is the one way this can miss. That
    // is a far wider margin than any disk or scheduler hiccup.
    let all: Vec<Span> = servers.iter().flat_map(TestServer::spans).collect();
    assert_eq!(
        peak_overlap(&all),
        servers.len(),
        "not every host was in flight at once: {all:?}"
    );
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

    monitor.run_due();

    assert_eq!(a.peak_concurrent.load(Ordering::SeqCst), 1);
    assert_eq!(b.peak_concurrent.load(Ordering::SeqCst), 1);
    // Each host was internally sequential (above), so two requests open at
    // once can only be one from each: the queues ran side by side.
    let both: Vec<Span> = a.spans().into_iter().chain(b.spans()).collect();
    assert_eq!(
        peak_overlap(&both),
        2,
        "the two host queues ran one after the other: {both:?}"
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

    let spans = server.spans();
    assert_eq!(spans.len(), 4);
    for pair in spans.windows(2) {
        let delta = pair[1].start - pair[0].start;
        assert!(
            delta >= Duration::from_millis(120),
            "requests {delta:?} apart, expected at least ~{gap:?}"
        );
    }
}

/// A slow host should not have the gap stacked on top of its own latency —
/// the gap paces request *starts*, so a server slower than the gap is
/// already polite enough.
///
/// Measured as the idle time between one response and the next request. Were
/// the gap added on top of latency, the monitor would sleep the full gap
/// after every response, so *every* idle stretch would be at least 100ms. Done
/// correctly, the only idle time is recording the result. Taking the smallest
/// stretch keeps one slow disk write from failing the test, while the bug
/// would still fail it on every run.
#[test]
fn a_slow_host_does_not_pay_the_gap_twice() {
    let gap = Duration::from_millis(100);
    let server = TestServer::start(Duration::from_millis(200));
    let (_dir, vault) = vault_with((0..5).map(|i| server.url(i)).collect());
    let monitor = Monitor::new(vault, config(4, gap));

    monitor.run_due();

    let spans = server.spans();
    assert_eq!(spans.len(), 5);
    let shortest_idle = spans
        .windows(2)
        .map(|pair| pair[1].start.saturating_sub(pair[0].end))
        .min()
        .unwrap();
    assert!(
        shortest_idle < gap / 2,
        "the gap was added to latency instead of absorbed by it: \
         at least {shortest_idle:?} idle between every request ({spans:?})"
    );
}
