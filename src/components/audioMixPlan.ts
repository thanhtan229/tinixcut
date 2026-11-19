import { OperationEnvelope } from "../types/command";
import { Project, RationalTime, TimelineItem, rationalTimeToSeconds } from "../types/project";
import { AudioLevelReport, EditPlanDraftReceipt } from "../services/editorApi";

export interface AudioMixPlanOptions {
  voiceItemIds: string[];
  musicItemIds: string[];
  normalizeTargetDbfs: number | null;
  duckDb: number;
  attackSeconds: number;
  releaseSeconds: number;
  levelsByAssetId: Record<string, AudioLevelReport>;
}

const MAX_LINEAR_GAIN = 2;
const PEAK_CEILING_DBFS = -1;

function frameAt(seconds: number, project: Project): number {
  const fps = project.settings.frame_rate.numerator / project.settings.frame_rate.denominator;
  return Math.max(0, Math.round(seconds * fps));
}

function frameTime(frame: number, project: Project): RationalTime {
  return { value: frame, rate: project.settings.frame_rate };
}

function audioItems(project: Project): Array<{ trackId: string; item: TimelineItem }> {
  return project.tracks.flatMap((track) => track.items
    .filter((item) => item.kind === "Audio" || (item.kind === "Video" && item.embedded_audio_enabled !== false))
    .map((item) => ({ trackId: track.track_id, item })));
}

function normalizedVolume(item: TimelineItem, report: AudioLevelReport, targetDbfs: number): number {
  if (report.mean_dbfs === null || report.peak_dbfs === null || !Number.isFinite(report.mean_dbfs) || !Number.isFinite(report.peak_dbfs)) {
    throw new Error(`No valid mean/peak dBFS analysis is available for ${item.name}.`);
  }
  const requestedGainDb = targetDbfs - report.mean_dbfs;
  const peakLimitedGainDb = Math.min(requestedGainDb, PEAK_CEILING_DBFS - report.peak_dbfs);
  const multiplier = Math.min(MAX_LINEAR_GAIN, 10 ** (peakLimitedGainDb / 20));
  return Math.max(0, Math.min(MAX_LINEAR_GAIN, item.volume * multiplier));
}

function mergeIntervals(intervals: Array<{ start: number; end: number }>, bridgeGapSeconds: number): Array<{ start: number; end: number }> {
  const sorted = [...intervals].sort((a, b) => a.start - b.start);
  const merged: Array<{ start: number; end: number }> = [];
  for (const interval of sorted) {
    const last = merged[merged.length - 1];
    if (last && interval.start <= last.end + bridgeGapSeconds) last.end = Math.max(last.end, interval.end);
    else merged.push({ ...interval });
  }
  return merged;
}

function duckOperations(
  music: TimelineItem,
  voices: TimelineItem[],
  project: Project,
  duckDb: number,
  attackSeconds: number,
  releaseSeconds: number,
): OperationEnvelope[] {
  if ((music.keyframes ?? []).some((keyframe) => keyframe.property === "Volume")) {
    throw new Error(`${music.name} already has volume automation. Remove or edit it before creating a ducking plan.`);
  }
  const musicStart = rationalTimeToSeconds(music.timeline_range.start);
  const musicEnd = musicStart + rationalTimeToSeconds(music.timeline_range.duration);
  const intervals = mergeIntervals(voices.map((voice) => {
    const start = rationalTimeToSeconds(voice.timeline_range.start);
    return {
      start: Math.max(musicStart, start),
      end: Math.min(musicEnd, start + rationalTimeToSeconds(voice.timeline_range.duration)),
    };
  }).filter((interval) => interval.end > interval.start), attackSeconds + releaseSeconds);
  if (intervals.length === 0) return [];

  const duckGain = 10 ** (duckDb / 20);
  const points = new Map<number, number>();
  points.set(0, 1);
  for (const interval of intervals) {
    const attackStart = Math.max(musicStart, interval.start - attackSeconds);
    const releaseEnd = Math.min(musicEnd, interval.end + releaseSeconds);
    points.set(frameAt(attackStart - musicStart, project), 1);
    points.set(frameAt(interval.start - musicStart, project), duckGain);
    points.set(frameAt(interval.end - musicStart, project), duckGain);
    points.set(frameAt(releaseEnd - musicStart, project), 1);
  }
  return [...points.entries()].sort(([a], [b]) => a - b).map(([frame, value], index) => ({
    operation_id: `audio_duck_${music.item_id}_${index + 1}`,
    operation: {
      type: "ClipKeyframeAdd",
      payload: {
        item_id: music.item_id,
        keyframe: { property: "Volume" as const, offset: frameTime(frame, project), value },
      },
    },
  }));
}

