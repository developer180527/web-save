//! Export → import, through real vaults.
//!
//! The unit tests cover each serializer against a hand-built `Save`. These
//! close the loop the user actually performs: export a populated vault to a
//! file, import that file into a clean one, and check what survived. That is
//! the path a device migration takes, and the one where a mismatch between
//! writer and reader turns into "import did nothing" with no error.

use websave_core::export::{exportable, to_json, to_netscape_html};
use websave_core::{CheckOutcome, LinkStatus, ListQuery, NewSave, Save, Vault};

fn vault() -> (tempfile::TempDir, Vault) {
    let dir = tempfile::tempdir().unwrap();
    let vault = Vault::open(dir.path().join("vault")).unwrap();
    (dir, vault)
}

/// A vault with a representative spread: tags, a favorite, an unread, one
/// link a server confirmed gone, and one merely unreachable.
fn populated() -> (tempfile::TempDir, Vault) {
    let (dir, vault) = vault();
    let book = vault
        .add_save(NewSave {
            url: "https://doc.rust-lang.org/book?utm_source=x".into(),
            title: "The Rust Book".into(),
            description: "Ownership & borrowing".into(),
            tags: vec!["rust".into(), "learning".into()],
            ..Default::default()
        })
        .unwrap();
    vault.set_favorite(book.id, true).unwrap();

    vault
        .add_save(NewSave {
            url: "https://news.ycombinator.com".into(),
            title: "Hacker News".into(),
            ..Default::default()
        })
        .unwrap();

    let gone = vault
        .add_save(NewSave {
            url: "https://example.com/deleted-page".into(),
            title: "Deleted".into(),
            ..Default::default()
        })
        .unwrap();
    vault
        .apply_check(
            gone.id,
            &CheckOutcome {
                status: LinkStatus::Dead,
                http_status: Some(404),
                ..Default::default()
            },
        )
        .unwrap();

    let private = vault
        .add_save(NewSave {
            url: "https://vault-server.taild29644.ts.net:9443".into(),
            title: "Home vault".into(),
            ..Default::default()
        })
        .unwrap();
    vault
        .apply_check(
            private.id,
            &CheckOutcome {
                status: LinkStatus::Unreachable,
                ..Default::default()
            },
        )
        .unwrap();

    (dir, vault)
}

fn all(vault: &Vault) -> Vec<Save> {
    vault.list_saves(&ListQuery::default()).unwrap()
}

#[test]
fn json_export_imports_into_a_clean_vault() {
    let (_a, source) = populated();
    let (saves, summary) = exportable(&source.all_saves().unwrap());
    assert_eq!(summary.exported, 3);
    assert_eq!(summary.skipped_dead, 1);

    let json = to_json(&saves, 1_700_000_000);

    let (_b, restored) = vault();
    let (format, items) = websave_core::import::parse(&json);
    assert_eq!(
        format,
        websave_core::ImportFormat::Json,
        "our own export must be recognised as such, not fall through to the \
         URL-list scraper"
    );
    let report = restored.import_items(&items).unwrap();
    assert_eq!(report.new, 3);
    assert_eq!(report.invalid, 0, "nothing in our own export is unusable");

    let restored_saves = all(&restored);
    assert_eq!(restored_saves.len(), 3);

    // The dead link is gone, and the unreachable private one survived.
    assert!(
        !restored_saves
            .iter()
            .any(|s| s.url.contains("deleted-page")),
        "a link a server said was gone is not carried into the backup"
    );
    assert!(
        restored_saves.iter().any(|s| s.url.contains("ts.net")),
        "an unreachable private host stays in the user's own backup"
    );

    // Metadata survives the trip.
    let book = restored_saves
        .iter()
        .find(|s| s.title == "The Rust Book")
        .expect("title preserved");
    assert_eq!(book.description, "Ownership & borrowing");
    assert_eq!(book.tags, vec!["learning", "rust"]);
    assert!(book.favorite, "favorite survives");
    assert!(
        !book.url.contains("utm_source"),
        "canonicalized URL stays canonical"
    );
}

#[test]
fn html_export_imports_into_a_clean_vault() {
    let (_a, source) = populated();
    let (saves, _) = exportable(&source.all_saves().unwrap());
    let html = to_netscape_html(&saves);

    let (_b, restored) = vault();
    let (format, items) = websave_core::import::parse(&html);
    assert_eq!(format, websave_core::ImportFormat::NetscapeHtml);
    let report = restored.import_items(&items).unwrap();
    assert_eq!(report.new, 3);
    assert_eq!(report.invalid, 0);

    let book = all(&restored)
        .into_iter()
        .find(|s| s.title == "The Rust Book")
        .expect("title survives the HTML trip");
    assert_eq!(book.tags, vec!["learning", "rust"]);
}

/// Importing the same export twice must not duplicate or damage anything —
/// the obvious thing a nervous user does when unsure the first one worked.
#[test]
fn importing_the_same_export_twice_is_a_no_op() {
    let (_a, source) = populated();
    let (saves, _) = exportable(&source.all_saves().unwrap());
    let json = to_json(&saves, 1_700_000_000);

    let (_b, restored) = vault();
    let (_, items) = websave_core::import::parse(&json);
    restored.import_items(&items).unwrap();
    let second = restored.import_items(&items).unwrap();

    assert_eq!(second.new, 0);
    assert_eq!(second.existing, 3);
    assert_eq!(all(&restored).len(), 3);
}

/// Malformed and hostile inputs must fail as "nothing to import" rather than
/// panicking or importing garbage.
#[test]
fn malformed_input_is_rejected_without_panicking() {
    let (_dir, vault) = vault();

    for (label, content) in [
        ("empty", ""),
        ("whitespace", "   \n\t  \n"),
        ("truncated json", "{\"saves\": [{\"url\": \"https://a.example\""),
        ("json null", "null"),
        ("json wrong shape", "{\"saves\": \"not an array\"}"),
        ("html with no links", "<html><body><p>nothing here</p></body></html>"),
        ("binary-ish", "\u{0}\u{1}\u{2}garbage\u{ff}"),
        ("csv headers only", "url,title\n"),
    ] {
        let (_, items) = websave_core::import::parse(content);
        let report = vault.import_items(&items).unwrap();
        assert_eq!(report.new, 0, "{label} imported something");
    }

    assert_eq!(all(&vault).len(), 0, "vault untouched by malformed input");
}

/// A file with a few good rows among junk imports the good ones rather than
/// failing wholesale.
#[test]
fn partial_garbage_still_imports_the_usable_rows() {
    let (_dir, vault) = vault();
    let messy = "https://good-one.example/page\n\
                 not a url at all\n\
                 javascript:alert(1)\n\
                 ftp://old.example/file\n\
                 https://good-two.example\n";

    let (_, items) = websave_core::import::parse(messy);
    let report = vault.import_items(&items).unwrap();

    assert_eq!(report.new, 2, "both http(s) URLs landed");
    assert_eq!(all(&vault).len(), 2);
}
