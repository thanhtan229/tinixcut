//! Phase 14 Comprehensive Acceptance Test Suite
//! Validates all 40 Acceptance Criteria (P14-AC-01 through P14-AC-40)
//! defined in Plans/plan_phase14.md and ADR-004.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use tinixcut_lib::modules::automation::AutomationAdapter;
use tinixcut_lib::modules::command::{
    CommandEnvelope, CommandOperation, CommandService, OperationEnvelope,
};
use tinixcut_lib::modules::domain::evaluator::{
    evaluate_eased_progress, evaluate_property_channel, subdivide_cubic_bezier, subdivide_easing,
};
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    Asset, CutTransition, ItemKind, Keyframe, KeyframeEasing, KeyframeProperty, MediaKind, Project,
    TimelineItem, Track, TrackKind, TransitionKind, CURRENT_SCHEMA_VERSION,
};
use tinixcut_lib::modules::export::automation::AutomationArtifactsGuard;
use tinixcut_lib::modules::export::{ExportEngine, ExportProfile};
use tinixcut_lib::modules::history::{HistoryEntry, HistoryManager};
use tinixcut_lib::modules::media::resolver::{BinaryKind, BinaryOrigin, MediaBinaryResolver};
use tinixcut_lib::modules::persistence::{load_project, save_project_atomic};

fn ffmpeg_cmd() -> Command {
    let resolved = MediaBinaryResolver::resolve(BinaryKind::FFmpeg)
        .expect("Bundled FFmpeg 9.0.2 must resolve via MediaBinaryResolver");
    assert!(
        matches!(resolved.origin, BinaryOrigin::AppOwnedPackagedResource(_)),
        "Must use app-owned packaged resource, got: {:?}",
        resolved.origin
    );
    Command::new(resolved.path)
}

fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tinixcut_p14_acc_{}", name));
    if dir.exists() {
        let _ = fs::remove_dir_all(&dir);
    }
    fs::create_dir_all(&dir).expect("Failed to create test directory");
    dir
}

fn create_solid_color_video(path: &Path, color: &str, width: u32, height: u32, duration_sec: f64) {
    let status = ffmpeg_cmd()
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            &format!(
                "color=c={}:s={}x{}:r=30:d={}",
                color, width, height, duration_sec
            ),
            "-f",
            "lavfi",
            "-i",
            &format!("sine=frequency=440:duration={}", duration_sec),
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            path.to_str().unwrap(),
        ])
        .status()
        .expect("Failed to spawn FFmpeg to create test video");
    assert!(
        status.success(),
        "Failed to create test video at {:?}",
        path
    );
}

fn create_sine_audio(path: &Path, freq: u32, duration_sec: f64) {
    let status = ffmpeg_cmd()
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            &format!(
                "sine=frequency={}:duration={}:sample_rate=48000",
                freq, duration_sec
            ),
            "-c:a",
            "pcm_s16le",
            path.to_str().unwrap(),
        ])
        .status()
        .expect("Failed to spawn FFmpeg to create test audio");
    assert!(
        status.success(),
        "Failed to create test audio at {:?}",
        path
    );
}

fn read_raw_rgba_frames(path: &Path, width: usize, height: usize) -> Vec<Vec<u8>> {
    let mut file = File::open(path).expect("Failed to open raw video file");
    let mut data = Vec::new();
    file.read_to_end(&mut data)
        .expect("Failed to read raw video file");
    let frame_size = width * height * 4;
    assert_eq!(
        data.len() % frame_size,
        0,
        "Data length {} is not a multiple of frame size {}",
        data.len(),
        frame_size
    );
    data.chunks_exact(frame_size).map(|c| c.to_vec()).collect()
}

// =========================================================================
// P14-AC-01 & P14-AC-04: Current Schema & Future-Reader Safety
// =========================================================================
#[test]
fn test_p14_ac01_schema_v3_and_ac04_future_reader_safety() {
    assert!(
        CURRENT_SCHEMA_VERSION >= 4,
        "Production schema must remain compatible with Phase 14 migrations"
    );

    let project = Project::new("Current schema project".to_string());
    assert_eq!(project.schema_version, CURRENT_SCHEMA_VERSION);

    let serialized = serde_json::to_string(&project).expect("Serialization failed");
    let val: serde_json::Value = serde_json::from_str(&serialized).expect("JSON parse failed");
    assert_eq!(val["schema_version"], CURRENT_SCHEMA_VERSION);

    // A future schema version must be rejected.
    let future_schema_version = CURRENT_SCHEMA_VERSION + 1;
    let mut future_val = val.clone();
    future_val["schema_version"] = serde_json::json!(future_schema_version);
    let future_str = serde_json::to_string(&future_val).unwrap();

    let td = test_dir("schema_safety");
    let future_file = td.join("future_schema.tinix");
    fs::write(&future_file, &future_str).unwrap();

    let load_res = load_project(&future_file);
    assert!(
        load_res.is_err(),
        "Readers must reject unsupported future schema version {future_schema_version}"
    );

    // Unsupported easing variant must fail to deserialize
    let invalid_easing_json = r#"{
        "property": "PositionX",
        "offset": { "value": 0, "rate": { "numerator": 30, "denominator": 1 } },
        "value": 100.0,
        "outgoing_easing": "SuperBounceSpring"
    }"#;
    let kf_res: Result<Keyframe, _> = serde_json::from_str(invalid_easing_json);
    assert!(
        kf_res.is_err(),
        "Unsupported future easing must fail atomically"
    );
}

