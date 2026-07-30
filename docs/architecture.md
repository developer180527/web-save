# WebSave — Architecture

WebSave is a local-first web bookmark and reference manager: capture pages from
the browser, enrich them with tags and notes, search everything instantly
(including the saved page text), and let a background monitor tell you when
links rot. Everything runs on your machine; nothing is sent to a server.

This document explains how the pieces fit together and why the boundaries are
drawn where they are.

---

## 1. Guiding principles

1. **The data model is independent of any UI.** All storage, search,
   validation, and link-monitoring logic lives in one portable Rust crate
   (`websave-core`) that has zero UI or framework dependencies. Every UI — the
   Tauri desktop app, the native macOS menubar app, a future CLI or mobile app
   — is just a *host* that drives the same core.
2. **One owner of the data.** Exactly one process writes business logic against
   the vault at a time conceptually: the desktop engine. Other components
   (extension, menubar) are thin clients. (The menubar is the one principled
   exception — see §8 — and it coexists safely because SQLite runs in WAL
   mode.)
3. **You own your data.** A vault is a plain directory you can back up, sync, or
   move between machines.
4. **Capture must be reliable and dumb; management is rich and deliberate.** The
   browser extension is a fire-and-forget capture pipe; the desktop app is the
   workbench.

---

## 2. Repository layout

```
web-save/
├── core/            websave-core — portable Rust library (NO UI deps).
│                    Vault storage (SQLite + FTS5), CRUD, tagging, search,
│                    import, link monitoring, thumbnails, archive extraction.
├── ffi/             websave-ffi — UniFFI bindings over the core, for native
│                    hosts (the Swift menubar app today; iOS/Android-ready).
├── src-tauri/       web-save — the desktop "engine": thin Tauri command
│                    wrappers, background link monitor, localhost capture
│                    server, tray + window chrome.
├── src/             React + TypeScript frontend (runs in the Tauri webview).
├── extension/       Chrome MV3 extension — right-click + popup capture.
├── macos-menubar/   Native AppKit menubar app (NSPopover) linking the core
│                    via UniFFI. Built separately with `npm run menubar`.
├── docs/            This document.
└── .github/         Release workflow (cross-platform build matrix).
```

The Cargo **workspace** contains three crates: `core`, `ffi`, `src-tauri`. The
extension and the menubar app are *not* Cargo members — the menubar is built by
its own script and is deliberately excluded from desktop release bundles.

---

## 3. The portable core (`websave-core`)

This crate is the heart of the system. It compiles to a plain library with no
knowledge of Tauri, React, or any UI. Its only inputs are a directory path and
function calls.

### 3.1 The vault

A **vault** is a directory containing:

```
vault/
├── websave.db        SQLite database (WAL mode) + FTS5 full-text index
└── assets/
    └── thumbs/       cached cover images (one file per save)
```

`Vault::open(path)` creates the directory if needed, opens the database in WAL
mode with foreign keys on, and runs migrations.

### 3.2 Vault module layout

`Vault` is one handle over one `Mutex<Connection>`, but its implementation is
split by job. Rust allows inherent `impl` blocks anywhere in the defining crate,
so each submodule adds its own methods and callers still see one flat API:

| File | Owns |
|---|---|
| `vault/mod.rs` | the `Vault` handle, directory layout, `open`, `now()` |
| `vault/saves.rs` | create, read, edit, delete |
| `vault/search.rs` | listing, filtering, FTS5 + BM25 |
| `vault/tags.rs` | the tag graph and the denormalized column FTS reads |
| `vault/link_checks.rs` | which saves are due, and what a check writes back |
| `vault/trash.rs` | the grace period before a dead link is really gone |
| `vault/imports.rs` | preview and apply an import |
| `vault/thumbnails.rs` | cached cover images |
| `vault/migrations.rs` | the migration array and `PRAGMA user_version` |
| `vault/save_rows.rs` | one column list and one row mapper, shared |
| `vault/url_normalization.rs` | canonicalizing URLs |
| `vault/saved_searches.rs`, `metadata.rs`, `stats.rs` | the smaller stores |

### 3.3 Schema and migrations

Migrations are an append-only array of SQL applied in order and tracked by
SQLite's `PRAGMA user_version`. The current schema version is **8**. An existing
vault upgrades itself on launch by applying only the migrations newer than its
recorded version, so upgrades never lose data.

Core tables:

