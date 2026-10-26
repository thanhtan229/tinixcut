import {
  RationalTime,
  Crop,
  TextStyle,
  TextTemplateReference,
  VisualEffectPreset,
  TextRichStyle,
  TextAnimation,
} from "./project";

export interface PreviewSampledLayer {
  item_id: string;
  name: string;
  z_index: number;
  kind: "Video" | "Audio" | "Image" | "Text";
  asset_id?: string | null;
  asset_path?: string | null;
  is_missing_media: boolean;
  source_time: RationalTime;
  playback_rate: number;
  position: [number, number];
  scale: [number, number];
  content_size: [number, number];
  rotation: number;
  crop: Crop;
  opacity: number;
  visual_effect: VisualEffectPreset;
  text?: string | null;
  font_family?: string | null;
  font_size?: number | null;
  text_style?: TextStyle | null;
  text_template?: TextTemplateReference | null;
  text_rich?: TextRichStyle | null;
  text_animation?: TextAnimation | null;
  has_animation?: boolean;
  has_template_instance?: boolean;
  computed_bounds: {
    x: number;
    y: number;
    width: number;
    height: number;
  };
}

export interface PreviewSampledAudio {
  item_id: string;
  effective_volume: number;
  source_time: RationalTime;
  playback_rate: number;
  asset_path?: string | null;
}

export interface PreviewTransitionGroup {
  transition_id: string;
  track_id: string;
  z_index: number;
  kind: "FadeIn" | "FadeOut" | "CrossDissolve" | "FadeBlack" | "DipToBlack";
  progress: number;
  left_layer: PreviewSampledLayer;
  right_layer: PreviewSampledLayer;
}

export interface PreviewFrameState {
  timestamp: RationalTime;
  active_layers: PreviewSampledLayer[];
  active_audio: PreviewSampledAudio[];
  transition_groups?: PreviewTransitionGroup[];
}

export interface PreviewTextOverlayDiagnostics {
  fontFamily: string;
  fontSha256: string;
  fontOrigin: string;
  missingGlyphs: string[];
  notes: string[];
  rendererVersion: number;
}

export interface PreviewRichTextOverlay {
  itemId: string;
  zIndex: number;
  roi: [number, number, number, number];
  roiPngBytes: number[];
  hasVisibleContent: boolean;
  diagnostics: PreviewTextOverlayDiagnostics;
  renderMicros: number;
}

export interface BenchmarkReport {
  backend_name: string;
  test_duration_seconds: number;
  target_fps: number;
  dropped_frames: number;
  drop_percentage: number;
  avg_seek_latency_ms: number;
  p95_seek_latency_ms: number;
  av_drift_ms: number;
  estimated_cpu_load_percent: number;
  memory_growth_mb: number;
  passes_targets: boolean;
}

export interface ParityDiffMetrics {
  mae: number;
  max_error: number;
  exceeded_ratio: number;
  width: number;
  height: number;
  total_pixels: number;
  exceeded_pixels: number;
}

export interface ParityFixtureResult {
  fixture_id: string;
  name: string;
  timestamp_frames: number;
  semantic_status: "PASS" | "FAIL";
  visual_status: "PASS" | "FAIL";
  overall_status: "PASS" | "FAIL";
  metrics?: ParityDiffMetrics | null;
  details: string;
  preview_artifact?: string | null;
  export_artifact?: string | null;
  diff_artifact?: string | null;
}

export interface ParitySuiteReport {
  total_fixtures: number;
  passed_fixtures: number;
  failed_fixtures: number;
  negative_control_passed: boolean;
  overall_decision: string;
  fixture_results: ParityFixtureResult[];
}

export interface WaveformData {
  schema_version: number;
  asset_id: string;
  source_duration: number;
  bucket_rate: number;
  channels: number;
  peaks: number[]; // pairs of [min, max] normalized to [-1.0, 1.0]
}

export type WaveformStatus =
  | { status: "Ready"; payload: WaveformData }
  | { status: "Pending"; payload: { progress: number } }
  | { status: "Unavailable"; payload: { reason: string } };

export type PreviewVideoSourceStatus =
  | { status: "Ready"; payload: { path: string; is_proxy: boolean } }
  | { status: "Pending" }
  | { status: "Unavailable"; payload: { reason: string } };
