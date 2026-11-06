import { describe, expect, it } from "vitest";
import { TimelineItem } from "../types/project";
import { adjacentRollItems, timelineArrowDelta, trimRangeByKeyboard } from "./timelineKeyboard";

const rate = { numerator: 30, denominator: 1 } as const;

function clip(itemId: string, start: number, duration: number, kind: "Video" | "Audio" | "Text" = "Video"): TimelineItem {
  return {
    item_id: itemId,
    name: itemId,
    kind,
    timeline_range: {
      start: { value: start, rate },
      duration: { value: duration, rate },
    },
    source_range: {
      start: { value: start, rate },
      duration: { value: duration, rate },
    },
    speed: "Normal",
  } as TimelineItem;
}

describe("timeline keyboard editing", () => {
  it("maps horizontal arrows to single-frame and accelerated deltas", () => {
    expect(timelineArrowDelta("ArrowLeft", false)).toBe(-1);
    expect(timelineArrowDelta("ArrowRight", true)).toBe(10);
    expect(timelineArrowDelta("ArrowUp", false)).toBeNull();
  });

  it("trims either edge without allowing negative starts or empty clips", () => {
    expect(trimRangeByKeyboard(4, 8, "left", -10)).toEqual({ startFrame: 0, durationFrames: 12 });
    expect(trimRangeByKeyboard(4, 8, "left", 3)).toEqual({ startFrame: 7, durationFrames: 5 });
    expect(trimRangeByKeyboard(4, 1, "right", -10)).toEqual({ startFrame: 4, durationFrames: 1 });
  });

  it("resolves only a same-kind clip touching the selected edge for a roll", () => {
    const left = clip("left", 0, 30);
    const right = clip("right", 30, 45);
    const separated = clip("separated", 90, 20);
    expect(adjacentRollItems([left, right, separated], right, "left", rate)).toEqual({ left, right });
    expect(adjacentRollItems([left, right], left, "right", rate)).toEqual({ left, right });
    expect(adjacentRollItems([left, separated], left, "right", rate)).toBeNull();
    expect(adjacentRollItems([left, clip("text", 30, 20, "Text")], left, "right", rate)).toBeNull();
  });
});
