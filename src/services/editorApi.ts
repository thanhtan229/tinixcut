import { invoke } from "@tauri-apps/api/core";
import { HealthStatus } from "../types/health";
import { ClipLut, Project } from "../types/project";
import { CommandEnvelope, CommandReceipt, DiffReport, OperationEnvelope } from "../types/command";
import { ExportPreset } from "../types/export";
import { TemplateDefinition, TemplatePackManifest } from "../types/template";
import {
  PreviewFrameState,
  BenchmarkReport,
  ParitySuiteReport,
  PreviewVideoSourceStatus,
  PreviewRichTextOverlay,
} from "../types/preview";

export interface FrameScopes {
  width: number;
  height: number;
  histogram: { red: number[]; green: number[]; blue: number[]; luma: number[] };
  luma_stats: {
    min: number;
    max: number;
    mean: number;
    blackClipShare: number;
    whiteClipShare: number;
  };
  waveform: { columns: number; luma: number[]; red: number[]; green: number[]; blue: number[] };
  vectorscope: { size: number; density: number[] };
}

export interface LutTable {
  size: number;
  domainMin: [number, number, number];
  domainMax: [number, number, number];
  /** Little-endian f32 RGB triples, red-fastest, base64 encoded. */
  dataBase64: string;
}

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

export interface AudioLevelReport {
  source_fingerprint: string;
  mean_dbfs: number | null;
  peak_dbfs: number | null;
  measurement: string;
}

export interface MediaCacheCategoryStatus {
  files: number;
  bytes: number;
}

export interface MediaCacheStatus {
  totalFiles: number;
  totalBytes: number;
  categories: Record<string, MediaCacheCategoryStatus>;
}

export interface AutoCaptionRequest {
  itemId: string;
  whisperCliPath: string;
  modelPath: string;
  language?: string;
  analyzeOnly?: boolean;
}

export interface TranscriptSourceEntry {
  sourceStart: { value: number; rate: { numerator: number; denominator: number } };
  sourceDuration: { value: number; rate: { numerator: number; denominator: number } };
  timelineStart: { value: number; rate: { numerator: number; denominator: number } } | null;
  text: string;
}

export interface AppSettings {
  schemaVersion: number;
  whisperCliPath: string | null;
  whisperModelPath: string | null;
  transcriptionLanguage: "auto" | "vi" | "en";
  recentProjects: string[];
  legacyBrowserSettingsMigrated: boolean;
  previewProxyMode: "off" | "auto";
}

export interface AutoCaptionReport {
  jobId: string;
  state: "queued" | "running" | "completed" | "failed" | "cancelled";
  progress: number;
  message: string;
  captionCount?: number;
  trackId?: string;
  transcript?: TranscriptSourceEntry[];
}

export interface VisualSceneDescription {
  sceneId: string;
  assetId: string;
  assetName: string;
  sourceStartMicros: number;
  sourceEndMicros: number;
  sampledAtMicros: number;
  caption: string;
  tags: string[];
  relevanceScore: number;
  rationale: string;
}

export interface VisualSceneAnalysisReport {
  schemaVersion: number;
  algorithmVersion: string;
  provider: string;
  model: string;
  query: string;
  sourceFingerprint: string;
  configFingerprint: string;
  truncated: boolean;
  scenes: VisualSceneDescription[];
  warnings: string[];
}

export interface SceneSelectionSuggestion {
  provider: string;
  model: string;
  orderedSceneIds: string[];
  rationale: string;
  selectedDurationMicros: number;
  targetDurationSeconds: number | null;
  openingTitleSuggestion: string | null;
  constraints: {
    allowedAssetIds: string[];
    mustKeepSceneIds: string[];
    mustOmitSceneIds: string[];
    captionIntent: string | null;
    audioIntent: string | null;
  };
}

export type SceneCanvasPreset = "landscape_16x9" | "portrait_9x16" | "square_1x1" | "portrait_4x5";
export type SceneTitleTemplate = "clean_lower_third" | "bold_headline" | "center_pop" | "subtitle_card" | "quote_card" | "call_to_action";

