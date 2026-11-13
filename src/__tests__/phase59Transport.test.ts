import { describe, expect, it } from "vitest";
import {
  chordFromEvent,
  DEFAULT_SHORTCUTS,
  effectiveShortcuts,
  findConflicts,
  overridesFromMap,
  resolveShortcut,
  SHORTCUT_ACTIONS,
  validateRemap,
} from "../hooks/shortcutMap";
import {
  describeShuttle,
  nextShuttle,
  SHUTTLE_IDLE,
  shuttleFramesPerTick,
  stepPlayhead,
  usesRealPlayback,
} from "../hooks/shuttle";
import { exportRangeFor, planRippleDeleteRange, selectedSpan, setMark } from "../hooks/inOutRange";
import type { Project } from "../types/project";

const key = (k: string, mods: Partial<{ ctrlKey: boolean; altKey: boolean; shiftKey: boolean }> = {}, code?: string) => ({
  key: k,
  code,
  ...mods,
});

describe("shortcut map", () => {
  it("has no conflicts among the built-in chords", () => {
    expect(findConflicts(DEFAULT_SHORTCUTS)).toEqual([]);
  });

  it("resolves the same keys the old hard-coded handler did", () => {
    expect(resolveShortcut(key(" ", {}, "Space"), DEFAULT_SHORTCUTS)).toBe("play_pause");
    expect(resolveShortcut(key("c", { ctrlKey: true }), DEFAULT_SHORTCUTS)).toBe("copy");
    expect(resolveShortcut(key("V", { ctrlKey: true }), DEFAULT_SHORTCUTS)).toBe("paste");
    expect(resolveShortcut(key("k", { ctrlKey: true }), DEFAULT_SHORTCUTS)).toBe("split");
    expect(resolveShortcut(key("Delete", { shiftKey: true }), DEFAULT_SHORTCUTS)).toBe("ripple_delete");
    expect(resolveShortcut(key("Delete"), DEFAULT_SHORTCUTS)).toBe("delete");
    expect(resolveShortcut(key("Backspace"), DEFAULT_SHORTCUTS)).toBe("delete");
    expect(resolveShortcut(key("["), DEFAULT_SHORTCUTS)).toBe("trim_head");
    expect(resolveShortcut(key("+", { shiftKey: true }), DEFAULT_SHORTCUTS)).toBe("zoom_in");
    expect(resolveShortcut(key("="), DEFAULT_SHORTCUTS)).toBe("zoom_in");
    expect(resolveShortcut(key("-"), DEFAULT_SHORTCUTS)).toBe("zoom_out");
    expect(resolveShortcut(key("s"), DEFAULT_SHORTCUTS)).toBe("toggle_snapping");
  });

  it("separates K (stop) from Ctrl+K (split) and Alt+I (clear) from I (mark)", () => {
    expect(resolveShortcut(key("k"), DEFAULT_SHORTCUTS)).toBe("shuttle_pause");
    expect(resolveShortcut(key("k", { ctrlKey: true }), DEFAULT_SHORTCUTS)).toBe("split");
    expect(resolveShortcut(key("i"), DEFAULT_SHORTCUTS)).toBe("mark_in");
    expect(resolveShortcut(key("i", { altKey: true }), DEFAULT_SHORTCUTS)).toBe("clear_in");
    expect(resolveShortcut(key("Shift", { shiftKey: true }), DEFAULT_SHORTCUTS)).toBeNull();
    expect(chordFromEvent(key("Control", { ctrlKey: true }))).toBeNull();
  });

  it("applies overrides and persists only differences", () => {
    const map = effectiveShortcuts({ shuttle_forward: ["Ctrl+Alt+L"], bogus: ["X"] });
    expect(map.shuttle_forward).toEqual(["Ctrl+Alt+L"]);
    expect(resolveShortcut(key("l"), map)).toBeNull();
    expect(resolveShortcut(key("l", { ctrlKey: true, altKey: true }), map)).toBe("shuttle_forward");
    expect(overridesFromMap(map)).toEqual({ shuttle_forward: ["Ctrl+Alt+L"] });
    expect(overridesFromMap(DEFAULT_SHORTCUTS)).toEqual({});
    expect(SHORTCUT_ACTIONS.every((action) => map[action].length > 0)).toBe(true);
  });

  it("rejects conflicts and reserved chords when remapping", () => {
    expect(validateRemap(DEFAULT_SHORTCUTS, "mark_in", "J")).toContain("already used");
    expect(validateRemap(DEFAULT_SHORTCUTS, "mark_in", "F5")).toContain("reserved");
    expect(validateRemap(DEFAULT_SHORTCUTS, "mark_in", "I")).toBeNull();
    expect(validateRemap(DEFAULT_SHORTCUTS, "mark_in", "Ctrl+Shift+J")).toBeNull();
    const broken = { ...DEFAULT_SHORTCUTS, mark_in: ["J"] };
    expect(findConflicts(broken)).toEqual([{ chord: "J", actions: ["shuttle_reverse", "mark_in"] }]);
  });
});

