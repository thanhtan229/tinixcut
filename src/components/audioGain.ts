export const MIN_AUDIO_GAIN_DB = -60;
export const MAX_AUDIO_GAIN_DB = 6;

export function linearGainToDb(gain: number): number {
  if (!Number.isFinite(gain) || gain <= 0) return MIN_AUDIO_GAIN_DB;
  return Math.max(MIN_AUDIO_GAIN_DB, Math.min(MAX_AUDIO_GAIN_DB, 20 * Math.log10(gain)));
}

export function dbToLinearGain(db: number): number {
  if (!Number.isFinite(db) || db <= MIN_AUDIO_GAIN_DB) return 0;
  return Math.min(2, 10 ** (Math.min(MAX_AUDIO_GAIN_DB, db) / 20));
}

export function formatGainDb(gain: number): string {
  if (!Number.isFinite(gain) || gain <= 0) return "−∞ dB";
  return `${linearGainToDb(gain).toFixed(1)} dB`;
}
