// WebSave extension popup: shows whether the desktop app is reachable,
// lets the user save the current page, and points to the app download when
// it isn't running. The app itself can never see the extension until a
// capture arrives — this popup is how the extension sees the app.

const ENDPOINT = "http://127.0.0.1:38917";
const APP_DOWNLOAD_URL =
  "https://github.com/developer180527/web-save/releases";
const QUEUE_KEY = "pendingSaves";

const statusEl = document.getElementById("status");
const saveBtn = document.getElementById("save");
const queuedEl = document.getElementById("queued");
const downloadEl = document.getElementById("download");

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
    // app not running — leave running = false
  }

  if (running) {
    statusEl.className = "status connected";
    statusEl.textContent = `Connected to the app${version ? ` · v${version}` : ""}`;
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
}

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
  setTimeout(() => {
    saveBtn.disabled = false;
    saveBtn.textContent = "Save this page";
    refreshStatus();
  }, 1300);
});

downloadEl.addEventListener("click", (e) => {
  e.preventDefault();
  chrome.tabs.create({ url: APP_DOWNLOAD_URL });
});

refreshStatus();
