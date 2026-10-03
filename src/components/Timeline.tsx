import React, { useState, useRef } from "react";
import { Project, TimelineItem } from "../types/project";

interface TimelineProps {
  project: Project | null;
  selectedItemId: string | null;
  playheadFrame: number;
  onSelectItem: (itemId: string | null) => void;
  onPlayheadChange: (frame: number) => void;
  onMoveClip: (itemId: string, newStartFrame: number) => void;
  onTrimClip: (itemId: string, newStartFrame: number, newDurationFrames: number) => void;
  onSplitClip: (itemId: string, splitFrame: number) => void;
  onDeleteClip: (itemId: string) => void;
  onDuplicateClip: (itemId: string) => void;
}

const HEADER_WIDTH = 110; // Fixed width for track header column and ruler origin in pixels
const RULER_HEIGHT = 28; // Height of the time ruler in pixels

export const Timeline: React.FC<TimelineProps> = ({
  project,
  selectedItemId,
  playheadFrame,
  onSelectItem,
  onPlayheadChange,
  onMoveClip,
  onTrimClip,
  onSplitClip,
  onDeleteClip,
  onDuplicateClip,
}) => {
  const [zoom, setZoom] = useState<number>(3); // pixels per frame
  const [isScrubbing, setIsScrubbing] = useState<boolean>(false);

  // Transient gesture state (Workstream 3.4: local ephemeral state during drag)
  const [transientDrag, setTransientDrag] = useState<{
    itemId: string;
    mode: "move" | "trim-left" | "trim-right";
    initialPointerX: number;
    initialStartFrame: number;
    initialDurationFrames: number;
    currentDeltaFrames: number;
  } | null>(null);

  const tracksContainerRef = useRef<HTMLDivElement>(null);

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

  const canSplit =
    selectedItem &&
    playheadFrame > selectedItem.timeline_range.start.value &&
    playheadFrame < selectedItem.timeline_range.start.value + selectedItem.timeline_range.duration.value;

  // Calculate maximum frame across all tracks to determine total timeline width
  let maxFrame = 300; // minimum 10 seconds at 30fps
  if (project) {
    for (const track of project.tracks) {
      for (const item of track.items) {
        const endF = item.timeline_range.start.value + item.timeline_range.duration.value;
        if (endF > maxFrame) {
          maxFrame = endF;
        }
      }
    }
  }
  if (playheadFrame > maxFrame) {
    maxFrame = playheadFrame;
  }
  const totalFrames = maxFrame + 300; // extra space to the right
  const timelineContentWidth = Math.max(1200, HEADER_WIDTH + totalFrames * zoom);

  // Determine tick steps adaptively based on current zoom level
  let majorStep = 30; // 1 second at 30fps
  let minorStep = 10;
  if (zoom < 1.5) {
    majorStep = 90; // 3 seconds
    minorStep = 30;
  } else if (zoom < 2.5) {
    majorStep = 60; // 2 seconds
    minorStep = 15;
  } else if (zoom >= 6) {
    majorStep = 30; // 1 second
    minorStep = 5;
  }

  // Generate ticks for the ruler
  const ticks = [];
  for (let f = 0; f <= totalFrames; f += minorStep) {
    const isMajor = f % majorStep === 0;
    const x = HEADER_WIDTH + f * zoom;
    let label = null;
    if (isMajor) {
      const sec = Math.floor(f / 30);
      const m = Math.floor(sec / 60);
      const s = sec % 60;
      const timeStr = `${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
      label = zoom >= 2.5 ? `${timeStr} (${f}f)` : timeStr;
    }
    ticks.push({ f, x, isMajor, label });
  }

  // Calculate playhead frame from pointer position
  const seekFromPointerEvent = (e: React.PointerEvent | React.MouseEvent) => {
    if (!tracksContainerRef.current) return;
    const rect = tracksContainerRef.current.getBoundingClientRect();
    const clickX = e.clientX - rect.left + tracksContainerRef.current.scrollLeft;
    const frameX = clickX - HEADER_WIDTH;
    const frame = Math.max(0, Math.round(frameX / zoom));
    onPlayheadChange(frame);
  };

  const handleRulerPointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    e.stopPropagation();
    try {
      (e.target as HTMLElement).setPointerCapture(e.pointerId);
    } catch {
      // Ignore if capture fails
    }
    setIsScrubbing(true);
    seekFromPointerEvent(e);
  };

  const handleRulerPointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    if (isScrubbing) {
      seekFromPointerEvent(e);
    }
  };

  const handleRulerPointerUp = (e: React.PointerEvent<HTMLDivElement>) => {
    if (isScrubbing) {
      try {
        (e.target as HTMLElement).releasePointerCapture(e.pointerId);
      } catch {
        // Ignore
      }
      setIsScrubbing(false);
    }
  };

  const handlePointerDownMove = (
    e: React.PointerEvent,
    item: TimelineItem
  ) => {
    e.stopPropagation();
    onSelectItem(item.item_id);
    try {
      (e.target as HTMLElement).setPointerCapture(e.pointerId);
    } catch {
      // Ignore
    }

    setTransientDrag({
      itemId: item.item_id,
      mode: "move",
      initialPointerX: e.clientX,
      initialStartFrame: item.timeline_range.start.value,
      initialDurationFrames: item.timeline_range.duration.value,
      currentDeltaFrames: 0,
    });
  };

  const handlePointerDownTrim = (
    e: React.PointerEvent,
    item: TimelineItem,
    mode: "trim-left" | "trim-right"
  ) => {
    e.stopPropagation();
    onSelectItem(item.item_id);
    try {
      (e.target as HTMLElement).setPointerCapture(e.pointerId);
    } catch {
      // Ignore
    }

    setTransientDrag({
      itemId: item.item_id,
      mode,
      initialPointerX: e.clientX,
      initialStartFrame: item.timeline_range.start.value,
      initialDurationFrames: item.timeline_range.duration.value,
      currentDeltaFrames: 0,
    });
  };

  const handlePointerMove = (e: React.PointerEvent) => {
    if (!transientDrag) return;
    const deltaPixels = e.clientX - transientDrag.initialPointerX;
    const deltaFrames = Math.round(deltaPixels / zoom);
    setTransientDrag((prev) => (prev ? { ...prev, currentDeltaFrames: deltaFrames } : null));
  };

  const handlePointerUp = (e: React.PointerEvent) => {
    if (!transientDrag) return;
    try {
      (e.target as HTMLElement).releasePointerCapture(e.pointerId);
    } catch {
      // Ignore if not captured
    }

    const { mode, itemId, initialStartFrame, initialDurationFrames, currentDeltaFrames } = transientDrag;
    setTransientDrag(null);

    // Commit single authoritative domain command
    if (mode === "move") {
      const finalStart = Math.max(0, initialStartFrame + currentDeltaFrames);
      if (finalStart !== initialStartFrame) {
        onMoveClip(itemId, finalStart);
      }
    } else if (mode === "trim-left") {
      const proposedStart = Math.max(0, initialStartFrame + currentDeltaFrames);
      const deltaApplied = proposedStart - initialStartFrame;
      const finalDur = Math.max(1, initialDurationFrames - deltaApplied);
      onTrimClip(itemId, proposedStart, finalDur);
    } else if (mode === "trim-right") {
      const finalDur = Math.max(1, initialDurationFrames + currentDeltaFrames);
      onTrimClip(itemId, initialStartFrame, finalDur);
    }
  };

  const handleTracksAreaClick = (e: React.MouseEvent<HTMLDivElement>) => {
    // Only seek when clicking on empty background, not on buttons or clips
    if (e.target === e.currentTarget || (e.target as HTMLElement).dataset.trackLane) {
      seekFromPointerEvent(e);
    }
  };

  return (
    <div
      className="timeline-panel"
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      style={{ userSelect: "none" }}
    >
      {/* Timeline Toolbar */}
      <div className="timeline-toolbar">
        <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
          <button
            className="btn-action"
            disabled={!canSplit}
            onClick={() => selectedItem && onSplitClip(selectedItem.item_id, playheadFrame)}
            title="Split selected clip at playhead"
          >
            ✂ Split at Playhead
          </button>
          <button
            className="btn-action"
            disabled={!selectedItem}
            onClick={() => selectedItem && onDuplicateClip(selectedItem.item_id)}
            title="Duplicate selected clip"
          >
            📋 Duplicate
          </button>
          <button
            className="btn-action"
            disabled={!selectedItem}
            onClick={() => selectedItem && onDeleteClip(selectedItem.item_id)}
            title="Delete selected clip"
          >
            🗑 Delete
          </button>
        </div>

        <div style={{ display: "flex", gap: "12px", alignItems: "center" }}>
          <span style={{ fontSize: "11px", color: "var(--text-muted)" }}>Zoom</span>
          <input
            type="range"
            min="1"
            max="10"
            step="0.5"
            value={zoom}
            onChange={(e) => setZoom(parseFloat(e.target.value))}
            style={{ width: "80px", cursor: "pointer" }}
            title="Timeline Zoom level"
          />
          <span style={{ fontSize: "12px", fontFamily: "ui-monospace, monospace", color: "var(--accent-primary)" }}>
            Frame {playheadFrame}
          </span>
        </div>
      </div>

      {/* Timeline Content: Scrollable Ruler and Track Rows */}
      <div
        ref={tracksContainerRef}
        onClick={handleTracksAreaClick}
        style={{
          flex: 1,
          overflowX: "auto",
          overflowY: "auto",
          position: "relative",
          backgroundColor: "#141417",
          padding: 0,
        }}
      >
        {/* TIME RULER (Sticky at top when scrolling vertically) */}
        <div
          className="timeline-ruler"
          onPointerDown={handleRulerPointerDown}
          onPointerMove={handleRulerPointerMove}
          onPointerUp={handleRulerPointerUp}
          style={{
            position: "sticky",
            top: 0,
            height: `${RULER_HEIGHT}px`,
            minHeight: `${RULER_HEIGHT}px`,
            backgroundColor: "#18181c",
            borderBottom: "1px solid var(--border-color)",
            zIndex: 20,
            minWidth: `${timelineContentWidth}px`,
            cursor: "col-resize",
            userSelect: "none",
          }}
          title="Click or drag on the ruler to scrub playhead"
        >
          {/* Sticky Top-Left Corner Box */}
          <div
            style={{
              position: "sticky",
              left: 0,
              width: `${HEADER_WIDTH}px`,
              minWidth: `${HEADER_WIDTH}px`,
              height: "100%",
              backgroundColor: "#18181c",
              borderRight: "1px solid var(--border-color)",
              zIndex: 25,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              fontSize: "10px",
              fontWeight: 700,
              color: "var(--text-muted)",
              letterSpacing: "0.5px",
            }}
          >
            ⏱ RULER
          </div>

          {/* SVG Ticks and Time Labels */}
          <svg
            style={{
              position: "absolute",
              top: 0,
              left: 0,
              width: `${timelineContentWidth}px`,
              height: `${RULER_HEIGHT}px`,
              pointerEvents: "none",
            }}
          >
            {ticks.map((t) => (
              <g key={t.f}>
                <line
                  x1={t.x}
                  y1={t.isMajor ? 14 : 20}
                  x2={t.x}
                  y2={RULER_HEIGHT}
                  stroke={t.isMajor ? "#64748b" : "#334155"}
                  strokeWidth="1"
                />
                {t.label && (
                  <text
                    x={t.x + 4}
                    y={13}
                    fill="#94a3b8"
                    fontSize="10"
                    fontFamily="ui-monospace, monospace"
                    fontWeight="500"
                  >
                    {t.label}
                  </text>
                )}
              </g>
            ))}
          </svg>
        </div>

        {/* PLAYHEAD INDICATOR (Spans across ruler and all tracks) */}
        <div
          style={{
            position: "absolute",
            top: 0,
            bottom: 0,
            left: `${HEADER_WIDTH + playheadFrame * zoom}px`,
            width: "2px",
            backgroundColor: "#ef4444",
            zIndex: 30,
            pointerEvents: "none",
            minHeight: "100%",
          }}
        >
          {/* Top Playhead Scrub Head Badge on Ruler */}
          <div
            style={{
              position: "absolute",
              top: 0,
              left: "-6px",
              width: "14px",
              height: "16px",
              backgroundColor: "#ef4444",
              clipPath: "polygon(0 0, 100% 0, 100% 55%, 50% 100%, 0 55%)",
              filter: "drop-shadow(0 2px 4px rgba(0, 0, 0, 0.6))",
            }}
          />
        </div>

        {/* TRACKS LIST */}
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            gap: "8px",
            minWidth: `${timelineContentWidth}px`,
            padding: "8px 0 24px 0",
          }}
        >
          {project?.tracks.map((track) => (
            <div
              key={track.track_id}
              data-track-lane="true"
              style={{
                height: "46px",
                backgroundColor: "var(--bg-surface)",
                position: "relative",
                borderTop: "1px solid var(--border-color)",
                borderBottom: "1px solid var(--border-color)",
                cursor: "pointer",
              }}
            >
              {/* Track Header Label (Sticky Left) */}
              <div
                style={{
                  position: "sticky",
                  left: 0,
                  width: `${HEADER_WIDTH}px`,
                  minWidth: `${HEADER_WIDTH}px`,
                  height: "100%",
                  backgroundColor: "rgba(24, 24, 28, 0.95)",
                  borderRight: "1px solid var(--border-color)",
                  zIndex: 10,
                  display: "flex",
                  alignItems: "center",
                  padding: "0 10px",
                  fontSize: "11px",
                  fontWeight: 600,
                  color: "var(--text-muted)",
                  boxShadow: "2px 0 4px rgba(0, 0, 0, 0.3)",
                }}
              >
                {track.name}
              </div>

              {/* Items in Track */}
              {track.items.map((item) => {
                const isSelected = item.item_id === selectedItemId;
                const isDragging = transientDrag?.itemId === item.item_id;

                let startF = item.timeline_range.start.value;
                let durF = item.timeline_range.duration.value;

                // Apply transient geometry offsets without committing to backend
                if (isDragging && transientDrag) {
                  if (transientDrag.mode === "move") {
                    startF = Math.max(0, transientDrag.initialStartFrame + transientDrag.currentDeltaFrames);
                  } else if (transientDrag.mode === "trim-left") {
                    const proposed = Math.max(0, transientDrag.initialStartFrame + transientDrag.currentDeltaFrames);
                    const delta = proposed - transientDrag.initialStartFrame;
                    startF = proposed;
                    durF = Math.max(1, transientDrag.initialDurationFrames - delta);
                  } else if (transientDrag.mode === "trim-right") {
                    durF = Math.max(1, transientDrag.initialDurationFrames + transientDrag.currentDeltaFrames);
                  }
                }

                return (
                  <div
                    key={item.item_id}
                    onPointerDown={(e) => handlePointerDownMove(e, item)}
                    style={{
                      position: "absolute",
                      top: "4px",
                      bottom: "4px",
                      left: `${HEADER_WIDTH + startF * zoom}px`,
                      width: `${Math.max(8, durF * zoom)}px`,
                      backgroundColor: isSelected ? "#2563eb" : "#1e40af",
                      border: isSelected ? "2px solid #60a5fa" : "1px solid #3b82f6",
                      borderRadius: "4px",
                      cursor: "grab",
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "space-between",
                      padding: "0 4px",
                      overflow: "hidden",
                      color: "#fff",
                      fontSize: "11px",
                      fontWeight: 500,
                      boxShadow: isSelected ? "0 0 10px rgba(59, 130, 246, 0.6)" : "none",
                      zIndex: isSelected ? 8 : 5,
                    }}
                  >
                    {/* Left Trim Handle */}
                    <div
                      onPointerDown={(e) => handlePointerDownTrim(e, item, "trim-left")}
                      style={{
                        width: "8px",
                        height: "100%",
                        cursor: "ew-resize",
                        backgroundColor: "rgba(255, 255, 255, 0.2)",
                        borderRadius: "2px 0 0 2px",
                      }}
                      title="Drag to trim start"
                    />

                    {/* Clip Title & Caption */}
                    <span
                      style={{
                        whiteSpace: "nowrap",
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        padding: "0 4px",
                        pointerEvents: "none",
                        display: "flex",
                        alignItems: "center",
                        gap: "6px",
                      }}
                    >
                      <span style={{ fontWeight: 600 }}>{item.name}</span>
                      {item.text_content && item.text_content.trim() !== "" && (
                        <span
                          style={{
                            backgroundColor: "rgba(0, 0, 0, 0.45)",
                            color: "#bfdbfe",
                            padding: "1px 5px",
                            borderRadius: "3px",
                            fontSize: "10px",
                            fontWeight: 600,
                            border: "1px solid rgba(191, 219, 254, 0.3)",
                            maxWidth: "180px",
                            overflow: "hidden",
                            textOverflow: "ellipsis",
                          }}
                          title={`Caption: ${item.text_content}`}
                        >
                          💬 {item.text_content}
                        </span>
                      )}
                      <span style={{ opacity: 0.7, fontSize: "10px" }}>({durF}f)</span>
                    </span>

                    {/* Right Trim Handle */}
                    <div
                      onPointerDown={(e) => handlePointerDownTrim(e, item, "trim-right")}
                      style={{
                        width: "8px",
                        height: "100%",
                        cursor: "ew-resize",
                        backgroundColor: "rgba(255, 255, 255, 0.2)",
                        borderRadius: "0 2px 2px 0",
                      }}
                      title="Drag to trim end"
                    />
                  </div>
                );
              })}
            </div>
          ))}

          {/* Empty Tracks Message */}
          {(!project || project.tracks.length === 0) && (
            <div
              style={{
                padding: "24px 16px",
                color: "var(--text-muted)",
                fontSize: "12px",
                fontStyle: "italic",
                display: "flex",
                alignItems: "center",
                gap: "8px",
              }}
            >
              <span>Chưa có track nào trên timeline. Nhấn <strong>+ Track</strong> ở thanh trên để thêm track mới.</span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
