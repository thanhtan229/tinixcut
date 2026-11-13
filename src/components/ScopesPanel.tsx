import { useEffect, useRef, useState } from "react";
import { editorApi, type FrameScopes } from "../services/editorApi";
import { createLatestOnlyRunner } from "./latestOnlyRunner";
import {
  formatPercent,
  histogramHeights,
  vectorscopePixels,
  waveformPixels,
  type WaveformMode,
} from "./scopeDraw";

interface ScopesPanelProps {
  frame: number;
  revision: number;
}

const DEBOUNCE_MS = 250;

function putImage(canvas: HTMLCanvasElement | null, width: number, height: number, pixels: Uint8ClampedArray) {
  if (!canvas) return;
  canvas.width = width;
  canvas.height = height;
  canvas.getContext("2d")?.putImageData(new ImageData(new Uint8ClampedArray(pixels), width, height), 0, 0);
}

/**
 * Read-only video scopes of the exported frame at the playhead. Requests are debounced and run
 * one at a time; a result for an older frame/revision is dropped instead of being drawn.
 */
export function ScopesPanel({ frame, revision }: ScopesPanelProps) {
  const [scopes, setScopes] = useState<FrameScopes | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [mode, setMode] = useState<WaveformMode>("luma");
  const waveformRef = useRef<HTMLCanvasElement | null>(null);
  const vectorRef = useRef<HTMLCanvasElement | null>(null);
  const histogramRef = useRef<HTMLCanvasElement | null>(null);
  const runnerRef = useRef<ReturnType<typeof createLatestOnlyRunner<number, FrameScopes>> | null>(null);

  useEffect(() => {
    const runner = createLatestOnlyRunner((target: number) => editorApi.frameScopes(target), {
      onResult: (result) => {
        setScopes(result);
        setError(null);
      },
      onError: (err) => setError(String(err)),
      onBusy: setBusy,
    });
    runnerRef.current = runner;
    return () => {
      runner.dispose();
      runnerRef.current = null;
    };
  }, []);

  useEffect(() => {
    const timer = window.setTimeout(() => runnerRef.current?.request(frame), DEBOUNCE_MS);
    return () => window.clearTimeout(timer);
    // `revision` re-runs the analysis after edits at the same playhead position.
  }, [frame, revision]);

  useEffect(() => {
    if (!scopes) return;
    putImage(waveformRef.current, scopes.waveform.columns, 256, waveformPixels(scopes.waveform, mode));
    putImage(vectorRef.current, scopes.vectorscope.size, scopes.vectorscope.size, vectorscopePixels(scopes.vectorscope));
    const canvas = histogramRef.current;
    const context = canvas?.getContext("2d");
    if (canvas && context) {
      canvas.width = 256;
      canvas.height = 80;
      context.clearRect(0, 0, 256, 80);
      context.globalCompositeOperation = "lighter";
      const channels: Array<[number[], string]> = [
        [scopes.histogram.red, "rgb(200,40,40)"],
        [scopes.histogram.green, "rgb(40,200,40)"],
        [scopes.histogram.blue, "rgb(40,40,200)"],
      ];
      for (const [bins, color] of channels) {
        context.fillStyle = color;
        histogramHeights(bins).forEach((height, level) => {
          context.fillRect(level, 80 - height * 80, 1, height * 80);
        });
      }
    }
  }, [scopes, mode]);

  const stats = scopes?.luma_stats;
  return (
    <section style={{ display: "flex", flexDirection: "column", gap: "6px" }} aria-label="Video scopes">
      <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)" }}>
        VIDEO SCOPES {busy ? "…" : ""}
      </div>
      {error && (
        <div role="status" style={{ fontSize: "11px", color: "var(--danger, #e66)" }}>
          Scopes unavailable: {error}
        </div>
      )}
      <canvas ref={histogramRef} aria-label="Histogram" style={{ width: "100%", height: 80, background: "#111" }} />
      <label style={{ fontSize: "11px", display: "flex", gap: "6px", alignItems: "center" }}>
        Waveform
        <select aria-label="Waveform mode" value={mode} onChange={(e) => setMode(e.currentTarget.value as WaveformMode)}>
          <option value="luma">Luma</option>
          <option value="rgb">RGB</option>
        </select>
      </label>
      <canvas ref={waveformRef} aria-label="Waveform" style={{ width: "100%", height: 120, background: "#111" }} />
      <canvas
        ref={vectorRef}
        aria-label="Vectorscope"
        style={{ width: "60%", aspectRatio: "1", alignSelf: "center", background: "#111", borderRadius: "50%" }}
      />
      {stats && (
        <div style={{ fontSize: "11px", fontVariantNumeric: "tabular-nums" }}>
          Luma {stats.min}–{stats.max}, mean {stats.mean.toFixed(0)} · black clip {formatPercent(stats.blackClipShare)} ·
          white clip {formatPercent(stats.whiteClipShare)}
          <div style={{ color: "var(--text-muted)" }}>Text overlays are not included.</div>
        </div>
      )}
    </section>
  );
}
