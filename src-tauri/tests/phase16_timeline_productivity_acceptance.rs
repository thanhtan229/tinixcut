//! Phase 16 Acceptance Test Suite — Timeline Editing Productivity
//! Validates P16-AC-21..P16-AC-41, P16-AC-51..P16-AC-54:
//! - ClipGroupMove simultaneous validation and execution
//! - ClipRippleDelete single-track downstream shift and transition preservation
//! - Multi-delete atomic envelope
//! - Primary duplicate
//! - Automation generic compatibility and supported operations

use tinixcut_lib::modules::automation::mcp::TinixCutMcpServer;
use tinixcut_lib::modules::automation::{AutomationAdapter, AutomationSession};
use tinixcut_lib::modules::command::{
    CommandEnvelope, CommandError, CommandOperation, CommandService, DiffChange, OperationEnvelope,
};
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    Asset, CutTransition, ItemKind, MediaKind, Project, TimelineItem, Track, TrackKind,
    TransitionKind, CURRENT_SCHEMA_VERSION,
};
use tinixcut_lib::modules::history::{HistoryEntry, HistoryManager};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader as TokioBufReader};

fn create_test_project_with_asset(fps: u32) -> (Project, String) {
    let mut project = Project::new("Phase 16 Test Project".to_string());
    project.settings.frame_rate = FrameRate::new(fps, 1);
    let asset = Asset {
        asset_id: "asset-1".to_string(),
        name: "test_media.mp4".to_string(),
        path: "C:/media/test_media.mp4".to_string(),
        media_kind: MediaKind::Video,
        file_size: 10 * 1024 * 1024,
        dimensions: None,
        duration: Some(RationalTime::new(7200, FrameRate::new(fps, 1))), // 240 seconds
    };
    project.assets.push(asset);
    (project, "asset-1".to_string())
}

fn create_clip(
    item_id: &str,
    asset_id: &str,
    start_sec: i64,
    dur_sec: i64,
    source_start_sec: i64,
    fps: u32,
) -> TimelineItem {
    let rate = FrameRate::new(fps, 1);
    let t_range = TimeRange::new(
        RationalTime::new(start_sec * fps as i64, rate),
        RationalTime::new(dur_sec * fps as i64, rate),
    );
    let s_range = TimeRange::new(
        RationalTime::new(source_start_sec * fps as i64, rate),
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
        transaction_id: Some(format!("tx-p16-{}", base_revision)),
        operations: ops
            .into_iter()
            .map(|(op_id, op)| OperationEnvelope {
                operation_id: op_id.to_string(),
                operation: op,
            })
            .collect(),
    }
}

// =========================================================================
// P16-AC-21..P16-AC-31: ClipGroupMove Tests
// =========================================================================

#[test]
fn test_p16_group_move_two_adjacent_same_track_move_right() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    track
        .items
        .push(create_clip("clip_a", &asset_id, 0, 5, 0, fps));
    track
        .items
        .push(create_clip("clip_b", &asset_id, 5, 5, 5, fps));
    project.tracks.push(track);
    let rate = FrameRate::new(fps, 1);

    let mut svc = CommandService::new(project.clone());
    let env = make_envelope(
        &project.project_id,
        project.revision,
        vec![(
            "op_gm_1",
            CommandOperation::ClipGroupMove {
                item_ids: vec!["clip_a".to_string(), "clip_b".to_string()],
                delta: RationalTime::new(2 * fps as i64, rate),
            },
        )],
    );

    let res = svc.execute(env);
    assert!(
        res.is_ok(),
        "ClipGroupMove must succeed moving two adjacent clips right simultaneously"
    );

    let p = svc.project();
    let tr = &p.tracks[0];
    assert_eq!(
        tr.find_item("clip_a").unwrap().timeline_range.start.value,
        2 * fps as i64
    );
    assert_eq!(
        tr.find_item("clip_b").unwrap().timeline_range.start.value,
        7 * fps as i64
    );
    assert_eq!(p.revision, 1);
}

#[test]
fn test_p16_group_move_two_adjacent_same_track_move_left() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    track
        .items
        .push(create_clip("clip_a", &asset_id, 2, 5, 0, fps));
    track
        .items
        .push(create_clip("clip_b", &asset_id, 7, 5, 5, fps));
    project.tracks.push(track);
    let rate = FrameRate::new(fps, 1);

    let mut svc = CommandService::new(project.clone());
    let env = make_envelope(
        &project.project_id,
        project.revision,
        vec![(
            "op_gm_2",
            CommandOperation::ClipGroupMove {
                item_ids: vec!["clip_a".to_string(), "clip_b".to_string()],
                delta: RationalTime::new(-2 * fps as i64, rate),
            },
        )],
    );

    let res = svc.execute(env);
    assert!(
        res.is_ok(),
        "ClipGroupMove must succeed moving two adjacent clips left simultaneously"
    );

    let p = svc.project();
    let tr = &p.tracks[0];
    assert_eq!(
        tr.find_item("clip_a").unwrap().timeline_range.start.value,
        0
    );
    assert_eq!(
        tr.find_item("clip_b").unwrap().timeline_range.start.value,
        5 * fps as i64
    );
}

#[test]
fn test_p16_group_move_three_adjacent_both_directions() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    track
        .items
        .push(create_clip("clip_a", &asset_id, 3, 3, 0, fps));
    track
        .items
        .push(create_clip("clip_b", &asset_id, 6, 3, 3, fps));
    track
        .items
        .push(create_clip("clip_c", &asset_id, 9, 3, 6, fps));
    project.tracks.push(track);
    let rate = FrameRate::new(fps, 1);

    let mut svc = CommandService::new(project.clone());
    // Move right by +3s
    let env_r = make_envelope(
        &project.project_id,
        project.revision,
        vec![(
            "op_gm_r",
            CommandOperation::ClipGroupMove {
                item_ids: vec![
                    "clip_a".to_string(),
                    "clip_b".to_string(),
                    "clip_c".to_string(),
                ],
                delta: RationalTime::new(3 * fps as i64, rate),
            },
        )],
    );
    assert!(svc.execute(env_r).is_ok());
    assert_eq!(
        svc.project().tracks[0]
            .find_item("clip_a")
            .unwrap()
            .timeline_range
            .start
            .value,
        6 * fps as i64
    );
    assert_eq!(
        svc.project().tracks[0]
            .find_item("clip_b")
            .unwrap()
            .timeline_range
            .start
            .value,
        9 * fps as i64
    );
    assert_eq!(
        svc.project().tracks[0]
            .find_item("clip_c")
            .unwrap()
            .timeline_range
            .start
            .value,
        12 * fps as i64
    );

    // Move left by -6s to origin
    let env_l = make_envelope(
        &project.project_id,
        svc.project().revision,
        vec![(
            "op_gm_l",
            CommandOperation::ClipGroupMove {
                item_ids: vec![
                    "clip_a".to_string(),
                    "clip_b".to_string(),
                    "clip_c".to_string(),
                ],
                delta: RationalTime::new(-6 * fps as i64, rate),
            },
        )],
    );
    assert!(svc.execute(env_l).is_ok());
    assert_eq!(
        svc.project().tracks[0]
            .find_item("clip_a")
            .unwrap()
            .timeline_range
            .start
            .value,
        0
    );
    assert_eq!(
        svc.project().tracks[0]
            .find_item("clip_b")
            .unwrap()
            .timeline_range
            .start
            .value,
        3 * fps as i64
    );
    assert_eq!(
        svc.project().tracks[0]
            .find_item("clip_c")
            .unwrap()
            .timeline_range
            .start
            .value,
        6 * fps as i64
    );
}

#[test]
fn test_p16_group_move_gap_separated_selection_preserves_relative_spacing() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    track
        .items
        .push(create_clip("clip_a", &asset_id, 0, 4, 0, fps));
    track
        .items
        .push(create_clip("clip_b", &asset_id, 10, 4, 4, fps)); // 6s gap
    project.tracks.push(track);
    let rate = FrameRate::new(fps, 1);

    let mut svc = CommandService::new(project.clone());
    let env = make_envelope(
        &project.project_id,
        project.revision,
        vec![(
            "op_gm_gap",
            CommandOperation::ClipGroupMove {
                item_ids: vec!["clip_a".to_string(), "clip_b".to_string()],
                delta: RationalTime::new(5 * fps as i64, rate),
            },
        )],
    );
    assert!(svc.execute(env).is_ok());
    let tr = &svc.project().tracks[0];
    assert_eq!(
        tr.find_item("clip_a").unwrap().timeline_range.start.value,
        5 * fps as i64
    );
    assert_eq!(
        tr.find_item("clip_b").unwrap().timeline_range.start.value,
        15 * fps as i64
    );
}

