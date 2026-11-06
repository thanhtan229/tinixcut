import {
  Asset,
  Project,
  FrameRate,
  RationalTime,
  TimeRange,
  rationalTimeToFrames,
  rationalTimeToSeconds,
} from "../types/project";

export interface TimelineRollPlan {
  trackId: string;
  leftItemId: string;
  rightItemId: string;
  deltaFrames: number;
  leftTimelineRange: TimeRange;
  leftSourceRange: TimeRange;
  rightTimelineRange: TimeRange;
  rightSourceRange: TimeRange;
}

function sourceDeltaForTimelineFrames(
  frames: number,
  project: Project,
  item: Project["tracks"][number]["items"][number],
): RationalTime {
  const speedNumerator = item.speed === "Half" ? 1 : item.speed === "Double" ? 2 : 1;
  const speedDenominator = item.speed === "Half" ? 2 : 1;
  const numerator = project.settings.frame_rate.numerator * speedDenominator;
  const denominator = project.settings.frame_rate.denominator * speedNumerator;
  if (!Number.isSafeInteger(numerator) || !Number.isSafeInteger(denominator) || numerator <= 0 || denominator <= 0) {
    throw new Error(`Roll offset is outside the supported timebase for '${item.name}'.`);
  }
  return { value: frames, rate: { numerator, denominator } };
}

function gcd(left: bigint, right: bigint): bigint {
  let a = left < 0n ? -left : left;
  let b = right < 0n ? -right : right;
  while (b !== 0n) [a, b] = [b, a % b];
  return a || 1n;
}

function commonExactSourceRate(item: Project["tracks"][number]["items"][number], delta: RationalTime): FrameRate {
  const rates = [item.source_range.start.rate.numerator, item.source_range.duration.rate.numerator, delta.rate.numerator];
  let common = 1n;
  for (const numerator of rates) {
    if (!Number.isSafeInteger(numerator) || numerator <= 0) {
      throw new Error(`Source timebase is invalid for '${item.name}'.`);
    }
    const next = BigInt(numerator);
    common = (common / gcd(common, next)) * next;
  }
  if (common > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new Error(`Roll timebase is outside the supported range for '${item.name}'.`);
  }
  return { numerator: Number(common), denominator: 1 };
}

function rescaleExact(time: RationalTime, rate: FrameRate, itemName: string): number {
  const numerator = BigInt(time.value)
    * BigInt(rate.numerator)
    * BigInt(time.rate.denominator);
  const denominator = BigInt(time.rate.numerator) * BigInt(rate.denominator);
  if (denominator <= 0n || numerator % denominator !== 0n) {
    throw new Error(`Roll offset cannot be represented exactly for '${itemName}'.`);
  }
  const value = numerator / denominator;
  if (value > BigInt(Number.MAX_SAFE_INTEGER) || value < BigInt(Number.MIN_SAFE_INTEGER)) {
    throw new Error(`Roll time is outside the supported range for '${itemName}'.`);
  }
  return Number(value);
}

function shiftedSourceRange(
  project: Project,
  item: Project["tracks"][number]["items"][number],
  asset: Asset,
  startDeltaFrames: number,
  durationDeltaFrames: number,
): TimeRange {
  const sourceStartDelta = sourceDeltaForTimelineFrames(startDeltaFrames, project, item);
  const sourceDurationDelta = sourceDeltaForTimelineFrames(durationDeltaFrames, project, item);
  const commonRate = commonExactSourceRate(item, sourceStartDelta);
  const nextStart = rescaleExact(item.source_range.start, commonRate, item.name)
    + rescaleExact(sourceStartDelta, commonRate, item.name);
  const nextDuration = rescaleExact(item.source_range.duration, commonRate, item.name)
    + rescaleExact(sourceDurationDelta, commonRate, item.name);
  if (!Number.isSafeInteger(nextStart) || nextStart < 0 || !Number.isSafeInteger(nextDuration) || nextDuration <= 0) {
    throw new Error(`Roll would leave '${item.name}' with an invalid source range.`);
  }
  const sourceEndSeconds = rationalTimeToSeconds({ value: nextStart, rate: commonRate })
    + rationalTimeToSeconds({ value: nextDuration, rate: commonRate });
  if (asset.duration && sourceEndSeconds > rationalTimeToSeconds(asset.duration) + 1e-9) {
    throw new Error(`Roll would move '${item.name}' beyond the end of its media.`);
  }
  return {
    start: { value: nextStart, rate: commonRate },
    duration: { value: nextDuration, rate: commonRate },
  };
}

export function getTimelineRollPlan(
  project: Project,
  leftItemId: string,
  rightItemId: string,
  deltaFrames: number,
): TimelineRollPlan {
  if (!Number.isInteger(deltaFrames) || deltaFrames === 0) {
    throw new Error("Roll offset must be a non-zero integer number of project frames.");
  }
  const track = project.tracks.find((candidate) =>
    candidate.items.some((item) => item.item_id === leftItemId)
      && candidate.items.some((item) => item.item_id === rightItemId));
  if (!track) throw new Error("Roll clips must be on the same track.");
  if (track.locked) throw new Error(`Track '${track.name}' is locked.`);
  const left = track.items.find((item) => item.item_id === leftItemId)!;
  const right = track.items.find((item) => item.item_id === rightItemId)!;
  if (!["Video", "Audio"].includes(left.kind) || left.kind !== right.kind) {
    throw new Error("Roll edit requires adjacent clips of the same video or audio kind.");
  }
  const rate = project.settings.frame_rate;
  const leftStart = rationalTimeToFrames(left.timeline_range.start, rate);
  const leftDuration = rationalTimeToFrames(left.timeline_range.duration, rate);
  const rightStart = rationalTimeToFrames(right.timeline_range.start, rate);
  const rightDuration = rationalTimeToFrames(right.timeline_range.duration, rate);
  const cutFrame = leftStart + leftDuration;
  if (rightStart !== cutFrame) throw new Error("Roll edit requires two clips that meet at the same cut.");
  const newCutFrame = cutFrame + deltaFrames;
  if (newCutFrame <= leftStart || newCutFrame >= rightStart + rightDuration) {
    throw new Error("Roll would make one of the clips empty.");
  }
  const leftAsset = project.assets.find((asset) => asset.asset_id === left.asset_id);
  const rightAsset = project.assets.find((asset) => asset.asset_id === right.asset_id);
  if (!leftAsset || !rightAsset || !leftAsset.duration || !rightAsset.duration) {
    throw new Error("Roll edit requires both source media durations to be known.");
  }

  const leftSourceRange = shiftedSourceRange(project, left, leftAsset, 0, deltaFrames);
  const rightSourceRange = shiftedSourceRange(project, right, rightAsset, deltaFrames, -deltaFrames);
  const delta: RationalTime = { value: deltaFrames, rate };
  const newCut: RationalTime = { value: newCutFrame, rate };
  const leftTimelineRange: TimeRange = {
    start: { value: leftStart, rate: { ...rate } },
    duration: { value: newCutFrame - leftStart, rate: { ...rate } },
  };
  const rightTimelineRange: TimeRange = {
    start: newCut,
    duration: { value: rightStart + rightDuration - newCutFrame, rate: { ...rate } },
  };
  return {
    trackId: track.track_id,
    leftItemId,
    rightItemId,
    deltaFrames: delta.value,
    leftTimelineRange,
    leftSourceRange,
    rightTimelineRange,
    rightSourceRange,
  };
}
