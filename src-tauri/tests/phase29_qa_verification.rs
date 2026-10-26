use std::fs;
use std::path::PathBuf;
use tinixcut_lib::modules::automation::AutomationAdapter;
use tinixcut_lib::modules::command::{
    CommandEnvelope, CommandError, CommandOperation, CommandService, OperationEnvelope,
};
use tinixcut_lib::modules::composition::CompositionPlan;
use tinixcut_lib::modules::domain::text::*;
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    AnimationTransform, AnimationUnit, DomainError, DurationConflictPolicy, FontReference,
    ItemKind, Keyframe, KeyframeEasing, KeyframeProperty, LoopMode, Project, RichTextSpan,
    StaggerOrder, StrokeAlign, TextAlignment, TextAnimation, TextAnimationSegment,
    TextBackgroundBox, TextFill, TextLayout, TextRichStyle, TextShadow, TextSpanStyle, TextStroke,
    TimelineItem, Track, TrackKind, CURRENT_SCHEMA_VERSION,
};
use tinixcut_lib::modules::history::{HistoryEntry, HistoryManager};
use tinixcut_lib::modules::persistence::{
    load_project, run_migrations_if_needed, save_project_atomic, PersistenceError,
};
use unicode_normalization::UnicodeNormalization;

fn qa_test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tinixcut_phase29_qa_{name}"));
    if dir.exists() {
        let _ = fs::remove_dir_all(&dir);
    }
    fs::create_dir_all(&dir).expect("create qa test dir");
    dir
}

fn create_qa_project(rate: FrameRate) -> (Project, String, String) {
    let mut project = Project::new("QA Verification Project".to_string());

    let mut video_track = Track::new("Video Track".to_string(), TrackKind::Video);
    let vid_item = TimelineItem::new(
        None,
        "QA Video".to_string(),
        ItemKind::Video,
        TimeRange::new(
            RationalTime::zero(rate),
            RationalTime::from_frames(100, rate),
        ),
        TimeRange::new(
            RationalTime::zero(rate),
            RationalTime::from_frames(100, rate),
        ),
    )
    .expect("create video item");
    let vid_id = vid_item.item_id.clone();
    video_track.items.push(vid_item);
    project.tracks.push(video_track);

    let mut text_track = Track::new("Text Track".to_string(), TrackKind::Video);
    let mut text_item = TimelineItem::new(
        None,
        "QA Text".to_string(),
        ItemKind::Text,
        TimeRange::new(
            RationalTime::zero(rate),
            RationalTime::from_frames(100, rate),
        ),
        TimeRange::new(
            RationalTime::zero(rate),
            RationalTime::from_frames(100, rate),
        ),
    )
    .expect("create text item");
    text_item.text_content = Some("Initial QA Text".to_string());
    let text_id = text_item.item_id.clone();
    text_track.items.push(text_item);
    project.tracks.push(text_track);

    (project, text_id, vid_id)
}

#[test]
fn test_qa_ac01_grapheme_limits_and_unicode_indexing() {
    // 512 graphemes limit
    let text_512 = "a".repeat(512);
    let rich = TextRichStyle::default();
    assert!(rich.validate(&text_512).is_ok());

    let text_513 = "a".repeat(513);
    let err = rich.validate(&text_513).unwrap_err();
    match err {
        DomainError::TextTooLong { max, found } => {
            assert_eq!(max, 512);
            assert_eq!(found, 513);
        }
        other => panic!("Expected TextTooLong, got {:?}", other),
    }

    // Vietnamese NFC vs NFD
    let vn_str = "Tiếng Việt — Xin chào thế giới!";
    let vn_nfc: String = vn_str.nfc().collect();
    let vn_nfd: String = vn_str.nfd().collect();
    assert_ne!(vn_nfc.len(), vn_nfd.len());
    assert_eq!(grapheme_count(&vn_nfc), grapheme_count(&vn_nfd));
    assert_eq!(grapheme_count(&vn_nfc), 31);

    // ZWJ Emoji & Modifiers
    let zwj_family = "👨‍👩‍👧‍👦";
    assert_eq!(grapheme_count(zwj_family), 1);
    let flag = "🇻🇳";
    assert_eq!(grapheme_count(flag), 1);
    let hand_tone = "👋🏽";
    assert_eq!(grapheme_count(hand_tone), 1);
}