#[test]
fn test_p16_group_move_cross_track() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track1 = Track::new("Track 1".to_string(), TrackKind::Video);
    let mut track2 = Track::new("Track 2".to_string(), TrackKind::Video);
    track1
        .items
        .push(create_clip("clip_1", &asset_id, 0, 5, 0, fps));
    track2
        .items
        .push(create_clip("clip_2", &asset_id, 2, 5, 5, fps));
    project.tracks.push(track1);
    project.tracks.push(track2);
    let rate = FrameRate::new(fps, 1);

    let mut svc = CommandService::new(project.clone());
    let env = make_envelope(
        &project.project_id,
        project.revision,
        vec![(
            "op_gm_cross",
            CommandOperation::ClipGroupMove {
                item_ids: vec!["clip_1".to_string(), "clip_2".to_string()],
                delta: RationalTime::new(3 * fps as i64, rate),
            },
        )],
    );
    assert!(svc.execute(env).is_ok());
    assert_eq!(
        svc.project().tracks[0].items[0].timeline_range.start.value,
        3 * fps as i64
    );
    assert_eq!(
        svc.project().tracks[1].items[0].timeline_range.start.value,
        5 * fps as i64
    );
    // Tracks remain unchanged
    assert_eq!(svc.project().tracks[0].items[0].item_id, "clip_1");
    assert_eq!(svc.project().tracks[1].items[0].item_id, "clip_2");
}

#[test]
fn test_p16_group_move_collision_with_unselected_clip_rejects() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    track
        .items
        .push(create_clip("clip_a", &asset_id, 0, 4, 0, fps));
    track
        .items
        .push(create_clip("clip_b", &asset_id, 4, 4, 4, fps));
    track
        .items
        .push(create_clip("clip_c_stationary", &asset_id, 10, 5, 8, fps));
    project.tracks.push(track);
    let rate = FrameRate::new(fps, 1);

    let mut svc = CommandService::new(project.clone());
    // Move A & B right by +4s -> B would land at 8s..12s, colliding with clip_c at 10s..15s
    let env = make_envelope(
        &project.project_id,
        project.revision,
        vec![(
            "op_gm_coll",
            CommandOperation::ClipGroupMove {
                item_ids: vec!["clip_a".to_string(), "clip_b".to_string()],
                delta: RationalTime::new(4 * fps as i64, rate),
            },
        )],
    );
    let res = svc.execute(env);
    assert!(
        res.is_err(),
        "Must reject collision with unselected stationary clip"
    );
    assert_eq!(
        svc.project().revision,
        0,
        "Revision must remain unchanged on rejection"
    );
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
fn test_p16_group_move_negative_boundary_rejects() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    track
        .items
        .push(create_clip("clip_a", &asset_id, 2, 5, 0, fps));
    track
        .items
        .push(create_clip("clip_b", &asset_id, 7, 5, 5, fps));
    project.tracks.push(track);
    let rate = FrameRate::new(fps, 1);

    let mut svc = CommandService::new(project.clone());
    // Delta of -3s would push clip_a from 2s to -1s
    let env = make_envelope(
        &project.project_id,
        project.revision,
        vec![(
            "op_gm_neg",
            CommandOperation::ClipGroupMove {
                item_ids: vec!["clip_a".to_string(), "clip_b".to_string()],
                delta: RationalTime::new(-3 * fps as i64, rate),
            },
        )],
    );
    let res = svc.execute(env);
    assert!(res.is_err(), "Must reject negative start");
    assert_eq!(svc.project().revision, 0);
}

#[test]
fn test_p16_group_move_locked_track_rejects() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    track
        .items
        .push(create_clip("clip_a", &asset_id, 0, 5, 0, fps));
    track.locked = true;
    project.tracks.push(track);
    let rate = FrameRate::new(fps, 1);

    let mut svc = CommandService::new(project.clone());
    let env = make_envelope(
        &project.project_id,
        project.revision,
        vec![(
            "op_gm_lock",
            CommandOperation::ClipGroupMove {
                item_ids: vec!["clip_a".to_string()],
                delta: RationalTime::new(2 * fps as i64, rate),
            },
        )],
    );
    let res = svc.execute(env);
    assert!(res.is_err(), "Must reject move on locked track");
}

#[test]
fn test_p16_group_move_empty_and_duplicate_and_missing_ids_reject() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    track
        .items
        .push(create_clip("clip_a", &asset_id, 0, 5, 0, fps));
    project.tracks.push(track);
    let rate = FrameRate::new(fps, 1);

    let mut svc = CommandService::new(project.clone());

    // Empty
    let env_empty = make_envelope(
        &project.project_id,
        project.revision,
        vec![(
            "op_empty",
            CommandOperation::ClipGroupMove {
                item_ids: vec![],
                delta: RationalTime::new(1 * fps as i64, rate),
            },
        )],
    );
    assert!(
        svc.execute(env_empty).is_err(),
        "Empty item_ids must reject"
    );

    // Duplicate
    let env_dup = make_envelope(
        &project.project_id,
        project.revision,
        vec![(
            "op_dup",
            CommandOperation::ClipGroupMove {
                item_ids: vec!["clip_a".to_string(), "clip_a".to_string()],
                delta: RationalTime::new(fps as i64, rate),
            },
        )],
    );
    assert!(
        svc.execute(env_dup).is_err(),
        "Duplicate item_ids must reject"
    );

    // Missing
    let env_missing = make_envelope(
        &project.project_id,
        project.revision,
        vec![(
            "op_missing",
            CommandOperation::ClipGroupMove {
                item_ids: vec!["clip_nonexistent".to_string()],
                delta: RationalTime::new(fps as i64, rate),
            },
        )],
    );
    assert!(
        svc.execute(env_missing).is_err(),
        "Non-existent item_id must reject"
    );
}

#[test]
fn test_p16_group_move_internal_cross_dissolve_preserved() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    let track_id = track.track_id.clone();
    let rate = FrameRate::new(fps, 1);

    // Both clips have sufficient media handles (source_start = 5s for clip_b, duration 5s, asset 240s)
    let clip_a = create_clip("clip_a", &asset_id, 0, 5, 0, fps);
    let clip_b = create_clip("clip_b", &asset_id, 5, 5, 5, fps);
    track.items.push(clip_a);
    track.items.push(clip_b);

    track.transitions.push(CutTransition {
        transition_id: "trans_ab".to_string(),
        track_id: track_id.clone(),
        left_item_id: "clip_a".to_string(),
        right_item_id: "clip_b".to_string(),
        kind: TransitionKind::CrossDissolve,
        duration: RationalTime::new(30, rate), // 1s
    });
    project.tracks.push(track);

    assert_eq!(project.tracks[0].transitions.len(), 1);

    let mut svc = CommandService::new(project.clone());
    let env = make_envelope(
        &project.project_id,
        project.revision,
        vec![(
            "op_gm_trans",
            CommandOperation::ClipGroupMove {
                item_ids: vec!["clip_a".to_string(), "clip_b".to_string()],
                delta: RationalTime::new(3 * fps as i64, rate),
            },
        )],
    );

    let res = svc.execute(env);
    assert!(res.is_ok(), "Simultaneous ClipGroupMove must succeed");

    // Invariant: Internal cut transition survives!
    let committed_transitions = &svc.project().tracks[0].transitions;
    assert_eq!(
        committed_transitions.len(),
        1,
        "CRITICAL INVARIANT P16-AC-28: Internal CrossDissolve MUST survive ClipGroupMove!"
    );
    assert_eq!(committed_transitions[0].transition_id, "trans_ab");
}

#[test]
fn test_p16_group_move_external_transition_removed() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    let track_id = track.track_id.clone();
    let rate = FrameRate::new(fps, 1);

    track
        .items
        .push(create_clip("clip_a_stationary", &asset_id, 0, 5, 0, fps));
    track
        .items
        .push(create_clip("clip_b_moved", &asset_id, 5, 5, 5, fps));

    track.transitions.push(CutTransition {
        transition_id: "trans_ab".to_string(),
        track_id: track_id.clone(),
        left_item_id: "clip_a_stationary".to_string(),
        right_item_id: "clip_b_moved".to_string(),
        kind: TransitionKind::CrossDissolve,
        duration: RationalTime::new(30, rate),
    });
    project.tracks.push(track);

    let mut svc = CommandService::new(project.clone());
    // Move only clip_b_moved right by +2s
    let env = make_envelope(
        &project.project_id,
        project.revision,
        vec![(
            "op_gm_ext",
            CommandOperation::ClipGroupMove {
                item_ids: vec!["clip_b_moved".to_string()],
                delta: RationalTime::new(2 * fps as i64, rate),
            },
        )],
    );

    let res = svc.execute(env);
    assert!(res.is_ok());

    // External transition between stationary clip_a and moved clip_b must be removed because adjacency is broken
    let committed_transitions = &svc.project().tracks[0].transitions;
    assert_eq!(
        committed_transitions.len(),
        0,
        "P16-AC-29: External transition broken by group move MUST be safely removed"
    );
}

