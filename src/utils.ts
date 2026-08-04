import type { LinkStatus } from "./types";

// Where the app points users to get the other half of the duo. Update these
// to the Chrome Web Store / Releases URLs once published.
export const EXTENSION_INSTALL_URL =
  "https://github.com/developer180527/web-save#browser-extension";
export const APP_DOWNLOAD_URL =
  "https://github.com/developer180527/web-save/releases";

export function hostOf(url: string): string {
  try {
    return new URL(url).host.replace(/^www\./, "");
  } catch {
    return url;
  }
}

export function relativeTime(epochSecs: number): string {
  const delta = Math.floor(Date.now() / 1000) - epochSecs;
  if (delta < 60) return "just now";
  if (delta < 3600) return `${Math.floor(delta / 60)}m ago`;
  if (delta < 86400) return `${Math.floor(delta / 3600)}h ago`;
  if (delta < 30 * 86400) return `${Math.floor(delta / 86400)}d ago`;
  return new Date(epochSecs * 1000).toLocaleDateString();
}

export const STATUS_LABELS: Record<LinkStatus, string> = {
  unchecked: "Unchecked",
  active: "Active",
  changed: "Changed",
  redirected: "Redirected",
  unreachable: "Unreachable",
  dead: "Dead",
};

/// Longer explanations, for tooltips and the detail panel.
export const STATUS_HINTS: Record<LinkStatus, string> = {
  unchecked: "Not checked yet.",
  active: "Reachable and unchanged.",
  changed: "Reachable, but the page content changed.",
  redirected: "Now resolves somewhere meaningfully different.",
  unreachable:
    "Couldn't be reached from this machine — it may be on a network you're not connected to (Tailscale, VPN, LAN), or the server may be down. Never deleted automatically.",
  dead: "A working server said this page is gone (404/410).",
};

/** First http(s) URL found in arbitrary text, or null. */
export function firstWebUrl(text: string): string | null {
  if (!text) return null;
  const match = text.trim().match(/https?:\/\/[^\s<>"']+/i);
  return match ? match[0] : null;
}

/** How long a dead link is kept before permanent deletion. */
export const TRASH_GRACE_DAYS = 30;

/**
 * Days left before a dead link is permanently deleted. Returns null when the
 * link isn't dead; 0 means it's due on the next sweep.
 */
export function daysUntilPurge(deadSince: number | null): number | null {
  if (deadSince == null) return null;
  const elapsedDays = (Date.now() / 1000 - deadSince) / 86400;
  return Math.max(0, Math.ceil(TRASH_GRACE_DAYS - elapsedDays));
}

export function parseTagsInput(input: string): string[] {
  return input
    .split(",")
    .map((t) => t.trim())
    .filter(Boolean);
}
