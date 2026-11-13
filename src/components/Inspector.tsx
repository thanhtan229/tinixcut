import React, { useState, useEffect, useRef } from "react";
import { dbToLinearGain, formatGainDb, linearGainToDb, MAX_AUDIO_GAIN_DB, MIN_AUDIO_GAIN_DB } from "./audioGain";
import {
  Asset,
  AudioCrossfade,
  CutTransition,
  KeyframeEasing,
  KeyframeProperty,
  TextAlignment,
  TextAnchor,
  TextStyle,
  TextFill,
  TextStroke,
  TextShadow,
  TextBackgroundBox,
  TextAnimation,
  GradientKind,
  PlacementMode,
  ClipSpeed,
  Crop,
  FrameRate,
  rationalTimeToFrames,
  rationalTimeToSeconds,
  TimelineItem,
  TransitionKind,
  VisualEffectPreset,
  ChromaKey,
  Ducking,
  formatClipSpeed,
  parseSpeedInput,
  ColorAdjustments,
} from "../types/project";
import { TemplateLayer, TemplateParameterValue } from "../types/template";
import { ScopesPanel } from "./ScopesPanel";
import { ChaptersSection } from "./ChaptersSection";
import { TemplateInstanceInspector } from "./TemplateInstanceInspector";
import { shouldResetInspectorDrafts } from "./inspectorDraftSync";

const DEFAULT_TEXT_STYLE: TextStyle = {
  font_family: "Inter",
  font_size: 48,
  font_weight: 700,
  font_color: "#FFFFFF",
  accent_color: null,
  background_color: null,
  background_opacity: 0,
  background_corner_radius: 0,
  padding_x: 0,
  padding_y: 0,
  alignment: "Center",
  line_height: 1,
  anchor: "Center",
};

interface InspectorProps {
  projectRevision: number;
  displayFrameRate: FrameRate;
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
  onUpdateCanvasPosition?: (itemId: string, x: number, y: number) => void;
  onUpdateCrop?: (crop: Crop) => void;
  onUpdatePlacementMode?: (placementMode: PlacementMode) => void;
  onUpdateOpacity: (opacity: number) => void;
  onUpdateVolume?: (volume: number) => void;
  onSetEmbeddedAudioEnabled?: (enabled: boolean) => void;
  onDetachEmbeddedAudio?: () => void;
  onUnlinkEmbeddedAudio?: () => void;
  onUpdateText?: (text: string) => void;
  onApplyTextTemplate?: (templateId: string, templateVersion?: number) => void;
  onApplyTextTemplateToTrack?: (
    trackId: string,
    templateId: string,
    templateVersion?: number
  ) => void;
  onResetTextTemplate?: () => void;
  onTemplateParameterSet?: (parameterKey: string, value: TemplateParameterValue) => void;
  onTemplateReset?: () => void;
  onTemplateDetach?: () => void;
  onTemplateLayerAdd?: (layer: TemplateLayer) => void;
  onTemplateLayerRemove?: (layerId: string) => void;
  onTemplateLayerReorder?: (layerIds: string[]) => void;
  onTemplateLayerUpdate?: (layerId: string, patch: Partial<TemplateLayer>) => Promise<unknown> | void;
  onSaveAsTemplate?: () => void;
  onUpdateTextStyle?: (style: TextStyle) => void;
  onApplyTextStyleToTrack?: (trackId: string, style: TextStyle) => void;
  onUpdateTextAppearance?: (
    fill: TextFill,
    strokes: TextStroke[],
    shadows: TextShadow[],
    background?: TextBackgroundBox | null
  ) => void;
  onUpdateTextAnimation?: (animation?: TextAnimation | null) => void;
  textTrack?: {
    trackId: string;
    name: string;
    textItemCount: number;
  } | null;
  onUpdateAudioFade?: (fadeInFrames: number, fadeOutFrames: number) => void;
  onUpdateTransition?: (
    kind: TransitionKind | null,
    durationFrames: number
  ) => void;
  onUpdateVisualEffect?: (visualEffect: VisualEffectPreset) => void;
  onUpdateColorAdjustments?: (adjustments: ColorAdjustments) => void;
  onUpdateChromaKey?: (chromaKey: ChromaKey | null) => void;
  onUpdateDucking?: (ducking: Ducking | null) => void;
  onUpdateReverse?: (reverse: boolean) => void;
  onChooseLut?: () => void;
  onClearLut?: () => void;
  onUpdateSpeed?: (speed: ClipSpeed) => void;
  onAddKeyframe?: (property: KeyframeProperty, offsetFrame: number, value: number) => void;
  onRemoveKeyframe?: (property: KeyframeProperty, offsetFrame: number) => void;
  onSetKeyframeEasing?: (property: KeyframeProperty, offsetFrame: number, easing: KeyframeEasing) => void;
  onSeekPlayhead?: (frame: number) => void;
  onCreateCutTransition?: (
    trackId: string,
    leftItemId: string,
    rightItemId: string,
    kind: "CrossDissolve" | "DipToBlack",
    durationFrames: number
  ) => void;
  onUpdateCutTransitionDuration?: (transitionId: string, newDurationFrames: number) => void;
  onRemoveCutTransition?: (transitionId: string) => void;
  onUpdateAudioCrossfadeDuration?: (transitionId: string, newDurationFrames: number) => void;
  onRemoveAudioCrossfade?: (transitionId: string) => void;
}

