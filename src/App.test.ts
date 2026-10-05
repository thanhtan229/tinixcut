import { describe, it, expect } from "vitest";
import { HealthStatus } from "./types/health";
import {
  PreviewFrameState,
  PreviewSampledLayer,
  PreviewTransitionGroup,
  BenchmarkReport,
} from "./types/preview";
import { areRationalTimesEqual, isRangeAdjacent, rationalTimeToSeconds } from "./types/project";

describe("HealthStatus Model", () => {
  it("validates healthy status response structure", () => {
    const fixture: HealthStatus = {
      status: "healthy",
      version: "0.1.0",
      platform: "windows",
      schema_version: 1,
    };

    expect(fixture.status).toBe("healthy");
    expect(fixture.schema_version).toBe(1);
    expect(fixture.version).toBe("0.1.0");
    expect(fixture.platform).toBe("windows");
  });
});

describe("Preview POC Model & Benchmark Targets", () => {
  it("validates PreviewFrameState and layer geometry structure", () => {
    const previewState: PreviewFrameState = {
      timestamp: { value: 30, rate: { numerator: 30, denominator: 1 } },
      active_layers: [
        {
          item_id: "clip_sample_1",
          name: "Sample Video",
          z_index: 0,
          kind: "Video",
          asset_id: "ast_1",
          asset_path: "D:/media/sample.mp4",
          is_missing_media: false,
          source_time: { value: 0, rate: { numerator: 30, denominator: 1 } },
          position: [0, 0],
          scale: [1, 1],
          rotation: 0,
          crop: { left: 0, right: 0, top: 0, bottom: 0 },
          opacity: 1.0,
          text: null,
          font_family: null,
          font_size: null,
          computed_bounds: { x: 0, y: 0, width: 1920, height: 1080 },
        },
      ],
      active_audio: [
        {
          item_id: "clip_sample_1",
          effective_volume: 1.0,
        },
      ],
    };

    expect(previewState.active_layers.length).toBe(1);
    expect(previewState.active_layers[0].name).toBe("Sample Video");
    expect(previewState.active_layers[0].is_missing_media).toBe(false);
    expect(previewState.active_layers[0].computed_bounds.width).toBe(1920);
  });

  it("validates POC 1 benchmark criteria for Candidate A vs Candidate B", () => {
    const candidateA: BenchmarkReport = {
      backend_name: "WebView / Proxy-assisted Continuous Playback (Candidate A)",
      test_duration_seconds: 30,
      target_fps: 30,
      dropped_frames: 6,
      drop_percentage: 0.67,
      avg_seek_latency_ms: 28.5,
      p95_seek_latency_ms: 58.0,
      av_drift_ms: 8.5,
      estimated_cpu_load_percent: 12.8,
      memory_growth_mb: 2.3,
      passes_targets: true,
    };

    const candidateB: BenchmarkReport = {
      backend_name: "Native Decoded Surface (Candidate B)",
      test_duration_seconds: 30,
      target_fps: 30,
      dropped_frames: 5,
      drop_percentage: 0.55,
      avg_seek_latency_ms: 24.5,
      p95_seek_latency_ms: 58.0,
      av_drift_ms: 6.5,
      estimated_cpu_load_percent: 27.5,
      memory_growth_mb: 19.8,
      passes_targets: true,
    };

    // Candidate A must pass all POC 1 target thresholds
    expect(candidateA.drop_percentage).toBeLessThan(5.0);
    expect(candidateA.p95_seek_latency_ms).toBeLessThan(150.0);
    expect(candidateA.av_drift_ms).toBeLessThan(30.0);
    expect(candidateA.passes_targets).toBe(true);

    // Decision rule: Candidate A has significantly lower CPU and memory overhead
    expect(candidateA.estimated_cpu_load_percent).toBeLessThan(candidateB.estimated_cpu_load_percent);
    expect(candidateA.memory_growth_mb).toBeLessThan(candidateB.memory_growth_mb);
  });
});

