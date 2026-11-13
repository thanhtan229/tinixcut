import { describe, expect, it } from "vitest";
import golden from "./__fixtures__/transitionGolden.json";
import { circleOpenWeight, isGeometricKind, transitionShape, type GeometricKind } from "./transitionGeometry";

describe("transition geometry vs edges measured on real exports", () => {
  const entries = golden as Array<{ kind: GeometricKind; progress: number; measured: number; unit: string }>;

  it("has measurements for every geometric kind", () => {
    const kinds = new Set(entries.map((entry) => entry.kind));
    for (const kind of ["WipeLeft", "WipeRight", "WipeUp", "WipeDown", "SlideLeft", "SlideRight", "CircleOpen"]) {
      expect(kinds.has(kind as GeometricKind)).toBe(true);
    }
  });

  for (const entry of entries) {
    it(`${entry.kind} at progress ${entry.progress.toFixed(2)} matches the export`, () => {
      const shape = transitionShape(entry.kind, entry.progress);
      if (entry.kind === "CircleOpen") {
        expect(shape.radiusFraction).not.toBeNull();
        // Radius in half-diagonals; measurement resolution is one pixel of 90.
        expect(Math.abs((shape.radiusFraction as number) - entry.measured)).toBeLessThan(0.05);
      } else {
        expect(shape.boundary).not.toBeNull();
        expect(Math.abs((shape.boundary as number) - entry.measured)).toBeLessThan(0.04);
      }
    });
  }

  it("is sensitive to direction (negative control)", () => {
    const left = transitionShape("WipeLeft", 0.25).boundary as number;
    const right = transitionShape("WipeRight", 0.25).boundary as number;
    expect(Math.abs(left - right)).toBeGreaterThan(0.4);
  });
});

describe("transition shapes", () => {
  it("builds the expected CSS for each family", () => {
    expect(transitionShape("WipeLeft", 0.25).incomingClipPath).toBe("inset(0 0 0 75.0000%)");
    expect(transitionShape("WipeRight", 0.25).incomingClipPath).toBe("inset(0 75.0000% 0 0)");
    expect(transitionShape("WipeUp", 0.5).incomingClipPath).toBe("inset(50.0000% 0 0 0)");
    expect(transitionShape("WipeDown", 0.5).incomingClipPath).toBe("inset(0 0 50.0000% 0)");
    const slide = transitionShape("SlideLeft", 0.25);
    expect(slide.outgoingTransform).toBe("translateX(-25.0000%)");
    expect(slide.incomingTransform).toBe("translateX(75.0000%)");
    expect(transitionShape("CircleOpen", 0.5).incomingMask).toContain("farthest-corner");
  });

  it("starts on the outgoing clip and ends on the incoming clip", () => {
    expect(circleOpenWeight(0, 0)).toBe(0);
    expect(circleOpenWeight(1, 0)).toBe(0);
    expect(circleOpenWeight(0, 1)).toBe(1);
    expect(circleOpenWeight(1, 1)).toBe(1);
    expect(transitionShape("WipeLeft", 0).boundary).toBe(1);
    expect(transitionShape("WipeLeft", 1).boundary).toBe(0);
    expect(transitionShape("SlideLeft", 1).incomingTransform).toBe("translateX(0.0000%)");
  });

  it("clamps progress and recognises only geometric kinds", () => {
    expect(transitionShape("WipeLeft", -1).boundary).toBe(1);
    expect(transitionShape("WipeLeft", 3).boundary).toBe(0);
    expect(isGeometricKind("CircleOpen")).toBe(true);
    expect(isGeometricKind("CrossDissolve")).toBe(false);
  });
});
