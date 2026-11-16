import { describe, expect, it } from "vitest";
import { Project } from "../types/project";
import { buildAudioMixPlan } from "./audioMixPlan";

function fixture(musicKeyframes: Array<{ property: "Volume"; offset: { value: number; rate: { numerator: number; denominator: number } }; value: number }> = []): Project {
  const rate = { numerator: 30, denominator: 1 };
  const item = (item_id: string, start: number, duration: number, kind: "Audio" | "Video", asset_id: string) => ({
    item_id, asset_id, kind, name: item_id, volume: 1, embedded_audio_enabled: true,
    timeline_range: { start: { value: start * 30, rate }, duration: { value: duration * 30, rate } },
    source_range: { start: { value: 0, rate }, duration: { value: duration * 30, rate } },
    keyframes: item_id === "music" ? musicKeyframes : [],
  });
  return {
    project_id: "p1", revision: 8, schema_version: 16,
    settings: { frame_rate: rate },
    tracks: [
      { track_id: "voice", items: [item("voice", 4, 3, "Video", "voice-asset")] },
      { track_id: "music-track", items: [item("music", 0, 12, "Audio", "music-asset")] },
    ],
  } as unknown as Project;
}

const options = {
  voiceItemIds: ["voice"], musicItemIds: ["music"], normalizeTargetDbfs: -18,
  duckDb: -12, attackSeconds: 0.5, releaseSeconds: 1,
  levelsByAssetId: { "music-asset": { source_fingerprint: "sha", mean_dbfs: -30, peak_dbfs: -4, measurement: "FFmpeg volumedetect" } },
};

describe("reviewed audio mix plan", () => {
  it("rejects locked music and stale selections while allowing locked voice as read-only reference",()=>{
    const p=fixture();p.tracks[1].name="Music";p.tracks[1].locked=true;
    expect(()=>buildAudioMixPlan(p,options)).toThrow("Unlock track Music");
    p.tracks[1].locked=false;p.tracks[0].locked=true;
    expect(buildAudioMixPlan(p,options).validation.valid).toBe(true);
    expect(()=>buildAudioMixPlan(p,{...options,musicItemIds:["missing"]})).toThrow("selected audio clips changed");
    expect(()=>buildAudioMixPlan(p,{...options,duckDb:-60})).toThrow("Ducking must");
  });
  it("creates peak-limited normalization and a timeline-relative duck envelope", () => {
    const plan = buildAudioMixPlan(fixture(), options);
    const normalize = plan.plan.operations.find(({ operation }) => operation.type === "ClipVolumeSet");
    expect(normalize?.operation.type === "ClipVolumeSet" && normalize.operation.payload.volume).toBeCloseTo(10 ** (3 / 20));

    const duck = plan.plan.operations.filter(({ operation }) => operation.type === "ClipKeyframeAdd");
    expect(duck).toHaveLength(5);
    if (duck.some(({ operation }) => operation.type !== "ClipKeyframeAdd")) throw new Error("Expected volume keyframes");
    const keyframes = duck.map(({ operation }) => operation.type === "ClipKeyframeAdd" ? operation.payload.keyframe : null);
    expect(keyframes.map((keyframe) => keyframe?.offset.value)).toEqual([0, 105, 120, 210, 240]);
    expect(keyframes[2]?.value).toBeCloseTo(10 ** (-12 / 20));
    expect(plan.plan.baseRevision).toBe(8);
    expect(plan.plan.assumptions[0]).toContain("not LUFS");
  });

  it("rejects clips with existing volume automation instead of replacing it silently", () => {
    expect(() => buildAudioMixPlan(fixture([{ property: "Volume", offset: { value: 30, rate: { numerator: 30, denominator: 1 } }, value: 0.7 }]), options))
      .toThrow("already has volume automation");
  });

  it("requires source analysis for normalization", () => {
    expect(() => buildAudioMixPlan(fixture(), { ...options, levelsByAssetId: {} }))
      .toThrow("Analyze music before");
  });
});
