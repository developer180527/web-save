use websave_core::{Error, LinkStatus, ListQuery, NewSave, SavePatch, Vault};

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
    let s = save(&vault, "https://tauri.app/start", "Tauri Start", &["rust", "desktop"]);
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
    let a = save(&vault, "https://doc.rust-lang.org/book", "The Book", &["learning"]);
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
    let a = save(&vault, "https://a.example", "A", &["Rust", "rust", " rust ", "web"]);
    assert_eq!(a.tags.len(), 2, "case-insensitive dedupe: {:?}", a.tags);

    let b = save(&vault, "https://b.example", "B", &["web"]);
    vault.set_tags(a.id, &[]).unwrap();
    let names: Vec<String> = vault.list_tags().unwrap().into_iter().map(|t| t.name).collect();
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

#[test]
fn due_for_check_orders_by_staleness_and_respects_age() {
    let (_dir, vault) = vault();
    let a = save(&vault, "https://a.example", "A", &[]);
    let b = save(&vault, "https://b.example", "B", &[]);

    let due = vault.saves_due_for_check(3600, 10).unwrap();
    assert_eq!(due.len(), 2, "never-checked saves are due");

    vault
        .apply_check(
            a.id,
            &websave_core::CheckOutcome {
                status: LinkStatus::Active,
                http_status: Some(200),
                redirect_url: Some(String::new()),
                content_hash: Some("abc".into()),
            },
        )
        .unwrap();
    let due = vault.saves_due_for_check(3600, 10).unwrap();
    assert_eq!(due.iter().map(|t| t.id).collect::<Vec<_>>(), vec![b.id]);
    // max_age 0 means everything is due again; freshly checked goes last.
    let due = vault.saves_due_for_check(-1, 10).unwrap();
    assert_eq!(due.last().unwrap().id, a.id);
    assert_eq!(due.last().unwrap().content_hash, "abc");
}

#[test]
fn delete_removes_save_and_index_entry() {
    let (_dir, vault) = vault();
    let s = save(&vault, "https://gone.example", "Disappearing act", &["temp"]);
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
