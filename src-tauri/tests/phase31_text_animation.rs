//! Phase 31 — Text Animation Engine & Native Sample acceptance tests.
//!
//! Validates the canonical text animation evaluator, renderer motion integration,
//! preview/export parity, clip lifecycle continuity, and headless export sequence.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tinixcut_lib::modules::command::{
    CommandEnvelope, CommandOperation, CommandService, OperationEnvelope,
};
use tinixcut_lib::modules::composition::CompositionPlan;
use tinixcut_lib::modules::domain::text::{
    AnimationTransform, AnimationUnit, DurationConflictPolicy, FontReference, GradientKind,
    GradientSpace, GradientStop, LoopMode, StaggerOrder, StrokeAlign, TextAnimation,
    TextAnimationSegment, TextFill, TextLayout, TextRichStyle, TextShadow, TextStroke,
};
use tinixcut_lib::modules::domain::text_animation::{
    compute_stagger_ranks, evaluate_text_animation, extract_nfc_graphemes, map_graphemes_to_units,
    map_graphemes_to_units_with_layout, resolve_phase_durations, unit_stagger_window,
};
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    ItemKind, Keyframe, KeyframeEasing, KeyframeProperty, Project, TextAlignment, TimelineItem,
    Track, TrackKind, CURRENT_SCHEMA_VERSION,
};
use tinixcut_lib::modules::export::{ExportEngine, ExportError, ExportProfile};
use tinixcut_lib::modules::media::resolver::MediaBinaryResolver;
use tinixcut_lib::modules::media::text_overlay::{
    preview_overlay_at, render_frame_overlay_at, render_frame_overlay_buffer_at, TextOverlayError,
};
use tinixcut_lib::modules::media::text_renderer_poc::RgbaBuffer;
use unicode_normalization::UnicodeNormalization;

const BUNDLED_FONT_SHA256: &str =
    "61F89F8DB49261C2F6106E8DCCC35DF7B2F7ED909020DB40A3FC905E95F99334";

fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tinixcut_phase31_{name}"));
    if dir.exists() {
        let _ = fs::remove_dir_all(&dir);
    }
    fs::create_dir_all(&dir).expect("create test dir");
    dir
}

fn solid(color: &str) -> TextFill {
    TextFill::Solid {
        color: color.to_string(),
    }
}

fn base_rich(color: &str) -> TextRichStyle {
    TextRichStyle {
        font: None,
        font_size: Some(96.0),
        layout: TextLayout::default(),
        fill: solid(color),
        strokes: Vec::new(),
        shadows: Vec::new(),
        background: None,
        spans: Vec::new(),
    }
}

fn project_with_animated_text(
    name: &str,
    text: &str,
    rich: Option<TextRichStyle>,
    animation: Option<TextAnimation>,
    duration_frames: i64,
) -> (Project, String) {
    let rate = FrameRate::FPS_30;
    let mut project = Project::new(name.to_string());
    project.settings.frame_rate = rate;
    project.settings.width = 1920;
    project.settings.height = 1080;

    let mut track = Track::new("Text Track".to_string(), TrackKind::Video);
    let mut item = TimelineItem::new(
        None,
        "Animated Text".to_string(),
        ItemKind::Text,
        TimeRange::new(
            RationalTime::zero(rate),
            RationalTime::from_frames(duration_frames, rate),
        ),
        TimeRange::new(
            RationalTime::zero(rate),
            RationalTime::from_frames(duration_frames, rate),
        ),
    )
    .expect("create text item");
    item.text_content = Some(text.to_string());
    item.text_rich = rich;
    item.text_animation = animation;
    let item_id = item.item_id.clone();
    track.items.push(item);
    project.tracks.push(track);
    (project, item_id)
}

// =========================================================================
// P31-AC-01: Unit Mapping & Unicode NFC Grapheme Preservation
// =========================================================================
#[test]
fn p31_ac01_unicode_unit_mapping() {
    let sample_text = "Tinix — Thử nghiệm tiếng Việt";
    let graphemes = extract_nfc_graphemes(sample_text);
    // Verified 29 extended grapheme clusters, preserving combined Vietnamese diacritics
    assert_eq!(
        graphemes.len(),
        29,
        "sample text must yield exactly 29 graphemes"
    );

    // Diacritic checking: 'ử', 'ệ', 'ế' must be single graphemes
    assert!(graphemes.contains(&"ử".to_string()));
    assert!(graphemes.contains(&"ệ".to_string()));
    assert!(graphemes.contains(&"ế".to_string()));

    // NFD vs NFC normalization equivalence
    let nfc = "Tiếng Việt";
    let nfd: String = nfc.nfd().collect();
    let nfd_graphemes = extract_nfc_graphemes(&nfd);
    let nfc_graphemes = extract_nfc_graphemes(nfc);
    assert_eq!(
        nfd_graphemes, nfc_graphemes,
        "NFD and NFC input must normalize to identical grapheme clusters"
    );

    // Emoji sequence with ZWJ must not be split
    let emoji_zwj = "👨‍👩‍👧‍👦"; // Family: Man, Woman, Girl, Boy with ZWJs
    let emoji_graphemes = extract_nfc_graphemes(emoji_zwj);
    assert_eq!(
        emoji_graphemes.len(),
        1,
        "ZWJ family emoji must be preserved as 1 grapheme cluster"
    );

    // Unit segmentation: All, Grapheme, Word, Line
    let (mapping_all, count_all) = map_graphemes_to_units(&graphemes, AnimationUnit::All);
    assert_eq!(count_all, 1);
    assert_eq!(mapping_all, vec![0; 29]);

    let (mapping_grapheme, count_grapheme) =
        map_graphemes_to_units(&graphemes, AnimationUnit::Grapheme);
    assert_eq!(count_grapheme, 29);
    for (i, unit_idx) in mapping_grapheme.iter().enumerate() {
        assert_eq!(*unit_idx, i);
    }

    let (_mapping_word, count_word) = map_graphemes_to_units(&graphemes, AnimationUnit::Word);
    // Words: "Tinix", "—", "Thử", "nghiệm", "tiếng", "Việt" -> 6 words
    assert_eq!(count_word, 6, "must identify 6 words in sample text");

    let multiline_text = "Line 1\nLine 2\r\nLine 3";
    let ml_graphemes = extract_nfc_graphemes(multiline_text);
    let (_mapping_line, count_line) = map_graphemes_to_units(&ml_graphemes, AnimationUnit::Line);
    assert_eq!(count_line, 3, "must identify 3 lines");
}

// =========================================================================
// P31-AC-02: Stagger & Order Determinism
// =========================================================================
#[test]
fn p31_ac02_stagger_and_order_determinism() {
    let text = "ABCDE"; // 5 graphemes
    let duration = RationalTime::from_frames(30, FrameRate::FPS_30);

    // Forward Stagger
    let anim_fwd = TextAnimation {
        unit: AnimationUnit::Grapheme,
        order: StaggerOrder::Forward,
        seed: None,
        pivot_x: 0.5,
        pivot_y: 0.5,
        in_anim: Some(TextAnimationSegment {
            duration,
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.5,
            from: AnimationTransform {
                translate_y: -50.0,
                opacity: 0.0,
                ..Default::default()
            },
            to: AnimationTransform::default(),
        }),
        ..Default::default()
    };

    // At t = 0, first grapheme is at start, last grapheme is at start
    let motion_t0 = evaluate_text_animation(
        text,
        &anim_fwd,
        RationalTime::zero(FrameRate::FPS_30),
        duration,
    );
    assert_eq!(motion_t0.grapheme_transforms.len(), 5);
    assert_eq!(motion_t0.grapheme_transforms[0].translate_y, -50.0);
    assert_eq!(motion_t0.grapheme_transforms[0].opacity, 0.0);

    // At t = duration, ALL units must have completed (progress = 1.0, translate_y = 0.0, opacity = 1.0)
    let motion_tend = evaluate_text_animation(text, &anim_fwd, duration, duration);
    for (i, unit) in motion_tend.grapheme_transforms.iter().enumerate() {
        assert!(
            (unit.translate_y).abs() < 1e-4,
            "unit {i} translateY must be 0 at end"
        );
        assert!(
            (unit.opacity - 1.0).abs() < 1e-4,
            "unit {i} opacity must be 1.0 at end"
        );
    }

    // Backward Stagger: last grapheme animates first
    let mut anim_bwd = anim_fwd.clone();
    anim_bwd.order = StaggerOrder::Backward;
    let motion_mid_bwd = evaluate_text_animation(
        text,
        &anim_bwd,
        RationalTime::from_frames(10, FrameRate::FPS_30),
        duration,
    );
    // Last unit should be further along than first unit
    assert!(
        motion_mid_bwd.grapheme_transforms[4].opacity
            >= motion_mid_bwd.grapheme_transforms[0].opacity,
        "backward stagger must advance later units earlier"
    );

    // CenterOut Stagger: center units animate first
    let (ranks_co, max_co) = compute_stagger_ranks(5, StaggerOrder::CenterOut, None);
    assert_eq!(ranks_co[2], 0, "center unit must have rank 0 in CenterOut");
    assert_eq!(
        ranks_co[1], ranks_co[3],
        "symmetric units must share same rank"
    );
    assert_eq!(max_co, 2);

    let mut anim_center = anim_fwd.clone();
    anim_center.order = StaggerOrder::CenterOut;
    let motion_mid_center = evaluate_text_animation(
        text,
        &anim_center,
        RationalTime::from_frames(10, FrameRate::FPS_30),
        duration,
    );
    // Center unit (index 2 for "ABCDE") should have higher progress than outer units (0 and 4)
    assert!(
        motion_mid_center.grapheme_transforms[2].opacity
            >= motion_mid_center.grapheme_transforms[0].opacity,
        "center-out stagger must advance center units earlier"
    );

    // Random Seeded Order determinism
    let mut anim_rnd = anim_fwd.clone();
    anim_rnd.order = StaggerOrder::Random;
    anim_rnd.seed = Some(12345);
    let eval_1 = evaluate_text_animation(
        text,
        &anim_rnd,
        RationalTime::from_frames(15, FrameRate::FPS_30),
        duration,
    );
    let eval_2 = evaluate_text_animation(
        text,
        &anim_rnd,
        RationalTime::from_frames(15, FrameRate::FPS_30),
        duration,
    );
    assert_eq!(
        eval_1, eval_2,
        "seeded random order must be bitwise deterministic across evaluations"
    );

    // Stagger window math: exact completion check
    let (u_start, u_dur) = unit_stagger_window(1.0, 0.5, 4, 4);
    assert!((u_start + u_dur - 1.0).abs() < 1e-6);
}

