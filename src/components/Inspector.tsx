import React, { useState, useEffect, useRef } from "react";
import { Asset, AudioCrossfade, CutTransition, KeyframeEasing, KeyframeProperty, TimelineItem } from "../types/project";

interface InspectorProps {
  selectedItem: TimelineItem | null;
  selectedTransition?: CutTransition | null;
  selectedAudioCrossfade?: AudioCrossfade | null;
  selectedCount?: number;
  adjacentInfo?: {
    prevClip?: TimelineItem;
    nextClip?: TimelineItem;
    outgoingTransition?: CutTransition;
    incomingTransition?: CutTransition;
    trackId?: string;
  };
  assets?: Asset[];
  playheadFrame?: number;
  onUpdateTransform: (x: number, y: number, scale: number, rotation: number) => void;
  onUpdateOpacity: (opacity: number) => void;
  onUpdateVolume?: (volume: number) => void;
  onUpdateText?: (text: string) => void;
  onUpdateAudioFade?: (fadeInFrames: number, fadeOutFrames: number) => void;
  onUpdateTransition?: (
    kind: "FadeIn" | "FadeOut" | "CrossDissolve" | "FadeBlack" | null,
    durationFrames: number
  ) => void;
  onAddKeyframe?: (property: KeyframeProperty, offsetFrame: number, value: number) => void;
  onRemoveKeyframe?: (property: KeyframeProperty, offsetFrame: number) => void;
  onSetKeyframeEasing?: (property: KeyframeProperty, offsetFrame: number, easing: KeyframeEasing) => void;
  onSeekPlayhead?: (frame: number) => void;
  onCreateCutTransition?: (trackId: string, leftItemId: string, rightItemId: string, durationFrames: number) => void;
  onUpdateCutTransitionDuration?: (transitionId: string, newDurationFrames: number) => void;
  onRemoveCutTransition?: (transitionId: string) => void;
  onUpdateAudioCrossfadeDuration?: (transitionId: string, newDurationFrames: number) => void;
  onRemoveAudioCrossfade?: (transitionId: string) => void;
}

