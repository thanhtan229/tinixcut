//! Phase 16 Research POC — Group Move Transaction Semantics & Safety
//! Tests the 10 critical POC cases for group move via existing CommandService,
//! plus multi-delete and multi-duplicate batching safety.

use tinixcut_lib::modules::command::{
    CommandEnvelope, CommandError, CommandOperation, CommandService, OperationEnvelope,
};
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    Asset, CutTransition, ItemKind, MediaKind, Project, TimelineItem, Track, TrackKind,
    TransitionKind, CURRENT_SCHEMA_VERSION,
};

fn create_test_project_with_asset(fps: u32) -> (Project, String) {
    let mut project = Project::new("POC 16 Test".to_string());
    project.settings.frame_rate = FrameRate::new(fps, 1);
    let asset = Asset {
        asset_id: "asset-1".to_string(),
        name: "test.mp4".to_string(),
        path: "C:/test.mp4".to_string(),
        media_kind: MediaKind::Video,
        file_size: 1024 * 1024,
        dimensions: None,
        duration: Some(RationalTime::new(3600, FrameRate::new(fps, 1))),
    };
    project.assets.push(asset);
    (project, "asset-1".to_string())
}

fn create_clip(
    item_id: &str,
    asset_id: &str,
    start_sec: i64,
    dur_sec: i64,
    fps: u32,
) -> TimelineItem {
    let rate = FrameRate::new(fps, 1);
    let t_range = TimeRange::new(
        RationalTime::new(start_sec * fps as i64, rate),
        RationalTime::new(dur_sec * fps as i64, rate),
    );
    let s_range = TimeRange::new(
        RationalTime::new(0, rate),
        RationalTime::new(dur_sec * fps as i64, rate),
    );
    let mut item = TimelineItem::new(
        Some(asset_id.to_string()),
        item_id.to_string(),
        ItemKind::Video,
        t_range,
        s_range,
    )
    .unwrap();
    item.item_id = item_id.to_string();
    item
}

fn make_envelope(
    project_id: &str,
    base_revision: u64,
    ops: Vec<(&str, CommandOperation)>,
) -> CommandEnvelope {
    CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project_id.to_string(),
        base_revision,
        client_id: None,
        client_epoch: None,
        sequence_number: None,
        dry_run: false,
        transaction_id: Some("tx-poc16".to_string()),
        operations: ops
            .into_iter()
            .map(|(op_id, op)| OperationEnvelope {
                operation_id: op_id.to_string(),
                operation: op,
            })
            .collect(),
    }
}

#[test]
fn test_case_1_two_adjacent_move_right_order_sensitivity() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    let clip_a = create_clip("clip_a", &asset_id, 0, 5, fps);
    let clip_b = create_clip("clip_b", &asset_id, 5, 5, fps);
    track.items.push(clip_a);
    track.items.push(clip_b);
    project.tracks.push(track);
    let rate = FrameRate::new(fps, 1);

    // Goal: Move group right by +2s -> A to 2s [2, 7s), B to 7s [7, 12s)
    // Order 1: A then B (natural left-to-right order)
    let mut svc_natural = CommandService::new(project.clone());
    let env_ab = make_envelope(
        &project.project_id,
        project.revision,
        vec![
            (
                "op1",
                CommandOperation::ClipMove {
                    item_id: "clip_a".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(2 * fps as i64, rate),
                },
            ),
            (
                "op2",
                CommandOperation::ClipMove {
                    item_id: "clip_b".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(7 * fps as i64, rate),
                },
            ),
        ],
    );
    let res_natural = svc_natural.execute(env_ab);
    println!(
        "Case 1 Natural (A then B) Result: {:?}",
        res_natural.is_ok()
    );
    assert!(
        res_natural.is_err(),
        "Natural left-to-right order MUST fail with intermediate overlap error when moving right!"
    );

    // Order 2: B then A (right-to-left order)
    let mut svc_reverse = CommandService::new(project.clone());
    let env_ba = make_envelope(
        &project.project_id,
        project.revision,
        vec![
            (
                "op1",
                CommandOperation::ClipMove {
                    item_id: "clip_b".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(7 * fps as i64, rate),
                },
            ),
            (
                "op2",
                CommandOperation::ClipMove {
                    item_id: "clip_a".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(2 * fps as i64, rate),
                },
            ),
        ],
    );
    let res_reverse = svc_reverse.execute(env_ba);
    println!(
        "Case 1 Reverse (B then A) Result: {:?}",
        res_reverse.is_ok()
    );
    assert!(
        res_reverse.is_ok(),
        "Reverse right-to-left order succeeds when moving right!"
    );
}

