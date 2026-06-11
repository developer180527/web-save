import { useEffect, useState } from "react";
import * as api from "../api";

export type Theme = "system" | "light" | "dark";

interface Props {
  theme: Theme;
  onThemeChange: (theme: Theme) => void;
  onError: (message: string) => void;
}

const THEMES: { value: Theme; label: string; hint: string }[] = [
  { value: "system", label: "System", hint: "Follow macOS appearance" },
  { value: "light", label: "Light", hint: "Always light" },
  { value: "dark", label: "Dark", hint: "Always dark" },
];

export default function SettingsPage({ theme, onThemeChange, onError }: Props) {
  const [vaultPath, setVaultPath] = useState("");
  const [logsPath, setLogsPath] = useState("");
  const [endpoint, setEndpoint] = useState("");

  useEffect(() => {
    api.vaultPath().then(setVaultPath).catch((e) => onError(String(e)));
    api.logsPath().then(setLogsPath).catch((e) => onError(String(e)));
    api.captureEndpoint().then(setEndpoint).catch((e) => onError(String(e)));
  }, [onError]);

  return (
    <div className="settings">
      <h1>Settings</h1>

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
        <p className="settings-text">
          The Chrome extension captures pages into this app while it is
          running (saves queue up in the browser otherwise). Install it from
          the project's <code>extension/</code> folder via{" "}
          <code>chrome://extensions</code> → Developer mode → Load unpacked.
          It talks to the app at:
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
