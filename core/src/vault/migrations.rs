//! Schema migrations: the append-only SQL that brings any vault up to
//! the current schema version, and the `PRAGMA user_version` bookkeeping
//! that applies only what a given database is missing.

use rusqlite::Connection;

use crate::error::Result;

/// Schema migrations, applied in order and tracked via `PRAGMA user_version`.
const MIGRATIONS: &[&str] = &[
    r#"
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
"#,
    // v2: cached cover images (og:image), stored under assets/thumbs/.
    "ALTER TABLE saves ADD COLUMN thumbnail TEXT NOT NULL DEFAULT '';",
    // v3: archived readable text, searchable via a rebuilt FTS index with
    // an extra `archive` column.
    r#"
ALTER TABLE saves ADD COLUMN archive_text TEXT NOT NULL DEFAULT '';
ALTER TABLE saves ADD COLUMN archived_at INTEGER;

DROP TRIGGER saves_fts_insert;
DROP TRIGGER saves_fts_delete;
DROP TRIGGER saves_fts_update;
DROP TABLE saves_fts;

CREATE VIRTUAL TABLE saves_fts USING fts5(
    title, url, description, notes, tags, archive,
    content='saves',
    content_rowid='id',
    tokenize='unicode61 remove_diacritics 2'
);

CREATE TRIGGER saves_fts_insert AFTER INSERT ON saves BEGIN
    INSERT INTO saves_fts(rowid, title, url, description, notes, tags, archive)
    VALUES (new.id, new.title, new.url, new.description, new.notes, new.tags_text, new.archive_text);
END;

CREATE TRIGGER saves_fts_delete AFTER DELETE ON saves BEGIN
    INSERT INTO saves_fts(saves_fts, rowid, title, url, description, notes, tags, archive)
    VALUES ('delete', old.id, old.title, old.url, old.description, old.notes, old.tags_text, old.archive_text);
END;

CREATE TRIGGER saves_fts_update AFTER UPDATE ON saves BEGIN
    INSERT INTO saves_fts(saves_fts, rowid, title, url, description, notes, tags, archive)
    VALUES ('delete', old.id, old.title, old.url, old.description, old.notes, old.tags_text, old.archive_text);
    INSERT INTO saves_fts(rowid, title, url, description, notes, tags, archive)
    VALUES (new.id, new.title, new.url, new.description, new.notes, new.tags_text, new.archive_text);
END;

INSERT INTO saves_fts(rowid, title, url, description, notes, tags, archive)
SELECT id, title, url, description, notes, tags_text, archive_text FROM saves;
"#,
    // v4: read/unread (Inbox) + saved searches. Existing saves start read so
    // an upgrade doesn't flood the Inbox; new captures arrive unread.
    r#"
ALTER TABLE saves ADD COLUMN is_read INTEGER NOT NULL DEFAULT 0;
UPDATE saves SET is_read = 1;

CREATE TABLE saved_searches (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    query_json TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
"#,
    // v5: generic key-value metadata. A portable primitive the host can use
    // for whatever it needs (e.g. tracking when a capture client last
    // checked in) — core stays unaware of any specific key's meaning.
    r#"
CREATE TABLE meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
) WITHOUT ROWID;
"#,
    // v6: when a link first went dead. Dead saves are quarantined (Trash)
    // but keep being health-checked; recovery clears the stamp, and the host
    // can purge anything still dead after its grace period.
    r#"
ALTER TABLE saves ADD COLUMN dead_since INTEGER;
UPDATE saves SET dead_since = last_checked_at WHERE status = 'dead';
"#,
    // v7: which hash basis `content_hash` was computed under. Existing rows
    // keep 0 (raw response bytes); the next check re-baselines them to the
    // current basis without reporting a spurious change. See
    // `docs/content-hashing.md`.
    r#"
ALTER TABLE saves ADD COLUMN content_hash_algo INTEGER NOT NULL DEFAULT 0;
"#,
    // v8: per-save check scheduling and HTTP validators.
    //
    // `next_check_at` replaces the old "last_checked_at older than 24h" rule
    // with adaptive backoff (see `crate::schedule`), and `etag` /
    // `last_modified` let a check ask "changed?" instead of re-downloading
    // the page. Both exist to make a sweep cost scale with how volatile the
    // vault is rather than how big it is.
    //
    // The interval default is `schedule::DEFAULT_INTERVAL_SECS` as a literal
    // (SQLite column defaults must be constants); `default_interval_matches_schedule`
    // holds the two together. Backfill puts every existing save's first
    // scheduled check one interval after its last one, and never-checked
    // saves at 0 so they stay immediately due.
    r#"
ALTER TABLE saves ADD COLUMN next_check_at INTEGER;
ALTER TABLE saves ADD COLUMN check_interval_secs INTEGER NOT NULL DEFAULT 21600;
ALTER TABLE saves ADD COLUMN stable_streak INTEGER NOT NULL DEFAULT 0;
ALTER TABLE saves ADD COLUMN etag TEXT NOT NULL DEFAULT '';
ALTER TABLE saves ADD COLUMN last_modified TEXT NOT NULL DEFAULT '';

UPDATE saves SET next_check_at = COALESCE(last_checked_at + 21600, 0);

CREATE INDEX idx_saves_next_check ON saves(next_check_at);
"#,
    // v9: where each host is served from.
    //
    // Keyed by host rather than by save, because that is what the fact is
    // about: a 10k-save vault is a few hundred hosts, so the map costs a few
    // hundred DNS lookups rather than ten thousand. The same key the link
    // sweep paces and circuit-breaks on (`monitor::host_grouping::host_key`).
    //
    // Latitude and longitude are nullable on purpose: a country-level
    // database has no coordinates, and inventing a centroid would present a
    // guess as a measurement. Such rows still power a per-country breakdown.
    r#"
CREATE TABLE host_locations (
    host TEXT PRIMARY KEY,
    ip TEXT NOT NULL DEFAULT '',
    country TEXT NOT NULL DEFAULT '',
    city TEXT NOT NULL DEFAULT '',
    latitude REAL,
    longitude REAL,
    resolved_at INTEGER NOT NULL
) WITHOUT ROWID;
"#,
    // v10: which network announces each host's address.
    //
    // Separate from v9 rather than folded into it: any vault that has already
    // opened once is recorded as being at v9, so editing v9 in place would
    // leave it permanently missing these columns.
    //
    // This is the column that makes the map truthful. A coordinate for an
    // anycast CDN address is close to arbitrary, but the announcing AS is a
    // fact — "Cloudflare, 17 sites" is true where "Toronto, 17 sites" is not.
    r#"
ALTER TABLE host_locations ADD COLUMN asn INTEGER;
ALTER TABLE host_locations ADD COLUMN asn_org TEXT NOT NULL DEFAULT '';
"#,
];

