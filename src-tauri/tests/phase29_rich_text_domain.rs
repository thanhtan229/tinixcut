use std::fs;
use std::path::PathBuf;
use tinixcut_lib::modules::automation::{AutomationAdapter, AutomationSession};
use tinixcut_lib::modules::command::{
    CommandEnvelope, CommandError, CommandOperation, CommandService, OperationEnvelope,
};
use tinixcut_lib::modules::composition::CompositionPlan;
use tinixcut_lib::modules::domain::text::*;
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    AnimationTransform, AnimationUnit, AudioCrossfade, CommandLedgerEntry, CutTransition,
    DomainError, DurationConflictPolicy, FontReference, ItemKind, Keyframe, KeyframeEasing,
    KeyframeProperty, LoopMode, Project, RichTextSpan, StaggerOrder, StrokeAlign, TextAlignment,
    TextAnimation, TextAnimationSegment, TextBackgroundBox, TextFill, TextLayout, TextRichStyle,
    TextShadow, TextSpanStyle, TextStroke, TextStyle, TextTemplateReference, TimelineItem, Track,
    TrackKind, Transition, TransitionKind, CURRENT_SCHEMA_VERSION,
};
use tinixcut_lib::modules::history::{HistoryEntry, HistoryManager};
use tinixcut_lib::modules::persistence::{
    load_project, run_migrations_if_needed, save_project_atomic, PersistenceError,
};

fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tinixcut_phase29_{name}"));
    if dir.exists() {
        let _ = fs::remove_dir_all(&dir);
    }
    fs::create_dir_all(&dir).expect("create test dir");
    dir
}

