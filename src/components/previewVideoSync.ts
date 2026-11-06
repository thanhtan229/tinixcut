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
