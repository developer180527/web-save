// WebSave extension popup — a compact mirror of the macOS menubar panel:
// shows whether the desktop app is reachable, lets you save the current page,
// and fuzzy-searches your saved links so you can jump straight to one.
//
// Reads come from the app's loopback server (same custom-header gate as
// writes), so the vault never leaves the machine.

// Cross-browser: prefer the promise-based `browser` namespace (Firefox/Safari).
globalThis.chrome = globalThis.browser ?? globalThis.chrome;

const ENDPOINT = "http://127.0.0.1:38917";
const APP_DOWNLOAD_URL =
  "https://github.com/developer180527/web-save/releases";
const QUEUE_KEY = "pendingSaves";
const RESULT_LIMIT = 10;

const statusEl = document.getElementById("status");
const saveBtn = document.getElementById("save");
const queuedEl = document.getElementById("queued");
const downloadEl = document.getElementById("download");
const searchEl = document.getElementById("search");
const listEl = document.getElementById("list");

/** Everything we've loaded from the app, newest first. */
let allSaves = [];

// ---------------------------------------------------------------- fuzzy --

/**
 * Subsequence match with a light quality score: consecutive hits and matches
 * at word boundaries score higher, so "gh" ranks "GitHub" above "graph".
 * Returns null when the query isn't a subsequence of the text at all.
 */
function fuzzyScore(text, query) {
  const t = text.toLowerCase();
  const q = query.toLowerCase();
  if (!q) return 0;

  let score = 0;
  let ti = 0;
  let streak = 0;
  for (const ch of q) {
    const found = t.indexOf(ch, ti);
    if (found === -1) return null;
    // Word starts are strong signals; runs of adjacent chars are too.
    const atBoundary = found === 0 || /[\s\-_/.:]/.test(t[found - 1]);
    streak = found === ti ? streak + 1 : 0;
    score += 1 + streak * 2 + (atBoundary ? 3 : 0);
    ti = found + 1;
  }
  // Prefer tighter matches in shorter text.
  return score - t.length * 0.01;
}

function bestMatch(save, query) {
  const title = fuzzyScore(save.title || "", query);
  const url = fuzzyScore(save.url || "", query);
  if (title === null && url === null) return null;
  // A title hit is worth more than the same hit buried in a URL.
  return Math.max(title ?? -Infinity, (url ?? -Infinity) - 2);
}

function hostOf(url) {
  try {
    return new URL(url).host.replace(/^www\./, "");
  } catch {
    return url;
  }
}

// ----------------------------------------------------------------- view --

function render() {
  const query = searchEl.value.trim();
  let rows;
  if (query) {
    rows = allSaves
      .map((save) => ({ save, score: bestMatch(save, query) }))
      .filter((r) => r.score !== null)
      .sort((a, b) => b.score - a.score)
      .slice(0, RESULT_LIMIT)
      .map((r) => r.save);
  } else {
    rows = allSaves.slice(0, RESULT_LIMIT);
  }

  listEl.replaceChildren();
  if (rows.length === 0) {
    const empty = document.createElement("div");
    empty.className = "empty";
    empty.textContent = allSaves.length
      ? "No matches"
      : "Nothing saved yet";
    listEl.append(empty);
    return;
  }

  for (const save of rows) {
    const host = hostOf(save.url);
    const row = document.createElement("button");
    row.className = "row";
    row.title = save.url;
    row.addEventListener("click", () => {
      chrome.tabs.create({ url: save.url });
      window.close();
    });

    const icon = document.createElement("img");
    icon.className = "row-icon";
    icon.src =
      save.faviconUrl || `https://icons.duckduckgo.com/ip3/${host}.ico`;
    icon.addEventListener("error", () => {
      icon.style.visibility = "hidden";
    });

    const text = document.createElement("span");
    text.className = "row-text";
    const title = document.createElement("span");
    title.className = "row-title";
    title.textContent = save.title || host;
    const sub = document.createElement("span");
    sub.className = "row-host";
    sub.textContent = host;
    text.append(title, sub);

    row.append(icon, text);
    if (save.status === "dead") {
      const dot = document.createElement("span");
      dot.className = "row-dead";
      dot.title = "This link stopped responding";
      row.append(dot);
    }
    listEl.append(row);
  }
}

// ---------------------------------------------------------------- data --

async function loadSaves() {
  try {
    const resp = await fetch(`${ENDPOINT}/recent?limit=300`, {
      headers: { "x-websave-client": "extension" },
      signal: AbortSignal.timeout(2000),
    });
    if (resp.ok) allSaves = await resp.json();
  } catch {
    allSaves = [];
  }
  render();
}

async function refreshStatus() {
  let running = false;
  let version = null;
  try {
    const resp = await fetch(`${ENDPOINT}/ping`, {
      signal: AbortSignal.timeout(1500),
    });
    if (resp.ok) {
      running = true;
      version = (await resp.json()).version;
    }
  } catch {
    // app not running
  }

  if (running) {
    statusEl.className = "status connected";
    statusEl.textContent = `Connected${version ? ` · v${version}` : ""}`;
    downloadEl.hidden = true;
  } else {
    statusEl.className = "status disconnected";
    statusEl.textContent = "Desktop app isn't running";
    downloadEl.hidden = false;
  }

  const { [QUEUE_KEY]: queue = [] } = await chrome.storage.local.get(QUEUE_KEY);
  queuedEl.textContent = queue.length
    ? `${queue.length} save${queue.length > 1 ? "s" : ""} waiting to sync`
    : "";
  return running;
}

// -------------------------------------------------------------- events --

searchEl.addEventListener("input", render);

// Enter opens the top result — search then jump, no mouse needed.
searchEl.addEventListener("keydown", (e) => {
  if (e.key !== "Enter") return;
  const first = listEl.querySelector(".row");
  if (first) first.click();
});

saveBtn.addEventListener("click", async () => {
  saveBtn.disabled = true;
  saveBtn.textContent = "Saving…";
  let status = "invalid";
  try {
    status = (await chrome.runtime.sendMessage({ type: "capture-active-tab" }))
      ?.status;
  } catch {
    status = "invalid";
  }
  saveBtn.textContent =
    status === "saved"
      ? "Saved ✓"
      : status === "queued"
        ? "Queued — app offline"
        : "Can't save this page";
  setTimeout(async () => {
    saveBtn.disabled = false;
    saveBtn.textContent = "Save this page";
    await refreshStatus();
    loadSaves();
  }, 1200);
});

downloadEl.addEventListener("click", (e) => {
  e.preventDefault();
  chrome.tabs.create({ url: APP_DOWNLOAD_URL });
});

(async () => {
  if (await refreshStatus()) loadSaves();
  else render();
  searchEl.focus();
})();
