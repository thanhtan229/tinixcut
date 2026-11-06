import { describe, expect, it } from "vitest";
import { buildTranscriptCutOperations, suggestSpeechRangesFromTranscript } from "./transcriptCut";
import type { Asset } from "../types/project";

const asset = {
  asset_id: "asset-video",
  name: "interview.mp4",
  media_kind: "Video",
  duration: { value: 1200, rate: { numerator: 30, denominator: 1 } },
} as Asset;

describe("transcript rough-cut operations", () => {
  it("groups speech around short transcript gaps and preserves padding around long gaps", () => {
    const segments = [
      { sourceStart: { value: 75, rate: { numerator: 30, denominator: 1 } }, sourceDuration: { value: 30, rate: { numerator: 30, denominator: 1 } } },
      { sourceStart: { value: 120, rate: { numerator: 30, denominator: 1 } }, sourceDuration: { value: 15, rate: { numerator: 30, denominator: 1 } } },
      { sourceStart: { value: 300, rate: { numerator: 30, denominator: 1 } }, sourceDuration: { value: 30, rate: { numerator: 30, denominator: 1 } } },
    ];
    expect(suggestSpeechRangesFromTranscript(segments, 1, 0.2, 20)).toEqual([
      { sourceInSeconds: 2.3, sourceOutSeconds: 4.7 },
      { sourceInSeconds: 9.8, sourceOutSeconds: 11.2 },
    ]);
  });

  it("clamps padding to source bounds and rejects invalid pause parameters", () => {
    const segments = [{
      sourceStart: { value: 90, rate: { numerator: 30, denominator: 1 } },
      sourceDuration: { value: 30, rate: { numerator: 30, denominator: 1 } },
    }];
    expect(suggestSpeechRangesFromTranscript(segments, 0.8, 0.5, 3.8)).toEqual([
      { sourceInSeconds: 2.5, sourceOutSeconds: 3.8 },
    ]);
    expect(suggestSpeechRangesFromTranscript([{
      ...segments[0], sourceStart: { value: 3, rate: { numerator: 30, denominator: 1 } },
    }], 0.8, 0.5, 3.8)).toEqual([{ sourceInSeconds: 0, sourceOutSeconds: 1.6 }]);
    expect(() => suggestSpeechRangesFromTranscript(segments, 0, 0.2)).toThrow(/Pause threshold/);
    expect(() => suggestSpeechRangesFromTranscript(segments, 1, 3)).toThrow(/padding/);
  });

  it("creates one reviewable track with ordered source ranges at the playhead", () => {
    const operations = buildTranscriptCutOperations(asset, [
      { sourceInSeconds: 1, sourceOutSeconds: 2.5 },
      { sourceInSeconds: 5, sourceOutSeconds: 6 },
    ], { numerator: 30, denominator: 1 }, 90, 44);
    expect(operations).toHaveLength(3);
    expect(operations[0].operation).toEqual({
      type: "TrackCreate",
      payload: { track_id: "trk_transcript_cut_44", name: "Transcript Cut · interview.mp4", kind: "Video" },
    });
    expect(operations[1].operation).toMatchObject({ type: "ClipCreate", payload: {
      item_id: "clip_transcript_cut_44_0", track_id: "trk_transcript_cut_44", asset_id: "asset-video",
      timeline_start: { value: 90 }, duration: { value: 45 }, source_start: { value: 30 },
    } });
    expect(operations[2].operation).toMatchObject({ type: "ClipCreate", payload: {
      item_id: "clip_transcript_cut_44_1", timeline_start: { value: 135 }, duration: { value: 30 }, source_start: { value: 150 },
    } });
  });

  it("rejects empty, invalid, out-of-source, and image ranges", () => {
    const rate = { numerator: 30, denominator: 1 };
    expect(() => buildTranscriptCutOperations(asset, [], rate, 0)).toThrow(/at least one/);
    expect(() => buildTranscriptCutOperations(asset, [{ sourceInSeconds: 3, sourceOutSeconds: 3 }], rate, 0)).toThrow(/invalid/);
    expect(() => buildTranscriptCutOperations(asset, [{ sourceInSeconds: 39, sourceOutSeconds: 41 }], rate, 0)).toThrow(/asset duration/);
    expect(() => buildTranscriptCutOperations({ ...asset, media_kind: "Image" } as Asset, [{ sourceInSeconds: 0, sourceOutSeconds: 1 }], rate, 0)).toThrow(/audio or video/);
  });
});