#[test]
fn test_p16_group_move_mixed_rates_semantic_rescaling() {
    let (mut project, asset_id) = create_test_project_with_asset(30);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);

    // Item 1 at 30 fps
    track
        .items
        .push(create_clip("clip_30", &asset_id, 0, 5, 0, 30));

    // Item 2 at 60 fps
    let rate60 = FrameRate::new(60, 1);
    let t_range60 = TimeRange::new(
        RationalTime::new(5 * 60, rate60),
        RationalTime::new(5 * 60, rate60),
    );
    let mut item60 = TimelineItem::new(
        Some(asset_id.clone()),
        "clip_60".to_string(),
        ItemKind::Video,
        t_range60,
        t_range60,
    )
    .unwrap();
    item60.item_id = "clip_60".to_string();
    track.items.push(item60);
    project.tracks.push(track);

    let mut svc = CommandService::new(project.clone());
    // Delta in 24 fps rate (1 second = 24 frames)
    let delta = RationalTime::new(24, FrameRate::new(24, 1)); // exactly 1 second

    let env = make_envelope(
        &project.project_id,
        project.revision,
        vec![(
            "op_gm_mixed",
            CommandOperation::ClipGroupMove {
                item_ids: vec!["clip_30".to_string(), "clip_60".to_string()],
                delta,
            },
        )],
    );

    assert!(svc.execute(env).is_ok());
    let tr = &svc.project().tracks[0];
    // clip_30 shifted by 1s (30 frames at 30fps) -> starts at 30
    assert_eq!(
        tr.find_item("clip_30").unwrap().timeline_range.start.value,
        30
    );
    assert_eq!(
        tr.find_item("clip_30").unwrap().timeline_range.start.rate,
        FrameRate::new(30, 1)
    );
    // clip_60 shifted by 1s (60 frames at 60fps) -> starts at 360 (was 300)
    assert_eq!(
        tr.find_item("clip_60").unwrap().timeline_range.start.value,
        360
    );
    assert_eq!(
        tr.find_item("clip_60").unwrap().timeline_range.start.rate,
        FrameRate::new(60, 1)
    );
}

#[test]
fn test_p16_group_move_dry_run_leaves_project_unmutated() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    track
        .items
        .push(create_clip("clip_a", &asset_id, 0, 5, 0, fps));
    track
        .items
        .push(create_clip("clip_b", &asset_id, 5, 5, 5, fps));
    project.tracks.push(track);
    let rate = FrameRate::new(fps, 1);

    let svc = CommandService::new(project.clone());
    let mut env = make_envelope(
        &project.project_id,
        project.revision,
        vec![(
            "op_dry_gm",
            CommandOperation::ClipGroupMove {
                item_ids: vec!["clip_a".to_string(), "clip_b".to_string()],
                delta: RationalTime::new(2 * fps as i64, rate),
            },
        )],
    );
    env.dry_run = true;

    let diff = svc.dry_run(&env).expect("dry run must succeed");
    assert_eq!(diff.changes.len(), 2);
    assert!(diff
        .changes
        .iter()
        .any(|c| matches!(c, DiffChange::ClipMoved { item_id, .. } if item_id == "clip_a")));
    assert!(diff
        .changes
        .iter()
        .any(|c| matches!(c, DiffChange::ClipMoved { item_id, .. } if item_id == "clip_b")));

    // Authoritative project in svc remains untouched!
    assert_eq!(
        svc.project().tracks[0].items[0].timeline_range.start.value,
        0
    );
    assert_eq!(svc.project().revision, 0);
}

#[test]
fn test_p16_group_move_undo_redo() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    track
        .items
        .push(create_clip("clip_a", &asset_id, 0, 5, 0, fps));
    track
        .items
        .push(create_clip("clip_b", &asset_id, 5, 5, 5, fps));
    project.tracks.push(track);
    let rate = FrameRate::new(fps, 1);

    let mut history = HistoryManager::new(20);
    let mut svc = CommandService::new(project.clone());
    let prior = svc.project().clone();

    let env = make_envelope(
        &project.project_id,
        project.revision,
        vec![(
            "op_gm_undo",
            CommandOperation::ClipGroupMove {
                item_ids: vec!["clip_a".to_string(), "clip_b".to_string()],
                delta: RationalTime::new(4 * fps as i64, rate),
            },
        )],
    );
    assert!(svc.execute(env).is_ok());
    assert_eq!(
        svc.project().tracks[0].items[0].timeline_range.start.value,
        4 * fps as i64
    );

    history.record_edit(HistoryEntry {
        description: "Group Move".to_string(),
        transaction_id: Some("tx_gm".to_string()),
        prior_snapshot: prior,
        resulting_snapshot: svc.project().clone(),
    });

    // Undo
    let mut current = svc.project().clone();
    let undo_res = history.undo(&mut current);
    assert!(undo_res.is_some());
    assert_eq!(current.tracks[0].items[0].timeline_range.start.value, 0);

    // Redo
    let redo_res = history.redo(&mut current);
    assert!(redo_res.is_some());
    assert_eq!(
        current.tracks[0].items[0].timeline_range.start.value,
        4 * fps as i64
    );
}

// =========================================================================
// P16-AC-32..P16-AC-41: ClipRippleDelete Tests
// =========================================================================

#[test]
fn test_p16_ripple_delete_first_clip_shifts_downstream_to_zero() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    track
        .items
        .push(create_clip("clip_a", &asset_id, 0, 5, 0, fps));
    track
        .items
        .push(create_clip("clip_b", &asset_id, 5, 5, 5, fps));
    track
        .items
        .push(create_clip("clip_c", &asset_id, 10, 5, 10, fps));
    project.tracks.push(track);

    let mut svc = CommandService::new(project.clone());
    let env = make_envelope(
        &project.project_id,
        project.revision,
        vec![(
            "op_rd_1",
            CommandOperation::ClipRippleDelete {
                item_id: "clip_a".to_string(),
            },
        )],
    );

    assert!(svc.execute(env).is_ok());
    let tr = &svc.project().tracks[0];
    assert_eq!(tr.items.len(), 2);
    // clip_b was at 5s, now starts at 0s
    assert_eq!(
        tr.find_item("clip_b").unwrap().timeline_range.start.value,
        0
    );
    // clip_c was at 10s, now starts at 5s
    assert_eq!(
        tr.find_item("clip_c").unwrap().timeline_range.start.value,
        5 * fps as i64
    );
}

#[test]
fn test_p16_ripple_delete_middle_clip_preserves_preceding_and_shifts_downstream() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    track
        .items
        .push(create_clip("clip_a", &asset_id, 0, 4, 0, fps));
    track
        .items
        .push(create_clip("clip_b", &asset_id, 4, 6, 4, fps)); // 6s duration
    track
        .items
        .push(create_clip("clip_c", &asset_id, 10, 4, 10, fps));
    project.tracks.push(track);

    let mut svc = CommandService::new(project.clone());
    let env = make_envelope(
        &project.project_id,
        project.revision,
        vec![(
            "op_rd_2",
            CommandOperation::ClipRippleDelete {
                item_id: "clip_b".to_string(),
            },
        )],
    );

    assert!(svc.execute(env).is_ok());
    let tr = &svc.project().tracks[0];
    assert_eq!(tr.items.len(), 2);
    // clip_a untouched at 0..4s
    assert_eq!(
        tr.find_item("clip_a").unwrap().timeline_range.start.value,
        0
    );
    assert_eq!(
        tr.find_item("clip_a")
            .unwrap()
            .timeline_range
            .duration
            .value,
        4 * fps as i64
    );
    // clip_c was at 10s, shifted left by 6s -> starts at 4s
    assert_eq!(
        tr.find_item("clip_c").unwrap().timeline_range.start.value,
        4 * fps as i64
    );
}

#[test]
fn test_p16_ripple_delete_last_clip_no_downstream_shifts() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    track
        .items
        .push(create_clip("clip_a", &asset_id, 0, 5, 0, fps));
    track
        .items
        .push(create_clip("clip_b", &asset_id, 5, 5, 5, fps));
    project.tracks.push(track);

    let mut svc = CommandService::new(project.clone());
    let env = make_envelope(
        &project.project_id,
        project.revision,
        vec![(
            "op_rd_3",
            CommandOperation::ClipRippleDelete {
                item_id: "clip_b".to_string(),
            },
        )],
    );

    assert!(svc.execute(env).is_ok());
    let tr = &svc.project().tracks[0];
    assert_eq!(tr.items.len(), 1);
    assert_eq!(tr.items[0].item_id, "clip_a");
    assert_eq!(tr.items[0].timeline_range.start.value, 0);
}

