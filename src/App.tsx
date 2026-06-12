import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import * as api from "./api";
import type { ListQuery, Save, TagCount, VaultStats } from "./types";
import Sidebar, { type View } from "./components/Sidebar";
import SaveCard from "./components/SaveCard";
import EditPanel from "./components/EditPanel";
import SettingsPage, {
  MENUBAR_AUTOLAUNCH_KEY,
  type Theme,
} from "./components/SettingsPage";
import ImportDialog from "./components/ImportDialog";
import { GridIcon, ImportIcon, ListIcon, PlusIcon } from "./components/Icons";
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
  const [rechecking, setRechecking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [screen, setScreen] = useState<"library" | "settings">("library");
  const [importOpen, setImportOpen] = useState(false);
  const [addOpen, setAddOpen] = useState(false);
  const [vaultDir, setVaultDir] = useState("");
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
    if (!addOpen) return;
    const onClick = () => setAddOpen(false);
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setAddOpen(false);
    };
    window.addEventListener("click", onClick);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("click", onClick);
      window.removeEventListener("keydown", onKey);
    };
  }, [addOpen]);

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

  const refresh = useCallback(async () => {
    const query: ListQuery = {
      query: search.trim() || null,
      tag: activeTag,
      favoritesOnly: view === "favorites",
      status: view !== "all" && view !== "favorites" ? view : null,
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

  // Bring the macOS menubar companion up alongside the engine, if enabled.
  useEffect(() => {
    if (
      navigator.userAgent.includes("Mac") &&
      localStorage.getItem(MENUBAR_AUTOLAUNCH_KEY) === "true"
    ) {
      api.launchMenubarApp().catch(() => {
        // Not installed/built yet — settings has the button and the hint.
      });
    }
  }, []);

  // Background monitor (and future capture clients) signal through this event.
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

  async function handleAdd(e: React.FormEvent) {
    e.preventDefault();
    const url = addUrl.trim();
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

  async function handleToggleFavorite(save: Save) {
    try {
      applyChange(await api.setFavorite(save.id, !save.favorite));
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleOpen(save: Save) {
    try {
      await openUrl(save.url);
    } catch (e) {
      setError(String(e));
    }
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
      // Results stream in via "saves-updated"; release the button right away.
      setRechecking(false);
    }
  }

  const showPanel = screen === "library" && selected !== null;
  const gridColumns = `${sidebarWidth}px 5px 1fr${showPanel ? " 360px" : ""}`;

  return (
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
              type="search"
              className="search-input"
              placeholder="Search titles, URLs, notes, tags…"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
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
              <button
                className="btn btn-primary"
                title="Save a URL"
                onClick={() => setAddOpen((v) => !v)}
              >
                <PlusIcon size={15} /> Add
              </button>
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

          <div className={viewMode === "cards" ? "save-grid" : "save-list"}>
            {saves.map((save) => (
              <SaveCard
                key={save.id}
                save={save}
                selected={selected?.id === save.id}
                variant={viewMode === "cards" ? "card" : "list"}
                vaultPath={vaultDir}
                onOpen={handleOpen}
                onEdit={setSelected}
                onDelete={handleDelete}
                onToggleFavorite={handleToggleFavorite}
                onPickTag={(t) => setActiveTag(t)}
              />
            ))}
            {saves.length === 0 && (
              <div className="empty-state">
                {search.trim() || activeTag || view !== "all"
                  ? "Nothing matches the current filters."
                  : "No saves yet. Paste a URL above to get started — the browser extension lands in phase 2."}
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
  );
}

export default App;
