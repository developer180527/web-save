import { useEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { Save } from "../types";
import { hostOf, relativeTime, STATUS_LABELS } from "../utils";
import {
  CheckIcon,
  DotsIcon,
  ExternalIcon,
  PencilIcon,
  StarIcon,
  TrashIcon,
} from "./Icons";

interface Props {
  save: Save;
  selected: boolean;
  /** Keyboard focus (j/k navigation). */
  focused: boolean;
  /** Part of the current multi-selection. */
  bulkSelected: boolean;
  variant: "list" | "card";
  /** Vault root path, needed to resolve cached thumbnails. */
  vaultPath: string;
  /** Raw click — the app decides between open and (multi-)select. */
  onCardClick: (e: React.MouseEvent, save: Save) => void;
  onOpen: (save: Save) => void;
  onEdit: (save: Save) => void;
  onDelete: (save: Save) => void;
  onToggleFavorite: (save: Save) => void;
  onPickTag: (tag: string) => void;
}

/** Stable hue derived from the hostname, so every site gets its own color. */
function hostHue(host: string): number {
  let h = 0;
  for (let i = 0; i < host.length; i++) {
    h = (h * 31 + host.charCodeAt(i)) % 360;
  }
  return h;
}

function Thumbnail({ save, vaultPath }: { save: Save; vaultPath: string }) {
  const [failed, setFailed] = useState(false);
  if (save.thumbnail && vaultPath && !failed) {
    return (
      <img
        className="card-thumb"
        src={convertFileSrc(`${vaultPath}/assets/${save.thumbnail}`)}
        alt=""
        loading="lazy"
        onError={() => setFailed(true)}
      />
    );
  }
  const host = hostOf(save.url);
  const hue = hostHue(host);
  return (
    <div
      className="card-thumb card-thumb-fallback"
      style={{
        background: `linear-gradient(135deg,
          hsl(${hue} 42% 30%),
          hsl(${(hue + 45) % 360} 50% 18%))`,
      }}
    >
      <Favicon save={save} />
      <span className="card-thumb-host">{host}</span>
    </div>
  );
}

export function Favicon({ save }: { save: Save }) {
  const [failed, setFailed] = useState(false);
  const src =
    save.faviconUrl ||
    `https://icons.duckduckgo.com/ip3/${hostOf(save.url)}.ico`;
  if (failed) {
    const letter = (save.title || hostOf(save.url)).charAt(0).toUpperCase();
    return <div className="favicon favicon-fallback">{letter}</div>;
  }
  return (
    <img
      className="favicon"
      src={src}
      alt=""
      loading="lazy"
      onError={() => setFailed(true)}
    />
  );
}

export default function SaveCard({
  save,
  selected,
  focused,
  bulkSelected,
  variant,
  vaultPath,
  onCardClick,
  onOpen,
  onEdit,
  onDelete,
  onToggleFavorite,
  onPickTag,
}: Props) {
  const [menuOpen, setMenuOpen] = useState(false);
  const cardRef = useRef<HTMLElement>(null);

  useEffect(() => {
    if (focused) {
      cardRef.current?.scrollIntoView({ block: "nearest" });
    }
  }, [focused]);

  useEffect(() => {
    if (!menuOpen) return;
    const close = () => setMenuOpen(false);
    window.addEventListener("click", close);
    window.addEventListener("keydown", close);
    return () => {
      window.removeEventListener("click", close);
      window.removeEventListener("keydown", close);
    };
  }, [menuOpen]);

  return (
    <article
      ref={cardRef}
      className={`save-card ${variant === "card" ? "grid-card" : ""} ${selected ? "selected" : ""} ${focused ? "focused" : ""} ${bulkSelected ? "bulk-selected" : ""}`}
      title={save.url}
      onClick={(e) => onCardClick(e, save)}
    >
      {bulkSelected && (
        <span className="bulk-check">
          <CheckIcon size={12} />
        </span>
      )}
      {variant === "card" ? (
        <Thumbnail save={save} vaultPath={vaultPath} />
      ) : (
        <Favicon save={save} />
      )}
      <div className="save-body">
        <div className="save-title-row">
          {!save.isRead && <span className="unread-dot" title="Unread" />}
          <span className="save-title">{save.title || hostOf(save.url)}</span>
          {save.status !== "unchecked" && (
            <span
              className={`status-pill status-${save.status}`}
              title={
                save.httpStatus
                  ? `HTTP ${save.httpStatus}`
                  : STATUS_LABELS[save.status]
              }
            >
              {STATUS_LABELS[save.status]}
            </span>
          )}
        </div>
        <div className="save-meta">
          <span className="save-host">{hostOf(save.url)}</span>
          <span className="save-dot">·</span>
          <span>{relativeTime(save.createdAt)}</span>
        </div>
        {save.description && (
          <p className="save-description">{save.description}</p>
        )}
        {save.tags.length > 0 && (
          <div className="save-tags">
            {save.tags.map((t) => (
              <button
                key={t}
                className="tag-chip"
                onClick={(e) => {
                  e.stopPropagation();
                  onPickTag(t);
                }}
              >
                #{t}
              </button>
            ))}
          </div>
        )}
      </div>

      <div className="save-actions" onClick={(e) => e.stopPropagation()}>
        <button
          className={`star-btn ${save.favorite ? "starred" : ""}`}
          title={save.favorite ? "Remove from favorites" : "Add to favorites"}
          onClick={() => onToggleFavorite(save)}
        >
          <StarIcon size={16} filled={save.favorite} />
        </button>
        <div className="menu-wrap">
          <button
            className={`menu-btn ${menuOpen ? "open" : ""}`}
            title="More options"
            onClick={() => setMenuOpen((v) => !v)}
          >
            <DotsIcon size={16} />
          </button>
          {menuOpen && (
            <div className="menu">
              <button className="menu-item" onClick={() => onEdit(save)}>
                <PencilIcon size={14} /> Edit details
              </button>
              <button className="menu-item" onClick={() => onOpen(save)}>
                <ExternalIcon size={14} /> Open in browser
              </button>
              <button
                className="menu-item menu-item-danger"
                onClick={() => onDelete(save)}
              >
                <TrashIcon size={14} /> Delete
              </button>
            </div>
          )}
        </div>
      </div>
    </article>
  );
}
