import { describe, expect, it } from "vitest";
import { buildTimelinePasteOperations, captureTimelineClipboard } from "./timelineClipboard";
import { Project } from "../types/project";

const rate = { numerator: 30, denominator: 1 };
const project = {
  project_id: "clipboard-project",
  name: "Clipboard fixture",
  schema_version: 11,
  revision: 2,
  settings: { width: 1920, height: 1080, frame_rate: rate },
  assets: [],
  tracks: [
    {
      track_id: "video-a",
      name: "Video A",
      kind: "Video",
      items: [
        { item_id: "clip-a", timeline_range: { start: { value: 10, rate }, duration: { value: 30, rate } } },
        { item_id: "clip-c", timeline_range: { start: { value: 70, rate }, duration: { value: 20, rate } } },
      ],
    },
    {
      track_id: "video-b",
      name: "Video B",
      kind: "Video",
      items: [{ item_id: "clip-b", timeline_range: { start: { value: 25, rate }, duration: { value: 15, rate } } }],
    },
  ],
} as unknown as Project;

describe("timeline clipboard", () => {
  it("captures only selected clips and pastes a group preserving relative timing and tracks", () => {
    const clipboard = captureTimelineClipboard(project, new Set(["clip-b", "clip-a", "clip-c"]));
    expect(clipboard?.itemIds).toEqual(["clip-a", "clip-b", "clip-c"]);

    let id = 0;
    const operations = buildTimelinePasteOperations(project, clipboard!, 100, () => `id-${++id}`);
    expect(operations.map(({ operation }) => operation)).toEqual([
      {
        type: "ClipDuplicate",
        payload: { item_id: "clip-a", target_start: { value: 100, rate }, target_track_id: "video-a", new_item_id: "clip_paste_0_id-1" },
      },
      {
        type: "ClipDuplicate",
        payload: { item_id: "clip-b", target_start: { value: 115, rate }, target_track_id: "video-b", new_item_id: "clip_paste_1_id-2" },
      },
      {
        type: "ClipDuplicate",
        payload: { item_id: "clip-c", target_start: { value: 160, rate }, target_track_id: "video-a", new_item_id: "clip_paste_2_id-3" },
      },
    ]);
  });

  it("rejects cross-project and stale clipboard references before creating operations", () => {
    const clipboard = captureTimelineClipboard(project, new Set(["clip-a"]))!;
    expect(() => buildTimelinePasteOperations({ ...project, project_id: "other" }, clipboard, 50)).toThrow("different project");
    expect(() => buildTimelinePasteOperations(project, { ...clipboard, itemIds: ["deleted"] }, 50)).toThrow("no longer exist");
    expect(() => buildTimelinePasteOperations(project, clipboard, -1)).toThrow("non-negative project frame");
  });
});
