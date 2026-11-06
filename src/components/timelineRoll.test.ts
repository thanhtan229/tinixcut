import { describe, expect, it } from "vitest";
import { Project, TimelineItem } from "../types/project";
import { getTimelineRollPlan } from "./timelineRoll";

function makeProject(locked = false): Project {
  const item = (itemId: string, start: number, sourceStart: number, speed: TimelineItem["speed"] = "Normal"): TimelineItem => ({
    item_id: itemId,
    name: itemId,
    kind: "Video",
    asset_id: "asset",
    timeline_range: {
      start: { value: start, rate: { numerator: 30, denominator: 1 } },
      duration: { value: 30, rate: { numerator: 30, denominator: 1 } },
    },
    source_range: {
      start: { value: sourceStart, rate: { numerator: 30, denominator: 1 } },
      duration: { value: speed === "Half" ? 15 : 30, rate: { numerator: 30, denominator: 1 } },
    },
    speed,
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
    assets: [{ asset_id: "asset", name: "source.mp4", path: "source.mp4", media_kind: "Video", duration: { value: 300, rate: { numerator: 30, denominator: 1 } }, file_size: 10 }],
    tracks: [{
      track_id: "track-1",
      name: "Video 1",
      kind: "Video",
      muted: false,
      locked,
      items: [item("left", 0, 30), item("right", 30, 60)],
      transitions: [],
      audio_transitions: [],
    }],
    client_registry: { clients: {}, min_accepted_epoch: 0, current_epoch: 0, max_active_clients: 16 },
  } as Project;
}

describe("getTimelineRollPlan", () => {
  it("moves a cut while preserving total duration and source continuity", () => {
    const plan = getTimelineRollPlan(makeProject(), "left", "right", 6);
    expect(plan.leftTimelineRange).toMatchObject({ start: { value: 0 }, duration: { value: 36 } });
    expect(plan.rightTimelineRange).toMatchObject({ start: { value: 36 }, duration: { value: 24 } });
    expect(plan.leftSourceRange).toMatchObject({ start: { value: 30 }, duration: { value: 36 } });
    expect(plan.rightSourceRange).toMatchObject({ start: { value: 66 }, duration: { value: 24 } });
    expect(plan.leftTimelineRange.duration.value + plan.rightTimelineRange.duration.value).toBe(60);
  });

  it("supports negative and speed-adjusted rolls", () => {
    const reverse = getTimelineRollPlan(makeProject(), "left", "right", -6);
    expect(reverse.leftSourceRange.duration.value).toBe(24);
    expect(reverse.rightSourceRange.start.value).toBe(54);
    const halfSpeed = makeProject();
    halfSpeed.tracks[0].items[0].speed = "Half";
    halfSpeed.tracks[0].items[1].speed = "Half";
    halfSpeed.tracks[0].items[0].source_range.duration.value = 15;
    halfSpeed.tracks[0].items[1].source_range.duration.value = 15;
    const half = getTimelineRollPlan(halfSpeed, "left", "right", 6);
    expect(half.leftSourceRange.duration).toEqual({ value: 36, rate: { numerator: 60, denominator: 1 } });
    expect(half.rightSourceRange.start).toEqual({ value: 126, rate: { numerator: 60, denominator: 1 } });
  });

  it("keeps one-frame rolls exact when split clips use a reduced source timebase", () => {
    const project = makeProject();
    const [left, right] = project.tracks[0].items;
    left.timeline_range.duration = { value: 450, rate: { numerator: 30, denominator: 1 } };
    right.timeline_range.start = { value: 450, rate: { numerator: 30, denominator: 1 } };
    right.timeline_range.duration = { value: 450, rate: { numerator: 30, denominator: 1 } };
    left.source_range = {
      start: { value: 10, rate: { numerator: 2, denominator: 1 } },
      duration: { value: 30, rate: { numerator: 2, denominator: 1 } },
    };
    right.source_range = {
      start: { value: 40, rate: { numerator: 2, denominator: 1 } },
      duration: { value: 30, rate: { numerator: 2, denominator: 1 } },
    };
    project.assets[0].duration = { value: 1200, rate: { numerator: 30, denominator: 1 } };

    const plan = getTimelineRollPlan(project, "left", "right", 1);

    expect(plan.leftSourceRange).toEqual({
      start: { value: 150, rate: { numerator: 30, denominator: 1 } },
      duration: { value: 451, rate: { numerator: 30, denominator: 1 } },
    });
    expect(plan.rightSourceRange).toEqual({
      start: { value: 601, rate: { numerator: 30, denominator: 1 } },
      duration: { value: 449, rate: { numerator: 30, denominator: 1 } },
    });
  });

  it("rejects non-adjacent, locked, empty, and out-of-source rolls", () => {
    expect(() => getTimelineRollPlan(makeProject(), "left", "right", 0)).toThrow(/non-zero/);
    expect(() => getTimelineRollPlan(makeProject(), "left", "right", 30)).toThrow(/empty/);
    expect(() => getTimelineRollPlan(makeProject(true), "left", "right", 1)).toThrow(/locked/);
    const separated = makeProject();
    separated.tracks[0].items[1].timeline_range.start.value = 31;
    expect(() => getTimelineRollPlan(separated, "left", "right", 1)).toThrow(/same cut/);
    const noHandle = makeProject();
    noHandle.tracks[0].items[1].source_range.start.value = 0;
    expect(() => getTimelineRollPlan(noHandle, "left", "right", -1)).toThrow(/source range/);
  });
});