fn create_test_project_with_text_and_video(rate: FrameRate) -> (Project, String, String) {
    let mut project = Project::new("Phase 29 Project".to_string());

    let mut video_track = Track::new("Video Track".to_string(), TrackKind::Video);
    let vid_item = TimelineItem::new(
        None,
        "Test Video".to_string(),
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
        "Test Text".to_string(),
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
    text_item.text_content = Some("Initial text".to_string());
    let text_id = text_item.item_id.clone();
    text_track.items.push(text_item);
    project.tracks.push(text_track);

    (project, text_id, vid_id)
}

#[test]
fn test_persistence_v9_to_v10_fixture_migration() {
    let rate = FrameRate::FPS_30;
    let (mut project, text_id, _) = create_test_project_with_text_and_video(rate);

    let text_item = project.tracks[1].find_item_mut(&text_id).unwrap();
    text_item.keyframes.push(Keyframe::new(
        KeyframeProperty::Opacity,
        RationalTime::zero(rate),
        1.0,
    ));

    let mut v9_val = serde_json::to_value(&project).expect("serialize");
    v9_val["schema_version"] = serde_json::json!(9);

    let raw = serde_json::to_string(&v9_val).expect("to string");
    let migrated_json = run_migrations_if_needed(&raw).expect("run migration");
    let loaded: Project = serde_json::from_str(&migrated_json).expect("deserialize");

    assert_eq!(loaded.schema_version, 10);
    let item = loaded.tracks[1].find_item(&text_id).unwrap();
    assert_eq!(item.text_content.as_deref(), Some("Initial text"));
    assert!(item.text_rich.is_none());
    assert!(item.text_animation.is_none());
    assert_eq!(item.keyframes.len(), 1);
    loaded.validate().expect("migrated project is valid");

    let future_json = r#"{"schema_version": 11, "name": "Future"}"#;
    let err = run_migrations_if_needed(future_json).unwrap_err();
    match err {
        PersistenceError::UnsupportedSchemaVersion {
            found,
            max_supported,
        } => {
            assert_eq!(found, 11);
            assert_eq!(max_supported, 10);
        }
        other => panic!("Expected UnsupportedSchemaVersion, got {:?}", other),
    }
}

#[test]
fn test_commands_all_seven_operations_apply_and_roundtrip_undo_redo() {
    let rate = FrameRate::FPS_30;
    let (project, text_id, _) = create_test_project_with_text_and_video(rate);
    let mut service = CommandService::new(project.clone());
    let mut history = HistoryManager::new(20);

    // 1. ClipTextRichSet
    let initial_rich = TextRichStyle {
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
        fill: TextFill::Solid {
            color: "#FFAA00".to_string(),
        },
        strokes: vec![TextStroke {
            color: "#000000".to_string(),
            width: 2.0,
            align: StrokeAlign::Outside,
        }],
        shadows: vec![TextShadow {
            color: "#00000088".to_string(),
            offset_x: 2.0,
            offset_y: 2.0,
            blur: 4.0,
        }],
        background: Some(TextBackgroundBox {
            fill: TextFill::Solid {
                color: "#222222".to_string(),
            },
            padding_x: 10.0,
            padding_y: 5.0,
            corner_radius: 8.0,
            opacity: 0.9,
        }),
        spans: vec![RichTextSpan {
            start: 0,
            end: 7,
            style: TextSpanStyle {
                fill: Some(TextFill::Solid {
                    color: "#FF0000".to_string(),
                }),
                ..Default::default()
            },
        }],
    };

    let pre_snap = service.project().clone();
    service
        .execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision: 0,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_rich_set".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_rich_set".to_string(),
                operation: CommandOperation::ClipTextRichSet {
                    item_id: text_id.clone(),
                    rich: initial_rich.clone(),
                },
            }],
        })
        .expect("ClipTextRichSet");

    let post_snap = service.project().clone();
    history.record_edit(HistoryEntry {
        description: "Set rich text".to_string(),
        transaction_id: Some("tx_rich_set".to_string()),
        prior_snapshot: pre_snap.clone(),
        resulting_snapshot: post_snap.clone(),
    });

    let mut active = post_snap;
    assert_eq!(
        active.tracks[1]
            .find_item(&text_id)
            .unwrap()
            .text_rich
            .as_ref()
            .unwrap(),
        &initial_rich
    );

    // Undo -> restores pre_snap (None)
    history.undo(&mut active).expect("undo rich set");
    assert!(active.tracks[1]
        .find_item(&text_id)
        .unwrap()
        .text_rich
        .is_none());

    // Redo -> restores initial_rich
    history.redo(&mut active).expect("redo rich set");
    assert_eq!(
        active.tracks[1]
            .find_item(&text_id)
            .unwrap()
            .text_rich
            .as_ref()
            .unwrap(),
        &initial_rich
    );

    // 2. ClipTextSpansSet
    service
        .execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision: 1,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_spans".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_spans".to_string(),
                operation: CommandOperation::ClipTextSpansSet {
                    item_id: text_id.clone(),
                    spans: vec![RichTextSpan {
                        start: 8,
                        end: 12,
                        style: TextSpanStyle {
                            font_size: Some(40.0),
                            ..Default::default()
                        },
                    }],
                },
            }],
        })
        .expect("ClipTextSpansSet");
    assert_eq!(
        service.project().tracks[1]
            .find_item(&text_id)
            .unwrap()
            .text_rich
            .as_ref()
            .unwrap()
            .spans
            .len(),
        1
    );

    // 3. ClipTextLayoutSet
    let new_layout = TextLayout {
        alignment: TextAlignment::Right,
        line_height: 1.5,
        letter_spacing: 2.0,
        ..Default::default()
    };
    service
        .execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision: 2,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_layout".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_layout".to_string(),
                operation: CommandOperation::ClipTextLayoutSet {
                    item_id: text_id.clone(),
                    layout: new_layout.clone(),
                },
            }],
        })
        .expect("ClipTextLayoutSet");
    assert_eq!(
        service.project().tracks[1]
            .find_item(&text_id)
            .unwrap()
            .text_rich
            .as_ref()
            .unwrap()
            .layout,
        new_layout
    );

    // 4. ClipTextAppearanceSet
    service
        .execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision: 3,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_appearance".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_appearance".to_string(),
                operation: CommandOperation::ClipTextAppearanceSet {
                    item_id: text_id.clone(),
                    fill: TextFill::Solid {
                        color: "#00FF00".to_string(),
                    },
                    strokes: vec![],
                    shadows: vec![],
                    background: None,
                },
            }],
        })
        .expect("ClipTextAppearanceSet");
    let appearance_rich = service.project().tracks[1]
        .find_item(&text_id)
        .unwrap()
        .text_rich
        .clone()
        .unwrap();
    assert_eq!(
        appearance_rich.fill,
        TextFill::Solid {
            color: "#00FF00".to_string()
        }
    );
    assert!(appearance_rich.strokes.is_empty());
    assert!(appearance_rich.shadows.is_empty());
    assert!(appearance_rich.background.is_none());

    // 5. ClipTextFontSet
    let new_font = FontReference {
        family: "Roboto".to_string(),
        weight: 900,
        style: FontStyle::Italic,
        asset_id: None,
    };
    service
        .execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision: 4,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_font".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_font".to_string(),
                operation: CommandOperation::ClipTextFontSet {
                    item_id: text_id.clone(),
                    font: new_font.clone(),
                    font_size: Some(64.0),
                },
            }],
        })
        .expect("ClipTextFontSet");
    let font_rich = service.project().tracks[1]
        .find_item(&text_id)
        .unwrap()
        .text_rich
        .clone()
        .unwrap();
    assert_eq!(font_rich.font, Some(new_font));
    assert_eq!(font_rich.font_size, Some(64.0));

    // 6. ClipTextAnimationSet
    let anim = TextAnimation {
        unit: AnimationUnit::Word,
        order: StaggerOrder::Forward,
        seed: None,
        pivot_x: 0.5,
        pivot_y: 0.5,
        in_anim: Some(TextAnimationSegment {
            duration: RationalTime::from_frames(15, rate),
            easing: KeyframeEasing::EaseOut,
            stagger_ratio: 0.2,
            from: AnimationTransform {
                translate_x: 0.0,
                translate_y: 50.0,
                scale_x: 1.0,
                scale_y: 1.0,
                rotation: 0.0,
                opacity: 0.0,
            },
            to: AnimationTransform::default(),
        }),
        loop_anim: None,
        out_anim: None,
        loop_mode: LoopMode::None,
        conflict_policy: DurationConflictPolicy::Reject,
    };
    service
        .execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision: 5,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_anim".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_anim".to_string(),
                operation: CommandOperation::ClipTextAnimationSet {
                    item_id: text_id.clone(),
                    animation: Some(anim.clone()),
                },
            }],
        })
        .expect("ClipTextAnimationSet");
    assert_eq!(
        service.project().tracks[1]
            .find_item(&text_id)
            .unwrap()
            .text_animation,
        Some(anim)
    );

    // 7. ClipTextRichReset
    service
        .execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision: 6,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_reset".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_reset".to_string(),
                operation: CommandOperation::ClipTextRichReset {
                    item_id: text_id.clone(),
                },
            }],
        })
        .expect("ClipTextRichReset");
    assert!(service.project().tracks[1]
        .find_item(&text_id)
        .unwrap()
        .text_rich
        .is_none());
    // Animation is NOT cleared by ClipTextRichReset
    assert!(service.project().tracks[1]
        .find_item(&text_id)
        .unwrap()
        .text_animation
        .is_some());
}

