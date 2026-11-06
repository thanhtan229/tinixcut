import { FormEvent, useEffect, useRef, useState } from "react";
import { Asset } from "../types/project";
import { editorApi, VisualSceneAnalysisJobReport, VisualSceneAnalysisReport } from "../services/editorApi";

interface Props {
  asset: Asset | null;
  onClose: () => void;
  onCreateDraft: (report: VisualSceneAnalysisReport) => void;
}

function timestamp(micros: number): string {
  const total = Math.max(0, Math.floor(micros / 1_000_000));
  return `${Math.floor(total / 60).toString().padStart(2, "0")}:${(total % 60).toString().padStart(2, "0")}`;
}

export function VisualSceneAnalysisDialog({ asset, onClose, onCreateDraft }: Props) {
  const [query, setQuery] = useState("");
  const [model, setModel] = useState("google/gemma-4-e4b");
  const [report, setReport] = useState<VisualSceneAnalysisJobReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const active = useRef(true);
  const running = report?.state === "queued" || report?.state === "running";

  useEffect(() => {
    active.current = true;
    return () => { active.current = false; };
  }, []);

  useEffect(() => {
    const jobId = report?.jobId;
    if (!jobId || !running) return;
    let stopped = false;
    const poll = async () => {
      try {
        const next = await editorApi.getVisualSceneAnalysisStatus(jobId);
        if (!stopped && active.current) setReport(next);
      } catch (pollError) {
        if (!stopped && active.current) setError(String(pollError));
      }
    };
    const timer = window.setInterval(() => void poll(), 900);
    return () => { stopped = true; window.clearInterval(timer); };
  }, [report?.jobId, running]);

  if (!asset) return null;

  const start = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setError(null);
    setReport(null);
    try {
      const initial = await editorApi.startVisualSceneAnalysis({
        assetIds: [asset.asset_id], query: query.trim(), model: model.trim(), maxScenes: 16,
      });
      setReport(initial);
    } catch (startError) {
      setError(startError instanceof Error ? startError.message : String(startError));
    }
  };

  const cancel = async () => {
    if (!report?.jobId) return;
    try {
      await editorApi.cancelVisualSceneAnalysis(report.jobId);
      setReport({ ...report, state: "cancelled", progress: 0, message: "Đã hủy phân tích.", report: null });
    } catch (cancelError) {
      setError(cancelError instanceof Error ? cancelError.message : String(cancelError));
    }
  };

  const close = async () => {
    if (running) await cancel();
    onClose();
  };

  return (
    <div className="dialog-backdrop" role="presentation">
      <section className="modal-dialog" role="dialog" aria-modal="true" aria-labelledby="visual-scene-title">
        <header className="modal-dialog-header">
          <div>
            <h2 id="visual-scene-title">Tìm cảnh trong video</h2>
            <p>{asset.name}</p>
          </div>
        </header>
        <form onSubmit={(event) => void start(event)}>
          <label className="dialog-field">
            Bạn muốn tìm cảnh nào?
            <input value={query} onChange={(event) => setQuery(event.target.value)} maxLength={4000} required disabled={running} placeholder="Ví dụ: cảnh bờ biển lúc hoàng hôn" />
          </label>
          <label className="dialog-field">
            Mô hình vision cục bộ
            <input value={model} onChange={(event) => setModel(event.target.value)} maxLength={160} required disabled={running} />
          </label>
          <p className="dialog-help">
            TinixCut chỉ gửi tối đa 16 ảnh khung hình đã thu nhỏ cùng câu tìm kiếm tới LM Studio trên máy này (127.0.0.1). Không gửi video, đường dẫn hay project. Kết quả là gợi ý và không sửa timeline.
          </p>
          {error && <p role="alert" className="dialog-error">{error}</p>}
          {report && <div role="status" className="dialog-review-summary">{report.message}</div>}
          {report?.report && (
            <div className="dialog-review-list">
              {report.report.truncated && <p>Một phần nội dung vượt giới hạn lấy mẫu.</p>}
              {report.report.scenes.map((scene) => (
                <article className="dialog-review-summary" key={scene.sceneId}>
                  <strong>{timestamp(scene.sampledAtMicros)} · {scene.caption}</strong>
                  <div>Caption và tag do model gợi ý từ một khung hình · Mức khớp {Math.round(scene.relevanceScore * 100)}% là điểm xếp hạng, không phải xác suất</div>
                  <div>{scene.tags.join(" · ")}</div>
                  <small>{scene.rationale}</small>
                </article>
              ))}
              {report.report.warnings.map((warning) => <p key={warning} className="dialog-help">{warning}</p>)}
            </div>
          )}
          <div className="modal-dialog-actions">
            {!running && <button type="submit" className="btn-primary" disabled={!query.trim() || !model.trim()}>Phân tích cục bộ</button>}
            {running && <button type="button" className="btn-action" onClick={() => void cancel()}>Hủy phân tích</button>}
            {report?.state === "completed" && report.report && <button type="button" className="btn-action btn-primary" onClick={() => onCreateDraft(report.report!)}>Tạo draft từ cảnh</button>}
            <button type="button" className="btn-action" onClick={() => void close()}>Đóng</button>
          </div>
        </form>
      </section>
    </div>
  );
}
