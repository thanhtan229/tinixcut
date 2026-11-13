import {
  Crop,
  ItemKind,
  Keyframe,
  KeyframeEasing,
  KeyframeProperty,
  AssetDimensions,
  PlacementMode,
  RationalTime,
  TimeRange,
  TrackKind,
  TextStyle,
  Transform,
  Transition,
  TransitionKind,
  VisualEffectPreset,
  ClipSpeed,
  FontReference,
  RichTextSpan,
  TextAnimation,
  TextBackgroundBox,
  TextFill,
  TextLayout,
  TextRichStyle,
  TextShadow,
  TextStroke,
  TemplateLayer,
  TemplateParameterValue,
} from "./project";

export type CommandOperation =
  | {
      type: "CutTransitionCreate";
      payload: {
        transition_id?: string;
        track_id: string;
        left_item_id: string;
        right_item_id: string;
        kind: TransitionKind;
        duration: RationalTime;
      };
    }
  | {
      type: "CutTransitionUpdateDuration";
      payload: {
        transition_id: string;
        new_duration: RationalTime;
      };
    }
  | {
      type: "CutTransitionRemove";
      payload: {
        transition_id: string;
      };
    }
  | {
      type: "AudioCrossfadeCreate";
      payload: {
        transition_id?: string;
        track_id: string;
        left_item_id: string;
        right_item_id: string;
        duration: RationalTime;
      };
    }
  | {
      type: "AudioCrossfadeUpdateDuration";
      payload: {
        transition_id: string;
        new_duration: RationalTime;
      };
    }
  | {
      type: "AudioCrossfadeRemove";
      payload: {
        transition_id: string;
      };
    }
  | {
      type: "ClipTransitionSet";
      payload: {
        item_id: string;
        transition?: Transition | null;
      };
    }
  | {
      type: "ClipVisualEffectSet";
      payload: {
        item_id: string;
        visual_effect: VisualEffectPreset;
      };
    }
  | {
      type: "ClipColorAdjustmentsSet";
      payload: {
        item_id: string;
        color_adjustments: import("./project").ColorAdjustments;
      };
    }
  | {
      type: "ClipReverseSet";
      payload: { item_id: string; reverse: boolean };
    }
  | {
      type: "ClipDuckingSet";
      payload: {
        item_id: string;
        ducking: import("./project").Ducking | null;
      };
    }
  | {
      type: "ClipChromaKeySet";
      payload: {
        item_id: string;
        chroma_key: import("./project").ChromaKey | null;
      };
    }
  | {
      type: "ClipColorLutSet";
      payload: {
        item_id: string;
        color_lut: import("./project").ClipLut | null;
      };
    }
  | {
      type: "ClipSpeedSet";
      payload: {
        item_id: string;
        speed: ClipSpeed;
      };
    }
  | {
      type: "TrackCreate";
      payload: {
        track_id?: string;
        name: string;
        kind: TrackKind;
      };
    }
  | {
      type: "TrackDelete";
      payload: {
        track_id: string;
      };
    }
  | {
      type: "TrackRename";
      payload: { track_id: string; name: string };
    }
  | {
      type: "TrackMuteSet";
      payload: {
        track_id: string;
        muted: boolean;
      };
    }
  | {
      type: "TrackSoloSet";
      payload: {
        track_id: string;
        solo: boolean;
      };
    }
  | {
      type: "TrackVolumeSet";
      payload: { track_id: string; volume: number };
    }
  | {
      type: "TrackLockSet";
      payload: {
        track_id: string;
        locked: boolean;
      };
    }
  | {
      type: "TrackReorder";
      payload: {
        track_id: string;
        new_index: number;
      };
    }
  | {
      type: "ProjectMarkerSet";
      payload: { marker_id?: string | null; label: string; position: RationalTime };
    }
  | {
      type: "ProjectMarkerDelete";
      payload: { marker_id: string };
    }
  | {
      type: "ProjectSettingsSet";
      payload: {
        width: number;
        height: number;
      };
    }
  | {
      type: "CanvasBackgroundSet";
      payload: {
        color: string;
      };
    }
  | {
      type: "ClipCreate";
      payload: {
        track_id: string;
        item_id?: string;
        name: string;
        kind: ItemKind;
        asset_id?: string;
        timeline_start: RationalTime;
        duration: RationalTime;
        source_start?: RationalTime;
      };
    }
  | {
      type: "ClipMove";
      payload: {
        item_id: string;
        target_track_id?: string;
        target_start: RationalTime;
      };
    }
  | {
      type: "ClipTrim";
      payload: {
        item_id: string;
        new_timeline_range: TimeRange;
        new_source_range: TimeRange;
      };
    }
  | {
      type: "ClipSlip";
      payload: {
        item_id: string;
        new_source_start: RationalTime;
      };
    }
  | {
      type: "ClipRoll";
      payload: {
        left_item_id: string;
        right_item_id: string;
        delta: RationalTime;
      };
    }
  | {
      type: "ClipSplit";
      payload: {
        item_id: string;
        split_time: RationalTime;
        new_right_item_id?: string;
      };
    }
  | {
      type: "ClipGroupMove";
      payload: {
        item_ids: string[];
        delta: RationalTime;
      };
    }
  | {
      type: "ClipRippleDelete";
      payload: {
        item_id: string;
      };
    }
  | {
      type: "ClipDelete";
      payload: {
        item_id: string;
      };
    }
  | {
      type: "ClipDuplicate";
      payload: {
        item_id: string;
        target_start?: RationalTime;
        target_track_id?: string;
        new_item_id?: string;
      };
    }
  | {
      type: "ClipTransformSet";
      payload: {
        item_id: string;
        transform: Transform;
      };
    }
  | {
      type: "ClipCropSet";
      payload: {
        item_id: string;
        crop: Crop;
      };
    }
  | {
      type: "ClipOpacitySet";
      payload: {
        item_id: string;
        opacity: number;
      };
    }
  | {
      type: "ClipVolumeSet";
      payload: {
        item_id: string;
        volume: number;
      };
    }
  | {
      type: "ClipEmbeddedAudioSet";
      payload: {
        item_id: string;
        enabled: boolean;
      };
    }
  | {
      type: "ClipAudioDetach";
      payload: {
        item_id: string;
        detached_item_id?: string;
        target_track_id?: string;
      };
    }
  | {
      type: "ClipLinkSet";
      payload: { video_item_id: string; audio_item_id: string; linked: boolean };
    }
  | {
      type: "ClipTextSet";
      payload: {
        item_id: string;
        text: string;
      };
    }
  | {
      type: "ClipTextStyleSet";
      payload: {
        item_id: string;
        style: TextStyle;
      };
    }
  | {
      type: "TextTrackStyleSet";
      payload: {
        track_id: string;
        style: TextStyle;
      };
    }
  | {
      type: "TextTemplateApply";
      payload: {
        item_id: string;
        template_id: string;
        template_version: number;
      };
    }
  | {
      type: "TextTrackTemplateApply";
      payload: {
        track_id: string;
        template_id: string;
        template_version: number;
      };
    }
  | {
      type: "ClipTextTemplateReset";
      payload: {
        item_id: string;
      };
    }
  | {
      type: "ClipTextRichSet";
      payload: {
        item_id: string;
        rich: TextRichStyle;
      };
    }
  | {
      type: "ClipTextSpansSet";
      payload: {
        item_id: string;
        spans: RichTextSpan[];
      };
    }
  | {
      type: "ClipTextLayoutSet";
      payload: {
        item_id: string;
        layout: TextLayout;
      };
    }
  | {
      type: "ClipTextAppearanceSet";
      payload: {
        item_id: string;
        fill: TextFill;
        strokes: TextStroke[];
        shadows: TextShadow[];
        background?: TextBackgroundBox | null;
      };
    }
  | {
      type: "ClipTextFontSet";
      payload: {
        item_id: string;
        font: FontReference;
        font_size?: number | null;
      };
    }
  | {
      type: "ClipTextAnimationSet";
      payload: {
        item_id: string;
        animation?: TextAnimation | null;
      };
    }
  | {
      type: "ClipTextRichReset";
      payload: {
        item_id: string;
      };
    }
  | {
      type: "TemplateApply";
      payload: {
        item_id: string;
        template_id: string;
        template_version: number;
        initial_parameters?: Record<string, TemplateParameterValue> | null;
      };
    }
  | {
      type: "TemplateTrackApply";
      payload: {
        track_id: string;
        template_id: string;
        template_version: number;
        initial_parameters?: Record<string, TemplateParameterValue> | null;
      };
    }
  | {
      type: "TemplateLayerAdd";
      payload: {
        item_id: string;
        layer: TemplateLayer;
      };
    }
  | {
      type: "TemplateLayerRemove";
      payload: {
        item_id: string;
        layer_id: string;
      };
    }
  | {
      type: "TemplateLayerReorder";
      payload: {
        item_id: string;
        layer_ids: string[];
      };
    }
  | {
      type: "TemplateLayerUpdate";
      payload: {
        item_id: string;
        layer_id: string;
        layer: TemplateLayer;
      };
    }
  | {
      type: "TemplateParameterSet";
      payload: {
        item_id: string;
        parameter_key: string;
        value: TemplateParameterValue;
      };
    }
  | {
      type: "TemplateParametersBatchSet";
      payload: {
        item_id: string;
        parameters: Record<string, TemplateParameterValue>;
      };
    }
  | {
      type: "TemplateReset";
      payload: {
        item_id: string;
      };
    }
  | {
      type: "TemplateDetach";
      payload: {
        item_id: string;
      };
    }
  | {
      type: "ClipAudioFadeSet";
      payload: {
        item_id: string;
        fade_in?: RationalTime;
        fade_out?: RationalTime;
      };
    }
  | {
      type: "AssetImport";
      payload: {
        asset_id?: string;
        name: string;
        path: string;
        media_kind: "Video" | "Audio" | "Image";
        duration?: RationalTime | null;
        file_size: number;
        dimensions?: AssetDimensions | null;
      };
    }
  | {
      type: "AssetRelink";
      payload: { asset_id: string; path: string };
    }
  | {
      type: "ClipPlacementModeSet";
      payload: {
        item_id: string;
        placement_mode: PlacementMode;
      };
    }
  | {
      type: "ClipKeyframeAdd";
      payload: {
        item_id: string;
        keyframe: Keyframe;
      };
    }
  | {
      type: "ClipKeyframeRemove";
      payload: {
        item_id: string;
        property: KeyframeProperty;
        offset: RationalTime;
      };
    }
  | {
      type: "ClipKeyframeMove";
      payload: {
        item_id: string;
        property: KeyframeProperty;
        old_offset: RationalTime;
        new_offset: RationalTime;
      };
    }
  | {
      type: "ClipKeyframeEasingSet";
      payload: {
        item_id: string;
        property: KeyframeProperty;
        offset: RationalTime;
        easing: KeyframeEasing;
      };
    };

export interface OperationEnvelope {
  operation_id: string;
  operation: CommandOperation;
}

export interface CommandEnvelope {
  schema_version: number;
  project_id: string;
  base_revision: number;
  client_id?: string;
  client_epoch?: number;
  sequence_number?: number;
  dry_run: boolean;
  transaction_id?: string;
  operations: OperationEnvelope[];
}

export interface OperationReceipt {
  operation_id: string;
  status: string;
  affected_item_id?: string;
  affected_track_id?: string;
}

export interface CommandReceipt {
  projectId: string;
  priorRevision: number;
  newRevision: number;
  transactionId?: string;
  operationsApplied: number;
  receipts: OperationReceipt[];
  idempotencyScope?: string;
}

export interface DiffReport {
  project_id: string;
  base_revision: number;
  operations_evaluated: number;
  changes: unknown[];
  warnings: string[];
}
