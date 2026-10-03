import React from "react";
import { Asset } from "../types/project";

interface MediaPanelProps {
  assets: Asset[];
  onAddSampleAsset: () => void;
  onAddAssetToTimeline?: (asset: Asset) => void;
}

export const MediaPanel: React.FC<MediaPanelProps> = ({
  assets,
  onAddSampleAsset,
  onAddAssetToTimeline,
}) => {
  return (
    <aside className="left-panel">
      <div className="panel-header" style={{ justifyContent: "space-between" }}>
        <span>Media Library</span>
        <button
          onClick={onAddSampleAsset}
          style={{
            background: "none",
            border: "none",
            color: "var(--accent-primary)",
            cursor: "pointer",
            fontSize: "11px",
            fontWeight: 600,
          }}
        >
          + Import
        </button>
      </div>
      <div className="panel-content">
        {assets.length === 0 ? (
          <div style={{ textAlign: "center", marginTop: "24px" }}>
            <p>No media imported.</p>
            <p style={{ fontSize: "11px", color: "var(--text-muted)" }}>
              Import local footage or fixtures.
            </p>
          </div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
            {assets.map((a) => (
              <div
                key={a.asset_id}
                style={{
                  padding: "8px",
                  backgroundColor: "var(--bg-surface)",
                  borderRadius: "4px",
                  border: "1px solid var(--border-color)",
                  display: "flex",
                  flexDirection: "column",
                  gap: "4px",
                }}
              >
                <div style={{ fontWeight: 600, fontSize: "12px", color: "var(--text-main)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                  🎬 {a.name}
                </div>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginTop: "4px" }}>
                  <span style={{ fontSize: "10px", color: "var(--text-muted)" }}>
                    {a.media_kind} • {(a.file_size / (1024 * 1024)).toFixed(1)} MB
                  </span>
                  {onAddAssetToTimeline && (
                    <button
                      className="btn-action"
                      onClick={() => onAddAssetToTimeline(a)}
                      title={`Thêm ${a.name} vào Timeline để biên tập`}
                      style={{
                        padding: "2px 8px",
                        fontSize: "10px",
                        fontWeight: 600,
                        backgroundColor: "var(--accent-primary)",
                        color: "#fff",
                        border: "none",
                        borderRadius: "3px",
                        cursor: "pointer",
                      }}
                    >
                      + Timeline
                    </button>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </aside>
  );
};
