import { describe, expect, it } from "vitest";
import { buildCaptionMergeOperations, buildCaptionTimingOperations, canMergeCaptions, canSplitCaptionAtFrame, captionMatchesSearch, collectCaptionRows, getCaptionTimingNotices, isValidCaptionDurationFrames, isValidCaptionStartFrame } from "./captionEditor";
import type { Project, TimelineItem } from "../types/project";

const item = (id: string, start: number, kind: TimelineItem["kind"] = "Text"): TimelineItem => ({
  item_id: id,
  name: id,
  kind,
  asset_id: undefined,
  timeline_range: { start: { value: start, rate: { numerator: 30, denominator: 1 } }, duration: { value: 30, rate: { numerator: 30, denominator: 1 } } },
  source_range: { start: { value: 0, rate: { numerator: 30, denominator: 1 } }, duration: { value: 30, rate: { numerator: 30, denominator: 1 } } },
  transform: { position_x: 0, position_y: 0, scale_x: 1, scale_y: 1, rotation: 0 },
  crop: { left: 0, right: 0, top: 0, bottom: 0 },
  opacity: 1,
  volume: 1,
  text_content: id === "caption-later" ? "Xin chào Việt Nam" : undefined,
});

const project = {
  settings: { frame_rate: { numerator: 30, denominator: 1 } },
  tracks: [
    { name: "Captions", items: [item("caption-later", 60), item("caption-first", 0), item("video", 30, "Video")] },
  ],
} as Project;

