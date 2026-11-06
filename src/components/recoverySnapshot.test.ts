import { describe, expect, it } from "vitest";
import { getPendingRecoverySnapshotRequest } from "./recoverySnapshot";

describe("getPendingRecoverySnapshotRequest", () => {
  it("does not write a recovery snapshot for a pristine project", () => {
    expect(getPendingRecoverySnapshotRequest("project-a", 0, null, null)).toBeNull();
  });

  it("snapshots an edited project that has never been saved", () => {
    expect(getPendingRecoverySnapshotRequest("project-a", 4, null, null)).toEqual({
      kind: "unsaved-project",
      projectId: "project-a",
      revision: 4,
    });
  });

  it("snapshots a saved project only when its in-memory revision is newer", () => {
    expect(getPendingRecoverySnapshotRequest("project-a", 5, "movie.tinix", 4)).toEqual({
      kind: "saved-project",
      path: "movie.tinix",
      revision: 5,
    });
    expect(getPendingRecoverySnapshotRequest("project-a", 4, "movie.tinix", 4)).toBeNull();
  });

  it("does not invent a baseline revision for a saved project", () => {
    expect(getPendingRecoverySnapshotRequest("project-a", 5, "movie.tinix", null)).toBeNull();
  });

  it("handles rehydrated saved project session correctly", () => {
    // When rehydrated saved project matches persisted revision on reload: no recovery snapshot needed
    expect(getPendingRecoverySnapshotRequest("project-a", 8, "C:/projects/film.tinix", 8)).toBeNull();

    // When rehydrated saved project has dirty edits after reload: snapshots to canonical path
    expect(getPendingRecoverySnapshotRequest("project-a", 9, "C:/projects/film.tinix", 8)).toEqual({
      kind: "saved-project",
      path: "C:/projects/film.tinix",
      revision: 9,
    });
  });

  it("handles rehydrated unsaved project session correctly", () => {
    // Rehydrated pristine unsaved project: no snapshot
    expect(getPendingRecoverySnapshotRequest("project-b", 0, null, null)).toBeNull();

    // Rehydrated dirty unsaved project: snapshots as unsaved
    expect(getPendingRecoverySnapshotRequest("project-b", 2, null, null)).toEqual({
      kind: "unsaved-project",
      projectId: "project-b",
      revision: 2,
    });
  });
});
