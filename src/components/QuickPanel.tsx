import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { openUrl } from "@tauri-apps/plugin-opener";
import * as api from "../api";
import type { Save } from "../types";
import { Favicon } from "./SaveCard";
import { StarIcon } from "./Icons";
import { hostOf } from "../utils";

function Row({
  save,
  onOpen,
  onStar,
}: {
  save: Save;
  onOpen: (save: Save) => void;
  onStar: (save: Save) => void;
}) {
  return (
    <button className="quick-row" onClick={() => onOpen(save)} title={save.url}>
      <Favicon save={save} />
      <span className="quick-row-text">
        <span className="quick-row-title">{save.title || hostOf(save.url)}</span>
        <span className="quick-row-host">{hostOf(save.url)}</span>
      </span>
      <span
        role="button"
        className={`star-btn ${save.favorite ? "starred" : ""}`}
        title={save.favorite ? "Remove from favorites" : "Add to favorites"}
        onClick={(e) => {
          e.stopPropagation();
          onStar(save);
        }}
      >
        <StarIcon size={14} filled={save.favorite} />
      </span>
    </button>
  );
}

const IS_MAC = navigator.userAgent.includes("Mac");

export default function QuickPanel() {
  const [query, setQuery] = useState("");
  const [favorites, setFavorites] = useState<Save[]>([]);
  const [recent, setRecent] = useState<Save[]>([]);
  const [results, setResults] = useState<Save[]>([]);
  const inputRef = useRef<HTMLInputElement>(null);

  const searching = query.trim().length > 0;

  // On macOS the window itself is transparent; the panel draws its own
  // rounded card + arrow, so the page background must not paint.
  useEffect(() => {
    if (IS_MAC) {
      document.documentElement.classList.add("transparent-window");
    }
  }, []);

  const refresh = useCallback(async () => {
    try {
      if (query.trim()) {
        setResults(await api.listSaves({ query: query.trim(), limit: 20 }));
      } else {
        const [favs, rec] = await Promise.all([
          api.listSaves({ favoritesOnly: true, limit: 8 }),
          api.listSaves({ limit: 10 }),
        ]);
        setFavorites(favs);
        // Don't repeat favorites inside "Recent".
        const favIds = new Set(favs.map((s) => s.id));
        setRecent(rec.filter((s) => !favIds.has(s.id)));
      }
    } catch {
      // Quick panel stays silent on errors; the main app surfaces them.
    }
  }, [query]);

  useEffect(() => {
    const timer = setTimeout(refresh, 100);
    return () => clearTimeout(timer);
  }, [refresh]);

  const refreshRef = useRef(refresh);
  refreshRef.current = refresh;
  useEffect(() => {
    const unlistenSaves = listen("saves-updated", () => refreshRef.current());
    // Each time the panel pops up: fresh data, cleared search, focused input.
    const unlistenFocus = getCurrentWebviewWindow().onFocusChanged(
      ({ payload: focused }) => {
        if (focused) {
          setQuery("");
          refreshRef.current();
          inputRef.current?.focus();
        }
      },
    );
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") api.hideQuickWindow();
    };
    window.addEventListener("keydown", onKey);
    return () => {
      unlistenSaves.then((fn) => fn());
      unlistenFocus.then((fn) => fn());
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  async function handleOpen(save: Save) {
    try {
      await openUrl(save.url);
    } finally {
      api.hideQuickWindow();
    }
  }

  async function handleStar(save: Save) {
    try {
      await api.setFavorite(save.id, !save.favorite);
      refresh();
    } catch {
      // ignore — see main app
    }
  }

  const section = (label: string, items: Save[]) =>
    items.length > 0 && (
      <>
        <div className="quick-heading">{label}</div>
        {items.map((s) => (
          <Row key={s.id} save={s} onOpen={handleOpen} onStar={handleStar} />
        ))}
      </>
    );

  return (
    <div className={`quick-shell ${IS_MAC ? "mac" : ""}`}>
      {IS_MAC && <div className="quick-arrow" />}
      <div className="quick-panel">
        <input
          ref={inputRef}
          className="quick-search"
          placeholder="Search saves…"
          value={query}
          autoFocus
          onChange={(e) => setQuery(e.target.value)}
        />
        <div className="quick-list">
          {searching ? (
            results.length > 0 ? (
              section("Results", results)
            ) : (
              <div className="quick-empty">No matches</div>
            )
          ) : (
            <>
              {section("Favorites", favorites)}
              {section("Recent", recent)}
              {favorites.length === 0 && recent.length === 0 && (
                <div className="quick-empty">Nothing saved yet</div>
              )}
            </>
          )}
        </div>
        <button
          className="quick-footer"
          onClick={() => {
            api.showMainWindow();
            api.hideQuickWindow();
          }}
        >
          Open WebSave
        </button>
      </div>
    </div>
  );
}