#[test]
fn test_clip_text_set_grapheme_rebinding() {
    let rate = FrameRate::FPS_30;
    let (project, text_id, _) = create_test_project_with_text_and_video(rate);
    let mut service = CommandService::new(project.clone());

    // Initial text: "Hello" (5 graphemes) with full span [0, 5)
    service
        .execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision: 0,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_init".to_string()),
            operations: vec![
                OperationEnvelope {
                    operation_id: "op_text".to_string(),
                    operation: CommandOperation::ClipTextSet {
                        item_id: text_id.clone(),
                        text: "Hello".to_string(),
                    },
                },
                OperationEnvelope {
                    operation_id: "op_rich".to_string(),
                    operation: CommandOperation::ClipTextRichSet {
                        item_id: text_id.clone(),
                        rich: TextRichStyle {
                            spans: vec![RichTextSpan {
                                start: 0,
                                end: 5, // full span [0, old_count)
                                style: TextSpanStyle::default(),
                            }],
                            ..Default::default()
                        },
                    },
                },
            ],
        })
        .expect("init hello with full span");

    // Case 1: Full-span expansion. New text: "Hello World!" (12 graphemes)
    service
        .execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision: 1,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_expand".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_expand".to_string(),
                operation: CommandOperation::ClipTextSet {
                    item_id: text_id.clone(),
                    text: "Hello World!".to_string(),
                },
            }],
        })
        .expect("expand text");

    let item = service.project().tracks[1].find_item(&text_id).unwrap();
    let spans = &item.text_rich.as_ref().unwrap().spans;
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].start, 0);
    assert_eq!(
        spans[0].end, 12,
        "Full span expanded from [0, 5) to [0, 12)"
    );

    // Set non-overlapping spans: [0, 2) and [2, 5)
    service
        .execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision: 2,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_two_spans".to_string()),
            operations: vec![
                OperationEnvelope {
                    operation_id: "op_two_text".to_string(),
                    operation: CommandOperation::ClipTextSet {
                        item_id: text_id.clone(),
                        text: "Hello".to_string(),
                    },
                },
                OperationEnvelope {
                    operation_id: "op_two_spans".to_string(),
                    operation: CommandOperation::ClipTextSpansSet {
                        item_id: text_id.clone(),
                        spans: vec![
                            RichTextSpan {
                                start: 0,
                                end: 2,
                                style: TextSpanStyle::default(),
                            },
                            RichTextSpan {
                                start: 2,
                                end: 5,
                                style: TextSpanStyle::default(),
                            },
                        ],
                    },
                },
            ],
        })
        .expect("set two spans");

    // Case 2: Clamp. Shorten text to "Hey" (3 graphemes)
    // Span 0: [0, 2) stays [0, 2)
    // Span 1: [2, 5) clamped to [2, 3)
    service
        .execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision: 3,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_clamp".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_clamp".to_string(),
                operation: CommandOperation::ClipTextSet {
                    item_id: text_id.clone(),
                    text: "Hey".to_string(),
                },
            }],
        })
        .expect("clamp text");

    let item = service.project().tracks[1].find_item(&text_id).unwrap();
    let spans = &item.text_rich.as_ref().unwrap().spans;
    assert_eq!(spans.len(), 2);
    assert_eq!(spans[0].start, 0);
    assert_eq!(spans[0].end, 2);
    assert_eq!(spans[1].start, 2);
    assert_eq!(spans[1].end, 3, "Clamped from [2, 5) to [2, 3)");

    // Case 3: Prune. Shorten text to "He" (2 graphemes)
    // Span 0: [0, 2)
    // Span 1: start = min(2, 2) = 2, end = min(3, 2) = 2 -> start >= end -> PRUNED!
    service
        .execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision: 4,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_prune".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_prune".to_string(),
                operation: CommandOperation::ClipTextSet {
                    item_id: text_id.clone(),
                    text: "He".to_string(),
                },
            }],
        })
        .expect("prune text");

    let item = service.project().tracks[1].find_item(&text_id).unwrap();
    let spans = &item.text_rich.as_ref().unwrap().spans;
    assert_eq!(spans.len(), 1, "Second span pruned");
    assert_eq!(spans[0].start, 0);
    assert_eq!(spans[0].end, 2);
}

#[test]
fn test_invalid_second_operation_rolls_back_transaction() {
    let rate = FrameRate::FPS_30;
    let (project, text_id, _) = create_test_project_with_text_and_video(rate);
    let mut service = CommandService::new(project.clone());

    let valid_rich = TextRichStyle {
        fill: TextFill::Solid {
            color: "#123456".to_string(),
        },
        ..Default::default()
    };

    // Op 1 is valid, Op 2 is invalid (conflicting animation under Reject policy)
    let invalid_anim = TextAnimation {
        in_anim: Some(TextAnimationSegment {
            duration: RationalTime::from_frames(60, rate),
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.0,
            from: AnimationTransform::default(),
            to: AnimationTransform::default(),
        }),
        out_anim: Some(TextAnimationSegment {
            duration: RationalTime::from_frames(60, rate), // 60 + 60 = 120 > clip duration 100
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.0,
            from: AnimationTransform::default(),
            to: AnimationTransform::default(),
        }),
        conflict_policy: DurationConflictPolicy::Reject,
        ..Default::default()
    };

    let result = service.execute(CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: 0,
        client_id: None,
        client_epoch: None,
        sequence_number: None,
        dry_run: false,
        transaction_id: Some("tx_atomic_rollback".to_string()),
        operations: vec![
            OperationEnvelope {
                operation_id: "op_valid_rich".to_string(),
                operation: CommandOperation::ClipTextRichSet {
                    item_id: text_id.clone(),
                    rich: valid_rich,
                },
            },
            OperationEnvelope {
                operation_id: "op_invalid_anim".to_string(),
                operation: CommandOperation::ClipTextAnimationSet {
                    item_id: text_id.clone(),
                    animation: Some(invalid_anim),
                },
            },
        ],
    });

    assert!(result.is_err(), "Transaction must fail");
    // Verify rollback: text_rich must still be None
    let item = service.project().tracks[1].find_item(&text_id).unwrap();
    assert!(
        item.text_rich.is_none(),
        "Op 1 must be rolled back on Op 2 failure"
    );
    assert_eq!(service.project().revision, 0);
}