pub(super) fn migrate(conn: &Connection) -> Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
        conn.execute_batch(sql)?;
        conn.pragma_update(None, "user_version", (i + 1) as i64)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::{params, Connection};

    /// Bring a fresh in-memory database up to schema version `to`.
    fn db_at_version(to: usize) -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        for sql in super::MIGRATIONS.iter().take(to) {
            conn.execute_batch(sql).unwrap();
        }
        conn.pragma_update(None, "user_version", to as i64).unwrap();
        conn
    }

    /// Upgrading must not make the whole vault due at once.
    ///
    /// A 10k-save vault that all came due on the first launch after upgrade
    /// would reproduce the exact stampede this schedule exists to prevent —
    /// so previously-checked saves are backfilled one interval past their
    /// last check, and only never-checked ones are due immediately.
    #[test]
    fn v8_backfill_staggers_existing_saves() {
        let conn = db_at_version(7);
        let now = crate::vault::now();
        for (id, last_checked) in [(1i64, Some(now - 3600)), (2, Some(now - 86_400)), (3, None)] {
            conn.execute(
                "INSERT INTO saves (id, url, created_at, updated_at, last_checked_at)
                 VALUES (?1, ?2, ?3, ?3, ?4)",
                params![id, format!("https://e{id}.example"), now, last_checked],
            )
            .unwrap();
        }

        super::migrate(&conn).unwrap();

        let due: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM saves WHERE next_check_at <= ?1",
                params![now],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            due, 2,
            "only the never-checked and the day-old save are due"
        );

        let recent: i64 = conn
            .query_row("SELECT next_check_at FROM saves WHERE id = 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(recent, now - 3600 + crate::schedule::DEFAULT_INTERVAL_SECS);

        let never: i64 = conn
            .query_row("SELECT next_check_at FROM saves WHERE id = 3", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(never, 0, "never-checked saves stay immediately due");
    }

    /// SQLite column defaults must be literals, so the v8 migration spells
    /// the starting interval out as a number. If the policy's floor moves and
    /// the literal doesn't, every newly added save would start on a stale
    /// cadence — silently, and only for rows created before their first check.
    #[test]
    fn default_interval_matches_schedule() {
        // Found by content rather than by position: an earlier version of
        // this test read MIGRATIONS.last(), and started failing the moment a
        // v9 was appended for something unrelated.
        let declaration = "check_interval_secs INTEGER NOT NULL DEFAULT";
        let migration = super::MIGRATIONS
            .iter()
            .find(|sql| sql.contains(declaration))
            .expect("some migration declares check_interval_secs");
        assert!(
            migration.contains(&format!(
                "{declaration} {}",
                crate::schedule::DEFAULT_INTERVAL_SECS
            )),
            "the interval default drifted from schedule::DEFAULT_INTERVAL_SECS"
        );
    }
}
