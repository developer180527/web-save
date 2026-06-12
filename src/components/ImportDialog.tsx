import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import * as api from "../api";
import type { ImportPreview, ImportReport } from "../types";
import { XIcon } from "./Icons";

interface Props {
  onClose: () => void;
}

type Stage =
  | { kind: "pick" }
  | { kind: "preview"; path: string; preview: ImportPreview }
  | { kind: "importing" }
  | { kind: "done"; report: ImportReport };

export default function ImportDialog({ onClose }: Props) {
  const [stage, setStage] = useState<Stage>({ kind: "pick" });
  const [error, setError] = useState<string | null>(null);

  async function pickFile() {
    setError(null);
    try {
      const path = await open({
        multiple: false,
        filters: [
          {
            name: "Bookmarks",
            extensions: ["html", "htm", "csv", "txt", "md"],
          },
        ],
      });
      if (typeof path !== "string") return; // cancelled
      const preview = await api.previewImport(path);
      setStage({ kind: "preview", path, preview });
    } catch (e) {
      setError(String(e));
    }
  }

  async function confirm(path: string) {
    setStage({ kind: "importing" });
    setError(null);
    try {
      setStage({ kind: "done", report: await api.runImport(path) });
    } catch (e) {
      setError(String(e));
      setStage({ kind: "pick" });
    }
  }

  const stat = (value: number, label: string, accent?: string) => (
    <div className="import-stat">
      <span className="import-stat-value" style={accent ? { color: accent } : undefined}>
        {value}
      </span>
      <span className="import-stat-label">{label}</span>
    </div>
  );

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2>Import bookmarks</h2>
          <button className="icon-btn" onClick={onClose} title="Close">
            <XIcon size={15} />
          </button>
        </div>

        {error && <div className="error-banner">{error}</div>}

        {stage.kind === "pick" && (
          <>
            <p className="settings-text">
              Supported: browser bookmark exports (Chrome, Firefox, Safari,
              Edge — HTML), Raindrop.io CSV, Pocket exports, or a plain list
              of URLs. Nothing is overwritten: existing saves only gain
              missing details and tags.
            </p>
            <button className="btn btn-primary" onClick={pickFile}>
              Choose file…
            </button>
          </>
        )}

        {stage.kind === "preview" && (
          <>
            <p className="settings-text">
              Detected <strong>{stage.preview.format}</strong>:
            </p>
            <div className="import-stats">
              {stat(stage.preview.new, "new", "var(--green)")}
              {stat(stage.preview.existing, "already saved")}
              {stat(
                stage.preview.invalid,
                "skipped",
                stage.preview.invalid > 0 ? "var(--amber)" : undefined,
              )}
            </div>
            <div className="modal-actions">
              <button className="btn" onClick={() => setStage({ kind: "pick" })}>
                Back
              </button>
              <button
                className="btn btn-primary"
                disabled={stage.preview.new + stage.preview.existing === 0}
                onClick={() => confirm(stage.path)}
              >
                Import {stage.preview.new + stage.preview.existing} bookmarks
              </button>
            </div>
          </>
        )}

        {stage.kind === "importing" && (
          <p className="settings-text">Importing…</p>
        )}

        {stage.kind === "done" && (
          <>
            <p className="settings-text">
              Done — {stage.report.new} added, {stage.report.existing} merged
              into existing saves
              {stage.report.invalid > 0 &&
                `, ${stage.report.invalid} skipped (no valid URL)`}
              . Imported links start as “unchecked”; the background monitor
              will verify them over the coming hours.
            </p>
            <div className="modal-actions">
              <button className="btn btn-primary" onClick={onClose}>
                Close
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