#[test]
fn test_p16_ripple_delete_downstream_gaps_preserved() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    track
        .items
        .push(create_clip("clip_a", &asset_id, 0, 5, 0, fps));
    track
        .items
        .push(create_clip("clip_b", &asset_id, 5, 5, 5, fps)); // deleted (5s)
    track
        .items
        .push(create_clip("clip_c", &asset_id, 13, 4, 10, fps)); // 3s gap from 10 to 13
    track
        .items
        .push(create_clip("clip_d", &asset_id, 20, 5, 14, fps)); // 3s gap from 17 to 20
    project.tracks.push(track);

    let mut svc = CommandService::new(project.clone());
    let env = make_envelope(
        &project.project_id,
        project.revision,
        vec![(
            "op_rd_gap",
            CommandOperation::ClipRippleDelete {
                item_id: "clip_b".to_string(),
            },
        )],
    );

    assert!(svc.execute(env).is_ok());
    let tr = &svc.project().tracks[0];
    assert_eq!(tr.items.len(), 3);
    // clip_c shifted from 13 to 8s (gap from clip_a at 5 to clip_c at 8 is 3s!)
    assert_eq!(
        tr.find_item("clip_c").unwrap().timeline_range.start.value,
        8 * fps as i64
    );
    // clip_d shifted from 20 to 15s (gap from clip_c ending at 12 to 15 is 3s!)
    assert_eq!(
        tr.find_item("clip_d").unwrap().timeline_range.start.value,
        15 * fps as i64
    );
}

#[test]
fn test_p16_ripple_delete_other_tracks_untouched() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track1 = Track::new("Track 1".to_string(), TrackKind::Video);
    let mut track2 = Track::new("Track 2".to_string(), TrackKind::Audio);
    track1
        .items
        .push(create_clip("v_clip_1", &asset_id, 0, 5, 0, fps));
    track1
        .items
        .push(create_clip("v_clip_2", &asset_id, 5, 5, 5, fps));
    track2
        .items
        .push(create_clip("a_clip_1", &asset_id, 2, 10, 0, fps));
    project.tracks.push(track1);
    project.tracks.push(track2);

    let mut svc = CommandService::new(project.clone());
    let env = make_envelope(
        &project.project_id,
        project.revision,
        vec![(
            "op_rd_other",
            CommandOperation::ClipRippleDelete {
                item_id: "v_clip_1".to_string(),
            },
        )],
    );

    assert!(svc.execute(env).is_ok());
    // Track 2 audio clip is completely unchanged!
    assert_eq!(svc.project().tracks[1].items.len(), 1);
    assert_eq!(svc.project().tracks[1].items[0].item_id, "a_clip_1");
    assert_eq!(
        svc.project().tracks[1].items[0].timeline_range.start.value,
        2 * fps as i64
    );
    assert_eq!(
        svc.project().tracks[1].items[0]
            .timeline_range
            .duration
            .value,
        10 * fps as i64
    );
}

#[test]
fn test_p16_ripple_delete_transitions_cleanup_and_downstream_preservation() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    let track_id = track.track_id.clone();
    let rate = FrameRate::new(fps, 1);

    // Setup:
    // clip_a [0, 5) with source [0, 5)
    // clip_b [5, 10) with source [5, 10) -> target for ripple delete
    // clip_c [10, 15) with source [10, 15)
    // clip_d [15, 20) with source [15, 20)
    track
        .items
        .push(create_clip("clip_a", &asset_id, 0, 5, 0, fps));
    track
        .items
        .push(create_clip("clip_b", &asset_id, 5, 5, 5, fps));
    track
        .items
        .push(create_clip("clip_c", &asset_id, 10, 5, 10, fps));
    track
        .items
        .push(create_clip("clip_d", &asset_id, 15, 5, 15, fps));

    // Transitions:
    // trans_ab touching deleted target
    // trans_bc touching deleted target
    // trans_cd downstream between C & D
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
        transition_id: "trans_cd".to_string(),
        track_id: track_id.clone(),
        left_item_id: "clip_c".to_string(),
        right_item_id: "clip_d".to_string(),
        kind: TransitionKind::CrossDissolve,
        duration: RationalTime::new(30, rate),
    });
    project.tracks.push(track);

    let mut svc = CommandService::new(project.clone());
    let env = make_envelope(
        &project.project_id,
        project.revision,
        vec![(
            "op_rd_trans",
            CommandOperation::ClipRippleDelete {
                item_id: "clip_b".to_string(),
            },
        )],
    );

    assert!(svc.execute(env).is_ok());

    let p = svc.project();
    let tr = &p.tracks[0];
    assert_eq!(tr.items.len(), 3);
    assert_eq!(
        tr.find_item("clip_c").unwrap().timeline_range.start.value,
        5 * fps as i64
    );
    assert_eq!(
        tr.find_item("clip_d").unwrap().timeline_range.start.value,
        10 * fps as i64
    );

    // Verify transitions:
    // trans_ab and trans_bc removed because they touched clip_b
    // trans_cd preserved because both participants shifted together
    assert_eq!(
        tr.transitions.len(),
        1,
        "Direct transitions removed, downstream preserved"
    );
    assert_eq!(tr.transitions[0].transition_id, "trans_cd");

    // Project validates cleanly
    assert!(p.validate().is_ok());
}

#[test]
fn test_p16_ripple_delete_duration_shrink_behavior() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track1 = Track::new("Track 1".to_string(), TrackKind::Video);
    let mut track2 = Track::new("Track 2".to_string(), TrackKind::Video);

    // Track 1 ends at 20s
    track1
        .items
        .push(create_clip("clip_1a", &asset_id, 0, 10, 0, fps));
    track1
        .items
        .push(create_clip("clip_1b", &asset_id, 10, 10, 10, fps));

    // Track 2 ends at 12s
    track2
        .items
        .push(create_clip("clip_2", &asset_id, 0, 12, 0, fps));
    project.tracks.push(track1);
    project.tracks.push(track2);

    let mut svc = CommandService::new(project.clone());
    // Ripple delete clip_1b (10s): Track 1 now ends at 10s.
    // But Track 2 ends at 12s! So project maximum end is determined by Track 2 (12s).
    let env = make_envelope(
        &project.project_id,
        project.revision,
        vec![(
            "op_rd_dur1",
            CommandOperation::ClipRippleDelete {
                item_id: "clip_1b".to_string(),
            },
        )],
    );
    assert!(svc.execute(env).is_ok());

    let max_end = svc
        .project()
        .tracks
        .iter()
        .flat_map(|t| &t.items)
        .map(|i| i.timeline_range.end())
        .max()
        .unwrap();
    assert_eq!(
        max_end.value,
        12 * fps as i64,
        "Track 2 remains longest at 12s"
    );

    // Now ripple delete clip_2 (12s) on Track 2: now Track 1 ends at 10s, which becomes new project max end!
    let env2 = make_envelope(
        &svc.project().project_id,
        svc.project().revision,
        vec![(
            "op_rd_dur2",
            CommandOperation::ClipRippleDelete {
                item_id: "clip_2".to_string(),
            },
        )],
    );
    assert!(svc.execute(env2).is_ok());
    let max_end2 = svc
        .project()
        .tracks
        .iter()
        .flat_map(|t| &t.items)
        .map(|i| i.timeline_range.end())
        .max()
        .unwrap();
    assert_eq!(
        max_end2.value,
        10 * fps as i64,
        "Project duration shrinks to 10s"
    );
}

#[test]
fn test_p16_ripple_delete_undo_redo() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    track
        .items
        .push(create_clip("clip_a", &asset_id, 0, 5, 0, fps));
    track
        .items
        .push(create_clip("clip_b", &asset_id, 5, 5, 5, fps));
    track
        .items
        .push(create_clip("clip_c", &asset_id, 10, 5, 10, fps));
    project.tracks.push(track);

    let mut history = HistoryManager::new(20);
    let mut svc = CommandService::new(project.clone());
    let prior = svc.project().clone();

    let env = make_envelope(
        &project.project_id,
        project.revision,
        vec![(
            "op_rd_undo",
            CommandOperation::ClipRippleDelete {
                item_id: "clip_b".to_string(),
            },
        )],
    );
    assert!(svc.execute(env).is_ok());
    assert_eq!(svc.project().tracks[0].items.len(), 2);
    assert_eq!(
        svc.project().tracks[0].items[1].timeline_range.start.value,
        5 * fps as i64
    );

    history.record_edit(HistoryEntry {
        description: "Ripple Delete".to_string(),
        transaction_id: Some("tx_rd".to_string()),
        prior_snapshot: prior,
        resulting_snapshot: svc.project().clone(),
    });

    // Undo -> restores clip_b and original positions
    let mut current = svc.project().clone();
    history.undo(&mut current);
    assert_eq!(current.tracks[0].items.len(), 3);
    assert_eq!(
        current.tracks[0]
            .find_item("clip_b")
            .unwrap()
            .timeline_range
            .start
            .value,
        5 * fps as i64
    );
    assert_eq!(
        current.tracks[0]
            .find_item("clip_c")
            .unwrap()
            .timeline_range
            .start
            .value,
        10 * fps as i64
    );

    // Redo -> re-applies ripple delete
    history.redo(&mut current);
    assert_eq!(current.tracks[0].items.len(), 2);
    assert_eq!(
        current.tracks[0]
            .find_item("clip_c")
            .unwrap()
            .timeline_range
            .start
            .value,
        5 * fps as i64
    );
}

