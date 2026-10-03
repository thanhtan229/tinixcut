import React from "react";
import { HealthStatus } from "../types/health";
import { Project } from "../types/project";

interface TopBarProps {
  project: Project | null;
  health: HealthStatus | null;
  onNewProject: () => void;
  onOpenProject: () => void;
  onSaveProject: () => void;
  onExportProject: () => void;
  onUndo: () => void;
  onRedo: () => void;
  onAddTrack: () => void;
  onAddClip: () => void;
}

export const TopBar: React.FC<TopBarProps> = ({
  project,
  health,
  onNewProject,
  onOpenProject,
  onSaveProject,
  onExportProject,
  onUndo,
  onRedo,
  onAddTrack,
  onAddClip,
}) => {
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
        <button className="btn-action" onClick={onSaveProject} title="Save Project (Atomic)">
          💾 Save
        </button>
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
