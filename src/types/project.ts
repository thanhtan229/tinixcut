import { TemplateInstance } from "./template";
export * from "./template";

export interface FrameRate {
  numerator: number;
  denominator: number;
}

export interface RationalTime {
  value: number;
  rate: FrameRate;
}

export interface TimeRange {
  start: RationalTime;
  duration: RationalTime;
}

export function rationalTimeToSeconds(rt: RationalTime): number {
  if (!rt || !rt.rate || rt.rate.numerator === 0) return 0;
  return (rt.value * rt.rate.denominator) / rt.rate.numerator;
}

export function rationalTimeToFrames(rt: RationalTime, targetRate: FrameRate): number {
  if (!rt?.rate?.numerator || !targetRate.numerator || !targetRate.denominator) return 0;
  const frames =
    (rt.value * targetRate.numerator * rt.rate.denominator) /
    (rt.rate.numerator * targetRate.denominator);
  return Number.isFinite(frames) ? Math.floor(frames) : 0;
}

export function getDefaultAudioCrossfadeDurationFrames(
  left: TimelineItem,
  right: TimelineItem,
  assets: Asset[],
  rate: FrameRate,
  maxDurationFrames = 30
): number {
  if (left.kind !== "Audio" || right.kind !== "Audio") return 0;
  const leftAsset = assets.find((asset) => asset.asset_id === left.asset_id);
  const rightAsset = assets.find((asset) => asset.asset_id === right.asset_id);
  if (
    leftAsset?.media_kind !== "Audio" ||
    rightAsset?.media_kind !== "Audio" ||
    !leftAsset.duration ||
    !rightAsset.duration
  ) {
    return 0;
  }

  const leftPostroll =
    rationalTimeToFrames(leftAsset.duration, rate) -
    rationalTimeToFrames(left.source_range.start, rate) -
    rationalTimeToFrames(left.source_range.duration, rate);
  const rightPreroll = rationalTimeToFrames(right.source_range.start, rate);
  const leftEditorial = rationalTimeToFrames(left.timeline_range.duration, rate);
  const rightEditorial = rationalTimeToFrames(right.timeline_range.duration, rate);
  const sourceFramesForTimelineFrames = (frames: number, speed: ClipSpeed | undefined): number | null => {
    const [numerator, denominator] =
      speed === "Half" ? [1, 2] : speed === "Double" ? [2, 1] : [1, 1];
    const sourceFrames = frames * numerator;
    return sourceFrames % denominator === 0 ? sourceFrames / denominator : null;
  };

  for (let duration = Math.floor(maxDurationFrames); duration >= 1; duration -= 1) {
    const leftHalf = Math.floor(duration / 2);
    const rightHalf = duration - leftHalf;
    const requiredPostroll = sourceFramesForTimelineFrames(rightHalf, left.speed);
    const requiredPreroll = sourceFramesForTimelineFrames(leftHalf, right.speed);
    if (
      requiredPostroll !== null &&
      requiredPreroll !== null &&
      leftPostroll >= requiredPostroll &&
      rightPreroll >= requiredPreroll &&
      leftEditorial >= leftHalf &&
      rightEditorial >= rightHalf
    ) {
      return duration;
    }
  }
  return 0;
}

export function areRationalTimesEqual(a: RationalTime, b: RationalTime): boolean {
  if (a.rate.numerator === b.rate.numerator && a.rate.denominator === b.rate.denominator) {
    return a.value === b.value;
  }
  return Math.abs(rationalTimeToSeconds(a) - rationalTimeToSeconds(b)) < 1e-5;
}

export function isRangeAdjacent(left: TimeRange, right: TimeRange): boolean {
  const leftEndSec = rationalTimeToSeconds(left.start) + rationalTimeToSeconds(left.duration);
  const rightStartSec = rationalTimeToSeconds(right.start);
  return Math.abs(leftEndSec - rightStartSec) < 1e-5;
}

