//! Phase 17 — Performance Requalification & Release Foundation Benchmarks
//! Explicit opt-in release qualification harness implementing PR-01..PR-18 and P17-AC-16..44.
//!
//! Invariant: Normal `cargo test` does not execute wall-clock assertions.
//! Run explicitly via:
//! `cargo test --release --test phase17_performance_benchmarks -- --ignored --nocapture`

mod phase17_fixtures;

use phase17_fixtures::{
    build_fixture_a, build_fixture_b, ensure_fixture_a_synthetic_media, FixtureAVariant,
};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

static BENCH_LOCK: Mutex<()> = Mutex::new(());

use tinixcut_lib::modules::command::{
    CommandEnvelope, CommandOperation, CommandService, OperationEnvelope,
};
use tinixcut_lib::modules::composition::CompositionPlan;
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime};
use tinixcut_lib::modules::domain::CURRENT_SCHEMA_VERSION;
use tinixcut_lib::modules::export::{ExportEngine, ExportProfile};
use tinixcut_lib::modules::media::ffmpeg::FFmpegService;
use tinixcut_lib::modules::persistence::{load_project, save_project_atomic};

static NEXT_REQ_ID: AtomicU64 = AtomicU64::new(100);

fn next_id() -> u64 {
    NEXT_REQ_ID.fetch_add(1, Ordering::SeqCst)
}

fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tinixcut_p17_bench_{}_{}", name, next_id()));
    if dir.exists() {
        let _ = fs::remove_dir_all(&dir);
    }
    fs::create_dir_all(&dir).expect("Failed to create test directory");
    dir
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct Stats {
    pub count: usize,
    pub min_ms: f64,
    pub median_ms: f64,
    pub p95_ms: f64,
    pub max_ms: f64,
    pub mean_ms: f64,
}

fn compute_stats(mut samples: Vec<Duration>) -> Stats {
    assert!(!samples.is_empty(), "Samples cannot be empty");
    samples.sort();
    let count = samples.len();
    let min_ms = samples.first().unwrap().as_secs_f64() * 1000.0;
    let max_ms = samples.last().unwrap().as_secs_f64() * 1000.0;
    let sum_ms: f64 = samples.iter().map(|s| s.as_secs_f64() * 1000.0).sum();
    let mean_ms = sum_ms / count as f64;

    let median_ms = if count % 2 == 1 {
        samples[count / 2].as_secs_f64() * 1000.0
    } else {
        (samples[count / 2 - 1].as_secs_f64() + samples[count / 2].as_secs_f64()) * 500.0
    };

    let p95_idx = (((count as f64) * 0.95).ceil() as usize).saturating_sub(1);
    let p95_ms = samples[p95_idx.min(count - 1)].as_secs_f64() * 1000.0;

    Stats {
        count,
        min_ms,
        median_ms,
        p95_ms,
        max_ms,
        mean_ms,
    }
}

// ===========================================================================
// P17-AC-06..15: Dual Deterministic Fixture Integrity (Runs in normal CI)
// ===========================================================================

#[test]
fn test_p17_fixture_a_and_b_structural_integrity() {
    // 1. Fixture A (720p and 1080p variants)
    let fix_a_720 = build_fixture_a(FixtureAVariant::Resolution720p, None, None);
    assert_eq!(fix_a_720.schema_version, CURRENT_SCHEMA_VERSION);
    assert_eq!(fix_a_720.settings.width, 1280);
    assert_eq!(fix_a_720.settings.height, 720);
    assert_eq!(
        fix_a_720.tracks.len(),
        3,
        "Fixture A must have exactly 3 tracks"
    );

    let video_tracks_count = fix_a_720
        .tracks
        .iter()
        .filter(|t| t.kind == tinixcut_lib::modules::domain::TrackKind::Video)
        .count();
    let audio_tracks_count = fix_a_720
        .tracks
        .iter()
        .filter(|t| t.kind == tinixcut_lib::modules::domain::TrackKind::Audio)
        .count();
    assert_eq!(
        video_tracks_count, 2,
        "Fixture A must have exactly 2 Video tracks (1 primary video, 1 title overlay)"
    );
    assert_eq!(
        audio_tracks_count, 1,
        "Fixture A must have exactly 1 Audio track (audio bed)"
    );

    // Track 0: Primary video with 2 clips and 1 cut-owned CrossDissolve
    assert_eq!(fix_a_720.tracks[0].items.len(), 2);
    assert_eq!(fix_a_720.tracks[0].transitions.len(), 1);
    assert_eq!(
        fix_a_720.tracks[0].transitions[0].kind,
        tinixcut_lib::modules::domain::TransitionKind::CrossDissolve
    );

    // Track 1: Title/Text overlay with 1 item
    assert_eq!(fix_a_720.tracks[1].items.len(), 1);
    assert_eq!(
        fix_a_720.tracks[1].items[0].kind,
        tinixcut_lib::modules::domain::ItemKind::Text
    );

    // Track 2: Audio bed with volume automation keyframes
    assert_eq!(fix_a_720.tracks[2].items.len(), 1);
    assert_eq!(
        fix_a_720.tracks[2].items[0].kind,
        tinixcut_lib::modules::domain::ItemKind::Audio
    );
    assert!(
        fix_a_720.tracks[2].items[0].keyframes.len() >= 3,
        "Audio bed must contain volume automation keyframes"
    );

    // Code-level evidence of nonlinear CubicBezier representation
    let kf_clip1 = &fix_a_720.tracks[0].items[0].keyframes[0];
    assert!(
        matches!(
            kf_clip1.outgoing_easing,
            tinixcut_lib::modules::domain::KeyframeEasing::CubicBezier { .. }
        ),
        "Clip 1 must use nonlinear CubicBezier easing"
    );
    let kf_text = &fix_a_720.tracks[1].items[0].keyframes[0];
    assert!(
        matches!(
            kf_text.outgoing_easing,
            tinixcut_lib::modules::domain::KeyframeEasing::CubicBezier { .. }
        ),
        "Title text must use nonlinear CubicBezier easing"
    );
    let kf_audio = &fix_a_720.tracks[2].items[0].keyframes[1];
    assert!(
        matches!(
            kf_audio.outgoing_easing,
            tinixcut_lib::modules::domain::KeyframeEasing::CubicBezier { .. }
        ),
        "Audio automation must use nonlinear CubicBezier easing"
    );

    assert!(fix_a_720.validate().is_ok());

    let fix_a_1080 = build_fixture_a(FixtureAVariant::Resolution1080p, None, None);
    assert_eq!(fix_a_1080.settings.width, 1920);
    assert_eq!(fix_a_1080.settings.height, 1080);
    assert_eq!(fix_a_1080.tracks.len(), 3);
    assert!(fix_a_1080.validate().is_ok());

    // 2. Fixture B (Scale Workload)
    let fix_b = build_fixture_b(None);
    assert_eq!(fix_b.schema_version, CURRENT_SCHEMA_VERSION);
    assert_eq!(fix_b.tracks.len(), 20, "Must have exactly 20 tracks");
    let total_items: usize = fix_b.tracks.iter().map(|t| t.items.len()).sum();
    assert_eq!(total_items, 200, "Must have exactly 200 items");

    let total_transitions: usize = fix_b.tracks.iter().map(|t| t.transitions.len()).sum();
    assert_eq!(total_transitions, 20, "Must have exactly 20 CrossDissolves");
    for t_idx in 0..4 {
        assert_eq!(
            fix_b.tracks[t_idx].transitions.len(),
            5,
            "Tracks 0..3 must each have exactly 5 CrossDissolves"
        );
    }

    let mut visual_kfs = 0;
    let mut audio_kfs = 0;
    for (t_idx, track) in fix_b.tracks.iter().enumerate() {
        for item in &track.items {
            if t_idx < 14 {
                visual_kfs += item.keyframes.len();
            } else if t_idx < 18 {
                audio_kfs += item.keyframes.len();
            }
        }
    }
    assert!(
        visual_kfs >= 60,
        "Must have >= 60 visual nonlinear keyframes (found {visual_kfs})"
    );
    assert_eq!(audio_kfs, 120, "Must have exactly 120 audio keyframes");
    assert!(fix_b.validate().is_ok());
}

