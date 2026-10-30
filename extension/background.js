// WebSave capture extension — background service worker.
//
// The extension is a thin capture client: it gathers page info and POSTs it
// to the desktop app's localhost endpoint. If the app isn't running, saves
// are queued in chrome.storage.local and flushed by an alarm.

// Cross-browser: Firefox/Safari expose a promise-based `browser` namespace.
// Alias it onto `chrome` so the `await chrome.*` calls below work everywhere
// (no-op on Chrome/Edge, where only `chrome` exists and is already promised).
globalThis.chrome = globalThis.browser ?? globalThis.chrome;

const ENDPOINT = "http://127.0.0.1:38917";
const ORIGIN = `${ENDPOINT}/*`;
const QUEUE_KEY = "pendingSaves";
// Why the last attempt to reach the app failed, for the popup to explain:
// "permission" | "offline" | "rejected". Cleared on the next success.
const ERROR_KEY = "lastSyncError";
const QUEUE_LIMIT = 200;
const FLUSH_ALARM = "flush-queue";

// The retry timer has to exist every time this script starts, not only on
// install: Firefox drops alarms when the browser restarts, which left the
// queue with nothing to flush it until the next save.
chrome.alarms.get(FLUSH_ALARM).then((alarm) => {
  if (!alarm) chrome.alarms.create(FLUSH_ALARM, { periodInMinutes: 1 });
});

/**
 * Ask for the one host permission the extension needs, from a click.
 *
 * Talking to the app means a request to 127.0.0.1 carrying a custom header.
 * From an origin without host permission that is a CORS request, and the app
 * deliberately refuses those (it is what keeps web pages out of the vault).
 * Chrome grants `host_permissions` at install; Firefox may not, and then
 * every save failed and queued forever. A browser only shows the prompt in
 * response to a user action, and Firefox only before the handler has awaited
 * anything, so callers make this their first call. Once granted it resolves
 * at once, with no prompt.
 */
function requestAccess() {
  return chrome.permissions.request({ origins: [ORIGIN] }).catch(() => false);
}

chrome.runtime.onInstalled.addListener(() => {
  chrome.contextMenus.create({
    id: "save-page",
    title: "Save page to WebSave",
    contexts: ["page"],
  });
  chrome.contextMenus.create({
    id: "save-link",
    title: "Save link to WebSave",
    contexts: ["link"],
  });
  chrome.contextMenus.create({
    id: "save-selection",
    title: "Save page to WebSave (selection as description)",
    contexts: ["selection"],
  });
});

chrome.contextMenus.onClicked.addListener(async (info, tab) => {
  await requestAccess();
  if (info.menuItemId === "save-link") {
    await capture({ url: info.linkUrl ?? "" });
  } else if (info.menuItemId === "save-selection") {
    await capture(await pagePayload(tab, info.selectionText));
  } else {
    await capture(await pagePayload(tab));
  }
});

// Toolbar button = save current page.
chrome.action.onClicked.addListener(async (tab) => {
  await requestAccess();
  await capture(await pagePayload(tab));
});

chrome.alarms.onAlarm.addListener((alarm) => {
  if (alarm.name === FLUSH_ALARM) flushQueue();
});
chrome.runtime.onStartup.addListener(() => {
  flushQueue();
});

/** Build a NewSave payload from the current tab, scraping meta when allowed. */
async function pagePayload(tab, selection) {
  const payload = {
    url: tab?.url ?? "",
    title: tab?.title ?? "",
    faviconUrl: tab?.favIconUrl ?? "",
  };
  // Pages that declare a cover image (og:image) get it fetched by the app;
  // for the rest we screenshot the visible tab so every card has an image.
  let hasCover = false;
  // Best effort: not available on chrome://, web store, PDFs, etc.
  try {
    const [result] = await chrome.scripting.executeScript({
      target: { tabId: tab.id },
      func: scrapeMeta,
    });
    if (result?.result) {
      payload.description = result.result.description;
      payload.faviconUrl = payload.faviconUrl || result.result.favicon;
      hasCover = result.result.hasCover;
    }
  } catch {
    // ignore — payload still has url/title
  }
  if (!hasCover) {
    try {
      payload.screenshot = await chrome.tabs.captureVisibleTab(tab.windowId, {
        format: "jpeg",
        quality: 75,
      });
    } catch {
      // ignore — capture can fail on protected pages
    }
  }
  if (selection) payload.description = selection;
  return payload;
}

/** Runs inside the page. Keep self-contained: no outer-scope references. */
function scrapeMeta() {
  const meta = document.querySelector(
    'meta[name="description"], meta[property="og:description"]',
  );
  const icon = document.querySelector('link[rel~="icon"]');
  let favicon = `${location.origin}/favicon.ico`;
  const href = icon?.getAttribute("href");
  if (href) {
    try {
      favicon = new URL(href, location.href).href;
    } catch {
      // keep default
    }
  }
  return {
    description: meta?.getAttribute("content")?.trim() ?? "",
    favicon,
    hasCover: !!document.querySelector(
      'meta[property="og:image"], meta[name="og:image"], ' +
        'meta[name="twitter:image"], meta[property="twitter:image"], ' +
        'meta[name="twitter:image:src"]',
    ),
  };
}

