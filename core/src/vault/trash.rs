//! The Trash grace period.
//!
//! Dead links are quarantined rather than deleted: they keep being health
//! checked, recover automatically if the site comes back (which clears
//! `dead_since`), and only links that stayed dead for the whole grace
//! period are removed.

use rusqlite::params;

use crate::error::Result;

use super::{now, Vault};

impl Vault {
    /// Permanently delete saves that have been continuously dead for longer
    /// than `grace_secs`. Recovered links clear `dead_since`, so only links
    /// that stayed dead for the whole window are removed. Returns the count.
    pub fn purge_expired_dead(&self, grace_secs: i64) -> Result<u32> {
        let cutoff = now() - grace_secs;
        let conn = self.conn.lock().unwrap();
        let removed = conn.execute(
            "DELETE FROM saves
             WHERE status = 'dead' AND dead_since IS NOT NULL AND dead_since <= ?1",
            params![cutoff],
        )?;
        if removed > 0 {
            conn.execute(
                "DELETE FROM tags WHERE id NOT IN (SELECT DISTINCT tag_id FROM save_tags)",
                [],
            )?;
            log::info!("trash: purged {removed} link(s) dead longer than {grace_secs}s");
        }
        Ok(removed as u32)
    }
}
