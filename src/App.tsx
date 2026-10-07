import { useEffect, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { HealthStatus } from "./types/health";
import { Asset, AudioCrossfade, CutTransition, isRangeAdjacent, KeyframeEasing, KeyframeProperty, Project, TimelineItem, TransitionKind } from "./types/project";
import { CommandEnvelope } from "./types/command";
import { editorApi } from "./services/editorApi";
import { TopBar } from "./components/TopBar";
import { MediaPanel } from "./components/MediaPanel";
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

  const executeCommandEnvelope = async (operations: CommandEnvelope["operations"], desc: string) => {
    if (!project) return;
    try {
      const envelope: CommandEnvelope = {
        schema_version: project.schema_version,
        project_id: project.project_id,
        base_revision: project.revision,
        dry_run: false,
        transaction_id: `tx_${Date.now()}`,
        operations,
      };

      const receipt = await editorApi.executeCommand(envelope);
      setStatusMsg(`${desc} committed (rev ${receipt.new_revision})`);
      await refreshProject();
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setStatusMsg(`Command failed: ${msg}`);
      // Invariant: Reconciliation on command failure
      await refreshProject();
    }
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
      setStatusMsg("Rendering snapshot via FFmpeg (isolated snapshot)...");
      const report = await editorApi.exportProject(chosen);
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

  const handleAddAssetToTimeline = (asset: Asset) => {
    if (!project) return;
    const rate = project.settings.frame_rate;
    const isAudio = asset.media_kind === "Audio";

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
                duration: { value: 150, rate }, // 5 seconds default
                source_start: { value: 0, rate },
              },
            },
          },
        ],
        `Thêm "${asset.name}" vào Timeline`
      );
      return;
    }

    // Find suitable track: matching kind or first track
    const targetTrack =
      project.tracks.find((t) => (isAudio ? t.kind === "Audio" : t.kind === "Video")) ||
      project.tracks[0];

    // Calculate start time to avoid overlap on this track
    let startF = 0;
    for (const it of targetTrack.items) {
      const endF = it.timeline_range.start.value + it.timeline_range.duration.value;
      if (endF > startF) startF = endF;
    }

    executeCommandEnvelope(
      [
        {
          operation_id: `op_add_clip_${Date.now()}`,
          operation: {
            type: "ClipCreate",
            payload: {
              track_id: targetTrack.track_id,
              name: asset.name,
              kind: isAudio ? "Audio" : (asset.media_kind === "Image" ? "Image" : "Video"),
              asset_id: asset.asset_id,
              timeline_start: { value: startF, rate },
              duration: { value: 150, rate }, // 5 seconds default
              source_start: { value: 0, rate },
            },
          },
        },
      ],
      `Thêm "${asset.name}" vào Timeline`
    );
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
    newDurationFrames: number,
    newSourceStartFrame?: number
  ) => {
    if (!project) return;
    const rate = project.settings.frame_rate;
    let sourceStart = 0;
    for (const track of project.tracks) {
      const it = track.items.find((i) => i.item_id === itemId);
      if (it) {
        sourceStart = it.source_range.start.value;
        break;
      }
    }
    if (newSourceStartFrame !== undefined) {
      sourceStart = newSourceStartFrame;
    }
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
                start: { value: sourceStart, rate },
                duration: { value: newDurationFrames, rate },
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
        handleTrimClip(itemId, it.timeline_range.start.value, newDuration, it.source_range.start.value);
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

  const handleCreateCutTransition = (
    trackId: string,
    leftItemId: string,
    rightItemId: string,
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
              kind: "CrossDissolve",
              duration: { value: durationFrames, rate },
            },
          },
        },
      ],
      "Create CrossDissolve"
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
      "Update CrossDissolve Duration"
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
      "Remove CrossDissolve"
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

  // Find currently selected item
  let selectedItem: TimelineItem | null = null;
  if (project && selectedItemId) {
    for (const track of project.tracks) {
      const it = track.items.find((i) => i.item_id === selectedItemId);
      if (it) {
        selectedItem = it;
        break;
      }
    }
  }

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

  useTimelineShortcuts({
    project,
    selectedItemIds: selection.selectedItemIds,
    primarySelectedItemId: selection.primarySelectedItemId,
    playheadFrame,
    onPlayPauseToggle: () => setIsPlaying((p) => !p),
    onSplitClip: handleSplitClip,
    onDuplicatePrimary: handleDuplicateClip,
    onDeleteSelected: handleDeleteSelected,
    onRippleDeletePrimary: handleRippleDeletePrimary,
    onTrimHeadToPlayhead: (itemId, newStart, newDuration, newSourceStart) => {
      handleTrimClip(itemId, newStart, newDuration, newSourceStart);
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
      />

      <div className="main-workspace">
        <MediaPanel
          assets={project?.assets ?? []}
          onAddSampleAsset={handleImportMedia}
          onAddAssetToTimeline={handleAddAssetToTimeline}
        />

        <section className="center-panel">
          <PreviewCanvas
            project={project}
            playheadFrame={playheadFrame}
            selectedItem={selectedItem}
            onPlayheadChange={setPlayheadFrame}
            isPlaying={isPlaying}
            onTogglePlay={() => setIsPlaying((p) => !p)}
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
          onUpdateOpacity={handleUpdateOpacity}
          onUpdateVolume={handleUpdateVolume}
          onUpdateText={handleUpdateText}
          onUpdateAudioFade={handleUpdateAudioFade}
          onUpdateTransition={handleUpdateTransition}
          onAddKeyframe={handleAddKeyframe}
          onRemoveKeyframe={handleRemoveKeyframe}
          onSetKeyframeEasing={handleSetKeyframeEasing}
          onSeekPlayhead={setPlayheadFrame}
          onCreateCutTransition={handleCreateCutTransition}
          onUpdateCutTransitionDuration={handleUpdateCutTransitionDuration}
          onRemoveCutTransition={handleRemoveCutTransition}
          onUpdateAudioCrossfadeDuration={handleUpdateAudioCrossfadeDuration}
          onRemoveAudioCrossfade={handleRemoveAudioCrossfade}
        />
      </div>

      <footer className="status-bar">
        <span>Status: {statusMsg}</span>
        <span>
          Tracks: {project?.tracks.length ?? 0} | Playhead: {playheadFrame}f | Revision:{" "}
          {project?.revision ?? 0}
        </span>
      </footer>
    </div>
  );
}

export default App;