export interface Transform {
  position_x: number;
  position_y: number;
  scale_x: number;
  scale_y: number;
  rotation: number;
}

export interface Crop {
  top: number;
  bottom: number;
  left: number;
  right: number;
}

export type MediaKind = "Video" | "Audio" | "Image";
export type TrackKind = "Video" | "Audio";
export type ItemKind = "Video" | "Audio" | "Image" | "Text";

export type TextAlignment = "Left" | "Center" | "Right";
export type TextAnchor =
  | "TopLeft"
  | "TopCenter"
  | "TopRight"
  | "MiddleLeft"
  | "Center"
  | "MiddleRight"
  | "BottomLeft"
  | "BottomCenter"
  | "BottomRight";

export interface TextStyle {
  font_family: string;
  font_size: number;
  font_weight: number;
  font_color: string;
  accent_color?: string | null;
  background_color?: string | null;
  background_opacity: number;
  padding_x: number;
  padding_y: number;
  alignment: TextAlignment;
  line_height: number;
  anchor: TextAnchor;
}

export interface TextTemplateReference {
  template_id: string;
  template_version: number;
}

export interface Asset {
  asset_id: string;
  name: string;
  path: string;
  media_kind: MediaKind;
  duration?: RationalTime;
  file_size: number;
  dimensions?: AssetDimensions | null;
}

export interface AssetDimensions {
  width: number;
  height: number;
}

export type PlacementMode = "Fit" | "Fill";

export type KeyframeProperty =
  | "PositionX"
  | "PositionY"
  | "ScaleX"
  | "ScaleY"
  | "Rotation"
  | "Opacity"
  | "Volume";

export type KeyframeEasing =
  | "Linear"
  | "EaseIn"
  | "EaseOut"
  | "EaseInOut"
  | { CubicBezier: { x1: number; y1: number; x2: number; y2: number } };

export interface Keyframe {
  property: KeyframeProperty;
  offset: RationalTime;
  value: number;
  outgoing_easing?: KeyframeEasing;
}

export type TransitionKind =
  | "FadeIn"
  | "FadeOut"
  | "CrossDissolve"
  | "FadeBlack"
  | "DipToBlack";

export type VisualEffectPreset = "None" | "Mono" | "Vivid" | "Muted";
export type ClipSpeed = "Half" | "Normal" | "Double";

export interface Transition {
  kind: TransitionKind;
  duration: RationalTime;
}

export type FontStyle = "Normal" | "Italic" | "Oblique";

export interface FontReference {
  family: string;
  weight: number;
  style?: FontStyle;
  asset_id?: string | null;
}

export type TextWrapMode = "None" | "Wrap" | "AutoFit";

export interface TextLayout {
  max_width?: number | null;
  alignment?: TextAlignment;
  line_height?: number;
  letter_spacing?: number;
  wrap_mode?: TextWrapMode;
  min_font_size?: number | null;
  max_font_size?: number | null;
}

export type GradientKind = "Linear" | "Radial";

export type GradientSpace = "All" | "Character";

export interface GradientStop {
  offset: number;
  color: string;
}

export type TextFill =
  | { kind: "Solid"; color: string }
  | {
      kind: "Gradient";
      gradient_kind: GradientKind;
      angle_deg: number;
      radius?: number | null;
      space?: GradientSpace;
      stops: GradientStop[];
    };

export type StrokeAlign = "Outside" | "Center" | "Inside";

export interface TextStroke {
  color: string;
  width: number;
  align?: StrokeAlign;
}

export interface TextShadow {
  color: string;
  offset_x?: number;
  offset_y?: number;
  blur?: number;
}

export interface TextBackgroundBox {
  fill?: TextFill;
  padding_x?: number;
  padding_y?: number;
  corner_radius?: number;
  opacity?: number;
}

export interface TextSpanStyle {
  fill?: TextFill | null;
  font?: FontReference | null;
  font_size?: number | null;
  letter_spacing?: number | null;
}