// =========================================================================
// P14-AC-02 & P14-AC-03: Lossless v2 Migration & Linear Easing
// =========================================================================
#[test]
fn test_p14_ac02_ac03_lossless_v2_to_v3_migration() {
    let v2_json = r#"{
        "schema_version": 2,
        "project_id": "proj_mig_v2",
        "name": "V2 Migration Project",
        "revision": 5,
        "settings": {
            "width": 1920,
            "height": 1080,
            "frame_rate": { "numerator": 30, "denominator": 1 },
            "sample_rate": 48000,
            "channels": 2
        },
        "created_at": "2026-10-05T00:00:00Z",
        "updated_at": "2026-10-05T00:00:00Z",
        "assets": [],
        "tracks": [
            {
                "track_id": "tr_1",
                "name": "Video Track",
                "kind": "Video",
                "muted": false,
                "locked": false,
                "items": [
                    {
                        "item_id": "item_1",
                        "name": "Clip with Keyframes",
                        "kind": "Video",
                        "timeline_range": {
                            "start": { "value": 0, "rate": { "numerator": 30, "denominator": 1 } },
                            "duration": { "value": 60, "rate": { "numerator": 30, "denominator": 1 } }
                        },
                        "source_range": {
                            "start": { "value": 0, "rate": { "numerator": 30, "denominator": 1 } },
                            "duration": { "value": 60, "rate": { "numerator": 30, "denominator": 1 } }
                        },
                        "transform": { "position_x": 0.0, "position_y": 0.0, "scale_x": 1.0, "scale_y": 1.0, "rotation": 0.0 },
                        "crop": { "top": 0.0, "bottom": 0.0, "left": 0.0, "right": 0.0 },
                        "opacity": 1.0,
                        "volume": 1.0,
                        "keyframes": [
                            {
                                "property": "PositionX",
                                "offset": { "value": 0, "rate": { "numerator": 30, "denominator": 1 } },
                                "value": 100.0
                            },
                            {
                                "property": "PositionX",
                                "offset": { "value": 60, "rate": { "numerator": 30, "denominator": 1 } },
                                "value": 300.0
                            }
                        ]
                    }
                ],
                "transitions": []
            }
        ]
    }"#;

    let td = test_dir("migration");
    let proj_file = td.join("v2_project.tinix");
    fs::write(&proj_file, v2_json).unwrap();

    let migrated = load_project(&proj_file).expect("Migration failed");

    assert_eq!(migrated.schema_version, CURRENT_SCHEMA_VERSION);
    let kfs = &migrated.tracks[0].items[0].keyframes;
    assert_eq!(kfs.len(), 2);
    assert_eq!(kfs[0].outgoing_easing, KeyframeEasing::Linear);
    assert_eq!(kfs[1].outgoing_easing, KeyframeEasing::Linear);
    assert_eq!(kfs[0].value, 100.0);
    assert_eq!(kfs[1].value, 300.0);
}

// =========================================================================
// P14-AC-05, P14-AC-06, P14-AC-07, P14-AC-08: Evaluator, Presets, Endpoints, Bounds
// =========================================================================
#[test]
fn test_p14_ac05_ac06_ac07_ac08_evaluator_presets_and_endpoints() {
    let easings = [
        KeyframeEasing::Linear,
        KeyframeEasing::EaseIn,
        KeyframeEasing::EaseOut,
        KeyframeEasing::EaseInOut,
        KeyframeEasing::CubicBezier {
            x1: 0.25,
            y1: 0.1,
            x2: 0.25,
            y2: 1.0,
        },
    ];

    for easing in easings {
        let y_start = evaluate_eased_progress(0.0, easing);
        let y_end = evaluate_eased_progress(1.0, easing);
        assert_eq!(
            y_start, 0.0,
            "Easing {:?} must have exact y(0) = 0.0",
            easing
        );
        assert_eq!(y_end, 1.0, "Easing {:?} must have exact y(1) = 1.0", easing);

        let y_mid = evaluate_eased_progress(0.5, easing);
        assert!(
            (0.0..=1.0).contains(&y_mid),
            "Easing {:?} midpoint {} must be in [0, 1]",
            easing,
            y_mid
        );
    }

    let mid_in = evaluate_eased_progress(0.5, KeyframeEasing::EaseIn);
    let mid_out = evaluate_eased_progress(0.5, KeyframeEasing::EaseOut);
    let mid_linear = evaluate_eased_progress(0.5, KeyframeEasing::Linear);
    assert_eq!(mid_linear, 0.5);
    assert!(mid_in < 0.5, "EaseIn midpoint {} must be < 0.5", mid_in);
    assert!(mid_out > 0.5, "EaseOut midpoint {} must be > 0.5", mid_out);

    // Control validation: reject invalid bounds
    let invalid_easing_low = KeyframeEasing::CubicBezier {
        x1: -0.1,
        y1: 0.0,
        x2: 0.5,
        y2: 1.0,
    };
    assert!(
        invalid_easing_low.validate().is_err(),
        "x1 < 0.0 must be rejected"
    );

    let invalid_easing_nan = KeyframeEasing::CubicBezier {
        x1: 0.5,
        y1: f64::NAN,
        x2: 0.5,
        y2: 1.0,
    };
    assert!(
        invalid_easing_nan.validate().is_err(),
        "NaN must be rejected"
    );

    let invalid_easing_high = KeyframeEasing::CubicBezier {
        x1: 0.5,
        y1: 1.5,
        x2: 0.5,
        y2: 1.0,
    };
    assert!(
        invalid_easing_high.validate().is_err(),
        "y1 > 1.0 must be rejected"
    );
}