export const Inspector: React.FC<InspectorProps> = ({
  selectedItem,
  selectedTransition,
  selectedAudioCrossfade,
  selectedCount,
  adjacentInfo,
  assets,
  playheadFrame = 0,
  onUpdateTransform,
  onUpdateOpacity,
  onUpdateVolume,
  onUpdateText,
  onUpdateAudioFade,
  onUpdateTransition,
  onAddKeyframe,
  onRemoveKeyframe,
  onSetKeyframeEasing,
  onSeekPlayhead,
  onCreateCutTransition,
  onUpdateCutTransitionDuration,
  onRemoveCutTransition,
  onUpdateAudioCrossfadeDuration,
  onRemoveAudioCrossfade,
}) => {
  if (selectedAudioCrossfade) {
    return (
      <aside className="right-panel">
        <div className="panel-header">Inspector: Audio Crossfade</div>
        <div className="panel-content" style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
          <section>
            <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)", marginBottom: "4px" }}>
              AUDIO CROSSFADE
            </div>
            <div style={{ fontSize: "12px", color: "var(--text-main)", display: "flex", flexDirection: "column", gap: "4px" }}>
              <div><strong>Curve:</strong> Equal-power</div>
              <div><strong>Alignment:</strong> Centered cut</div>
              <div><strong>Outgoing:</strong> {selectedAudioCrossfade.left_item_id}</div>
              <div><strong>Incoming:</strong> {selectedAudioCrossfade.right_item_id}</div>
            </div>
          </section>
          <label style={{ display: "flex", flexDirection: "column", gap: "6px", fontSize: "11px", color: "var(--text-muted)" }}>
            DURATION (FRAMES)
            <input
              type="number"
              min="1"
              value={selectedAudioCrossfade.duration.value}
              onChange={(event) => {
                const duration = Number.parseInt(event.target.value, 10);
                if (Number.isFinite(duration) && duration > 0) {
                  onUpdateAudioCrossfadeDuration?.(selectedAudioCrossfade.transition_id, duration);
                }
              }}
              style={{ width: "90px", padding: "4px", fontSize: "12px" }}
            />
          </label>
          <button
            onClick={() => onRemoveAudioCrossfade?.(selectedAudioCrossfade.transition_id)}
            style={{
              width: "100%",
              padding: "6px 12px",
              backgroundColor: "#dc2626",
              color: "#fff",
              border: "none",
              borderRadius: "4px",
              fontSize: "12px",
              fontWeight: 600,
              cursor: "pointer",
            }}
          >
            Remove Audio Crossfade
          </button>
        </div>
      </aside>
    );
  }

  if (selectedTransition) {
    return (
      <aside className="right-panel">
        <div className="panel-header">Inspector: CrossDissolve</div>
        <div className="panel-content" style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
          <section>
            <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)", marginBottom: "4px" }}>
              CROSS DISSOLVE
            </div>
            <div style={{ fontSize: "12px", color: "var(--text-main)", display: "flex", flexDirection: "column", gap: "4px" }}>
              <div><strong>Alignment:</strong> Centered Cut</div>
              <div><strong>Duration:</strong> {selectedTransition.duration.value} frames (~{(selectedTransition.duration.value / 30).toFixed(2)}s)</div>
              <div><strong>Left Clip:</strong> {selectedTransition.left_item_id}</div>
              <div><strong>Right Clip:</strong> {selectedTransition.right_item_id}</div>
            </div>
          </section>

          <section style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
            <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)" }}>
              DURATION (FRAMES)
            </div>
            <input
              type="number"
              min="1"
              value={selectedTransition.duration.value}
              onChange={(e) => {
                const dur = Math.max(1, parseInt(e.target.value, 10) || 1);
                onUpdateCutTransitionDuration?.(selectedTransition.transition_id, dur);
              }}
              style={{ width: "80px", padding: "4px", fontSize: "12px" }}
            />
          </section>

          <section>
            <button
              onClick={() => onRemoveCutTransition?.(selectedTransition.transition_id)}
              style={{
                width: "100%",
                padding: "6px 12px",
                backgroundColor: "#dc2626",
                color: "#fff",
                border: "none",
                borderRadius: "4px",
                fontSize: "12px",
                fontWeight: 600,
                cursor: "pointer",
              }}
            >
              Remove CrossDissolve
            </button>
          </section>
        </div>
      </aside>
    );
  }

  if (!selectedItem) {
    return (
      <aside className="right-panel">
        <div className="panel-header">Inspector</div>
        <div className="panel-content" style={{ textAlign: "center", marginTop: "24px" }}>
          <p>No clip or transition selected.</p>
          <p style={{ fontSize: "11px", color: "var(--text-muted)" }}>
            Select a clip or transition on the timeline to inspect and edit properties.
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

  const clipStart = timeline_range.start.value;
  const currentOffset = playheadFrame - clipStart;
  const isInside = currentOffset >= 0 && currentOffset < timeline_range.duration.value;

  const hasKfAtOffset = (property: KeyframeProperty) => {
    return (selectedItem.keyframes || []).some(
      (k) => k.property === property && k.offset.value === currentOffset
    );
  };

  const renderKeyframeControl = (property: KeyframeProperty, currentValue: number) => {
    if (!onAddKeyframe || !onRemoveKeyframe) return null;
    const kfs = (selectedItem.keyframes || [])
      .filter((k) => k.property === property)
      .sort((a, b) => a.offset.value - b.offset.value);
    const activeKf = kfs.find((k) => k.offset.value === currentOffset);
    const hasKf = !!activeKf;

    const prevKf = [...kfs].reverse().find((k) => k.offset.value < currentOffset);
    const nextKf = kfs.find((k) => k.offset.value > currentOffset);

    return (
      <div style={{ display: "flex", gap: "2px", alignItems: "center" }}>
        <button
          className="btn-kf"
          disabled={!prevKf}
          onClick={() => prevKf && onSeekPlayhead && onSeekPlayhead(clipStart + prevKf.offset.value)}
          title={prevKf ? `Jump to previous keyframe (+${prevKf.offset.value}f)` : "No earlier keyframe"}
        >
          ⏴
        </button>
        <button
          className={`btn-kf ${hasKf ? "active" : ""}`}
          disabled={!isInside}
          onClick={() => {
            if (hasKf) {
              onRemoveKeyframe(property, currentOffset);
            } else {
              onAddKeyframe(property, currentOffset, currentValue);
            }
          }}
          title={
            !isInside
              ? "Move playhead inside clip to add keyframe"
              : hasKf
              ? `Remove keyframe at offset +${currentOffset}f`
              : `Add keyframe at offset +${currentOffset}f`
          }
        >
          <span className={`kf-diamond ${hasKf ? "filled" : ""}`} />
        </button>
        <button
          className="btn-kf"
          disabled={!nextKf}
          onClick={() => nextKf && onSeekPlayhead && onSeekPlayhead(clipStart + nextKf.offset.value)}
          title={nextKf ? `Jump to next keyframe (+${nextKf.offset.value}f)` : "No later keyframe"}
        >
          ⏵
        </button>
      </div>
    );
  };

  const renderKeyframeEasingControl = (property: KeyframeProperty) => {
    if (!onSetKeyframeEasing) return null;
    const kfs = (selectedItem.keyframes || [])
      .filter((k) => k.property === property)
      .sort((a, b) => a.offset.value - b.offset.value);
    const activeKf = kfs.find((k) => k.offset.value === currentOffset);
    if (!activeKf) return null;

    const isLastKf = kfs[kfs.length - 1].offset.value === activeKf.offset.value;
    const currentEasing = activeKf.outgoing_easing ?? "Linear";
    const easingType: "Linear" | "EaseIn" | "EaseOut" | "EaseInOut" | "Custom" =
      typeof currentEasing === "string"
        ? currentEasing
        : "CubicBezier" in currentEasing
        ? "Custom"
        : "Linear";

    const cubicParams =
      typeof currentEasing === "object" && "CubicBezier" in currentEasing
        ? currentEasing.CubicBezier
        : { x1: 0.25, y1: 0.1, x2: 0.25, y2: 1.0 };

    return (
      <div
        className="kf-easing-container"
        style={{
          display: "flex",
          flexDirection: "column",
          gap: "4px",
          marginTop: "4px",
          marginBottom: "6px",
          padding: "6px 8px",
          backgroundColor: "var(--bg-secondary)",
          border: "1px solid var(--border-color)",
          borderRadius: "4px",
          fontSize: "11px",
        }}
      >
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <span style={{ color: "var(--text-muted)", fontWeight: 600 }}>
            Outgoing Easing (+{activeKf.offset.value}f):
          </span>
          {isLastKf ? (
            <span
              style={{ color: "var(--text-muted)", fontStyle: "italic", fontSize: "11px" }}
              title="Terminal keyframe cannot have an outgoing transition"
            >
              Linear (Hold/Terminal)
            </span>
          ) : (
            <select
              aria-label={`${property} Keyframe Easing`}
              value={easingType}
              onChange={(e) => {
                const val = e.target.value;
                if (val === "Linear" || val === "EaseIn" || val === "EaseOut" || val === "EaseInOut") {
                  onSetKeyframeEasing(property, activeKf.offset.value, val);
                } else if (val === "Custom") {
                  onSetKeyframeEasing(property, activeKf.offset.value, {
                    CubicBezier: cubicParams,
                  });
                }
              }}
              style={{
                padding: "2px 4px",
                fontSize: "11px",
                backgroundColor: "var(--bg-surface)",
                color: "var(--text-main)",
                border: "1px solid var(--border-color)",
                borderRadius: "3px",
              }}
            >
              <option value="Linear">Linear</option>
              <option value="EaseIn">Ease In</option>
              <option value="EaseOut">Ease Out</option>
              <option value="EaseInOut">Ease In Out</option>
              <option value="Custom">Custom (Bezier)</option>
            </select>
          )}
        </div>

        {!isLastKf && easingType === "Custom" && (
          <div style={{ display: "flex", gap: "6px", alignItems: "center", marginTop: "4px" }}>
            {(["x1", "y1", "x2", "y2"] as const).map((param) => (
              <div key={param} style={{ display: "flex", flexDirection: "column", flex: 1, gap: "2px" }}>
                <label style={{ fontSize: "10px", color: "var(--text-muted)" }}>{param}:</label>
                <input
                  type="number"
                  aria-label={`${property} Custom ${param}`}
                  min="0"
                  max="1"
                  step="0.05"
                  value={cubicParams[param]}
                  onChange={(e) => {
                    const rawVal = parseFloat(e.target.value);
                    const clamped = isNaN(rawVal) ? 0 : Math.max(0, Math.min(1, rawVal));
                    onSetKeyframeEasing(property, activeKf.offset.value, {
                      CubicBezier: {
                        ...cubicParams,
                        [param]: clamped,
                      },
                    });
                  }}
                  style={{
                    width: "100%",
                    padding: "2px 4px",
                    fontSize: "11px",
                    backgroundColor: "var(--bg-surface)",
                    color: "var(--text-main)",
                    border: "1px solid var(--border-color)",
                    borderRadius: "3px",
                  }}
                />
              </div>
            ))}
          </div>
        )}
      </div>
    );
  };

  return (
    <aside className="right-panel">
      <div className="panel-header" style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <span>Inspector: {selectedItem.name}</span>
        {selectedCount && selectedCount > 1 ? (
          <span
            style={{
              fontSize: "10px",
              backgroundColor: "var(--accent-primary, #3b82f6)",
              padding: "2px 6px",
              borderRadius: "8px",
              color: "#fff",
              fontWeight: 600,
            }}
          >
            {selectedCount} clips selected (Primary)
          </span>
        ) : null}
      </div>
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
            {isInside && (
              <div style={{ color: "var(--accent-warning)", fontSize: "11px", marginTop: "2px" }}>
                Playhead at clip offset: +{currentOffset}f
              </div>
            )}
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

        {/* Transform controls with keyframing */}
        <section style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
          <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)" }}>
            TRANSFORM (WITH KEYFRAMES)
          </div>

          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
            <label style={{ fontSize: "12px" }}>Position X:</label>
            <div style={{ display: "flex", gap: "6px", alignItems: "center" }}>
              <input
                type="number"
                value={transform.position_x}
                onChange={(e) => {
                  const val = parseFloat(e.target.value) || 0;
                  onUpdateTransform(val, transform.position_y, transform.scale_x, transform.rotation);
                  if (hasKfAtOffset("PositionX") && onAddKeyframe) {
                    onAddKeyframe("PositionX", currentOffset, val);
                  }
                }}
                style={{ width: "70px", padding: "4px", fontSize: "12px" }}
              />
              {renderKeyframeControl("PositionX", transform.position_x)}
            </div>
          </div>
          {renderKeyframeEasingControl("PositionX")}

          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
            <label style={{ fontSize: "12px" }}>Position Y:</label>
            <div style={{ display: "flex", gap: "6px", alignItems: "center" }}>
              <input
                type="number"
                value={transform.position_y}
                onChange={(e) => {
                  const val = parseFloat(e.target.value) || 0;
                  onUpdateTransform(transform.position_x, val, transform.scale_x, transform.rotation);
                  if (hasKfAtOffset("PositionY") && onAddKeyframe) {
                    onAddKeyframe("PositionY", currentOffset, val);
                  }
                }}
                style={{ width: "70px", padding: "4px", fontSize: "12px" }}
              />
              {renderKeyframeControl("PositionY", transform.position_y)}
            </div>
          </div>
          {renderKeyframeEasingControl("PositionY")}

          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
            <label style={{ fontSize: "12px" }}>Scale:</label>
            <div style={{ display: "flex", gap: "6px", alignItems: "center" }}>
              <input
                type="number"
                step="0.1"
                min="0.1"
                max="10"
                value={transform.scale_x}
                onChange={(e) => {
                  const val = parseFloat(e.target.value) || 1.0;
                  onUpdateTransform(transform.position_x, transform.position_y, val, transform.rotation);
                  if (hasKfAtOffset("ScaleX") && onAddKeyframe) {
                    onAddKeyframe("ScaleX", currentOffset, val);
                  }
                }}
                style={{ width: "70px", padding: "4px", fontSize: "12px" }}
              />
              {renderKeyframeControl("ScaleX", transform.scale_x)}
            </div>
          </div>
          {renderKeyframeEasingControl("ScaleX")}

          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
            <label style={{ fontSize: "12px" }}>Rotation (°):</label>
            <div style={{ display: "flex", gap: "6px", alignItems: "center" }}>
              <input
                type="number"
                value={transform.rotation}
                onChange={(e) => {
                  const val = parseFloat(e.target.value) || 0;
                  onUpdateTransform(transform.position_x, transform.position_y, transform.scale_x, val);
                  if (hasKfAtOffset("Rotation") && onAddKeyframe) {
                    onAddKeyframe("Rotation", currentOffset, val);
                  }
                }}
                style={{ width: "70px", padding: "4px", fontSize: "12px" }}
              />
              {renderKeyframeControl("Rotation", transform.rotation)}
            </div>
          </div>
          {renderKeyframeEasingControl("Rotation")}
        </section>

        {/* Opacity control with keyframing */}
        <section style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)" }}>
              OPACITY ({Math.round(opacity * 100)}%)
            </div>
            {renderKeyframeControl("Opacity", opacity)}
          </div>
          <input
            type="range"
            min="0"
            max="1"
            step="0.05"
            value={opacity}
            onChange={(e) => {
              const val = parseFloat(e.target.value);
              onUpdateOpacity(val);
              if (hasKfAtOffset("Opacity") && onAddKeyframe) {
                onAddKeyframe("Opacity", currentOffset, val);
              }
            }}
            style={{ width: "100%" }}
          />
          {renderKeyframeEasingControl("Opacity")}
        </section>

        {/* Visual Transitions (FadeIn, FadeOut, CrossDissolve, FadeBlack) */}
        {onUpdateTransition && (selectedItem.kind === "Video" || selectedItem.kind === "Image") && (
          <section style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
            <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)" }}>
              TRANSITION
            </div>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <label style={{ fontSize: "12px" }}>Type:</label>
              <select
                aria-label="Transition Type"
                value={selectedItem.transition?.kind ?? "None"}
                onChange={(e) => {
                  const val = e.target.value;
                  if (val === "None") {
                    onUpdateTransition(null, selectedItem.transition?.duration.value ?? 30);
                  } else if (val === "FadeIn" || val === "FadeOut" || val === "FadeBlack") {
                    onUpdateTransition(
                      val,
                      selectedItem.transition?.duration.value ?? 30
                    );
                  }
                }}
                style={{
                  width: "120px",
                  padding: "4px",
                  fontSize: "12px",
                  background: "var(--bg-secondary)",
                  color: "var(--text-primary)",
                  border: "1px solid var(--border-color)",
                  borderRadius: "4px",
                }}
              >
                <option value="None">None</option>
                <option value="FadeIn">Fade In</option>
                <option value="FadeOut">Fade Out</option>
                <option value="CrossDissolve" disabled>
                  Cross Dissolve (Deferred)
                </option>
                <option value="FadeBlack">Fade Black</option>
              </select>
            </div>
            {selectedItem.transition && (
              <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                <label style={{ fontSize: "12px" }}>Duration (f):</label>
                <input
                  type="number"
                  aria-label="Transition Duration"
                  min="1"
                  max={selectedItem.timeline_range.duration.value}
                  value={selectedItem.transition.duration.value}
                  onChange={(e) => {
                    const dur = Math.max(1, parseInt(e.target.value, 10) || 1);
                    onUpdateTransition(selectedItem.transition!.kind, dur);
                  }}
                  style={{ width: "80px", padding: "4px", fontSize: "12px" }}
                />
              </div>
            )}
          </section>
        )}

        {/* Cut Transitions (CrossDissolve) */}
        {(adjacentInfo?.nextClip || adjacentInfo?.prevClip) && selectedItem.kind === "Video" && (
          <section style={{ display: "flex", flexDirection: "column", gap: "8px", borderTop: "1px solid var(--border-color)", paddingTop: "12px" }}>
            <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)" }}>
              CROSS DISSOLVE (CUT TRANSITION)
            </div>
            {adjacentInfo?.outgoingTransition ? (
              <div style={{ display: "flex", flexDirection: "column", gap: "6px", backgroundColor: "var(--bg-secondary)", padding: "8px", borderRadius: "4px" }}>
                <div style={{ fontSize: "12px", display: "flex", justifyContent: "space-between" }}>
                  <span>With next clip: <strong>{adjacentInfo.nextClip?.name}</strong></span>
                  <span style={{ color: "#f59e0b", fontWeight: 600 }}>Active</span>
                </div>
                <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                  <label style={{ fontSize: "11px" }}>Duration (f):</label>
                  <input
                    type="number"
                    min="1"
                    value={adjacentInfo.outgoingTransition.duration.value}
                    onChange={(e) => {
                      const dur = Math.max(1, parseInt(e.target.value, 10) || 1);
                      onUpdateCutTransitionDuration?.(adjacentInfo.outgoingTransition!.transition_id, dur);
                    }}
                    style={{ width: "70px", padding: "3px", fontSize: "12px" }}
                  />
                </div>
                <button
                  onClick={() => onRemoveCutTransition?.(adjacentInfo.outgoingTransition!.transition_id)}
                  style={{
                    padding: "4px 8px",
                    backgroundColor: "#dc2626",
                    color: "#fff",
                    border: "none",
                    borderRadius: "3px",
                    fontSize: "11px",
                    cursor: "pointer",
                    marginTop: "4px",
                  }}
                >
                  Remove CrossDissolve
                </button>
              </div>
            ) : adjacentInfo?.nextClip && adjacentInfo.nextClip.kind === "Video" ? (
              <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
                <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>
                  Adjacent to <strong>{adjacentInfo.nextClip.name}</strong>
                </div>
                <button
                  onClick={() =>
                    onCreateCutTransition?.(
                      adjacentInfo.trackId!,
                      selectedItem.item_id,
                      adjacentInfo.nextClip!.item_id,
                      30
                    )
                  }
                  style={{
                    padding: "5px 10px",
                    backgroundColor: "#f59e0b",
                    color: "#000",
                    border: "none",
                    borderRadius: "4px",
                    fontSize: "11px",
                    fontWeight: 600,
                    cursor: "pointer",
                  }}
                >
                  + Add CrossDissolve (30f)
                </button>
              </div>
            ) : null}
          </section>
        )}

        {/* Volume control with keyframing if Video or Audio */}
        {onUpdateVolume && (selectedItem.kind === "Video" || selectedItem.kind === "Audio") && (
          <section style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)" }}>
                VOLUME ({Math.round((selectedItem.volume ?? 1.0) * 100)}%)
              </div>
              {renderKeyframeControl("Volume", selectedItem.volume ?? 1.0)}
            </div>
            <input
              type="range"
              min="0"
              max="2"
              step="0.05"
              value={selectedItem.volume ?? 1.0}
              onChange={(e) => {
                const val = parseFloat(e.target.value);
                onUpdateVolume(val);
                if (hasKfAtOffset("Volume") && onAddKeyframe) {
                  onAddKeyframe("Volume", currentOffset, val);
                }
              }}
              style={{ width: "100%" }}
            />
            {renderKeyframeEasingControl("Volume")}
          </section>
        )}

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