// Returns "saved" | "queued" | "invalid" so the popup can report the result.
async function capture(payload) {
  if (!/^https?:\/\//i.test(payload.url ?? "")) {
    await flashBadge("✗", "#d4373e");
    return "invalid";
  }
  try {
    await post(payload);
    await chrome.storage.local.remove(ERROR_KEY);
    await flashBadge("✓", "#2e9e5b");
    // The app is reachable now, so anything queued earlier can go too,
    // rather than waiting up to a minute for the alarm.
    flushQueue();
    return "saved";
  } catch (error) {
    await enqueue(payload);
    const reason = await diagnose(error);
    await chrome.storage.local.set({ [ERROR_KEY]: reason });
    await flashBadge("○", "#c98a1b");
    chrome.notifications.create({
      type: "basic",
      iconUrl: "icons/128.png",
      ...NOTICES[reason],
    });
    return "queued";
  }
}

const NOTICES = {
  permission: {
    title: "WebSave needs permission",
    message:
      "Saved to the queue. Click the WebSave toolbar button and choose " +
      "Allow access so it can reach the app.",
  },
  offline: {
    title: "WebSave is not running",
    message: "Saved to the queue. It will sync once the app is open.",
  },
  rejected: {
    title: "WebSave refused the save",
    message:
      "Saved to the queue. The app answered with an error; it will retry.",
  },
};

/**
 * Why a send failed. "Not running" was the only answer before, which hid a
 * missing permission behind a message that sent people to check an app that
 * was already open.
 */
async function diagnose(error) {
  if (!(await chrome.permissions.contains({ origins: [ORIGIN] }))) {
    return "permission";
  }
  return error instanceof HttpError ? "rejected" : "offline";
}

class HttpError extends Error {}

async function post(payload) {
  const resp = await fetch(`${ENDPOINT}/save`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "x-websave-client": "extension",
      // Lets the app record which extension version is connected.
      "x-websave-ext-version": chrome.runtime.getManifest().version,
    },
    body: JSON.stringify(payload),
  });
  if (!resp.ok) {
    throw new HttpError(`HTTP ${resp.status}`);
  }
}

// The popup delegates the actual capture here (the background has the
// scraping + screenshot logic and the host permissions).
chrome.runtime.onMessage.addListener((msg, _sender, sendResponse) => {
  if (msg?.type === "flush-queue") {
    flushQueue().then(() => sendResponse({ ok: true }));
    return true;
  }
  if (msg?.type === "capture-active-tab") {
    chrome.tabs
      .query({ active: true, currentWindow: true })
      .then(async ([tab]) => {
        sendResponse({ status: await capture(await pagePayload(tab)) });
      });
    return true; // async sendResponse
  }
});

async function enqueue(payload) {
  const { [QUEUE_KEY]: queue = [] } = await chrome.storage.local.get(QUEUE_KEY);
  // Drop duplicates of the same URL already waiting, and shed screenshots —
  // base64 images would blow through the storage quota.
  const { screenshot: _dropped, ...slim } = payload;
  const next = queue.filter((p) => p.url !== slim.url);
  next.push(slim);
  await chrome.storage.local.set({ [QUEUE_KEY]: next.slice(-QUEUE_LIMIT) });
}

// Two flushes at once (a save's follow-up flush landing on the alarm's)
// would post the same queued pages twice.
let flushing = null;

function flushQueue() {
  flushing ??= flushOnce().finally(() => {
    flushing = null;
  });
  return flushing;
}

async function flushOnce() {
  const { [QUEUE_KEY]: queue = [] } = await chrome.storage.local.get(QUEUE_KEY);
  if (queue.length === 0) return;
  const remaining = [];
  let lastError = null;
  for (const payload of queue) {
    try {
      await post(payload);
    } catch (error) {
      lastError = error;
      remaining.push(payload);
    }
  }
  // Pages saved while this ran were appended to the stored queue; keep them.
  const { [QUEUE_KEY]: now = [] } = await chrome.storage.local.get(QUEUE_KEY);
  const sent = new Set(
    queue.filter((p) => !remaining.includes(p)).map((p) => p.url),
  );
  await chrome.storage.local.set({
    [QUEUE_KEY]: now.filter((p) => !sent.has(p.url)),
  });
  if (lastError) {
    await chrome.storage.local.set({ [ERROR_KEY]: await diagnose(lastError) });
  } else {
    await chrome.storage.local.remove(ERROR_KEY);
  }
  if (remaining.length < queue.length) {
    await flashBadge("✓", "#2e9e5b");
  }
}

async function flashBadge(text, color) {
  await chrome.action.setBadgeBackgroundColor({ color });
  await chrome.action.setBadgeText({ text });
  setTimeout(() => chrome.action.setBadgeText({ text: "" }), 1500);
}