// ===========================================================================
// P17-AC-21..24: Fixture B Persistence, Compile, and Preview Sampling Benchmarks
// ===========================================================================

#[test]
#[ignore]
fn test_p17_fixture_b_scale_persistence_and_sampling() {
    let _lock = BENCH_LOCK.lock().unwrap();
    let dir = test_dir("fixb_scale");
    let project_file = dir.join("fixture_b_bench.tcproj");
    let fixture_b = build_fixture_b(Some(&dir));

    println!("\n=== [BENCHMARK] Fixture B Scale Operations (N=50) ===");

    // 1. save_project_atomic
    for _ in 0..5 {
        save_project_atomic(&fixture_b, &project_file).unwrap();
    }
    let mut save_samples = Vec::new();
    for _ in 0..50 {
        let t0 = Instant::now();
        save_project_atomic(&fixture_b, &project_file).unwrap();
        save_samples.push(t0.elapsed());
    }
    let save_stats = compute_stats(save_samples);
    println!(
        "save_project_atomic: min={:.3}ms median={:.3}ms p95={:.3}ms max={:.3}ms (ceiling < 200ms, budget < 10ms)",
        save_stats.min_ms, save_stats.median_ms, save_stats.p95_ms, save_stats.max_ms
    );
    assert!(
        save_stats.max_ms < 200.0,
        "save_project_atomic exceeded qualification ceiling of 200ms"
    );
    assert!(
        save_stats.median_ms < 10.0,
        "save_project_atomic exceeded reference median budget of 10ms"
    );

    // 2. load_project
    for _ in 0..5 {
        let _ = load_project(&project_file).unwrap();
    }
    let mut load_samples = Vec::new();
    for _ in 0..50 {
        let t0 = Instant::now();
        let loaded = load_project(&project_file).unwrap();
        load_samples.push(t0.elapsed());
        assert_eq!(loaded.tracks.len(), 20);
    }
    let load_stats = compute_stats(load_samples);
    println!(
        "load_project: min={:.3}ms median={:.3}ms p95={:.3}ms max={:.3}ms (ceiling < 200ms, budget < 10ms)",
        load_stats.min_ms, load_stats.median_ms, load_stats.p95_ms, load_stats.max_ms
    );
    assert!(
        load_stats.max_ms < 200.0,
        "load_project exceeded qualification ceiling of 200ms"
    );
    assert!(
        load_stats.median_ms < 10.0,
        "load_project exceeded reference median budget of 10ms"
    );

    // 3. CompositionPlan::from_project
    for _ in 0..5 {
        let _ = CompositionPlan::from_project(&fixture_b).unwrap();
    }
    let mut compile_samples = Vec::new();
    for _ in 0..50 {
        let t0 = Instant::now();
        let plan = CompositionPlan::from_project(&fixture_b).unwrap();
        compile_samples.push(t0.elapsed());
        assert_eq!(plan.transitions.len(), 20);
    }
    let compile_stats = compute_stats(compile_samples);
    println!(
        "CompositionPlan::from_project: min={:.3}ms median={:.3}ms p95={:.3}ms max={:.3}ms (ceiling < 20ms, budget < 1ms)",
        compile_stats.min_ms, compile_stats.median_ms, compile_stats.p95_ms, compile_stats.max_ms
    );
    assert!(
        compile_stats.max_ms < 20.0,
        "CompositionPlan::from_project exceeded qualification ceiling of 20ms"
    );
    assert!(
        compile_stats.median_ms < 1.0,
        "CompositionPlan::from_project exceeded reference median budget of 1ms"
    );

    // 4. sample_preview_at (preview semantic sampling latency)
    let plan = CompositionPlan::from_project(&fixture_b).unwrap();
    let rate = FrameRate::FPS_30;
    let sample_times = [
        RationalTime::from_frames(10, rate), // ordinary frame
        RationalTime::from_frames(15, rate), // dense active layers
        RationalTime::from_frames(32, rate), // CrossDissolve active
        RationalTime::from_frames(45, rate), // nonlinear Bézier evaluation
        RationalTime::from_frames(92, rate), // combined transition + automation
    ];

    for t in &sample_times {
        let _ = plan.sample_preview_at(*t);
    }

    let mut sampling_samples = Vec::new();
    for _ in 0..10 {
        for t in &sample_times {
            let t0 = Instant::now();
            let state = plan.sample_preview_at(*t);
            sampling_samples.push(t0.elapsed());
            assert!(!state.active_layers.is_empty() || !state.transition_groups.is_empty());
        }
    }
    let sample_stats = compute_stats(sampling_samples);
    println!(
        "preview semantic sampling latency: min={:.3}ms median={:.3}ms p95={:.3}ms max={:.3}ms (ceiling < 10ms, budget < 0.5ms)",
        sample_stats.min_ms, sample_stats.median_ms, sample_stats.p95_ms, sample_stats.max_ms
    );
    assert!(
        sample_stats.max_ms < 10.0,
        "sample_preview_at exceeded qualification ceiling of 10ms"
    );
    assert!(
        sample_stats.median_ms < 0.5,
        "sample_preview_at exceeded reference median budget of 0.5ms"
    );

    let _ = fs::remove_dir_all(&dir);
}