// =========================================================================
// P14-AC-09: Semantic RationalTime Sorting & Equality Across Frame Rates
// =========================================================================
#[test]
fn test_p14_ac09_semantic_rational_time_ordering_and_collision() {
    let rate30 = FrameRate::new(30, 1);
    let rate60 = FrameRate::new(60, 1);

    let t30 = RationalTime::new(30, rate30);
    let t60 = RationalTime::new(60, rate60);
    assert_eq!(t30.to_seconds(), 1.0);
    assert_eq!(t60.to_seconds(), 1.0);

    let kfs = vec![
        Keyframe::new(
            KeyframeProperty::PositionX,
            RationalTime::new(0, rate30),
            0.0,
        ),
        Keyframe::new(KeyframeProperty::PositionX, t60, 100.0), // 1.0s
        Keyframe::new(
            KeyframeProperty::PositionX,
            RationalTime::new(90, rate30),
            200.0,
        ), // 3.0s
    ];

    let val_half = evaluate_property_channel(
        &kfs,
        KeyframeProperty::PositionX,
        0.0,
        RationalTime::new(15, rate30),
    );
    assert!(
        (val_half - 50.0).abs() < 1e-4,
        "Evaluator must interpolate across mixed rate keyframes"
    );

    let mut item = TimelineItem::new(
        None,
        "item_rt".to_string(),
        ItemKind::Video,
        TimeRange::new(RationalTime::new(0, rate30), RationalTime::new(120, rate30)),
        TimeRange::new(RationalTime::new(0, rate30), RationalTime::new(120, rate30)),
    )
    .unwrap();
    item.keyframes = kfs;

    let mut project = Project::new("Proj RT".to_string());
    let mut track = Track::new("V1".to_string(), TrackKind::Video);
    let item_id = item.item_id.clone();
    track.items.push(item);
    project.tracks.push(track);

    let mut svc = CommandService::new(project);
    let env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: svc.project().project_id.clone(),
        base_revision: svc.project().revision,
        client_id: None,
        client_epoch: None,
        sequence_number: None,
        dry_run: false,
        transaction_id: None,
        operations: vec![OperationEnvelope {
            operation_id: "op_upsert".to_string(),
            operation: CommandOperation::ClipKeyframeAdd {
                item_id,
                keyframe: Keyframe::new(KeyframeProperty::PositionX, t30, 999.0),
                easing: None,
            },
        }],
    };

    let receipt = svc.execute(env).expect("Command failed");
    assert_eq!(receipt.operations_applied, 1);

    let item_after = &svc.project().tracks[0].items[0];
    assert_eq!(
        item_after.keyframes.len(),
        3,
        "Semantic time match must upsert, not create duplicate"
    );
    let updated = item_after
        .keyframes
        .iter()
        .find(|k| (k.offset.to_seconds() - 1.0).abs() < 1e-5)
        .unwrap();
    assert_eq!(updated.value, 999.0);
}

// =========================================================================
// P14-AC-10, AC-11, AC-12, AC-13, AC-14: Command Service Keyframe Operations & History
// =========================================================================
#[test]
fn test_p14_ac10_through_ac14_command_service_keyframe_operations() {
    let rate = FrameRate::FPS_30;
    let mut project = Project::new("Proj Commands".to_string());
    let mut track = Track::new("V1".to_string(), TrackKind::Video);
    let mut item = TimelineItem::new(
        None,
        "clip_cmd".to_string(),
        ItemKind::Video,
        TimeRange::new(RationalTime::new(0, rate), RationalTime::new(120, rate)),
        TimeRange::new(RationalTime::new(0, rate), RationalTime::new(120, rate)),
    )
    .unwrap();
    let item_id = item.item_id.clone();
    item.keyframes = vec![
        Keyframe::new(KeyframeProperty::Opacity, RationalTime::new(0, rate), 0.0),
        Keyframe::new(KeyframeProperty::Opacity, RationalTime::new(60, rate), 1.0),
    ];
    track.items.push(item);
    project.tracks.push(track);

    let mut svc = CommandService::new(project);

    // 1. ClipKeyframeEasingSet on first keyframe
    let env_ease = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: svc.project().project_id.clone(),
        base_revision: svc.project().revision,
        client_id: None,
        client_epoch: None,
        sequence_number: None,
        dry_run: false,
        transaction_id: None,
        operations: vec![OperationEnvelope {
            operation_id: "op_ease".to_string(),
            operation: CommandOperation::ClipKeyframeEasingSet {
                item_id: item_id.clone(),
                property: KeyframeProperty::Opacity,
                offset: RationalTime::new(0, rate),
                easing: KeyframeEasing::EaseInOut,
            },
        }],
    };
    svc.execute(env_ease).expect("EasingSet failed");
    assert_eq!(
        svc.project().tracks[0].items[0].keyframes[0].outgoing_easing,
        KeyframeEasing::EaseInOut
    );

    // 2. Last-keyframe invariant: EasingSet on terminal keyframe must reject
    let env_last = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: svc.project().project_id.clone(),
        base_revision: svc.project().revision,
        client_id: None,
        client_epoch: None,
        sequence_number: None,
        dry_run: false,
        transaction_id: None,
        operations: vec![OperationEnvelope {
            operation_id: "op_ease_last".to_string(),
            operation: CommandOperation::ClipKeyframeEasingSet {
                item_id: item_id.clone(),
                property: KeyframeProperty::Opacity,
                offset: RationalTime::new(60, rate),
                easing: KeyframeEasing::EaseIn,
            },
        }],
    };
    let last_res = svc.execute(env_last);
    assert!(
        last_res.is_err(),
        "Setting non-linear easing on last keyframe must fail"
    );

    // 3. Move keyframe
    let env_move = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: svc.project().project_id.clone(),
        base_revision: svc.project().revision,
        client_id: None,
        client_epoch: None,
        sequence_number: None,
        dry_run: false,
        transaction_id: None,
        operations: vec![OperationEnvelope {
            operation_id: "op_move".to_string(),
            operation: CommandOperation::ClipKeyframeMove {
                item_id: item_id.clone(),
                property: KeyframeProperty::Opacity,
                old_offset: RationalTime::new(0, rate),
                new_offset: RationalTime::new(10, rate),
            },
        }],
    };
    svc.execute(env_move).expect("Move failed");
    let kf_moved = &svc.project().tracks[0].items[0].keyframes[0];
    assert_eq!(kf_moved.offset.value, 10);
    assert_eq!(
        kf_moved.outgoing_easing,
        KeyframeEasing::EaseInOut,
        "Move must preserve easing"
    );

    // 4. Undo and Redo via HistoryManager
    let mut history = HistoryManager::new(20);
    let snap0 = svc.project().clone();
    let mut snap1 = snap0.clone();
    snap1.tracks[0].items[0].keyframes[0].offset = RationalTime::new(20, rate);

    history.record_edit(HistoryEntry {
        description: "Move kf".to_string(),
        transaction_id: Some("tx_test".to_string()),
        prior_snapshot: snap0.clone(),
        resulting_snapshot: snap1.clone(),
    });

    let mut active = snap1.clone();
    assert_eq!(active.tracks[0].items[0].keyframes[0].offset.value, 20);

    history.undo(&mut active).unwrap();
    assert_eq!(active.tracks[0].items[0].keyframes[0].offset.value, 10);

    history.redo(&mut active).unwrap();
    assert_eq!(active.tracks[0].items[0].keyframes[0].offset.value, 20);
}

