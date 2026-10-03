import { useEffect, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { HealthStatus } from "./types/health";
import { Asset, Project, TimelineItem } from "./types/project";
import { CommandEnvelope } from "./types/command";
import { editorApi } from "./services/editorApi";
import { TopBar } from "./components/TopBar";
import { MediaPanel } from "./components/MediaPanel";
import { PreviewCanvas } from "./components/PreviewCanvas";
import { Timeline } from "./components/Timeline";
import { Inspector } from "./components/Inspector";
import "./App.css";

export function App() {
  const [project, setProject] = useState<Project | null>(null);
  const [savedFilePath, setSavedFilePath] = useState<string | null>(null);
  const [health, setHealth] = useState<HealthStatus | null>(null);
  const [selectedItemId, setSelectedItemId] = useState<string | null>(null);
  const [playheadFrame, setPlayheadFrame] = useState<number>(0);
  const [statusMsg, setStatusMsg] = useState<string>("Ready");

  const refreshProject = async () => {
    try {
      const p = await editorApi.getCurrentProject();
      setProject(p);
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
      setSelectedItemId(null);
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
      setSelectedItemId(null);
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

  const handleTrimClip = (itemId: string, newStartFrame: number, newDurationFrames: number) => {
    if (!project) return;
    const rate = project.settings.frame_rate;
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
                start: { value: 0, rate },
                duration: { value: newDurationFrames, rate },
              },
            },
          },
        },
      ],
      "Trim Clip"
    );
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

  const handleDeleteClip = (itemId: string) => {
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
    setSelectedItemId(null);
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
          />

          <Timeline
            project={project}
            selectedItemId={selectedItemId}
            playheadFrame={playheadFrame}
            onSelectItem={setSelectedItemId}
            onPlayheadChange={setPlayheadFrame}
            onMoveClip={handleMoveClip}
            onTrimClip={handleTrimClip}
            onSplitClip={handleSplitClip}
            onDeleteClip={handleDeleteClip}
            onDuplicateClip={handleDuplicateClip}
          />
        </section>

        <Inspector
          selectedItem={selectedItem}
          assets={project?.assets ?? []}
          onUpdateTransform={handleUpdateTransform}
          onUpdateOpacity={handleUpdateOpacity}
          onUpdateText={handleUpdateText}
          onUpdateAudioFade={handleUpdateAudioFade}
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