#[test]
fn test_qa_ac01_limits_and_validation_edge_cases() {
    let text = "Hello World";

    // Stroke limit: 4 allowed, 5 rejected
    let mut rich = TextRichStyle::default();
    for _ in 0..4 {
        rich.strokes.push(TextStroke {
            color: "#FFFFFF".to_string(),
            width: 1.0,
            align: StrokeAlign::Center,
        });
    }
    assert!(rich.validate(text).is_ok());
    rich.strokes.push(TextStroke {
        color: "#FFFFFF".to_string(),
        width: 1.0,
        align: StrokeAlign::Center,
    });
    assert!(rich.validate(text).is_err());
    rich.strokes.pop();

    // Shadow limit: 4 allowed, 5 rejected
    for _ in 0..4 {
        rich.shadows.push(TextShadow {
            color: "#000000".to_string(),
            offset_x: 1.0,
            offset_y: 1.0,
            blur: 2.0,
        });
    }
    assert!(rich.validate(text).is_ok());
    rich.shadows.push(TextShadow {
        color: "#000000".to_string(),
        offset_x: 1.0,
        offset_y: 1.0,
        blur: 2.0,
    });
    assert!(rich.validate(text).is_err());
    rich.shadows.pop();

    // Gradient stops: 8 allowed, 9 rejected
    let mut stops_8 = Vec::new();
    for i in 1..=8 {
        stops_8.push(GradientStop {
            offset: i as f32 / 8.0,
            color: "#FFFFFF".to_string(),
        });
    }
    let grad_fill = TextFill::Gradient {
        gradient_kind: GradientKind::Linear,
        angle_deg: 90.0,
        radius: None,
        space: GradientSpace::All,
        stops: stops_8,
    };
    rich.fill = grad_fill;
    assert!(rich.validate(text).is_ok());

    let mut stops_9 = Vec::new();
    for i in 1..=9 {
        stops_9.push(GradientStop {
            offset: i as f32 / 9.0,
            color: "#FFFFFF".to_string(),
        });
    }
    rich.fill = TextFill::Gradient {
        gradient_kind: GradientKind::Linear,
        angle_deg: 90.0,
        radius: None,
        space: GradientSpace::All,
        stops: stops_9,
    };
    assert!(rich.validate(text).is_err());

    // Gradient stops must be strictly increasing and end at 1.0
    rich.fill = TextFill::Gradient {
        gradient_kind: GradientKind::Linear,
        angle_deg: 0.0,
        radius: None,
        space: GradientSpace::All,
        stops: vec![
            GradientStop {
                offset: 0.5,
                color: "#111111".to_string(),
            },
            GradientStop {
                offset: 0.5, // non-increasing!
                color: "#222222".to_string(),
            },
            GradientStop {
                offset: 1.0,
                color: "#333333".to_string(),
            },
        ],
    };
    assert!(rich.validate(text).is_err());

    rich.fill = TextFill::Gradient {
        gradient_kind: GradientKind::Linear,
        angle_deg: 0.0,
        radius: None,
        space: GradientSpace::All,
        stops: vec![
            GradientStop {
                offset: 0.2,
                color: "#111111".to_string(),
            },
            GradientStop {
                offset: 0.8, // last stop not 1.0!
                color: "#222222".to_string(),
            },
        ],
    };
    assert!(rich.validate(text).is_err());
    rich.fill = TextFill::default();

    // Spans validation edge cases:
    // [0, 0] rejected (start >= end)
    rich.spans = vec![RichTextSpan {
        start: 0,
        end: 0,
        style: TextSpanStyle::default(),
    }];
    assert!(rich.validate(text).is_err());

    // [5, 2] rejected (start >= end)
    rich.spans = vec![RichTextSpan {
        start: 5,
        end: 2,
        style: TextSpanStyle::default(),
    }];
    assert!(rich.validate(text).is_err());

    // Duplicate spans: [0, 2] and [0, 2] rejected
    rich.spans = vec![
        RichTextSpan {
            start: 0,
            end: 2,
            style: TextSpanStyle::default(),
        },
        RichTextSpan {
            start: 0,
            end: 2,
            style: TextSpanStyle::default(),
        },
    ];
    assert!(rich.validate(text).is_err());

    // Overlapping spans: [0, 4] and [3, 6] rejected
    rich.spans = vec![
        RichTextSpan {
            start: 0,
            end: 4,
            style: TextSpanStyle::default(),
        },
        RichTextSpan {
            start: 3,
            end: 6,
            style: TextSpanStyle::default(),
        },
    ];
    assert!(rich.validate(text).is_err());

    // Unsorted spans: [4, 6] and [0, 2] rejected
    rich.spans = vec![
        RichTextSpan {
            start: 4,
            end: 6,
            style: TextSpanStyle::default(),
        },
        RichTextSpan {
            start: 0,
            end: 2,
            style: TextSpanStyle::default(),
        },
    ];
    assert!(rich.validate(text).is_err());

    // Exactly at boundary 512 for a 512-grapheme text
    let text_512 = "x".repeat(512);
    rich.spans = vec![RichTextSpan {
        start: 0,
        end: 512,
        style: TextSpanStyle::default(),
    }];
    assert!(rich.validate(&text_512).is_ok());

    rich.spans = vec![RichTextSpan {
        start: 511,
        end: 512,
        style: TextSpanStyle::default(),
    }];
    assert!(rich.validate(&text_512).is_ok());

    rich.spans = vec![RichTextSpan {
        start: 512,
        end: 513,
        style: TextSpanStyle::default(),
    }];
    assert!(rich.validate(&text_512).is_err());

    // Random stagger order without seed is rejected
    let mut anim = TextAnimation {
        unit: AnimationUnit::Grapheme,
        order: StaggerOrder::Random,
        seed: None,
        ..Default::default()
    };
    let clip_dur = RationalTime::from_frames(100, FrameRate::FPS_30);
    assert!(anim.validate_for_clip_duration(clip_dur).is_err());

    anim.seed = Some(42);
    assert!(anim.validate_for_clip_duration(clip_dur).is_ok());

    // Duration conflict exact equality: in + out == clip under Reject is accepted
    anim.in_anim = Some(TextAnimationSegment {
        duration: RationalTime::from_frames(50, FrameRate::FPS_30),
        easing: KeyframeEasing::Linear,
        stagger_ratio: 0.1,
        from: AnimationTransform::default(),
        to: AnimationTransform::default(),
    });
    anim.out_anim = Some(TextAnimationSegment {
        duration: RationalTime::from_frames(50, FrameRate::FPS_30),
        easing: KeyframeEasing::Linear,
        stagger_ratio: 0.1,
        from: AnimationTransform::default(),
        to: AnimationTransform::default(),
    });
    anim.conflict_policy = DurationConflictPolicy::Reject;
    assert!(
        anim.validate_for_clip_duration(clip_dur).is_ok(),
        "Exact equality in+out == clip must be accepted"
    );

    // in + out > clip (50 + 51 = 101 > 100) under Reject is rejected
    anim.out_anim.as_mut().unwrap().duration = RationalTime::from_frames(51, FrameRate::FPS_30);
    assert!(anim.validate_for_clip_duration(clip_dur).is_err());

    // under ProportionalScale and ClampOut it is accepted
    anim.conflict_policy = DurationConflictPolicy::ProportionalScale;
    assert!(anim.validate_for_clip_duration(clip_dur).is_ok());

    anim.conflict_policy = DurationConflictPolicy::ClampOut;
    assert!(anim.validate_for_clip_duration(clip_dur).is_ok());
}

#[test]
fn test_qa_ac02_and_ac03_migration_and_atomic_save_roundtrip() {
    let rate = FrameRate::FPS_30;
    let (mut project, text_id, _) = create_qa_project(rate);
    let dir = qa_test_dir("save_recovery");
    let project_path = dir.join("qa_project.json");

    // Add keyframe to text item
    let text_item = project.tracks[1].find_item_mut(&text_id).unwrap();
    text_item.keyframes.push(Keyframe::new(
        KeyframeProperty::Opacity,
        RationalTime::zero(rate),
        0.75,
    ));

    // Simulate v9 JSON
    let mut v9_val = serde_json::to_value(&project).expect("serialize");
    v9_val["schema_version"] = serde_json::json!(9);
    let v9_raw = serde_json::to_string(&v9_val).expect("to string");

    // Migrate v9 -> current (v11)
    let migrated_str = run_migrations_if_needed(&v9_raw).expect("migrate v9 to v11");
    let migrated_proj: Project = serde_json::from_str(&migrated_str).expect("deserialize v11");
    assert_eq!(migrated_proj.schema_version, 11);
    let migrated_item = migrated_proj.tracks[1].find_item(&text_id).unwrap();
    assert!(migrated_item.text_rich.is_none());
    assert!(migrated_item.text_animation.is_none());
    assert_eq!(migrated_item.keyframes.len(), 1);

    // Schema 12 (future) rejected
    let schema_12_json = r#"{"schema_version": 12, "project_id": "proj_fut", "name": "Future"}"#;
    let err = run_migrations_if_needed(schema_12_json).unwrap_err();
    assert!(matches!(
        err,
        PersistenceError::UnsupportedSchemaVersion {
            found: 12,
            max_supported: 11,
        }
    ));

    // AC-03: Atomic save and recovery untouched
    save_project_atomic(&migrated_proj, &project_path).expect("save atomic");
    let loaded = load_project(&project_path).expect("load atomic project");
    assert_eq!(loaded.schema_version, CURRENT_SCHEMA_VERSION);
    assert_eq!(loaded.tracks.len(), migrated_proj.tracks.len());
    let loaded_item = loaded.tracks[1].find_item(&text_id).unwrap();
    assert_eq!(loaded_item.keyframes.len(), 1);
}

