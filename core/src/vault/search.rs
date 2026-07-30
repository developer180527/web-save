//! Listing and full-text search over the vault.
//!
//! Filters combine with AND; free text goes through FTS5 with weighted
//! BM25 so fields the user authored outrank matches deep in an archived
//! page body.

use rusqlite::params_from_iter;
use rusqlite::types::Value;

use crate::error::Result;
use crate::models::{ListQuery, Save};

use super::save_rows::{row_to_save, SAVE_COLS};
use super::tags::tags_for;
use super::Vault;

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

impl Vault {
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
        if q.unread_only {
            where_clauses.push("s.is_read = 0".into());
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
            // Weighted relevance: user-authored fields (title, notes, tags)
            // outrank deep matches inside archived page text.
            " ORDER BY bm25(saves_fts, 12.0, 6.0, 4.0, 8.0, 10.0, 1.0)"
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
}
