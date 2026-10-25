import { invoke } from "@tauri-apps/api/core";
import { HealthStatus } from "../types/health";
import { Project } from "../types/project";
import { CommandEnvelope, CommandReceipt, DiffReport } from "../types/command";
import { ExportPreset } from "../types/export";
import {
  PreviewFrameState,
  BenchmarkReport,
  ParitySuiteReport,
  PreviewVideoSourceStatus,
} from "../types/preview";

export interface CaptionsImportReport {
  format: string;
  trackId: string;
  cueCount: number;
}

export interface CaptionsExportReport {
  outputPath: string;
  format: string;
  cueCount: number;
}

export interface AutoCaptionRequest {
  itemId: string;
  whisperCliPath: string;
  modelPath: string;
  language?: string;
}

export interface AutoCaptionReport {
  jobId: string;
  state: "queued" | "running" | "completed" | "failed" | "cancelled";
  progress: number;
  message: string;
  captionCount?: number;
  trackId?: string;
}

export interface PreviewOverlayIpcResponse {
  width: number;
  height: number;
  roi: [number, number, number, number];
  roiPngBytes: number[];
  hasVisibleContent: boolean;
  fontSha256: string;
  renderElapsedMs: number;
}

export const editorApi = {
  async getHealth(): Promise<HealthStatus> {
    return invoke<HealthStatus>("app_health_check");
  },

  async getCurrentProject(): Promise<Project> {
    return invoke<Project>("project_get_current");
  },

  async newProject(name: string): Promise<Project> {
    return invoke<Project>("project_new", { name });
  },

  async executeCommand(envelope: CommandEnvelope): Promise<CommandReceipt> {
    return invoke<CommandReceipt>("project_execute_command", { envelope });
  },

  async dryRun(envelope: CommandEnvelope): Promise<DiffReport> {
    return invoke<DiffReport>("project_dry_run", { envelope });
  },

  async saveProject(path?: string): Promise<string> {
    return invoke<string>("project_save", { path });
  },

  async openProject(path: string): Promise<Project> {
    return invoke<Project>("project_open", { path });
  },

  async checkRecovery(path: string): Promise<unknown> {
    return invoke("project_check_recovery", { path });
  },

  async undo(): Promise<string | null> {
    return invoke<string | null>("project_undo");
  },

  async redo(): Promise<string | null> {
    return invoke<string | null>("project_redo");
  },

  async exportProject(outputPath: string, preset?: ExportPreset): Promise<{ output_path: string; duration_secs: number; file_size_bytes: number; render_time_ms: number }> {
    return invoke("project_export", { outputPath, preset });
  },

  async importMedia(filePath: string): Promise<unknown> {
    return invoke("media_import", { filePath });
  },

  async importCaptions(filePath: string): Promise<CaptionsImportReport> {
    return invoke<CaptionsImportReport>("captions_import", { filePath });
  },

  async exportCaptions(outputPath: string): Promise<CaptionsExportReport> {
    return invoke<CaptionsExportReport>("captions_export", { outputPath });
  },

  async startAutoCaptions(request: AutoCaptionRequest): Promise<AutoCaptionReport> {
    return invoke<AutoCaptionReport>("captions_auto_start", { request });
  },

  async getAutoCaptionStatus(jobId: string): Promise<AutoCaptionReport> {
    return invoke<AutoCaptionReport>("captions_auto_status", { jobId });
  },

  async cancelAutoCaptions(jobId: string): Promise<void> {
    return invoke<void>("captions_auto_cancel", { jobId });
  },

  async samplePreviewAt(frame: number): Promise<PreviewFrameState> {
    return invoke<PreviewFrameState>("preview_sample_at", { frame });
  },

  async getCompatiblePreviewVideo(assetId: string): Promise<PreviewVideoSourceStatus> {
    return invoke<PreviewVideoSourceStatus>("media_get_compatible_preview_video", { assetId });
  },

  async runPreviewBenchmark(durationSecs: number = 30): Promise<BenchmarkReport[]> {
    return invoke<BenchmarkReport[]>("preview_run_benchmark", { durationSecs });
  },

  async runParitySuite(): Promise<ParitySuiteReport> {
    return invoke<ParitySuiteReport>("parity_run_suite");
  },

  async getWaveform(assetId: string): Promise<import("../types/preview").WaveformStatus> {
    return invoke<import("../types/preview").WaveformStatus>("media_get_waveform", { assetId });
  },

  async renderTextTemplatePocPreview(
    text: string,
    progress: number
  ): Promise<PreviewOverlayIpcResponse> {
    return invoke<PreviewOverlayIpcResponse>("render_text_template_poc_preview", {
      text,
      progress,
    });
  },
};
