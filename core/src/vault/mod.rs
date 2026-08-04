//! A portable bookmark vault: a directory holding the SQLite database and
//! optional local assets (thumbnails, etc.).
//!
//! All storage, search and validation logic lives here, independent of any UI
//! framework. This module owns the [`Vault`] handle and the directory layout;
//! everything it can do is implemented in a submodule named for the job:
//!
//! - [`saves`] — create, read, edit, delete.
//! - [`host_locations`] — where each host is served from, for the map.
//! - [`search`] — listing, filtering, full-text search.
//! - [`tags`] — the tag graph and the denormalized column FTS reads.
//! - [`link_checks`] — which saves are due, and what a check writes back.
//! - [`trash`] — the grace period before a dead link is really gone.
//! - [`imports`] — bringing in bookmarks from other tools.
//! - [`thumbnails`] — cached cover images.
//! - [`saved_searches`], [`metadata`], [`stats`] — the smaller stores.
//! - [`migrations`] — schema versioning.
//! - [`save_rows`], [`url_normalization`] — shared helpers.
//!
//! Rust allows inherent `impl` blocks anywhere in the defining crate, so each
//! submodule adds its own methods to [`Vault`]; callers see one flat API.

mod activity;
mod host_locations;
mod imports;
mod link_checks;
mod metadata;
mod migrations;
mod save_rows;
mod saved_searches;
mod saves;
mod search;
mod stats;
mod tags;
mod thumbnails;
mod trash;
mod url_normalization;

pub use activity::ActivityEntry;
pub use thumbnails::THUMBS_DIR;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use crate::error::{Error, Result};

pub const DB_FILE: &str = "websave.db";
pub const ASSETS_DIR: &str = "assets";

/// A handle to an open vault.
///
/// The single `Connection` behind a `Mutex` is deliberate: SQLite in WAL mode
/// serializes writers anyway, and one lock is far easier to reason about than
/// a pool when a background sweep and the UI are both touching the vault.
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
        migrations::migrate(&conn)?;
        log::info!("vault opened at {}", root.display());
        Ok(Vault {
            conn: Mutex::new(conn),
            root,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Absolute path of the assets directory (thumbnails etc.).
    pub fn assets_dir(&self) -> PathBuf {
        self.root.join(ASSETS_DIR)
    }
}

/// Unix epoch seconds — the vault's only notion of time.
pub(crate) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
