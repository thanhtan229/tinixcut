export type PreviewPlaybackKind = "audio" | "video";

export function formatPreviewPlaybackFailure(kind: PreviewPlaybackKind, error: unknown): string {
  const fallback = kind === "audio" ? "Audio playback could not be started." : "Video playback could not be started.";
  const message =
    error instanceof Error && error.message.trim()
      ? error.message
      : typeof error === "string" && error.trim()
        ? error
        : fallback;

  return kind === "audio"
    ? `Audio playback failed: ${message}`
    : `Video preview playback failed: ${message}`;
}