- **`saves`** — one row per bookmark. Columns include `url` (unique, canonical),
  `title`, `description`, `notes`, `favicon_url`, `favorite`, `is_read`,
  `status` (link health), `redirect_url`, `http_status`, `content_hash` (for
  change detection), `tags_text` (denormalized tags for FTS), `thumbnail`
  (relative path under `assets/`), `archive_text` (extracted readable page
  text), `archived_at`, `dead_since`, and `created_at` / `updated_at` /
  `last_checked_at`. Monitoring adds `content_hash_algo` (which hash basis the
  stored hash used), `etag` / `last_modified` (validators for conditional
  requests), and `next_check_at` / `check_interval_secs` / `stable_streak`
  (adaptive scheduling — see §3.6). `next_check_at` is indexed, since selecting
  due saves is the hottest query the monitor runs.
- **`tags`** + **`save_tags`** — normalized many-to-many tags. Orphaned tags are
  cleaned up automatically when their last save is removed.
- **`saves_fts`** — an FTS5 **external-content** virtual table mirroring the
  `saves` table over six columns: `title, url, description, notes, tags,
  archive`. Kept in sync by `AFTER INSERT/UPDATE/DELETE` triggers.
- **`saved_searches`** — persisted query+filter combinations (stored as JSON).
- **`meta`** — a generic key/value store. The core attaches no meaning to the
  keys; the host uses it for things like "when did the extension last check in".

### 3.4 Search (FTS5 + weighted BM25)

Free-form user input is tokenized, each token quoted (so FTS5 operators and
punctuation in the query can never break it) and matched by prefix. Results are
ranked with **weighted BM25** so that fields *you* authored outrank deep matches
inside the archived page body:

```
title (12) > tags (10) > notes (8) > url (6) > description (4) > archive (1)
```

This means searching a phrase you remember from inside an article finds it, but
a title/tag match for the same words still ranks first.

### 3.5 URL canonicalization

Before any insert, URLs are validated (http/https only) and **canonicalized**:
tracking parameters (`utm_*`, `fbclid`, `gclid`, `igshid`, Mailchimp ids,
YouTube/Spotify `si`, …) are stripped while real query params survive. The same
article shared three different ways therefore dedupes onto a single save —
`add_save` upserts by the canonical URL.

### 3.6 Link monitoring

Monitoring is owned by core, not by the hosts. `Monitor` holds the vault, the
sweep config, a cancellation handle and an event sink; the host supplies only
*when* to run and *how* to display progress. That split exists because there are
three hosts (Tauri, SwiftUI/iOS, and a planned Qt shell) and the sweep logic
used to be copy-pasted into each one.

