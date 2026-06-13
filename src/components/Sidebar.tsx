import type { LinkStatus, SavedSearch, TagCount, VaultStats } from "../types";
import { STATUS_LABELS } from "../utils";
import {
  BookmarkIcon,
  CheckCircleIcon,
  GearIcon,
  HashIcon,
  HelpCircleIcon,
  InboxIcon,
  RedirectIcon,
  RefreshIcon,
  SearchIcon,
  StarIcon,
  XCircleIcon,
  XIcon,
} from "./Icons";

export type View = "all" | "inbox" | "favorites" | LinkStatus;

interface Props {
  view: View;
  onViewChange: (view: View) => void;
  tags: TagCount[];
  activeTag: string | null;
  onTagChange: (tag: string | null) => void;
  stats: VaultStats | null;
  savedSearches: SavedSearch[];
  onApplySavedSearch: (search: SavedSearch) => void;
  onDeleteSavedSearch: (id: number) => void;
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
  savedSearches,
  onApplySavedSearch,
  onDeleteSavedSearch,
  onRecheckAll,
  rechecking,
  onOpenSettings,
  settingsOpen,
}: Props) {
  const VIEW_ICONS: Record<View, React.ReactNode> = {
    all: <BookmarkIcon size={15} />,
    inbox: <InboxIcon size={15} />,
    favorites: <StarIcon size={15} />,
    active: <CheckCircleIcon size={15} />,
    changed: <RefreshIcon size={15} />,
    redirected: <RedirectIcon size={15} />,
    dead: <XCircleIcon size={15} />,
    unchecked: <HelpCircleIcon size={15} />,
  };

  const item = (v: View, label: string, count?: number) => (
    <button
      key={v}
      // A view is highlighted only when it's the active selection — i.e. no
      // tag is currently chosen (tags and views are mutually exclusive).
      className={`nav-item ${view === v && !activeTag ? "active" : ""}`}
      onClick={() => onViewChange(v)}
    >
      <span className={`nav-icon nav-icon-${v}`}>{VIEW_ICONS[v]}</span>
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
        {item("inbox", "Inbox", stats?.unread)}
        {item("favorites", "Favorites", stats?.favorites)}
      </nav>

      {savedSearches.length > 0 && (
        <>
          <div className="sidebar-heading">Saved searches</div>
          <nav className="sidebar-section">
            {savedSearches.map((s) => (
              <button
                key={s.id}
                className="nav-item saved-search-item"
                onClick={() => onApplySavedSearch(s)}
              >
                <SearchIcon size={12} />
                <span className="nav-label">{s.name}</span>
                <span
                  role="button"
                  className="saved-search-delete"
                  title="Delete saved search"
                  onClick={(e) => {
                    e.stopPropagation();
                    onDeleteSavedSearch(s.id);
                  }}
                >
                  <XIcon size={11} />
                </span>
              </button>
            ))}
          </nav>
        </>
      )}

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
            <span className="nav-icon">
              <HashIcon size={13} />
            </span>
            <span className="nav-label">{t.name}</span>
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
