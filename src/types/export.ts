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