// =========================================================================
// P16-AC-42..P16-AC-44: Multi-Delete & Duplicate Tests
// =========================================================================

#[test]
fn test_p16_multi_delete_batched_in_one_atomic_envelope() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    track
        .items
        .push(create_clip("clip_1", &asset_id, 0, 5, 0, fps));
    track
        .items
        .push(create_clip("clip_2", &asset_id, 5, 5, 5, fps));
    track
        .items
        .push(create_clip("clip_3", &asset_id, 10, 5, 10, fps));
    project.tracks.push(track);

    let mut svc = CommandService::new(project.clone());
    // Delete clip_1 and clip_2 in one atomic envelope
    let env = make_envelope(
        &project.project_id,
        project.revision,
        vec![
            (
                "op_d1",
                CommandOperation::ClipDelete {
                    item_id: "clip_1".to_string(),
                },
            ),
            (
                "op_d2",
                CommandOperation::ClipDelete {
                    item_id: "clip_2".to_string(),
                },
            ),
        ],
    );

    let res = svc.execute(env);
    assert!(res.is_ok(), "Multi-delete inside one envelope must succeed");
    assert_eq!(svc.project().tracks[0].items.len(), 1);
    assert_eq!(svc.project().tracks[0].items[0].item_id, "clip_3");
    // Only one revision bump for the entire atomic envelope!
    assert_eq!(svc.project().revision, 1);
}

#[test]
fn test_p16_multi_delete_atomic_rollback_on_failure() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    track
        .items
        .push(create_clip("clip_1", &asset_id, 0, 5, 0, fps));
    track
        .items
        .push(create_clip("clip_2", &asset_id, 5, 5, 5, fps));
    project.tracks.push(track);

    let mut svc = CommandService::new(project.clone());
    // op1 valid, op2 refers to non-existent clip -> whole envelope must roll back!
    let env = make_envelope(
        &project.project_id,
        project.revision,
        vec![
            (
                "op_d1",
                CommandOperation::ClipDelete {
                    item_id: "clip_1".to_string(),
                },
            ),
            (
                "op_d2",
                CommandOperation::ClipDelete {
                    item_id: "nonexistent".to_string(),
                },
            ),
        ],
    );

    let res = svc.execute(env);
    assert!(res.is_err(), "Must reject envelope with invalid delete");
    assert_eq!(svc.project().revision, 0);
    // clip_1 was NOT deleted
    assert_eq!(svc.project().tracks[0].items.len(), 2);
}

// =========================================================================
// P16-AC-52..P16-AC-53: Automation Inheritance Tests
// =========================================================================

#[test]
fn test_p16_supported_operations_includes_phase16() {
    let ops = AutomationAdapter::supported_operations();
    assert!(
        ops.contains(&"ClipGroupMove".to_string()),
        "Must advertise ClipGroupMove"
    );
    assert!(
        ops.contains(&"ClipRippleDelete".to_string()),
        "Must advertise ClipRippleDelete"
    );
}

#[test]
fn test_p16_automation_session_generic_dry_run_and_apply_for_phase16_commands() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    track
        .items
        .push(create_clip("clip_a", &asset_id, 0, 5, 0, fps));
    track
        .items
        .push(create_clip("clip_b", &asset_id, 5, 5, 5, fps));
    track
        .items
        .push(create_clip("clip_c", &asset_id, 10, 5, 10, fps));
    project.tracks.push(track);
    let rate = FrameRate::new(fps, 1);

    let mut session = AutomationSession::new();
    let temp_dir = std::env::temp_dir().join(format!(
        "tinix_p16_auto_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let proj_file = temp_dir.join("test_p16.tcproj");
    tinixcut_lib::modules::persistence::save_project_atomic(&project, &proj_file).unwrap();

    session.open_project(&proj_file).unwrap();

    // 1. Dry run ClipGroupMove via session
    let gm_env = make_envelope(
        &project.project_id,
        project.revision,
        vec![(
            "auto_gm",
            CommandOperation::ClipGroupMove {
                item_ids: vec!["clip_b".to_string(), "clip_c".to_string()],
                delta: RationalTime::new(2 * fps as i64, rate),
            },
        )],
    );
    let diff = session
        .dry_run(&gm_env)
        .expect("AutomationSession dry-run ClipGroupMove must succeed");
    assert_eq!(diff.changes.len(), 2);

    // 2. Apply ClipGroupMove via session
    let receipt = session
        .apply(gm_env, false)
        .expect("AutomationSession apply ClipGroupMove must succeed");
    assert_eq!(receipt.new_revision, 1);

    // 3. Apply ClipRippleDelete via session
    let rd_env = make_envelope(
        &project.project_id,
        1,
        vec![(
            "auto_rd",
            CommandOperation::ClipRippleDelete {
                item_id: "clip_a".to_string(),
            },
        )],
    );
    let rd_receipt = session
        .apply(rd_env, false)
        .expect("AutomationSession apply ClipRippleDelete must succeed");
    assert_eq!(rd_receipt.new_revision, 2);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

// =========================================================================
// P16 Remediation: ADR-001 Replay/Idempotency & Mixed-Rate RationalTime
// & Generic CLI/JSONL/MCP Transport Inheritance
// =========================================================================

struct DuplexMcpClient {
    reader: TokioBufReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>,
    writer: tokio::io::WriteHalf<tokio::io::DuplexStream>,
}

impl DuplexMcpClient {
    async fn new(server: TinixCutMcpServer) -> Self {
        let (client_io, server_io) = tokio::io::duplex(131072);
        tokio::spawn(async move {
            if let Ok(running) = rmcp::serve_server(server, server_io).await {
                let _ = running.waiting().await;
            }
        });
        let (read_half, write_half) = tokio::io::split(client_io);
        Self {
            reader: TokioBufReader::new(read_half),
            writer: write_half,
        }
    }

    async fn send_request(
        &mut self,
        method: &str,
        mut params: serde_json::Value,
    ) -> serde_json::Value {
        static REQ_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1000);
        let id = REQ_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if let Some(map) = params.as_object_mut() {
            if !map.contains_key("_meta") {
                map.insert(
                    "_meta".to_string(),
                    serde_json::json!({
                        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                        "io.modelcontextprotocol/clientCapabilities": {},
                        "io.modelcontextprotocol/clientInfo": {
                            "name": "test-client-p16",
                            "version": "1.0.0"
                        }
                    }),
                );
            }
        }
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let req_str = format!("{}\n", serde_json::to_string(&req).unwrap());
        self.writer.write_all(req_str.as_bytes()).await.unwrap();
        self.writer.flush().await.unwrap();
        let mut line = String::new();
        self.reader.read_line(&mut line).await.unwrap();
        serde_json::from_str(&line).expect("Expected valid JSON-RPC response")
    }

    async fn call_tool(&mut self, name: &str, arguments: serde_json::Value) -> serde_json::Value {
        let params = serde_json::json!({
            "name": name,
            "arguments": arguments,
        });
        self.send_request("tools/call", params).await
    }
}

#[test]
fn test_p16_ac30_clip_group_move_adr001_replay_deduplication_and_collision() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    track
        .items
        .push(create_clip("clip_a", &asset_id, 0, 5, 0, fps));
    track
        .items
        .push(create_clip("clip_b", &asset_id, 5, 5, 5, fps));
    project.tracks.push(track);
    let rate = FrameRate::new(fps, 1);

    let mut service = CommandService::new(project.clone());

    let env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: 0,
        client_id: Some("agent-idem".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(1),
        dry_run: false,
        transaction_id: Some("tx-gm-idem-1".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op-gm-idem-1".to_string(),
            operation: CommandOperation::ClipGroupMove {
                item_ids: vec!["clip_a".to_string(), "clip_b".to_string()],
                delta: RationalTime::new(2 * fps as i64, rate),
            },
        }],
    };

    // 1. Initial execution with structured client identity
    let r1 = service
        .execute(env.clone())
        .expect("First execute must succeed");
    assert_eq!(r1.new_revision, 1);
    assert_eq!(r1.operations_applied, 1);
    assert_eq!(r1.receipts[0].status, "applied");
    assert_eq!(service.project().revision, 1);
    assert_eq!(
        service.project().tracks[0].items[0]
            .timeline_range
            .start
            .value,
        2 * fps as i64
    );
    assert_eq!(
        service.project().tracks[0].items[1]
            .timeline_range
            .start
            .value,
        7 * fps as i64
    );

    // 2. Exact Replay (same envelope identity + payload)
    let replay = service.execute(env.clone()).expect("Replay must succeed");
    assert_eq!(
        replay.new_revision, 1,
        "Replay must return cached revision without incrementing"
    );
    assert_eq!(
        replay.operations_applied, 0,
        "Replay must report 0 operations applied"
    );
    assert_eq!(
        replay.receipts[0].status, "deduplicated_noop",
        "Replay receipt must report deduplicated_noop"
    );
    assert_eq!(service.project().revision, 1, "Project revision unchanged");
    assert_eq!(
        service.project().tracks[0].items[0]
            .timeline_range
            .start
            .value,
        2 * fps as i64,
        "Clips must NOT be shifted a second time"
    );

    // 3. Idempotency Collision: same operation_id with different payload
    let mut collision_env = env.clone();
    collision_env.sequence_number = Some(2);
    collision_env.operations[0].operation = CommandOperation::ClipGroupMove {
        item_ids: vec!["clip_a".to_string(), "clip_b".to_string()],
        delta: RationalTime::new(5 * fps as i64, rate), // Changed delta
    };
    let col_res = service.execute(collision_env);
    assert!(
        matches!(
            col_res,
            Err(CommandError::IdempotencyCollision { ref operation_id, .. }) if operation_id == "op-gm-idem-1"
        ),
        "Same operation_id with altered payload must yield IdempotencyCollision: {:?}",
        col_res
    );

    // 4. Stale Revision: attempting operation with outdated base_revision
    let stale_env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: 0, // Stale! Service is at revision 1
        client_id: Some("agent-idem".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(2),
        dry_run: false,
        transaction_id: Some("tx-gm-idem-2".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op-gm-idem-2".to_string(),
            operation: CommandOperation::ClipGroupMove {
                item_ids: vec!["clip_a".to_string()],
                delta: RationalTime::new(1 * fps as i64, rate),
            },
        }],
    };
    let stale_res = service.execute(stale_env);
    assert!(
        matches!(stale_res, Err(CommandError::StaleRevision { .. })),
        "Stale base revision must be rejected: {:?}",
        stale_res
    );
}

