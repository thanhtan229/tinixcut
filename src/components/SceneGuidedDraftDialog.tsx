import { FormEvent, useEffect, useState } from "react";
import { EditPlanDraftReceipt, editorApi, SceneCanvasPreset, SceneSelectionSuggestion, SceneTitleTemplate, VisualSceneAnalysisReport } from "../services/editorApi";
import { OperationEnvelope } from "../types/command";
import { Project } from "../types/project";
import { countPlannedVideoClips } from "./editPlanReview";
import { applySceneRangeInputs, initialSceneRangeInputs, SceneRangeInputs } from "./sceneRangeDraft";

interface Props {
  report: VisualSceneAnalysisReport | null;
  project: Project | null;
  onClose: () => void;
  onApply: (receipt: EditPlanDraftReceipt) => Promise<void>;
}

function formatTime(micros: number): string {
  const seconds = Math.max(0, Math.floor(micros / 1_000_000));
  return `${Math.floor(seconds / 60).toString().padStart(2, "0")}:${(seconds % 60).toString().padStart(2, "0")}`;
}

function canvasDimensions(preset: SceneCanvasPreset): string {
  switch (preset) {
    case "landscape_16x9": return "1920 × 1080";
    case "portrait_9x16": return "1080 × 1920";
    case "square_1x1": return "1080 × 1080";
    case "portrait_4x5": return "1080 × 1350";
  }
}