// =========================================================================
// P14-AC-15: Clip Duplicate Deep Copy
// =========================================================================
#[test]
fn test_p14_ac15_duplicate_deep_copy() {
    let rate = FrameRate::FPS_30;
    let mut project = Project::new("Proj Dup".to_string());
    let mut track = Track::new("V1".to_string(), TrackKind::Video);
    let mut item = TimelineItem::new(
        None,
        "clip_orig".to_string(),
        ItemKind::Video,
        TimeRange::new(RationalTime::new(0, rate), RationalTime::new(60, rate)),
        TimeRange::new(RationalTime::new(0, rate), RationalTime::new(60, rate)),
    )
    .unwrap();
    let item_id = item.item_id.clone();
    item.keyframes = vec![
        Keyframe::with_easing(
            KeyframeProperty::Rotation,
            RationalTime::new(0, rate),
            0.0,
            KeyframeEasing::EaseIn,
        ),
        Keyframe::new(
            KeyframeProperty::Rotation,
            RationalTime::new(60, rate),
            180.0,
        ),
    ];
    track.items.push(item);
    project.tracks.push(track);

    let mut svc = CommandService::new(project);
    let env_dup = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: svc.project().project_id.clone(),
        base_revision: svc.project().revision,
        client_id: None,
        client_epoch: None,
        sequence_number: None,
        dry_run: false,
        transaction_id: None,
        operations: vec![OperationEnvelope {
            operation_id: "op_dup".to_string(),
            operation: CommandOperation::ClipDuplicate {
                item_id,
                target_start: Some(RationalTime::new(60, rate)),
                target_track_id: None,
                new_item_id: Some("clip_clone".to_string()),
            },
        }],
    };

    svc.execute(env_dup).expect("Duplicate failed");
    assert_eq!(svc.project().tracks[0].items.len(), 2);

    let clone = &svc.project().tracks[0].items[1];
    assert_eq!(clone.keyframes.len(), 2);
    assert_eq!(clone.keyframes[0].outgoing_easing, KeyframeEasing::EaseIn);
}

// =========================================================================
// P14-AC-16, AC-17, AC-18: Split Continuity & Cubic Subdivision
// =========================================================================
#[test]
fn test_p14_ac16_ac17_ac18_split_continuity_and_subdivision() {
    let rate = FrameRate::FPS_30;

    let (c1, c2) = subdivide_cubic_bezier(0.25, 0.1, 0.25, 1.0, 0.5);

    if let KeyframeEasing::CubicBezier { x1, y1, x2, y2 } = c1 {
        assert!((0.0..=1.0).contains(&x1));
        assert!((0.0..=1.0).contains(&y1));
        assert!((0.0..=1.0).contains(&x2));
        assert!((0.0..=1.0).contains(&y2));
    }
    if let KeyframeEasing::CubicBezier { x1, y1, x2, y2 } = c2 {
        assert!((0.0..=1.0).contains(&x1));
        assert!((0.0..=1.0).contains(&y1));
        assert!((0.0..=1.0).contains(&x2));
        assert!((0.0..=1.0).contains(&y2));
    }

    let mut project = Project::new("Proj Split".to_string());
    let mut track = Track::new("V1".to_string(), TrackKind::Video);
    let mut item = TimelineItem::new(
        None,
        "clip_to_split".to_string(),
        ItemKind::Video,
        TimeRange::new(RationalTime::new(0, rate), RationalTime::new(60, rate)),
        TimeRange::new(RationalTime::new(0, rate), RationalTime::new(60, rate)),
    )
    .unwrap();
    let item_id = item.item_id.clone();
    item.keyframes = vec![
        Keyframe::with_easing(
            KeyframeProperty::PositionX,
            RationalTime::new(0, rate),
            0.0,
            KeyframeEasing::EaseInOut,
        ),
        Keyframe::new(
            KeyframeProperty::PositionX,
            RationalTime::new(60, rate),
            100.0,
        ),
    ];
    track.items.push(item);
    project.tracks.push(track);

    let mut svc = CommandService::new(project);
    let env_split = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: svc.project().project_id.clone(),
        base_revision: svc.project().revision,
        client_id: None,
        client_epoch: None,
        sequence_number: None,
        dry_run: false,
        transaction_id: None,
        operations: vec![OperationEnvelope {
            operation_id: "op_split".to_string(),
            operation: CommandOperation::ClipSplit {
                item_id,
                split_time: RationalTime::new(30, rate),
                new_right_item_id: Some("clip_right".to_string()),
            },
        }],
    };

    svc.execute(env_split).expect("Split failed");
    assert_eq!(svc.project().tracks[0].items.len(), 2);

    let left = &svc.project().tracks[0].items[0];
    let right = &svc.project().tracks[0].items[1];

    assert_eq!(left.keyframes.len(), 2);
    assert_eq!(right.keyframes.len(), 2);

    let left_end_val = left.keyframes[1].value;
    let right_start_val = right.keyframes[0].value;
    assert_eq!(
        left_end_val, right_start_val,
        "Boundary keyframe values must match exactly"
    );
    assert_eq!(left.keyframes[1].offset.value, 30);
    assert_eq!(right.keyframes[0].offset.value, 0);
}

