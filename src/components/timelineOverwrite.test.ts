import { describe, expect, it } from "vitest";
import { Project, TimelineItem } from "../types/project";
import { getTimelineOverwritePlan } from "./timelineOverwrite";

function makeProject(locked = false): Project {
  const item = (itemId: string, start: number, duration = 30): TimelineItem => ({
    item_id: itemId,
    name: itemId,
    kind: "Video",
    asset_id: "asset",
    timeline_range: {
      start: { value: start, rate: { numerator: 30, denominator: 1 } },
      duration: { value: duration, rate: { numerator: 30, denominator: 1 } },
    },
    source_range: {
      start: { value: 0, rate: { numerator: 30, denominator: 1 } },
      duration: { value: duration, rate: { numerator: 30, denominator: 1 } },
    },
    transform: { position_x: 0, position_y: 0, scale_x: 1, scale_y: 1, rotation: 0 },
    crop: { top: 0, bottom: 0, left: 0, right: 0 },
    opacity: 1,
    volume: 1,
    placement_mode: "Fit",
    visual_effect: "None",
  });
  return {
    project_id: "project-1",
    name: "Project",
    created_at: "2026-11-04T00:00:00Z",
    updated_at: "2026-11-04T00:00:00Z",
    schema_version: 11,
    revision: 0,
    settings: { width: 1920, height: 1080, frame_rate: { numerator: 30, denominator: 1 }, sample_rate: 48000, channels: 2, canvas_background_color: "#000000" },
    assets: [],
    tracks: [{
      track_id: "track-1",
      name: "Video 1",
      kind: "Video",
      muted: false,
      locked,
      items: [item("before", 0), item("spanning", 30, 90), item("after", 120)],
      transitions: [],
      audio_transitions: [],
    }],
    client_registry: { clients: {}, min_accepted_epoch: 0, current_epoch: 0, max_active_clients: 16 },
  } as Project;
}

describe("getTimelineOverwritePlan", () => {
  it("splits both overwrite boundaries and deletes only the covered middle", () => {
    const plan = getTimelineOverwritePlan(makeProject(), "track-1", 45, 30, "overwrite-1");
    expect(plan.splits).toEqual([
      { itemId: "spanning", frame: 45, rightItemId: "overwrite-1_right_spanning" },
      { itemId: "overwrite-1_right_spanning", frame: 75, rightItemId: "overwrite-1_tail_spanning" },
    ]);
    expect(plan.deleteItemIds).toEqual(["overwrite-1_right_spanning"]);
    expect(plan.durationFrames).toBe(30);
  });

  it("deletes fully covered clips, preserves a tail when overwrite ends inside a clip, and rejects locked tracks", () => {
    const plan = getTimelineOverwritePlan(makeProject(), "track-1", 0, 60, "overwrite-2");
    expect(plan.deleteItemIds).toContain("before");
    expect(plan.deleteItemIds).toContain("spanning");
    expect(plan.splits).toContainEqual({ itemId: "spanning", frame: 60, rightItemId: "overwrite-2_tail_spanning" });
    expect(() => getTimelineOverwritePlan(makeProject(true), "track-1", 0, 30, "locked")).toThrow(/locked/);
  });
});