// ===========================================================================
// P17-AC-25..27: Phase-16 Command Scale Benchmarks on Fixture B
// ===========================================================================

#[test]
#[ignore]
fn test_p17_phase16_command_scale_on_fixture_b() {
    let _lock = BENCH_LOCK.lock().unwrap();
    let fixture_b = build_fixture_b(None);
    let rate = FrameRate::FPS_30;

    println!("\n=== [BENCHMARK] Phase-16 Command Scale on Fixture B (N=50) ===");

    // Command 1: ClipGroupMove 5 items simultaneous translation across tracks
    // Move last clips (c=9) on tracks 0..4 by 1 frame to avoid collision
    let group_items = vec![
        "item-t00-c09".to_string(),
        "item-t01-c09".to_string(),
        "item-t02-c09".to_string(),
        "item-t03-c09".to_string(),
        "item-t04-c09".to_string(),
    ];
    let gm_env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: fixture_b.project_id.clone(),
        base_revision: 0,
        client_id: Some("bench_client".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(1),
        dry_run: true,
        transaction_id: Some("tx-bench-gm".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op-bench-gm".to_string(),
            operation: CommandOperation::ClipGroupMove {
                item_ids: group_items.clone(),
                delta: RationalTime::from_frames(1, rate),
            },
        }],
    };

    let service = CommandService::new(fixture_b.clone());
    for _ in 0..5 {
        let _ = service.dry_run(&gm_env).unwrap();
    }
    let mut gm_dry_samples = Vec::new();
    for _ in 0..50 {
        let t0 = Instant::now();
        let diff = service.dry_run(&gm_env).unwrap();
        gm_dry_samples.push(t0.elapsed());
        assert!(!diff.changes.is_empty());
    }
    let gm_dry_stats = compute_stats(gm_dry_samples);
    println!(
        "ClipGroupMove dry-run: min={:.3}ms median={:.3}ms p95={:.3}ms max={:.3}ms (ceiling < 10ms, budget < 0.2ms)",
        gm_dry_stats.min_ms, gm_dry_stats.median_ms, gm_dry_stats.p95_ms, gm_dry_stats.max_ms
    );
    assert!(
        gm_dry_stats.max_ms < 10.0,
        "ClipGroupMove dry-run ceiling violation"
    );
    assert!(
        gm_dry_stats.median_ms < 0.2,
        "ClipGroupMove dry-run exceeded reference median budget of 0.2ms"
    );

    // ClipGroupMove execute
    let mut gm_exec_env = gm_env.clone();
    gm_exec_env.dry_run = false;
    let mut gm_exec_samples = Vec::new();
    for i in 0..50 {
        let mut local_service = CommandService::new(fixture_b.clone());
        let mut cur_env = gm_exec_env.clone();
        cur_env.client_id = Some(format!("bench_client_{}", i));
        cur_env.sequence_number = Some(1);
        cur_env.transaction_id = Some(format!("tx-bench-gm-{}", i));
        let t0 = Instant::now();
        let receipt = local_service.execute(cur_env).unwrap();
        gm_exec_samples.push(t0.elapsed());
        assert_eq!(receipt.new_revision, 1);
    }
    let gm_exec_stats = compute_stats(gm_exec_samples);
    println!(
        "ClipGroupMove execute: min={:.3}ms median={:.3}ms p95={:.3}ms max={:.3}ms (ceiling < 10ms, budget < 0.2ms)",
        gm_exec_stats.min_ms, gm_exec_stats.median_ms, gm_exec_stats.p95_ms, gm_exec_stats.max_ms
    );
    assert!(
        gm_exec_stats.max_ms < 10.0,
        "ClipGroupMove execute ceiling violation"
    );
    assert!(
        gm_exec_stats.median_ms < 0.2,
        "ClipGroupMove execute exceeded reference median budget of 0.2ms"
    );

    // Command 2: ClipRippleDelete dry-run (middle clip on track 0 + downstream shift)
    let rd_env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: fixture_b.project_id.clone(),
        base_revision: 0,
        client_id: Some("bench_client_rd".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(1),
        dry_run: true,
        transaction_id: Some("tx-bench-rd".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op-bench-rd".to_string(),
            operation: CommandOperation::ClipRippleDelete {
                item_id: "item-t00-c04".to_string(),
            },
        }],
    };
    for _ in 0..5 {
        let _ = service.dry_run(&rd_env).unwrap();
    }
    let mut rd_samples = Vec::new();
    for _ in 0..50 {
        let t0 = Instant::now();
        let diff = service.dry_run(&rd_env).unwrap();
        rd_samples.push(t0.elapsed());
        assert!(!diff.changes.is_empty());
    }
    let rd_stats = compute_stats(rd_samples);
    println!(
        "ClipRippleDelete dry-run: min={:.3}ms median={:.3}ms p95={:.3}ms max={:.3}ms (ceiling < 10ms, budget < 0.2ms)",
        rd_stats.min_ms, rd_stats.median_ms, rd_stats.p95_ms, rd_stats.max_ms
    );
    assert!(
        rd_stats.max_ms < 10.0,
        "ClipRippleDelete dry-run ceiling violation"
    );
    assert!(
        rd_stats.median_ms < 0.2,
        "ClipRippleDelete dry-run exceeded reference median budget of 0.2ms"
    );

    // Command 3: Batched multi-delete dry-run (5 items atomic deletion on Tracks 10..14)
    let multi_delete_env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: fixture_b.project_id.clone(),
        base_revision: 0,
        client_id: Some("bench_client_md".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(1),
        dry_run: true,
        transaction_id: Some("tx-bench-md".to_string()),
        operations: vec![
            OperationEnvelope {
                operation_id: "op-md-1".to_string(),
                operation: CommandOperation::ClipDelete {
                    item_id: "item-t10-c09".to_string(),
                },
            },
            OperationEnvelope {
                operation_id: "op-md-2".to_string(),
                operation: CommandOperation::ClipDelete {
                    item_id: "item-t11-c09".to_string(),
                },
            },
            OperationEnvelope {
                operation_id: "op-md-3".to_string(),
                operation: CommandOperation::ClipDelete {
                    item_id: "item-t12-c09".to_string(),
                },
            },
            OperationEnvelope {
                operation_id: "op-md-4".to_string(),
                operation: CommandOperation::ClipDelete {
                    item_id: "item-t13-c09".to_string(),
                },
            },
            OperationEnvelope {
                operation_id: "op-md-5".to_string(),
                operation: CommandOperation::ClipDelete {
                    item_id: "item-t14-c09".to_string(),
                },
            },
        ],
    };
    for _ in 0..5 {
        let _ = service.dry_run(&multi_delete_env).unwrap();
    }
    let mut md_samples = Vec::new();
    for _ in 0..50 {
        let t0 = Instant::now();
        let diff = service.dry_run(&multi_delete_env).unwrap();
        md_samples.push(t0.elapsed());
        assert_eq!(diff.changes.len(), 5);
    }
    let md_stats = compute_stats(md_samples);
    println!(
        "Batched multi-delete dry-run: min={:.3}ms median={:.3}ms p95={:.3}ms max={:.3}ms (ceiling < 10ms, budget < 0.2ms)",
        md_stats.min_ms, md_stats.median_ms, md_stats.p95_ms, md_stats.max_ms
    );
    assert!(
        md_stats.max_ms < 10.0,
        "Batched multi-delete dry-run ceiling violation"
    );
    assert!(
        md_stats.median_ms < 0.2,
        "Batched multi-delete dry-run exceeded reference median budget of 0.2ms"
    );
}