#[test]
fn test_qa_ac04_all_seven_commands_and_transaction_rollback() {
    let rate = FrameRate::FPS_30;
    let (project, text_id, _) = create_qa_project(rate);
    let mut service = CommandService::new(project.clone());
    let mut history = HistoryManager::new(50);

    // 1. ClipTextRichSet
    let rich_1 = TextRichStyle {
        fill: TextFill::Solid {
            color: "#112233".to_string(),
        },
        font_size: Some(28.0),
        ..Default::default()
    };
    let pre_snap = service.project().clone();
    let env_1 = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: service.revision(),
        client_id: None,
        client_epoch: None,
        sequence_number: None,
        dry_run: false,
        transaction_id: Some("tx_1".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op_rich_set".to_string(),
            operation: CommandOperation::ClipTextRichSet {
                item_id: text_id.clone(),
                rich: rich_1.clone(),
            },
        }],
    };
    service.execute(env_1).expect("exec op 1");
    history.record_edit(HistoryEntry {
        description: "Op 1".to_string(),
        transaction_id: Some("tx_1".to_string()),
        prior_snapshot: pre_snap,
        resulting_snapshot: service.project().clone(),
    });

    // 2. ClipTextSpansSet
    let spans_2 = vec![RichTextSpan {
        start: 0,
        end: 5,
        style: TextSpanStyle {
            font_size: Some(32.0),
            ..Default::default()
        },
    }];
    let pre_snap = service.project().clone();
    let env_2 = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: service.revision(),
        client_id: None,
        client_epoch: None,
        sequence_number: None,
        dry_run: false,
        transaction_id: Some("tx_2".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op_spans_set".to_string(),
            operation: CommandOperation::ClipTextSpansSet {
                item_id: text_id.clone(),
                spans: spans_2.clone(),
            },
        }],
    };
    service.execute(env_2).expect("exec op 2");
    history.record_edit(HistoryEntry {
        description: "Op 2".to_string(),
        transaction_id: Some("tx_2".to_string()),
        prior_snapshot: pre_snap,
        resulting_snapshot: service.project().clone(),
    });

    // 3. ClipTextLayoutSet
    let layout_3 = TextLayout {
        line_height: 1.5,
        alignment: TextAlignment::Right,
        ..Default::default()
    };
    let pre_snap = service.project().clone();
    let env_3 = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: service.revision(),
        client_id: None,
        client_epoch: None,
        sequence_number: None,
        dry_run: false,
        transaction_id: Some("tx_3".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op_layout_set".to_string(),
            operation: CommandOperation::ClipTextLayoutSet {
                item_id: text_id.clone(),
                layout: layout_3.clone(),
            },
        }],
    };
    service.execute(env_3).expect("exec op 3");
    history.record_edit(HistoryEntry {
        description: "Op 3".to_string(),
        transaction_id: Some("tx_3".to_string()),
        prior_snapshot: pre_snap,
        resulting_snapshot: service.project().clone(),
    });

    // 4. ClipTextAppearanceSet
    let pre_snap = service.project().clone();
    let env_4 = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: service.revision(),
        client_id: None,
        client_epoch: None,
        sequence_number: None,
        dry_run: false,
        transaction_id: Some("tx_4".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op_appearance_set".to_string(),
            operation: CommandOperation::ClipTextAppearanceSet {
                item_id: text_id.clone(),
                fill: TextFill::Solid {
                    color: "#AABBCC".to_string(),
                },
                strokes: vec![TextStroke {
                    color: "#FF0000".to_string(),
                    width: 2.0,
                    align: StrokeAlign::Outside,
                }],
                shadows: vec![TextShadow {
                    color: "#000000".to_string(),
                    offset_x: 1.0,
                    offset_y: 1.0,
                    blur: 3.0,
                }],
                background: Some(TextBackgroundBox {
                    fill: TextFill::Solid {
                        color: "#00FF00".to_string(),
                    },
                    padding_x: 8.0,
                    padding_y: 4.0,
                    corner_radius: 2.0,
                    opacity: 0.9,
                }),
            },
        }],
    };
    service.execute(env_4).expect("exec op 4");
    history.record_edit(HistoryEntry {
        description: "Op 4".to_string(),
        transaction_id: Some("tx_4".to_string()),
        prior_snapshot: pre_snap,
        resulting_snapshot: service.project().clone(),
    });

    // 5. ClipTextFontSet
    let pre_snap = service.project().clone();
    let env_5 = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: service.revision(),
        client_id: None,
        client_epoch: None,
        sequence_number: None,
        dry_run: false,
        transaction_id: Some("tx_5".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op_font_set".to_string(),
            operation: CommandOperation::ClipTextFontSet {
                item_id: text_id.clone(),
                font: FontReference {
                    family: "Roboto".to_string(),
                    weight: 700,
                    style: FontStyle::Italic,
                    asset_id: None,
                },
                font_size: Some(40.0),
            },
        }],
    };
    service.execute(env_5).expect("exec op 5");
    history.record_edit(HistoryEntry {
        description: "Op 5".to_string(),
        transaction_id: Some("tx_5".to_string()),
        prior_snapshot: pre_snap,
        resulting_snapshot: service.project().clone(),
    });

    // 6. ClipTextAnimationSet
    let pre_snap = service.project().clone();
    let env_6 = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: service.revision(),
        client_id: None,
        client_epoch: None,
        sequence_number: None,
        dry_run: false,
        transaction_id: Some("tx_6".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op_anim_set".to_string(),
            operation: CommandOperation::ClipTextAnimationSet {
                item_id: text_id.clone(),
                animation: Some(TextAnimation {
                    unit: AnimationUnit::Word,
                    order: StaggerOrder::Forward,
                    seed: None,
                    pivot_x: 0.5,
                    pivot_y: 0.5,
                    in_anim: Some(TextAnimationSegment {
                        duration: RationalTime::from_frames(20, rate),
                        easing: KeyframeEasing::EaseIn,
                        stagger_ratio: 0.2,
                        from: AnimationTransform::default(),
                        to: AnimationTransform::default(),
                    }),
                    loop_anim: None,
                    out_anim: None,
                    loop_mode: LoopMode::None,
                    conflict_policy: DurationConflictPolicy::Reject,
                    phase_origin: None,
                    phase_end: None,
                }),
            },
        }],
    };
    service.execute(env_6).expect("exec op 6");
    history.record_edit(HistoryEntry {
        description: "Op 6".to_string(),
        transaction_id: Some("tx_6".to_string()),
        prior_snapshot: pre_snap,
        resulting_snapshot: service.project().clone(),
    });

    // 7. ClipTextRichReset
    let pre_snap = service.project().clone();
    let env_7 = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: service.revision(),
        client_id: None,
        client_epoch: None,
        sequence_number: None,
        dry_run: false,
        transaction_id: Some("tx_7".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op_rich_reset".to_string(),
            operation: CommandOperation::ClipTextRichReset {
                item_id: text_id.clone(),
            },
        }],
    };
    service.execute(env_7).expect("exec op 7");
    history.record_edit(HistoryEntry {
        description: "Op 7".to_string(),
        transaction_id: Some("tx_7".to_string()),
        prior_snapshot: pre_snap,
        resulting_snapshot: service.project().clone(),
    });

    let post_reset_item = service.project().tracks[1].find_item(&text_id).unwrap();
    assert!(post_reset_item.text_rich.is_none());
    assert!(post_reset_item.text_animation.is_some()); // animation untouched by rich reset!

    // Undo all 7 operations
    let mut active = service.project().clone();
    for _ in 0..7 {
        history.undo(&mut active).expect("undo");
    }
    service.set_project(active.clone());
    let orig_item = service.project().tracks[1].find_item(&text_id).unwrap();
    assert!(orig_item.text_rich.is_none());
    assert!(orig_item.text_animation.is_none());

    // Redo all 7 operations
    for _ in 0..7 {
        history.redo(&mut active).expect("redo");
    }
    service.set_project(active.clone());
    let redone_item = service.project().tracks[1].find_item(&text_id).unwrap();
    assert!(redone_item.text_rich.is_none());
    assert!(redone_item.text_animation.is_some());

    // Transaction Rollback: op1 is valid, op2 has invalid span -> entire transaction rolls back
    let pre_rollback = service.project().clone();
    let rollback_env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: service.revision(),
        client_id: None,
        client_epoch: None,
        sequence_number: None,
        dry_run: false,
        transaction_id: Some("tx_rollback".to_string()),
        operations: vec![
            OperationEnvelope {
                operation_id: "op_valid_font".to_string(),
                operation: CommandOperation::ClipTextFontSet {
                    item_id: text_id.clone(),
                    font: FontReference {
                        family: "Inter".to_string(),
                        weight: 600,
                        style: FontStyle::Normal,
                        asset_id: None,
                    },
                    font_size: Some(55.0),
                },
            },
            OperationEnvelope {
                operation_id: "op_invalid_span".to_string(),
                operation: CommandOperation::ClipTextSpansSet {
                    item_id: text_id.clone(),
                    spans: vec![RichTextSpan {
                        start: 10,
                        end: 2, // invalid: start >= end!
                        style: TextSpanStyle::default(),
                    }],
                },
            },
        ],
    };
    let res = service.execute(rollback_env);
    assert!(res.is_err(), "Transaction with invalid op must fail");
    assert_eq!(
        service.project().revision,
        pre_rollback.revision,
        "Revision must remain unchanged after rollback"
    );
    let after_failed_item = service.project().tracks[1].find_item(&text_id).unwrap();
    assert_eq!(
        after_failed_item.text_rich,
        pre_rollback.tracks[1]
            .find_item(&text_id)
            .unwrap()
            .text_rich,
        "First op in failed transaction must not be committed"
    );
}

