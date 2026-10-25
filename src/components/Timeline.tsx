import React, { useState, useRef, useEffect } from "react";
import { getDefaultAudioCrossfadeDurationFrames, isRangeAdjacent, Keyframe, KeyframeProperty, Project, TimelineItem } from "../types/project";
import { WaveformData } from "../types/preview";
import { editorApi } from "../services/editorApi";
import { buildSnapTargets, calculateSnap, SnapCandidate } from "../utils/timelineSnapping";

interface TimelineProps {
  project: Project | null;
  selectedItemId?: string | null;
  selectedItemIds?: Set<string>;
  primarySelectedItemId?: string | null;
  selectedTransitionId?: string | null;
  playheadFrame: number;
  snapEnabled?: boolean;
  onToggleSnapping?: () => void;
  onSelectItem: (itemId: string | null, isCtrl?: boolean) => void;
  onClearSelection?: () => void;
  onSelectTransition?: (transitionId: string | null) => void;
  onPlayheadChange: (frame: number) => void;
  onMoveClip: (itemId: string, newStartFrame: number) => void;
  onGroupMoveClip?: (itemIds: string[], deltaFrames: number) => void;
  onTrimClip: (itemId: string, newStartFrame: number, newDurationFrames: number) => void;
  onSplitClip: (itemId: string, splitFrame: number) => void;
  onDeleteClip: (itemId: string) => void;
  onDuplicateClip: (itemId: string) => void;
  onMoveKeyframe?: (itemId: string, property: KeyframeProperty, oldOffsetFrame: number, newOffsetFrame: number) => void;
  onCreateCutTransition?: (
    trackId: string,
    leftItemId: string,
    rightItemId: string,
    kind: "CrossDissolve" | "DipToBlack",
    durationFrames: number
  ) => void;
  onCreateAudioCrossfade?: (trackId: string, leftItemId: string, rightItemId: string, durationFrames: number) => void;
  onToggleTrackMute?: (trackId: string, muted: boolean) => void;
  zoom?: number;
  onZoomIn?: () => void;
  onZoomOut?: () => void;
  onSetZoom?: (zoom: number) => void;
}

const HEADER_WIDTH = 110; // Fixed width for track header column and ruler origin in pixels
const RULER_HEIGHT = 28; // Height of the time ruler in pixels

