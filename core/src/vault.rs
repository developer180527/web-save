use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};

use crate::error::{Error, Result};
use crate::models::{LinkStatus, ListQuery, NewSave, Save, SavePatch, TagCount, VaultStats};
use crate::monitor::{self, CheckOutcome, CheckTarget};

pub const DB_FILE: &str = "websave.db";
pub const ASSETS_DIR: &str = "assets";

/// Schema migrations, applied in order and tracked via `PRAGMA user_version`.
const MIGRATIONS: &[&str] = &[r#"
CREATE TABLE saves (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    url TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    notes TEXT NOT NULL DEFAULT '',
    favicon_url TEXT NOT NULL DEFAULT '',
    favorite INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'unchecked',
    redirect_url TEXT NOT NULL DEFAULT '',
    http_status INTEGER,
    content_hash TEXT NOT NULL DEFAULT '',
    tags_text TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    last_checked_at INTEGER
);

CREATE INDEX idx_saves_created ON saves(created_at DESC);
CREATE INDEX idx_saves_favorite ON saves(favorite) WHERE favorite = 1;
CREATE INDEX idx_saves_status ON saves(status);

CREATE TABLE tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE
);

CREATE TABLE save_tags (
    save_id INTEGER NOT NULL REFERENCES saves(id) ON DELETE CASCADE,
    tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (save_id, tag_id)
) WITHOUT ROWID;

CREATE VIRTUAL TABLE saves_fts USING fts5(
    title, url, description, notes, tags,
    content='saves',
    content_rowid='id',
    tokenize='unicode61 remove_diacritics 2'
);

CREATE TRIGGER saves_fts_insert AFTER INSERT ON saves BEGIN
    INSERT INTO saves_fts(rowid, title, url, description, notes, tags)
    VALUES (new.id, new.title, new.url, new.description, new.notes, new.tags_text);
END;

CREATE TRIGGER saves_fts_delete AFTER DELETE ON saves BEGIN
    INSERT INTO saves_fts(saves_fts, rowid, title, url, description, notes, tags)
    VALUES ('delete', old.id, old.title, old.url, old.description, old.notes, old.tags_text);
END;

CREATE TRIGGER saves_fts_update AFTER UPDATE ON saves BEGIN
    INSERT INTO saves_fts(saves_fts, rowid, title, url, description, notes, tags)
    VALUES ('delete', old.id, old.title, old.url, old.description, old.notes, old.tags_text);
    INSERT INTO saves_fts(rowid, title, url, description, notes, tags)
    VALUES (new.id, new.title, new.url, new.description, new.notes, new.tags_text);
END;
"#];

const SAVE_COLS: &str = "s.id, s.url, s.title, s.description, s.notes, s.favicon_url, s.favorite, \
     s.status, s.redirect_url, s.http_status, s.created_at, s.updated_at, s.last_checked_at";

/// A portable bookmark vault: a directory holding the SQLite database and
/// optional local assets (thumbnails, etc.). All storage, search and
/// validation logic lives here, independent of any UI framework.
pub struct Vault {
    conn: Mutex<Connection>,
    root: PathBuf,
}

impl Vault {
    /// Open (or create) a vault rooted at `root`.
    pub fn open(root: impl Into<PathBuf>) -> Result<Vault> {
        let root = root.into();
        fs::create_dir_all(root.join(ASSETS_DIR)).map_err(|source| Error::Io {
            path: root.join(ASSETS_DIR),
            source,
        })?;
        let conn = Connection::open(root.join(DB_FILE))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrate(&conn)?;
        log::info!("vault opened at {}", root.display());
        Ok(Vault {
            conn: Mutex::new(conn),
            root,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

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

    /// List saves with optional full-text query and filters.
    /// FTS results are ordered by relevance; plain listings by recency.
    pub fn list_saves(&self, q: &ListQuery) -> Result<Vec<Save>> {
        let conn = self.conn.lock().unwrap();
        let mut sql = format!("SELECT {SAVE_COLS} FROM saves s");
        let mut where_clauses: Vec<String> = Vec::new();
        let mut values: Vec<Value> = Vec::new();

        let fts = q.query.as_deref().and_then(fts_expression);
        if let Some(expr) = &fts {
            sql.push_str(" JOIN saves_fts ON saves_fts.rowid = s.id");
            values.push(Value::Text(expr.clone()));
            where_clauses.push(format!("saves_fts MATCH ?{}", values.len()));
        }
        if q.favorites_only {
            where_clauses.push("s.favorite = 1".into());
        }
        if let Some(status) = q.status {
            values.push(Value::Text(status.as_str().into()));
            where_clauses.push(format!("s.status = ?{}", values.len()));
        }
        if let Some(tag) = q.tag.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
            values.push(Value::Text(tag.into()));
            where_clauses.push(format!(
                "s.id IN (SELECT st.save_id FROM save_tags st
                          JOIN tags t ON t.id = st.tag_id WHERE t.name = ?{})",
                values.len()
            ));
        }

        if !where_clauses.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&where_clauses.join(" AND "));
        }
        sql.push_str(if fts.is_some() {
            " ORDER BY saves_fts.rank"
        } else {
            " ORDER BY s.created_at DESC, s.id DESC"
        });

        values.push(Value::Integer(q.limit.unwrap_or(200).clamp(1, 1000)));
        sql.push_str(&format!(" LIMIT ?{}", values.len()));
        values.push(Value::Integer(q.offset.unwrap_or(0).max(0)));
        sql.push_str(&format!(" OFFSET ?{}", values.len()));

