import React from "react";
import { Project, TimelineItem } from "../types/project";

interface PreviewCanvasProps {
  project?: Project | null;
  playheadFrame: number;
  selectedItem: TimelineItem | null;
  onPlayheadChange: (frame: number) => void;
}

export const PreviewCanvas: React.FC<PreviewCanvasProps> = ({
  project,
  playheadFrame,
  selectedItem,
}) => {
  // Format frame to SMPTE timecode (HH:MM:SS:FF) at 30 fps
  const formatTimecode = (frames: number) => {
    const fps = 30;
    const ff = Math.floor(frames % fps);
    const totalSecs = Math.floor(frames / fps);
    const ss = totalSecs % 60;
    const totalMins = Math.floor(totalSecs / 60);
    const mm = totalMins % 60;
    const hh = Math.floor(totalMins / 60);
    const pad = (n: number) => n.toString().padStart(2, "0");
    return `${pad(hh)}:${pad(mm)}:${pad(ss)}:${pad(ff)}`;
  };

  // Find active items under playhead across all unmuted tracks
  const activeItems: TimelineItem[] = [];
  if (project) {
    for (const track of project.tracks) {
      if (track.muted) continue;
      for (const item of track.items) {
        const start = item.timeline_range.start.value;
        const end = start + item.timeline_range.duration.value;
        if (playheadFrame >= start && playheadFrame < end) {
          activeItems.push(item);
        }
      }
    }
  }

  // Determine active text caption to display: prefer selectedItem, or active clip under playhead
  let captionText: string | null = null;
  if (selectedItem?.text_content && selectedItem.text_content.trim() !== "") {
    captionText = selectedItem.text_content;
  } else {
    for (const item of activeItems) {
      if (item.text_content && item.text_content.trim() !== "") {
        captionText = item.text_content;
        break;
      }
    }
  }

  return (
    <div className="canvas-area">
      <div className="canvas-viewport" style={{ position: "relative", overflow: "hidden" }}>
        {selectedItem ? (
          <div
            style={{
              position: "absolute",
              width: "55%",
              height: "55%",
              backgroundColor: "rgba(59, 130, 246, 0.25)",
              border: "2px solid var(--accent-primary)",
              borderRadius: "6px",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              flexDirection: "column",
              color: "#fff",
              transform: `translate(${selectedItem.transform.position_x}px, ${selectedItem.transform.position_y}px) scale(${selectedItem.transform.scale_x}) rotate(${selectedItem.transform.rotation}deg)`,
              opacity: selectedItem.opacity,
              padding: "12px",
              boxShadow: "0 4px 16px rgba(0, 0, 0, 0.4)",
            }}
          >
            <span style={{ fontSize: "24px", marginBottom: "4px" }}>🎞️</span>
            <span style={{ fontWeight: 600, fontSize: "14px" }}>{selectedItem.name}</span>
            <span style={{ fontSize: "10px", color: "var(--text-muted)" }}>
              {selectedItem.timeline_range.duration.value} frames
            </span>
            {selectedItem.text_content && selectedItem.text_content.trim() !== "" && (
              <div
                style={{
                  marginTop: "8px",
                  backgroundColor: "rgba(0, 0, 0, 0.75)",
                  color: "#93c5fd",
                  padding: "4px 10px",
                  borderRadius: "4px",
                  fontSize: "12px",
                  fontWeight: 600,
                  maxWidth: "90%",
                  textAlign: "center",
                  border: "1px solid rgba(147, 197, 253, 0.4)",
                  wordBreak: "break-word",
                }}
              >
                💬 {selectedItem.text_content}
              </div>
            )}
          </div>
        ) : activeItems.length > 0 ? (
          <div
            style={{
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              justifyContent: "center",
              gap: "4px",
            }}
          >
            <span style={{ fontSize: "28px" }}>🎬</span>
            <span style={{ fontWeight: 600, fontSize: "13px" }}>
              Active Playhead: {activeItems.length} clip(s) visible
            </span>
            <span style={{ fontSize: "11px", color: "var(--text-muted)" }}>
              {activeItems.map((a) => a.name).join(", ")}
            </span>
          </div>
        ) : (
          <>
            <span style={{ fontSize: "36px" }}>🎬</span>
            <span style={{ fontWeight: 600, fontSize: "14px" }}>TinixCut Preview Surface</span>
            <span style={{ fontSize: "11px", color: "var(--text-muted)" }}>
              1920 × 1080 (16:9) | Rec.709 Standard
            </span>
          </>
        )}

        {/* Dedicated Subtitle Caption Overlay */}
        {captionText && (
          <div
            style={{
              position: "absolute",
              bottom: "44px",
              left: "50%",
              transform: "translateX(-50%)",
              backgroundColor: "rgba(0, 0, 0, 0.82)",
              color: "#ffffff",
              padding: "8px 24px",
              borderRadius: "6px",
              fontSize: "17px",
              fontWeight: 600,
              textAlign: "center",
              maxWidth: "85%",
              border: "1px solid rgba(255, 255, 255, 0.3)",
              boxShadow: "0 4px 20px rgba(0, 0, 0, 0.8)",
              pointerEvents: "none",
              letterSpacing: "0.5px",
              zIndex: 15,
              wordBreak: "break-word",
              textShadow: "0 2px 4px rgba(0, 0, 0, 0.9)",
            }}
          >
            {captionText}
          </div>
        )}

        {/* Timecode overlay badge */}
        <div
          style={{
            position: "absolute",
            bottom: "10px",
            right: "10px",
            backgroundColor: "rgba(0, 0, 0, 0.75)",
            padding: "4px 8px",
            borderRadius: "4px",
            fontSize: "11px",
            fontFamily: "ui-monospace, monospace",
            color: "var(--text-main)",
            border: "1px solid var(--border-color)",
            zIndex: 10,
          }}
        >
          {formatTimecode(playheadFrame)} (Frame {playheadFrame})
        </div>
      </div>
    </div>
  );
};