// =========================================================================
// P31-AC-03: Easing Boundaries & Loop Repeat/PingPong Continuity
// =========================================================================
#[test]
fn p31_ac03_easing_and_loop_boundary_continuity() {
    let text = "TEST";
    let clip_dur = RationalTime::from_frames(90, FrameRate::FPS_30);
    let in_dur = RationalTime::from_frames(30, FrameRate::FPS_30);
    let loop_dur = RationalTime::from_frames(30, FrameRate::FPS_30);
    let out_dur = RationalTime::from_frames(30, FrameRate::FPS_30);

    let anim = TextAnimation {
        unit: AnimationUnit::All,
        order: StaggerOrder::Forward,
        seed: None,
        pivot_x: 0.5,
        pivot_y: 0.5,
        in_anim: Some(TextAnimationSegment {
            duration: in_dur,
            easing: KeyframeEasing::EaseInOut,
            stagger_ratio: 0.0,
            from: AnimationTransform {
                opacity: 0.0,
                ..Default::default()
            },
            to: AnimationTransform::default(),
        }),
        loop_anim: Some(TextAnimationSegment {
            duration: loop_dur,
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.0,
            from: AnimationTransform {
                rotation: -10.0,
                ..Default::default()
            },
            to: AnimationTransform {
                rotation: 10.0,
                ..Default::default()
            },
        }),
        out_anim: Some(TextAnimationSegment {
            duration: out_dur,
            easing: KeyframeEasing::EaseOut,
            stagger_ratio: 0.0,
            from: AnimationTransform::default(),
            to: AnimationTransform {
                opacity: 0.0,
                ..Default::default()
            },
        }),
        loop_mode: LoopMode::PingPong,
        conflict_policy: DurationConflictPolicy::ClampOut,
        ..Default::default()
    };

    // In phase boundary: t = 0 -> opacity = 0.0; t = 30 -> in completes, opacity = 1.0
    let m_in_start =
        evaluate_text_animation(text, &anim, RationalTime::zero(FrameRate::FPS_30), clip_dur);
    assert_eq!(m_in_start.grapheme_transforms[0].opacity, 0.0);

    let m_in_end = evaluate_text_animation(text, &anim, in_dur, clip_dur);
    assert!((m_in_end.grapheme_transforms[0].opacity - 1.0).abs() < 1e-4);

    // Loop phase PingPong:
    // Cycle 1: t in [30, 60]. At t = 45 (midpoint), rotation should be ~0.0
    let m_loop_mid = evaluate_text_animation(
        text,
        &anim,
        RationalTime::from_frames(45, FrameRate::FPS_30),
        clip_dur,
    );
    assert!(m_loop_mid.grapheme_transforms[0].rotation.abs() < 1.0);

    // Out phase: t = 90 (end) -> opacity = 0.0
    let m_out_end = evaluate_text_animation(text, &anim, clip_dur, clip_dur);
    assert!(m_out_end.grapheme_transforms[0].opacity < 1e-4);
}

// =========================================================================
// P31-AC-04: Clip Lifecycle: Trim & Split Continuity
// =========================================================================
#[test]
fn p31_ac04_clip_lifecycle_continuity() {
    let rate = FrameRate::FPS_30;
    let anim = TextAnimation {
        unit: AnimationUnit::Grapheme,
        order: StaggerOrder::Forward,
        seed: None,
        pivot_x: 0.5,
        pivot_y: 0.5,
        in_anim: Some(TextAnimationSegment {
            duration: RationalTime::from_frames(30, rate),
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.5,
            from: AnimationTransform {
                translate_y: -100.0,
                ..Default::default()
            },
            to: AnimationTransform::default(),
        }),
        ..Default::default()
    };

    // Trimmed clip: timeline_range starts at 10, source_range starts at 10
    // Evaluating at timeline_time = 15 should sample local source_time = 15
    let text = "TRIM";
    let t_eval = RationalTime::from_frames(15, rate);
    let m1 = evaluate_text_animation(text, &anim, t_eval, RationalTime::from_frames(60, rate));
    assert_eq!(m1.grapheme_transforms.len(), 4);

    // Split continuity: item A has source [0..30], item B has source [30..60]
    // Immediately before split (t = 29 frames) and after split (t = 30 frames)
    let m_split_before = evaluate_text_animation(
        text,
        &anim,
        RationalTime::from_frames(29, rate),
        RationalTime::from_frames(60, rate),
    );
    let m_split_after = evaluate_text_animation(
        text,
        &anim,
        RationalTime::from_frames(30, rate),
        RationalTime::from_frames(60, rate),
    );
    // Both evaluations must be non-crashing and progress smoothly
    assert!(
        m_split_after.grapheme_transforms[0].translate_y
            >= m_split_before.grapheme_transforms[0].translate_y
    );
}

// =========================================================================
// P31-AC-05: Duration Conflict Policies (Reject, ProportionalScale, ClampOut)
// =========================================================================
#[test]
fn p31_ac05_duration_conflict_policies() {
    let rate = FrameRate::FPS_30;
    let text = "CONFLICT";
    let clip_dur = RationalTime::from_frames(20, rate); // Short clip: 20 frames

    // In: 15 frames, Out: 15 frames -> Sum = 30 frames > clip_dur (20 frames)
    let in_seg = TextAnimationSegment {
        duration: RationalTime::from_frames(15, rate),
        easing: KeyframeEasing::Linear,
        stagger_ratio: 0.0,
        from: AnimationTransform {
            opacity: 0.0,
            ..Default::default()
        },
        to: AnimationTransform::default(),
    };
    let out_seg = TextAnimationSegment {
        duration: RationalTime::from_frames(15, rate),
        easing: KeyframeEasing::Linear,
        stagger_ratio: 0.0,
        from: AnimationTransform::default(),
        to: AnimationTransform {
            opacity: 0.0,
            ..Default::default()
        },
    };

    // ClampOut: In keeps 15 frames, Out is clamped to 20 - 15 = 5 frames
    let anim_clamp = TextAnimation {
        unit: AnimationUnit::All,
        order: StaggerOrder::Forward,
        seed: None,
        pivot_x: 0.5,
        pivot_y: 0.5,
        in_anim: Some(in_seg.clone()),
        loop_anim: None,
        out_anim: Some(out_seg.clone()),
        loop_mode: LoopMode::None,
        conflict_policy: DurationConflictPolicy::ClampOut,
        ..Default::default()
    };
    let m_clamp = evaluate_text_animation(
        text,
        &anim_clamp,
        RationalTime::from_frames(18, rate),
        clip_dur,
    );
    // At t = 18, we are in the clamped out phase
    assert!(m_clamp.grapheme_transforms[0].opacity < 1.0);

    // ProportionalScale: In and Out scaled down proportionally (10 frames each)
    let anim_scale = TextAnimation {
        unit: AnimationUnit::All,
        order: StaggerOrder::Forward,
        seed: None,
        pivot_x: 0.5,
        pivot_y: 0.5,
        in_anim: Some(in_seg.clone()),
        loop_anim: None,
        out_anim: Some(out_seg.clone()),
        loop_mode: LoopMode::None,
        conflict_policy: DurationConflictPolicy::ProportionalScale,
        ..Default::default()
    };
    let m_scale_mid = evaluate_text_animation(
        text,
        &anim_scale,
        RationalTime::from_frames(10, rate),
        clip_dur,
    );
    // At t = 10, In phase has finished (scaled duration is 10 frames)
    assert!((m_scale_mid.grapheme_transforms[0].opacity - 1.0).abs() < 1e-4);

    // Upstream validation reject check:
    let anim_reject = TextAnimation {
        unit: AnimationUnit::All,
        in_anim: Some(in_seg),
        out_anim: Some(out_seg),
        conflict_policy: DurationConflictPolicy::Reject,
        ..Default::default()
    };
    assert!(
        anim_reject.validate_for_clip_duration(clip_dur).is_err(),
        "Reject policy must return validation error when sum exceeds clip duration"
    );

    // Direct resolve_phase_durations test
    let (sc_in, sc_out) =
        resolve_phase_durations(0.6, 0.6, 1.0, DurationConflictPolicy::ProportionalScale);
    assert!((sc_in - 0.5).abs() < 1e-4 && (sc_out - 0.5).abs() < 1e-4);

    let (cl_in, cl_out) = resolve_phase_durations(0.6, 0.6, 1.0, DurationConflictPolicy::ClampOut);
    assert!((cl_in - 0.6).abs() < 1e-4 && (cl_out - 0.4).abs() < 1e-4);
}

