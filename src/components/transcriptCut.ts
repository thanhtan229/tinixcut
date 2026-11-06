import type { OperationEnvelope } from "../types/command";
import type { Asset, FrameRate, RationalTime } from "../types/project";
import { rationalTimeToSeconds } from "../types/project";
import { sourceRangeSecondsToFrames } from "./mediaLibrary";

export interface TranscriptCutRange {
  sourceInSeconds: number;
  sourceOutSeconds: number;
}

export interface TimedTranscriptSegment {
  sourceStart: RationalTime;
  sourceDuration: RationalTime;
}

export function suggestSpeechRangesFromTranscript(
  segments: TimedTranscriptSegment[],
  minimumGapSeconds: number,
  paddingSeconds: number,
  sourceDurationSeconds?: number,
): TranscriptCutRange[] {
  if (!Number.isFinite(minimumGapSeconds) || minimumGapSeconds <= 0) {
    throw new Error("Pause threshold must be a positive number of seconds.");
  }
  if (!Number.isFinite(paddingSeconds) || paddingSeconds < 0 || paddingSeconds > 2) {
    throw new Error("Speech padding must be between zero and two seconds.");
  }
  if (sourceDurationSeconds !== undefined && (!Number.isFinite(sourceDurationSeconds) || sourceDurationSeconds <= 0)) {
    throw new Error("Source duration must be positive when provided.");
  }

  const timed = segments.map((segment, index) => {
    const start = rationalTimeToSeconds(segment.sourceStart);
    const duration = rationalTimeToSeconds(segment.sourceDuration);
    if (!Number.isFinite(start) || !Number.isFinite(duration) || start < 0 || duration <= 0) {
      throw new Error(`Transcript segment ${index + 1} has invalid source timing.`);
    }
    return { start, end: start + duration };
  }).sort((left, right) => left.start - right.start);

  if (timed.length === 0) return [];
  const groups: Array<{ start: number; end: number }> = [];
  for (const segment of timed) {
    const current = groups[groups.length - 1];
    if (current && segment.start - current.end < minimumGapSeconds) {
      current.end = Math.max(current.end, segment.end);
    } else {
      groups.push({ ...segment });
    }
  }

  return groups.map(({ start, end }) => ({
    sourceInSeconds: Math.max(0, start - paddingSeconds),
    sourceOutSeconds: Math.min(sourceDurationSeconds ?? Number.POSITIVE_INFINITY, end + paddingSeconds),
  })).filter((range) => range.sourceOutSeconds > range.sourceInSeconds);
}

export function buildTranscriptCutOperations(
  asset: Asset,
  ranges: TranscriptCutRange[],
  frameRate: FrameRate,
  timelineStartFrame: number,
  operationNonce = Date.now(),
): OperationEnvelope[] {
  if (asset.media_kind === "Image") throw new Error("Transcript cuts require an audio or video source.");
  if (!Number.isSafeInteger(timelineStartFrame) || timelineStartFrame < 0) {
    throw new Error("Transcript cut must start at a nonnegative whole frame.");
  }
  if (ranges.length === 0) throw new Error("Select at least one transcript range.");

  const assetDurationSeconds = asset.duration
    ? asset.duration.value * asset.duration.rate.denominator / asset.duration.rate.numerator
    : null;
  const trackId = `trk_transcript_cut_${operationNonce}`;
  const kind = asset.media_kind === "Audio" ? "Audio" as const : "Video" as const;
  const operations: OperationEnvelope[] = [{
    operation_id: `op_transcript_cut_track_${operationNonce}`,
    operation: { type: "TrackCreate", payload: { track_id: trackId, name: `Transcript Cut · ${asset.name}`, kind } },
  }];
  let timelineOffset = 0;

  ranges.forEach(({ sourceInSeconds, sourceOutSeconds }, index) => {
    if (assetDurationSeconds !== null && sourceOutSeconds > assetDurationSeconds) {
      throw new Error(`Transcript range ${index + 1} exceeds the source asset duration.`);
    }
    const sourceRange = sourceRangeSecondsToFrames(sourceInSeconds, sourceOutSeconds, frameRate);
    if (!sourceRange) throw new Error(`Transcript range ${index + 1} is invalid or shorter than one project frame.`);
    const timelineStart = timelineStartFrame + timelineOffset;
    if (!Number.isSafeInteger(timelineStart)) throw new Error("Transcript cut exceeds the supported timeline range.");

    const itemId = `clip_transcript_cut_${operationNonce}_${index}`;
    operations.push({
      operation_id: `op_transcript_cut_clip_${operationNonce}_${index}`,
      operation: {
        type: "ClipCreate",
        payload: {
          item_id: itemId,
          track_id: trackId,
          name: asset.name,
          kind,
          asset_id: asset.asset_id,
          timeline_start: { value: timelineStart, rate: frameRate },
          duration: { value: sourceRange.duration, rate: frameRate },
          source_start: { value: sourceRange.start, rate: frameRate },
        },
      },
    });
    timelineOffset += sourceRange.duration;
    if (!Number.isSafeInteger(timelineOffset)) throw new Error("Transcript cut duration exceeds the supported timeline range.");
  });

  return operations;
}
