export function isProjectRevisionConflictStatus(status: string): boolean {
  return status.startsWith("Save error: Project revision conflict:");
}
