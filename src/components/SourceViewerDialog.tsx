import { useEffect, useMemo, useRef, useState, type RefObject } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Asset } from "../types/project";

interface SourceViewerDialogProps {
  reviewControls?: React.ReactNode;
  asset: Asset | null;
  initialSeekSeconds?: number;
  onClose: () => void;
  onAddRange: (asset: Asset, sourceInSeconds: number, sourceOutSeconds: number) => void;
}

function assetDurationSeconds(asset: Asset): number {
  if (!asset.duration || asset.duration.rate.numerator <= 0) return 5;
  return Math.max(0.001, asset.duration.value * asset.duration.rate.denominator / asset.duration.rate.numerator);
}

export function SourceViewerDialog({ asset, initialSeekSeconds = 0, onClose, onAddRange, reviewControls }: SourceViewerDialogProps) {
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
    if (mediaRef.current) { mediaRef.current.pause(); mediaRef.current.currentTime = seekTo; }
  }, [asset?.asset_id, duration, initialSeekSeconds]);

  if (!asset) return null;
  const sourceUrl = convertFileSrc(asset.path);
  const canAdd = Number.isFinite(sourceIn) && Number.isFinite(sourceOut) && sourceIn >= 0 && sourceOut > sourceIn && sourceOut <= duration;
  const seekBoundary = (seconds: number) => {
    const media = mediaRef.current;
    if (!media || !Number.isFinite(seconds) || seconds < 0 || seconds > duration) return;
    media.pause();
    media.currentTime = seconds;
    setPlayhead(seconds);
  };
  const canSeek = asset.media_kind !== "Image";

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
        {reviewControls}
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
          <label>In (seconds)<input aria-label="Source In seconds" type="number" min="0" max={duration} step="0.001" value={Number.isFinite(sourceIn) ? sourceIn : ""} onChange={(event) => setSourceIn(event.target.value === "" ? NaN : Number(event.target.value))} /></label>
          <button type="button" className="btn-action" disabled={!Number.isFinite(sourceOut) || playhead >= sourceOut} onClick={() => setSourceIn(playhead)}>Set In at playhead</button>
          <button type="button" className="btn-action" disabled={!canSeek || !Number.isFinite(sourceIn) || sourceIn < 0 || sourceIn > duration} onClick={() => seekBoundary(sourceIn)}>Go to source In</button>
          <label>Out (seconds)<input aria-label="Source Out seconds" type="number" min="0" max={duration} step="0.001" value={Number.isFinite(sourceOut) ? sourceOut : ""} onChange={(event) => setSourceOut(event.target.value === "" ? NaN : Number(event.target.value))} /></label>
          <button type="button" className="btn-action" disabled={!Number.isFinite(sourceIn) || playhead <= sourceIn} onClick={() => setSourceOut(playhead)}>Set Out at playhead</button>
          <button type="button" className="btn-action" disabled={!canSeek || !Number.isFinite(sourceOut) || sourceOut < 0 || sourceOut > duration} onClick={() => seekBoundary(sourceOut)}>Go to source Out</button>
          <button type="button" className="btn-action" onClick={() => { setSourceIn(0); setSourceOut(duration); }}>Reset source range</button>
          <span>{canAdd ? `${(sourceOut - sourceIn).toFixed(3)} s selected` : "Enter a valid source range"}</span>
        </div>
        <footer className="modal-dialog-actions">
          {!reviewControls && <button type="button" className="btn-primary" disabled={!canAdd} onClick={() => onAddRange(asset, sourceIn, sourceOut)}>
            Add range to timeline
          </button>}
          <button type="button" className="btn-action" onClick={onClose}>Cancel</button>
        </footer>
      </section>
    </div>
  );
}
