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
    pub is_read: bool,
    pub tags: Vec<String>,
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
        is_read: s.is_read,
        tags: s.tags,
    }
}

#[derive(uniffi::Object)]
pub struct VaultHandle {
    /// `Arc` because [`websave_core::Monitor`] takes shared ownership of the
    /// vault for the duration of a sweep.
    inner: Arc<Vault>,
}

#[uniffi::export]
impl VaultHandle {
    /// Open (or create) the vault at `path`.
    #[uniffi::constructor]
    pub fn new(path: String) -> Result<Arc<Self>, VaultError> {
        Ok(Arc::new(VaultHandle {
            inner: Arc::new(Vault::open(path)?),
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

    /// Capture a URL (upsert by canonical URL), returning the resulting save.
    /// This is what the Share Extension and the in-app "Add" call.
    pub fn add(&self, url: String, title: String) -> Result<SaveSummary, VaultError> {
        let save = self.inner.add_save(websave_core::NewSave {
            url,
            title,
            ..Default::default()
        })?;
        Ok(summarize(save))
    }

    pub fn delete(&self, id: i64) -> Result<(), VaultError> {
        self.inner.delete_save(id)?;
        Ok(())
    }

    /// A foreground health-check sweep: iOS can't run a background daemon, so
    /// the app checks due links while it's open.
    ///
    /// Which links are due is `websave_core::schedule`'s decision, the same
    /// one the desktop daemon uses — this runs the shared
    /// [`websave_core::Monitor`] rather than its own loop, so the two hosts
    /// can't drift apart.
    ///
    /// Bounded by `limit` checks and `budget_secs` of wall time, because the
    /// foreground window may be a few seconds long; whatever it doesn't
    /// reach stays due for the next launch. Blocks on the network — call it
    /// off the main actor. Returns how many were checked.
    pub fn check_due(&self, limit: i64, budget_secs: u32) -> Result<u32, VaultError> {
        let config = websave_core::MonitorConfig {
            max_per_sweep: (limit > 0).then_some(limit as usize),
            time_budget: (budget_secs > 0)
                .then(|| std::time::Duration::from_secs(budget_secs as u64)),
            // Half the desktop's concurrency: a phone is likelier to be on a
            // metered connection and on battery, and the sweep is bounded by
            // time anyway.
            max_concurrency: 8,
            ..Default::default()
        };
        let monitor = websave_core::Monitor::new(self.inner.clone(), config);
        Ok(monitor.run_due().checked as u32)
    }
}