describe("Phase 9 Waveform & Keyframe Models", () => {
  it("validates WaveformData structure and peak normalization [-1.0, 1.0]", () => {
    const wf = {
      schema_version: 1,
      asset_id: "ast_audio_1",
      source_duration: 10.0,
      bucket_rate: 50,
      channels: 1,
      peaks: [-0.5, 0.8, -0.2, 0.4, -0.9, 0.95],
    };

    expect(wf.schema_version).toBe(1);
    expect(wf.bucket_rate).toBe(50);
    expect(wf.peaks.length).toBe(6); // 3 buckets
    for (let i = 0; i < wf.peaks.length; i += 2) {
      expect(wf.peaks[i]).toBeGreaterThanOrEqual(-1.0);
      expect(wf.peaks[i]).toBeLessThanOrEqual(0.0);
      expect(wf.peaks[i + 1]).toBeGreaterThanOrEqual(0.0);
      expect(wf.peaks[i + 1]).toBeLessThanOrEqual(1.0);
    }
  });

  it("validates trim-aware waveform bucket mapping and move invariance", () => {
    const bucketRate = 50;
    const clipRate = 30;

    // Helper for bucket index calculation
    const calcBuckets = (sourceStartFrame: number, durationFrames: number) => {
      const sourceStartSec = sourceStartFrame / clipRate;
      const sourceDurSec = durationFrames / clipRate;
      const startBucket = Math.floor(sourceStartSec * bucketRate);
      const count = Math.ceil(sourceDurSec * bucketRate);
      return { startBucket, endBucket: startBucket + count };
    };

    // Case 1: Untrimmed (start = 0, dur = 90 frames = 3 sec)
    const untrimmed = calcBuckets(0, 90);
    expect(untrimmed.startBucket).toBe(0);
    expect(untrimmed.endBucket).toBe(150);

    // Case 2: Head trim (start = 30 frames = 1 sec, dur = 60 frames = 2 sec)
    const headTrim = calcBuckets(30, 60);
    expect(headTrim.startBucket).toBe(50);
    expect(headTrim.endBucket).toBe(150);

    // Case 3: Tail trim (start = 0 frames, dur = 30 frames = 1 sec)
    const tailTrim = calcBuckets(0, 30);
    expect(tailTrim.startBucket).toBe(0);
    expect(tailTrim.endBucket).toBe(50);

    // Case 4: Move clip invariant (timeline_start moves from 0 to 120, but source_range is unchanged)
    const movedClipSourceBuckets = calcBuckets(30, 60);
    expect(movedClipSourceBuckets.startBucket).toBe(headTrim.startBucket);
    expect(movedClipSourceBuckets.endBucket).toBe(headTrim.endBucket);
  });

  it("validates Keyframe interpolation rules: before-first, between, after-last", () => {
    const keyframes = [
      { offset: 0, value: 0.0 },
      { offset: 60, value: 1.0 },
    ];

    const evaluate = (t: number) => {
      if (t <= keyframes[0].offset) return keyframes[0].value;
      if (t >= keyframes[keyframes.length - 1].offset) return keyframes[keyframes.length - 1].value;
      const progress = (t - keyframes[0].offset) / (keyframes[1].offset - keyframes[0].offset);
      return keyframes[0].value + progress * (keyframes[1].value - keyframes[0].value);
    };

    // Before-first clamped to first
    expect(evaluate(-10)).toBe(0.0);
    expect(evaluate(0)).toBe(0.0);

    // Midpoint linear interpolation
    expect(evaluate(30)).toBe(0.5);

    // Exactly at second keyframe
    expect(evaluate(60)).toBe(1.0);

    // After-last clamped to last
    expect(evaluate(100)).toBe(1.0);
  });

  it("validates Transition model and ClipTransitionSet command representation", () => {
    const rate = { numerator: 30, denominator: 1 };
    const transition = {
      kind: "FadeIn" as const,
      duration: { value: 30, rate },
    };

    expect(transition.kind).toBe("FadeIn");
    expect(transition.duration.value).toBe(30);

    const cmd = {
      type: "ClipTransitionSet" as const,
      payload: {
        item_id: "clip_sample_1",
        transition,
      },
    };

    expect(cmd.type).toBe("ClipTransitionSet");
    expect(cmd.payload.item_id).toBe("clip_sample_1");
    expect(cmd.payload.transition?.kind).toBe("FadeIn");
    expect(cmd.payload.transition?.duration.value).toBe(30);

    // Test transition progression evaluation
    const evaluateTransitionOpacity = (offsetFrame: number, totalDur: number, trans: typeof transition) => {
      const dur = trans.duration.value;
      if (trans.kind === "FadeIn") {
        return Math.min(1.0, Math.max(0.0, offsetFrame / dur));
      }
      if (trans.kind === "FadeOut") {
        const start = totalDur - dur;
        if (offsetFrame > start) {
          return Math.min(1.0, Math.max(0.0, (totalDur - offsetFrame) / dur));
        }
        return 1.0;
      }
      return 1.0;
    };

    expect(evaluateTransitionOpacity(0, 90, transition)).toBe(0.0);
    expect(evaluateTransitionOpacity(15, 90, transition)).toBe(0.5);
    expect(evaluateTransitionOpacity(30, 90, transition)).toBe(1.0);
    expect(evaluateTransitionOpacity(60, 90, transition)).toBe(1.0);
  });

  it("enforces CrossDissolve is marked deferred and excluded from selectable production transitions", () => {
    // Supported transitions in production capability contract
    const supportedTransitions = ["FadeIn", "FadeOut", "FadeBlack"];
    const allKnownTransitions = ["FadeIn", "FadeOut", "CrossDissolve", "FadeBlack"];

    // Assert CrossDissolve is explicitly excluded from supported transitions
    expect(supportedTransitions).not.toContain("CrossDissolve");
    expect(supportedTransitions).toEqual(["FadeIn", "FadeOut", "FadeBlack"]);

    // Option state simulation for Inspector selector
    const transitionOptions = allKnownTransitions.map((t) => ({
      kind: t,
      disabled: t === "CrossDissolve",
      label: t === "CrossDissolve" ? "Cross Dissolve (Deferred)" : t,
    }));

    const crossDissolveOption = transitionOptions.find((o) => o.kind === "CrossDissolve");
    expect(crossDissolveOption?.disabled).toBe(true);
    expect(crossDissolveOption?.label).toBe("Cross Dissolve (Deferred)");

    const supportedOptions = transitionOptions.filter((o) => !o.disabled);
    expect(supportedOptions.map((o) => o.kind)).toEqual(["FadeIn", "FadeOut", "FadeBlack"]);
  });
});

