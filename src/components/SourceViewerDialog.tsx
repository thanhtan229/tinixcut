import { useEffect, useMemo, useRef, useState, type RefObject } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Asset } from "../types/project";

interface SourceViewerDialogProps {
  asset: Asset | null;
  initialSeekSeconds?: number;
  onClose: () => void;
  onAddRange: (asset: Asset, sourceInSeconds: number, sourceOutSeconds: number) => void;
}

function assetDurationSeconds(asset: Asset): number {
  if (!asset.duration || asset.duration.rate.numerator <= 0) return 5;
  return Math.max(0.001, asset.duration.value * asset.duration.rate.denominator / asset.duration.rate.numerator);
}

export function SourceViewerDialog({ asset, initialSeekSeconds = 0, onClose, onAddRange }: SourceViewerDialogProps) {
  const duration = useMemo(() => asset ? assetDurationSeconds(asset) : 0, [asset]);
  const [sourceIn, setSourceIn] = useState(0);
  const [sourceOut, setSourceOut] = useState(duration);
  const [playhead, setPlayhead] = useState(0);
  const mediaRef = useRef<HTMLVideoElement | HTMLAudioElement>(null);

  useEffect(() => {
    setSourceIn(0);
    setSourceOut(duration);
    const seekTo = Math.min(Math.max(initialSeekSeconds, 0), duration);
    setPlayhead(seekTo);
  }, [asset?.asset_id, duration, initialSeekSeconds]);

  if (!asset) return null;
  const sourceUrl = convertFileSrc(asset.path);
  const canAdd = Number.isFinite(sourceIn) && Number.isFinite(sourceOut) && sourceIn >= 0 && sourceOut > sourceIn && sourceOut <= duration;

  return (
    <div className="dialog-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <section className="modal-dialog source-viewer-dialog" role="dialog" aria-modal="true" aria-labelledby="source-viewer-title">
        <header className="modal-dialog-header">
          <div>
            <h2 id="source-viewer-title">Source viewer</h2>
            <p>{asset.name} · {asset.media_kind}</p>
          </div>
          <button type="button" className="btn-action" onClick={onClose}>Close</button>
        </header>
        <div className="source-viewer-preview">
          {asset.media_kind === "Video" ? (
            <video ref={mediaRef as RefObject<HTMLVideoElement>} src={sourceUrl} controls preload="metadata" onLoadedMetadata={(event) => {
              event.currentTarget.currentTime = Math.min(Math.max(initialSeekSeconds, 0), duration);
            }} onTimeUpdate={(event) => {
              setPlayhead(event.currentTarget.currentTime);
            }} />
          ) : asset.media_kind === "Audio" ? (
            <audio ref={mediaRef as RefObject<HTMLAudioElement>} src={sourceUrl} controls preload="metadata" onLoadedMetadata={(event) => {
              event.currentTarget.currentTime = Math.min(Math.max(initialSeekSeconds, 0), duration);
            }} onTimeUpdate={(event) => {
              setPlayhead(event.currentTarget.currentTime);
            }} />
          ) : (
            <img src={sourceUrl} alt={asset.name} />
          )}
        </div>
        <div className="source-viewer-range">
          <label>In (seconds)<input aria-label="Source In seconds" type="number" min="0" max={duration} step="0.001" value={sourceIn} onChange={(event) => setSourceIn(Number(event.target.value))} /></label>
          <button type="button" className="btn-action" disabled={playhead >= sourceOut} onClick={() => setSourceIn(playhead)}>Set In at playhead</button>
          <label>Out (seconds)<input aria-label="Source Out seconds" type="number" min="0" max={duration} step="0.001" value={sourceOut} onChange={(event) => setSourceOut(Number(event.target.value))} /></label>
          <button type="button" className="btn-action" disabled={playhead <= sourceIn} onClick={() => setSourceOut(playhead)}>Set Out at playhead</button>
          <span>{Math.max(0, sourceOut - sourceIn).toFixed(3)} s selected</span>
        </div>
        <footer className="modal-dialog-actions">
          <button type="button" className="btn-primary" disabled={!canAdd} onClick={() => onAddRange(asset, sourceIn, sourceOut)}>
            Add range to timeline
          </button>
          <button type="button" className="btn-action" onClick={onClose}>Cancel</button>
        </footer>
      </section>
    </div>
  );
}