#[test]
fn test_qa_ac05_stale_revision_and_replay_deduplication() {
    let rate = FrameRate::FPS_30;
    let (project, text_id, _) = create_qa_project(rate);
    let dir = qa_test_dir("revision_replay");
    let file_path = dir.join("replay_test.json");
    let mut service = CommandService::new(project.clone());

    // Apply op 1
    let env_1 = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: 0,
        client_id: None,
        client_epoch: None,
        sequence_number: None,
        dry_run: false,
        transaction_id: Some("tx_rev_1".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op_rev_1".to_string(),
            operation: CommandOperation::ClipTextRichSet {
                item_id: text_id.clone(),
                rich: TextRichStyle::default(),
            },
        }],
    };
    let receipt = service.execute(env_1.clone()).expect("op 1 executes");
    assert_eq!(receipt.new_revision, 1);

    // Stale revision: sending base_revision: 0 when current revision is 1
    let stale_env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: 0,
        client_id: None,
        client_epoch: None,
        sequence_number: None,
        dry_run: false,
        transaction_id: Some("tx_stale".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op_stale_new".to_string(),
            operation: CommandOperation::ClipTextRichReset {
                item_id: text_id.clone(),
            },
        }],
    };
    let stale_res = service.execute(stale_env);
    assert!(
        matches!(stale_res, Err(CommandError::StaleRevision { .. })),
        "Expected StaleRevision on stale base_revision"
    );

    // Save and reopen
    save_project_atomic(service.project(), &file_path).expect("save project");
    let reloaded = load_project(&file_path).expect("reload project");
    let mut reopened_service = CommandService::new(reloaded);

    // Replay op_rev_1
    let replay_res = reopened_service
        .execute(env_1)
        .expect("replay executes as no-op");
    assert_eq!(replay_res.operations_applied, 0);
    assert_eq!(replay_res.receipts[0].status, "deduplicated_noop");
}

#[test]
fn test_qa_ac06_clip_text_set_rebinding_edge_cases() {
    let rate = FrameRate::FPS_30;
    let (project, text_id, _) = create_qa_project(rate);
    let mut service = CommandService::new(project.clone());

    // Setup initial text: "Hello" (5 graphemes)
    service
        .execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision: service.revision(),
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_init_text".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_set_hello".to_string(),
                operation: CommandOperation::ClipTextSet {
                    item_id: text_id.clone(),
                    text: "Hello".to_string(),
                },
            }],
        })
        .unwrap();

    // Set full span [0, 5]
    service
        .execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision: service.revision(),
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_spans_hello".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_set_span_0_5".to_string(),
                operation: CommandOperation::ClipTextSpansSet {
                    item_id: text_id.clone(),
                    spans: vec![RichTextSpan {
                        start: 0,
                        end: 5,
                        style: TextSpanStyle {
                            font_size: Some(20.0),
                            ..Default::default()
                        },
                    }],
                },
            }],
        })
        .unwrap();

    // 1. Full-span expansion: change text to "Hello World!" (12 graphemes)
    service
        .execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision: service.revision(),
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_expand".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_expand_text".to_string(),
                operation: CommandOperation::ClipTextSet {
                    item_id: text_id.clone(),
                    text: "Hello World!".to_string(),
                },
            }],
        })
        .unwrap();

    let item = service.project().tracks[1].find_item(&text_id).unwrap();
    let spans = &item.text_rich.as_ref().unwrap().spans;
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].start, 0);
    assert_eq!(spans[0].end, 12, "Full-span [0, 5) expands to [0, 12)");

    // 2. Clamp & prune: add non-full span [6, 12)
    service
        .execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision: service.revision(),
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_two_spans".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_set_two_spans".to_string(),
                operation: CommandOperation::ClipTextSpansSet {
                    item_id: text_id.clone(),
                    spans: vec![
                        RichTextSpan {
                            start: 0,
                            end: 5,
                            style: TextSpanStyle::default(),
                        },
                        RichTextSpan {
                            start: 6,
                            end: 12,
                            style: TextSpanStyle::default(),
                        },
                    ],
                },
            }],
        })
        .unwrap();

    // Now shorten text to "Hello" (5 graphemes): span [0, 5) stays [0, 5), [6, 12) clamped to [5, 5) and pruned!
    service
        .execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision: service.revision(),
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_shrink".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_shrink_text".to_string(),
                operation: CommandOperation::ClipTextSet {
                    item_id: text_id.clone(),
                    text: "Hello".to_string(),
                },
            }],
        })
        .unwrap();

    let item_shrunk = service.project().tracks[1].find_item(&text_id).unwrap();
    let spans_shrunk = &item_shrunk.text_rich.as_ref().unwrap().spans;
    assert_eq!(spans_shrunk.len(), 1);
    assert_eq!(spans_shrunk[0].start, 0);
    assert_eq!(spans_shrunk[0].end, 5);

    // 3. Rebind when new text is empty ""
    service
        .execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision: service.revision(),
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_empty".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_empty_text".to_string(),
                operation: CommandOperation::ClipTextSet {
                    item_id: text_id.clone(),
                    text: "".to_string(),
                },
            }],
        })
        .unwrap();

    let item_empty = service.project().tracks[1].find_item(&text_id).unwrap();
    assert_eq!(item_empty.text_content.as_deref(), Some(""));
    assert!(
        item_empty.text_rich.as_ref().unwrap().spans.is_empty(),
        "All spans pruned when text is empty"
    );
}