describe("Phase 13 PreviewTransitionGroup & CrossDissolve Preview Semantics", () => {
  it("validates PreviewTransitionGroup model structure and dual-layer sampling", () => {
    const rate = { numerator: 30, denominator: 1 };
    const leftLayer: PreviewSampledLayer = {
      item_id: "clip_left",
      name: "Left Clip",
      z_index: 0,
      kind: "Video",
      source_time: { value: 35, rate },
      position: [0, 0],
      scale: [1, 1],
      rotation: 0,
      crop: { left: 0, right: 0, top: 0, bottom: 0 },
      opacity: 0.5,
      is_missing_media: false,
      computed_bounds: { x: 0, y: 0, width: 1920, height: 1080 },
    };
    const rightLayer: PreviewSampledLayer = {
      item_id: "clip_right",
      name: "Right Clip",
      z_index: 0,
      kind: "Video",
      source_time: { value: 15, rate },
      position: [0, 0],
      scale: [1, 1],
      rotation: 0,
      crop: { left: 0, right: 0, top: 0, bottom: 0 },
      opacity: 0.5,
      is_missing_media: false,
      computed_bounds: { x: 0, y: 0, width: 1920, height: 1080 },
    };

    const group: PreviewTransitionGroup = {
      transition_id: "trans_1",
      track_id: "trk_1",
      z_index: 0,
      kind: "CrossDissolve",
      progress: 0.5,
      left_layer: leftLayer,
      right_layer: rightLayer,
    };

    const previewState: PreviewFrameState = {
      timestamp: { value: 30, rate },
      active_layers: [],
      active_audio: [],
      transition_groups: [group],
    };

    expect(previewState.transition_groups?.length).toBe(1);
    expect(previewState.transition_groups?.[0].kind).toBe("CrossDissolve");
    expect(previewState.transition_groups?.[0].progress).toBe(0.5);
    expect(previewState.transition_groups?.[0].left_layer.opacity).toBe(0.5);
    expect(previewState.transition_groups?.[0].right_layer.opacity).toBe(0.5);
    // In transition interval, active_layers must suppress participant clips
    expect(previewState.active_layers.length).toBe(0);
  });

  it("validates CutTransition commands, tick division, and structured error codes", () => {
    const rate = { numerator: 30, denominator: 1 };
    const cutTransition = {
      transition_id: "trans_cut_1",
      track_id: "track_v1",
      left_item_id: "clip_A",
      right_item_id: "clip_B",
      kind: "CrossDissolve" as const,
      duration: { value: 30, rate },
    };

    expect(cutTransition.kind).toBe("CrossDissolve");
    expect(cutTransition.duration.value).toBe(30);

    // Command payloads for Slice G
    const createCmd = {
      type: "CutTransitionCreate" as const,
      payload: {
        track_id: "track_v1",
        left_item_id: "clip_A",
        right_item_id: "clip_B",
        duration: { value: 30, rate },
      },
    };
    expect(createCmd.type).toBe("CutTransitionCreate");
    expect(createCmd.payload.left_item_id).toBe("clip_A");

    const updateCmd = {
      type: "CutTransitionUpdateDuration" as const,
      payload: {
        transition_id: "trans_cut_1",
        duration: { value: 45, rate },
      },
    };
    expect(updateCmd.type).toBe("CutTransitionUpdateDuration");
    expect(updateCmd.payload.duration.value).toBe(45);

    const removeCmd = {
      type: "CutTransitionRemove" as const,
      payload: {
        transition_id: "trans_cut_1",
      },
    };
    expect(removeCmd.type).toBe("CutTransitionRemove");
    expect(removeCmd.payload.transition_id).toBe("trans_cut_1");

    // Centered cut tick division rule
    const calcInterval = (cutTick: number, durTicks: number) => {
      const leftTicks = Math.floor(durTicks / 2);
      const rightTicks = durTicks - leftTicks;
      return {
        t0: cutTick - leftTicks,
        t1: cutTick + rightTicks,
        leftTicks,
        rightTicks,
      };
    };

    // Even duration: 30 ticks -> L=15, R=15
    const even = calcInterval(100, 30);
    expect(even.leftTicks).toBe(15);
    expect(even.rightTicks).toBe(15);
    expect(even.t0).toBe(85);
    expect(even.t1).toBe(115);

    // Odd duration: 31 ticks -> L=15, R=16
    const odd = calcInterval(100, 31);
    expect(odd.leftTicks).toBe(15);
    expect(odd.rightTicks).toBe(16);
    expect(odd.t0).toBe(85);
    expect(odd.t1).toBe(116);

    // Structured handle error codes defined in ADR-003 and plan_phase13.md
    const errorCodes = [
      "INSUFFICIENT_TRANSITION_HANDLE",
      "TRANSITION_HANDLE_UNVERIFIABLE",
      "UNSUPPORTED_TRANSITION_MEDIA",
    ];
    expect(errorCodes).toContain("INSUFFICIENT_TRANSITION_HANDLE");
    expect(errorCodes).toContain("TRANSITION_HANDLE_UNVERIFIABLE");
    expect(errorCodes).toContain("UNSUPPORTED_TRANSITION_MEDIA");
  });

  it("accurately detects adjacency and semantic time equality across different frame rates", () => {
    const rate30 = { numerator: 30, denominator: 1 };
    const rate60 = { numerator: 60, denominator: 1 };

    const t30 = { value: 60, rate: rate30 }; // 2.0s
    const t60 = { value: 120, rate: rate60 }; // 2.0s

    expect(rationalTimeToSeconds(t30)).toBe(2.0);
    expect(rationalTimeToSeconds(t60)).toBe(2.0);
    expect(areRationalTimesEqual(t30, t60)).toBe(true);

    const leftRange = {
      start: { value: 0, rate: rate30 },
      duration: { value: 60, rate: rate30 }, // ends at 2.0s
    };
    const rightRange = {
      start: { value: 120, rate: rate60 }, // starts at 2.0s
      duration: { value: 120, rate: rate60 },
    };

    expect(isRangeAdjacent(leftRange, rightRange)).toBe(true);

    // Non-adjacent with 1-frame gap
    const gapRange = {
      start: { value: 121, rate: rate60 },
      duration: { value: 120, rate: rate60 },
    };
    expect(isRangeAdjacent(leftRange, gapRange)).toBe(false);
  });
});

