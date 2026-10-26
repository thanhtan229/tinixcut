import { invoke } from "@tauri-apps/api/core";
import { HealthStatus } from "../types/health";
import { Project } from "../types/project";
import { CommandEnvelope, CommandReceipt, DiffReport } from "../types/command";
import { ExportPreset } from "../types/export";
import { TemplateDefinition, TemplatePackManifest } from "../types/template";
import {
  PreviewFrameState,
  BenchmarkReport,
  ParitySuiteReport,
  PreviewVideoSourceStatus,
  PreviewRichTextOverlay,
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

export interface CapCutInspectionResult {
  profile: {
    draft_path: string;
    draft_sha256: string;
    app_version: string | null;
    fps: number;
    duration_us: number;
    duration_seconds: number;
    canvas_width: number;
    canvas_height: number;
    template_name: string | null;
    layers_count: number;
    resources: Array<{
      resource_id: string;
      panel: string;
      original_path: string;
      resolved_path: string | null;
      status: "AVAILABLE" | "MISSING" | "CORRUPT" | "RELOCATED";
      sha256: string | null;
      byte_size: number | null;
    }>;
    missing_resources: string[];
    recognized_family: "STAGGERED_CHARACTER_IN" | "VLOG_TITLE_MULTI_LAYER" | "TEXT_WITH_BACKGROUND_CARD" | "UNKNOWN";
  };
  report: {
    status: "FULL_CONVERSION" | "PARTIAL_CONVERSION" | "UNSUPPORTED_SOURCE" | "MISSING_RESOURCES" | "BAKED_MEDIA_ONLY";
    source_path: string;
    source_sha256: string;
    staging_token?: string | null;
    template_name: string;
    recognized_profile: string;
    imported_elements: string[];
    loss_details: Array<{ component: string; original: string; treatment: string; impact: string }>;
    warnings: string[];
    converted_definition: TemplateDefinition | null;
  };
  converted_definition: TemplateDefinition | null;
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

  async previewRichTextOverlay(
    itemId: string,
    frame: number
  ): Promise<PreviewRichTextOverlay | null> {
    return invoke<PreviewRichTextOverlay | null>("preview_rich_text_overlay", {
      itemId,
      frame,
    });
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

  async templateListDefinitions(): Promise<TemplateDefinition[]> {
    return invoke<TemplateDefinition[]>("template_list_definitions");
  },

  async templateGetDefinition(
    id: string,
    version?: number
  ): Promise<TemplateDefinition | null> {
    return invoke<TemplateDefinition | null>("template_get_definition", { id, version });
  },

  async templateSaveUserDefinition(
    definition: TemplateDefinition
  ): Promise<string> {
    return invoke<string>("template_save_user_definition", { definition });
  },

  async templateDeleteUserDefinition(
    id: string,
    version: number
  ): Promise<boolean> {
    return invoke<boolean>("template_delete_user_definition", { id, version });
  },

  async templateImportPack(
    packPath: string
  ): Promise<TemplatePackManifest> {
    return invoke<TemplatePackManifest>("template_import_pack", { packPath });
  },

  async hashLocalFile(path: string): Promise<string> {
    return invoke<string>("media_file_sha256", { path });
  },

  async templateExportPack(
    manifest: TemplatePackManifest,
    definitions: TemplateDefinition[],
    assetFiles: [string, string][],
    outputPath: string
  ): Promise<void> {
    return invoke<void>("template_export_pack", {
      manifest,
      definitions,
      assetFiles,
      outputPath,
    });
  },

  async capcutCreateInspectionToken(): Promise<string> {
    return invoke<string>("capcut_create_inspection_token");
  },

  async capcutCancelInspection(token: string): Promise<boolean> {
    return invoke<boolean>("capcut_cancel_inspection", { token });
  },

  async capcutInspectTemplateDraft(
    draftPath: string,
    selectedRoots: string[],
    cancelToken: string
  ): Promise<CapCutInspectionResult> {
    return invoke<CapCutInspectionResult>("capcut_inspect_template_draft", {
      draftPath,
      selectedRoots,
      cancelToken,
    });
  },

  async capcutSaveTemplateToLibrary(
    report: CapCutInspectionResult["report"],
    assetSourceFiles: [string, string][],
    expectedSourceHash: string
  ): Promise<string> {
    return invoke<string>("capcut_save_template_to_library", {
      report,
      assetSourceFiles,
      expectedSourceHash,
    });
  },

  async capcutApplyTemplateToProject(args: {
    itemId: string;
    templateId: string;
    templateVersion: number;
    textVariant?: string;
    expectedRevision: number;
    expectedSourceHash: string;
    sourceDraftPath: string;
    expectedResourceFingerprints: Record<string, string>;
    stagingToken: string;
    cancelToken?: string;
  }): Promise<CommandReceipt> {
    return invoke<CommandReceipt>("capcut_apply_template_to_project", args);
  },
};
