use websave_core::import::{parse, ImportFormat};
use websave_core::{ImportItem, ListQuery, NewSave, Vault};

const NETSCAPE_SAMPLE: &str = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<META HTTP-EQUIV="Content-Type" CONTENT="text/html; charset=UTF-8">
<TITLE>Bookmarks</TITLE>
<H1>Bookmarks</H1>
<DL><p>
    <DT><H3 ADD_DATE="1588327800">Bookmarks bar</H3>
    <DL><p>
        <DT><A HREF="https://news.ycombinator.com/" ADD_DATE="1588327800">Hacker News</A>
        <DT><H3>Dev &amp; Tools</H3>
        <DL><p>
            <DT><A HREF="https://doc.rust-lang.org/book/?a=1&amp;b=2" ADD_DATE="1600000000" TAGS="rust,learning">The Rust Book</A>
            <DT><A HREF="ftp://old.example.com/file">Skip me</A>
        </DL><p>
    </DL><p>
    <DT><A HREF="https://tauri.app/" ADD_DATE="1700000000000">Tauri</A>
</DL><p>
"#;

#[test]
fn netscape_html_folders_dates_and_tags() {
    let (format, items) = parse(NETSCAPE_SAMPLE);
    assert_eq!(format, ImportFormat::NetscapeHtml);
    assert_eq!(items.len(), 3, "ftp link skipped: {items:#?}");

    let hn = &items[0];
    assert_eq!(hn.url, "https://news.ycombinator.com/");
    assert_eq!(hn.title, "Hacker News");
    assert_eq!(hn.created_at, Some(1_588_327_800));
    assert!(hn.tags.is_empty(), "generic folder dropped: {:?}", hn.tags);

    let book = &items[1];
    assert_eq!(book.url, "https://doc.rust-lang.org/book/?a=1&b=2");
    assert_eq!(book.title, "The Rust Book");
    assert_eq!(book.tags, vec!["Dev & Tools", "rust", "learning"]);
    assert_eq!(book.created_at, Some(1_600_000_000));

    let tauri = &items[2];
    assert_eq!(tauri.title, "Tauri");
    assert_eq!(tauri.created_at, Some(1_700_000_000), "ms normalized");
    assert!(tauri.tags.is_empty(), "folder stack unwound: {:?}", tauri.tags);
}

#[test]
fn pocket_html_variant() {
    let html = r#"<!DOCTYPE html><html><body>
<ul>
<li><a href="https://example.com/article" time_added="1650000000" tags="read-later|tech">An Article</a></li>
</ul></body></html>"#;
    let (format, items) = parse(html);
    assert_eq!(format, ImportFormat::NetscapeHtml);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].created_at, Some(1_650_000_000));
    assert_eq!(items[0].tags, vec!["read-later", "tech"]);
}

#[test]
fn raindrop_csv() {
    let csv = "id,title,note,excerpt,url,folder,tags,created,cover,highlights,favorite\n\
1,\"Rust Book\",\"my note, with comma\",\"An excerpt\",https://doc.rust-lang.org/book,Dev,\"rust, learning\",2020-05-01T10:10:00.000Z,,,true\n\
2,Tauri,,,https://tauri.app,Unsorted bookmarks,,2021-01-01T00:00:00.000Z,,,false\n";
    let (format, items) = parse(csv);
    assert_eq!(format, ImportFormat::RaindropCsv);
    assert_eq!(items.len(), 2);

    let book = &items[0];
    assert_eq!(book.title, "Rust Book");
    assert_eq!(book.notes, "my note, with comma");
    assert_eq!(book.description, "An excerpt");
    assert_eq!(book.tags, vec!["rust", "learning", "Dev"]);
    assert!(book.favorite);
    assert_eq!(book.created_at, Some(1_588_327_800));

    let tauri = &items[1];
    assert!(!tauri.favorite);
    assert!(tauri.tags.is_empty(), "generic folder dropped");
}