#[test]
fn test_case_2_two_adjacent_move_left_order_sensitivity() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    let clip_a = create_clip("clip_a", &asset_id, 2, 5, fps);
    let clip_b = create_clip("clip_b", &asset_id, 7, 5, fps);
    track.items.push(clip_a);
    track.items.push(clip_b);
    project.tracks.push(track);
    let rate = FrameRate::new(fps, 1);

    // Goal: Move group left by -2s -> A to 0s [0, 5s), B to 5s [5, 10s)
    // If we use B then A (the order that worked for moving right):
    let mut svc_ba = CommandService::new(project.clone());
    let env_ba = make_envelope(
        &project.project_id,
        project.revision,
        vec![
            (
                "op1",
                CommandOperation::ClipMove {
                    item_id: "clip_b".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(5 * fps as i64, rate),
                },
            ),
            (
                "op2",
                CommandOperation::ClipMove {
                    item_id: "clip_a".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(0, rate),
                },
            ),
        ],
    );
    let res_ba = svc_ba.execute(env_ba);
    assert!(
        res_ba.is_err(),
        "Right-to-left order MUST fail with intermediate overlap when moving left!"
    );

    // If we use A then B (left-to-right order):
    let mut svc_ab = CommandService::new(project.clone());
    let env_ab = make_envelope(
        &project.project_id,
        project.revision,
        vec![
            (
                "op1",
                CommandOperation::ClipMove {
                    item_id: "clip_a".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(0, rate),
                },
            ),
            (
                "op2",
                CommandOperation::ClipMove {
                    item_id: "clip_b".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(5 * fps as i64, rate),
                },
            ),
        ],
    );
    let res_ab = svc_ab.execute(env_ab);
    assert!(
        res_ab.is_ok(),
        "Left-to-right order succeeds when moving left!"
    );
}

#[test]
fn test_case_3_and_4_three_adjacent_clips_move_right_and_left() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    track
        .items
        .push(create_clip("clip_a", &asset_id, 0, 4, fps));
    track
        .items
        .push(create_clip("clip_b", &asset_id, 4, 4, fps));
    track
        .items
        .push(create_clip("clip_c", &asset_id, 8, 4, fps));
    project.tracks.push(track);
    let rate = FrameRate::new(fps, 1);

    // Case 3: Move right +3s -> Natural order (A, B, C) fails!
    let mut svc_nat = CommandService::new(project.clone());
    let env_nat = make_envelope(
        &project.project_id,
        project.revision,
        vec![
            (
                "op1",
                CommandOperation::ClipMove {
                    item_id: "clip_a".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(3 * fps as i64, rate),
                },
            ),
            (
                "op2",
                CommandOperation::ClipMove {
                    item_id: "clip_b".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(7 * fps as i64, rate),
                },
            ),
            (
                "op3",
                CommandOperation::ClipMove {
                    item_id: "clip_c".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(11 * fps as i64, rate),
                },
            ),
        ],
    );
    assert!(
        svc_nat.execute(env_nat).is_err(),
        "Natural order fails for 3 clips moving right"
    );

    // Reverse order (C, B, A) succeeds:
    let mut svc_rev = CommandService::new(project.clone());
    let env_rev = make_envelope(
        &project.project_id,
        project.revision,
        vec![
            (
                "op1",
                CommandOperation::ClipMove {
                    item_id: "clip_c".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(11 * fps as i64, rate),
                },
            ),
            (
                "op2",
                CommandOperation::ClipMove {
                    item_id: "clip_b".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(7 * fps as i64, rate),
                },
            ),
            (
                "op3",
                CommandOperation::ClipMove {
                    item_id: "clip_a".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(3 * fps as i64, rate),
                },
            ),
        ],
    );
    assert!(
        svc_rev.execute(env_rev).is_ok(),
        "Reverse order succeeds for 3 clips moving right"
    );

    // Case 4: Move left -3s (from 3, 7, 11 back to 0, 4, 8)
    let project_shifted = svc_rev.project().clone();
    // Reverse order (C, B, A) fails when moving left:
    let mut svc_left_rev = CommandService::new(project_shifted.clone());
    let env_left_rev = make_envelope(
        &project_shifted.project_id,
        project_shifted.revision,
        vec![
            (
                "op_rev1",
                CommandOperation::ClipMove {
                    item_id: "clip_c".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(8 * fps as i64, rate),
                },
            ),
            (
                "op_rev2",
                CommandOperation::ClipMove {
                    item_id: "clip_b".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(4 * fps as i64, rate),
                },
            ),
            (
                "op_rev3",
                CommandOperation::ClipMove {
                    item_id: "clip_a".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(0, rate),
                },
            ),
        ],
    );
    assert!(
        svc_left_rev.execute(env_left_rev).is_err(),
        "Reverse order fails for 3 clips moving left"
    );

    // Natural order (A, B, C) succeeds when moving left:
    let mut svc_left_nat = CommandService::new(project_shifted.clone());
    let env_left_nat = make_envelope(
        &project_shifted.project_id,
        project_shifted.revision,
        vec![
            (
                "op_nat1",
                CommandOperation::ClipMove {
                    item_id: "clip_a".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(0, rate),
                },
            ),
            (
                "op_nat2",
                CommandOperation::ClipMove {
                    item_id: "clip_b".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(4 * fps as i64, rate),
                },
            ),
            (
                "op_nat3",
                CommandOperation::ClipMove {
                    item_id: "clip_c".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(8 * fps as i64, rate),
                },
            ),
        ],
    );
    assert!(
        svc_left_nat.execute(env_left_nat).is_ok(),
        "Natural order succeeds for 3 clips moving left"
    );
}

