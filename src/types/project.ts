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
}

export interface Track {
  track_id: string;
  name: string;
  kind: TrackKind;
  muted: boolean;
  locked: boolean;
  items: TimelineItem[];
}

export interface ProjectSettings {
  width: number;
  height: number;
  frame_rate: FrameRate;
  sample_rate: number;
  channels: number;
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
}
