import { describe, expect, it } from "vitest";
import { visualColorMatrix } from "./colorAdjustments";
import {
  applyPreviewPipeline,
  decodeLutTable,
  LUT_FRAGMENT_SHADER,
  packColorMatrix,
  type DecodedLutTable,
} from "./lutPreview";

function identityLut(size: number): DecodedLutTable {
  const samples = new Float32Array(size ** 3 * 3);
  let i = 0;
  for (let b = 0; b < size; b += 1) {
    for (let g = 0; g < size; g += 1) {
      for (let r = 0; r < size; r += 1) {
        samples[i++] = r / (size - 1);
        samples[i++] = g / (size - 1);
        samples[i++] = b / (size - 1);
      }
    }
  }
  return { size, domainMin: [0, 0, 0], domainMax: [1, 1, 1], samples };
}

function swapRedBlue(lut: DecodedLutTable): DecodedLutTable {
  const samples = new Float32Array(lut.samples);
  for (let i = 0; i < samples.length; i += 3) {
    const r = samples[i];
    samples[i] = samples[i + 2];
    samples[i + 2] = r;
  }
  return { ...lut, samples };
}

const neutral = visualColorMatrix("None", undefined);

function toBase64(floats: number[]): string {
  const view = new DataView(new ArrayBuffer(floats.length * 4));
  floats.forEach((value, index) => view.setFloat32(index * 4, value, true));
  return btoa(String.fromCharCode(...new Uint8Array(view.buffer)));
}

describe("lutPreview pipeline", () => {
  it("is the identity for a neutral matrix and an identity LUT", () => {
    const lut = identityLut(5);
    for (const rgb of [[0, 0, 0], [0.3, 0.6, 0.9], [1, 1, 1], [0.11, 0.97, 0.5]] as const) {
      const out = applyPreviewPipeline([...rgb], neutral, lut);
      out.forEach((value, c) => expect(value).toBeCloseTo(rgb[c], 5));
    }
  });

  it("maps channels through the LUT red-fastest, like the export reference", () => {
    const out = applyPreviewPipeline([1, 0, 0], neutral, swapRedBlue(identityLut(3)));
    expect(out).toEqual([0, 0, 1]);
  });

  it("applies the color matrix before the LUT, clamping in between", () => {
    const brighter = visualColorMatrix("None", { brightness: 0.6, contrast: 1, saturation: 1, temperature: 0 });
    const matrixOnly = applyPreviewPipeline([0.7, 0.7, 0.7], brighter, null);
    expect(matrixOnly).toEqual([1, 1, 1]); // clamped to the 8-bit range between the two steps
    const lut = swapRedBlue(identityLut(3));
    expect(applyPreviewPipeline([0.7, 0.7, 0.7], brighter, lut)).toEqual([1, 1, 1]);
  });

  it("interpolates trilinearly between LUT corners", () => {
    const lut: DecodedLutTable = {
      size: 2,
      domainMin: [0, 0, 0],
      domainMax: [1, 1, 1],
      // Output is 1 only when blue is 1, otherwise 0 (all channels).
      samples: new Float32Array([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1]),
    };
    const out = applyPreviewPipeline([0.2, 0.9, 0.25], neutral, lut);
    out.forEach((value) => expect(value).toBeCloseTo(0.25, 6));
  });

  it("scales the input by the LUT domain", () => {
    const lut = { ...identityLut(2), domainMax: [2, 2, 2] as [number, number, number] };
    const out = applyPreviewPipeline([1, 1, 1], neutral, lut);
    out.forEach((value) => expect(value).toBeCloseTo(0.5, 6));
  });

  it("is sensitive to a wrong LUT (negative control)", () => {
    const wrong = applyPreviewPipeline([0.9, 0.2, 0.1], neutral, swapRedBlue(identityLut(5)));
    const right = applyPreviewPipeline([0.9, 0.2, 0.1], neutral, identityLut(5));
    expect(Math.abs(wrong[0] - right[0])).toBeGreaterThan(0.5);
  });
});

