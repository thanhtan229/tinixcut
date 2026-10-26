import { useEffect, useRef, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { HealthStatus } from "./types/health";
import { EXPORT_PRESET_OPTIONS, ExportPreset } from "./types/export";
import { Asset, AudioCrossfade, ClipSpeed, CutTransition, FrameRate, isRangeAdjacent, KeyframeEasing, KeyframeProperty, PlacementMode, Project, rationalTimeToFrames, TextStyle, TimelineItem, TransitionKind, VisualEffectPreset } from "./types/project";
import { CommandEnvelope, OperationEnvelope } from "./types/command";
import { editorApi } from "./services/editorApi";
import { TopBar } from "./components/TopBar";
import { AutoCaptionDialog } from "./components/AutoCaptionDialog";
import { MediaPanel } from "./components/MediaPanel";
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

export function App() {
  const [project, setProject] = useState<Project | null>(null);
  const [savedFilePath, setSavedFilePath] = useState<string | null>(null);
  const [health, setHealth] = useState<HealthStatus | null>(null);
  const [selection, setSelection] = useState<SelectionState>(createInitialSelection());
  const selectedItemId = selection.primarySelectedItemId;
  const [selectedTransitionId, setSelectedTransitionId] = useState<string | null>(null);
  const [playheadFrame, setPlayheadFrame] = useState<number>(0);
  const [snapEnabled, setSnapEnabled] = useState<boolean>(true);
  const [isPlaying, setIsPlaying] = useState<boolean>(false);
  const [zoom, setZoom] = useState<number>(3);
  const [statusMsg, setStatusMsg] = useState<string>("Ready");
  const [exportPreset, setExportPreset] = useState<ExportPreset>("balanced");
  const [isAutoCaptionOpen, setIsAutoCaptionOpen] = useState(false);
  const [activeLeftTab, setActiveLeftTab] = useState<"media" | "templates">("media");
  const [isAuthoringOpen, setIsAuthoringOpen] = useState(false);
  const [autoCaptionReport, setAutoCaptionReport] = useState<Awaited<ReturnType<typeof editorApi.startAutoCaptions>> | null>(null);
  const audioUnlockRef = useRef<() => void>(() => {});
  const selectedItem = findTimelineItem(project, selectedItemId);
  const requestPlaybackToggle = () => {
    if (!isPlaying) audioUnlockRef.current();
    setIsPlaying((playing) => !playing);
  };

  const refreshProject = async () => {
    try {
      const p = await editorApi.getCurrentProject();
      setProject(p);
      setSelection((prev) => reconcileSelection(prev, p));
    } catch (err) {
      console.warn("Could not get current project:", err);
    }
  };

  useEffect(() => {
    editorApi.getHealth().then(setHealth).catch(console.warn);
    refreshProject();
  }, []);

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
        setStatusMsg(`${desc} committed (rev ${receipt.new_revision})`);
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

  const handleNewProject = async () => {
    try {
      const p = await editorApi.newProject("Untitled Video Project");
      setProject(p);
      setSavedFilePath(null);
      setSelection(clearSelection());
      setSelectedTransitionId(null);
      setPlayheadFrame(0);
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
      const p = await editorApi.openProject(chosenPath);
      setProject(p);
      setSavedFilePath(chosenPath);
      setSelection(clearSelection());
      setSelectedTransitionId(null);
      setPlayheadFrame(0);
      setStatusMsg(`Loaded project: ${p.name}`);
    } catch (err) {
      setStatusMsg(`Open error: ${err}`);
    }
  };

  const handleSaveProject = async () => {
    try {
      let targetPath = savedFilePath;
      if (!targetPath) {
        const chosen = await save({
          title: "Save TinixCut Project",
          defaultPath: `${project?.name || "project"}.tinix`,
          filters: [{ name: "TinixCut Project", extensions: ["tinix", "tcproj"] }],
        });
        if (!chosen) {
          setStatusMsg("Save cancelled");
          return;
        }
        targetPath = chosen;
      }

      const path = await editorApi.saveProject(targetPath);
      setSavedFilePath(path);
      setStatusMsg(`Project saved atomically to ${path}`);
    } catch (err) {
      setStatusMsg(`Save error: ${err}`);
    }
  };

  const handleExportProject = async () => {
    try {
      if (!project) return;
      const chosen = await save({
        title: "Export Project to MP4",
        defaultPath: `${project.name || "video"}.mp4`,
        filters: [{ name: "MP4 Video", extensions: ["mp4"] }],
      });
      if (!chosen) {
        setStatusMsg("Export cancelled");
        return;
      }
      const preset = EXPORT_PRESET_OPTIONS.find((option) => option.value === exportPreset);
      setStatusMsg(`Rendering ${preset?.label ?? "Balanced"} snapshot via FFmpeg (isolated snapshot)...`);
      const report = await editorApi.exportProject(chosen, exportPreset);
      setStatusMsg(
        `Export complete! ${report.output_path} (${report.duration_secs.toFixed(1)}s in ${report.render_time_ms}ms)`
      );
    } catch (err) {
      setStatusMsg(`Export error: ${err}`);
    }
  };

  const handleImportMedia = async () => {
    try {
      const selected = await open({
        title: "Import Media Files",
        filters: [
          {
            name: "Media Files",
            extensions: ["mp4", "mov", "avi", "mkv", "mp3", "wav", "aac", "png", "jpg", "jpeg", "webp"],
          },
        ],
        multiple: true,
      });
      if (!selected) return;
      const paths = Array.isArray(selected) ? selected : [selected];
      for (const p of paths) {
        await editorApi.importMedia(p);
      }
      setStatusMsg(`Imported ${paths.length} media file(s)`);
      await refreshProject();
    } catch (err) {
      setStatusMsg(`Import failed: ${err}`);
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

  const handleUndo = async () => {
    try {
      const desc = await editorApi.undo();
      if (desc) {
        setStatusMsg(`Undid: ${desc}`);
        await refreshProject();
      } else {
        setStatusMsg("Nothing to undo");
      }
    } catch (err) {
      setStatusMsg(`Undo failed: ${err}`);
    }
  };

  const handleRedo = async () => {
    try {
      const desc = await editorApi.redo();
      if (desc) {
        setStatusMsg(`Redid: ${desc}`);
        await refreshProject();
      } else {
        setStatusMsg("Nothing to redo");
      }
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
    const track = project.tracks.find((candidate) => candidate.kind === "Video");
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
            duration: { value: 150, rate },
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

  const handleAddAssetToTimeline = (asset: Asset) => {
    if (!project) return;
    const rate = project.settings.frame_rate;
    const isAudio = asset.media_kind === "Audio";
    const durationFrames = asset.duration ? Math.max(1, rationalTimeToFrames(asset.duration, rate)) : 150;

    // If no tracks exist, create a track first along with the clip
    if (project.tracks.length === 0) {
      const newTrackId = `trk_${Date.now()}`;
      executeCommandEnvelope(
        [
          {
            operation_id: `op_add_trk_${Date.now()}`,
            operation: {
              type: "TrackCreate",
              payload: {
                track_id: newTrackId,
                name: isAudio ? "Audio Track 1" : "Video Track 1",
                kind: isAudio ? "Audio" : "Video",
              },
            },
          },
          {
            operation_id: `op_add_clip_${Date.now() + 1}`,
            operation: {
              type: "ClipCreate",
              payload: {
                track_id: newTrackId,
                name: asset.name,
                kind: isAudio ? "Audio" : (asset.media_kind === "Image" ? "Image" : "Video"),
                asset_id: asset.asset_id,
                timeline_start: { value: 0, rate },
                duration: { value: durationFrames, rate },
                source_start: { value: 0, rate },
              },
            },
          },
        ],
        `Thêm "${asset.name}" vào Timeline`
      );
      return;
    }

    // Keep audio and video clips on compatible tracks. The command service remains
    // authoritative; both operations are committed atomically when a track is needed.
    const matchingTrack = project.tracks.find((t) => (isAudio ? t.kind === "Audio" : t.kind === "Video"));
    const targetTrack = matchingTrack ?? project.tracks[0];

    // Calculate start time to avoid overlap on this track
    let startF = 0;
    for (const it of targetTrack.items) {
      const endF = it.timeline_range.start.value + it.timeline_range.duration.value;
      if (endF > startF) startF = endF;
    }

    const now = Date.now();
    const newTrackId = matchingTrack ? targetTrack.track_id : `trk_${now}`;
    const operations: OperationEnvelope[] = [];
    if (!matchingTrack) {
      operations.push({
        operation_id: `op_add_trk_${now}`,
        operation: {
          type: "TrackCreate" as const,
          payload: {
            track_id: newTrackId,
            name: isAudio ? `Audio Track ${project.tracks.length + 1}` : `Video Track ${project.tracks.length + 1}`,
            kind: isAudio ? "Audio" as const : "Video" as const,
          },
        },
      });
    }
    operations.push({
      operation_id: `op_add_clip_${now + 1}`,
      operation: {
        type: "ClipCreate" as const,
        payload: {
          track_id: newTrackId,
          name: asset.name,
          kind: isAudio ? "Audio" : (asset.media_kind === "Image" ? "Image" : "Video"),
          asset_id: asset.asset_id,
          timeline_start: { value: startF, rate },
          duration: { value: durationFrames, rate },
          source_start: { value: 0, rate },
        },
      },
    });
    executeCommandEnvelope(operations, `Thêm "${asset.name}" vào Timeline`);
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
      let targetTrack = project.tracks.find((t) => t.kind === "Video");
      const operations: OperationEnvelope[] = [];

      let trackId = targetTrack?.track_id;
      if (!trackId) {
        trackId = `trk_video_${now}`;
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
      const duration = definition.default_duration ?? { value: 90, rate };

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
  }) => {
    if (!selectedItem) {
      setStatusMsg("Select one video or audio clip before generating captions.");
      return;
    }
    try {
      const report = await editorApi.startAutoCaptions({
        itemId: selectedItem.item_id,
        whisperCliPath: settings.whisperCliPath,
        modelPath: settings.modelPath,
        language: settings.language,
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
    project,
    selectedItemIds: selection.selectedItemIds,
    primarySelectedItemId: selection.primarySelectedItemId,
    playheadFrame,
    onPlayPauseToggle: requestPlaybackToggle,
    onSplitClip: handleSplitClip,
    onDuplicatePrimary: handleDuplicateClip,
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

  return (
    <div className="editor-shell">
      <TopBar
        project={project}
        health={health}
        onNewProject={handleNewProject}
        onOpenProject={handleOpenProject}
        onSaveProject={handleSaveProject}
        onExportProject={handleExportProject}
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
        onAutoCaptions={handleOpenAutoCaptions}
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
                onAddSampleAsset={handleImportMedia}
                onAddAssetToTimeline={handleAddAssetToTimeline}
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
            onPlayheadChange={setPlayheadFrame}
            isPlaying={isPlaying}
            onTogglePlay={requestPlaybackToggle}
            onRegisterAudioUnlock={(unlock) => {
              audioUnlockRef.current = unlock;
            }}
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
            onGroupMoveClip={handleGroupMoveClip}
            onTrimClip={handleTrimClip}
            onSplitClip={handleSplitClip}
            onDeleteClip={handleDeleteClip}
            onDuplicateClip={handleDuplicateClip}
            onMoveKeyframe={handleMoveKeyframe}
            onCreateCutTransition={handleCreateCutTransition}
            onCreateAudioCrossfade={handleCreateAudioCrossfade}
            onToggleTrackMute={handleToggleTrackMute}
            zoom={zoom}
            onZoomIn={() => setZoom((z) => Math.min(10, z + 0.5))}
            onZoomOut={() => setZoom((z) => Math.max(1, z - 0.5))}
            onSetZoom={setZoom}
          />
        </section>

        <Inspector
          selectedItem={selectedItem}
          selectedTransition={selectedTransition}
          selectedAudioCrossfade={selectedAudioCrossfade}
          selectedCount={selection.selectedItemIds.size}
          adjacentInfo={adjacentInfo}
          assets={project?.assets ?? []}
          playheadFrame={playheadFrame}
          onUpdateTransform={handleUpdateTransform}
          onUpdatePlacementMode={handleUpdatePlacementMode}
          onUpdateOpacity={handleUpdateOpacity}
          onUpdateVolume={handleUpdateVolume}
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
        <span>Status: {statusMsg}</span>
        <span>
          Tracks: {project?.tracks.length ?? 0} | Playhead: {playheadFrame}f | Revision:{" "}
          {project?.revision ?? 0}
        </span>
      </footer>

      <AutoCaptionDialog
        isOpen={isAutoCaptionOpen}
        selectedClipName={selectedItem?.name ?? null}
        report={autoCaptionReport}
        onClose={() => setIsAutoCaptionOpen(false)}
        onStart={handleStartAutoCaptions}
        onCancel={handleCancelAutoCaptions}
      />

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
