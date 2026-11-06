import { describe, expect, it } from "vitest";
import {
  colorFilterId,
  NEUTRAL_COLOR_ADJUSTMENTS,
  svgColorMatrixValues,
  visualColorMatrix,
} from "./colorAdjustments";

describe("SDR color adjustments", () => {
  it("keeps the default transform as identity", () => {
    expect(visualColorMatrix("None", NEUTRAL_COLOR_ADJUSTMENTS)).toEqual([
      [1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0],
    ]);
  });

  it("combines the preset and adjustments in one affine matrix", () => {
    const matrix = visualColorMatrix("Vivid", {
      brightness: 0.1,
      contrast: 1.5,
      saturation: 0.8,
      temperature: 0.5,
    });
    expect(matrix).toHaveLength(3);
    expect(matrix[0]).toHaveLength(4);
    expect(matrix[0][0]).toBeCloseTo(1.12 * 1.5 * (0.2126 * (1 - 1.25 * 0.8) + 1.25 * 0.8) * 1.06);
    expect(matrix[0][3]).toBeCloseTo((0.5 * (1 - 1.68) + 0.1) * 1.06);
  });

  it("serializes the SVG matrix and sanitizes filter IDs", () => {
    expect(svgColorMatrixValues(visualColorMatrix("None", NEUTRAL_COLOR_ADJUSTMENTS)).split(" "))
      .toHaveLength(20);
    expect(colorFilterId("clip:one/2")).toBe("tinix-color-clip_one_2");
  });
});
