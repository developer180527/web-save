import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import * as api from "../api";
import type { Save } from "../types";
import { parseTagsInput, relativeTime, STATUS_LABELS } from "../utils";
import { XIcon } from "./Icons";

interface Props {
  save: Save;
  onClose: () => void;
  onChanged: (save: Save) => void;
  onDeleted: (id: number) => void;
  onOpen: (save: Save) => void;
  onError: (message: string) => void;
}

export default function EditPanel({
  save,
  onClose,
  onChanged,
  onDeleted,
  onOpen,
  onError,
}: Props) {
  const [title, setTitle] = useState(save.title);
  const [description, setDescription] = useState(save.description);
  const [notes, setNotes] = useState(save.notes);
  const [tagsInput, setTagsInput] = useState(save.tags.join(", "));
  const [saving, setSaving] = useState(false);
  const [checking, setChecking] = useState(false);
  const [archiveText, setArchiveText] = useState<string | null>(null);

  async function handleViewArchive() {
    try {
      setArchiveText(await api.getArchive(save.id));
    } catch (e) {
      onError(String(e));
    }
  }

  // Re-seed local fields when a different save is selected.
  useEffect(() => {
    setTitle(save.title);
    setDescription(save.description);
    setNotes(save.notes);
    setTagsInput(save.tags.join(", "));
  }, [save.id]); // eslint-disable-line react-hooks/exhaustive-deps

  const dirty =
    title !== save.title ||
    description !== save.description ||
    notes !== save.notes ||
    tagsInput !== save.tags.join(", ");

  async function handleSave() {
    setSaving(true);
    try {
      await api.updateSave(save.id, { title, description, notes });
      const updated = await api.setTags(save.id, parseTagsInput(tagsInput));
      onChanged(updated);
    } catch (e) {
      onError(String(e));
    } finally {
      setSaving(false);
    }
  }

  async function handleCheckNow() {
    setChecking(true);
    try {
      onChanged(await api.checkSaveNow(save.id));
    } catch (e) {
      onError(String(e));
    } finally {
      setChecking(false);
    }
  }

  async function handleDelete() {
    if (!confirm(`Delete "${save.title || save.url}"?`)) return;
    try {
      await api.deleteSave(save.id);
      onDeleted(save.id);
    } catch (e) {
      onError(String(e));
    }
  }

  return (
    <section className="edit-panel">
      <div className="edit-header">
        <h2>Details</h2>
        <button className="btn btn-subtle" onClick={onClose}>
          <XIcon size={15} />
        </button>
      </div>

      <div className="edit-scroll">
        <button className="edit-url" onClick={() => onOpen(save)} title={save.url}>
          {save.url}
        </button>

        <div className="edit-status">
          <span className={`status-pill status-${save.status}`}>
            {STATUS_LABELS[save.status]}
          </span>
          {save.httpStatus != null && <span>HTTP {save.httpStatus}</span>}
          {save.lastCheckedAt != null && (
            <span>checked {relativeTime(save.lastCheckedAt)}</span>
          )}
          <button
            className="btn btn-subtle"
            onClick={async () => {
              try {
                onChanged(await api.setRead(save.id, !save.isRead));
              } catch (e) {
                onError(String(e));
              }
            }}
          >
            {save.isRead ? "Mark unread" : "Mark read"}
          </button>
        </div>
        {save.status === "redirected" && save.redirectUrl && (
          <div className="edit-redirect" title={save.redirectUrl}>
            <span className="edit-redirect-url">→ now at {save.redirectUrl}</span>
            <button
              className="btn btn-subtle"
              title="Replace the saved URL with the redirect target"
              onClick={async () => {
                try {
                  onChanged(await api.setUrl(save.id, save.redirectUrl));
                } catch (e) {
                  onError(String(e));
                }
              }}
            >
              Accept new URL
            </button>
          </div>
        )}

        {save.status === "dead" && (
          <div className="edit-redirect" title="Look for a snapshot on the Internet Archive">
            <span className="edit-redirect-url">Link appears dead.</span>
            <button
              className="btn btn-subtle"
              onClick={() =>
                openUrl(`https://web.archive.org/web/${save.url}`).catch((e) =>
                  onError(String(e)),
                )
              }
            >
              Open in Wayback Machine
            </button>
          </div>
        )}

        {save.archivedAt != null && (
          <div className="edit-archive-row">
            <span>
              Archived copy · {relativeTime(save.archivedAt)} — searchable
              even if the site goes down
            </span>
            <button className="btn btn-subtle" onClick={handleViewArchive}>
              View
            </button>
          </div>
        )}

        <label className="field">
          <span>Title</span>
          <input value={title} onChange={(e) => setTitle(e.target.value)} />
        </label>

        <label className="field">
          <span>Description</span>
          <textarea
            rows={3}
            value={description}
            onChange={(e) => setDescription(e.target.value)}
          />
        </label>

        <label className="field">
          <span>Notes</span>
          <textarea
            rows={6}
            placeholder="Your own notes — searchable"
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
          />
        </label>

        <label className="field">
          <span>Tags (comma separated)</span>
          <input
            placeholder="rust, reference, read-later"
            value={tagsInput}
            onChange={(e) => setTagsInput(e.target.value)}
          />
        </label>

        <div className="edit-dates">
          <span>Saved {relativeTime(save.createdAt)}</span>
          <span>Updated {relativeTime(save.updatedAt)}</span>
        </div>
      </div>

      {archiveText !== null && (
        <div className="modal-overlay" onClick={() => setArchiveText(null)}>
          <div
            className="modal modal-wide"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="modal-header">
              <h2>Archived text</h2>
              <button
                className="icon-btn"
                onClick={() => setArchiveText(null)}
                title="Close"
              >
                <XIcon size={15} />
              </button>
            </div>
            <div className="archive-text selectable">
              {archiveText || "No archived text for this save yet."}
            </div>
          </div>
        </div>
      )}

      <div className="edit-actions">
        <button
          className="btn btn-primary"
          disabled={!dirty || saving}
          onClick={handleSave}
        >
          {saving ? "Saving…" : "Save changes"}
        </button>
        <button className="btn" disabled={checking} onClick={handleCheckNow}>
          {checking ? "Checking…" : "Check link"}
        </button>
        <button className="btn btn-danger" onClick={handleDelete}>
          Delete
        </button>
      </div>
    </section>
  );
}
