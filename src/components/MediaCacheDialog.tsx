import { useEffect, useState } from "react";
import { editorApi, MediaCacheStatus } from "../services/editorApi";

interface MediaCacheDialogProps {
  isOpen: boolean;
  onClose: () => void;
  previewProxyMode: "off" | "auto";
  // Persists the app setting; rejects with the reason when the save fails.
  onPreviewProxyModeChange: (mode: "off" | "auto") => Promise<void>;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(1)} ${units[unit]}`;
}

export function MediaCacheDialog({ isOpen, onClose, previewProxyMode, onPreviewProxyModeChange }: MediaCacheDialogProps) {
  const [savingProxyMode, setSavingProxyMode] = useState(false);
  const [status, setStatus] = useState<MediaCacheStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [confirmClear, setConfirmClear] = useState(false);

  const refreshStatus = async () => {
    setLoading(true);
    setError(null);
    try {
      setStatus(await editorApi.getMediaCacheStatus());
    } catch (cause) {
      setError(`Could not read the media cache: ${cause}`);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (isOpen) {
      setConfirmClear(false);
      void refreshStatus();
    }
  }, [isOpen]);

  if (!isOpen) return null;

  const clearCache = async () => {
    setLoading(true);
    setError(null);
    try {
      setStatus(await editorApi.clearMediaCache());
      setConfirmClear(false);
    } catch (cause) {
      setError(`Could not clear the media cache: ${cause}`);
    } finally {
      setLoading(false);
    }
  };

  const changeProxyMode = async (enabled: boolean) => {
    setSavingProxyMode(true);
    setError(null);
    try {
      await onPreviewProxyModeChange(enabled ? "auto" : "off");
    } catch (cause) {
      setError(`Could not save the preview proxy setting: ${cause}`);
    } finally {
      setSavingProxyMode(false);
    }
  };

  return (
    <div className="dialog-backdrop" role="presentation" onMouseDown={onClose}>
      <section
        className="modal-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="media-cache-title"
        onMouseDown={(event) => event.stopPropagation()}
        style={{ width: 480, maxWidth: "calc(100vw - 32px)" }}
      >
        <div className="modal-dialog-header">
          <div>
            <h2 id="media-cache-title">Media cache</h2>
            <p>Derived previews and analysis can be rebuilt from the original project media.</p>
          </div>
          <button className="btn-action" type="button" onClick={onClose} aria-label="Close media cache">×</button>
        </div>

        <fieldset style={{ border: "1px solid var(--border-color)", borderRadius: 4, padding: "8px 12px", margin: "0 0 12px" }}>
          <legend>Preview proxies</legend>
          <label style={{ display: "flex", gap: 8, alignItems: "flex-start" }}>
            <input
              type="checkbox"
              checked={previewProxyMode === "auto"}
              disabled={savingProxyMode}
              onChange={(event) => void changeProxyMode(event.currentTarget.checked)}
              aria-describedby="preview-proxy-help"
            />
            <span>Use seek-optimized proxies for video preview</span>
          </label>
          <p id="preview-proxy-help" style={{ margin: "6px 0 0", fontSize: 12, color: "var(--text-muted)" }}>
            Builds a 720p copy of each project video in the background, then previews from it for faster seeking. Proxies use extra disk space, sometimes more than the original files, and appear under "proxies" below. Export always uses the original media.
          </p>
        </fieldset>

        {error && <p className="form-message form-message-error" role="alert">{error}</p>}
        {loading && <p role="status">Reading local cache…</p>}
        {status && (
          <>
            <p><strong>{formatBytes(status.totalBytes)}</strong> across {status.totalFiles} cached file(s)</p>
            <div style={{ display: "grid", gridTemplateColumns: "1fr auto auto", gap: "6px 12px", maxHeight: 240, overflow: "auto" }}>
              {Object.entries(status.categories).map(([category, value]) => (
                <div key={category} style={{ display: "contents" }}>
                  <span>{category}</span>
                  <span>{value.files} file(s)</span>
                  <span>{formatBytes(value.bytes)}</span>
                </div>
              ))}
            </div>
          </>
        )}

        {confirmClear ? (
          <div role="group" aria-label="Confirm clear media cache" style={{ marginTop: 16 }}>
            <p>This removes only known derived cache folders and preserves other files in the cache directory. The current app instance will refuse while media or export jobs are active.</p>
            <div className="modal-dialog-actions">
              <button className="btn-action" type="button" onClick={() => setConfirmClear(false)} disabled={loading}>Keep cache</button>
              <button className="btn-action btn-danger" type="button" onClick={() => void clearCache()} disabled={loading}>Clear derived cache</button>
            </div>
          </div>
        ) : (
          <div className="modal-dialog-actions">
            <button className="btn-action" type="button" onClick={() => void refreshStatus()} disabled={loading}>Refresh</button>
            <button className="btn-action btn-danger" type="button" onClick={() => setConfirmClear(true)} disabled={loading || !status || status.totalFiles === 0}>Clear derived cache…</button>
          </div>
        )}
      </section>
    </div>
  );
}
