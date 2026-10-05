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

export interface Asset {
  asset_id: string;
  name: string;
  path: string;
  media_kind: MediaKind;
  duration?: RationalTime;
  file_size: number;
}

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
  | "FadeBlack";

export interface Transition {
  kind: TransitionKind;
  duration: RationalTime;
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
  fade_in?: RationalTime;
  fade_out?: RationalTime;
  transition?: Transition;
  keyframes?: Keyframe[];
}

export interface CutTransition {
  transition_id: string;
  track_id: string;
  left_item_id: string;
  right_item_id: string;
  kind: TransitionKind;
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
}

export interface ProjectSettings {
  width: number;
  height: number;
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

