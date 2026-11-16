import type { ExportPreset } from "../types/export";
import type { ProjectExportJobStatus } from "../services/editorApi";

export interface ActiveExport {
  jobId: string; outputPath: string; progress: number; cancelling: boolean;
  preset: ExportPreset; range: { start_frame: number; end_frame: number } | null;
}
export interface ExportResult extends ActiveExport { status: "completed" | "failed" | "cancelled"; error: string | null }

export function terminalExportResult(active: ActiveExport | null, report: ProjectExportJobStatus): ExportResult | null {
  if (!active || active.jobId !== report.jobId || !["completed", "failed", "cancelled"].includes(report.status)) return null;
  return {...active, status: report.status as ExportResult["status"], error: report.error};
}