export function SceneGuidedDraftDialog({ report, project, onClose, onApply }: Props) {
  const [goal, setGoal] = useState("");
  const [selectedSceneIds, setSelectedSceneIds] = useState<string[]>([]);
  const [receipt, setReceipt] = useState<EditPlanDraftReceipt | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [targetDurationSeconds, setTargetDurationSeconds] = useState("");
  const [localModel, setLocalModel] = useState("");
  const [suggestion, setSuggestion] = useState<SceneSelectionSuggestion | null>(null);
  const [rangeInputs, setRangeInputs] = useState<SceneRangeInputs>({});
  const [durationMode, setDurationMode] = useState<"advisory" | "exact">("advisory");
  const [mustKeepSceneIds, setMustKeepSceneIds] = useState<string[]>([]);
  const [mustOmitSceneIds, setMustOmitSceneIds] = useState<string[]>([]);
  const [allowedAssetIds, setAllowedAssetIds] = useState<string[]>([]);
  const [captionIntent, setCaptionIntent] = useState("");
  const [audioIntent, setAudioIntent] = useState("");
  const [suggestOpeningTitle, setSuggestOpeningTitle] = useState(false);
  const [targetCanvas, setTargetCanvas] = useState<SceneCanvasPreset | "">("");
  const [openingTitle, setOpeningTitle] = useState("");
  const [titleTemplate, setTitleTemplate] = useState<SceneTitleTemplate | "">("");

  useEffect(() => {
    if (!report) return;
    setGoal(`Create a rough cut for: ${report.query}`);
    setSelectedSceneIds(report.scenes.map((scene) => scene.sceneId));
    setTargetDurationSeconds("");
    setDurationMode("advisory");
    setMustKeepSceneIds([]);
    setMustOmitSceneIds([]);
    setAllowedAssetIds([...new Set(report.scenes.map((scene) => scene.assetId))]);
    setCaptionIntent("");
    setAudioIntent("");
    setSuggestOpeningTitle(false);
    setTargetCanvas("");
    setOpeningTitle("");
    setTitleTemplate("");
    setLocalModel(report.model);
    setRangeInputs(initialSceneRangeInputs(report));
    setSuggestion(null);
    setReceipt(null);
    setError(null);
  }, [report]);

  if (!report || !project) return null;

  const allowedAssets = new Set(allowedAssetIds);
  const reportAssets = [...new Map(report.scenes.map((scene) => [scene.assetId, scene.assetName])).entries()];

  const moveSelected = (sceneId: string, direction: -1 | 1) => {
    setReceipt(null);
    setSuggestion(null);
    setSelectedSceneIds((current) => {
      const index = current.indexOf(sceneId);
      const nextIndex = index + direction;
      if (index < 0 || nextIndex < 0 || nextIndex >= current.length) return current;
      const next = [...current];
      [next[index], next[nextIndex]] = [next[nextIndex], next[index]];
      return next;
    });
  };

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setBusy(true);
    setError(null);
    setReceipt(null);
    try {
      const targetSeconds = targetDurationSeconds.trim() ? Number(targetDurationSeconds) : null;
      if (targetSeconds !== null && (!Number.isFinite(targetSeconds) || targetSeconds < 1 || targetSeconds > 1800)) {
        throw new Error("Thời lượng mục tiêu phải từ 1 đến 1800 giây.");
      }
      if (durationMode === "exact" && targetSeconds === null) {
        throw new Error("Chế độ chính xác cần có thời lượng mục tiêu.");
      }
      if (!!openingTitle.trim() !== !!titleTemplate) {
        throw new Error("Hãy nhập tiêu đề và chọn template, hoặc để trống cả hai.");
      }
      if (new TextEncoder().encode(openingTitle.trim()).length > 240) {
        throw new Error("Tiêu đề mở đầu tối đa 240 byte UTF-8.");
      }
      const rangedReport = applySceneRangeInputs(report, selectedSceneIds, rangeInputs);
      const frameRate = project.settings.frame_rate;
      const targetFrames = targetSeconds === null ? null : Math.round(targetSeconds * frameRate.numerator / frameRate.denominator);
      const next = await editorApi.previewSceneGuidedEditPlan(goal, rangedReport, selectedSceneIds, targetFrames, durationMode === "exact", mustKeepSceneIds, mustOmitSceneIds, targetCanvas || null, openingTitle.trim() || null, titleTemplate || null, allowedAssetIds);
      if (!next.validation.valid) throw new Error(next.validation.issues.join("; ") || "Scene draft validation failed");
      setReceipt(next);
    } catch (previewError) {
      setError(previewError instanceof Error ? previewError.message : String(previewError));
    } finally {
      setBusy(false);
    }
  };

  const suggest = async () => {
    setBusy(true);
    setError(null);
    setReceipt(null);
    setSuggestion(null);
    try {
      const seconds = targetDurationSeconds.trim() ? Number(targetDurationSeconds) : null;
      if (seconds !== null && (!Number.isInteger(seconds) || seconds < 1 || seconds > 1800)) {
        throw new Error("Thời lượng mục tiêu phải từ 1 đến 1800 giây.");
      }
      const rangedReport = applySceneRangeInputs(report, selectedSceneIds, rangeInputs);
      const result = await editorApi.suggestVisualSceneSelection(goal, rangedReport, seconds, localModel, mustKeepSceneIds, mustOmitSceneIds, allowedAssetIds, captionIntent.trim() || null, audioIntent.trim() || null, suggestOpeningTitle);
      setSelectedSceneIds(result.orderedSceneIds);
      setSuggestion(result);
    } catch (suggestionError) {
      setError(suggestionError instanceof Error ? suggestionError.message : String(suggestionError));
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

  const operations: OperationEnvelope[] = receipt?.plan.operations.map((operation) => ({
    operation_id: operation.operationId,
    operation: operation.operation,
  })) ?? [];
  const selected = new Set(selectedSceneIds);
  let sceneDurationSeconds = 0;
  try {
    sceneDurationSeconds = applySceneRangeInputs(report, selectedSceneIds, rangeInputs).scenes
      .filter((scene) => selected.has(scene.sceneId))
      .reduce((total, scene) => total + (scene.sourceEndMicros - scene.sourceStartMicros) / 1_000_000, 0);
  } catch {
    // The preview action reports the precise invalid range while the user edits it.
  }

  return (
    <div className="dialog-backdrop" role="presentation" onMouseDown={() => !busy && onClose()}>
      <section className="modal-dialog edit-plan-dialog" role="dialog" aria-modal="true" aria-labelledby="scene-draft-title" onMouseDown={(event) => event.stopPropagation()}>
        <header className="modal-dialog-header">
          <div>
            <h2 id="scene-draft-title">Tạo draft từ cảnh đã chọn</h2>
            <p>{report.model} · nội dung hình ảnh là gợi ý cần xem lại</p>
          </div>
          <button className="btn-action" type="button" onClick={onClose} disabled={busy} aria-label="Đóng">×</button>
        </header>
        <form onSubmit={(event) => void submit(event)}>
          <label className="dialog-field">
            <span>Mục tiêu bản dựng</span>
            <input value={goal} onChange={(event) => { setGoal(event.target.value); setReceipt(null); setSuggestion(null); }} maxLength={4000} disabled={busy || !!receipt} required />
          </label>
          <div className="dialog-field-row">
            <label className="dialog-field">
              <span>Thời lượng mục tiêu (giây, tùy chọn)</span>
              <input type="number" min={1} max={1800} step={1} value={targetDurationSeconds} onChange={(event) => { setTargetDurationSeconds(event.target.value); setSuggestion(null); }} disabled={busy || !!receipt} placeholder="Ví dụ: 30" />
            </label>
            <label className="dialog-field">
              <span>Model LM Studio local</span>
              <input value={localModel} onChange={(event) => { setLocalModel(event.target.value); setSuggestion(null); }} maxLength={160} disabled={busy || !!receipt} required />
            </label>
          </div>
          <label className="scene-draft-row">
            <input aria-label="Suggest opening title" type="checkbox" checked={suggestOpeningTitle} disabled={busy || !!receipt} onChange={(event) => { setSuggestOpeningTitle(event.target.checked); setSuggestion(null); }} />
            <span>Nhờ model gợi ý tiêu đề mở đầu (chỉ gửi brief và cảnh được phép)</span>
          </label>
          <div className="dialog-field-row">
            <label className="dialog-field">
              <span>Ý định phụ đề (gợi ý cho chọn cảnh)</span>
              <textarea aria-label="Caption intent" value={captionIntent} maxLength={500} disabled={busy || !!receipt} onChange={(event) => { setCaptionIntent(event.target.value); setSuggestion(null); }} placeholder="Ví dụ: ưu tiên cảnh có lời thoại rõ; phụ đề tiếng Việt ngắn" />
            </label>
            <label className="dialog-field">
              <span>Ý định âm thanh (gợi ý cho chọn cảnh)</span>
              <textarea aria-label="Audio intent" value={audioIntent} maxLength={500} disabled={busy || !!receipt} onChange={(event) => { setAudioIntent(event.target.value); setSuggestion(null); }} placeholder="Ví dụ: nhịp yên bình, giữ cảnh có âm thanh môi trường" />
            </label>
          </div>
          <label className="dialog-field">
            <span>Chính sách thời lượng draft</span>
            <select aria-label="Duration mode" value={durationMode} disabled={busy || !!receipt} onChange={(event) => setDurationMode(event.target.value as "advisory" | "exact")}>
              <option value="advisory">Gợi ý — giữ nguyên In/Out đã chọn</option>
              <option value="exact">Chính xác — core chia frame và trim đuôi cảnh</option>
            </select>
          </label>
          <label className="dialog-field">
            <span>Tỷ lệ khung hình đầu ra</span>
            <select aria-label="Target canvas" value={targetCanvas} disabled={busy || !!receipt} onChange={(event) => { setTargetCanvas(event.target.value as SceneCanvasPreset | ""); setReceipt(null); }}>
              <option value="">Giữ nguyên canvas project</option>
              <option value="landscape_16x9">Ngang 16:9 (1920 × 1080)</option>
              <option value="portrait_9x16">Dọc 9:16 (1080 × 1920)</option>
              <option value="square_1x1">Vuông 1:1 (1080 × 1080)</option>
              <option value="portrait_4x5">Dọc 4:5 (1080 × 1350)</option>
            </select>
          </label>
          <div className="dialog-field-row">
            <label className="dialog-field">
              <span>Tiêu đề mở đầu (tùy chọn)</span>
              <input aria-label="Opening title" value={openingTitle} maxLength={240} disabled={busy || !!receipt} onChange={(event) => { setOpeningTitle(event.target.value); setReceipt(null); }} placeholder="Ví dụ: Một ngày ở Hội An" />
            </label>
            <label className="dialog-field">
              <span>Template chữ</span>
              <select aria-label="Title template" value={titleTemplate} disabled={busy || !!receipt} onChange={(event) => { setTitleTemplate(event.target.value as SceneTitleTemplate | ""); setReceipt(null); }}>
                <option value="">Không thêm tiêu đề</option>
                <option value="clean_lower_third">Clean lower third</option>
                <option value="bold_headline">Bold headline</option>
                <option value="center_pop">Center pop</option>
                <option value="subtitle_card">Subtitle card</option>
                <option value="quote_card">Quote card</option>
                <option value="call_to_action">Call to action</option>
              </select>
            </label>
          </div>
          <p className="draft-muted">
            Có thể tự chọn cảnh, chỉnh in/out bên trong khoảng đã phân tích hoặc nhờ model local đề xuất theo brief. Model chỉ nhận brief, ý định biên tập và caption/tag/khoảng thời gian của nguồn được cho phép; không nhận tên file, đường dẫn hay media. Ý định phụ đề/âm thanh chỉ hỗ trợ xếp hạng cảnh, không tạo phụ đề hoặc nhạc. Đề xuất là gợi ý cần xem lại. Chưa có thay đổi nào được áp dụng; Apply dùng Command Service và có thể Undo.
          </p>
          <fieldset className="draft-media-list source-policy-list" disabled={busy || !!receipt}>
            <legend>Nguồn media được phép dùng</legend>
            <p className="draft-muted">Chỉ cảnh từ các nguồn đã chọn mới được đưa vào đề xuất hoặc bản nháp.</p>
            {reportAssets.map(([assetId, assetName]) => (
              <label className="scene-draft-row" key={assetId}>
                <input
                  type="checkbox"
                  aria-label={`Allow source ${assetName}`}
                  checked={allowedAssets.has(assetId)}
                  onChange={(event) => {
                    const checked = event.target.checked;
                    const sceneIds = report.scenes.filter((scene) => scene.assetId === assetId).map((scene) => scene.sceneId);
                    setSuggestion(null);
                    setReceipt(null);
                    setAllowedAssetIds((current) => checked ? [...current, assetId] : current.filter((id) => id !== assetId));
                    if (!checked) {
                      setSelectedSceneIds((current) => current.filter((id) => !sceneIds.includes(id)));
                      setMustKeepSceneIds((current) => current.filter((id) => !sceneIds.includes(id)));
                      setMustOmitSceneIds((current) => current.filter((id) => !sceneIds.includes(id)));
                    }
                  }}
                />
                <span>{assetName}</span>
              </label>
            ))}
          </fieldset>
          <p className="draft-muted">Đánh dấu các cảnh bắt buộc giữ hoặc loại trừ để giới hạn đề xuất của model.</p>
          <div className="scene-draft-order" aria-label="Chọn và sắp xếp cảnh">
            {report.scenes.map((scene) => {
              const order = selectedSceneIds.indexOf(scene.sceneId);
              return (
                <article className="dialog-review-summary" key={scene.sceneId}>
                  <label className="scene-draft-row">
                      <input type="checkbox" checked={order >= 0} disabled={busy || !!receipt || !allowedAssets.has(scene.assetId)} onChange={(event) => {
                      setReceipt(null);
                        setSuggestion(null);
                      setSelectedSceneIds((current) => event.target.checked ? [...current, scene.sceneId] : current.filter((id) => id !== scene.sceneId));
                    }} />
                    <span>
                      <strong>{order >= 0 ? `${order + 1}. ` : ""}{formatTime(scene.sourceStartMicros)}–{formatTime(scene.sourceEndMicros)} · {scene.caption}</strong>
                      <small>{scene.assetName} · khoảng nguồn do TinixCut phân tích · caption/tag từ một khung hình của model · điểm khớp {Math.round(scene.relevanceScore * 100)}% là xếp hạng, không phải xác suất</small>
                    </span>
                  </label>
                  <div className="dialog-field-row scene-range-fields">
                    <label className="scene-draft-row">
                      <input type="checkbox" aria-label={`Must keep ${scene.sceneId}`} checked={mustKeepSceneIds.includes(scene.sceneId)} disabled={busy || !!receipt || !allowedAssets.has(scene.assetId) || mustOmitSceneIds.includes(scene.sceneId)} onChange={(event) => {
                        setSuggestion(null);
                        setMustKeepSceneIds((current) => event.target.checked ? [...current, scene.sceneId] : current.filter((id) => id !== scene.sceneId));
                      }} />
                      <span>Bắt buộc giữ</span>
                    </label>
                    <label className="scene-draft-row">
                      <input type="checkbox" aria-label={`Must omit ${scene.sceneId}`} checked={mustOmitSceneIds.includes(scene.sceneId)} disabled={busy || !!receipt || !allowedAssets.has(scene.assetId) || mustKeepSceneIds.includes(scene.sceneId)} onChange={(event) => {
                        setSuggestion(null);
                        setMustOmitSceneIds((current) => event.target.checked ? [...current, scene.sceneId] : current.filter((id) => id !== scene.sceneId));
                      }} />
                      <span>Bắt buộc loại</span>
                    </label>
                  </div>
                  {order >= 0 && <div className="scene-draft-move">
                    <button type="button" className="btn-action" aria-label={`Move ${scene.sceneId} up`} disabled={busy || !!receipt || order === 0} onClick={() => moveSelected(scene.sceneId, -1)}>↑</button>
                    <button type="button" className="btn-action" aria-label={`Move ${scene.sceneId} down`} disabled={busy || !!receipt || order === selectedSceneIds.length - 1} onClick={() => moveSelected(scene.sceneId, 1)}>↓</button>
                  </div>}
                  {order >= 0 && <div className="dialog-field-row scene-range-fields">
                    <label className="dialog-field">
                      <span>In (giây)</span>
                      <input aria-label={`In point ${scene.sceneId}`} type="number" min={scene.sourceStartMicros / 1_000_000} max={scene.sourceEndMicros / 1_000_000} step="0.001" value={rangeInputs[scene.sceneId]?.startSeconds ?? ""} disabled={busy || !!receipt} onChange={(event) => {
                        setSuggestion(null);
                        setReceipt(null);
                        setRangeInputs((current) => ({ ...current, [scene.sceneId]: { ...(current[scene.sceneId] ?? { startSeconds: "", endSeconds: "" }), startSeconds: event.target.value } }));
                      }} />
                    </label>
                    <label className="dialog-field">
                      <span>Out (giây)</span>
                      <input aria-label={`Out point ${scene.sceneId}`} type="number" min={scene.sourceStartMicros / 1_000_000} max={scene.sourceEndMicros / 1_000_000} step="0.001" value={rangeInputs[scene.sceneId]?.endSeconds ?? ""} disabled={busy || !!receipt} onChange={(event) => {
                        setSuggestion(null);
                        setReceipt(null);
                        setRangeInputs((current) => ({ ...current, [scene.sceneId]: { ...(current[scene.sceneId] ?? { startSeconds: "", endSeconds: "" }), endSeconds: event.target.value } }));
                      }} />
                    </label>
                  </div>}
                </article>
              );
            })}
          </div>
          <p className="draft-muted">{selectedSceneIds.length} cảnh · khoảng {sceneDurationSeconds.toFixed(1)} giây ở tốc độ bình thường.{targetDurationSeconds && ` Mục tiêu: ${targetDurationSeconds} giây.`}</p>
          {suggestion && <section className="draft-review" aria-label="Đề xuất chọn cảnh bằng AI">
            <h3>Đề xuất từ model local</h3>
            <p>{suggestion.provider} · {suggestion.model} · {suggestion.orderedSceneIds.length} cảnh · {(suggestion.selectedDurationMicros / 1_000_000).toFixed(1)} giây</p>
            <p>{suggestion.rationale}</p>
            {suggestion.openingTitleSuggestion && <div className="dialog-field-row">
              <p><strong>Gợi ý tiêu đề:</strong> {suggestion.openingTitleSuggestion} <span className="draft-muted">(do model tạo, cần kiểm tra)</span></p>
              <button className="btn-action" type="button" disabled={busy || !!receipt} onClick={() => { setOpeningTitle(suggestion.openingTitleSuggestion ?? ""); setReceipt(null); }}>Dùng tiêu đề này</button>
            </div>}
            {suggestion.targetDurationSeconds !== null && (() => {
              const actualSeconds = suggestion.selectedDurationMicros / 1_000_000;
              const deltaSeconds = actualSeconds - suggestion.targetDurationSeconds;
              const delta = `${deltaSeconds > 0 ? "+" : ""}${deltaSeconds.toFixed(1)}`;
              return <p className="draft-muted">Mục tiêu {suggestion.targetDurationSeconds} giây · chênh lệch {delta} giây. Các cảnh dùng nguyên khoảng thời gian nguồn nên mục tiêu chỉ mang tính gợi ý. Hãy xem lại trước khi tạo preview.</p>;
            })()}
          </section>}
          {error && <p className="form-message form-message-error" role="alert">{error}</p>}
          {receipt && <section className="draft-review" aria-label="Xem lại draft từ cảnh">
            <h3>Xem lại thay đổi</h3>
            <p>{countPlannedVideoClips(operations)} clip video · {receipt.validation.diff?.operations_evaluated ?? operations.length} thao tác đã xác thực · project rev {receipt.plan.baseRevision}</p>
            <p className="draft-muted">Provider: {receipt.provider}. Project chưa thay đổi.</p>
            {targetCanvas && <p>Canvas draft: {canvasDimensions(targetCanvas)} — thay đổi kích thước nằm trong cùng edit plan và chỉ áp dụng khi nhấn Apply.</p>}
            <h4>Giả định</h4>
            <ul>{receipt.plan.assumptions.map((item, index) => <li key={`${index}-${item}`}>{item}</li>)}</ul>
            {receipt.plan.warnings.map((warning) => <p className="draft-warning" key={warning}>{warning}</p>)}
          </section>}
          {report.warnings.map((warning) => <p className="draft-muted" key={warning}>{warning}</p>)}
          <div className="modal-dialog-actions">
            {receipt ? <>
              <button className="btn-action" type="button" disabled={busy} onClick={() => { setReceipt(null); setError(null); }}>Sửa lựa chọn</button>
              <button className="btn-action btn-primary" type="button" disabled={busy} onClick={() => void apply()}>Apply draft</button>
            </> : <>
              <button className="btn-action" type="button" onClick={() => void suggest()} disabled={busy || !goal.trim() || !localModel.trim()}>{busy ? "Đang xử lý…" : "Đề xuất chọn cảnh"}</button>
              <button className="btn-action btn-primary" type="submit" disabled={busy || !goal.trim() || selectedSceneIds.length === 0}>{busy ? "Đang xác thực…" : "Tạo preview"}</button>
            </>}
            <button className="btn-action" type="button" disabled={busy} onClick={onClose}>Hủy</button>
          </div>
        </form>
      </section>
    </div>
  );
}
