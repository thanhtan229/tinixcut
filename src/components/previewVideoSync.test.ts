import { describe, expect, it } from "vitest";
import {
  PLAYING_AUDIO_SEEK_TOLERANCE_SECONDS,
  shouldSeekPreviewAudio,
} from "./previewVideoSync";

describe("preview audio synchronization", () => {
  it("keeps the active audio element within the 120ms correction window", () => {
    expect(PLAYING_AUDIO_SEEK_TOLERANCE_SECONDS).toBe(0.12);
    expect(shouldSeekPreviewAudio(4, 4.12)).toBe(false);
    expect(shouldSeekPreviewAudio(4, 4.121)).toBe(true);
    expect(shouldSeekPreviewAudio(4.2, 4)).toBe(true);
  });

  it("supports a tighter caller tolerance when requested", () => {
    expect(shouldSeekPreviewAudio(1, 1.01, 0.005)).toBe(true);
    expect(shouldSeekPreviewAudio(1, 1.004, 0.005)).toBe(false);
  });
});
