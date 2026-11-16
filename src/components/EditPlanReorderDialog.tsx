import { FormEvent, useEffect, useMemo, useRef, useState } from "react";
import { EditPlanDraftReceipt, editorApi } from "../services/editorApi";
import { Project, TimelineItem, rationalTimeToFrames } from "../types/project";
import { getOrderedClipIds } from "./editPlanReview";
import { useReviewContext } from "../hooks/useReviewContext";

interface EditPlanReorderDialogProps {
  isOpen: boolean;
  project: Project | null;
  offlineMedia: Record<string, { last_known_path: string; permission_required: boolean }>;
  selectedItemIds: string[];
  onClose: () => void;
  onApply: (receipt: EditPlanDraftReceipt) => Promise<void>;
}

function trackItemsInTimelineOrder(items: TimelineItem[], project: Project): TimelineItem[] {
  return [...items].sort((left, right) =>
    rationalTimeToFrames(left.timeline_range.start, project.settings.frame_rate) -
    rationalTimeToFrames(right.timeline_range.start, project.settings.frame_rate),
  );
}

export function EditPlanReorderDialog({
  isOpen,
  project,
  offlineMedia,
  selectedItemIds,
  onClose,
  onApply,
}: EditPlanReorderDialogProps) {
  const [goal, setGoal] = useState("Reorder the clips to improve the story flow");
  const [trackId, setTrackId] = useState("");
  const [orderedIds, setOrderedIds] = useState<string[]>([]);
  const [plannerProvider, setPlannerProvider] = useState<"caller-order-v1" | "lm-studio-local">("caller-order-v1");
  const [localModel, setLocalModel] = useState("google/gemma-4-e4b");
  const [receipt, setReceipt] = useState<EditPlanDraftReceipt | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isBusy, setIsBusy] = useState(false);
  const activeRequestId = useRef<string | null>(null);
  const context = useReviewContext(JSON.stringify([isOpen, project?.project_id, project?.revision, project?.assets, Object.keys(offlineMedia).sort(), selectedItemIds]), isOpen, () => setReceipt(null));

  const candidateTracks = useMemo(() => {
    if (!project) return [];
    return project.tracks.filter((track) => {
      if (track.kind !== "Video" || track.locked || track.items.length < 2 || track.items.length > 100) return false;
      if (track.items.some((item) => item.kind !== "Video" || !item.asset_id || offlineMedia[item.asset_id] || item.transition)) return false;
      if ((track.transitions?.length ?? 0) > 0 || (track.audio_transitions?.length ?? 0) > 0) return false;
      const ordered = trackItemsInTimelineOrder(track.items, project);
      let expectedStart = rationalTimeToFrames(ordered[0].timeline_range.start, project.settings.frame_rate);
      for (const item of ordered) {
        const start = rationalTimeToFrames(item.timeline_range.start, project.settings.frame_rate);
        const duration = rationalTimeToFrames(item.timeline_range.duration, project.settings.frame_rate);
        if (start !== expectedStart || duration <= 0) return false;
        expectedStart += duration;
      }
      return true;
    });
  }, [project, offlineMedia]);

  const track = candidateTracks.find((candidate) => candidate.track_id === trackId) ?? null;
  const timelineItems = useMemo(
    () => track && project ? trackItemsInTimelineOrder(track.items, project) : [],
    [track, project],
  );
  const clipIds = timelineItems.map((item) => item.item_id);
  const selectedItemsKey = selectedItemIds.join("|");
  const effectiveOrder = receipt
    ? getOrderedClipIds(
        receipt.plan.operations.map((operation) => ({ operation_id: operation.operationId, operation: operation.operation })),
        clipIds,
        trackId,
      )
    : orderedIds;
  const itemById = new Map(timelineItems.map((item) => [item.item_id, item]));
  const displayedOrder = receipt ? effectiveOrder : orderedIds;

  useEffect(() => {
    if (!isOpen || !project) return;
    const selectedTrack = project.tracks.find((candidate) => candidate.items.some((item) => selectedItemIds.includes(item.item_id)));
    const nextTrack = candidateTracks.find((candidate) => candidate.track_id === selectedTrack?.track_id) ?? candidateTracks[0];
    setGoal("Reorder the clips to improve the story flow");
    setPlannerProvider("caller-order-v1");
    setReceipt(null);
    setError(null);
    setTrackId(nextTrack?.track_id ?? "");
    setOrderedIds(nextTrack ? trackItemsInTimelineOrder(nextTrack.items, project).map((item) => item.item_id) : []);
  }, [isOpen, project?.project_id, project?.revision, candidateTracks, selectedItemsKey, project]);

  if (!isOpen || !project) return null;

  const handleGenerate = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!track || clipIds.length < 2) {
      setError("Choose a supported video track with at least two clips.");
      return;
    }
    setIsBusy(true);
    setError(null);
    setReceipt(null);
    const requestId = crypto.randomUUID();
    activeRequestId.current = requestId;
    const version = context.current.version;
    const stale = () => {
      if (context.current.version === version) return false;
      if (context.current.isOpen) setError("The project or media changed during preview. Generate a new preview.");
      return true;
    };
    try {
      const preview = await editorApi.previewEditPlanReorder(
        goal,
        track.track_id,
        clipIds,
        plannerProvider === "caller-order-v1" ? orderedIds : clipIds,
        plannerProvider,
        localModel,
        requestId,
      );
      if (stale()) return;
      if (!preview.validation.valid) throw new Error(preview.validation.issues.join("; ") || "Reorder plan validation failed");
      setReceipt(preview);
    } catch (previewError) {
      if (!stale()) setError(previewError instanceof Error ? previewError.message : String(previewError));
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

  const moveClip = (itemId: string, offset: -1 | 1) => {
    setReceipt(null);
    setError(null);
    setOrderedIds((current) => {
      const index = current.indexOf(itemId);
      const target = index + offset;
      if (index < 0 || target < 0 || target >= current.length) return current;
      const next = [...current];
      [next[index], next[target]] = [next[target], next[index]];
      return next;
    });
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

  const operationCount = receipt?.validation.diff?.operationsEvaluated ?? receipt?.plan.operations.length ?? 0;

  return (
    <div className="dialog-backdrop" role="presentation" onMouseDown={() => !isBusy && onClose()}>
      <section className="modal-dialog edit-plan-dialog" role="dialog" aria-modal="true" aria-labelledby="reorder-plan-title" onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-dialog-header">
          <div>
            <h2 id="reorder-plan-title">Reorder video clips</h2>
            <p>Preview a clip order, then apply it as one undoable edit.</p>
          </div>
          <button className="btn-action" type="button" onClick={onClose} disabled={isBusy} aria-label="Close">×</button>
        </div>

        <form onSubmit={handleGenerate}>
          <label className="dialog-field">
            <span>Video track</span>
            <select aria-label="Video track to reorder" value={trackId} disabled={isBusy || !!receipt} onChange={(event) => {
              const nextTrack = candidateTracks.find((candidate) => candidate.track_id === event.target.value);
              setTrackId(event.target.value);
              setOrderedIds(nextTrack && project ? trackItemsInTimelineOrder(nextTrack.items, project).map((item) => item.item_id) : []);
              setReceipt(null);
              setError(null);
            }}>
              {candidateTracks.length === 0 ? <option value="">No eligible tracks</option> : candidateTracks.map((candidate) => <option value={candidate.track_id} key={candidate.track_id}>{candidate.name} ({candidate.items.length} clips)</option>)}
            </select>
          </label>

          <label className="dialog-field">
            <span>Editing request</span>
            <input aria-label="Reorder goal" value={goal} disabled={isBusy || !!receipt} onChange={(event) => { setGoal(event.target.value); setReceipt(null); }} />
          </label>

          <label className="dialog-field">
            <span>Planner</span>
            <select aria-label="Reorder planner" value={plannerProvider} disabled={isBusy || !!receipt} onChange={(event) => { setPlannerProvider(event.target.value as typeof plannerProvider); setReceipt(null); }}>
              <option value="caller-order-v1">Manual order (offline)</option>
              <option value="lm-studio-local">Local AI (LM Studio)</option>
            </select>
          </label>
          {plannerProvider === "lm-studio-local" && <>
            <label className="dialog-field">
              <span>LM Studio model ID</span>
              <input aria-label="Local reorder model ID" value={localModel} disabled={isBusy || !!receipt} onChange={(event) => { setLocalModel(event.target.value); setReceipt(null); }} />
            </label>
            <p className="draft-muted">The local model receives clip names, IDs, durations and this request. It does not inspect image or audio content. Start LM Studio at 127.0.0.1:1234 first.</p>
          </>}

          <fieldset className="draft-media-list" disabled={isBusy || !!receipt || plannerProvider === "lm-studio-local"}>
            <legend>{plannerProvider === "lm-studio-local" ? "Current clip order" : "Requested clip order"}</legend>
            {displayedOrder.map((itemId, index) => {
              const item = itemById.get(itemId);
              if (!item) return null;
              return (
              <div className="draft-media-row" key={item.item_id}>
                <span>{index + 1}. {item.name}</span>
                {plannerProvider === "caller-order-v1" && <span className="draft-order-controls">
                  <button className="btn-action" type="button" aria-label={`Move ${item.name} up`} disabled={index === 0 || !!receipt} onClick={() => moveClip(item.item_id, -1)}>↑</button>
                  <button className="btn-action" type="button" aria-label={`Move ${item.name} down`} disabled={index === timelineItems.length - 1 || !!receipt} onClick={() => moveClip(item.item_id, 1)}>↓</button>
                </span>}
              </div>
              );
            })}
            {candidateTracks.length === 0 && <p className="draft-muted">Use an unlocked video track with 2–100 online video clips, no transitions, and no gaps.</p>}
          </fieldset>

          {error && <p className="form-message form-message-error" role="alert">{error}</p>}

          {receipt && <section className="draft-review" aria-label="Reorder plan review">
            <h3>Review new order</h3>
            <p>{effectiveOrder.length} clips · {operationCount} validated operations · {receipt.provider}</p>
            <ol>{effectiveOrder.map((itemId) => <li key={itemId}>{itemById.get(itemId)?.name ?? itemId}</li>)}</ol>
            <p className="draft-muted">The project is unchanged until you apply. The preview is tied to revision {receipt.plan.baseRevision}.</p>
            {receipt.plan.assumptions.map((assumption) => <p className="draft-muted" key={assumption}>{assumption}</p>)}
            {receipt.plan.warnings.map((warning) => <p className="draft-warning" key={warning}>{warning}</p>)}
          </section>}

          <div className="modal-dialog-actions">
            <button className="btn-action" type="button" onClick={isBusy ? handleCancelGeneration : onClose} disabled={isBusy && plannerProvider !== "lm-studio-local"}>{isBusy ? "Stop generation" : "Cancel"}</button>
            {receipt ? <>
              <button className="btn-action" type="button" onClick={() => { setReceipt(null); setError(null); }} disabled={isBusy}>Change order</button>
              <button className="btn-action btn-primary" type="button" onClick={handleApply} disabled={isBusy}>Apply reorder</button>
            </> : <button className="btn-action btn-primary" type="submit" disabled={isBusy || !track || clipIds.length < 2 || !goal.trim() || (plannerProvider === "caller-order-v1" && orderedIds.every((id, index) => id === clipIds[index]))}>{isBusy ? "Validating…" : "Preview order"}</button>}
          </div>
        </form>
      </section>
    </div>
  );
}