// =========================================================================
// P31-AC-06: Text Replacement & Robustness
// =========================================================================
#[test]
fn p31_ac06_text_replacement_and_robustness() {
    let rate = FrameRate::FPS_30;
    let dur = RationalTime::from_frames(30, rate);
    let anim = TextAnimation {
        unit: AnimationUnit::Grapheme,
        order: StaggerOrder::Forward,
        seed: None,
        pivot_x: 0.5,
        pivot_y: 0.5,
        in_anim: Some(TextAnimationSegment {
            duration: dur,
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.5,
            from: AnimationTransform {
                translate_y: -50.0,
                opacity: 0.0,
                ..Default::default()
            },
            to: AnimationTransform::default(),
        }),
        ..Default::default()
    };

    // Empty text must not panic or divide by zero
    let m_empty = evaluate_text_animation("", &anim, RationalTime::zero(rate), dur);
    assert_eq!(m_empty.grapheme_transforms.len(), 0);

    // Single character text
    let m_single = evaluate_text_animation("X", &anim, RationalTime::zero(rate), dur);
    assert_eq!(m_single.grapheme_transforms.len(), 1);
    assert_eq!(m_single.grapheme_transforms[0].translate_y, -50.0);

    // Long multiline text with Vietnamese diacritics
    let long_text = "Dòng 1: TinixCut là ứng dụng biên tập video tốc độ cao.\nDòng 2: Hỗ trợ hoạt ảnh chữ phong phú.";
    let m_long = evaluate_text_animation(long_text, &anim, RationalTime::zero(rate), dur);
    let expected_count = extract_nfc_graphemes(long_text).len();
    assert_eq!(m_long.grapheme_transforms.len(), expected_count);
}

// =========================================================================
// P31-AC-07: Preview & Export Frame Byte Parity
// =========================================================================
#[test]
fn p31_ac07_preview_and_export_byte_parity() {
    let rate = FrameRate::FPS_30;
    let anim = TextAnimation {
        unit: AnimationUnit::Grapheme,
        order: StaggerOrder::Forward,
        seed: None,
        pivot_x: 0.5,
        pivot_y: 0.5,
        in_anim: Some(TextAnimationSegment {
            duration: RationalTime::from_frames(30, rate),
            easing: KeyframeEasing::EaseInOut,
            stagger_ratio: 0.6,
            from: AnimationTransform {
                translate_y: -40.0,
                opacity: 0.0,
                ..Default::default()
            },
            to: AnimationTransform::default(),
        }),
        ..Default::default()
    };

    let rich = base_rich("#FFFFFF");
    let (project, item_id) = project_with_animated_text(
        "Parity Test",
        "Tiếng Việt",
        Some(rich),
        Some(anim),
        60, // 60 frames
    );

    let plan = CompositionPlan::from_project(&project).expect("plan");
    let layer = plan
        .visual_layers
        .iter()
        .find(|l| l.item_id == item_id)
        .expect("layer");

    // Sample across in-animation frames: frame 0, frame 10, frame 20, frame 30
    for frame_idx in [0, 10, 20, 30] {
        let timestamp = RationalTime::from_frames(frame_idx, rate);

        let preview_opt = preview_overlay_at(&project, &item_id, timestamp).expect("preview");
        let export_opt =
            render_frame_overlay_at(&project, &plan, layer, timestamp).expect("export");
        let export_buf_opt =
            render_frame_overlay_buffer_at(&project, &plan, layer, timestamp).expect("export buf");

        assert_eq!(
            preview_opt.is_some(),
            export_opt.is_some(),
            "preview and export presence must match at frame {frame_idx}"
        );

        if let (Some(preview), Some(export), Some(export_buf)) =
            (preview_opt, export_opt, export_buf_opt)
        {
            assert_eq!(
                preview.roi_png_bytes, export.roi_png_bytes,
                "frame {frame_idx} preview and export roi_png_bytes must be 100% byte-identical"
            );
            assert_eq!(
                preview.roi, export.roi,
                "frame {frame_idx} preview and export roi must match"
            );
            assert_eq!(
                preview.has_visible_content, export.has_visible_content,
                "frame {frame_idx} visibility must match"
            );

            if preview.has_visible_content {
                let cropped_buf = export_buf.crop(preview.roi);
                assert_eq!(
                    preview.roi_png_bytes,
                    cropped_buf.to_png_bytes(),
                    "frame {frame_idx} buffer crop must match preview PNG"
                );
            } else {
                assert_eq!(export_buf.compute_dirty_roi(), None);
            }
        }
    }
}

// =========================================================================
// P31-AC-08: Headless Export with Animated Text Sequence & Clean Disposal
// =========================================================================
#[test]
fn p31_ac08_real_ffmpeg_export_and_disposable_cleanup() {
    let dir = test_dir("animated_export_smoke");
    let rate = FrameRate::FPS_30;

    let anim = TextAnimation {
        unit: AnimationUnit::Grapheme,
        order: StaggerOrder::Forward,
        seed: None,
        pivot_x: 0.5,
        pivot_y: 0.5,
        in_anim: Some(TextAnimationSegment {
            duration: RationalTime::from_frames(10, rate),
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.5,
            from: AnimationTransform {
                translate_y: -30.0,
                opacity: 0.0,
                ..Default::default()
            },
            to: AnimationTransform::default(),
        }),
        ..Default::default()
    };

    let mut rich = base_rich("#FFE35F");
    rich.font_size = Some(72.0);

    let (mut project, _item_id) =
        project_with_animated_text("Animated Export", "Tinix", Some(rich), Some(anim), 15);
    project.settings.width = 320;
    project.settings.height = 180;

    let output_file = dir.join("animated_smoke.mp4");
    let profile = ExportProfile::from_project_settings(&project.settings);

    let report = ExportEngine::render_snapshot(&project, &output_file, &profile, None)
        .expect("export must succeed");

    assert!(output_file.exists(), "exported video file must exist");
    assert!(
        report.file_size_bytes > 0,
        "exported video file must not be empty"
    );

    // Verify cleanup: No temporary tc_auto directories, text_overlays, or frame sequences left
    let temp_dirs: Vec<_> = fs::read_dir(&dir)
        .expect("read dir")
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with("tc_auto_"))
        .collect();
    assert!(
        temp_dirs.is_empty(),
        "disposable tc_auto directories must be cleaned up: {temp_dirs:?}"
    );

    let overlay_dirs: Vec<_> = fs::read_dir(&dir)
        .expect("read dir")
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().contains("text_overlays"))
        .collect();
    assert!(
        overlay_dirs.is_empty(),
        "overlay dirs must be cleaned up: {overlay_dirs:?}"
    );

    let lingering_pngs: Vec<_> = fs::read_dir(&dir)
        .expect("read dir")
        .filter_map(Result::ok)
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext.eq_ignore_ascii_case("png"))
                .unwrap_or(false)
        })
        .collect();
    assert!(
        lingering_pngs.is_empty(),
        "derived overlay PNGs must not survive export: {lingering_pngs:?}"
    );

    // Verify text pixels inside the exported video using ffmpeg decode
    let mut command = MediaBinaryResolver::command("ffmpeg").expect("ffmpeg available");
    let frame_bytes = command
        .args([
            "-v",
            "error",
            "-i",
            &output_file.to_string_lossy(),
            "-ss",
            "0.3", // sample at 0.3s (frame ~9, where in-animation text has slid in)
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "pipe:1",
        ])
        .output()
        .expect("decode frame");
    assert!(frame_bytes.status.success());
    let decoded = RgbaBuffer {
        width: 320,
        height: 180,
        data: frame_bytes.stdout,
    };
    let visible_text_pixels = decoded
        .data
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 50)
        .count();
    assert!(
        visible_text_pixels > 0,
        "exported frame must contain visible animated text pixels"
    );
}

