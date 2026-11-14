import { describe, expect, it } from "vitest";
import { reverseProxyKey, reverseProxySeconds, reversedVideoItems } from "./reversePreview";
import { TimelineItem } from "../types/project";

const rate = { numerator: 30, denominator: 1 };

function clip(overrides: Partial<TimelineItem> = {}): TimelineItem {
  return {
    item_id: "clip_a",
    asset_id: "ast_a",
    kind: "Video",
    reverse: true,
    source_range: { start: { value: 30, rate }, duration: { value: 90, rate } },
    ...overrides,
  } as unknown as TimelineItem;
}

describe("reverse preview mapping", () => {
  it("maps the forward source time onto the reversed file", () => {
    // Source range 1 s..4 s: forward source 1 s is proxy 0 s, forward 2.5 s is proxy 1.5 s.
    expect(reverseProxySeconds(1, clip())).toBeCloseTo(0);
    expect(reverseProxySeconds(2.5, clip())).toBeCloseTo(1.5);
  });

  it("stays inside the file when the sampled time is outside the range", () => {
    expect(reverseProxySeconds(0, clip())).toBe(0);
    expect(reverseProxySeconds(99, clip())).toBeCloseTo(3);
  });

  it("changes the key when the source range or media changes", () => {
    const base = reverseProxyKey(clip(), "a.mp4");
    expect(reverseProxyKey(clip(), "a.mp4")).toBe(base);
    expect(reverseProxyKey(clip(), "b.mp4")).not.toBe(base);
    expect(
      reverseProxyKey(
        clip({ source_range: { start: { value: 30, rate }, duration: { value: 60, rate } } }),
        "a.mp4",
      ),
    ).not.toBe(base);
  });

  it("selects only active reversed video clips", () => {
    const items = [
      clip(),
      clip({ item_id: "fwd", reverse: false }),
      clip({ item_id: "aud", kind: "Audio" }),
      clip({ item_id: "idle" }),
    ];
    const active = new Set(["clip_a", "fwd", "aud"]);
    expect(reversedVideoItems(items, active).map((item) => item.item_id)).toEqual(["clip_a"]);
  });
});
