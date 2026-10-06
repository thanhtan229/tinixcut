import { describe, it, expect } from "vitest";
import {
  buildSnapTargets,
  calculateSnap,
  SnapCandidate,
} from "../utils/timelineSnapping";

describe("Phase 16 Snapping Engine (P16-TP-06..11, P16-AC-12..20)", () => {
  it("builds snap targets for playhead, timeline zero, and unselected clip edges", () => {
    const playheadFrame = 45;
    const unselectedClips = [
      { itemId: "c1", start: 10, end: 40 },
      { itemId: "c2", start: 60, end: 120 },
    ];

    const targets = buildSnapTargets(playheadFrame, unselectedClips);

    expect(targets.some((t) => t.kind === "playhead" && t.frame === 45)).toBe(true);
    expect(targets.some((t) => t.kind === "zero" && t.frame === 0)).toBe(true);
    expect(targets.some((t) => t.kind === "clipEdge" && t.frame === 10 && t.sourceItemId === "c1")).toBe(true);
    expect(targets.some((t) => t.kind === "clipEdge" && t.frame === 40 && t.sourceItemId === "c1")).toBe(true);
    expect(targets.some((t) => t.kind === "clipEdge" && t.frame === 60 && t.sourceItemId === "c2")).toBe(true);
    expect(targets.some((t) => t.kind === "clipEdge" && t.frame === 120 && t.sourceItemId === "c2")).toBe(true);
  });

  it("P16-AC-13: snaps when pixel distance <= 8px exactly, and rejects when > 8px", () => {
    // Zoom = 2 px per frame -> 4 frames = 8px (eligible), 5 frames = 10px (ineligible)
    const zoom = 2;
    const targets = [{ frame: 50, kind: "clipEdge" as const, stableOrder: 0 }];
    const candidates: SnapCandidate[] = [{ anchorFrame: 46, anchorKind: "start" }];

    // Raw delta = 0 -> proposed anchor = 46. Distance to 50 is 4 frames = 8px -> ELIGIBLE
    const resultEligible = calculateSnap(candidates, 0, targets, zoom, true, false);
    expect(resultEligible.snapped).toBe(true);
    expect(resultEligible.snappedDeltaFrames).toBe(4);
    expect(resultEligible.distancePx).toBe(8);

    // Proposed anchor = 45. Distance to 50 is 5 frames = 10px -> INELIGIBLE
    const candidatesFar: SnapCandidate[] = [{ anchorFrame: 45, anchorKind: "start" }];
    const resultIneligible = calculateSnap(candidatesFar, 0, targets, zoom, true, false);
    expect(resultIneligible.snapped).toBe(false);
    expect(resultIneligible.snappedDeltaFrames).toBe(0);
  });

  it("P16-AC-13: does NOT force a 1-frame minimum threshold at high zoom", () => {
    // At zoom = 16 px/frame, 1 frame = 16px which is > 8px!
    // A target 1 full frame away (16px) must NOT be snapped.
    const zoom = 16;
    const targets = [{ frame: 10, kind: "clipEdge" as const, stableOrder: 0 }];
    const candidates: SnapCandidate[] = [{ anchorFrame: 9, anchorKind: "start" }];

    const result = calculateSnap(candidates, 0, targets, zoom, true, false);
    // Distance = 1 frame * 16 px/frame = 16px > 8px -> MUST NOT SNAP
    expect(result.snapped).toBe(false);
    expect(result.snappedDeltaFrames).toBe(0);
  });

  it("P16-AC-14: snapped commit uses exact target semantic frame", () => {
    const zoom = 4;
    const targets = [{ frame: 100, kind: "clipEdge" as const, stableOrder: 0 }];
    const candidates: SnapCandidate[] = [{ anchorFrame: 99, anchorKind: "start" }];

    const result = calculateSnap(candidates, 0, targets, zoom, true, false);
    expect(result.snapped).toBe(true);
    // Delta must be exact integer 1 frame, landing exactly at 100
    expect(result.snappedDeltaFrames).toBe(1);
    expect(candidates[0].anchorFrame + result.snappedDeltaFrames).toBe(100);
  });

  it("P16-AC-15: tie-breaking priority Playhead > Zero > ClipEdge", () => {
    const zoom = 1;
    // Playhead and ClipEdge both at distance 4px
    const targets = [
      { frame: 14, kind: "clipEdge" as const, stableOrder: 0 },
      { frame: 6, kind: "playhead" as const, stableOrder: 1 },
    ];
    // Anchor at 10: distance to 14 is 4px, distance to 6 is 4px
    const candidates: SnapCandidate[] = [{ anchorFrame: 10, anchorKind: "start" }];

    const result = calculateSnap(candidates, 0, targets, zoom, true, false);
    expect(result.snapped).toBe(true);
    expect(result.target?.kind).toBe("playhead");
    expect(result.snappedDeltaFrames).toBe(-4);
  });

  it("P16-AC-15: tie-breaking priority Zero > ClipEdge", () => {
    const zoom = 1;
    const targets = [
      { frame: 8, kind: "clipEdge" as const, stableOrder: 0 },
      { frame: 0, kind: "zero" as const, stableOrder: 1 },
    ];
    // Anchor at 4: distance to 0 is 4px, distance to 8 is 4px
    const candidates: SnapCandidate[] = [{ anchorFrame: 4, anchorKind: "start" }];

    const result = calculateSnap(candidates, 0, targets, zoom, true, false);
    expect(result.snapped).toBe(true);
    expect(result.target?.kind).toBe("zero");
  });

  it("P16-AC-15: tie-breaking earlier semantic target time for equal kinds", () => {
    const zoom = 1;
    const targets = [
      { frame: 20, kind: "clipEdge" as const, stableOrder: 0, sourceItemId: "b" },
      { frame: 10, kind: "clipEdge" as const, stableOrder: 1, sourceItemId: "a" },
    ];
    // Anchor at 15: distance to 10 is 5px, distance to 20 is 5px
    const candidates: SnapCandidate[] = [{ anchorFrame: 15, anchorKind: "start" }];

    const result = calculateSnap(candidates, 0, targets, zoom, true, false);
    expect(result.snapped).toBe(true);
    expect(result.target?.frame).toBe(10);
  });

  it("P16-AC-16: Alt key suppresses snapping", () => {
    const zoom = 2;
    const targets = [{ frame: 50, kind: "clipEdge" as const, stableOrder: 0 }];
    const candidates: SnapCandidate[] = [{ anchorFrame: 48, anchorKind: "start" }];

    // Alt key held = true
    const result = calculateSnap(candidates, 0, targets, zoom, true, true);
    expect(result.snapped).toBe(false);
    expect(result.snappedDeltaFrames).toBe(0);
  });

  it("P16-AC-17: disabled snapEnabled suppresses snapping", () => {
    const zoom = 2;
    const targets = [{ frame: 50, kind: "clipEdge" as const, stableOrder: 0 }];
    const candidates: SnapCandidate[] = [{ anchorFrame: 48, anchorKind: "start" }];

    // snapEnabled = false
    const result = calculateSnap(candidates, 0, targets, zoom, false, false);
    expect(result.snapped).toBe(false);
    expect(result.snappedDeltaFrames).toBe(0);
  });

  it("P16-AC-18: selected clips excluded from snap targets (no self-snapping)", () => {
    const playheadFrame = 0;
    const selectedItemIds = new Set(["sel1", "sel2"]);
    const allClips = [
      { itemId: "sel1", start: 10, end: 40 },
      { itemId: "sel2", start: 50, end: 80 },
      { itemId: "other", start: 100, end: 150 },
    ];

    const unselectedEdges = allClips
      .filter((c) => !selectedItemIds.has(c.itemId))
      .map((c) => ({ start: c.start, end: c.end, itemId: c.itemId }));

    const targets = buildSnapTargets(playheadFrame, unselectedEdges);

    expect(targets.some((t) => t.sourceItemId === "sel1")).toBe(false);
    expect(targets.some((t) => t.sourceItemId === "sel2")).toBe(false);
    expect(targets.some((t) => t.sourceItemId === "other")).toBe(true);
  });

  it("P16-AC-19: evaluates candidate anchors of primary dragged clip (start and end)", () => {
    const zoom = 2;
    // Clip start: 20, end: 50. Target at 52 (distance to end is 2 frames = 4px <= 8px)
    const targets = [{ frame: 52, kind: "clipEdge" as const, stableOrder: 0 }];
    const candidates: SnapCandidate[] = [
      { anchorFrame: 20, anchorKind: "start" },
      { anchorFrame: 50, anchorKind: "end" },
    ];

    const result = calculateSnap(candidates, 0, targets, zoom, true, false);
    expect(result.snapped).toBe(true);
    expect(result.snappedDeltaFrames).toBe(2);
    // End anchor (50) snapped to 52 by moving +2 frames
    expect(50 + result.snappedDeltaFrames).toBe(52);
  });

  it("snaps single-anchor for trim head and trim tail", () => {
    const zoom = 2;
    const targets = [{ frame: 30, kind: "playhead" as const, stableOrder: 0 }];

    // Trim head: proposed start is 28 (distance 2 frames = 4px)
    const headCandidate: SnapCandidate[] = [{ anchorFrame: 28, anchorKind: "start" }];
    const headResult = calculateSnap(headCandidate, 0, targets, zoom, true, false);
    expect(headResult.snapped).toBe(true);
    expect(headResult.snappedDeltaFrames).toBe(2);

    // Trim tail: proposed end is 32 (distance 2 frames = 4px)
    const tailCandidate: SnapCandidate[] = [{ anchorFrame: 32, anchorKind: "end" }];
    const tailResult = calculateSnap(tailCandidate, 0, targets, zoom, true, false);
    expect(tailResult.snapped).toBe(true);
    expect(tailResult.snappedDeltaFrames).toBe(-2);
  });
});
