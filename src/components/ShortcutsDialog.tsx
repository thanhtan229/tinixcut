import { useEffect, useState } from "react";
import {
  chordFromEvent,
  DEFAULT_SHORTCUTS,
  SHORTCUT_ACTIONS,
  SHORTCUT_LABELS,
  validateRemap,
  type ShortcutAction,
  type ShortcutMap,
} from "../hooks/shortcutMap";

interface ShortcutsDialogProps {
  isOpen: boolean;
  onClose: () => void;
  shortcuts: ShortcutMap;
  /** Persists the new map; rejects with the reason when the save fails. */
  onSave: (map: ShortcutMap) => Promise<void>;
}

/** Lists every action with its chords; "Change" captures the next key press as the new chord. */
export function ShortcutsDialog({ isOpen, onClose, shortcuts, onSave }: ShortcutsDialogProps) {
  const [draft, setDraft] = useState<ShortcutMap>(shortcuts);
  const [capturing, setCapturing] = useState<ShortcutAction | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (isOpen) {
      setDraft(shortcuts);
      setCapturing(null);
      setMessage(null);
    }
  }, [isOpen, shortcuts]);

  useEffect(() => {
    if (!capturing) return undefined;
    const onKeyDown = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopPropagation();
      if (event.key === "Escape") {
        setCapturing(null);
        return;
      }
      const chord = chordFromEvent(event);
      if (!chord) return; // a modifier on its own; wait for the key
      const problem = validateRemap(draft, capturing, chord);
      if (problem) {
        setMessage(problem);
        return;
      }
      setDraft({ ...draft, [capturing]: [chord] });
      setMessage(null);
      setCapturing(null);
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [capturing, draft]);

  if (!isOpen) return null;

  const save = async () => {
    setSaving(true);
    try {
      await onSave(draft);
      onClose();
    } catch (cause) {
      setMessage(`Could not save the shortcuts: ${cause}`);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="dialog-backdrop" role="presentation" onMouseDown={onClose}>
      <section
        className="modal-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="shortcuts-title"
        onMouseDown={(event) => event.stopPropagation()}
        style={{ width: 520, maxWidth: "calc(100vw - 32px)" }}
      >
        <div className="modal-dialog-header">
          <div>
            <h2 id="shortcuts-title">Keyboard shortcuts</h2>
            <p>Choose Change, then press the new key combination. Escape cancels.</p>
          </div>
          <button className="btn-action" type="button" onClick={onClose} aria-label="Close keyboard shortcuts">×</button>
        </div>
        {message && <p className="form-message form-message-error" role="alert">{message}</p>}
        <div style={{ display: "grid", gridTemplateColumns: "1fr auto auto", gap: "6px 12px", maxHeight: 360, overflow: "auto" }}>
          {SHORTCUT_ACTIONS.map((action) => (
            <div key={action} style={{ display: "contents" }}>
              <span>{SHORTCUT_LABELS[action]}</span>
              <code aria-label={`${SHORTCUT_LABELS[action]} shortcut`}>
                {capturing === action ? "Press a key…" : draft[action].join(" / ")}
              </code>
              <button
                type="button"
                aria-label={`Change ${SHORTCUT_LABELS[action]}`}
                onClick={() => {
                  setMessage(null);
                  setCapturing(action);
                }}
              >
                Change
              </button>
            </div>
          ))}
        </div>
        <div style={{ display: "flex", justifyContent: "space-between", marginTop: 12 }}>
          <button type="button" onClick={() => setDraft({ ...DEFAULT_SHORTCUTS })}>
            Reset to defaults
          </button>
          <button type="button" disabled={saving} onClick={() => void save()}>
            Save
          </button>
        </div>
      </section>
    </div>
  );
}
