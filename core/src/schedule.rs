//! When to check a link next.
//!
//! The old rule was "everything, every 24 hours". That is a fixed cost of one
//! request per save per day, which at 10k saves is a sweep that cannot finish
//! before the next one is due — and almost all of it re-fetches pages that
//! have not changed in months.
//!
//! This module replaces it with per-save adaptive backoff. A page that keeps
//! coming back unchanged is checked exponentially less often, up to
//! [`MAX_INTERVAL_SECS`]; the moment it changes it drops back to
//! [`MIN_INTERVAL_SECS`]. Saves the user is actively engaged with — starred,
//! or still unread in the Inbox — never back off past [`HOT_MAX_INTERVAL_SECS`],
//! because those are the ones where a stale status is actually noticed.
//!
//! The result is that the daily working set tracks how volatile the vault
//! really is rather than how large it is.

use crate::models::LinkStatus;

/// Floor: a page that just changed is re-checked this often.
pub const MIN_INTERVAL_SECS: i64 = 6 * 3600;

/// Ceiling for a long-stable page.
pub const MAX_INTERVAL_SECS: i64 = 30 * 86_400;

/// Ceiling for saves the user is engaged with (favorite, or unread).
pub const HOT_MAX_INTERVAL_SECS: i64 = 24 * 3600;

/// Dead links keep a fixed, fairly brisk cadence instead of backing off.
///
/// They are on a 30-day deletion countdown in the Trash, and the only thing
/// that can rescue them is a check noticing the site came back. Backing off a
/// dead link would mean quietly reducing its chance of recovery the longer it
/// has been gone, which is exactly backwards.
pub const DEAD_INTERVAL_SECS: i64 = 12 * 3600;

/// Interval a never-checked save starts at. Mirrored as a literal in the v8
/// migration's column default; [`crate::vault`] tests that they agree.
pub const DEFAULT_INTERVAL_SECS: i64 = MIN_INTERVAL_SECS;

/// A save's current place in the backoff curve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cadence {
    /// Seconds between checks.
    pub interval_secs: i64,
    /// Consecutive checks that found nothing changed. Drives the doubling.
    pub stable_streak: i64,
}

impl Default for Cadence {
    fn default() -> Cadence {
        Cadence {
            interval_secs: DEFAULT_INTERVAL_SECS,
            stable_streak: 0,
        }
    }
}

/// The cadence a save earns from the check that just finished.
///
/// `hot` marks saves the user is engaged with (favorite or unread), which are
/// capped at [`HOT_MAX_INTERVAL_SECS`].
pub fn next_cadence(status: LinkStatus, previous: Cadence, hot: bool) -> Cadence {
    match status {
        // Fixed cadence — see DEAD_INTERVAL_SECS. `Unreachable` shares it:
        // a private host we couldn't route to should be retried promptly and
        // often, because the fix is usually "the user reconnected to the
        // VPN", and that recovery should show up quickly.
        LinkStatus::Dead | LinkStatus::Unreachable => Cadence {
            interval_secs: DEAD_INTERVAL_SECS,
            stable_streak: 0,
        },
        // Something actually happened: back to the floor, and start the
        // streak over so a page that changes weekly never drifts to monthly.
        LinkStatus::Changed | LinkStatus::Redirected => Cadence {
            interval_secs: MIN_INTERVAL_SECS,
            stable_streak: 0,
        },
        LinkStatus::Active | LinkStatus::Unchecked => {
            let streak = previous.stable_streak.saturating_add(1);
            let ceiling = if hot {
                HOT_MAX_INTERVAL_SECS
            } else {
                MAX_INTERVAL_SECS
            };
            // 6h, 12h, 24h, 2d, 4d … clamped. Shift is bounded before use so
            // a long-lived vault cannot overflow it.
            let doublings = streak.clamp(0, 32) as u32;
            let interval = MIN_INTERVAL_SECS
                .checked_shl(doublings)
                .unwrap_or(MAX_INTERVAL_SECS)
                .clamp(MIN_INTERVAL_SECS, ceiling);
            Cadence {
                interval_secs: interval,
                stable_streak: streak,
            }
        }
    }
}

