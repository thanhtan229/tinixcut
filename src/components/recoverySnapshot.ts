export type PendingRecoverySnapshotRequest =
  | { kind: "saved-project"; path: string; revision: number }
  | { kind: "unsaved-project"; projectId: string; revision: number };

export function getPendingRecoverySnapshotRequest(
  projectId: string,
  revision: number,
  savedFilePath: string | null,
  savedRevision: number | null,
): PendingRecoverySnapshotRequest | null {
  if (revision <= 0) return null;
  if (savedFilePath) {
    if (savedRevision === null || revision === savedRevision) return null;
    return { kind: "saved-project", path: savedFilePath, revision };
  }
  return { kind: "unsaved-project", projectId, revision };
}
