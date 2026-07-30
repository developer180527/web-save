//! Sweep tuning knobs and their defaults.

use std::time::Duration;

/// How many targets are pulled from the vault per round-trip.
///
/// This is a scheduling window, not a politeness knob: the batch is split by
/// host and worked in parallel, so it wants to be comfortably larger than
/// [`DEFAULT_CONCURRENCY`] or there won't be enough distinct hosts in it to
/// keep the workers busy.
pub const DEFAULT_BATCH: i64 = 128;

/// Hosts contacted at once by default.
///
/// Link checking is IO-bound — each worker spends its time waiting on a
/// socket — so this is well above the core count on purpose. It is capped
/// rather than unbounded because every worker is an OS thread holding a
/// connection, and because a laptop on hotel wifi does not benefit from 200
/// simultaneous requests.
pub const DEFAULT_CONCURRENCY: usize = 16;

/// Default minimum spacing between two requests to the same host.
pub const DEFAULT_HOST_GAP: Duration = Duration::from_secs(1);

/// Consecutive unreachable links before a host is abandoned for the sweep.
///
/// Three rather than one: a single connection failure can be a flaky socket,
/// a captive portal blink, or one genuinely broken box behind a load
/// balancer. Three in a row on the same host is the host.
pub const DEFAULT_HOST_FAILURE_THRESHOLD: usize = 3;

/// Sweep tuning. [`Default`] suits a desktop daemon; iOS wants a tight
/// [`MonitorConfig::time_budget`] because its foreground window is short.
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    /// Targets fetched from the vault per round-trip. A scheduling window —
    /// see [`DEFAULT_BATCH`].
    pub batch: i64,
    /// How many *hosts* may be checked at once. Links belonging to the same
    /// host are always checked one at a time regardless of this.
    pub max_concurrency: usize,
    /// Minimum spacing between two requests to the same host. This is where
    /// politeness lives now that batches run in parallel.
    pub min_host_gap: Duration,
    /// Consecutive transport failures before a host's remaining links are
    /// skipped for the rest of the sweep. `0` disables the breaker.
    pub host_failure_threshold: usize,
    /// Pause between batches. Defaults to none — pacing is per-host — but
    /// available for hosts that want to throttle overall bandwidth.
    pub pause_between_batches: Duration,
    /// Stop after this many checks. `None` means "everything due".
    pub max_per_sweep: Option<usize>,
    /// Stop once a sweep has run this long. Checked between saves, so a
    /// single slow request can overshoot it by up to one request timeout.
    pub time_budget: Option<Duration>,
    /// Download and cache cover images for saves that lack one.
    pub fetch_thumbnails: bool,
}

impl Default for MonitorConfig {
    fn default() -> MonitorConfig {
        MonitorConfig {
            batch: DEFAULT_BATCH,
            max_concurrency: DEFAULT_CONCURRENCY,
            min_host_gap: DEFAULT_HOST_GAP,
            host_failure_threshold: DEFAULT_HOST_FAILURE_THRESHOLD,
            pause_between_batches: Duration::ZERO,
            max_per_sweep: None,
            time_budget: None,
            fetch_thumbnails: true,
        }
    }
}