#[test]
fn test_qa_ac07_composition_and_json_serialization_spotcheck() {
    let rate = FrameRate::FPS_30;
    let (mut project, text_id, vid_id) = create_qa_project(rate);

    let rich = TextRichStyle {
        font: Some(FontReference {
            family: "Inter".to_string(),
            weight: 700,
            style: FontStyle::Normal,
            asset_id: None,
        }),
        font_size: Some(36.0),
        layout: TextLayout {
            alignment: TextAlignment::Center,
            line_height: 1.2,
            ..Default::default()
        },
        fill: TextFill::Gradient {
            gradient_kind: GradientKind::Linear,
            angle_deg: 45.0,
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
        strokes: vec![TextStroke {
            color: "#FFFFFF".to_string(),
            width: 2.0,
            align: StrokeAlign::Outside,
        }],
        shadows: vec![TextShadow {
            color: "#000000".to_string(),
            offset_x: 2.0,
            offset_y: 2.0,
            blur: 4.0,
        }],
        background: Some(TextBackgroundBox {
            fill: TextFill::Solid {
                color: "#111111".to_string(),
            },
            padding_x: 10.0,
            padding_y: 6.0,
            corner_radius: 4.0,
            opacity: 0.8,
        }),
        spans: vec![RichTextSpan {
            start: 0,
            end: 7,
            style: TextSpanStyle {
                fill: Some(TextFill::Solid {
                    color: "#00FF00".to_string(),
                }),
                font: None,
                font_size: Some(40.0),
                letter_spacing: Some(1.5),
            },
        }],
    };

    let anim = TextAnimation {
        unit: AnimationUnit::Grapheme,
        order: StaggerOrder::CenterOut,
        seed: None,
        pivot_x: 0.5,
        pivot_y: 0.5,
        in_anim: Some(TextAnimationSegment {
            duration: RationalTime::from_frames(15, rate),
            easing: KeyframeEasing::EaseInOut,
            stagger_ratio: 0.25,
            from: AnimationTransform {
                translate_x: 0.0,
                translate_y: 20.0,
                scale_x: 0.8,
                scale_y: 0.8,
                rotation: 0.0,
                opacity: 0.0,
            },
            to: AnimationTransform::default(),
        }),
        loop_anim: None,
        out_anim: None,
        loop_mode: LoopMode::None,
        conflict_policy: DurationConflictPolicy::Reject,
        phase_origin: None,
        phase_end: None,
    };

    {
        let text_item = project.tracks[1].find_item_mut(&text_id).unwrap();
        text_item.text_rich = Some(rich.clone());
        text_item.text_animation = Some(anim.clone());
    }

    // 1. Composition verification
    let plan = CompositionPlan::from_project(&project).expect("generate plan");
    assert_eq!(plan.visual_layers.len(), 2);

    let text_vl = plan
        .visual_layers
        .iter()
        .find(|vl| vl.item_id == text_id)
        .unwrap();
    assert_eq!(text_vl.text_rich.as_ref(), Some(&rich));
    assert_eq!(text_vl.text_animation.as_ref(), Some(&anim));

    let vid_vl = plan
        .visual_layers
        .iter()
        .find(|vl| vl.item_id == vid_id)
        .unwrap();
    assert!(vid_vl.text_rich.is_none());
    assert!(vid_vl.text_animation.is_none());

    let sampled = plan.sample_preview_at(RationalTime::from_frames(10, rate));
    let text_sample = sampled
        .active_layers
        .iter()
        .find(|l| l.item_id == text_id)
        .unwrap();
    assert_eq!(text_sample.text_rich.as_ref(), Some(&rich));
    assert_eq!(text_sample.text_animation.as_ref(), Some(&anim));

    let vid_sample = sampled
        .active_layers
        .iter()
        .find(|l| l.item_id == vid_id)
        .unwrap();
    assert!(vid_sample.text_rich.is_none());
    assert!(vid_sample.text_animation.is_none());

    // 2. JSON Serialization Spot-Check against TS types
    let text_item = project.tracks[1].find_item(&text_id).unwrap();
    let item_json = serde_json::to_value(text_item).expect("serialize text item");
    assert_eq!(item_json["kind"], "Text");
    assert!(item_json.get("text_rich").is_some());
    assert!(item_json.get("text_animation").is_some());

    // TextFill internally tagged with "kind"
    let rich_json = &item_json["text_rich"];
    assert_eq!(rich_json["fill"]["kind"], "Gradient");
    assert_eq!(rich_json["fill"]["gradient_kind"], "Linear");
    assert_eq!(rich_json["background"]["fill"]["kind"], "Solid");
    assert_eq!(rich_json["background"]["fill"]["color"], "#111111");

    // Span fill internally tagged
    let span_0_fill = &rich_json["spans"][0]["style"]["fill"];
    assert_eq!(span_0_fill["kind"], "Solid");
    assert_eq!(span_0_fill["color"], "#00FF00");

    // Animation fields match TS snake_case definitions
    let anim_json = &item_json["text_animation"];
    assert_eq!(anim_json["unit"], "Grapheme");
    assert_eq!(anim_json["order"], "CenterOut");
    assert_eq!(anim_json["in_anim"]["easing"], "EaseInOut");
    assert_eq!(anim_json["in_anim"]["from"]["translate_y"], 20.0);
}

#[test]
fn test_qa_invariants_trim_shortening_reject_under_reject_policy() {
    let rate = FrameRate::FPS_30;
    let (project, text_id, _) = create_qa_project(rate);
    let mut service = CommandService::new(project.clone());

    // Attach animation with in = 30 frames, out = 30 frames (total 60 frames)
    // Clip duration is 100 frames. DurationConflictPolicy::Reject.
    let anim = TextAnimation {
        unit: AnimationUnit::All,
        order: StaggerOrder::Forward,
        seed: None,
        pivot_x: 0.5,
        pivot_y: 0.5,
        in_anim: Some(TextAnimationSegment {
            duration: RationalTime::from_frames(30, rate),
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.0,
            from: AnimationTransform::default(),
            to: AnimationTransform::default(),
        }),
        loop_anim: None,
        out_anim: Some(TextAnimationSegment {
            duration: RationalTime::from_frames(30, rate),
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.0,
            from: AnimationTransform::default(),
            to: AnimationTransform::default(),
        }),
        loop_mode: LoopMode::None,
        conflict_policy: DurationConflictPolicy::Reject,
        phase_origin: None,
        phase_end: None,
    };

    service
        .execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision: service.revision(),
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_set_anim".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_set_anim".to_string(),
                operation: CommandOperation::ClipTextAnimationSet {
                    item_id: text_id.clone(),
                    animation: Some(anim),
                },
            }],
        })
        .unwrap();

    // Now attempt to trim the text item to 50 frames (which is < in + out = 60 frames)
    // Under DurationConflictPolicy::Reject, this must fail validation and roll back!
    let trim_env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: service.revision(),
        client_id: None,
        client_epoch: None,
        sequence_number: None,
        dry_run: false,
        transaction_id: Some("tx_trim_short".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op_trim".to_string(),
            operation: CommandOperation::ClipTrim {
                item_id: text_id.clone(),
                new_timeline_range: TimeRange::new(
                    RationalTime::zero(rate),
                    RationalTime::from_frames(50, rate),
                ),
                new_source_range: TimeRange::new(
                    RationalTime::zero(rate),
                    RationalTime::from_frames(50, rate),
                ),
            },
        }],
    };

    let trim_res = service.execute(trim_env);
    assert!(
        trim_res.is_err(),
        "Trimming clip shorter than animation in+out under Reject policy must fail"
    );

    // Verify item duration remained 100 frames
    let item = service.project().tracks[1].find_item(&text_id).unwrap();
    assert_eq!(item.timeline_range.duration.value, 100);
}