#[test]
fn test_case_5_clips_separated_by_gaps() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    // Clip A [0, 4), Gap [4, 10), Clip B [10, 14)
    track
        .items
        .push(create_clip("clip_a", &asset_id, 0, 4, fps));
    track
        .items
        .push(create_clip("clip_b", &asset_id, 10, 4, fps));
    project.tracks.push(track);
    let rate = FrameRate::new(fps, 1);

    // Delta < gap: +2s shift -> A [2, 6) doesn't overlap B [10, 14)
    let mut svc = CommandService::new(project.clone());
    let env = make_envelope(
        &project.project_id,
        project.revision,
        vec![
            (
                "op1",
                CommandOperation::ClipMove {
                    item_id: "clip_a".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(2 * fps as i64, rate),
                },
            ),
            (
                "op2",
                CommandOperation::ClipMove {
                    item_id: "clip_b".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(12 * fps as i64, rate),
                },
            ),
        ],
    );
    assert!(svc.execute(env).is_ok());

    // Delta >= gap: +7s shift -> A [7, 11) overlaps B [10, 14)!
    let mut svc2 = CommandService::new(project.clone());
    let env2 = make_envelope(
        &project.project_id,
        project.revision,
        vec![
            (
                "op1",
                CommandOperation::ClipMove {
                    item_id: "clip_a".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(7 * fps as i64, rate),
                },
            ),
            (
                "op2",
                CommandOperation::ClipMove {
                    item_id: "clip_b".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(17 * fps as i64, rate),
                },
            ),
        ],
    );
    assert!(
        svc2.execute(env2).is_err(),
        "Natural order fails when delta >= gap!"
    );
}

#[test]
fn test_case_6_clips_on_different_tracks() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut t1 = Track::new("Track 1".to_string(), TrackKind::Video);
    let mut t2 = Track::new("Track 2".to_string(), TrackKind::Audio);
    t1.items.push(create_clip("clip_v", &asset_id, 0, 5, fps));
    t2.items.push(create_clip("clip_a", &asset_id, 0, 5, fps));
    project.tracks.push(t1);
    project.tracks.push(t2);
    let rate = FrameRate::new(fps, 1);

    let mut svc = CommandService::new(project.clone());
    let env = make_envelope(
        &project.project_id,
        project.revision,
        vec![
            (
                "op1",
                CommandOperation::ClipMove {
                    item_id: "clip_v".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(3 * fps as i64, rate),
                },
            ),
            (
                "op2",
                CommandOperation::ClipMove {
                    item_id: "clip_a".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(3 * fps as i64, rate),
                },
            ),
        ],
    );
    assert!(
        svc.execute(env).is_ok(),
        "Cross-track moves have no inter-track overlap conflict"
    );
}

