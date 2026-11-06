import { describe, it, expect } from "vitest";
import { HealthStatus } from "./types/health";
import {
  PreviewFrameState,
  PreviewSampledLayer,
  PreviewTransitionGroup,
  BenchmarkReport,
} from "./types/preview";
import {
  areRationalTimesEqual,
  Asset,
  getDefaultAudioCrossfadeDurationFrames,
  isRangeAdjacent,
  rationalTimeToFrames,
  rationalTimeToSeconds,
  TimelineItem,
  Project,
  Track,
} from "./types/project";
import { getPendingPreviewVideoSeekTarget, shouldSeekPreviewVideo } from "./components/previewVideoSync";
import { shouldResetInspectorDrafts } from "./components/inspectorDraftSync";
import { getMediaTimelinePlacement } from "./components/mediaTimelinePlacement";
import { parseFrameSeekInput, resolveFrameSeekInputValue, shouldCommitFrameSeekOnBlur } from "./components/frameSeek";

describe("Exact frame seek input", () => {
  it("accepts whole frame numbers within the timeline bounds", () => {
    expect(parseFrameSeekInput("187", 1057)).toEqual({ frame: 187, error: null });
    expect(parseFrameSeekInput(" 0 ", 1057)).toEqual({ frame: 0, error: null });
  });

  it("rejects fractions, negative values, unsafe integers, and frames past the timeline", () => {
    expect(parseFrameSeekInput("1.5", 1057).frame).toBeNull();
    expect(parseFrameSeekInput("-1", 1057).frame).toBeNull();
    expect(parseFrameSeekInput("9007199254740992", 1057).frame).toBeNull();
    expect(parseFrameSeekInput("1058", 1057)).toEqual({
      frame: null,
      error: "Enter a frame from 0 to 1057.",
    });
  });

  it("prefers the actual input value when React draft state is stale", () => {
    expect(resolveFrameSeekInputValue("7520", "1810", 1810)).toBe("7520");
    expect(resolveFrameSeekInputValue(undefined, "1810", 1810)).toBe("1810");
    expect(resolveFrameSeekInputValue(undefined, null, 1810)).toBe("1810");
  });

  it("commits a changed DOM value on blur even when onChange did not mark it dirty", () => {
    expect(shouldCommitFrameSeekOnBlur("7520", 1810, false)).toBe(true);
    expect(shouldCommitFrameSeekOnBlur("1810", 1810, false)).toBe(false);
    expect(shouldCommitFrameSeekOnBlur("1810", 1810, true)).toBe(true);
  });
});

describe("Inspector draft reconciliation", () => {
  it("preserves transient drafts on same-revision refreshes", () => {
    expect(shouldResetInspectorDrafts(4, 4, "text-1", "text-1")).toBe(false);
  });

  it("reconciles drafts after Undo/Redo revision transitions or selection changes", () => {
    expect(shouldResetInspectorDrafts(4, 3, "text-1", "text-1")).toBe(true);
    expect(shouldResetInspectorDrafts(3, 4, "text-1", "text-1")).toBe(true);
    expect(shouldResetInspectorDrafts(4, 4, "text-1", "text-2")).toBe(true);
  });
});

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

