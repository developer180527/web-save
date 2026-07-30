//! Persisted query + filter combinations, shown in the sidebar.

use rusqlite::params;

use crate::error::Result;
use crate::models::{ListQuery, SavedSearch};

use super::{now, Vault};
use crate::error::Error;

impl Vault {
    pub fn list_saved_searches(&self) -> Result<Vec<SavedSearch>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, query_json, created_at FROM saved_searches ORDER BY name COLLATE NOCASE",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows
            .into_iter()
            .map(|(id, name, json, created_at)| SavedSearch {
                id,
                name,
                query: serde_json::from_str(&json).unwrap_or_default(),
                created_at,
            })
            .collect())
    }

    pub fn add_saved_search(&self, name: &str, query: &ListQuery) -> Result<SavedSearch> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Conflict("a saved search needs a name".into()));
        }
        let json = serde_json::to_string(query)
            .map_err(|e| Error::Conflict(format!("unserializable query: {e}")))?;
        let created_at = now();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO saved_searches (name, query_json, created_at) VALUES (?1, ?2, ?3)",
            params![name, json, created_at],
        )?;
        Ok(SavedSearch {
            id: conn.last_insert_rowid(),
            name: name.to_string(),
            query: query.clone(),
            created_at,
        })
    }

    pub fn delete_saved_search(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM saved_searches WHERE id = ?1", params![id])?;
        Ok(())
    }
}
