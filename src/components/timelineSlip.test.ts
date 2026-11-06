import { describe, expect, it } from "vitest";
import { Asset, TimelineItem, rationalTimeToSeconds } from "../types/project";
import { calculateSlipSourceRange } from "./timelineSlip";

const item = (speed: TimelineItem["speed"] = "Normal"): TimelineItem => ({
  item_id: "clip-1",
  name: "Clip",
  kind: "Video",
  asset_id: "asset-1",
  timeline_range: { start: { value: 60, rate: { numerator: 30, denominator: 1 } }, duration: { value: 30, rate: { numerator: 30, denominator: 1 } } },
  source_range: { start: { value: 30, rate: { numerator: 30, denominator: 1 } }, duration: { value: 30, rate: { numerator: 30, denominator: 1 } } },
  speed: speed ?? "Normal",
  volume: 1,
  opacity: 1,
  transform: { position_x: 0, position_y: 0, scale_x: 1, scale_y: 1, rotation: 0 },
  crop: { top: 0, bottom: 0, left: 0, right: 0 },
  placement_mode: "Fit",
  visual_effect: "None",
  keyframes: [],
} as TimelineItem);

const asset: Asset = {
  asset_id: "asset-1",
  name: "source.mp4",
  path: "source.mp4",
  media_kind: "Video",
  duration: { value: 180, rate: { numerator: 30, denominator: 1 } },
  file_size: 10,
};

describe("calculateSlipSourceRange", () => {
  it("moves only source time and scales by clip speed", () => {
    expect(calculateSlipSourceRange(item(), asset, 12, { numerator: 30, denominator: 1 })).toEqual({
      start: { value: 42, rate: { numerator: 30, denominator: 1 } },
      duration: { value: 30, rate: { numerator: 30, denominator: 1 } },
    });
    expect(rationalTimeToSeconds(calculateSlipSourceRange(item("Half"), asset, 12, { numerator: 30, denominator: 1 }).start)).toBeCloseTo(1.2);
    expect(rationalTimeToSeconds(calculateSlipSourceRange(item("Double"), asset, 12, { numerator: 30, denominator: 1 }).start)).toBeCloseTo(1.8);
  });

  it("converts timeline frames using the project and source rates", () => {
    const result = calculateSlipSourceRange(item(), asset, 15, { numerator: 60, denominator: 1 }).start;
    expect(rationalTimeToSeconds(result)).toBeCloseTo(1.25);
  });

  it("preserves a one-frame slip from a reduced source timebase", () => {
    const reduced = item();
    reduced.source_range = {
      start: { value: 150, rate: { numerator: 2, denominator: 1 } },
      duration: { value: 30, rate: { numerator: 2, denominator: 1 } },
    };
    const result = calculateSlipSourceRange(reduced, { ...asset, duration: { value: 3000, rate: { numerator: 30, denominator: 1 } } }, 1, { numerator: 30, denominator: 1 });

    expect(result).toEqual({
      start: { value: 2251, rate: { numerator: 30, denominator: 1 } },
      duration: { value: 30, rate: { numerator: 2, denominator: 1 } },
    });
  });

  it("rejects source bounds violations without changing the timeline range", () => {
    expect(() => calculateSlipSourceRange(item(), asset, -40, { numerator: 30, denominator: 1 })).toThrow(/before/);
    expect(() => calculateSlipSourceRange(item(), asset, 130, { numerator: 30, denominator: 1 })).toThrow(/beyond/);
    expect(item().timeline_range.start.value).toBe(60);
  });
});
