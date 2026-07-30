//! Creating, reading, editing and deleting saves.
//!
//! The vault's core CRUD. Search lives in [`super::search`], tags in
//! [`super::tags`], and link health in [`super::link_checks`].

use rusqlite::{params, OptionalExtension};

use crate::error::{Error, Result};
use crate::models::{NewSave, Save, SavePatch};

use super::save_rows::row_to_save;
use super::save_rows::{get_save_on, SAVE_COLS};
use super::tags::set_tags_on;
use super::tags::tags_for;
use super::url_normalization::normalize_url;
use super::{now, Vault};
use crate::schedule;

impl Vault {
    /// Insert a capture, or refresh metadata if the URL is already saved.
    /// Returns the resulting save either way.
    pub fn add_save(&self, new: NewSave) -> Result<Save> {
        let url = normalize_url(&new.url)?;
        let now = now();
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        let existing: Option<i64> = tx
            .query_row("SELECT id FROM saves WHERE url = ?1", params![url], |r| {
                r.get(0)
            })
            .optional()?;

        let id = match existing {
            Some(id) => {
                log::debug!("add_save: refreshing existing save #{id} for {url}");
                tx.execute(
                    "UPDATE saves SET
                        title = CASE WHEN ?1 = '' THEN title ELSE ?1 END,
                        description = CASE WHEN ?2 = '' THEN description ELSE ?2 END,
                        favicon_url = CASE WHEN ?3 = '' THEN favicon_url ELSE ?3 END,
                        updated_at = ?4
                     WHERE id = ?5",
                    params![new.title, new.description, new.favicon_url, now, id],
                )?;
                id
            }
            None => {
                tx.execute(
                    "INSERT INTO saves (url, title, description, favicon_url, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                    params![url, new.title, new.description, new.favicon_url, now],
                )?;
                let id = tx.last_insert_rowid();
                log::info!("add_save: captured #{id} {url}");
                id
            }
        };

        if !new.tags.is_empty() {
            set_tags_on(&tx, id, &new.tags)?;
        }

        tx.commit()?;
        get_save_on(&conn, id)
    }

    pub fn get_save(&self, id: i64) -> Result<Save> {
        let conn = self.conn.lock().unwrap();
        get_save_on(&conn, id)
    }

    /// Update user-editable metadata; `None` fields are left untouched.
    pub fn update_save(&self, id: i64, patch: SavePatch) -> Result<Save> {
        let conn = self.conn.lock().unwrap();
        let changed = conn.execute(
            "UPDATE saves SET
                title = COALESCE(?1, title),
                description = COALESCE(?2, description),
                notes = COALESCE(?3, notes),
                favicon_url = COALESCE(?4, favicon_url),
                updated_at = ?5
             WHERE id = ?6",
            params![
                patch.title,
                patch.description,
                patch.notes,
                patch.favicon_url,
                now(),
                id
            ],
        )?;
        if changed == 0 {
            return Err(Error::NotFound(id));
        }
        log::debug!("update_save: edited metadata of #{id}");
        get_save_on(&conn, id)
    }

    pub fn set_favorite(&self, id: i64, favorite: bool) -> Result<Save> {
        let conn = self.conn.lock().unwrap();
        let changed = conn.execute(
            "UPDATE saves SET favorite = ?1, updated_at = ?2 WHERE id = ?3",
            params![favorite, now(), id],
        )?;
        if changed == 0 {
            return Err(Error::NotFound(id));
        }
        log::debug!("set_favorite: #{id} -> {favorite}");
        get_save_on(&conn, id)
    }

    pub fn set_read(&self, id: i64, is_read: bool) -> Result<Save> {
        let conn = self.conn.lock().unwrap();
        let changed = conn.execute(
            "UPDATE saves SET is_read = ?1 WHERE id = ?2",
            params![is_read, id],
        )?;
        if changed == 0 {
            return Err(Error::NotFound(id));
        }
        get_save_on(&conn, id)
    }

    // ---- bulk operations (single transaction each) ----