#[test]
fn test_stale_revision_and_replay_after_save_reopen() {
    let rate = FrameRate::FPS_30;
    let (project, text_id, _) = create_test_project_with_text_and_video(rate);
    let mut service = CommandService::new(project.clone());

    // Stale revision: project is at revision 0, base_revision = 5
    let err = service
        .execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision: 5,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_stale".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_stale".to_string(),
                operation: CommandOperation::ClipTextRichReset {
                    item_id: text_id.clone(),
                },
            }],
        })
        .unwrap_err();

    assert!(matches!(err, CommandError::StaleRevision { .. }));

    // Replay after save/reopen does not duplicate mutation
    let temp_dir = test_dir("save_reopen");
    let proj_file = temp_dir.join("project.tcproj");

    let valid_envelope = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: 0,
        client_id: Some("agent-1".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(1),
        dry_run: false,
        transaction_id: Some("tx_1".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op_1".to_string(),
            operation: CommandOperation::ClipTextSet {
                item_id: text_id.clone(),
                text: "Mutated text".to_string(),
            },
        }],
    };

    let receipt1 = service
        .execute(valid_envelope.clone())
        .expect("execute op 1");
    save_project_atomic(service.project(), &proj_file).expect("save project");

    let loaded = load_project(&proj_file).expect("load project");
    let mut reopened_service = CommandService::new(loaded);

    // Replay identical envelope
    let receipt2 = reopened_service
        .execute(valid_envelope)
        .expect("replay idempotent");
    assert_eq!(
        receipt1.new_revision, receipt2.new_revision,
        "Idempotent replay does not advance revision"
    );
}

#[test]
fn test_operations_reject_non_text_items() {
    let rate = FrameRate::FPS_30;
    let (project, _, vid_id) = create_test_project_with_text_and_video(rate);
    let mut service = CommandService::new(project.clone());

    let ops = vec![
        CommandOperation::ClipTextSet {
            item_id: vid_id.clone(),
            text: "Fail".to_string(),
        },
        CommandOperation::ClipTextRichSet {
            item_id: vid_id.clone(),
            rich: TextRichStyle::default(),
        },
        CommandOperation::ClipTextSpansSet {
            item_id: vid_id.clone(),
            spans: vec![],
        },
        CommandOperation::ClipTextLayoutSet {
            item_id: vid_id.clone(),
            layout: TextLayout::default(),
        },
        CommandOperation::ClipTextAppearanceSet {
            item_id: vid_id.clone(),
            fill: TextFill::default(),
            strokes: vec![],
            shadows: vec![],
            background: None,
        },
        CommandOperation::ClipTextFontSet {
            item_id: vid_id.clone(),
            font: FontReference::default(),
            font_size: Some(24.0),
        },
        CommandOperation::ClipTextAnimationSet {
            item_id: vid_id.clone(),
            animation: None,
        },
        CommandOperation::ClipTextRichReset {
            item_id: vid_id.clone(),
        },
    ];

    for (i, op) in ops.into_iter().enumerate() {
        let err = service
            .execute(CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: project.project_id.clone(),
                base_revision: 0,
                client_id: None,
                client_epoch: None,
                sequence_number: None,
                dry_run: false,
                transaction_id: Some(format!("tx_non_text_{i}")),
                operations: vec![OperationEnvelope {
                    operation_id: format!("op_non_text_{i}"),
                    operation: op,
                }],
            })
            .unwrap_err();

        assert!(
            matches!(err, CommandError::OperationValidationFailed { .. }),
            "Operation on non-Text item must fail with OperationValidationFailed"
        );
    }
}

#[test]
fn test_composition_visual_layer_and_preview_sampled_layer_carry_through() {
    let rate = FrameRate::FPS_30;
    let (mut project, text_id, vid_id) = create_test_project_with_text_and_video(rate);

    let rich = TextRichStyle {
        fill: TextFill::Solid {
            color: "#FFEE00".to_string(),
        },
        ..Default::default()
    };
    let anim = TextAnimation {
        unit: AnimationUnit::Grapheme,
        ..Default::default()
    };

    let text_item = project.tracks[1].find_item_mut(&text_id).unwrap();
    text_item.text_rich = Some(rich.clone());
    text_item.text_animation = Some(anim.clone());

    let plan = CompositionPlan::from_project(&project).expect("generate plan");

    let text_layer = plan
        .visual_layers
        .iter()
        .find(|l| l.item_id == text_id)
        .expect("text layer in plan");
    assert_eq!(text_layer.text_rich, Some(rich.clone()));
    assert_eq!(text_layer.text_animation, Some(anim.clone()));

    let video_layer = plan
        .visual_layers
        .iter()
        .find(|l| l.item_id == vid_id)
        .expect("video layer in plan");
    assert!(video_layer.text_rich.is_none());
    assert!(video_layer.text_animation.is_none());

    let frame = plan.sample_preview_at(RationalTime::from_frames(10, rate));
    let sampled_text = frame
        .active_layers
        .iter()
        .find(|l| l.item_id == text_id)
        .expect("sampled text layer");
    assert_eq!(sampled_text.text_rich, Some(rich));
    assert_eq!(sampled_text.text_animation, Some(anim));

    let sampled_video = frame
        .active_layers
        .iter()
        .find(|l| l.item_id == vid_id)
        .expect("sampled video layer");
    assert!(sampled_video.text_rich.is_none());
    assert!(sampled_video.text_animation.is_none());
}