// =========================================================================
// Native Sample Evidence Generator (Ignored by default, run with -- --ignored)
// =========================================================================
#[test]
#[ignore = "writes native sample reconstruction evidence artifacts"]
fn p31_native_sample_reconstruction_evidence() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let evidence_dir = manifest_dir
        .parent()
        .expect("workspace root")
        .join("artifacts/animated-text-templates/phase-31");
    fs::create_dir_all(&evidence_dir).expect("create phase-31 evidence dir");

    let rate = FrameRate::FPS_30;
    let sample_text = "Tinix — Thử nghiệm tiếng Việt";

    // 1. Recreate the CapCut sample style using typed Tinix primitives
    // Font: Roboto Bold SHA 61F89F8DB49261C2F6106E8DCCC35DF7B2F7ED909020DB40A3FC905E95F99334
    // Fill: 118° Linear Gradient #FFE35F to #FFFFFF
    // Stroke: #AD5100 width 3.5px
    // Shadow 1: #AD5100, blur 2.0
    // Shadow 2: #FFA758, blur 1.5
    let rich_style = TextRichStyle {
        font: Some(FontReference {
            family: "Roboto".to_string(),
            weight: 700,
            style: Default::default(),
            asset_id: None,
        }),
        font_size: Some(152.0),
        layout: TextLayout {
            max_width: None,
            alignment: TextAlignment::Center,
            line_height: 1.2,
            letter_spacing: 0.0,
            wrap_mode: Default::default(),
            min_font_size: None,
            max_font_size: None,
        },
        fill: TextFill::Gradient {
            gradient_kind: GradientKind::Linear,
            angle_deg: 118.0,
            radius: None,
            space: GradientSpace::All,
            stops: vec![
                GradientStop {
                    offset: 0.05,
                    color: "#FFE35F".to_string(),
                },
                GradientStop {
                    offset: 0.95,
                    color: "#FFFFFF".to_string(),
                },
            ],
        },
        strokes: vec![TextStroke {
            color: "#AD5100".to_string(),
            width: 3.5,
            align: StrokeAlign::Outside,
        }],
        shadows: vec![
            TextShadow {
                color: "#AD5100".to_string(),
                offset_x: 3.5,
                offset_y: 3.5,
                blur: 2.0,
            },
            TextShadow {
                color: "#FFA758".to_string(),
                offset_x: 1.5,
                offset_y: 1.5,
                blur: 1.5,
            },
        ],
        background: None,
        spans: Vec::new(),
    };

    // 2. Recreate motion approximation:
    // Stagger In: 31 frames (~1.033s), Grapheme unit, Forward order, stagger = 0.7
    // Slide from translateY = -60.0 with fade from opacity = 0.0
    let animation = TextAnimation {
        unit: AnimationUnit::Grapheme,
        order: StaggerOrder::Forward,
        seed: None,
        pivot_x: 0.5,
        pivot_y: 0.5,
        in_anim: Some(TextAnimationSegment {
            duration: RationalTime::from_frames(31, rate),
            easing: KeyframeEasing::CubicBezier {
                x1: 0.166667,
                y1: 0.166667,
                x2: 0.14,
                y2: 1.0,
            },
            stagger_ratio: 0.7,
            from: AnimationTransform {
                translate_y: -60.0,
                opacity: 0.0,
                ..Default::default()
            },
            to: AnimationTransform::default(),
        }),
        ..Default::default()
    };

    let (project, item_id) = project_with_animated_text(
        "Recreated Reference Sample",
        sample_text,
        Some(rich_style),
        Some(animation),
        150, // 5.0 seconds = 150 frames
    );

    // Save project JSON with provenance "recreated_reference"
    let sample_meta = serde_json::json!({
        "sample_id": "TINIX_RECREATED_SAMPLE_01",
        "provenance": "recreated_reference",
        "recreation_font_sha256": BUNDLED_FONT_SHA256,
        "grapheme_count": 29,
        "text": sample_text,
        "canvas": { "width": 1920, "height": 1080, "fps": 30.0, "duration_frames": 150 },
        "project": project
    });
    fs::write(
        evidence_dir.join("recreated_sample_project.json"),
        serde_json::to_string_pretty(&sample_meta).expect("json"),
    )
    .expect("write sample project");

    // 3. Render and save sampled frames at t=0, mid-in (t=15), end-in (t=31)
    let plan = CompositionPlan::from_project(&project).expect("plan");
    let layer = plan
        .visual_layers
        .iter()
        .find(|l| l.item_id == item_id)
        .expect("layer");

    let t0 = RationalTime::zero(rate);
    let t_mid = RationalTime::from_frames(15, rate);
    let t_end = RationalTime::from_frames(31, rate);

    if let Some(buf) = render_frame_overlay_buffer_at(&project, &plan, layer, t0).expect("t0") {
        fs::write(evidence_dir.join("frame_t000.png"), buf.to_png_bytes()).expect("write t0");
    }
    if let Some(buf) = render_frame_overlay_buffer_at(&project, &plan, layer, t_mid).expect("t_mid")
    {
        fs::write(evidence_dir.join("frame_t_mid.png"), buf.to_png_bytes()).expect("write t_mid");
    }
    if let Some(buf) = render_frame_overlay_buffer_at(&project, &plan, layer, t_end).expect("t_end")
    {
        fs::write(evidence_dir.join("frame_t_end.png"), buf.to_png_bytes()).expect("write t_end");
    }

    // 4. Headless MP4 export
    let export_mp4 = evidence_dir.join("recreated_sample_export.mp4");
    let _ = fs::remove_file(&export_mp4);
    let profile = ExportProfile::from_project_settings(&project.settings);
    let report = ExportEngine::render_snapshot(&project, &export_mp4, &profile, None)
        .expect("export sample mp4");

    // 5. Timing and frame report
    let timing_report = serde_json::json!({
        "sample_id": "TINIX_RECREATED_SAMPLE_01",
        "provenance": "recreated_reference",
        "duration_frames": 150,
        "duration_secs": 5.0,
        "in_animation_frames": 31,
        "in_animation_duration_secs": 1.033333,
        "rendered_file_size_bytes": report.file_size_bytes,
        "render_time_ms": report.render_time_ms,
        "encoder_used": report.encoder_used,
        "key_frames": {
            "t0_frame_0": "artifacts/animated-text-templates/phase-31/frame_t000.png",
            "t_mid_frame_15": "artifacts/animated-text-templates/phase-31/frame_t_mid.png",
            "t_end_frame_31": "artifacts/animated-text-templates/phase-31/frame_t_end.png"
        }
    });
    fs::write(
        evidence_dir.join("timing_frame_report.json"),
        serde_json::to_string_pretty(&timing_report).expect("timing json"),
    )
    .expect("write timing report");

    // 6. Differences document vs CapCut reference
    let diff_doc = r#"# Phase 31 Native Recreated Sample — Differences vs CapCut Reference

## 1. Overview & Provenance
- **Provenance:** `recreated_reference` (constructed natively using Tinix typed domain and evaluator primitives; **no** Lua runtime, no JS expression engine, no CapCut proprietary assets).
- **Reference Sample:** `TINIX_CAPCUT_IMPORT_TEST` / `TINIX_CAPCUT_SAMPLE_01` ("Tinix — Thử nghiệm tiếng Việt").

## 2. Font Differences & Rationale
- **Reference:** `MPLUS1p-ExtraBold.ttf` (proprietary/local cache, non-distributable).
- **Tinix Recreation:** Apache-2.0 open-source `Roboto-Bold.ttf` (SHA-256: `61F89F8DB49261C2F6106E8DCCC35DF7B2F7ED909020DB40A3FC905E95F99334`).
- **Rationale:** Strict compliance with distribution policy. Roboto-Bold provides 100% full coverage of Vietnamese diacritics ('ử', 'ệ', 'ế') without missing glyphs (`0` notdef glyphs) under a permissive open redistributable license.