// ===========================================================================
// P17-AC-28..31: Real JSONL OS-Stdio Subprocess Qualification
// ===========================================================================

#[test]
#[ignore]
fn test_p17_jsonl_os_stdio_session_scale() {
    let _lock = BENCH_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_tinixcut");
    let dir = test_dir("jsonl_bench");
    let project_file = dir.join("fixture_b_jsonl.tcproj");
    let fixture_b = build_fixture_b(Some(&dir));
    save_project_atomic(&fixture_b, &project_file).unwrap();

    let mut child = Command::new(bin)
        .arg("session")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn tinixcut session subprocess");

    let mut stdin = child.stdin.take().expect("Failed to open child stdin");
    let stdout = child.stdout.take().expect("Failed to open child stdout");
    let mut reader = BufReader::new(stdout);

    // 1. Open project
    let req_open = serde_json::json!({
        "protocolVersion": 1,
        "requestId": "open-01",
        "method": "session.open",
        "params": { "path": project_file.to_str().unwrap() }
    });
    stdin
        .write_all(format!("{}\n", serde_json::to_string(&req_open).unwrap()).as_bytes())
        .unwrap();
    stdin.flush().unwrap();
    let mut line_open = String::new();
    reader.read_line(&mut line_open).unwrap();
    let val_open: serde_json::Value = serde_json::from_str(&line_open).expect("JSON open response");
    assert_eq!(val_open["ok"], true, "session.open failed: {line_open}");

    println!("\n=== [BENCHMARK] JSONL OS-Stdio Automation Session (N=50) ===");

    // 2. Steady-state inspect (5 warmups, 50 measured)
    for _ in 0..5 {
        let req = serde_json::json!({
            "protocolVersion": 1,
            "requestId": "warmup-inspect",
            "method": "project.inspect",
            "params": {}
        });
        stdin
            .write_all(format!("{}\n", serde_json::to_string(&req).unwrap()).as_bytes())
            .unwrap();
        stdin.flush().unwrap();
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
    }
    let mut inspect_samples = Vec::new();
    let inspect_wall_start = Instant::now();
    for i in 0..50 {
        let req_id = format!("inspect-{}", i);
        let req = serde_json::json!({
            "protocolVersion": 1,
            "requestId": req_id,
            "method": "project.inspect",
            "params": {}
        });
        let req_bytes = format!("{}\n", serde_json::to_string(&req).unwrap());
        let mut line = String::new();

        let t0 = Instant::now();
        stdin.write_all(req_bytes.as_bytes()).unwrap();
        stdin.flush().unwrap();
        reader.read_line(&mut line).unwrap();
        let val: serde_json::Value = serde_json::from_str(&line).expect("JSON inspect response");
        let elapsed = t0.elapsed();

        assert_eq!(val["requestId"], req_id);
        assert_eq!(val["ok"], true);
        inspect_samples.push(elapsed);
    }
    let inspect_wall_elapsed = inspect_wall_start.elapsed();
    let inspect_stats = compute_stats(inspect_samples);
    println!(
        "JSONL project.inspect: min={:.3}ms median={:.3}ms p95={:.3}ms max={:.3}ms | 50-req aggregate wall={:.3}ms (avg={:.3}ms/call) (ceiling < 10ms, median budget < 0.15ms)",
        inspect_stats.min_ms, inspect_stats.median_ms, inspect_stats.p95_ms, inspect_stats.max_ms,
        inspect_wall_elapsed.as_secs_f64() * 1000.0,
        inspect_wall_elapsed.as_secs_f64() * 1000.0 / 50.0
    );
    assert!(
        inspect_stats.max_ms < 10.0,
        "JSONL project.inspect ceiling violation"
    );
    assert!(
        inspect_stats.median_ms < 0.15,
        "JSONL project.inspect median budget violation"
    );

    // 3. Steady-state dry-run (5 warmups, 50 measured)
    let rate = FrameRate::FPS_30;
    let dry_env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: fixture_b.project_id.clone(),
        base_revision: 0,
        client_id: Some("session".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(1),
        dry_run: true,
        transaction_id: Some("tx-jsonl-dry".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op-jsonl-dry".to_string(),
            operation: CommandOperation::ClipGroupMove {
                item_ids: vec!["item-t00-c09".to_string()],
                delta: RationalTime::from_frames(1, rate),
            },
        }],
    };
    for _ in 0..5 {
        let req = serde_json::json!({
            "protocolVersion": 1,
            "requestId": "warmup-dry",
            "method": "command.dryRun",
            "params": { "envelope": dry_env.clone() }
        });
        stdin
            .write_all(format!("{}\n", serde_json::to_string(&req).unwrap()).as_bytes())
            .unwrap();
        stdin.flush().unwrap();
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
    }
    let mut dry_samples = Vec::new();
    let dry_wall_start = Instant::now();
    for i in 0..50 {
        let req_id = format!("dry-{}", i);
        let req = serde_json::json!({
            "protocolVersion": 1,
            "requestId": req_id,
            "method": "command.dryRun",
            "params": { "envelope": dry_env.clone() }
        });
        let req_bytes = format!("{}\n", serde_json::to_string(&req).unwrap());
        let mut line = String::new();

        let t0 = Instant::now();
        stdin.write_all(req_bytes.as_bytes()).unwrap();
        stdin.flush().unwrap();
        reader.read_line(&mut line).unwrap();
        let val: serde_json::Value = serde_json::from_str(&line).expect("JSON dryRun response");
        let elapsed = t0.elapsed();

        assert_eq!(val["requestId"], req_id);
        assert_eq!(val["ok"], true);
        dry_samples.push(elapsed);
    }
    let dry_wall_elapsed = dry_wall_start.elapsed();
    let dry_stats = compute_stats(dry_samples);
    println!(
        "JSONL command.dryRun: min={:.3}ms median={:.3}ms p95={:.3}ms max={:.3}ms | 50-req aggregate wall={:.3}ms (avg={:.3}ms/call) (ceiling < 10ms, median budget < 0.15ms)",
        dry_stats.min_ms, dry_stats.median_ms, dry_stats.p95_ms, dry_stats.max_ms,
        dry_wall_elapsed.as_secs_f64() * 1000.0,
        dry_wall_elapsed.as_secs_f64() * 1000.0 / 50.0
    );
    assert!(
        dry_stats.max_ms < 10.0,
        "JSONL command.dryRun ceiling violation"
    );
    assert!(
        dry_stats.median_ms < 0.15,
        "JSONL command.dryRun median budget violation"
    );

    // 4. Steady-state in-memory apply (saveAfter: false) (5 warmups, 50 measured)
    let mut apply_samples = Vec::new();
    let mut cur_revision = 0u64;

    // 5 warmups for apply
    for w in 0..5 {
        let warmup_env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: fixture_b.project_id.clone(),
            base_revision: cur_revision,
            client_id: Some("session".to_string()),
            client_epoch: Some(1),
            sequence_number: Some((w + 1) as u64),
            dry_run: false,
            transaction_id: Some(format!("tx-jsonl-warmup-{}", w)),
            operations: vec![OperationEnvelope {
                operation_id: format!("op-jsonl-warmup-{}", w),
                operation: CommandOperation::ClipGroupMove {
                    item_ids: vec!["item-t00-c09".to_string()],
                    delta: RationalTime::from_frames(1, rate),
                },
            }],
        };
        let req = serde_json::json!({
            "protocolVersion": 1,
            "requestId": format!("warmup-apply-{}", w),
            "method": "command.apply",
            "params": { "envelope": warmup_env, "saveAfter": false }
        });
        stdin
            .write_all(format!("{}\n", serde_json::to_string(&req).unwrap()).as_bytes())
            .unwrap();
        stdin.flush().unwrap();
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        cur_revision += 1;
    }

    let apply_wall_start = Instant::now();
    for i in 0..50 {
        let req_id = format!("apply-{}", i);
        let apply_env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: fixture_b.project_id.clone(),
            base_revision: cur_revision,
            client_id: Some("session".to_string()),
            client_epoch: Some(1),
            sequence_number: Some((6 + i) as u64),
            dry_run: false,
            transaction_id: Some(format!("tx-jsonl-apply-{}", i)),
            operations: vec![OperationEnvelope {
                operation_id: format!("op-jsonl-apply-{}", i),
                operation: CommandOperation::ClipGroupMove {
                    item_ids: vec!["item-t00-c09".to_string()],
                    delta: RationalTime::from_frames(1, rate),
                },
            }],
        };
        let req = serde_json::json!({
            "protocolVersion": 1,
            "requestId": req_id,
            "method": "command.apply",
            "params": { "envelope": apply_env, "saveAfter": false }
        });
        let req_bytes = format!("{}\n", serde_json::to_string(&req).unwrap());
        let mut line = String::new();

        let t0 = Instant::now();
        stdin.write_all(req_bytes.as_bytes()).unwrap();
        stdin.flush().unwrap();
        reader.read_line(&mut line).unwrap();
        let val: serde_json::Value = serde_json::from_str(&line).expect("JSON apply response");
        let elapsed = t0.elapsed();

        assert_eq!(val["requestId"], req_id);
        assert_eq!(val["ok"], true, "JSONL apply failed: {val}");
        cur_revision += 1;
        apply_samples.push(elapsed);
    }
    let apply_wall_elapsed = apply_wall_start.elapsed();
    let apply_stats = compute_stats(apply_samples);
    println!(
        "JSONL command.apply (in-memory saveAfter:false): min={:.3}ms median={:.3}ms p95={:.3}ms max={:.3}ms | 50-req aggregate wall={:.3}ms (avg={:.3}ms/call) (ceiling < 10ms, median budget < 0.15ms)",
        apply_stats.min_ms, apply_stats.median_ms, apply_stats.p95_ms, apply_stats.max_ms,
        apply_wall_elapsed.as_secs_f64() * 1000.0,
        apply_wall_elapsed.as_secs_f64() * 1000.0 / 50.0
    );
    assert!(
        apply_stats.max_ms < 10.0,
        "JSONL in-memory apply ceiling violation"
    );
    assert!(
        apply_stats.median_ms < 0.15,
        "JSONL in-memory apply median budget violation"
    );

    // Shutdown
    let req_shutdown = serde_json::json!({
        "protocolVersion": 1,
        "requestId": "shutdown-01",
        "method": "session.shutdown"
    });
    stdin
        .write_all(format!("{}\n", serde_json::to_string(&req_shutdown).unwrap()).as_bytes())
        .unwrap();
    stdin.flush().unwrap();
    let _ = child.wait().unwrap();
    let _ = fs::remove_dir_all(&dir);
}

