import type { FrameScopes } from "../services/editorApi";

export type WaveformMode = "luma" | "rgb";

/**
 * RGBA pixels for the waveform: horizontal axis is the frame's x position, vertical axis is the
 * level (255 at the top). `rgb` mode adds the three channels so overlaps read as secondaries.
 */
export function waveformPixels(waveform: FrameScopes["waveform"], mode: WaveformMode): Uint8ClampedArray {
  const { columns } = waveform;
  const levels = 256;
  const out = new Uint8ClampedArray(columns * levels * 4);
  for (let column = 0; column < columns; column += 1) {
    for (let level = 0; level < levels; level += 1) {
      const source = column * levels + level;
      const target = ((levels - 1 - level) * columns + column) * 4;
      if (mode === "luma") {
        const v = waveform.luma[source];
        out[target] = v;
        out[target + 1] = v;
        out[target + 2] = v;
      } else {
        out[target] = waveform.red[source];
        out[target + 1] = waveform.green[source];
        out[target + 2] = waveform.blue[source];
      }
      out[target + 3] = 255;
    }
  }
  return out;
}

/** RGBA pixels for the vectorscope density map, drawn as a green phosphor trace. */
export function vectorscopePixels(vectorscope: FrameScopes["vectorscope"]): Uint8ClampedArray {
  const { size, density } = vectorscope;
  const out = new Uint8ClampedArray(size * size * 4);
  for (let i = 0; i < size * size; i += 1) {
    const v = density[i];
    out[i * 4] = Math.round(v * 0.35);
    out[i * 4 + 1] = v;
    out[i * 4 + 2] = Math.round(v * 0.5);
    out[i * 4 + 3] = 255;
  }
  return out;
}

/** Heights (0..1) of a histogram channel, scaled by the tallest non-extreme bin. */
export function histogramHeights(bins: number[]): number[] {
  // Ignore the two end bins when scaling: clipped black/white would otherwise flatten the rest.
  const inner = bins.slice(1, -1);
  const peak = Math.max(1, ...inner);
  return bins.map((count) => Math.min(1, count / peak));
}

export function formatPercent(share: number): string {
  return `${(share * 100).toFixed(share > 0 && share < 0.001 ? 2 : 1)}%`;
}