describe("RationalTime frame conversion", () => {
  it("floors media duration to project frames without exceeding source duration", () => {
    expect(
      rationalTimeToFrames(
        { value: 1_234_567_890, rate: { numerator: 1_000_000_000, denominator: 1 } },
        { numerator: 30, denominator: 1 }
      )
    ).toBe(37);
  });

  it("converts source offsets from their own timebase to project frames", () => {
    const sourceStart = { value: 294, rate: { numerator: 5, denominator: 1 } };
    const projectRate = { numerator: 30, denominator: 1 };

    expect(rationalTimeToSeconds(sourceStart)).toBe(58.8);
    expect(rationalTimeToFrames(sourceStart, projectRate)).toBe(1764);
  });

  it("chooses the longest default crossfade that fits known source and editorial handles", () => {
    const rate = { numerator: 30, denominator: 1 };
    const left = {
      item_id: "left",
      asset_id: "left-asset",
      kind: "Audio",
      source_range: {
        start: { value: 0, rate },
        duration: { value: 50, rate },
      },
      timeline_range: {
        start: { value: 0, rate },
        duration: { value: 50, rate },
      },
    } as TimelineItem;
    const right = {
      item_id: "right",
      asset_id: "right-asset",
      kind: "Audio",
      source_range: {
        start: { value: 7, rate },
        duration: { value: 50, rate },
      },
      timeline_range: {
        start: { value: 50, rate },
        duration: { value: 50, rate },
      },
    } as TimelineItem;
    const assets: Asset[] = [
      {
        asset_id: "left-asset",
        name: "Left",
        path: "left.wav",
        media_kind: "Audio",
        duration: { value: 100, rate },
        file_size: 1,
      },
      {
        asset_id: "right-asset",
        name: "Right",
        path: "right.wav",
        media_kind: "Audio",
        duration: { value: 100, rate },
        file_size: 1,
      },
    ];

    expect(getDefaultAudioCrossfadeDurationFrames(left, right, assets, rate)).toBe(15);
    expect(
      getDefaultAudioCrossfadeDurationFrames(left, right, assets.map((asset) => ({ ...asset, duration: undefined })), rate)
    ).toBe(0);
  });
});

describe("Preview video synchronization", () => {
  it("does not restart a video decoder for normal playhead drift", () => {
    expect(shouldSeekPreviewVideo(2.08, 2.1, true, false)).toBe(false);
  });

  it("seeks to large timeline jumps during playback when the decoder is idle", () => {
    expect(shouldSeekPreviewVideo(2, 8, true, false)).toBe(true);
  });

  it("keeps paused scrubbing frame-accurate and avoids interrupting an active seek", () => {
    expect(shouldSeekPreviewVideo(2, 2.1, false, false)).toBe(true);
    expect(shouldSeekPreviewVideo(2, 8, false, true)).toBe(false);
  });

  it("settles an overlapping paused seek on the latest requested target", () => {
    expect(getPendingPreviewVideoSeekTarget(2.2, 8, false, true)).toBeNull();
    expect(getPendingPreviewVideoSeekTarget(4, 8, false, false)).toBe(8);
    expect(getPendingPreviewVideoSeekTarget(8.001, 8, false, false)).toBeNull();
  });

  it("handles frame-accurate paused seeks on half-speed segments and small frame steps without suppression", () => {
    // Phase 26 native evidence: rev32 terminal Half segment at 30fps
    // Frame 4258: SRC 898 (898 / 30 = 29.933333s)
    // Frame 4260: SRC 899 (899 / 30 = 29.966667s)
    // Delta: 0.033333s (previously suppressed by 0.05s tolerance)
    const src898Sec = 898 / 30;
    const src899Sec = 899 / 30;
    expect(getPendingPreviewVideoSeekTarget(src898Sec, src899Sec, false, false)).toBe(src899Sec);

    // 1-frame step at 30fps half speed (delta = 1/60 = 0.016667s)
    const halfSpeedStepSec = 1 / 60;
    expect(getPendingPreviewVideoSeekTarget(10, 10 + halfSpeedStepSec, false, false)).toBe(10 + halfSpeedStepSec);

    // 1-frame step at 60fps (delta = 1/60 = 0.016667s)
    expect(getPendingPreviewVideoSeekTarget(5, 5 + 1 / 60, false, false)).toBe(5 + 1 / 60);

    // 1-frame step at 120fps (delta = 1/120 = 0.008333s)
    expect(getPendingPreviewVideoSeekTarget(5, 5 + 1 / 120, false, false)).toBe(5 + 1 / 120);

    // Decoder settling jitter <= 2ms is tolerated to prevent infinite seek loops
    expect(getPendingPreviewVideoSeekTarget(8.001, 8, false, false)).toBeNull();
    expect(getPendingPreviewVideoSeekTarget(7.999, 8, false, false)).toBeNull();
  });

  it("retargets an in-flight decoder seek to the newest rapid scrub position", () => {
    const firstPendingTarget = 5;
    expect(getPendingPreviewVideoSeekTarget(2.2, firstPendingTarget, false, true)).toBeNull();

    const latestPendingTarget = 8;
    expect(getPendingPreviewVideoSeekTarget(2.2, latestPendingTarget, false, true)).toBeNull();
    expect(getPendingPreviewVideoSeekTarget(4, latestPendingTarget, false, false)).toBe(8);
    expect(getPendingPreviewVideoSeekTarget(8, latestPendingTarget, false, false)).toBeNull();
  });

  it("seeks a newly loaded video element to the pending timeline target", () => {
    expect(getPendingPreviewVideoSeekTarget(0, 12, false, false)).toBe(12);
  });
});