#[test]
fn test_automation_capabilities_reporting() {
    let ops = AutomationAdapter::supported_operations();
    for expected in [
        "ClipTextRichSet",
        "ClipTextSpansSet",
        "ClipTextLayoutSet",
        "ClipTextAppearanceSet",
        "ClipTextFontSet",
        "ClipTextAnimationSet",
        "ClipTextRichReset",
    ] {
        assert!(
            ops.contains(&expected.to_string()),
            "AutomationAdapter::supported_operations must contain {expected}"
        );
    }

    assert_eq!(
        AutomationAdapter::supported_animation_units(),
        vec!["All", "Grapheme", "Word", "Line"]
    );
    assert_eq!(
        AutomationAdapter::supported_stagger_orders(),
        vec!["Forward", "Backward", "CenterOut", "Random"]
    );
    assert_eq!(
        AutomationAdapter::supported_loop_modes(),
        vec!["None", "Repeat", "PingPong"]
    );
    assert_eq!(
        AutomationAdapter::supported_duration_conflict_policies(),
        vec!["Reject", "ProportionalScale", "ClampOut"]
    );

    let rate = FrameRate::FPS_30;
    let (project, _, _) = create_test_project_with_text_and_video(rate);
    let report = AutomationAdapter::inspect_project(&project);

    assert!(report
        .supported_operations
        .contains(&"ClipTextRichSet".to_string()));
    assert_eq!(report.supported_animation_units.len(), 4);
    assert_eq!(report.supported_stagger_orders.len(), 4);
    assert_eq!(report.supported_loop_modes.len(), 3);
    assert_eq!(report.supported_duration_conflict_policies.len(), 3);

    // BF-01 item 4: AutomationSession capabilities path parity check
    let session = AutomationSession::new();
    let caps = session.capabilities().expect("session capabilities");
    assert!(!caps.supported_animation_units.is_empty());
    assert!(!caps.supported_stagger_orders.is_empty());
    assert!(!caps.supported_loop_modes.is_empty());
    assert!(!caps.supported_duration_conflict_policies.is_empty());
    assert_eq!(
        caps.supported_animation_units,
        report.supported_animation_units
    );
    assert_eq!(
        caps.supported_stagger_orders,
        report.supported_stagger_orders
    );
    assert_eq!(caps.supported_loop_modes, report.supported_loop_modes);
    assert_eq!(
        caps.supported_duration_conflict_policies,
        report.supported_duration_conflict_policies
    );
}