#[test]
fn test_case_7_group_next_to_unselected_clip_conflict_rejection() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    track
        .items
        .push(create_clip("clip_a", &asset_id, 0, 4, fps));
    track
        .items
        .push(create_clip("clip_b", &asset_id, 4, 4, fps));
    track
        .items
        .push(create_clip("clip_c", &asset_id, 10, 5, fps));
    project.tracks.push(track);
    let rate = FrameRate::new(fps, 1);

    let mut svc = CommandService::new(project.clone());
    let env = make_envelope(
        &project.project_id,
        project.revision,
        vec![
            (
                "op1",
                CommandOperation::ClipMove {
                    item_id: "clip_b".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(8 * fps as i64, rate),
                },
            ),
            (
                "op2",
                CommandOperation::ClipMove {
                    item_id: "clip_a".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(4 * fps as i64, rate),
                },
            ),
        ],
    );
    let res = svc.execute(env);
    assert!(res.is_err(), "Must reject conflict with unselected clip!");
    assert_eq!(
        svc.project().tracks[0].items[0].timeline_range.start.value,
        0
    );
    assert_eq!(
        svc.project().tracks[0].items[1].timeline_range.start.value,
        4 * fps as i64
    );
}

#[test]
#[allow(non_snake_case)]
fn test_case_8_selected_clips_with_cross_dissolve_FATAL_TRANSITION_DESTRUCTION() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    let track_id = track.track_id.clone();
    let clip_a = create_clip("clip_a", &asset_id, 0, 5, fps);
    let clip_b = create_clip("clip_b", &asset_id, 5, 5, fps);
    track.items.push(clip_a);
    track.items.push(clip_b);
    let rate = FrameRate::new(fps, 1);

    let trans = CutTransition {
        transition_id: "trans-ab".to_string(),
        track_id: track_id.clone(),
        left_item_id: "clip_a".to_string(),
        right_item_id: "clip_b".to_string(),
        kind: TransitionKind::CrossDissolve,
        duration: RationalTime::new(30, rate), // 1 second
    };
    track.transitions.push(trans);
    project.tracks.push(track);

    assert_eq!(project.tracks[0].transitions.len(), 1);

    // Attempt to move both right by +2s using the reverse order (B then A)
    let mut svc = CommandService::new(project.clone());
    let env = make_envelope(
        &project.project_id,
        project.revision,
        vec![
            (
                "op1",
                CommandOperation::ClipMove {
                    item_id: "clip_b".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(7 * fps as i64, rate),
                },
            ),
            (
                "op2",
                CommandOperation::ClipMove {
                    item_id: "clip_a".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(2 * fps as i64, rate),
                },
            ),
        ],
    );
    let res = svc.execute(env);
    assert!(res.is_ok(), "Moves succeeded in reverse order");

    let committed_transitions = &svc.project().tracks[0].transitions;
    println!(
        "Remaining transitions count: {}",
        committed_transitions.len()
    );
    // Proves that existing ClipMove destroys the CrossDissolve transition!
    assert!(
        committed_transitions.is_empty(),
        "PROOF OF DESTRUCTIVE BEHAVIOR: Transition was silently deleted during intermediate step of multi-clip move!"
    );
}

#[test]
fn test_case_9_stale_revision() {
    let fps = 30;
    let (project, _asset_id) = create_test_project_with_asset(fps);
    let mut svc = CommandService::new(project.clone());
    let rate = FrameRate::new(fps, 1);

    let env = make_envelope(
        &project.project_id,
        project.revision + 99,
        vec![(
            "op1",
            CommandOperation::ClipMove {
                item_id: "clip_a".to_string(),
                target_track_id: None,
                target_start: RationalTime::new(2 * fps as i64, rate),
            },
        )],
    );
    let res = svc.execute(env);
    assert!(matches!(res, Err(CommandError::StaleRevision { .. })));
}

#[test]
fn test_case_10_transaction_rollback_when_one_member_invalid() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    track
        .items
        .push(create_clip("clip_a", &asset_id, 0, 5, fps));
    track
        .items
        .push(create_clip("clip_b", &asset_id, 5, 5, fps));
    project.tracks.push(track);
    let initial_rev = project.revision;
    let rate = FrameRate::new(fps, 1);

    let mut svc = CommandService::new(project.clone());
    let env = make_envelope(
        &project.project_id,
        initial_rev,
        vec![
            (
                "op1",
                CommandOperation::ClipMove {
                    item_id: "clip_b".to_string(),
                    target_track_id: None,
                    target_start: RationalTime::new(10 * fps as i64, rate),
                },
            ),
            (
                "op2",
                CommandOperation::ClipMove {
                    item_id: "clip_a".to_string(),
                    target_track_id: Some("nonexistent_track".to_string()),
                    target_start: RationalTime::new(2 * fps as i64, rate),
                },
            ),
        ],
    );
    let res = svc.execute(env);
    assert!(res.is_err(), "Transaction must fail");
    assert_eq!(svc.project().revision, initial_rev);
    assert_eq!(
        svc.project().tracks[0].items[1].timeline_range.start.value,
        5 * fps as i64
    );
}

