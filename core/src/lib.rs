//! # websave-core
//!
//! Portable, local-first storage engine for saved web references.
//!
//! Everything lives in a [`Vault`]: a plain directory containing a SQLite
//! database (with an FTS5 full-text index) plus an `assets/` folder for
//! optional local files such as thumbnails. The crate has no UI or framework
//! dependencies, so the same vault can back a Tauri desktop app, a CLI, a
//! mobile app, or be embedded as a plugin in another application.
//!
//! ```no_run
//! use websave_core::{Vault, NewSave, ListQuery};
//!
//! let vault = Vault::open("/path/to/vault")?;
//! vault.add_save(NewSave {
//!     url: "https://tauri.app".into(),
//!     title: "Tauri".into(),
//!     tags: vec!["rust".into(), "desktop".into()],
//!     ..Default::default()
//! })?;
//! let hits = vault.list_saves(&ListQuery {
//!     query: Some("tauri".into()),
//!     ..Default::default()
//! })?;
//! # Ok::<(), websave_core::Error>(())
//! ```

pub mod error;
pub mod models;
pub mod monitor;
mod vault;

pub use error::{Error, Result};
pub use models::{LinkStatus, ListQuery, NewSave, Save, SavePatch, TagCount, VaultStats};
pub use monitor::{check_url, CheckOutcome, CheckTarget};
pub use vault::{Vault, ASSETS_DIR, DB_FILE};