describe("caption editor helpers", () => {
  it("finds gaps and nested overlaps within each track using pending frame edits", () => {
    const p = { ...project, tracks: [
      { track_id: "one", name: "One", items: [item("a", 0), item("b", 10), item("c", 20), item("d", 50), item("e", 90)] },
      { track_id: "two", name: "Two", items: [item("f", 10), item("g", 40)] },
    ] } as Project;
    const rows = collectCaptionRows(p);
    const result = getCaptionTimingNotices(rows, p.settings.frame_rate, {
      a: { start_frame: 0, duration_frames: 100 },
      b: { start_frame: 10, duration_frames: 5 },
      c: { start_frame: 20, duration_frames: 5 },
      d: { start_frame: 110, duration_frames: 10 },
      e: { start_frame: 120, duration_frames: 10 },
    });
    expect(result.notices).toEqual({ b: { kind: "overlap", frames: 5 }, c: { kind: "overlap", frames: 5 }, d: { kind: "gap", frames: 10 } });
    expect(result.invalidTrackIds.size).toBe(0);
  });

  it("suppresses the whole invalid track without hiding notices on other tracks", () => {
    const p = { ...project, tracks: [
      { track_id: "one", name: "One", items: [item("a", 0), item("b", 60)] },
      { track_id: "two", name: "Two", items: [item("c", 0), item("d", 60)] },
    ] } as Project;
    for (const draft of [{ start_frame: NaN, duration_frames: 30 }, { start_frame: 0, duration_frames: 0 }, { start_frame: Number.MAX_SAFE_INTEGER, duration_frames: 30 }]) {
      const result = getCaptionTimingNotices(collectCaptionRows(p), p.settings.frame_rate, { a: draft });
      expect([...result.invalidTrackIds]).toEqual(["one"]);
      expect([...result.invalidItemIds]).toEqual(["a"]);
      expect(result.notices).toEqual({ d: { kind: "gap", frames: 30 } });
    }
  });
  it("lists only text clips in timeline order", () => {
    expect(collectCaptionRows(project).map(({ item: caption }) => caption.item_id))
      .toEqual(["caption-first", "caption-later"]);
  });

  it("searches case-insensitively and splits only inside the clip range", () => {
    const later = item("caption-later", 60);
    expect(captionMatchesSearch("Xin chào Việt Nam", "  VIỆT ")).toBe(true);
    expect(canSplitCaptionAtFrame(later, 75, project.settings.frame_rate)).toBe(true);
    expect(canSplitCaptionAtFrame(later, 60, project.settings.frame_rate)).toBe(false);
    expect(canSplitCaptionAtFrame(later, 90, project.settings.frame_rate)).toBe(false);
  });

  it("accepts only nonnegative safe integer frame positions", () => {
    expect(isValidCaptionStartFrame(0)).toBe(true);
    expect(isValidCaptionStartFrame(120)).toBe(true);
    expect(isValidCaptionStartFrame(-1)).toBe(false);
    expect(isValidCaptionStartFrame(1.5)).toBe(false);
    expect(isValidCaptionStartFrame(Number.MAX_SAFE_INTEGER + 1)).toBe(false);
    expect(isValidCaptionDurationFrames(1)).toBe(true);
    expect(isValidCaptionDurationFrames(0)).toBe(false);
  });

  it("builds a move when only the start changes and a trim when duration changes", () => {
    const operations = buildCaptionTimingOperations(project, [
      { item_id: "caption-first", start_frame: 12, duration_frames: 30 },
      { item_id: "caption-later", start_frame: 60, duration_frames: 30 },
    ], 99);
    expect(operations).toHaveLength(1);
    expect(operations[0].operation).toMatchObject({
      type: "ClipMove",
      payload: { item_id: "caption-first", target_start: { value: 12, rate: project.settings.frame_rate } },
    });
    expect(operations[0].operation_id).toBe("op_caption_move_caption-first_99");

    const trim = buildCaptionTimingOperations(project, [
      { item_id: "caption-first", start_frame: 12, duration_frames: 60 },
    ], 100);
    expect(trim[0].operation).toMatchObject({
      type: "ClipTrim",
      payload: {
        item_id: "caption-first",
        new_timeline_range: { start: { value: 12, rate: project.settings.frame_rate }, duration: { value: 60, rate: project.settings.frame_rate } },
        new_source_range: { start: { value: 0, rate: project.settings.frame_rate }, duration: { value: 60, rate: project.settings.frame_rate } },
      },
    });
    expect(trim[0].operation_id).toBe("op_caption_trim_caption-first_100");
    expect(buildCaptionTimingOperations(project, [{ item_id: "caption-first", start_frame: 0, duration_frames: 30 }])).toEqual([]);
    expect(() => buildCaptionTimingOperations(project, [{ item_id: "video", start_frame: 1, duration_frames: 30 }])).toThrow(/no longer exists/);
    expect(() => buildCaptionTimingOperations(project, [{ item_id: "caption-first", start_frame: -1, duration_frames: 30 }])).toThrow(/nonnegative whole frame/);
    expect(() => buildCaptionTimingOperations(project, [{ item_id: "caption-first", start_frame: 1, duration_frames: 0 }])).toThrow(/positive whole-frame duration/);
  });

  it("rejects a duration that cannot be represented by the text clip source time base", () => {
    const fractionalSourceProject = {
      ...project,
      tracks: [{
        ...project.tracks[0],
        items: [
          { ...item("fractional-caption", 0), source_range: {
            start: { value: 0, rate: { numerator: 24, denominator: 1 } },
            duration: { value: 24, rate: { numerator: 24, denominator: 1 } },
          } },
        ],
      }],
    } as Project;
    expect(() => buildCaptionTimingOperations(fractionalSourceProject, [
      { item_id: "fractional-caption", start_frame: 1, duration_frames: 1 },
    ])).toThrow(/source time grid/);
  });

  it("rejects timing edits on locked tracks", () => {
    const lockedProject = {
      ...project,
      tracks: project.tracks.map((track) => ({ ...track, locked: true })),
    } as Project;
    expect(() => buildCaptionTimingOperations(lockedProject, [{ item_id: "caption-first", start_frame: 5, duration_frames: 30 }])).toThrow(/locked track/);
  });

  it("merges adjacent captions atomically and rejects gaps or unrelated tracks", () => {
    const mergeProject = {
      ...project,
      tracks: [{
        ...project.tracks[0],
        items: [
          { ...item("left", 0), text_content: "Xin chào" },
          { ...item("right", 30), text_content: "Việt Nam" },
        ],
      }],
    } as Project;
    expect(canMergeCaptions(mergeProject, "left", "right")).toBe(true);
    const ops = buildCaptionMergeOperations(mergeProject, "left", "right", 5);
    expect(ops).toHaveLength(3);
    expect(ops[0].operation).toEqual({ type: "ClipTextSet", payload: { item_id: "left", text: "Xin chào\nViệt Nam" } });
    expect(ops[1].operation).toEqual({ type: "ClipDelete", payload: { item_id: "right" } });
    expect(ops[2].operation).toMatchObject({ type: "ClipTrim", payload: { item_id: "left", new_timeline_range: { duration: { value: 60, rate: project.settings.frame_rate } } } });
    expect(canMergeCaptions({ ...mergeProject, tracks: [{ ...mergeProject.tracks[0], items: [mergeProject.tracks[0].items[0], { ...mergeProject.tracks[0].items[1], timeline_range: { ...mergeProject.tracks[0].items[1].timeline_range, start: { value: 45, rate: project.settings.frame_rate } } }] }] } as Project, "left", "right")).toBe(false);
  });
});
