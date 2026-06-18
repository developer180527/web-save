<<<<<<< HEAD
<img width="1262" height="872" alt="Screenshot 2026-06-16 at 5 31 50 PM" src="https://github.com/user-attachments/assets/ae83e25e-9121-4131-b8af-8e800af6a485" />
=======
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
npm run tauri dev      # run the desktop app (dev)
npm run menubar        # (macOS) build + launch the menubar app for dev
npm run build:mac      # (macOS) packaged build: engine + embedded menubar
cargo test --workspace # core test suite
```

On macOS the menubar companion ships **inside** the engine bundle as a
login-item helper (`web-save.app/Contents/Library/LoginItems/WebSave
Menubar.app`), so it installs as one download and is deleted together with the
engine app. `npm run build:mac` runs `tauri build` then
`macos-menubar/embed.sh` to inject and re-sign it. The engine launches that
embedded copy on demand (Settings → Menubar app), falling back to a loose
install, the in-repo dev build, or LaunchServices.

The menubar is macOS-only and is never part of the Windows/Linux bundles. The
GitHub Actions release workflow builds the engine for all platforms; embedding
the menubar is the extra macOS-packaging step (`build:mac`) layered on top.

### Installing the extension

The extension is one shared source (`extension/`) that builds into per-browser
packages. The localhost capture channel works identically in every browser, so
the code is shared; only the manifest differs.

```sh
npm run ext:build      # → extension/dist/chrome/ and extension/dist/firefox/
```

**Chrome / Edge / Brave**
1. Open `chrome://extensions`, enable **Developer mode**.
2. **Load unpacked** → pick `extension/dist/chrome` (or `extension/` directly).

**Firefox**
1. Open `about:debugging#/runtime/this-firefox` → **Load Temporary Add-on** →
   pick `extension/dist/firefox/manifest.json`.
   (Or `npm run ext:firefox` to launch a dev Firefox with auto-reload via
   `web-ext`.)
2. For a permanent install, the package must be signed by Mozilla (AMO) —
   `npx web-ext sign -s extension/dist/firefox`.

**Safari** — not yet packaged. Safari requires wrapping the web extension in a
native app via `xcrun safari-web-extension-converter extension/dist/chrome`,
plus an Apple Developer account; the localhost capture call may also need the
containing app's local-network permission. Tracked as future work.

With the desktop app running, right-click any page → *Save page to WebSave*, use
the toolbar popup, or use the link/selection context-menu entries.

## Roadmap

- [x] Phase 1 — portable core (`websave-core`), Tauri commands, desktop UI
- [x] Phase 2 — Chrome extension (right-click capture → desktop app)
- [x] Phase 3 — tray quick access: native SwiftUI menubar app on macOS
      (UniFFI → websave-core); webview popover + native menu on Windows/Linux
- [x] Thumbnails (og:image + extension screenshots), bookmark import,
      archive snapshots (searchable readable-text copies of saved pages)
- [x] Firefox extension (shared source → per-browser packages)
- [ ] Later — export, Safari extension (native wrapper), Noter plugin packaging
>>>>>>> af5cbee (Did changes in background saving and updated macos menubar app building.)
