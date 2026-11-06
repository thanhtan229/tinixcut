import { describe, expect, it } from "vitest";
import { Project, Track, TimelineItem } from "../types/project";
import { getTimelineInsertPlan } from "./timelineInsert";

function makeProject(locked = false): Project {
  const item = (itemId: string, start: number): TimelineItem => ({
    item_id: itemId,
    name: itemId,
    kind: "Video",
    asset_id: "asset",
    timeline_range: { start: { value: start, rate: { numerator: 30, denominator: 1 } }, duration: { value: 30, rate: { numerator: 30, denominator: 1 } } },
    source_range: { start: { value: 0, rate: { numerator: 30, denominator: 1 } }, duration: { value: 30, rate: { numerator: 30, denominator: 1 } } },
    speed: "Normal",
    volume: 1,
    opacity: 1,
    transform: { position_x: 0, position_y: 0, scale_x: 1, scale_y: 1, rotation: 0 },
    crop: { top: 0, bottom: 0, left: 0, right: 0 },
    placement_mode: "Fit",
    visual_effect: "None",
    keyframes: [],
  });
  const track: Track = {
    track_id: "track-1",
    name: "Video 1",
    kind: "Video",
    muted: false,
    locked,
    items: [item("clip-a", 0), item("clip-b", 60), item("clip-c", 120)],
    transitions: [],
    audio_transitions: [],
  };
  return {
    project_id: "project-1",
    schema_version: 11,
    revision: 0,
    settings: { width: 1920, height: 1080, frame_rate: { numerator: 30, denominator: 1 }, sample_rate: 48000, channels: 2 },
    assets: [{ asset_id: "asset", name: "source.mp4", path: "source.mp4", media_kind: "Video", duration: { value: 300, rate: { numerator: 30, denominator: 1 } }, file_size: 10, dimensions: null }],
    tracks: [track],
    client_registry: { clients: {} },
  } as Project;
}

describe("getTimelineInsertPlan", () => {
  it("splits a clip at the insertion frame and shifts its right half and later clips", () => {
    const plan = getTimelineInsertPlan(makeProject(), "track-1", 15, 45, "clip-split-right");
    expect(plan.splitItemId).toBe("clip-a");
    expect(plan.downstreamItemIds).toEqual(["clip-split-right", "clip-b", "clip-c"]);
  });

  it("does not split when insertion is on a cut and rejects locked tracks", () => {
    expect(getTimelineInsertPlan(makeProject(), "track-1", 60, 30, "unused")).toMatchObject({
      splitItemId: null,
      downstreamItemIds: ["clip-b", "clip-c"],
    });
    expect(() => getTimelineInsertPlan(makeProject(true), "track-1", 60, 30, "unused")).toThrow(/locked/);
  });
});