export function buildAudioMixPlan(project: Project, options: AudioMixPlanOptions): EditPlanDraftReceipt {
  if (options.voiceItemIds.some(id => options.musicItemIds.includes(id))) {
    throw new Error("A clip cannot be both voice and music in the same mix plan.");
  }
  const items = audioItems(project);
  const itemById = new Map(items.map(({ item }) => [item.item_id, item]));
  const voiceItems = options.voiceItemIds.map((id) => itemById.get(id));
  const musicItems = options.musicItemIds.map((id) => itemById.get(id));
  if (voiceItems.some((item) => !item) || musicItems.some((item) => !item)) {
    throw new Error("The selected audio clips changed. Refresh the mixer plan and review it again.");
  }
  for (const id of options.musicItemIds) {
    const track=project.tracks.find(candidate=>candidate.items.some(item=>item.item_id===id));
    if (track?.locked) throw new Error(`Unlock track ${track.name} before editing its music clips.`);
  }
  if (options.normalizeTargetDbfs !== null && (!Number.isFinite(options.normalizeTargetDbfs) || options.normalizeTargetDbfs > 0 || options.normalizeTargetDbfs < -60)) {
    throw new Error("Normalization target must be between −60 and 0 dBFS.");
  }
  if (!Number.isFinite(options.duckDb) || options.duckDb > 0 || options.duckDb < -48 || !Number.isFinite(options.attackSeconds) || options.attackSeconds < 0 || options.attackSeconds > 10 || !Number.isFinite(options.releaseSeconds) || options.releaseSeconds < 0 || options.releaseSeconds > 20) {
    throw new Error("Ducking must be between −48 and 0 dB with attack 0–10 s and release 0–20 s.");
  }

  const operations: OperationEnvelope[] = [];
  if (options.normalizeTargetDbfs !== null) {
    for (const item of musicItems as TimelineItem[]) {
      if (!item.asset_id) throw new Error(`${item.name} has no source asset to analyze.`);
      const report = options.levelsByAssetId[item.asset_id];
      if (!report) throw new Error(`Analyze ${item.name} before creating the normalization plan.`);
      operations.push({
        operation_id: `audio_normalize_${item.item_id}`,
        operation: { type: "ClipVolumeSet", payload: { item_id: item.item_id, volume: normalizedVolume(item, report, options.normalizeTargetDbfs) } },
      });
    }
  }
  for (const music of musicItems as TimelineItem[]) {
    operations.push(...duckOperations(music, voiceItems as TimelineItem[], project, options.duckDb, options.attackSeconds, options.releaseSeconds));
  }
  if (operations.length === 0) throw new Error("No selected music clip overlaps the selected voice clips.");

  return {
    provider: "deterministic-audio-mix-v1",
    plan: {
      schemaVersion: 1,
      goal: "Normalize selected music and duck it under selected voice clips",
      projectId: project.project_id,
      baseRevision: project.revision,
      referencedMediaIds: [...new Set((musicItems as TimelineItem[]).map((item) => item.asset_id).filter((id): id is string => Boolean(id)))],
      assumptions: ["Normalization uses source-file mean/peak dBFS, not LUFS or only the trimmed source region.", "A −1 dBFS sample-peak ceiling limits normalization gain; this does not guarantee true-peak compliance."],
      warnings: ["Ducking uses deterministic linear volume keyframes; review the envelope in the timeline and adjust after applying if needed."],
      operations: operations.map(({ operation_id, operation }) => ({ operationId: operation_id, operation })),
    },
    validation: { valid: true, issues: [], diff: null },
  };
}
