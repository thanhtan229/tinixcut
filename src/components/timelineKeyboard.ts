import { FrameRate, rationalTimeToFrames, TimelineItem } from "../types/project";

export type TimelineClipEdge = "left" | "right";

export function timelineArrowDelta(key: string, accelerated: boolean): number | null {
  const direction = key === "ArrowLeft" ? -1 : key === "ArrowRight" ? 1 : 0;
  return direction === 0 ? null : direction * (accelerated ? 10 : 1);
}

export function trimRangeByKeyboard(
  startFrame: number,
  durationFrames: number,
  edge: TimelineClipEdge,
  deltaFrames: number
): { startFrame: number; durationFrames: number } {
  if (edge === "left") {
    const nextStartFrame = Math.max(0, startFrame + deltaFrames);
    const appliedDelta = nextStartFrame - startFrame;
    return {
      startFrame: nextStartFrame,
      durationFrames: Math.max(1, durationFrames - appliedDelta),
    };
  }

  return {
    startFrame,
    durationFrames: Math.max(1, durationFrames + deltaFrames),
  };
}

export function adjacentRollItems(
  items: TimelineItem[],
  item: TimelineItem,
  edge: TimelineClipEdge,
  frameRate: FrameRate
): { left: TimelineItem; right: TimelineItem } | null {
  if (item.kind !== "Video" && item.kind !== "Audio") return null;

  const start = rationalTimeToFrames(item.timeline_range.start, frameRate);
  const duration = rationalTimeToFrames(item.timeline_range.duration, frameRate);
  const matching = items.find((candidate) => {
    if (candidate.item_id === item.item_id || candidate.kind !== item.kind) return false;
    const candidateStart = rationalTimeToFrames(candidate.timeline_range.start, frameRate);
    const candidateDuration = rationalTimeToFrames(candidate.timeline_range.duration, frameRate);
    return edge === "left"
      ? candidateStart + candidateDuration === start
      : candidateStart === start + duration;
  });

  if (!matching) return null;
  return edge === "left"
    ? { left: matching, right: item }
    : { left: item, right: matching };
}