    pub fn set_favorite_many(&self, ids: &[i64], favorite: bool) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        for id in ids {
            tx.execute(
                "UPDATE saves SET favorite = ?1, updated_at = ?2 WHERE id = ?3",
                params![favorite, now(), id],
            )?;
        }
        tx.commit()?;
        log::debug!("bulk favorite={favorite} for {} save(s)", ids.len());
        Ok(())
    }

    pub fn set_read_many(&self, ids: &[i64], is_read: bool) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        for id in ids {
            tx.execute(
                "UPDATE saves SET is_read = ?1 WHERE id = ?2",
                params![is_read, id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn delete_many(&self, ids: &[i64]) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        for id in ids {
            tx.execute("DELETE FROM saves WHERE id = ?1", params![id])?;
        }
        tx.execute(
            "DELETE FROM tags WHERE id NOT IN (SELECT DISTINCT tag_id FROM save_tags)",
            [],
        )?;
        tx.commit()?;
        log::info!("bulk delete: removed {} save(s)", ids.len());
        Ok(())
    }

    pub fn delete_save(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let changed = conn.execute("DELETE FROM saves WHERE id = ?1", params![id])?;
        if changed == 0 {
            return Err(Error::NotFound(id));
        }
        conn.execute(
            "DELETE FROM tags WHERE id NOT IN (SELECT DISTINCT tag_id FROM save_tags)",
            [],
        )?;
        log::info!("delete_save: removed #{id}");
        Ok(())
    }

    /// Point a save at a new URL — the "accept redirect" workflow. Check
    /// state resets so the monitor re-verifies the new location promptly.
    pub fn set_url(&self, id: i64, new_url: &str) -> Result<Save> {
        let url = normalize_url(new_url)?;
        let conn = self.conn.lock().unwrap();
        let taken: Option<i64> = conn
            .query_row(
                "SELECT id FROM saves WHERE url = ?1 AND id != ?2",
                params![url, id],
                |r| r.get(0),
            )
            .optional()?;
        if taken.is_some() {
            return Err(Error::Conflict(format!("{url} is already saved")));
        }
        let changed = conn.execute(
            // A different URL is a different page: the old validators and the
            // backoff it had earned belong to the old one. Reset to the floor
            // and mark it due now.
            "UPDATE saves SET url = ?1, redirect_url = '', status = 'unchecked',
                content_hash = '', content_hash_algo = 0,
                etag = '', last_modified = '',
                check_interval_secs = ?4, stable_streak = 0, next_check_at = 0,
                http_status = NULL, last_checked_at = NULL,
                updated_at = ?2
             WHERE id = ?3",
            params![url, now(), id, schedule::DEFAULT_INTERVAL_SECS],
        )?;
        if changed == 0 {
            return Err(Error::NotFound(id));
        }
        log::info!("set_url: #{id} now points at {url}");
        get_save_on(&conn, id)
    }

    /// Every save, oldest first — used for export.
    pub fn all_saves(&self) -> Result<Vec<Save>> {
        let conn = self.conn.lock().unwrap();
        // Tie-break on id so exports are byte-stable between runs.
        let mut stmt = conn.prepare(&format!(
            "SELECT {SAVE_COLS} FROM saves s ORDER BY s.created_at, s.id"
        ))?;
        let mut saves = stmt
            .query_map([], row_to_save)?
            .collect::<rusqlite::Result<Vec<Save>>>()?;
        for save in &mut saves {
            save.tags = tags_for(&conn, save.id)?;
        }
        Ok(saves)
    }

    // ---- generic metadata key-value store ----
    //
    // Host-defined keys only; the core attaches no meaning to them. Used by
    // the desktop app to remember, e.g., when the browser extension last
    // delivered a capture.

    /// The archived readable text of a save, if a snapshot exists.
    pub fn archive_text(&self, id: i64) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let text: String = conn
            .query_row(
                "SELECT archive_text FROM saves WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or(Error::NotFound(id))?;
        Ok((!text.is_empty()).then_some(text))
    }

    // ---- link monitoring ----
}