#[test]
fn test_p16_ac41_clip_ripple_delete_adr001_replay_deduplication_and_collision() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    track
        .items
        .push(create_clip("clip_a", &asset_id, 0, 5, 0, fps));
    track
        .items
        .push(create_clip("clip_b", &asset_id, 5, 5, 5, fps));
    track
        .items
        .push(create_clip("clip_c", &asset_id, 10, 5, 10, fps));
    project.tracks.push(track);

    let mut service = CommandService::new(project.clone());

    let env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: 0,
        client_id: Some("agent-idem".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(1),
        dry_run: false,
        transaction_id: Some("tx-rd-idem-1".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op-rd-idem-1".to_string(),
            operation: CommandOperation::ClipRippleDelete {
                item_id: "clip_b".to_string(),
            },
        }],
    };

    // 1. Initial Apply
    let r1 = service
        .execute(env.clone())
        .expect("First ripple delete execute must succeed");
    assert_eq!(r1.new_revision, 1);
    assert_eq!(r1.operations_applied, 1);
    assert_eq!(r1.receipts[0].status, "applied");
    assert_eq!(service.project().revision, 1);
    assert_eq!(service.project().tracks[0].items.len(), 2);
    assert_eq!(service.project().tracks[0].items[0].item_id, "clip_a");
    assert_eq!(service.project().tracks[0].items[1].item_id, "clip_c");
    assert_eq!(
        service.project().tracks[0].items[1]
            .timeline_range
            .start
            .value,
        5 * fps as i64,
        "clip_c must be shifted left by 5s (deleted clip_b duration)"
    );

    // 2. Exact Replay
    let replay = service.execute(env.clone()).expect("Replay must succeed");
    assert_eq!(
        replay.new_revision, 1,
        "Replay must return cached revision without incrementing"
    );
    assert_eq!(
        replay.operations_applied, 0,
        "Replay must report 0 operations applied"
    );
    assert_eq!(replay.receipts[0].status, "deduplicated_noop");
    assert_eq!(
        service.project().tracks[0].items.len(),
        2,
        "No second deletion"
    );
    assert_eq!(
        service.project().tracks[0].items[1]
            .timeline_range
            .start
            .value,
        5 * fps as i64,
        "clip_c must NOT be shifted downstream a second time"
    );

    // 3. Collision: same operation_id with different payload
    let mut collision_env = env.clone();
    collision_env.sequence_number = Some(2);
    collision_env.operations[0].operation = CommandOperation::ClipRippleDelete {
        item_id: "clip_a".to_string(), // Different payload!
    };
    let col_res = service.execute(collision_env);
    assert!(
        matches!(
            col_res,
            Err(CommandError::IdempotencyCollision { ref operation_id, .. }) if operation_id == "op-rd-idem-1"
        ),
        "Same operation_id with different ripple delete payload must fail with IdempotencyCollision: {:?}",
        col_res
    );
}

