//! Turning `saves` rows into [`Save`] values.
//!
//! One column list and one row mapper, shared by every query that returns
//! whole saves — so adding a field is a change in one place rather than a
//! hunt through every `SELECT` in the crate.

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::error::{Error, Result};
use crate::models::{LinkStatus, Save};

use super::tags::tags_for;

pub(super) const SAVE_COLS: &str =
    "s.id, s.url, s.title, s.description, s.notes, s.favicon_url, s.favorite, \
     s.status, s.redirect_url, s.http_status, s.created_at, s.updated_at, s.last_checked_at, \
     s.thumbnail, s.archived_at, s.is_read, s.dead_since";

pub(super) fn row_to_save(row: &Row) -> rusqlite::Result<Save> {
    Ok(Save {
        id: row.get(0)?,
        url: row.get(1)?,
        title: row.get(2)?,
        description: row.get(3)?,
        notes: row.get(4)?,
        favicon_url: row.get(5)?,
        favorite: row.get(6)?,
        status: LinkStatus::parse(&row.get::<_, String>(7)?),
        redirect_url: row.get(8)?,
        http_status: row.get(9)?,
        tags: Vec::new(),
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
        last_checked_at: row.get(12)?,
        thumbnail: row.get(13)?,
        archived_at: row.get(14)?,
        is_read: row.get(15)?,
        dead_since: row.get(16)?,
    })
}

pub(super) fn get_save_on(conn: &Connection, id: i64) -> Result<Save> {
    let mut save = conn
        .query_row(
            &format!("SELECT {SAVE_COLS} FROM saves s WHERE s.id = ?1"),
            params![id],
            row_to_save,
        )
        .optional()?
        .ok_or(Error::NotFound(id))?;
    save.tags = tags_for(conn, id)?;
    Ok(save)
}
