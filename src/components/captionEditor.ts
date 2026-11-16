import { Project, TimelineItem, rationalTimeToFrames } from "../types/project";
import type { OperationEnvelope } from "../types/command";

export interface CaptionRow {
  trackId: string;
  item: TimelineItem;
  trackName: string;
  locked: boolean;
}

export interface CaptionTimingDraft {
  item_id: string;
  start_frame: number;
  duration_frames: number;
}

export function isValidCaptionStartFrame(value: number): boolean {
  return Number.isSafeInteger(value) && value >= 0;
}

export function isValidCaptionDurationFrames(value: number): boolean {
  return Number.isSafeInteger(value) && value > 0;
}

export function buildCaptionTimingOperations(
  project: Project,
  drafts: CaptionTimingDraft[],
  operationNonce = Date.now(),
): OperationEnvelope[] {
  const seen = new Set<string>();
  const operations: OperationEnvelope[] = [];
  for (const { item_id, start_frame, duration_frames } of drafts) {
    if (!isValidCaptionStartFrame(start_frame)) {
      throw new Error(`Caption ${item_id} must start at a nonnegative whole frame.`);
    }
    if (!isValidCaptionDurationFrames(duration_frames)) {
      throw new Error(`Caption ${item_id} must have a positive whole-frame duration.`);
    }
    if (seen.has(item_id)) throw new Error(`Caption ${item_id} appears more than once in the timing edit.`);
    seen.add(item_id);

    const track = project.tracks.find((candidate) => candidate.items.some((item) => item.item_id === item_id));
    const item = track?.items.find((candidate) => candidate.item_id === item_id);
    if (!track || item?.kind !== "Text") throw new Error(`Caption ${item_id} no longer exists.`);
    if (track.locked) throw new Error(`Caption ${item_id} is on a locked track.`);
    const originalStart = rationalTimeToFrames(item.timeline_range.start, project.settings.frame_rate);
    const originalDuration = rationalTimeToFrames(item.timeline_range.duration, project.settings.frame_rate);
    if (originalStart === start_frame && originalDuration === duration_frames) continue;
    if (originalDuration === duration_frames) {
      operations.push({
        operation_id: `op_caption_move_${item_id}_${operationNonce}`,
        operation: {
          type: "ClipMove",
          payload: { item_id, target_start: { value: start_frame, rate: project.settings.frame_rate } },
        },
      });
      continue;
    }
    const sourceRate = item.source_range.duration.rate;
    const sourceNumerator = duration_frames * sourceRate.numerator * project.settings.frame_rate.denominator;
    const sourceDenominator = project.settings.frame_rate.numerator * sourceRate.denominator;
    if (!Number.isSafeInteger(sourceNumerator / sourceDenominator) || sourceNumerator % sourceDenominator !== 0) {
      throw new Error(`Caption ${item_id} duration cannot be represented on its source time grid.`);
    }
    operations.push({
      operation_id: `op_caption_trim_${item_id}_${operationNonce}`,
      operation: {
        type: "ClipTrim",
        payload: {
          item_id,
          new_timeline_range: {
            start: { value: start_frame, rate: project.settings.frame_rate },
            duration: { value: duration_frames, rate: project.settings.frame_rate },
          },
          new_source_range: {
            start: item.source_range.start,
            duration: { value: sourceNumerator / sourceDenominator, rate: sourceRate },
          },
        },
      },
    });
  }
  return operations;
}

export function canMergeCaptions(project: Project, leftId: string, rightId: string): boolean {
  try {
    buildCaptionMergeOperations(project, leftId, rightId, 0);
    return true;
  } catch {
    return false;
  }
}

