import { describe, expect, it } from "vitest";
import { OperationEnvelope } from "../types/command";
import { countPlannedVideoClips, formatDraftTime, getOrderedClipIds } from "./editPlanReview";

describe("formatDraftTime", () => {
  it("shows rounded seconds alongside exact frame time for integer and fractional rates", () => {
    expect(formatDraftTime({ value: 150, rate: { numerator: 30, denominator: 1 } })).toBe("~5.000 s (150 frames at 30/1 fps)");
    expect(formatDraftTime({ value: 30, rate: { numerator: 30000, denominator: 1001 } })).toBe("~1.001 s (30 frames at 30000/1001 fps)");
    expect(formatDraftTime({ value: 0, rate: { numerator: 0, denominator: 1 } })).toBe("0 frames at 0/1 fps");
  });
});

describe("countPlannedVideoClips", () => {
  it("counts only sourced video clips, excluding audio and subtitle items", () => {
    const rate = { numerator: 30, denominator: 1 };
    const operations: OperationEnvelope[] = [
      {
        operation_id: "video",
        operation: {
          type: "ClipCreate",
          payload: {
            track_id: "video-track",
            item_id: "video-1",
            name: "source.mp4",
            kind: "Video",
            asset_id: "video-asset",
            timeline_start: { value: 0, rate },
            duration: { value: 900, rate },
          },
        },
      },
      {
        operation_id: "audio",
        operation: {
          type: "ClipCreate",
          payload: {
            track_id: "audio-track",
            item_id: "audio-1",
            name: "music.wav",
            kind: "Audio",
            asset_id: "audio-asset",
            timeline_start: { value: 0, rate },
            duration: { value: 900, rate },
          },
        },
      },
      {
        operation_id: "subtitle",
        operation: {
          type: "ClipCreate",
          payload: {
            track_id: "text-track",
            item_id: "text-1",
            name: "Subtitle 1",
            kind: "Text",
            timeline_start: { value: 30, rate },
            duration: { value: 60, rate },
          },
        },
      },
    ];

    expect(countPlannedVideoClips(operations)).toBe(1);
  });
});

describe("getOrderedClipIds", () => {
  it("reads the final clip positions from a plan rather than its temporary parking moves", () => {
    const rate = { numerator: 30, denominator: 1 };
    const operations: OperationEnvelope[] = [
      { operation_id: "lift-a", operation: { type: "ClipMove", payload: { item_id: "a", target_track_id: "v1", target_start: { value: 30, rate } } } },
      { operation_id: "lift-b", operation: { type: "ClipMove", payload: { item_id: "b", target_track_id: "v1", target_start: { value: 60, rate } } } },
      { operation_id: "place-b", operation: { type: "ClipMove", payload: { item_id: "b", target_track_id: "v1", target_start: { value: 0, rate } } } },
      { operation_id: "place-a", operation: { type: "ClipMove", payload: { item_id: "a", target_track_id: "v1", target_start: { value: 30, rate } } } },
    ];
    expect(getOrderedClipIds(operations, ["a", "b"], "v1")).toEqual(["b", "a"]);
  });

  it("compares canonical times across different rates and keeps the original order when the plan is incomplete", () => {
    const operations: OperationEnvelope[] = [
      { operation_id: "move-a", operation: { type: "ClipMove", payload: { item_id: "a", target_track_id: "v1", target_start: { value: 20, rate: { numerator: 30, denominator: 1 } } } } },
      { operation_id: "move-b", operation: { type: "ClipMove", payload: { item_id: "b", target_track_id: "v1", target_start: { value: 1, rate: { numerator: 2, denominator: 1 } } } } },
    ];
    expect(getOrderedClipIds(operations, ["a", "b"], "v1")).toEqual(["b", "a"]);
    expect(getOrderedClipIds(operations.slice(0, 1), ["a", "b"], "v1")).toEqual(["a", "b"]);
  });
});
