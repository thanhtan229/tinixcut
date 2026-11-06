import { describe, expect, it } from "vitest";
import { formatPreviewPlaybackFailure } from "./previewPlaybackErrors";

describe("preview playback errors", () => {
  it("keeps video start failures out of the audio error channel", () => {
    expect(formatPreviewPlaybackFailure("video", new Error("decoder unavailable"))).toBe(
      "Video preview playback failed: decoder unavailable"
    );
  });

  it("preserves useful audio diagnostics", () => {
    expect(formatPreviewPlaybackFailure("audio", "media element rejected playback")).toBe(
      "Audio playback failed: media element rejected playback"
    );
  });

  it("provides a fallback when the browser error message is empty", () => {
    expect(formatPreviewPlaybackFailure("audio", new Error("  "))).toBe(
      "Audio playback failed: Audio playback could not be started."
    );
  });
});