#[test]
fn test_qa_automation_discovery_and_non_text_rejections() {
    let rate = FrameRate::FPS_30;
    let (project, _, vid_id) = create_qa_project(rate);

    // Verify non-text item cannot receive rich text or animation directly
    let mut non_text_item = project.tracks[0].find_item(&vid_id).unwrap().clone();
    non_text_item.text_rich = Some(TextRichStyle::default());
    assert!(
        non_text_item.validate().is_err(),
        "TimelineItem::validate must reject text_rich on non-Text item"
    );

    non_text_item.text_rich = None;
    non_text_item.text_animation = Some(TextAnimation::default());
    assert!(
        non_text_item.validate().is_err(),
        "TimelineItem::validate must reject text_animation on non-Text item"
    );

    // Automation capability payload verification
    let report = AutomationAdapter::inspect_project(&project);
    assert_eq!(
        report.supported_animation_units,
        vec!["All", "Grapheme", "Word", "Line"]
    );
    assert_eq!(
        report.supported_stagger_orders,
        vec!["Forward", "Backward", "CenterOut", "Random"]
    );
    assert_eq!(
        report.supported_loop_modes,
        vec!["None", "Repeat", "PingPong"]
    );
    assert_eq!(
        report.supported_duration_conflict_policies,
        vec!["Reject", "ProportionalScale", "ClampOut"]
    );

    let ops = AutomationAdapter::supported_operations();
    for op in [
        "ClipTextRichSet",
        "ClipTextSpansSet",
        "ClipTextLayoutSet",
        "ClipTextAppearanceSet",
        "ClipTextFontSet",
        "ClipTextAnimationSet",
        "ClipTextRichReset",
    ] {
        assert!(ops.contains(&op.to_string()), "Must support operation {op}");
        assert!(
            report.supported_operations.contains(&op.to_string()),
            "Inspect report must list {op}"
        );
    }
}

#[test]
fn test_qa_bf01_capabilities_parity_and_mcp_schema() {
    use rmcp::schemars::{JsonSchema, SchemaGenerator};
    use tinixcut_lib::modules::automation::{
        AutomationAdapter, AutomationSession, CapabilitiesReport, ProjectInspectReport,
    };

    let session = AutomationSession::new();
    let session_caps = session.capabilities().expect("session capabilities");

    let rate = FrameRate::FPS_30;
    let (project, _, _) = create_qa_project(rate);
    let inspect_report = AutomationAdapter::inspect_project(&project);

    // 1. Direct/CLI parity check: CapabilitiesReport vs ProjectInspectReport vs static helpers
    assert_eq!(
        session_caps.supported_animation_units, inspect_report.supported_animation_units,
        "Animation units list must match between CapabilitiesReport and ProjectInspectReport"
    );
    assert_eq!(
        session_caps.supported_animation_units,
        AutomationAdapter::supported_animation_units()
    );
    assert!(!session_caps.supported_animation_units.is_empty());

    assert_eq!(
        session_caps.supported_stagger_orders, inspect_report.supported_stagger_orders,
        "Stagger orders list must match between CapabilitiesReport and ProjectInspectReport"
    );
    assert_eq!(
        session_caps.supported_stagger_orders,
        AutomationAdapter::supported_stagger_orders()
    );
    assert!(!session_caps.supported_stagger_orders.is_empty());

    assert_eq!(
        session_caps.supported_loop_modes, inspect_report.supported_loop_modes,
        "Loop modes list must match between CapabilitiesReport and ProjectInspectReport"
    );
    assert_eq!(
        session_caps.supported_loop_modes,
        AutomationAdapter::supported_loop_modes()
    );
    assert!(!session_caps.supported_loop_modes.is_empty());

    assert_eq!(
        session_caps.supported_duration_conflict_policies,
        inspect_report.supported_duration_conflict_policies,
        "Duration conflict policies list must match between CapabilitiesReport and ProjectInspectReport"
    );
    assert_eq!(
        session_caps.supported_duration_conflict_policies,
        AutomationAdapter::supported_duration_conflict_policies()
    );
    assert!(!session_caps.supported_duration_conflict_policies.is_empty());

    // 7 new operation names present in session_caps.supported_operations
    for op in [
        "ClipTextRichSet",
        "ClipTextSpansSet",
        "ClipTextLayoutSet",
        "ClipTextAppearanceSet",
        "ClipTextFontSet",
        "ClipTextAnimationSet",
        "ClipTextRichReset",
    ] {
        assert!(
            session_caps.supported_operations.contains(&op.to_string()),
            "CapabilitiesReport must include operation {op}"
        );
    }

    // 2. BF-01 MCP schema verification
    let mut gen = SchemaGenerator::default();
    let inspect_schema = ProjectInspectReport::json_schema(&mut gen);
    let inspect_json = serde_json::to_value(&inspect_schema).expect("serialize inspect schema");

    let required_props = [
        "supportedAnimationUnits",
        "supportedStaggerOrders",
        "supportedLoopModes",
        "supportedDurationConflictPolicies",
    ];

    for prop in &required_props {
        let prop_obj = &inspect_json["properties"][prop];
        assert_eq!(
            prop_obj["type"], "array",
            "ProjectInspectReport schema property '{}' must be an array, found: {:?}",
            prop, prop_obj
        );
        assert_eq!(
            prop_obj["items"]["type"], "string",
            "ProjectInspectReport schema property '{}' items must be string",
            prop
        );
    }

    let caps_schema = CapabilitiesReport::json_schema(&mut gen);
    let caps_json = serde_json::to_value(&caps_schema).expect("serialize caps schema");

    for prop in &required_props {
        let prop_obj = &caps_json["properties"][prop];
        assert_eq!(
            prop_obj["type"], "array",
            "CapabilitiesReport schema property '{}' must be an array, found: {:?}",
            prop, prop_obj
        );
        assert_eq!(
            prop_obj["items"]["type"], "string",
            "CapabilitiesReport schema property '{}' items must be string",
            prop
        );
    }
}

