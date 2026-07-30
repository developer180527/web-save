//! Cached cover images, stored under `assets/thumbs/`.

use std::fs;

use rusqlite::params;

use crate::error::{Error, Result};
use crate::probe;

use super::{Vault, ASSETS_DIR};
use rusqlite::OptionalExtension;

/// Subdirectory of the assets dir holding cached cover images.
pub const THUMBS_DIR: &str = "thumbs";

impl Vault {
    /// Store a downloaded cover image under `assets/thumbs/` and point the
    /// save at it.
    pub fn set_thumbnail(&self, id: i64, bytes: &[u8], ext: &str) -> Result<String> {
        let dir = self.root.join(ASSETS_DIR).join(THUMBS_DIR);
        fs::create_dir_all(&dir).map_err(|source| Error::Io {
            path: dir.clone(),
            source,
        })?;
        let relative = format!("{THUMBS_DIR}/{id}.{ext}");
        let path = self.root.join(ASSETS_DIR).join(&relative);
        fs::write(&path, bytes).map_err(|source| Error::Io { path, source })?;

        let conn = self.conn.lock().unwrap();
        let changed = conn.execute(
            "UPDATE saves SET thumbnail = ?1 WHERE id = ?2",
            params![relative, id],
        )?;
        if changed == 0 {
            return Err(Error::NotFound(id));
        }
        log::debug!("thumbnail: cached {relative}");
        Ok(relative)
    }

    /// Download and cache a cover image for a save that doesn't have one
    /// yet. Best-effort: failures are logged and ignored.
    pub fn maybe_fetch_thumbnail(&self, id: i64, og_image: Option<&str>) {
        let Some(og_image) = og_image else { return };
        let has_thumb: Option<String> = {
            let conn = self.conn.lock().unwrap();
            conn.query_row(
                "SELECT thumbnail FROM saves WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .optional()
            .ok()
            .flatten()
        };
        if !matches!(has_thumb.as_deref(), Some("")) {
            return; // missing save, or already has a thumbnail
        }
        if let Some((bytes, ext)) = probe::download_image(og_image) {
            if let Err(e) = self.set_thumbnail(id, &bytes, ext) {
                log::warn!("thumbnail: failed to store for #{id}: {e}");
            }
        }
    }
}