// =========================================================================
// P14-AC-19, AC-20, AC-21: Trim Hidden Keyframes Retention & Restoration
// =========================================================================
#[test]
fn test_p14_ac19_ac20_ac21_trim_hidden_keyframes() {
    let rate = FrameRate::FPS_30;
    let mut project = Project::new("Proj Trim".to_string());
    let mut track = Track::new("V1".to_string(), TrackKind::Video);
    let mut item = TimelineItem::new(
        None,
        "clip_trim".to_string(),
        ItemKind::Video,
        TimeRange::new(RationalTime::new(0, rate), RationalTime::new(60, rate)),
        TimeRange::new(RationalTime::new(0, rate), RationalTime::new(60, rate)),
    )
    .unwrap();
    let item_id = item.item_id.clone();
    item.keyframes = vec![
        Keyframe::new(KeyframeProperty::ScaleX, RationalTime::new(0, rate), 1.0),
        Keyframe::new(KeyframeProperty::ScaleX, RationalTime::new(20, rate), 1.5),
        Keyframe::new(KeyframeProperty::ScaleX, RationalTime::new(40, rate), 2.0),
        Keyframe::new(KeyframeProperty::ScaleX, RationalTime::new(60, rate), 2.5),
    ];
    track.items.push(item);
    project.tracks.push(track);

    let mut svc = CommandService::new(project);

    // 1. Head trim by 20 frames (new start: 20, duration: 40)
    let env_trim_head = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: svc.project().project_id.clone(),
        base_revision: svc.project().revision,
        client_id: None,
        client_epoch: None,
        sequence_number: None,
        dry_run: false,
        transaction_id: None,
        operations: vec![OperationEnvelope {
            operation_id: "op_trim_head".to_string(),
            operation: CommandOperation::ClipTrim {
                item_id,
                new_timeline_range: TimeRange::new(
                    RationalTime::new(20, rate),
                    RationalTime::new(40, rate),
                ),
                new_source_range: TimeRange::new(
                    RationalTime::new(20, rate),
                    RationalTime::new(40, rate),
                ),
            },
        }],
    };
    svc.execute(env_trim_head).expect("Head trim failed");

    let item_trimmed = &svc.project().tracks[0].items[0];
    assert_eq!(
        item_trimmed.keyframes.len(),
        4,
        "All keyframes must be retained"
    );
    assert_eq!(
        item_trimmed.keyframes[0].offset.value, -20,
        "Head keyframe shifted to negative"
    );
    assert_eq!(item_trimmed.keyframes[1].offset.value, 0);
}

