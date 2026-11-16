import { FormEvent, useEffect, useMemo, useState } from "react";
import { EditPlanDraftReceipt, editorApi } from "../services/editorApi";
import { Project, TimelineItem, rationalTimeToFrames } from "../types/project";
import { useReviewContext } from "../hooks/useReviewContext";

interface EditPlanTrimDialogProps {
  isOpen: boolean;
  project: Project | null;
  item: TimelineItem | null;
  offlineMedia: Record<string, { last_known_path: string; permission_required: boolean }>;
  onClose: () => void;
  onApply: (receipt: EditPlanDraftReceipt) => Promise<void>;
}

export function EditPlanTrimDialog({
  isOpen,
  project,
  item,
  offlineMedia,
  onClose,
  onApply,
}: EditPlanTrimDialogProps) {
  const currentDurationFrames = useMemo(
    () => project && item ? rationalTimeToFrames(item.timeline_range.duration, project.settings.frame_rate) : 0,
    [item, project],
  );
  const selectedTrack = project?.tracks.find((candidate) =>
    candidate.items.some((candidateItem) => candidateItem.item_id === item?.item_id),
  );
  const [goal, setGoal] = useState("Shorten the selected clip from its tail");
  const [targetDurationFrames, setTargetDurationFrames] = useState(0);
  const [receipt, setReceipt] = useState<EditPlanDraftReceipt | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isBusy, setIsBusy] = useState(false);
  const context = useReviewContext(JSON.stringify([isOpen, project?.project_id, project?.revision, project?.assets, item, Object.keys(offlineMedia).sort()]), isOpen, () => setReceipt(null));
  const isEligible = !!project && !!item && !!selectedTrack && !selectedTrack.locked && item.kind === "Video" && !!item.asset_id &&
    !offlineMedia[item.asset_id] && currentDurationFrames > 1;

  useEffect(() => {
    if (!isOpen || !project || !item) return;
    setGoal(`Shorten ${item.name} from its tail`);
    setTargetDurationFrames(Math.max(1, rationalTimeToFrames(item.timeline_range.duration, project.settings.frame_rate) - 1));
    setReceipt(null);
    setError(null);
  }, [isOpen, item?.item_id, item?.timeline_range.duration.value, project?.project_id, project?.revision]);

  if (!isOpen || !project) return null;

  const handlePreview = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!item || !isEligible || targetDurationFrames < 1 || targetDurationFrames >= currentDurationFrames) {
      setError("Select an online video clip and enter a shorter duration in whole frames.");
      return;
    }
    setIsBusy(true);
    setError(null);
    setReceipt(null);
    const version = context.current.version;
    const stale = () => {
      if (context.current.version === version) return false;
      if (context.current.isOpen) setError("The project or media changed during preview. Generate a new preview.");
      return true;
    };
    try {
      const preview = await editorApi.previewEditPlanTrim(
        goal,
        item.item_id,
        targetDurationFrames,
      );
      if (stale()) return;
      if (!preview.validation.valid) {
        throw new Error(preview.validation.issues.join("; ") || "Trim plan validation failed");
      }
      setReceipt(preview);
    } catch (previewError) {
      if (!stale()) setError(previewError instanceof Error ? previewError.message : String(previewError));
    } finally {
      setIsBusy(false);
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

  const framesPerSecond = project.settings.frame_rate.numerator / project.settings.frame_rate.denominator;
  const durationLabel = (frames: number) => `${frames} frames (${(frames / framesPerSecond).toFixed(2)} s)`;

  return (
    <div className="dialog-backdrop" role="presentation" onMouseDown={() => !isBusy && onClose()}>
      <section className="modal-dialog edit-plan-dialog" role="dialog" aria-modal="true" aria-labelledby="trim-plan-title" onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-dialog-header">
          <div>
            <h2 id="trim-plan-title">Shorten selected clip</h2>
            <p>Review a tail trim, then apply it as one undoable edit.</p>
          </div>
          <button className="btn-action" type="button" onClick={onClose} disabled={isBusy} aria-label="Close">×</button>
        </div>

        <form onSubmit={handlePreview}>
          <label className="dialog-field">
            <span>Selected video clip</span>
            <input aria-label="Selected video clip" value={item?.name ?? "No clip selected"} disabled readOnly />
          </label>
          {item && <p className="draft-muted">Current duration: {durationLabel(currentDurationFrames)}. Start and source in-point stay fixed; the clip tail moves earlier.</p>}

          <label className="dialog-field">
            <span>Editing request</span>
            <input aria-label="Trim goal" value={goal} disabled={isBusy || !!receipt} onChange={(event) => { setGoal(event.target.value); setReceipt(null); }} />
          </label>

          <label className="dialog-field">
            <span>New duration (frames)</span>
            <input
              aria-label="New clip duration in frames"
              type="number"
              min={1}
              max={Math.max(1, currentDurationFrames - 1)}
              step={1}
              value={targetDurationFrames || ""}
              disabled={isBusy || !!receipt || !isEligible}
              onChange={(event) => {
                const parsed = Number(event.target.value);
                setTargetDurationFrames(Number.isSafeInteger(parsed) ? parsed : 0);
                setReceipt(null);
                setError(null);
              }}
            />
          </label>
          {!isEligible && <p className="draft-warning">Select one online video clip on an unlocked track before shortening it.</p>}

          {error && <p className="form-message form-message-error" role="alert">{error}</p>}

          {receipt && <section className="draft-review" aria-label="Trim plan review">
            <h3>Review clip trim</h3>
            <p>{item?.name}: {durationLabel(currentDurationFrames)} → {durationLabel(targetDurationFrames)}</p>
            <p>{receipt.validation.diff?.operationsEvaluated ?? receipt.plan.operations.length} validated operation · revision {receipt.plan.baseRevision} · {receipt.provider}</p>
            <p className="draft-muted">No project changes are made until Apply. Undo restores the original duration.</p>
            {receipt.plan.assumptions.map((assumption) => <p className="draft-muted" key={assumption}>{assumption}</p>)}
            {receipt.plan.warnings.map((warning) => <p className="draft-warning" key={warning}>{warning}</p>)}
          </section>}

          <div className="modal-dialog-actions">
            <button className="btn-action" type="button" onClick={onClose} disabled={isBusy}>Cancel</button>
            {receipt ? <>
              <button className="btn-action" type="button" onClick={() => { setReceipt(null); setError(null); }} disabled={isBusy}>Change duration</button>
              <button className="btn-action btn-primary" type="button" onClick={handleApply} disabled={isBusy}>Apply trim</button>
            </> : <button className="btn-action btn-primary" type="submit" disabled={isBusy || !isEligible || !goal.trim() || targetDurationFrames < 1 || targetDurationFrames >= currentDurationFrames}>{isBusy ? "Validating…" : "Preview trim"}</button>}
          </div>
        </form>
      </section>
    </div>
  );
}
