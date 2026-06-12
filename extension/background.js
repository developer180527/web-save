// WebSave capture extension — background service worker.
//
// The extension is a thin capture client: it gathers page info and POSTs it
// to the desktop app's localhost endpoint. If the app isn't running, saves
// are queued in chrome.storage.local and flushed by an alarm.

const ENDPOINT = "http://127.0.0.1:38917";
const QUEUE_KEY = "pendingSaves";
const QUEUE_LIMIT = 200;

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
  chrome.alarms.create("flush-queue", { periodInMinutes: 1 });
});

chrome.contextMenus.onClicked.addListener(async (info, tab) => {
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
  await capture(await pagePayload(tab));
});

chrome.alarms.onAlarm.addListener((alarm) => {
  if (alarm.name === "flush-queue") flushQueue();
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

async function capture(payload) {
  if (!/^https?:\/\//i.test(payload.url ?? "")) {
    await flashBadge("✗", "#d4373e");
    return;
  }
  try {
    await post(payload);
    await flashBadge("✓", "#2e9e5b");
  } catch {
    await enqueue(payload);
    await flashBadge("○", "#c98a1b");
    chrome.notifications.create({
      type: "basic",
      iconUrl: "icons/128.png",
      title: "WebSave is not running",
      message: "Saved to the queue — it will sync once the app is open.",
    });
  }
}

async function post(payload) {
  const resp = await fetch(`${ENDPOINT}/save`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "x-websave-client": "extension",
    },
    body: JSON.stringify(payload),
  });
  if (!resp.ok) {
    throw new Error(`HTTP ${resp.status}`);
  }
}

async function enqueue(payload) {
  const { [QUEUE_KEY]: queue = [] } = await chrome.storage.local.get(QUEUE_KEY);
  // Drop duplicates of the same URL already waiting, and shed screenshots —
  // base64 images would blow through the storage quota.
  const { screenshot: _dropped, ...slim } = payload;
  const next = queue.filter((p) => p.url !== slim.url);
  next.push(slim);
  await chrome.storage.local.set({ [QUEUE_KEY]: next.slice(-QUEUE_LIMIT) });
}

async function flushQueue() {
  const { [QUEUE_KEY]: queue = [] } = await chrome.storage.local.get(QUEUE_KEY);
  if (queue.length === 0) return;
  const remaining = [];
  for (const payload of queue) {
    try {
      await post(payload);
    } catch {
      remaining.push(payload);
    }
  }
  await chrome.storage.local.set({ [QUEUE_KEY]: remaining });
  if (remaining.length < queue.length) {
    await flashBadge("✓", "#2e9e5b");
  }
}

async function flashBadge(text, color) {
  await chrome.action.setBadgeBackgroundColor({ color });
  await chrome.action.setBadgeText({ text });
  setTimeout(() => chrome.action.setBadgeText({ text: "" }), 1500);
}
