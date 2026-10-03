import React, { useState, useEffect, useRef } from "react";
import { Asset, TimelineItem } from "../types/project";

interface InspectorProps {
  selectedItem: TimelineItem | null;
  assets?: Asset[];
  onUpdateTransform: (x: number, y: number, scale: number, rotation: number) => void;
  onUpdateOpacity: (opacity: number) => void;
  onUpdateText?: (text: string) => void;
  onUpdateAudioFade?: (fadeInFrames: number, fadeOutFrames: number) => void;
}

export const Inspector: React.FC<InspectorProps> = ({
  selectedItem,
  assets,
  onUpdateTransform,
  onUpdateOpacity,
  onUpdateText,
  onUpdateAudioFade,
}) => {
  if (!selectedItem) {
    return (
      <aside className="right-panel">
        <div className="panel-header">Inspector</div>
        <div className="panel-content" style={{ textAlign: "center", marginTop: "24px" }}>
          <p>No clip selected.</p>
          <p style={{ fontSize: "11px", color: "var(--text-muted)" }}>
            Select a clip on the timeline to inspect and edit transform, crop, opacity, text, and audio.
          </p>
        </div>
      </aside>
    );
  }

  const { transform, opacity, timeline_range, source_range } = selectedItem;
  const [localText, setLocalText] = useState(selectedItem.text_content ?? "");
  const debounceTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Synchronize local state when selected item or backend text changes
  useEffect(() => {
    setLocalText(selectedItem.text_content ?? "");
  }, [selectedItem.item_id, selectedItem.text_content]);

  const handleTextChange = (val: string) => {
    setLocalText(val);
    if (debounceTimerRef.current) {
      clearTimeout(debounceTimerRef.current as any);
    }
    debounceTimerRef.current = setTimeout(() => {
      if (onUpdateText) {
        onUpdateText(val);
      }
    }, 400);
  };

  const handleTextCommit = () => {
    if (debounceTimerRef.current) {
      clearTimeout(debounceTimerRef.current as any);
    }
    if (onUpdateText && localText !== (selectedItem.text_content ?? "")) {
      onUpdateText(localText);
    }
  };

  return (
    <aside className="right-panel">
      <div className="panel-header">Inspector: {selectedItem.name}</div>
      <div className="panel-content" style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
        {/* Clip Summary */}
        <section>
          <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)", marginBottom: "4px" }}>
            CLIP PROPERTIES
          </div>
          <div style={{ fontSize: "12px", color: "var(--text-main)", display: "flex", flexDirection: "column", gap: "2px" }}>
            <div>ID: <span style={{ fontFamily: "monospace" }}>{selectedItem.item_id}</span></div>
            <div>Kind: <span style={{ fontWeight: 600 }}>{selectedItem.kind}</span></div>
            <div>
              Media:{" "}
              {selectedItem.asset_id ? (
                <span style={{ color: "var(--accent-primary)", fontWeight: 600 }}>
                  🎬 {assets?.find((a) => a.asset_id === selectedItem.asset_id)?.name || selectedItem.asset_id}
                </span>
              ) : (
                <span style={{ color: "var(--text-muted)" }}>🎨 Sample Placeholder</span>
              )}
            </div>
            <div>Start: {timeline_range.start.value}f | Dur: {timeline_range.duration.value}f</div>
            <div>Source Start: {source_range.start.value}f</div>
          </div>
        </section>

        {/* Text Caption Section with immediate local state & commit */}
        {onUpdateText && (
          <section style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <span style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)" }}>
                TEXT CAPTION
              </span>
              {localText !== (selectedItem.text_content ?? "") ? (
                <span style={{ fontSize: "10px", color: "var(--accent-warning)" }}>● Unsaved</span>
              ) : selectedItem.text_content ? (
                <span style={{ fontSize: "10px", color: "var(--accent-success)" }}>✓ Active</span>
              ) : null}
            </div>
            <div style={{ display: "flex", gap: "6px" }}>
              <input
                type="text"
                placeholder="Nhập chữ phụ đề (caption)..."
                value={localText}
                onChange={(e) => handleTextChange(e.target.value)}
                onBlur={handleTextCommit}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    handleTextCommit();
                  }
                }}
                style={{
                  flex: 1,
                  padding: "6px 8px",
                  fontSize: "12px",
                  backgroundColor: "var(--bg-surface)",
                  border: "1px solid var(--border-color)",
                  borderRadius: "4px",
                  color: "var(--text-main)",
                }}
              />
              <button
                className="btn-action"
                onClick={handleTextCommit}
                title="Apply text caption immediately (Enter)"
                style={{ padding: "4px 8px", fontSize: "11px", whiteSpace: "nowrap" }}
              >
                ✓ Apply
              </button>
            </div>
          </section>
        )}

        {/* Transform controls */}
        <section style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
          <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)" }}>
            TRANSFORM (BOUNDED DOMAIN)
          </div>

          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
            <label style={{ fontSize: "12px" }}>Position X:</label>
            <input
              type="number"
              value={transform.position_x}
              onChange={(e) =>
                onUpdateTransform(
                  parseFloat(e.target.value) || 0,
                  transform.position_y,
                  transform.scale_x,
                  transform.rotation
                )
              }
              style={{ width: "80px", padding: "4px", fontSize: "12px" }}
            />
          </div>

          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
            <label style={{ fontSize: "12px" }}>Position Y:</label>
            <input
              type="number"
              value={transform.position_y}
              onChange={(e) =>
                onUpdateTransform(
                  transform.position_x,
                  parseFloat(e.target.value) || 0,
                  transform.scale_x,
                  transform.rotation
                )
              }
              style={{ width: "80px", padding: "4px", fontSize: "12px" }}
            />
          </div>

          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
            <label style={{ fontSize: "12px" }}>Scale:</label>
            <input
              type="number"
              step="0.1"
              min="0.1"
              max="10"
              value={transform.scale_x}
              onChange={(e) =>
                onUpdateTransform(
                  transform.position_x,
                  transform.position_y,
                  parseFloat(e.target.value) || 1.0,
                  transform.rotation
                )
              }
              style={{ width: "80px", padding: "4px", fontSize: "12px" }}
            />
          </div>

          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
            <label style={{ fontSize: "12px" }}>Rotation (°):</label>
            <input
              type="number"
              value={transform.rotation}
              onChange={(e) =>
                onUpdateTransform(
                  transform.position_x,
                  transform.position_y,
                  transform.scale_x,
                  parseFloat(e.target.value) || 0
                )
              }
              style={{ width: "80px", padding: "4px", fontSize: "12px" }}
            />
          </div>
        </section>

        {/* Opacity control */}
        <section style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
          <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)" }}>
            OPACITY ({Math.round(opacity * 100)}%)
          </div>
          <input
            type="range"
            min="0"
            max="1"
            step="0.05"
            value={opacity}
            onChange={(e) => onUpdateOpacity(parseFloat(e.target.value))}
            style={{ width: "100%" }}
          />
        </section>

        {/* Audio Fades if Video or Audio */}
        {onUpdateAudioFade && (selectedItem.kind === "Video" || selectedItem.kind === "Audio") && (
          <section style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
            <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)" }}>
              AUDIO FADES (FRAMES)
            </div>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <label style={{ fontSize: "12px" }}>Fade In:</label>
              <input
                type="number"
                min="0"
                value={selectedItem.fade_in?.value ?? 0}
                onChange={(e) =>
                  onUpdateAudioFade(
                    parseInt(e.target.value, 10) || 0,
                    selectedItem.fade_out?.value ?? 0
                  )
                }
                style={{ width: "80px", padding: "4px", fontSize: "12px" }}
              />
            </div>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <label style={{ fontSize: "12px" }}>Fade Out:</label>
              <input
                type="number"
                min="0"
                value={selectedItem.fade_out?.value ?? 0}
                onChange={(e) =>
                  onUpdateAudioFade(
                    selectedItem.fade_in?.value ?? 0,
                    parseInt(e.target.value, 10) || 0
                  )
                }
                style={{ width: "80px", padding: "4px", fontSize: "12px" }}
              />
            </div>
          </section>
        )}
      </div>
    </aside>
  );
};
