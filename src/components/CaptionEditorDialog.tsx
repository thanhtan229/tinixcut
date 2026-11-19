import React, { useEffect, useMemo, useRef, useState } from "react";
import { Project, TimelineItem, rationalTimeToFrames, rationalTimeToSeconds } from "../types/project";
import { buildCaptionTimingOperations, canMergeCaptions, canSplitCaptionAtFrame, captionMatchesSearch, collectCaptionRows, getCaptionTimingNotices, isValidCaptionDurationFrames, isValidCaptionStartFrame, type CaptionTimingDraft } from "./captionEditor";

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
  onApplyEdits: (textDrafts: CaptionDraft[], timingDrafts: CaptionTimingDraft[], expectedProject: { projectId: string; revision: number }) => Promise<unknown>;
  onMerge: (leftId: string, rightId: string) => Promise<unknown>;
  onSplit: (itemId: string, frame: number) => Promise<unknown>;
}

function formatTime(item: TimelineItem): string {
  const start = rationalTimeToSeconds(item.timeline_range.start);
  const end = start + rationalTimeToSeconds(item.timeline_range.duration);
  return `${start.toFixed(2)}–${end.toFixed(2)}s`;
}

export const CaptionEditorDialog: React.FC<CaptionEditorDialogProps> = ({
  project: incomingProject,
  playheadFrame,
  isOpen,
  onClose,
  onSeek,
  onApplyEdits,
  onMerge,
  onSplit,
}) => {
  const [project, setDraftProject] = useState(incomingProject);
  const captions = useMemo(() => collectCaptionRows(project), [project]);
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [timingDrafts, setTimingDrafts] = useState<Record<string, { start_frame: number; duration_frames: number }>>({});
  const [query, setQuery] = useState("");
  const [trackFilter, setTrackFilter] = useState("");
  const [timingFilter, setTimingFilter] = useState<"all" | "notices" | "gap" | "overlap" | "edited" | "invalid">("all");
  const [focusedCaptionId, setFocusedCaptionId] = useState<string | null>(null);
  useEffect(()=>{setTrackFilter("");},[project?.project_id]);
  const [replaceFrom, setReplaceFrom] = useState("");
  const [replaceTo, setReplaceTo] = useState("");
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [confirmClose, setConfirmClose] = useState(false);
  const closeFocusRef = useRef<HTMLElement | null>(null);
  const [mergeSelection, setMergeSelection] = useState<string[]>([]);
  const applySucceeded = useRef(false);
  const wasOpen = useRef(false);
  const resetDrafts = (nextProject: Project | null) => {
    const nextRows=collectCaptionRows(nextProject);
    const nextRate=nextProject?.settings.frame_rate ?? {numerator:30,denominator:1};
    setDraftProject(nextProject);
    setDrafts(Object.fromEntries(nextRows.map(({item})=>[item.item_id,item.text_content ?? ""])));
    setTimingDrafts(Object.fromEntries(nextRows.map(({item})=>[item.item_id,{start_frame:rationalTimeToFrames(item.timeline_range.start,nextRate),duration_frames:rationalTimeToFrames(item.timeline_range.duration,nextRate)}])));
  };

  useEffect(() => {
    const opening = isOpen && !wasOpen.current;
    wasOpen.current = isOpen;
    if (!isOpen || busy) return;
    const rate = project?.settings.frame_rate;
    const hasDrafts = !!rate && captions.some(({item}) => {
      const timing=timingDrafts[item.item_id];
      return (drafts[item.item_id] ?? item.text_content ?? "") !== (item.text_content ?? "") || !!timing && (timing.start_frame !== rationalTimeToFrames(item.timeline_range.start,rate) || timing.duration_frames !== rationalTimeToFrames(item.timeline_range.duration,rate));
    });
    const revisionChanged = incomingProject?.project_id !== project?.project_id || incomingProject?.revision !== project?.revision;
    const committedRevision = applySucceeded.current && revisionChanged;
    if (!opening && hasDrafts && !committedRevision) { applySucceeded.current = false; return; }
    applySucceeded.current = false;
    resetDrafts(incomingProject);
  }, [incomingProject?.project_id, incomingProject?.revision, isOpen, busy]);

  if (!isOpen || !project) return null;
  const stale = !incomingProject || incomingProject.project_id !== project.project_id || incomingProject.revision !== project.revision;

  const scopedCaptions = captions.filter(row=>!trackFilter || row.trackId===trackFilter);
  const timingReview = getCaptionTimingNotices(scopedCaptions, project.settings.frame_rate, timingDrafts);
  const filtered = scopedCaptions.filter(({ item }) => {
    const notice = timingReview.notices[item.item_id];
    const draft = timingDrafts[item.item_id];
    const edited = (drafts[item.item_id] ?? item.text_content ?? "") !== (item.text_content ?? "") || (draft !== undefined && (draft.start_frame !== rationalTimeToFrames(item.timeline_range.start, project.settings.frame_rate) || draft.duration_frames !== rationalTimeToFrames(item.timeline_range.duration, project.settings.frame_rate)));
    const matchesTiming = timingFilter === "all" || item.item_id === focusedCaptionId || (timingFilter === "invalid" ? timingReview.invalidItemIds.has(item.item_id) : timingFilter === "edited" ? edited : timingFilter === "notices" ? !!notice : notice?.kind === timingFilter);
    return matchesTiming && (item.item_id === focusedCaptionId || captionMatchesSearch(`${item.text_content ?? ""} ${drafts[item.item_id] ?? ""}`, query));
  });
  const invalidTimingTracks = [...timingReview.invalidTrackIds].map(id => project.tracks.find(track => track.track_id === id)?.name ?? id);
  const dirtyDrafts = captions
    .filter(({ item }) => (drafts[item.item_id] ?? item.text_content ?? "") !== (item.text_content ?? ""))
    .map(({ item }) => ({ item_id: item.item_id, text: drafts[item.item_id] ?? "" }));
  const dirtyTimingDrafts = captions.flatMap(({ item }) => {
    const draft = timingDrafts[item.item_id];
    const startFrame = draft?.start_frame ?? rationalTimeToFrames(item.timeline_range.start, project.settings.frame_rate);
    const durationFrames = draft?.duration_frames ?? rationalTimeToFrames(item.timeline_range.duration, project.settings.frame_rate);
    const originalStart = rationalTimeToFrames(item.timeline_range.start, project.settings.frame_rate);
    const originalDuration = rationalTimeToFrames(item.timeline_range.duration, project.settings.frame_rate);
    return startFrame === originalStart && durationFrames === originalDuration ? [] : [{ item_id: item.item_id, start_frame: startFrame, duration_frames: durationFrames }];
  });
  const pendingIds = new Set([...dirtyDrafts.map(draft => draft.item_id), ...dirtyTimingDrafts.map(draft => draft.item_id)]);
  const visibleIds = new Set(filtered.map(({ item }) => item.item_id));
  const hiddenPendingCount = [...pendingIds].filter(id => !visibleIds.has(id)).length;
  const closeEditor = () => {
    if (busy) return;
    if (pendingIds.size > 0) { closeFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null; setConfirmClose(true); return; }
    onClose();
  };
  const keepEditing = () => {
    setConfirmClose(false);
    requestAnimationFrame(() => closeFocusRef.current?.focus());
  };
  let timingDraftsValid = dirtyTimingDrafts.every(({ start_frame, duration_frames }) => isValidCaptionStartFrame(start_frame) && isValidCaptionDurationFrames(duration_frames));
  if (timingDraftsValid && dirtyTimingDrafts.length > 0) {
    try { buildCaptionTimingOperations(project, dirtyTimingDrafts); } catch { timingDraftsValid = false; }
  }
  const mergeReady = mergeSelection.length === 2
    && dirtyDrafts.length === 0
    && dirtyTimingDrafts.length === 0
    && !stale && canMergeCaptions(project, mergeSelection[0], mergeSelection[1]);

  const applyReplace = () => {
    if (!replaceFrom) return;
    setDrafts((current) => {
      const next = { ...current };
      for (const { item, locked } of scopedCaptions) {
        if (locked) continue;
        const text = next[item.item_id] ?? item.text_content ?? "";
        if (text.includes(replaceFrom)) next[item.item_id] = text.split(replaceFrom).join(replaceTo);
      }
      return next;
    });
  };

  const applyEdits = async () => {
    if (stale || dirtyDrafts.length === 0 && dirtyTimingDrafts.length === 0) return;
    setBusy(true);
    setActionError(null);
    try {
      await onApplyEdits(dirtyDrafts, dirtyTimingDrafts, {projectId:project.project_id,revision:project.revision});
      applySucceeded.current = true;
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
    <div role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) closeEditor(); }} style={{ position: "fixed", inset: 0, zIndex: 1200, background: "rgba(0,0,0,.68)", display: "grid", placeItems: "center", padding: 20 }}>
      <section role="dialog" aria-modal="true" aria-labelledby="caption-editor-title" style={{ width: "min(960px, 96vw)", maxHeight: "88vh", display: "flex", flexDirection: "column", gap: 12, padding: 18, borderRadius: 10, background: "var(--bg-primary)", color: "var(--text-primary)", border: "1px solid var(--border-color)" }}>
        <header style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
          <div>
            <h2 id="caption-editor-title" style={{ margin: 0, fontSize: 18 }}>Caption editor</h2>
            <span style={{ color: "var(--text-muted)", fontSize: 12 }}>{captions.length} text clips · edit captions before applying one undoable change</span>
          </div>
          <button type="button" onClick={closeEditor} disabled={busy} aria-label="Close caption editor">Close</button>
        </header>

        <div style={{ display: "flex", flexWrap: "wrap", alignItems: "center", gap: 8 }}>
          <select aria-label="Filter caption track" value={trackFilter} onChange={event=>{setTrackFilter(event.target.value);setMergeSelection([]);}}>
            <option value="">All tracks</option>
            {project.tracks.filter(track=>track.items.some(item=>item.kind==="Text")).map(track=><option key={track.track_id} value={track.track_id}>{track.name}</option>)}
          </select>
          <select aria-label="Filter caption timing" value={timingFilter} onChange={event=>{setTimingFilter(event.target.value as typeof timingFilter);setFocusedCaptionId(null);setMergeSelection([]);}}>
            <option value="all">All timing</option>
            <option value="notices">Gaps and overlaps</option>
            <option value="gap">Gaps only</option>
            <option value="overlap">Overlaps only</option>
            <option value="edited">Pending edits</option>
            <option value="invalid">Invalid timing</option>
          </select>
          <input aria-label="Search captions" placeholder="Find caption text" value={query} onChange={(event) => setQuery(event.currentTarget.value)} style={{ flex: "1 1 180px" }} />
          {(query !== "" || trackFilter !== "" || timingFilter !== "all") && <button type="button" onClick={() => {
            setQuery(""); setTrackFilter(""); setTimingFilter("all"); setFocusedCaptionId(null); setMergeSelection([]);
          }} title="Clear search, track and timing filters; keep pending edits">Clear filters</button>}
          <input aria-label="Replace text" placeholder="Find" value={replaceFrom} onChange={(event) => setReplaceFrom(event.currentTarget.value)} />
          <input aria-label="Replace with" placeholder="Replace with" value={replaceTo} onChange={(event) => setReplaceTo(event.currentTarget.value)} />
          <button type="button" onClick={applyReplace} disabled={!replaceFrom || busy} title="Replaces matching text on all unlocked captions in the selected track scope, including rows hidden by search or timing filters">{trackFilter ? "Replace in track" : "Replace all"}</button>
        </div>

        {actionError && <p role="alert" style={{ color: "var(--danger, #f66)" }}>{actionError}</p>}
        {stale && <p role="alert">Project changed while editing. Your draft is preserved for copying; discard edits to load the current project before applying.</p>}
        {confirmClose && <div role="alertdialog" aria-modal="true" aria-labelledby="caption-discard-title" onKeyDown={event => {
          if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); keepEditing(); }
          if (event.key === "Tab") { const buttons = event.currentTarget.querySelectorAll<HTMLButtonElement>("button"); if (buttons.length === 2) { event.preventDefault(); (document.activeElement === buttons[0] ? buttons[1] : buttons[0]).focus(); } }
        }} style={{position:"absolute",inset:0,zIndex:1,display:"flex",flexDirection:"column",alignItems:"center",justifyContent:"center",padding:20,background:"var(--bg-primary)"}}>
          <p id="caption-discard-title">Discard pending edits in {pendingIds.size} caption{pendingIds.size === 1 ? "" : "s"}?</p>
          <button type="button" autoFocus onClick={keepEditing}>Keep editing</button>{" "}
          <button type="button" onClick={() => { setConfirmClose(false); resetDrafts(incomingProject); onClose(); }}>Discard and close</button>
        </div>}
        <p style={{fontSize:12,color:"var(--text-muted)"}}>{filtered.length} shown · Replace affects {trackFilter ? "the selected track" : "all tracks"}, including captions hidden by search or timing filters. Apply includes pending edits from every track.</p>
        {hiddenPendingCount > 0 && <p aria-live="polite" style={{fontSize:12,color:"var(--text-warning, #f59e0b)"}}>{hiddenPendingCount} {hiddenPendingCount === 1 ? "caption has" : "captions have"} pending edits hidden by filters.</p>}
        <p style={{fontSize:12,color:"var(--text-muted)"}}>Timing notices compare captions within each track, including rows hidden by search and valid pending edits. Gaps can be intentional.</p>
        {invalidTimingTracks.length > 0 && <p role="status" style={{fontSize:12,color:"var(--text-warning, #f59e0b)"}}>Timing notices unavailable for {invalidTimingTracks.join(", ")}: enter valid start and duration frames for every caption on that track. Select Invalid timing to find invalid entries.</p>}

        <div style={{ overflow: "auto", display: "flex", flexDirection: "column", gap: 8 }}>
          {filtered.map(({ item, trackName, locked }) => {
            const startFrame = rationalTimeToFrames(item.timeline_range.start, project.settings.frame_rate);
            const draftStart = timingDrafts[item.item_id]?.start_frame ?? startFrame;
            const originalDuration = rationalTimeToFrames(item.timeline_range.duration, project.settings.frame_rate);
            const draftDuration = timingDrafts[item.item_id]?.duration_frames ?? originalDuration;
            const validDraftStart = isValidCaptionStartFrame(draftStart);
            const lastFrame = draftStart + draftDuration - 1;
            const validLastFrame = validDraftStart && isValidCaptionDurationFrames(draftDuration) && Number.isSafeInteger(draftStart + draftDuration);
            const hasDraftTiming = draftStart !== startFrame || draftDuration !== originalDuration;
            const canSplit = canSplitCaptionAtFrame(item, playheadFrame, project.settings.frame_rate);
            const hasPendingEdits = dirtyDrafts.some(draft => draft.item_id === item.item_id) || dirtyTimingDrafts.some(draft => draft.item_id === item.item_id);
            return (
              <article key={item.item_id} onFocusCapture={() => setFocusedCaptionId(item.item_id)} onBlurCapture={(event) => { if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setFocusedCaptionId(null); }} style={{ display: "grid", gridTemplateColumns: "24px 90px 78px 78px 130px minmax(220px,1fr) auto", gap: 8, alignItems: "start", padding: 8, border: "1px solid var(--border-color)", borderRadius: 6 }}>
                <input type="checkbox" aria-label={`Select caption ${item.item_id} to merge`} checked={mergeSelection.includes(item.item_id)} disabled={locked || busy || (!mergeSelection.includes(item.item_id) && mergeSelection.length >= 2)} onChange={(event) => {
                  const checked = event.currentTarget.checked;
                  setMergeSelection((current) => checked ? [...current, item.item_id] : current.filter((id) => id !== item.item_id));
                }} />
                <div style={{display:"flex",flexDirection:"column",gap:4}}><button type="button" disabled={stale || !validDraftStart} onClick={() => onSeek(draftStart)} title={!validDraftStart ? "Enter a valid start frame to seek" : draftStart !== startFrame ? "Seek to draft caption start; edits are not applied yet" : "Seek to caption start"} style={{ textAlign: "left" }}>{formatTime(item)}{draftStart !== startFrame && validDraftStart && <><br />Draft: {draftStart}f</>}</button>
                <button type="button" aria-label={`Go to last frame of caption ${item.item_id}`} disabled={stale || !validLastFrame} onClick={() => onSeek(lastFrame)} title={!validLastFrame ? "Enter valid start and duration frames to seek" : hasDraftTiming ? "Seek to draft caption last frame; edits are not applied yet" : "Seek to caption last frame"}>Last frame</button></div>
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
                <span style={{ color: "var(--text-muted)", fontSize: 11 }}>{trackName}{locked ? " · locked" : ""}{timingReview.notices[item.item_id] && <><br /><span style={{color:"var(--text-warning, #f59e0b)"}}>{timingReview.notices[item.item_id].kind === "gap" ? "Gap before" : "Overlap with earlier captions"}: {timingReview.notices[item.item_id].frames} frames</span></>}</span>
                <textarea aria-label={`Caption text ${formatTime(item)}`} rows={2} disabled={locked} value={drafts[item.item_id] ?? item.text_content ?? ""} onChange={(event) => {
                  const value = event.currentTarget.value;
                  setDrafts((current) => ({ ...current, [item.item_id]: value }));
                }} />
                <div style={{display:"flex", flexDirection:"column", gap:4}}><button type="button" disabled={stale || locked || !canSplit || busy || dirtyDrafts.length > 0 || dirtyTimingDrafts.length > 0} onClick={() => void splitAtPlayhead(item.item_id)} title={locked ? "Unlock this track before editing" : dirtyDrafts.length > 0 || dirtyTimingDrafts.length > 0 ? "Apply caption edits before splitting" : "Split this caption at the playhead"}>Split here</button>
                <button type="button" aria-label={`Discard caption edits ${item.item_id}`} disabled={locked || busy || !hasPendingEdits} onClick={() => {
                  setDrafts(current => ({...current, [item.item_id]: item.text_content ?? ""}));
                  setTimingDrafts(current => ({...current, [item.item_id]: {start_frame: startFrame, duration_frames: rationalTimeToFrames(item.timeline_range.duration, project.settings.frame_rate)}}));
                }}>Discard caption edits</button></div>
              </article>
            );
          })}
          {filtered.length === 0 && <div style={{ padding: 24, textAlign: "center", color: "var(--text-muted)" }}>No captions match the current filters.</div>}
        </div>

        <footer style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 8 }}>
          <span style={{ color: "var(--text-muted)", fontSize: 12 }}>{dirtyDrafts.length} text edits · {dirtyTimingDrafts.length} timing edits</span>
          <div style={{ display: "flex", gap: 8 }}>
            <button type="button" onClick={() => {
              resetDrafts(incomingProject);
              setActionError(null);
            }} disabled={!stale && dirtyDrafts.length === 0 && dirtyTimingDrafts.length === 0 || busy}>Discard edits</button>
            <button type="button" onClick={() => void applyEdits()} disabled={stale || dirtyDrafts.length === 0 && dirtyTimingDrafts.length === 0 || !timingDraftsValid || busy}>{busy ? "Applying…" : "Apply edits"}</button>
            <button type="button" onClick={() => void mergeSelected()} disabled={!mergeReady || busy}>{busy ? "Applying…" : "Merge adjacent"}</button>
          </div>
        </footer>
      </section>
    </div>
  );
};