#[test]
fn test_bf02_json_disk_roundtrip_and_command_serde() {
    let rate = FrameRate::FPS_30;
    let (mut project, text_id, _) = create_test_project_with_text_and_video(rate);

    let text_content = "Hello World! 🚀";
    let text_item = project.tracks[1].find_item_mut(&text_id).unwrap();
    text_item.text_content = Some(text_content.to_string());

    let rich_style = TextRichStyle {
        font: Some(FontReference {
            family: "Inter".to_string(),
            weight: 700,
            style: FontStyle::Normal,
            asset_id: None,
        }),
        font_size: Some(48.0),
        layout: TextLayout {
            alignment: TextAlignment::Center,
            wrap_mode: TextWrapMode::Wrap,
            max_width: Some(1920.0),
            line_height: 1.2,
            letter_spacing: 1.5,
            min_font_size: None,
            max_font_size: None,
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
        strokes: vec![
            TextStroke {
                color: "#FFFFFF".to_string(),
                width: 2.0,
                align: StrokeAlign::Outside,
            },
            TextStroke {
                color: "#000000".to_string(),
                width: 1.0,
                align: StrokeAlign::Center,
            },
        ],
        shadows: vec![
            TextShadow {
                color: "#00000088".to_string(),
                blur: 4.0,
                offset_x: 2.0,
                offset_y: 2.0,
            },
            TextShadow {
                color: "#FFFFFF44".to_string(),
                blur: 8.0,
                offset_x: 0.0,
                offset_y: 4.0,
            },
        ],
        background: Some(TextBackgroundBox {
            fill: TextFill::Solid {
                color: "#111111AA".to_string(),
            },
            padding_x: 16.0,
            padding_y: 8.0,
            corner_radius: 4.0,
            opacity: 1.0,
        }),
        spans: vec![
            RichTextSpan {
                start: 0,
                end: 5,
                style: TextSpanStyle {
                    fill: Some(TextFill::Solid {
                        color: "#00FF00".to_string(),
                    }),
                    font: None,
                    font_size: None,
                    letter_spacing: None,
                },
            },
            RichTextSpan {
                start: 6,
                end: 11,
                style: TextSpanStyle {
                    fill: None,
                    font: Some(FontReference {
                        family: "Arial".to_string(),
                        weight: 400,
                        style: FontStyle::Italic,
                        asset_id: None,
                    }),
                    font_size: None,
                    letter_spacing: None,
                },
            },
        ],
    };

    let anim = TextAnimation {
        unit: AnimationUnit::Word,
        order: StaggerOrder::CenterOut,
        seed: None,
        pivot_x: 0.5,
        pivot_y: 0.5,
        in_anim: Some(TextAnimationSegment {
            duration: RationalTime::from_frames(15, rate),
            easing: KeyframeEasing::EaseOut,
            stagger_ratio: 0.2,
            from: AnimationTransform {
                translate_x: 0.0,
                translate_y: 50.0,
                scale_x: 0.8,
                scale_y: 0.8,
                rotation: 0.0,
                opacity: 0.0,
            },
            to: AnimationTransform::default(),
        }),
        out_anim: Some(TextAnimationSegment {
            duration: RationalTime::from_frames(15, rate),
            easing: KeyframeEasing::EaseIn,
            stagger_ratio: 0.1,
            from: AnimationTransform::default(),
            to: AnimationTransform {
                translate_x: 0.0,
                translate_y: -50.0,
                scale_x: 1.0,
                scale_y: 1.0,
                rotation: 0.0,
                opacity: 0.0,
            },
        }),
        loop_anim: Some(TextAnimationSegment {
            duration: RationalTime::from_frames(30, rate),
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.0,
            from: AnimationTransform::default(),
            to: AnimationTransform::default(),
        }),
        loop_mode: LoopMode::PingPong,
        conflict_policy: DurationConflictPolicy::Reject,
    };

    text_item.text_rich = Some(rich_style);
    text_item.text_animation = Some(anim);

    let saved_item = text_item.clone();

    let dir = test_dir("bf02_roundtrip");
    let file_path = dir.join("roundtrip_project.tinix");
    save_project_atomic(&project, &file_path).expect("save project atomic");

    // Assert raw JSON file contains tagged kind discriminators for TextFill
    let raw_json = fs::read_to_string(&file_path).expect("read project json");
    assert!(
        raw_json.contains(r#""kind":"Gradient""#) || raw_json.contains(r#""kind": "Gradient""#),
        "TextFill::Gradient must serialize with internal 'kind' discriminator"
    );
    assert!(
        raw_json.contains(r#""kind":"Solid""#) || raw_json.contains(r#""kind": "Solid""#),
        "TextFill::Solid must serialize with internal 'kind' discriminator"
    );

    let loaded_project = load_project(&file_path).expect("load project");
    let loaded_item = loaded_project.tracks[1].find_item(&text_id).unwrap();
    assert_eq!(
        loaded_item, &saved_item,
        "Loaded TimelineItem must match saved TimelineItem exactly"
    );

    // Serde round-trip all 7 new CommandOperation variants
    let ops: Vec<CommandOperation> = vec![
        CommandOperation::ClipTextRichSet {
            item_id: "item_1".to_string(),
            rich: saved_item.text_rich.clone().unwrap(),
        },
        CommandOperation::ClipTextSpansSet {
            item_id: "item_1".to_string(),
            spans: saved_item.text_rich.as_ref().unwrap().spans.clone(),
        },
        CommandOperation::ClipTextLayoutSet {
            item_id: "item_1".to_string(),
            layout: saved_item.text_rich.as_ref().unwrap().layout.clone(),
        },
        CommandOperation::ClipTextAppearanceSet {
            item_id: "item_1".to_string(),
            fill: saved_item.text_rich.as_ref().unwrap().fill.clone(),
            strokes: saved_item.text_rich.as_ref().unwrap().strokes.clone(),
            shadows: saved_item.text_rich.as_ref().unwrap().shadows.clone(),
            background: saved_item.text_rich.as_ref().unwrap().background.clone(),
        },
        CommandOperation::ClipTextFontSet {
            item_id: "item_1".to_string(),
            font: saved_item.text_rich.as_ref().unwrap().font.clone().unwrap(),
            font_size: saved_item.text_rich.as_ref().unwrap().font_size,
        },
        CommandOperation::ClipTextAnimationSet {
            item_id: "item_1".to_string(),
            animation: saved_item.text_animation.clone(),
        },
        CommandOperation::ClipTextRichReset {
            item_id: "item_1".to_string(),
        },
    ];

    for op in ops {
        let op_json = serde_json::to_string(&op).expect("serialize CommandOperation");
        let op_deser: CommandOperation =
            serde_json::from_str(&op_json).expect("deserialize CommandOperation");
        assert_eq!(op_deser, op, "CommandOperation round-trip mismatch");
    }
}

#[test]
fn test_bf02_v9_to_v10_fixture_comprehensive_preservation() {
    let rate = FrameRate::FPS_30;
    let (mut project, text_id, vid_id) = create_test_project_with_text_and_video(rate);

    // Populate text_style, text_template, item transition on text item
    let text_item = project.tracks[1].find_item_mut(&text_id).unwrap();
    let original_style = TextStyle {
        font_family: "Roboto".to_string(),
        font_size: 32.0,
        font_weight: 600,
        font_color: "#123456".to_string(),
        accent_color: Some("#AABBCC".to_string()),
        background_color: Some("#333333".to_string()),
        background_opacity: 0.8,
        padding_x: 10.0,
        padding_y: 5.0,
        alignment: TextAlignment::Left,
        line_height: 1.1,
        anchor: tinixcut_lib::modules::domain::TextAnchor::TopLeft,
    };
    text_item.text_style = Some(original_style.clone());

    let original_template = TextTemplateReference {
        template_id: "tmpl_news_lower_third".to_string(),
        template_version: 2,
    };
    text_item.text_template = Some(original_template.clone());

    let original_item_transition = Transition {
        kind: TransitionKind::FadeIn,
        duration: RationalTime::from_frames(12, rate),
    };
    text_item.transition = Some(original_item_transition.clone());

    // Add another video item to video track so we can put a CutTransition
    let vid_item_2 = TimelineItem::new(
        None,
        "Test Video 2".to_string(),
        ItemKind::Video,
        TimeRange::new(
            RationalTime::from_frames(100, rate),
            RationalTime::from_frames(100, rate),
        ),
        TimeRange::new(
            RationalTime::from_frames(100, rate),
            RationalTime::from_frames(100, rate),
        ),
    )
    .expect("create video item 2");
    let vid_2_id = vid_item_2.item_id.clone();
    project.tracks[0].items.push(vid_item_2);

    let cut_trans = CutTransition::new(
        project.tracks[0].track_id.clone(),
        vid_id.clone(),
        vid_2_id.clone(),
        TransitionKind::CrossDissolve,
        RationalTime::from_frames(10, rate),
    )
    .expect("valid cut transition");
    project.tracks[0].transitions.push(cut_trans.clone());

    // Add audio track with 2 audio items and AudioCrossfade
    let audio_asset = tinixcut_lib::modules::domain::Asset {
        asset_id: "ast_audio_1".to_string(),
        name: "test_audio.wav".to_string(),
        path: "C:/media/test_audio.wav".to_string(),
        media_kind: tinixcut_lib::modules::domain::MediaKind::Audio,
        duration: Some(RationalTime::from_frames(200, rate)),
        file_size: 1024,
        dimensions: None,
    };
    project.assets.push(audio_asset);

    let mut audio_track = Track::new("Audio Track".to_string(), TrackKind::Audio);
    let audio_1 = TimelineItem::new(
        Some("ast_audio_1".to_string()),
        "Audio 1".to_string(),
        ItemKind::Audio,
        TimeRange::new(
            RationalTime::zero(rate),
            RationalTime::from_frames(100, rate),
        ),
        TimeRange::new(
            RationalTime::zero(rate),
            RationalTime::from_frames(100, rate),
        ),
    )
    .expect("audio 1");
    let audio_1_id = audio_1.item_id.clone();
    let audio_2 = TimelineItem::new(
        Some("ast_audio_1".to_string()),
        "Audio 2".to_string(),
        ItemKind::Audio,
        TimeRange::new(
            RationalTime::from_frames(100, rate),
            RationalTime::from_frames(100, rate),
        ),
        TimeRange::new(
            RationalTime::from_frames(100, rate),
            RationalTime::from_frames(100, rate),
        ),
    )
    .expect("audio 2");
    let audio_2_id = audio_2.item_id.clone();
    let audio_crossfade = AudioCrossfade {
        transition_id: "trans_audio_crossfade".to_string(),
        track_id: audio_track.track_id.clone(),
        left_item_id: audio_1_id.clone(),
        right_item_id: audio_2_id.clone(),
        duration: RationalTime::from_frames(10, rate),
    };
    audio_track.items.push(audio_1);
    audio_track.items.push(audio_2);
    audio_track.audio_transitions.push(audio_crossfade.clone());
    project.tracks.push(audio_track);

    // Add command ledger entry
    let ledger_entry = CommandLedgerEntry {
        operation_id: "op_v9_preservation".to_string(),
        transaction_id: Some("tx_v9_migration".to_string()),
        client_id: None,
        client_epoch: None,
        sequence_number: None,
        fingerprint: "fingerprint_123".to_string(),
        status: "Applied".to_string(),
        applied_at_revision: 1,
        affected_item_id: Some(text_id.clone()),
        affected_track_id: None,
    };
    project.command_ledger.push(ledger_entry.clone());

    // Serialize as v9 JSON
    let mut v9_val = serde_json::to_value(&project).expect("serialize");
    v9_val["schema_version"] = serde_json::json!(9);

    let raw_v9 = serde_json::to_string(&v9_val).expect("to string");
    let migrated_str = run_migrations_if_needed(&raw_v9).expect("run migration");
    let loaded: Project =
        serde_json::from_str(&migrated_str).expect("deserialize migrated project");

    assert_eq!(loaded.schema_version, 10);

    // Assert preserved unchanged
    let loaded_text_item = loaded.tracks[1].find_item(&text_id).unwrap();
    assert_eq!(loaded_text_item.text_style, Some(original_style));
    assert_eq!(loaded_text_item.text_template, Some(original_template));
    assert_eq!(loaded_text_item.transition, Some(original_item_transition));
    assert!(loaded_text_item.text_rich.is_none());
    assert!(loaded_text_item.text_animation.is_none());

    assert_eq!(loaded.tracks[0].transitions, vec![cut_trans]);
    assert_eq!(loaded.tracks[2].audio_transitions, vec![audio_crossfade]);
    assert_eq!(loaded.command_ledger, vec![ledger_entry]);

    loaded
        .validate()
        .expect("migrated project validates cleanly");
}

#[test]
fn test_bf02_clip_text_set_empty_text_rebinding() {
    let rate = FrameRate::FPS_30;
    let (mut project, text_id, _) = create_test_project_with_text_and_video(rate);
    let item = project.tracks[1].find_item_mut(&text_id).unwrap();
    item.text_content = Some("Hello".to_string());
    item.text_rich = Some(TextRichStyle {
        spans: vec![RichTextSpan {
            start: 0,
            end: 5,
            style: TextSpanStyle {
                fill: Some(TextFill::Solid {
                    color: "#FF0000".to_string(),
                }),
                ..Default::default()
            },
        }],
        ..Default::default()
    });

    let mut service = CommandService::new(project);

    // Rebind from "Hello" (full span [0, 5)) to "" -> prunes all spans and validates
    let env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: service.project().project_id.clone(),
        base_revision: 0,
        client_id: None,
        client_epoch: None,
        sequence_number: None,
        dry_run: false,
        transaction_id: Some("tx_rebind_empty".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op_empty_text".to_string(),
            operation: CommandOperation::ClipTextSet {
                item_id: text_id.clone(),
                text: "".to_string(),
            },
        }],
    };

    let receipt = service
        .execute(env)
        .expect("execute ClipTextSet with empty string");
    assert_eq!(receipt.new_revision, 1);

    let updated_item = service.project().tracks[1].find_item(&text_id).unwrap();
    assert_eq!(updated_item.text_content.as_deref(), Some(""));
    let rich = updated_item.text_rich.as_ref().unwrap();
    assert!(
        rich.spans.is_empty(),
        "Full span must be pruned when text becomes empty"
    );
    service
        .project()
        .validate()
        .expect("project must validate after pruning spans");

    // Partial span test: reset text to "Hello" with partial span [1, 4), then rebind to ""
    let (mut project2, text_id2, _) = create_test_project_with_text_and_video(rate);
    let item2 = project2.tracks[1].find_item_mut(&text_id2).unwrap();
    item2.text_content = Some("Hello".to_string());
    item2.text_rich = Some(TextRichStyle {
        spans: vec![RichTextSpan {
            start: 1,
            end: 4,
            style: TextSpanStyle {
                fill: Some(TextFill::Solid {
                    color: "#00FF00".to_string(),
                }),
                ..Default::default()
            },
        }],
        ..Default::default()
    });

    let mut service2 = CommandService::new(project2);

    // First test partial span clamping on shorter text "Hi" (grapheme count 2): [1, 4) clamps to [1, 2)
    let env_clamp = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: service2.project().project_id.clone(),
        base_revision: 0,
        client_id: None,
        client_epoch: None,
        sequence_number: None,
        dry_run: false,
        transaction_id: Some("tx_clamp".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op_clamp_text".to_string(),
            operation: CommandOperation::ClipTextSet {
                item_id: text_id2.clone(),
                text: "Hi".to_string(),
            },
        }],
    };
    service2.execute(env_clamp).expect("execute clamp");
    let clamped_item = service2.project().tracks[1].find_item(&text_id2).unwrap();
    assert_eq!(clamped_item.text_rich.as_ref().unwrap().spans.len(), 1);
    assert_eq!(clamped_item.text_rich.as_ref().unwrap().spans[0].start, 1);
    assert_eq!(clamped_item.text_rich.as_ref().unwrap().spans[0].end, 2);

    // Now test partial span on empty text: [1, 2) clamps to [0, 0) and is pruned
    let env_empty = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: service2.project().project_id.clone(),
        base_revision: 1,
        client_id: None,
        client_epoch: None,
        sequence_number: None,
        dry_run: false,
        transaction_id: Some("tx_empty_partial".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op_empty_partial".to_string(),
            operation: CommandOperation::ClipTextSet {
                item_id: text_id2.clone(),
                text: "".to_string(),
            },
        }],
    };
    service2
        .execute(env_empty)
        .expect("execute empty on partial span");
    let emptied_item = service2.project().tracks[1].find_item(&text_id2).unwrap();
    assert!(emptied_item.text_rich.as_ref().unwrap().spans.is_empty());
    service2
        .project()
        .validate()
        .expect("project must validate after partial span pruned on empty text");
}

