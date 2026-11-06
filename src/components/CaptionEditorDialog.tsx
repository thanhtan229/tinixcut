import React, { useEffect, useMemo, useState } from "react";
import { Project, TimelineItem, rationalTimeToFrames, rationalTimeToSeconds } from "../types/project";
import { buildCaptionTimingOperations, canMergeCaptions, canSplitCaptionAtFrame, captionMatchesSearch, collectCaptionRows, isValidCaptionDurationFrames, isValidCaptionStartFrame, type CaptionTimingDraft } from "./captionEditor";

export interface CaptionDraft {
  item_id: string;
  text: string;
}

interface CaptionEditorDialogProps {
  project: Project | null;
  playheadFrame: number;
  isOpen: boolean;
  onClose: () => void;
  onSeek: (frame: number) => void;
  onApplyEdits: (textDrafts: CaptionDraft[], timingDrafts: CaptionTimingDraft[]) => Promise<unknown>;
  onMerge: (leftId: string, rightId: string) => Promise<unknown>;
  onSplit: (itemId: string, frame: number) => Promise<unknown>;
}

function formatTime(item: TimelineItem): string {
  const start = rationalTimeToSeconds(item.timeline_range.start);
  const end = start + rationalTimeToSeconds(item.timeline_range.duration);
  return `${start.toFixed(2)}–${end.toFixed(2)}s`;
}