export interface VisualSceneAnalysisJobReport {
  jobId: string;
  state: "queued" | "running" | "completed" | "failed" | "cancelled";
  progress: number;
  message: string;
  report: VisualSceneAnalysisReport | null;
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

export interface OfflineMediaAsset {
  asset_id: string;
  last_known_path: string;
  permission_required: boolean;
}

export interface ProjectExportJobStatus {
  jobId: string;
  status: "queued" | "running" | "completed" | "failed" | "cancelled";
  progress: number;
  error: string | null;
}

export interface ProjectExportStartReceipt {
  jobId: string;
  status: string;
  outputPath: string;
}

export interface EditPlanDraftReceipt {
  provider: string;
  plan: {
    schemaVersion: number;
    goal: string;
    projectId: string;
    baseRevision: number;
    referencedMediaIds: string[];
    assumptions: string[];
    warnings: string[];
    operations: Array<{ operationId: string; operation: OperationEnvelope["operation"] }>;
  };
  validation: {
    valid: boolean;
    issues: string[];
    diff: DiffReport | null;
  };
}

export type ProjectRecoveryStatus =
  | "NoRecoveryFound"
  | "RecoveryMatchesProject"
  | {
      NewerRecoveryAvailable: {
        recovery_path: string;
        recovery_revision: number;
        project_revision: number;
        recovery_fingerprint: string;
      };
    }
  | {
      RecoveryDiffersFromProject: {
        recovery_path: string;
        recovery_revision: number;
        project_revision: number;
        recovery_fingerprint: string;
      };
    }
  | {
      RecoveryUnreadable: {
        recovery_path: string;
        reason: string;
      };
    };

export interface RecoverySnapshotReceipt {
  projectRevision: number;
  recoveryRevision: number;
}

export interface UnsavedRecoveryCandidate {
  projectId: string;
  name: string;
  revision: number;
  updatedAtMs: number;
  assetCount: number;
  trackCount: number;
  clipCount: number;
  previewItemNames: string[];
}

export interface UnsavedRecoveryReceipt {
  projectRevision: number;
  recoveryPath: string;
}

export interface RecoveryRestoreReceipt {
  project: Project;
  persistedRevision: number;
}

export interface ProjectVariantCreateReceipt {
  path: string;
  sourceProjectId: string;
  sourceRevision: number;
  project: Project;
}

export interface CollectedMediaReceipt {
  projectPath: string;
  mediaDirectory: string;
  project: Project;
  copiedAssets: number;
  totalBytes: number;
}

export interface ProjectCompareReport {
  left: { projectId: string; name: string; revision: number };
  right: { projectId: string; name: string; revision: number };
  contentEquivalent: boolean;
  settingsChangedFields: string[];
  assets: { added: string[]; removed: string[]; modified: Array<{ entityId: string; changedFields: string[] }> };
  tracks: { added: string[]; removed: string[]; modified: Array<{ entityId: string; changedFields: string[] }> };
  items: { added: string[]; removed: string[]; modified: Array<{ entityId: string; changedFields: string[] }> };
  transitions: { added: string[]; removed: string[]; modified: Array<{ entityId: string; changedFields: string[] }> };
  audioTransitions: { added: string[]; removed: string[]; modified: Array<{ entityId: string; changedFields: string[] }> };
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

export interface CurrentProjectSession {
  project: Project;
  canonicalPath: string | null;
  persistedRevision: number | null;
}

export const editorApi = {
  getAppSettings: () => invoke<AppSettings>("app_settings_get"),
  describeLut: (path: string) => invoke<ClipLut>("lut_describe", { path }),
  frameScopes: (frame: number) => invoke<FrameScopes>("frame_scopes", { frame }),
  readLutTable: (path: string, sha256: string) =>
    invoke<LutTable>("lut_read_table", { path, sha256 }),
  saveAppSettings: (settings: AppSettings) => invoke<AppSettings>("app_settings_save", { settings }),
  addRecentProject: (projectPath: string) =>
    invoke<AppSettings>("app_settings_add_recent_project", { projectPath }),
  async getHealth(): Promise<HealthStatus> {
    return invoke<HealthStatus>("app_health_check");
  },

  async getCurrentProject(): Promise<Project> {
    return invoke<Project>("project_get_current");
  },

  async getCurrentProjectSession(): Promise<CurrentProjectSession> {
    return invoke<CurrentProjectSession>("project_get_current_session");
  },

  async createProjectVariant(path: string, name: string, expectedProjectId: string, expectedRevision: number): Promise<ProjectVariantCreateReceipt> {
    return invoke<ProjectVariantCreateReceipt>("project_variant_create", { path, name, expectedProjectId, expectedRevision });
  },
  async collectProjectMedia(destinationDirectory: string, expectedProjectId: string, expectedRevision: number): Promise<CollectedMediaReceipt> {
    return invoke<CollectedMediaReceipt>("project_collect_media", { destinationDirectory, expectedProjectId, expectedRevision });
  },

  async compareCurrentWithPath(path: string, expectedProjectId: string, expectedRevision: number): Promise<ProjectCompareReport> {
    return invoke<ProjectCompareReport>("project_compare_current_with_path", { path, expectedProjectId, expectedRevision });
  },

  async previewEditPlanDraft(
    goal: string,
    mediaIds: string[],
    targetDurationFrames: number,
    audioAssetId: string | null,
    subtitleFilePath: string | null,
    plannerProvider: "deterministic-mock-v1" | "lm-studio-local",
    localModel: string,
    requestId: string,
  ): Promise<EditPlanDraftReceipt> {
    return invoke<EditPlanDraftReceipt>("edit_plan_draft_preview", {
      goal,
      mediaIds,
      targetDurationFrames,
      audioAssetId,
      subtitleFilePath,
      plannerProvider,
      localModel,
      requestId,
    });
  },

  async previewSceneGuidedEditPlan(
    goal: string,
    report: VisualSceneAnalysisReport,
    selectedSceneIds: string[],
    targetDurationFrames: number | null,
    exactDuration: boolean,
    mustKeepSceneIds: string[] = [],
    mustOmitSceneIds: string[] = [],
    targetCanvas: SceneCanvasPreset | null = null,
    openingTitle: string | null = null,
    titleTemplate: SceneTitleTemplate | null = null,
    allowedAssetIds: string[] = [],
  ): Promise<EditPlanDraftReceipt> {
    return invoke<EditPlanDraftReceipt>("edit_plan_scene_draft_preview", {
      goal,
      report,
      selectedSceneIds,
      targetDurationFrames,
      exactDuration,
      mustKeepSceneIds,
      mustOmitSceneIds,
      allowedAssetIds,
      targetCanvas,
      openingTitle,
      titleTemplate,
    });
  },

  async suggestVisualSceneSelection(
    goal: string,
    report: VisualSceneAnalysisReport,
    targetDurationSeconds: number | null,
    localModel: string,
    mustKeepSceneIds: string[] = [],
    mustOmitSceneIds: string[] = [],
    allowedAssetIds: string[] = [],
    captionIntent: string | null = null,
    audioIntent: string | null = null,
    suggestOpeningTitle = false,
  ): Promise<SceneSelectionSuggestion> {
    return invoke<SceneSelectionSuggestion>("edit_plan_scene_selection_suggest", {
      goal,
      report,
      targetDurationSeconds,
      mustKeepSceneIds,
      mustOmitSceneIds,
      allowedAssetIds,
      captionIntent,
      audioIntent,
      suggestOpeningTitle,
      localModel,
    });
  },

  async previewEditPlanReorder(
    goal: string,
    trackId: string,
    itemIds: string[],
    orderedItemIds: string[],
    plannerProvider: "caller-order-v1" | "lm-studio-local",
    localModel: string,
    requestId: string,
  ): Promise<EditPlanDraftReceipt> {
    return invoke<EditPlanDraftReceipt>("edit_plan_reorder_preview", {
      goal,
      trackId,
      itemIds,
      orderedItemIds,
      plannerProvider,
      localModel,
      requestId,
    });
  },

  async previewEditPlanTrim(
    goal: string,
    itemId: string,
    targetDurationFrames: number,
  ): Promise<EditPlanDraftReceipt> {
    return invoke<EditPlanDraftReceipt>("edit_plan_trim_preview", {
      goal,
      itemId,
      targetDurationFrames,
    });
  },

  async cancelEditPlanDraft(requestId: string): Promise<boolean> {
    return invoke<boolean>("edit_plan_draft_cancel", { requestId });
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

  async checkRecovery(path: string): Promise<ProjectRecoveryStatus> {
    return invoke<ProjectRecoveryStatus>("project_check_recovery", { path });
  },

  async saveRecoverySnapshot(path: string, expectedRevision: number): Promise<RecoverySnapshotReceipt> {
    return invoke<RecoverySnapshotReceipt>("project_save_recovery_snapshot", {
      path,
      expectedRevision,
    });
  },

  async restoreRecovery(
    path: string,
    expectedProjectRevision: number,
    expectedRecoveryRevision: number,
    expectedRecoveryFingerprint: string,
  ): Promise<RecoveryRestoreReceipt> {
    return invoke<RecoveryRestoreReceipt>("project_restore_recovery", {
      path,
      expectedProjectRevision,
      expectedRecoveryRevision,
      expectedRecoveryFingerprint,
    });
  },

  async saveUnsavedRecovery(projectId: string, expectedRevision: number): Promise<UnsavedRecoveryReceipt> {
    return invoke<UnsavedRecoveryReceipt>("project_save_unsaved_recovery", {
      expectedProjectId: projectId,
      expectedRevision,
    });
  },

  async listUnsavedRecoveries(): Promise<UnsavedRecoveryCandidate[]> {
    return invoke<UnsavedRecoveryCandidate[]>("project_list_unsaved_recoveries");
  },

  async restoreUnsavedRecovery(
    projectId: string,
    expectedRecoveryRevision: number,
    expectedCurrentProjectId: string,
    expectedCurrentRevision: number,
  ): Promise<Project> {
    return invoke<Project>("project_restore_unsaved_recovery", {
      projectId,
      expectedRecoveryRevision,
      expectedCurrentProjectId,
      expectedCurrentRevision,
    });
  },

  async discardUnsavedRecovery(projectId: string, expectedRecoveryRevision: number): Promise<void> {
    return invoke<void>("project_discard_unsaved_recovery", { projectId, expectedRecoveryRevision });
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

  async startProjectExport(outputPath: string, preset?: ExportPreset): Promise<ProjectExportStartReceipt> {
    return invoke("project_export_start", { outputPath, preset });
  },

  async getProjectExportStatus(jobId: string): Promise<ProjectExportJobStatus> {
    return invoke("project_export_status", { jobId });
  },

  async cancelProjectExport(jobId: string): Promise<boolean> {
    return invoke("project_export_cancel", { jobId });
  },

  async preflightExport(outputPath: string, preset?: ExportPreset): Promise<{
    projectId: string;
    projectRevision: number;
    outputPath: string;
    outputExists: boolean;
    availableDiskBytes: number;
    sourceCount: number;
    durationSecs: number;
    width: number;
    height: number;
    frameRateNumerator: number;
    frameRateDenominator: number;
    warnings: string[];
  }> {
    return invoke("project_export_preflight", { outputPath, preset });
  },

  async importMedia(filePath: string): Promise<unknown> {
    return invoke("media_import", { filePath });
  },

  async listOfflineMedia(): Promise<OfflineMediaAsset[]> {
    return invoke("media_list_offline_assets");
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

  async startVisualSceneAnalysis(request: { assetIds: string[]; query: string; model: string; maxScenes: number }): Promise<VisualSceneAnalysisJobReport> {
    return invoke<VisualSceneAnalysisJobReport>("visual_scene_analysis_start", { request });
  },

  async getVisualSceneAnalysisStatus(jobId: string): Promise<VisualSceneAnalysisJobReport> {
    return invoke<VisualSceneAnalysisJobReport>("visual_scene_analysis_status", { jobId });
  },

  async cancelVisualSceneAnalysis(jobId: string): Promise<void> {
    return invoke<void>("visual_scene_analysis_cancel", { jobId });
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

  async getMediaCacheStatus(): Promise<MediaCacheStatus> {
    return invoke<MediaCacheStatus>("media_cache_status");
  },

  async clearMediaCache(): Promise<MediaCacheStatus> {
    return invoke<MediaCacheStatus>("media_cache_clear");
  },

  async analyzeAudioLevels(assetId: string): Promise<AudioLevelReport> {
    return invoke<AudioLevelReport>("media_analyze_audio_levels", { assetId });
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