#[test]
fn test_p16_ac24_ac34_ac40_mixed_rate_rational_time_edge_cases_zero_float() {
    // 1. Semantically equal times represented at different frame rates
    let t_25 = RationalTime::new(100, FrameRate::FPS_25); // 4.0s
    let t_30 = RationalTime::new(120, FrameRate::FPS_30); // 4.0s
    let t_24 = RationalTime::new(96, FrameRate::FPS_24); // 4.0s
    let t_60 = RationalTime::new(240, FrameRate::FPS_60); // 4.0s

    // Semantic equality succeeds across representations
    assert!(t_25.is_same_time(&t_30));
    assert!(t_25.semantic_eq(&t_24));
    assert!(t_30.semantic_eq(&t_60));
    assert_eq!(t_25.semantic_cmp(&t_30), std::cmp::Ordering::Equal);
    assert_eq!(t_24.semantic_cmp(&t_60), std::cmp::Ordering::Equal);

    // Representation equality and Ord contract consistency:
    // Different representations of same time must NOT be ==, and representation Ord must NOT return Equal
    assert_ne!(t_25, t_30);
    assert_ne!(t_25.cmp(&t_30), std::cmp::Ordering::Equal);

    // BTreeSet regression test: distinct representations must not be collapsed
    let mut btree = std::collections::BTreeSet::new();
    btree.insert(t_25);
    btree.insert(t_30);
    assert_eq!(
        btree.len(),
        2,
        "BTreeSet must preserve representation-distinct times"
    );

    // NTSC drop/non-integer rates: 30000 frames @ 29.97 (30000/1001) is EXACTLY 1001.0 seconds!
    let t_ntsc_29_97 = RationalTime::new(30000, FrameRate::FPS_29_97);
    let t_ntsc_23_976 = RationalTime::new(24000, FrameRate::FPS_23_976); // 24000 @ 24000/1001 is EXACTLY 1001.0 seconds!
    let t_30_1001s = RationalTime::new(1001 * 30, FrameRate::FPS_30);

    assert!(t_ntsc_29_97.is_same_time(&t_ntsc_23_976));
    assert!(t_ntsc_29_97.is_same_time(&t_30_1001s));
    assert_eq!(
        t_ntsc_29_97.semantic_cmp(&t_30_1001s),
        std::cmp::Ordering::Equal
    );
    assert_ne!(t_ntsc_29_97.cmp(&t_30_1001s), std::cmp::Ordering::Equal);

    // 2. Large timeline frame values (test 128-bit cross multiplication precision without overflow)
    let t_large_60 = RationalTime::new(600_000_000, FrameRate::FPS_60); // 10,000,000 seconds
    let t_large_30 = RationalTime::new(300_000_000, FrameRate::FPS_30); // 10,000,000 seconds
    assert!(t_large_60.is_same_time(&t_large_30));
    assert_eq!(
        t_large_60.semantic_cmp(&t_large_30),
        std::cmp::Ordering::Equal
    );
    assert_ne!(t_large_60.cmp(&t_large_30), std::cmp::Ordering::Equal);

    let t_large_30_minus_1 = RationalTime::new(299_999_999, FrameRate::FPS_30);
    let t_large_30_plus_1 = RationalTime::new(300_000_001, FrameRate::FPS_30);
    assert!(t_large_30_minus_1.semantic_lt(&t_large_60));
    assert!(t_large_30_plus_1.semantic_gt(&t_large_60));
    assert!(t_large_60.semantic_ge(&t_large_30_minus_1));
    assert!(!t_large_60.semantic_ge(&t_large_30_plus_1));

    // 3. Ripple delete downstream boundary: exactly equal, 1 tick before, 1 tick after
    // Setup A: Downstream start exactly equal to deleted end across mixed frame rates
    {
        let (mut project, asset_id) = create_test_project_with_asset(30);
        let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
        // clip_del: 0 to 100 frames @ 25fps (end is exactly 4.0s)
        let t_range_del = TimeRange::new(
            RationalTime::new(0, FrameRate::FPS_25),
            RationalTime::new(100, FrameRate::FPS_25),
        );
        let mut clip_del = TimelineItem::new(
            Some(asset_id.clone()),
            "clip_del".to_string(),
            ItemKind::Video,
            t_range_del,
            t_range_del,
        )
        .unwrap();
        clip_del.item_id = "clip_del".to_string();

        // clip_exact: start at 120 frames @ 30fps (exactly 4.0s)
        let t_range_exact = TimeRange::new(
            RationalTime::new(120, FrameRate::FPS_30),
            RationalTime::new(60, FrameRate::FPS_30),
        );
        let mut clip_exact = TimelineItem::new(
            Some(asset_id.clone()),
            "clip_exact".to_string(),
            ItemKind::Video,
            t_range_exact,
            t_range_exact,
        )
        .unwrap();
        clip_exact.item_id = "clip_exact".to_string();

        track.items.push(clip_del);
        track.items.push(clip_exact);
        project.tracks.push(track);

        let mut service = CommandService::new(project);
        let env = make_envelope(
            &service.project().project_id,
            0,
            vec![(
                "op_rd",
                CommandOperation::ClipRippleDelete {
                    item_id: "clip_del".to_string(),
                },
            )],
        );
        let receipt = service
            .execute(env)
            .expect("Ripple delete with exact cross-rate boundary must succeed");
        assert_eq!(receipt.receipts[0].status, "applied");
        let surviving = &service.project().tracks[0].items;
        assert_eq!(surviving.len(), 1);
        assert_eq!(surviving[0].item_id, "clip_exact");
        assert_eq!(
            surviving[0].timeline_range.start.value, 0,
            "clip_exact (start exactly equal to deleted end at 4.0s) must be shifted to 0"
        );
    }

    // Setup B: Downstream start 1 tick AFTER boundary (121 frames @ 30fps = 4.0333s > 4.0s)
    {
        let (mut project, asset_id) = create_test_project_with_asset(30);
        let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
        let t_range_del = TimeRange::new(
            RationalTime::new(0, FrameRate::FPS_25),
            RationalTime::new(100, FrameRate::FPS_25), // ends at 4.0s
        );
        let mut clip_del = TimelineItem::new(
            Some(asset_id.clone()),
            "clip_del".to_string(),
            ItemKind::Video,
            t_range_del,
            t_range_del,
        )
        .unwrap();
        clip_del.item_id = "clip_del".to_string();

        let t_range_after = TimeRange::new(
            RationalTime::new(121, FrameRate::FPS_30), // 1 tick after 4.0s (120 frames)
            RationalTime::new(60, FrameRate::FPS_30),
        );
        let mut clip_after = TimelineItem::new(
            Some(asset_id.clone()),
            "clip_after".to_string(),
            ItemKind::Video,
            t_range_after,
            t_range_after,
        )
        .unwrap();
        clip_after.item_id = "clip_after".to_string();

        track.items.push(clip_del);
        track.items.push(clip_after);
        project.tracks.push(track);

        let mut service = CommandService::new(project);
        let env = make_envelope(
            &service.project().project_id,
            0,
            vec![(
                "op_rd",
                CommandOperation::ClipRippleDelete {
                    item_id: "clip_del".to_string(),
                },
            )],
        );
        service.execute(env).unwrap();
        let surviving = &service.project().tracks[0].items;
        assert_eq!(surviving.len(), 1);
        assert_eq!(surviving[0].item_id, "clip_after");
        assert_eq!(
            surviving[0].timeline_range.start.value, 1,
            "clip_after (1 tick after deleted end) must preserve its 1 tick offset after shift"
        );
    }

    // Setup C: Preceding clip start 1 tick BEFORE deleted start
    {
        let (mut project, asset_id) = create_test_project_with_asset(30);
        let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
        let t_range_prec = TimeRange::new(
            RationalTime::new(0, FrameRate::FPS_30),
            RationalTime::new(119, FrameRate::FPS_30), // ends at 119 frames
        );
        let mut clip_prec = TimelineItem::new(
            Some(asset_id.clone()),
            "clip_prec".to_string(),
            ItemKind::Video,
            t_range_prec,
            t_range_prec,
        )
        .unwrap();
        clip_prec.item_id = "clip_prec".to_string();

        let t_range_del = TimeRange::new(
            RationalTime::new(120, FrameRate::FPS_30), // deleted clip starts at 120 frames (4.0s)
            RationalTime::new(60, FrameRate::FPS_30),
        );
        let mut clip_del = TimelineItem::new(
            Some(asset_id.clone()),
            "clip_del".to_string(),
            ItemKind::Video,
            t_range_del,
            t_range_del,
        )
        .unwrap();
        clip_del.item_id = "clip_del".to_string();

        track.items.push(clip_prec);
        track.items.push(clip_del);
        project.tracks.push(track);

        let mut service = CommandService::new(project);
        let env = make_envelope(
            &service.project().project_id,
            0,
            vec![(
                "op_rd",
                CommandOperation::ClipRippleDelete {
                    item_id: "clip_del".to_string(),
                },
            )],
        );
        service.execute(env).unwrap();
        let surviving = &service.project().tracks[0].items;
        assert_eq!(surviving.len(), 1);
        assert_eq!(surviving[0].item_id, "clip_prec");
        assert_eq!(
            surviving[0].timeline_range.start.value, 0,
            "Preceding clip must NOT be shifted by ripple delete"
        );
    }

    // Setup D: Group move across mixed rates
    {
        let (mut project, asset_id) = create_test_project_with_asset(30);
        let mut track1 = Track::new("Track 1".to_string(), TrackKind::Video);
        let mut track2 = Track::new("Track 2".to_string(), TrackKind::Video);

        // clip1 at 30fps: start = 30 frames (1.0s)
        let t1 = TimeRange::new(
            RationalTime::new(30, FrameRate::FPS_30),
            RationalTime::new(60, FrameRate::FPS_30),
        );
        let mut c1 = TimelineItem::new(
            Some(asset_id.clone()),
            "c1".to_string(),
            ItemKind::Video,
            t1,
            t1,
        )
        .unwrap();
        c1.item_id = "c1".to_string();
        track1.items.push(c1);

        // clip2 at 24fps: start = 24 frames (1.0s)
        let t2 = TimeRange::new(
            RationalTime::new(24, FrameRate::FPS_24),
            RationalTime::new(48, FrameRate::FPS_24),
        );
        let mut c2 = TimelineItem::new(
            Some(asset_id.clone()),
            "c2".to_string(),
            ItemKind::Video,
            t2,
            t2,
        )
        .unwrap();
        c2.item_id = "c2".to_string();
        track2.items.push(c2);

        project.tracks.push(track1);
        project.tracks.push(track2);

        let mut service = CommandService::new(project);
        // Move by delta = 25 frames @ 25fps (= 1.0s)
        let env = make_envelope(
            &service.project().project_id,
            0,
            vec![(
                "op_gm",
                CommandOperation::ClipGroupMove {
                    item_ids: vec!["c1".to_string(), "c2".to_string()],
                    delta: RationalTime::new(25, FrameRate::FPS_25),
                },
            )],
        );
        service.execute(env).unwrap();
        let item1 = service.project().tracks[0].find_item("c1").unwrap();
        let item2 = service.project().tracks[1].find_item("c2").unwrap();
        // 1.0s + 1.0s = 2.0s
        assert_eq!(item1.timeline_range.start.value, 60); // 60 frames @ 30fps
        assert_eq!(item2.timeline_range.start.value, 48); // 48 frames @ 24fps
    }
}

