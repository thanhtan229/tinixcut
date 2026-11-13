import { describe, expect, it } from "vitest";
import { clipSpeedRatio, formatClipSpeed, parseSpeedInput, speedFromRatio } from "./project";

describe("clip speed helpers", () => {
  it("returns exact ratios for named and custom speeds", () => {
    expect(clipSpeedRatio("Half")).toEqual([1, 2]);
    expect(clipSpeedRatio("Double")).toEqual([2, 1]);
    expect(clipSpeedRatio(undefined)).toEqual([1, 1]);
    expect(clipSpeedRatio({ Custom: { numerator: 3, denominator: 2 } })).toEqual([3, 2]);
  });

  it("canonicalises ratios the way the backend does", () => {
    expect(speedFromRatio(2, 4)).toBe("Half");
    expect(speedFromRatio(5, 5)).toBe("Normal");
    expect(speedFromRatio(8, 4)).toBe("Double");
    expect(speedFromRatio(6, 4)).toEqual({ Custom: { numerator: 3, denominator: 2 } });
    expect(typeof speedFromRatio(1, 9)).toBe("string");
    expect(typeof speedFromRatio(9, 1)).toBe("string");
    expect(typeof speedFromRatio(0, 1)).toBe("string");
  });

  it("parses fractions, decimals and an x suffix", () => {
    expect(parseSpeedInput("3/2")).toEqual({ Custom: { numerator: 3, denominator: 2 } });
    expect(parseSpeedInput("0.75")).toEqual({ Custom: { numerator: 3, denominator: 4 } });
    expect(parseSpeedInput("1.5x")).toEqual({ Custom: { numerator: 3, denominator: 2 } });
    expect(parseSpeedInput("2")).toBe("Double");
    expect(parseSpeedInput("0.125")).toEqual({ Custom: { numerator: 1, denominator: 8 } });
  });

  it("rejects input the backend would also refuse", () => {
    for (const bad of ["", "abc", "0", "-1", "1/0", "0.0001", "10", "1/9"]) {
      expect(typeof parseSpeedInput(bad)).toBe("string");
    }
  });

  it("formats speeds for display", () => {
    expect(formatClipSpeed("Normal")).toBe("1×");
    expect(formatClipSpeed({ Custom: { numerator: 3, denominator: 2 } })).toBe("3/2×");
  });
});
