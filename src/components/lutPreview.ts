import type { ColorMatrix } from "./colorAdjustments";
import type { ChromaKey } from "../types/project";

/**
 * Preview-side 3D LUT pipeline. Export applies, per clip, `geq` color matrix -> `lut3d`
 * (trilinear). The preview shader runs the same two steps, in the same order, on the decoded
 * video frame. The pure helpers here define that math so it can be unit-tested without WebGL;
 * `LutVideoLayer.tsx` only uploads the values these helpers produce.
 */

export interface DecodedLutTable {
  size: number;
  domainMin: [number, number, number];
  domainMax: [number, number, number];
  /** Red-fastest RGB triples: index = 3 * (r + size * (g + size * b)). */
  samples: Float32Array;
}

export function decodeLutTable(table: {
  size: number;
  domainMin: [number, number, number];
  domainMax: [number, number, number];
  dataBase64: string;
}): DecodedLutTable {
  const binary = atob(table.dataBase64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  const expected = table.size ** 3 * 3 * 4;
  if (bytes.length !== expected) {
    throw new Error(`LUT table has ${bytes.length} bytes; ${expected} expected for size ${table.size}`);
  }
  // Rebuild through DataView so endianness never depends on the host.
  const view = new DataView(bytes.buffer);
  const samples = new Float32Array(bytes.length / 4);
  for (let i = 0; i < samples.length; i += 1) samples[i] = view.getFloat32(i * 4, true);
  return { size: table.size, domainMin: table.domainMin, domainMax: table.domainMax, samples };
}

/**
 * Column-major `mat3` + offset for `uniformMatrix3fv` / `uniform3fv`, from the same 3x4 matrix
 * that drives the SVG preview filter and the export `geq` expression.
 */
export function packColorMatrix(matrix: ColorMatrix): { mat3: Float32Array; offset: Float32Array } {
  const mat3 = new Float32Array(9);
  const offset = new Float32Array(3);
  for (let row = 0; row < 3; row += 1) {
    for (let column = 0; column < 3; column += 1) mat3[column * 3 + row] = matrix[row][column];
    offset[row] = matrix[row][3];
  }
  return { mat3, offset };
}

function clamp01(value: number): number {
  return Math.min(1, Math.max(0, value));
}

export function parseKeyColor(hex: string): [number, number, number] {
  const match = /^#?([0-9a-fA-F]{2})([0-9a-fA-F]{2})([0-9a-fA-F]{2})$/.exec(hex.trim());
  if (!match) throw new Error(`Invalid key color '${hex}'`);
  return [parseInt(match[1], 16), parseInt(match[2], 16), parseInt(match[3], 16)];
}

export interface KeyedPixel {
  rgb: [number, number, number];
  alpha: number;
}

/**
 * CPU reference of FFmpeg `colorkey` followed by `despill` (libavfilter/vf_colorkey.c and
 * vf_despill.c, FFmpeg 8): colorkey replaces alpha from the RGB distance to the key color;
 * despill (expand 0, only the keyed channel scaled by -1) removes the key-color spill from RGB.
 * Both read the original RGB, so spill removal does not depend on the computed alpha.
 */
export function chromaKeyPixel(rgb: [number, number, number], key: ChromaKey): KeyedPixel {
  const [kr, kg, kb] = parseKeyColor(key.key_color);
  const dr = rgb[0] * 255 - kr;
  const dg = rgb[1] * 255 - kg;
  const db = rgb[2] * 255 - kb;
  const diff = Math.sqrt((dr * dr + dg * dg + db * db) / (255 * 255 * 3));
  const alpha =
    key.blend > 0.0001
      ? clamp01((diff - key.similarity) / key.blend)
      : diff > key.similarity
        ? 1
        : 0;
  let [r, g, b] = rgb;
  if (key.despill) {
    const mix = key.despill.mix;
    const factor = 1 - mix;
    if (key.despill.screen === "Blue") {
      const spill = Math.max(b - (r * mix + g * factor), 0);
      b = Math.max(b - spill, 0);
    } else {
      const spill = Math.max(g - (r * mix + b * factor), 0);
      g = Math.max(g - spill, 0);
    }
    r = Math.min(r, 1);
    g = Math.min(g, 1);
    b = Math.min(b, 1);
  }
  return { rgb: [r, g, b], alpha };
}

/** Full per-pixel reference: chroma key, then color matrix, then LUT (export order). */
export function applyPreviewPixel(
  rgb: [number, number, number],
  matrix: ColorMatrix,
  lut: DecodedLutTable | null,
  key: ChromaKey | null
): KeyedPixel {
  const keyed = key ? chromaKeyPixel(rgb, key) : { rgb, alpha: 1 };
  return { rgb: applyPreviewPipeline(keyed.rgb, matrix, lut), alpha: keyed.alpha };
}

/** CPU reference of the fragment shader (color matrix, then trilinear LUT). */
export function applyPreviewPipeline(
  rgb: [number, number, number],
  matrix: ColorMatrix,
  lut: DecodedLutTable | null
): [number, number, number] {
  const m: [number, number, number] = [0, 1, 2].map((row) =>
    clamp01(matrix[row][0] * rgb[0] + matrix[row][1] * rgb[1] + matrix[row][2] * rgb[2] + matrix[row][3])
  ) as [number, number, number];
  if (!lut) return m;
  const n = lut.size;
  const base: number[] = [];
  const frac: number[] = [];
  for (let c = 0; c < 3; c += 1) {
    const unit =
      clamp01((m[c] - lut.domainMin[c]) / (lut.domainMax[c] - lut.domainMin[c])) * (n - 1);
    const floor = Math.min(Math.floor(unit), n - 2);
    base.push(floor);
    frac.push(unit - floor);
  }
  const out: [number, number, number] = [0, 0, 0];
  for (let dr = 0; dr < 2; dr += 1) {
    for (let dg = 0; dg < 2; dg += 1) {
      for (let db = 0; db < 2; db += 1) {
        const weight =
          (dr ? frac[0] : 1 - frac[0]) * (dg ? frac[1] : 1 - frac[1]) * (db ? frac[2] : 1 - frac[2]);
        const index = 3 * (base[0] + dr + n * (base[1] + dg + n * (base[2] + db)));
        for (let c = 0; c < 3; c += 1) out[c] += weight * lut.samples[index + c];
      }
    }
  }
  return out;
}

export const LUT_VERTEX_SHADER = `#version 300 es
out vec2 v_uv;
void main() {
  // Full-screen triangle; v_uv covers 0..1 over the visible area.
  vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
  v_uv = p;
  gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
`;

export const LUT_FRAGMENT_SHADER = `#version 300 es
precision highp float;
precision highp sampler3D;
uniform sampler2D u_video;
uniform sampler3D u_lut;
uniform mat3 u_matrix;
uniform vec3 u_offset;
uniform bool u_has_lut;
uniform vec3 u_domain_min;
uniform vec3 u_domain_max;
uniform float u_lut_size;
uniform bool u_key_enabled;
uniform vec3 u_key_color;
uniform float u_key_similarity;
uniform float u_key_blend;
uniform int u_despill_mode; // 0 off, 1 green screen, 2 blue screen
uniform float u_despill_mix;
in vec2 v_uv;
out vec4 out_color;
void main() {
  vec4 texel = texture(u_video, vec2(v_uv.x, 1.0 - v_uv.y));
  vec3 source = texel.rgb;
  float alpha = texel.a;
  if (u_key_enabled) {
    // FFmpeg colorkey: alpha from the RGB distance to the key color (0..1 units, so the
    // 255^2 * 3 divisor of the 8-bit formula becomes 3).
    vec3 delta = source - u_key_color;
    float diff = sqrt(dot(delta, delta) / 3.0);
    alpha = u_key_blend > 0.0001
      ? clamp((diff - u_key_similarity) / u_key_blend, 0.0, 1.0)
      : (diff > u_key_similarity ? 1.0 : 0.0);
    // FFmpeg despill (expand 0): subtract the spill from the keyed channel only.
    if (u_despill_mode == 1) {
      float spill = max(source.g - (source.r * u_despill_mix + source.b * (1.0 - u_despill_mix)), 0.0);
      source.g = max(source.g - spill, 0.0);
    } else if (u_despill_mode == 2) {
      float spill = max(source.b - (source.r * u_despill_mix + source.g * (1.0 - u_despill_mix)), 0.0);
      source.b = max(source.b - spill, 0.0);
    }
    source = min(source, vec3(1.0));
  }
  vec3 color = clamp(u_matrix * source + u_offset, 0.0, 1.0);
  if (u_has_lut) {
    vec3 unit = clamp((color - u_domain_min) / (u_domain_max - u_domain_min), 0.0, 1.0);
    // Sample at texel centres so the first/last LUT entries are hit exactly at 0 and 1.
    vec3 coord = unit * ((u_lut_size - 1.0) / u_lut_size) + 0.5 / u_lut_size;
    color = texture(u_lut, coord).rgb;
  }
  out_color = vec4(color, alpha);
}
`;
