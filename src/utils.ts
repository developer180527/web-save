import type { LinkStatus } from "./types";

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
  dead: "Dead",
};

/** First http(s) URL found in arbitrary text, or null. */
export function firstWebUrl(text: string): string | null {
  if (!text) return null;
  const match = text.trim().match(/https?:\/\/[^\s<>"']+/i);
  return match ? match[0] : null;
}

export function parseTagsInput(input: string): string[] {
  return input
    .split(",")
    .map((t) => t.trim())
    .filter(Boolean);
}