#[test]
fn test_multi_selection_delete_safety() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    let track_id = track.track_id.clone();
    track
        .items
        .push(create_clip("clip_a", &asset_id, 0, 5, fps));
    track
        .items
        .push(create_clip("clip_b", &asset_id, 5, 5, fps));
    track
        .items
        .push(create_clip("clip_c", &asset_id, 10, 5, fps));
    let rate = FrameRate::new(fps, 1);
    track.transitions.push(CutTransition {
        transition_id: "trans_ab".to_string(),
        track_id: track_id.clone(),
        left_item_id: "clip_a".to_string(),
        right_item_id: "clip_b".to_string(),
        kind: TransitionKind::CrossDissolve,
        duration: RationalTime::new(30, rate),
    });
    project.tracks.push(track);

    let mut svc = CommandService::new(project.clone());
    let env = make_envelope(
        &project.project_id,
        project.revision,
        vec![
            (
                "op1",
                CommandOperation::ClipDelete {
                    item_id: "clip_a".to_string(),
                },
            ),
            (
                "op2",
                CommandOperation::ClipDelete {
                    item_id: "clip_b".to_string(),
                },
            ),
        ],
    );
    let res = svc.execute(env);
    assert!(
        res.is_ok(),
        "Multi-delete of adjacent clips with transition succeeds"
    );
    assert_eq!(svc.project().tracks[0].items.len(), 1);
    assert_eq!(svc.project().tracks[0].items[0].item_id, "clip_c");
    assert_eq!(svc.project().tracks[0].transitions.len(), 0);

    let mut svc2 = CommandService::new(project.clone());
    let env2 = make_envelope(
        &project.project_id,
        project.revision,
        vec![
            (
                "op1",
                CommandOperation::ClipDelete {
                    item_id: "clip_b".to_string(),
                },
            ),
            (
                "op2",
                CommandOperation::ClipDelete {
                    item_id: "clip_a".to_string(),
                },
            ),
        ],
    );
    let res2 = svc2.execute(env2);
    assert!(res2.is_ok(), "Multi-delete is order-independent");
}

#[test]
fn test_multi_selection_duplicate_safety() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    track
        .items
        .push(create_clip("clip_a", &asset_id, 0, 5, fps));
    track
        .items
        .push(create_clip("clip_b", &asset_id, 5, 5, fps));
    project.tracks.push(track);

    let mut svc = CommandService::new(project.clone());
    let env = make_envelope(
        &project.project_id,
        project.revision,
        vec![(
            "op1",
            CommandOperation::ClipDuplicate {
                item_id: "clip_a".to_string(),
                target_start: None,
                target_track_id: None,
                new_item_id: None,
            },
        )],
    );
    let res = svc.execute(env);
    assert!(
        res.is_err(),
        "Default ClipDuplicate without target_start on adjacent clip causes collision with downstream clip!"
    );
}

