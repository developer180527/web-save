//! Vault-wide counts, for the sidebar and for sweep progress.

use crate::error::Result;
use crate::models::VaultStats;

use super::Vault;

impl Vault {
    pub fn stats(&self) -> Result<VaultStats> {
        let conn = self.conn.lock().unwrap();
        let stats = conn.query_row(
            "SELECT COUNT(*),
                    COALESCE(SUM(favorite), 0),
                    COALESCE(SUM(is_read = 0), 0),
                    COALESCE(SUM(status = 'unchecked'), 0),
                    COALESCE(SUM(status = 'active'), 0),
                    COALESCE(SUM(status = 'changed'), 0),
                    COALESCE(SUM(status = 'redirected'), 0),
                    COALESCE(SUM(status = 'unreachable'), 0),
                    COALESCE(SUM(status = 'dead'), 0)
             FROM saves",
            [],
            |r| {
                Ok(VaultStats {
                    total: r.get(0)?,
                    favorites: r.get(1)?,
                    unread: r.get(2)?,
                    unchecked: r.get(3)?,
                    active: r.get(4)?,
                    changed: r.get(5)?,
                    redirected: r.get(6)?,
                    unreachable: r.get(7)?,
                    dead: r.get(8)?,
                })
            },
        )?;
        Ok(stats)
    }

    // ---- import ----

    pub fn count_saves(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.query_row("SELECT COUNT(*) FROM saves", [], |r| r.get(0))?)
    }
}