#[test]
fn test_qa_bf02_full_rich_and_animation_disk_roundtrip_and_command_serde() {
    let rate = FrameRate::FPS_30;
    let (mut project, text_id, _) = create_qa_project(rate);
    let dir = qa_test_dir("bf02_disk_roundtrip");
    let file_path = dir.join("bf02_full_rich.json");

    // Populated TextRichStyle with Gradient (>=2 stops ending at 1.0), strokes, shadows, background, 2 spans
    let rich = TextRichStyle {
        font: Some(FontReference {
            family: "Inter".to_string(),
            weight: 600,
            style: FontStyle::Oblique,
            asset_id: None,
        }),
        font_size: Some(42.0),
        layout: TextLayout {
            max_width: Some(800.0),
            alignment: TextAlignment::Center,
            line_height: 1.25,
            letter_spacing: 0.5,
            wrap_mode: TextWrapMode::Wrap,
            min_font_size: Some(18.0),
            max_font_size: Some(64.0),
        },
        fill: TextFill::Gradient {
            gradient_kind: GradientKind::Radial,
            angle_deg: 90.0,
            radius: Some(250.0),
            space: GradientSpace::Character,
            stops: vec![
                GradientStop {
                    offset: 0.0,
                    color: "#FF3366".to_string(),
                },
                GradientStop {
                    offset: 0.5,
                    color: "#33CCFF".to_string(),
                },
                GradientStop {
                    offset: 1.0,
                    color: "#00FF66".to_string(),
                },
            ],
        },
        strokes: vec![
            TextStroke {
                color: "#000000".to_string(),
                width: 3.0,
                align: StrokeAlign::Outside,
            },
            TextStroke {
                color: "#FFFFFF".to_string(),
                width: 1.5,
                align: StrokeAlign::Inside,
            },
        ],
        shadows: vec![
            TextShadow {
                color: "#000000AA".to_string(),
                offset_x: 3.0,
                offset_y: 3.0,
                blur: 5.0,
            },
            TextShadow {
                color: "#FF000055".to_string(),
                offset_x: -2.0,
                offset_y: -2.0,
                blur: 2.0,
            },
        ],
        background: Some(TextBackgroundBox {
            fill: TextFill::Solid {
                color: "#181818".to_string(),
            },
            padding_x: 16.0,
            padding_y: 8.0,
            corner_radius: 6.0,
            opacity: 0.95,
        }),
        spans: vec![
            RichTextSpan {
                start: 0,
                end: 7,
                style: TextSpanStyle {
                    fill: Some(TextFill::Solid {
                        color: "#FFAA00".to_string(),
                    }),
                    font: Some(FontReference {
                        family: "Inter".to_string(),
                        weight: 800,
                        style: FontStyle::Normal,
                        asset_id: None,
                    }),
                    font_size: Some(48.0),
                    letter_spacing: Some(1.0),
                },
            },
            RichTextSpan {
                start: 8,
                end: 15,
                style: TextSpanStyle {
                    fill: Some(TextFill::Solid {
                        color: "#00E5FF".to_string(),
                    }),
                    font: None,
                    font_size: Some(36.0),
                    letter_spacing: Some(0.0),
                },
            },
        ],
    };

    // Populated TextAnimation with in/loop/out segments
    let anim = TextAnimation {
        unit: AnimationUnit::Word,
        order: StaggerOrder::CenterOut,
        seed: None,
        pivot_x: 0.5,
        pivot_y: 0.5,
        in_anim: Some(TextAnimationSegment {
            duration: RationalTime::from_frames(20, rate),
            easing: KeyframeEasing::EaseInOut,
            stagger_ratio: 0.15,
            from: AnimationTransform {
                translate_x: 0.0,
                translate_y: 40.0,
                scale_x: 0.7,
                scale_y: 0.7,
                rotation: -10.0,
                opacity: 0.0,
            },
            to: AnimationTransform::default(),
        }),
        loop_anim: Some(TextAnimationSegment {
            duration: RationalTime::from_frames(30, rate),
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.0,
            from: AnimationTransform::default(),
            to: AnimationTransform {
                translate_x: 5.0,
                translate_y: 0.0,
                scale_x: 1.05,
                scale_y: 1.05,
                rotation: 0.0,
                opacity: 1.0,
            },
        }),
        out_anim: Some(TextAnimationSegment {
            duration: RationalTime::from_frames(20, rate),
            easing: KeyframeEasing::EaseIn,
            stagger_ratio: 0.1,
            from: AnimationTransform::default(),
            to: AnimationTransform {
                translate_x: 0.0,
                translate_y: -30.0,
                scale_x: 0.9,
                scale_y: 0.9,
                rotation: 5.0,
                opacity: 0.0,
            },
        }),
        loop_mode: LoopMode::PingPong,
        conflict_policy: DurationConflictPolicy::Reject,
        phase_origin: None,
        phase_end: None,
    };

    let text_item = project.tracks[1].find_item_mut(&text_id).unwrap();
    text_item.text_content = Some("Initial QA Text".to_string());
    text_item.text_rich = Some(rich.clone());
    text_item.text_animation = Some(anim.clone());

    // Save and load via persistence module
    save_project_atomic(&project, &file_path).expect("save project with full rich text");
    let reloaded = load_project(&file_path).expect("load project with full rich text");

    let reloaded_item = reloaded.tracks[1].find_item(&text_id).unwrap();
    assert_eq!(
        reloaded_item.text_rich.as_ref(),
        Some(&rich),
        "Reloaded TextRichStyle must match original exactly"
    );
    assert_eq!(
        reloaded_item.text_animation.as_ref(),
        Some(&anim),
        "Reloaded TextAnimation must match original exactly"
    );

    // CommandOperation JSON Serde round-trip for each of the 7 new variants
    let ops = vec![
        CommandOperation::ClipTextRichSet {
            item_id: text_id.clone(),
            rich: rich.clone(),
        },
        CommandOperation::ClipTextSpansSet {
            item_id: text_id.clone(),
            spans: rich.spans.clone(),
        },
        CommandOperation::ClipTextLayoutSet {
            item_id: text_id.clone(),
            layout: rich.layout.clone(),
        },
        CommandOperation::ClipTextAppearanceSet {
            item_id: text_id.clone(),
            fill: rich.fill.clone(),
            strokes: rich.strokes.clone(),
            shadows: rich.shadows.clone(),
            background: rich.background.clone(),
        },
        CommandOperation::ClipTextFontSet {
            item_id: text_id.clone(),
            font: rich.font.clone().unwrap(),
            font_size: rich.font_size,
        },
        CommandOperation::ClipTextAnimationSet {
            item_id: text_id.clone(),
            animation: Some(anim.clone()),
        },
        CommandOperation::ClipTextRichReset {
            item_id: text_id.clone(),
        },
    ];

    for op in ops {
        let serialized = serde_json::to_string(&op).expect("serialize CommandOperation");
        let deserialized: CommandOperation =
            serde_json::from_str(&serialized).expect("deserialize CommandOperation");
        assert_eq!(op, deserialized, "Serde round-trip failed for operation");
    }
}

