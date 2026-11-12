// Timeline waveform geometry. The timeline re-renders on every playhead change, so
// path construction is cached per waveform object: on a 300-clip project rebuilding
// every clip's path on each seek blocked the UI thread for seconds (G54).

export type WaveformPeaks = {
  bucket_rate: number;
  // Interleaved [min, max] pairs per bucket.
  peaks: ArrayLike<number>;
};

const MAX_CACHED_PATHS_PER_WAVEFORM = 4096;
const pathCache = new WeakMap<WaveformPeaks, Map<string, string | null>>();

/** SVG path for a clip's waveform, or null when there is too little data or width to draw. */
export function waveformPathD(
  wf: WaveformPeaks,
  sourceStartF: number,
  sourceDurF: number,
  frameRate: number,
  widthPx: number,
  heightPx: number
): string | null {
  const key = `${sourceStartF}|${sourceDurF}|${frameRate}|${widthPx}|${heightPx}`;
  let cache = pathCache.get(wf);
  if (!cache) {
    cache = new Map();
    pathCache.set(wf, cache);
  }
  const cached = cache.get(key);
  if (cached !== undefined) return cached;

  const pathD = buildWaveformPathD(wf, sourceStartF, sourceDurF, frameRate, widthPx, heightPx);
  // Zoom and trim drags generate many transient keys; bound memory per waveform.
  if (cache.size >= MAX_CACHED_PATHS_PER_WAVEFORM) cache.clear();
  cache.set(key, pathD);
  return pathD;
}

function buildWaveformPathD(
  wf: WaveformPeaks,
  sourceStartF: number,
  sourceDurF: number,
  frameRate: number,
  widthPx: number,
  heightPx: number
): string | null {
  const rate = frameRate || 30;
  const sourceStartSec = sourceStartF / rate;
  const sourceDurSec = sourceDurF / rate;
  const bucketStart = Math.max(0, Math.floor(sourceStartSec * wf.bucket_rate));
  const bucketCount = Math.max(1, Math.ceil(sourceDurSec * wf.bucket_rate));
  const totalBuckets = Math.floor(wf.peaks.length / 2);
  const bucketEnd = Math.min(totalBuckets, bucketStart + bucketCount);

  const availableBuckets = bucketEnd - bucketStart;
  if (availableBuckets <= 1 || widthPx <= 4) return null;

  // Resample adaptively: up to 1 sample per 2 pixels
  const numSamples = Math.min(availableBuckets, Math.max(20, Math.floor(widthPx / 2)));
  const step = availableBuckets / numSamples;
  const midY = heightPx / 2;
  const ampScale = midY * 0.85;

  const topPoints: string[] = new Array(numSamples);
  const bottomPoints: string[] = new Array(numSamples);

  for (let i = 0; i < numSamples; i++) {
    const bIdx = Math.min(totalBuckets - 1, Math.floor(bucketStart + i * step));
    const minVal = wf.peaks[bIdx * 2] ?? 0;
    const maxVal = wf.peaks[bIdx * 2 + 1] ?? 0;
    const x = ((i / (numSamples - 1)) * widthPx).toFixed(1);
    topPoints[i] = `${x},${(midY - maxVal * ampScale).toFixed(1)}`;
    // The bottom edge is traced right-to-left to close the shape.
    bottomPoints[numSamples - 1 - i] = `${x},${(midY - minVal * ampScale).toFixed(1)}`;
  }

  return `M ${topPoints.join(" L ")} L ${bottomPoints.join(" L ")} Z`;
}