/// When a save on `cadence` becomes due, given a check finishing at `now`.
///
/// Spread by up to ±10%, derived from the save id, so that a 5,000-link
/// import does not come due as one thundering herd every time — without
/// needing a random source, which would make the schedule untestable and
/// pull a dependency into core.
pub fn due_at(now: i64, cadence: Cadence, id: i64) -> i64 {
    now + cadence.interval_secs + jitter(cadence.interval_secs, id)
}

/// Deterministic offset in `±interval/10`.
fn jitter(interval_secs: i64, id: i64) -> i64 {
    let span = (interval_secs / 5).max(1);
    // Knuth multiplicative hash: cheap, and enough to decorrelate adjacent
    // ids, which is the whole job here.
    let scattered = (id as u64).wrapping_mul(2_654_435_761) >> 8;
    (scattered % span as u64) as i64 - span / 2
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stable(mut cadence: Cadence, rounds: usize, hot: bool) -> Cadence {
        for _ in 0..rounds {
            cadence = next_cadence(LinkStatus::Active, cadence, hot);
        }
        cadence
    }

    #[test]
    fn unchanged_pages_back_off_and_stop_at_the_ceiling() {
        let mut cadence = Cadence::default();
        cadence = next_cadence(LinkStatus::Active, cadence, false);
        assert_eq!(cadence.interval_secs, 12 * 3600);
        cadence = next_cadence(LinkStatus::Active, cadence, false);
        assert_eq!(cadence.interval_secs, 24 * 3600);
        cadence = next_cadence(LinkStatus::Active, cadence, false);
        assert_eq!(cadence.interval_secs, 2 * 86_400);

        assert_eq!(stable(cadence, 100, false).interval_secs, MAX_INTERVAL_SECS);
    }

    #[test]
    fn engaged_saves_stay_hot() {
        let cold = stable(Cadence::default(), 100, false);
        let hot = stable(Cadence::default(), 100, true);
        assert_eq!(cold.interval_secs, MAX_INTERVAL_SECS);
        assert_eq!(hot.interval_secs, HOT_MAX_INTERVAL_SECS);
    }

    /// The point of the whole module: a change has to pull the save back to
    /// the floor, however long it had been quiet before.
    #[test]
    fn a_change_resets_the_curve() {
        let settled = stable(Cadence::default(), 100, false);
        assert_eq!(settled.interval_secs, MAX_INTERVAL_SECS);

        let after = next_cadence(LinkStatus::Changed, settled, false);
        assert_eq!(after.interval_secs, MIN_INTERVAL_SECS);
        assert_eq!(after.stable_streak, 0);

        // And it has to climb again from the bottom, not resume where it was.
        let next = next_cadence(LinkStatus::Active, after, false);
        assert_eq!(next.interval_secs, 12 * 3600);
    }

    #[test]
    fn redirects_reset_too_and_dead_links_hold_a_fixed_cadence() {
        let settled = stable(Cadence::default(), 100, false);
        assert_eq!(
            next_cadence(LinkStatus::Redirected, settled, false).interval_secs,
            MIN_INTERVAL_SECS
        );

        let dead = next_cadence(LinkStatus::Dead, settled, false);
        assert_eq!(dead.interval_secs, DEAD_INTERVAL_SECS);
        // Still dead a second time: no backoff, recovery stays detectable.
        let still = next_cadence(LinkStatus::Dead, dead, false);
        assert_eq!(still.interval_secs, DEAD_INTERVAL_SECS);
    }

    #[test]
    fn jitter_spreads_a_herd_without_leaving_the_band() {
        let cadence = Cadence::default();
        let due: Vec<i64> = (1..=500).map(|id| due_at(1_000_000, cadence, id)).collect();

        let span = cadence.interval_secs / 10;
        for (id, &at) in due.iter().enumerate() {
            let drift = at - 1_000_000 - cadence.interval_secs;
            assert!(
                drift.abs() <= span,
                "id {id} drifted {drift}s, past ±{span}s"
            );
        }

        let distinct: std::collections::HashSet<_> = due.iter().collect();
        assert!(
            distinct.len() > 400,
            "500 imported saves collapsed onto {} check times",
            distinct.len()
        );
    }

    #[test]
    fn due_at_is_stable_for_a_given_save() {
        let cadence = Cadence::default();
        assert_eq!(due_at(500, cadence, 42), due_at(500, cadence, 42));
    }
}
