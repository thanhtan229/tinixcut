import { describe, expect, it } from "vitest";
import { isProjectRevisionConflictStatus } from "./projectSaveConflict";

describe("project save conflict recovery action", () => {
  it("offers Save a Copy only for a project revision conflict", () => {
    expect(
      isProjectRevisionConflictStatus(
        "Save error: Project revision conflict: expected disk revision Some(0), found Some(2)"
      )
    ).toBe(true);
    expect(isProjectRevisionConflictStatus("Save error: disk is full")).toBe(false);
    expect(isProjectRevisionConflictStatus("Project saved atomically")).toBe(false);
  });
});
