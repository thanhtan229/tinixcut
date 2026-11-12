import { describe, expect, it } from "vitest";
import { waveformPathD } from "./waveformPath";

// Reference: the previous inline Timeline implementation (unshift-based).
function legacyPathD(
  wf: { bucket_rate: number; peaks: number[] },
  sourceStartF: number,
  sourceDurF: number,
  frameRate: number,
  widthPx: number,
  heightPx: number
): string | null {
  const rate = frameRate || 30;
  const bucketStart = Math.max(0, Math.floor((sourceStartF / rate) * wf.bucket_rate));
  const bucketCount = Math.max(1, Math.ceil((sourceDurF / rate) * wf.bucket_rate));
  const totalBuckets = Math.floor(wf.peaks.length / 2);
  const availableBuckets = Math.min(totalBuckets, bucketStart + bucketCount) - bucketStart;
  if (availableBuckets <= 1 || widthPx <= 4) return null;
  const numSamples = Math.min(availableBuckets, Math.max(20, Math.floor(widthPx / 2)));
  const step = availableBuckets / numSamples;
  const midY = heightPx / 2;
  const ampScale = midY * 0.85;
  const top: string[] = [];
  const bottom: string[] = [];
  for (let i = 0; i < numSamples; i++) {
    const bIdx = Math.min(totalBuckets - 1, Math.floor(bucketStart + i * step));
    const x = ((i / (numSamples - 1)) * widthPx).toFixed(1);
    top.push(`${x},${(midY - (wf.peaks[bIdx * 2 + 1] ?? 0) * ampScale).toFixed(1)}`);
    bottom.unshift(`${x},${(midY - (wf.peaks[bIdx * 2] ?? 0) * ampScale).toFixed(1)}`);
  }
  return `M ${top.join(" L ")} L ${bottom.join(" L ")} Z`;
}

const waveform = {
  bucket_rate: 50,
  peaks: Array.from({ length: 2 * 50 * 60 }, (_, i) => (i % 2 === 0 ? -1 : 1) * ((i % 37) / 37)),
};

describe("timeline waveform path", () => {
  it("matches the previous geometry exactly", () => {
    for (const [start, dur, width, height] of [
      [0, 180, 540, 40],
      [300, 90, 37, 28],
      [1500, 300, 1200, 60],
      [1790, 20, 200, 40],
    ]) {
      expect(waveformPathD(waveform, start, dur, 30, width, height)).toBe(
        legacyPathD(waveform, start, dur, 30, width, height)
      );
    }
  });

  it("returns null when there is too little data or width", () => {
    expect(waveformPathD(waveform, 0, 0, 30, 300, 40)).toBeNull();
    expect(waveformPathD(waveform, 0, 0, 30, 300, 40)).toBe(legacyPathD(waveform, 0, 0, 30, 300, 40));
    expect(waveformPathD(waveform, 0, 180, 30, 4, 40)).toBeNull();
  });

  it("reuses the cached path for the same clip geometry", () => {
    const wf = { bucket_rate: 50, peaks: waveform.peaks.slice() };
    const first = waveformPathD(wf, 0, 180, 30, 540, 40);
    wf.peaks[1] = 0.123; // mutation is invisible: cached by waveform object identity
    expect(waveformPathD(wf, 0, 180, 30, 540, 40)).toBe(first);
  });
});