// ===========================================================================
// P17-AC-32..38: Real MCP OS-Stdio Subprocess Qualification
// ===========================================================================

#[test]
#[ignore]
fn test_p17_mcp_os_stdio_scale_and_persistence() {
    let _lock = BENCH_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_tinixcut");
    let dir = test_dir("mcp_bench");
    let project_file = dir.join("fixture_b_mcp.tcproj");
    let fixture_b = build_fixture_b(Some(&dir));
    save_project_atomic(&fixture_b, &project_file).unwrap();

    let mut child = Command::new(bin)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn tinixcut mcp subprocess");

    let mut stdin = child.stdin.take().expect("Failed to open child stdin");
    let stdout = child.stdout.take().expect("Failed to open child stdout");
    let mut reader = BufReader::new(stdout);

    println!("\n=== [BENCHMARK] Validated MCP OS-Stdio Protocol (N=30) ===");

    let mcp_meta = serde_json::json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientCapabilities": {},
        "io.modelcontextprotocol/clientInfo": {
            "name": "phase17-bench-agent",
            "version": "1.0.0"
        }
    });

    // 1. Handshake server/discover
    let disc_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "server/discover",
        "params": {
            "_meta": mcp_meta.clone()
        }
    });
    stdin
        .write_all(format!("{}\n", serde_json::to_string(&disc_req).unwrap()).as_bytes())
        .unwrap();
    stdin.flush().unwrap();
    let mut line_disc = String::new();
    reader.read_line(&mut line_disc).unwrap();
    let val_disc: serde_json::Value =
        serde_json::from_str(&line_disc).expect("MCP discover response");
    assert_eq!(val_disc["id"], 1);
    assert!(val_disc.get("error").is_none());

    // 2. tinixcut_inspect (5 warmups, 30 measured)
    for i in 0..5 {
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 10 + i,
            "method": "tools/call",
            "params": {
                "_meta": mcp_meta.clone(),
                "name": "tinixcut_inspect",
                "arguments": { "projectPath": project_file.to_str().unwrap() }
            }
        });
        stdin
            .write_all(format!("{}\n", serde_json::to_string(&req).unwrap()).as_bytes())
            .unwrap();
        stdin.flush().unwrap();
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
    }
    let mut inspect_samples = Vec::new();
    let inspect_wall_start = Instant::now();
    for i in 0..30 {
        let req_id = 100 + i;
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": req_id,
            "method": "tools/call",
            "params": {
                "_meta": mcp_meta.clone(),
                "name": "tinixcut_inspect",
                "arguments": { "projectPath": project_file.to_str().unwrap() }
            }
        });
        let req_bytes = format!("{}\n", serde_json::to_string(&req).unwrap());
        let mut line = String::new();

        let t0 = Instant::now();
        stdin.write_all(req_bytes.as_bytes()).unwrap();
        stdin.flush().unwrap();
        reader.read_line(&mut line).unwrap();
        let val: serde_json::Value = serde_json::from_str(&line).expect("MCP inspect response");
        let elapsed = t0.elapsed();

        assert_eq!(val["id"], req_id);
        assert!(val.get("error").is_none(), "RPC error: {line}");
        assert_ne!(val["result"]["isError"], true, "MCP inspect error: {line}");
        inspect_samples.push(elapsed);
    }
    let inspect_wall_elapsed = inspect_wall_start.elapsed();
    let inspect_stats = compute_stats(inspect_samples);
    println!(
        "MCP tinixcut_inspect: min={:.3}ms median={:.3}ms p95={:.3}ms max={:.3}ms | 30-req aggregate wall={:.3}ms (avg={:.3}ms/call) (ceiling < 20ms, median budget < 5ms)",
        inspect_stats.min_ms, inspect_stats.median_ms, inspect_stats.p95_ms, inspect_stats.max_ms,
        inspect_wall_elapsed.as_secs_f64() * 1000.0,
        inspect_wall_elapsed.as_secs_f64() * 1000.0 / 30.0
    );
    assert!(
        inspect_stats.max_ms < 20.0,
        "MCP tinixcut_inspect ceiling violation"
    );
    assert!(
        inspect_stats.median_ms < 5.0,
        "MCP tinixcut_inspect median budget violation"
    );

    // 3. tinixcut_dry_run (5 warmups, 30 measured)
    let rate = FrameRate::FPS_30;
    let dry_env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: fixture_b.project_id.clone(),
        base_revision: 0,
        client_id: Some("mcp_agent".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(1),
        dry_run: true,
        transaction_id: Some("tx-mcp-dry".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op-mcp-dry".to_string(),
            operation: CommandOperation::ClipGroupMove {
                item_ids: vec!["item-t00-c09".to_string()],
                delta: RationalTime::from_frames(1, rate),
            },
        }],
    };
    for i in 0..5 {
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 20 + i,
            "method": "tools/call",
            "params": {
                "_meta": mcp_meta.clone(),
                "name": "tinixcut_dry_run",
                "arguments": {
                    "projectPath": project_file.to_str().unwrap(),
                    "envelope": dry_env.clone()
                }
            }
        });
        stdin
            .write_all(format!("{}\n", serde_json::to_string(&req).unwrap()).as_bytes())
            .unwrap();
        stdin.flush().unwrap();
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
    }
    let mut dry_samples = Vec::new();
    let dry_wall_start = Instant::now();
    for i in 0..30 {
        let req_id = 200 + i;
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": req_id,
            "method": "tools/call",
            "params": {
                "_meta": mcp_meta.clone(),
                "name": "tinixcut_dry_run",
                "arguments": {
                    "projectPath": project_file.to_str().unwrap(),
                    "envelope": dry_env.clone()
                }
            }
        });
        let req_bytes = format!("{}\n", serde_json::to_string(&req).unwrap());
        let mut line = String::new();

        let t0 = Instant::now();
        stdin.write_all(req_bytes.as_bytes()).unwrap();
        stdin.flush().unwrap();
        reader.read_line(&mut line).unwrap();
        let val: serde_json::Value = serde_json::from_str(&line).expect("MCP dryRun response");
        let elapsed = t0.elapsed();

        assert_eq!(val["id"], req_id);
        assert!(val.get("error").is_none(), "RPC error: {line}");
        assert_ne!(val["result"]["isError"], true, "MCP dry_run error: {line}");
        dry_samples.push(elapsed);
    }
    let dry_wall_elapsed = dry_wall_start.elapsed();
    let dry_stats = compute_stats(dry_samples);
    println!(
        "MCP tinixcut_dry_run: min={:.3}ms median={:.3}ms p95={:.3}ms max={:.3}ms | 30-req aggregate wall={:.3}ms (avg={:.3}ms/call) (ceiling < 20ms, median budget < 5ms)",
        dry_stats.min_ms, dry_stats.median_ms, dry_stats.p95_ms, dry_stats.max_ms,
        dry_wall_elapsed.as_secs_f64() * 1000.0,
        dry_wall_elapsed.as_secs_f64() * 1000.0 / 30.0
    );
    assert!(
        dry_stats.max_ms < 20.0,
        "MCP tinixcut_dry_run ceiling violation"
    );
    assert!(
        dry_stats.median_ms < 5.0,
        "MCP tinixcut_dry_run median budget violation"
    );

    // 4. tinixcut_apply persistent (durable reload + mutate + atomic save) (5 warmups, 30 measured)
    let mut apply_samples = Vec::new();
    let mut expected_revision = 0u64;

    // 5 warmups for apply
    for w in 0..5 {
        let warmup_env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: fixture_b.project_id.clone(),
            base_revision: expected_revision,
            client_id: Some("mcp_agent".to_string()),
            client_epoch: Some(1),
            sequence_number: Some((w + 1) as u64),
            dry_run: false,
            transaction_id: Some(format!("tx-mcp-warmup-{}", w)),
            operations: vec![OperationEnvelope {
                operation_id: format!("op-mcp-warmup-{}", w),
                operation: CommandOperation::ClipGroupMove {
                    item_ids: vec!["item-t00-c09".to_string()],
                    delta: RationalTime::from_frames(1, rate),
                },
            }],
        };
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 50 + w,
            "method": "tools/call",
            "params": {
                "_meta": mcp_meta.clone(),
                "name": "tinixcut_apply",
                "arguments": {
                    "projectPath": project_file.to_str().unwrap(),
                    "envelope": warmup_env
                }
            }
        });
        stdin
            .write_all(format!("{}\n", serde_json::to_string(&req).unwrap()).as_bytes())
            .unwrap();
        stdin.flush().unwrap();
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        expected_revision += 1;
    }

    let apply_wall_start = Instant::now();
    for i in 0..30 {
        let req_id = 300 + i;
        let apply_env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: fixture_b.project_id.clone(),
            base_revision: expected_revision,
            client_id: Some("mcp_agent".to_string()),
            client_epoch: Some(1),
            sequence_number: Some((6 + i) as u64),
            dry_run: false,
            transaction_id: Some(format!("tx-mcp-apply-{}", i)),
            operations: vec![OperationEnvelope {
                operation_id: format!("op-mcp-apply-{}", i),
                operation: CommandOperation::ClipGroupMove {
                    item_ids: vec!["item-t00-c09".to_string()],
                    delta: RationalTime::from_frames(1, rate),
                },
            }],
        };
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": req_id,
            "method": "tools/call",
            "params": {
                "_meta": mcp_meta.clone(),
                "name": "tinixcut_apply",
                "arguments": {
                    "projectPath": project_file.to_str().unwrap(),
                    "envelope": apply_env
                }
            }
        });
        let req_bytes = format!("{}\n", serde_json::to_string(&req).unwrap());
        let mut line = String::new();

        let t0 = Instant::now();
        stdin.write_all(req_bytes.as_bytes()).unwrap();
        stdin.flush().unwrap();
        reader.read_line(&mut line).unwrap();
        let val: serde_json::Value = serde_json::from_str(&line).expect("MCP apply response");
        let elapsed = t0.elapsed();

        assert_eq!(val["id"], req_id);
        assert!(val.get("error").is_none(), "RPC error: {line}");
        assert_ne!(
            val["result"]["isError"], true,
            "tinixcut_apply failed: {line}"
        );

        expected_revision += 1;

        // Independent reload confirms mutation & durable atomic write
        let reloaded = load_project(&project_file).unwrap();
        assert_eq!(
            reloaded.revision, expected_revision,
            "Durable persistence verification failed for apply #{i}"
        );

        apply_samples.push(elapsed);
    }
    let apply_wall_elapsed = apply_wall_start.elapsed();
    let apply_stats = compute_stats(apply_samples);
    println!(
        "MCP tinixcut_apply (durable persist): min={:.3}ms median={:.3}ms p95={:.3}ms max={:.3}ms | 30-req aggregate wall={:.3}ms (avg={:.3}ms/call) (ceiling < 100ms, budget < 20ms)",
        apply_stats.min_ms, apply_stats.median_ms, apply_stats.p95_ms, apply_stats.max_ms,
        apply_wall_elapsed.as_secs_f64() * 1000.0,
        apply_wall_elapsed.as_secs_f64() * 1000.0 / 30.0
    );
    assert!(
        apply_stats.max_ms < 100.0,
        "MCP tinixcut_apply ceiling violation"
    );
    assert!(
        apply_stats.median_ms < 20.0,
        "MCP tinixcut_apply median budget violation"
    );

    // Clean shutdown: drop stdin so child gets EOF on OS pipe
    drop(stdin);
    let _ = child.wait().unwrap();
    let _ = fs::remove_dir_all(&dir);
}