export function buildCaptionMergeOperations(
  project: Project,
  leftId: string,
  rightId: string,
  operationNonce = Date.now(),
): OperationEnvelope[] {
  const track = project.tracks.find((candidate) => candidate.items.some((item) => item.item_id === leftId));
  if (!track || track.locked) throw new Error("Captions must be on an unlocked track.");
  const left = track.items.find((item) => item.item_id === leftId);
  const right = track.items.find((item) => item.item_id === rightId);
  if (!left || !right || left.kind !== "Text" || right.kind !== "Text") {
    throw new Error("Choose two existing text captions on the same track.");
  }
  const ordered = [...track.items].sort((a, b) =>
    rationalTimeToFrames(a.timeline_range.start, project.settings.frame_rate)
    - rationalTimeToFrames(b.timeline_range.start, project.settings.frame_rate));
  const leftIndex = ordered.findIndex((item) => item.item_id === leftId);
  const leftStart = rationalTimeToFrames(left.timeline_range.start, project.settings.frame_rate);
  const leftDuration = rationalTimeToFrames(left.timeline_range.duration, project.settings.frame_rate);
  const rightStart = rationalTimeToFrames(right.timeline_range.start, project.settings.frame_rate);
  if (ordered[leftIndex + 1]?.item_id !== rightId || leftStart + leftDuration !== rightStart) {
    throw new Error("Captions must be adjacent on the same track to merge.");
  }

  const combinedDurationFrames = leftDuration + rationalTimeToFrames(right.timeline_range.duration, project.settings.frame_rate);
  const sourceRate = left.source_range.duration.rate;
  const sourceNumerator = combinedDurationFrames * sourceRate.numerator * project.settings.frame_rate.denominator;
  const sourceDenominator = project.settings.frame_rate.numerator * sourceRate.denominator;
  if (!Number.isSafeInteger(sourceNumerator / sourceDenominator) || sourceNumerator % sourceDenominator !== 0) {
    throw new Error("Merged caption duration cannot be represented on its source time grid.");
  }
  const mergedText = [left.text_content ?? "", right.text_content ?? ""].filter(Boolean).join("\n");
  if (!mergedText) throw new Error("Captions have no text to merge.");

  return [
    { operation_id: `op_caption_merge_text_${leftId}_${operationNonce}`, operation: { type: "ClipTextSet", payload: { item_id: leftId, text: mergedText } } },
    { operation_id: `op_caption_merge_delete_${rightId}_${operationNonce}`, operation: { type: "ClipDelete", payload: { item_id: rightId } } },
    { operation_id: `op_caption_merge_trim_${leftId}_${operationNonce}`, operation: {
      type: "ClipTrim",
      payload: {
        item_id: leftId,
        new_timeline_range: {
          start: { value: leftStart, rate: project.settings.frame_rate },
          duration: { value: combinedDurationFrames, rate: project.settings.frame_rate },
        },
        new_source_range: {
          start: left.source_range.start,
          duration: { value: sourceNumerator / sourceDenominator, rate: sourceRate },
        },
      },
    } },
  ];
}

export function collectCaptionRows(project: Project | null): CaptionRow[] {
  if (!project) return [];
  return project.tracks
    .flatMap((track) => track.items
      .filter((item) => item.kind === "Text")
      .map((item) => ({ item, trackId: track.track_id, trackName: track.name, locked: track.locked ?? false })))
    .sort((left, right) =>
      rationalTimeToFrames(left.item.timeline_range.start, project.settings.frame_rate)
      - rationalTimeToFrames(right.item.timeline_range.start, project.settings.frame_rate));
}

export type CaptionTimingNotice = { kind: "gap" | "overlap"; frames: number };

/** Compare against earlier coverage on the same track, including nested captions. */
export function getCaptionTimingNotices(
  rows: CaptionRow[],
  rate: Project["settings"]["frame_rate"],
  drafts: Record<string, { start_frame: number; duration_frames: number }>,
): { notices: Record<string, CaptionTimingNotice>; invalidTrackIds: Set<string> } {
  const tracks = new Map<string, Array<{ id: string; start: number; end: number }>>();
  const invalidTrackIds = new Set<string>();
  for (const { item, trackId } of rows) {
    const draft = drafts[item.item_id];
    const start = draft?.start_frame ?? rationalTimeToFrames(item.timeline_range.start, rate);
    const duration = draft?.duration_frames ?? rationalTimeToFrames(item.timeline_range.duration, rate);
    const end = start + duration;
    if (!isValidCaptionStartFrame(start) || !isValidCaptionDurationFrames(duration) || !Number.isSafeInteger(end)) {
      invalidTrackIds.add(trackId);
      continue;
    }
    const entries = tracks.get(trackId) ?? [];
    entries.push({ id: item.item_id, start, end });
    tracks.set(trackId, entries);
  }
  const notices: Record<string, CaptionTimingNotice> = {};
  for (const [trackId, entries] of tracks) {
    if (invalidTrackIds.has(trackId)) continue;
    entries.sort((a, b) => a.start - b.start || a.id.localeCompare(b.id));
    let coveredEnd: number | undefined;
    for (const entry of entries) {
      if (coveredEnd !== undefined && entry.start > coveredEnd) {
        notices[entry.id] = { kind: "gap", frames: entry.start - coveredEnd };
      } else if (coveredEnd !== undefined && entry.start < coveredEnd) {
        notices[entry.id] = { kind: "overlap", frames: Math.min(entry.end, coveredEnd) - entry.start };
      }
      coveredEnd = Math.max(coveredEnd ?? entry.end, entry.end);
    }
  }
  return { notices, invalidTrackIds };
}

export function captionMatchesSearch(text: string, query: string): boolean {
  return text.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase());
}

export function canSplitCaptionAtFrame(item: TimelineItem, frame: number, frameRate: Project["settings"]["frame_rate"]): boolean {
  const start = rationalTimeToFrames(item.timeline_range.start, frameRate);
  const end = start + rationalTimeToFrames(item.timeline_range.duration, frameRate);
  return Number.isInteger(frame) && frame > start && frame < end;
}