export const Timeline: React.FC<TimelineProps> = ({
  project,
  selectedItemId,
  selectedItemIds,
  primarySelectedItemId,
  selectedTransitionId,
  playheadFrame,
  snapEnabled = true,
  onToggleSnapping,
  onSelectItem,
  onClearSelection,
  onSelectTransition,
  onPlayheadChange,
  onMoveClip,
  onGroupMoveClip,
  onTrimClip,
  onSplitClip,
  onDeleteClip,
  onDuplicateClip,
  onMoveKeyframe,
  onCreateCutTransition,
  onCreateAudioCrossfade,
  onToggleTrackMute,
  zoom: externalZoom,
  onZoomIn,
  onZoomOut,
  onSetZoom,
}) => {
  const [internalZoom, setInternalZoom] = useState<number>(3); // pixels per frame
  const zoom = externalZoom !== undefined ? externalZoom : internalZoom;
  const setZoom = onSetZoom || setInternalZoom;

  const effectiveSelectedItemIds = React.useMemo(() => {
    if (selectedItemIds) return selectedItemIds;
    if (selectedItemId) return new Set([selectedItemId]);
    return new Set<string>();
  }, [selectedItemIds, selectedItemId]);

  const effectivePrimarySelectedItemId = primarySelectedItemId ?? selectedItemId ?? null;

  const [isScrubbing, setIsScrubbing] = useState<boolean>(false);
  const [waveforms, setWaveforms] = useState<Record<string, WaveformData>>({});
  const [waveformPollTick, setWaveformPollTick] = useState(0);
  const waveformRequestsInFlight = useRef(new Set<string>());

  // Transient gesture state (Workstream 3.4: local ephemeral state during drag)
  const [transientDrag, setTransientDrag] = useState<{
    itemId: string;
    mode: "move" | "trim-left" | "trim-right";
    initialPointerX: number;
    initialStartFrame: number;
    initialDurationFrames: number;
    currentDeltaFrames: number;
    isMulti: boolean;
  } | null>(null);

  // Transient keyframe drag state (Section B6)
  const [transientKfDrag, setTransientKfDrag] = useState<{
    itemId: string;
    property: KeyframeProperty;
    initialOffset: number;
    currentOffset: number;
    initialPointerX: number;
    maxDuration: number;
  } | null>(null);

  const tracksContainerRef = useRef<HTMLDivElement>(null);

  // Load disposable waveform cache for referenced media assets
  useEffect(() => {
    if (!project) return;
    const assetIds = new Set<string>();
    for (const track of project.tracks) {
      for (const item of track.items) {
        if (item.asset_id && (item.kind === "Audio" || item.kind === "Video")) {
          assetIds.add(item.asset_id);
        }
      }
    }
    let disposed = false;
    let shouldPollAgain = false;

    const loadWaveforms = async () => {
      await Promise.all(
        [...assetIds].map(async (assetId) => {
          if (waveforms[assetId] || waveformRequestsInFlight.current.has(assetId)) {
            return;
          }

          waveformRequestsInFlight.current.add(assetId);
          try {
            const result = await editorApi.getWaveform(assetId);
            if (disposed) return;

            if (result.status === "Ready") {
              setWaveforms((previous) => ({ ...previous, [assetId]: result.payload }));
            } else if (result.status === "Pending") {
              // Waveform extraction is intentionally a background job. Poll until its
              // disposable cache is ready instead of waiting for an unrelated render.
              shouldPollAgain = true;
            }
          } catch {
            // Graceful non-blocking degradation if asset has no audio or is missing.
          } finally {
            waveformRequestsInFlight.current.delete(assetId);
          }
        })
      );

      if (!disposed && shouldPollAgain) {
        window.setTimeout(() => {
          if (!disposed) setWaveformPollTick((tick) => tick + 1);
        }, 400);
      }
    };

    void loadWaveforms();
    return () => {
      disposed = true;
    };
  }, [project, waveforms, waveformPollTick]);

  // Find currently selected primary item
  let selectedItem: TimelineItem | null = null;
  if (project && effectivePrimarySelectedItemId) {
    for (const track of project.tracks) {
      const it = track.items.find((i) => i.item_id === effectivePrimarySelectedItemId);
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
    const isCtrl = e.ctrlKey || e.metaKey;

    if (isCtrl) {
      // Ctrl+Click: toggle item in selection, DO NOT start drag (P16-TP-05, P16-AC-06)
      onSelectItem(item.item_id, true);
      return;
    }

    const isAlreadySelected = effectiveSelectedItemIds.has(item.item_id);
    const isMulti = isAlreadySelected && effectiveSelectedItemIds.size > 1;

    // Normal click / pointer down:
    onSelectItem(item.item_id, false);

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
      isMulti,
    });
  };

  const handlePointerDownTrim = (
    e: React.PointerEvent,
    item: TimelineItem,
    mode: "trim-left" | "trim-right"
  ) => {
    e.stopPropagation();
    onSelectItem(item.item_id, false);
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
      isMulti: false,
    });
  };

  const handleKfPointerDown = (
    e: React.PointerEvent,
    item: TimelineItem,
    kf: Keyframe
  ) => {
    e.stopPropagation();
    onSelectItem(item.item_id, false);
    try {
      (e.target as HTMLElement).setPointerCapture(e.pointerId);
    } catch {
      // Ignore
    }

    setTransientKfDrag({
      itemId: item.item_id,
      property: kf.property,
      initialOffset: kf.offset.value,
      currentOffset: kf.offset.value,
      initialPointerX: e.clientX,
      maxDuration: item.timeline_range.duration.value,
    });
  };

  const handlePointerMove = (e: React.PointerEvent) => {
    if (transientDrag) {
      const deltaPixels = e.clientX - transientDrag.initialPointerX;
      const rawDeltaFrames = Math.round(deltaPixels / zoom);

      // Collect unselected clip edges for snap targets
      // Selected clips do NOT contribute snap targets during their own move (P16-TP-09, P16-AC-18)
      const unselectedClipEdges: Array<{ start: number; end: number; itemId: string }> = [];
      if (project) {
        for (const track of project.tracks) {
          for (const item of track.items) {
            if (!effectiveSelectedItemIds.has(item.item_id)) {
              unselectedClipEdges.push({
                start: item.timeline_range.start.value,
                end: item.timeline_range.start.value + item.timeline_range.duration.value,
                itemId: item.item_id,
              });
            }
          }
        }
      }

      const snapTargets = buildSnapTargets(playheadFrame, unselectedClipEdges);

      let snapCandidates: SnapCandidate[] = [];
      if (transientDrag.mode === "move") {
        snapCandidates = [
          { anchorFrame: transientDrag.initialStartFrame, anchorKind: "start" },
          { anchorFrame: transientDrag.initialStartFrame + transientDrag.initialDurationFrames, anchorKind: "end" },
        ];
      } else if (transientDrag.mode === "trim-left") {
        snapCandidates = [
          { anchorFrame: transientDrag.initialStartFrame, anchorKind: "start" },
        ];
      } else if (transientDrag.mode === "trim-right") {
        snapCandidates = [
          { anchorFrame: transientDrag.initialStartFrame + transientDrag.initialDurationFrames, anchorKind: "end" },
        ];
      }

      const snapRes = calculateSnap(
        snapCandidates,
        rawDeltaFrames,
        snapTargets,
        zoom,
        snapEnabled,
        e.altKey
      );

      const effectiveDelta = snapRes.snappedDeltaFrames;
      setTransientDrag((prev) => (prev ? { ...prev, currentDeltaFrames: effectiveDelta } : null));
    } else if (transientKfDrag) {
      const deltaPixels = e.clientX - transientKfDrag.initialPointerX;
      const deltaFrames = Math.round(deltaPixels / zoom);
      const proposedOffset = Math.max(
        0,
        Math.min(transientKfDrag.maxDuration, transientKfDrag.initialOffset + deltaFrames)
      );
      setTransientKfDrag((prev) => (prev ? { ...prev, currentOffset: proposedOffset } : null));
    }
  };

  const handlePointerUp = (e: React.PointerEvent) => {
    if (transientDrag) {
      try {
        (e.target as HTMLElement).releasePointerCapture(e.pointerId);
      } catch {
        // Ignore if not captured
      }

      const { mode, itemId, initialStartFrame, initialDurationFrames, currentDeltaFrames, isMulti } = transientDrag;
      setTransientDrag(null);

      // Commit single authoritative domain command
      if (mode === "move") {
        if (isMulti && onGroupMoveClip && effectiveSelectedItemIds.size > 1) {
          if (currentDeltaFrames !== 0) {
            onGroupMoveClip(Array.from(effectiveSelectedItemIds), currentDeltaFrames);
          }
        } else {
          const finalStart = Math.max(0, initialStartFrame + currentDeltaFrames);
          if (finalStart !== initialStartFrame) {
            onMoveClip(itemId, finalStart);
          }
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
    } else if (transientKfDrag) {
      try {
        (e.target as HTMLElement).releasePointerCapture(e.pointerId);
      } catch {
        // Ignore
      }
      const { itemId, property, initialOffset, currentOffset } = transientKfDrag;
      setTransientKfDrag(null);
      if (currentOffset !== initialOffset && onMoveKeyframe) {
        onMoveKeyframe(itemId, property, initialOffset, currentOffset);
      }
    }
  };

  const getKfColor = (prop: KeyframeProperty) => {
    switch (prop) {
      case "PositionX":
      case "PositionY":
        return "#38bdf8"; // Sky blue
      case "ScaleX":
      case "ScaleY":
        return "#c084fc"; // Purple
      case "Rotation":
        return "#f472b6"; // Pink
      case "Opacity":
        return "#34d399"; // Emerald
      case "Volume":
        return "#fbbf24"; // Amber
      default:
        return "#fbbf24";
    }
  };

  const renderWaveformSvg = (
    wf: WaveformData,
    sourceStartF: number,
    sourceDurF: number,
    frameRate: number,
    widthPx: number,
    heightPx: number
  ) => {
    const rate = frameRate || 30;
    const sourceStartSec = sourceStartF / rate;
    const sourceDurSec = sourceDurF / rate;
    const bucketStart = Math.max(0, Math.floor(sourceStartSec * wf.bucket_rate));
    const bucketCount = Math.max(1, Math.ceil(sourceDurSec * wf.bucket_rate));
    const totalBuckets = Math.floor(wf.peaks.length / 2);
    const bucketEnd = Math.min(totalBuckets, bucketStart + bucketCount);

    const availableBuckets = bucketEnd - bucketStart;
    if (availableBuckets <= 1 || widthPx <= 4) return null;

    // Resample adaptively: up to 1 sample per 2 pixels
    const numSamples = Math.min(availableBuckets, Math.max(20, Math.floor(widthPx / 2)));
    const step = availableBuckets / numSamples;
    const midY = heightPx / 2;
    const ampScale = midY * 0.85;

    const topPoints: string[] = [];
    const bottomPoints: string[] = [];

    for (let i = 0; i < numSamples; i++) {
      const bIdx = Math.min(totalBuckets - 1, Math.floor(bucketStart + i * step));
      const minVal = wf.peaks[bIdx * 2] ?? 0;
      const maxVal = wf.peaks[bIdx * 2 + 1] ?? 0;
      const x = ((i / (numSamples - 1)) * widthPx).toFixed(1);
      const yTop = (midY - maxVal * ampScale).toFixed(1);
      const yBottom = (midY - minVal * ampScale).toFixed(1);

      topPoints.push(`${x},${yTop}`);
      bottomPoints.unshift(`${x},${yBottom}`);
    }

    const pathD = `M ${topPoints.join(" L ")} L ${bottomPoints.join(" L ")} Z`;

    return (
      <svg
        style={{
          position: "absolute",
          top: 0,
          left: 0,
          width: `${widthPx}px`,
          height: `${heightPx}px`,
          pointerEvents: "none",
          opacity: 0.35,
          zIndex: 1,
        }}
      >
        <path d={pathD} fill="#67e8f9" />
      </svg>
    );
  };

  const handleTracksAreaClick = (e: React.MouseEvent<HTMLDivElement>) => {
    // Only seek when clicking on empty background, not on buttons or clips
    if (e.target === e.currentTarget || (e.target as HTMLElement).dataset.trackLane) {
      if (onClearSelection) {
        onClearSelection();
      } else {
        onSelectItem(null);
      }
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
            title="Split selected clip at playhead (Ctrl+K)"
          >
            ✂ Split at Playhead
          </button>
          <button
            className="btn-action"
            disabled={!selectedItem}
            onClick={() => selectedItem && onDuplicateClip(selectedItem.item_id)}
            title="Duplicate selected primary clip (Ctrl+D)"
          >
            📋 Duplicate
          </button>
          <button
            className="btn-action"
            disabled={effectiveSelectedItemIds.size === 0}
            onClick={() => {
              if (effectiveSelectedItemIds.size > 0) {
                // If single item, existing onDeleteClip works; if multi, App.tsx will handle
                if (selectedItem) onDeleteClip(selectedItem.item_id);
              }
            }}
            title="Delete selected clip(s) (Delete / Backspace)"
          >
            🗑 Delete {effectiveSelectedItemIds.size > 1 ? `(${effectiveSelectedItemIds.size})` : ""}
          </button>
        </div>

        <div style={{ display: "flex", gap: "12px", alignItems: "center" }}>
          <button
            className="btn-action"
            style={{
              padding: "4px 8px",
              fontSize: "11px",
              backgroundColor: snapEnabled ? "var(--accent-primary, #3b82f6)" : "transparent",
              color: snapEnabled ? "#fff" : "var(--text-muted)",
              border: "1px solid var(--border-color)",
            }}
            onClick={onToggleSnapping}
            title="Toggle Snapping (S)"
          >
            🧲 Snap {snapEnabled ? "ON" : "OFF"}
          </button>
          <span style={{ fontSize: "11px", color: "var(--text-muted)" }}>Zoom</span>
          <button
            className="btn-action"
            style={{ padding: "2px 8px", fontSize: "12px" }}
            onClick={() => (onZoomOut ? onZoomOut() : setZoom(Math.max(1, zoom - 0.5)))}
            title="Zoom Out (-)"
          >
            -
          </button>
          <input
            type="range"
            min="1"
            max="10"
            step="0.5"
            value={zoom}
            onChange={(e) => setZoom(parseFloat(e.target.value))}
            style={{ width: "70px", cursor: "pointer" }}
            title="Timeline Zoom level (= / -)"
          />
          <button
            className="btn-action"
            style={{ padding: "2px 8px", fontSize: "12px" }}
            onClick={() => (onZoomIn ? onZoomIn() : setZoom(Math.min(10, zoom + 0.5)))}
            title="Zoom In (+)"
          >
            +
          </button>
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
                <span
                  style={{
                    minWidth: 0,
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                  }}
                  title={track.name}
                >
                  {track.name}
                </span>
                <button
                  type="button"
                  aria-label={`${track.muted ? "Unmute" : "Mute"} ${track.name}`}
                  aria-pressed={track.muted}
                  title={track.muted ? `Unmute ${track.name}` : `Mute ${track.name}`}
                  onPointerDown={(event) => event.stopPropagation()}
                  onClick={(event) => {
                    event.stopPropagation();
                    onToggleTrackMute?.(track.track_id, !track.muted);
                  }}
                  style={{
                    marginLeft: "auto",
                    padding: "3px 6px",
                    border: "1px solid var(--border-color)",
                    borderRadius: "3px",
                    backgroundColor: track.muted ? "#7f1d1d" : "#27272a",
                    color: track.muted ? "#fecaca" : "var(--text-muted)",
                    cursor: "pointer",
                    fontSize: "10px",
                    fontWeight: 700,
                    lineHeight: 1.2,
                  }}
                >
                  {track.muted ? "Unmute" : "Mute"}
                </button>
              </div>

              {/* Items in Track */}
              {track.items.map((item) => {
                const isSelected = effectiveSelectedItemIds.has(item.item_id);
                const isPrimary = item.item_id === effectivePrimarySelectedItemId;
                const isDraggingThis = transientDrag?.itemId === item.item_id;
                const isGroupMoving = transientDrag?.mode === "move" && transientDrag.isMulti && isSelected;

                let startF = item.timeline_range.start.value;
                let durF = item.timeline_range.duration.value;

                // Apply transient geometry offsets without committing to backend
                if (transientDrag) {
                  if (transientDrag.mode === "move") {
                    if (isGroupMoving) {
                      startF = Math.max(0, item.timeline_range.start.value + transientDrag.currentDeltaFrames);
                    } else if (isDraggingThis) {
                      startF = Math.max(0, transientDrag.initialStartFrame + transientDrag.currentDeltaFrames);
                    }
                  } else if (isDraggingThis) {
                    if (transientDrag.mode === "trim-left") {
                      const proposed = Math.max(0, transientDrag.initialStartFrame + transientDrag.currentDeltaFrames);
                      const delta = proposed - transientDrag.initialStartFrame;
                      startF = proposed;
                      durF = Math.max(1, transientDrag.initialDurationFrames - delta);
                    } else if (transientDrag.mode === "trim-right") {
                      durF = Math.max(1, transientDrag.initialDurationFrames + transientDrag.currentDeltaFrames);
                    }
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
                      backgroundColor: isPrimary ? "#1d4ed8" : isSelected ? "#2563eb" : "#1e40af",
                      border: isPrimary
                        ? "2px solid #93c5fd"
                        : isSelected
                        ? "2px solid #60a5fa"
                        : "1px solid #3b82f6",
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
                      boxShadow: isPrimary
                        ? "0 0 12px rgba(147, 197, 253, 0.8)"
                        : isSelected
                        ? "0 0 8px rgba(59, 130, 246, 0.6)"
                        : "none",
                      zIndex: isPrimary ? 9 : isSelected ? 8 : 5,
                    }}
                  >
                    {/* Audio Waveform Visualization (Trim-aware & Zoom-resilient) */}
                    {item.asset_id &&
                      waveforms[item.asset_id] &&
                      renderWaveformSvg(
                        waveforms[item.asset_id],
                        transientDrag?.itemId === item.item_id && transientDrag.mode === "trim-left"
                          ? item.source_range.start.value +
                              (startF - transientDrag.initialStartFrame) *
                                (item.speed === "Half" ? 0.5 : item.speed === "Double" ? 2 : 1)
                          : item.source_range.start.value,
                        durF * (item.speed === "Half" ? 0.5 : item.speed === "Double" ? 2 : 1),
                        project?.settings.frame_rate
                          ? project.settings.frame_rate.numerator / (project.settings.frame_rate.denominator || 1)
                          : 30,
                        Math.max(8, durF * zoom),
                        38
                      )}

                    {/* Left Trim Handle */}
                    <div
                      onPointerDown={(e) => handlePointerDownTrim(e, item, "trim-left")}
                      style={{
                        width: "8px",
                        height: "100%",
                        cursor: "ew-resize",
                        backgroundColor: "rgba(255, 255, 255, 0.2)",
                        borderRadius: "2px 0 0 2px",
                        zIndex: 7,
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
                        zIndex: 4,
                      }}
                    >
                      {isPrimary && effectiveSelectedItemIds.size > 1 && (
                        <span
                          style={{
                            fontSize: "9px",
                            backgroundColor: "#1e3a8a",
                            padding: "1px 4px",
                            borderRadius: "3px",
                            border: "1px solid #93c5fd",
                            fontWeight: 700,
                          }}
                        >
                          Primary
                        </span>
                      )}
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

                    {/* Keyframe Markers Strip */}
                    {item.keyframes && item.keyframes.length > 0 && (
                      <div
                        style={{
                          position: "absolute",
                          left: 0,
                          right: 0,
                          bottom: "1px",
                          height: "12px",
                          backgroundColor: "rgba(0, 0, 0, 0.45)",
                          borderTop: "1px solid rgba(255, 255, 255, 0.15)",
                          zIndex: 6,
                          pointerEvents: "auto",
                        }}
                        onClick={(e) => e.stopPropagation()}
                      >
                        {item.keyframes.map((kf, kfIdx) => {
                          const isDraggingThis =
                            transientKfDrag &&
                            transientKfDrag.itemId === item.item_id &&
                            transientKfDrag.property === kf.property &&
                            transientKfDrag.initialOffset === kf.offset.value;
                          const currentOffset = isDraggingThis
                            ? transientKfDrag.currentOffset
                            : kf.offset.value;
                          const markerX = currentOffset * zoom;

                          return (
                            <div
                              key={`${kf.property}_${kf.offset.value}_${kfIdx}`}
                              onPointerDown={(e) => handleKfPointerDown(e, item, kf)}
                              onClick={(e) => {
                                e.stopPropagation();
                                onPlayheadChange(item.timeline_range.start.value + kf.offset.value);
                              }}
                              style={{
                                position: "absolute",
                                left: `${markerX}px`,
                                top: "50%",
                                transform: "translate(-50%, -50%) rotate(45deg)",
                                width: "7px",
                                height: "7px",
                                backgroundColor: isDraggingThis ? "#38bdf8" : getKfColor(kf.property),
                                border: isDraggingThis ? "1px solid #fff" : "1px solid rgba(0, 0, 0, 0.8)",
                                boxShadow: isDraggingThis
                                  ? "0 0 6px #38bdf8"
                                  : "0 1px 2px rgba(0, 0, 0, 0.5)",
                                cursor: "ew-resize",
                                zIndex: 10,
                              }}
                              title={`${kf.property}: ${kf.value} @ +${kf.offset.value}f (Kéo để di chuyển, click để nhảy playhead)`}
                            />
                          );
                        })}
                      </div>
                    )}

                    {/* Right Trim Handle */}
                    <div
                      onPointerDown={(e) => handlePointerDownTrim(e, item, "trim-right")}
                      style={{
                        width: "8px",
                        height: "100%",
                        cursor: "ew-resize",
                        backgroundColor: "rgba(255, 255, 255, 0.2)",
                        borderRadius: "0 2px 2px 0",
                        zIndex: 7,
                      }}
                      title="Drag to trim end"
                    />
                  </div>
                );
              })}

              {/* Transitions on Track */}
              {track.transitions?.map((trans) => {
                const leftItem = track.items.find((i) => i.item_id === trans.left_item_id);
                const rightItem = track.items.find((i) => i.item_id === trans.right_item_id);
                if (!leftItem || !rightItem) return null;

                const cutF = leftItem.timeline_range.start.value + leftItem.timeline_range.duration.value;
                const durF = trans.duration.value;
                const lF = Math.floor(durF / 2);
                const rF = durF - lF;
                const t0F = cutF - lF;
                const t1F = cutF + rF;
                const isSelected = selectedTransitionId === trans.transition_id;

                return (
                  <div
                    key={trans.transition_id}
                    onClick={(e) => {
                      e.stopPropagation();
                      onSelectTransition?.(trans.transition_id);
                      onSelectItem(null);
                    }}
                    style={{
                      position: "absolute",
                      top: "6px",
                      bottom: "6px",
                      left: `${HEADER_WIDTH + t0F * zoom}px`,
                      width: `${Math.max(16, (t1F - t0F) * zoom)}px`,
                      background: isSelected
                        ? "linear-gradient(135deg, rgba(245, 158, 11, 0.95), rgba(217, 119, 6, 0.95))"
                        : "linear-gradient(135deg, rgba(245, 158, 11, 0.65), rgba(59, 130, 246, 0.65))",
                      border: isSelected ? "2px solid #fbbf24" : "1px solid #f59e0b",
                      borderRadius: "3px",
                      cursor: "pointer",
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "center",
                      zIndex: 12,
                      color: "#fff",
                      fontSize: "10px",
                      fontWeight: 700,
                      boxShadow: isSelected ? "0 0 8px rgba(245, 158, 11, 0.9)" : "0 1px 3px rgba(0, 0, 0, 0.4)",
                      overflow: "hidden",
                      whiteSpace: "nowrap",
                      padding: "0 2px",
                      userSelect: "none",
                    }}
                    title={`CrossDissolve: ${durF} frames [${t0F}, ${t1F}) (Click to select)`}
                  >
                    ⧓ {durF}f
                  </div>
                );
              })}

              {track.audio_transitions?.map((trans) => {
                const leftItem = track.items.find((item) => item.item_id === trans.left_item_id);
                if (!leftItem) return null;
                const cutF = leftItem.timeline_range.start.value + leftItem.timeline_range.duration.value;
                const durF = trans.duration.value;
                const leftF = Math.floor(durF / 2);
                const startF = cutF - leftF;
                const isSelected = selectedTransitionId === trans.transition_id;
                return (
                  <div
                    key={trans.transition_id}
                    onClick={(event) => {
                      event.stopPropagation();
                      onSelectTransition?.(trans.transition_id);
                      onSelectItem(null);
                    }}
                    onKeyDown={(event) => {
                      if (event.key === "Enter" || event.key === " ") {
                        event.preventDefault();
                        event.stopPropagation();
                        onSelectTransition?.(trans.transition_id);
                        onSelectItem(null);
                      }
                    }}
                    role="button"
                    tabIndex={0}
                    aria-label={`Select audio crossfade between ${leftItem.name} and ${track.items.find((item) => item.item_id === trans.right_item_id)?.name ?? "incoming clip"}, ${durF} frames`}
                    style={{
                      position: "absolute",
                      top: "6px",
                      bottom: "6px",
                      left: `${HEADER_WIDTH + startF * zoom}px`,
                      width: `${Math.max(16, durF * zoom)}px`,
                      background: isSelected ? "rgba(34, 197, 94, 0.95)" : "rgba(22, 163, 74, 0.72)",
                      border: isSelected ? "2px solid #86efac" : "1px solid #4ade80",
                      borderRadius: "3px",
                      cursor: "pointer",
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "center",
                      zIndex: 13,
                      color: "#fff",
                      fontSize: "10px",
                      fontWeight: 700,
                      overflow: "hidden",
                      whiteSpace: "nowrap",
                      userSelect: "none",
                    }}
                    title={`Audio Crossfade: ${durF} frames (click to edit)`}
                  >
                    ◖ {durF}f ◗
                  </div>
                );
              })}

              {track.kind === "Audio" &&
                track.items.map((item, index) => {
                  if (index >= track.items.length - 1) return null;
                  const nextItem = track.items[index + 1];
                  if (!isRangeAdjacent(item.timeline_range, nextItem.timeline_range)) return null;
                  const hasCrossfade = track.audio_transitions?.some(
                    (transition) =>
                      transition.left_item_id === item.item_id &&
                      transition.right_item_id === nextItem.item_id
                  );
                  if (hasCrossfade) return null;
                  const durationFrames = getDefaultAudioCrossfadeDurationFrames(
                    item,
                    nextItem,
                    project.assets,
                    project.settings.frame_rate
                  );
                  const cutF = item.timeline_range.start.value + item.timeline_range.duration.value;
                  return (
                    <button
                      key={`audio-crossfade-${item.item_id}-${nextItem.item_id}`}
                      aria-label={`Create audio crossfade between ${item.name} and ${nextItem.name}`}
                      onClick={(event) => {
                        event.stopPropagation();
                        if (durationFrames > 0) {
                          onCreateAudioCrossfade?.(track.track_id, item.item_id, nextItem.item_id, durationFrames);
                        }
                      }}
                      disabled={durationFrames <= 0}
                      style={{
                        position: "absolute",
                        top: "13px",
                        left: `${HEADER_WIDTH + cutF * zoom - 9}px`,
                        width: "18px",
                        height: "18px",
                        borderRadius: "50%",
                        backgroundColor: "#16a34a",
                        color: "#fff",
                        border: "1px solid #86efac",
                        fontSize: "11px",
                        fontWeight: "bold",
                        cursor: durationFrames > 0 ? "pointer" : "not-allowed",
                        opacity: durationFrames > 0 ? 1 : 0.45,
                        display: "flex",
                        alignItems: "center",
                        justifyContent: "center",
                        zIndex: 15,
                        padding: 0,
                      }}
                      title={durationFrames > 0
                        ? `Create ${durationFrames}-frame audio crossfade between "${item.name}" and "${nextItem.name}".`
                        : "Cannot create an audio crossfade: media duration or usable source handles are unavailable."}
                    >
                      ◖
                    </button>
                  );
                })}

              {/* Seam Cut Handles for Adjacent Video Clips Without Transition */}
              {track.kind === "Video" &&
                track.items.map((item, idx) => {
                  if (idx >= track.items.length - 1) return null;
                  const nextItem = track.items[idx + 1];
                  if (!isRangeAdjacent(item.timeline_range, nextItem.timeline_range)) return null;
                  const cutF = item.timeline_range.start.value + item.timeline_range.duration.value;
                  const hasTrans = track.transitions?.some(
                    (t) => t.left_item_id === item.item_id && t.right_item_id === nextItem.item_id
                  );
                  if (hasTrans) return null;

                  return (
                    <button
                      key={`cut_btn_${item.item_id}_${nextItem.item_id}`}
                      onClick={(e) => {
                        e.stopPropagation();
                        onCreateCutTransition?.(
                          track.track_id,
                          item.item_id,
                          nextItem.item_id,
                          "CrossDissolve",
                          30
                        );
                      }}
                      style={{
                        position: "absolute",
                        top: "13px",
                        left: `${HEADER_WIDTH + cutF * zoom - 9}px`,
                        width: "18px",
                        height: "18px",
                        borderRadius: "50%",
                        backgroundColor: "#f59e0b",
                        color: "#000",
                        border: "1px solid #fbbf24",
                        fontSize: "11px",
                        fontWeight: "bold",
                        cursor: "pointer",
                        display: "flex",
                        alignItems: "center",
                        justifyContent: "center",
                        zIndex: 15,
                        boxShadow: "0 1px 4px rgba(0, 0, 0, 0.6)",
                        padding: 0,
                      }}
                      title={`Add CrossDissolve between "${item.name}" and "${nextItem.name}"`}
                    >
                      ⧓
                    </button>
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