#[test]
fn pocket_csv() {
    let csv = "title,url,time_added,tags,status\n\
Article,https://example.com/a,1650000000,tech|later,unread\n";
    let (format, items) = parse(csv);
    assert_eq!(format, ImportFormat::PocketCsv);
    assert_eq!(items[0].created_at, Some(1_650_000_000));
    assert_eq!(items[0].tags, vec!["tech", "later"]);
}

#[test]
fn url_list_with_markdown() {
    let text = "# my links\n\
- https://example.com/one\n\
[Two](https://example.com/two)\n\
* [Three](https://example.com/three) trailing words\n\
not a url line\n\
check https://example.com/four out\n";
    let (format, items) = parse(text);
    assert_eq!(format, ImportFormat::UrlList);
    let urls: Vec<&str> = items.iter().map(|i| i.url.as_str()).collect();
    assert_eq!(
        urls,
        vec![
            "https://example.com/one",
            "https://example.com/two",
            "https://example.com/three",
            "https://example.com/four"
        ]
    );
    assert_eq!(items[1].title, "Two");
}

#[test]
fn import_merges_without_destroying_user_data() {
    let dir = tempfile::tempdir().unwrap();
    let vault = Vault::open(dir.path().join("vault")).unwrap();

    // Pre-existing save with user edits.
    let existing = vault
        .add_save(NewSave {
            url: "https://tauri.app".into(),
            title: "My Tauri title".into(),
            tags: vec!["desktop".into()],
            ..Default::default()
        })
        .unwrap();

    let items = vec![
        ImportItem {
            url: "https://tauri.app/".into(), // same after normalization
            title: "Tauri — imported title".into(),
            tags: vec!["rust".into(), "Desktop".into()],
            favorite: true,
            created_at: Some(1_500_000_000),
            ..Default::default()
        },
        ImportItem {
            url: "https://new.example.com".into(),
            title: "Brand new".into(),
            created_at: Some(1_600_000_000),
            ..Default::default()
        },
        ImportItem {
            url: "javascript:alert(1)".into(),
            ..Default::default()
        },
    ];

    let preview = vault.preview_import(&items).unwrap();
    assert_eq!(
        (preview.total, preview.new, preview.existing, preview.invalid),
        (3, 1, 1, 1)
    );

    let report = vault.import_items(&items).unwrap();
    assert_eq!(
        (report.new, report.existing, report.invalid),
        (1, 1, 1)
    );

    let merged = vault.get_save(existing.id).unwrap();
    assert_eq!(merged.title, "My Tauri title", "user title not overwritten");
    assert!(merged.favorite, "favorite is sticky");
    assert_eq!(merged.created_at, 1_500_000_000, "earliest date wins");
    assert_eq!(
        merged.tags,
        vec!["desktop", "rust"],
        "tag union, case-insensitive dedupe"
    );

    let all = vault.list_saves(&ListQuery::default()).unwrap();
    assert_eq!(all.len(), 2);
    let new_save = all.iter().find(|s| s.title == "Brand new").unwrap();
    assert_eq!(new_save.created_at, 1_600_000_000, "original date kept");

    // Re-importing the same file is a no-op for counts of new items.
    let again = vault.import_items(&items).unwrap();
    assert_eq!((again.new, again.existing), (0, 2));
    assert_eq!(vault.list_saves(&ListQuery::default()).unwrap().len(), 2);
}

#[test]
fn imported_bookmarks_are_searchable() {
    let dir = tempfile::tempdir().unwrap();
    let vault = Vault::open(dir.path().join("vault")).unwrap();
    let (_, items) = parse(NETSCAPE_SAMPLE);
    vault.import_items(&items).unwrap();

    let hits = vault
        .list_saves(&ListQuery {
            query: Some("rust book".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].title, "The Rust Book");

    let tagged = vault
        .list_saves(&ListQuery {
            tag: Some("Dev & Tools".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(tagged.len(), 1, "folder became a usable tag");
}
