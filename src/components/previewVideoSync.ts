const PAUSED_SEEK_TOLERANCE_SECONDS = 0.05;
const PLAYING_SEEK_TOLERANCE_SECONDS = 0.5;

export function shouldSeekPreviewVideo(
  currentTime: number,
  targetTime: number,
  isPlaying: boolean,
  isSeeking: boolean
): boolean {
  if (isSeeking) return false;
  const tolerance = isPlaying ? PLAYING_SEEK_TOLERANCE_SECONDS : PAUSED_SEEK_TOLERANCE_SECONDS;
  return Math.abs(currentTime - targetTime) > tolerance;
}

export function getPendingPreviewVideoSeekTarget(
  currentTime: number,
  pendingTargetTime: number | undefined,
  isPlaying: boolean,
  isSeeking: boolean
): number | null {
  if (pendingTargetTime === undefined || isSeeking) return null;
  return shouldSeekPreviewVideo(currentTime, pendingTargetTime, isPlaying, false)
    ? Math.max(0, pendingTargetTime)
    : null;
}