#[test]
fn test_qa_bf02_empty_rebind_duration_equality_rescale_and_video_rejection() {
    let rate = FrameRate::FPS_30;
    let (mut project, text_id, vid_id) = create_qa_project(rate);
    let mut service = CommandService::new(project.clone());

    // 1. Empty string rebind with pre-existing spans
    service
        .execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision: service.revision(),
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_setup_spans".to_string()),
            operations: vec![
                OperationEnvelope {
                    operation_id: "op_text".to_string(),
                    operation: CommandOperation::ClipTextSet {
                        item_id: text_id.clone(),
                        text: "ABCDEF".to_string(),
                    },
                },
                OperationEnvelope {
                    operation_id: "op_spans".to_string(),
                    operation: CommandOperation::ClipTextSpansSet {
                        item_id: text_id.clone(),
                        spans: vec![
                            RichTextSpan {
                                start: 0,
                                end: 3,
                                style: TextSpanStyle::default(),
                            },
                            RichTextSpan {
                                start: 3,
                                end: 6,
                                style: TextSpanStyle::default(),
                            },
                        ],
                    },
                },
            ],
        })
        .unwrap();

    // Rebind to empty string ""
    service
        .execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision: service.revision(),
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_rebind_empty".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_empty".to_string(),
                operation: CommandOperation::ClipTextSet {
                    item_id: text_id.clone(),
                    text: "".to_string(),
                },
            }],
        })
        .expect("Rebinding text to empty string must succeed without error");

    let empty_item = service.project().tracks[1].find_item(&text_id).unwrap();
    assert_eq!(empty_item.text_content.as_deref(), Some(""));
    assert!(
        empty_item.text_rich.as_ref().unwrap().spans.is_empty(),
        "All spans must be pruned when rebind text is empty"
    );

    // 2. Exact equality (in + out == clip) under Reject
    let clip_duration_24 = RationalTime::from_frames(24, FrameRate::new(24, 1));
    let in_anim_12 = TextAnimationSegment {
        duration: RationalTime::from_frames(12, FrameRate::new(24, 1)),
        easing: KeyframeEasing::Linear,
        stagger_ratio: 0.0,
        from: AnimationTransform::default(),
        to: AnimationTransform::default(),
    };
    let out_anim_12 = TextAnimationSegment {
        duration: RationalTime::from_frames(12, FrameRate::new(24, 1)),
        easing: KeyframeEasing::Linear,
        stagger_ratio: 0.0,
        from: AnimationTransform::default(),
        to: AnimationTransform::default(),
    };
    let anim_exact = TextAnimation {
        in_anim: Some(in_anim_12),
        out_anim: Some(out_anim_12),
        conflict_policy: DurationConflictPolicy::Reject,
        ..Default::default()
    };
    assert!(
        anim_exact
            .validate_for_clip_duration(clip_duration_24)
            .is_ok(),
        "in (12f) + out (12f) == clip (24f) must be accepted under Reject policy"
    );

    // 3. Cross-rate duration rescale:
    // Clip duration is 48 frames at 24fps (= 2.0 seconds).
    // In-animation is 30 frames at 30fps (= 1.0 second -> rescales to 24 frames at 24fps).
    // Out-animation is 60 frames at 60fps (= 1.0 second -> rescales to 24 frames at 24fps).
    // Total = 1.0s + 1.0s = 2.0s = exactly 48 frames at 24fps -> Accepted!
    let in_anim_30fps = TextAnimationSegment {
        duration: RationalTime::from_frames(30, FrameRate::new(30, 1)),
        easing: KeyframeEasing::Linear,
        stagger_ratio: 0.0,
        from: AnimationTransform::default(),
        to: AnimationTransform::default(),
    };
    let out_anim_60fps = TextAnimationSegment {
        duration: RationalTime::from_frames(60, FrameRate::new(60, 1)),
        easing: KeyframeEasing::Linear,
        stagger_ratio: 0.0,
        from: AnimationTransform::default(),
        to: AnimationTransform::default(),
    };
    let clip_duration_48 = RationalTime::from_frames(48, FrameRate::new(24, 1));
    let cross_rate_anim = TextAnimation {
        in_anim: Some(in_anim_30fps),
        out_anim: Some(out_anim_60fps),
        conflict_policy: DurationConflictPolicy::Reject,
        ..Default::default()
    };
    assert!(
        cross_rate_anim
            .validate_for_clip_duration(clip_duration_48)
            .is_ok(),
        "Cross-rate in (1.0s) + out (1.0s) == clip (2.0s) must be accepted after rescaling"
    );

    // 4. load_project rejection of a Video item carrying text_rich or text_animation
    let dir = qa_test_dir("video_rich_rejection");
    let corrupt_file = dir.join("corrupt_video_rich.json");

    let video_item = project.tracks[0].find_item_mut(&vid_id).unwrap();
    video_item.text_rich = Some(TextRichStyle::default());

    // Serialize corrupt project directly to bypass memory validate() before writing
    let json_str = serde_json::to_string_pretty(&project).unwrap();
    fs::write(&corrupt_file, json_str).unwrap();

    let load_res = load_project(&corrupt_file);
    assert!(
        load_res.is_err(),
        "load_project must reject project containing Video item with text_rich"
    );
    match load_res.unwrap_err() {
        PersistenceError::Validation(msg) => {
            assert!(
                msg.contains("Only Text items can have text_rich or text_animation"),
                "Expected validation message about Text items, got: {msg}"
            );
        }
        other => panic!("Expected PersistenceError::Validation, got {:?}", other),
    }
}
