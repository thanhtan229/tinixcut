import { OperationEnvelope } from "../types/command";
import { RationalTime } from "../types/project";

export function formatDraftTime(time: RationalTime): string {
  const { value, rate } = time;
  const seconds = value * rate.denominator / rate.numerator;
  const frames = `${value} frames at ${rate.numerator}/${rate.denominator} fps`;
  return Number.isFinite(seconds) && rate.numerator > 0 && rate.denominator > 0
    ? `~${seconds.toFixed(3)} s (${frames})`
    : frames;
}

export function countPlannedVideoClips(operations: OperationEnvelope[]): number {
  return operations.filter(({ operation }) =>
    operation.type === "ClipCreate"
    && operation.payload.kind === "Video"
    && Boolean(operation.payload.asset_id)
  ).length;
}

export function getOrderedClipIds(
  operations: OperationEnvelope[],
  initialItemIds: string[],
  trackId: string,
): string[] {
  const positions = new Map<string, RationalTime>();
  for (const { operation } of operations) {
    if (operation.type === "ClipMove" && operation.payload.target_track_id === trackId) {
      positions.set(operation.payload.item_id, operation.payload.target_start);
    }
  }
  const compareTime = (left: RationalTime, right: RationalTime) => {
    const leftScaled = BigInt(left.value) * BigInt(left.rate.denominator) * BigInt(right.rate.numerator);
    const rightScaled = BigInt(right.value) * BigInt(right.rate.denominator) * BigInt(left.rate.numerator);
    return leftScaled < rightScaled ? -1 : leftScaled > rightScaled ? 1 : 0;
  };
  if (initialItemIds.some((itemId) => !positions.has(itemId))) return initialItemIds;
  return [...initialItemIds].sort((leftId, rightId) =>
    compareTime(positions.get(leftId)!, positions.get(rightId)!) ||
    initialItemIds.indexOf(leftId) - initialItemIds.indexOf(rightId),
  );
}