// ===========================================================================
// P17-AC-39..44: Fixture-A CPU Export Qualification (720p30 & 1080p30)
// ===========================================================================

#[test]
#[ignore]
fn test_p17_fixture_a_cpu_libx264_export_qualification() {
    let _lock = BENCH_LOCK.lock().unwrap();
    let dir = test_dir("export_bench");
    println!("\n=== [BENCHMARK] Fixture A CPU libx264 Export Qualification ===");

    // 1. 720p30 CPU Export Qualification (RTF >= 2.20x budget, ceiling >= 1.00x)
    let (v_720, a_720) = ensure_fixture_a_synthetic_media(&dir, FixtureAVariant::Resolution720p)
        .expect("Synthetic media 720p generation");
    let proj_720 = build_fixture_a(FixtureAVariant::Resolution720p, Some(&v_720), Some(&a_720));

    let profile_720 = ExportProfile {
        width: 1280,
        height: 720,
        frame_rate: FrameRate::FPS_30,
        crf: 23,
        preset: "fast".to_string(),
        video_codec: "libx264".to_string(),
        audio_codec: "aac".to_string(),
        audio_bitrate: "192k".to_string(),
        audio_sample_rate: 48000,
    };

    let out_720 = dir.join("bench_export_720p.mp4");

    // Warmup run
    println!("Running 720p30 warmup run...");
    let _ = fs::remove_file(&out_720);
    ExportEngine::render_snapshot(&proj_720, &out_720, &profile_720, None)
        .expect("720p warmup render failed");

    // 5 Measured runs
    let mut wall_times_720 = Vec::new();
    let mut rtfs_720 = Vec::new();
    for run in 1..=5 {
        let _ = fs::remove_file(&out_720);
        let t0 = Instant::now();
        let report = ExportEngine::render_snapshot(&proj_720, &out_720, &profile_720, None)
            .expect("720p measured render failed");
        let elapsed = t0.elapsed();
        let wall_s = elapsed.as_secs_f64();
        let rtf = 6.0 / wall_s;
        wall_times_720.push(elapsed);
        rtfs_720.push(rtf);
        println!(
            "720p30 Run #{run}: wall={:.3}s RTF={:.2}x encoder={:?}",
            wall_s, rtf, report.encoder_used
        );
        assert_eq!(report.encoder_used.as_deref(), Some("libx264"));
    }

    let stats_720 = compute_stats(wall_times_720);
    rtfs_720.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let min_rtf_720 = rtfs_720.first().unwrap();
    let median_rtf_720 = rtfs_720[2];
    let max_rtf_720 = rtfs_720.last().unwrap();

    println!(
        "720p30 Aggregate: median_wall={:.3}s median_RTF={:.2}x (min_RTF={:.2}x, max_RTF={:.2}x)",
        stats_720.median_ms / 1000.0,
        median_rtf_720,
        min_rtf_720,
        max_rtf_720
    );

    // Validate probed output contract
    let probe_720 = FFmpegService::probe_file(&out_720).expect("Probe 720p output");
    assert_eq!(probe_720.width, 1280);
    assert_eq!(probe_720.height, 720);
    assert_eq!(probe_720.video_codec.as_deref(), Some("h264"));
    assert_eq!(probe_720.audio_codec.as_deref(), Some("aac"));
    let dur_720 = probe_720.duration_seconds;
    assert!(
        (dur_720 - 6.0).abs() < 0.25,
        "Output duration must be 6.0s (found {dur_720:.2}s)"
    );

    // 2. 1080p30 CPU Export Qualification (RTF >= 1.00x budget, ceiling >= 0.50x)
    let (v_1080, a_1080) = ensure_fixture_a_synthetic_media(&dir, FixtureAVariant::Resolution1080p)
        .expect("Synthetic media 1080p generation");
    let proj_1080 = build_fixture_a(
        FixtureAVariant::Resolution1080p,
        Some(&v_1080),
        Some(&a_1080),
    );

    let profile_1080 = ExportProfile {
        width: 1920,
        height: 1080,
        frame_rate: FrameRate::FPS_30,
        crf: 23,
        preset: "fast".to_string(),
        video_codec: "libx264".to_string(),
        audio_codec: "aac".to_string(),
        audio_bitrate: "192k".to_string(),
        audio_sample_rate: 48000,
    };

    let out_1080 = dir.join("bench_export_1080p.mp4");

    // Warmup run
    println!("\nRunning 1080p30 warmup run...");
    let _ = fs::remove_file(&out_1080);
    ExportEngine::render_snapshot(&proj_1080, &out_1080, &profile_1080, None)
        .expect("1080p warmup render failed");

    // 5 Measured runs
    let mut wall_times_1080 = Vec::new();
    let mut rtfs_1080 = Vec::new();
    for run in 1..=5 {
        let _ = fs::remove_file(&out_1080);
        let t0 = Instant::now();
        let report = ExportEngine::render_snapshot(&proj_1080, &out_1080, &profile_1080, None)
            .expect("1080p measured render failed");
        let elapsed = t0.elapsed();
        let wall_s = elapsed.as_secs_f64();
        let rtf = 6.0 / wall_s;
        wall_times_1080.push(elapsed);
        rtfs_1080.push(rtf);
        println!(
            "1080p30 Run #{run}: wall={:.3}s RTF={:.2}x encoder={:?}",
            wall_s, rtf, report.encoder_used
        );
        assert_eq!(report.encoder_used.as_deref(), Some("libx264"));
    }

    let stats_1080 = compute_stats(wall_times_1080);
    rtfs_1080.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let min_rtf_1080 = rtfs_1080.first().unwrap();
    let median_rtf_1080 = rtfs_1080[2];
    let max_rtf_1080 = rtfs_1080.last().unwrap();

    println!(
        "1080p30 Aggregate: median_wall={:.3}s median_RTF={:.2}x (min_RTF={:.2}x, max_RTF={:.2}x)",
        stats_1080.median_ms / 1000.0,
        median_rtf_1080,
        min_rtf_1080,
        max_rtf_1080
    );

    // Validate probed output contract
    let probe_1080 = FFmpegService::probe_file(&out_1080).expect("Probe 1080p output");
    assert_eq!(probe_1080.width, 1920);
    assert_eq!(probe_1080.height, 1080);
    assert_eq!(probe_1080.video_codec.as_deref(), Some("h264"));
    assert_eq!(probe_1080.audio_codec.as_deref(), Some("aac"));
    let dur_1080 = probe_1080.duration_seconds;
    assert!(
        (dur_1080 - 6.0).abs() < 0.25,
        "Output duration must be 6.0s (found {dur_1080:.2}s)"
    );

    // Assert qualification ceilings
    assert!(
        median_rtf_720 >= 1.0,
        "720p30 median RTF ({:.2}x) violated qualification ceiling (>= 1.00x)",
        median_rtf_720
    );
    assert!(
        median_rtf_1080 >= 0.50,
        "1080p30 median RTF ({:.2}x) violated qualification ceiling (>= 0.50x)",
        median_rtf_1080
    );
    assert!(
        median_rtf_720 >= 2.20,
        "720p30 median RTF ({:.2}x) violated reference median budget (>= 2.20x)",
        median_rtf_720
    );
    assert!(
        median_rtf_1080 >= 1.00,
        "1080p30 median RTF ({:.2}x) violated reference median budget (>= 1.00x)",
        median_rtf_1080
    );

    let _ = fs::remove_dir_all(&dir);
}