#[tokio::test]
async fn test_p16_ac53_generic_cli_jsonl_mcp_transport_inheritance() {
    let fps = 30;
    let (mut project, asset_id) = create_test_project_with_asset(fps);
    let mut track = Track::new("Track 1".to_string(), TrackKind::Video);
    track
        .items
        .push(create_clip("clip_a", &asset_id, 0, 5, 0, fps));
    track
        .items
        .push(create_clip("clip_b", &asset_id, 5, 5, 5, fps));
    track
        .items
        .push(create_clip("clip_c", &asset_id, 10, 5, 10, fps));
    project.tracks.push(track);
    let rate = FrameRate::new(fps, 1);

    let temp_dir = std::env::temp_dir().join(format!(
        "tinixcut_p16_transport_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let proj_file = temp_dir.join("project.tcproj");
    tinixcut_lib::modules::persistence::save_project_atomic(&project, &proj_file).unwrap();

    // -------------------------------------------------------------------------
    // Transport 1: CLI apply/diff smoke via AutomationAdapter::handle_cli
    // -------------------------------------------------------------------------
    let gm_env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: 0,
        client_id: Some("cli".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(1),
        dry_run: false,
        transaction_id: Some("tx-cli-gm".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "cli_gm_1".to_string(),
            operation: CommandOperation::ClipGroupMove {
                item_ids: vec!["clip_b".to_string(), "clip_c".to_string()],
                delta: RationalTime::new(2 * fps as i64, rate),
            },
        }],
    };
    let env_gm_file = temp_dir.join("env_gm.json");
    std::fs::write(&env_gm_file, serde_json::to_string(&gm_env).unwrap()).unwrap();

    // CLI diff (dry-run)
    let cli_diff_out = AutomationAdapter::handle_cli(&[
        "diff".to_string(),
        proj_file.to_str().unwrap().to_string(),
        env_gm_file.to_str().unwrap().to_string(),
    ])
    .expect("CLI diff for ClipGroupMove must succeed");
    assert!(
        cli_diff_out.contains("clipMoved"),
        "CLI diff output must contain clipMoved: {cli_diff_out}"
    );

    // CLI apply
    let cli_apply_out = AutomationAdapter::handle_cli(&[
        "apply".to_string(),
        proj_file.to_str().unwrap().to_string(),
        env_gm_file.to_str().unwrap().to_string(),
    ])
    .expect("CLI apply for ClipGroupMove must succeed");
    assert!(
        cli_apply_out.contains("applied"),
        "CLI apply output must indicate success"
    );

    // CLI apply for ClipRippleDelete
    let rd_env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: 1,
        client_id: Some("cli".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(2),
        dry_run: false,
        transaction_id: Some("tx-cli-rd".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "cli_rd_1".to_string(),
            operation: CommandOperation::ClipRippleDelete {
                item_id: "clip_a".to_string(),
            },
        }],
    };
    let env_rd_file = temp_dir.join("env_rd.json");
    std::fs::write(&env_rd_file, serde_json::to_string(&rd_env).unwrap()).unwrap();

    let cli_rd_out = AutomationAdapter::handle_cli(&[
        "apply".to_string(),
        proj_file.to_str().unwrap().to_string(),
        env_rd_file.to_str().unwrap().to_string(),
    ])
    .expect("CLI apply for ClipRippleDelete must succeed");
    assert!(
        cli_rd_out.contains("applied"),
        "CLI apply output must indicate success"
    );

    // -------------------------------------------------------------------------
    // Transport 2: Long-lived JSONL stdio session smoke via run_stdio_session
    // -------------------------------------------------------------------------
    let jsonl_gm_env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: 2,
        client_id: Some("session".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(1),
        dry_run: false,
        transaction_id: Some("tx-jsonl-gm".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "jsonl_gm_1".to_string(),
            operation: CommandOperation::ClipGroupMove {
                item_ids: vec!["clip_b".to_string()],
                delta: RationalTime::new(1 * fps as i64, rate),
            },
        }],
    };
    let jsonl_rd_env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: 3,
        client_id: Some("session".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(2),
        dry_run: false,
        transaction_id: Some("tx-jsonl-rd".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "jsonl_rd_1".to_string(),
            operation: CommandOperation::ClipRippleDelete {
                item_id: "clip_b".to_string(),
            },
        }],
    };

    let req_open = serde_json::json!({
        "protocolVersion": 1,
        "requestId": "req-open",
        "method": "project.open",
        "params": { "path": proj_file.to_str().unwrap() }
    });
    let req_diff_gm = serde_json::json!({
        "protocolVersion": 1,
        "requestId": "req-diff-gm",
        "method": "command.diff",
        "params": { "envelope": jsonl_gm_env }
    });
    let req_apply_gm = serde_json::json!({
        "protocolVersion": 1,
        "requestId": "req-apply-gm",
        "method": "command.apply",
        "params": { "envelope": jsonl_gm_env, "saveAfter": true }
    });
    let req_apply_rd = serde_json::json!({
        "protocolVersion": 1,
        "requestId": "req-apply-rd",
        "method": "command.apply",
        "params": { "envelope": jsonl_rd_env, "saveAfter": true }
    });
    let req_shutdown = serde_json::json!({
        "protocolVersion": 1,
        "requestId": "req-shutdown",
        "method": "session.shutdown"
    });

    let jsonl_input = format!(
        "{}\n{}\n{}\n{}\n{}\n",
        serde_json::to_string(&req_open).unwrap(),
        serde_json::to_string(&req_diff_gm).unwrap(),
        serde_json::to_string(&req_apply_gm).unwrap(),
        serde_json::to_string(&req_apply_rd).unwrap(),
        serde_json::to_string(&req_shutdown).unwrap()
    );

    let mut stdin_cursor = std::io::Cursor::new(jsonl_input.into_bytes());
    let mut stdout_buf = Vec::new();
    let mut stderr_buf = Vec::new();
    tinixcut_lib::modules::automation::run_stdio_session(
        &mut stdin_cursor,
        &mut stdout_buf,
        &mut stderr_buf,
    );
    let stdout_str = String::from_utf8(stdout_buf).expect("Valid UTF-8 JSONL output");
    assert!(
        stdout_str.contains("req-open") && stdout_str.contains("\"ok\":true"),
        "project.open in JSONL session must succeed: {stdout_str}"
    );
    assert!(
        stdout_str.contains("req-diff-gm") && stdout_str.contains("\"ok\":true"),
        "command.diff for ClipGroupMove in JSONL session must succeed"
    );
    assert!(
        stdout_str.contains("req-apply-gm") && stdout_str.contains("\"ok\":true"),
        "command.apply for ClipGroupMove in JSONL session must succeed"
    );
    assert!(
        stdout_str.contains("req-apply-rd") && stdout_str.contains("\"ok\":true"),
        "command.apply for ClipRippleDelete in JSONL session must succeed"
    );

    // -------------------------------------------------------------------------
    // Transport 3: MCP smoke via TinixCutMcpServer & DuplexMcpClient
    // -------------------------------------------------------------------------
    let mcp_proj_file = temp_dir.join("mcp_test.tcproj");
    tinixcut_lib::modules::persistence::save_project_atomic(&project, &mcp_proj_file).unwrap();

    let server = TinixCutMcpServer::new();
    let mut mcp_client = DuplexMcpClient::new(server).await;

    // MCP dry_run for ClipGroupMove
    let mcp_gm_env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: 0,
        client_id: Some("agent-mcp".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(1),
        dry_run: false,
        transaction_id: Some("tx-mcp-gm-1".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "mcp_gm_1".to_string(),
            operation: CommandOperation::ClipGroupMove {
                item_ids: vec!["clip_b".to_string(), "clip_c".to_string()],
                delta: RationalTime::new(2 * fps as i64, rate),
            },
        }],
    };
    let dry_res = mcp_client
        .call_tool(
            "tinixcut_dry_run",
            serde_json::json!({
                "projectPath": mcp_proj_file.to_str().unwrap(),
                "envelope": mcp_gm_env
            }),
        )
        .await;
    assert_eq!(
        dry_res["result"]["isError"], false,
        "MCP tinixcut_dry_run must succeed"
    );

    // MCP apply for ClipGroupMove
    let apply_gm_res = mcp_client
        .call_tool(
            "tinixcut_apply",
            serde_json::json!({
                "projectPath": mcp_proj_file.to_str().unwrap(),
                "envelope": mcp_gm_env
            }),
        )
        .await;
    assert_eq!(
        apply_gm_res["result"]["isError"], false,
        "MCP tinixcut_apply for ClipGroupMove must succeed"
    );

    // MCP apply for ClipRippleDelete
    let mcp_rd_env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: 1,
        client_id: Some("agent-mcp".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(2),
        dry_run: false,
        transaction_id: Some("tx-mcp-rd-1".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "mcp_rd_1".to_string(),
            operation: CommandOperation::ClipRippleDelete {
                item_id: "clip_a".to_string(),
            },
        }],
    };
    let apply_rd_res = mcp_client
        .call_tool(
            "tinixcut_apply",
            serde_json::json!({
                "projectPath": mcp_proj_file.to_str().unwrap(),
                "envelope": mcp_rd_env
            }),
        )
        .await;
    assert_eq!(
        apply_rd_res["result"]["isError"], false,
        "MCP tinixcut_apply for ClipRippleDelete must succeed"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}