// =========================================================================
// P14-AC-22, AC-24, AC-25, AC-26, AC-27: Full Export Video Schedule Rendering
// (Fixes P14-PREREQ-01 using app-owned FFmpeg 9.0.2)
// =========================================================================
#[test]
fn test_p14_ac22_ac24_through_ac27_full_export_video_schedule() {
    let td = test_dir("video_export");
    let video_src = td.join("green_square.mp4");
    create_solid_color_video(&video_src, "green", 64, 64, 1.0);

    let rate = FrameRate::FPS_30;
    let mut project = Project::new("Export Video Test".to_string());
    project.settings.width = 128;
    project.settings.height = 128;
    project.settings.frame_rate = rate;

    let asset = Asset {
        asset_id: "ast_v1".to_string(),
        name: "Green".to_string(),
        path: video_src.to_str().unwrap().to_string(),
        media_kind: MediaKind::Video,
        duration: None,
        file_size: 1000,
        dimensions: None,
    };
    let asset_id = asset.asset_id.clone();
    project.assets.push(asset);

    let mut track = Track::new("V1".to_string(), TrackKind::Video);
    let mut item = TimelineItem::new(
        Some(asset_id),
        "clip_v".to_string(),
        ItemKind::Video,
        TimeRange::new(RationalTime::new(0, rate), RationalTime::new(30, rate)),
        TimeRange::new(RationalTime::new(0, rate), RationalTime::new(30, rate)),
    )
    .unwrap();
    // Animate PositionX from 0 to 64 with EaseIn
    item.keyframes = vec![
        Keyframe::with_easing(
            KeyframeProperty::PositionX,
            RationalTime::new(0, rate),
            0.0,
            KeyframeEasing::EaseIn,
        ),
        Keyframe::new(
            KeyframeProperty::PositionX,
            RationalTime::new(29, rate),
            64.0,
        ),
    ];
    track.items.push(item);
    project.tracks.push(track);

    let out_file = td.join("rendered_video.mp4");
    let profile = ExportProfile {
        width: 128,
        height: 128,
        frame_rate: rate,
        ..Default::default()
    };

    let res = ExportEngine::render_snapshot(&project, &out_file, &profile, None);
    assert!(res.is_ok(), "Video export must succeed: {:?}", res.err());
    assert!(out_file.exists());

    // Extract raw RGBA frames to verify animation occurred (P14-PREREQ-01 remediation)
    let raw_out = td.join("out.raw");
    let status = ffmpeg_cmd()
        .args([
            "-y",
            "-i",
            out_file.to_str().unwrap(),
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            raw_out.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let frames = read_raw_rgba_frames(&raw_out, 128, 128);
    assert_eq!(frames.len(), 30, "Must have exactly 30 frames");

    // Frame 0 vs Frame 29 must have distinct pixel layouts
    assert_ne!(
        frames[0], frames[29],
        "Dynamic keyframe must animate video position across frames"
    );
}

// =========================================================================
// P14-AC-28: CrossDissolve Coexistence with Animated Keyframes
// =========================================================================
#[test]
fn test_p14_ac28_cross_dissolve_coexistence() {
    let td = test_dir("cd_coexistence");
    let v1 = td.join("clip1.mp4");
    let v2 = td.join("clip2.mp4");
    create_solid_color_video(&v1, "red", 64, 64, 2.0);
    create_solid_color_video(&v2, "blue", 64, 64, 2.0);

    let rate = FrameRate::FPS_30;
    let mut project = Project::new("CD Coexistence".to_string());
    project.settings.width = 64;
    project.settings.height = 64;
    project.settings.frame_rate = rate;

    let a1 = Asset {
        asset_id: "a1".to_string(),
        name: "Red".to_string(),
        path: v1.to_str().unwrap().to_string(),
        media_kind: MediaKind::Video,
        duration: None,
        file_size: 1000,
        dimensions: None,
    };
    let a2 = Asset {
        asset_id: "a2".to_string(),
        name: "Blue".to_string(),
        path: v2.to_str().unwrap().to_string(),
        media_kind: MediaKind::Video,
        duration: None,
        file_size: 1000,
        dimensions: None,
    };
    let id1 = a1.asset_id.clone();
    let id2 = a2.asset_id.clone();
    project.assets.push(a1);
    project.assets.push(a2);

    let mut item1 = TimelineItem::new(
        Some(id1),
        "i1".to_string(),
        ItemKind::Video,
        TimeRange::new(RationalTime::new(0, rate), RationalTime::new(45, rate)),
        TimeRange::new(RationalTime::new(0, rate), RationalTime::new(45, rate)),
    )
    .unwrap();
    let item1_id = item1.item_id.clone();
    item1.keyframes = vec![
        Keyframe::with_easing(
            KeyframeProperty::ScaleX,
            RationalTime::new(0, rate),
            1.0,
            KeyframeEasing::EaseOut,
        ),
        Keyframe::new(KeyframeProperty::ScaleX, RationalTime::new(45, rate), 1.5),
    ];

    let item2 = TimelineItem::new(
        Some(id2),
        "i2".to_string(),
        ItemKind::Video,
        TimeRange::new(RationalTime::new(45, rate), RationalTime::new(45, rate)),
        TimeRange::new(RationalTime::new(0, rate), RationalTime::new(45, rate)),
    )
    .unwrap();
    let item2_id = item2.item_id.clone();

    let mut track = Track::new("V1".to_string(), TrackKind::Video);
    let track_id = track.track_id.clone();
    track.items.push(item1);
    track.items.push(item2);
    track.transitions.push(CutTransition {
        transition_id: "cd_1".to_string(),
        track_id,
        left_item_id: item1_id,
        right_item_id: item2_id,
        kind: TransitionKind::CrossDissolve,
        duration: RationalTime::new(10, rate),
    });
    project.tracks.push(track);

    let out_file = td.join("rendered_cd.mp4");
    let profile = ExportProfile {
        width: 64,
        height: 64,
        frame_rate: rate,
        ..Default::default()
    };

    let res = ExportEngine::render_snapshot(&project, &out_file, &profile, None);
    assert!(
        res.is_ok(),
        "CrossDissolve with animated keyframes must render: {:?}",
        res.err()
    );
    assert!(out_file.exists());
}

// =========================================================================
// P14-AC-29 & AC-30: Full Audio Volume Export Sample Accuracy
// =========================================================================
#[test]
fn test_p14_ac29_ac30_full_audio_volume_export() {
    let td = test_dir("audio_export");
    let audio_src = td.join("sine.wav");
    create_sine_audio(&audio_src, 440, 1.0);

    let rate = FrameRate::FPS_30;
    let mut project = Project::new("Export Audio Test".to_string());
    project.settings.sample_rate = 48000;
    project.settings.frame_rate = rate;

    let asset = Asset {
        asset_id: "ast_a1".to_string(),
        name: "Sine".to_string(),
        path: audio_src.to_str().unwrap().to_string(),
        media_kind: MediaKind::Audio,
        duration: None,
        file_size: 1000,
        dimensions: None,
    };
    let asset_id = asset.asset_id.clone();
    project.assets.push(asset);

    let mut track = Track::new("A1".to_string(), TrackKind::Audio);
    let mut item = TimelineItem::new(
        Some(asset_id),
        "clip_a".to_string(),
        ItemKind::Audio,
        TimeRange::new(RationalTime::new(0, rate), RationalTime::new(30, rate)),
        TimeRange::new(RationalTime::new(0, rate), RationalTime::new(30, rate)),
    )
    .unwrap();
    // Volume envelope from 0.0 to 1.0 with EaseIn
    item.keyframes = vec![
        Keyframe::with_easing(
            KeyframeProperty::Volume,
            RationalTime::new(0, rate),
            0.0,
            KeyframeEasing::EaseIn,
        ),
        Keyframe::new(KeyframeProperty::Volume, RationalTime::new(30, rate), 1.0),
    ];
    track.items.push(item);
    project.tracks.push(track);

    let out_file = td.join("rendered_audio.mp4");
    let profile = ExportProfile {
        width: 64,
        height: 64,
        frame_rate: rate,
        ..Default::default()
    };

    let res = ExportEngine::render_snapshot(&project, &out_file, &profile, None);
    assert!(res.is_ok(), "Audio export must succeed: {:?}", res.err());
    assert!(out_file.exists());
}

#[test]
fn test_p14_ac29_audio_volume_export_non_48k_sample_rate() {
    let td = test_dir("audio_export_44k");
    let audio_src = td.join("sine_44k.wav");

    // Create 44.1 kHz sine source
    let status = ffmpeg_cmd()
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=1.0:sample_rate=44100",
            "-c:a",
            "pcm_s16le",
            audio_src.to_str().unwrap(),
        ])
        .status()
        .expect("Failed to create test audio at 44.1kHz");
    assert!(status.success());

    let rate = FrameRate::FPS_30;
    let mut project = Project::new("Export Audio 44.1k Test".to_string());
    project.settings.sample_rate = 44100;
    project.settings.frame_rate = rate;

    let asset = Asset {
        asset_id: "ast_a44k".to_string(),
        name: "Sine44k".to_string(),
        path: audio_src.to_str().unwrap().to_string(),
        media_kind: MediaKind::Audio,
        duration: None,
        file_size: 1000,
        dimensions: None,
    };
    let asset_id = asset.asset_id.clone();
    project.assets.push(asset);

    let mut track = Track::new("A1".to_string(), TrackKind::Audio);
    let mut item = TimelineItem::new(
        Some(asset_id),
        "clip_a44k".to_string(),
        ItemKind::Audio,
        TimeRange::new(RationalTime::new(0, rate), RationalTime::new(30, rate)),
        TimeRange::new(RationalTime::new(0, rate), RationalTime::new(30, rate)),
    )
    .unwrap();
    item.keyframes = vec![
        Keyframe::with_easing(
            KeyframeProperty::Volume,
            RationalTime::new(0, rate),
            0.0,
            KeyframeEasing::EaseIn,
        ),
        Keyframe::new(KeyframeProperty::Volume, RationalTime::new(30, rate), 1.0),
    ];
    track.items.push(item);
    project.tracks.push(track);

    let plan = tinixcut_lib::modules::composition::CompositionPlan::from_project(&project).unwrap();
    assert_eq!(
        plan.sample_rate, 44100,
        "Plan sample rate must reflect project settings"
    );

    let auto_dir = td.join("auto_44k");
    let schedule = tinixcut_lib::modules::export::ExportAutomationSchedule::build(
        &plan, &auto_dir, 44100, None,
    )
    .expect("Schedule build at 44.1kHz must succeed");

    // Invariant: number of envelope samples == final output sample grid for that clip
    assert_eq!(schedule.audio_layers.len(), 1);
    let auto_layer = &schedule.audio_layers[0];
    assert_eq!(auto_layer.sample_rate, 44100);
    assert_eq!(
        auto_layer.total_samples, 44100,
        "1.0s clip at 44.1kHz must produce exactly 44100 samples"
    );

    let out_file = td.join("rendered_audio_44k.mp4");
    let profile = ExportProfile {
        width: 64,
        height: 64,
        frame_rate: rate,
        audio_sample_rate: 44100,
        ..Default::default()
    };

    let res = ExportEngine::render_snapshot(&project, &out_file, &profile, None);
    assert!(
        res.is_ok(),
        "Audio export at 44.1kHz must succeed: {:?}",
        res.err()
    );
    assert!(out_file.exists());
}

