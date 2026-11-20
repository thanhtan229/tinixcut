import { open } from "@tauri-apps/plugin-dialog";
import { FormEvent, useEffect, useState } from "react";
import { AppSettings, AutoCaptionReport, editorApi } from "../services/editorApi";
import { rationalTimeToFrames, rationalTimeToSeconds } from "../types/project";
import { captionMatchesSearch } from "./captionEditor";
import { suggestSpeechRangesFromTranscript, type TranscriptCutRange } from "./transcriptCut";
import type { Asset } from "../types/project";

export interface TranscriptSourceContext {
  projectId: string;
  assetId: string;
  assetName: string;
  sourcePath: string;
  durationSeconds?: number;
}

interface CachedTranscriptSource extends TranscriptSourceContext {
  segments: NonNullable<AutoCaptionReport["transcript"]>;
}

const WHISPER_PATH_KEY = "tinixcut.autoCaptions.whisperCliPath";
const MODEL_PATH_KEY = "tinixcut.autoCaptions.modelPath";
const LANGUAGE_KEY = "tinixcut.autoCaptions.language";

interface AutoCaptionDialogProps {
  isOpen: boolean;
  selectedClipName: string | null;
  projectId: string | null;
  reportSource: TranscriptSourceContext | null;
  availableTranscriptAssets: Asset[];
  report: AutoCaptionReport | null;
  onClose: () => void;
  onStart: (settings: {
    whisperCliPath: string;
    modelPath: string;
    language: string;
    analyzeOnly: boolean;
  }) => Promise<void>;
  onCancel: () => Promise<void>;
  onSeekTranscript: (frame: number) => void;
  onOpenSourceTranscript: (assetId: string, sourceSeconds: number) => void;
  onInsertTranscriptRange: (assetId: string, sourceInSeconds: number, sourceOutSeconds: number) => void;
  onBuildTranscriptCut: (assetId: string, ranges: TranscriptCutRange[]) => Promise<void>;
  frameRate: { numerator: number; denominator: number };
}

