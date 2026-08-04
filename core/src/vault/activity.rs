//! A durable record of what the app did, readable from inside the app.
//!
//! The iOS background sweep is the reason this exists. It runs with no Flutter
//! engine, on a schedule the OS decides, usually while the phone is in a
//! pocket — so the only evidence it ran at all is what it writes down. os_log
//! would show it in Console.app on a tethered Mac, which does not help someone
//! asking why their links stopped being checked.
//!
//! Writes must never break the thing they are describing. Every function here
//! swallows its own failure: a log that cannot be written is worth strictly
//! less than the sweep it was recording.

use rusqlite::params;

use super::Vault;
use crate::error::Result;

/// Entries kept. Old rows are trimmed on insert.
///
/// This answers "what happened recently", not "what has ever happened", and an
/// unbounded log on a phone is a slow leak nobody goes looking for.
const MAX_ENTRIES: i64 = 500;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityEntry {
    pub id: i64,
    /// Unix seconds.
    pub at: i64,
    /// "info", "warn" or "error".
    pub level: String,
    /// Which part of the app spoke: "background", "monitor", "import", "app".
    pub source: String,
    pub message: String,
}

impl Vault {
    /// Record one entry. Never fails the caller.
    ///
    /// Returns nothing on purpose: there is no useful way for a caller to
    /// respond to a log write failing, and making it a Result invites
    /// `?` in places where losing the real error would be worse.
    pub fn log_activity(&self, level: &str, source: &str, message: &str) {
        let _ = self.try_log_activity(level, source, message);
    }

    fn try_log_activity(&self, level: &str, source: &str, message: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO activity_log (at, level, source, message) VALUES (?1, ?2, ?3, ?4)",
            params![super::now(), level, source, message],
        )?;
        // Trim here rather than on a timer: this is the only moment the table
        // is known to have grown, and it is already a write.
        conn.execute(
            "DELETE FROM activity_log WHERE id <= (
                 SELECT id FROM activity_log ORDER BY id DESC LIMIT 1 OFFSET ?1
             )",
            params![MAX_ENTRIES],
        )?;
        Ok(())
    }

    /// Most recent first.
    pub fn recent_activity(&self, limit: i64) -> Result<Vec<ActivityEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, at, level, source, message
               FROM activity_log
              ORDER BY at DESC, id DESC
              LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit], |row| {
                Ok(ActivityEntry {
                    id: row.get(0)?,
                    at: row.get(1)?,
                    level: row.get(2)?,
                    source: row.get(3)?,
                    message: row.get(4)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn clear_activity(&self) -> Result<()> {
        self.conn.lock().unwrap().execute("DELETE FROM activity_log", [])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vault() -> (tempfile::TempDir, Vault) {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(dir.path()).unwrap();
        (dir, vault)
    }

    #[test]
    fn records_and_reads_back_newest_first() {
        let (_dir, vault) = vault();
        vault.log_activity("info", "background", "first");
        vault.log_activity("warn", "monitor", "second");

        let entries = vault.recent_activity(10).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].message, "second");
        assert_eq!(entries[0].level, "warn");
        assert_eq!(entries[1].source, "background");
    }

    /// The cap is what keeps this from being a slow leak on a phone.
    #[test]
    fn trims_to_the_cap() {
        let (_dir, vault) = vault();
        for i in 0..(MAX_ENTRIES + 40) {
            vault.log_activity("info", "test", &format!("entry {i}"));
        }
        let all = vault.recent_activity(MAX_ENTRIES * 2).unwrap();
        assert!(
            all.len() as i64 <= MAX_ENTRIES,
            "kept {} entries, cap is {MAX_ENTRIES}",
            all.len()
        );
        // The newest survive; the oldest are the ones dropped.
        assert!(all[0].message.contains(&format!("{}", MAX_ENTRIES + 39)));
    }

    #[test]
    fn clearing_empties_it() {
        let (_dir, vault) = vault();
        vault.log_activity("info", "test", "something");
        vault.clear_activity().unwrap();
        assert!(vault.recent_activity(10).unwrap().is_empty());
    }
}
