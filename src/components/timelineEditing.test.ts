import { describe, expect, it } from "vitest";
import { getTimelineGapClosePlan } from "./timelineEditing";
import { Project } from "../types/project";

const rate = { numerator: 30, denominator: 1 };
const fixture = {
  project_id: "gap-project",
  name: "Gap fixture",
  schema_version: 11,
  revision: 1,
  settings: { width: 1920, height: 1080, frame_rate: rate },
  tracks: [{
    track_id: "v1",
    name: "Video 1",
    kind: "Video",
    locked: false,
    items: [
      { item_id: "before", timeline_range: { start: { value: 0, rate }, duration: { value: 40, rate } } },
      { item_id: "selected", timeline_range: { start: { value: 50, rate }, duration: { value: 20, rate } } },
      { item_id: "after", timeline_range: { start: { value: 80, rate }, duration: { value: 10, rate } } },
    ],
  }],
} as unknown as Project;

describe("timeline gap close planning", () => {
  it("moves the selected clip and all downstream clips by the exact gap", () => {
    expect(getTimelineGapClosePlan(fixture, "selected")).toEqual({
      trackId: "v1",
      itemIds: ["selected", "after"],
      gapFrames: 10,
    });
  });

  it("does not offer an edit when there is no preceding gap or the track is locked", () => {
    expect(getTimelineGapClosePlan(fixture, "before")).toBeNull();
    expect(getTimelineGapClosePlan({
      ...fixture,
      tracks: [{ ...fixture.tracks[0], locked: true }],
    }, "selected")).toBeNull();
  });
});
