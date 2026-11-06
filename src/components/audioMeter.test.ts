import { describe, expect, it } from "vitest";
import {
  getPlaybackMeterElement,
  hasAudioCaptureTrack,
  peakDbfs,
  peakMeterPercent,
  playbackMeterStatus,
  playbackMeterValueText,
} from "./audioMeter";

describe("playback peak meter", () => {
  it("finds standalone audio and embedded video audio elements by timeline item id", () => {
    const audioElement = { kind: "audio" };
    const videoElement = { kind: "video" };

    expect(getPlaybackMeterElement("audio-1", { "audio-1": audioElement }, {})).toBe(audioElement);
    expect(getPlaybackMeterElement("video-1", {}, { "video-1": videoElement })).toBe(videoElement);
    expect(getPlaybackMeterElement("missing", {}, {})).toBeNull();
  });

  it("reports silence at the display floor and full scale at 0 dBFS", () => {
    expect(peakDbfs([0, 0, 0])).toBe(-60);
    expect(peakDbfs([1, -1])).toBe(0);
    expect(peakMeterPercent(-60)).toBe(0);
    expect(peakMeterPercent(0)).toBe(100);
  });

  it("measures peak amplitude, ignores non-finite samples, and clamps over-range samples", () => {
    expect(peakDbfs([0.5, Number.NaN, Number.POSITIVE_INFINITY])).toBeCloseTo(-6.0206, 3);
    expect(peakDbfs([1.5])).toBe(0);
    expect(peakMeterPercent(-30)).toBe(50);
  });

  it("distinguishes an unavailable capture stream from an idle meter", () => {
    expect(hasAudioCaptureTrack({ getAudioTracks: () => [] })).toBe(false);
    expect(hasAudioCaptureTrack({ getAudioTracks: () => [{} as MediaStreamTrack] })).toBe(true);
    expect(playbackMeterStatus(0, true)).toBe("unsupported");
    expect(playbackMeterStatus(0, false)).toBe("idle");
    expect(playbackMeterStatus(1, true)).toBe("available");
  });

  it("exposes unsupported, idle, and measured meter states accessibly", () => {
    expect(playbackMeterValueText(null, "unsupported")).toBe("N/A");
    expect(playbackMeterValueText(null, "idle")).toBe("Meter idle");
    expect(playbackMeterValueText(-12.34, "available")).toBe("-12.3 dBFS");
  });
});