export function AutoCaptionDialog({
  isOpen,
  selectedClipName,
  projectId,
  reportSource,
  availableTranscriptAssets,
  report,
  onClose,
  onStart,
  onCancel,
  onSeekTranscript,
  onOpenSourceTranscript,
  onInsertTranscriptRange,
  onBuildTranscriptCut,
  frameRate,
}: AutoCaptionDialogProps) {
  const [whisperCliPath, setWhisperCliPath] = useState("");
  const [modelPath, setModelPath] = useState("");
  const [language, setLanguage] = useState("auto");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [transcriptQuery, setTranscriptQuery] = useState("");
  const [selectedTranscriptKeys, setSelectedTranscriptKeys] = useState<Set<string>>(() => new Set());
  const [minimumPauseSeconds, setMinimumPauseSeconds] = useState(1);
  const [speechPaddingSeconds, setSpeechPaddingSeconds] = useState(0.2);
  const [suggestedSpeechRanges, setSuggestedSpeechRanges] = useState<TranscriptCutRange[]>([]);
  const [transcriptLibrary, setTranscriptLibrary] = useState<Record<string, CachedTranscriptSource>>({});
  const isRunning = report?.state === "queued" || report?.state === "running";
  const availableAssetsById = new Map(availableTranscriptAssets.map((asset) => [asset.asset_id, asset]));
  const validTranscriptSources = Object.values(transcriptLibrary).filter((source) =>
    availableAssetsById.get(source.assetId)?.path === source.sourcePath);
  const allTranscriptRows = validTranscriptSources.flatMap((source) => source.segments.map((segment, index) => ({
    ...source, segment, index,
  })));
  const transcriptKey = ({assetId, segment, index}: Pick<typeof allTranscriptRows[number], "assetId" | "segment" | "index">) =>
    `${assetId}:${segment.sourceStart.value}/${segment.sourceStart.rate.numerator}/${segment.sourceStart.rate.denominator}:${segment.sourceDuration.value}/${segment.sourceDuration.rate.numerator}/${segment.sourceDuration.rate.denominator}:${index}`;
  const transcriptRows = allTranscriptRows.filter(({ segment }) => captionMatchesSearch(segment.text, transcriptQuery));
  const selectedTranscriptRanges = allTranscriptRows.filter(row => selectedTranscriptKeys.has(transcriptKey(row)));
  const selectedSourceIds = new Set(selectedTranscriptRanges.map(({ assetId }) => assetId));
  const hiddenSelectedCount = selectedTranscriptRanges.filter(({ segment }) => !captionMatchesSearch(segment.text, transcriptQuery)).length;

  useEffect(() => {
    setSelectedTranscriptKeys(new Set());
    setSuggestedSpeechRanges([]);
  }, [report?.jobId]);

  useEffect(() => {
    setTranscriptLibrary({});
    setSelectedTranscriptKeys(new Set());
    setSuggestedSpeechRanges([]);
    setTranscriptQuery("");
  }, [projectId]);

  useEffect(() => {
    if (report?.state !== "completed" || !report.transcript || !reportSource || reportSource.projectId !== projectId) return;
    if (availableAssetsById.get(reportSource.assetId)?.path !== reportSource.sourcePath) return;
    setTranscriptLibrary((current) => ({
      ...current,
      [reportSource.assetId]: { ...reportSource, segments: report.transcript! },
    }));
  }, [report?.state, report?.jobId, report?.transcript, reportSource, projectId, availableTranscriptAssets]);

  useEffect(() => {
    if (!isOpen) return;
    let active = true;
    setSettingsError(null);
    void (async () => {
      try {
        let settings = await editorApi.getAppSettings();
        if (!settings.legacyBrowserSettingsMigrated) {
          const readLegacyString = (key: string) => {
            const value = localStorage.getItem(key)?.trim() ?? "";
            return value.length > 0 && value.length <= 4096 ? value : null;
          };
          const legacyLanguage = localStorage.getItem(LANGUAGE_KEY);
          const migratedLanguage = legacyLanguage === "vi" || legacyLanguage === "en" ? legacyLanguage : "auto";
          settings = await editorApi.saveAppSettings({
            ...settings,
            whisperCliPath: settings.whisperCliPath ?? readLegacyString(WHISPER_PATH_KEY),
            whisperModelPath: settings.whisperModelPath ?? readLegacyString(MODEL_PATH_KEY),
            transcriptionLanguage:
              settings.transcriptionLanguage === "auto" && legacyLanguage ? migratedLanguage : settings.transcriptionLanguage,
            legacyBrowserSettingsMigrated: true,
          });
        }
        if (!active) return;
        setWhisperCliPath(settings.whisperCliPath ?? "");
        setModelPath(settings.whisperModelPath ?? "");
        setLanguage(settings.transcriptionLanguage);
      } catch (error) {
        if (active) setSettingsError(`Could not load saved app settings: ${error}`);
      }
    })();
    return () => {
      active = false;
    };
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
    await startJob(false);
  };

  const startJob = async (analyzeOnly: boolean) => {
    if (isRunning || !whisperCliPath.trim() || !modelPath.trim()) return;
    setIsSubmitting(true);
    setSettingsError(null);
    try {
      const current = await editorApi.getAppSettings();
      await editorApi.saveAppSettings({
        ...current,
        whisperCliPath: whisperCliPath.trim(),
        whisperModelPath: modelPath.trim(),
        transcriptionLanguage: language as AppSettings["transcriptionLanguage"],
        legacyBrowserSettingsMigrated: true,
      });
      await onStart({ whisperCliPath: whisperCliPath.trim(), modelPath: modelPath.trim(), language, analyzeOnly });
    } catch (error) {
      setSettingsError(`Could not save app settings or start captions: ${error}`);
    } finally {
      setIsSubmitting(false);
    }
  };

  const buildTranscriptCut = async (assetId: string, ranges: TranscriptCutRange[]) => {
    if (ranges.length === 0) return;
    setIsSubmitting(true);
    setSettingsError(null);
    try {
      await onBuildTranscriptCut(assetId, ranges);
      setSelectedTranscriptKeys(new Set());
      setSuggestedSpeechRanges([]);
    } catch (error) {
      setSettingsError(`Could not create transcript rough cut: ${error}`);
    } finally {
      setIsSubmitting(false);
    }
  };

  const suggestCutsAtPauses = () => {
    try {
      const ranges = suggestSpeechRangesFromTranscript(
        report?.transcript ?? [], minimumPauseSeconds, speechPaddingSeconds, reportSource?.durationSeconds,
      );
      if (ranges.length === 0) {
        setSettingsError("No speech ranges were available for a pause-cut draft.");
        return;
      }
      setSettingsError(null);
      setSuggestedSpeechRanges(ranges);
    } catch (error) {
      setSettingsError(String(error));
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

        {settingsError && <p className="form-message form-message-error" role="alert">{settingsError}</p>}

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

          {validTranscriptSources.length > 0 && (
            <section aria-label="Source transcript" style={{ maxHeight: 240, overflow: "auto", border: "1px solid var(--border-color)", borderRadius: 6, padding: 8 }}>
              <strong>Analyzed source transcripts · {validTranscriptSources.length} asset(s) · {transcriptRows.length} match(es)</strong>
              <input aria-label="Search source transcript" placeholder="Find in source transcript" value={transcriptQuery} onChange={(event) => setTranscriptQuery(event.currentTarget.value)} />
              {transcriptQuery && <button type="button" onClick={() => setTranscriptQuery("")}>Clear transcript search</button>}
              {validTranscriptSources.map(source => {
                const visible = transcriptRows.filter(row => row.assetId === source.assetId);
                if (!visible.length) return null;
                const visibleKeys=visible.map(transcriptKey);
                const selectedVisible=visibleKeys.filter(key=>selectedTranscriptKeys.has(key)).length;
                return <div key={source.assetId}>
                  <button type="button" disabled={isRunning || isSubmitting || selectedSourceIds.size > 0 && !selectedSourceIds.has(source.assetId)} onClick={() => setSelectedTranscriptKeys(current => new Set([...current, ...visibleKeys]))}>Select {visible.length} visible segment{visible.length === 1 ? "" : "s"} from {source.assetName}</button>{" "}
                  <button type="button" disabled={isRunning || isSubmitting || selectedVisible===0} onClick={() => setSelectedTranscriptKeys(current => {const next=new Set(current);visibleKeys.forEach(key=>next.delete(key));return next;})}>Deselect {selectedVisible} visible segment{selectedVisible === 1 ? "" : "s"} from {source.assetName}</button>
                </div>;
              })}
              {transcriptRows.map(({ segment, assetId, assetName, index }) => {
                const selectionKey = transcriptKey({assetId, segment, index});
                return (
                <div key={selectionKey} style={{ display: "grid", gridTemplateColumns: "130px minmax(0,1fr)", gap: 8, padding: "6px 0", borderBottom: "1px solid var(--border-color)" }}>
                  <div style={{ display: "flex", flexDirection: "column", gap: 3, alignItems: "flex-start" }}>
                    <label style={{ fontSize: 11 }}>
                      <input type="checkbox" aria-label={`Select transcript segment ${index + 1} from ${assetName}`} checked={selectedTranscriptKeys.has(selectionKey)} disabled={isRunning || isSubmitting || selectedSourceIds.size > 0 && !selectedSourceIds.has(assetId)} onChange={(event) => {
                        const checked = event.currentTarget.checked;
                        setSelectedTranscriptKeys((current) => {
                          const next = new Set(current);
                          if (checked) next.add(selectionKey);
                          else next.delete(selectionKey);
                          return next;
                        });
                      }} /> Draft
                    </label>
                    <button type="button" onClick={() => onOpenSourceTranscript(assetId, rationalTimeToSeconds(segment.sourceStart))} title={`Open ${assetName} at this source time`}>
                      Source {rationalTimeToSeconds(segment.sourceStart).toFixed(2)}s
                    </button>
                    <button type="button" onClick={() => onInsertTranscriptRange(
                      assetId,
                      rationalTimeToSeconds(segment.sourceStart),
                      rationalTimeToSeconds(segment.sourceStart) + rationalTimeToSeconds(segment.sourceDuration),
                    )} title="Insert this spoken source range at the timeline playhead">
                      Insert range
                    </button>
                    {report?.state === "completed" && reportSource?.assetId === assetId && segment.timelineStart ? (
                      <button type="button" onClick={() => onSeekTranscript(rationalTimeToFrames(segment.timelineStart!, frameRate))} title="Jump to this source transcript segment on the timeline">
                        Timeline
                      </button>
                    ) : <span style={{ fontSize: 10 }}>outside selected clip</span>}
                  </div>
                  <span><small style={{ display: "block", color: "var(--text-muted)" }}>{assetName} · {rationalTimeToSeconds(segment.sourceStart).toFixed(2)}–{(rationalTimeToSeconds(segment.sourceStart) + rationalTimeToSeconds(segment.sourceDuration)).toFixed(2)} s</small>{segment.text}</span>
                </div>
              );})}
              {selectedTranscriptRanges.length > 0 && (
                <div aria-label="Transcript selection actions" style={{ position: "sticky", bottom: 0, zIndex: 1, background: "var(--bg-panel, #18181c)", display: "flex", flexWrap: "wrap", alignItems: "center", justifyContent: "space-between", gap: 8, padding: "8px 0", borderTop: "1px solid var(--border-color)" }}>
                  <span>{selectedTranscriptRanges.length} selected{hiddenSelectedCount > 0 ? ` · ${hiddenSelectedCount} hidden by search` : ""} · {selectedTranscriptRanges.reduce((sum, { segment }) => sum + rationalTimeToSeconds(segment.sourceDuration), 0).toFixed(2)} s · {selectedSourceIds.size === 1 ? `source: ${selectedTranscriptRanges[0].assetName}` : "choose one source for a rough cut"}</span>
                  {hiddenSelectedCount > 0 && <button type="button" onClick={() => setTranscriptQuery("")}>Show hidden selections</button>}
                  <button type="button" onClick={() => setSelectedTranscriptKeys(new Set())} disabled={isSubmitting || isRunning}>Clear selection</button>
                  <button className="btn-primary" type="button" disabled={isSubmitting || isRunning || selectedSourceIds.size !== 1} onClick={() => void buildTranscriptCut(selectedTranscriptRanges[0].assetId, selectedTranscriptRanges.map(({ segment }) => {
                    const sourceInSeconds = rationalTimeToSeconds(segment.sourceStart);
                    return { sourceInSeconds, sourceOutSeconds: sourceInSeconds + rationalTimeToSeconds(segment.sourceDuration) };
                  }))}>
                    {isSubmitting ? "Building draft…" : "Create rough cut track"}
                  </button>
                </div>
              )}
              <div style={{ display: "flex", flexWrap: "wrap", alignItems: "center", gap: 8, paddingTop: 8, borderTop: "1px solid var(--border-color)" }}>
                <label style={{ fontSize: 11 }}>Pause threshold (s)
                  <input aria-label="Pause threshold seconds" type="number" min={0.1} max={10} step={0.1} value={minimumPauseSeconds} onChange={(event) => setMinimumPauseSeconds(Number(event.currentTarget.value))} style={{ width: 72, marginLeft: 4 }} />
                </label>
                <label style={{ fontSize: 11 }}>Speech padding (s)
                  <input aria-label="Speech padding seconds" type="number" min={0} max={2} step={0.05} value={speechPaddingSeconds} onChange={(event) => setSpeechPaddingSeconds(Number(event.currentTarget.value))} style={{ width: 72, marginLeft: 4 }} />
                </label>
                <button type="button" disabled={isRunning || isSubmitting || !report?.transcript?.length || !reportSource} onClick={suggestCutsAtPauses}>Suggest pause cuts</button>
                <span style={{ color: "var(--text-muted)", fontSize: 11 }}>Estimated from transcript gaps; listen through before applying.</span>
              </div>
              {suggestedSpeechRanges.length > 0 && (
                <div style={{ display: "flex", flexDirection: "column", gap: 6, paddingTop: 8 }}>
                  <strong>{suggestedSpeechRanges.length} speech range(s) · {suggestedSpeechRanges.reduce((sum, range) => sum + range.sourceOutSeconds - range.sourceInSeconds, 0).toFixed(2)} s kept</strong>
                  <ol aria-label="Suggested speech ranges" style={{ margin: 0, paddingLeft: 22, maxHeight: 100, overflow: "auto" }}>
                    {suggestedSpeechRanges.map((range, index) => <li key={`${range.sourceInSeconds}-${index}`}>{range.sourceInSeconds.toFixed(2)}–{range.sourceOutSeconds.toFixed(2)} s</li>)}
                  </ol>
                  <button className="btn-primary" type="button" disabled={isSubmitting || isRunning || !reportSource} onClick={() => reportSource && void buildTranscriptCut(reportSource.assetId, suggestedSpeechRanges)}>
                    {isSubmitting ? "Building draft…" : "Apply pause-cut rough cut"}
                  </button>
                </div>
              )}
              {transcriptRows.length === 0 && <p>No transcript segments match this search.</p>}
              <p style={{ color: "var(--text-muted)", fontSize: 11 }}>Transcript is read-only source analysis. Editing or generating captions does not change these source-time results.</p>
            </section>
          )}

          <div className="modal-dialog-actions">
            {isRunning ? (
              <button className="btn-action btn-danger" type="button" onClick={() => void onCancel()}>
                Cancel job
              </button>
            ) : (
              <>
                <button
                  className="btn-action btn-primary"
                  type="submit"
                  disabled={!selectedClipName || !whisperCliPath.trim() || !modelPath.trim() || isSubmitting}
                >
                  {isSubmitting ? "Starting…" : "Generate captions"}
                </button>
                <button className="btn-action" type="button" onClick={() => void startJob(true)} disabled={!selectedClipName || !whisperCliPath.trim() || !modelPath.trim() || isSubmitting}>
                  {isSubmitting ? "Starting…" : "Analyze transcript"}
                </button>
              </>
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
