export type LinkStatus =
  | "unchecked"
  | "active"
  | "changed"
  | "redirected"
  | "dead";

export interface Save {
  id: number;
  url: string;
  title: string;
  description: string;
  notes: string;
  faviconUrl: string;
  thumbnail: string;
  favorite: boolean;
  isRead: boolean;
  status: LinkStatus;
  redirectUrl: string;
  httpStatus: number | null;
  tags: string[];
  createdAt: number;
  updatedAt: number;
  lastCheckedAt: number | null;
  archivedAt: number | null;
}

export interface NewSave {
  url: string;
  title?: string;
  description?: string;
  faviconUrl?: string;
  tags?: string[];
}

export interface SavePatch {
  title?: string;
  description?: string;
  notes?: string;
  faviconUrl?: string;
}

export interface ListQuery {
  query?: string | null;
  tag?: string | null;
  favoritesOnly?: boolean;
  unreadOnly?: boolean;
  status?: LinkStatus | null;
  limit?: number | null;
  offset?: number | null;
}

export interface SavedSearch {
  id: number;
  name: string;
  query: ListQuery;
  createdAt: number;
}

export interface TagCount {
  name: string;
  count: number;
}

export interface ImportReport {
  total: number;
  new: number;
  existing: number;
  invalid: number;
}

export interface ImportPreview extends ImportReport {
  format: string;
}

export interface ExtensionStatus {
  /** Unix seconds of the last extension capture, or null if never. */
  lastSeen: number | null;
  version: string | null;
}

export interface VaultStats {
  total: number;
  favorites: number;
  unread: number;
  unchecked: number;
  active: number;
  changed: number;
  redirected: number;
  dead: number;
}