describe("shuttle state machine", () => {
  it("L ramps 1x, 2x, 4x forward and stops raising at 4x", () => {
    let state = nextShuttle(SHUTTLE_IDLE, "L");
    expect(state).toEqual({ direction: 1, rate: 1 });
    expect(usesRealPlayback(state)).toBe(true);
    state = nextShuttle(state, "L");
    expect(state).toEqual({ direction: 1, rate: 2 });
    state = nextShuttle(state, "L");
    state = nextShuttle(state, "L");
    expect(state).toEqual({ direction: 1, rate: 4 });
  });

  it("J reverses, switching direction restarts at 1x and K stops", () => {
    let state = nextShuttle(nextShuttle(SHUTTLE_IDLE, "L"), "L");
    state = nextShuttle(state, "J");
    expect(state).toEqual({ direction: -1, rate: 1 });
    expect(usesRealPlayback(state)).toBe(false);
    expect(nextShuttle(state, "K")).toEqual(SHUTTLE_IDLE);
  });

  it("steps backwards and fast-forward by whole frames and stops at the edges", () => {
    expect(shuttleFramesPerTick({ direction: -1, rate: 2 })).toBe(-2);
    expect(shuttleFramesPerTick({ direction: 1, rate: 4 })).toBe(4);
    expect(shuttleFramesPerTick({ direction: 1, rate: 1 })).toBe(0);
    expect(shuttleFramesPerTick(SHUTTLE_IDLE)).toBe(0);
    expect(stepPlayhead(10, { direction: -1, rate: 4 }, 100)).toEqual({ playhead: 6, stopped: false });
    expect(stepPlayhead(2, { direction: -1, rate: 4 }, 100)).toEqual({ playhead: 0, stopped: true });
    expect(stepPlayhead(98, { direction: 1, rate: 4 }, 100)).toEqual({ playhead: 100, stopped: true });
    expect(describeShuttle({ direction: -1, rate: 2 })).toBe("◀ 2×");
    expect(describeShuttle(SHUTTLE_IDLE)).toBe("");
  });
});

describe("In/Out range", () => {
  it("keeps the pair ordered and yields a span only when both marks exist", () => {
    let marks = setMark({ markIn: null, markOut: null }, "in", 30);
    expect(selectedSpan(marks)).toBeNull();
    marks = setMark(marks, "out", 90);
    expect(selectedSpan(marks)).toEqual({ start: 30, end: 90 });
    expect(setMark(marks, "in", 120)).toEqual({ markIn: 120, markOut: null });
    expect(setMark(marks, "out", 10)).toEqual({ markIn: null, markOut: 10 });
  });

  it("clamps the export range to the timeline", () => {
    expect(exportRangeFor({ markIn: 30, markOut: 90 }, 300)).toEqual({ start_frame: 30, end_frame: 90 });
    expect(exportRangeFor({ markIn: 30, markOut: 900 }, 300)).toEqual({ start_frame: 30, end_frame: 300 });
    expect(exportRangeFor({ markIn: 400, markOut: 900 }, 300)).toBeNull();
    expect(exportRangeFor({ markIn: null, markOut: 90 }, 300)).toBeNull();
  });
});

function fixture(): Project {
  const rate = { numerator: 30, denominator: 1 };
  const item = (id: string, start: number, duration: number) => ({
    item_id: id,
    timeline_range: { start: { value: start, rate }, duration: { value: duration, rate } },
  });
  return {
    settings: { frame_rate: rate },
    tracks: [
      { track_id: "t1", locked: false, items: [item("a", 0, 60), item("b", 60, 60), item("c", 120, 60)] },
      { track_id: "t2", locked: true, items: [item("x", 0, 180)] },
      { track_id: "t3", locked: false, items: [item("y", 100, 40)] },
    ],
  } as unknown as Project;
}

describe("ripple delete of the In/Out span", () => {
  it("splits straddling clips, deletes the pieces inside, skips locked tracks", () => {
    let counter = 0;
    const plan = planRippleDeleteRange(fixture(), { start: 30, end: 150 }, () => `new_${counter++}`);
    expect(plan.skippedLockedTracks).toEqual(["t2"]);
    expect(plan.affectedClips).toBe(4);

    const ops = plan.operations.map((o) => o.operation);
    const splits = ops.filter((o) => o.type === "ClipSplit");
    const deletes = ops.filter((o) => o.type === "ClipRippleDelete");
    expect(splits).toHaveLength(2);
    expect(deletes).toHaveLength(4);
    expect(ops.findIndex((o) => o.type === "ClipRippleDelete")).toBeGreaterThanOrEqual(splits.length);

    const first = splits[0] as { payload: { item_id: string; split_time: { value: number }; new_right_item_id: string } };
    expect(first.payload).toMatchObject({ item_id: "a", split_time: { value: 30 } });
    const deletedIds = deletes.map((o) => (o as { payload: { item_id: string } }).payload.item_id);
    expect(deletedIds).toContain(first.payload.new_right_item_id);
    expect(deletedIds).not.toContain("a");
    expect(deletedIds).toContain("b");
    expect(deletedIds).toContain("c");
    expect(deletedIds).toContain("y");
  });

  it("does nothing outside the span and splits a clip spanning both points twice", () => {
    let counter = 0;
    const none = planRippleDeleteRange(fixture(), { start: 500, end: 600 }, () => `n${counter++}`);
    expect(none.operations).toEqual([]);
    const span = planRippleDeleteRange(fixture(), { start: 70, end: 100 }, () => `m${counter++}`);
    const types = span.operations.map((o) => o.operation.type);
    expect(types).toEqual(["ClipSplit", "ClipSplit", "ClipRippleDelete"]);
  });
});
