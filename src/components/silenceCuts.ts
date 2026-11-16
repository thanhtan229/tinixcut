import { Asset, FrameRate } from "../types/project";
import { buildTranscriptCutOperations, TranscriptCutRange } from "./transcriptCut";

export interface SilenceConfig { thresholdDbfs: number; minimumSilenceMs: number; paddingMs: number }
export interface SilenceCut { startMicros: number; endMicros: number }
export interface SilenceAnalysis { sourceFingerprint: string; durationMicros: number; cuts: SilenceCut[]; config: SilenceConfig; measurement: string }
export interface SilenceJobReport {
  jobId: string; projectId: string; revision: number; assetId: string; sourcePath: string;
  state: "running" | "completed" | "failed" | "cancelled"; progress: number; message: string;
  durationMicros: number; analysis: SilenceAnalysis | null;
}

/** Round cuts inward, retaining any boundary audio that does not fill a frame. */
export function silenceKeepRanges(analysis: SilenceAnalysis, selected: ReadonlySet<number>, rate: FrameRate): { ranges: TranscriptCutRange[]; removedFrames: number; totalFrames: number } {
  const fps = rate.numerator / rate.denominator;
  if (!Number.isFinite(fps) || fps <= 0 || !Number.isSafeInteger(analysis.durationMicros) || analysis.durationMicros <= 0) throw new Error("Invalid analysis duration/frame rate");
  const totalFrames = Math.floor(analysis.durationMicros / 1e6 * fps + 1e-8);
  if ([...selected].some(i => !Number.isInteger(i) || i < 0 || i >= analysis.cuts.length)) throw new Error("Selected silence intervals are stale; review the draft again");
  const ranges: TranscriptCutRange[] = [];let cursor = 0, removedFrames = 0;
  for (const [i, cut] of analysis.cuts.entries()) {
    if (!Number.isSafeInteger(cut.startMicros) || !Number.isSafeInteger(cut.endMicros) || cut.startMicros < 0 || cut.endMicros <= cut.startMicros || cut.endMicros > analysis.durationMicros || (i > 0 && cut.startMicros < analysis.cuts[i - 1].endMicros)) throw new Error("Invalid or overlapping silence intervals");
    if (!selected.has(i)) continue;
    const start = Math.ceil(cut.startMicros / 1e6 * fps - 1e-8), end = Math.floor(cut.endMicros / 1e6 * fps + 1e-8);
    if (end <= start) continue;
    if (start > cursor) ranges.push({ sourceInSeconds: cursor / fps, sourceOutSeconds: start / fps });
    removedFrames += end - start;cursor = end;
  }
  if (cursor < totalFrames) ranges.push({ sourceInSeconds: cursor / fps, sourceOutSeconds: totalFrames / fps });
  return { ranges, removedFrames, totalFrames };
}

export function buildSilenceCutOperations(asset: Asset, analysis: SilenceAnalysis, selected: ReadonlySet<number>, rate: FrameRate, frame: number, nonce = Date.now()) {
  const result = silenceKeepRanges(analysis, selected, rate);
  if (result.removedFrames === 0) throw new Error("Select at least one removable silence interval");
  if (!result.ranges.length) throw new Error("These cuts remove the entire source; retain at least one frame");
  const operations = buildTranscriptCutOperations(asset, result.ranges, rate, frame, nonce);
  const track = operations[0].operation;
  if (track.type === "TrackCreate") track.payload.name = `Silence Cut · ${asset.name}`;
  return operations;
}