describe("lutPreview uniforms and decoding", () => {
  it("packs the 3x4 matrix column-major for uniformMatrix3fv", () => {
    const matrix = [
      [1, 2, 3, 0.1],
      [4, 5, 6, 0.2],
      [7, 8, 9, 0.3],
    ] as unknown as Parameters<typeof packColorMatrix>[0];
    const { mat3, offset } = packColorMatrix(matrix);
    expect(Array.from(mat3)).toEqual([1, 4, 7, 2, 5, 8, 3, 6, 9]);
    expect(Array.from(offset).map((v) => Number(v.toFixed(2)))).toEqual([0.1, 0.2, 0.3]);
  });

  it("decodes little-endian float tables and rejects a wrong length", () => {
    const values = Array.from({ length: 8 * 3 }, (_, i) => i / 24);
    const table = decodeLutTable({
      size: 2,
      domainMin: [0, 0, 0],
      domainMax: [1, 1, 1],
      dataBase64: toBase64(values),
    });
    expect(table.samples.length).toBe(24);
    expect(table.samples[5]).toBeCloseTo(5 / 24, 6);
    expect(() =>
      decodeLutTable({ size: 3, domainMin: [0, 0, 0], domainMax: [1, 1, 1], dataBase64: toBase64(values) })
    ).toThrow(/expected/);
  });

  it("keeps the shader contract the component binds to", () => {
    for (const name of ["u_video", "u_lut", "u_matrix", "u_offset", "u_has_lut", "u_domain_min", "u_domain_max", "u_lut_size"]) {
      expect(LUT_FRAGMENT_SHADER).toContain(name);
    }
  });
});

import goldenVectors from "./__fixtures__/chromaKeyGolden.json";
import { chromaKeyPixel, applyPreviewPixel, parseKeyColor } from "./lutPreview";
import type { ChromaKey } from "../types/project";

describe("chroma key preview reference vs measured FFmpeg output", () => {
  for (const entry of goldenVectors as Array<{
    key: ChromaKey;
    samples: Array<{ input: number[]; output: number[]; alpha: number }>;
  }>) {
    it(`matches FFmpeg colorkey/despill for ${entry.key.key_color} sim ${entry.key.similarity} blend ${entry.key.blend}${entry.key.despill ? " +despill" : ""}`, () => {
      expect(entry.samples.length).toBeGreaterThan(50);
      let maxRgb = 0;
      let maxAlpha = 0;
      for (const sample of entry.samples) {
        const keyed = chromaKeyPixel(
          [sample.input[0] / 255, sample.input[1] / 255, sample.input[2] / 255],
          entry.key
        );
        for (let c = 0; c < 3; c += 1) {
          maxRgb = Math.max(maxRgb, Math.abs(keyed.rgb[c] * 255 - sample.output[c]));
        }
        maxAlpha = Math.max(maxAlpha, Math.abs(keyed.alpha * 255 - sample.alpha));
      }
      // FFmpeg truncates to 8 bits, so the float reference may differ by just under one level.
      expect(maxRgb).toBeLessThan(1.01);
      expect(maxAlpha).toBeLessThan(1.01);
    });
  }

  it("is sensitive to the key color (negative control)", () => {
    const entry = (goldenVectors as Array<{ key: ChromaKey; samples: Array<{ input: number[]; alpha: number }> }>)[1];
    const wrong: ChromaKey = { ...entry.key, key_color: "#ff0000" };
    const worst = Math.max(
      ...entry.samples.map((s) =>
        Math.abs(
          chromaKeyPixel([s.input[0] / 255, s.input[1] / 255, s.input[2] / 255], wrong).alpha * 255 - s.alpha
        )
      )
    );
    expect(worst).toBeGreaterThan(40);
  });

  it("chains key, color matrix and LUT in export order", () => {
    const key: ChromaKey = { key_color: "#00ff00", similarity: 0.1, blend: 0, despill: null };
    const pixel = applyPreviewPixel([0, 1, 0], visualColorMatrixNeutral(), null, key);
    expect(pixel.alpha).toBe(0);
    const kept = applyPreviewPixel([1, 0, 0], visualColorMatrixNeutral(), null, key);
    expect(kept.alpha).toBe(1);
    expect(parseKeyColor("#FF8000")).toEqual([255, 128, 0]);
    expect(() => parseKeyColor("green")).toThrow();
  });
});

function visualColorMatrixNeutral() {
  return neutral;
}
