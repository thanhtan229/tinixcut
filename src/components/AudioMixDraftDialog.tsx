import { FormEvent, useEffect, useMemo, useState } from "react";
import { EditPlanDraftReceipt, editorApi } from "../services/editorApi";
import { Project, TimelineItem } from "../types/project";
import { CommandEnvelope } from "../types/command";
import { buildAudioMixPlan } from "./audioMixPlan";
import { formatGainDb } from "./audioGain";

interface Props {
  isOpen: boolean;
  project: Project | null;
  onClose: () => void;
  onApply: (receipt: EditPlanDraftReceipt) => Promise<void>;
}

function startSeconds(item: TimelineItem): number {
  const range = item.timeline_range;
  return (range.start.value * range.start.rate.denominator / range.start.rate.numerator);
}

export function AudioMixDraftDialog({ isOpen, project, onClose, onApply }: Props) {
  const [voiceIds, setVoiceIds] = useState<string[]>([]);
  const [musicIds, setMusicIds] = useState<string[]>([]);
  const [normalize, setNormalize] = useState(true);
  const [targetDbfs, setTargetDbfs] = useState(-18);
  const [duckDb, setDuckDb] = useState(-12);
  const [attack, setAttack] = useState(0.25);
  const [release, setRelease] = useState(0.75);
  const [receipt, setReceipt] = useState<EditPlanDraftReceipt | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const candidates = useMemo(() => project?.tracks.flatMap((track) => track.items
    .filter((item) => item.kind === "Audio" || (item.kind === "Video" && item.embedded_audio_enabled !== false))
    .map((item) => ({ item, trackName: track.name }))) ?? [], [project]);

  useEffect(() => {
    if (!isOpen || !project) return;
    setVoiceIds(candidates.filter(({ item }) => item.kind === "Video").map(({ item }) => item.item_id));
    setMusicIds(candidates.filter(({ item }) => item.kind === "Audio").map(({ item }) => item.item_id));
    setNormalize(true);
    setTargetDbfs(-18);
    setDuckDb(-12);
    setAttack(0.25);
    setRelease(0.75);
    setReceipt(null);
    setError(null);
  }, [isOpen, project?.project_id, project?.revision, candidates]);

  if (!isOpen || !project) return null;

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const selectedMusic = candidates.filter(({ item }) => musicIds.includes(item.item_id)).map(({ item }) => item);
      const levelsByAssetId: Record<string, Awaited<ReturnType<typeof editorApi.analyzeAudioLevels>>> = {};
      if (normalize) {
        for (const assetId of new Set(selectedMusic.map((item) => item.asset_id).filter((id): id is string => Boolean(id)))) {
          levelsByAssetId[assetId] = await editorApi.analyzeAudioLevels(assetId);
        }
      }
      const proposal = buildAudioMixPlan(project, {
        voiceItemIds: voiceIds,
        musicItemIds: musicIds,
        normalizeTargetDbfs: normalize ? targetDbfs : null,
        duckDb,
        attackSeconds: attack,
        releaseSeconds: release,
        levelsByAssetId,
      });
      const envelope: CommandEnvelope = {
        schema_version: project.schema_version,
        project_id: project.project_id,
        base_revision: project.revision,
        dry_run: true,
        transaction_id: `audio_mix_preview_${crypto.randomUUID()}`,
        operations: proposal.plan.operations.map(({ operationId, operation }) => ({ operation_id: operationId, operation })),
      };
      const diff = await editorApi.dryRun(envelope);
      if (diff.project_id !== project.project_id || diff.base_revision !== project.revision) throw new Error("The project changed while the audio plan was being checked. Build the review plan again.");
      setReceipt({ ...proposal, validation: { valid: true, issues: [], diff } });
    } catch (planError) {
      setReceipt(null);
      setError(planError instanceof Error ? planError.message : String(planError));
    } finally {
      setBusy(false);
    }
  };

  const apply = async () => {
    if (!receipt) return;
    setBusy(true);
    setError(null);
    try {
      await onApply(receipt);
      onClose();
    } catch (applyError) {
      setError(applyError instanceof Error ? applyError.message : String(applyError));
    } finally {
      setBusy(false);
    }
  };

  const operations = receipt?.plan.operations ?? [];
  const normalizedCount = operations.filter(({ operation }) => operation.type === "ClipVolumeSet").length;
  const keyframeCount = operations.filter(({ operation }) => operation.type === "ClipKeyframeAdd").length;
  const toggle = (setter: (value: string[] | ((current: string[]) => string[])) => void, id: string, checked: boolean) => {
    setReceipt(null);
    setter((current) => checked ? [...current, id] : current.filter((entry) => entry !== id));
  };

  return (
    <div className="dialog-backdrop" role="presentation" onMouseDown={() => !busy && onClose()}>
      <section className="modal-dialog" role="dialog" aria-modal="true" aria-labelledby="audio-mix-title" onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-dialog-header">
          <div><h2 id="audio-mix-title">Review audio mix plan</h2><p>Build deterministic music gain and voice ducking edits. Applying uses one undoable Command Service transaction.</p></div>
          <button className="btn-action" type="button" onClick={onClose} disabled={busy} aria-label="Close">×</button>
        </div>
        <form onSubmit={submit}>
          <fieldset className="draft-media-list" disabled={busy || !!receipt}>
            <legend>Voice clips</legend>
            {candidates.filter(({ item }) => item.kind === "Video").length === 0 ? <p className="draft-muted">No video clips with embedded audio are available.</p> : candidates.filter(({ item }) => item.kind === "Video").map(({ item, trackName }) => (
              <label className="draft-media-row" key={item.item_id}><input type="checkbox" checked={voiceIds.includes(item.item_id)} onChange={(event) => toggle(setVoiceIds, item.item_id, event.target.checked)} />
                <span>{item.name} · {trackName} · starts {startSeconds(item).toFixed(2)} s</span></label>
            ))}
          </fieldset>
          <fieldset className="draft-media-list" disabled={busy || !!receipt}>
            <legend>Music clips</legend>
            {candidates.filter(({ item }) => item.kind === "Audio").length === 0 ? <p className="draft-muted">No standalone audio clips are available.</p> : candidates.filter(({ item }) => item.kind === "Audio").map(({ item, trackName }) => (
              <label className="draft-media-row" key={item.item_id}><input type="checkbox" checked={musicIds.includes(item.item_id)} onChange={(event) => toggle(setMusicIds, item.item_id, event.target.checked)} />
                <span>{item.name} · {trackName} · starts {startSeconds(item).toFixed(2)} s</span></label>
            ))}
          </fieldset>
          <label className="draft-media-row"><input type="checkbox" checked={normalize} disabled={busy || !!receipt} onChange={(event) => { setNormalize(event.target.checked); setReceipt(null); }} /> Normalize music from source audio levels</label>
          {normalize && <label className="dialog-field"><span>Target mean level (dBFS)</span><input type="number" min="-60" max="0" step="1" value={targetDbfs} disabled={busy || !!receipt} onChange={(event) => { setTargetDbfs(Number(event.target.value)); setReceipt(null); }} /></label>}
          <label className="dialog-field"><span>Music duck under voice (dB)</span><input type="number" min="-48" max="0" step="1" value={duckDb} disabled={busy || !!receipt} onChange={(event) => { setDuckDb(Number(event.target.value)); setReceipt(null); }} /></label>
          <div className="dialog-field"><span>Attack / release (seconds)</span><div className="draft-subtitle-picker"><input aria-label="Ducking attack seconds" type="number" min="0" max="10" step="0.05" value={attack} disabled={busy || !!receipt} onChange={(event) => { setAttack(Number(event.target.value)); setReceipt(null); }} /><input aria-label="Ducking release seconds" type="number" min="0" max="20" step="0.05" value={release} disabled={busy || !!receipt} onChange={(event) => { setRelease(Number(event.target.value)); setReceipt(null); }} /></div></div>
          {error && <p className="inline-error" role="alert">{error}</p>}
          {receipt && <div className="draft-preview" role="status"><strong>{normalizedCount} gain edits · {keyframeCount} ducking keyframes</strong>
            <p>Project revision {receipt.plan.baseRevision}; application will fail if the project changes before Apply.</p>
            <p>Command Service dry-run checked {receipt.validation.diff?.operations_evaluated ?? operations.length} operations without changing the project.</p>
            {operations.filter(({ operation }) => operation.type === "ClipVolumeSet").map(({ operation }) => operation.type === "ClipVolumeSet" && <p key={operation.payload.item_id}>{candidates.find(({ item }) => item.item_id === operation.payload.item_id)?.item.name ?? operation.payload.item_id}: {formatGainDb(operation.payload.volume)} proposed clip gain</p>)}
            {musicIds.map((itemId) => <p key={`duck-${itemId}`}>{candidates.find(({ item }) => item.item_id === itemId)?.item.name ?? itemId}: {operations.filter(({ operation }) => operation.type === "ClipKeyframeAdd" && operation.payload.item_id === itemId).length} volume keyframes for ducking</p>)}
            {receipt.plan.assumptions.map((assumption) => <p className="draft-muted" key={assumption}>{assumption}</p>)}
            {receipt.plan.warnings.map((warning) => <p className="draft-muted" key={warning}>{warning}</p>)}
          </div>}
          <div className="modal-dialog-actions">
            {!receipt ? <button className="btn-primary" type="submit" disabled={busy}>{busy ? "Analyzing…" : "Build review plan"}</button> : <button className="btn-primary" type="button" onClick={() => void apply()} disabled={busy}>{busy ? "Applying…" : "Apply reviewed mix"}</button>}
            <button className="btn-action" type="button" onClick={onClose} disabled={busy}>Cancel</button>
          </div>
        </form>
      </section>
    </div>
  );
}