export interface RichTextSpan {
  start: number;
  end: number;
  style?: TextSpanStyle;
}

export interface TextRichStyle {
  font?: FontReference | null;
  font_size?: number | null;
  layout?: TextLayout;
  fill?: TextFill;
  strokes?: TextStroke[];
  shadows?: TextShadow[];
  background?: TextBackgroundBox | null;
  spans?: RichTextSpan[];
}

export type AnimationUnit = "All" | "Grapheme" | "Word" | "Line";

export type StaggerOrder = "Forward" | "Backward" | "CenterOut" | "Random";

export type LoopMode = "None" | "Repeat" | "PingPong";

export type DurationConflictPolicy = "Reject" | "ProportionalScale" | "ClampOut";

export interface AnimationTransform {
  translate_x: number;
  translate_y: number;
  scale_x: number;
  scale_y: number;
  rotation: number;
  opacity: number;
}

export interface TextAnimationSegment {
  duration: RationalTime;
  easing?: KeyframeEasing;
  stagger_ratio?: number;
  from?: AnimationTransform;
  to?: AnimationTransform;
}

export interface TextAnimation {
  unit?: AnimationUnit;
  order?: StaggerOrder;
  seed?: number | null;
  pivot_x?: number;
  pivot_y?: number;
  in_anim?: TextAnimationSegment | null;
  loop_anim?: TextAnimationSegment | null;
  out_anim?: TextAnimationSegment | null;
  loop_mode?: LoopMode;
  conflict_policy?: DurationConflictPolicy;
}
export interface TimelineItem {
  item_id: string;
  asset_id?: string;
  name: string;
  kind: ItemKind;
  timeline_range: TimeRange;
  source_range: TimeRange;
  transform: Transform;
  crop: Crop;
  opacity: number;
  volume: number;
  text_content?: string;
  template_instance?: TemplateInstance | null;
  text_style?: TextStyle | null;
  text_template?: TextTemplateReference | null;
  text_rich?: TextRichStyle | null;
  text_animation?: TextAnimation | null;
  fade_in?: RationalTime;
  fade_out?: RationalTime;
  transition?: Transition;
  keyframes?: Keyframe[];
  placement_mode?: PlacementMode;
  visual_effect?: VisualEffectPreset;
  speed?: ClipSpeed;
}

export interface CutTransition {
  transition_id: string;
  track_id: string;
  left_item_id: string;
  right_item_id: string;
  kind: TransitionKind;
  duration: RationalTime;
}

export interface AudioCrossfade {
  transition_id: string;
  track_id: string;
  left_item_id: string;
  right_item_id: string;
  duration: RationalTime;
}

export interface Track {
  track_id: string;
  name: string;
  kind: TrackKind;
  muted: boolean;
  locked: boolean;
  items: TimelineItem[];
  transitions?: CutTransition[];
  audio_transitions?: AudioCrossfade[];
}

export interface ProjectSettings {
  width: number;
  height: number;
  canvas_background_color: string;
  frame_rate: FrameRate;
  sample_rate: number;
  channels: number;
}

export interface ClientRecord {
  client_epoch: number;
  highest_sequence: number;
  last_active_revision: number;
  registered_at_revision: number;
}

export interface ClientRegistry {
  min_accepted_epoch: number;
  current_epoch: number;
  max_active_clients: number;
  clients: Record<string, ClientRecord>;
}

export interface CommandLedgerEntry {
  operation_id: string;
  transaction_id?: string;
  client_id?: string;
  client_epoch?: number;
  sequence_number?: number;
  fingerprint: string;
  status: string;
  applied_at_revision: number;
  affected_item_id?: string;
  affected_track_id?: string;
}

export interface Project {
  schema_version: number;
  project_id: string;
  name: string;
  revision: number;
  settings: ProjectSettings;
  assets: Asset[];
  tracks: Track[];
  created_at: string;
  updated_at: string;
  client_registry?: ClientRegistry;
  command_ledger?: CommandLedgerEntry[];
}