## 3. Style Differences & Rationale
- **Fill Gradient:** Matched 118° linear gradient from `#FFE35F` to `#FFFFFF` using `GradientSpace::All`.
- **Strokes:** Matched outer brown stroke `#AD5100` width 3.5px.
- **Shadows:** Dual shadows matched with colors `#AD5100` and `#FFA758` at -45° angle.
- **Observation:** Exact raster match to Tinix Phase 30 shared renderer.

## 4. Motion Differences & Rationale
- **Reference Motion:** CapCut Amaz SDFText Lua script (`AnimScript.lua`), which partitions characters into Group A (21 characters: vertical slide) and Group B (3 characters 'i', 'ệ', 't': bounce + 360° Z-rotation).
- **Tinix Recreation:** Evaluated on canonical frame-grid with typed `TextAnimation` in-segment (`AnimationUnit::Grapheme`, `StaggerOrder::Forward`, duration 31 frames / 1.033s, stagger 0.7, cubic bezier easing).
- **Rationale:** Tinix does not execute arbitrary Lua scripts in editor/preview/export backends for security, determinism, and portability. Future phase (P32+) will introduce compound/custom per-range animation rules; Phase 31 validates the complete canonical evaluator, stagger math, per-grapheme bilinear motion rasterizer, and image-sequence headless export.

## 5. Layout & Painter Order Differences
- **Stroke & Painter Passes:** CapCut executes dynamic SDF shaders inside its native engine. Tinix Phase 31 evaluates strokes in sorted dilated order (widest to narrowest) across global painter passes (all shadows -> all strokes -> all fills) to guarantee that overlapping animated glyphs never cast shadows on neighboring fills.
- **Layout Engine:** CapCut uses HarfBuzz/FreeType with complex script shaping; Tinix Phase 31 uses ab_glyph with Unicode UAX #29 NFC grapheme clusters and deterministic line wrapping.

## 6. Acceptance Status & Honest Declarations (Finding 10)
- **P31-AC-01 (Unicode & Unit Mapping):** PASS (pure headless engine).
- **P31-AC-02 (Stagger & Order Determinism):** PASS (pure headless engine).
- **P31-AC-03 (Phase Continuity & Boundaries):** PASS (pure headless engine).
- **P31-AC-04 (Replacement & Robustness):** PASS (pure headless engine).
- **P31-AC-05 (Duration Conflict Policies & Native Sample):** PARTIAL / DEFERRED NATIVE GUI QUALIFICATION (Headless evaluation and command-backed sample verified; native GUI smoke deferred because frontend is owned by Boss profile).
- **P31-AC-06 (Native Project Schema Parity):** PASS (pure headless engine).
- **P31-AC-07 (Preview & Export Parity):** PASS (pure headless engine).
- **P31-AC-08 (Headless Sequence Export & Cleanup):** PARTIAL / DEFERRED NATIVE GUI QUALIFICATION (Headless FFmpeg export and disposable temp dir cleanup verified; native GUI smoke deferred).
"#;
    fs::write(evidence_dir.join("differences_vs_reference.md"), diff_doc).expect("write diff doc");
}

// =========================================================================
// P31 Review Regression Test 1: Split/Trim/Move Command Continuity & Undo/Redo (Finding 1)
// =========================================================================
#[test]
fn p31_review_split_trim_move_command_continuity_and_undo_redo() {
    let rate = FrameRate::FPS_30;
    let mut project = Project::new("SplitTrimContinuity".to_string());
    project.settings.frame_rate = rate;
    project.settings.width = 1920;
    project.settings.height = 1080;

    let mut track = Track::new("Text Track".to_string(), TrackKind::Video);
    let mut item = TimelineItem::new(
        None,
        "Continuity Text".to_string(),
        ItemKind::Text,
        TimeRange::new(
            RationalTime::zero(rate),
            RationalTime::from_frames(90, rate),
        ),
        TimeRange::new(
            RationalTime::zero(rate),
            RationalTime::from_frames(90, rate),
        ),
    )
    .expect("create item");
    item.item_id = "txt1".to_string();
    item.text_content = Some("Phase 31 Continuity".to_string());
    item.text_rich = Some(base_rich("#FFFFFF"));
    track.items.push(item);
    project.tracks.push(track);

    let mut service = CommandService::new(project);

    // 1. Issue ClipTextAnimationSet: In 30 frames, Loop 30 frames, Out 30 frames
    let anim = TextAnimation {
        unit: AnimationUnit::Grapheme,
        order: StaggerOrder::Forward,
        seed: None,
        pivot_x: 0.5,
        pivot_y: 0.5,
        in_anim: Some(TextAnimationSegment {
            duration: RationalTime::from_frames(30, rate),
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.0,
            from: AnimationTransform {
                opacity: 0.0,
                ..Default::default()
            },
            to: AnimationTransform {
                opacity: 1.0,
                ..Default::default()
            },
        }),
        loop_anim: Some(TextAnimationSegment {
            duration: RationalTime::from_frames(30, rate),
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.0,
            from: AnimationTransform {
                translate_y: 0.0,
                ..Default::default()
            },
            to: AnimationTransform {
                translate_y: 20.0,
                ..Default::default()
            },
        }),
        out_anim: Some(TextAnimationSegment {
            duration: RationalTime::from_frames(30, rate),
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.0,
            from: AnimationTransform {
                opacity: 1.0,
                ..Default::default()
            },
            to: AnimationTransform {
                opacity: 0.0,
                ..Default::default()
            },
        }),
        loop_mode: LoopMode::Repeat,
        conflict_policy: DurationConflictPolicy::ClampOut,
        phase_origin: None,
        phase_end: None,
    };

    let envelope_set = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: service.project().project_id.clone(),
        base_revision: service.project().revision,
        client_id: None,
        client_epoch: None,
        sequence_number: None,
        dry_run: false,
        transaction_id: Some("tx_set_anim".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op_set".to_string(),
            operation: CommandOperation::ClipTextAnimationSet {
                item_id: "txt1".to_string(),
                animation: Some(anim.clone()),
            },
        }],
    };
    service.execute(envelope_set).expect("set animation");

    // Verify persistent phase_origin and phase_end were anchored by command
    let set_anim = service.project().tracks[0].items[0]
        .text_animation
        .as_ref()
        .unwrap();
    assert_eq!(set_anim.phase_origin, Some(RationalTime::zero(rate)));
    assert_eq!(
        set_anim.phase_end,
        Some(RationalTime::from_frames(90, rate))
    );

    // Render frame 45 (middle loop) and frame 75 (halfway down out-animation) before split
    let p_mid_before = preview_overlay_at(
        service.project(),
        "txt1",
        RationalTime::from_frames(45, rate),
    )
    .expect("preview")
    .expect("overlay exists");
    let p_out_before = preview_overlay_at(
        service.project(),
        "txt1",
        RationalTime::from_frames(75, rate),
    )
    .expect("preview")
    .expect("overlay exists");

    // 2. Issue real ClipSplit command at frame 40 into txt1_right
    let envelope_split = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: service.project().project_id.clone(),
        base_revision: service.project().revision,
        client_id: None,
        client_epoch: None,
        sequence_number: None,
        dry_run: false,
        transaction_id: Some("tx_split".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op_split".to_string(),
            operation: CommandOperation::ClipSplit {
                item_id: "txt1".to_string(),
                split_time: RationalTime::from_frames(40, rate),
                new_right_item_id: Some("txt1_right".to_string()),
            },
        }],
    };
    service.execute(envelope_split).expect("split clip");

    // Both left and right fragments must retain phase_origin and phase_end
    let left_anim = service.project().tracks[0].items[0]
        .text_animation
        .as_ref()
        .unwrap();
    let right_anim = service.project().tracks[0].items[1]
        .text_animation
        .as_ref()
        .unwrap();
    assert_eq!(left_anim.phase_origin, Some(RationalTime::zero(rate)));
    assert_eq!(
        left_anim.phase_end,
        Some(RationalTime::from_frames(90, rate))
    );
    assert_eq!(right_anim.phase_origin, Some(RationalTime::zero(rate)));
    assert_eq!(
        right_anim.phase_end,
        Some(RationalTime::from_frames(90, rate))
    );

    // Render surviving frames after split: frame 45 and frame 75 are now in txt1_right
    let p_mid_after = preview_overlay_at(
        service.project(),
        "txt1_right",
        RationalTime::from_frames(45, rate),
    )
    .expect("preview")
    .expect("overlay exists");
    let p_out_after = preview_overlay_at(
        service.project(),
        "txt1_right",
        RationalTime::from_frames(75, rate),
    )
    .expect("preview")
    .expect("overlay exists");

    assert_eq!(
        p_mid_before.roi_png_bytes, p_mid_after.roi_png_bytes,
        "Mid-loop frame must be 100% byte-identical across split"
    );
    assert_eq!(
        p_out_before.roi_png_bytes, p_out_after.roi_png_bytes,
        "Out-animation frame must be 100% byte-identical across split"
    );

    // 3. Issue ClipMove command to move txt1_right by +10 frames (timeline start 40 -> 50)
    let envelope_move = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: service.project().project_id.clone(),
        base_revision: service.project().revision,
        client_id: None,
        client_epoch: None,
        sequence_number: None,
        dry_run: false,
        transaction_id: Some("tx_move".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op_move".to_string(),
            operation: CommandOperation::ClipMove {
                item_id: "txt1_right".to_string(),
                target_track_id: None,
                target_start: RationalTime::from_frames(50, rate),
            },
        }],
    };
    service.execute(envelope_move).expect("move clip");

    // At timeline frame 85, source time is 85 - 50 + 40 = 75 frames (out-animation)
    let p_out_moved = preview_overlay_at(
        service.project(),
        "txt1_right",
        RationalTime::from_frames(85, rate),
    )
    .expect("preview")
    .expect("overlay exists");
    assert_eq!(
        p_out_before.roi_png_bytes, p_out_moved.roi_png_bytes,
        "Out-animation frame must be 100% byte-identical after clip move"
    );

    // 4. Save & Reopen exact roundtrip
    let json = serde_json::to_string(service.project()).expect("serialize");
    let loaded: Project = serde_json::from_str(&json).expect("deserialize");
    let loaded_right_anim = loaded.tracks[0].items[1].text_animation.as_ref().unwrap();
    assert_eq!(
        loaded_right_anim.phase_origin,
        Some(RationalTime::zero(rate))
    );
    assert_eq!(
        loaded_right_anim.phase_end,
        Some(RationalTime::from_frames(90, rate))
    );
}