#[test]
fn test_bf02_duration_conflict_equality_and_cross_rate() {
    let rate30 = FrameRate::new(30, 1);
    let clip_dur_30 = RationalTime::from_frames(30, rate30); // 30 frames at 30fps = 1.0s

    let in_seg_15 = TextAnimationSegment {
        duration: RationalTime::from_frames(15, rate30),
        easing: KeyframeEasing::Linear,
        stagger_ratio: 0.0,
        from: AnimationTransform::default(),
        to: AnimationTransform::default(),
    };
    let out_seg_15 = TextAnimationSegment {
        duration: RationalTime::from_frames(15, rate30),
        easing: KeyframeEasing::Linear,
        stagger_ratio: 0.0,
        from: AnimationTransform::default(),
        to: AnimationTransform::default(),
    };

    // 1. Boundary equality: 15f + 15f == 30f under DurationConflictPolicy::Reject -> Accepted
    let equal_anim = TextAnimation {
        in_anim: Some(in_seg_15.clone()),
        out_anim: Some(out_seg_15.clone()),
        conflict_policy: DurationConflictPolicy::Reject,
        ..Default::default()
    };
    assert!(
        equal_anim.validate_for_clip_duration(clip_dur_30).is_ok(),
        "15f + 15f == 30f must be accepted on a 30f clip under Reject"
    );

    // 2. Cross-rate case: segment at 60fps on a 30fps 30-frame clip.
    // 30 frames at 60fps = 0.5s -> rescales to 15 frames at 30fps.
    let rate60 = FrameRate::new(60, 1);
    let in_seg_60fps = TextAnimationSegment {
        duration: RationalTime::from_frames(30, rate60), // 0.5s = 15f @ 30fps
        easing: KeyframeEasing::Linear,
        stagger_ratio: 0.0,
        from: AnimationTransform::default(),
        to: AnimationTransform::default(),
    };

    let cross_rate_equal = TextAnimation {
        in_anim: Some(in_seg_60fps),
        out_anim: Some(out_seg_15), // 15f @ 30fps
        conflict_policy: DurationConflictPolicy::Reject,
        ..Default::default()
    };
    assert!(
        cross_rate_equal
            .validate_for_clip_duration(clip_dur_30)
            .is_ok(),
        "30 frames at 60fps (15f at 30fps) + 15f at 30fps == 30f must be accepted under Reject"
    );

    // Cross-rate exceeding: 32 frames @ 60fps (16 frames @ 30fps) + 15 frames @ 30fps = 31 frames > 30 frames
    let in_seg_60fps_exceeding = TextAnimationSegment {
        duration: RationalTime::from_frames(32, rate60), // 16f @ 30fps
        easing: KeyframeEasing::Linear,
        stagger_ratio: 0.0,
        from: AnimationTransform::default(),
        to: AnimationTransform::default(),
    };
    let cross_rate_exceeding = TextAnimation {
        in_anim: Some(in_seg_60fps_exceeding),
        out_anim: Some(in_seg_15),
        conflict_policy: DurationConflictPolicy::Reject,
        ..Default::default()
    };
    assert!(
        matches!(
            cross_rate_exceeding
                .validate_for_clip_duration(clip_dur_30)
                .unwrap_err(),
            DomainError::AnimationDurationConflict { .. }
        ),
        "31 scaled frames > 30 frame clip must be rejected under Reject"
    );
}

