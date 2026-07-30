//! Where a sweep's next batch of targets comes from.

use crate::probe::CheckTarget;
use crate::vault::Vault;

pub(super) enum TargetCursor {
    /// Saves that are due. Each pass re-queries rather than paging, because
    /// checking a save reschedules it out of the due set — so the next query
    /// naturally returns the next window.
    ///
    /// `skipped` is the exception that makes this correct. Links abandoned by
    /// the circuit breaker stay due, and being the oldest-due rows they sort
    /// to the front of every subsequent query. Without offsetting past them
    /// the sweep would be handed the same batch forever.
    Due { skipped: i64 },
    /// Every save, paged by id — a user-forced full re-check. Not by due
    /// time: a forced sweep rewrites `next_check_at` as it goes, so paging on
    /// that column would reshuffle the cursor underneath itself and skip rows.
    All { after_id: i64 },
}

impl TargetCursor {
    pub(super) fn next_batch(
        &mut self,
        vault: &Vault,
        batch: i64,
    ) -> crate::Result<Vec<CheckTarget>> {
        match self {
            TargetCursor::Due { skipped } => vault.saves_due_for_check(batch, *skipped),
            TargetCursor::All { after_id } => {
                let targets = vault.all_check_targets(*after_id, batch)?;
                if let Some(last) = targets.last() {
                    *after_id = last.id;
                }
                Ok(targets)
            }
        }
    }

    /// Report links the sweep chose not to check, so the next query steps
    /// over them instead of returning them again.
    pub(super) fn note_skipped(&mut self, count: usize) {
        if let TargetCursor::Due { skipped } = self {
            *skipped += count as i64;
        }
    }
}