export const Inspector: React.FC<InspectorProps> = ({
  projectRevision,
  displayFrameRate,
  selectedItem,
  selectedTransition,
  selectedAudioCrossfade,
  selectedCount,
  adjacentInfo,
  assets,
  playheadFrame = 0,
  onUpdateTransform,
  onUpdateCanvasPosition,
  onUpdateCrop,
  onUpdatePlacementMode,
  onUpdateOpacity,
  onUpdateVolume,
  onSetEmbeddedAudioEnabled,
  onDetachEmbeddedAudio,
  onUnlinkEmbeddedAudio,
  onUpdateText,
  onApplyTextTemplate,
  onApplyTextTemplateToTrack,
  onResetTextTemplate,
  onTemplateParameterSet,
  onTemplateReset,
  onTemplateDetach,
  onTemplateLayerAdd,
  onTemplateLayerRemove,
  onTemplateLayerReorder,
  onTemplateLayerUpdate,
  onSaveAsTemplate,
  onUpdateTextStyle,
  onApplyTextStyleToTrack,
  onUpdateTextAppearance,
  onUpdateTextAnimation,
  textTrack,
  onUpdateAudioFade,
  onUpdateTransition,
  onUpdateVisualEffect,
  onUpdateColorAdjustments,
  onUpdateChromaKey,
  onUpdateDucking,
  onUpdateReverse,
  onChooseLut,
  onClearLut,
  onUpdateSpeed,
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
  const [audioCrossfadeDurationDraft, setAudioCrossfadeDurationDraft] = useState("");
  const [scopesOpen, setScopesOpen] = useState(false);
  const [customSpeedDraft, setCustomSpeedDraft] = useState("");
  const [customSpeedError, setCustomSpeedError] = useState<string | null>(null);
  const [localText, setLocalText] = useState(selectedItem?.text_content ?? "");
  const [localCrop, setLocalCrop] = useState<Crop>(selectedItem?.crop ?? { top: 0, bottom: 0, left: 0, right: 0 });
  const [applyTemplateToTrack, setApplyTemplateToTrack] = useState(false);
  const [appearanceExpanded, setAppearanceExpanded] = useState(false);
  const [animationExpanded, setAnimationExpanded] = useState(false);
  const debounceTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const draftIdentityRef = useRef<{
    revision: number;
    itemId?: string;
    canonicalText: string;
  } | null>(null);

  useEffect(() => {
    setAudioCrossfadeDurationDraft(
      selectedAudioCrossfade ? String(selectedAudioCrossfade.duration.value) : ""
    );
  }, [selectedAudioCrossfade?.transition_id, selectedAudioCrossfade?.duration.value]);

  useEffect(() => {
    setLocalCrop(selectedItem?.crop ?? { top: 0, bottom: 0, left: 0, right: 0 });
  }, [selectedItem?.item_id, projectRevision]);

  useEffect(() => {
    const previous = draftIdentityRef.current;
    if (
      !previous ||
      previous.canonicalText !== (selectedItem?.text_content ?? "") ||
      shouldResetInspectorDrafts(
        previous.revision,
        projectRevision,
        previous.itemId,
        selectedItem?.item_id
      )
    ) {
      if (debounceTimerRef.current) {
        clearTimeout(debounceTimerRef.current);
        debounceTimerRef.current = null;
      }
      setLocalText(selectedItem?.text_content ?? "");
    }
    draftIdentityRef.current = {
      revision: projectRevision,
      itemId: selectedItem?.item_id,
      canonicalText: selectedItem?.text_content ?? "",
    };
  }, [projectRevision, selectedItem?.item_id, selectedItem?.text_content]);

  useEffect(() => {
    return () => {
      if (debounceTimerRef.current) clearTimeout(debounceTimerRef.current);
    };
  }, []);

  useEffect(() => {
    setApplyTemplateToTrack(false);
  }, [selectedItem?.item_id]);

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
              value={audioCrossfadeDurationDraft}
              onChange={(event) => {
                setAudioCrossfadeDurationDraft(event.target.value);
              }}
              onBlur={(event) => {
                if (event.currentTarget.dataset.cancelCommit === "true") {
                  delete event.currentTarget.dataset.cancelCommit;
                  setAudioCrossfadeDurationDraft(String(selectedAudioCrossfade.duration.value));
                  return;
                }
                const duration = Number.parseInt(audioCrossfadeDurationDraft, 10);
                if (Number.isFinite(duration) && duration > 0) {
                  if (duration !== selectedAudioCrossfade.duration.value) {
                    onUpdateAudioCrossfadeDuration?.(selectedAudioCrossfade.transition_id, duration);
                  }
                } else {
                  setAudioCrossfadeDurationDraft(String(selectedAudioCrossfade.duration.value));
                }
              }}
              onKeyDown={(event) => {
                if (event.key === "Enter") event.currentTarget.blur();
                if (event.key === "Escape") {
                  event.preventDefault();
                  event.currentTarget.dataset.cancelCommit = "true";
                  setAudioCrossfadeDurationDraft(String(selectedAudioCrossfade.duration.value));
                  event.currentTarget.blur();
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
    const transitionName =
      selectedTransition.kind === "DipToBlack" ? "Dip to Black" : "Cross Dissolve";

    return (
      <aside className="right-panel">
        <div className="panel-header">Inspector: {transitionName}</div>
        <div className="panel-content" style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
          <section>
            <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)", marginBottom: "4px" }}>
              {transitionName.toUpperCase()}
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
              Remove {transitionName}
            </button>
          </section>
        </div>
      </aside>
    );
  }

  const scopesSection = (
    <section style={{ display: "flex", flexDirection: "column", gap: "6px", marginTop: "12px" }}>
      <label style={{ display: "flex", gap: "6px", alignItems: "center", fontSize: "11px" }}>
        <input
          type="checkbox"
          aria-label="Show video scopes"
          checked={scopesOpen}
          onChange={(event) => setScopesOpen(event.currentTarget.checked)}
        />
        Show video scopes
      </label>
      {scopesOpen && <ScopesPanel frame={playheadFrame} revision={projectRevision} />}
      <ChaptersSection />
    </section>
  );

  if (!selectedItem) {
    return (
      <aside className="right-panel">
        <div className="panel-header">Inspector</div>
        <div className="panel-content" style={{ textAlign: "center", marginTop: "24px" }}>
          <p>No clip or transition selected.</p>
          <p style={{ fontSize: "11px", color: "var(--text-muted)" }}>
            Select a clip or transition on the timeline to inspect and edit properties.
          </p>
          <div style={{ textAlign: "left" }}>{scopesSection}</div>
        </div>
      </aside>
    );
  }

  const { transform, opacity, timeline_range, source_range } = selectedItem;

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
            <div>
              Source Start: {rationalTimeToSeconds(source_range.start).toFixed(3)}s
              {` (${rationalTimeToFrames(source_range.start, displayFrameRate)}f at project fps)`}
            </div>
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

        {selectedItem.kind === "Text" && onApplyTextTemplate && (
          <section style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
            <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)" }}>
              TEXT TEMPLATES
            </div>
            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "6px" }}>
              {[
                ["clean-lower-third", "Lower third"],
                ["bold-headline", "Bold headline"],
                ["center-pop", "Center pop"],
                ["subtitle-card", "Subtitle card"],
                ["quote-card", "Quote card"],
                ["call-to-action", "Call to action"],
              ].map(([templateId, label]) => (
                <button
                  key={templateId}
                  className="btn-action"
                  type="button"
                  onClick={() => {
                    if (applyTemplateToTrack && textTrack && onApplyTextTemplateToTrack) {
                      onApplyTextTemplateToTrack(textTrack.trackId, templateId);
                      setApplyTemplateToTrack(false);
                    } else {
                      onApplyTextTemplate(templateId);
                    }
                  }}
                  style={{ padding: "6px 7px", fontSize: "10px", textAlign: "left" }}
                >
                  {label}
                </button>
              ))}
            </div>
            {textTrack && textTrack.textItemCount > 1 && onApplyTextTemplateToTrack && (
              <label
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: "7px",
                  fontSize: "10px",
                  color: "var(--text-muted)",
                }}
              >
                <input
                  type="checkbox"
                  checked={applyTemplateToTrack}
                  onChange={(event) => setApplyTemplateToTrack(event.target.checked)}
                />
                Apply next template to all {textTrack.textItemCount} text clips on {textTrack.name}
              </label>
            )}
            {selectedItem.text_template && (
              <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px" }}>
                <div style={{ fontSize: "10px", color: "var(--text-muted)" }}>
                  Active: {selectedItem.text_template.template_id} v{selectedItem.text_template.template_version}
                </div>
                {onResetTextTemplate && (
                  <button
                    className="btn-action"
                    type="button"
                    onClick={onResetTextTemplate}
                    style={{ padding: "4px 7px", fontSize: "10px" }}
                  >
                    Reset
                  </button>
                )}
              </div>
            )}
          </section>
        )}

        {selectedItem.kind === "Text" && (
          <TemplateInstanceInspector
            item={selectedItem}
            projectRevision={projectRevision}
            onParameterSet={onTemplateParameterSet}
            onReset={onTemplateReset || onResetTextTemplate}
            onDetach={onTemplateDetach}
            onLayerAdd={onTemplateLayerAdd}
            onLayerRemove={onTemplateLayerRemove}
            onLayerReorder={onTemplateLayerReorder}
            onLayerUpdate={onTemplateLayerUpdate}
            onSaveAsTemplate={onSaveAsTemplate}
          />
        )}

        {selectedItem.kind === "Text" && onUpdateTextStyle && (
          <section style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
            <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)" }}>
              TEXT STYLE
            </div>
            {(() => {
              const style = selectedItem.text_style ?? DEFAULT_TEXT_STYLE;
              const update = (patch: Partial<TextStyle>) => onUpdateTextStyle({ ...style, ...patch });
              const alignmentOptions: TextAlignment[] = ["Left", "Center", "Right"];
              const anchorOptions: TextAnchor[] = [
                "TopLeft", "TopCenter", "TopRight",
                "MiddleLeft", "Center", "MiddleRight",
                "BottomLeft", "BottomCenter", "BottomRight",
              ];
              return (
                <>
                  <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "8px" }}>
                    <label style={{ display: "flex", flexDirection: "column", gap: "4px", fontSize: "11px", color: "var(--text-muted)" }}>
                      FONT SIZE
                      <input
                        type="number"
                        min="8"
                        max="240"
                        value={style.font_size}
                        onChange={(event) => {
                          const value = Math.max(8, Math.min(240, Number(event.target.value) || 8));
                          update({ font_size: value });
                        }}
                        style={{ width: "100%", padding: "4px", fontSize: "12px" }}
                      />
                    </label>
                    <label style={{ display: "flex", flexDirection: "column", gap: "4px", fontSize: "11px", color: "var(--text-muted)" }}>
                      TEXT COLOR
                      <input
                        type="color"
                        value={style.font_color.slice(0, 7)}
                        onChange={(event) => update({ font_color: event.target.value.toUpperCase() })}
                        style={{ width: "100%", height: "28px", padding: "1px" }}
                      />
                    </label>
                  </div>
                  <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "8px" }}>
                    <label style={{ display: "flex", flexDirection: "column", gap: "4px", fontSize: "11px", color: "var(--text-muted)" }}>
                      ALIGNMENT
                      <select
                        value={style.alignment}
                        onChange={(event) => update({ alignment: event.target.value as TextAlignment })}
                        style={{ width: "100%", padding: "4px", fontSize: "12px" }}
                      >
                        {alignmentOptions.map((option) => <option key={option} value={option}>{option}</option>)}
                      </select>
                    </label>
                    <label style={{ display: "flex", flexDirection: "column", gap: "4px", fontSize: "11px", color: "var(--text-muted)" }}>
                      ANCHOR
                      <select
                        value={style.anchor}
                        onChange={(event) => update({ anchor: event.target.value as TextAnchor })}
                        style={{ width: "100%", padding: "4px", fontSize: "12px" }}
                      >
                        {anchorOptions.map((option) => <option key={option} value={option}>{option}</option>)}
                      </select>
                    </label>
                  </div>
                  <label style={{ display: "flex", alignItems: "center", gap: "8px", fontSize: "11px", color: "var(--text-muted)" }}>
                    <input
                      type="checkbox"
                      checked={Boolean(style.background_color) && style.background_opacity > 0}
                      onChange={(event) => {
                        const enabled = event.target.checked;
                        update({
                          background_color: enabled ? (style.background_color ?? "#000000") : null,
                          background_opacity: enabled ? Math.max(style.background_opacity, 0.72) : 0,
                          background_corner_radius: enabled
                            ? (style.background_corner_radius ?? 0) || 14
                            : (style.background_corner_radius ?? 0),
                        });
                      }}
                    />
                    BACKGROUND
                    <input
                      type="color"
                      value={(style.background_color ?? "#000000").slice(0, 7)}
                      disabled={!style.background_color || style.background_opacity <= 0}
                      onChange={(event) => update({ background_color: event.target.value.toUpperCase() })}
                      style={{ marginLeft: "auto", width: "72px", height: "24px", padding: "1px" }}
                    />
                  </label>
                  <label style={{ display: "flex", alignItems: "center", gap: "8px", fontSize: "11px", color: "var(--text-muted)" }}>
                    Bo góc nền
                    <input
                      type="range"
                      min={0}
                      max={48}
                      step={1}
                      value={style.background_corner_radius ?? 0}
                      disabled={!style.background_color || style.background_opacity <= 0}
                      onChange={(event) => update({ background_corner_radius: Number(event.target.value) })}
                      style={{ flex: 1, accentColor: "var(--accent)" }}
                    />
                    <span style={{ minWidth: "32px", textAlign: "right" }}>{Math.round(style.background_corner_radius ?? 0)} px</span>
                  </label>
                  {textTrack && textTrack.textItemCount > 1 && onApplyTextStyleToTrack && (
                    <button
                      className="btn-action"
                      type="button"
                      onClick={() => onApplyTextStyleToTrack(textTrack.trackId, style)}
                      style={{ padding: "6px 7px", fontSize: "10px", textAlign: "left" }}
                    >
                      Apply current style to all {textTrack.textItemCount} text clips on {textTrack.name}
                    </button>
                  )}
                </>
              );
            })()}
          </section>
        )}

        {selectedItem.kind === "Text" && onUpdateTextAppearance && (
          <section style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
            <button
              type="button"
              onClick={() => setAppearanceExpanded(!appearanceExpanded)}
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                background: "none",
                border: "none",
                color: "var(--text-muted)",
                fontSize: "11px",
                fontWeight: 600,
                cursor: "pointer",
                padding: "2px 0",
                textAlign: "left",
              }}
            >
              <span>APPEARANCE & FILL (GRADIENT / STROKE / SHADOW)</span>
              <span>{appearanceExpanded ? "▼" : "▶"}</span>
            </button>

            {appearanceExpanded && (() => {
              const rich = selectedItem.text_rich;
              const fill = rich?.fill ?? { kind: "Solid" as const, color: selectedItem.text_style?.font_color ?? "#FFFFFF" };
              const strokes = rich?.strokes ?? [];
              const shadows = rich?.shadows ?? [];
              const bg = rich?.background ?? null;

              const isGradient = fill.kind === "Gradient";

              return (
                <div style={{ display: "flex", flexDirection: "column", gap: "8px", fontSize: "11px" }}>
                  <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                    <span style={{ color: "var(--text-muted)" }}>Fill Mode</span>
                    <div style={{ display: "flex", gap: "4px" }}>
                      <button
                        type="button"
                        className="btn-action"
                        onClick={() => {
                          onUpdateTextAppearance(
                            { kind: "Solid", color: selectedItem.text_style?.font_color ?? "#FFFFFF" },
                            strokes,
                            shadows,
                            bg
                          );
                        }}
                        style={{
                          padding: "2px 6px",
                          fontSize: "10px",
                          fontWeight: !isGradient ? 700 : 400,
                          backgroundColor: !isGradient ? "var(--accent-primary)" : "var(--bg-surface)",
                          color: !isGradient ? "#FFF" : "var(--text-main)",
                        }}
                      >
                        Solid
                      </button>
                      <button
                        type="button"
                        className="btn-action"
                        onClick={() => {
                          onUpdateTextAppearance(
                            {
                              kind: "Gradient",
                              gradient_kind: "Linear",
                              angle_deg: 90,
                              stops: [
                                { offset: 0, color: "#60A5FA" },
                                { offset: 1, color: "#A855F7" },
                              ],
                            },
                            strokes,
                            shadows,
                            bg
                          );
                        }}
                        style={{
                          padding: "2px 6px",
                          fontSize: "10px",
                          fontWeight: isGradient ? 700 : 400,
                          backgroundColor: isGradient ? "var(--accent-primary)" : "var(--bg-surface)",
                          color: isGradient ? "#FFF" : "var(--text-main)",
                        }}
                      >
                        Gradient
                      </button>
                    </div>
                  </div>

                  {isGradient && fill.kind === "Gradient" && (
                    <div style={{ display: "flex", flexDirection: "column", gap: "6px", padding: "6px", backgroundColor: "var(--bg-surface)", borderRadius: "4px" }}>
                      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                        <span style={{ color: "var(--text-muted)" }}>Type</span>
                        <select
                          value={fill.gradient_kind}
                          onChange={(e) => {
                            onUpdateTextAppearance(
                              { ...fill, gradient_kind: e.target.value as GradientKind },
                              strokes,
                              shadows,
                              bg
                            );
                          }}
                          style={{ fontSize: "10px", padding: "2px 4px" }}
                        >
                          <option value="Linear">Linear</option>
                          <option value="Radial">Radial</option>
                        </select>
                      </div>

                      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                        <span style={{ color: "var(--text-muted)" }}>Angle</span>
                        <div style={{ display: "flex", alignItems: "center", gap: "4px" }}>
                          <input
                            type="range"
                            min="0"
                            max="360"
                            value={fill.angle_deg}
                            onChange={(e) => {
                              onUpdateTextAppearance(
                                { ...fill, angle_deg: Number(e.target.value) || 0 },
                                strokes,
                                shadows,
                                bg
                              );
                            }}
                            style={{ width: "80px" }}
                          />
                          <span>{fill.angle_deg}°</span>
                        </div>
                      </div>

                      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                        <span style={{ color: "var(--text-muted)" }}>Start / End</span>
                        <div style={{ display: "flex", gap: "6px" }}>
                          <input
                            type="color"
                            value={(fill.stops[0]?.color ?? "#60A5FA").slice(0, 7)}
                            onChange={(e) => {
                              const newStops = [...fill.stops];
                              newStops[0] = { offset: 0, color: e.target.value.toUpperCase() };
                              onUpdateTextAppearance(
                                { ...fill, stops: newStops },
                                strokes,
                                shadows,
                                bg
                              );
                            }}
                            style={{ width: "28px", height: "20px", border: "none", cursor: "pointer" }}
                          />
                          <input
                            type="color"
                            value={(fill.stops[1]?.color ?? "#A855F7").slice(0, 7)}
                            onChange={(e) => {
                              const newStops = [...fill.stops];
                              newStops[1] = { offset: 1, color: e.target.value.toUpperCase() };
                              onUpdateTextAppearance(
                                { ...fill, stops: newStops },
                                strokes,
                                shadows,
                                bg
                              );
                            }}
                            style={{ width: "28px", height: "20px", border: "none", cursor: "pointer" }}
                          />
                        </div>
                      </div>
                    </div>
                  )}

                  <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
                    <label style={{ display: "flex", alignItems: "center", justifyContent: "space-between", color: "var(--text-muted)" }}>
                      <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                        <input
                          type="checkbox"
                          checked={strokes.length > 0}
                          onChange={(e) => {
                            const newStrokes = e.target.checked
                              ? [{ color: "#000000", width: 3, align: "Outside" as const }]
                              : [];
                            onUpdateTextAppearance(fill, newStrokes, shadows, bg);
                          }}
                        />
                        <span>Stroke Outline</span>
                      </div>
                      {strokes.length > 0 && (
                        <div style={{ display: "flex", alignItems: "center", gap: "4px" }}>
                          <input
                            type="color"
                            value={(strokes[0].color ?? "#000000").slice(0, 7)}
                            onChange={(e) => {
                              const updated = [{ ...strokes[0], color: e.target.value.toUpperCase() }];
                              onUpdateTextAppearance(fill, updated, shadows, bg);
                            }}
                            style={{ width: "24px", height: "18px", border: "none", cursor: "pointer" }}
                          />
                          <input
                            type="number"
                            min="1"
                            max="20"
                            value={strokes[0].width}
                            onChange={(e) => {
                              const updated = [{ ...strokes[0], width: Math.max(1, Number(e.target.value) || 1) }];
                              onUpdateTextAppearance(fill, updated, shadows, bg);
                            }}
                            style={{ width: "36px", padding: "1px 3px", fontSize: "10px" }}
                          />
                          <span style={{ fontSize: "9px" }}>px</span>
                        </div>
                      )}
                    </label>
                  </div>

                  <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
                    <label style={{ display: "flex", alignItems: "center", justifyContent: "space-between", color: "var(--text-muted)" }}>
                      <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                        <input
                          type="checkbox"
                          checked={shadows.length > 0}
                          onChange={(e) => {
                            const newShadows = e.target.checked
                              ? [{ color: "#00000088", offset_x: 2, offset_y: 2, blur: 4 }]
                              : [];
                            onUpdateTextAppearance(fill, strokes, newShadows, bg);
                          }}
                        />
                        <span>Drop Shadow</span>
                      </div>
                      {shadows.length > 0 && (
                        <div style={{ display: "flex", alignItems: "center", gap: "4px" }}>
                          <input
                            type="color"
                            value={(shadows[0].color ?? "#000000").slice(0, 7)}
                            onChange={(e) => {
                              const updated = [{ ...shadows[0], color: e.target.value.toUpperCase() }];
                              onUpdateTextAppearance(fill, strokes, updated, bg);
                            }}
                            style={{ width: "24px", height: "18px", border: "none", cursor: "pointer" }}
                          />
                          <span style={{ fontSize: "9px" }}>blur {shadows[0].blur ?? 4}px</span>
                        </div>
                      )}
                    </label>
                  </div>
                </div>
              );
            })()}
          </section>
        )}

        {selectedItem.kind === "Text" && onUpdateTextAnimation && (
          <section style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
            <button
              type="button"
              onClick={() => setAnimationExpanded(!animationExpanded)}
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                background: "none",
                border: "none",
                color: "var(--text-muted)",
                fontSize: "11px",
                fontWeight: 600,
                cursor: "pointer",
                padding: "2px 0",
                textAlign: "left",
              }}
            >
              <span>TEXT ANIMATION (IN / LOOP / OUT)</span>
              <span>{animationExpanded ? "▼" : "▶"}</span>
            </button>

            {animationExpanded && (() => {
              const anim = selectedItem.text_animation;
              const inActive = Boolean(anim?.in_anim);
              const loopActive = Boolean(anim?.loop_anim);
              const outActive = Boolean(anim?.out_anim);

              return (
                <div style={{ display: "flex", flexDirection: "column", gap: "6px", fontSize: "11px" }}>
                  <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                    <span style={{ color: "var(--text-muted)" }}>Intro (In)</span>
                    <select
                      value={inActive ? "FadeIn" : "None"}
                      onChange={(e) => {
                        const val = e.target.value;
                        if (val === "None") {
                          onUpdateTextAnimation(anim ? { ...anim, in_anim: null } : null);
                        } else {
                          onUpdateTextAnimation({
                            ...(anim ?? {}),
                            in_anim: {
                              duration: { value: 15, rate: { numerator: 30, denominator: 1 } },
                              from: {
                                translate_x: 0,
                                translate_y: 20,
                                scale_x: 0.9,
                                scale_y: 0.9,
                                rotation: 0,
                                opacity: 0,
                              },
                              to: {
                                translate_x: 0,
                                translate_y: 0,
                                scale_x: 1,
                                scale_y: 1,
                                rotation: 0,
                                opacity: 1,
                              },
                            },
                          });
                        }
                      }}
                      style={{ fontSize: "11px", padding: "2px 4px" }}
                    >
                      <option value="None">None</option>
                      <option value="FadeIn">Fade & Rise In</option>
                    </select>
                  </div>

                  <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                    <span style={{ color: "var(--text-muted)" }}>Continuous (Loop)</span>
                    <select
                      value={loopActive ? "Pulse" : "None"}
                      onChange={(e) => {
                        const val = e.target.value;
                        if (val === "None") {
                          onUpdateTextAnimation(anim ? { ...anim, loop_anim: null } : null);
                        } else {
                          onUpdateTextAnimation({
                            ...(anim ?? {}),
                            loop_anim: {
                              duration: { value: 30, rate: { numerator: 30, denominator: 1 } },
                              from: {
                                translate_x: 0,
                                translate_y: 0,
                                scale_x: 1,
                                scale_y: 1,
                                rotation: 0,
                                opacity: 1,
                              },
                              to: {
                                translate_x: 0,
                                translate_y: 0,
                                scale_x: 1.05,
                                scale_y: 1.05,
                                rotation: 0,
                                opacity: 1,
                              },
                            },
                          });
                        }
                      }}
                      style={{ fontSize: "11px", padding: "2px 4px" }}
                    >
                      <option value="None">None</option>
                      <option value="Pulse">Gentle Pulse</option>
                    </select>
                  </div>

                  <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                    <span style={{ color: "var(--text-muted)" }}>Outro (Out)</span>
                    <select
                      value={outActive ? "FadeOut" : "None"}
                      onChange={(e) => {
                        const val = e.target.value;
                        if (val === "None") {
                          onUpdateTextAnimation(anim ? { ...anim, out_anim: null } : null);
                        } else {
                          onUpdateTextAnimation({
                            ...(anim ?? {}),
                            out_anim: {
                              duration: { value: 15, rate: { numerator: 30, denominator: 1 } },
                              from: {
                                translate_x: 0,
                                translate_y: 0,
                                scale_x: 1,
                                scale_y: 1,
                                rotation: 0,
                                opacity: 1,
                              },
                              to: {
                                translate_x: 0,
                                translate_y: -20,
                                scale_x: 0.95,
                                scale_y: 0.95,
                                rotation: 0,
                                opacity: 0,
                              },
                            },
                          });
                        }
                      }}
                      style={{ fontSize: "11px", padding: "2px 4px" }}
                    >
                      <option value="None">None</option>
                      <option value="FadeOut">Fade & Rise Out</option>
                    </select>
                  </div>
                </div>
              );
            })()}
          </section>
        )}

        {/* Transform controls with keyframing */}
        <section style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
          <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)" }}>
            TRANSFORM (WITH KEYFRAMES)
          </div>

          {selectedItem.kind === "Video" && onUpdatePlacementMode && (
            <label style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px", fontSize: "12px" }}>
              Frame placement:
              <select
                aria-label="Frame placement"
                value={selectedItem.placement_mode ?? "Fit"}
                onChange={(event) => onUpdatePlacementMode(event.target.value as PlacementMode)}
                style={{
                  flex: 1,
                  maxWidth: "120px",
                  padding: "4px 6px",
                  backgroundColor: "var(--bg-surface)",
                  color: "var(--text-main)",
                  border: "1px solid var(--border-color)",
                  borderRadius: "3px",
                }}
              >
                <option value="Fit">Fit</option>
                <option value="Fill">Fill</option>
              </select>
            </label>
          )}

          <button
            type="button"
            className="btn-secondary"
            aria-label="Center selected clip position"
            onClick={() => onUpdateCanvasPosition?.(selectedItem.item_id, 0, 0)}
            disabled={!onUpdateCanvasPosition}
          >
            Center position
          </button>

          {selectedItem.kind === "Video" && onSetEmbeddedAudioEnabled && (
            <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
              <label style={{ display: "flex", alignItems: "center", gap: "8px", fontSize: "12px" }}>
                <input
                  type="checkbox"
                  aria-label="Use embedded video audio"
                  checked={selectedItem.embedded_audio_enabled !== false}
                  disabled={!!selectedItem.linked_item_id}
                  onChange={(event) => onSetEmbeddedAudioEnabled(event.target.checked)}
                />
                Use embedded audio
              </label>
              {onDetachEmbeddedAudio && selectedItem.embedded_audio_enabled !== false && (
                <button type="button" className="btn-secondary" onClick={onDetachEmbeddedAudio}>
                  Detach audio to track
                </button>
              )}
              {selectedItem.linked_item_id && onUnlinkEmbeddedAudio && (
                <button type="button" className="btn-secondary" onClick={onUnlinkEmbeddedAudio}>
                  Unlink detached audio
                </button>
              )}
            </div>
          )}
          {selectedItem.kind === "Audio" && selectedItem.linked_item_id && onUnlinkEmbeddedAudio && (
            <button type="button" className="btn-secondary" onClick={onUnlinkEmbeddedAudio}>
              Unlink linked video
            </button>
          )}

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

        {onUpdateCrop && (selectedItem.kind === "Video" || selectedItem.kind === "Image") && (
          <section style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)" }}>CROP</div>
              <button
                type="button"
                className="btn-secondary"
                aria-label="Reset crop"
                onClick={() => {
                  const reset = { top: 0, bottom: 0, left: 0, right: 0 };
                  setLocalCrop(reset);
                  onUpdateCrop(reset);
                }}
              >
                Reset
              </button>
            </div>
            {(["top", "right", "bottom", "left"] as const).map((edge) => {
              const opposite = edge === "top" ? "bottom" : edge === "bottom" ? "top" : edge === "left" ? "right" : "left";
              const maxPercent = Math.max(0, 99 - localCrop[opposite] * 100);
              return (
                <label key={edge} style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px", fontSize: "12px" }}>
                  {edge[0].toUpperCase() + edge.slice(1)}:
                  <input
                    type="number"
                    aria-label={`Crop ${edge}`}
                    min={0}
                    max={maxPercent}
                    step={1}
                    value={Math.round(localCrop[edge] * 100)}
                    onChange={(event) => {
                      const percent = Math.min(maxPercent, Math.max(0, Number(event.target.value) || 0));
                      setLocalCrop((previous) => ({ ...previous, [edge]: percent / 100 }));
                    }}
                    onBlur={() => {
                      const canonicalCrop = selectedItem.crop ?? { top: 0, bottom: 0, left: 0, right: 0 };
                      if ((["top", "bottom", "left", "right"] as const).some((key) => localCrop[key] !== canonicalCrop[key])) {
                        onUpdateCrop(localCrop);
                      }
                    }}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") event.currentTarget.blur();
                    }}
                    style={{ width: "74px", padding: "4px", fontSize: "12px" }}
                  />
                  <span style={{ color: "var(--text-muted)" }}>%</span>
                </label>
              );
            })}
            <div style={{ fontSize: "10px", color: "var(--text-muted)" }}>
              Crop values apply to the source frame before transform.
            </div>
          </section>
        )}

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

        {onUpdateVisualEffect && (selectedItem.kind === "Video" || selectedItem.kind === "Image") && (
          <section style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
            <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)" }}>
              VISUAL EFFECT
            </div>
            <select
              aria-label="Visual Effect"
              value={selectedItem.visual_effect ?? "None"}
              onChange={(event) => onUpdateVisualEffect(event.target.value as VisualEffectPreset)}
              style={{
                width: "100%",
                padding: "4px",
                fontSize: "12px",
                background: "var(--bg-secondary)",
                color: "var(--text-primary)",
                border: "1px solid var(--border-color)",
                borderRadius: "4px",
              }}
            >
              <option value="None">None</option>
              <option value="Mono">Mono</option>
              <option value="Vivid">Vivid</option>
              <option value="Muted">Muted</option>
            </select>
          </section>
        )}

        {onUpdateColorAdjustments && (selectedItem.kind === "Video" || selectedItem.kind === "Image") && (
          <section style={{ display: "flex", flexDirection: "column", gap: "7px" }}>
            <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)" }}>
              SDR COLOR ADJUSTMENTS
            </div>
            {([
              ["brightness", "Brightness", -1, 1, 0.01, 0],
              ["contrast", "Contrast", 0, 2, 0.01, 1],
              ["saturation", "Saturation", 0, 2, 0.01, 1],
              ["temperature", "Temperature", -1, 1, 0.01, 0],
            ] as const).map(([key, label, min, max, step, defaultValue]) => {
              const adjustments = selectedItem.color_adjustments ?? {
                brightness: 0, contrast: 1, saturation: 1, temperature: 0,
              };
              return (
                <label key={key} style={{ display: "grid", gridTemplateColumns: "82px 1fr 38px", gap: "6px", alignItems: "center", fontSize: "11px" }}>
                  <span>{label}</span>
                  <input
                    aria-label={label}
                    type="range"
                    min={min}
                    max={max}
                    step={step}
                    value={adjustments[key] ?? defaultValue}
                    onChange={(event) => onUpdateColorAdjustments({
                      ...adjustments,
                      [key]: Number(event.currentTarget.value),
                    })}
                  />
                  <span style={{ textAlign: "right", fontVariantNumeric: "tabular-nums" }}>
                    {(adjustments[key] ?? defaultValue).toFixed(2)}
                  </span>
                </label>
              );
            })}
            <button
              type="button"
              onClick={() => onUpdateColorAdjustments({ brightness: 0, contrast: 1, saturation: 1, temperature: 0 })}
              style={{ alignSelf: "flex-end", fontSize: "11px" }}
            >Reset color</button>
          </section>
        )}

        {onUpdateChromaKey && (selectedItem.kind === "Video" || selectedItem.kind === "Image") && (() => {
          const key = selectedItem.chroma_key ?? null;
          const defaults: ChromaKey = { key_color: "#00ff00", similarity: 0.3, blend: 0.1, despill: null };
          const update = (patch: Partial<ChromaKey>) => onUpdateChromaKey({ ...(key ?? defaults), ...patch });
          return (
            <section style={{ display: "flex", flexDirection: "column", gap: "7px" }}>
              <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)" }}>CHROMA KEY</div>
              <label style={{ display: "flex", gap: "6px", alignItems: "center", fontSize: "11px" }}>
                <input
                  type="checkbox"
                  aria-label="Enable chroma key"
                  checked={key !== null}
                  onChange={(event) => onUpdateChromaKey(event.currentTarget.checked ? defaults : null)}
                />
                Key out a color
              </label>
              {key && (
                <>
                  <label style={{ display: "grid", gridTemplateColumns: "82px 1fr", gap: "6px", alignItems: "center", fontSize: "11px" }}>
                    <span>Key color</span>
                    <input
                      aria-label="Key color"
                      type="color"
                      value={key.key_color}
                      onChange={(event) => update({ key_color: event.currentTarget.value })}
                    />
                  </label>
                  {([
                    ["similarity", "Similarity", 0.001, 1, 0.001],
                    ["blend", "Edge blend", 0, 1, 0.001],
                  ] as const).map(([field, label, min, max, step]) => (
                    <label key={field} style={{ display: "grid", gridTemplateColumns: "82px 1fr 38px", gap: "6px", alignItems: "center", fontSize: "11px" }}>
                      <span>{label}</span>
                      <input
                        aria-label={label}
                        type="range"
                        min={min}
                        max={max}
                        step={step}
                        value={key[field]}
                        onChange={(event) => update({ [field]: Number(event.currentTarget.value) })}
                      />
                      <span style={{ textAlign: "right", fontVariantNumeric: "tabular-nums" }}>{key[field].toFixed(2)}</span>
                    </label>
                  ))}
                  <label style={{ display: "grid", gridTemplateColumns: "82px 1fr", gap: "6px", alignItems: "center", fontSize: "11px" }}>
                    <span>Despill</span>
                    <select
                      aria-label="Despill"
                      value={key.despill?.screen ?? "Off"}
                      onChange={(event) => {
                        const value = event.currentTarget.value;
                        update({ despill: value === "Off" ? null : { screen: value as "Green" | "Blue", mix: key.despill?.mix ?? 0.5 } });
                      }}
                    >
                      <option value="Off">Off</option>
                      <option value="Green">Green screen</option>
                      <option value="Blue">Blue screen</option>
                    </select>
                  </label>
                  {key.despill && (
                    <label style={{ display: "grid", gridTemplateColumns: "82px 1fr 38px", gap: "6px", alignItems: "center", fontSize: "11px" }}>
                      <span>Despill mix</span>
                      <input
                        aria-label="Despill mix"
                        type="range"
                        min={0}
                        max={1}
                        step={0.01}
                        value={key.despill.mix}
                        onChange={(event) => update({ despill: { screen: key.despill!.screen, mix: Number(event.currentTarget.value) } })}
                      />
                      <span style={{ textAlign: "right", fontVariantNumeric: "tabular-nums" }}>{key.despill.mix.toFixed(2)}</span>
                    </label>
                  )}
                </>
              )}
            </section>
          );
        })()}

        {onChooseLut && (selectedItem.kind === "Video" || selectedItem.kind === "Image") && (
          <section style={{ display: "flex", flexDirection: "column", gap: "7px" }}>
            <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)" }}>
              3D LUT
            </div>
            <div style={{ fontSize: "11px" }} aria-label="Current LUT">
              {selectedItem.color_lut ? selectedItem.color_lut.name : "None"}
            </div>
            <div style={{ display: "flex", gap: "6px", justifyContent: "flex-end" }}>
              <button type="button" onClick={onChooseLut} style={{ fontSize: "11px" }}>
                {selectedItem.color_lut ? "Replace LUT…" : "Choose LUT…"}
              </button>
              {selectedItem.color_lut && onClearLut && (
                <button type="button" onClick={onClearLut} style={{ fontSize: "11px" }}>
                  Clear LUT
                </button>
              )}
            </div>
          </section>
        )}

        {onUpdateSpeed && (selectedItem.kind === "Video" || selectedItem.kind === "Audio") && (
          <section style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
            <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)" }}>
              SPEED
            </div>
            <select
              aria-label="Speed"
              value={typeof selectedItem.speed === "object" ? "Custom" : (selectedItem.speed ?? "Normal")}
              onChange={(event) => {
                if (event.target.value !== "Custom") onUpdateSpeed(event.target.value as ClipSpeed);
              }}
              style={{
                width: "100%",
                padding: "4px",
                fontSize: "12px",
                background: "var(--bg-secondary)",
                color: "var(--text-primary)",
                border: "1px solid var(--border-color)",
                borderRadius: "4px",
              }}
            >
              <option value="Half">0.5×</option>
              <option value="Normal">1×</option>
              <option value="Double">2×</option>
              {typeof selectedItem.speed === "object" && (
                <option value="Custom">{formatClipSpeed(selectedItem.speed)} (custom)</option>
              )}
            </select>
            <form
              style={{ display: "flex", gap: "6px", alignItems: "center" }}
              onSubmit={(event) => {
                event.preventDefault();
                const parsed = parseSpeedInput(customSpeedDraft);
                if (typeof parsed === "string" && !["Half", "Normal", "Double"].includes(parsed)) {
                  setCustomSpeedError(parsed);
                  return;
                }
                setCustomSpeedError(null);
                onUpdateSpeed(parsed as ClipSpeed);
              }}
            >
              <input
                aria-label="Custom speed"
                placeholder="3/2 or 0.75"
                value={customSpeedDraft}
                onChange={(event) => setCustomSpeedDraft(event.currentTarget.value)}
                style={{ flex: 1, fontSize: "11px", padding: "3px" }}
              />
              <button type="submit" style={{ fontSize: "11px" }}>Set</button>
            </form>
            {customSpeedError && (
              <div role="status" style={{ fontSize: "11px", color: "var(--danger, #e66)" }}>{customSpeedError}</div>
            )}
          </section>
        )}

        {onUpdateReverse && (selectedItem.kind === "Video" || selectedItem.kind === "Audio") && (
          <section style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
            <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)" }}>REVERSE</div>
            <label style={{ display: "flex", gap: "6px", alignItems: "center", fontSize: "11px" }}>
              <input
                type="checkbox"
                aria-label="Reverse clip"
                checked={selectedItem.reverse === true}
                onChange={(event) => onUpdateReverse(event.currentTarget.checked)}
              />
              Play this clip backwards
            </label>
            <div style={{ fontSize: "10px", color: "var(--text-muted)" }}>
              Applied when exporting (up to 15 s of source); the preview plays it forward.
            </div>
          </section>
        )}

        {/* Item transitions only apply to the selected item's own boundaries. */}
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

        {/* Cut transitions are owned by the track seam between two adjacent clips. */}
        {(adjacentInfo?.nextClip || adjacentInfo?.prevClip) && selectedItem.kind === "Video" && (
          <section style={{ display: "flex", flexDirection: "column", gap: "8px", borderTop: "1px solid var(--border-color)", paddingTop: "12px" }}>
            <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)" }}>
              CUT TRANSITION
            </div>
            {adjacentInfo?.outgoingTransition ? (
              <div style={{ display: "flex", flexDirection: "column", gap: "6px", backgroundColor: "var(--bg-secondary)", padding: "8px", borderRadius: "4px" }}>
                <div style={{ fontSize: "12px", display: "flex", justifyContent: "space-between" }}>
                  <span>
                    {adjacentInfo.outgoingTransition.kind === "CrossDissolve"
                      ? "Cross Dissolve"
                      : "Dip to Black"}{" "}
                    with <strong>{adjacentInfo.nextClip?.name}</strong>
                  </span>
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
                  Remove cut transition
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
                      "CrossDissolve",
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
                <button
                  onClick={() =>
                    onCreateCutTransition?.(
                      adjacentInfo.trackId!,
                      selectedItem.item_id,
                      adjacentInfo.nextClip!.item_id,
                      "DipToBlack",
                      30
                    )
                  }
                  style={{
                    padding: "5px 10px",
                    backgroundColor: "var(--bg-secondary)",
                    color: "var(--text-primary)",
                    border: "1px solid var(--border-color)",
                    borderRadius: "4px",
                    fontSize: "11px",
                    fontWeight: 600,
                    cursor: "pointer",
                  }}
                >
                  + Add Dip to Black (30f)
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
                VOLUME ({formatGainDb(selectedItem.volume ?? 1.0)})
              </div>
              {renderKeyframeControl("Volume", selectedItem.volume ?? 1.0)}
            </div>
            <input
              type="range"
              min={MIN_AUDIO_GAIN_DB}
              max={MAX_AUDIO_GAIN_DB}
              step="0.5"
              value={linearGainToDb(selectedItem.volume ?? 1.0)}
              onChange={(e) => {
                const val = dbToLinearGain(parseFloat(e.target.value));
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

        {onUpdateDucking && (selectedItem.kind === "Video" || selectedItem.kind === "Audio") && (() => {
          const ducking = selectedItem.ducking ?? null;
          const defaults: Ducking = { depth_db: 12, attack_ms: 30, release_ms: 400, threshold_db: -40 };
          const update = (patch: Partial<Ducking>) => onUpdateDucking({ ...(ducking ?? defaults), ...patch });
          return (
            <section style={{ display: "flex", flexDirection: "column", gap: "7px" }}>
              <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)" }}>DUCKING</div>
              <label style={{ display: "flex", gap: "6px", alignItems: "center", fontSize: "11px" }}>
                <input
                  type="checkbox"
                  aria-label="Duck under other audio"
                  checked={ducking !== null}
                  onChange={(event) => onUpdateDucking(event.currentTarget.checked ? defaults : null)}
                />
                Lower this clip while other audio plays
              </label>
              {ducking && (
                <>
                  {([
                    ["depth_db", "Duck depth (dB)", 1, 30, 1],
                    ["attack_ms", "Attack (ms)", 5, 500, 5],
                    ["release_ms", "Release (ms)", 20, 2000, 10],
                    ["threshold_db", "Threshold (dB)", -60, -10, 1],
                  ] as const).map(([field, label, min, max, step]) => (
                    <label key={field} style={{ display: "grid", gridTemplateColumns: "100px 1fr 44px", gap: "6px", alignItems: "center", fontSize: "11px" }}>
                      <span>{label}</span>
                      <input
                        aria-label={label}
                        type="range"
                        min={min}
                        max={max}
                        step={step}
                        value={ducking[field]}
                        onChange={(event) => update({ [field]: Number(event.currentTarget.value) })}
                      />
                      <span style={{ textAlign: "right", fontVariantNumeric: "tabular-nums" }}>{ducking[field]}</span>
                    </label>
                  ))}
                </>
              )}
            </section>
          );
        })()}

        {scopesSection}
      </div>
    </aside>
  );
};
