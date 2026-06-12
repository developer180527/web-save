import { useEffect, useState } from "react";
import * as api from "../api";
import type { Save } from "../types";
import { Favicon } from "./SaveCard";
import { SearchIcon } from "./Icons";
import { hostOf } from "../utils";

export interface PaletteAction {
  id: string;
  label: string;
  run: () => void;
}

interface Props {
  actions: PaletteAction[];
  onClose: () => void;
  onOpenSave: (save: Save) => void;
}

type Item =
  | { kind: "save"; save: Save }
  | { kind: "action"; action: PaletteAction };

export default function CommandPalette({ actions, onClose, onOpenSave }: Props) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<Save[]>([]);
  const [idx, setIdx] = useState(0);

  const q = query.trim();

  useEffect(() => {
    if (!q) {
      setResults([]);
      return;
    }
    const timer = setTimeout(() => {
      api
        .listSaves({ query: q, limit: 6 })
        .then((r) => {
          setResults(r);
          setIdx(0);
        })
        .catch(() => {});
    }, 100);
    return () => clearTimeout(timer);
  }, [q]);

  const matchedActions = q
    ? actions.filter((a) => a.label.toLowerCase().includes(q.toLowerCase()))
    : actions;
  const items: Item[] = [
    ...results.map((save) => ({ kind: "save" as const, save })),
    ...matchedActions.map((action) => ({ kind: "action" as const, action })),
  ];
  const clampedIdx = Math.min(idx, Math.max(items.length - 1, 0));

  function execute(item: Item | undefined) {
    if (!item) return;
    onClose();
    if (item.kind === "save") {
      onOpenSave(item.save);
    } else {
      item.action.run();
    }
  }

  function handleKey(e: React.KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setIdx((i) => Math.min(i + 1, items.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setIdx((i) => Math.max(i - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      execute(items[clampedIdx]);
    } else if (e.key === "Escape") {
      onClose();
    }
  }

  return (
    <div className="modal-overlay palette-overlay" onClick={onClose}>
      <div className="palette" onClick={(e) => e.stopPropagation()}>
        <input
          autoFocus
          className="palette-input"
          placeholder="Search saves or type a command…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={handleKey}
        />
        <div className="palette-list">
          {items.map((item, i) => (
            <button
              key={item.kind === "save" ? `s${item.save.id}` : item.action.id}
              className={`palette-item ${i === clampedIdx ? "active" : ""}`}
              onMouseEnter={() => setIdx(i)}
              onClick={() => execute(item)}
            >
              {item.kind === "save" ? (
                <>
                  <Favicon save={item.save} />
                  <span className="palette-item-label">
                    {item.save.title || hostOf(item.save.url)}
                  </span>
                  <span className="palette-item-hint">
                    {hostOf(item.save.url)}
                  </span>
                </>
              ) : (
                <>
                  <span className="palette-action-icon">
                    <SearchIcon size={13} />
                  </span>
                  <span className="palette-item-label">
                    {item.action.label}
                  </span>
                </>
              )}
            </button>
          ))}
          {items.length === 0 && (
            <div className="palette-empty">Nothing matches</div>
          )}
        </div>
        <div className="palette-footer">
          ↑↓ navigate · Enter open · Esc close
        </div>
      </div>
    </div>
  );
}
