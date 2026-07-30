//! The storage side of link monitoring: which saves are due, and what a
//! finished check writes back.
//!
//! The sweep itself belongs to [`crate::monitor`]; this module only hands
//! it lightweight targets — so the network work happens without holding
//! the vault lock — and records the outcomes.

use rusqlite::{params, OptionalExtension, Row};

use crate::error::{Error, Result};
use crate::models::Save;
use crate::probe::{self, CheckOutcome, CheckTarget};
use crate::schedule::{self, Cadence};

use super::{now, Vault};

/// Columns hydrating a [`CheckTarget`], in the order [`check_target`] reads.
const CHECK_COLS: &str = "id, url, content_hash, content_hash_algo, etag, last_modified";

fn check_target(row: &Row) -> rusqlite::Result<CheckTarget> {
    Ok(CheckTarget {
        id: row.get(0)?,
        url: row.get(1)?,
        content_hash: row.get(2)?,
        hash_algo: row.get::<_, i64>(3)? as u32,
        etag: row.get(4)?,
        last_modified: row.get(5)?,
    })
}

impl Vault {
    /// Saves whose scheduled check time has arrived, soonest-due first.
    ///
    /// Returns lightweight targets so the caller can perform network checks
    /// without holding the vault lock. Backed by `idx_saves_next_check`, so
    /// this stays a bounded index range scan on a vault of any size — the
    /// previous version sorted the whole table on every batch.
    ///
    /// A sweep normally re-queries with `offset` 0 each batch: checking a save
    /// reschedules it, which takes it out of the due set, so the next query
    /// naturally returns the next window. `offset` exists for the rows a sweep
    /// deliberately *doesn't* check — a host whose circuit breaker has tripped
    /// leaves its remaining links due, and without skipping past them the same
    /// batch would come back forever. `id` breaks ties so that window is
    /// stable across queries.
    pub fn saves_due_for_check(&self, limit: i64, offset: i64) -> Result<Vec<CheckTarget>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&format!(
            "SELECT {CHECK_COLS} FROM saves
             WHERE next_check_at IS NULL OR next_check_at <= ?1
             ORDER BY next_check_at ASC, id ASC LIMIT ?2 OFFSET ?3"
        ))?;
        let targets = stmt
            .query_map(params![now(), limit, offset], check_target)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(targets)
    }

    /// How many saves are currently due. Cheap enough to call before a sweep
    /// so the host can report real progress.
    pub fn count_due_for_check(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.query_row(
            "SELECT COUNT(*) FROM saves WHERE next_check_at IS NULL OR next_check_at <= ?1",
            params![now()],
            |r| r.get(0),
        )?)
    }

    /// A page of check targets ignoring the schedule, for a user-forced sweep
    /// of the whole vault.
    ///
    /// Paged by `id` rather than by due time: a forced sweep rewrites
    /// `next_check_at` as it goes, so paging on that column would reshuffle
    /// the cursor underneath itself and skip rows.
    pub fn all_check_targets(&self, after_id: i64, limit: i64) -> Result<Vec<CheckTarget>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&format!(
            "SELECT {CHECK_COLS} FROM saves WHERE id > ?1 ORDER BY id ASC LIMIT ?2"
        ))?;
        let targets = stmt
            .query_map(params![after_id, limit], check_target)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(targets)
    }

    /// How far apart this save's checks currently are — where it has reached
    /// on the backoff curve. Useful for a debug view, and for asserting the
    /// schedule from outside.
    pub fn check_interval_secs(&self, id: i64) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT check_interval_secs FROM saves WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .optional()?
        .ok_or(Error::NotFound(id))
    }

    /// Persist the outcome of a link check and schedule the next one.
    pub fn apply_check(&self, id: i64, outcome: &CheckOutcome) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        // `favorite`/`is_read` decide whether this save is allowed to back
        // off past the hot ceiling; the rest is for the transition log.
        let previous: Option<(String, String, Cadence, bool)> = conn
            .query_row(
                "SELECT status, url, check_interval_secs, stable_streak, favorite, is_read
                 FROM saves WHERE id = ?1",
                params![id],
                |r| {
                    let cadence = Cadence {
                        interval_secs: r.get(2)?,
                        stable_streak: r.get(3)?,
                    };
                    let hot = r.get::<_, bool>(4)? || !r.get::<_, bool>(5)?;
                    Ok((r.get(0)?, r.get(1)?, cadence, hot))
                },
            )
            .optional()?;
        let (cadence, hot) = previous
            .as_ref()
            .map(|(_, _, c, hot)| (*c, *hot))
            .unwrap_or_default();
        let cadence = schedule::next_cadence(outcome.status, cadence, hot);
        let now = now();

        // Empty extracted text is not an archive — don't clobber a previous
        // good snapshot with it.
        let archive = outcome
            .archive_text
            .as_deref()
            .filter(|t| !t.trim().is_empty());
        let changed = conn.execute(
            "UPDATE saves SET
                status = ?1,
                http_status = ?2,
                redirect_url = COALESCE(?3, redirect_url),
                content_hash = COALESCE(?4, content_hash),
                -- Stamp the basis alongside the hash, so a future change to
                -- the hashing rule re-baselines instead of crying `changed`.
                content_hash_algo = CASE
                    WHEN ?4 IS NOT NULL THEN ?8 ELSE content_hash_algo
                END,
                archive_text = COALESCE(?5, archive_text),
                archived_at = CASE WHEN ?5 IS NOT NULL THEN ?6 ELSE archived_at END,
                -- Stamp the first time it goes dead; clear it on recovery.
                dead_since = CASE
                    WHEN ?1 = 'dead' THEN COALESCE(dead_since, ?6)
                    ELSE NULL
                END,
                last_checked_at = ?6,
                -- Validators from this response. A 304 leaves them alone
                -- unless it restated one; a 200 always writes, so a server
                -- that stopped sending one stops being revalidated against
                -- a value it no longer honours.
                etag = COALESCE(?9, etag),
                last_modified = COALESCE(?10, last_modified),
                check_interval_secs = ?11,
                stable_streak = ?12,
                next_check_at = ?13
             WHERE id = ?7",
            params![
                outcome.status.as_str(),
                outcome.http_status.map(i64::from),
                outcome.redirect_url,
                outcome.content_hash,
                archive,
                now,
                id,
                probe::HASH_ALGO as i64,
                outcome.etag,
                outcome.last_modified,
                cadence.interval_secs,
                cadence.stable_streak,
                schedule::due_at(now, cadence, id),
            ],
        )?;
        if changed == 0 {
            return Err(Error::NotFound(id));
        }
        if let Some((old_status, url, _, _)) = previous {
            if old_status != outcome.status.as_str() {
                log::info!(
                    "link check: #{id} {url} {old_status} -> {} (http {:?})",
                    outcome.status.as_str(),
                    outcome.http_status
                );
            } else {
                log::debug!(
                    "link check: #{id} {url} still {old_status}{}, next in {}h",
                    if outcome.not_modified { " (304)" } else { "" },
                    cadence.interval_secs / 3600
                );
            }
        }
        Ok(())
    }

    /// The stored check state for one save.
    pub fn check_target(&self, id: i64) -> Result<CheckTarget> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            &format!("SELECT {CHECK_COLS} FROM saves WHERE id = ?1"),
            params![id],
            check_target,
        )
        .optional()?
        .ok_or(Error::NotFound(id))
    }

    /// Check a single save right now (blocking network call) and persist the result.
    pub fn check_save(&self, id: i64) -> Result<Save> {
        let target = self.check_target(id)?;
        let outcome = probe::check(&target);
        self.apply_check(id, &outcome)?;
        self.maybe_fetch_thumbnail(id, outcome.og_image.as_deref());
        self.get_save(id)
    }
}
