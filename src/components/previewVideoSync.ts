export const DEFAULT_PAUSED_SEEK_TOLERANCE_SECONDS = 0.002;
export const PLAYING_SEEK_TOLERANCE_SECONDS = 0.5;
export const PLAYING_AUDIO_SEEK_TOLERANCE_SECONDS = 0.12;

export function shouldSeekPreviewAudio(
  currentTime: number,
  targetTime: number,
  tolerance: number = PLAYING_AUDIO_SEEK_TOLERANCE_SECONDS
): boolean {
  const difference = Math.abs(currentTime - targetTime);
  const floatingPointSlack =
    Number.EPSILON * Math.max(1, Math.abs(currentTime), Math.abs(targetTime), Math.abs(tolerance)) * 2;
  return difference - tolerance > floatingPointSlack;
}

export function shouldSeekPreviewVideo(
  currentTime: number,
  targetTime: number,
  isPlaying: boolean,
  isSeeking: boolean,
  pausedTolerance: number = DEFAULT_PAUSED_SEEK_TOLERANCE_SECONDS
): boolean {
  if (isSeeking) return false;
  const tolerance = isPlaying ? PLAYING_SEEK_TOLERANCE_SECONDS : pausedTolerance;
  return Math.abs(currentTime - targetTime) > tolerance;
}

export function getPendingPreviewVideoSeekTarget(
  currentTime: number,
  pendingTargetTime: number | undefined,
  isPlaying: boolean,
  isSeeking: boolean,
  pausedTolerance: number = DEFAULT_PAUSED_SEEK_TOLERANCE_SECONDS
): number | null {
  if (pendingTargetTime === undefined || isSeeking) return null;
  return shouldSeekPreviewVideo(currentTime, pendingTargetTime, isPlaying, false, pausedTolerance)
    ? Math.max(0, pendingTargetTime)
    : null;
}

// WebView2 can leave a paused element in `seeking` without ever raising `seeked`
// (G54: frame 14700 stalled for 5 s on both layers while an isolated retry of the
// same target settled in ~150 ms). A seek issued while one is pending is queued
// behind the stalled one, so recovery reloads the element and re-seeks from
// `loadedmetadata`. Recovery is bounded per stall episode to avoid reload loops.
// Normal paused seeks on the G54 workload settle in up to ~770 ms, so the threshold
// leaves headroom before treating a seek as stalled.
export const SEEK_STALL_RECOVERY_DELAY_MS = 1_500;
export const MAX_SEEK_STALL_RECOVERIES = 1;

export function shouldRecoverStalledSeek(
  isSeeking: boolean,
  isCurrentElement: boolean,
  recoveriesUsed: number,
  maxRecoveries: number = MAX_SEEK_STALL_RECOVERIES
): boolean {
  return isSeeking && isCurrentElement && recoveriesUsed < maxRecoveries;
}