#[test]
fn test_bf02_video_with_rich_or_animation_rejected() {
    let rate = FrameRate::FPS_30;
    let (mut project, _, vid_id) = create_test_project_with_text_and_video(rate);

    let vid_item = project.tracks[0].find_item_mut(&vid_id).unwrap();

    // 1. Video with text_rich: Some(...) rejected by TimelineItem::validate
    vid_item.text_rich = Some(TextRichStyle::default());
    assert!(
        matches!(
            vid_item.validate(),
            Err(DomainError::Validation(ref msg)) if msg.contains("Only Text items can have text_rich or text_animation")
        ),
        "Video item with text_rich must be rejected by TimelineItem::validate"
    );

    // 2. Video with text_animation: Some(...) rejected by TimelineItem::validate
    vid_item.text_rich = None;
    vid_item.text_animation = Some(TextAnimation::default());
    assert!(
        matches!(
            vid_item.validate(),
            Err(DomainError::Validation(ref msg)) if msg.contains("Only Text items can have text_rich or text_animation")
        ),
        "Video item with text_animation must be rejected by TimelineItem::validate"
    );

    // 3. Corrupted project JSON with video item containing text_rich rejected on load_project / deserialization + Project::validate
    let mut corrupt_val = serde_json::to_value(&project).expect("serialize");
    corrupt_val["tracks"][0]["items"][0]["text_rich"] = serde_json::json!({
        "fill": { "kind": "Solid", "color": "#FFFFFF" },
        "strokes": [],
        "shadows": [],
        "spans": []
    });

    let dir = test_dir("corrupt_video_rich");
    let file_path = dir.join("corrupt_project.tinix");
    fs::write(
        &file_path,
        serde_json::to_string_pretty(&corrupt_val).unwrap(),
    )
    .unwrap();

    let load_err = load_project(&file_path).unwrap_err();
    assert!(
        matches!(load_err, PersistenceError::Validation(ref _msg))
            || matches!(load_err, PersistenceError::Serialization(_)),
        "load_project must reject project containing video item with text_rich: {load_err:?}"
    );

    // Also verify Project::validate directly on deserialized project
    let corrupt_proj: Project =
        serde_json::from_str(&serde_json::to_string(&corrupt_val).unwrap()).unwrap();
    assert!(
        matches!(
            corrupt_proj.validate(),
            Err(DomainError::Validation(ref msg)) if msg.contains("Only Text items can have text_rich or text_animation")
        ),
        "Project::validate must reject project with corrupted video item"
    );
}
