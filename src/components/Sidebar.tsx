import type { LinkStatus, TagCount, VaultStats } from "../types";
import { STATUS_LABELS } from "../utils";
import { GearIcon } from "./Icons";

export type View = "all" | "favorites" | LinkStatus;

interface Props {
  view: View;
  onViewChange: (view: View) => void;
  tags: TagCount[];
  activeTag: string | null;
  onTagChange: (tag: string | null) => void;
  stats: VaultStats | null;
  onRecheckAll: () => void;
  rechecking: boolean;
  onOpenSettings: () => void;
  settingsOpen: boolean;
}

const STATUS_VIEWS: LinkStatus[] = [
  "active",
  "changed",
  "redirected",
  "dead",
  "unchecked",
];

export default function Sidebar({
  view,
  onViewChange,
  tags,
  activeTag,
  onTagChange,
  stats,
  onRecheckAll,
  rechecking,
  onOpenSettings,
  settingsOpen,
}: Props) {
  const item = (v: View, label: string, count?: number) => (
    <button
      key={v}
      className={`nav-item ${view === v ? "active" : ""}`}
      onClick={() => onViewChange(v)}
    >
      <span className={`status-dot status-${v}`} />
      <span className="nav-label">{label}</span>
      {count !== undefined && count > 0 && (
        <span className="nav-count">{count}</span>
      )}
    </button>
  );

  return (
    <aside className="sidebar">
      <div className="sidebar-brand">WebSave</div>

      <nav className="sidebar-section">
        {item("all", "All saves", stats?.total)}
        {item("favorites", "Favorites", stats?.favorites)}
      </nav>

      <div className="sidebar-heading">Link health</div>
      <nav className="sidebar-section">
        {STATUS_VIEWS.map((s) => item(s, STATUS_LABELS[s], stats?.[s]))}
      </nav>

      <div className="sidebar-heading">Tags</div>
      <nav className="sidebar-section sidebar-tags">
        {tags.length === 0 && <div className="sidebar-empty">No tags yet</div>}
        {tags.map((t) => (
          <button
            key={t.name}
            className={`nav-item ${activeTag === t.name ? "active" : ""}`}
            onClick={() => onTagChange(activeTag === t.name ? null : t.name)}
          >
            <span className="nav-label">#{t.name}</span>
            <span className="nav-count">{t.count}</span>
          </button>
        ))}
      </nav>

      <div className="sidebar-footer">
        <button
          className="btn btn-subtle"
          onClick={onRecheckAll}
          disabled={rechecking}
        >
          {rechecking ? "Checking links…" : "Re-check all links"}
        </button>
        <button
          className={`icon-btn ${settingsOpen ? "active" : ""}`}
          title="Settings"
          onClick={onOpenSettings}
        >
          <GearIcon size={17} />
        </button>
      </div>
    </aside>
  );
}
