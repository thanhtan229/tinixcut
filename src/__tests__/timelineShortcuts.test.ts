import { describe, it, expect } from "vitest";
import { findClipForSplitAtPlayhead, isFocusInInput } from "../hooks/useTimelineShortcuts";
import { Project } from "../types/project";

describe("Phase 16 Shortcuts Helpers (P16-TP-29..31, P16-AC-44..48)", () => {
  const rate = { numerator: 30, denominator: 1 };
  const mockProject = {
    project_id: "p_split",
    name: "Split Test",
    schema_version: 3,
    revision: 1,
    settings: {
      width: 1920,
      height: 1080,
      frame_rate: rate,
    },
    tracks: [
      {
        track_id: "track_0",
        name: "Track 0 (lowest canonical)",
        kind: "Video",
        items: [
          {
            item_id: "item_t0_1",
            name: "T0 Clip 1",
            kind: "Video",
            timeline_range: { start: { value: 10, rate }, duration: { value: 50, rate } }, // 10..60
            source_range: { start: { value: 0, rate }, duration: { value: 50, rate } },
          },
        ],
      },
      {
        track_id: "track_1",
        name: "Track 1",
        kind: "Video",
        items: [
          {
            item_id: "item_t1_1",
            name: "T1 Clip 1",
            kind: "Video",
            timeline_range: { start: { value: 20, rate }, duration: { value: 40, rate } }, // 20..60
            source_range: { start: { value: 0, rate }, duration: { value: 40, rate } },
          },
        ],
      },
    ],
    assets: [],
  } as unknown as Project;

  it("P16-AC-44: splits primary selected clip first when playhead is in its strict interior", () => {
    // Playhead at 30: intersects both item_t0_1 (10..60) and item_t1_1 (20..60)
    // Primary is item_t1_1 on track 1
    const clip = findClipForSplitAtPlayhead(mockProject, "item_t1_1", 30);
    expect(clip).not.toBeNull();
    expect(clip?.item_id).toBe("item_t1_1");
  });

  it("P16-AC-44: falls back to lowest canonical track index when primary is unselected or does not intersect", () => {
    // Playhead at 30, primary is null -> choose lowest track (track_0 -> item_t0_1)
    const clip = findClipForSplitAtPlayhead(mockProject, null, 30);
    expect(clip).not.toBeNull();
    expect(clip?.item_id).toBe("item_t0_1");

    // Playhead at 30, primary is an item that doesn't exist or does not intersect
    const clip2 = findClipForSplitAtPlayhead(mockProject, "non_intersecting", 30);
    expect(clip2?.item_id).toBe("item_t0_1");
  });

  it("P16-AC-44: strict interior requirement — does NOT split at exact start or end boundary", () => {
    // Playhead at 10 (exact start of item_t0_1) -> no-op
    const atStart = findClipForSplitAtPlayhead(mockProject, "item_t0_1", 10);
    expect(atStart).toBeNull();

    // Playhead at 60 (exact end of item_t0_1 and item_t1_1) -> no-op
    const atEnd = findClipForSplitAtPlayhead(mockProject, "item_t0_1", 60);
    expect(atEnd).toBeNull();
  });

  it("P16-AC-44: returns null when playhead is in an empty timeline interval", () => {
    const emptyInterval = findClipForSplitAtPlayhead(mockProject, null, 5);
    expect(emptyInterval).toBeNull();

    const pastAllClips = findClipForSplitAtPlayhead(mockProject, null, 200);
    expect(pastAllClips).toBeNull();
  });

  it("P16-AC-47: isFocusInInput detects input, textarea, and contenteditable", () => {
    // When no element or default body/null
    expect(isFocusInInput(null)).toBe(false);
    expect(isFocusInInput({ tagName: "DIV", isContentEditable: false })).toBe(false);
    expect(isFocusInInput({ tagName: "BUTTON", isContentEditable: false })).toBe(false);

    // Input tag
    expect(isFocusInInput({ tagName: "INPUT" })).toBe(true);
    expect(isFocusInInput({ tagName: "input" })).toBe(true);

    // Textarea tag
    expect(isFocusInInput({ tagName: "TEXTAREA" })).toBe(true);
    expect(isFocusInInput({ tagName: "textarea" })).toBe(true);

    // ContentEditable element
    expect(isFocusInInput({ tagName: "DIV", isContentEditable: true })).toBe(true);
    expect(isFocusInInput({ tagName: "SPAN", isContentEditable: true })).toBe(true);
  });
});