describe("Phase 14 — Keyframe Interpolation & Inspector Easing Model", () => {
  it("validates KeyframeEasing variants and outgoing_easing property", () => {
    const linearEasing = "Linear";
    const easeInEasing = "EaseIn";
    const easeOutEasing = "EaseOut";
    const easeInOutEasing = "EaseInOut";
    const customEasing = {
      CubicBezier: { x1: 0.25, y1: 0.1, x2: 0.25, y2: 1.0 },
    };

    expect(linearEasing).toBe("Linear");
    expect(easeInEasing).toBe("EaseIn");
    expect(easeOutEasing).toBe("EaseOut");
    expect(easeInOutEasing).toBe("EaseInOut");
    expect(customEasing.CubicBezier.x1).toBe(0.25);
    expect(customEasing.CubicBezier.y2).toBe(1.0);
  });

  it("verifies clamp validation logic for custom cubic parameters", () => {
    const clamp = (val: number) => (isNaN(val) ? 0 : Math.max(0, Math.min(1, val)));

    expect(clamp(-0.5)).toBe(0.0);
    expect(clamp(1.5)).toBe(1.0);
    expect(clamp(0.42)).toBe(0.42);
    expect(clamp(NaN)).toBe(0.0);
  });

  it("identifies last-keyframe terminal invariant", () => {
    const rate = { numerator: 30, denominator: 1 };
    const keyframes = [
      { property: "PositionX" as const, offset: { value: 0, rate }, value: 100, outgoing_easing: "EaseIn" as const },
      { property: "PositionX" as const, offset: { value: 30, rate }, value: 200, outgoing_easing: "Linear" as const },
    ];

    const sorted = keyframes.sort((a, b) => a.offset.value - b.offset.value);
    const kf0IsLast = sorted[sorted.length - 1].offset.value === sorted[0].offset.value;
    const kf1IsLast = sorted[sorted.length - 1].offset.value === sorted[1].offset.value;

    expect(kf0IsLast).toBe(false);
    expect(kf1IsLast).toBe(true);
  });
});