        let mut stmt = conn.prepare(&sql)?;
        let mut saves = stmt
            .query_map(params_from_iter(values), row_to_save)?
            .collect::<rusqlite::Result<Vec<Save>>>()?;
        for save in &mut saves {
            save.tags = tags_for(&conn, save.id)?;
        }
        log::trace!(
            "list_saves: {} hits (query={:?}, tag={:?}, favorites={}, status={:?})",
            saves.len(),
            q.query,
            q.tag,
            q.favorites_only,
            q.status.map(|s| s.as_str())
        );
        Ok(saves)
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

    pub fn stats(&self) -> Result<VaultStats> {
        let conn = self.conn.lock().unwrap();
        let stats = conn.query_row(
            "SELECT COUNT(*),
                    COALESCE(SUM(favorite), 0),
                    COALESCE(SUM(status = 'unchecked'), 0),
                    COALESCE(SUM(status = 'active'), 0),
                    COALESCE(SUM(status = 'changed'), 0),
                    COALESCE(SUM(status = 'redirected'), 0),
                    COALESCE(SUM(status = 'dead'), 0)
             FROM saves",
            [],
            |r| {
                Ok(VaultStats {
                    total: r.get(0)?,
                    favorites: r.get(1)?,
                    unchecked: r.get(2)?,
                    active: r.get(3)?,
                    changed: r.get(4)?,
                    redirected: r.get(5)?,
                    dead: r.get(6)?,
                })
            },
        )?;
        Ok(stats)
    }

    // ---- link monitoring ----

    /// Saves whose last check is older than `max_age_secs` (or never checked),
    /// oldest first. Returns lightweight targets so the caller can perform
    /// network checks without holding the vault lock.
    pub fn saves_due_for_check(&self, max_age_secs: i64, limit: i64) -> Result<Vec<CheckTarget>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, url, content_hash FROM saves
             WHERE last_checked_at IS NULL OR last_checked_at < ?1
             ORDER BY COALESCE(last_checked_at, 0) ASC LIMIT ?2",
        )?;
        let targets = stmt
            .query_map(params![now() - max_age_secs, limit], |r| {
                Ok(CheckTarget {
                    id: r.get(0)?,
                    url: r.get(1)?,
                    content_hash: r.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(targets)
    }

    /// Persist the outcome of a link check.
    pub fn apply_check(&self, id: i64, outcome: &CheckOutcome) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let previous: Option<(String, String)> = conn
            .query_row(
                "SELECT status, url FROM saves WHERE id = ?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let changed = conn.execute(
            "UPDATE saves SET
                status = ?1,
                http_status = ?2,
                redirect_url = COALESCE(?3, redirect_url),
                content_hash = COALESCE(?4, content_hash),
                last_checked_at = ?5
             WHERE id = ?6",
            params![
                outcome.status.as_str(),
                outcome.http_status.map(i64::from),
                outcome.redirect_url,
                outcome.content_hash,
                now(),
                id
            ],
        )?;
        if changed == 0 {
            return Err(Error::NotFound(id));
        }
        if let Some((old_status, url)) = previous {
            if old_status != outcome.status.as_str() {
                log::info!(
                    "link check: #{id} {url} {old_status} -> {} (http {:?})",
                    outcome.status.as_str(),
                    outcome.http_status
                );
            } else {
                log::debug!("link check: #{id} {url} still {old_status}");
            }
        }
        Ok(())
    }

    /// Check a single save right now (blocking network call) and persist the result.
    pub fn check_save(&self, id: i64) -> Result<Save> {
        let target = {
            let conn = self.conn.lock().unwrap();
            conn.query_row(
                "SELECT id, url, content_hash FROM saves WHERE id = ?1",
                params![id],
                |r| {
                    Ok(CheckTarget {
                        id: r.get(0)?,
                        url: r.get(1)?,
                        content_hash: r.get(2)?,
                    })
                },
            )
            .optional()?
            .ok_or(Error::NotFound(id))?
        };
        let outcome = monitor::check_url(&target.url, &target.content_hash);
        self.apply_check(id, &outcome)?;
        self.get_save(id)
    }
}

fn migrate(conn: &Connection) -> Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
        conn.execute_batch(sql)?;
        conn.pragma_update(None, "user_version", (i + 1) as i64)?;
    }
    Ok(())
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn normalize_url(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    let parsed = url::Url::parse(trimmed).map_err(|_| Error::InvalidUrl(trimmed.to_string()))?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(Error::InvalidUrl(trimmed.to_string()));
    }
    Ok(parsed.to_string())
}

/// Build an FTS5 MATCH expression from free-form user input: each token is
/// quoted (so FTS5 operators/punctuation can't break the query) and matched
/// by prefix.
fn fts_expression(input: &str) -> Option<String> {
    let tokens: Vec<String> = input
        .split_whitespace()
        .map(|t| t.replace('"', ""))
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{t}\"*"))
        .collect();
    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join(" "))
    }
}

fn row_to_save(row: &Row) -> rusqlite::Result<Save> {
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
    })
}

fn get_save_on(conn: &Connection, id: i64) -> Result<Save> {
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

fn tags_for(conn: &Connection, id: i64) -> Result<Vec<String>> {
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
fn set_tags_on(conn: &Connection, id: i64, tags: &[String]) -> Result<()> {
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
