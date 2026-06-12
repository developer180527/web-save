//! UniFFI surface over [`websave_core`].
//!
//! Kept deliberately small: it exposes what a quick-access native client
//! needs (read lists, search, toggle favorites). Heavy management stays in
//! the desktop app. Multiple processes may open the same vault — SQLite
//! runs in WAL mode — so the Swift menubar app and the Tauri engine
//! coexist safely.

use std::sync::Arc;

use websave_core::{ListQuery, Vault};

uniffi::setup_scaffolding!();

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum VaultError {
    #[error("{message}")]
    Failure { message: String },
}

impl From<websave_core::Error> for VaultError {
    fn from(e: websave_core::Error) -> Self {
        VaultError::Failure {
            message: e.to_string(),
        }
    }
}

/// The slice of a save a quick-access UI needs.
#[derive(uniffi::Record)]
pub struct SaveSummary {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub favicon_url: String,
    pub favorite: bool,
    pub status: String,
    pub created_at: i64,
}

fn summarize(s: websave_core::Save) -> SaveSummary {
    SaveSummary {
        id: s.id,
        url: s.url,
        title: s.title,
        favicon_url: s.favicon_url,
        favorite: s.favorite,
        status: s.status.as_str().to_string(),
        created_at: s.created_at,
    }
}

#[derive(uniffi::Object)]
pub struct VaultHandle {
    inner: Vault,
}

#[uniffi::export]
impl VaultHandle {
    /// Open (or create) the vault at `path`.
    #[uniffi::constructor]
    pub fn new(path: String) -> Result<Arc<Self>, VaultError> {
        Ok(Arc::new(VaultHandle {
            inner: Vault::open(path)?,
        }))
    }

    pub fn starred(&self, limit: i64) -> Result<Vec<SaveSummary>, VaultError> {
        let saves = self.inner.list_saves(&ListQuery {
            favorites_only: true,
            limit: Some(limit),
            ..Default::default()
        })?;
        Ok(saves.into_iter().map(summarize).collect())
    }

    pub fn recent(&self, limit: i64) -> Result<Vec<SaveSummary>, VaultError> {
        let saves = self.inner.list_saves(&ListQuery {
            limit: Some(limit),
            ..Default::default()
        })?;
        Ok(saves.into_iter().map(summarize).collect())
    }

    pub fn search(&self, query: String, limit: i64) -> Result<Vec<SaveSummary>, VaultError> {
        let saves = self.inner.list_saves(&ListQuery {
            query: Some(query),
            limit: Some(limit),
            ..Default::default()
        })?;
        Ok(saves.into_iter().map(summarize).collect())
    }

    pub fn set_favorite(&self, id: i64, favorite: bool) -> Result<(), VaultError> {
        self.inner.set_favorite(id, favorite)?;
        Ok(())
    }
}
