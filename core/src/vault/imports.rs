//! Bringing in bookmarks from other tools.
//!
//! [`Vault::preview_import`] answers "what would this do?" without
//! touching the database, so the UI can show a real diff before the user
//! commits to it.

use rusqlite::{params, OptionalExtension};

use crate::error::Result;
use crate::import::{ImportItem, ImportReport};

use super::tags::set_tags_on;
use super::tags::tags_for;
use super::url_normalization::normalize_url;
use super::{now, Vault};

impl Vault {
    /// Dry-run: counts what an import would do, without writing anything.
    pub fn preview_import(&self, items: &[ImportItem]) -> Result<ImportReport> {
        let conn = self.conn.lock().unwrap();
        let mut report = ImportReport {
            total: items.len() as u32,
            ..Default::default()
        };
        let mut stmt = conn.prepare("SELECT 1 FROM saves WHERE url = ?1")?;
        for item in items {
            match normalize_url(&item.url) {
                Err(_) => report.invalid += 1,
                Ok(url) => {
                    if stmt.exists(params![url])? {
                        report.existing += 1;
                    } else {
                        report.new += 1;
                    }
                }
            }
        }
        Ok(report)
    }

    /// Merge parsed bookmarks into the vault in one transaction.
    ///
    /// Import never destroys user data: existing saves only get empty
    /// fields filled in, tags are unioned, `favorite` is sticky once set,
    /// and `created_at` keeps the earliest known save date.
    pub fn import_items(&self, items: &[ImportItem]) -> Result<ImportReport> {
        let mut report = ImportReport {
            total: items.len() as u32,
            ..Default::default()
        };
        let now = now();
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        for item in items {
            let Ok(url) = normalize_url(&item.url) else {
                report.invalid += 1;
                continue;
            };
            let existing: Option<i64> = tx
                .query_row("SELECT id FROM saves WHERE url = ?1", params![url], |r| {
                    r.get(0)
                })
                .optional()?;

            let id = match existing {
                Some(id) => {
                    tx.execute(
                        "UPDATE saves SET
                            title = CASE WHEN title = '' THEN ?1 ELSE title END,
                            description = CASE WHEN description = '' THEN ?2 ELSE description END,
                            notes = CASE WHEN notes = '' THEN ?3 ELSE notes END,
                            favorite = MAX(favorite, ?4),
                            created_at = MIN(created_at, COALESCE(?5, created_at)),
                            updated_at = ?6
                         WHERE id = ?7",
                        params![
                            item.title,
                            item.description,
                            item.notes,
                            item.favorite,
                            item.created_at,
                            now,
                            id
                        ],
                    )?;
                    report.existing += 1;
                    id
                }
                None => {
                    tx.execute(
                        "INSERT INTO saves
                            (url, title, description, notes, favorite, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                        params![
                            url,
                            item.title,
                            item.description,
                            item.notes,
                            item.favorite,
                            item.created_at.unwrap_or(now),
                            now
                        ],
                    )?;
                    report.new += 1;
                    tx.last_insert_rowid()
                }
            };

            if !item.tags.is_empty() {
                let mut tags = tags_for(&tx, id)?;
                for tag in &item.tags {
                    if !tags.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
                        tags.push(tag.clone());
                    }
                }
                set_tags_on(&tx, id, &tags)?;
            }
        }

        tx.commit()?;
        log::info!(
            "import: {} new, {} merged, {} invalid (of {})",
            report.new,
            report.existing,
            report.invalid,
            report.total
        );
        Ok(report)
    }
}
