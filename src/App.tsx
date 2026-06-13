import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { readText } from "@tauri-apps/plugin-clipboard-manager";
import { firstWebUrl, hostOf } from "./utils";
import * as api from "./api";
import type {
  ListQuery,
  Save,
  SavedSearch,
  TagCount,
  VaultStats,
} from "./types";
import Sidebar, { type View } from "./components/Sidebar";
import SaveCard from "./components/SaveCard";
import EditPanel from "./components/EditPanel";
import SettingsPage, {
  MENUBAR_AUTOLAUNCH_KEY,
  type Theme,
} from "./components/SettingsPage";
import ImportDialog from "./components/ImportDialog";
import CommandPalette, {
  type PaletteAction,
} from "./components/CommandPalette";
import TitleBar from "./components/TitleBar";
import {
  ChevronUpIcon,
  ClipboardIcon,
  GridIcon,
  ImportIcon,
  ListIcon,
  PlusIcon,
  SearchIcon,
} from "./components/Icons";
import "./App.css";

const MIN_SIDEBAR = 180;
const MAX_SIDEBAR = 420;

function App() {
  const [saves, setSaves] = useState<Save[]>([]);
  const [tags, setTags] = useState<TagCount[]>([]);
  const [stats, setStats] = useState<VaultStats | null>(null);
  const [view, setView] = useState<View>("all");
  const [activeTag, setActiveTag] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [selected, setSelected] = useState<Save | null>(null);
  const [addUrl, setAddUrl] = useState("");
  const [adding, setAdding] = useState(false);
  // A web URL sitting on the clipboard, surfaced as one-click "Paste & Add".
  const [clipboardUrl, setClipboardUrl] = useState<string | null>(null);
  const [handledClipboard, setHandledClipboard] = useState<string | null>(null);
  const [rechecking, setRechecking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [screen, setScreen] = useState<"library" | "settings">("library");
  const [importOpen, setImportOpen] = useState(false);
  const [addOpen, setAddOpen] = useState(false);
  const [vaultDir, setVaultDir] = useState("");
  const [savedSearches, setSavedSearches] = useState<SavedSearch[]>([]);
  const [saveSearchOpen, setSaveSearchOpen] = useState(false);
  const [saveSearchName, setSaveSearchName] = useState("");
  const [paletteOpen, setPaletteOpen] = useState(false);

  // Multi-selection for bulk operations.
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());
  const [anchorId, setAnchorId] = useState<number | null>(null);
  const [bulkTagOpen, setBulkTagOpen] = useState(false);
  const [bulkTag, setBulkTag] = useState("");

  // Keyboard focus (j/k).
  const [focusedIdx, setFocusedIdx] = useState(-1);
  const searchRef = useRef<HTMLInputElement>(null);

  const [viewMode, setViewMode] = useState<"list" | "cards">(
    () => (localStorage.getItem("viewMode") as "list" | "cards") || "list",
  );
  useEffect(() => {
    localStorage.setItem("viewMode", viewMode);
  }, [viewMode]);
  useEffect(() => {
    api.vaultPath().then(setVaultDir).catch(() => {});
  }, []);

  // The add popover closes on Escape or any outside click.
  useEffect(() => {
    if (!addOpen && !saveSearchOpen && !bulkTagOpen) return;
    const close = () => {
      setAddOpen(false);
      setSaveSearchOpen(false);
      setBulkTagOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    window.addEventListener("click", close);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("click", close);
      window.removeEventListener("keydown", onKey);
    };
  }, [addOpen, saveSearchOpen, bulkTagOpen]);

  const [theme, setTheme] = useState<Theme>(
    () => (localStorage.getItem("theme") as Theme) || "system",
  );
  useEffect(() => {
    if (theme === "system") {
      document.documentElement.removeAttribute("data-theme");
    } else {
      document.documentElement.setAttribute("data-theme", theme);
    }
    localStorage.setItem("theme", theme);
  }, [theme]);

  const [sidebarWidth, setSidebarWidth] = useState(
    () => Number(localStorage.getItem("sidebarWidth")) || 230,
  );
  useEffect(() => {
    localStorage.setItem("sidebarWidth", String(sidebarWidth));
  }, [sidebarWidth]);

  function startSidebarResize(e: React.MouseEvent) {
    e.preventDefault();
    const onMove = (ev: MouseEvent) =>
      setSidebarWidth(Math.min(MAX_SIDEBAR, Math.max(MIN_SIDEBAR, ev.clientX)));
    const onUp = () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      document.body.classList.remove("resizing");
    };
    document.body.classList.add("resizing");
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  }

  function currentQuery(): ListQuery {
    return {
      query: search.trim() || null,
      tag: activeTag,
      favoritesOnly: view === "favorites",
      unreadOnly: view === "inbox",
      status:
        view !== "all" && view !== "favorites" && view !== "inbox"
          ? view
          : null,
    };
  }

  const refresh = useCallback(async () => {
    const query: ListQuery = {
      query: search.trim() || null,
      tag: activeTag,
      favoritesOnly: view === "favorites",
      unreadOnly: view === "inbox",
      status:
        view !== "all" && view !== "favorites" && view !== "inbox"
          ? view
          : null,
    };
    try {
      const [s, t, st] = await Promise.all([
        api.listSaves(query),
        api.listTags(),
        api.vaultStats(),
      ]);
      setSaves(s);
      setTags(t);
      setStats(st);
    } catch (e) {
      setError(String(e));
    }
  }, [search, activeTag, view]);

  // Debounced refresh whenever search/filters change (and on mount).
  useEffect(() => {
    const timer = setTimeout(refresh, 120);
    return () => clearTimeout(timer);
  }, [refresh]);

  // Keep keyboard focus inside the list as it changes.
  useEffect(() => {
    setFocusedIdx((i) => Math.min(i, saves.length - 1));
  }, [saves]);

  const loadSavedSearches = useCallback(() => {
    api.listSavedSearches().then(setSavedSearches).catch(() => {});
  }, []);
  useEffect(loadSavedSearches, [loadSavedSearches]);

  // Watch the clipboard for a web URL (only while the window is focused, to
  // avoid reading it in the background) and offer one-click "Paste & Add".
  useEffect(() => {
    let cancelled = false;
    async function check() {
      if (!document.hasFocus()) return;
      try {
        const text = await readText();
        const url = firstWebUrl(text);
        if (!cancelled) setClipboardUrl(url);
      } catch {
        // clipboard empty or non-text — ignore
      }
    }
    check();
    const onFocus = () => check();
    window.addEventListener("focus", onFocus);
    const timer = setInterval(check, 1500);
    return () => {
      cancelled = true;
      window.removeEventListener("focus", onFocus);
      clearInterval(timer);
    };
  }, []);

  // A handled URL stays suppressed until the clipboard contents change.
  useEffect(() => {
    if (clipboardUrl && clipboardUrl !== handledClipboard) {
      setHandledClipboard(null);
    }
  }, [clipboardUrl, handledClipboard]);

  // Bring the macOS menubar companion up alongside the engine, if enabled.
  useEffect(() => {
    if (
      navigator.userAgent.includes("Mac") &&
      localStorage.getItem(MENUBAR_AUTOLAUNCH_KEY) === "true"
    ) {
      api.launchMenubarApp().catch(() => {});
    }
  }, []);

  // Background monitor (and capture clients) signal through this event.
  const refreshRef = useRef(refresh);
  refreshRef.current = refresh;
  useEffect(() => {
    const unlisten = listen("saves-updated", () => refreshRef.current());
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  function applyChange(updated: Save) {
    setSaves((prev) => prev.map((s) => (s.id === updated.id ? updated : s)));
    setSelected((prev) => (prev?.id === updated.id ? updated : prev));
    refresh();
  }

  async function saveUrl(raw: string) {
    const url = raw.trim();
    if (!url) return;
    setAdding(true);
    try {
      const save = await api.addSave({
        url: /^https?:\/\//i.test(url) ? url : `https://${url}`,
      });
      setAddUrl("");
      setAddOpen(false);
      setSelected(save);
      setError(null);
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setAdding(false);
    }
  }

  function handleAdd(e: React.FormEvent) {
    e.preventDefault();
    saveUrl(addUrl);
  }

  async function handlePasteAndAdd() {
    if (!clipboardUrl) return;
    // Don't re-suggest this URL until the clipboard changes again.
    setHandledClipboard(clipboardUrl);
    setClipboardUrl(null);
    await saveUrl(clipboardUrl);
  }

  async function handleToggleFavorite(save: Save) {
    try {
      applyChange(await api.setFavorite(save.id, !save.favorite));
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleToggleRead(save: Save) {
    try {
      applyChange(await api.setRead(save.id, !save.isRead));
    } catch (e) {
      setError(String(e));
    }
  }

  /** Open in browser; an opened save is a read save. */
  async function handleOpen(save: Save) {
    try {
      await openUrl(save.url);
    } catch (e) {
      setError(String(e));
      return;
    }
    if (!save.isRead) {
      try {
        applyChange(await api.setRead(save.id, true));
      } catch {
        // non-fatal
      }
    }
  }

  function toggleSelect(id: number) {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }

  /** Plain click opens; ⌘-click toggles selection; shift-click selects a range. */
  function handleCardClick(e: React.MouseEvent, save: Save) {
    if (e.metaKey || e.ctrlKey) {
      toggleSelect(save.id);
      setAnchorId(save.id);
      return;
    }
    if (e.shiftKey && anchorId !== null) {
      const ai = saves.findIndex((s) => s.id === anchorId);
      const bi = saves.findIndex((s) => s.id === save.id);
      if (ai >= 0 && bi >= 0) {
        const [lo, hi] = ai < bi ? [ai, bi] : [bi, ai];
        setSelectedIds((prev) => {
          const next = new Set(prev);
          for (let i = lo; i <= hi; i++) next.add(saves[i].id);
          return next;
        });
        return;
      }
    }
    handleOpen(save);
  }

  async function handleDelete(save: Save) {
    if (!confirm(`Delete "${save.title || save.url}"?`)) return;
    try {
      await api.deleteSave(save.id);
      setSaves((prev) => prev.filter((s) => s.id !== save.id));
      setSelected((prev) => (prev?.id === save.id ? null : prev));
      refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleRecheckAll() {
    setRechecking(true);
    try {
      await api.recheckAll();
    } catch (e) {
      setError(String(e));
    } finally {
      setRechecking(false);
    }
  }

  // ---- bulk operations ----

  const bulkIds = [...selectedIds];

  async function runBulk(op: () => Promise<void>) {
    try {
      await op();
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  async function bulkDeleteSelected() {
    if (!confirm(`Delete ${bulkIds.length} saves?`)) return;
    await runBulk(() => api.bulkDelete(bulkIds));
    setSelectedIds(new Set());
    setSelected((prev) =>
      prev && selectedIds.has(prev.id) ? null : prev,
    );
  }

  // ---- saved searches ----

  const filtersActive =
    search.trim() !== "" || activeTag !== null || view !== "all";

  async function handleSaveSearch(e: React.FormEvent) {
    e.preventDefault();
    if (!saveSearchName.trim()) return;
    try {
      await api.addSavedSearch(saveSearchName.trim(), currentQuery());
      setSaveSearchName("");
      setSaveSearchOpen(false);
      loadSavedSearches();
    } catch (e) {
      setError(String(e));
    }
  }

  function applySavedSearch(s: SavedSearch) {
    setScreen("library");
    setSearch(s.query.query ?? "");
    setActiveTag(s.query.tag ?? null);
    setView(
      s.query.favoritesOnly
        ? "favorites"
        : s.query.unreadOnly
          ? "inbox"
          : (s.query.status ?? "all"),
    );
    setSelected(null);
  }

  // ---- keyboard layer ----

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement;
      const typing =
        target.tagName === "INPUT" ||
        target.tagName === "TEXTAREA" ||
        target.isContentEditable;

      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPaletteOpen((v) => !v);
        return;
      }
      if (typing || paletteOpen || importOpen || screen === "settings") return;

      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "a") {
        e.preventDefault();
        setSelectedIds(new Set(saves.map((s) => s.id)));
        return;
      }
      if (e.metaKey || e.ctrlKey || e.altKey) return;

      const focusedSave =
        focusedIdx >= 0 && focusedIdx < saves.length
          ? saves[focusedIdx]
          : undefined;

      switch (e.key) {
        case "j":
        case "ArrowDown":
          e.preventDefault();
          setFocusedIdx((i) => Math.min(i + 1, saves.length - 1));
          break;
        case "k":
        case "ArrowUp":
          e.preventDefault();
          setFocusedIdx((i) => Math.max(i - 1, 0));
          break;
        case "Enter":
          if (focusedSave) handleOpen(focusedSave);
          break;
        case "e":
          if (focusedSave) setSelected(focusedSave);
          break;
        case "s":
          if (focusedSave) handleToggleFavorite(focusedSave);
          break;
        case "r":
          if (focusedSave) handleToggleRead(focusedSave);
          break;
        case "x":
          if (focusedSave) {
            toggleSelect(focusedSave.id);
            setAnchorId(focusedSave.id);
          }
          break;
        case "/":
          e.preventDefault();
          searchRef.current?.focus();
          break;
        case "Escape":
          if (selectedIds.size > 0) {
            setSelectedIds(new Set());
          } else if (selected) {
            setSelected(null);
          } else {
            setFocusedIdx(-1);
          }
          break;
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const paletteActions: PaletteAction[] = [
    { id: "add", label: "Add a URL…", run: () => setAddOpen(true) },
    { id: "import", label: "Import bookmarks…", run: () => setImportOpen(true) },
    {
      id: "view",
      label:
        viewMode === "list" ? "Switch to card view" : "Switch to list view",
      run: () => setViewMode((m) => (m === "list" ? "cards" : "list")),
    },
    {
      id: "inbox",
      label: "Go to Inbox",
      run: () => {
        setScreen("library");
        setView("inbox");
      },
    },
    { id: "settings", label: "Open settings", run: () => setScreen("settings") },
    { id: "recheck", label: "Re-check all links", run: handleRecheckAll },
    ...savedSearches.map((s) => ({
      id: `ss-${s.id}`,
      label: `Saved search: ${s.name}`,
      run: () => applySavedSearch(s),
    })),
  ];

  const showPanel = screen === "library" && selected !== null;
  const gridColumns = `${sidebarWidth}px 5px 1fr${showPanel ? " 360px" : ""}`;
  // Offer one-click paste only for a fresh, unhandled clipboard URL, and not
  // while the manual input is open.
  const pasteReady =
    clipboardUrl !== null && clipboardUrl !== handledClipboard && !addOpen;

  return (
    <div className="app-shell">
      <TitleBar />
      <div className="app" style={{ gridTemplateColumns: gridColumns }}>
        <Sidebar
        view={view}
        onViewChange={(v) => {
          setView(v);
          setSelected(null);
          setScreen("library");
        }}
        tags={tags}
        activeTag={activeTag}
        onTagChange={(t) => {
          setActiveTag(t);
          setScreen("library");
        }}
        stats={stats}
        savedSearches={savedSearches}
        onApplySavedSearch={applySavedSearch}
        onDeleteSavedSearch={(id) =>
          api.deleteSavedSearch(id).then(loadSavedSearches).catch((e) =>
            setError(String(e)),
          )
        }
        onRecheckAll={handleRecheckAll}
        rechecking={rechecking}
        onOpenSettings={() =>
          setScreen((s) => (s === "settings" ? "library" : "settings"))
        }
        settingsOpen={screen === "settings"}
      />

      <div className="sidebar-resizer" onMouseDown={startSidebarResize} />

      {screen === "settings" ? (
        <main className="main">
          {error && (
            <div className="error-banner" onClick={() => setError(null)}>
              {error} <span className="error-dismiss">(dismiss)</span>
            </div>
          )}
          <SettingsPage
            theme={theme}
            onThemeChange={setTheme}
            onError={setError}
          />
        </main>
      ) : (
        <main className="main">
          <div className="toolbar">
            <input
              ref={searchRef}
              type="search"
              className="search-input"
              placeholder="Search titles, URLs, notes, tags, page text…  ( / )"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
            {filtersActive && (
              <div className="add-wrap" onClick={(e) => e.stopPropagation()}>
                <button
                  className="icon-btn"
                  title="Save this search to the sidebar"
                  onClick={() => setSaveSearchOpen((v) => !v)}
                >
                  <SearchIcon size={15} />
                </button>
                {saveSearchOpen && (
                  <form className="add-popover" onSubmit={handleSaveSearch}>
                    <input
                      autoFocus
                      className="add-input"
                      placeholder="Name this search…"
                      value={saveSearchName}
                      onChange={(e) => setSaveSearchName(e.target.value)}
                    />
                    <button
                      className="btn btn-primary"
                      disabled={!saveSearchName.trim()}
                    >
                      Save
                    </button>
                  </form>
                )}
              </div>
            )}
            <div className="view-toggle">
              <button
                className={`icon-btn ${viewMode === "list" ? "active" : ""}`}
                title="List view"
                onClick={() => setViewMode("list")}
              >
                <ListIcon size={16} />
              </button>
              <button
                className={`icon-btn ${viewMode === "cards" ? "active" : ""}`}
                title="Card view"
                onClick={() => setViewMode("cards")}
              >
                <GridIcon size={16} />
              </button>
            </div>
            <button
              className="btn"
              title="Import bookmarks from a browser, Raindrop or Pocket"
              onClick={() => setImportOpen(true)}
            >
              <ImportIcon size={15} /> Import
            </button>
            <div className="add-wrap" onClick={(e) => e.stopPropagation()}>
              {pasteReady ? (
                <div className="split-btn">
                  <button
                    className="btn btn-primary split-main"
                    title={`Save ${clipboardUrl}`}
                    disabled={adding}
                    onClick={handlePasteAndAdd}
                  >
                    <ClipboardIcon size={15} />
                    <span className="split-main-text">
                      Paste &amp; Add
                      <span className="split-host">
                        {hostOf(clipboardUrl!)}
                      </span>
                    </span>
                  </button>
                  <button
                    className="btn btn-primary split-toggle"
                    title="Add a different URL"
                    onClick={() => setAddOpen((v) => !v)}
                  >
                    <ChevronUpIcon size={15} />
                  </button>
                </div>
              ) : (
                <button
                  className="btn btn-primary"
                  title="Save a URL"
                  onClick={() => setAddOpen((v) => !v)}
                >
                  <PlusIcon size={15} /> Add
                </button>
              )}
              {addOpen && (
                <form className="add-popover" onSubmit={handleAdd}>
                  <input
                    autoFocus
                    className="add-input"
                    placeholder="Paste a URL to save…"
                    value={addUrl}
                    onChange={(e) => setAddUrl(e.target.value)}
                  />
                  <button
                    className="btn btn-primary"
                    disabled={adding || !addUrl.trim()}
                  >
                    {adding ? "Saving…" : "Save"}
                  </button>
                </form>
              )}
            </div>
          </div>

          {error && (
            <div className="error-banner" onClick={() => setError(null)}>
              {error} <span className="error-dismiss">(dismiss)</span>
            </div>
          )}

          {activeTag && (
            <div className="filter-banner">
              Filtering by <strong>#{activeTag}</strong>
              <button
                className="btn btn-subtle"
                onClick={() => setActiveTag(null)}
              >
                clear
              </button>
            </div>
          )}

          {selectedIds.size > 0 && (
            <div className="bulk-bar">
              <span className="bulk-count">{selectedIds.size} selected</span>
              <button
                className="btn btn-subtle"
                onClick={() => runBulk(() => api.bulkSetFavorite(bulkIds, true))}
              >
                Star
              </button>
              <button
                className="btn btn-subtle"
                onClick={() =>
                  runBulk(() => api.bulkSetFavorite(bulkIds, false))
                }
              >
                Unstar
              </button>
              <button
                className="btn btn-subtle"
                onClick={() => runBulk(() => api.bulkSetRead(bulkIds, true))}
              >
                Mark read
              </button>
              <button
                className="btn btn-subtle"
                onClick={() => runBulk(() => api.bulkSetRead(bulkIds, false))}
              >
                Mark unread
              </button>
              <div className="add-wrap" onClick={(e) => e.stopPropagation()}>
                <button
                  className="btn btn-subtle"
                  onClick={() => setBulkTagOpen((v) => !v)}
                >
                  Add tag…
                </button>
                {bulkTagOpen && (
                  <form
                    className="add-popover bulk-tag-popover"
                    onSubmit={async (e) => {
                      e.preventDefault();
                      if (!bulkTag.trim()) return;
                      await runBulk(() =>
                        api.bulkAddTag(bulkIds, bulkTag.trim()),
                      );
                      setBulkTag("");
                      setBulkTagOpen(false);
                    }}
                  >
                    <input
                      autoFocus
                      className="add-input"
                      placeholder="Tag name…"
                      value={bulkTag}
                      onChange={(e) => setBulkTag(e.target.value)}
                    />
                    <button
                      className="btn btn-primary"
                      disabled={!bulkTag.trim()}
                    >
                      Tag
                    </button>
                  </form>
                )}
              </div>
              <button
                className="btn btn-subtle bulk-danger"
                onClick={bulkDeleteSelected}
              >
                Delete
              </button>
              <span className="bulk-spacer" />
              <button
                className="btn btn-subtle"
                onClick={() => setSelectedIds(new Set())}
              >
                Clear (Esc)
              </button>
            </div>
          )}

          <div className={viewMode === "cards" ? "save-grid" : "save-list"}>
            {saves.map((save, i) => (
              <SaveCard
                key={save.id}
                save={save}
                selected={selected?.id === save.id}
                focused={focusedIdx === i}
                bulkSelected={selectedIds.has(save.id)}
                variant={viewMode === "cards" ? "card" : "list"}
                vaultPath={vaultDir}
                onCardClick={handleCardClick}
                onOpen={handleOpen}
                onEdit={setSelected}
                onDelete={handleDelete}
                onToggleFavorite={handleToggleFavorite}
                onPickTag={(t) => setActiveTag(t)}
              />
            ))}
            {saves.length === 0 && (
              <div className="empty-state">
                {view === "inbox"
                  ? "Inbox zero — everything's read. 🎉"
                  : filtersActive
                    ? "Nothing matches the current filters."
                    : "No saves yet. Hit Add or save a page from the browser extension."}
              </div>
            )}
          </div>
        </main>
      )}

      {importOpen && (
        <ImportDialog
          onClose={() => {
            setImportOpen(false);
            refresh();
          }}
        />
      )}

      {paletteOpen && (
        <CommandPalette
          actions={paletteActions}
          onClose={() => setPaletteOpen(false)}
          onOpenSave={handleOpen}
        />
      )}

      {showPanel && selected && (
        <EditPanel
          save={selected}
          onClose={() => setSelected(null)}
          onChanged={applyChange}
          onDeleted={(id) => {
            setSaves((prev) => prev.filter((s) => s.id !== id));
            setSelected(null);
            refresh();
          }}
          onOpen={handleOpen}
          onError={setError}
        />
      )}
      </div>
    </div>
  );
}

export default App;
