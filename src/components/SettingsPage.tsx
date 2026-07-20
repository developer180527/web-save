import { useEffect, useState } from "react";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import { relativeTime } from "../utils";
import * as api from "../api";

export type Theme = "system" | "light" | "dark";

interface Props {
  theme: Theme;
  onThemeChange: (theme: Theme) => void;
  onError: (message: string) => void;
}

const THEMES: { value: Theme; label: string; hint: string }[] = [
  { value: "system", label: "System", hint: "Follow system appearance" },
  { value: "light", label: "Light", hint: "Always light" },
  { value: "dark", label: "Dark", hint: "Always dark" },
];

const IS_MAC = navigator.userAgent.includes("Mac");

export const MENUBAR_AUTOLAUNCH_KEY = "launchMenubarOnStart";

export default function SettingsPage({ theme, onThemeChange, onError }: Props) {
  const [vaultPath, setVaultPath] = useState("");
  const [logsPath, setLogsPath] = useState("");
  const [endpoint, setEndpoint] = useState("");
  const [autostart, setAutostart] = useState(false);
  const [ext, setExt] = useState<import("../types").ExtensionStatus | null>(
    null,
  );
  const [version, setVersion] = useState("");
  const [autoPurge, setAutoPurge] = useState(true);
  const [exporting, setExporting] = useState(false);

  async function toggleAutoPurge() {
    const next = !autoPurge;
    setAutoPurge(next);
    try {
      await api.setPref("trash.auto_purge", String(next));
    } catch (e) {
      onError(String(e));
    }
  }

  async function handleExport(format: "json" | "html") {
    setExporting(true);
    try {
      const contents = await api.exportData(format);
      const stamp = new Date().toISOString().slice(0, 10);
      const path = await save({
        defaultPath: `websave-${stamp}.${format}`,
        filters: [
          {
            name: format === "json" ? "JSON backup" : "Bookmarks HTML",
            extensions: [format],
          },
        ],
      });
      if (path) await writeTextFile(path, contents);
    } catch (e) {
      onError(String(e));
    } finally {
      setExporting(false);
    }
  }
  const [menubarAutolaunch, setMenubarAutolaunch] = useState(
    () => localStorage.getItem(MENUBAR_AUTOLAUNCH_KEY) === "true",
  );

  function toggleMenubarAutolaunch() {
    const next = !menubarAutolaunch;
    setMenubarAutolaunch(next);
    localStorage.setItem(MENUBAR_AUTOLAUNCH_KEY, String(next));
    if (next) {
      api.launchMenubarApp().catch((e) => onError(String(e)));
    }
  }

  useEffect(() => {
    api.vaultPath().then(setVaultPath).catch((e) => onError(String(e)));
    api.logsPath().then(setLogsPath).catch((e) => onError(String(e)));
    api.captureEndpoint().then(setEndpoint).catch((e) => onError(String(e)));
    isEnabled().then(setAutostart).catch((e) => onError(String(e)));
    api.extensionStatus().then(setExt).catch(() => {});
    api.appVersion().then(setVersion).catch(() => {});
    api
      .getPref("trash.auto_purge")
      .then((v) => setAutoPurge(v !== "false"))
      .catch(() => {});
  }, [onError]);

  async function toggleAutostart() {
    try {
      if (autostart) {
        await disable();
      } else {
        await enable();
      }
      setAutostart(await isEnabled());
    } catch (e) {
      onError(String(e));
    }
  }

  return (
    <div className="settings">
      <div className="settings-header">
        <h1>Settings</h1>
        {version && <span className="settings-version">WebSave v{version}</span>}
      </div>

      <section className="settings-section">
        <h2>Appearance</h2>
        <div className="theme-options">
          {THEMES.map((t) => (
            <button
              key={t.value}
              className={`theme-option ${theme === t.value ? "active" : ""}`}
              onClick={() => onThemeChange(t.value)}
            >
              <span className={`theme-swatch theme-swatch-${t.value}`} />
              <span className="theme-label">{t.label}</span>
              <span className="theme-hint">{t.hint}</span>
            </button>
          ))}
        </div>
      </section>

      <section className="settings-section">
        <h2>General</h2>
        <label className="toggle-row">
          <input
            type="checkbox"
            checked={autostart}
            onChange={toggleAutostart}
          />
          Launch WebSave at login
        </label>
        <p className="settings-text">
          Recommended: closing the window keeps WebSave running in the
          menubar/tray, so the extension can always capture and link
          monitoring stays active. Quit from the tray icon.
        </p>
      </section>

      {IS_MAC && (
        <section className="settings-section">
          <h2>Menubar app</h2>
          <p className="settings-text">
            A native companion app shows your starred and recent saves from
            the menubar. It is a separate lightweight app (~12 MB) that reads
            the same vault.
          </p>
          <div className="settings-row">
            <button
              className="btn"
              onClick={() =>
                api.launchMenubarApp().catch((e) => onError(String(e)))
              }
            >
              Launch menubar app
            </button>
          </div>
          <label className="toggle-row">
            <input
              type="checkbox"
              checked={menubarAutolaunch}
              onChange={toggleMenubarAutolaunch}
            />
            Start it together with WebSave
          </label>
        </section>
      )}

      <section className="settings-section">
        <h2>Trash</h2>
        <label className="toggle-row">
          <input
            type="checkbox"
            checked={autoPurge}
            onChange={toggleAutoPurge}
          />
          Permanently delete links after 30 days in the trash
        </label>
        <p className="settings-text">
          Links that stop responding move to the trash but keep being
          health-checked — if a site comes back, it returns to your library
          automatically. Turn this off to keep dead links until you empty the
          trash yourself.
        </p>
      </section>

      <section className="settings-section">
        <h2>Export</h2>
        <p className="settings-text">
          Your whole library, on demand. <strong>JSON</strong> is a complete
          backup (notes, tags, status, timestamps). <strong>HTML</strong> is
          the standard bookmarks format — importable by any browser, and by
          WebSave itself.
        </p>
        <div className="settings-row">
          <button
            className="btn"
            disabled={exporting}
            onClick={() => handleExport("json")}
          >
            Export JSON…
          </button>
          <button
            className="btn"
            disabled={exporting}
            onClick={() => handleExport("html")}
          >
            Export HTML…
          </button>
        </div>
      </section>

      <section className="settings-section">
        <h2>Storage</h2>
        <p className="settings-text">
          Your vault is a portable folder — back it up or move it between
          machines and nothing breaks.
        </p>
        <div className="settings-row">
          <code className="settings-path selectable">{vaultPath}</code>
          <button
            className="btn"
            onClick={() => api.openVaultDir().catch((e) => onError(String(e)))}
          >
            Open folder
          </button>
        </div>
      </section>

      <section className="settings-section">
        <h2>Browser extension</h2>
        <div className="ext-status-row">
          <span
            className={`ext-status-dot ${
              ext?.lastSeen != null ? "connected" : ""
            }`}
          />
          {ext?.lastSeen != null ? (
            <span>
              Extension connected
              {ext.version ? ` (v${ext.version})` : ""} · last capture{" "}
              {relativeTime(ext.lastSeen)}
            </span>
          ) : (
            <span>
              Waiting for the browser extension — it'll connect on its first
              capture.
            </span>
          )}
        </div>
        <p className="settings-text">
          The extension captures pages into this app while it is running
          (saves queue in the browser otherwise). Install it from the
          project's <code>extension/</code> folder via{" "}
          <code>chrome://extensions</code> → Developer mode → Load unpacked.
          It talks to the app locally at:
        </p>
        <div className="settings-row">
          <code className="settings-path selectable">{endpoint}</code>
        </div>
      </section>

      <section className="settings-section">
        <h2>Logs</h2>
        <p className="settings-text">
          The backend writes everything it does (captures, edits, link checks)
          to a log file. Logs also stream to the devtools console while the
          app is running.
        </p>
        <div className="settings-row">
          <code className="settings-path selectable">{logsPath}</code>
          <button
            className="btn"
            onClick={() => api.openLogsDir().catch((e) => onError(String(e)))}
          >
            Open folder
          </button>
        </div>
      </section>
    </div>
  );
}
