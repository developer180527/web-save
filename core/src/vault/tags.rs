//! Tags: the normalized many-to-many side, plus the denormalized
//! `tags_text` column the FTS index reads.
//!
//! Writes go through [`set_tags_on`] so both representations can never
//! drift apart, and tags orphaned by their last save are cleaned up.

use rusqlite::params;
use rusqlite::Connection;

use crate::error::Result;
use crate::models::TagCount;

use super::save_rows::get_save_on;
use super::{now, Vault};
use crate::error::Error;
use crate::models::Save;
use rusqlite::OptionalExtension;

pub(super) fn tags_for(conn: &Connection, id: i64) -> Result<Vec<String>> {
    let mut stmt = conn.prepare_cached(
        "SELECT t.name FROM tags t
         JOIN save_tags st ON st.tag_id = t.id
         WHERE st.save_id = ?1 ORDER BY t.name COLLATE NOCASE",
    )?;
    let tags = stmt
        .query_map(params![id], |r| r.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(tags)
}

/// Replace the tag set of a save inside an open transaction/connection.
/// Also refreshes the denormalized `tags_text` column that feeds FTS.
pub(super) fn set_tags_on(conn: &Connection, id: i64, tags: &[String]) -> Result<()> {
    let mut clean: Vec<String> = Vec::new();
    for tag in tags {
        let t = tag.trim();
        if t.is_empty() {
            continue;
        }
        if !clean.iter().any(|c| c.eq_ignore_ascii_case(t)) {
            clean.push(t.to_string());
        }
    }

    conn.execute("DELETE FROM save_tags WHERE save_id = ?1", params![id])?;
    for tag in &clean {
        conn.execute(
            "INSERT INTO tags (name) VALUES (?1) ON CONFLICT(name) DO NOTHING",
            params![tag],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO save_tags (save_id, tag_id)
             VALUES (?1, (SELECT id FROM tags WHERE name = ?2))",
            params![id, tag],
        )?;
    }
    conn.execute(
        "DELETE FROM tags WHERE id NOT IN (SELECT DISTINCT tag_id FROM save_tags)",
        [],
    )?;
    conn.execute(
        "UPDATE saves SET tags_text = ?1, updated_at = ?2 WHERE id = ?3",
        params![clean.join(" "), now(), id],
    )?;
    Ok(())
}

impl Vault {
    /// Replace the tag set of a save. Tags are deduplicated case-insensitively
    /// and orphaned tags are removed from the vault.
    pub fn set_tags(&self, id: i64, tags: &[String]) -> Result<Save> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let exists: Option<i64> = tx
            .query_row("SELECT id FROM saves WHERE id = ?1", params![id], |r| {
                r.get(0)
            })
            .optional()?;
        if exists.is_none() {
            return Err(Error::NotFound(id));
        }
        set_tags_on(&tx, id, tags)?;
        tx.commit()?;
        log::debug!("set_tags: #{id} -> {tags:?}");
        get_save_on(&conn, id)
    }

    /// Add one tag to each save (existing tags kept).
    pub fn add_tag_many(&self, ids: &[i64], tag: &str) -> Result<()> {
        let tag = tag.trim();
        if tag.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        for id in ids {
            let mut tags = tags_for(&tx, *id)?;
            if !tags.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
                tags.push(tag.to_string());
                set_tags_on(&tx, *id, &tags)?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    // ---- saved searches ----

    pub fn list_tags(&self) -> Result<Vec<TagCount>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT t.name, COUNT(st.save_id) FROM tags t
             LEFT JOIN save_tags st ON st.tag_id = t.id
             GROUP BY t.id ORDER BY t.name COLLATE NOCASE",
        )?;
        let tags = stmt
            .query_map([], |r| {
                Ok(TagCount {
                    name: r.get(0)?,
                    count: r.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(tags)
    }
}