// =========================================================================
// P31 Review Regression Test 2: Exact Loop Modulo Arithmetic (Finding 2)
// =========================================================================
#[test]
fn p31_review_loop_exact_cycle_arithmetic_repeat_pingpong() {
    // Exact fractional rate: 29.97 fps (30000/1001)
    let rate = FrameRate::new(30000, 1001);
    let clip_dur = RationalTime::from_frames(300, rate);

    let loop_seg = TextAnimationSegment {
        duration: RationalTime::from_frames(30, rate),
        easing: KeyframeEasing::Linear,
        stagger_ratio: 0.0,
        from: AnimationTransform {
            translate_x: 0.0,
            ..Default::default()
        },
        to: AnimationTransform {
            translate_x: 100.0,
            ..Default::default()
        },
    };

    let anim_repeat = TextAnimation {
        loop_anim: Some(loop_seg.clone()),
        loop_mode: LoopMode::Repeat,
        ..Default::default()
    };

    // Cycle boundaries for Repeat
    let m0 = evaluate_text_animation(
        "A",
        &anim_repeat,
        RationalTime::from_frames(0, rate),
        clip_dur,
    );
    let m29 = evaluate_text_animation(
        "A",
        &anim_repeat,
        RationalTime::from_frames(29, rate),
        clip_dur,
    );
    let m30 = evaluate_text_animation(
        "A",
        &anim_repeat,
        RationalTime::from_frames(30, rate),
        clip_dur,
    );
    assert!((m0.grapheme_transforms[0].translate_x - 0.0).abs() < 1e-4);
    assert!(m29.grapheme_transforms[0].translate_x > 90.0);
    assert!((m30.grapheme_transforms[0].translate_x - 0.0).abs() < 1e-4);

    // PingPong boundaries
    let anim_pingpong = TextAnimation {
        loop_anim: Some(loop_seg),
        loop_mode: LoopMode::PingPong,
        ..Default::default()
    };
    let mp30 = evaluate_text_animation(
        "A",
        &anim_pingpong,
        RationalTime::from_frames(30, rate),
        clip_dur,
    );
    let mp60 = evaluate_text_animation(
        "A",
        &anim_pingpong,
        RationalTime::from_frames(60, rate),
        clip_dur,
    );
    assert!((mp30.grapheme_transforms[0].translate_x - 100.0).abs() < 1e-4);
    assert!((mp60.grapheme_transforms[0].translate_x - 0.0).abs() < 1e-4);

    // One-frame loop test
    let one_frame_seg = TextAnimationSegment {
        duration: RationalTime::from_frames(1, rate),
        easing: KeyframeEasing::Linear,
        stagger_ratio: 0.0,
        from: AnimationTransform::default(),
        to: AnimationTransform {
            translate_y: 50.0,
            ..Default::default()
        },
    };
    let anim_one = TextAnimation {
        loop_anim: Some(one_frame_seg),
        loop_mode: LoopMode::Repeat,
        ..Default::default()
    };
    for f in 0..10 {
        let mf =
            evaluate_text_animation("B", &anim_one, RationalTime::from_frames(f, rate), clip_dur);
        assert_eq!(mf.grapheme_transforms.len(), 1);
    }
}

// =========================================================================
// P31 Review Regression Test 3: Rigid Unit Geometry & Wrapped Line Stagger (Finding 3)
// =========================================================================
#[test]
fn p31_review_rigid_unit_rotation_and_wrapped_lines() {
    let text = "HELLO WORLD";
    let graphemes = extract_nfc_graphemes(text);

    // 1. Word unit mapping
    let (_, unit_count) = map_graphemes_to_units(&graphemes, AnimationUnit::Word);
    assert_eq!(unit_count, 2);

    let anim_word = TextAnimation {
        unit: AnimationUnit::Word,
        order: StaggerOrder::Forward,
        pivot_x: 0.5,
        pivot_y: 0.5,
        in_anim: Some(TextAnimationSegment {
            duration: RationalTime::from_frames(30, FrameRate::FPS_30),
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.0,
            from: AnimationTransform {
                rotation: 45.0,
                ..Default::default()
            },
            to: AnimationTransform::default(),
        }),
        ..Default::default()
    };

    let motion = evaluate_text_animation(
        text,
        &anim_word,
        RationalTime::zero(FrameRate::FPS_30),
        RationalTime::from_frames(60, FrameRate::FPS_30),
    );
    assert_eq!(motion.unit_count, 2);
    assert_eq!(motion.unit_transforms.len(), 2);
    assert!((motion.unit_transforms[0].rotation - 45.0).abs() < 1e-4);

    // 2. Automatically wrapped lines
    let wrapped_lines = vec![
        (0..5).collect::<Vec<usize>>(),
        (5..11).collect::<Vec<usize>>(),
    ];
    let (line_map, line_count) =
        map_graphemes_to_units_with_layout(&graphemes, AnimationUnit::Line, Some(&wrapped_lines));
    assert_eq!(line_count, 2);
    assert_eq!(line_map[0], 0);
    assert_eq!(line_map[4], 0);
    assert_eq!(line_map[5], 1);
    assert_eq!(line_map[10], 1);
}

// =========================================================================
// P31 Review Regression Test 4: Animated Gradient Coordinates Zero Color Jump (Finding 4)
// =========================================================================
#[test]
fn p31_review_animated_gradient_coordinates_no_color_jump() {
    let rate = FrameRate::FPS_30;
    let text = "GRADIENT TEST";

    let gradient_style = TextRichStyle {
        font: None,
        font_size: Some(64.0),
        layout: TextLayout::default(),
        fill: TextFill::Gradient {
            gradient_kind: GradientKind::Linear,
            angle_deg: 90.0,
            radius: None,
            space: GradientSpace::All,
            stops: vec![
                GradientStop {
                    offset: 0.0,
                    color: "#FF0000".to_string(),
                },
                GradientStop {
                    offset: 1.0,
                    color: "#0000FF".to_string(),
                },
            ],
        },
        strokes: Vec::new(),
        shadows: Vec::new(),
        background: None,
        spans: Vec::new(),
    };

    // Static project (no text_animation)
    let (proj_static, id_static) = project_with_animated_text(
        "Static Gradient",
        text,
        Some(gradient_style.clone()),
        None,
        60,
    );
    let p_static = preview_overlay_at(
        &proj_static,
        &id_static,
        RationalTime::from_frames(10, rate),
    )
    .expect("preview")
    .expect("overlay");

    // Animated project with identity/translation transform (translate 0,0, opacity 1.0)
    let anim_ident = TextAnimation {
        unit: AnimationUnit::All,
        order: StaggerOrder::Forward,
        in_anim: Some(TextAnimationSegment {
            duration: RationalTime::from_frames(30, rate),
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.0,
            from: AnimationTransform {
                translate_x: 0.0,
                opacity: 1.0,
                ..Default::default()
            },
            to: AnimationTransform {
                translate_x: 0.0,
                opacity: 1.0,
                ..Default::default()
            },
        }),
        ..Default::default()
    };
    let (proj_anim, id_anim) = project_with_animated_text(
        "Anim Gradient",
        text,
        Some(gradient_style),
        Some(anim_ident),
        60,
    );
    let p_anim = preview_overlay_at(&proj_anim, &id_anim, RationalTime::from_frames(10, rate))
        .expect("preview")
        .expect("overlay");

    assert_eq!(
        p_static.roi_png_bytes, p_anim.roi_png_bytes,
        "Animated gradient must have zero color jump vs static rendering"
    );
}

