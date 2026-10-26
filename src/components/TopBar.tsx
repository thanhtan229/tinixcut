import React from "react";
import { EXPORT_PRESET_OPTIONS, ExportPreset } from "../types/export";
import { HealthStatus } from "../types/health";
import { Project } from "../types/project";

interface TopBarProps {
  project: Project | null;
  health: HealthStatus | null;
  onNewProject: () => void;
  onOpenProject: () => void;
  onSaveProject: () => void;
  onExportProject: () => void;
  onImportCaptions: () => void;
  onExportCaptions: () => void;
  onAutoCaptions: () => void;
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
  onSaveProject,
  onExportProject,
  onImportCaptions,
  onExportCaptions,
  onAutoCaptions,
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
            disabled={!project}
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
        <button
          className="btn-action"
          onClick={onAutoCaptions}
          disabled={!project}
          title="Generate text captions from the selected audio or video clip"
        >
          ✨ Auto captions
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
        </label>
        <button
          className="btn-action"
          onClick={onExportProject}
          title="Export Project to MP4 (FFmpeg)"
          style={{ backgroundColor: "var(--accent-primary)", color: "#fff", fontWeight: 600 }}
        >
          🎬 Export
        </button>
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