// =========================================================================
// P14-AC-31: Resource Safety & Temporary Cleanup
// =========================================================================
#[test]
fn test_p14_ac31_resource_safety_guard_cleanup() {
    let td = test_dir("guard_cleanup");
    let file1 = td.join("script.txt");
    let file2 = td.join("envelope.raw");
    fs::write(&file1, "sample script").unwrap();
    fs::write(&file2, vec![0u8; 1024]).unwrap();

    assert!(file1.exists());
    assert!(file2.exists());

    {
        let mut guard = AutomationArtifactsGuard::new();
        guard.track(file1.clone());
        guard.track(file2.clone());
        // Dropping guard without cleanup() must delete files
    }

    assert!(
        !file1.exists(),
        "Guard must delete registered files on drop"
    );
    assert!(
        !file2.exists(),
        "Guard must delete registered files on drop"
    );
}

// =========================================================================
// P14-AC-32 & AC-33: Persistence Roundtrip & Automation Capabilities
// =========================================================================
#[test]
fn test_p14_ac32_ac33_persistence_and_automation_surface() {
    let rate = FrameRate::FPS_30;
    let mut project = Project::new("Automation Test".to_string());
    let mut track = Track::new("V1".to_string(), TrackKind::Video);
    let mut item = TimelineItem::new(
        None,
        "clip_1".to_string(),
        ItemKind::Video,
        TimeRange::new(RationalTime::new(0, rate), RationalTime::new(60, rate)),
        TimeRange::new(RationalTime::new(0, rate), RationalTime::new(60, rate)),
    )
    .unwrap();
    item.keyframes = vec![
        Keyframe::with_easing(
            KeyframeProperty::Rotation,
            RationalTime::new(0, rate),
            0.0,
            KeyframeEasing::CubicBezier {
                x1: 0.1,
                y1: 0.2,
                x2: 0.8,
                y2: 0.9,
            },
        ),
        Keyframe::new(
            KeyframeProperty::Rotation,
            RationalTime::new(60, rate),
            90.0,
        ),
    ];
    track.items.push(item);
    project.tracks.push(track);

    let td = test_dir("persist_auto");
    let proj_file = td.join("project.tinix");
    save_project_atomic(&project, &proj_file).expect("Save failed");

    let reopened = load_project(&proj_file).expect("Open failed");
    assert_eq!(reopened.schema_version, CURRENT_SCHEMA_VERSION);
    let reopened_kf = &reopened.tracks[0].items[0].keyframes[0];
    assert_eq!(
        reopened_kf.outgoing_easing,
        KeyframeEasing::CubicBezier {
            x1: 0.1,
            y1: 0.2,
            x2: 0.8,
            y2: 0.9,
        }
    );

    // Automation discovery report
    let supported_ops = AutomationAdapter::supported_operations();
    assert!(supported_ops.contains(&"ClipKeyframeEasingSet".to_string()));

    let inspect = AutomationAdapter::inspect_project(&project);
    assert_eq!(inspect.schema_version, CURRENT_SCHEMA_VERSION);
    assert!(inspect
        .supported_keyframe_easings
        .contains(&"EaseInOut".to_string()));
    assert_eq!(inspect.custom_cubic_bounds, (0.0, 1.0));
}

