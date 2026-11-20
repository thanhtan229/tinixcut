import { FormEvent, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { EditPlanDraftReceipt, editorApi } from "../services/editorApi";
import { OperationEnvelope } from "../types/command";
import { Project } from "../types/project";
import { countPlannedVideoClips, formatDraftTime } from "./editPlanReview";

interface EditPlanDraftDialogProps {
  isOpen: boolean;
  project: Project | null;
  offlineMedia: Record<string, { last_known_path: string; permission_required: boolean }>;
  onClose: () => void;
  onApply: (receipt: EditPlanDraftReceipt) => Promise<void>;
}

export function EditPlanDraftDialog({ isOpen, project, offlineMedia, onClose, onApply }: EditPlanDraftDialogProps) {
  const [goal, setGoal] = useState("Create a short vertical video draft");
  const [durationSeconds, setDurationSeconds] = useState("30");
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [audioAssetId, setAudioAssetId] = useState("");
  const [subtitleFilePath, setSubtitleFilePath] = useState("");
  const [plannerProvider, setPlannerProvider] = useState<"deterministic-mock-v1" | "lm-studio-local">("deterministic-mock-v1");
  const [localModel, setLocalModel] = useState("google/gemma-4-e4b");
  const [receipt, setReceipt] = useState<EditPlanDraftReceipt | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isBusy, setIsBusy] = useState(false);
  const activeRequestId = useRef<string | null>(null);
  const contextKey = JSON.stringify([isOpen, project?.project_id, project?.revision, project?.assets, Object.keys(offlineMedia).sort()]);
  const context = useRef({ key: contextKey, version: 0, isOpen });
  const videoAssets = useMemo(() => project?.assets.filter((asset) => asset.media_kind === "Video") ?? [], [project]);
  const audioAssets = useMemo(() => project?.assets.filter((asset) => asset.media_kind === "Audio") ?? [], [project]);
  const orderedSelectedIds = selectedIds.filter(id => videoAssets.some(asset => asset.asset_id === id) && !offlineMedia[id]);
  const orderedVideoAssets = [...orderedSelectedIds.map(id => videoAssets.find(asset => asset.asset_id === id)!), ...videoAssets.filter(asset => !orderedSelectedIds.includes(asset.asset_id))];

  useEffect(() => {
    if (!isOpen) return;
    setGoal("Create a short vertical video draft");
    setDurationSeconds("30");
    setSelectedIds(videoAssets.filter((asset) => !offlineMedia[asset.asset_id]).map((asset) => asset.asset_id));
    setReceipt(null);
    setError(null);
    setAudioAssetId("");
    setSubtitleFilePath("");
  }, [isOpen, project?.project_id]);

  useEffect(() => {
    if (!isOpen) return;
    setSelectedIds(current => {
      const next=current.filter(id => videoAssets.some(asset => asset.asset_id === id) && !offlineMedia[id]);
      return next.length===current.length ? current : next;
    });
  }, [isOpen, videoAssets, offlineMedia]);

  useLayoutEffect(() => {
    if (context.current.key === contextKey) return;
    context.current = { key: contextKey, version: context.current.version + 1, isOpen };
    setReceipt(null);
    if (isOpen && activeRequestId.current) {
      setError("The project or media changed during generation. Generate a new preview when it finishes.");
    }
  }, [contextKey, isOpen]);

  if (!isOpen || !project) return null;

  const fps = project.settings.frame_rate.numerator / project.settings.frame_rate.denominator;
  const handleGenerate = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const seconds = Number(durationSeconds);
    const frames = Math.round(seconds * fps);
    if (!Number.isFinite(seconds) || seconds <= 0 || !Number.isSafeInteger(frames) || frames <= 0) {
      setError("Enter a positive draft duration that fits the project frame rate.");
      return;
    }
    setIsBusy(true);
    setError(null);
    setReceipt(null);
    const requestId = crypto.randomUUID();
    activeRequestId.current = requestId;
    const requestContextVersion = context.current.version;
    const requestIsStale = () => {
      if (context.current.version === requestContextVersion) return false;
      if (context.current.isOpen) setError("The project or media changed during generation. Generate a new preview.");
      return true;
    };
    try {
      const draft = await editorApi.previewEditPlanDraft(goal, orderedSelectedIds, frames, audioAssetId || null, subtitleFilePath || null, plannerProvider, localModel, requestId);
      if (requestIsStale()) return;
      if (!draft.validation.valid) throw new Error(draft.validation.issues.join("; ") || "Draft validation failed");
      setReceipt(draft);
    } catch (draftError) {
      if (!requestIsStale()) {
        setError(draftError instanceof Error ? draftError.message : String(draftError));
      }
    } finally {
      if (activeRequestId.current === requestId) activeRequestId.current = null;
      setIsBusy(false);
    }
  };

  const handleCancelGeneration = async () => {
    const requestId = activeRequestId.current;
    if (!requestId) return;
    try {
      const cancelled = await editorApi.cancelEditPlanDraft(requestId);
      setError(cancelled ? "Stopping local planner request…" : "The planner request has already finished.");
    } catch (cancelError) {
      setError(cancelError instanceof Error ? cancelError.message : String(cancelError));
    }
  };

  const handleChooseSubtitles = async () => {
    try {
      const selected = await open({
        title: "Choose draft subtitles",
        filters: [{ name: "Subtitle Files", extensions: ["srt", "vtt"] }],
        multiple: false,
      });
      if (typeof selected === "string") {
        setSubtitleFilePath(selected);
        setReceipt(null);
        setError(null);
      }
    } catch (selectionError) {
      setError(selectionError instanceof Error ? selectionError.message : String(selectionError));
    }
  };

  const handleApply = async () => {
    if (!receipt) return;
    setIsBusy(true);
    setError(null);
    try {
      await onApply(receipt);
      onClose();
    } catch (applyError) {
      setError(applyError instanceof Error ? applyError.message : String(applyError));
    } finally {
      setIsBusy(false);
    }
  };

  const planOperations: OperationEnvelope[] = receipt?.plan.operations.map((operation) => ({
    operation_id: operation.operationId,
    operation: operation.operation,
  })) ?? [];
  const plannedClips = countPlannedVideoClips(planOperations);
  const plannedVideos = planOperations.flatMap(({operation})=>operation.type === "ClipCreate" && operation.payload.kind === "Video" ? [operation.payload] : []);
  const plannedAudio = planOperations.flatMap(({operation})=>operation.type === "ClipCreate" && operation.payload.kind === "Audio" ? [operation.payload] : []);
  const evaluatedCount = receipt?.validation.diff?.operationsEvaluated ?? planOperations.length;

  return (
    <div className="dialog-backdrop" role="presentation" onMouseDown={() => !isBusy && onClose()}>
      <section className="modal-dialog edit-plan-dialog" role="dialog" aria-modal="true" aria-labelledby="edit-plan-title" onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-dialog-header">
          <div>
            <h2 id="edit-plan-title">Create video draft</h2>
            <p>Generate a proposed rough cut, review the changes, then apply them through the editor command service.</p>
          </div>
          <button className="btn-action" type="button" onClick={onClose} disabled={isBusy} aria-label="Close">×</button>
        </div>

        <form onSubmit={handleGenerate}>
          <label className="dialog-field">
            <span>Draft goal</span>
            <input value={goal} onChange={(event) => { setGoal(event.target.value); setReceipt(null); }} disabled={isBusy} />
          </label>
          <label className="dialog-field">
            <span>Planner</span>
            <select aria-label="Draft planner" value={plannerProvider} disabled={isBusy || !!receipt} onChange={(event) => { setPlannerProvider(event.target.value as typeof plannerProvider); setReceipt(null); }}>
              <option value="deterministic-mock-v1">Deterministic (no AI, offline)</option>
              <option value="lm-studio-local">Local AI (LM Studio)</option>
            </select>
          </label>
          {plannerProvider === "lm-studio-local" && <>
            <label className="dialog-field">
              <span>LM Studio model ID</span>
              <input aria-label="Local model ID" value={localModel} onChange={(event) => { setLocalModel(event.target.value); setReceipt(null); }} disabled={isBusy || !!receipt} />
            </label>
            <p className="draft-muted">Connects only to LM Studio at 127.0.0.1:1234. The model receives your goal and selected clip names/durations; video/audio bytes and file paths stay local. Start the LM Studio server and load the model first.</p>
          </>}
          <label className="dialog-field">
            <span>Target duration (seconds)</span>
            <input aria-label="Target duration in seconds" type="number" min="0.1" step="0.1" value={durationSeconds} onChange={(event) => { setDurationSeconds(event.target.value); setReceipt(null); }} disabled={isBusy} />
          </label>
          <fieldset className="draft-media-list" disabled={isBusy || !!receipt}>
            <legend>Video order</legend>
            {videoAssets.length === 0 ? <p className="draft-muted">No video assets in this project. Import video media first.</p> : orderedVideoAssets.map((asset) => {
              const offline = Boolean(offlineMedia[asset.asset_id]);
              return (
                <div className="draft-media-row" key={asset.asset_id}><label>
                  <input type="checkbox" checked={selectedIds.includes(asset.asset_id)} disabled={offline || isBusy || !!receipt} onChange={(event) => {
                    setReceipt(null);
                    setSelectedIds((current) => event.target.checked ? [...current, asset.asset_id] : current.filter((id) => id !== asset.asset_id));
                  }} />
                  <span>{asset.name}{offline ? " (offline)" : ""}</span></label>
                  {orderedSelectedIds.includes(asset.asset_id) && ([[-1,"up"],[1,"down"]] as const).map(([delta,label])=>{
                    const index=orderedSelectedIds.indexOf(asset.asset_id);
                    return <button key={label} type="button" className="btn-action" aria-label={`Move ${asset.name} ${label}`} disabled={index+delta<0 || index+delta>=orderedSelectedIds.length} onClick={()=>setSelectedIds(current=>{const next=[...current];const i=next.indexOf(asset.asset_id);const j=i+delta;if(i<0||j<0||j>=next.length)return current;[next[i],next[j]]=[next[j],next[i]];return next;})}>{label === "up" ? "↑" : "↓"}</button>;
                  })}
                </div>
              );
            })}
            <p className="draft-muted">Deterministic planner uses the selected order. Local AI may propose a different order; review the result. Offline assets cannot be selected.</p>
          </fieldset>

          <label className="dialog-field">
            <span>Music bed (optional)</span>
            <select aria-label="Draft music bed" value={audioAssetId} disabled={isBusy || !!receipt} onChange={(event) => { setAudioAssetId(event.target.value); setReceipt(null); }}>
              <option value="">No music bed</option>
              {audioAssets.map((asset) => <option key={asset.asset_id} value={asset.asset_id} disabled={Boolean(offlineMedia[asset.asset_id])}>{asset.name}{offlineMedia[asset.asset_id] ? " (offline)" : ""}</option>)}
            </select>
          </label>
          <div className="dialog-field">
            <span>Subtitles (optional, SRT/VTT)</span>
            <div className="draft-subtitle-picker">
              <button className="btn-action" type="button" onClick={handleChooseSubtitles} disabled={isBusy || !!receipt}>Choose subtitle file</button>
              <span className="draft-muted">{subtitleFilePath || "No subtitle file selected"}</span>
              {subtitleFilePath && <button className="btn-action" type="button" onClick={() => { setSubtitleFilePath(""); setReceipt(null); }} disabled={isBusy || !!receipt}>Clear</button>}
            </div>
          </div>

          {error && <p className="form-message form-message-error" role="alert">{error}</p>}

          {receipt && (
            <section className="draft-review" aria-label="Draft review">
              <h3>Review changes</h3>
              <p>{plannedClips} video clips · {evaluatedCount} validated operations</p>
              <h4>Planned video order</h4>
              <ol>{plannedVideos.map((clip,index)=><li key={clip.item_id ?? index}>{clip.name} · timeline start {formatDraftTime(clip.timeline_start)} · duration {formatDraftTime(clip.duration)}{clip.source_start && <> · source start {formatDraftTime(clip.source_start)}</>}</li>)}</ol>
              <p className="draft-muted">Seconds are rounded; frame values and their rates are exact. Check the plan before applying.</p>
              {plannedAudio.length > 0 && <><p>Includes audio clips.</p><h4>Planned audio</h4><ol aria-label="Planned audio clips">{plannedAudio.map((clip,index)=><li key={clip.item_id ?? index}>{clip.name} · timeline start {formatDraftTime(clip.timeline_start)} · duration {formatDraftTime(clip.duration)}{clip.source_start && <> · source start {formatDraftTime(clip.source_start)}</>}</li>)}</ol></>}
              {planOperations.filter((operation) => operation.operation.type === "ClipTextSet").length > 0 && <p>{planOperations.filter((operation) => operation.operation.type === "ClipTextSet").length} subtitle cues included.</p>}
              <p className="draft-muted">Provider: {receipt.provider}. Nothing has changed in the project yet.</p>
              <h4>Assumptions</h4>
              <ul>{receipt.plan.assumptions.map((item) => <li key={item}>{item}</li>)}</ul>
              {receipt.plan.warnings.map((warning) => <p className="draft-warning" key={warning}>{warning}</p>)}
            </section>
          )}

          <div className="modal-dialog-actions">
            <button className="btn-action" type="button" onClick={isBusy ? handleCancelGeneration : onClose} disabled={isBusy && plannerProvider !== "lm-studio-local"}>{isBusy ? "Stop generation" : "Cancel"}</button>
            {receipt ? (
              <>
                <button className="btn-action" type="button" onClick={() => { setReceipt(null); setError(null); }} disabled={isBusy}>Change selection</button>
                <button className="btn-action btn-primary" type="button" onClick={handleApply} disabled={isBusy}>Apply draft</button>
              </>
            ) : (
              <button className="btn-action btn-primary" type="submit" disabled={isBusy || orderedSelectedIds.length === 0 || !goal.trim()}>{isBusy ? "Validating…" : "Generate preview"}</button>
            )}
          </div>
        </form>
      </section>
    </div>
  );
}
