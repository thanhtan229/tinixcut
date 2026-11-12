import { useCallback, useEffect, useRef, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { HealthStatus } from "./types/health";
import { EXPORT_PRESET_OPTIONS, ExportPreset } from "./types/export";
import { Asset, AudioCrossfade, ClipSpeed, ColorAdjustments, Crop, CutTransition, FrameRate, isRangeAdjacent, KeyframeEasing, KeyframeProperty, PlacementMode, Project, rationalTimeToFrames, rationalTimeToSeconds, TextStyle, TimelineItem, Transform, TransitionKind, VisualEffectPreset } from "./types/project";
import { CommandEnvelope, OperationEnvelope } from "./types/command";
import { EditPlanDraftReceipt, editorApi, OfflineMediaAsset, ProjectExportJobStatus, UnsavedRecoveryCandidate, VisualSceneAnalysisReport } from "./services/editorApi";
import { TopBar } from "./components/TopBar";
import { AutoCaptionDialog, type TranscriptSourceContext } from "./components/AutoCaptionDialog";
import { MediaCacheDialog } from "./components/MediaCacheDialog";
import { CaptionDraft, CaptionEditorDialog } from "./components/CaptionEditorDialog";
import { buildCaptionMergeOperations, buildCaptionTimingOperations, type CaptionTimingDraft } from "./components/captionEditor";
import { buildTranscriptCutOperations, type TranscriptCutRange } from "./components/transcriptCut";
import { EditPlanDraftDialog } from "./components/EditPlanDraftDialog";
import { AudioMixDraftDialog } from "./components/AudioMixDraftDialog";
import { MEDIA_FILE_EXTENSIONS } from "./utils/mediaFileTypes";
import { EditPlanReorderDialog } from "./components/EditPlanReorderDialog";
import { EditPlanTrimDialog } from "./components/EditPlanTrimDialog";
import { ProjectVariantDialog } from "./components/ProjectVariantDialog";
import { MediaPanel } from "./components/MediaPanel";
import { SourceViewerDialog } from "./components/SourceViewerDialog";
import { BulkRelinkDialog } from "./components/BulkRelinkDialog";
import { VisualSceneAnalysisDialog } from "./components/VisualSceneAnalysisDialog";
import { SceneGuidedDraftDialog } from "./components/SceneGuidedDraftDialog";
import { getMediaTimelinePlacement } from "./components/mediaTimelinePlacement";
import { sourceRangeSecondsToFrames } from "./components/mediaLibrary";
import { getPendingRecoverySnapshotRequest } from "./components/recoverySnapshot";
import { isProjectRevisionConflictStatus } from "./components/projectSaveConflict";
import { buildTimelinePasteOperations, captureTimelineClipboard, TimelineClipboard } from "./components/timelineClipboard";
import { getTimelineGapClosePlan } from "./components/timelineEditing";
import { getTimelineInsertPlan } from "./components/timelineInsert";
import { getTimelineOverwritePlan } from "./components/timelineOverwrite";
import { getTimelineRollPlan } from "./components/timelineRoll";
import { calculateSlipSourceRange } from "./components/timelineSlip";
import { TemplateBrowser } from "./components/TemplateBrowser";
import { TemplateAuthoringDialog } from "./components/TemplateAuthoringDialog";
import { TemplateDefinition, TemplateLayer, TemplateParameterValue } from "./types/template";
import {
  createTemplateApplyOp,
  createTemplateTrackApplyOp,
  createTextTrackTemplateApplyOp,
  isLegacyTemplateId,
  createTemplateParameterSetOp,
  createTemplateResetOp,
  createTemplateDetachOp,
  createTemplateLayerAddOp,
  createTemplateLayerRemoveOp,
  createTemplateLayerReorderOp,
  createTemplateLayerUpdateOp,
} from "./utils/templateHelpers";
import { PreviewCanvas } from "./components/PreviewCanvas";
import { Timeline } from "./components/Timeline";
import { Inspector } from "./components/Inspector";
import {
  clearSelection,
  createInitialSelection,
  reconcileSelection,
  selectSingle,
  SelectionState,
  toggleSelect,
} from "./utils/timelineSelection";
import { useTimelineShortcuts } from "./hooks/useTimelineShortcuts";
import "./App.css";

function timelineFramesToSourceFrames(
  timelineFrames: number,
  timelineRate: FrameRate,
  sourceRate: FrameRate,
  speed: ClipSpeed | undefined
): number | null {
  const [speedNumerator, speedDenominator] =
    speed === "Half" ? [1, 2] : speed === "Double" ? [2, 1] : [1, 1];
  const numerator =
    timelineFrames *
    timelineRate.denominator *
    speedNumerator *
    sourceRate.numerator;
  const denominator =
    timelineRate.numerator *
    speedDenominator *
    sourceRate.denominator;
  if (!Number.isSafeInteger(numerator) || !Number.isSafeInteger(denominator) || denominator === 0) {
    return null;
  }
  return numerator % denominator === 0 ? numerator / denominator : null;
}

function findTimelineItem(project: Project | null, itemId: string | null): TimelineItem | null {
  if (!project || !itemId) return null;
  for (const track of project.tracks) {
    const item = track.items.find((candidate) => candidate.item_id === itemId);
    if (item) return item;
  }
  return null;
}

function findVideoTrackForRange(
  project: Project,
  startFrame: number,
  durationFrames: number
) {
  const rate = project.settings.frame_rate;
  const endFrame = startFrame + durationFrames;
  return project.tracks.find((track) =>
    track.kind === "Video" &&
    !track.items.some((item) => {
      const itemStart = rationalTimeToFrames(item.timeline_range.start, rate);
      const itemEnd = itemStart + rationalTimeToFrames(item.timeline_range.duration, rate);
      return itemStart < endFrame && startFrame < itemEnd;
    })
  );
}

export function App() {
  const [project, setProject] = useState<Project | null>(null);
  const [offlineMedia, setOfflineMedia] = useState<Record<string, OfflineMediaAsset>>({});
  const [savedFilePath, setSavedFilePath] = useState<string | null>(null);
  const [savedRevision, setSavedRevision] = useState<number | null>(null);
  const [isInitialized, setIsInitialized] = useState<boolean>(false);
  const [hydrationError, setHydrationError] = useState<string | null>(null);
  const hydrationRunIdRef = useRef<number>(0);
  const [recoveryPrompt, setRecoveryPrompt] = useState<{
    path: string;
    projectRevision?: number;
    recoveryRevision?: number;
    recoveryFingerprint?: string;
    unreadableReason?: string;
  } | null>(null);
  const [recoverySaveState, setRecoverySaveState] = useState<"idle" | "saving" | "saved" | "failed">("idle");
  const [recoverySaveError, setRecoverySaveError] = useState<string | null>(null);
  const [recoverySaveRevision, setRecoverySaveRevision] = useState<number | null>(null);
  const [unsavedRecoveries, setUnsavedRecoveries] = useState<UnsavedRecoveryCandidate[]>([]);
  const [unsavedRecoveryDismissed, setUnsavedRecoveryDismissed] = useState(false);
  const recoverySaveGenerationRef = useRef(0);
  const [health, setHealth] = useState<HealthStatus | null>(null);
  const [selection, setSelection] = useState<SelectionState>(createInitialSelection());
  const selectedItemId = selection.primarySelectedItemId;
  const [selectedTransitionId, setSelectedTransitionId] = useState<string | null>(null);
  const [playheadFrame, setPlayheadFrame] = useState<number>(0);
  const timelineClipboardRef = useRef<TimelineClipboard | null>(null);
  const [snapEnabled, setSnapEnabled] = useState<boolean>(true);
  const [isPlaying, setIsPlaying] = useState<boolean>(false);
  const [playbackTrackPeaks, setPlaybackTrackPeaks] = useState<Record<string, number>>({});
  const [zoom, setZoom] = useState<number>(3);
  const [statusMsg, setStatusMsg] = useState<string>("Ready");
  const [exportPreset, setExportPreset] = useState<ExportPreset>("balanced");
  const [activeExport, setActiveExport] = useState<{
    jobId: string;
    outputPath: string;
    progress: number;
    cancelling: boolean;
  } | null>(null);
  const [isAutoCaptionOpen, setIsAutoCaptionOpen] = useState(false);
  const [isMediaCacheOpen, setIsMediaCacheOpen] = useState(false);
  const [previewProxyMode, setPreviewProxyMode] = useState<"off" | "auto">("off");
  const [mediaCacheGeneration, setMediaCacheGeneration] = useState(0);
  const [isCaptionEditorOpen, setIsCaptionEditorOpen] = useState(false);
  const [isEditPlanDraftOpen, setIsEditPlanDraftOpen] = useState(false);
  const [isAudioMixDraftOpen, setIsAudioMixDraftOpen] = useState(false);
  const [isEditPlanReorderOpen, setIsEditPlanReorderOpen] = useState(false);
  const [isEditPlanTrimOpen, setIsEditPlanTrimOpen] = useState(false);
  const [isProjectVariantOpen, setIsProjectVariantOpen] = useState(false);
  const [activeLeftTab, setActiveLeftTab] = useState<"media" | "templates">("media");
  const [isAuthoringOpen, setIsAuthoringOpen] = useState(false);
  const [autoCaptionReport, setAutoCaptionReport] = useState<Awaited<ReturnType<typeof editorApi.startAutoCaptions>> | null>(null);
  const [autoCaptionReportSource, setAutoCaptionReportSource] = useState<TranscriptSourceContext | null>(null);
  const [visualSceneAsset, setVisualSceneAsset] = useState<Asset | null>(null);
  const [sourceViewerAsset, setSourceViewerAsset] = useState<Asset | null>(null);
  const [sourceViewerSeekSeconds, setSourceViewerSeekSeconds] = useState(0);
  const [mediaImportProgress, setMediaImportProgress] = useState<string | null>(null);
  const mediaImportCancelRef = useRef(false);
  const [isBulkRelinkOpen, setIsBulkRelinkOpen] = useState(false);
  const [bulkRelinkMappings, setBulkRelinkMappings] = useState<Record<string, string>>({});
  const [bulkRelinkApplying, setBulkRelinkApplying] = useState(false);
  const [sceneDraftReport, setSceneDraftReport] = useState<VisualSceneAnalysisReport | null>(null);
  const audioUnlockRef = useRef<() => void>(() => {});
  const handlePlaybackTrackPeaksChange = useCallback((levels: Record<string, number> | null) => {
    setPlaybackTrackPeaks(levels ?? {});
  }, []);
  const selectedItem = findTimelineItem(project, selectedItemId);
  const selectedItemAsset = project?.assets.find((asset) => asset.asset_id === selectedItem?.asset_id) ?? null;
  const selectedItemTrack = project?.tracks.find((track) =>
    track.items.some((item) => item.item_id === selectedItem?.item_id),
  );
  const gapClosePlan = getTimelineGapClosePlan(project, selection.primarySelectedItemId);
  const requestPlaybackToggle = () => {
    if (!isPlaying) audioUnlockRef.current();
    setIsPlaying((playing) => !playing);
  };

  const hydrateProjectSession = async () => {
    const runId = ++hydrationRunIdRef.current;
    setHydrationError(null);
    let session: Awaited<ReturnType<typeof editorApi.getCurrentProjectSession>>;
    try {
      session = await editorApi.getCurrentProjectSession();
    } catch (err) {
      if (runId !== hydrationRunIdRef.current) return;
      const msg = err instanceof Error ? err.message : String(err);
      console.warn("Could not hydrate project session:", msg);
      setHydrationError(msg);
      return;
    }

    if (runId !== hydrationRunIdRef.current) return;
    setProject(session.project);
    setSavedFilePath(session.canonicalPath);
    setSavedRevision(session.persistedRevision);
    setSelection((prev) => reconcileSelection(prev, session.project));
    setIsInitialized(true);

    try {
      const offline = await editorApi.listOfflineMedia();
      if (runId !== hydrationRunIdRef.current) return;
      setOfflineMedia(Object.fromEntries(offline.map((asset) => [asset.asset_id, asset])));
    } catch (offlineErr) {
      if (runId !== hydrationRunIdRef.current) return;
      const offlineMsg = offlineErr instanceof Error ? offlineErr.message : String(offlineErr);
      setStatusMsg(`Project loaded, but offline media status check failed: ${offlineMsg}`);
    }
  };

  const refreshProject = async () => {
    try {
      const p = await editorApi.getCurrentProject();
      setProject(p);
      setSelection((prev) => reconcileSelection(prev, p));
      const offline = await editorApi.listOfflineMedia();
      setOfflineMedia(Object.fromEntries(offline.map((asset) => [asset.asset_id, asset])));
    } catch (err) {
      console.warn("Could not get current project:", err);
    }
  };

  useEffect(() => {
    let cancelled = false;
    editorApi.getHealth().then((h) => {
      if (!cancelled) setHealth(h);
    }).catch(console.warn);

    hydrateProjectSession();

    editorApi.getAppSettings().then((settings) => {
      if (!cancelled) setPreviewProxyMode(settings.previewProxyMode);
    }).catch((error) => {
      if (!cancelled) setStatusMsg(`Could not read app settings; preview proxies stay off: ${error}`);
    });

    editorApi.listUnsavedRecoveries().then((recoveries) => {
      if (!cancelled) setUnsavedRecoveries(recoveries);
    }).catch((error) => {
      if (!cancelled) setStatusMsg(`Could not check unsaved recovery copies: ${error}`);
    });

    return () => {
      cancelled = true;
      hydrationRunIdRef.current++;
    };
  }, []);

  useEffect(() => {
    const generation = ++recoverySaveGenerationRef.current;
    if (!isInitialized || !project) {
      setRecoverySaveState("idle");
      setRecoverySaveError(null);
      return;
    }

    const request = getPendingRecoverySnapshotRequest(
      project.project_id,
      project.revision,
      savedFilePath,
      savedRevision,
    );

    if (!request) {
      setRecoverySaveState("idle");
      setRecoverySaveError(null);
      return;
    }

    setRecoverySaveState("saving");
    setRecoverySaveError(null);
    const timer = window.setTimeout(() => {
      const saveSnapshot =
        request.kind === "saved-project"
          ? editorApi.saveRecoverySnapshot(request.path, request.revision)
          : editorApi.saveUnsavedRecovery(request.projectId, request.revision);
      void saveSnapshot.then((receipt) => {
        if (generation !== recoverySaveGenerationRef.current) return;
        if (receipt) setRecoverySaveRevision("recoveryRevision" in receipt ? receipt.recoveryRevision : receipt.projectRevision);
        setRecoverySaveState("saved");
      }).catch((error: unknown) => {
        if (generation !== recoverySaveGenerationRef.current) return;
        setRecoverySaveError(error instanceof Error ? error.message : String(error));
        setRecoverySaveState("failed");
      });
    }, 900);

    return () => window.clearTimeout(timer);
  }, [isInitialized, project?.project_id, project?.revision, savedFilePath, savedRevision]);

  useEffect(() => {
    if (!autoCaptionReport || !["queued", "running"].includes(autoCaptionReport.state)) return;
    let cancelled = false;
    const poll = async () => {
      try {
        const report = await editorApi.getAutoCaptionStatus(autoCaptionReport.jobId);
        if (cancelled) return;
        setAutoCaptionReport(report);
        if (report.state === "completed") {
          setStatusMsg(report.message);
          await refreshProject();
        } else if (report.state === "failed" || report.state === "cancelled") {
          setStatusMsg(report.message);
        }
      } catch (err) {
        if (!cancelled) setStatusMsg(`Auto captions status failed: ${err}`);
      }
    };
    void poll();
    const timer = window.setInterval(() => void poll(), 500);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [autoCaptionReport?.jobId, autoCaptionReport?.state]);

  useEffect(() => {
    if (!activeExport) return;
    let cancelled = false;
    let timer = 0;
    const poll = async () => {
      try {
        const report: ProjectExportJobStatus = await editorApi.getProjectExportStatus(activeExport.jobId);
        if (cancelled) return;
        setActiveExport((current) => current?.jobId === report.jobId
          ? { ...current, progress: report.progress }
          : current);
        if (report.status === "completed") {
          setStatusMsg(`Export complete: ${activeExport.outputPath}`);
          setActiveExport(null);
          return;
        }
        if (report.status === "cancelled") {
          setStatusMsg("Export cancelled; temporary output was removed.");
          setActiveExport(null);
          return;
        }
        if (report.status === "failed") {
          setStatusMsg(`Export error: ${report.error ?? "Unknown export failure"}`);
          setActiveExport(null);
          return;
        }
      } catch (error) {
        if (!cancelled) setStatusMsg(`Export status check failed: ${error}`);
      }
      if (!cancelled) timer = window.setTimeout(() => void poll(), 400);
    };
    void poll();
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [activeExport?.jobId]);

  const commandQueueRef = useRef<Promise<any>>(Promise.resolve());

  const executeCommandEnvelope = async (
    operations:
      | CommandEnvelope["operations"]
      | ((project: Project) => CommandEnvelope["operations"]),
    desc: string
  ): Promise<any> => {
    const task = commandQueueRef.current.then(async () => {
      const currentProject = await editorApi.getCurrentProject();
      if (!currentProject) {
        throw new Error("No active project");
      }
      const resolvedOperations =
        typeof operations === "function" ? operations(currentProject) : operations;

      const envelope: CommandEnvelope = {
        schema_version: currentProject.schema_version,
        project_id: currentProject.project_id,
        base_revision: currentProject.revision,
        dry_run: false,
        transaction_id: `tx_${Date.now()}`,
        operations: resolvedOperations,
      };

      try {
        const receipt = await editorApi.executeCommand(envelope);
        setStatusMsg(`${desc} committed (rev ${receipt.newRevision})`);
        const updated = await editorApi.getCurrentProject();
        setProject(updated);
        setSelection((prev) => reconcileSelection(prev, updated));
        return receipt;
      } catch (err: unknown) {
        const msg = err instanceof Error ? err.message : String(err);
        setStatusMsg(`Command failed: ${msg}`);
        try {
          const reconciled = await editorApi.getCurrentProject();
          setProject(reconciled);
          setSelection((prev) => reconcileSelection(prev, reconciled));
        } catch {
          // ignore reconcile error
        }
        throw err;
      }
    });

    commandQueueRef.current = task.catch(() => {});
    return task;
  };

  const handleApplyEditPlanDraft = async (receipt: EditPlanDraftReceipt) => {
    await executeCommandEnvelope((currentProject) => {
      if (currentProject.project_id !== receipt.plan.projectId || currentProject.revision !== receipt.plan.baseRevision) {
        throw new Error("The project changed after this draft was reviewed. Generate a new preview before applying.");
      }
      return receipt.plan.operations.map((operation) => ({
        operation_id: operation.operationId,
        operation: operation.operation,
      }));
    }, "Apply edit plan");
  };

  const persistPendingRecovery = async () => {
    if (!project) return;
    const request = getPendingRecoverySnapshotRequest(
      project.project_id,
      project.revision,
      savedFilePath,
      savedRevision,
    );
    if (request?.kind === "saved-project") {
      await editorApi.saveRecoverySnapshot(request.path, request.revision);
    } else if (request?.kind === "unsaved-project") {
      await editorApi.saveUnsavedRecovery(request.projectId, request.revision);
    }
  };

  const handleNewProject = async () => {
    try {
      await persistPendingRecovery();
      const p = await editorApi.newProject("Untitled Video Project");
      setProject(p);
      setSavedFilePath(null);
      setSavedRevision(null);
      setSelection(clearSelection());
      setSelectedTransitionId(null);
      setPlayheadFrame(0);
      setIsInitialized(true);
      setStatusMsg("New project created");
    } catch (err) {
      setStatusMsg(`Failed to create project: ${err}`);
    }
  };

  const handleCanvasPresetChange = (width: number, height: number, label: string) => {
    if (!project || (project.settings.width === width && project.settings.height === height)) return;

    executeCommandEnvelope(
      [
        {
          operation_id: `op_canvas_${width}x${height}_${Date.now()}`,
          operation: {
            type: "ProjectSettingsSet",
            payload: { width, height },
          },
        },
      ],
      `Canvas preset: ${label}`,
    );
  };

  const handleCanvasBackgroundChange = (color: string) => {
    if (!project || project.settings.canvas_background_color === color) return;

    executeCommandEnvelope(
      [
        {
          operation_id: `op_canvas_background_${Date.now()}`,
          operation: {
            type: "CanvasBackgroundSet",
            payload: { color },
          },
        },
      ],
      "Canvas background",
    );
  };

  const handleOpenProject = async () => {
    try {
      const selected = await open({
        title: "Open TinixCut Project",
        filters: [{ name: "TinixCut Project", extensions: ["tinix", "tcproj"] }],
        multiple: false,
      });
      if (!selected) return;
      const chosenPath = Array.isArray(selected) ? selected[0] : selected;
      const recovery = await editorApi.checkRecovery(chosenPath);
      if (
        typeof recovery === "object" &&
        ("NewerRecoveryAvailable" in recovery || "RecoveryDiffersFromProject" in recovery || "RecoveryUnreadable" in recovery)
      ) {
        if ("RecoveryUnreadable" in recovery) {
          setRecoveryPrompt({
            path: chosenPath,
            unreadableReason: recovery.RecoveryUnreadable.reason,
          });
          return;
        }
        const details = "NewerRecoveryAvailable" in recovery
          ? recovery.NewerRecoveryAvailable
          : recovery.RecoveryDiffersFromProject;
        setRecoveryPrompt({
          path: chosenPath,
          projectRevision: details.project_revision,
          recoveryRevision: details.recovery_revision,
          recoveryFingerprint: details.recovery_fingerprint,
        });
        return;
      }
      await openProjectIntoEditor(chosenPath);
    } catch (err) {
      setStatusMsg(`Open error: ${err}`);
    }
  };

  const openProjectIntoEditor = async (path: string) => {
    await persistPendingRecovery();
    const p = await editorApi.openProject(path);
    setProject(p);
    setSavedFilePath(path);
    setSavedRevision(p.revision);
    setSelection(clearSelection());
    setSelectedTransitionId(null);
    setPlayheadFrame(0);
    setIsInitialized(true);
    try {
      await editorApi.addRecentProject(path);
      setStatusMsg(`Loaded project: ${p.name}`);
    } catch (error) {
      setStatusMsg(`Loaded project: ${p.name}; recent-project settings could not be updated: ${error}`);
    }
    const offline = await editorApi.listOfflineMedia();
    setOfflineMedia(Object.fromEntries(offline.map((asset) => [asset.asset_id, asset])));
  };

  const handleOpenVariant = async (path: string) => {
    if (!project) return;
    await openProjectIntoEditor(path);
  };

  const handleOpenSavedVersion = async () => {
    if (!recoveryPrompt) return;
    const prompt = recoveryPrompt;
    try {
      await openProjectIntoEditor(prompt.path);
      setRecoveryPrompt(null);
    } catch (err) {
      setStatusMsg(`Open error: ${err}`);
    }
  };

  const handleRestoreRecovery = async () => {
    if (!recoveryPrompt || recoveryPrompt.unreadableReason || recoveryPrompt.projectRevision === undefined || recoveryPrompt.recoveryRevision === undefined || recoveryPrompt.recoveryFingerprint === undefined) return;
    const prompt = recoveryPrompt;
    const projectRevision = prompt.projectRevision as number;
    const recoveryRevision = prompt.recoveryRevision as number;
    const recoveryFingerprint = prompt.recoveryFingerprint as string;
    try {
      await persistPendingRecovery();
      const receipt = await editorApi.restoreRecovery(
        prompt.path,
        projectRevision,
        recoveryRevision,
        recoveryFingerprint,
      );
      setProject(receipt.project);
      setSavedFilePath(prompt.path);
      setSavedRevision(receipt.persistedRevision);
      setSelection(clearSelection());
      setSelectedTransitionId(null);
      setPlayheadFrame(0);
      setIsInitialized(true);
      setStatusMsg(`Recovered edits from revision ${prompt.recoveryRevision}; the saved project file is unchanged`);
      const offline = await editorApi.listOfflineMedia();
      setOfflineMedia(Object.fromEntries(offline.map((asset) => [asset.asset_id, asset])));
      setRecoveryPrompt(null);
    } catch (err) {
      setStatusMsg(`Recovery failed: ${err}`);
    }
  };

  const handleRestoreUnsavedRecovery = async (candidate: UnsavedRecoveryCandidate) => {
    if (!project) return;
    try {
      const recovered = await editorApi.restoreUnsavedRecovery(
        candidate.projectId,
        candidate.revision,
        project.project_id,
        project.revision,
      );
      setProject(recovered);
      setSavedFilePath(null);
      setSavedRevision(null);
      setSelection(clearSelection());
      setSelectedTransitionId(null);
      setPlayheadFrame(0);
      setIsInitialized(true);
      setStatusMsg(`Recovered unsaved project “${recovered.name}” at revision ${recovered.revision}`);
      setUnsavedRecoveries((current) => current.filter((entry) => entry.projectId !== candidate.projectId));
      setUnsavedRecoveryDismissed(true);
      const offline = await editorApi.listOfflineMedia();
      setOfflineMedia(Object.fromEntries(offline.map((asset) => [asset.asset_id, asset])));
    } catch (error) {
      setStatusMsg(`Unsaved recovery failed: ${error}`);
    }
  };

  const handleDiscardUnsavedRecovery = async (candidate: UnsavedRecoveryCandidate) => {
    if (!window.confirm(`Permanently discard the recovery copy “${candidate.name}” at revision ${candidate.revision}?`)) return;
    try {
      await editorApi.discardUnsavedRecovery(candidate.projectId, candidate.revision);
      setUnsavedRecoveries((current) => current.filter((entry) => entry.projectId !== candidate.projectId));
      setStatusMsg(`Discarded recovery copy “${candidate.name}”`);
    } catch (error) {
      setStatusMsg(`Could not discard recovery copy: ${error}`);
    }
  };

  const handleSaveProject = async (forceSaveAs = false) => {
    try {
      let targetPath = forceSaveAs ? null : savedFilePath;
      if (!targetPath) {
        const chosen = await save({
          title: forceSaveAs ? "Save a Copy of TinixCut Project" : "Save TinixCut Project",
          defaultPath: forceSaveAs
            ? `${project?.name || "project"} - recovered.tinix`
            : `${project?.name || "project"}.tinix`,
          filters: [{ name: "TinixCut Project", extensions: ["tinix", "tcproj"] }],
        });
        if (!chosen) {
          setStatusMsg("Save cancelled");
          return;
        }
        targetPath = chosen;
      }

      const task = commandQueueRef.current.then(async () => {
        const current = await editorApi.getCurrentProject();
        const path = await editorApi.saveProject(targetPath);
        setProject(current);
        setSavedFilePath(path);
        setSavedRevision(current.revision);
        setIsInitialized(true);
        setUnsavedRecoveries((recoveries) => recoveries.filter((candidate) => candidate.projectId !== current.project_id));
        try {
          await editorApi.addRecentProject(path);
          setStatusMsg(`Project saved atomically to ${path}`);
        } catch (error) {
          setStatusMsg(`Project saved to ${path}; recent-project settings could not be updated: ${error}`);
        }
      });
      commandQueueRef.current = task.catch(() => {});
      await task;
    } catch (err) {
      setStatusMsg(`Save error: ${err}`);
    }
  };

  const handleCollectProjectMedia = async () => {
    if (!project) return;
    try {
      const chosen = await open({
        title: "Choose a folder for the collected project",
        directory: true,
        multiple: false,
      });
      if (!chosen) {
        setStatusMsg("Media collection cancelled");
        return;
      }

      const task = commandQueueRef.current.then(async () => {
        const current = await editorApi.getCurrentProject();
        const destinationDirectory = Array.isArray(chosen) ? chosen[0] : chosen;
        if (!destinationDirectory) return;
        const receipt = await editorApi.collectProjectMedia(destinationDirectory, current.project_id, current.revision);
        let opened: Project;
        try {
          opened = await editorApi.openProject(receipt.projectPath);
        } catch (openError) {
          setStatusMsg(`Collection succeeded at ${receipt.projectPath}, but TinixCut could not open the collected copy: ${openError}`);
          return;
        }
        setProject(opened);
        setSavedFilePath(receipt.projectPath);
        setSavedRevision(opened.revision);
        setIsInitialized(true);
        try {
          setOfflineMedia(Object.fromEntries((await editorApi.listOfflineMedia()).map((asset) => [asset.asset_id, asset])));
        } catch (refreshError) {
          setStatusMsg(`Collected project opened, but media status could not refresh: ${refreshError}`);
          return;
        }
        try {
          await editorApi.addRecentProject(receipt.projectPath);
        } catch (settingsError) {
          setStatusMsg(`Collected project opened, but recent-project settings could not be updated: ${settingsError}`);
          return;
        }
        setStatusMsg(`Collected ${receipt.copiedAssets} media file(s) into ${receipt.mediaDirectory}; opened ${receipt.projectPath}`);
      });
      commandQueueRef.current = task.catch(() => {});
      await task;
    } catch (error) {
      setStatusMsg(`Media collection failed: ${error}`);
    }
  };

  const handleExportProject = async () => {
    try {
      if (!project || activeExport) return;
      const chosen = await save({
        title: "Export Project to MP4",
        defaultPath: `${project.name || "video"}.mp4`,
        filters: [{ name: "MP4 Video", extensions: ["mp4"] }],
      });
      if (!chosen) {
        setStatusMsg("Export cancelled");
        return;
      }
      setStatusMsg("Checking media, project settings, output path and available disk space...");
      const preflight = await editorApi.preflightExport(chosen, exportPreset);
      const preset = EXPORT_PRESET_OPTIONS.find((option) => option.value === exportPreset);
      const diskSpace = `${(preflight.availableDiskBytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
      const replacement = preflight.outputExists ? " Existing output will be replaced only after a successful render." : "";
      setStatusMsg(
        `Preflight passed: ${preflight.sourceCount} source(s), ${preflight.width}×${preflight.height}, ${preflight.durationSecs.toFixed(1)}s; ${diskSpace} currently free.${replacement} Rendering ${preset?.label ?? "Balanced"} snapshot...`
      );
      const receipt = await editorApi.startProjectExport(chosen, exportPreset);
      setActiveExport({ jobId: receipt.jobId, outputPath: receipt.outputPath, progress: 0, cancelling: false });
      setStatusMsg("Export started. Progress and cancellation are available in the toolbar.");
    } catch (err) {
      setStatusMsg(`Export error: ${err}`);
    }
  };

  const handleCancelExport = async () => {
    if (!activeExport || activeExport.cancelling) return;
    setActiveExport((current) => current ? { ...current, cancelling: true } : current);
    try {
      const accepted = await editorApi.cancelProjectExport(activeExport.jobId);
      if (!accepted) {
        setActiveExport((current) => current?.jobId === activeExport.jobId
          ? { ...current, cancelling: false }
          : current);
      }
    } catch (error) {
      setActiveExport((current) => current?.jobId === activeExport.jobId
        ? { ...current, cancelling: false }
        : current);
      setStatusMsg(`Could not cancel export: ${error}`);
    }
  };

  const handleImportMedia = async () => {
    try {
      const selected = await open({
        title: "Import Media Files",
        filters: [
          {
            name: "Media Files",
            extensions: [...MEDIA_FILE_EXTENSIONS],
          },
        ],
        multiple: true,
      });
      if (!selected) return;
      const paths = Array.isArray(selected) ? selected : [selected];
      mediaImportCancelRef.current = false;
      const failures: string[] = [];
      let imported = 0;
      for (const [index, path] of paths.entries()) {
        if (mediaImportCancelRef.current) break;
        const fileName = path.split(/[\\/]/).pop() || `File ${index + 1}`;
        setMediaImportProgress(`Importing ${index + 1} of ${paths.length}: ${fileName}`);
        try {
          await editorApi.importMedia(path);
          imported += 1;
        } catch (error) {
          failures.push(`${fileName}: ${error}`);
        }
      }
      if (imported > 0) await refreshProject();
      const cancelled = mediaImportCancelRef.current;
      const summary = `Imported ${imported} of ${paths.length} media file(s)${cancelled ? " (cancelled)" : ""}`;
      setStatusMsg(failures.length ? `${summary}; ${failures.join("; ")}` : summary);
    } catch (err) {
      setStatusMsg(`Import failed: ${err}`);
    } finally {
      setMediaImportProgress(null);
    }
  };

  const handleCancelMediaImport = () => {
    mediaImportCancelRef.current = true;
    setMediaImportProgress("Cancelling after the current file…");
  };

  const handleRelinkAsset = async (asset: Asset) => {
    try {
      const selected = await open({
        title: `Relink ${asset.name}`,
        filters: [{ name: "Media Files", extensions: [...MEDIA_FILE_EXTENSIONS] }],
        multiple: false,
      });
      if (!selected) return;
      const path = Array.isArray(selected) ? selected[0] : selected;
      await executeCommandEnvelope(
        [{
          operation_id: `op_asset_relink_${asset.asset_id}_${Date.now()}`,
          operation: { type: "AssetRelink", payload: { asset_id: asset.asset_id, path } },
        }],
        `Relink ${asset.name}`,
      );
      await refreshProject();
      setStatusMsg(`Relinked ${asset.name}`);
    } catch (err) {
      setStatusMsg(`Relink failed for ${asset.name}: ${err}`);
    }
  };

  const handleChooseBulkRelinkFile = async (asset: Asset) => {
    const selected = await open({
      title: `Choose replacement for ${asset.name}`,
      filters: [{ name: "Media Files", extensions: [...MEDIA_FILE_EXTENSIONS] }],
      multiple: false,
    });
    if (!selected) return;
    const path = Array.isArray(selected) ? selected[0] : selected;
    setBulkRelinkMappings((current) => ({ ...current, [asset.asset_id]: path }));
  };

  const handleApplyBulkRelink = async () => {
    if (!project) return;
    const offlineIds = Object.keys(offlineMedia);
    const assetsToRelink = project.assets.filter((asset) => offlineIds.includes(asset.asset_id));
    if (assetsToRelink.length === 0 || assetsToRelink.some((asset) => !bulkRelinkMappings[asset.asset_id])) return;
    setBulkRelinkApplying(true);
    try {
      await executeCommandEnvelope(
        assetsToRelink.map((asset, index) => ({
          operation_id: `op_asset_relink_${asset.asset_id}_${Date.now()}_${index}`,
          operation: { type: "AssetRelink" as const, payload: { asset_id: asset.asset_id, path: bulkRelinkMappings[asset.asset_id] } },
        })),
        `Relink ${assetsToRelink.length} media assets`,
      );
      setBulkRelinkMappings({});
      setIsBulkRelinkOpen(false);
      try {
        setOfflineMedia(Object.fromEntries((await editorApi.listOfflineMedia()).map((asset) => [asset.asset_id, asset])));
      } catch (refreshError) {
        let reconciliation = "";
        try {
          await refreshProject();
        } catch (reconciliationError) {
          reconciliation = ` Project refresh also failed: ${reconciliationError}`;
        }
        setStatusMsg(`Relink committed, but offline-media status could not refresh: ${refreshError}.${reconciliation}`);
      }
    } catch (error) {
      setStatusMsg(`Bulk relink failed; project transaction was rejected: ${error}`);
    } finally {
      setBulkRelinkApplying(false);
    }
  };

  const handleImportCaptions = async () => {
    try {
      const selected = await open({
        title: "Import Captions",
        filters: [{ name: "Caption Files", extensions: ["srt", "vtt"] }],
        multiple: false,
      });
      if (!selected) return;
      const chosenPath = Array.isArray(selected) ? selected[0] : selected;
      const report = await editorApi.importCaptions(chosenPath);
      setStatusMsg(`Imported ${report.cueCount} caption(s) into a new track`);
      await refreshProject();
    } catch (err) {
      setStatusMsg(`Caption import failed: ${err}`);
    }
  };

  const handleExportCaptions = async () => {
    try {
      if (!project) return;
      const chosen = await save({
        title: "Export Captions",
        defaultPath: `${project.name || "captions"}.srt`,
        filters: [{ name: "SubRip Captions", extensions: ["srt"] }, { name: "WebVTT Captions", extensions: ["vtt"] }],
      });
      if (!chosen) {
        setStatusMsg("Caption export cancelled");
        return;
      }
      const report = await editorApi.exportCaptions(chosen);
      setStatusMsg(`Exported ${report.cueCount} caption(s) to ${report.outputPath}`);
    } catch (err) {
      setStatusMsg(`Caption export failed: ${err}`);
    }
  };

  const handleApplyCaptionEdits = async (textDrafts: CaptionDraft[], timingDrafts: CaptionTimingDraft[]) => {
    if (!project || (textDrafts.length === 0 && timingDrafts.length === 0)) return;
    const textOperations = textDrafts.map(({ item_id, text }) => ({
        operation_id: `op_caption_${item_id}_${Date.now()}`,
        operation: { type: "ClipTextSet" as const, payload: { item_id, text } },
      }));
    const timingOperations = buildCaptionTimingOperations(project, timingDrafts);
    const operations = [...textOperations, ...timingOperations];
    if (operations.length === 0) return;
    await executeCommandEnvelope(operations, `Update ${textDrafts.length} caption text(s) and ${timingOperations.length} timing(s)`);
  };

  const handleBuildTranscriptCut = async (assetId: string, ranges: TranscriptCutRange[]) => {
    if (!project) throw new Error("Open a project before creating a transcript cut.");
    const asset = project.assets.find((candidate) => candidate.asset_id === assetId);
    if (!asset) throw new Error("The selected source asset is no longer in this project.");
    if (asset.media_kind === "Image") throw new Error("Transcript cuts require an audio or video source.");
    const operations = buildTranscriptCutOperations(asset, ranges, project.settings.frame_rate, playheadFrame);
    await executeCommandEnvelope(operations, `Create transcript rough cut from ${asset.name}`);
  };

  const handleMergeCaptions = async (leftId: string, rightId: string) => {
    if (!project) return;
    await executeCommandEnvelope(buildCaptionMergeOperations(project, leftId, rightId), "Merge adjacent captions");
  };

  const handleSplitCaption = async (itemId: string, frame: number) => {
    if (!project) return;
    await executeCommandEnvelope(
      [{
        operation_id: `op_caption_split_${itemId}_${frame}_${Date.now()}`,
        operation: {
          type: "ClipSplit",
          payload: {
            item_id: itemId,
            split_time: { value: frame, rate: project.settings.frame_rate },
          },
        },
      }],
      "Split caption"
    );
  };

  const handleUndo = async () => {
    const task = commandQueueRef.current.then(async () => {
      const desc = await editorApi.undo();
      if (desc) {
        setStatusMsg(`Undid: ${desc}`);
        await refreshProject();
      } else {
        setStatusMsg("Nothing to undo");
      }
    });
    commandQueueRef.current = task.catch(() => {});
    try {
      await task;
    } catch (err) {
      setStatusMsg(`Undo failed: ${err}`);
    }
  };

  const handleRedo = async () => {
    const task = commandQueueRef.current.then(async () => {
      const desc = await editorApi.redo();
      if (desc) {
        setStatusMsg(`Redid: ${desc}`);
        await refreshProject();
      } else {
        setStatusMsg("Nothing to redo");
      }
    });
    commandQueueRef.current = task.catch(() => {});
    try {
      await task;
    } catch (err) {
      setStatusMsg(`Redo failed: ${err}`);
    }
  };

  const handleAddTrack = () => {
    if (!project) return;
    const trackNum = project.tracks.length + 1;
    executeCommandEnvelope(
      [
        {
          operation_id: `op_add_trk_${Date.now()}`,
          operation: {
            type: "TrackCreate",
            payload: {
              name: `Track ${trackNum}`,
              kind: "Video",
            },
          },
        },
      ],
      "Add Track"
    );
  };

  const handleAddClip = () => {
    if (!project) return;
    if (project.tracks.length === 0) {
      handleAddTrack();
      return;
    }
    const targetTrack = project.tracks[0];
    const rate = project.settings.frame_rate;
    // Calculate start time to avoid overlap
    let startF = 0;
    for (const it of targetTrack.items) {
      const endF = it.timeline_range.start.value + it.timeline_range.duration.value;
      if (endF > startF) startF = endF;
    }

    const clipNum = targetTrack.items.length + 1;
    executeCommandEnvelope(
      [
        {
          operation_id: `op_add_clip_${Date.now()}`,
          operation: {
            type: "ClipCreate",
            payload: {
              track_id: targetTrack.track_id,
              name: `Clip ${clipNum}`,
              kind: "Video",
              timeline_start: { value: startF, rate },
              duration: { value: 60, rate }, // 2 seconds at 30fps
            },
          },
        },
      ],
      "Add Clip"
    );
  };

  const handleAddText = () => {
    if (!project) return;

    const rate = project.settings.frame_rate;
    const now = Date.now();
    const duration = { value: 150, rate };
    const track = findVideoTrackForRange(project, playheadFrame, duration.value);
    const trackId = track?.track_id ?? `trk_text_${now}`;
    const itemId = `txt_${now}`;
    const operations: OperationEnvelope[] = [];

    if (!track) {
      operations.push({
        operation_id: `op_add_text_trk_${now}`,
        operation: {
          type: "TrackCreate",
          payload: {
            track_id: trackId,
            name: "Text Track",
            kind: "Video",
          },
        },
      });
    }

    operations.push(
      {
        operation_id: `op_add_text_clip_${now}`,
        operation: {
          type: "ClipCreate",
          payload: {
            track_id: trackId,
            item_id: itemId,
            name: "Text",
            kind: "Text",
            timeline_start: { value: playheadFrame, rate },
            duration,
          },
        },
      },
      {
        operation_id: `op_add_text_template_${now}`,
        operation: {
          type: "TextTemplateApply",
          payload: {
            item_id: itemId,
            template_id: "clean-lower-third",
            template_version: 1,
          },
        },
      }
    );

    executeCommandEnvelope(operations, "Add Text");
  };

  const handleAddAssetToTimeline = (
    asset: Asset,
    sourceInSeconds = 0,
    sourceOutSeconds?: number,
    insertAtPlayhead = false,
    overwriteAtPlayhead = false,
  ) => {
    if (!project) return;
    const rate = project.settings.frame_rate;
    const isAudio = asset.media_kind === "Audio";
    const assetDurationSeconds = asset.duration
      ? asset.duration.value * asset.duration.rate.denominator / asset.duration.rate.numerator
      : 5;
    const selectedOut = sourceOutSeconds ?? assetDurationSeconds;
    const sourceRange = sourceRangeSecondsToFrames(sourceInSeconds, selectedOut, rate);
    if (!sourceRange || selectedOut > assetDurationSeconds) {
      setStatusMsg(`Invalid source range for ${asset.name}`);
      return;
    }
    const { start: sourceStartFrame, duration: durationFrames } = sourceRange;
    const now = Date.now();
    const placement = getMediaTimelinePlacement(project, asset);
    const trackId = placement.track?.track_id ?? `trk_${now}`;
    const operations: OperationEnvelope[] = [];
    if (!placement.track) {
      operations.push({
        operation_id: `op_add_trk_${now}`,
        operation: {
          type: "TrackCreate" as const,
          payload: {
            track_id: trackId,
            name: isAudio ? `Audio Track ${project.tracks.length + 1}` : `Video Track ${project.tracks.length + 1}`,
            kind: isAudio ? "Audio" as const : "Video" as const,
          },
        },
      });
    }
    if (insertAtPlayhead && placement.track) {
      const rightItemId = `clip_insert_right_${now}`;
      let plan;
      try {
        plan = getTimelineInsertPlan(project, trackId, playheadFrame, durationFrames, rightItemId);
      } catch (error) {
        setStatusMsg(`Could not insert ${asset.name}: ${error}`);
        return;
      }
      if (plan.splitItemId) {
        operations.push({
          operation_id: `op_insert_split_${now}`,
          operation: {
            type: "ClipSplit",
            payload: {
              item_id: plan.splitItemId,
              split_time: { value: playheadFrame, rate },
              new_right_item_id: rightItemId,
            },
          },
        });
      }
      if (plan.downstreamItemIds.length > 0) {
        operations.push({
          operation_id: `op_insert_shift_${now}`,
          operation: {
            type: "ClipGroupMove",
            payload: {
              item_ids: plan.downstreamItemIds,
              delta: { value: durationFrames, rate },
            },
          },
        });
      }
    }
    if (overwriteAtPlayhead && placement.track) {
      let plan;
      try {
        plan = getTimelineOverwritePlan(project, trackId, playheadFrame, durationFrames, `overwrite_${now}`);
      } catch (error) {
        setStatusMsg(`Could not overwrite with ${asset.name}: ${error}`);
        return;
      }
      plan.splits.forEach((split, index) => {
        operations.push({
          operation_id: `op_overwrite_split_${now}_${index}`,
          operation: {
            type: "ClipSplit",
            payload: {
              item_id: split.itemId,
              split_time: { value: split.frame, rate },
              new_right_item_id: split.rightItemId,
            },
          },
        });
      });
      plan.deleteItemIds.forEach((itemId, index) => {
        operations.push({
          operation_id: `op_overwrite_delete_${now}_${index}`,
          operation: { type: "ClipDelete", payload: { item_id: itemId } },
        });
      });
    }
    const placingAtPlayhead = insertAtPlayhead || overwriteAtPlayhead;
    const insertedItemId = placingAtPlayhead ? `clip_at_playhead_${now}` : undefined;
    operations.push({
      operation_id: `op_add_clip_${now + 1}`,
      operation: {
        type: "ClipCreate" as const,
        payload: {
          ...(insertedItemId ? { item_id: insertedItemId } : {}),
          track_id: trackId,
          name: asset.name,
          kind: isAudio ? "Audio" : (asset.media_kind === "Image" ? "Image" : "Video"),
          asset_id: asset.asset_id,
          timeline_start: { value: placingAtPlayhead ? playheadFrame : placement.startFrame, rate },
          duration: { value: durationFrames, rate },
          source_start: { value: sourceStartFrame, rate },
        },
      },
    });
    if (placement.insertBeforeIndex !== null) {
      operations.push({
        operation_id: `op_reorder_media_trk_${now + 2}`,
        operation: {
          type: "TrackReorder",
          payload: { track_id: trackId, new_index: placement.insertBeforeIndex },
        },
      });
    }
    if (insertedItemId) {
      void executeCommandEnvelope(operations, `${overwriteAtPlayhead ? "Ghi đè" : "Chèn"} "${asset.name}" tại playhead`)
        .then(() => {
          setSelection({ selectedItemIds: new Set([insertedItemId]), primarySelectedItemId: insertedItemId });
        })
        .catch(() => {});
    } else {
      executeCommandEnvelope(operations, `Thêm "${asset.name}" vào Timeline`);
    }
  };

  const handleInsertAssetAtPlayhead = (asset: Asset) => {
    handleAddAssetToTimeline(asset, 0, undefined, true);
  };

  const handleOverwriteAssetAtPlayhead = (asset: Asset) => {
    handleAddAssetToTimeline(asset, 0, undefined, false, true);
  };

  const handleMoveClip = (itemId: string, newStartFrame: number) => {
    if (!project) return;
    const rate = project.settings.frame_rate;
    executeCommandEnvelope(
      [
        {
          operation_id: `op_move_${Date.now()}`,
          operation: {
            type: "ClipMove",
            payload: {
              item_id: itemId,
              target_start: { value: newStartFrame, rate },
            },
          },
        },
      ],
      "Move Clip"
    );
  };

  const handleGroupMoveClip = (itemIds: string[], deltaFrames: number) => {
    if (!project || deltaFrames === 0 || itemIds.length === 0) return;
    const rate = project.settings.frame_rate;
    executeCommandEnvelope(
      [
        {
          operation_id: `op_grp_move_${Date.now()}`,
          operation: {
            type: "ClipGroupMove",
            payload: {
              item_ids: itemIds,
              delta: { value: deltaFrames, rate },
            },
          },
        },
      ],
      `Move ${itemIds.length} Clip(s)`
    );
  };

  const handleTrimClip = (
    itemId: string,
    newStartFrame: number,
    newDurationFrames: number
  ) => {
    if (!project) return;
    const rate = project.settings.frame_rate;
    const item = project.tracks.flatMap((track) => track.items).find((candidate) => candidate.item_id === itemId);
    if (!item) return;
    const sourceRate = item.source_range.duration.rate;
    const sourceDuration = timelineFramesToSourceFrames(
      newDurationFrames,
      rate,
      sourceRate,
      item.speed
    );
    const sourceStartDelta = timelineFramesToSourceFrames(
      newStartFrame - item.timeline_range.start.value,
      rate,
      item.source_range.start.rate,
      item.speed
    );
    if (sourceDuration === null || sourceStartDelta === null) {
      setStatusMsg("This trim cannot be represented exactly at the selected clip speed.");
      return;
    }
    const sourceStart = item.source_range.start.value + sourceStartDelta;
    executeCommandEnvelope(
      [
        {
          operation_id: `op_trim_${Date.now()}`,
          operation: {
            type: "ClipTrim",
            payload: {
              item_id: itemId,
              new_timeline_range: {
                start: { value: newStartFrame, rate },
                duration: { value: newDurationFrames, rate },
              },
              new_source_range: {
                start: { value: sourceStart, rate: item.source_range.start.rate },
                duration: { value: sourceDuration, rate: sourceRate },
              },
            },
          },
        },
      ],
      "Trim Clip"
    );
  };

  const handleSlipClip = (itemId: string, deltaFrames: number) => {
    if (!project || deltaFrames === 0) return;
    const track = project.tracks.find((candidate) => candidate.items.some((item) => item.item_id === itemId));
    const item = track?.items.find((candidate) => candidate.item_id === itemId);
    if (!track || !item || !item.asset_id || item.kind === "Text") return;
    if (track.locked) {
      setStatusMsg("Cannot slip a clip on a locked track.");
      return;
    }
    const asset = project.assets.find((candidate) => candidate.asset_id === item.asset_id);
    try {
      const sourceRange = calculateSlipSourceRange(item, asset, deltaFrames, project.settings.frame_rate);
      void executeCommandEnvelope(
        [
          {
            operation_id: `op_slip_${itemId}_${Date.now()}`,
            operation: {
              type: "ClipSlip",
              payload: {
                item_id: itemId,
                new_source_start: sourceRange.start,
              },
            },
          },
        ],
        "Slip Clip"
      ).catch(() => {});
    } catch (error) {
      setStatusMsg(error instanceof Error ? error.message : String(error));
    }
  };

  const handleRollClipBoundary = (leftItemId: string, rightItemId: string, deltaFrames: number) => {
    if (!project || deltaFrames === 0) return;
    try {
      getTimelineRollPlan(project, leftItemId, rightItemId, deltaFrames);
      void executeCommandEnvelope((currentProject) => {
        getTimelineRollPlan(currentProject, leftItemId, rightItemId, deltaFrames);
        return [{
          operation_id: `op_roll_${leftItemId}_${rightItemId}_${Date.now()}`,
          operation: {
            type: "ClipRoll",
            payload: {
              left_item_id: leftItemId,
              right_item_id: rightItemId,
              delta: { value: deltaFrames, rate: currentProject.settings.frame_rate },
            },
          },
        }];
      }, "Roll Edit").catch(() => {});
    } catch (error) {
      setStatusMsg(error instanceof Error ? error.message : String(error));
    }
  };

  const handleTrimTailToPlayhead = (itemId: string, newDuration: number) => {
    if (!project) return;
    for (const track of project.tracks) {
      const it = track.items.find((i) => i.item_id === itemId);
      if (it) {
        handleTrimClip(itemId, it.timeline_range.start.value, newDuration);
        break;
      }
    }
  };

  const handleSplitClip = (itemId: string, splitFrame: number) => {
    if (!project) return;
    const rate = project.settings.frame_rate;
    executeCommandEnvelope(
      [
        {
          operation_id: `op_split_${Date.now()}`,
          operation: {
            type: "ClipSplit",
            payload: {
              item_id: itemId,
              split_time: { value: splitFrame, rate },
            },
          },
        },
      ],
      "Split Clip"
    );
  };

  const handleDeleteSelected = (itemIds: string[]) => {
    if (!project || itemIds.length === 0) return;
    // P16-TP-27: Multi-delete uses one envelope with ordinary ClipDelete operations
    const operations = itemIds.map((id, idx) => ({
      operation_id: `op_del_${Date.now()}_${idx}`,
      operation: {
        type: "ClipDelete" as const,
        payload: {
          item_id: id,
        },
      },
    }));
    executeCommandEnvelope(operations, `Delete ${itemIds.length} Clip(s)`);
    setSelection((prev) => {
      const newSet = new Set(prev.selectedItemIds);
      itemIds.forEach((id) => newSet.delete(id));
      let newPrimary: string | null = null;
      if (prev.primarySelectedItemId && newSet.has(prev.primarySelectedItemId)) {
        newPrimary = prev.primarySelectedItemId;
      } else if (newSet.size > 0) {
        newPrimary = Array.from(newSet)[0];
      }
      return { selectedItemIds: newSet, primarySelectedItemId: newPrimary };
    });
  };

  const handleDeleteClip = (itemId: string) => {
    if (selection.selectedItemIds.size > 1 && selection.selectedItemIds.has(itemId)) {
      handleDeleteSelected(Array.from(selection.selectedItemIds));
      return;
    }
    executeCommandEnvelope(
      [
        {
          operation_id: `op_del_${Date.now()}`,
          operation: {
            type: "ClipDelete",
            payload: {
              item_id: itemId,
            },
          },
        },
      ],
      "Delete Clip"
    );
    setSelection((prev) => {
      const newSet = new Set(prev.selectedItemIds);
      newSet.delete(itemId);
      const remaining = Array.from(newSet);
      return {
        selectedItemIds: newSet,
        primarySelectedItemId: remaining.length > 0 ? remaining[0] : null,
      };
    });
  };

  const handleRippleDeletePrimary = (itemId: string) => {
    if (!project) return;
    executeCommandEnvelope(
      [
        {
          operation_id: `op_ripple_del_${Date.now()}`,
          operation: {
            type: "ClipRippleDelete",
            payload: {
              item_id: itemId,
            },
          },
        },
      ],
      "Ripple Delete Clip"
    );
    setSelection((prev) => {
      const newSet = new Set(prev.selectedItemIds);
      newSet.delete(itemId);
      const remaining = Array.from(newSet);
      return {
        selectedItemIds: newSet,
        primarySelectedItemId: remaining.length > 0 ? remaining[0] : null,
      };
    });
  };

  const handleCloseGap = async () => {
    if (!project || !gapClosePlan) return;
    try {
      await executeCommandEnvelope([
        {
          operation_id: `op_close_gap_${Date.now()}`,
          operation: {
            type: "ClipGroupMove",
            payload: {
              item_ids: gapClosePlan.itemIds,
              delta: { value: -gapClosePlan.gapFrames, rate: project.settings.frame_rate },
            },
          },
        },
      ], `Close ${gapClosePlan.gapFrames}-frame gap`);
      setSelection({
        selectedItemIds: new Set(gapClosePlan.itemIds),
        primarySelectedItemId: gapClosePlan.itemIds[0] ?? null,
      });
    } catch (error) {
      setStatusMsg(`Could not close gap: ${error}`);
    }
  };

  const handleSelectItem = (itemId: string | null, isCtrl?: boolean) => {
    if (!itemId) {
      setSelection(clearSelection());
      setSelectedTransitionId(null);
      return;
    }
    setSelectedTransitionId(null);
    if (isCtrl) {
      setSelection((prev) => toggleSelect(prev, itemId));
    } else {
      setSelection((prev) => {
        if (prev.selectedItemIds.has(itemId)) {
          return { ...prev, primarySelectedItemId: itemId };
        } else {
          return selectSingle(itemId);
        }
      });
    }
  };

  const handleDuplicateClip = (itemId: string) => {
    executeCommandEnvelope(
      [
        {
          operation_id: `op_dup_${Date.now()}`,
          operation: {
            type: "ClipDuplicate",
            payload: {
              item_id: itemId,
            },
          },
        },
      ],
      "Duplicate Clip"
    );
  };

  const handleCopySelectedClips = () => {
    const clipboard = captureTimelineClipboard(project, selection.selectedItemIds);
    if (!clipboard) return;
    timelineClipboardRef.current = clipboard;
    setStatusMsg(`Copied ${clipboard.itemIds.length} clip(s)`);
  };

  const handlePasteClips = async () => {
    const clipboard = timelineClipboardRef.current;
    if (!clipboard) {
      setStatusMsg("Copy one or more clips in this project before pasting.");
      return;
    }

    let pastedItemIds: string[] = [];
    try {
      await executeCommandEnvelope((currentProject) => {
        const operations = buildTimelinePasteOperations(currentProject, clipboard, playheadFrame);
        pastedItemIds = operations.map((entry) => {
          if (entry.operation.type !== "ClipDuplicate") return "";
          return entry.operation.payload.new_item_id ?? "";
        }).filter(Boolean);
        return operations;
      }, `Paste ${clipboard.itemIds.length} clip(s)`);
      if (pastedItemIds.length > 0) {
        setSelection({ selectedItemIds: new Set(pastedItemIds), primarySelectedItemId: pastedItemIds[0] });
      }
    } catch (error) {
      setStatusMsg(`Paste failed: ${error}`);
    }
  };

  const handleUpdateTransform = (x: number, y: number, scale: number, rotation: number) => {
    if (!selectedItemId) return;
    executeCommandEnvelope(
      [
        {
          operation_id: `op_tx_${Date.now()}`,
          operation: {
            type: "ClipTransformSet",
            payload: {
              item_id: selectedItemId,
              transform: {
                position_x: x,
                position_y: y,
                scale_x: scale,
                scale_y: scale,
                rotation,
              },
            },
          },
        },
      ],
      "Set Transform"
    );
  };

  const handleUpdateCrop = (crop: Crop) => {
    if (!selectedItemId) return;
    handleUpdateCanvasCrop(selectedItemId, crop);
  };

  const handleUpdateCanvasCrop = (itemId: string, crop: Crop) => {
    executeCommandEnvelope(
      [
        {
          operation_id: `op_crop_${itemId}_${Date.now()}`,
          operation: { type: "ClipCropSet", payload: { item_id: itemId, crop } },
        },
      ],
      "Set Crop"
    );
  };

  const handleUpdateCanvasPosition = (itemId: string, x: number, y: number) => {
    if (!project) return;
    const item = project.tracks.flatMap((track) => track.items).find((candidate) => candidate.item_id === itemId);
    if (!item) return;
    const rate = project.settings.frame_rate;
    const offsetFrame = playheadFrame - rationalTimeToFrames(item.timeline_range.start, rate);
    const operations: OperationEnvelope[] = [
      {
        operation_id: `op_canvas_position_${itemId}_${Date.now()}`,
        operation: {
          type: "ClipTransformSet",
          payload: {
            item_id: itemId,
            transform: { ...item.transform, position_x: x, position_y: y },
          },
        },
      },
    ];
    for (const [property, value] of [["PositionX", x], ["PositionY", y]] as const) {
      const propertyIsAnimated = (item.keyframes ?? []).some(
        (keyframe) => keyframe.property === property
      );
      if (propertyIsAnimated) {
        operations.push({
          operation_id: `op_canvas_keyframe_${property}_${itemId}_${Date.now()}`,
          operation: {
            type: "ClipKeyframeAdd",
            payload: {
              item_id: itemId,
              keyframe: { property, offset: { value: offsetFrame, rate }, value },
            },
          },
        });
      }
    }
    executeCommandEnvelope(operations, "Move Canvas Item");
  };

  const handleUpdateCanvasTransform = (itemId: string, transform: Transform) => {
    if (!project) return;
    const item = project.tracks.flatMap((track) => track.items).find((candidate) => candidate.item_id === itemId);
    if (!item) return;
    const rate = project.settings.frame_rate;
    const offsetFrame = playheadFrame - rationalTimeToFrames(item.timeline_range.start, rate);
    const operations: OperationEnvelope[] = [{
      operation_id: `op_canvas_transform_${itemId}_${Date.now()}`,
      operation: { type: "ClipTransformSet", payload: { item_id: itemId, transform } },
    }];
    const channels = [
      ["PositionX", transform.position_x],
      ["PositionY", transform.position_y],
      ["ScaleX", transform.scale_x],
      ["ScaleY", transform.scale_y],
      ["Rotation", transform.rotation],
    ] as const;
    for (const [property, value] of channels) {
      if ((item.keyframes ?? []).some((keyframe) => keyframe.property === property)) {
        operations.push({
          operation_id: `op_canvas_keyframe_${property}_${itemId}_${Date.now()}`,
          operation: {
            type: "ClipKeyframeAdd",
            payload: { item_id: itemId, keyframe: { property, offset: { value: offsetFrame, rate }, value } },
          },
        });
      }
    }
    executeCommandEnvelope(operations, "Transform Canvas Item");
  };

  const handleUpdatePlacementMode = (placementMode: PlacementMode) => {
    if (!selectedItemId) return;
    executeCommandEnvelope(
      [
        {
          operation_id: `op_placement_${selectedItemId}_${Date.now()}`,
          operation: {
            type: "ClipPlacementModeSet",
            payload: { item_id: selectedItemId, placement_mode: placementMode },
          },
        },
      ],
      "Frame placement"
    );
  };

  const handleUpdateOpacity = (opacity: number) => {
    if (!selectedItemId) return;
    executeCommandEnvelope(
      [
        {
          operation_id: `op_op_${Date.now()}`,
          operation: {
            type: "ClipOpacitySet",
            payload: {
              item_id: selectedItemId,
              opacity,
            },
          },
        },
      ],
      "Set Opacity"
    );
  };

  const handleUpdateText = (text: string) => {
    if (!selectedItemId) return;
    executeCommandEnvelope(
      [
        {
          operation_id: `op_txt_${Date.now()}`,
          operation: {
            type: "ClipTextSet",
            payload: {
              item_id: selectedItemId,
              text,
            },
          },
        },
      ],
      "Set Text Caption"
    );
  };

  const handleApplyTemplateDefinition = async (definition: TemplateDefinition) => {
    if (!project) return;
    const rate = project.settings.frame_rate;
    const now = Date.now();

    if (selectedItem?.kind === "Text") {
      await executeCommandEnvelope(
        [createTemplateApplyOp(selectedItem.item_id, definition.id, definition.version)],
        `Apply template "${definition.name}"`
      );
    } else {
      const operations: OperationEnvelope[] = [];
      const duration = definition.default_duration ?? { value: 90, rate };
      const targetTrack = findVideoTrackForRange(project, playheadFrame, duration.value);
      const trackId = targetTrack?.track_id ?? `trk_video_${now}`;

      if (!targetTrack) {
        operations.push({
          operation_id: `op_add_trk_${now}`,
          operation: {
            type: "TrackCreate",
            payload: {
              track_id: trackId,
              name: "Video Track 1",
              kind: "Video",
            },
          },
        });
      }

      const newItemId = `txt_${now}`;

      operations.push({
        operation_id: `op_add_txt_clip_${now}`,
        operation: {
          type: "ClipCreate",
          payload: {
            track_id: trackId,
            item_id: newItemId,
            name: definition.name,
            kind: "Text",
            timeline_start: { value: playheadFrame, rate },
            duration,
          },
        },
      });

      operations.push(
        createTemplateApplyOp(newItemId, definition.id, definition.version)
      );

      await executeCommandEnvelope(operations, `Insert template "${definition.name}"`);
      setSelection(selectSingle(newItemId));
    }
  };

  const handleTemplateParameterSet = (parameterKey: string, value: TemplateParameterValue) => {
    if (!selectedItemId) return;
    return executeCommandEnvelope(
      [createTemplateParameterSetOp(selectedItemId, parameterKey, value)],
      `Set parameter "${parameterKey}"`
    );
  };

  const handleTemplateReset = () => {
    if (!selectedItemId) return;
    if (selectedItem?.template_instance) {
      executeCommandEnvelope(
        [createTemplateResetOp(selectedItemId)],
        "Reset template"
      );
    } else {
      executeCommandEnvelope(
        [
          {
            operation_id: `op_tpl_reset_${Date.now()}`,
            operation: {
              type: "ClipTextTemplateReset",
              payload: {
                item_id: selectedItemId,
              },
            },
          },
        ],
        "Reset text template"
      );
    }
  };

  const handleTemplateDetach = () => {
    if (!selectedItemId) return;
    executeCommandEnvelope(
      [createTemplateDetachOp(selectedItemId)],
      "Detach template"
    );
  };

  const handleTemplateLayerAdd = (layer: TemplateLayer) => {
    if (!selectedItemId) return;
    executeCommandEnvelope(
      [createTemplateLayerAddOp(selectedItemId, layer)],
      `Add layer "${layer.name}"`
    );
  };

  const handleTemplateLayerRemove = (layerId: string) => {
    if (!selectedItemId) return;
    executeCommandEnvelope(
      [createTemplateLayerRemoveOp(selectedItemId, layerId)],
      "Remove template layer"
    );
  };

  const handleTemplateLayerReorder = (layerIds: string[]) => {
    if (!selectedItemId) return;
    executeCommandEnvelope(
      [createTemplateLayerReorderOp(selectedItemId, layerIds)],
      "Reorder template layers"
    );
  };

  const handleTemplateLayerUpdate = (layerId: string, patch: Partial<TemplateLayer>) => {
    if (!selectedItemId) return Promise.reject(new Error("No selected item"));
    return executeCommandEnvelope((currentProject) => {
      const item = currentProject.tracks
        .flatMap((track) => track.items)
        .find((candidate) => candidate.item_id === selectedItemId);
      const canonical = item?.template_instance?.layers.find(
        (layer) => layer.layer_id === layerId
      );
      if (!canonical) {
        throw new Error("Template layer '" + layerId + "' was not found in the current project");
      }
      const merged: TemplateLayer = {
        ...canonical,
        ...patch,
        transform: patch.transform
          ? { ...canonical.transform, ...patch.transform }
          : canonical.transform,
        kind: patch.kind
          ? patch.kind.type === canonical.kind.type
            ? ({
                type: canonical.kind.type,
                data: {
                  ...canonical.kind.data,
                  ...patch.kind.data,
                  ...(canonical.kind.type === "Text" &&
                  patch.kind.type === "Text" &&
                  patch.kind.data.text_style
                    ? {
                        text_style: {
                          font_family: "sans-serif",
                          font_size: 28,
                          font_weight: 400,
                          font_color: "#FFFFFF",
                          accent_color: null,
                          background_color: null,
                          background_opacity: 0,
                          padding_x: 0,
                          padding_y: 0,
                          alignment: "Center",
                          line_height: 1.2,
                          anchor: "Center",
                          ...(((canonical.kind.data.is_primary
                            ? item?.text_style
                            : canonical.kind.data.text_style) ?? {}) as Partial<TextStyle>),
                          ...(patch.kind.data.text_style as Partial<TextStyle>),
                        },
                      }
                    : {}),
                },
              } as TemplateLayer["kind"])
            : patch.kind
          : canonical.kind,
      };
      return [createTemplateLayerUpdateOp(selectedItemId, layerId, merged)];
    }, "Update template layer");
  };

  const handleApplyTextTemplate = (templateId: string, templateVersion = 1) => {
    if (!selectedItemId) return;
    const isLegacy = isLegacyTemplateId(templateId);
    const op = isLegacy
      ? {
          operation_id: `op_tpl_${Date.now()}`,
          operation: {
            type: "TextTemplateApply" as const,
            payload: {
              item_id: selectedItemId,
              template_id: templateId,
              template_version: templateVersion,
            },
          },
        }
      : createTemplateApplyOp(selectedItemId, templateId, templateVersion);

    return executeCommandEnvelope([op], `Apply text template "${templateId}"`);
  };

  const handleOpenAutoCaptions = () => {
    if (!selectedItem || (selectedItem.kind !== "Video" && selectedItem.kind !== "Audio")) {
      setStatusMsg("Select one video or audio clip before generating captions.");
      return;
    }
    if (autoCaptionReport && !["queued", "running"].includes(autoCaptionReport.state)) {
      setAutoCaptionReport(null);
    }
    setIsAutoCaptionOpen(true);
  };

  const handleStartAutoCaptions = async (settings: {
    whisperCliPath: string;
    modelPath: string;
    language: string;
    analyzeOnly: boolean;
  }) => {
    if (!project || !selectedItem?.asset_id || !selectedItemAsset) {
      setStatusMsg("Select one video or audio clip before generating captions.");
      return;
    }
    setAutoCaptionReportSource({
      projectId: project.project_id,
      assetId: selectedItemAsset.asset_id,
      assetName: selectedItemAsset.name,
      sourcePath: selectedItemAsset.path,
      durationSeconds: selectedItemAsset.duration ? rationalTimeToSeconds(selectedItemAsset.duration) : undefined,
    });
    try {
      const report = await editorApi.startAutoCaptions({
        itemId: selectedItem.item_id,
        whisperCliPath: settings.whisperCliPath,
        modelPath: settings.modelPath,
        language: settings.language,
        analyzeOnly: settings.analyzeOnly,
      });
      setAutoCaptionReport(report);
      setStatusMsg(report.message);
    } catch (err) {
      setStatusMsg(`Auto captions failed to start: ${err}`);
    }
  };

  const handleCancelAutoCaptions = async () => {
    if (!autoCaptionReport) return;
    try {
      await editorApi.cancelAutoCaptions(autoCaptionReport.jobId);
      const report = await editorApi.getAutoCaptionStatus(autoCaptionReport.jobId);
      setAutoCaptionReport(report);
      setStatusMsg(report.message);
    } catch (err) {
      setStatusMsg(`Could not cancel auto captions: ${err}`);
    }
  };

  const handleApplyTextTemplateToTrack = (
    trackId: string,
    templateId: string,
    templateVersion = 1
  ) => {
    const isLegacy = isLegacyTemplateId(templateId);
    const op = isLegacy
      ? createTextTrackTemplateApplyOp(trackId, templateId, templateVersion)
      : createTemplateTrackApplyOp(trackId, templateId, templateVersion);

    return executeCommandEnvelope([op], `Apply template "${templateId}" to text track`);
  };

  const handleResetTextTemplate = () => {
    if (!selectedItemId) return;
    executeCommandEnvelope(
      [
        {
          operation_id: `op_tpl_reset_${Date.now()}`,
          operation: {
            type: "ClipTextTemplateReset",
            payload: {
              item_id: selectedItemId,
            },
          },
        },
      ],
      "Reset text template"
    );
  };

  const handleUpdateTextStyle = (style: TextStyle) => {
    if (!selectedItemId) return;
    executeCommandEnvelope(
      [
        {
          operation_id: `op_txt_style_${Date.now()}`,
          operation: {
            type: "ClipTextStyleSet",
            payload: {
              item_id: selectedItemId,
              style,
            },
          },
        },
      ],
      "Update text style"
    );
  };

  const handleUpdateTextAppearance = (
    fill: import("./types/project").TextFill,
    strokes: import("./types/project").TextStroke[],
    shadows: import("./types/project").TextShadow[],
    background?: import("./types/project").TextBackgroundBox | null
  ) => {
    if (!selectedItemId) return;
    return executeCommandEnvelope(
      [
        {
          operation_id: `op_txt_appear_${Date.now()}`,
          operation: {
            type: "ClipTextAppearanceSet",
            payload: {
              item_id: selectedItemId,
              fill,
              strokes,
              shadows,
              background: background ?? null,
            },
          },
        },
      ],
      "Update text appearance"
    );
  };

  const handleUpdateTextAnimation = (
    animation?: import("./types/project").TextAnimation | null
  ) => {
    if (!selectedItemId) return;
    return executeCommandEnvelope(
      [
        {
          operation_id: `op_txt_anim_${Date.now()}`,
          operation: {
            type: "ClipTextAnimationSet",
            payload: {
              item_id: selectedItemId,
              animation: animation ?? null,
            },
          },
        },
      ],
      "Update text animation"
    );
  };

  const handleUpdateTextStyleForTrack = (trackId: string, style: TextStyle) => {
    executeCommandEnvelope(
      [
        {
          operation_id: `op_track_txt_style_${trackId}_${Date.now()}`,
          operation: {
            type: "TextTrackStyleSet",
            payload: {
              track_id: trackId,
              style,
            },
          },
        },
      ],
      "Apply text style to text track"
    );
  };

  const handleUpdateAudioFade = (fadeInFrames: number, fadeOutFrames: number) => {
    if (!selectedItemId || !project) return;
    const rate = project.settings.frame_rate;
    executeCommandEnvelope(
      [
        {
          operation_id: `op_fade_${Date.now()}`,
          operation: {
            type: "ClipAudioFadeSet",
            payload: {
              item_id: selectedItemId,
              fade_in: { value: fadeInFrames, rate },
              fade_out: { value: fadeOutFrames, rate },
            },
          },
        },
      ],
      "Set Audio Fades"
    );
  };

  const handleUpdateTransition = (
    kind: TransitionKind | null,
    durationFrames: number
  ) => {
    if (!selectedItemId || !project) return;
    const rate = project.settings.frame_rate;
    executeCommandEnvelope(
      [
        {
          operation_id: `op_trans_${Date.now()}`,
          operation: {
            type: "ClipTransitionSet",
            payload: {
              item_id: selectedItemId,
              transition: kind
                ? {
                    kind,
                    duration: { value: durationFrames, rate },
                  }
                : null,
            },
          },
        },
      ],
      kind ? `Set Transition (${kind})` : "Remove Transition"
    );
  };

  const handleUpdateVisualEffect = (visualEffect: VisualEffectPreset) => {
    if (!selectedItemId) return;
    executeCommandEnvelope(
      [
        {
          operation_id: `op_visual_effect_${Date.now()}`,
          operation: {
            type: "ClipVisualEffectSet",
            payload: {
              item_id: selectedItemId,
              visual_effect: visualEffect,
            },
          },
        },
      ],
      `Set Visual Effect (${visualEffect})`
    );
  };

  const handleUpdateColorAdjustments = (colorAdjustments: ColorAdjustments) => {
    if (!selectedItemId) return;
    executeCommandEnvelope(
      [{
        operation_id: `op_color_adjustments_${Date.now()}`,
        operation: {
          type: "ClipColorAdjustmentsSet",
          payload: { item_id: selectedItemId, color_adjustments: colorAdjustments },
        },
      }],
      "Update SDR color adjustments"
    );
  };

  const handleUpdateSpeed = (speed: ClipSpeed) => {
    if (!selectedItemId) return;
    executeCommandEnvelope(
      [
        {
          operation_id: `op_speed_${Date.now()}`,
          operation: {
            type: "ClipSpeedSet",
            payload: {
              item_id: selectedItemId,
              speed,
            },
          },
        },
      ],
      `Set Speed (${speed})`
    );
  };

  const handleAddKeyframe = (property: KeyframeProperty, offsetFrame: number, value: number) => {
    if (!selectedItemId || !project) return;
    const rate = project.settings.frame_rate;
    executeCommandEnvelope(
      [
        {
          operation_id: `op_kf_add_${Date.now()}`,
          operation: {
            type: "ClipKeyframeAdd",
            payload: {
              item_id: selectedItemId,
              keyframe: {
                property,
                offset: { value: offsetFrame, rate },
                value,
              },
            },
          },
        },
      ],
      `Add Keyframe (${property})`
    );
  };

  const handleRemoveKeyframe = (property: KeyframeProperty, offsetFrame: number) => {
    if (!selectedItemId || !project) return;
    const rate = project.settings.frame_rate;
    executeCommandEnvelope(
      [
        {
          operation_id: `op_kf_rm_${Date.now()}`,
          operation: {
            type: "ClipKeyframeRemove",
            payload: {
              item_id: selectedItemId,
              property,
              offset: { value: offsetFrame, rate },
            },
          },
        },
      ],
      `Remove Keyframe (${property})`
    );
  };

  const handleMoveKeyframe = (
    itemId: string,
    property: KeyframeProperty,
    oldOffsetFrame: number,
    newOffsetFrame: number
  ) => {
    if (!project) return;
    const rate = project.settings.frame_rate;
    executeCommandEnvelope(
      [
        {
          operation_id: `op_kf_mv_${Date.now()}`,
          operation: {
            type: "ClipKeyframeMove",
            payload: {
              item_id: itemId,
              property,
              old_offset: { value: oldOffsetFrame, rate },
              new_offset: { value: newOffsetFrame, rate },
            },
          },
        },
      ],
      `Move Keyframe (${property})`
    );
  };

  const handleSetKeyframeEasing = (
    property: KeyframeProperty,
    offsetFrame: number,
    easing: KeyframeEasing
  ) => {
    if (!selectedItemId || !project) return;
    const rate = project.settings.frame_rate;
    executeCommandEnvelope(
      [
        {
          operation_id: `op_kf_ease_${Date.now()}`,
          operation: {
            type: "ClipKeyframeEasingSet",
            payload: {
              item_id: selectedItemId,
              property,
              offset: { value: offsetFrame, rate },
              easing,
            },
          },
        },
      ],
      `Set Keyframe Easing (${property})`
    );
  };

  const handleUpdateVolume = (volume: number) => {
    if (!selectedItemId) return;
    executeCommandEnvelope(
      [
        {
          operation_id: `op_vol_${Date.now()}`,
          operation: {
            type: "ClipVolumeSet",
            payload: {
              item_id: selectedItemId,
              volume,
            },
          },
        },
      ],
      "Update Volume"
    );
  };

  const handleSetEmbeddedAudioEnabled = (enabled: boolean) => {
    if (!selectedItemId) return;
    executeCommandEnvelope(
      [{
        operation_id: `op_embedded_audio_${Date.now()}`,
        operation: {
          type: "ClipEmbeddedAudioSet",
          payload: { item_id: selectedItemId, enabled },
        },
      }],
      enabled ? "Enable Embedded Audio" : "Disable Embedded Audio"
    );
  };

  const handleDetachEmbeddedAudio = () => {
    if (!selectedItemId) return;
    executeCommandEnvelope(
      [{
        operation_id: `op_detach_audio_${Date.now()}`,
        operation: {
          type: "ClipAudioDetach",
          payload: { item_id: selectedItemId },
        },
      }],
      "Detach Video Audio"
    );
  };

  const handleUnlinkEmbeddedAudio = () => {
    if (!selectedItem) return;
    const linkedItem = project?.tracks.flatMap((track) => track.items)
      .find((item) => item.item_id === selectedItem.linked_item_id);
    if (!linkedItem) return;
    const videoItem = selectedItem.kind === "Video" ? selectedItem : linkedItem;
    const audioItem = selectedItem.kind === "Audio" ? selectedItem : linkedItem;
    if (videoItem.kind !== "Video" || audioItem.kind !== "Audio") return;
    executeCommandEnvelope(
      [{
        operation_id: `op_unlink_audio_${Date.now()}`,
        operation: {
          type: "ClipLinkSet",
          payload: { video_item_id: videoItem.item_id, audio_item_id: audioItem.item_id, linked: false },
        },
      }],
      "Unlink Video Audio"
    );
  };

  const handleToggleTrackMute = (trackId: string, muted: boolean) => {
    executeCommandEnvelope(
      [
        {
          operation_id: `op_track_mute_${trackId}_${Date.now()}`,
          operation: {
            type: "TrackMuteSet",
            payload: {
              track_id: trackId,
              muted,
            },
          },
        },
      ],
      muted ? "Mute Track" : "Unmute Track"
    );
  };

  const handleToggleTrackSolo = (trackId: string, solo: boolean) => {
    executeCommandEnvelope(
      [{
        operation_id: `op_track_solo_${trackId}_${Date.now()}`,
        operation: { type: "TrackSoloSet", payload: { track_id: trackId, solo } },
      }],
      solo ? "Solo Track" : "Unsolo Track"
    );
  };

  const handleSetTrackVolume = (trackId: string, volume: number) => {
    executeCommandEnvelope([{
      operation_id: `op_track_volume_${trackId}_${Date.now()}`,
      operation: { type: "TrackVolumeSet", payload: { track_id: trackId, volume } },
    }], "Set Track Gain");
  };

  const handleToggleTrackLock = (trackId: string, locked: boolean) => {
    executeCommandEnvelope(
      [
        {
          operation_id: `op_track_lock_${trackId}_${Date.now()}`,
          operation: {
            type: "TrackLockSet",
            payload: { track_id: trackId, locked },
          },
        },
      ],
      locked ? "Lock Track" : "Unlock Track"
    );
  };

  const handleReorderTrack = (trackId: string, newIndex: number) => {
    executeCommandEnvelope([{
      operation_id: `op_track_reorder_${trackId}_${Date.now()}`,
      operation: { type: "TrackReorder", payload: { track_id: trackId, new_index: newIndex } },
    }], "Reorder Track");
  };

  const handleRenameTrack = (trackId: string, name: string) => {
    executeCommandEnvelope([{
      operation_id: `op_track_rename_${trackId}_${Date.now()}`,
      operation: { type: "TrackRename", payload: { track_id: trackId, name } },
    }], "Rename Track");
  };

  const handleAddMarker = (label: string, positionFrame: number) => {
    if (!project) return;
    void executeCommandEnvelope([{
      operation_id: `op_marker_add_${Date.now()}`,
      operation: {
        type: "ProjectMarkerSet",
        payload: { label, position: { value: positionFrame, rate: project.settings.frame_rate } },
      },
    }], "Add Marker");
  };

  const handleRenameMarker = (markerId: string, label: string) => {
    const marker = project?.markers?.find((candidate) => candidate.marker_id === markerId);
    if (!marker) return;
    void executeCommandEnvelope([{
      operation_id: `op_marker_rename_${markerId}_${Date.now()}`,
      operation: {
        type: "ProjectMarkerSet",
        payload: { marker_id: markerId, label, position: marker.position },
      },
    }], "Rename Marker");
  };

  const handleDeleteMarker = (markerId: string) => {
    void executeCommandEnvelope([{
      operation_id: `op_marker_delete_${markerId}_${Date.now()}`,
      operation: { type: "ProjectMarkerDelete", payload: { marker_id: markerId } },
    }], "Delete Marker");
  };

  const handleCreateCutTransition = (
    trackId: string,
    leftItemId: string,
    rightItemId: string,
    kind: "CrossDissolve" | "DipToBlack",
    durationFrames: number = 30
  ) => {
    if (!project) return;
    const rate = project.settings.frame_rate;
    executeCommandEnvelope(
      [
        {
          operation_id: `op_cut_trans_create_${Date.now()}`,
          operation: {
            type: "CutTransitionCreate",
            payload: {
              track_id: trackId,
              left_item_id: leftItemId,
              right_item_id: rightItemId,
              kind,
              duration: { value: durationFrames, rate },
            },
          },
        },
      ],
      `Create ${kind === "CrossDissolve" ? "Cross Dissolve" : "Dip to Black"}`
    );
  };

  const handleUpdateCutTransitionDuration = (
    transitionId: string,
    newDurationFrames: number
  ) => {
    if (!project) return;
    const rate = project.settings.frame_rate;
    executeCommandEnvelope(
      [
        {
          operation_id: `op_cut_trans_dur_${Date.now()}`,
          operation: {
            type: "CutTransitionUpdateDuration",
            payload: {
              transition_id: transitionId,
              new_duration: { value: newDurationFrames, rate },
            },
          },
        },
      ],
      "Update Cut Transition Duration"
    );
  };

  const handleRemoveCutTransition = (transitionId: string) => {
    if (!project) return;
    executeCommandEnvelope(
      [
        {
          operation_id: `op_cut_trans_rm_${Date.now()}`,
          operation: {
            type: "CutTransitionRemove",
            payload: {
              transition_id: transitionId,
            },
          },
        },
      ],
      "Remove Cut Transition"
    );
    setSelectedTransitionId(null);
  };

  const handleCreateAudioCrossfade = (
    trackId: string,
    leftItemId: string,
    rightItemId: string,
    durationFrames: number = 30
  ) => {
    if (!project) return;
    executeCommandEnvelope(
      [{
        operation_id: `op_audio_crossfade_create_${Date.now()}`,
        operation: {
          type: "AudioCrossfadeCreate",
          payload: {
            track_id: trackId,
            left_item_id: leftItemId,
            right_item_id: rightItemId,
            duration: { value: durationFrames, rate: project.settings.frame_rate },
          },
        },
      }],
      "Create Audio Crossfade"
    );
  };

  const handleUpdateAudioCrossfadeDuration = (transitionId: string, durationFrames: number) => {
    if (!project) return;
    executeCommandEnvelope(
      [{
        operation_id: `op_audio_crossfade_duration_${Date.now()}`,
        operation: {
          type: "AudioCrossfadeUpdateDuration",
          payload: {
            transition_id: transitionId,
            new_duration: { value: durationFrames, rate: project.settings.frame_rate },
          },
        },
      }],
      "Update Audio Crossfade"
    );
  };

  const handleRemoveAudioCrossfade = (transitionId: string) => {
    executeCommandEnvelope(
      [{
        operation_id: `op_audio_crossfade_remove_${Date.now()}`,
        operation: {
          type: "AudioCrossfadeRemove",
          payload: { transition_id: transitionId },
        },
      }],
      "Remove Audio Crossfade"
    );
    setSelectedTransitionId(null);
  };

  // Find currently selected cut transition
  let selectedTransition: CutTransition | null = null;
  let selectedAudioCrossfade: AudioCrossfade | null = null;
  if (project && selectedTransitionId) {
    for (const track of project.tracks) {
      const tr = track.transitions?.find((t) => t.transition_id === selectedTransitionId);
      if (tr) {
        selectedTransition = tr;
        break;
      }
      const audioTransition = track.audio_transitions?.find(
        (t) => t.transition_id === selectedTransitionId
      );
      if (audioTransition) {
        selectedAudioCrossfade = audioTransition;
        break;
      }
    }
  }

  // Find adjacent items for the selected item
  const adjacentInfo: {
    prevClip?: TimelineItem;
    nextClip?: TimelineItem;
    outgoingTransition?: CutTransition;
    incomingTransition?: CutTransition;
    trackId?: string;
  } = {};

  if (project && selectedItem) {
    for (const track of project.tracks) {
      const itIdx = track.items.findIndex((i) => i.item_id === selectedItem?.item_id);
      if (itIdx !== -1) {
        adjacentInfo.trackId = track.track_id;
        if (itIdx > 0) {
          const prev = track.items[itIdx - 1];
          if (isRangeAdjacent(prev.timeline_range, selectedItem.timeline_range)) {
            adjacentInfo.prevClip = prev;
            adjacentInfo.incomingTransition = track.transitions?.find(
              (t) => t.left_item_id === prev.item_id && t.right_item_id === selectedItem?.item_id
            );
          }
        }
        if (itIdx < track.items.length - 1) {
          const next = track.items[itIdx + 1];
          if (isRangeAdjacent(selectedItem.timeline_range, next.timeline_range)) {
            adjacentInfo.nextClip = next;
            adjacentInfo.outgoingTransition = track.transitions?.find(
              (t) => t.left_item_id === selectedItem?.item_id && t.right_item_id === next.item_id
            );
          }
        }
        break;
      }
    }
  }

  const selectedTextTrack =
    project && selectedItem?.kind === "Text" && adjacentInfo.trackId
      ? (() => {
          const track = project.tracks.find((candidate) => candidate.track_id === adjacentInfo.trackId);
          if (!track) return null;
          return {
            trackId: track.track_id,
            name: track.name,
            textItemCount: track.items.filter((item) => item.kind === "Text").length,
          };
        })()
      : null;

  useTimelineShortcuts({
    enabled: isInitialized,
    project,
    selectedItemIds: selection.selectedItemIds,
    primarySelectedItemId: selection.primarySelectedItemId,
    playheadFrame,
    onPlayPauseToggle: requestPlaybackToggle,
    onSplitClip: handleSplitClip,
    onDuplicatePrimary: handleDuplicateClip,
    onCopySelected: handleCopySelectedClips,
    onPasteSelected: handlePasteClips,
    onDeleteSelected: handleDeleteSelected,
    onRippleDeletePrimary: handleRippleDeletePrimary,
    onTrimHeadToPlayhead: (itemId, newStart, newDuration) => {
      handleTrimClip(itemId, newStart, newDuration);
    },
    onTrimTailToPlayhead: handleTrimTailToPlayhead,
    onZoomIn: () => setZoom((z) => Math.min(10, z + 0.5)),
    onZoomOut: () => setZoom((z) => Math.max(1, z - 0.5)),
    onToggleSnapping: () => setSnapEnabled((s) => !s),
  });

  if (!isInitialized) {
    return (
      <div
        className="editor-shell session-loading-shell"
        style={{
          display: "flex",
          flexDirection: "column",
          height: "100vh",
          alignItems: "center",
          justifyContent: "center",
          padding: "32px",
          background: "var(--bg-app)",
          color: "var(--text-main)",
        }}
      >
        {hydrationError ? (
          <div
            className="modal-dialog"
            role="alert"
            style={{
              maxWidth: "520px",
              width: "100%",
              padding: "24px",
              boxShadow: "0 12px 32px rgba(0, 0, 0, 0.4)",
            }}
          >
            <div className="modal-dialog-header" style={{ marginBottom: "16px" }}>
              <h2 style={{ fontSize: "16px", color: "var(--text-main)" }}>Could not open editor</h2>
              <p style={{ fontSize: "13px", color: "var(--text-muted)", marginTop: "4px" }}>
                TinixCut could not restore your active project session.
              </p>
            </div>
            <div
              className="dialog-error"
              style={{
                marginBottom: "20px",
                fontSize: "12px",
                wordBreak: "break-word",
                fontFamily: "monospace",
              }}
            >
              {hydrationError}
            </div>
            <div className="modal-dialog-actions" style={{ justifyContent: "flex-end" }}>
              <button
                type="button"
                className="btn-primary"
                onClick={() => void hydrateProjectSession()}
              >
                Retry
              </button>
            </div>
          </div>
        ) : (
          <div role="status" style={{ textAlign: "center", color: "var(--text-muted)" }}>
            <p style={{ fontSize: "14px" }}>Loading editor session…</p>
          </div>
        )}
      </div>
    );
  }

  return (
    <div className="editor-shell">
      <TopBar
        project={project}
        health={health}
        onNewProject={handleNewProject}
        onOpenProject={handleOpenProject}
        onOpenMediaCache={() => setIsMediaCacheOpen(true)}
        onSaveProject={() => { void handleSaveProject(); }}
        onExportProject={handleExportProject}
        onCancelExport={handleCancelExport}
        exportJob={activeExport}
        onUndo={handleUndo}
        onRedo={handleRedo}
        onAddTrack={handleAddTrack}
        onAddClip={handleAddClip}
        onAddText={handleAddText}
        onCanvasPresetChange={handleCanvasPresetChange}
        onCanvasBackgroundChange={handleCanvasBackgroundChange}
        exportPreset={exportPreset}
        onExportPresetChange={setExportPreset}
        onImportCaptions={handleImportCaptions}
        onExportCaptions={handleExportCaptions}
        onOpenCaptionEditor={() => setIsCaptionEditorOpen(true)}
        onAutoCaptions={handleOpenAutoCaptions}
        onCreateDraft={() => setIsEditPlanDraftOpen(true)}
        onCreateAudioMix={() => setIsAudioMixDraftOpen(true)}
        onCreateReorder={() => setIsEditPlanReorderOpen(true)}
        onCreateTrim={() => setIsEditPlanTrimOpen(true)}
        onCreateVariant={() => setIsProjectVariantOpen(true)}
        canTrimSelection={!!selectedItem && !!selectedItemTrack && !selectedItemTrack.locked && selectedItem.kind === "Video" && !!selectedItem.asset_id && !offlineMedia[selectedItem.asset_id]}
        onOpenTemplates={() => setActiveLeftTab("templates")}
      />

      <div className="main-workspace">
        <div
          className="left-sidebar-container"
          style={{
            display: "flex",
            flexDirection: "column",
            flex: "0 0 280px",
            height: "100%",
            borderRight: "1px solid var(--border-color)",
            backgroundColor: "var(--bg-panel)",
            minWidth: 0,
            overflow: "hidden",
          }}
        >
          <div
            className="left-panel-tab-bar"
            style={{
              display: "flex",
              borderBottom: "1px solid var(--border-color)",
              backgroundColor: "var(--bg-panel)",
              flexShrink: 0,
            }}
          >
            <button
              type="button"
              className={`left-tab-btn ${activeLeftTab === "media" ? "active" : ""}`}
              onClick={() => setActiveLeftTab("media")}
              style={{
                flex: 1,
                padding: "8px 12px",
                fontSize: "11px",
                fontWeight: 600,
                backgroundColor: activeLeftTab === "media" ? "var(--bg-surface)" : "transparent",
                color: activeLeftTab === "media" ? "var(--accent-primary)" : "var(--text-muted)",
                border: "none",
                borderBottom: activeLeftTab === "media" ? "2px solid var(--accent-primary)" : "2px solid transparent",
                cursor: "pointer",
              }}
            >
              🎬 Media
            </button>
            <button
              type="button"
              className={`left-tab-btn ${activeLeftTab === "templates" ? "active" : ""}`}
              onClick={() => setActiveLeftTab("templates")}
              style={{
                flex: 1,
                padding: "8px 12px",
                fontSize: "11px",
                fontWeight: 600,
                backgroundColor: activeLeftTab === "templates" ? "var(--bg-surface)" : "transparent",
                color: activeLeftTab === "templates" ? "var(--accent-primary)" : "var(--text-muted)",
                border: "none",
                borderBottom: activeLeftTab === "templates" ? "2px solid var(--accent-primary)" : "2px solid transparent",
                cursor: "pointer",
              }}
            >
              🎨 Templates
            </button>
          </div>

          <div style={{ flex: 1, minHeight: 0, display: "flex", flexDirection: "column", overflow: "hidden" }}>
            {activeLeftTab === "media" ? (
              <MediaPanel
                assets={project?.assets ?? []}
                offlineMedia={offlineMedia}
                onAddSampleAsset={handleImportMedia}
                onAddAssetToTimeline={handleAddAssetToTimeline}
                onInsertAssetAtPlayhead={handleInsertAssetAtPlayhead}
                onOverwriteAssetAtPlayhead={handleOverwriteAssetAtPlayhead}
                onRelinkAsset={handleRelinkAsset}
                onAnalyzeVisualScenes={setVisualSceneAsset}
                onAnalyzeAudioLevels={(asset) => editorApi.analyzeAudioLevels(asset.asset_id)}
                onViewAsset={setSourceViewerAsset}
                importProgress={mediaImportProgress}
                onCancelImport={handleCancelMediaImport}
                onRelinkMultiple={() => {
                  setBulkRelinkMappings({});
                  setIsBulkRelinkOpen(true);
                }}
                onCollectMedia={() => { void handleCollectProjectMedia(); }}
              />
            ) : (
              <TemplateBrowser
                selectedItem={selectedItem}
                assets={project?.assets ?? []}
                projectRevision={project?.revision ?? 0}
                onProjectChanged={refreshProject}
                onApplyTemplate={handleApplyTemplateDefinition}
                onStatusChange={setStatusMsg}
              />
            )}
          </div>
        </div>

        <section className="center-panel">
          <PreviewCanvas
            project={project}
            playheadFrame={playheadFrame}
            selectedItem={selectedItem}
            onUpdateCanvasPosition={handleUpdateCanvasPosition}
            onUpdateCanvasTransform={handleUpdateCanvasTransform}
            onUpdateCanvasCrop={handleUpdateCanvasCrop}
            onPlayheadChange={setPlayheadFrame}
            isPlaying={isPlaying}
            onTogglePlay={requestPlaybackToggle}
            onRegisterAudioUnlock={(unlock) => {
              audioUnlockRef.current = unlock;
            }}
            onPlaybackTrackPeaksChange={handlePlaybackTrackPeaksChange}
            previewProxyMode={previewProxyMode}
            mediaCacheGeneration={mediaCacheGeneration}
          />

          <Timeline
            project={project}
            selectedItemId={selectedItemId}
            selectedItemIds={selection.selectedItemIds}
            primarySelectedItemId={selection.primarySelectedItemId}
            selectedTransitionId={selectedTransitionId}
            playheadFrame={playheadFrame}
            snapEnabled={snapEnabled}
            onToggleSnapping={() => setSnapEnabled((s) => !s)}
            onSelectItem={handleSelectItem}
            onClearSelection={() => {
              setSelection(clearSelection());
              setSelectedTransitionId(null);
            }}
            onSelectTransition={(transId) => {
              setSelectedTransitionId(transId);
              if (transId) setSelection(clearSelection());
            }}
            onPlayheadChange={setPlayheadFrame}
            onMoveClip={handleMoveClip}
            onSlipClip={handleSlipClip}
            onRollClipBoundary={handleRollClipBoundary}
            onGroupMoveClip={handleGroupMoveClip}
            onTrimClip={handleTrimClip}
            onSplitClip={handleSplitClip}
            onDeleteClip={handleDeleteClip}
            onDuplicateClip={handleDuplicateClip}
            canCloseGap={Boolean(gapClosePlan)}
            onCloseGap={handleCloseGap}
            onMoveKeyframe={handleMoveKeyframe}
            onCreateCutTransition={handleCreateCutTransition}
            onCreateAudioCrossfade={handleCreateAudioCrossfade}
            onToggleTrackMute={handleToggleTrackMute}
            onToggleTrackSolo={handleToggleTrackSolo}
            onSetTrackVolume={handleSetTrackVolume}
            playbackTrackPeaks={playbackTrackPeaks}
            onToggleTrackLock={handleToggleTrackLock}
            onLockedTrackEdit={(item) => {
              const track = project?.tracks.find((candidate) => candidate.items.some((entry) => entry.item_id === item.item_id));
              setStatusMsg(`Cannot edit "${item.name}": ${track?.name ?? "its track"} is locked. Unlock the track to edit its clips.`);
            }}
            onReorderTrack={handleReorderTrack}
            onRenameTrack={handleRenameTrack}
            onAddMarker={handleAddMarker}
            onRenameMarker={handleRenameMarker}
            onDeleteMarker={handleDeleteMarker}
            zoom={zoom}
            onZoomIn={() => setZoom((z) => Math.min(10, z + 0.5))}
            onZoomOut={() => setZoom((z) => Math.max(1, z - 0.5))}
            onSetZoom={setZoom}
          />
        </section>

        <Inspector
          projectRevision={project?.revision ?? 0}
          displayFrameRate={project?.settings.frame_rate ?? { numerator: 30, denominator: 1 }}
          selectedItem={selectedItem}
          selectedTransition={selectedTransition}
          selectedAudioCrossfade={selectedAudioCrossfade}
          selectedCount={selection.selectedItemIds.size}
          adjacentInfo={adjacentInfo}
          assets={project?.assets ?? []}
          playheadFrame={playheadFrame}
          onUpdateTransform={handleUpdateTransform}
          onUpdateCanvasPosition={handleUpdateCanvasPosition}
          onUpdateCrop={handleUpdateCrop}
          onUpdatePlacementMode={handleUpdatePlacementMode}
          onUpdateOpacity={handleUpdateOpacity}
          onUpdateVolume={handleUpdateVolume}
          onSetEmbeddedAudioEnabled={handleSetEmbeddedAudioEnabled}
          onDetachEmbeddedAudio={handleDetachEmbeddedAudio}
          onUnlinkEmbeddedAudio={handleUnlinkEmbeddedAudio}
          onUpdateText={handleUpdateText}
          onApplyTextTemplate={handleApplyTextTemplate}
          onApplyTextTemplateToTrack={handleApplyTextTemplateToTrack}
          onResetTextTemplate={handleResetTextTemplate}
          onUpdateTextStyle={handleUpdateTextStyle}
          onApplyTextStyleToTrack={handleUpdateTextStyleForTrack}
          textTrack={selectedTextTrack}
          onUpdateAudioFade={handleUpdateAudioFade}
          onUpdateTransition={handleUpdateTransition}
          onUpdateVisualEffect={handleUpdateVisualEffect}
          onUpdateColorAdjustments={handleUpdateColorAdjustments}
          onUpdateSpeed={handleUpdateSpeed}
          onAddKeyframe={handleAddKeyframe}
          onRemoveKeyframe={handleRemoveKeyframe}
          onSetKeyframeEasing={handleSetKeyframeEasing}
          onSeekPlayhead={setPlayheadFrame}
          onCreateCutTransition={handleCreateCutTransition}
          onUpdateCutTransitionDuration={handleUpdateCutTransitionDuration}
          onRemoveCutTransition={handleRemoveCutTransition}
          onUpdateAudioCrossfadeDuration={handleUpdateAudioCrossfadeDuration}
          onRemoveAudioCrossfade={handleRemoveAudioCrossfade}
          onTemplateParameterSet={handleTemplateParameterSet}
          onTemplateReset={handleTemplateReset}
          onTemplateDetach={handleTemplateDetach}
          onTemplateLayerAdd={handleTemplateLayerAdd}
          onTemplateLayerRemove={handleTemplateLayerRemove}
          onTemplateLayerReorder={handleTemplateLayerReorder}
          onTemplateLayerUpdate={handleTemplateLayerUpdate}
          onSaveAsTemplate={() => setIsAuthoringOpen(true)}
          onUpdateTextAppearance={handleUpdateTextAppearance}
          onUpdateTextAnimation={handleUpdateTextAnimation}
        />
      </div>

      <footer className="status-bar">
        <span>
          Status: {statusMsg}
          {recoverySaveState === "saving" && " | Recovery: saving unsaved edits…"}
          {recoverySaveState === "saved" && ` | Recovery saved (rev ${recoverySaveRevision})`}
          {recoverySaveState === "failed" && ` | Recovery save failed: ${recoverySaveError}`}
        </span>
        {isProjectRevisionConflictStatus(statusMsg) && (
          <button
            type="button"
            className="btn-action status-save-copy"
            onClick={() => { void handleSaveProject(true); }}
            aria-label="Save a copy to preserve edits after a project revision conflict"
            title="Save your current edits to a new project file without replacing the newer disk version"
          >
            Save a Copy…
          </button>
        )}
        <span>
          Tracks: {project?.tracks.length ?? 0} | Playhead: {playheadFrame}f | Revision:{" "}
          {project?.revision ?? 0}
        </span>
      </footer>

      {unsavedRecoveries.length > 0 && !unsavedRecoveryDismissed && project && !recoveryPrompt && (
        <div className="dialog-backdrop">
          <section className="modal-dialog" role="alertdialog" aria-modal="true" aria-labelledby="unsaved-recovery-title">
            <header className="modal-dialog-header">
              <div>
                <h2 id="unsaved-recovery-title">Unsaved projects recovered</h2>
                <p>These local recovery copies were saved before the app closed unexpectedly. Restoring opens the copy as an editable project.</p>
              </div>
            </header>
            {unsavedRecoveries.map((candidate) => (
              <div className="dialog-review-summary" key={candidate.projectId}>
                <strong>{candidate.name}</strong>
                <div>Revision {candidate.revision} · Saved {new Date(candidate.updatedAtMs).toLocaleString()}</div>
                <div>
                  {candidate.trackCount} tracks · {candidate.clipCount} clips · {candidate.assetCount} media files
                </div>
                {candidate.previewItemNames.length > 0 && (
                  <small>Contents: {candidate.previewItemNames.join(" · ")}</small>
                )}
                <div className="modal-dialog-actions">
                  <button
                    type="button"
                    className="btn-primary"
                    disabled={project.revision !== 0 || project.assets.length > 0 || project.tracks.length > 0}
                    onClick={() => void handleRestoreUnsavedRecovery(candidate)}
                  >Restore project</button>
                  <button type="button" className="btn-action" onClick={() => void handleDiscardUnsavedRecovery(candidate)}>Discard recovery copy</button>
                </div>
              </div>
            ))}
            <div className="modal-dialog-actions">
              <button type="button" className="btn-action" onClick={() => setUnsavedRecoveryDismissed(true)}>Later</button>
            </div>
          </section>
        </div>
      )}

      {recoveryPrompt && (
        <div className="dialog-backdrop">
          <section
            className="modal-dialog"
            role="alertdialog"
            aria-modal="true"
            aria-labelledby="project-recovery-title"
            aria-describedby="project-recovery-description"
          >
            <header className="modal-dialog-header">
              <div>
                <h2 id="project-recovery-title">Unsaved recovery version found</h2>
                {recoveryPrompt.unreadableReason ? (
                  <p id="project-recovery-description">
                    The recovery file could not be read and has been preserved. You can still open the saved project.
                    <br />{recoveryPrompt.unreadableReason}
                  </p>
                ) : (
                  <p id="project-recovery-description">
                    A separate recovery copy at revision {recoveryPrompt.recoveryRevision} differs from the
                    saved project at revision {recoveryPrompt.projectRevision}. Recovering loads that copy into
                    the editor and leaves the saved project file unchanged.
                  </p>
                )}
              </div>
            </header>
            <div className="modal-dialog-actions">
              {!recoveryPrompt.unreadableReason && (
                <button type="button" className="btn-primary" onClick={handleRestoreRecovery}>
                  Recover unsaved edits
                </button>
              )}
              <button type="button" className="btn-action" onClick={handleOpenSavedVersion}>
                Open saved project
              </button>
              <button type="button" className="btn-action" onClick={() => setRecoveryPrompt(null)}>
                Cancel
              </button>
            </div>
          </section>
        </div>
      )}

      <AutoCaptionDialog
        isOpen={isAutoCaptionOpen}
        selectedClipName={selectedItem?.name ?? null}
        projectId={project?.project_id ?? null}
        reportSource={autoCaptionReportSource}
        availableTranscriptAssets={project?.assets ?? []}
        report={autoCaptionReport}
        onClose={() => setIsAutoCaptionOpen(false)}
        onStart={handleStartAutoCaptions}
        onCancel={handleCancelAutoCaptions}
        onSeekTranscript={setPlayheadFrame}
        onOpenSourceTranscript={(assetId, sourceSeconds) => {
          const asset = project?.assets.find((candidate) => candidate.asset_id === assetId) ?? null;
          if (!asset) {
            setStatusMsg("This transcript source is no longer available in the project.");
            return;
          }
          setSourceViewerSeekSeconds(sourceSeconds);
          setSourceViewerAsset(asset);
        }}
        onInsertTranscriptRange={(assetId, sourceInSeconds, sourceOutSeconds) => {
          const asset = project?.assets.find((candidate) => candidate.asset_id === assetId) ?? null;
          if (!asset || asset.media_kind === "Image" || sourceOutSeconds <= sourceInSeconds) {
            setStatusMsg("This transcript range has no valid audio or video source.");
            return;
          }
          handleAddAssetToTimeline(asset, sourceInSeconds, sourceOutSeconds, true);
        }}
        onBuildTranscriptCut={handleBuildTranscriptCut}
        frameRate={project?.settings.frame_rate ?? { numerator: 30, denominator: 1 }}
      />

      <MediaCacheDialog
        isOpen={isMediaCacheOpen}
        onClose={() => setIsMediaCacheOpen(false)}
        previewProxyMode={previewProxyMode}
        onCacheCleared={() => setMediaCacheGeneration((generation) => generation + 1)}
        onPreviewProxyModeChange={async (mode) => {
          const current = await editorApi.getAppSettings();
          const saved = await editorApi.saveAppSettings({ ...current, previewProxyMode: mode });
          setPreviewProxyMode(saved.previewProxyMode);
        }}
      />

      <CaptionEditorDialog
        project={project}
        playheadFrame={playheadFrame}
        isOpen={isCaptionEditorOpen}
        onClose={() => setIsCaptionEditorOpen(false)}
        onSeek={setPlayheadFrame}
        onApplyEdits={handleApplyCaptionEdits}
        onMerge={handleMergeCaptions}
        onSplit={handleSplitCaption}
      />

      <VisualSceneAnalysisDialog
        asset={visualSceneAsset}
        onClose={() => setVisualSceneAsset(null)}
        onCreateDraft={(report) => {
          setVisualSceneAsset(null);
          setSceneDraftReport(report);
        }}
      />

      <SourceViewerDialog
        asset={sourceViewerAsset}
        initialSeekSeconds={sourceViewerSeekSeconds}
        onClose={() => setSourceViewerAsset(null)}
        onAddRange={(asset, sourceIn, sourceOut) => {
          handleAddAssetToTimeline(asset, sourceIn, sourceOut);
          setSourceViewerAsset(null);
        }}
      />

      {isBulkRelinkOpen && (
        <BulkRelinkDialog
          assets={project?.assets ?? []}
          offlineAssetIds={Object.keys(offlineMedia)}
          mappings={bulkRelinkMappings}
          onChoose={(asset) => { void handleChooseBulkRelinkFile(asset).catch((error) => setStatusMsg(`Could not choose replacement: ${error}`)); }}
          onApply={() => { void handleApplyBulkRelink(); }}
          onClose={() => setIsBulkRelinkOpen(false)}
          applying={bulkRelinkApplying}
        />
      )}

      <SceneGuidedDraftDialog
        report={sceneDraftReport}
        project={project}
        onClose={() => setSceneDraftReport(null)}
        onApply={handleApplyEditPlanDraft}
      />

      <EditPlanDraftDialog
        isOpen={isEditPlanDraftOpen}
        project={project}
        offlineMedia={offlineMedia}
        onClose={() => setIsEditPlanDraftOpen(false)}
        onApply={handleApplyEditPlanDraft}
      />

      <AudioMixDraftDialog
        isOpen={isAudioMixDraftOpen}
        project={project}
        onClose={() => setIsAudioMixDraftOpen(false)}
        onApply={handleApplyEditPlanDraft}
      />

      <EditPlanReorderDialog
        isOpen={isEditPlanReorderOpen}
        project={project}
        offlineMedia={offlineMedia}
        selectedItemIds={Array.from(selection.selectedItemIds)}
        onClose={() => setIsEditPlanReorderOpen(false)}
        onApply={handleApplyEditPlanDraft}
      />

      <EditPlanTrimDialog
        isOpen={isEditPlanTrimOpen}
        project={project}
        item={selectedItem}
        offlineMedia={offlineMedia}
        onClose={() => setIsEditPlanTrimOpen(false)}
        onApply={handleApplyEditPlanDraft}
      />

      {isProjectVariantOpen && project && <ProjectVariantDialog
        project={project}
        onClose={() => setIsProjectVariantOpen(false)}
        onOpenVariant={handleOpenVariant}
      />}

      <TemplateAuthoringDialog
        isOpen={isAuthoringOpen}
        selectedItem={selectedItem}
        assets={project?.assets ?? []}
        onClose={() => setIsAuthoringOpen(false)}
        onSaved={(def) => {
          setStatusMsg(`Saved template "${def.name}" to catalog`);
          refreshProject();
        }}
      />
    </div>
  );
}

export default App;