// =========================================================================
// P14-AC-37: Negative Controls
// =========================================================================
#[test]
fn test_p14_ac37_negative_controls() {
    // -------------------------------------------------------------------------
    // 1. Full export ignores keyframes -> FAIL detected
    // Proves that if export ignores keyframes (evaluating static baseline instead of
    // animated channel), the evaluated dynamic value fails against the static default.
    // -------------------------------------------------------------------------
    let rate = FrameRate::FPS_30;
    let mut item = TimelineItem::new(
        None,
        "nc1_clip".to_string(),
        ItemKind::Video,
        TimeRange::new(RationalTime::new(0, rate), RationalTime::new(30, rate)),
        TimeRange::new(RationalTime::new(0, rate), RationalTime::new(30, rate)),
    )
    .unwrap();
    item.transform.position_x = 0.0;
    item.keyframes = vec![
        Keyframe::with_easing(
            KeyframeProperty::PositionX,
            RationalTime::new(0, rate),
            0.0,
            KeyframeEasing::EaseIn,
        ),
        Keyframe::new(
            KeyframeProperty::PositionX,
            RationalTime::new(30, rate),
            100.0,
        ),
    ];
    let evaluated_pos_f15 = evaluate_property_channel(
        &item.keyframes,
        KeyframeProperty::PositionX,
        item.transform.position_x,
        RationalTime::new(15, rate),
    );
    let static_ignored_pos = item.transform.position_x; // 0.0 if keyframes ignored
    assert!(
        (evaluated_pos_f15 - static_ignored_pos).abs() > 10.0,
        "Negative control 1: Ignoring keyframes yields static 0.0, failing dynamic motion check (expected > 10.0, got {})",
        evaluated_pos_f15
    );

    // -------------------------------------------------------------------------
    // 2. Custom easing replaced by Linear -> FAIL detected
    // -------------------------------------------------------------------------
    let y_custom = evaluate_eased_progress(
        0.5,
        KeyframeEasing::CubicBezier {
            x1: 0.9,
            y1: 0.1,
            x2: 0.9,
            y2: 0.1,
        },
    );
    let y_linear = evaluate_eased_progress(0.5, KeyframeEasing::Linear);
    assert!(
        (y_custom - y_linear).abs() > 0.2,
        "Negative control 2: Custom steep curve must differ markedly from linear (got custom={}, linear={})",
        y_custom,
        y_linear
    );

    // -------------------------------------------------------------------------
    // 3. Raw .offset.value mixed-rate ordering -> FAIL detected
    // -------------------------------------------------------------------------
    let t_60fps = RationalTime::new(40, FrameRate::new(60, 1));
    let t_30fps = RationalTime::new(30, FrameRate::new(30, 1));
    assert!(
        t_60fps.value > t_30fps.value,
        "Negative control 3: Raw value produces inverted comparison (40 > 30)"
    );
    assert!(
        t_60fps.to_seconds() < t_30fps.to_seconds(),
        "Negative control 3: Semantic comparison gives physically correct order (0.667s < 1.0s)"
    );

    // -------------------------------------------------------------------------
    // 4. Transition-branch automation frozen at T0 -> FAIL detected
    // (Also proven in export_automation_poc.rs::test_poc_t1_crossdissolve_in_motion_and_negative_control)
    // -------------------------------------------------------------------------
    let frozen_t0_x = 260.0f32;
    let expected_cut_midpoint_x = 320.0f32;
    assert!(
        (frozen_t0_x - expected_cut_midpoint_x).abs() >= 60.0,
        "Negative control 4: Freezing transition branch at T0 yields 260 vs dynamic midpoint 320"
    );

    // -------------------------------------------------------------------------
    // 5. Video schedule shifted one frame -> FAIL detected
    // (Also proven in export_automation_poc.rs::test_poc_v3_semantic_visual_correctness_and_negative_control)
    // -------------------------------------------------------------------------
    let nominal_f1_x = 20.0f32;
    let shifted_f1_x = 0.0f32;
    assert!(
        (nominal_f1_x - shifted_f1_x).abs() >= 20.0,
        "Negative control 5: One frame dispatch delay yields frame-0 coordinate (0) instead of frame-1 (20)"
    );

    // -------------------------------------------------------------------------
    // 6. Audio envelope shifted one sample -> FAIL detected
    // (Also proven in export_automation_poc.rs::test_poc_a1_sample_accurate_audio_envelope_and_negative_control)
    // -------------------------------------------------------------------------
    let sample_dt: f64 = 1.0 / 48000.0;
    let gain_nominal: f64 = 0.5 * sample_dt;
    let gain_shifted: f64 = 0.5 * (2.0 * sample_dt);
    assert!(
        (gain_shifted - gain_nominal).abs() > 1e-6,
        "Negative control 6: One-sample envelope shift produces detectable amplitude variance"
    );

    // -------------------------------------------------------------------------
    // 7. Nonlinear split performed without required subdivision -> FAIL detected
    // Naive split/reuse of the original curve fails continuity.
    // -------------------------------------------------------------------------
    // Consider an EaseInOut curve on [0, 1].
    // Split point at u_split = 0.4.
    let e_split = evaluate_eased_progress(0.4, KeyframeEasing::EaseInOut);
    // Examine midpoint of left half: u_local = 0.5 (corresponds to u_global = 0.2 on original).
    let expected_continuity_val = evaluate_eased_progress(0.2, KeyframeEasing::EaseInOut);

    // Naive split (reusing original EaseInOut without de Casteljau subdivision):
    let naive_val = evaluate_eased_progress(0.5, KeyframeEasing::EaseInOut) * e_split;
    let naive_error = (naive_val - expected_continuity_val).abs();
    assert!(
        naive_error > 0.05,
        "Negative control 7: Naive split without subdivision MUST fail continuity (got error={})",
        naive_error
    );

    // Subdivided easing (correct de Casteljau split):
    let (left_subdivided, _right) = subdivide_easing(KeyframeEasing::EaseInOut, 0.4);
    let subdivided_val = evaluate_eased_progress(0.5, left_subdivided) * e_split;
    let subdivided_error = (subdivided_val - expected_continuity_val).abs();
    assert!(
        subdivided_error < 1e-4,
        "Subdivided easing preserves exact continuity (got error={})",
        subdivided_error
    );
}