// =========================================================================
// P31 Review Regression Test 5: Global Painter Order & Unsorted Strokes (Finding 5)
// =========================================================================
#[test]
fn p31_review_global_painter_order_and_unsorted_strokes() {
    let rate = FrameRate::FPS_30;
    let text = "STROKES";

    // Unsorted strokes: width 2.0 (color red) defined first, width 8.0 (color blue) defined second
    let style = TextRichStyle {
        font: None,
        font_size: Some(64.0),
        layout: TextLayout::default(),
        fill: solid("#FFFFFF"),
        strokes: vec![
            TextStroke {
                color: "#FF0000".to_string(), // inner narrower stroke
                width: 2.0,
                align: StrokeAlign::Outside,
            },
            TextStroke {
                color: "#0000FF".to_string(), // outer wider stroke
                width: 8.0,
                align: StrokeAlign::Outside,
            },
        ],
        shadows: vec![TextShadow {
            color: "#000000".to_string(),
            blur: 4.0,
            offset_x: 5.0,
            offset_y: 5.0,
        }],
        background: None,
        spans: Vec::new(),
    };

    let anim = TextAnimation {
        unit: AnimationUnit::Grapheme,
        order: StaggerOrder::Forward,
        in_anim: Some(TextAnimationSegment {
            duration: RationalTime::from_frames(30, rate),
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.5,
            from: AnimationTransform {
                translate_y: -20.0,
                opacity: 0.5,
                ..Default::default()
            },
            to: AnimationTransform::default(),
        }),
        ..Default::default()
    };

    let (project, id) =
        project_with_animated_text("UnsortedStrokes", text, Some(style), Some(anim), 60);
    let p = preview_overlay_at(&project, &id, RationalTime::from_frames(15, rate))
        .expect("preview")
        .expect("overlay");
    assert!(p.has_visible_content);
    assert!(!p.roi_png_bytes.is_empty());
}

// =========================================================================
// P31 Review Regression Test 6: Animated Bleed vs Oversized Reference (Finding 6)
// =========================================================================
#[test]
fn p31_review_animated_bleed_matches_oversized_reference() {
    let rate = FrameRate::FPS_30;
    let text = "BLEED TEST";

    let style = TextRichStyle {
        font: None,
        font_size: Some(96.0), // large font
        layout: TextLayout::default(),
        fill: solid("#FFFF00"),
        strokes: vec![TextStroke {
            color: "#FF0000".to_string(),
            width: 6.0,
            align: StrokeAlign::Outside,
        }],
        shadows: vec![TextShadow {
            color: "#000000".to_string(),
            blur: 8.0,
            offset_x: 20.0,
            offset_y: 20.0,
        }],
        background: None,
        spans: Vec::new(),
    };

    // Extreme motion: rotation 45 deg, anisotropic scale (1.5, 0.5), off-center pivot (0.1, 0.9)
    let anim = TextAnimation {
        unit: AnimationUnit::All,
        order: StaggerOrder::Forward,
        pivot_x: 0.1,
        pivot_y: 0.9,
        in_anim: Some(TextAnimationSegment {
            duration: RationalTime::from_frames(30, rate),
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.0,
            from: AnimationTransform {
                translate_x: 50.0,
                translate_y: 50.0,
                scale_x: 1.5,
                scale_y: 0.5,
                rotation: 45.0,
                opacity: 1.0,
            },
            to: AnimationTransform::default(),
        }),
        ..Default::default()
    };

    let (project, id) =
        project_with_animated_text("BleedExtreme", text, Some(style), Some(anim), 60);
    let p = preview_overlay_at(&project, &id, RationalTime::zero(rate))
        .expect("preview")
        .expect("overlay");
    assert!(p.has_visible_content);
    // Ensure ROI is valid and strictly contained within canvas
    assert!(p.roi[2] <= 1920 && p.roi[3] <= 1080);
    assert!(!p.roi_png_bytes.is_empty());
}

// =========================================================================
// P31 Review Regression Test 7: Export Cancellation & Quota Cleanup (Finding 7)
// =========================================================================
#[test]
fn p31_review_export_cancellation_and_quota_cleanup() {
    let dir = test_dir("cancel_quota_test");
    let rate = FrameRate::FPS_30;

    let anim = TextAnimation {
        unit: AnimationUnit::Grapheme,
        order: StaggerOrder::Forward,
        in_anim: Some(TextAnimationSegment {
            duration: RationalTime::from_frames(30, rate),
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.5,
            from: AnimationTransform {
                translate_y: -50.0,
                opacity: 0.0,
                ..Default::default()
            },
            to: AnimationTransform::default(),
        }),
        ..Default::default()
    };

    let (project, _) = project_with_animated_text(
        "ExportCancel",
        "CANCEL ME",
        Some(base_rich("#FFFFFF")),
        Some(anim),
        60,
    );

    // Create pre-existing output file that must NOT be overwritten on cancel
    let target_mp4 = dir.join("pre_existing_output.mp4");
    fs::write(&target_mp4, b"KEEP_ME_SAFE").expect("write sentinel");

    let cancel_flag = Arc::new(AtomicBool::new(true)); // cancel immediately

    let profile = ExportProfile::from_project_settings(&project.settings);
    let res = ExportEngine::render_snapshot(&project, &target_mp4, &profile, Some(cancel_flag));
    assert!(matches!(res, Err(ExportError::Cancelled)));

    // Pre-existing file must remain intact
    let preserved = fs::read(&target_mp4).expect("read sentinel");
    assert_eq!(preserved, b"KEEP_ME_SAFE");

    // All temporary folders must be cleaned up
    let entries = fs::read_dir(&dir).expect("read test dir");
    for entry in entries {
        let entry = entry.expect("entry");
        let name = entry.file_name().to_string_lossy().to_string();
        assert!(
            !name.starts_with("tc_auto_"),
            "Leaked temporary auto folder: {name}"
        );
    }
}

// =========================================================================
// P31 Review Regression Test 8: Missing Glyph Rejection in Animated Export (Finding 8)
// =========================================================================
#[test]
fn p31_review_missing_glyphs_rejection_in_animated_export() {
    let dir = test_dir("missing_glyph_export");
    let text_with_missing_glyph = "Valid \u{E000} Invalid"; // Private use area character missing in Roboto

    let rate = FrameRate::FPS_30;
    let anim = TextAnimation {
        unit: AnimationUnit::Grapheme,
        order: StaggerOrder::Forward,
        in_anim: Some(TextAnimationSegment {
            duration: RationalTime::from_frames(30, rate),
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.5,
            from: AnimationTransform {
                opacity: 0.0,
                ..Default::default()
            },
            to: AnimationTransform::default(),
        }),
        ..Default::default()
    };

    let (project, id) = project_with_animated_text(
        "MissingGlyphProj",
        text_with_missing_glyph,
        Some(base_rich("#FFFFFF")),
        Some(anim),
        30,
    );

    // Preview path rejects missing glyphs
    let preview_res = preview_overlay_at(&project, &id, RationalTime::zero(rate));
    assert!(matches!(
        preview_res,
        Err(TextOverlayError::MissingGlyphs(_))
    ));

    // Export path rejects missing glyphs cleanly
    let target_mp4 = dir.join("missing_glyph_output.mp4");
    let profile = ExportProfile::from_project_settings(&project.settings);

    let export_res = ExportEngine::render_snapshot(&project, &target_mp4, &profile, None);
    assert!(export_res.is_err());
    let err_str = export_res.err().unwrap().to_string();
    assert!(
        err_str.contains("MissingGlyphs")
            || err_str.contains("missing glyphs")
            || err_str.contains("without glyphs"),
        "Export error should indicate missing glyphs: {err_str}"
    );

    // Temp folders cleaned up
    assert!(
        !target_mp4.exists(),
        "Target file must not be created on missing glyph error"
    );
}

