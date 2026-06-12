# WebSave

Local-first web bookmark and reference manager. A lightweight, fast alternative
to browser bookmarks and services like Raindrop.io: capture pages, enrich them
with tags and notes, search everything instantly, and let a background monitor
tell you when links rot.

## Architecture

```
web-save/
├── core/           websave-core — portable Rust library (no UI dependencies)
│                   vault storage (SQLite + FTS5), CRUD, tagging, search,
│                   link monitoring. Reusable from any host: this app, a CLI,
│                   mobile, or as a plugin in another Tauri app (e.g. Noter).
├── ffi/            websave-ffi — UniFFI bindings over the core for native
│                   hosts (Swift today; Kotlin/iOS-ready).
├── macos-menubar/  Native SwiftUI menubar app (MenuBarExtra popover with
│                   search, starred & recent) linking websave-core directly.
│                   Build & launch: npm run menubar
├── src-tauri/      Tauri shell — thin #[tauri::command] wrappers around the
│                   vault, a background link-monitor thread, and a localhost
│                   capture server (127.0.0.1:38917) for the extension.
├── extension/      Chrome extension (MV3) — right-click capture. Pure capture
│                   client: gathers url/title/description/favicon and POSTs to
│                   the capture server; queues offline saves in the browser.
└── src/            React + TypeScript frontend (list, search, filters,
                    tag/notes editor, settings).
```

Quick access per platform: on macOS the SwiftUI menubar app owns the tray —
the engine creates no tray icon there. On Windows/Linux the engine shows a
tray icon with a webview popover (left click) and a native menu (right
click). The menubar app reads/writes the same WAL-mode SQLite vault as the
engine and nudges it over localhost (`/reload` after writes, `/show` to
raise the main window) so both UIs stay in sync.

### The vault

All data lives in a portable directory (`~/Library/Application
Support/com.venugopal.web-save/vault` on macOS):

- `websave.db` — SQLite database (WAL mode) with an FTS5 full-text index over
  titles, URLs, descriptions, notes and tags. Schema changes are applied via
  `PRAGMA user_version` migrations.
- `assets/` — reserved for local assets such as thumbnails.

Back it up, sync it, or move it between machines — it is self-contained.

### Link monitoring

Each save carries a status: `unchecked`, `active`, `changed` (content hash
differs from the last check), `redirected` (now resolves somewhere
meaningfully different; http→https and `www.` changes don't count), or `dead`
(DNS/transport failure, 404/410, server errors). A background thread re-checks
stale links in small batches every 30 minutes; the UI can also check a single
link or re-check everything on demand.

### Capture flow

The extension never touches the database. It POSTs a `NewSave` JSON payload
to `http://127.0.0.1:38917/save`; the app validates, stores, and notifies the
UI. The endpoint binds to loopback only, sends no CORS headers, and requires
an `x-websave-client` header (which forces a failing preflight for normal web
pages), so random websites can't write into your vault. If the app isn't
running, the extension queues saves in `chrome.storage.local` and retries
every minute.

## Development

```sh
npm install
npm run tauri dev      # run the desktop app
npm run menubar        # (macOS) build + launch the native menubar app
cargo test --workspace # core test suite
```

`npm run tauri build` bundles only the engine (desktop app); the menubar
companion is a separate macOS-only artifact produced by `npm run menubar`
(`macos-menubar/dist/WebSave Menubar.app`) and is never included in
mac/Windows/Linux release bundles. The engine can launch it on demand
(Settings → Menubar app), looking it up via LaunchServices, /Applications,
next to the engine binary, or the in-repo dev build.

### Installing the extension

1. Open `chrome://extensions` and enable **Developer mode**.
2. Click **Load unpacked** and pick this repo's `extension/` folder.
3. With the desktop app running, right-click any page → *Save page to
   WebSave* (or click the toolbar button). Links and selections have their
   own context-menu entries.

## Roadmap

- [x] Phase 1 — portable core (`websave-core`), Tauri commands, desktop UI
- [x] Phase 2 — Chrome extension (right-click capture → desktop app)
- [x] Phase 3 — tray quick access: native SwiftUI menubar app on macOS
      (UniFFI → websave-core); webview popover + native menu on Windows/Linux
- [ ] Later — thumbnails in `assets/`, import/export, Noter plugin packaging
