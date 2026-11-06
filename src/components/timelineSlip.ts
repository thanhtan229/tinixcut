import { Asset, FrameRate, RationalTime, TimelineItem, rationalTimeToSeconds } from "../types/project";

export function calculateSlipSourceRange(
  item: TimelineItem,
  asset: Asset | undefined,
  timelineDeltaFrames: number,
  projectRate: FrameRate,
): { start: RationalTime; duration: RationalTime } {
  if (!Number.isInteger(timelineDeltaFrames)) {
    throw new Error("Slip offset must be an integer number of timeline frames.");
  }
  const start = item.source_range.start;
  const duration = item.source_range.duration;
  const [speedNumerator, speedDenominator] = item.speed === "Half"
    ? [1, 2]
    : item.speed === "Double"
      ? [2, 1]
      : [1, 1];
  const deltaRateNumerator = projectRate.numerator * speedDenominator;
  const deltaRateDenominator = projectRate.denominator * speedNumerator;
  if (!Number.isSafeInteger(deltaRateNumerator) || !Number.isSafeInteger(deltaRateDenominator)
    || deltaRateNumerator <= 0 || deltaRateDenominator <= 0) {
    throw new Error("Slip offset cannot be represented with the current frame rates.");
  }
  const deltaRate = { numerator: deltaRateNumerator, denominator: deltaRateDenominator };
  const commonNumerator = lcm(BigInt(start.rate.numerator), BigInt(deltaRate.numerator));
  if (commonNumerator > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new Error("Slip offset cannot be represented with the current frame rates.");
  }
  const commonRate = { numerator: Number(commonNumerator), denominator: 1 };
  const startTicks = rescaleExact(start, commonRate);
  const deltaTicks = rescaleExact({ value: timelineDeltaFrames, rate: deltaRate }, commonRate);
  const nextStartValue = startTicks + deltaTicks;
  if (!Number.isSafeInteger(nextStartValue) || nextStartValue < 0) {
    throw new Error("Slip would move the source range before the start of the media.");
  }

  const sourceEndSeconds = rationalTimeToSeconds({ value: nextStartValue, rate: commonRate })
    + rationalTimeToSeconds(duration);
  if (asset?.duration && sourceEndSeconds > rationalTimeToSeconds(asset.duration) + 1e-9) {
    throw new Error("Slip would move the source range beyond the end of the media.");
  }

  return {
    start: { value: nextStartValue, rate: commonRate },
    duration: { ...duration, rate: { ...duration.rate } },
  };
}

function gcd(left: bigint, right: bigint): bigint {
  let a = left < 0n ? -left : left;
  let b = right < 0n ? -right : right;
  while (b !== 0n) [a, b] = [b, a % b];
  return a || 1n;
}

function lcm(left: bigint, right: bigint): bigint {
  return (left / gcd(left, right)) * right;
}

function rescaleExact(time: RationalTime, rate: FrameRate): number {
  const numerator = BigInt(time.value) * BigInt(rate.numerator) * BigInt(time.rate.denominator);
  const denominator = BigInt(time.rate.numerator) * BigInt(rate.denominator);
  if (denominator <= 0n || numerator % denominator !== 0n) {
    throw new Error("Slip offset cannot be represented exactly with the current frame rates.");
  }
  const value = numerator / denominator;
  if (value > BigInt(Number.MAX_SAFE_INTEGER) || value < BigInt(Number.MIN_SAFE_INTEGER)) {
    throw new Error("Slip offset is outside the supported range.");
  }
  return Number(value);
}
