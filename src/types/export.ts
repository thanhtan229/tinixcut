export type ExportPreset = "balanced" | "high_quality" | "smaller_file";

export interface ExportPresetOption {
  value: ExportPreset;
  label: string;
  description: string;
}

export const EXPORT_PRESET_OPTIONS: ExportPresetOption[] = [
  {
    value: "balanced",
    label: "Balanced",
    description: "Good quality and render speed",
  },
  {
    value: "high_quality",
    label: "High quality",
    description: "Higher quality with a slower render",
  },
  {
    value: "smaller_file",
    label: "Smaller file",
    description: "Smaller MP4 with more compression",
  },
];

/** Optional integrated-loudness target applied to the exported audio. */
export interface LoudnessTarget {
  integrated_lufs: number;
  true_peak_db: number;
}

export interface LoudnessPresetOption {
  value: string;
  label: string;
  target: LoudnessTarget | null;
}

export const LOUDNESS_PRESET_OPTIONS: LoudnessPresetOption[] = [
  { value: "off", label: "Loudness: off", target: null },
  { value: "youtube", label: "YouTube / Spotify (-14 LUFS)", target: { integrated_lufs: -14, true_peak_db: -1 } },
  { value: "apple", label: "Apple Podcasts (-16 LUFS)", target: { integrated_lufs: -16, true_peak_db: -1 } },
  { value: "ebu", label: "EBU R128 broadcast (-23 LUFS)", target: { integrated_lufs: -23, true_peak_db: -1 } },
  { value: "atsc", label: "ATSC A/85 broadcast (-24 LUFS)", target: { integrated_lufs: -24, true_peak_db: -2 } },
];
