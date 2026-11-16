import React from "react";
import type { ExportResult } from "./exportResult";
import { EXPORT_PRESET_OPTIONS, ExportPreset, LOUDNESS_PRESET_OPTIONS } from "../types/export";
import { HealthStatus } from "../types/health";
import { Project } from "../types/project";

interface TopBarProps {
  project: Project | null;
  health: HealthStatus | null;
  onNewProject: () => void;
  onOpenProject: () => void;
  onOpenMediaCache: () => void;
  onSaveProject: () => void;
  onExportProject: () => void;
  onCancelExport: () => void;
  exportJob: { progress: number; cancelling: boolean } | null;
  exportResult?: ExportResult | null;
  onDismissExportResult?: () => void;
  onImportCaptions: () => void;
  onExportCaptions: () => void;
  onOpenCaptionEditor: () => void;
  onAutoCaptions: () => void;
  onCreateDraft: () => void;
  onCreateAudioMix: () => void;
  onCreateReorder: () => void;
  onCreateTrim: () => void;
  onCreateVariant: () => void;
  canTrimSelection: boolean;
  onUndo: () => void;
  onRedo: () => void;
  onAddTrack: () => void;
  onAddClip: () => void;
  onAddText: () => void;
  onOpenTemplates?: () => void;
  onCanvasPresetChange: (width: number, height: number, label: string) => void;
  onCanvasBackgroundChange: (color: string) => void;
  exportPreset: ExportPreset;
  onExportPresetChange: (preset: ExportPreset) => void;
  loudnessPreset: string;
  onLoudnessPresetChange: (value: string) => void;
  hasInOutRange: boolean;
  exportInOutOnly: boolean;
  onExportInOutOnlyChange: (value: boolean) => void;
  loopInOut: boolean;
  onLoopInOutChange: (value: boolean) => void;
  onRippleDeleteInOut: () => void;
  onOpenShortcuts: () => void;
  shuttleLabel: string;
}

const canvasPresets = [
  { value: "1920x1080", width: 1920, height: 1080, label: "16:9 Landscape" },
  { value: "1080x1920", width: 1080, height: 1920, label: "9:16 Vertical" },
  { value: "1080x1080", width: 1080, height: 1080, label: "1:1 Square" },
  { value: "1080x1350", width: 1080, height: 1350, label: "4:5 Portrait" },
];

