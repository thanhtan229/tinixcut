import { describe, expect, it } from "vitest";
import {
  canvasPositionFromPointerDelta,
  canvasCropFromPointerDelta,
  canvasRotationFromPointerDelta,
  canvasScaleFromPointerDelta,
  canvasTransformFromKeyboard,
} from "./canvasDrag";

describe("canvas transform drag", () => {
  it("converts viewport pointer movement back to canonical project pixels", () => {
    expect(canvasPositionFromPointerDelta({ x: 12, y: -8 }, 36, -18, 0.5)).toEqual({ x: 84, y: -44 });
  });

  it("preserves the starting position when the canvas scale is invalid", () => {
    const start = { x: 12, y: -8 };
    expect(canvasPositionFromPointerDelta(start, 36, -18, 0)).toBe(start);
    expect(canvasPositionFromPointerDelta(start, 36, -18, Number.NaN)).toBe(start);
  });
});

describe("canvas keyboard transform", () => {
  const transform = { position_x: 12, position_y: -8, scale_x: 2, scale_y: 1.5, rotation: 15 };

  it("scales uniformly while preserving position and non-uniform scale ratio", () => {
    const next = canvasTransformFromKeyboard(transform, "ArrowUp", { control: true, alt: false, shift: false });
    expect(next?.scale_x).toBeCloseTo(2.1);
    expect(next?.scale_y).toBeCloseTo(1.575);
    expect(next?.scale_x! / next?.scale_y!).toBeCloseTo(transform.scale_x / transform.scale_y);
  });

  it("rotates in one or ten degree steps without changing scale or position", () => {
    expect(canvasTransformFromKeyboard(transform, "ArrowLeft", { control: false, alt: true, shift: false }))
      .toEqual({ ...transform, rotation: 14 });
    expect(canvasTransformFromKeyboard(transform, "ArrowRight", { control: false, alt: true, shift: true }))
      .toEqual({ ...transform, rotation: 25 });
  });

  it("clamps scale and ignores ambiguous modifier combinations", () => {
    const bounded = canvasTransformFromKeyboard({ ...transform, scale_x: 19.9, scale_y: 0.051 }, "ArrowUp", { control: true, alt: false, shift: true });
    expect(bounded?.scale_x).toBeCloseTo(20);
    expect(bounded?.scale_y).toBeCloseTo(0.051 * (20 / 19.9));
    expect(bounded?.scale_x! / bounded?.scale_y!).toBeCloseTo(19.9 / 0.051);
    expect(canvasTransformFromKeyboard(transform, "ArrowUp", { control: false, alt: false, shift: false })).toBeNull();
    expect(canvasTransformFromKeyboard(transform, "ArrowUp", { control: true, alt: true, shift: false })).toBeNull();
  });
});

describe("canvas pointer transform", () => {
  const transform = { position_x: 12, position_y: -8, scale_x: 2, scale_y: 1.5, rotation: 15 };

  it("scales exponentially, preserves non-uniform ratio, and respects bounds", () => {
    const next = canvasScaleFromPointerDelta(transform, 30);
    expect(next.scale_x).toBeCloseTo(2 * Math.exp(1 / 6));
    expect(next.scale_x / next.scale_y).toBeCloseTo(transform.scale_x / transform.scale_y);
    const bounded = canvasScaleFromPointerDelta({ ...transform, scale_x: 19, scale_y: 10 }, 500);
    expect(bounded.scale_x).toBeCloseTo(20);
    expect(bounded.scale_x / bounded.scale_y).toBeCloseTo(19 / 10);
    expect(canvasScaleFromPointerDelta({ ...transform, scale_x: 0 }, 20)).toEqual({ ...transform, scale_x: 0 });
  });

  it("rotates at a half degree per horizontal pointer pixel", () => {
    expect(canvasRotationFromPointerDelta(transform, -18)).toEqual({ ...transform, rotation: 6 });
    expect(canvasRotationFromPointerDelta(transform, Number.NaN)).toBe(transform);
  });
});

describe("canvas crop handles", () => {
  const crop = { top: 0.1, right: 0.1, bottom: 0.1, left: 0.1 };

  it("moves crop edges in the selected layer's rotated local coordinates", () => {
    expect(canvasCropFromPointerDelta(crop, "left", 20, 0, 200, 100, 0).left).toBeCloseTo(0.2);
    expect(canvasCropFromPointerDelta(crop, "top", 0, 10, 200, 100, 0).top).toBeCloseTo(0.2);
    expect(canvasCropFromPointerDelta(crop, "left", 0, 20, 200, 100, 90).left).toBeCloseTo(0.2);
  });

  it("keeps at least one source pixel fraction visible and rejects invalid geometry", () => {
    expect(canvasCropFromPointerDelta(crop, "left", 1000, 0, 200, 100, 0).left).toBeCloseTo(0.899);
    expect(canvasCropFromPointerDelta(crop, "top", 0, -1000, 200, 100, 0).top).toBe(0);
    expect(canvasCropFromPointerDelta(crop, "left", 10, 0, 0, 100, 0)).toBe(crop);
    expect(canvasCropFromPointerDelta({ ...crop, left: 0.95 }, "left", 10, 0, 200, 100, 0))
      .toEqual({ ...crop, left: 0.95 });
  });
});
