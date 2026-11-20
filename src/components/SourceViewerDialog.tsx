import { useEffect, useMemo, useRef, useState, type RefObject } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Asset, FrameRate } from "../types/project";
import { sourceRangeSecondsToFrames } from "./mediaLibrary";

interface SourceViewerDialogProps {
  reviewControls?: React.ReactNode;
  asset: Asset | null;
  initialSeekSeconds?: number;
  frameRate?: FrameRate;
  onClose: () => void;
  onAddRange: (asset: Asset, sourceInSeconds: number, sourceOutSeconds: number) => void;
}

function assetDurationSeconds(asset: Asset): number {
  if (asset.media_kind === "Image" && !asset.duration) return 5;
  const time = asset.duration;
  if (!time || time.value <= 0 || time.rate.numerator <= 0 || time.rate.denominator <= 0) return 0;
  const seconds = time.value * time.rate.denominator / time.rate.numerator;
  return Number.isFinite(seconds) && seconds > 0 ? seconds : 0;
}

export function SourceViewerDialog({ asset, initialSeekSeconds = 0, frameRate, onClose, onAddRange, reviewControls }: SourceViewerDialogProps) {
  const duration = useMemo(() => asset ? assetDurationSeconds(asset) : 0, [asset]);
  const [sourceIn, setSourceIn] = useState(0);
  const [sourceOut, setSourceOut] = useState(duration);
  const [playhead, setPlayhead] = useState(0);
  const mediaRef = useRef<HTMLVideoElement | HTMLAudioElement>(null);
  const dialogRef = useRef<HTMLElement>(null);

  useEffect(() => { if (asset) dialogRef.current?.focus(); }, [asset?.asset_id]);

  useEffect(() => {
    setSourceIn(0);
    setSourceOut(duration);
    const seekTo = Math.min(Math.max(initialSeekSeconds, 0), duration);
    setPlayhead(seekTo);
    if (mediaRef.current) { mediaRef.current.pause(); mediaRef.current.currentTime = seekTo; }
  }, [asset?.asset_id, duration, initialSeekSeconds]);

  if (!asset) return null;
  const sourceUrl = convertFileSrc(asset.path);
  const validRange = duration > 0 && Number.isFinite(sourceIn) && Number.isFinite(sourceOut) && sourceIn >= 0 && sourceOut > sourceIn && sourceOut <= duration;
  const frameRange = validRange && frameRate ? sourceRangeSecondsToFrames(sourceIn, sourceOut, frameRate) : null;
  const canAdd = validRange && (!frameRate || frameRange !== null);
  const seekBoundary = (seconds: number) => {
    const media = mediaRef.current;
    if (!media || !Number.isFinite(seconds) || seconds < 0 || seconds > duration) return;
    media.pause();
    media.currentTime = seconds;
    setPlayhead(seconds);
  };
  const canSeek = asset.media_kind !== "Image" && duration > 0;
  const canSetIn = duration > 0 && Number.isFinite(sourceOut) && playhead < sourceOut;
  const canSetOut = duration > 0 && Number.isFinite(sourceIn) && playhead > sourceIn;
  const fps = frameRate ? frameRate.numerator / frameRate.denominator : 0;
  const canStep = canSeek && Number.isFinite(fps) && fps > 0 && duration > 0;
  const stepFrame = (direction: -1 | 1) => {
    if (!canStep || !mediaRef.current) return;
    const frame = Math.round(mediaRef.current.currentTime * fps) + direction;
    seekBoundary(Math.max(0, Math.min(duration, frame / fps)));
  };

  return (
    <div className="dialog-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <section ref={dialogRef} tabIndex={-1} className="modal-dialog source-viewer-dialog" role="dialog" aria-modal="true" aria-labelledby="source-viewer-title" onKeyDown={event => {
        const target = event.target as HTMLElement;
        if (event.repeat || event.ctrlKey || event.metaKey || event.altKey || target.closest('input, textarea, select, [contenteditable]:not([contenteditable="false"])')) return;
        if ((event.key === "ArrowLeft" || event.key === "ArrowRight") && (target === event.currentTarget || target.closest(".source-viewer-preview") && !target.closest("video, audio, button"))) {
          if (canStep) { event.preventDefault(); event.stopPropagation(); stepFrame(event.key === "ArrowLeft" ? -1 : 1); }
          return;
        }
        const key = event.key.toLowerCase();
        if (key === "i" || key === "o") {
          event.preventDefault(); event.stopPropagation();
          if (key === "i" && canSetIn) setSourceIn(playhead);
          if (key === "o" && canSetOut) setSourceOut(playhead);
        }
      }}>
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
          <button type="button" className="btn-action" disabled={!canSeek || playhead <= 0} onClick={() => seekBoundary(0)}>Go to source start</button>
          <button type="button" className="btn-action" title="Left arrow when the viewer has focus" disabled={!canStep || playhead <= 0} onClick={() => stepFrame(-1)}>Previous project frame</button>
          <button type="button" className="btn-action" title="Right arrow when the viewer has focus" disabled={!canStep || playhead >= duration} onClick={() => stepFrame(1)}>Next project frame</button>
          <button type="button" className="btn-action" disabled={!canSeek || playhead >= duration} onClick={() => seekBoundary(duration)}>Go to source end</button>
          <span>Position: {playhead.toFixed(3)} s{canStep ? ` · project frame ${Math.round(playhead * fps)}` : ""}</span>
          <label>In (seconds)<input aria-label="Source In seconds" type="number" min="0" max={duration} step="0.001" value={Number.isFinite(sourceIn) ? sourceIn : ""} onChange={(event) => setSourceIn(event.target.value === "" ? NaN : Number(event.target.value))} /></label>
          <button type="button" className="btn-action" title="Shortcut: I" disabled={!canSetIn} onClick={() => setSourceIn(playhead)}>Set In at playhead</button>
          <button type="button" className="btn-action" disabled={!canSeek || !Number.isFinite(sourceIn) || sourceIn < 0 || sourceIn > duration} onClick={() => seekBoundary(sourceIn)}>Go to source In</button>
          <label>Out (seconds)<input aria-label="Source Out seconds" type="number" min="0" max={duration} step="0.001" value={Number.isFinite(sourceOut) ? sourceOut : ""} onChange={(event) => setSourceOut(event.target.value === "" ? NaN : Number(event.target.value))} /></label>
          <button type="button" className="btn-action" title="Shortcut: O" disabled={!canSetOut} onClick={() => setSourceOut(playhead)}>Set Out at playhead</button>
          <button type="button" className="btn-action" disabled={!canSeek || !Number.isFinite(sourceOut) || sourceOut < 0 || sourceOut > duration} onClick={() => seekBoundary(sourceOut)}>Go to source Out</button>
          <button type="button" className="btn-action" onClick={() => { setSourceIn(0); setSourceOut(duration); }}>Reset source range</button>
          <span>{duration <= 0 ? "Source duration unavailable; reimport or relink the source" : !validRange ? "Enter a valid source range" : !canAdd ? "Select at least one project frame" : `${(sourceOut - sourceIn).toFixed(3)} s selected${frameRange ? ` · ${frameRange.duration} project frames` : ""}`}</span>
          {frameRange && <span aria-label="Frame-aligned source range">Source frames {frameRange.start}–{frameRange.start + frameRange.duration} (end exclusive)</span>}
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
