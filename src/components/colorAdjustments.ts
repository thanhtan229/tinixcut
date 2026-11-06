import type { ColorAdjustments, VisualEffectPreset } from "../types/project";

export const NEUTRAL_COLOR_ADJUSTMENTS: ColorAdjustments = {
  brightness: 0,
  contrast: 1,
  saturation: 1,
  temperature: 0,
};

export type ColorMatrix = [number, number, number, number][];

export function visualColorMatrix(
  effect: VisualEffectPreset | undefined,
  adjustments: ColorAdjustments | undefined
): ColorMatrix {
  const value = adjustments ?? NEUTRAL_COLOR_ADJUSTMENTS;
  const preset = effect === "Mono"
    ? { contrast: 1, saturation: 0 }
    : effect === "Vivid"
      ? { contrast: 1.12, saturation: 1.25 }
      : effect === "Muted"
        ? { contrast: 0.96, saturation: 0.7 }
        : { contrast: 1, saturation: 1 };
  const contrast = preset.contrast * value.contrast;
  const saturation = preset.saturation * value.saturation;
  const offset = 0.5 * (1 - contrast) + value.brightness;
  const temperature = value.temperature * 0.12;
  const gains = [1 + temperature, 1, 1 - temperature];
  const luma = [0.2126, 0.7152, 0.0722];
  return [0, 1, 2].map((row) => [0, 1, 2, 3].map((column) => {
    if (column === 3) return gains[row] * offset;
    const saturationComponent = (row === column ? saturation : 0)
      + luma[column] * (1 - saturation);
    return gains[row] * contrast * saturationComponent;
  }) as [number, number, number, number]);
}

export function svgColorMatrixValues(matrix: ColorMatrix): string {
  return [
    ...matrix.flatMap((row) => [row[0], row[1], row[2], 0, row[3]]),
    0, 0, 0, 1, 0,
  ].join(" ");
}

export function colorFilterId(itemId: string): string {
  return `tinix-color-${itemId.replace(/[^a-zA-Z0-9_-]/g, "_")}`;
}

export function colorFilterUrl(itemId: string): string {
  return `url(#${colorFilterId(itemId)})`;
}