export const TopBar: React.FC<TopBarProps> = ({
  project,
  health,
  onNewProject,
  onOpenProject,
  onOpenMediaCache,
  onSaveProject,
  onExportProject,
  onCancelExport,
  exportJob,
  exportResult,
  onDismissExportResult,
  onImportCaptions,
  onExportCaptions,
  onOpenCaptionEditor,
  onAutoCaptions,
  onCreateDraft,
  onCreateAudioMix,
  onCreateReorder,
  onCreateTrim,
  onCreateVariant,
  canTrimSelection,
  onUndo,
  onRedo,
  onAddTrack,
  onAddClip,
  onAddText,
  onOpenTemplates,
  onCanvasPresetChange,
  onCanvasBackgroundChange,
  exportPreset,
  onExportPresetChange,
  loudnessPreset,
  onLoudnessPresetChange,
  hasInOutRange,
  exportInOutOnly,
  onExportInOutOnlyChange,
  loopInOut,
  onLoopInOutChange,
  onRippleDeleteInOut,
  onOpenShortcuts,
  shuttleLabel,
}) => {
  const currentCanvasValue = project ? `${project.settings.width}x${project.settings.height}` : "";
  const usesKnownPreset = canvasPresets.some((preset) => preset.value === currentCanvasValue);

  return (
    <header className="top-bar">
      <div className="brand">
        <span style={{ color: "var(--accent-primary)", fontSize: "16px" }}>▶</span>
        <span style={{ fontWeight: 700 }}>TinixCut</span>
        {health && (
          <span className="badge badge-healthy" title={`Engine v${health.version} (${health.platform})`}>
            ● Core Healthy (v{health.version})
          </span>
        )}
        {project && (
          <span style={{ color: "var(--text-muted)", fontSize: "12px", marginLeft: "8px" }}>
            {project.name} (rev {project.revision})
          </span>
        )}
        <label className="canvas-preset-control">
          <span>Canvas</span>
          <select
            aria-label="Canvas aspect ratio"
            className="canvas-preset-select"
            disabled={!project || exportJob !== null}
            value={currentCanvasValue}
            onChange={(event) => {
              const preset = canvasPresets.find((candidate) => candidate.value === event.target.value);
              if (preset) {
                onCanvasPresetChange(preset.width, preset.height, preset.label);
              }
            }}
          >
            {project && !usesKnownPreset && (
              <option value={currentCanvasValue}>
                Custom ({project.settings.width} × {project.settings.height})
              </option>
            )}
            {canvasPresets.map((preset) => (
              <option key={preset.value} value={preset.value}>
                {preset.label}
              </option>
            ))}
          </select>
        </label>
        <label className="canvas-preset-control">
          <span>BG</span>
          <input
            aria-label="Canvas background color"
            className="canvas-background-input"
            type="color"
            disabled={!project}
            value={project?.settings.canvas_background_color ?? "#000000"}
            onChange={(event) => onCanvasBackgroundChange(event.target.value)}
            title="Canvas background color"
          />
        </label>
      </div>

      <div className="top-actions">
        <button className="btn-action" onClick={onNewProject} title="Create New Project">
          + New
        </button>
        <button className="btn-action" onClick={onOpenProject} title="Open Existing Project">
          📂 Open
        </button>
        <button className="btn-action" onClick={onOpenMediaCache} title="View and clear rebuildable media cache">Cache</button>
        <button className="btn-action" onClick={onCreateVariant} disabled={!project} title="Create an isolated editable copy">Variant</button>
        <button className="btn-action" onClick={onAddTrack} title="Add Video Track">
          + Track
        </button>
        <button className="btn-action" onClick={onAddClip} title="Add Sample Clip to Track">
          + Clip
        </button>
        <button className="btn-action" onClick={onAddText} title="Add a text item with the default template">
          + Text
        </button>
        {onOpenTemplates && (
          <button
            className="btn-action"
            onClick={onOpenTemplates}
            title="Browse and apply animated text templates"
            style={{ color: "var(--accent-primary)", fontWeight: 600 }}
          >
            🎨 Templates
          </button>
        )}
        <button className="btn-action" onClick={onImportCaptions} title="Import SRT or WebVTT captions">
          💬 Import captions
        </button>
        <button className="btn-action" onClick={onExportCaptions} title="Export text items as SRT or WebVTT captions">
          📝 Export captions
        </button>
        <button className="btn-action" onClick={onOpenCaptionEditor} disabled={!project} title="Review, search and edit caption clips">
          ✍ Caption editor
        </button>
        <button
          className="btn-action"
          onClick={onAutoCaptions}
          disabled={!project}
          title="Generate text captions from the selected audio or video clip"
        >
          ✨ Auto captions
        </button>
        <button className="btn-action" onClick={onCreateDraft} disabled={!project} title="Create a reviewable video draft from selected media">
          ✨ Draft
        </button>
        <button className="btn-action" onClick={onCreateAudioMix} disabled={!project} title="Review normalization and voice ducking for timeline audio">
          Audio mix
        </button>
        <button className="btn-action" onClick={onCreateReorder} disabled={!project} title="Preview and reorder clips on a video track">
          ↕ Reorder
        </button>
        <button className="btn-action" onClick={onCreateTrim} disabled={!canTrimSelection} title="Preview a tail trim for the selected video clip">
          ✂ Shorten
        </button>
        <button className="btn-action" onClick={onSaveProject} title="Save Project (Atomic)">
          💾 Save
        </button>
        <label className="export-preset-control">
          <span>Output</span>
          <select
            aria-label="Export quality"
            className="export-preset-select"
            disabled={!project}
            value={exportPreset}
            onChange={(event) => onExportPresetChange(event.target.value as ExportPreset)}
            title="Export quality. Canvas and frame rate always follow the project."
          >
            {EXPORT_PRESET_OPTIONS.map((preset) => (
              <option key={preset.value} value={preset.value} title={preset.description}>
                {preset.label}
              </option>
            ))}
          </select>
          <select
            aria-label="Export loudness"
            className="export-preset-select"
            disabled={!project}
            value={loudnessPreset}
            onChange={(event) => onLoudnessPresetChange(event.target.value)}
            title="Normalize the exported audio to a loudness target. Off leaves the mix unchanged."
          >
            {LOUDNESS_PRESET_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>
        <label style={{ display: "flex", gap: 4, alignItems: "center", fontSize: 11 }} title="Export only the range between the In and Out points (set with I and O)">
          <input
            type="checkbox"
            aria-label="Export In-Out range only"
            checked={exportInOutOnly}
            disabled={!project}
            onChange={(event) => onExportInOutOnlyChange(event.currentTarget.checked)}
          />
          In–Out
        </label>
        <label style={{ display: "flex", gap: 4, alignItems: "center", fontSize: 11 }} title="Loop playback between the In and Out points">
          <input
            type="checkbox"
            aria-label="Loop In-Out playback"
            checked={loopInOut}
            disabled={!hasInOutRange}
            onChange={(event) => onLoopInOutChange(event.currentTarget.checked)}
          />
          Loop
        </label>
        <button
          className="btn-action"
          onClick={onRippleDeleteInOut}
          disabled={!project || !hasInOutRange}
          title="Remove the In–Out range from every unlocked track and close the gap"
        >
          Delete In–Out
        </button>
        <button className="btn-action" onClick={onOpenShortcuts} title="View and change keyboard shortcuts">
          Keys
        </button>
        {shuttleLabel && <span aria-label="Shuttle speed" style={{ fontSize: 11 }}>{shuttleLabel}</span>}
        <button
          className="btn-action"
          onClick={onExportProject}
          disabled={exportJob !== null}
          title="Export Project to MP4 (FFmpeg)"
          style={{ backgroundColor: "var(--accent-primary)", color: "#fff", fontWeight: 600 }}
        >
          🎬 Export
        </button>
        {exportJob && (
          <div className="export-job-status" role="status" aria-live="polite">
            <div className="export-job-status-line">
              <span>Exporting video</span>
              <span>{Math.round(exportJob.progress * 100)}%</span>
            </div>
            <progress aria-label="Export progress" max={1} value={exportJob.progress} />
            <button
              className="btn-action"
              onClick={onCancelExport}
              disabled={exportJob.cancelling}
            >
              {exportJob.cancelling ? "Cancelling…" : "Cancel export"}
            </button>
          </div>
        )}
        {exportResult && <div role="status" className="export-job-status">
          <strong>Export {exportResult.status}</strong>
          <div title={exportResult.outputPath} style={{overflowWrap:"anywhere"}}>{exportResult.outputPath}</div>
          <div>{EXPORT_PRESET_OPTIONS.find(option=>option.value===exportResult.preset)?.label ?? exportResult.preset} · {exportResult.range ? `Frames ${exportResult.range.start_frame}–${exportResult.range.end_frame} (end exclusive)` : "Full timeline"}</div>
          {exportResult.status === "failed" && <div>{exportResult.error || "Unknown export failure"}</div>}
          <button type="button" className="btn-action" onClick={onDismissExportResult}>Dismiss export result</button>
        </div>}
        <button className="btn-action" onClick={onUndo} title="Undo committed edit">
          ↩ Undo
        </button>
        <button className="btn-action" onClick={onRedo} title="Redo committed edit">
          ↪ Redo
        </button>
        <span style={{ fontSize: "11px", color: "var(--text-muted)", marginLeft: "8px" }}>
          Schema v{project?.schema_version ?? 1}
        </span>
      </div>
    </header>
  );
};
