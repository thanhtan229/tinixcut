import { describe, expect, it } from "vitest";
import { formatPercent, histogramHeights, vectorscopePixels, waveformPixels } from "./scopeDraw";

function column(level: number, value: number): number[] {
  const data = new Array<number>(256).fill(0);
  data[level] = value;
  return data;
}

describe("scopeDraw", () => {
  it("draws the waveform with level 255 at the top row and one column per sample", () => {
    const waveform = {
      columns: 2,
      luma: [...column(255, 200), ...column(0, 100)],
      red: new Array(512).fill(0),
      green: new Array(512).fill(0),
      blue: new Array(512).fill(0),
    };
    const pixels = waveformPixels(waveform, "luma");
    expect(pixels.length).toBe(2 * 256 * 4);
    expect(pixels[(0 * 2 + 0) * 4]).toBe(200); // top row, column 0
    expect(pixels[(255 * 2 + 1) * 4]).toBe(100); // bottom row, column 1
    expect(pixels[3]).toBe(255);
  });

  it("keeps the RGB waveform channels separate", () => {
    const zeros = new Array(256).fill(0);
    const waveform = { columns: 1, luma: zeros, red: column(10, 255), green: column(20, 255), blue: zeros };
    const pixels = waveformPixels(waveform, "rgb");
    expect(pixels[(245 * 1 + 0) * 4]).toBe(255); // red at level 10
    expect(pixels[(245 * 1 + 0) * 4 + 1]).toBe(0);
    expect(pixels[(235 * 1 + 0) * 4 + 1]).toBe(255); // green at level 20
  });

  it("renders vectorscope density as a green trace", () => {
    const density = new Array(4).fill(0);
    density[3] = 255;
    const pixels = vectorscopePixels({ size: 2, density });
    expect(Array.from(pixels.slice(12, 16))).toEqual([89, 255, 128, 255]);
    expect(pixels[4]).toBe(0);
  });

  it("scales histogram heights by the tallest inner bin so clipping does not flatten the rest", () => {
    const heights = histogramHeights([1000, 5, 10, 5, 1000]);
    expect(heights[2]).toBe(1);
    expect(heights[1]).toBe(0.5);
    expect(heights[0]).toBe(1);
  });

  it("formats clipping shares", () => {
    expect(formatPercent(0)).toBe("0.0%");
    expect(formatPercent(0.0004)).toBe("0.04%");
    expect(formatPercent(0.25)).toBe("25.0%");
  });
});
