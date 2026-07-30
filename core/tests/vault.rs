use websave_core::{schedule, Error, LinkStatus, ListQuery, NewSave, SavePatch, Vault};

fn vault() -> (tempfile::TempDir, Vault) {
    let dir = tempfile::tempdir().unwrap();
    let vault = Vault::open(dir.path().join("vault")).unwrap();
    (dir, vault)
}

fn save(vault: &Vault, url: &str, title: &str, tags: &[&str]) -> websave_core::Save {
    vault
        .add_save(NewSave {
            url: url.into(),
            title: title.into(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            ..Default::default()
        })
        .unwrap()
}

#[test]
fn open_creates_vault_layout() {
    let (dir, vault) = vault();
    assert!(dir.path().join("vault").join("websave.db").exists());
    assert!(dir.path().join("vault").join("assets").is_dir());
    // Reopening an existing vault must not fail or re-run migrations.
    drop(vault);
    Vault::open(dir.path().join("vault")).unwrap();
}

#[test]
fn add_and_get_roundtrip() {
    let (_dir, vault) = vault();
    let s = save(
        &vault,
        "https://tauri.app/start",
        "Tauri Start",
        &["rust", "desktop"],
    );
    assert_eq!(s.title, "Tauri Start");
    assert_eq!(s.tags, vec!["desktop", "rust"]);
    assert_eq!(s.status, LinkStatus::Unchecked);
    assert!(!s.favorite);
    assert!(s.created_at > 0);

    let fetched = vault.get_save(s.id).unwrap();
    assert_eq!(fetched.url, "https://tauri.app/start");
}

#[test]
fn add_same_url_upserts_instead_of_duplicating() {
    let (_dir, vault) = vault();
    let first = save(&vault, "https://example.com/a", "First title", &[]);
    let second = save(&vault, "https://example.com/a", "Better title", &[]);
    assert_eq!(first.id, second.id);
    assert_eq!(second.title, "Better title");
    assert_eq!(vault.stats().unwrap().total, 1);

    // Empty fields in a re-capture must not wipe existing metadata.
    let third = vault
        .add_save(NewSave {
            url: "https://example.com/a".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(third.title, "Better title");
}

#[test]
fn rejects_invalid_urls() {
    let (_dir, vault) = vault();
    for bad in ["not a url", "ftp://example.com/file", "javascript:alert(1)"] {
        match vault.add_save(NewSave {
            url: bad.into(),
            ..Default::default()
        }) {
            Err(Error::InvalidUrl(_)) => {}
            other => panic!("expected InvalidUrl for {bad:?}, got {other:?}"),
        }
    }
}

#[test]
fn fts_searches_title_url_notes_and_tags_with_prefixes() {
    let (_dir, vault) = vault();
    let a = save(
        &vault,
        "https://doc.rust-lang.org/book",
        "The Book",
        &["learning"],
    );
    save(&vault, "https://react.dev", "React", &["javascript"]);
    vault
        .update_save(
            a.id,
            SavePatch {
                notes: Some("ownership and borrowing chapters are great".into()),
                ..Default::default()
            },
        )
        .unwrap();

    let q = |query: &str| {
        vault
            .list_saves(&ListQuery {
                query: Some(query.into()),
                ..Default::default()
            })
            .unwrap()
    };

    assert_eq!(q("book").len(), 1, "title match");
    assert_eq!(q("rust-lang").len(), 1, "url match");
    assert_eq!(q("borrow").len(), 1, "notes prefix match");
    assert_eq!(q("learn").len(), 1, "tag prefix match");
    assert_eq!(q("zzz-nothing").len(), 0);
    // FTS operators in user input must not break the query.
    q("\"unbalanced AND ( NEAR");
}

#[test]
fn search_reflects_tag_and_note_edits() {
    let (_dir, vault) = vault();
    let s = save(&vault, "https://sqlite.org/fts5.html", "FTS5", &[]);

    vault.set_tags(s.id, &["fulltext".into()]).unwrap();
    let hits = vault
        .list_saves(&ListQuery {
            query: Some("fulltext".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(hits.len(), 1);

    vault.set_tags(s.id, &["search".into()]).unwrap();
    let hits = vault
        .list_saves(&ListQuery {
            query: Some("fulltext".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(hits.len(), 0, "removed tag no longer matches");
}

#[test]
fn set_tags_dedupes_and_cleans_orphans() {
    let (_dir, vault) = vault();
    let a = save(
        &vault,
        "https://a.example",
        "A",
        &["Rust", "rust", " rust ", "web"],
    );
    assert_eq!(a.tags.len(), 2, "case-insensitive dedupe: {:?}", a.tags);

    let b = save(&vault, "https://b.example", "B", &["web"]);
    vault.set_tags(a.id, &[]).unwrap();
    let names: Vec<String> = vault
        .list_tags()
        .unwrap()
        .into_iter()
        .map(|t| t.name)
        .collect();
    assert_eq!(names, vec!["web"], "orphaned 'Rust' tag removed");

    vault.set_tags(b.id, &[]).unwrap();
    assert!(vault.list_tags().unwrap().is_empty());
}

#[test]
fn filters_favorites_tags_and_status() {
    let (_dir, vault) = vault();
    let a = save(&vault, "https://a.example", "A", &["keep"]);
    let b = save(&vault, "https://b.example", "B", &["keep"]);
    save(&vault, "https://c.example", "C", &["other"]);

    vault.set_favorite(a.id, true).unwrap();
    let favs = vault
        .list_saves(&ListQuery {
            favorites_only: true,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(favs.iter().map(|s| s.id).collect::<Vec<_>>(), vec![a.id]);

    let tagged = vault
        .list_saves(&ListQuery {
            tag: Some("keep".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(tagged.len(), 2);

    vault
        .apply_check(
            b.id,
            &websave_core::CheckOutcome {
                status: LinkStatus::Dead,
                http_status: Some(404),
                redirect_url: None,
                content_hash: None,
                og_image: None,
                archive_text: None,
                ..Default::default()
            },
        )
        .unwrap();
    let dead = vault
        .list_saves(&ListQuery {
            status: Some(LinkStatus::Dead),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(dead.iter().map(|s| s.id).collect::<Vec<_>>(), vec![b.id]);
    assert_eq!(dead[0].http_status, Some(404));

    let stats = vault.stats().unwrap();
    assert_eq!((stats.total, stats.favorites, stats.dead), (3, 1, 1));
}

/// A checked save leaves the due set until its scheduled time comes round,
/// which is what stops a large vault from re-checking everything every day.
#[test]
fn checking_a_save_takes_it_out_of_the_due_set() {
    let (_dir, vault) = vault();
    let a = save(&vault, "https://a.example", "A", &[]);
    let b = save(&vault, "https://b.example", "B", &[]);

    assert_eq!(
        vault.saves_due_for_check(10, 0).unwrap().len(),
        2,
        "never-checked saves are due immediately"
    );
    assert_eq!(vault.count_due_for_check().unwrap(), 2);

    vault
        .apply_check(
            a.id,
            &websave_core::CheckOutcome {
                status: LinkStatus::Active,
                http_status: Some(200),
                redirect_url: Some(String::new()),
                content_hash: Some("abc".into()),
                etag: Some("W/\"v1\"".into()),
                ..Default::default()
            },
        )
        .unwrap();

    let due = vault.saves_due_for_check(10, 0).unwrap();
    assert_eq!(due.iter().map(|t| t.id).collect::<Vec<_>>(), vec![b.id]);
    assert_eq!(vault.count_due_for_check().unwrap(), 1);

    // A forced sweep ignores the schedule and still sees everything, with
    // the state a conditional request needs.
    let all = vault.all_check_targets(0, 10).unwrap();
    assert_eq!(
        all.iter().map(|t| t.id).collect::<Vec<_>>(),
        vec![a.id, b.id]
    );
    assert_eq!(all[0].content_hash, "abc");
    assert_eq!(all[0].etag, "W/\"v1\"");
}

/// The backoff curve, observed through the vault rather than the policy: a
/// save that keeps coming back unchanged is scheduled further and further
/// out, and a change drops it straight back to the floor.
#[test]
fn repeated_clean_checks_widen_the_interval_and_a_change_resets_it() {
    let (_dir, vault) = vault();
    let s = save(&vault, "https://stable.example", "Stable", &[]);
    // Read and unstarred, so it is allowed past the hot ceiling.
    vault.set_read(s.id, true).unwrap();

    let check = |status: LinkStatus, hash: &str| {
        vault
            .apply_check(
                s.id,
                &websave_core::CheckOutcome {
                    status,
                    http_status: Some(200),
                    content_hash: Some(hash.into()),
                    ..Default::default()
                },
            )
            .unwrap();
        vault.check_interval_secs(s.id).unwrap()
    };

    assert_eq!(check(LinkStatus::Active, "h1"), 12 * 3600);
    assert_eq!(check(LinkStatus::Active, "h1"), 24 * 3600);
    assert_eq!(check(LinkStatus::Active, "h1"), 2 * 86_400);

    assert_eq!(
        check(LinkStatus::Changed, "h2"),
        schedule::MIN_INTERVAL_SECS,
        "a changed page must go back to the floor"
    );
}

/// Starred and unread saves are the ones where a stale status is noticed, so
/// they never back off as far as the rest of the vault.
#[test]
fn engaged_saves_keep_a_tighter_schedule() {
    let (_dir, vault) = vault();
    let hot = save(&vault, "https://hot.example", "Hot", &[]);
    let cold = save(&vault, "https://cold.example", "Cold", &[]);
    vault.set_favorite(hot.id, true).unwrap();
    vault.set_read(hot.id, true).unwrap();
    vault.set_read(cold.id, true).unwrap();

    for _ in 0..12 {
        for id in [hot.id, cold.id] {
            vault
                .apply_check(
                    id,
                    &websave_core::CheckOutcome {
                        status: LinkStatus::Active,
                        http_status: Some(200),
                        content_hash: Some("same".into()),
                        ..Default::default()
                    },
                )
                .unwrap();
        }
    }

    assert_eq!(
        vault.check_interval_secs(hot.id).unwrap(),
        schedule::HOT_MAX_INTERVAL_SECS
    );
    assert_eq!(
        vault.check_interval_secs(cold.id).unwrap(),
        schedule::MAX_INTERVAL_SECS
    );
}

/// A 304 leaves every content field alone. The archive, hash and thumbnail
/// have to survive a revalidation, or the cheap path would quietly destroy
/// the data the expensive path collected.
#[test]
fn a_not_modified_check_preserves_stored_content() {
    let (_dir, vault) = vault();
    let s = save(&vault, "https://cached.example", "Cached", &[]);

    vault
        .apply_check(
            s.id,
            &websave_core::CheckOutcome {
                status: LinkStatus::Active,
                http_status: Some(200),
                content_hash: Some("original".into()),
                archive_text: Some("The original prose of the page.".into()),
                etag: Some("\"abc\"".into()),
                last_modified: Some("Wed, 21 Oct 2015 07:28:00 GMT".into()),
                ..Default::default()
            },
        )
        .unwrap();

    vault
        .apply_check(
            s.id,
            &websave_core::CheckOutcome {
                status: LinkStatus::Active,
                http_status: Some(304),
                not_modified: true,
                ..Default::default()
            },
        )
        .unwrap();

    let target = vault.check_target(s.id).unwrap();
    assert_eq!(target.content_hash, "original");
    assert_eq!(target.etag, "\"abc\"");
    assert_eq!(target.last_modified, "Wed, 21 Oct 2015 07:28:00 GMT");
    assert_eq!(
        vault.archive_text(s.id).unwrap().as_deref(),
        Some("The original prose of the page.")
    );
    assert_eq!(vault.get_save(s.id).unwrap().status, LinkStatus::Active);
}

/// Pointing a save at a new URL invalidates everything the schedule and the
/// validators knew about the old one.
#[test]
fn changing_the_url_resets_the_schedule_and_validators() {
    let (_dir, vault) = vault();
    let s = save(&vault, "https://old.example", "Old", &[]);
    vault.set_read(s.id, true).unwrap();
    for _ in 0..4 {
        vault
            .apply_check(
                s.id,
                &websave_core::CheckOutcome {
                    status: LinkStatus::Active,
                    http_status: Some(200),
                    content_hash: Some("same".into()),
                    etag: Some("\"old\"".into()),
                    ..Default::default()
                },
            )
            .unwrap();
    }
    assert!(vault.check_interval_secs(s.id).unwrap() > schedule::MIN_INTERVAL_SECS);
    assert_eq!(vault.count_due_for_check().unwrap(), 0);

    vault.set_url(s.id, "https://new.example").unwrap();

    let target = vault.check_target(s.id).unwrap();
    assert_eq!(target.etag, "", "old validator would revalidate a new page");
    assert_eq!(target.content_hash, "");
    assert_eq!(
        vault.check_interval_secs(s.id).unwrap(),
        schedule::DEFAULT_INTERVAL_SECS
    );
    assert_eq!(
        vault.count_due_for_check().unwrap(),
        1,
        "the new URL should be checked promptly"
    );
}

#[test]
fn delete_removes_save_and_index_entry() {
    let (_dir, vault) = vault();
    let s = save(
        &vault,
        "https://gone.example",
        "Disappearing act",
        &["temp"],
    );
    vault.delete_save(s.id).unwrap();

    assert!(matches!(vault.get_save(s.id), Err(Error::NotFound(_))));
    let hits = vault
        .list_saves(&ListQuery {
            query: Some("disappearing".into()),
            ..Default::default()
        })
        .unwrap();
    assert!(hits.is_empty(), "FTS index entry removed");
    assert!(vault.list_tags().unwrap().is_empty(), "orphan tag removed");
    assert!(matches!(vault.delete_save(s.id), Err(Error::NotFound(_))));
}

#[test]
fn archive_snapshot_is_stored_and_searchable() {
    let (_dir, vault) = vault();
    let s = save(&vault, "https://blog.example/wal", "A blog post", &[]);
    assert_eq!(s.archived_at, None);
    assert_eq!(vault.archive_text(s.id).unwrap(), None);

    vault
        .apply_check(
            s.id,
            &websave_core::CheckOutcome {
                status: LinkStatus::Active,
                http_status: Some(200),
                redirect_url: Some(String::new()),
                content_hash: Some("h1".into()),
                og_image: None,
                archive_text: Some(
                    "SQLite write-ahead logging lets readers and writers coexist".into(),
                ),
                ..Default::default()
            },
        )
        .unwrap();

    let updated = vault.get_save(s.id).unwrap();
    assert!(updated.archived_at.is_some(), "archived_at recorded");
    assert!(vault
        .archive_text(s.id)
        .unwrap()
        .unwrap()
        .contains("write-ahead"));

    // Found by words that exist only in the page content.
    let hits = vault
        .list_saves(&ListQuery {
            query: Some("write-ahead logging".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id, s.id);

    // A later check with empty extraction must not clobber the archive.
    vault
        .apply_check(
            s.id,
            &websave_core::CheckOutcome {
                status: LinkStatus::Active,
                http_status: Some(200),
                redirect_url: Some(String::new()),
                content_hash: Some("h2".into()),
                og_image: None,
                archive_text: Some("   ".into()),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(vault.archive_text(s.id).unwrap().is_some(), "kept");
}

#[test]
fn urls_are_canonicalized_for_dedupe() {
    let (_dir, vault) = vault();
    let a = save(
        &vault,
        "https://blog.example/post?utm_source=x&utm_campaign=y&id=7&fbclid=abc",
        "Post",
        &[],
    );
    assert_eq!(
        a.url, "https://blog.example/post?id=7",
        "tracking stripped, real params kept"
    );

    // The same article via a different tracking link dedupes onto one save.
    let b = save(
        &vault,
        "https://blog.example/post?id=7&gclid=zzz",
        "Post again",
        &[],
    );
    assert_eq!(a.id, b.id);

    let c = save(
        &vault,
        "https://blog.example/clean?utm_medium=mail",
        "Clean",
        &[],
    );
    assert_eq!(
        c.url, "https://blog.example/clean",
        "empty query removed entirely"
    );
}

#[test]
fn read_flag_and_inbox_filter() {
    let (_dir, vault) = vault();
    let a = save(&vault, "https://a.example", "A", &[]);
    let b = save(&vault, "https://b.example", "B", &[]);
    assert!(!a.is_read, "new saves arrive unread");

    vault.set_read(a.id, true).unwrap();
    let inbox = vault
        .list_saves(&ListQuery {
            unread_only: true,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(inbox.iter().map(|s| s.id).collect::<Vec<_>>(), vec![b.id]);
    assert_eq!(vault.stats().unwrap().unread, 1);

    vault.set_read_many(&[a.id, b.id], false).unwrap();
    assert_eq!(vault.stats().unwrap().unread, 2);
}

#[test]
fn bulk_operations() {
    let (_dir, vault) = vault();
    let ids: Vec<i64> = (1..=4)
        .map(|i| {
            save(
                &vault,
                &format!("https://x{i}.example"),
                &format!("X{i}"),
                &[],
            )
            .id
        })
        .collect();

    vault.set_favorite_many(&ids[..2], true).unwrap();
    assert_eq!(vault.stats().unwrap().favorites, 2);

    vault.add_tag_many(&ids, "batch").unwrap();
    let tagged = vault
        .list_saves(&ListQuery {
            tag: Some("batch".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(tagged.len(), 4);

    vault.delete_many(&ids[..3]).unwrap();
    assert_eq!(vault.stats().unwrap().total, 1);
    assert_eq!(
        vault.list_tags().unwrap().len(),
        1,
        "tag survives on remaining save"
    );
}

#[test]
fn saved_searches_roundtrip() {
    let (_dir, vault) = vault();
    let q = ListQuery {
        query: Some("rust".into()),
        unread_only: true,
        ..Default::default()
    };
    let s = vault.add_saved_search("Rust inbox", &q).unwrap();
    let all = vault.list_saved_searches().unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].name, "Rust inbox");
    assert_eq!(all[0].query.query.as_deref(), Some("rust"));
    assert!(all[0].query.unread_only);

    vault.delete_saved_search(s.id).unwrap();
    assert!(vault.list_saved_searches().unwrap().is_empty());

    assert!(vault.add_saved_search("   ", &q).is_err(), "name required");
}

#[test]
fn set_url_accepts_redirects_and_guards_conflicts() {
    let (_dir, vault) = vault();
    let a = save(&vault, "https://old.example/page", "Old", &[]);
    let b = save(&vault, "https://taken.example", "Taken", &[]);

    let moved = vault.set_url(a.id, "https://new.example/page").unwrap();
    assert_eq!(moved.url, "https://new.example/page");
    assert_eq!(moved.status, LinkStatus::Unchecked, "re-check scheduled");
    assert_eq!(moved.redirect_url, "");

    match vault.set_url(a.id, &b.url) {
        Err(Error::Conflict(_)) => {}
        other => panic!("expected Conflict, got {other:?}"),
    }
}

#[test]
fn meta_key_value_roundtrip() {
    let (_dir, vault) = vault();
    assert_eq!(vault.get_meta("missing").unwrap(), None);

    vault.set_meta("ext.last_seen", "1700000000").unwrap();
    assert_eq!(
        vault.get_meta("ext.last_seen").unwrap().as_deref(),
        Some("1700000000")
    );

    // Upsert overwrites.
    vault.set_meta("ext.last_seen", "1800000000").unwrap();
    assert_eq!(
        vault.get_meta("ext.last_seen").unwrap().as_deref(),
        Some("1800000000")
    );
}

#[test]
fn dead_since_is_stamped_on_death_and_cleared_on_recovery() {
    let (_dir, vault) = vault();
    let s = save(&vault, "https://rot.example", "Rots", &[]);
    assert_eq!(s.dead_since, None);

    let outcome = |status: LinkStatus| websave_core::CheckOutcome {
        status,
        http_status: Some(404),
        redirect_url: None,
        content_hash: None,
        og_image: None,
        archive_text: None,
        ..Default::default()
    };

    vault.apply_check(s.id, &outcome(LinkStatus::Dead)).unwrap();
    let dead = vault.get_save(s.id).unwrap();
    let stamped = dead.dead_since.expect("dead_since stamped");

    // Staying dead must not move the stamp (the grace period keeps running).
    vault.apply_check(s.id, &outcome(LinkStatus::Dead)).unwrap();
    assert_eq!(vault.get_save(s.id).unwrap().dead_since, Some(stamped));

    // Recovery clears it, so the grace period restarts if it dies again.
    vault
        .apply_check(s.id, &outcome(LinkStatus::Active))
        .unwrap();
    assert_eq!(vault.get_save(s.id).unwrap().dead_since, None);
}

#[test]
fn purge_only_removes_links_dead_past_the_grace_period() {
    let (_dir, vault) = vault();
    let old = save(&vault, "https://long-dead.example", "Long dead", &[]);
    let fresh = save(&vault, "https://just-died.example", "Just died", &[]);
    let alive = save(&vault, "https://alive.example", "Alive", &[]);

    let outcome = |status: LinkStatus| websave_core::CheckOutcome {
        status,
        http_status: None,
        redirect_url: None,
        content_hash: None,
        og_image: None,
        archive_text: None,
        ..Default::default()
    };
    vault
        .apply_check(old.id, &outcome(LinkStatus::Dead))
        .unwrap();
    vault
        .apply_check(fresh.id, &outcome(LinkStatus::Dead))
        .unwrap();
    vault
        .apply_check(alive.id, &outcome(LinkStatus::Active))
        .unwrap();

    // Nothing is old enough yet.
    assert_eq!(vault.purge_expired_dead(30 * 86_400).unwrap(), 0);
    assert_eq!(vault.stats().unwrap().total, 3);

    // A zero-length grace period expires everything currently dead.
    assert_eq!(vault.purge_expired_dead(-1).unwrap(), 2);
    let left = vault.list_saves(&ListQuery::default()).unwrap();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].id, alive.id, "healthy links are never purged");
}

#[test]
fn all_saves_returns_everything_with_tags_for_export() {
    let (_dir, vault) = vault();
    save(&vault, "https://a.example", "A", &["x"]);
    save(&vault, "https://b.example", "B", &[]);
    let all = vault.all_saves().unwrap();
    assert_eq!(all.len(), 2);
    let tagged = all.iter().find(|s| s.title == "A").unwrap();
    assert_eq!(tagged.tags, vec!["x"]);
}
