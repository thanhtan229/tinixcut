//! Reliability, Security, and Performance Hardening Module
//! Verifies product behavior against harsh failure modes, shell injection attempts,
//! queue backpressure, and large project scaling.

#[cfg(test)]
pub mod tests {
    use crate::modules::command::{
        CommandEnvelope, CommandOperation, CommandService, OperationEnvelope,
    };
    use crate::modules::composition::CompositionPlan;
    use crate::modules::domain::time::{FrameRate, RationalTime};
    use crate::modules::domain::{
        ItemKind, Project, ProjectSettings, TrackKind, CURRENT_SCHEMA_VERSION,
    };
    use crate::modules::job::{JobKind, JobManager, JobStatus};
    use crate::modules::media::ffmpeg::FFmpegService;
    use crate::modules::persistence::{load_project, save_project_atomic};
    use std::fs;
    use std::path::PathBuf;
    use std::time::Instant;

    fn get_fixture_dir() -> PathBuf {
        let dir = std::env::temp_dir().join("tinixcut_test_fixtures_phase11");
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 11.1 Test corrupt project file loading does not panic and returns explicit error
    #[test]
    fn test_corrupt_project_file_handling() {
        let fixture_dir = get_fixture_dir();
        let corrupt_path = fixture_dir.join("corrupted.tcproj");
        fs::write(&corrupt_path, b"NOT_A_VALID_JSON_BINARY_\x00\xFF\xFE_DATA").unwrap();

        let load_result = load_project(&corrupt_path);
        assert!(
            load_result.is_err(),
            "Corrupt project file must fail to load"
        );
    }

    /// 11.1 Test corrupt media handling returns explicit probe error without panicking
    #[test]
    fn test_corrupt_media_probe_safe_failure() {
        let fixture_dir = get_fixture_dir();
        let fake_video = fixture_dir.join("broken_stream.mp4");
        fs::write(&fake_video, b"CORRUPTED_VIDEO_BYTES_RANDOM_NOISE_12345").unwrap();

        let probe_res = FFmpegService::probe_file(&fake_video);
        assert!(probe_res.is_err());
    }

    /// 11.2 Security Review: Verify shell metacharacters in assets and text overlays
    /// cannot inject arbitrary shell commands because Rust process arguments are typed Vec<String>
    #[test]
    fn test_security_injection_resistance() {
        let fixture_dir = get_fixture_dir();
        let mut project = Project::new("Security Test".to_string());
        let malicious_text = "'; calc.exe & rm -rf / | echo 'pwned";

        let mut track =
            crate::modules::domain::Track::new("Text Track".to_string(), TrackKind::Video);
        let mut item = crate::modules::domain::TimelineItem::new(
            None,
            "Hacker Text".to_string(),
            ItemKind::Text,
            crate::modules::domain::time::TimeRange::new(
                RationalTime::from_frames(0, FrameRate::FPS_30),
                RationalTime::from_frames(30, FrameRate::FPS_30),
            ),
            crate::modules::domain::time::TimeRange::new(
                RationalTime::from_frames(0, FrameRate::FPS_30),
                RationalTime::from_frames(30, FrameRate::FPS_30),
            ),
        )
        .unwrap();
        item.text_content = Some(malicious_text.to_string());
        track.items.push(item);
        project.tracks.push(track);

        let plan = CompositionPlan::from_project(&project).unwrap();
        let target_out = fixture_dir.join("sec_output.mp4");
        let (args, filter_complex) = plan.to_ffmpeg_export_args(&target_out, 23, "libx264");

        // Verify arguments are cleanly separated elements without shell execution
        assert!(filter_complex.contains("calc.exe")); // Passed as text string literal
        assert!(!args.iter().any(|a| a == "calc.exe")); // Not passed as a command argument!
    }

    /// 11.4 Queue / Backpressure review: Stress JobManager with 200 jobs
    #[test]
    fn test_queue_backpressure_and_concurrency_stress() {
        let mut jm = JobManager::new(4);

        // Submit 200 jobs, 50 unique keys repeated 4 times
        for i in 0..200 {
            let unique_key = format!("probe_asset_{}", i % 50);
            let job_id = format!("job_{}", i);
            let (_, is_dedup) = jm.submit_job(job_id, JobKind::Probe, unique_key, 1);
            if i >= 50 {
                assert!(is_dedup, "Job with key should have been deduplicated");
            }
        }

        // Mass cancellation test
        for i in 0..50 {
            let job_id = format!("job_{}", i);
            jm.cancel_job(&job_id);
            let job = jm.get_job(&job_id).unwrap();
            assert_eq!(job.status, JobStatus::Cancelled);
        }
    }

    /// 11.5 Long-project performance qualification: Test project with 20 tracks and 200 clips
    #[test]
    fn test_long_project_scalability_benchmark() {
        let rate = FrameRate::FPS_30;
        let mut project = Project::new("Long Benchmark Project".to_string());
        project.settings = ProjectSettings {
            width: 1920,
            height: 1080,
            frame_rate: rate,
            sample_rate: 48000,
            channels: 2,
        };

        // Create 20 tracks, each with 10 sequential clips (total 200 clips)
        for t in 0..20 {
            let mut track =
                crate::modules::domain::Track::new(format!("Track {}", t), TrackKind::Video);
            for c in 0..10 {
                let start_frame = c * 30;
                let item = crate::modules::domain::TimelineItem::new(
                    None,
                    format!("Clip_{}_{}", t, c),
                    ItemKind::Video,
                    crate::modules::domain::time::TimeRange::new(
                        RationalTime::from_frames(start_frame, rate),
                        RationalTime::from_frames(30, rate),
                    ),
                    crate::modules::domain::time::TimeRange::new(
                        RationalTime::from_frames(0, rate),
                        RationalTime::from_frames(30, rate),
                    ),
                )
                .unwrap();
                track.items.push(item);
            }
            project.tracks.push(track);
        }

        assert_eq!(project.tracks.len(), 20);
        let target_item_id = project.tracks[0].items[0].item_id.clone();

        let service = CommandService::new(project);

        // 1. Benchmark Dry-Run performance on 200-clip project
        let t_start = Instant::now();
        let diff = service
            .dry_run(&CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: service.project().project_id.clone(),
                base_revision: 0,
                client_id: None,
                client_epoch: None,
                sequence_number: None,
                dry_run: true,
                transaction_id: Some("tx_bench".to_string()),
                operations: vec![OperationEnvelope {
                    operation_id: "op_bench_trim".to_string(),
                    operation: CommandOperation::ClipTrim {
                        item_id: target_item_id,
                        new_timeline_range: crate::modules::domain::time::TimeRange::new(
                            RationalTime::from_frames(0, rate),
                            RationalTime::from_frames(25, rate),
                        ),
                        new_source_range: crate::modules::domain::time::TimeRange::new(
                            RationalTime::from_frames(0, rate),
                            RationalTime::from_frames(25, rate),
                        ),
                    },
                }],
            })
            .unwrap();

        let dry_run_elapsed = t_start.elapsed();
        assert_eq!(diff.changes.len(), 1);
        assert!(
            dry_run_elapsed.as_millis() < 50,
            "Dry-run took too long: {}ms",
            dry_run_elapsed.as_millis()
        );

        // 2. Benchmark Composition Plan compilation on 200-clip project
        let t_comp_start = Instant::now();
        let plan = CompositionPlan::from_project(service.project()).unwrap();
        let comp_elapsed = t_comp_start.elapsed();
        assert_eq!(plan.visual_layers.len(), 200);
        assert!(
            comp_elapsed.as_millis() < 50,
            "Composition compilation took too long: {}ms",
            comp_elapsed.as_millis()
        );

        // 3. Benchmark atomic save/load on 200-clip project
        let fixture_dir = get_fixture_dir();
        let bench_file = fixture_dir.join("bench_200_clips.tcproj");
        let t_save_start = Instant::now();
        save_project_atomic(service.project(), &bench_file).unwrap();
        let save_elapsed = t_save_start.elapsed();
        assert!(
            save_elapsed.as_millis() < 300,
            "Save took too long: {}ms",
            save_elapsed.as_millis()
        );

        let t_load_start = Instant::now();
        let loaded = load_project(&bench_file).unwrap();
        let load_elapsed = t_load_start.elapsed();
        assert_eq!(loaded.tracks.len(), 20);
        assert!(
            load_elapsed.as_millis() < 300,
            "Load took too long: {}ms",
            load_elapsed.as_millis()
        );

        let _ = fs::remove_file(bench_file);
    }
}