- `Monitor::run_due()` → check everything the schedule says is due.
- `Monitor::run_all()` → force a full sweep, ignoring the schedule ("Re-check
  all links"). Paged by `id`, since applying results rewrites the due times.
- `MonitorConfig` → batch size, concurrency, per-host pacing, and optional
  `max_per_sweep` / `time_budget` caps. iOS uses a tight time budget because its
  foreground window is short; whatever a sweep doesn't reach stays due.
- `MonitorEvent` → `Started`, `Checked`, `HostUnreachable`, `BatchDone`,
  `Finished(SweepSummary)`. The Tauri host maps these onto its IPC events; iOS
  updates an observable.
- `MonitorHandle::cancel()` → stops after the in-flight save, which is still
  persisted.

The module is split so each decision has one obvious home:
`monitor/host_grouping.rs` (carving a batch into per-host queues),
`monitor/host_circuit_breaker.rs` (writing off a host that stopped answering),
`monitor/target_cursor.rs` (what the next batch contains),
`monitor/sweep_state.rs` (live counters and the stop decision), and
`monitor/config.rs` / `events.rs` / `cancel_handle.rs` for the public surface.
`monitor/mod.rs` only orchestrates.

The pieces underneath stay public, so a host with unusual needs can still drive
them directly:

- `saves_due_for_check(limit)` → lightweight targets (id, url, hash, validators)
  that are due now, backed by `idx_saves_next_check`. Returning plain data means
  the network work happens **without holding the vault lock**.
- `probe::check(target)` → performs one blocking HTTP request and
  classifies the result into a `LinkStatus`:
  - `active` — reachable, body unchanged.
  - `changed` — reachable, but the SHA-256 of the body differs from last time.
  - `redirected` — ends at a meaningfully different location (http→https
    upgrades and `www.` changes are treated as the *same* place).
  - `dead` — DNS/transport failure, 404/410, or server error.
  - `unchecked` — never checked.
  - 401/403/429 count as *active* (the page exists, it just blocks bots). A
    status that looks `dead` is **retried once with a browser User-Agent**
    before being believed, which fixes false positives like crates.io serving
    404 to header-less requests.
- `apply_check(id, outcome)` → persists status, http code, redirect target,
  content hash, validators, the next scheduled check, and (if extracted) the
  archive text + timestamp.

The same fetch that checks a link also harvests its **cover image** (`og:image`
/ `twitter:image`) and its **readable text** (see §3.7, §3.8), so monitoring,
thumbnails, and archiving all ride one request.

#### Concurrency: parallel across hosts, serial within one

The unit of work handed to a worker thread is a **host**, not a link. A batch is
grouped by host (`host:port`, `www.` stripped and case-folded, so
`www.example.com` and `example.com` are the one machine they almost always are),
and each group is worked start-to-finish by a single thread.

That grouping is what lets both rules hold at once:

- **Different hosts run in parallel** — up to `max_concurrency`, 16 by default.
  Link checking is IO-bound, so this sits well above the core count.
- **One host is never hit concurrently**, and its requests are spaced by
  `min_host_gap` (1s default). The gap paces request *starts*, so a server
  slower than the gap isn't charged for it twice.

The obvious alternative — one shared queue of links plus a per-host lock — passes
the parallelism test and fails in production: 300 saves from one domain would
park every worker on the same lock. Grouping avoids the head-of-line block
entirely and needs no locking during the request at all.

Workers are scoped OS threads (`std::thread::scope`) running the blocking
prober, not an async runtime. Core is embedded through UniFFI into Swift, and a
Qt shell is planned; a synchronous engine stays embeddable in a synchronous host
without dragging a reactor into it. At 16–24 in-flight sockets the thread cost
is irrelevant, and per-host pacing caps the real concurrency long before thread
count would.

`Prober` holds one `ureq::Agent` for the whole sweep, so its connection pool
survives between requests — the previous code built an agent per URL and paid a
fresh TLS handshake every time, even for hundreds of pages on one host.

Measured on a 20-link vault spread across 18 hosts: **34.1s serial → 2.6s at
concurrency 16**. With validators stored, a re-check of the same vault runs in
2.4s, of which 12 links settled on a 304.

#### Dead-host circuit breaker

Because a worker owns a host's whole queue, it can also notice the host is
gone. After `host_failure_threshold` (3) consecutive **transport** failures —
DNS failure, connection refused, TLS failure, timeout — the sweep abandons that
host's remaining links. State is per sweep and shared across batches, so a dead
domain is learned once, not once per batch.

Two rules make it safe:

- **Only transport failures count.** A 404 is a healthy server answering a
  question about one page and predicts nothing about the next link on that host.
- **Skipped links are not judged.** They keep their stored status and stay due,
  and no `dead_since` countdown is started. Inheriting a `dead` verdict from
  their neighbours would be cheap and wrong: the same signal is produced by the
  user's wifi being off, and would march a whole vault into the Trash's 30-day
  deletion during a train journey. The breaker is an optimisation, not a
  verdict — a later sweep decides. A vault converges on the truth over
  successive sweeps, three links per host at a time.

Because skipped links stay due — and, being the oldest-due rows, sort to the
front of the next query — the due cursor has to offset past them or the sweep
would be handed the same batch forever. That's what
`saves_due_for_check(limit, offset)`'s second argument is for.

Measured against a host that accepts connections and never answers, 10 links at
the 20s probe timeout: **200s → 60s**. The saving scales with the number of
links on the host, since the breaker's cost is a constant 3 probes per host: 200
saved pages from a lapsed domain go from ~67 minutes to one.

#### Adaptive scheduling (`schedule.rs`)

The original rule was "every link, every 24 hours". That is one request per save
per day regardless of the vault, so a 10k-save vault needed a sweep that could
not finish before the next was due — and nearly all of it re-fetched pages that
had not changed in months.

Each save now carries its own `next_check_at`, `check_interval_secs` and
`stable_streak`:

- Unchanged → the interval doubles, from a **6-hour floor** to a **30-day
  ceiling**.
- Changed or redirected → straight back to the floor, streak reset.
- Dead → a fixed **12-hour** cadence. Dead links are on a 30-day deletion
  countdown, and only a check can notice the site coming back; backing them off
  would shrink their chance of recovery the longer they'd been gone.
- Favorite or unread → capped at **24 hours**. These are the saves where a
  stale status is actually noticed.
- Due times are jittered ±10%, derived from the save id, so a 5,000-link import
  doesn't come due as one thundering herd. Deterministic, so no random source
  and no untestable schedule.

The daily working set therefore tracks how volatile the vault is rather than how
large it is.

#### Conditional requests

A save stores the `ETag` and `Last-Modified` of its last successful fetch and
sends them back as `If-None-Match` / `If-Modified-Since`. A **304** settles the
check for a few hundred bytes: no body download, no text extraction, no hashing,
and every stored content field is left untouched.

Validators are only sent when the stored hash is present *and* computed under
the current `HASH_ALGO` — a stale hash basis has to re-baseline, and
re-baselining needs a body. See `docs/content-hashing.md`.

### 3.7 Thumbnails

On a check, the core extracts the page's declared cover image URL, downloads it
once (size/content-type validated), and stores it under `assets/thumbs/{id}.ext`,
recording the relative path on the save. It never overwrites an existing
thumbnail (so an extension-supplied screenshot, see §7, is preserved).

### 3.8 Archive snapshots

The core extracts the readable text of each checked page (scripts/styles/chrome
stripped, the `<article>`/`<main>` subtree preferred, whitespace normalized,
capped at 100K chars) and stores it in `archive_text`, which feeds the FTS
`archive` column. Result: pages stay **searchable by content even after they go
dead**, and you can read the snapshot from the edit panel.

### 3.9 Import

`import.rs` parses four bookmark formats by sniffing the file: Netscape bookmark
HTML (every browser's export, plus Pocket's HTML variant), Raindrop.io CSV,
Pocket CSV, and plain/markdown URL lists. `preview_import` is a dry run (counts
new / already-saved / invalid); `import_items` merges in one transaction with
**non-destructive** semantics: existing saves only gain empty fields, tags are
unioned, `favorite` is sticky, and `created_at` keeps the earliest known date.

---

## 4. The vault on disk (portability)

Because everything is one SQLite file plus an assets folder, the vault is fully
portable. On macOS it lives at:

```
~/Library/Application Support/com.venugopal.web-save/vault/
```

Back it up, drop it in a synced folder, or copy it to another machine — saves,
tags, saved searches, thumbnails, and archive text all travel together. The
database is in WAL mode, which is what makes the multi-process menubar scenario
(§8) safe.

---

## 5. The desktop engine (`src-tauri`)

The engine is a Tauri 2 application. It is the only component that opens the
vault with full business logic, and it hosts three long-running concerns.

### 5.1 Commands and state

The vault is wrapped in `Arc<Vault>` and stored as Tauri managed state. Every
`#[tauri::command]` is a **thin wrapper** that calls a core method and maps
errors to strings — no business logic lives in the command layer. Mutating
commands emit a `saves-updated` event so every surface (main window, tray,
menubar) refreshes.

### 5.2 Background link monitor (with a wake channel)

A single background thread drives `websave_core::Monitor`. Instead of a blind
timer it waits on an `mpsc` channel with a timeout (`recv_timeout`):

- It wakes on its own at least every 30 minutes and runs `run_due()`. Waking
  often is cheap — which saves are actually due is `schedule.rs`'s decision
  (§3.6), so most wake-ups find little or nothing to do. A user-triggered
  "Re-check all links" runs at higher concurrency (24) since someone is watching
  it; per-host pacing is the same either way, so no individual server notices.
- It wakes **immediately** when a new URL is added — `add_save` (UI), the
  capture server (extension), and import all poke a `MonitorWaker`. A
  one-second debounce coalesces bursts into a single batch.

So a freshly captured page gets its first health check, thumbnail, and archive
within seconds, not at the next half-hour tick.

### 5.3 Localhost capture server

A second background thread runs a tiny HTTP server bound to **`127.0.0.1:38917`**
(loopback only — never exposed to the network). Endpoints:

- `GET /ping` → `{"app":"websave","version":"…"}` — lets clients detect the app.
- `POST /save` → validate + `add_save`, with optional screenshot ingestion.
- `GET /show` → raise the main window (used by the menubar "Open WebSave").
- `GET /reload` → emit `saves-updated` (used by the menubar after it writes).

**Security model.** The server binds to loopback, sends **no CORS headers**, and
requires a custom `x-websave-client` header on `/save`. A custom header forces
browsers to send a CORS preflight, which fails without CORS headers — so a
random web page cannot POST into your vault. The extension is exempt because
Chrome lets it call hosts listed in its `host_permissions` directly. The
extension also sends `x-websave-ext-version`, which the engine records in `meta`
so the app can show "extension connected (vX)".

### 5.4 Windowing, tray, and chrome

- **Custom title bar.** On macOS the window uses `TitleBarStyle::Overlay` so the
  **native traffic lights** stay (nudged into alignment with the title via
  `tauri-plugin-decorum`, which re-applies the inset on resize); on
  Windows/Linux the window is frameless and the frontend renders its own
  minimize/maximize/close controls. The whole bar is a drag region.
- **Tray quick access** is platform-split: on Windows/Linux the engine shows a
  tray icon with a webview popover (left click) and a native menu (right click).
  On **macOS the engine creates no tray and no popover window** — the native
  menubar app owns that surface, saving a whole webview.
- **Lifecycle.** Closing the main window *hides* it (close-to-tray) so capture
  and monitoring keep running; quitting happens from the tray/menu.

Other plugins: clipboard-manager (for "Paste & Add"), autostart (launch at
login), dialog (import file picker), and log (file + stdout + webview console).

---

## 6. The React frontend (`src`)

A React + TypeScript app rendered inside the Tauri webview. It is a pure client
of the engine: it calls commands via `invoke` and listens for `saves-updated`.

Highlights:

- **`api.ts`** is the single typed boundary — every backend call is a one-line
  wrapper.
- **Data fetching is split for responsiveness.** The save list re-queries
  immediately on any filter change (one IPC call); search input is debounced;
  the sidebar's tag list and counts are fetched separately and only refreshed on
  mutations. This keeps navigation instant.
- **Sidebar** offers mutually-exclusive views (All, Inbox, Favorites, link-health
  statuses), tag filters, and saved searches, each with an icon and live count.
- **List vs. card view** — card view shows cached cover thumbnails (with a
  deterministic per-host gradient fallback for pages that declare none).
- **Edit panel** — title/description/notes/tags, link-health with "Accept new
  URL" (redirects) and "Open in Wayback Machine" (dead), the archived-text
  viewer, and check-now.
- **Command palette (⌘K)** and a **keyboard layer** (j/k navigate, Enter open, E
  edit, S star, R read, X select, `/` search).
- **Bulk operations** via multi-select (⌘-click, shift-range, ⌘A).
- **Paste & Add** — when a URL sits on the clipboard, the Add button becomes a
  one-click "Paste & Add" (clipboard is read only while the window is focused).
- **Inbox / read state** — new captures arrive unread; opening one marks it read.

The same bundle also renders the tray popover on Windows/Linux: `main.tsx`
checks the window label and mounts a compact `QuickPanel` instead of the full
app.

---

## 7. The browser extension (`extension`)

A Manifest V3 extension whose entire job is to deliver `{url, title,
description, faviconUrl, tags}` to the engine reliably. It has **no read access**
to the vault — it is a write-only capture pipe, by design.

**Cross-browser by construction.** Because the extension talks to the engine
over **localhost HTTP** rather than browser-specific native messaging, the same
code runs on every browser. The source lives once in `extension/`; a tiny build
script (`extension/build.mjs`, run via `npm run ext:build`) emits per-browser
packages under `extension/dist/`: Chrome/Edge/Brave use the MV3 service-worker
manifest as-is; Firefox gets an event-page `background.scripts` manifest plus a
`gecko` id and a `data_collection_permissions: ["none"]` declaration (it
collects no data). A one-line shim aliases `chrome.*` to the promise-based
`browser.*` namespace on Firefox/Safari, so the `await chrome.*` calls work
unchanged. The Firefox package passes `web-ext lint` with zero findings. Safari
would additionally need a native-app wrapper (`safari-web-extension-converter`)
and a local-network permission for the localhost call — tracked as future work.

- **Capture paths:** right-click context menus (page / link / selection) capture
  instantly; the toolbar button opens a **popup** that pings `/ping`, shows
  connection status, offers a "Save this page" button, and links to the app
  download when the engine isn't running.
- **Cover images:** if the page declares an `og:image`, the engine fetches it
  (§3.7). If it does *not*, the extension captures a **screenshot** of the
  visible tab (`captureVisibleTab`) and includes it in the payload; the engine
  decodes and stores it as the thumbnail. This is the one thing only the
  extension can do, because it lives inside the browser.
- **Offline queue:** if the app isn't running, saves are queued in
  `chrome.storage.local` and a one-minute alarm flushes them when the app comes
  back. Screenshots are stripped from queued items to respect the storage quota.

**Mutual discovery.** The detection is asymmetric: the *extension* can always
detect the app via `/ping`, but the *app* can only learn the extension exists
once it receives a capture (recorded in `meta` as `ext.last_seen`). So the app
never claims the extension is absent — it shows a dismissible "Get the
extension" onboarding card until the first capture arrives, then flips to
"connected". The offline queue makes install order irrelevant.

---

## 8. The macOS menubar companion (`macos-menubar`)

A native AppKit app (`NSStatusItem` + transient `NSPopover` + `NSTableView`) for
quick access to starred and recent saves from the menubar. It is the
architecture's one deliberate second reader of the vault, and it demonstrates
the portability promise:

- It links **`websave-core` directly** through the UniFFI bindings in `ffi/`.
  UniFFI generates idiomatic Swift from the Rust surface; the FFI dylib is
  embedded in the app bundle (`Contents/Frameworks`, referenced via `@rpath`) so
  the app is self-contained.
- It opens the **same vault directory** as the engine. Concurrent access is safe
  because SQLite is in WAL mode; the popup re-queries each time it opens.
- After it writes (e.g. toggling a star) it nudges the engine over localhost
  (`GET /reload`) so the desktop UI refreshes; "Open WebSave" calls `GET /show`.

It is AppKit rather than SwiftUI specifically to keep the always-running utility
lightweight (~12 MB resident vs. ~140 MB for the SwiftUI equivalent). It is a
*fancy utility on top of the engine*, never a replacement for it.

**Lifecycle — embedded as a login-item helper.** The menubar app is shipped
*inside* the engine bundle at
`web-save.app/Contents/Library/LoginItems/WebSave Menubar.app`. Because it lives
under `Contents/`, the two share a single lifecycle: it installs as one download
and **deleting (trashing) the engine app removes the menubar with it**. The
engine launches this embedded copy first (resolved relative to its own
executable; `launch_menubar_app` falls back to a loose install, the in-repo dev
build, or LaunchServices). `macos-menubar/embed.sh <engine.app>` builds the
helper and injects it (re-signing inside-out); `npm run build:mac` does a full
packaged build (`tauri build` + embed). The CI release workflow builds the
engine only — embedding is a macOS-packaging step layered on top, so the helper
is never bundled into Windows/Linux artifacts.

---

## 9. Data-flow walkthroughs

**Capture from the extension**

```
right-click / popup → background.js builds payload (+ screenshot if no og:image)
  → POST http://127.0.0.1:38917/save  (x-websave-client header)
  → engine: add_save (canonicalize + upsert) → store screenshot as thumb
  → record ext.last_seen → wake monitor → emit "saves-updated"
  → monitor checks the link, fetches og:image/archive within seconds
  → frontend (and menubar) refresh on the event
```
If the engine is down, the POST fails, the extension queues the save, and a
1-minute alarm retries until `/save` succeeds.

**Manual add / Paste & Add** — the frontend calls the `add_save` command
directly (no HTTP), which runs the same canonicalize → upsert → wake-monitor
path.

**Search** — the frontend calls `list_saves` with a `ListQuery`; the core builds
an FTS5 `MATCH` expression and a weighted `bm25()` order, ANDs in any
tag/favorite/unread/status filters, and returns ranked saves.

---

## 10. Cross-cutting concerns

- **Security:** loopback-only server, custom-header CORS gate, http/https-only
  URLs, no personal data ever leaves the machine.
- **Versioning:** the version lives in `tauri.conf.json`, the three crate
  `Cargo.toml`s, `package.json`, the extension manifest, and the menubar
  `Info.plist`. `/ping` and the `app_version` command report it.
- **Build & release:** a GitHub Actions matrix builds the engine for macOS
  (ARM/Intel), Linux (x64/ARM), and Windows (x64/ARM) on native runners and
  publishes installers to a draft release. The menubar companion is built
  separately and never included in those bundles.

---

## 11. Extensibility

The whole point of the core/host split is reuse:

- **Noter plugin** — Noter (another Tauri app) can depend on `websave-core`
  directly and embed WebSave's capabilities, exactly as the engine does.
- **CLI** — a `websave add/search` binary is a few lines over the core.
- **Mobile** — the UniFFI layer that backs the macOS app also targets iOS and
  Android; a share-sheet capture extension over the same `add_save` is the
  natural entry point.

In every case the storage, search, validation, and monitoring come for free from
the core; only the presentation is new.