#[test]
fn test_ripple_delete_candidate_semantics_and_invariants() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    let track_id = track.track_id.clone();
    let rate = FrameRate::new(fps, 1);

    // Setup:
    // A [0, 5), B [5, 10), C [10, 15), D [18, 22), E [22, 26)
    // Transitions:
    // trans_ab between A & B
    // trans_bc between B & C
    // trans_de between D & E
    track
        .items
        .push(create_clip("clip_a", &asset_id, 0, 5, fps));
    track
        .items
        .push(create_clip("clip_b", &asset_id, 5, 5, fps));
    track
        .items
        .push(create_clip("clip_c", &asset_id, 10, 5, fps));
    track
        .items
        .push(create_clip("clip_d", &asset_id, 18, 4, fps));
    track
        .items
        .push(create_clip("clip_e", &asset_id, 22, 4, fps));

    track.transitions.push(CutTransition {
        transition_id: "trans_ab".to_string(),
        track_id: track_id.clone(),
        left_item_id: "clip_a".to_string(),
        right_item_id: "clip_b".to_string(),
        kind: TransitionKind::CrossDissolve,
        duration: RationalTime::new(30, rate),
    });
    track.transitions.push(CutTransition {
        transition_id: "trans_bc".to_string(),
        track_id: track_id.clone(),
        left_item_id: "clip_b".to_string(),
        right_item_id: "clip_c".to_string(),
        kind: TransitionKind::CrossDissolve,
        duration: RationalTime::new(30, rate),
    });
    track.transitions.push(CutTransition {
        transition_id: "trans_de".to_string(),
        track_id: track_id.clone(),
        left_item_id: "clip_d".to_string(),
        right_item_id: "clip_e".to_string(),
        kind: TransitionKind::CrossDissolve,
        duration: RationalTime::new(30, rate),
    });
    project.tracks.push(track);

    // Verify initial project is valid
    assert!(project.validate().is_ok());

    // SIMULATE ClipRippleDelete for "clip_b" (middle clip with incoming AND outgoing transitions, plus downstream gap and downstream transition):
    let mut proj = project.clone();
    let target_item_id = "clip_b";

    // 1. Locate item and track
    let mut found_pos = None;
    let mut found_track_idx = None;
    for (t_idx, tr) in proj.tracks.iter().enumerate() {
        if let Some(pos) = tr.items.iter().position(|i| i.item_id == target_item_id) {
            found_pos = Some(pos);
            found_track_idx = Some(t_idx);
            break;
        }
    }
    let (t_idx, item_pos) = (found_track_idx.unwrap(), found_pos.unwrap());
    {
        let tr = &mut proj.tracks[t_idx];
        assert!(!tr.locked, "Locked track check");

        let deleted_item = tr.items.remove(item_pos);
        let deleted_end = deleted_item.timeline_range.end();
        let deleted_duration = deleted_item.timeline_range.duration;

        // 5. Remove directly dependent transitions
        tr.transitions
            .retain(|t| t.left_item_id != target_item_id && t.right_item_id != target_item_id);

        // 6. Shift surviving items on SAME track where timeline_start >= deleted_end
        for item in &mut tr.items {
            let item_start_aligned = item.timeline_range.start.rescaled_to(deleted_duration.rate);
            let deleted_end_aligned = deleted_end.rescaled_to(deleted_duration.rate);
            if item_start_aligned >= deleted_end_aligned {
                let shift_dur = deleted_duration.rescaled_to(item.timeline_range.start.rate);
                let new_start = item.timeline_range.start - shift_dur;
                item.timeline_range = TimeRange::new(new_start, item.timeline_range.duration);
            }
        }

        // 11. Downstream transitions: check trans_de
        assert_eq!(tr.transitions.len(), 1);
        assert_eq!(tr.transitions[0].transition_id, "trans_de");
    }

    // Validate the resulting project against canonical domain rules!
    let val_res = proj.validate();
    assert!(
        val_res.is_ok(),
        "Ripple delete result MUST validate cleanly against domain rules: {:?}",
        val_res.err()
    );

    let tr = &proj.tracks[t_idx];
    // Check resulting positions:
    // A: [0, 5) unchanged
    let a = tr.find_item("clip_a").unwrap();
    assert_eq!(a.timeline_range.start.value, 0);
    assert_eq!(a.timeline_range.duration.value, 5 * fps as i64);

    // C: shifted from [10, 15) to [5, 10)
    let c = tr.find_item("clip_c").unwrap();
    assert_eq!(c.timeline_range.start.value, 5 * fps as i64);
    assert_eq!(c.timeline_range.duration.value, 5 * fps as i64);

    // D: shifted from [18, 22) to [13, 17) -> gap between C and D was 3s (18-15), now is 3s (13-10)
    let d = tr.find_item("clip_d").unwrap();
    assert_eq!(d.timeline_range.start.value, 13 * fps as i64);
    assert_eq!(d.timeline_range.duration.value, 4 * fps as i64);

    // E: shifted from [22, 26) to [17, 21)
    let e = tr.find_item("clip_e").unwrap();
    assert_eq!(e.timeline_range.start.value, 17 * fps as i64);
    assert_eq!(e.timeline_range.duration.value, 4 * fps as i64);

    // Adjacent D & E with trans_de: still perfectly adjacent!
    assert!(d
        .timeline_range
        .is_adjacent_to_with_rate(&e.timeline_range, rate));
}