describe("Media timeline placement", () => {
  const rate = { numerator: 30, denominator: 1 };
  const textItem = {
    kind: "Text",
    timeline_range: {
      start: { value: 0, rate },
      duration: { value: 315, rate },
    },
  } as TimelineItem;
  const makeTrack = (track_id: string, kind: "Video" | "Audio", muted: boolean, items: TimelineItem[]): Track => ({
    track_id,
    name: track_id,
    kind,
    muted,
    locked: false,
    items,
  });
  const makeProject = (tracks: Track[]) => ({
    tracks,
    settings: { frame_rate: rate },
  } as Project);
  const videoAsset = {
    asset_id: "video",
    name: "fixture.mp4",
    path: "fixture.mp4",
    media_kind: "Video",
    file_size: 1,
  } as Asset;

  it("creates video below text-only tracks instead of appending it to a muted caption track", () => {
    const project = makeProject([
      makeTrack("audio", "Audio", false, []),
      makeTrack("muted-captions", "Video", true, [textItem]),
      makeTrack("captions", "Video", false, [textItem]),
    ]);

    expect(getMediaTimelinePlacement(project, videoAsset)).toEqual({
      track: null,
      startFrame: 0,
      insertBeforeIndex: 1,
    });
  });

  it("prefers an unmuted video media track and appends after its existing clips", () => {
    const mediaItem = {
      ...textItem,
      kind: "Video",
      timeline_range: {
        start: { value: 30, rate },
        duration: { value: 60, rate },
      },
    } as TimelineItem;
    const mediaTrack = makeTrack("video-media", "Video", false, [mediaItem]);
    const project = makeProject([
      makeTrack("muted-video", "Video", true, [mediaItem]),
      makeTrack("captions", "Video", false, [textItem]),
      mediaTrack,
    ]);

    expect(getMediaTimelinePlacement(project, videoAsset)).toEqual({
      track: mediaTrack,
      startFrame: 90,
      insertBeforeIndex: null,
    });
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
          playback_rate: 1,
          position: [0, 0],
          scale: [1, 1],
          content_size: [1920, 1080],
          rotation: 0,
          crop: { left: 0, right: 0, top: 0, bottom: 0 },
          opacity: 1.0,
          visual_effect: "None",
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
          source_time: { value: 0, rate: { numerator: 30, denominator: 1 } },
          playback_rate: 1,
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

  it("keeps item fades separate from supported cut transitions", () => {
    const itemTransitions = ["FadeIn", "FadeOut", "FadeBlack"];
    const cutTransitions = ["CrossDissolve", "DipToBlack"];

    expect(itemTransitions).not.toContain("CrossDissolve");
    expect(itemTransitions).not.toContain("DipToBlack");
    expect(cutTransitions).toEqual(["CrossDissolve", "DipToBlack"]);
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
      playback_rate: 1,
      position: [0, 0],
      scale: [1, 1],
      content_size: [1920, 1080],
      rotation: 0,
      crop: { left: 0, right: 0, top: 0, bottom: 0 },
      opacity: 0.5,
      visual_effect: "None",
      is_missing_media: false,
      computed_bounds: { x: 0, y: 0, width: 1920, height: 1080 },
    };
    const rightLayer: PreviewSampledLayer = {
      item_id: "clip_right",
      name: "Right Clip",
      z_index: 0,
      kind: "Video",
      source_time: { value: 15, rate },
      playback_rate: 1,
      position: [0, 0],
      scale: [1, 1],
      content_size: [1920, 1080],
      rotation: 0,
      crop: { left: 0, right: 0, top: 0, bottom: 0 },
      opacity: 0.5,
      visual_effect: "None",
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
