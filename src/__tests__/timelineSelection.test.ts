import { describe, it, expect } from "vitest";
import {
  clearSelection,
  createInitialSelection,
  reconcileSelection,
  selectSingle,
  toggleSelect,
} from "../utils/timelineSelection";
import { Project } from "../types/project";

describe("Phase 16 Selection Model (P16-TP-01..05, P16-AC-05..11)", () => {
  it("P16-AC-05: selectSingle replaces selection and sets item as primary", () => {
    const sel = selectSingle("clip_1");
    expect(sel.selectedItemIds.size).toBe(1);
    expect(sel.selectedItemIds.has("clip_1")).toBe(true);
    expect(sel.primarySelectedItemId).toBe("clip_1");

    const sel2 = selectSingle("clip_2");
    expect(sel2.selectedItemIds.size).toBe(1);
    expect(sel2.selectedItemIds.has("clip_2")).toBe(true);
    expect(sel2.selectedItemIds.has("clip_1")).toBe(false);
    expect(sel2.primarySelectedItemId).toBe("clip_2");
  });

  it("P16-AC-06: Ctrl+Click toggles membership and updates primary appropriately", () => {
    let state = createInitialSelection();

    // 1. Add clip_1 via Ctrl+Click -> becomes primary
    state = toggleSelect(state, "clip_1");
    expect(state.selectedItemIds.size).toBe(1);
    expect(state.selectedItemIds.has("clip_1")).toBe(true);
    expect(state.primarySelectedItemId).toBe("clip_1");

    // 2. Add clip_2 via Ctrl+Click -> becomes new primary
    state = toggleSelect(state, "clip_2");
    expect(state.selectedItemIds.size).toBe(2);
    expect(state.selectedItemIds.has("clip_1")).toBe(true);
    expect(state.selectedItemIds.has("clip_2")).toBe(true);
    expect(state.primarySelectedItemId).toBe("clip_2");

    // 3. Remove non-primary clip_1 -> clip_2 remains primary
    state = toggleSelect(state, "clip_1");
    expect(state.selectedItemIds.size).toBe(1);
    expect(state.selectedItemIds.has("clip_1")).toBe(false);
    expect(state.selectedItemIds.has("clip_2")).toBe(true);
    expect(state.primarySelectedItemId).toBe("clip_2");

    // 4. Remove primary clip_2 -> selection empty, primary null
    state = toggleSelect(state, "clip_2");
    expect(state.selectedItemIds.size).toBe(0);
    expect(state.primarySelectedItemId).toBeNull();
  });

  it("P16-AC-06: removing primary chooses remaining selected item as fallback", () => {
    let state = createInitialSelection();
    state = toggleSelect(state, "clip_a");
    state = toggleSelect(state, "clip_b");
    state = toggleSelect(state, "clip_c"); // primary is clip_c

    expect(state.primarySelectedItemId).toBe("clip_c");

    // Remove primary clip_c
    state = toggleSelect(state, "clip_c");
    expect(state.selectedItemIds.has("clip_c")).toBe(false);
    expect(state.selectedItemIds.size).toBe(2);
    // Primary must fallback to one of the remaining items
    expect(state.primarySelectedItemId).toBe("clip_b");
  });

  it("P16-AC-07: clearSelection resets selection and primary", () => {
    let state = selectSingle("clip_1");
    state = toggleSelect(state, "clip_2");
    expect(state.selectedItemIds.size).toBe(2);

    const cleared = clearSelection();
    expect(cleared.selectedItemIds.size).toBe(0);
    expect(cleared.primarySelectedItemId).toBeNull();
  });

  it("P16-AC-10: reconcileSelection filters deleted IDs and updates primary", () => {
    const mockProject = {
      project_id: "p1",
      name: "Test",
      schema_version: 3,
      revision: 1,
      settings: {
        width: 1920,
        height: 1080,
        frame_rate: { numerator: 30, denominator: 1 },
      },
      tracks: [
        {
          track_id: "t1",
          name: "V1",
          kind: "Video",
          items: [
            {
              item_id: "surviving_1",
              name: "C1",
              kind: "Video",
              timeline_range: { start: { value: 0, rate: { numerator: 30, denominator: 1 } }, duration: { value: 30, rate: { numerator: 30, denominator: 1 } } },
              source_range: { start: { value: 0, rate: { numerator: 30, denominator: 1 } }, duration: { value: 30, rate: { numerator: 30, denominator: 1 } } },
            },
            {
              item_id: "surviving_2",
              name: "C2",
              kind: "Video",
              timeline_range: { start: { value: 30, rate: { numerator: 30, denominator: 1 } }, duration: { value: 30, rate: { numerator: 30, denominator: 1 } } },
              source_range: { start: { value: 0, rate: { numerator: 30, denominator: 1 } }, duration: { value: 30, rate: { numerator: 30, denominator: 1 } } },
            },
          ],
        },
      ],
      assets: [],
    } as unknown as Project;

    // State had deleted_item as primary, plus surviving_1 and surviving_2
    const current = {
      selectedItemIds: new Set(["deleted_item", "surviving_1", "surviving_2"]),
      primarySelectedItemId: "deleted_item",
    };

    const reconciled = reconcileSelection(current, mockProject);
    expect(reconciled.selectedItemIds.size).toBe(2);
    expect(reconciled.selectedItemIds.has("deleted_item")).toBe(false);
    expect(reconciled.selectedItemIds.has("surviving_1")).toBe(true);
    expect(reconciled.selectedItemIds.has("surviving_2")).toBe(true);
    // Deleted primary was reconciled to a surviving element
    expect(reconciled.primarySelectedItemId).not.toBe("deleted_item");
    expect(["surviving_1", "surviving_2"]).toContain(reconciled.primarySelectedItemId);
  });

  it("P16-AC-10: reconcileSelection with all deleted items resets to empty/null", () => {
    const mockProject = {
      project_id: "p1",
      name: "Test",
      schema_version: 3,
      revision: 1,
      settings: {
        width: 1920,
        height: 1080,
        frame_rate: { numerator: 30, denominator: 1 },
      },
      tracks: [],
      assets: [],
    } as unknown as Project;

    const current = {
      selectedItemIds: new Set(["deleted_1", "deleted_2"]),
      primarySelectedItemId: "deleted_1",
    };

    const reconciled = reconcileSelection(current, mockProject);
    expect(reconciled.selectedItemIds.size).toBe(0);
    expect(reconciled.primarySelectedItemId).toBeNull();
  });
});