export const CaptionEditorDialog: React.FC<CaptionEditorDialogProps> = ({
  project,
  playheadFrame,
  isOpen,
  onClose,
  onSeek,
  onApplyEdits,
  onMerge,
  onSplit,
}) => {
  const captions = useMemo(() => collectCaptionRows(project), [project]);
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [timingDrafts, setTimingDrafts] = useState<Record<string, { start_frame: number; duration_frames: number }>>({});
  const [query, setQuery] = useState("");
  const [replaceFrom, setReplaceFrom] = useState("");
  const [replaceTo, setReplaceTo] = useState("");
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [mergeSelection, setMergeSelection] = useState<string[]>([]);

  useEffect(() => {
    setDrafts(Object.fromEntries(captions.map(({ item }) => [item.item_id, item.text_content ?? ""])))
    const rate = project?.settings.frame_rate ?? { numerator: 30, denominator: 1 };
    setTimingDrafts(Object.fromEntries(captions.map(({ item }) => [item.item_id, {
      start_frame: rationalTimeToFrames(item.timeline_range.start, rate),
      duration_frames: rationalTimeToFrames(item.timeline_range.duration, rate),
    }])))
  }, [project?.project_id, project?.revision]);

  if (!isOpen || !project) return null;

  const filtered = captions.filter(({ item }) =>
    captionMatchesSearch(`${item.text_content ?? ""} ${drafts[item.item_id] ?? ""}`, query));
  const dirtyDrafts = captions
    .filter(({ item }) => (drafts[item.item_id] ?? "") !== (item.text_content ?? ""))
    .map(({ item }) => ({ item_id: item.item_id, text: drafts[item.item_id] ?? "" }));
  const dirtyTimingDrafts = captions.flatMap(({ item }) => {
    const draft = timingDrafts[item.item_id];
    const startFrame = draft?.start_frame ?? rationalTimeToFrames(item.timeline_range.start, project.settings.frame_rate);
    const durationFrames = draft?.duration_frames ?? rationalTimeToFrames(item.timeline_range.duration, project.settings.frame_rate);
    const originalStart = rationalTimeToFrames(item.timeline_range.start, project.settings.frame_rate);
    const originalDuration = rationalTimeToFrames(item.timeline_range.duration, project.settings.frame_rate);
    return startFrame === originalStart && durationFrames === originalDuration ? [] : [{ item_id: item.item_id, start_frame: startFrame, duration_frames: durationFrames }];
  });
  let timingDraftsValid = dirtyTimingDrafts.every(({ start_frame, duration_frames }) => isValidCaptionStartFrame(start_frame) && isValidCaptionDurationFrames(duration_frames));
  if (timingDraftsValid && dirtyTimingDrafts.length > 0) {
    try { buildCaptionTimingOperations(project, dirtyTimingDrafts); } catch { timingDraftsValid = false; }
  }
  const mergeReady = mergeSelection.length === 2
    && dirtyDrafts.length === 0
    && dirtyTimingDrafts.length === 0
    && canMergeCaptions(project, mergeSelection[0], mergeSelection[1]);

  const applyReplace = () => {
    if (!replaceFrom) return;
    setDrafts((current) => {
      const next = { ...current };
      for (const { item, locked } of captions) {
        if (locked) continue;
        const text = next[item.item_id] ?? item.text_content ?? "";
        if (text.includes(replaceFrom)) next[item.item_id] = text.split(replaceFrom).join(replaceTo);
      }
      return next;
    });
  };

  const applyEdits = async () => {
    if (dirtyDrafts.length === 0 && dirtyTimingDrafts.length === 0) return;
    setBusy(true);
    setActionError(null);
    try {
      await onApplyEdits(dirtyDrafts, dirtyTimingDrafts);
    } catch (error) {
      setActionError(String(error));
    } finally {
      setBusy(false);
    }
  };

  const mergeSelected = async () => {
    if (!mergeReady) return;
    setBusy(true);
    try {
      await onMerge(mergeSelection[0], mergeSelection[1]);
      setMergeSelection([]);
    } catch (error) {
      setActionError(String(error));
    } finally {
      setBusy(false);
    }
  };

  const splitAtPlayhead = async (itemId: string) => {
    setBusy(true);
    try {
      await onSplit(itemId, playheadFrame);
    } catch (error) {
      setActionError(String(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }} style={{ position: "fixed", inset: 0, zIndex: 1200, background: "rgba(0,0,0,.68)", display: "grid", placeItems: "center", padding: 20 }}>
      <section role="dialog" aria-modal="true" aria-labelledby="caption-editor-title" style={{ width: "min(960px, 96vw)", maxHeight: "88vh", display: "flex", flexDirection: "column", gap: 12, padding: 18, borderRadius: 10, background: "var(--bg-primary)", color: "var(--text-primary)", border: "1px solid var(--border-color)" }}>
        <header style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
          <div>
            <h2 id="caption-editor-title" style={{ margin: 0, fontSize: 18 }}>Caption editor</h2>
            <span style={{ color: "var(--text-muted)", fontSize: 12 }}>{captions.length} text clips · edit captions before applying one undoable change</span>
          </div>
          <button type="button" onClick={onClose} aria-label="Close caption editor">Close</button>
        </header>

        <div style={{ display: "flex", flexWrap: "wrap", alignItems: "center", gap: 8 }}>
          <input aria-label="Search captions" placeholder="Find caption text" value={query} onChange={(event) => setQuery(event.currentTarget.value)} style={{ flex: "1 1 180px" }} />
          <input aria-label="Replace text" placeholder="Find" value={replaceFrom} onChange={(event) => setReplaceFrom(event.currentTarget.value)} />
          <input aria-label="Replace with" placeholder="Replace with" value={replaceTo} onChange={(event) => setReplaceTo(event.currentTarget.value)} />
          <button type="button" onClick={applyReplace} disabled={!replaceFrom}>Replace all</button>
        </div>

        {actionError && <p role="alert" style={{ color: "var(--danger, #f66)" }}>{actionError}</p>}

        <div style={{ overflow: "auto", display: "flex", flexDirection: "column", gap: 8 }}>
          {filtered.map(({ item, trackName, locked }) => {
            const startFrame = rationalTimeToFrames(item.timeline_range.start, project.settings.frame_rate);
            const canSplit = canSplitCaptionAtFrame(item, playheadFrame, project.settings.frame_rate);
            return (
              <article key={item.item_id} style={{ display: "grid", gridTemplateColumns: "24px 90px 78px 78px 130px minmax(220px,1fr) auto", gap: 8, alignItems: "start", padding: 8, border: "1px solid var(--border-color)", borderRadius: 6 }}>
                <input type="checkbox" aria-label={`Select caption ${item.item_id} to merge`} checked={mergeSelection.includes(item.item_id)} disabled={locked || busy || (!mergeSelection.includes(item.item_id) && mergeSelection.length >= 2)} onChange={(event) => {
                  const checked = event.currentTarget.checked;
                  setMergeSelection((current) => checked ? [...current, item.item_id] : current.filter((id) => id !== item.item_id));
                }} />
                <button type="button" onClick={() => onSeek(startFrame)} title="Seek to caption start" style={{ textAlign: "left" }}>{formatTime(item)}</button>
                <label style={{ fontSize: 11, color: "var(--text-muted)" }}>Start frame
                  <input aria-label={`Start frame ${item.item_id}`} type="number" min={0} step={1} disabled={locked || busy} value={timingDrafts[item.item_id]?.start_frame ?? startFrame} onChange={(event) => {
                    const value = event.currentTarget.value === "" ? Number.NaN : Number(event.currentTarget.value);
                    const current = timingDrafts[item.item_id] ?? { start_frame: startFrame, duration_frames: rationalTimeToFrames(item.timeline_range.duration, project.settings.frame_rate) };
                    setTimingDrafts((drafts) => ({ ...drafts, [item.item_id]: { ...current, start_frame: value } }));
                  }} style={{ width: "100%" }} />
                </label>
                <label style={{ fontSize: 11, color: "var(--text-muted)" }}>Duration frames
                  <input aria-label={`Duration frames ${item.item_id}`} type="number" min={1} step={1} disabled={locked || busy} value={timingDrafts[item.item_id]?.duration_frames ?? rationalTimeToFrames(item.timeline_range.duration, project.settings.frame_rate)} onChange={(event) => {
                    const value = event.currentTarget.value === "" ? Number.NaN : Number(event.currentTarget.value);
                    const current = timingDrafts[item.item_id] ?? { start_frame: startFrame, duration_frames: rationalTimeToFrames(item.timeline_range.duration, project.settings.frame_rate) };
                    setTimingDrafts((drafts) => ({ ...drafts, [item.item_id]: { ...current, duration_frames: value } }));
                  }} style={{ width: "100%" }} />
                </label>
                <span style={{ color: "var(--text-muted)", fontSize: 11 }}>{trackName}{locked ? " · locked" : ""}</span>
                <textarea aria-label={`Caption text ${formatTime(item)}`} rows={2} disabled={locked} value={drafts[item.item_id] ?? ""} onChange={(event) => {
                  const value = event.currentTarget.value;
                  setDrafts((current) => ({ ...current, [item.item_id]: value }));
                }} />
                <button type="button" disabled={locked || !canSplit || busy || dirtyDrafts.length > 0 || dirtyTimingDrafts.length > 0} onClick={() => void splitAtPlayhead(item.item_id)} title={locked ? "Unlock this track before editing" : dirtyDrafts.length > 0 || dirtyTimingDrafts.length > 0 ? "Apply caption edits before splitting" : "Split this caption at the playhead"}>Split here</button>
              </article>
            );
          })}
          {filtered.length === 0 && <div style={{ padding: 24, textAlign: "center", color: "var(--text-muted)" }}>No captions match this search.</div>}
        </div>

        <footer style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 8 }}>
          <span style={{ color: "var(--text-muted)", fontSize: 12 }}>{dirtyDrafts.length} text edits · {dirtyTimingDrafts.length} timing edits</span>
          <div style={{ display: "flex", gap: 8 }}>
            <button type="button" onClick={() => {
              setDrafts(Object.fromEntries(captions.map(({ item }) => [item.item_id, item.text_content ?? ""])));
              setTimingDrafts(Object.fromEntries(captions.map(({ item }) => [item.item_id, {
                start_frame: rationalTimeToFrames(item.timeline_range.start, project.settings.frame_rate),
                duration_frames: rationalTimeToFrames(item.timeline_range.duration, project.settings.frame_rate),
              }])));
              setActionError(null);
            }} disabled={dirtyDrafts.length === 0 && dirtyTimingDrafts.length === 0 || busy}>Discard edits</button>
            <button type="button" onClick={() => void applyEdits()} disabled={dirtyDrafts.length === 0 && dirtyTimingDrafts.length === 0 || !timingDraftsValid || busy}>{busy ? "Applying…" : "Apply edits"}</button>
            <button type="button" onClick={() => void mergeSelected()} disabled={!mergeReady || busy}>{busy ? "Applying…" : "Merge adjacent"}</button>
          </div>
        </footer>
      </section>
    </div>
  );
};
