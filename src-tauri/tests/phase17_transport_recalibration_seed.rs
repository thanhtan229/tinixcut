#![allow(dead_code)]

#[path = "phase17_fixtures/mod.rs"]
mod phase17_fixtures;

use std::fs;
use std::time::Instant;
use tinixcut_lib::modules::command::{
    CommandEnvelope, CommandOperation, CommandService, OperationEnvelope,
};
use tinixcut_lib::modules::domain::CURRENT_SCHEMA_VERSION;
use tinixcut_lib::modules::domain::{Project, Track, TrackKind};
use tinixcut_lib::modules::persistence::{load_project, save_project_atomic};

#[test]
fn seed_transport_recalibration_workloads() {
    let root = std::env::temp_dir().join("tinixcut_phase17_transport_recalibration");
    fs::create_dir_all(&root).unwrap();

    let mut w0 = Project::new("W0 Minimal".to_string());
    w0.project_id = "proj-recalibration-w0".to_string();
    let mut track = Track::new("Video".to_string(), TrackKind::Video);
    track.track_id = "track-recalibration-w0".to_string();
    w0.tracks.push(track);
    let w1 = phase17_fixtures::build_fixture_a(
        phase17_fixtures::FixtureAVariant::Resolution720p,
        None,
        None,
    );
    let w2 = phase17_fixtures::build_fixture_b(Some(&root));

    for (name, project) in [("W0", w0), ("W1", w1), ("W2", w2)] {
        let path = root.join(format!("{name}.tcproj"));
        save_project_atomic(&project, &path).unwrap();
        assert_eq!(load_project(&path).unwrap().project_id, project.project_id);
        println!(
            "SEED {name} path={} bytes={} tracks={} items={}",
            path.display(),
            fs::metadata(&path).unwrap().len(),
            project.tracks.len(),
            project
                .tracks
                .iter()
                .map(|track| track.items.len())
                .sum::<usize>()
        );
    }

    println!("SEED_ROOT={}", root.display());
    let path = std::env::temp_dir()
        .join("tinixcut_phase17_transport_recalibration")
        .join("W2.tcproj");
    for _ in 0..5 {
        let _ = load_project(&path).unwrap();
    }
    let mut load_samples = Vec::new();
    for _ in 0..50 {
        let started = Instant::now();
        let loaded = load_project(&path).unwrap();
        load_samples.push(started.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(loaded.tracks.len(), 20);
    }
    load_samples.sort_by(f64::total_cmp);
    println!(
        "DIRECT_LOAD_W2_N50_MEDIAN_MS={:.3} P95_MS={:.3} AVG_MS={:.3}",
        load_samples[24],
        load_samples[47],
        load_samples.iter().sum::<f64>() / 50.0
    );

    let project = load_project(&path).unwrap();
    for _ in 0..5 {
        save_project_atomic(&project, &path).unwrap();
    }
    let mut save_samples = Vec::new();
    for _ in 0..50 {
        let started = Instant::now();
        save_project_atomic(&project, &path).unwrap();
        save_samples.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    save_samples.sort_by(f64::total_cmp);
    println!(
        "DIRECT_SAVE_W2_N50_MEDIAN_MS={:.3} P95_MS={:.3} AVG_MS={:.3}",
        save_samples[24],
        save_samples[47],
        save_samples.iter().sum::<f64>() / 50.0
    );

    let initial = load_project(&path).unwrap();
    let dry_env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: initial.project_id.clone(),
        base_revision: initial.revision,
        client_id: Some("recalibration-dry".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(1),
        dry_run: true,
        transaction_id: Some("recalibration-dry-tx".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "recalibration-dry-op".to_string(),
            operation: CommandOperation::TrackMuteSet {
                track_id: initial.tracks[0].track_id.clone(),
                muted: true,
            },
        }],
    };
    let service = CommandService::new(initial.clone());
    for _ in 0..5 {
        service.dry_run(&dry_env).unwrap();
    }
    let mut dry_samples = Vec::new();
    for _ in 0..50 {
        let started = Instant::now();
        service.dry_run(&dry_env).unwrap();
        dry_samples.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    dry_samples.sort_by(f64::total_cmp);
    println!(
        "COMMANDSERVICE_DRYRUN_W2_N50_MEDIAN_MS={:.3} P95_MS={:.3} AVG_MS={:.3}",
        dry_samples[24],
        dry_samples[47],
        dry_samples.iter().sum::<f64>() / 50.0
    );

    let mut apply_samples = Vec::new();
    for i in 0..50 {
        let mut local_service = CommandService::new(initial.clone());
        let mut envelope = dry_env.clone();
        envelope.dry_run = false;
        envelope.client_id = Some(format!("recalibration-apply-{i}"));
        envelope.sequence_number = Some(1);
        envelope.transaction_id = Some(format!("recalibration-apply-tx-{i}"));
        envelope.operations[0].operation_id = format!("recalibration-apply-op-{i}");
        let started = Instant::now();
        local_service.execute(envelope).unwrap();
        apply_samples.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    apply_samples.sort_by(f64::total_cmp);
    println!(
        "COMMANDSERVICE_APPLY_W2_N50_MEDIAN_MS={:.3} P95_MS={:.3} AVG_MS={:.3}",
        apply_samples[24],
        apply_samples[47],
        apply_samples.iter().sum::<f64>() / 50.0
    );
}
