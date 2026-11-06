export const AUDIO_METER_FLOOR_DBFS = -60;

export type PlaybackMeterStatus = "idle" | "available" | "unsupported";

export function hasAudioCaptureTrack(stream: Pick<MediaStream, "getAudioTracks">): boolean {
  return stream.getAudioTracks().length > 0;
}

export function playbackMeterStatus(tapCount: number, hasUnsupportedSource: boolean): PlaybackMeterStatus {
  if (tapCount > 0) return "available";
  return hasUnsupportedSource ? "unsupported" : "idle";
}

export function playbackMeterValueText(dbfs: number | null, status: PlaybackMeterStatus): string {
  if (status === "unsupported") return "N/A";
  if (dbfs === null) return "Meter idle";
  return `${dbfs.toFixed(1)} dBFS`;
}

export function getPlaybackMeterElement<TAudio, TVideo>(
  itemId: string,
  audioElements: Readonly<Record<string, TAudio | null>>,
  videoElements: Readonly<Record<string, TVideo | null>>
): TAudio | TVideo | null {
  return audioElements[itemId] ?? videoElements[itemId] ?? null;
}

export function peakDbfs(samples: ArrayLike<number>, floorDbfs = AUDIO_METER_FLOOR_DBFS): number {
  let peak = 0;
  for (let index = 0; index < samples.length; index += 1) {
    const value = samples[index];
    if (Number.isFinite(value)) peak = Math.max(peak, Math.abs(value));
  }
  if (peak <= 0) return floorDbfs;
  return Math.max(floorDbfs, Math.min(0, 20 * Math.log10(peak)));
}

export function peakMeterPercent(dbfs: number, floorDbfs = AUDIO_METER_FLOOR_DBFS): number {
  if (!Number.isFinite(dbfs)) return 0;
  return Math.max(0, Math.min(100, ((dbfs - floorDbfs) / -floorDbfs) * 100));
}
