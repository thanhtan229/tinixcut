import { describe, expect, it } from "vitest";
import {
  exactFrameIsCurrent,
  exactFrameWidth,
  exactFrameZIndex,
  ExactFrameRequestState,
} from "./exactFrame";
import type { ParityFrame } from "../services/editorApi";

const base: ExactFrameRequestState = {
  enabled: true,
  isPlaying: false,
  gestureActive: false,
  hasProject: true,
  viewportWidth: 720,
  devicePixelRatio: 2,
};

const frame: ParityFrame = {
  frame: 90,
  width: 1440,
  revision: 7,
  pngBase64: "AAAA",
  unavailable: null,
  textItemIds: [],
  renderMillis: 400,
};

describe("exact frame", () => {
  it("requests device-pixel width only while paused with the feature on", () => {
    expect(exactFrameWidth(base)).toBe(1440);
    expect(exactFrameWidth({ ...base, devicePixelRatio: 0.5 })).toBe(720);
    expect(exactFrameWidth({ ...base, enabled: false })).toBeNull();
    expect(exactFrameWidth({ ...base, isPlaying: true })).toBeNull();
    expect(exactFrameWidth({ ...base, gestureActive: true })).toBeNull();
    expect(exactFrameWidth({ ...base, hasProject: false })).toBeNull();
  });

  it("is shown only for the same frame, revision and width", () => {
    expect(exactFrameIsCurrent(frame, 90, 7, 1440)).toBe(true);
    expect(exactFrameIsCurrent(frame, 91, 7, 1440)).toBe(false);
    expect(exactFrameIsCurrent(frame, 90, 8, 1440)).toBe(false);
    expect(exactFrameIsCurrent(frame, 90, 7, 720)).toBe(false);
    expect(exactFrameIsCurrent({ ...frame, pngBase64: null, unavailable: "transition" }, 90, 7, 1440)).toBe(false);
    expect(exactFrameIsCurrent(null, 90, 7, 1440)).toBe(false);
  });

  it("sits above picture layers and ignores text layers", () => {
    expect(exactFrameZIndex([])).toBe(1);
    expect(
      exactFrameZIndex([
        { kind: "Video", z_index: 2 },
        { kind: "Image", z_index: 5 },
        { kind: "Text", z_index: 9 },
      ]),
    ).toBe(6);
  });
});
