import { open } from "@tauri-apps/plugin-dialog";
import { FormEvent, useEffect, useState } from "react";
import { AutoCaptionReport } from "../services/editorApi";

const WHISPER_PATH_KEY = "tinixcut.autoCaptions.whisperCliPath";
const MODEL_PATH_KEY = "tinixcut.autoCaptions.modelPath";
const LANGUAGE_KEY = "tinixcut.autoCaptions.language";

interface AutoCaptionDialogProps {
  isOpen: boolean;
  selectedClipName: string | null;
  report: AutoCaptionReport | null;
  onClose: () => void;
  onStart: (settings: {
    whisperCliPath: string;
    modelPath: string;
    language: string;
  }) => Promise<void>;
  onCancel: () => Promise<void>;
}

export function AutoCaptionDialog({
  isOpen,
  selectedClipName,
  report,
  onClose,
  onStart,
  onCancel,
}: AutoCaptionDialogProps) {
  const [whisperCliPath, setWhisperCliPath] = useState("");
  const [modelPath, setModelPath] = useState("");
  const [language, setLanguage] = useState("auto");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const isRunning = report?.state === "queued" || report?.state === "running";

  useEffect(() => {
    if (!isOpen) return;
    setWhisperCliPath(localStorage.getItem(WHISPER_PATH_KEY) ?? "");
    setModelPath(localStorage.getItem(MODEL_PATH_KEY) ?? "");
    setLanguage(localStorage.getItem(LANGUAGE_KEY) ?? "auto");
  }, [isOpen]);

  if (!isOpen) return null;

  const chooseWhisperCli = async () => {
    const selected = await open({
      title: "Select whisper-cli.exe",
      filters: [{ name: "Whisper CLI", extensions: ["exe"] }],
      multiple: false,
    });
    if (selected && !Array.isArray(selected)) setWhisperCliPath(selected);
  };

  const chooseModel = async () => {
    const selected = await open({
      title: "Select whisper.cpp model",
      filters: [{ name: "Whisper model", extensions: ["bin"] }],
      multiple: false,
    });
    if (selected && !Array.isArray(selected)) setModelPath(selected);
  };

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (isRunning || !whisperCliPath.trim() || !modelPath.trim()) return;
    setIsSubmitting(true);
    try {
      localStorage.setItem(WHISPER_PATH_KEY, whisperCliPath.trim());
      localStorage.setItem(MODEL_PATH_KEY, modelPath.trim());
      localStorage.setItem(LANGUAGE_KEY, language);
      await onStart({ whisperCliPath: whisperCliPath.trim(), modelPath: modelPath.trim(), language });
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div className="dialog-backdrop" role="presentation" onMouseDown={() => !isRunning && onClose()}>
      <section
        className="modal-dialog auto-caption-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="auto-caption-title"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="modal-dialog-header">
          <div>
            <h2 id="auto-caption-title">Auto captions</h2>
            <p>Runs Whisper locally. Audio and transcript files are not uploaded or stored in the project.</p>
          </div>
          <button className="btn-action" type="button" onClick={onClose} disabled={isRunning} aria-label="Close">
            ×
          </button>
        </div>

        {!selectedClipName && (
          <p className="form-message form-message-error">Select one audio or video clip on the timeline first.</p>
        )}

        <form onSubmit={handleSubmit}>
          <label className="dialog-field">
            <span>Whisper CLI</span>
            <div className="dialog-file-field">
              <input
                value={whisperCliPath}
                onChange={(event) => setWhisperCliPath(event.target.value)}
                placeholder="Path to whisper-cli.exe"
                disabled={isRunning}
              />
              <button className="btn-action" type="button" onClick={chooseWhisperCli} disabled={isRunning}>
                Browse
              </button>
            </div>
          </label>

          <label className="dialog-field">
            <span>Whisper model</span>
            <div className="dialog-file-field">
              <input
                value={modelPath}
                onChange={(event) => setModelPath(event.target.value)}
                placeholder="Path to ggml model .bin"
                disabled={isRunning}
              />
              <button className="btn-action" type="button" onClick={chooseModel} disabled={isRunning}>
                Browse
              </button>
            </div>
          </label>

          <label className="dialog-field">
            <span>Spoken language</span>
            <select value={language} onChange={(event) => setLanguage(event.target.value)} disabled={isRunning}>
              <option value="auto">Detect automatically</option>
              <option value="vi">Vietnamese</option>
              <option value="en">English</option>
            </select>
          </label>

          {report && (
            <div className={`caption-job-status caption-job-${report.state}`}>
              <div className="caption-job-status-line">
                <span>{report.message}</span>
                <span>{Math.round(report.progress * 100)}%</span>
              </div>
              <div className="caption-job-progress" aria-label={`Caption progress ${Math.round(report.progress * 100)}%`}>
                <span style={{ width: `${Math.round(report.progress * 100)}%` }} />
              </div>
            </div>
          )}

          <div className="modal-dialog-actions">
            {isRunning ? (
              <button className="btn-action btn-danger" type="button" onClick={() => void onCancel()}>
                Cancel job
              </button>
            ) : (
              <button
                className="btn-action btn-primary"
                type="submit"
                disabled={!selectedClipName || !whisperCliPath.trim() || !modelPath.trim() || isSubmitting}
              >
                {isSubmitting ? "Starting…" : "Generate captions"}
              </button>
            )}
            <button className="btn-action" type="button" onClick={onClose} disabled={isRunning}>
              Close
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}