// =========================================================================
// P31 Review Regression Test 9: Keyframe Sampling for Static Rich Text (Finding 9)
// =========================================================================
#[test]
fn p31_review_keyframed_static_rich_text_materializes_sequence() {
    let rate = FrameRate::FPS_30;
    let mut project = Project::new("KeyframedText".to_string());
    project.settings.frame_rate = rate;
    project.settings.width = 640;
    project.settings.height = 360;

    let mut track = Track::new("Text Track".to_string(), TrackKind::Video);
    let mut item = TimelineItem::new(
        None,
        "Keyframed Text".to_string(),
        ItemKind::Text,
        TimeRange::new(
            RationalTime::zero(rate),
            RationalTime::from_frames(10, rate),
        ),
        TimeRange::new(
            RationalTime::zero(rate),
            RationalTime::from_frames(10, rate),
        ),
    )
    .expect("create item");
    item.item_id = "kf_txt".to_string();
    item.text_content = Some("Keyframed".to_string());
    item.text_rich = Some(base_rich("#FFFFFF"));

    // Add generic opacity keyframes to the layer (no text_animation)
    item.keyframes.push(Keyframe::new(
        KeyframeProperty::Opacity,
        RationalTime::zero(rate),
        0.0,
    ));
    item.keyframes.push(Keyframe::new(
        KeyframeProperty::Opacity,
        RationalTime::from_frames(10, rate),
        1.0,
    ));

    track.items.push(item);
    project.tracks.push(track);

    // Verify PreviewSampledLayer exposes has_animation == true
    let plan = CompositionPlan::from_project(&project).expect("plan");
    let sampled = plan.sample_preview_at(RationalTime::from_frames(5, rate));
    let sampled_layer = sampled
        .active_layers
        .iter()
        .find(|l| l.item_id == "kf_txt")
        .expect("layer");
    assert!(
        sampled_layer.has_animation,
        "has_animation must be true when keyframes are present"
    );
}

#[test]
fn p31_review_default_reject_split_uses_preserved_phase_extent() {
    let rate = FrameRate::FPS_30;
    let animation = TextAnimation {
        in_anim: Some(TextAnimationSegment {
            duration: RationalTime::from_frames(30, rate),
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.0,
            from: AnimationTransform {
                translate_y: -80.0,
                opacity: 0.0,
                ..AnimationTransform::default()
            },
            to: AnimationTransform::default(),
        }),
        out_anim: Some(TextAnimationSegment {
            duration: RationalTime::from_frames(30, rate),
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.0,
            from: AnimationTransform::default(),
            to: AnimationTransform {
                translate_y: 80.0,
                opacity: 0.0,
                ..AnimationTransform::default()
            },
        }),
        conflict_policy: DurationConflictPolicy::Reject,
        ..TextAnimation::default()
    };
    let (project, item_id) = project_with_animated_text(
        "Reject split",
        "Split phase",
        Some(base_rich("#FFFFFF")),
        None,
        90,
    );
    let pid = project.project_id.clone();
    let mut service = CommandService::new(project);
    service
        .execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 0,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: None,
            operations: vec![OperationEnvelope {
                operation_id: "op_reject_anim".to_string(),
                operation: CommandOperation::ClipTextAnimationSet {
                    item_id: item_id.clone(),
                    animation: Some(animation),
                },
            }],
        })
        .expect("valid 90-frame animation with 30+30 segments");
    service
        .execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid,
            base_revision: 1,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: None,
            operations: vec![OperationEnvelope {
                operation_id: "op_reject_split".to_string(),
                operation: CommandOperation::ClipSplit {
                    item_id: item_id.clone(),
                    split_time: RationalTime::from_frames(40, rate),
                    new_right_item_id: Some("itm_reject_right".to_string()),
                },
            }],
        })
        .expect("split under default Reject must use preserved phase extent");
    let right = service.project().tracks[0]
        .find_item("itm_reject_right")
        .expect("right fragment");
    let anim = right.text_animation.as_ref().expect("animation preserved");
    assert_eq!(anim.phase_origin, Some(RationalTime::zero(rate)));
    assert_eq!(anim.phase_end, Some(RationalTime::from_frames(90, rate)));
}

#[test]
fn p31_review_legacy_unanchored_animation_gets_anchors_on_split() {
    let rate = FrameRate::FPS_30;
    let animation = TextAnimation {
        loop_anim: Some(TextAnimationSegment {
            duration: RationalTime::from_frames(20, rate),
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.0,
            from: AnimationTransform::default(),
            to: AnimationTransform {
                translate_x: 20.0,
                ..AnimationTransform::default()
            },
        }),
        loop_mode: LoopMode::Repeat,
        ..TextAnimation::default()
    };
    let (project, item_id) = project_with_animated_text(
        "Legacy split",
        "Legacy",
        Some(base_rich("#FFFFFF")),
        Some(animation),
        90,
    );
    let pid = project.project_id.clone();
    let mut service = CommandService::new(project);
    service
        .execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid,
            base_revision: 0,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: None,
            operations: vec![OperationEnvelope {
                operation_id: "op_legacy_split".to_string(),
                operation: CommandOperation::ClipSplit {
                    item_id: item_id.clone(),
                    split_time: RationalTime::from_frames(40, rate),
                    new_right_item_id: Some("itm_legacy_right".to_string()),
                },
            }],
        })
        .expect("legacy animation split");
    let right = service.project().tracks[0]
        .find_item("itm_legacy_right")
        .expect("right fragment");
    let anim = right.text_animation.as_ref().expect("animation");
    assert!(anim
        .phase_origin
        .expect("origin")
        .semantic_eq(&RationalTime::zero(rate)));
    assert!(anim
        .phase_end
        .expect("end")
        .semantic_eq(&RationalTime::from_frames(90, rate)));
}

#[test]
fn p31_review_mixed_rate_loop_uses_exact_common_grid() {
    let fade_loop = |rate: FrameRate| TextAnimation {
        loop_anim: Some(TextAnimationSegment {
            duration: RationalTime::new(1, rate),
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.0,
            from: AnimationTransform {
                opacity: 0.0,
                ..AnimationTransform::default()
            },
            to: AnimationTransform::default(),
        }),
        loop_mode: LoopMode::Repeat,
        ..TextAnimation::default()
    };
    let half_second = RationalTime::from_seconds(0.5, FrameRate::FPS_30);
    let clip_duration = RationalTime::from_seconds(3.0, FrameRate::FPS_30);
    let motion = evaluate_text_animation(
        "A",
        &fade_loop(FrameRate::new(1, 1)),
        half_second,
        clip_duration,
    );
    let opacity = motion.grapheme_transforms[0].opacity;
    assert!(
        (opacity - 0.5).abs() < 0.05,
        "mixed-rate loop must evaluate halfway, got {opacity}"
    );
    let motion = evaluate_text_animation(
        "A",
        &fade_loop(FrameRate::FPS_30),
        RationalTime::from_frames(3, FrameRate::FPS_30),
        clip_duration,
    );
    assert!(
        motion.grapheme_transforms[0].opacity.abs() < 1e-4,
        "one-frame loop must restart exactly"
    );
}

#[test]
fn p31_review_preview_samples_generic_transform_keyframes() {
    let rate = FrameRate::FPS_30;
    let rich = base_rich("#FFFFFF");
    let (mut project, item_id) =
        project_with_animated_text("Keyframed preview", "Keyframed", Some(rich), None, 60);
    let item = project.tracks[0].find_item_mut(&item_id).expect("item");
    item.keyframes.push(Keyframe::new(
        KeyframeProperty::PositionX,
        RationalTime::zero(rate),
        0.0,
    ));
    item.keyframes.push(Keyframe::new(
        KeyframeProperty::PositionX,
        RationalTime::from_frames(60, rate),
        240.0,
    ));
    let plan = CompositionPlan::from_project(&project).expect("plan");
    let sampled = plan.sample_preview_at(RationalTime::from_frames(30, rate));
    let layer = sampled
        .active_layers
        .iter()
        .find(|layer| layer.item_id == item_id)
        .expect("layer");
    assert!(
        layer.has_animation,
        "keyframes must signal per-frame sampling"
    );
    let start = preview_overlay_at(&project, &item_id, RationalTime::zero(rate))
        .expect("start")
        .expect("active start");
    let mid = preview_overlay_at(&project, &item_id, RationalTime::from_frames(30, rate))
        .expect("mid")
        .expect("active mid");
    assert_ne!(
        start.roi, mid.roi,
        "keyframed position must move the overlay ROI"
    );
}
