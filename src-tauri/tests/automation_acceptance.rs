//! Phase 15 Comprehensive Acceptance Test Suite
//! Validates all 41 Acceptance Criteria (P15-AC-01 through P15-AC-41)
//! defined in Plans/plan_phase15.md, ADR-005, and ADR-001 v3.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use tinixcut_lib::modules::automation::operations::{
    AutomationOperations, OperationContext, OperationError,
};
use tinixcut_lib::modules::command::{
    CommandEnvelope, CommandOperation, CommandReceipt, OperationEnvelope,
};
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    Asset, ClipSpeed, ItemKind, MediaKind, Project, TimelineItem, Track, TrackKind,
    CURRENT_SCHEMA_VERSION,
};
use tinixcut_lib::modules::persistence::{load_project, save_project_atomic};

static NEXT_REQ_ID: AtomicU64 = AtomicU64::new(1);

fn next_id() -> u64 {
    NEXT_REQ_ID.fetch_add(1, Ordering::SeqCst)
}

fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tinixcut_p15_acc_{}_{}", name, next_id()));
    if dir.exists() {
        let _ = fs::remove_dir_all(&dir);
    }
    fs::create_dir_all(&dir).expect("Failed to create test directory");
    dir
}

fn create_sample_project(dir: &Path, name: &str) -> (PathBuf, Project) {
    let path = dir.join(format!("{}.tcproj", name));
    let mut project = Project::new(name.to_string());
    project
        .tracks
        .push(Track::new("Video Track 1".to_string(), TrackKind::Video));
    save_project_atomic(&project, &path).expect("Failed to save initial sample project");
    (path, project)
}

fn generate_synthetic_mp4(path: &Path, duration_secs: u32, color: &str) {
    if path.exists() {
        return;
    }
    if let Ok(mut cmd) =
        tinixcut_lib::modules::media::resolver::MediaBinaryResolver::command("ffmpeg")
    {
        let _ = cmd
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                &format!("color=c={}:s=128x128:r=30:d={}", color, duration_secs),
                "-f",
                "lavfi",
                "-i",
                &format!("sine=frequency=440:duration={}", duration_secs),
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
                path.to_str().unwrap(),
            ])
            .output();
    }
}

fn create_sample_project_with_media(
    dir: &Path,
    name: &str,
    duration_secs: u32,
) -> (PathBuf, Project, PathBuf) {
    let media_path = dir.join(format!("{}_media.mp4", name));
    generate_synthetic_mp4(&media_path, duration_secs, "green");

    let path = dir.join(format!("{}.tcproj", name));
    let mut project = Project::new(name.to_string());
    let asset_id = format!("ast_{}", name);
    project.assets.push(tinixcut_lib::modules::domain::Asset {
        asset_id: asset_id.clone(),
        name: format!("{}_media.mp4", name),
        path: media_path.to_string_lossy().to_string(),
        media_kind: tinixcut_lib::modules::domain::MediaKind::Video,
        duration: Some(tinixcut_lib::modules::domain::time::RationalTime::new(
            (duration_secs * 30) as i64,
            tinixcut_lib::modules::domain::time::FrameRate::FPS_30,
        )),
        file_size: 1024,
        dimensions: None,
    });
    let mut track = Track::new("Video Track 1".to_string(), TrackKind::Video);
    let item = tinixcut_lib::modules::domain::TimelineItem::new(
        Some(asset_id),
        "Clip 1".to_string(),
        tinixcut_lib::modules::domain::ItemKind::Video,
        tinixcut_lib::modules::domain::time::TimeRange::new(
            tinixcut_lib::modules::domain::time::RationalTime::new(
                0,
                tinixcut_lib::modules::domain::time::FrameRate::FPS_30,
            ),
            tinixcut_lib::modules::domain::time::RationalTime::new(
                (duration_secs * 30) as i64,
                tinixcut_lib::modules::domain::time::FrameRate::FPS_30,
            ),
        ),
        tinixcut_lib::modules::domain::time::TimeRange::new(
            tinixcut_lib::modules::domain::time::RationalTime::new(
                0,
                tinixcut_lib::modules::domain::time::FrameRate::FPS_30,
            ),
            tinixcut_lib::modules::domain::time::RationalTime::new(
                (duration_secs * 30) as i64,
                tinixcut_lib::modules::domain::time::FrameRate::FPS_30,
            ),
        ),
    )
    .unwrap();
    track.items.push(item);
    project.tracks.push(track);
    save_project_atomic(&project, &path).expect("Failed to save sample project with media");
    (path, project, media_path)
}

struct OperationsHarness {
    operations: AutomationOperations,
}

impl OperationsHarness {
    async fn new(operations: AutomationOperations) -> Self {
        Self { operations }
    }

    async fn execute(&mut self, name: &str, arguments: serde_json::Value) -> serde_json::Value {
        match self
            .operations
            .execute(name, arguments, OperationContext::default())
            .await
        {
            Ok(result) => serde_json::json!({"result": serde_json::to_value(result).unwrap()}),
            Err(OperationError::InvalidParams(message)) => serde_json::json!({"result":{"isError":true,"content":[{"type":"text","text":format!("failed to deserialize parameters: {message}")}]}}),
            Err(OperationError::Internal(message)) => serde_json::json!({"error": {"code": -32603, "message": message}}),
        }
    }
}

#[tokio::test]
async fn test_p40_operations_scene_candidates_are_read_only_and_cached() {
    let dir = test_dir("scene_candidates");
    let fixture = dir.join("three_scenes.mp4");
    let mut ffmpeg = tinixcut_lib::modules::media::resolver::MediaBinaryResolver::command("ffmpeg")
        .expect("bundled FFmpeg");
    let status = ffmpeg
        .args([
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=red:s=128x128:r=30:d=1",
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=128x128:r=30:d=1",
            "-f",
            "lavfi",
            "-i",
            "color=c=green:s=128x128:r=30:d=1",
            "-filter_complex",
            "[0:v][1:v][2:v]concat=n=3:v=1:a=0,format=yuv420p[v]",
            "-map",
            "[v]",
            "-c:v",
            "libx264",
            "-sc_threshold",
            "0",
        ])
        .arg(&fixture)
        .status()
        .expect("generate scene fixture");
    assert!(status.success());

    let server = AutomationOperations::new();
    let mut client = OperationsHarness::new(server).await;
    let project_path = dir.join("untouched.tinix");
    let created = client
        .execute(
            "tinixcut_project_create",
            serde_json::json!({ "projectPath": project_path.to_str().unwrap(), "name": "Read-only analysis check" }),
        )
        .await;
    assert_eq!(created["result"]["isError"], false);
    let before = client
        .execute(
            "tinixcut_inspect",
            serde_json::json!({ "projectPath": project_path.to_str().unwrap() }),
        )
        .await;
    let args = serde_json::json!({ "mediaPath": fixture.to_str().unwrap(), "threshold": 0.3 });
    let first = client
        .execute("tinixcut_media_scene_candidates", args.clone())
        .await;
    assert_eq!(
        first["result"]["isError"], false,
        "scene tool failed: {first}"
    );
    let result = &first["result"]["structuredContent"];
    assert_eq!(result["schemaVersion"], 1);
    assert_eq!(result["algorithmVersion"], "ffmpeg-select-scene-v1");
    let candidates = result["candidates"].as_array().expect("candidates");
    assert!(
        candidates.len() >= 2,
        "expected both synthetic scene cuts: {result}"
    );
    assert!(candidates.iter().any(|item| {
        item["sourceTimeMicros"]
            .as_u64()
            .is_some_and(|time| (950_000..=1_050_000).contains(&time))
    }));
    assert!(candidates.iter().any(|item| {
        item["sourceTimeMicros"]
            .as_u64()
            .is_some_and(|time| (1_950_000..=2_050_000).contains(&time))
    }));
    let second = client
        .execute("tinixcut_media_scene_candidates", args)
        .await;
    assert_eq!(
        first["result"]["structuredContent"],
        second["result"]["structuredContent"]
    );
    let limited = client
        .execute(
            "tinixcut_media_scene_candidates",
            serde_json::json!({ "mediaPath": fixture.to_str().unwrap(), "threshold": 0.3, "maxCandidates": 1 }),
        )
        .await;
    let limited_report = &limited["result"]["structuredContent"];
    assert_eq!(limited_report["candidates"].as_array().unwrap().len(), 1);
    assert_eq!(limited_report["truncated"], true);
    let offline = client
        .execute(
            "tinixcut_media_scene_candidates",
            serde_json::json!({ "mediaPath": dir.join("missing.mp4").to_str().unwrap() }),
        )
        .await;
    assert_eq!(
        offline["result"]["structuredContent"]["code"],
        "MEDIA_SCENE_ANALYSIS_INVALID"
    );
    let duration_cap = client
        .execute(
            "tinixcut_media_scene_candidates",
            serde_json::json!({ "mediaPath": fixture.to_str().unwrap(), "maxDurationSeconds": 1801 }),
        )
        .await;
    assert_eq!(duration_cap["result"]["isError"], true);
    assert_eq!(
        duration_cap["result"]["structuredContent"]["code"],
        "MEDIA_SCENE_ANALYSIS_FAILED"
    );
    let whisper_cli = dir.join("whisper-cli.exe");
    fs::write(&whisper_cli, b"test executable placeholder").unwrap();
    let missing_model = client
        .execute(
            "tinixcut_media_transcript",
            serde_json::json!({
                "mediaPath": fixture.to_str().unwrap(),
                "whisperCliPath": whisper_cli.to_str().unwrap(),
                "modelPath": dir.join("missing-model.bin").to_str().unwrap()
            }),
        )
        .await;
    assert_eq!(missing_model["result"]["isError"], true);
    assert_eq!(
        missing_model["result"]["structuredContent"]["code"],
        "WHISPER_MODEL_MISSING"
    );
    let offline_transcript = client
        .execute(
            "tinixcut_media_transcript",
            serde_json::json!({
                "mediaPath": dir.join("missing-audio.wav").to_str().unwrap(),
                "whisperCliPath": whisper_cli.to_str().unwrap(),
                "modelPath": dir.join("missing-model.bin").to_str().unwrap()
            }),
        )
        .await;
    assert_eq!(
        offline_transcript["result"]["structuredContent"]["code"],
        "MEDIA_TRANSCRIPT_INVALID"
    );
    let after = client
        .execute(
            "tinixcut_inspect",
            serde_json::json!({ "projectPath": project_path.to_str().unwrap() }),
        )
        .await;
    assert_eq!(
        before["result"]["structuredContent"]["revision"],
        after["result"]["structuredContent"]["revision"]
    );
    assert_eq!(
        before["result"]["structuredContent"],
        after["result"]["structuredContent"]
    );

    let saturated_server = AutomationOperations::new();
    let worker_permits: Vec<_> = (0..4)
        .map(|index| {
            saturated_server
                .worker_capacity
                .try_acquire(&format!("phase40-capacity-{index}"))
                .expect("fill the four-worker capacity")
        })
        .collect();
    let mut saturated_client = OperationsHarness::new(saturated_server).await;
    let saturated = saturated_client
        .execute(
            "tinixcut_media_scene_candidates",
            serde_json::json!({ "mediaPath": fixture.to_str().unwrap(), "threshold": 0.31 }),
        )
        .await;
    assert_eq!(saturated["result"]["isError"], true);
    assert_eq!(
        saturated["result"]["structuredContent"]["code"],
        "ANALYSIS_CAPACITY"
    );
    drop(worker_permits);
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn test_p41_edit_plan_validation_is_versioned_read_only_and_command_backed() {
    let dir = test_dir("edit_plan_validate");
    let (project_path, project) = create_sample_project(&dir, "plan_project");
    let server = AutomationOperations::new();
    let mut client = OperationsHarness::new(server).await;
    let plan = serde_json::json!({
        "schemaVersion": 1,
        "goal": "Add a second video track",
        "projectId": project.project_id,
        "baseRevision": project.revision,
        "referencedMediaIds": [],
        "assumptions": ["Keep existing track untouched"],
        "warnings": [],
        "operations": [{
            "operationId": "plan-track-create",
            "operation": {
                "type": "TrackCreate",
                "payload": { "track_id": "video-2", "name": "Draft Video", "kind": "Video" }
            }
        }]
    });
    let valid = client
        .execute(
            "tinixcut_edit_plan_validate",
            serde_json::json!({
                "projectPath": project_path.to_str().unwrap(), "plan": plan
            }),
        )
        .await;
    assert_eq!(valid["result"]["isError"], false, "{valid}");
    assert_eq!(
        valid["result"]["structuredContent"]["valid"], true,
        "{valid}"
    );
    assert_eq!(
        valid["result"]["structuredContent"]["diff"]["operationsEvaluated"],
        1
    );

    let stale_plan = serde_json::json!({
        "schemaVersion": 1,
        "goal": "Stale draft",
        "projectId": project.project_id,
        "baseRevision": project.revision + 1,
        "operations": [{
            "operationId": "stale-track-create",
            "operation": {
                "type": "TrackCreate",
                "payload": { "track_id": "video-3", "name": "Stale", "kind": "Video" }
            }
        }]
    });
    let stale = client
        .execute(
            "tinixcut_edit_plan_validate",
            serde_json::json!({
                "projectPath": project_path.to_str().unwrap(), "plan": stale_plan
            }),
        )
        .await;
    assert_eq!(stale["result"]["isError"], false);
    assert_eq!(stale["result"]["structuredContent"]["valid"], false);
    assert!(stale["result"]["structuredContent"]["issues"][0]
        .as_str()
        .unwrap()
        .contains("Stale edit plan"));

    let malformed = client
        .execute(
            "tinixcut_edit_plan_validate",
            serde_json::json!({
                "projectPath": project_path.to_str().unwrap(),
                "plan": {
                    "schemaVersion": 1,
                    "goal": "Reject unsupported operation output",
                    "projectId": project.project_id,
                    "baseRevision": project.revision,
                    "operations": [{
                        "operationId": "unsupported-op",
                        "operation": { "type": "ModelInventedOperation", "payload": {} }
                    }]
                }
            }),
        )
        .await;
    assert_eq!(malformed["result"]["isError"], true, "{malformed}");
    assert_eq!(
        malformed["result"]["structuredContent"]["code"],
        "EDIT_PLAN_INVALID"
    );

    let after = load_project(&project_path).unwrap();
    assert_eq!(after.revision, project.revision);
    assert_eq!(after.tracks.len(), project.tracks.len());
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn test_p41_deterministic_draft_validates_and_applies_through_command_service() {
    let dir = test_dir("edit_plan_draft");
    let (project_path, project, _media_path) =
        create_sample_project_with_media(&dir, "draft_project", 2);
    let server = AutomationOperations::new();
    let mut client = OperationsHarness::new(server).await;
    let drafted = client
        .execute(
            "tinixcut_edit_plan_draft",
            serde_json::json!({
                "projectPath": project_path.to_str().unwrap(),
                "goal": "Make a 2 second portrait test cut",
                "mediaIds": [project.assets[0].asset_id],
                "targetDurationFrames": 60
            }),
        )
        .await;
    assert_eq!(drafted["result"]["isError"], false, "{drafted}");
    let receipt = &drafted["result"]["structuredContent"];
    assert_eq!(receipt["provider"], "deterministic-mock-v1");
    assert_eq!(receipt["validation"]["valid"], true);
    assert_eq!(receipt["plan"]["schemaVersion"], 1);
    assert_eq!(receipt["plan"]["operations"].as_array().unwrap().len(), 3);
    assert_eq!(
        receipt["plan"]["operations"][0]["operation"]["type"],
        "ProjectSettingsSet"
    );

    let after = load_project(&project_path).unwrap();
    assert_eq!(after.revision, project.revision);
    assert_eq!(after.tracks, project.tracks);

    let applied = client
        .execute(
            "tinixcut_apply",
            serde_json::json!({
                "projectPath": project_path.to_str().unwrap(),
                "envelope": {
                    "schemaVersion": CURRENT_SCHEMA_VERSION,
                    "projectId": project.project_id,
                    "baseRevision": project.revision,
                    "clientId": "phase41-draft-test",
                    "clientEpoch": project.client_registry.current_epoch,
                    "sequenceNumber": 1,
                    "dryRun": false,
                    "transactionId": "phase41-draft-apply",
                    "operations": receipt["plan"]["operations"]
                }
            }),
        )
        .await;
    assert_eq!(applied["result"]["isError"], false, "{applied}");
    assert_eq!(
        applied["result"]["structuredContent"]["newRevision"],
        project.revision + 1
    );
    let applied_project = load_project(&project_path).unwrap();
    assert_eq!(applied_project.tracks.len(), project.tracks.len() + 1);
    assert_eq!(applied_project.tracks.last().unwrap().items.len(), 1);
    assert_eq!(applied_project.settings.width, 1080);
    assert_eq!(applied_project.settings.height, 1920);

    let second_draft = client
        .execute(
            "tinixcut_edit_plan_draft",
            serde_json::json!({
                "projectPath": project_path.to_str().unwrap(),
                "goal": "Create another draft after the first one",
                "mediaIds": [project.assets[0].asset_id],
                "targetDurationFrames": 60
            }),
        )
        .await;
    assert_eq!(second_draft["result"]["isError"], false, "{second_draft}");
    let second_operations = second_draft["result"]["structuredContent"]["plan"]["operations"]
        .as_array()
        .unwrap();
    assert!(second_operations[0]["operationId"]
        .as_str()
        .unwrap()
        .starts_with("draft_r1_"));
    let applied_twice = client
        .execute(
            "tinixcut_apply",
            serde_json::json!({
                "projectPath": project_path.to_str().unwrap(),
                "envelope": {
                    "schemaVersion": CURRENT_SCHEMA_VERSION,
                    "projectId": project.project_id,
                    "baseRevision": project.revision + 1,
                    "clientId": "phase41-draft-test",
                    "clientEpoch": project.client_registry.current_epoch,
                    "sequenceNumber": 2,
                    "dryRun": false,
                    "transactionId": "phase41-draft-apply-second",
                    "operations": second_operations
                }
            }),
        )
        .await;
    assert_eq!(applied_twice["result"]["isError"], false, "{applied_twice}");
    assert_eq!(
        applied_twice["result"]["structuredContent"]["newRevision"],
        project.revision + 2
    );
    let applied_twice_project = load_project(&project_path).unwrap();
    assert_eq!(applied_twice_project.tracks.len(), project.tracks.len() + 2);
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn test_p41_deterministic_draft_includes_selected_audio_and_srt_cues() {
    let dir = test_dir("edit_plan_supporting_media");
    let (project_path, mut project, _) =
        create_sample_project_with_media(&dir, "draft_with_supporting_media", 30);
    let audio_path = dir.join("music.wav");
    fs::write(&audio_path, b"audio fixture for read-only draft planning").unwrap();
    project.assets.push(Asset {
        asset_id: "ast_draft_music".to_string(),
        name: "music.wav".to_string(),
        path: audio_path.to_string_lossy().to_string(),
        media_kind: MediaKind::Audio,
        duration: Some(RationalTime::new(900, FrameRate::FPS_30)),
        file_size: 42,
        dimensions: None,
    });
    save_project_atomic(&project, &project_path).unwrap();
    let subtitle_path = dir.join("captions.srt");
    fs::write(
        &subtitle_path,
        "1\n00:00:01,000 --> 00:00:02,000\nA reviewed subtitle\n",
    )
    .unwrap();

    let mut client = OperationsHarness::new(AutomationOperations::new()).await;
    let drafted = client
        .execute(
            "tinixcut_edit_plan_draft",
            serde_json::json!({
                "projectPath": project_path.to_str().unwrap(),
                "goal": "Make a thirty second portrait draft with music and subtitles",
                "mediaIds": [project.assets[0].asset_id],
                "targetDurationFrames": 900,
                "audioAssetId": "ast_draft_music",
                "subtitleFilePath": subtitle_path.to_str().unwrap()
            }),
        )
        .await;
    assert_eq!(drafted["result"]["isError"], false, "{drafted}");
    let receipt = &drafted["result"]["structuredContent"];
    assert_eq!(receipt["validation"]["valid"], true, "{receipt}");
    let operations = receipt["plan"]["operations"].as_array().unwrap();
    assert!(operations.iter().any(|operation| {
        operation["operation"]["type"] == "ClipCreate"
            && operation["operation"]["payload"]["kind"] == "Audio"
    }));
    assert!(operations.iter().any(|operation| {
        operation["operation"]["type"] == "ClipTextSet"
            && operation["operation"]["payload"]["text"] == "A reviewed subtitle"
    }));
    assert!(receipt["plan"]["referencedMediaIds"]
        .as_array()
        .unwrap()
        .iter()
        .any(|media_id| media_id == "ast_draft_music"));
    let unchanged = load_project(&project_path).unwrap();
    assert_eq!(unchanged.revision, project.revision);
    assert_eq!(unchanged.tracks, project.tracks);
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
#[ignore = "requires an explicitly started local LM Studio server and loaded model"]
async fn test_p41_operations_opt_in_local_ai_draft_is_validated_and_read_only() {
    let dir = test_dir("local_ai_draft");
    let (project_path, project, _) = create_sample_project_with_media(&dir, "local_ai_draft", 30);
    let selected_ids = project
        .assets
        .iter()
        .map(|asset| asset.asset_id.clone())
        .collect::<Vec<_>>();
    let client = OperationsHarness::new(AutomationOperations::new()).await;
    let mut client = client;
    let response = client
        .execute(
            "tinixcut_edit_plan_draft",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "goal": "Start with the earliest scene and end with the most energetic scene",
                "mediaIds": selected_ids,
                "targetDurationFrames": 60,
                "plannerProvider": "lm-studio-local",
                "localModel": "google/gemma-4-e4b"
            }),
        )
        .await;
    assert_eq!(response["result"]["isError"], false, "{response}");
    let receipt = &response["result"]["structuredContent"];
    assert!(receipt["provider"]
        .as_str()
        .unwrap()
        .starts_with("lm-studio-local"));
    assert_eq!(receipt["validation"]["valid"], true, "{receipt}");
    assert!(receipt["plan"]["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item.as_str().unwrap().contains("Local model selected")));
    assert_eq!(load_project(&project_path).unwrap(), project);

    let unavailable_model = client
        .execute(
            "tinixcut_edit_plan_draft",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "goal": "Check provider error reporting",
                "mediaIds": selected_ids,
                "targetDurationFrames": 60,
                "plannerProvider": "lm-studio-local",
                "localModel": "tinixcut-model-that-does-not-exist"
            }),
        )
        .await;
    assert_eq!(
        unavailable_model["result"]["isError"], true,
        "{unavailable_model}"
    );
    assert_eq!(
        unavailable_model["result"]["structuredContent"]["code"],
        "EDIT_PLAN_DRAFT_FAILED"
    );
    assert_eq!(load_project(&project_path).unwrap(), project);
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn test_p15_ac09_ac10_inspect_success_and_error() {
    let dir = test_dir("inspect");
    let (proj_path, project) = create_sample_project(&dir, "inspect_proj");

    let server = AutomationOperations::new();
    let mut client = OperationsHarness::new(server).await;

    // AC-09: Inspect success
    let args = serde_json::json!({
        "projectPath": proj_path.to_str().unwrap()
    });
    let res = client.execute("tinixcut_inspect", args).await;
    assert_eq!(res["result"]["isError"], false);
    let content = &res["result"]["structuredContent"];
    assert_eq!(content["projectId"], project.project_id);
    assert_eq!(content["revision"], 0);
    assert_eq!(content["tracksCount"], 1);

    // AC-10: Inspect error on non-existent file
    let bad_args = serde_json::json!({
        "projectPath": dir.join("non_existent.tcproj").to_str().unwrap()
    });
    let bad_res = client.execute("tinixcut_inspect", bad_args).await;
    assert_eq!(bad_res["result"]["isError"], true);
    assert_eq!(
        bad_res["result"]["structuredContent"]["code"],
        "PROJECT_LOAD_FAILED"
    );
}

#[tokio::test]
async fn test_phase39_operations_project_bootstrap_and_read_only_media_probe() {
    let dir = test_dir("phase39_project_bootstrap_probe");
    let project_path = dir.join("created_via_operations.tcproj");
    let media_path = dir.join("probe_media.mp4");
    generate_synthetic_mp4(&media_path, 2, "blue");
    assert!(media_path.is_file(), "Synthetic media fixture must exist");

    let mut client = OperationsHarness::new(AutomationOperations::new()).await;
    let create = client
        .execute(
            "tinixcut_project_create",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "name": "Phase 39 bootstrap"
            }),
        )
        .await;
    assert_eq!(create["result"]["isError"], false, "{create:?}");
    let receipt = &create["result"]["structuredContent"];
    assert_eq!(receipt["name"], "Phase 39 bootstrap");
    assert_eq!(receipt["revision"], 0);
    let project = load_project(&project_path).expect("Created project must reopen");
    assert_eq!(project.project_id, receipt["projectId"]);
    assert!(project.tracks.is_empty());

    let original_bytes = fs::read(&project_path).unwrap();
    let duplicate_create = client
        .execute(
            "tinixcut_project_create",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "name": "Must not replace"
            }),
        )
        .await;
    assert_eq!(duplicate_create["result"]["isError"], true);
    assert_eq!(
        duplicate_create["result"]["structuredContent"]["code"],
        "PROJECT_REVISION_CONFLICT"
    );
    assert_eq!(fs::read(&project_path).unwrap(), original_bytes);

    let probe = client
        .execute(
            "tinixcut_media_probe",
            serde_json::json!({ "mediaPath": media_path.to_string_lossy() }),
        )
        .await;
    assert_eq!(probe["result"]["isError"], false, "{probe:?}");
    let report = &probe["result"]["structuredContent"];
    assert!(report["fileSizeBytes"].as_u64().unwrap() > 0);
    assert_eq!(report["width"], 128);
    assert_eq!(report["height"], 128);
    assert_eq!(report["videoCodec"], "h264");
    assert_eq!(report["audioCodec"], "aac");
    assert!(report["streams"].as_array().unwrap().len() >= 2);

    let missing = client
        .execute(
            "tinixcut_media_probe",
            serde_json::json!({ "mediaPath": dir.join("missing.mp4").to_string_lossy() }),
        )
        .await;
    assert_eq!(missing["result"]["isError"], true);
    assert_eq!(
        missing["result"]["structuredContent"]["code"],
        "MEDIA_PROBE_FAILED"
    );
}

#[tokio::test]
async fn test_p42_project_variant_creates_isolated_revision_checked_copy() {
    let dir = test_dir("phase42_project_variant");
    let (source_path, source_before, _) =
        create_sample_project_with_media(&dir, "variant_source", 4);
    let source_bytes = fs::read(&source_path).unwrap();
    let destination_path = dir.join("variant_copy.tinix");
    let mut client = OperationsHarness::new(AutomationOperations::new()).await;

    let variant_result = client
        .execute(
            "tinixcut_project_variant",
            serde_json::json!({
                "sourceProjectPath": source_path.to_string_lossy(),
                "destinationProjectPath": destination_path.to_string_lossy(),
                "expectedRevision": source_before.revision,
                "name": "Sunrise alternate"
            }),
        )
        .await;
    assert_eq!(
        variant_result["result"]["isError"], false,
        "{variant_result:?}"
    );
    let receipt = &variant_result["result"]["structuredContent"];
    let variant = load_project(&destination_path).expect("Variant should reopen");
    let source_after = load_project(&source_path).expect("Source should remain readable");
    assert_ne!(variant.project_id, source_before.project_id);
    assert_eq!(receipt["sourceProjectId"], source_before.project_id);
    assert_eq!(receipt["projectId"], variant.project_id);
    assert_eq!(variant.name, "Sunrise alternate");
    assert_eq!(variant.revision, 0);
    assert_eq!(variant.settings, source_before.settings);
    assert_eq!(variant.assets, source_before.assets);
    assert_eq!(variant.tracks, source_before.tracks);
    assert_eq!(source_after, source_before);
    assert_eq!(fs::read(&source_path).unwrap(), source_bytes);

    let existing_destination = client
        .execute(
            "tinixcut_project_variant",
            serde_json::json!({
                "sourceProjectPath": source_path.to_string_lossy(),
                "destinationProjectPath": destination_path.to_string_lossy(),
                "expectedRevision": source_before.revision,
                "name": "Must not overwrite"
            }),
        )
        .await;
    assert_eq!(existing_destination["result"]["isError"], true);
    assert_eq!(
        existing_destination["result"]["structuredContent"]["code"],
        "PROJECT_VARIANT_DESTINATION_EXISTS"
    );
    assert_eq!(load_project(&destination_path).unwrap(), variant);

    let stale_source = client
        .execute(
            "tinixcut_project_variant",
            serde_json::json!({
                "sourceProjectPath": source_path.to_string_lossy(),
                "destinationProjectPath": dir.join("stale_variant.tinix").to_string_lossy(),
                "expectedRevision": source_before.revision + 1,
                "name": "Stale source"
            }),
        )
        .await;
    assert_eq!(stale_source["result"]["isError"], true);
    assert_eq!(
        stale_source["result"]["structuredContent"]["code"],
        "PROJECT_REVISION_CONFLICT"
    );
    assert!(!dir.join("stale_variant.tinix").exists());
}

#[tokio::test]
async fn test_p42_project_compare_reports_revision_bound_content_changes_without_paths() {
    let dir = test_dir("phase42_project_compare");
    let (left_path, left, _) = create_sample_project_with_media(&dir, "compare_left", 4);
    let left_bytes = fs::read(&left_path).unwrap();
    let right_path = dir.join("compare_right.tinix");
    let mut client = OperationsHarness::new(AutomationOperations::new()).await;

    let variant = client
        .execute(
            "tinixcut_project_variant",
            serde_json::json!({
                "sourceProjectPath": left_path.to_string_lossy(),
                "destinationProjectPath": right_path.to_string_lossy(),
                "expectedRevision": left.revision,
                "name": "Comparable copy"
            }),
        )
        .await;
    assert_eq!(variant["result"]["isError"], false, "{variant:?}");
    let mut right = load_project(&right_path).unwrap();

    let equivalent = client
        .execute(
            "tinixcut_project_compare",
            serde_json::json!({
                "leftProjectPath": left_path.to_string_lossy(),
                "leftExpectedRevision": left.revision,
                "rightProjectPath": right_path.to_string_lossy(),
                "rightExpectedRevision": right.revision
            }),
        )
        .await;
    assert_eq!(equivalent["result"]["isError"], false, "{equivalent:?}");
    assert_eq!(
        equivalent["result"]["structuredContent"]["contentEquivalent"],
        true
    );

    right.tracks[0].items[0].speed = ClipSpeed::Half;
    right.tracks[0].items[0].source_range.duration = RationalTime::new(60, FrameRate::FPS_30);
    right.revision += 1;
    save_project_atomic(&right, &right_path).unwrap();
    let right_bytes = fs::read(&right_path).unwrap();
    let compared = client
        .execute(
            "tinixcut_project_compare",
            serde_json::json!({
                "leftProjectPath": left_path.to_string_lossy(),
                "leftExpectedRevision": left.revision,
                "rightProjectPath": right_path.to_string_lossy(),
                "rightExpectedRevision": right.revision
            }),
        )
        .await;
    assert_eq!(compared["result"]["isError"], false, "{compared:?}");
    let report = &compared["result"]["structuredContent"];
    assert_eq!(report["contentEquivalent"], false);
    assert_eq!(
        report["items"]["modified"][0]["entityId"],
        left.tracks[0].items[0].item_id
    );
    assert!(report["items"]["modified"][0]["changedFields"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| field == "speed"));
    let serialized = report.to_string();
    assert!(!serialized.contains(&left.assets[0].path));
    assert_eq!(fs::read(&left_path).unwrap(), left_bytes);
    assert_eq!(fs::read(&right_path).unwrap(), right_bytes);

    let stale = client
        .execute(
            "tinixcut_project_compare",
            serde_json::json!({
                "leftProjectPath": left_path.to_string_lossy(),
                "leftExpectedRevision": left.revision + 1,
                "rightProjectPath": right_path.to_string_lossy(),
                "rightExpectedRevision": right.revision
            }),
        )
        .await;
    assert_eq!(stale["result"]["isError"], true);
    assert_eq!(
        stale["result"]["structuredContent"]["code"],
        "PROJECT_REVISION_CONFLICT"
    );
    assert_eq!(fs::read(&left_path).unwrap(), left_bytes);
    assert_eq!(fs::read(&right_path).unwrap(), right_bytes);
}

#[tokio::test]
async fn test_p42_edit_plan_can_be_applied_to_variant_without_changing_source() {
    let dir = test_dir("phase42_variant_edit_workflow");
    let (source_path, source_before, _) =
        create_sample_project_with_media(&dir, "variant_edit_source", 4);
    let source_bytes = fs::read(&source_path).unwrap();
    let variant_path = dir.join("draft_variant.tinix");
    let mut client = OperationsHarness::new(AutomationOperations::new()).await;

    let branched = client
        .execute(
            "tinixcut_project_variant",
            serde_json::json!({
                "sourceProjectPath": source_path.to_string_lossy(),
                "destinationProjectPath": variant_path.to_string_lossy(),
                "expectedRevision": source_before.revision,
                "name": "Alternative draft"
            }),
        )
        .await;
    assert_eq!(branched["result"]["isError"], false, "{branched:?}");
    let variant_before = load_project(&variant_path).unwrap();
    assert_eq!(variant_before.revision, 0);

    let draft = client
        .execute(
            "tinixcut_edit_plan_draft",
            serde_json::json!({
                "projectPath": variant_path.to_string_lossy(),
                "goal": "Create a two-second alternate portrait draft",
                "mediaIds": [variant_before.assets[0].asset_id],
                "targetDurationFrames": 60
            }),
        )
        .await;
    assert_eq!(draft["result"]["isError"], false, "{draft:?}");
    assert_eq!(load_project(&variant_path).unwrap(), variant_before);
    let plan = &draft["result"]["structuredContent"]["plan"];

    let applied = client
        .execute(
            "tinixcut_apply",
            serde_json::json!({
                "projectPath": variant_path.to_string_lossy(),
                "envelope": {
                    "schemaVersion": CURRENT_SCHEMA_VERSION,
                    "projectId": variant_before.project_id,
                    "baseRevision": variant_before.revision,
                    "clientId": "phase42-variant-edit",
                    "clientEpoch": variant_before.client_registry.current_epoch,
                    "sequenceNumber": 1,
                    "dryRun": false,
                    "transactionId": "phase42-variant-draft-apply",
                    "operations": plan["operations"]
                }
            }),
        )
        .await;
    assert_eq!(applied["result"]["isError"], false, "{applied:?}");

    let variant_after = load_project(&variant_path).unwrap();
    let source_after = load_project(&source_path).unwrap();
    assert_eq!(variant_after.project_id, variant_before.project_id);
    assert_eq!(variant_after.revision, 1);
    assert_eq!(variant_after.tracks.len(), variant_before.tracks.len() + 1);
    assert_eq!(variant_after.settings.width, 1080);
    assert_eq!(variant_after.settings.height, 1920);
    assert_eq!(source_after, source_before);
    assert_eq!(fs::read(&source_path).unwrap(), source_bytes);
}

#[tokio::test]
async fn test_p42_edit_plan_reorder_preview_and_apply_are_revision_checked_and_isolated() {
    let dir = test_dir("phase42_variant_reorder");
    let (source_path, mut source, _) = create_sample_project_with_media(&dir, "reorder_source", 4);
    let fps = FrameRate::FPS_30;
    let source_asset_id = source.assets[0].asset_id.clone();
    let track = &mut source.tracks[0];
    let mut first = track.items[0].clone();
    first.name = "sunrise.mp4".to_string();
    first.timeline_range = TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps));
    first.source_range = TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps));
    let second = TimelineItem::new(
        Some(source_asset_id),
        "night-market.mp4".to_string(),
        ItemKind::Video,
        TimeRange::new(RationalTime::new(60, fps), RationalTime::new(60, fps)),
        TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps)),
    )
    .unwrap();
    let first_id = first.item_id.clone();
    let second_id = second.item_id.clone();
    track.items = vec![first, second];
    source
        .validate()
        .expect("Contiguous source fixture must be valid");
    save_project_atomic(&source, &source_path).unwrap();
    let source_bytes = fs::read(&source_path).unwrap();
    let variant_path = dir.join("reorder_variant.tinix");
    let mut client = OperationsHarness::new(AutomationOperations::new()).await;

    let branched = client
        .execute(
            "tinixcut_project_variant",
            serde_json::json!({
                "sourceProjectPath": source_path.to_string_lossy(),
                "destinationProjectPath": variant_path.to_string_lossy(),
                "expectedRevision": source.revision,
                "name": "Night market opening"
            }),
        )
        .await;
    assert_eq!(branched["result"]["isError"], false, "{branched:?}");
    let variant_before = load_project(&variant_path).unwrap();

    let preview = client
        .execute(
            "tinixcut_edit_plan_reorder",
            serde_json::json!({
                "projectPath": variant_path.to_string_lossy(),
                "goal": "Open with the night market, then show sunrise",
                "trackId": source.tracks[0].track_id,
                "itemIds": [second_id, first_id],
                "plannerProvider": "caller-order-v1"
            }),
        )
        .await;
    assert_eq!(preview["result"]["isError"], false, "{preview:?}");
    let receipt = &preview["result"]["structuredContent"];
    assert_eq!(receipt["validation"]["valid"], true, "{receipt:?}");
    assert_eq!(receipt["plan"]["baseRevision"], 0);
    assert_eq!(receipt["plan"]["operations"].as_array().unwrap().len(), 4);
    assert_eq!(load_project(&variant_path).unwrap(), variant_before);

    let applied = client
        .execute(
            "tinixcut_apply",
            serde_json::json!({
                "projectPath": variant_path.to_string_lossy(),
                "envelope": {
                    "schemaVersion": CURRENT_SCHEMA_VERSION,
                    "projectId": variant_before.project_id,
                    "baseRevision": variant_before.revision,
                    "clientId": "phase42-reorder",
                    "clientEpoch": variant_before.client_registry.current_epoch,
                    "sequenceNumber": 1,
                    "dryRun": false,
                    "transactionId": "phase42-reorder-apply",
                    "operations": receipt["plan"]["operations"]
                }
            }),
        )
        .await;
    assert_eq!(applied["result"]["isError"], false, "{applied:?}");

    let variant_after = load_project(&variant_path).unwrap();
    let reordered = &variant_after.tracks[0].items;
    let second_clip = reordered
        .iter()
        .find(|item| item.item_id == second_id)
        .unwrap();
    let first_clip = reordered
        .iter()
        .find(|item| item.item_id == first_id)
        .unwrap();
    assert_eq!(second_clip.timeline_range.start.value, 0);
    assert_eq!(first_clip.timeline_range.start.value, 60);
    assert_eq!(second_clip.timeline_range.duration.value, 60);
    assert_eq!(first_clip.timeline_range.duration.value, 60);
    assert_eq!(variant_after.revision, 1);
    assert_eq!(load_project(&source_path).unwrap(), source);
    assert_eq!(fs::read(&source_path).unwrap(), source_bytes);
}

#[tokio::test]
async fn test_p42_reorder_missing_media_fails_without_project_mutation_and_operations_recovers() {
    let dir = test_dir("phase42_reorder_missing_media");
    let (project_path, mut project, media_path) =
        create_sample_project_with_media(&dir, "reorder_missing", 4);
    let frame_rate = FrameRate::FPS_30;
    let track = &mut project.tracks[0];
    let mut first = track.items[0].clone();
    first.timeline_range = TimeRange::new(
        RationalTime::new(0, frame_rate),
        RationalTime::new(60, frame_rate),
    );
    first.source_range = TimeRange::new(
        RationalTime::new(0, frame_rate),
        RationalTime::new(60, frame_rate),
    );
    let second = TimelineItem::new(
        Some(project.assets[0].asset_id.clone()),
        "Second clip".to_string(),
        ItemKind::Video,
        TimeRange::new(
            RationalTime::new(60, frame_rate),
            RationalTime::new(60, frame_rate),
        ),
        TimeRange::new(
            RationalTime::new(60, frame_rate),
            RationalTime::new(60, frame_rate),
        ),
    )
    .unwrap();
    let first_id = first.item_id.clone();
    let second_id = second.item_id.clone();
    track.items = vec![first, second];
    project.validate().unwrap();
    save_project_atomic(&project, &project_path).unwrap();
    let project_bytes_before = fs::read(&project_path).unwrap();
    fs::remove_file(&media_path).unwrap();

    let mut client = OperationsHarness::new(AutomationOperations::new()).await;
    let rejected = client
        .execute(
            "tinixcut_edit_plan_reorder",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "goal": "Show the second clip first",
                "trackId": project.tracks[0].track_id,
                "itemIds": [second_id, first_id],
                "plannerProvider": "caller-order-v1"
            }),
        )
        .await;
    assert_eq!(rejected["result"]["isError"], true, "{rejected:?}");
    assert_eq!(
        rejected["result"]["structuredContent"]["code"], "EDIT_PLAN_REORDER_FAILED",
        "{rejected:?}"
    );
    assert!(
        rejected["result"]["structuredContent"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("offline or invalid"),
        "{rejected:?}"
    );
    assert_eq!(fs::read(&project_path).unwrap(), project_bytes_before);
    assert_eq!(load_project(&project_path).unwrap(), project);

    assert!(!AutomationOperations::command_schemas().is_empty());
    assert_eq!(fs::read(&project_path).unwrap(), project_bytes_before);

    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn test_p42_canvas_resize_plan_preview_and_apply_only_changes_canvas_settings() {
    let dir = test_dir("phase42_canvas_resize");
    let (source_path, source, _) =
        create_sample_project_with_media(&dir, "canvas_resize_source", 4);
    let source_bytes = fs::read(&source_path).unwrap();
    let variant_path = dir.join("canvas_resize_variant.tinix");
    let mut client = OperationsHarness::new(AutomationOperations::new()).await;

    let variant = client
        .execute(
            "tinixcut_project_variant",
            serde_json::json!({
                "sourceProjectPath": source_path.to_string_lossy(),
                "destinationProjectPath": variant_path.to_string_lossy(),
                "expectedRevision": source.revision,
                "name": "Portrait canvas variant"
            }),
        )
        .await;
    assert_eq!(variant["result"]["isError"], false, "{variant:?}");
    let variant_before = load_project(&variant_path).unwrap();
    let variant_bytes_before = fs::read(&variant_path).unwrap();

    let preview = client
        .execute(
            "tinixcut_edit_plan_canvas",
            serde_json::json!({
                "projectPath": variant_path.to_string_lossy(),
                "goal": "Prepare a vertical 9:16 version",
                "width": 1080,
                "height": 1920
            }),
        )
        .await;
    assert_eq!(preview["result"]["isError"], false, "{preview:?}");
    let receipt = &preview["result"]["structuredContent"];
    assert_eq!(receipt["validation"]["valid"], true, "{receipt:?}");
    assert_eq!(receipt["provider"], "caller-canvas-v1");
    assert_eq!(receipt["plan"]["baseRevision"], variant_before.revision);
    assert_eq!(receipt["plan"]["operations"].as_array().unwrap().len(), 1);
    let operation = &receipt["plan"]["operations"][0]["operation"];
    assert_eq!(operation["type"], "ProjectSettingsSet");
    assert_eq!(operation["payload"]["width"], 1080);
    assert_eq!(operation["payload"]["height"], 1920);
    assert!(receipt["plan"]["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|warning| warning.as_str().unwrap().contains("does not reposition")));
    assert_eq!(load_project(&variant_path).unwrap(), variant_before);
    assert_eq!(fs::read(&variant_path).unwrap(), variant_bytes_before);

    let applied = client
        .execute(
            "tinixcut_apply",
            serde_json::json!({
                "projectPath": variant_path.to_string_lossy(),
                "envelope": {
                    "schemaVersion": CURRENT_SCHEMA_VERSION,
                    "projectId": variant_before.project_id,
                    "baseRevision": variant_before.revision,
                    "clientId": "phase42-canvas",
                    "clientEpoch": variant_before.client_registry.current_epoch,
                    "sequenceNumber": 1,
                    "dryRun": false,
                    "transactionId": "phase42-canvas-apply",
                    "operations": receipt["plan"]["operations"]
                }
            }),
        )
        .await;
    assert_eq!(applied["result"]["isError"], false, "{applied:?}");
    let variant_after = load_project(&variant_path).unwrap();
    assert_eq!(variant_after.settings.width, 1080);
    assert_eq!(variant_after.settings.height, 1920);
    assert_eq!(variant_after.revision, variant_before.revision + 1);
    assert_eq!(variant_after.tracks, variant_before.tracks);
    assert_eq!(load_project(&source_path).unwrap(), source);
    assert_eq!(fs::read(&source_path).unwrap(), source_bytes);

    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn test_p42_edit_plan_volume_preview_and_apply_changes_only_selected_clip() {
    let dir = test_dir("phase42_volume_plan");
    let (project_path, source, _) = create_sample_project_with_media(&dir, "volume_source", 4);
    let source_bytes = fs::read(&project_path).unwrap();
    let item_id = source.tracks[0].items[0].item_id.clone();
    let mut client = OperationsHarness::new(AutomationOperations::new()).await;

    let preview = client
        .execute(
            "tinixcut_edit_plan_volume",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "goal": "Lower the selected clip volume",
                "itemId": item_id,
                "volume": 0.65
            }),
        )
        .await;
    assert_eq!(preview["result"]["isError"], false, "{preview:?}");
    let receipt = &preview["result"]["structuredContent"];
    assert_eq!(receipt["provider"], "caller-volume-v1");
    assert_eq!(receipt["validation"]["valid"], true, "{receipt:?}");
    assert_eq!(receipt["plan"]["baseRevision"], source.revision);
    assert_eq!(receipt["plan"]["operations"].as_array().unwrap().len(), 1);
    assert_eq!(
        receipt["plan"]["operations"][0]["operation"]["type"],
        "ClipVolumeSet"
    );
    let planned_volume = receipt["plan"]["operations"][0]["operation"]["payload"]["volume"]
        .as_f64()
        .unwrap();
    assert!((planned_volume - 0.65).abs() < 0.000_001);
    assert_eq!(fs::read(&project_path).unwrap(), source_bytes);

    let applied = client
        .execute(
            "tinixcut_apply",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "envelope": {
                    "schemaVersion": CURRENT_SCHEMA_VERSION,
                    "projectId": source.project_id,
                    "baseRevision": source.revision,
                    "clientId": "phase42-volume",
                    "clientEpoch": source.client_registry.current_epoch,
                    "sequenceNumber": 1,
                    "dryRun": false,
                    "transactionId": "phase42-volume-apply",
                    "operations": receipt["plan"]["operations"]
                }
            }),
        )
        .await;
    assert_eq!(applied["result"]["isError"], false, "{applied:?}");
    let updated = load_project(&project_path).unwrap();
    assert_eq!(updated.revision, source.revision + 1);
    assert_eq!(updated.tracks[0].items[0].volume, 0.65);
    let mut unchanged_tracks = source.tracks.clone();
    unchanged_tracks[0].items[0].volume = 0.65;
    assert_eq!(updated.tracks, unchanged_tracks);
    assert_eq!(
        load_project(&project_path).unwrap().settings,
        source.settings
    );
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn test_p42_edit_plan_text_replace_searches_selected_captions_and_applies_safely() {
    let dir = test_dir("phase42_text_replace");
    let (project_path, mut source) = create_sample_project(&dir, "text_source");
    let range = TimeRange::new(
        RationalTime::new(0, FrameRate::FPS_30),
        RationalTime::new(90, FrameRate::FPS_30),
    );
    let mut caption_track = Track::new("Captions".to_string(), TrackKind::Video);
    let mut selected = TimelineItem::new(
        None,
        "Caption A".to_string(),
        ItemKind::Text,
        range.clone(),
        range.clone(),
    )
    .unwrap();
    selected.text_content = Some("Meet in Da Nang; return to Da Nang".to_string());
    let selected_id = selected.item_id.clone();
    let mut unselected = TimelineItem::new(
        None,
        "Caption B".to_string(),
        ItemKind::Text,
        TimeRange::new(
            RationalTime::new(90, FrameRate::FPS_30),
            RationalTime::new(90, FrameRate::FPS_30),
        ),
        TimeRange::new(
            RationalTime::new(0, FrameRate::FPS_30),
            RationalTime::new(90, FrameRate::FPS_30),
        ),
    )
    .unwrap();
    unselected.text_content = Some("Another cue says Da Nang".to_string());
    let unselected_id = unselected.item_id.clone();
    caption_track.items.extend([selected, unselected]);
    source.tracks.push(caption_track);
    save_project_atomic(&source, &project_path).unwrap();
    let source_bytes = fs::read(&project_path).unwrap();
    let mut client = OperationsHarness::new(AutomationOperations::new()).await;

    let preview = client
        .execute(
            "tinixcut_edit_plan_text_replace",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "goal": "Update the selected location wording",
                "itemIds": [selected_id],
                "searchText": "Da Nang",
                "replacementText": "Hoi An"
            }),
        )
        .await;
    assert_eq!(preview["result"]["isError"], false, "{preview:?}");
    let receipt = &preview["result"]["structuredContent"];
    assert_eq!(receipt["provider"], "caller-text-replace-v1");
    assert_eq!(receipt["validation"]["valid"], true, "{receipt:?}");
    assert_eq!(receipt["plan"]["baseRevision"], source.revision);
    assert_eq!(receipt["plan"]["operations"].as_array().unwrap().len(), 1);
    assert_eq!(
        receipt["plan"]["operations"][0]["operation"]["type"],
        "ClipTextSet"
    );
    assert_eq!(
        receipt["plan"]["operations"][0]["operation"]["payload"]["text"],
        "Meet in Hoi An; return to Hoi An"
    );
    assert!(receipt["plan"]["assumptions"][0]
        .as_str()
        .unwrap()
        .contains("2 occurrence(s)"));
    assert_eq!(fs::read(&project_path).unwrap(), source_bytes);

    let applied = client
        .execute(
            "tinixcut_apply",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "envelope": {
                    "schemaVersion": CURRENT_SCHEMA_VERSION,
                    "projectId": source.project_id,
                    "baseRevision": source.revision,
                    "clientId": "phase42-text-replace",
                    "clientEpoch": source.client_registry.current_epoch,
                    "sequenceNumber": 1,
                    "dryRun": false,
                    "transactionId": "phase42-text-replace-apply",
                    "operations": receipt["plan"]["operations"]
                }
            }),
        )
        .await;
    assert_eq!(applied["result"]["isError"], false, "{applied:?}");
    let updated = load_project(&project_path).unwrap();
    let captions = &updated.tracks[1].items;
    assert_eq!(updated.revision, source.revision + 1);
    assert_eq!(
        captions
            .iter()
            .find(|item| item.item_id == selected_id)
            .unwrap()
            .text_content
            .as_deref(),
        Some("Meet in Hoi An; return to Hoi An")
    );
    assert_eq!(
        captions
            .iter()
            .find(|item| item.item_id == unselected_id)
            .unwrap()
            .text_content
            .as_deref(),
        Some("Another cue says Da Nang")
    );

    let not_found = client
        .execute(
            "tinixcut_edit_plan_text_replace",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "goal": "Try a stale transcript phrase",
                "itemIds": [selected_id],
                "searchText": "missing phrase",
                "replacementText": "changed"
            }),
        )
        .await;
    assert_eq!(not_found["result"]["isError"], true, "{not_found:?}");
    assert_eq!(
        not_found["result"]["structuredContent"]["code"],
        "EDIT_PLAN_TEXT_REPLACE_FAILED"
    );
    assert_eq!(load_project(&project_path).unwrap(), updated);
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn test_p42_edit_plan_trim_preview_and_apply_preserve_clip_source_in_point() {
    let dir = test_dir("phase42_clip_trim");
    let (project_path, source, _) = create_sample_project_with_media(&dir, "trim_source", 3);
    let source_bytes = fs::read(&project_path).unwrap();
    let source_item = &source.tracks[0].items[0];
    let source_item_id = source_item.item_id.clone();
    let mut client = OperationsHarness::new(AutomationOperations::new()).await;

    let preview = client
        .execute(
            "tinixcut_edit_plan_trim",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "goal": "Shorten the opening scene",
                "itemId": source_item_id,
                "targetDurationFrames": 60
            }),
        )
        .await;
    assert_eq!(preview["result"]["isError"], false, "{preview:?}");
    let receipt = &preview["result"]["structuredContent"];
    assert_eq!(receipt["validation"]["valid"], true, "{receipt:?}");
    assert_eq!(receipt["plan"]["baseRevision"], source.revision);
    assert_eq!(receipt["plan"]["operations"].as_array().unwrap().len(), 1);
    let operation = &receipt["plan"]["operations"][0]["operation"];
    assert_eq!(operation["type"], "ClipTrim");
    assert_eq!(
        operation["payload"]["new_timeline_range"]["start"]["value"],
        0
    );
    assert_eq!(
        operation["payload"]["new_timeline_range"]["duration"]["value"],
        60
    );
    assert_eq!(
        operation["payload"]["new_source_range"]["start"]["value"],
        0
    );
    assert_eq!(
        operation["payload"]["new_source_range"]["duration"]["value"],
        60
    );
    assert_eq!(fs::read(&project_path).unwrap(), source_bytes);

    let applied = client
        .execute(
            "tinixcut_apply",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "envelope": {
                    "schemaVersion": CURRENT_SCHEMA_VERSION,
                    "projectId": source.project_id,
                    "baseRevision": source.revision,
                    "clientId": "phase42-trim",
                    "clientEpoch": source.client_registry.current_epoch,
                    "sequenceNumber": 1,
                    "dryRun": false,
                    "transactionId": "phase42-trim-apply",
                    "operations": receipt["plan"]["operations"]
                }
            }),
        )
        .await;
    assert_eq!(applied["result"]["isError"], false, "{applied:?}");
    let trimmed = load_project(&project_path).unwrap();
    let trimmed_item = &trimmed.tracks[0].items[0];
    assert_eq!(trimmed.revision, source.revision + 1);
    assert_eq!(
        trimmed_item.timeline_range.start.value,
        source_item.timeline_range.start.value
    );
    assert_eq!(trimmed_item.timeline_range.duration.value, 60);
    assert_eq!(
        trimmed_item.source_range.start.value,
        source_item.source_range.start.value
    );
    assert_eq!(trimmed_item.source_range.duration.value, 60);

    let mut stale_operations = receipt["plan"]["operations"].clone();
    stale_operations[0]["operationId"] = serde_json::json!("phase42-stale-trim-operation");
    let stale_apply = client
        .execute(
            "tinixcut_apply",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "envelope": {
                    "schemaVersion": CURRENT_SCHEMA_VERSION,
                    "projectId": source.project_id,
                    "baseRevision": source.revision,
                    "clientId": "phase42-trim-stale",
                    "clientEpoch": source.client_registry.current_epoch,
                    "sequenceNumber": 1,
                    "dryRun": false,
                    "transactionId": "phase42-trim-stale-apply",
                    "operations": stale_operations
                }
            }),
        )
        .await;
    assert_eq!(stale_apply["result"]["isError"], true, "{stale_apply:?}");
    assert_eq!(load_project(&project_path).unwrap(), trimmed);

    let second_preview = client
        .execute(
            "tinixcut_edit_plan_trim",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "goal": "Shorten the opening scene further",
                "itemId": source_item_id,
                "targetDurationFrames": 45
            }),
        )
        .await;
    assert_eq!(
        second_preview["result"]["isError"], false,
        "{second_preview:?}"
    );
    let second_receipt = &second_preview["result"]["structuredContent"];
    assert_eq!(second_receipt["validation"]["valid"], true);
    assert_eq!(second_receipt["plan"]["baseRevision"], trimmed.revision);
    assert_eq!(
        second_receipt["plan"]["operations"][0]["operation"]["payload"]["new_source_range"]
            ["start"]["value"],
        source_item.source_range.start.value
    );

    let second_apply = client
        .execute(
            "tinixcut_apply",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "envelope": {
                    "schemaVersion": CURRENT_SCHEMA_VERSION,
                    "projectId": source.project_id,
                    "baseRevision": trimmed.revision,
                    "clientId": "phase42-trim",
                    "clientEpoch": source.client_registry.current_epoch,
                    "sequenceNumber": 2,
                    "dryRun": false,
                    "transactionId": "phase42-trim-second-apply",
                    "operations": second_receipt["plan"]["operations"]
                }
            }),
        )
        .await;
    assert_eq!(second_apply["result"]["isError"], false, "{second_apply:?}");
    let trimmed_again = load_project(&project_path).unwrap();
    let trimmed_again_item = &trimmed_again.tracks[0].items[0];
    assert_eq!(trimmed_again.revision, trimmed.revision + 1);
    assert_eq!(
        trimmed_again_item.timeline_range.start.value,
        source_item.timeline_range.start.value
    );
    assert_eq!(trimmed_again_item.timeline_range.duration.value, 45);
    assert_eq!(
        trimmed_again_item.source_range.start.value,
        source_item.source_range.start.value
    );
    assert_eq!(trimmed_again_item.source_range.duration.value, 45);
}

#[tokio::test]
async fn test_p45_operations_scene_edit_plan_is_revision_bound_and_read_only() {
    let dir = test_dir("p45_scene_edit_plan");
    let (project_path, project, media_path) =
        create_sample_project_with_media(&dir, "p45_scene_project", 30);
    let asset = project.assets.first().unwrap();
    let source = tinixcut_lib::modules::media::visual_scene::VisualSceneSource {
        asset_id: asset.asset_id.clone(),
        asset_name: asset.name.clone(),
        path: PathBuf::from(&asset.path),
    };
    let source_fingerprint =
        tinixcut_lib::modules::media::visual_scene::aggregate_source_fingerprint(&[source])
            .unwrap();
    let report = serde_json::json!({
        "schemaVersion": tinixcut_lib::modules::media::visual_scene::VISUAL_SCENE_SCHEMA_VERSION,
        "algorithmVersion": tinixcut_lib::modules::media::visual_scene::VISUAL_SCENE_ALGORITHM_VERSION,
        "provider": "lm-studio-local",
        "model": "google/gemma-4-e4b",
        "query": "lantern street",
        "sourceFingerprint": source_fingerprint,
        "configFingerprint": "0".repeat(64),
        "truncated": false,
        "scenes": [
            {
                "sceneId": "scene-01",
                "assetId": asset.asset_id,
                "assetName": asset.name,
                "sourceStartMicros": 0,
                "sourceEndMicros": 5000000,
                "sampledAtMicros": 2500000,
                "caption": "Opening view",
                "tags": ["street"],
                "relevanceScore": 0.7,
                "rationale": "Buildings are visible."
            },
            {
                "sceneId": "scene-02",
                "assetId": asset.asset_id,
                "assetName": asset.name,
                "sourceStartMicros": 10000000,
                "sourceEndMicros": 15000000,
                "sampledAtMicros": 12500000,
                "caption": "Lantern street",
                "tags": ["lanterns"],
                "relevanceScore": 0.9,
                "rationale": "Lanterns and wet pavement are visible."
            }
        ],
        "warnings": []
    });
    let mut client = OperationsHarness::new(AutomationOperations::new()).await;
    let invalid_constraints = client
        .execute(
            "tinixcut_edit_plan_scene_selection_suggest",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "goal": "Keep the lantern scene",
                "report": report.clone(),
                "targetDurationSeconds": 5,
                "mustKeepSceneIds": ["scene-99"],
                "localModel": "not-called-because-input-is-invalid"
            }),
        )
        .await;
    assert_eq!(invalid_constraints["result"]["isError"], true);
    assert_eq!(
        invalid_constraints["result"]["structuredContent"]["code"],
        "SCENE_SELECTION_SUGGESTION_FAILED"
    );
    assert!(
        invalid_constraints["result"]["structuredContent"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("Must-keep")
    );
    let invalid_asset_policy = client
        .execute(
            "tinixcut_edit_plan_scene_draft",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "goal": "Restrict sources",
                "report": report.clone(),
                "selectedSceneIds": ["scene-02"],
                "allowedAssetIds": ["missing-asset"]
            }),
        )
        .await;
    assert_eq!(invalid_asset_policy["result"]["isError"], true);
    assert!(
        invalid_asset_policy["result"]["structuredContent"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("Allowed source asset IDs")
    );
    let invalid_preview_constraints = client
        .execute(
            "tinixcut_edit_plan_scene_draft",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "goal": "Keep the required scene",
                "report": report.clone(),
                "selectedSceneIds": ["scene-01"],
                "targetDurationFrames": 120,
                "exactDuration": true,
                "mustKeepSceneIds": ["scene-02"],
                "mustOmitSceneIds": ["scene-01"]
            }),
        )
        .await;
    assert_eq!(invalid_preview_constraints["result"]["isError"], true);
    assert!(
        invalid_preview_constraints["result"]["structuredContent"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("must-keep")
    );
    let valid_preview_constraints = client
        .execute(
            "tinixcut_edit_plan_scene_draft",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "goal": "Keep only the lantern scene",
                "report": report.clone(),
                "selectedSceneIds": ["scene-02"],
                "targetDurationFrames": 120,
                "exactDuration": true,
                "mustKeepSceneIds": ["scene-02"],
                "mustOmitSceneIds": ["scene-01"],
                "allowedAssetIds": [asset.asset_id],
                "targetCanvas": "portrait_9x16",
                "openingTitle": "PHASE 55 TITLE",
                "titleTemplate": "bold_headline"
            }),
        )
        .await;
    assert_eq!(
        valid_preview_constraints["result"]["isError"], false,
        "{valid_preview_constraints}"
    );
    assert_eq!(
        valid_preview_constraints["result"]["structuredContent"]["validation"]["valid"],
        true
    );
    assert_eq!(
        valid_preview_constraints["result"]["structuredContent"]["plan"]["operations"][0]
            ["operation"]["type"],
        "ProjectSettingsSet"
    );
    assert_eq!(
        valid_preview_constraints["result"]["structuredContent"]["plan"]["operations"][0]
            ["operation"]["payload"]["width"],
        1080
    );
    assert_eq!(
        valid_preview_constraints["result"]["structuredContent"]["plan"]["operations"][0]
            ["operation"]["payload"]["height"],
        1920
    );
    assert_eq!(
        valid_preview_constraints["result"]["structuredContent"]["plan"]["operations"][2]
            ["operation"]["payload"]["duration"]["value"],
        120
    );
    assert_eq!(
        valid_preview_constraints["result"]["structuredContent"]["plan"]["operations"][5]
            ["operation"]["payload"]["text"],
        "PHASE 55 TITLE"
    );
    assert_eq!(
        valid_preview_constraints["result"]["structuredContent"]["plan"]["operations"][6]
            ["operation"]["payload"]["template_id"],
        "bold-headline"
    );
    let incomplete_title = client
        .execute(
            "tinixcut_edit_plan_scene_draft",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "goal": "Reject incomplete title brief",
                "report": report.clone(),
                "selectedSceneIds": ["scene-02"],
                "openingTitle": "Title without a style"
            }),
        )
        .await;
    assert_eq!(incomplete_title["result"]["isError"], true);
    assert!(incomplete_title["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .contains("supplied together"));
    let preview = client
        .execute(
            "tinixcut_edit_plan_scene_draft",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "goal": "Put the lantern scene first",
                "report": report.clone(),
                "selectedSceneIds": ["scene-02", "scene-01"],
                "targetDurationFrames": 240,
                "exactDuration": true
            }),
        )
        .await;
    assert_eq!(preview["result"]["isError"], false, "{preview}");
    let receipt = &preview["result"]["structuredContent"];
    assert_eq!(receipt["provider"], "visual-scene-report-v1");
    assert_eq!(receipt["plan"]["baseRevision"], project.revision);
    assert_eq!(receipt["validation"]["valid"], true, "{receipt}");
    let operations = receipt["plan"]["operations"].as_array().unwrap();
    assert_eq!(operations.len(), 3);
    assert_eq!(operations[0]["operation"]["type"], "TrackCreate");
    assert_eq!(operations[1]["operation"]["type"], "ClipCreate");
    assert_eq!(
        operations[1]["operation"]["payload"]["source_start"]["value"],
        300
    );
    assert_eq!(
        operations[2]["operation"]["payload"]["source_start"]["value"],
        0
    );
    assert_eq!(
        operations[1]["operation"]["payload"]["duration"]["value"],
        120
    );
    assert_eq!(
        operations[2]["operation"]["payload"]["duration"]["value"],
        120
    );
    let manifest_path = dir.join("scene-workflow.json");
    let manifest_result = client
        .execute(
            "tinixcut_workflow_manifest_create",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "manifestPath": manifest_path.to_string_lossy(),
                "plan": receipt["plan"].clone(),
                "artifactPaths": [dir.join("scene-draft.mp4").to_string_lossy()]
            }),
        )
        .await;
    assert_eq!(
        manifest_result["result"]["isError"], false,
        "{manifest_result}"
    );
    assert_eq!(
        manifest_result["result"]["structuredContent"]["manifest"]["planSha256"]
            .as_str()
            .map(str::len),
        Some(64)
    );
    let manifest_text = fs::read_to_string(&manifest_path).unwrap();
    assert!(!manifest_text.contains("Put the lantern scene first"));
    assert!(!manifest_text.contains(&asset.path));
    assert!(manifest_text.contains("scene-draft.mp4"));
    let validate = client
        .execute(
            "tinixcut_workflow_manifest_validate",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "manifestPath": manifest_path.to_string_lossy(),
                "plan": receipt["plan"].clone()
            }),
        )
        .await;
    assert_eq!(validate["result"]["isError"], false, "{validate}");
    assert_eq!(validate["result"]["structuredContent"]["valid"], true);
    assert_eq!(validate["result"]["structuredContent"]["state"], "planned");
    let mut changed_plan = receipt["plan"].clone();
    changed_plan["goal"] = serde_json::json!("Modified retry plan");
    let rejected = client
        .execute(
            "tinixcut_workflow_manifest_validate",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "manifestPath": manifest_path.to_string_lossy(),
                "plan": changed_plan
            }),
        )
        .await;
    assert_eq!(rejected["result"]["isError"], true, "{rejected}");
    assert_eq!(
        rejected["result"]["structuredContent"]["code"],
        "WORKFLOW_RESUME_REJECTED"
    );
    let mut changed_media = fs::read(&media_path).unwrap();
    changed_media.extend_from_slice(b"changed source bytes");
    fs::write(&media_path, changed_media).unwrap();
    let changed_input = client
        .execute(
            "tinixcut_workflow_manifest_validate",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "manifestPath": manifest_path.to_string_lossy(),
                "plan": receipt["plan"].clone()
            }),
        )
        .await;
    assert_eq!(changed_input["result"]["isError"], true, "{changed_input}");
    assert_eq!(
        changed_input["result"]["structuredContent"]["code"],
        "WORKFLOW_RESUME_REJECTED"
    );
    let collision = client
        .execute(
            "tinixcut_workflow_manifest_create",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "manifestPath": manifest_path.to_string_lossy(),
                "plan": receipt["plan"].clone(),
                "artifactPaths": []
            }),
        )
        .await;
    assert_eq!(collision["result"]["isError"], true);
    let mut stale_project = load_project(&project_path).unwrap();
    stale_project.revision += 1;
    save_project_atomic(&stale_project, &project_path).unwrap();
    let stale_revision = client
        .execute(
            "tinixcut_workflow_manifest_validate",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "manifestPath": manifest_path.to_string_lossy(),
                "plan": receipt["plan"].clone()
            }),
        )
        .await;
    assert_eq!(
        stale_revision["result"]["isError"], true,
        "{stale_revision}"
    );
    assert_eq!(
        stale_revision["result"]["structuredContent"]["code"],
        "WORKFLOW_RESUME_REJECTED"
    );
    let unchanged = load_project(&project_path).unwrap();
    assert_eq!(unchanged.revision, project.revision + 1);
    assert_eq!(unchanged.tracks, project.tracks);
    assert_eq!(unchanged.assets, project.assets);

    let mut stale_report = report.clone();
    stale_report["sourceFingerprint"] = serde_json::Value::String("f".repeat(64));
    let stale = client
        .execute(
            "tinixcut_edit_plan_scene_draft",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "goal": "Reject stale scene metadata",
                "report": stale_report,
                "selectedSceneIds": ["scene-01"]
            }),
        )
        .await;
    assert_eq!(stale["result"]["isError"], true);
    assert!(stale["result"]["structuredContent"]["message"]
        .as_str()
        .unwrap_or_default()
        .contains("scene"));
    assert!(media_path.is_file());
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn test_p57_operations_export_preflight_is_read_only_and_reports_replacement() {
    let dir = test_dir("p57_export_preflight");
    let (project_path, project) = create_sample_project(&dir, "preflight_project");
    let output_path = dir.join("existing-output.mp4");
    fs::write(
        &output_path,
        b"preserve this output until a render succeeds",
    )
    .unwrap();
    let mut client = OperationsHarness::new(AutomationOperations::new()).await;

    let report = client
        .execute(
            "tinixcut_export_preflight",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "outputPath": output_path.to_string_lossy()
            }),
        )
        .await;
    assert_eq!(report["result"]["isError"], false, "{report}");
    let evidence = &report["result"]["structuredContent"];
    assert_eq!(evidence["projectId"], project.project_id);
    assert_eq!(evidence["projectRevision"], project.revision);
    assert_eq!(evidence["outputExists"], true);
    assert!(evidence["availableDiskBytes"].as_u64().unwrap() > 0);
    assert!(evidence["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|warning| warning
            .as_str()
            .unwrap()
            .contains("replaced only after a successful render")));
    assert_eq!(load_project(&project_path).unwrap(), project);
    assert_eq!(
        fs::read(&output_path).unwrap(),
        b"preserve this output until a render succeeds"
    );

    let invalid_destination = client
        .execute(
            "tinixcut_export_preflight",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "outputPath": dir.join("missing-folder/out.mp4").to_string_lossy()
            }),
        )
        .await;
    assert_eq!(invalid_destination["result"]["isError"], true);
    assert_eq!(
        invalid_destination["result"]["structuredContent"]["code"],
        "EXPORT_PREFLIGHT_FAILED"
    );
    assert_eq!(load_project(&project_path).unwrap(), project);
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn test_p57_operations_export_preflight_rejects_offline_media_without_mutation() {
    let dir = test_dir("p57_export_preflight_offline_media");
    let (project_path, project, media_path) =
        create_sample_project_with_media(&dir, "offline_preflight_project", 1);
    fs::remove_file(&media_path).expect("fixture media should be removable for offline test");
    let output_path = dir.join("must-not-be-created.mp4");
    let mut client = OperationsHarness::new(AutomationOperations::new()).await;

    let report = client
        .execute(
            "tinixcut_export_preflight",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "outputPath": output_path.to_string_lossy()
            }),
        )
        .await;

    assert_eq!(report["result"]["isError"], true, "{report}");
    assert_eq!(
        report["result"]["structuredContent"]["code"],
        "EXPORT_PREFLIGHT_FAILED"
    );
    assert!(report["result"]["structuredContent"]["message"]
        .as_str()
        .unwrap_or_default()
        .contains("offline or missing"));

    let start = client
        .execute(
            "tinixcut_export_start",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "outputPath": output_path.to_string_lossy()
            }),
        )
        .await;

    assert_eq!(start["result"]["isError"], true, "{start}");
    assert_eq!(
        start["result"]["structuredContent"]["code"],
        "EXPORT_PREFLIGHT_FAILED"
    );
    assert!(start["result"]["structuredContent"]["message"]
        .as_str()
        .unwrap_or_default()
        .contains("offline or missing"));
    assert_eq!(load_project(&project_path).unwrap(), project);
    assert!(!output_path.exists());
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn test_p56_operations_workflow_checkpoints_cover_apply_save_and_export() {
    let dir = test_dir("p56_workflow_checkpoints");
    let (project_path, project) = create_sample_project(&dir, "checkpoint_project");
    let plan = tinixcut_lib::modules::automation::edit_plan::EditPlan {
        schema_version: 1,
        goal: "Create one empty video track".to_string(),
        project_id: project.project_id.clone(),
        base_revision: project.revision,
        referenced_media_ids: Vec::new(),
        assumptions: Vec::new(),
        warnings: Vec::new(),
        operations: vec![OperationEnvelope {
            operation_id: "p56_checkpoint_track".to_string(),
            operation: CommandOperation::TrackCreate {
                track_id: Some("p56_checkpoint_video".to_string()),
                name: "Checkpoint video".to_string(),
                kind: TrackKind::Video,
            },
        }],
    };
    let plan_json = serde_json::to_value(&plan).unwrap();
    let manifest_path = dir.join("workflow.json");
    let artifact_path = dir.join("workflow.mp4");
    let mut client = OperationsHarness::new(AutomationOperations::new()).await;

    let created = client
        .execute(
            "tinixcut_workflow_manifest_create",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "manifestPath": manifest_path.to_string_lossy(),
                "plan": plan_json,
                "artifactPaths": [artifact_path.to_string_lossy()]
            }),
        )
        .await;
    assert_eq!(created["result"]["isError"], false, "{created}");

    let applied = client
        .execute(
            "tinixcut_apply",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "envelope": {
                    "schemaVersion": CURRENT_SCHEMA_VERSION,
                    "projectId": project.project_id,
                    "baseRevision": project.revision,
                    "clientId": "phase56-acceptance",
                    "clientEpoch": 1,
                    "sequenceNumber": 1,
                    "dryRun": false,
                    "transactionId": "p56-checkpoint-transaction",
                    "operations": plan.operations
                }
            }),
        )
        .await;
    assert_eq!(applied["result"]["isError"], false, "{applied}");

    for state in ["applied", "saved"] {
        let checkpoint = client
            .execute(
                "tinixcut_workflow_manifest_checkpoint",
                serde_json::json!({
                    "projectPath": project_path.to_string_lossy(),
                    "manifestPath": manifest_path.to_string_lossy(),
                    "plan": plan_json,
                    "state": state
                }),
            )
            .await;
        assert_eq!(checkpoint["result"]["isError"], false, "{checkpoint}");
        assert_eq!(
            checkpoint["result"]["structuredContent"]["checkpoint"]["state"],
            state
        );
    }

    generate_synthetic_mp4(&artifact_path, 1, "blue");
    let exported = client
        .execute(
            "tinixcut_workflow_manifest_checkpoint",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "manifestPath": manifest_path.to_string_lossy(),
                "plan": plan_json,
                "state": "exported",
                "artifactPath": artifact_path.to_string_lossy()
            }),
        )
        .await;
    assert_eq!(exported["result"]["isError"], false, "{exported}");
    let evidence = &exported["result"]["structuredContent"]["checkpoint"];
    assert_eq!(evidence["videoCodec"], "h264");
    assert_eq!(evidence["width"], 128);
    assert_eq!(evidence["height"], 128);
    assert_eq!(evidence["artifactSha256"].as_str().unwrap().len(), 64);

    let retry = client
        .execute(
            "tinixcut_workflow_manifest_checkpoint",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "manifestPath": manifest_path.to_string_lossy(),
                "plan": plan_json,
                "state": "exported",
                "artifactPath": artifact_path.to_string_lossy()
            }),
        )
        .await;
    assert_eq!(retry["result"]["isError"], false, "{retry}");
    assert_eq!(
        retry["result"]["structuredContent"]["alreadyRecorded"],
        true
    );
    let persisted = load_project(&project_path).unwrap();
    assert_eq!(persisted.revision, project.revision + 1);
    assert_eq!(persisted.tracks.len(), project.tracks.len() + 1);
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
#[ignore = "requires LM Studio local API with google/gemma-4-e4b loaded"]
async fn test_p46_cli_local_vision_and_scene_selection_are_read_only() {
    fn cli_call(name: &str, arguments: serde_json::Value) -> serde_json::Value {
        let mut child = Command::new(env!("CARGO_BIN_EXE_tinixcut"))
            .args(["call", name, "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(serde_json::to_string(&arguments).unwrap().as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "CLI {name} failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }

    let dir = test_dir("p46_cli_operations_vision");
    let (project_path, mut project) = create_sample_project(&dir, "p46_scene_project");
    let media_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../artifacts/phase44/disposable/phase44_hoi_an_visual_fixture.mp4");
    let asset = Asset {
        asset_id: "ast_p46_scene_project".to_string(),
        name: "phase44_hoi_an_visual_fixture.mp4".to_string(),
        path: media_path.to_string_lossy().to_string(),
        media_kind: MediaKind::Video,
        duration: Some(RationalTime::new(450, FrameRate::FPS_30)),
        file_size: fs::metadata(&media_path).unwrap().len(),
        dimensions: None,
    };
    project.assets.push(asset.clone());
    save_project_atomic(&project, &project_path).unwrap();

    let cli_visual = cli_call(
        "media_visual_scenes",
        serde_json::json!({
            "media": [{"assetId":asset.asset_id,"mediaPath":media_path,"displayName":asset.name}],
            "query": "Describe useful scenes for a travel montage from the selected video.",
            "model": "google/gemma-4-e4b",
            "maxScenes": 3
        }),
    );
    let cli_report = &cli_visual["result"]["structuredContent"];
    let cli_scene_shape = cli_report["scenes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|scene| {
            (
                scene["sceneId"].as_str().unwrap().to_string(),
                scene["sourceStartMicros"].as_u64().unwrap(),
                scene["sourceEndMicros"].as_u64().unwrap(),
                scene["sampledAtMicros"].as_u64().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(cli_visual["result"]["isError"], false, "{cli_visual}");
    assert_eq!(cli_report["provider"], "lm-studio-local");
    assert_eq!(cli_report["model"], "google/gemma-4-e4b");
    assert!(!cli_scene_shape.is_empty() && cli_scene_shape.len() <= 3);
    assert!(cli_report["scenes"].as_array().unwrap().iter().all(|scene| {
        !scene["caption"].as_str().unwrap_or_default().is_empty()
            && !scene["tags"].as_array().unwrap().is_empty()
    }));

    let selection_args = serde_json::json!({
        "projectPath": project_path.to_string_lossy(),
        "goal": "Create a calm 5-second travel montage using the strongest available moments.",
        "report": cli_report,
        "targetDurationSeconds": 5,
        "localModel": "google/gemma-4-e4b"
    });
    let cli_suggestion = cli_call("edit_plan_scene_selection_suggest", selection_args.clone());
    assert_eq!(cli_suggestion["result"]["isError"], false, "{cli_suggestion}");
    let content = &cli_suggestion["result"]["structuredContent"];
    assert_eq!(content["provider"], "lm-studio-local");
    assert_eq!(content["model"], "google/gemma-4-e4b");
    assert_eq!(content["targetDurationSeconds"], 5);
    let ids = content["orderedSceneIds"].as_array().unwrap();
    assert!(!ids.is_empty());
    assert!(ids.iter().all(|id| {
        cli_report["scenes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|scene| &scene["sceneId"] == id)
    }));
    let unchanged = load_project(&project_path).unwrap();
    assert_eq!(unchanged.revision, project.revision);
    assert_eq!(unchanged.tracks, project.tracks);
    assert_eq!(unchanged.assets, project.assets);
    assert!(!cli_scene_shape.is_empty());
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn test_p15_ac11_capabilities_report() {
    let server = AutomationOperations::new();
    let mut client = OperationsHarness::new(server).await;

    let res = client
        .execute("tinixcut_capabilities", serde_json::json!({}))
        .await;
    assert_eq!(res["result"]["isError"], false);
    let caps = &res["result"]["structuredContent"];
    assert_eq!(caps["schemaVersion"], CURRENT_SCHEMA_VERSION);
    assert!(caps["supportedOperations"].as_array().unwrap().len() >= 25);
    assert!(caps["supportedTransitions"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("CrossDissolve")));
    assert!(caps["supportedExportCodecs"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("libx264")));
}

#[tokio::test]
async fn test_p15_ac12_ac13_dry_run_invariant_and_ledger_isolation() {
    let dir = test_dir("dry_run");
    let (proj_path, project) = create_sample_project(&dir, "dry_proj");
    let initial_meta = fs::metadata(&proj_path).unwrap();

    let server = AutomationOperations::new();
    let mut client = OperationsHarness::new(server).await;

    let envelope = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: 0,
        client_id: Some("agent-dry".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(1),
        dry_run: true,
        transaction_id: Some("tx-dry-1".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op-dry-1".to_string(),
            operation: CommandOperation::TrackCreate {
                track_id: Some("track_v2".to_string()),
                name: "Track 2".to_string(),
                kind: TrackKind::Video,
            },
        }],
    };

    let args = serde_json::json!({
        "projectPath": proj_path.to_str().unwrap(),
        "envelope": envelope
    });

    let res = client.execute("tinixcut_dry_run", args).await;
    assert_eq!(res["result"]["isError"], false);
    let diff = &res["result"]["structuredContent"];
    assert_eq!(diff["projectId"], project.project_id);
    assert_eq!(diff["baseRevision"], 0);
    assert_eq!(diff["operationsEvaluated"], 1);
    assert_eq!(diff["changes"].as_array().unwrap().len(), 1);

    // AC-12 & AC-13: Invariant checks
    // 1. File on disk was untouched
    let after_meta = fs::metadata(&proj_path).unwrap();
    assert_eq!(initial_meta.len(), after_meta.len());

    // 2. Project reloaded has revision 0 and zero ledger entries
    let loaded = load_project(&proj_path).unwrap();
    assert_eq!(loaded.revision, 0);
    assert_eq!(loaded.command_ledger.len(), 0);
}

// ===========================================================================
// P15-AC-14 through P15-AC-21: Mutation, Atomic Persistence, Structured Identity & Idempotency
// ===========================================================================

#[tokio::test]
async fn test_p15_ac14_ac15_apply_execution_and_durable_persistence() {
    let dir = test_dir("apply_durable");
    let (proj_path, project) = create_sample_project(&dir, "apply_proj");

    let server = AutomationOperations::new();
    let mut client = OperationsHarness::new(server).await;

    let envelope = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: 0,
        client_id: Some("agent-1".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(1),
        dry_run: false,
        transaction_id: Some("tx-apply-1".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op-apply-1".to_string(),
            operation: CommandOperation::TrackCreate {
                track_id: Some("track_v2".to_string()),
                name: "Video Track 2".to_string(),
                kind: TrackKind::Video,
            },
        }],
    };

    let args = serde_json::json!({
        "projectPath": proj_path.to_str().unwrap(),
        "envelope": envelope
    });

    let res = client.execute("tinixcut_apply", args).await;
    assert_eq!(res["result"]["isError"], false);
    let receipt: CommandReceipt =
        serde_json::from_value(res["result"]["structuredContent"].clone()).unwrap();
    assert_eq!(receipt.prior_revision, 0);
    assert_eq!(receipt.new_revision, 1);
    assert_eq!(receipt.operations_applied, 1);

    // AC-15: Reload from disk, verify track exists and revision is 1
    let reloaded = load_project(&proj_path).unwrap();
    assert_eq!(reloaded.revision, 1);
    assert_eq!(reloaded.tracks.len(), 2);
    assert_eq!(reloaded.tracks[1].track_id, "track_v2");
    assert_eq!(reloaded.command_ledger.len(), 1);
}

#[tokio::test]
async fn test_p18_audio_crossfade_operations_uses_command_service_and_idempotency() {
    let dir = test_dir("p18_audio_crossfade_operations");
    let project_path = dir.join("audio_crossfade.tcproj");
    let rate = FrameRate::FPS_30;
    let mut project = Project::new("Phase 18 shared operations".to_string());
    let mut track = Track::new("Audio Track".to_string(), TrackKind::Audio);

    for (index, (timeline_start, source_start)) in [(0, 10), (30, 20)].into_iter().enumerate() {
        let asset_id = format!("ast_p18_{index}");
        project.assets.push(Asset {
            asset_id: asset_id.clone(),
            name: format!("{asset_id}.wav"),
            path: dir
                .join(format!("{asset_id}.wav"))
                .to_string_lossy()
                .to_string(),
            media_kind: MediaKind::Audio,
            duration: Some(RationalTime::new(100, rate)),
            file_size: 1,
            dimensions: None,
        });
        track.items.push(
            TimelineItem::new(
                Some(asset_id),
                format!("Audio clip {index}"),
                ItemKind::Audio,
                TimeRange::new(
                    RationalTime::new(timeline_start, rate),
                    RationalTime::new(30, rate),
                ),
                TimeRange::new(
                    RationalTime::new(source_start, rate),
                    RationalTime::new(30, rate),
                ),
            )
            .unwrap(),
        );
    }
    let left_id = track.items[0].item_id.clone();
    let right_id = track.items[1].item_id.clone();
    let track_id = track.track_id.clone();
    project.tracks.push(track);
    save_project_atomic(&project, &project_path).unwrap();

    let mut client = OperationsHarness::new(AutomationOperations::new()).await;
    let create_envelope = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: 0,
        client_id: Some("p18-agent".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(1),
        dry_run: false,
        transaction_id: Some("tx_p18_operations_create".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op_p18_operations_create".to_string(),
            operation: CommandOperation::AudioCrossfadeCreate {
                transition_id: Some("audio_xfade_operations".to_string()),
                track_id,
                left_item_id: left_id,
                right_item_id: right_id,
                duration: RationalTime::new(10, rate),
            },
        }],
    };
    let create_args = serde_json::json!({
        "projectPath": project_path.to_string_lossy(),
        "envelope": create_envelope,
    });
    let created = client
        .execute("tinixcut_apply", create_args.clone())
        .await;
    assert_eq!(created["result"]["isError"], false, "{created:?}");
    let created_receipt: CommandReceipt =
        serde_json::from_value(created["result"]["structuredContent"].clone()).unwrap();
    assert_eq!(created_receipt.prior_revision, 0);
    assert_eq!(created_receipt.new_revision, 1);

    let replayed = client.execute("tinixcut_apply", create_args).await;
    assert_eq!(replayed["result"]["isError"], false, "{replayed:?}");
    let replay_receipt: CommandReceipt =
        serde_json::from_value(replayed["result"]["structuredContent"].clone()).unwrap();
    assert_eq!(replay_receipt.new_revision, 1);
    assert_eq!(replay_receipt.receipts[0].status, "deduplicated_noop");

    for (sequence_number, base_revision, operation_id, transaction_id, operation) in [
        (
            2,
            1,
            "op_p18_operations_update",
            "tx_p18_operations_update",
            CommandOperation::AudioCrossfadeUpdateDuration {
                transition_id: "audio_xfade_operations".to_string(),
                new_duration: RationalTime::new(12, rate),
            },
        ),
        (
            3,
            2,
            "op_p18_operations_remove",
            "tx_p18_operations_remove",
            CommandOperation::AudioCrossfadeRemove {
                transition_id: "audio_xfade_operations".to_string(),
            },
        ),
    ] {
        let envelope = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision,
            client_id: Some("p18-agent".to_string()),
            client_epoch: Some(1),
            sequence_number: Some(sequence_number),
            dry_run: false,
            transaction_id: Some(transaction_id.to_string()),
            operations: vec![OperationEnvelope {
                operation_id: operation_id.to_string(),
                operation,
            }],
        };
        let result = client
            .execute(
                "tinixcut_apply",
                serde_json::json!({
                    "projectPath": project_path.to_string_lossy(),
                    "envelope": envelope,
                }),
            )
            .await;
        assert_eq!(result["result"]["isError"], false, "{result:?}");
    }

    let persisted = load_project(&project_path).unwrap();
    assert_eq!(persisted.revision, 3);
    assert!(persisted.tracks[0].audio_transitions.is_empty());
    assert_eq!(persisted.command_ledger.len(), 3);
}

#[tokio::test]
async fn test_p15_ac16_save_failure_never_reports_success() {
    let dir = test_dir("save_fail");
    let (proj_path, project) = create_sample_project(&dir, "save_fail_proj");

    // Create a directory at the atomic temporary path to force a save failure
    let tmp_path = proj_path.with_extension("tcproj.tmp");
    fs::create_dir_all(&tmp_path).unwrap();

    let server = AutomationOperations::new();
    let mut client = OperationsHarness::new(server).await;

    let envelope = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: 0,
        client_id: Some("agent-fail".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(1),
        dry_run: false,
        transaction_id: Some("tx-fail-1".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op-fail-1".to_string(),
            operation: CommandOperation::TrackCreate {
                track_id: Some("track_v2".to_string()),
                name: "Video Track 2".to_string(),
                kind: TrackKind::Video,
            },
        }],
    };

    let args = serde_json::json!({
        "projectPath": proj_path.to_str().unwrap(),
        "envelope": envelope
    });

    let res = client.execute("tinixcut_apply", args).await;

    // Clean up temporary directory
    let _ = fs::remove_dir_all(&tmp_path);

    assert_eq!(res["result"]["isError"], true);
    assert_eq!(
        res["result"]["structuredContent"]["code"],
        "PROJECT_SAVE_FAILED"
    );
}

#[tokio::test]
async fn test_p15_ac17_ac18_mandatory_structured_identity() {
    let dir = test_dir("structured_id");
    let (proj_path, project) = create_sample_project(&dir, "struct_proj");

    let server = AutomationOperations::new();
    let mut client = OperationsHarness::new(server).await;

    // Case 1: Missing client_id
    let mut env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: 0,
        client_id: None,
        client_epoch: Some(1),
        sequence_number: Some(1),
        dry_run: false,
        transaction_id: Some("tx-1".to_string()),
        operations: vec![],
    };
    let res = client
        .execute(
            "tinixcut_apply",
            serde_json::json!({ "projectPath": proj_path.to_str().unwrap(), "envelope": env }),
        )
        .await;
    assert_eq!(res["result"]["isError"], true);
    assert_eq!(
        res["result"]["structuredContent"]["code"],
        "OPERATION_VALIDATION_FAILED"
    );

    // Case 2: Missing client_epoch
    env.client_id = Some("agent-1".to_string());
    env.client_epoch = None;
    let res = client
        .execute(
            "tinixcut_apply",
            serde_json::json!({ "projectPath": proj_path.to_str().unwrap(), "envelope": env }),
        )
        .await;
    assert_eq!(res["result"]["isError"], true);
    assert_eq!(
        res["result"]["structuredContent"]["code"],
        "OPERATION_VALIDATION_FAILED"
    );

    // Case 3: Missing or zero sequence_number
    env.client_epoch = Some(1);
    env.sequence_number = Some(0);
    let res = client
        .execute(
            "tinixcut_apply",
            serde_json::json!({ "projectPath": proj_path.to_str().unwrap(), "envelope": env }),
        )
        .await;
    assert_eq!(res["result"]["isError"], true);
    assert_eq!(
        res["result"]["structuredContent"]["code"],
        "OPERATION_VALIDATION_FAILED"
    );

    // Case 4: Missing transaction_id
    env.sequence_number = Some(1);
    env.transaction_id = None;
    let res = client
        .execute(
            "tinixcut_apply",
            serde_json::json!({ "projectPath": proj_path.to_str().unwrap(), "envelope": env }),
        )
        .await;
    assert_eq!(res["result"]["isError"], true);
    assert_eq!(
        res["result"]["structuredContent"]["code"],
        "OPERATION_VALIDATION_FAILED"
    );
}

#[tokio::test]
async fn test_p15_ac19_ac20_ac21_idempotency_replay_collision_and_sequence_gap() {
    let dir = test_dir("idempotency");
    let (proj_path, project) = create_sample_project(&dir, "idem_proj");

    let server = AutomationOperations::new();
    let mut client = OperationsHarness::new(server).await;

    let op1 = OperationEnvelope {
        operation_id: "op-idem-1".to_string(),
        operation: CommandOperation::TrackCreate {
            track_id: Some("track_v2".to_string()),
            name: "Video Track 2".to_string(),
            kind: TrackKind::Video,
        },
    };

    let env1 = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: 0,
        client_id: Some("agent-idem".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(1),
        dry_run: false,
        transaction_id: Some("tx-idem-1".to_string()),
        operations: vec![op1.clone()],
    };

    // 1. Initial Apply
    let res1 = client
        .execute(
            "tinixcut_apply",
            serde_json::json!({ "projectPath": proj_path.to_str().unwrap(), "envelope": env1 }),
        )
        .await;
    assert_eq!(res1["result"]["isError"], false);
    let r1: CommandReceipt =
        serde_json::from_value(res1["result"]["structuredContent"].clone()).unwrap();
    assert_eq!(r1.new_revision, 1);

    // 2. AC-19: Lost-Response Replay with exact same envelope
    let replay_res = client
        .execute(
            "tinixcut_apply",
            serde_json::json!({ "projectPath": proj_path.to_str().unwrap(), "envelope": env1 }),
        )
        .await;
    assert_eq!(replay_res["result"]["isError"], false);
    let replay_r: CommandReceipt =
        serde_json::from_value(replay_res["result"]["structuredContent"].clone()).unwrap();
    assert_eq!(
        replay_r.new_revision, 1,
        "Replay must return cached revision without re-incrementing"
    );
    assert_eq!(
        replay_r.operations_applied, 0,
        "Replay must report 0 operations applied (deduplicated no-op)"
    );
    assert_eq!(replay_r.receipts[0].status, "deduplicated_noop");

    // 3. AC-20: Collision (same operation_id op-idem-1, but different payload)
    let collision_op = OperationEnvelope {
        operation_id: "op-idem-1".to_string(),
        operation: CommandOperation::TrackCreate {
            track_id: Some("track_v3".to_string()),
            name: "Different Track".to_string(),
            kind: TrackKind::Video,
        },
    };
    let collision_env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: 1,
        client_id: Some("agent-idem".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(1),
        dry_run: false,
        transaction_id: Some("tx-idem-col".to_string()),
        operations: vec![collision_op],
    };
    let col_res = client.execute("tinixcut_apply", serde_json::json!({ "projectPath": proj_path.to_str().unwrap(), "envelope": collision_env })).await;
    assert_eq!(col_res["result"]["isError"], true);
    assert_eq!(
        col_res["result"]["structuredContent"]["code"],
        "IDEMPOTENCY_COLLISION"
    );

    // 4. AC-21: Sequence Gap (sequence jumped from 1 to 5)
    let gap_op = OperationEnvelope {
        operation_id: "op-gap-5".to_string(),
        operation: CommandOperation::TrackCreate {
            track_id: Some("track_v5".to_string()),
            name: "Gap Track".to_string(),
            kind: TrackKind::Video,
        },
    };
    let gap_env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: 1,
        client_id: Some("agent-idem".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(5),
        dry_run: false,
        transaction_id: Some("tx-gap-5".to_string()),
        operations: vec![gap_op],
    };
    let gap_res = client
        .execute(
            "tinixcut_apply",
            serde_json::json!({ "projectPath": proj_path.to_str().unwrap(), "envelope": gap_env }),
        )
        .await;
    assert_eq!(gap_res["result"]["isError"], true);
    assert_eq!(
        gap_res["result"]["structuredContent"]["code"],
        "SEQUENCE_GAP_DETECTED"
    );

    // 5. AC-21: Expired Epoch
    let exp_env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: 1,
        client_id: Some("agent-idem".to_string()),
        client_epoch: Some(0), // Expired!
        sequence_number: Some(2),
        dry_run: false,
        transaction_id: Some("tx-exp-2".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op-exp-2".to_string(),
            operation: CommandOperation::TrackCreate {
                track_id: Some("track_v_exp".to_string()),
                name: "Exp Track".to_string(),
                kind: TrackKind::Video,
            },
        }],
    };
    let exp_res = client
        .execute(
            "tinixcut_apply",
            serde_json::json!({ "projectPath": proj_path.to_str().unwrap(), "envelope": exp_env }),
        )
        .await;
    assert_eq!(exp_res["result"]["isError"], true);
    assert_eq!(
        exp_res["result"]["structuredContent"]["code"],
        "CLIENT_EPOCH_EXPIRED"
    );

    // 6. AC-20: OPERATION_ALREADY_COMMITTED (sequence already committed in past, but evicted from active ledger)
    let evicted_dir = test_dir("evicted");
    let (evicted_path, mut evicted_proj) = create_sample_project(&evicted_dir, "evicted_proj");
    evicted_proj.client_registry.clients.insert(
        "agent-evicted".to_string(),
        tinixcut_lib::modules::domain::ClientRecord {
            client_epoch: 1,
            highest_sequence: 10,
            last_active_revision: 10,
            registered_at_revision: 1,
        },
    );
    evicted_proj.revision = 10;
    save_project_atomic(&evicted_proj, &evicted_path).unwrap();

    let evicted_env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: evicted_proj.project_id.clone(),
        base_revision: 10,
        client_id: Some("agent-evicted".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(5), // 5 <= 10 (committed in past), but evicted from active ledger window!
        dry_run: false,
        transaction_id: Some("tx-evicted-5".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op-evicted-5".to_string(),
            operation: CommandOperation::TrackCreate {
                track_id: Some("track_v_evicted".to_string()),
                name: "Evicted Track".to_string(),
                kind: TrackKind::Video,
            },
        }],
    };
    let evicted_res = client
        .execute(
            "tinixcut_apply",
            serde_json::json!({ "projectPath": evicted_path.to_str().unwrap(), "envelope": evicted_env }),
        )
        .await;
    assert_eq!(evicted_res["result"]["isError"], true);
    assert_eq!(
        evicted_res["result"]["structuredContent"]["code"],
        "OPERATION_ALREADY_COMMITTED"
    );
    assert_eq!(
        evicted_res["result"]["structuredContent"]["details"]["operationId"],
        "op-evicted-5"
    );
    assert_eq!(
        evicted_res["result"]["structuredContent"]["details"]["clientId"],
        "agent-evicted"
    );
    assert_eq!(
        evicted_res["result"]["structuredContent"]["details"]["sequenceNumber"],
        5
    );
}

// ===========================================================================
// P15-AC-22 through P15-AC-27: Error Boundaries, Concurrency, & Atomicity
// ===========================================================================

#[tokio::test]
async fn test_p15_ac22_ac23_protocol_and_domain_error_boundaries() {
    let server = AutomationOperations::new();
    let mut client = OperationsHarness::new(server).await;

    // Domain errors preserve a structured format and later operations still run.
    let dir = test_dir("err_boundary");
    let (proj_path, _) = create_sample_project(&dir, "err_proj");

    let bad_apply = client
        .execute(
            "tinixcut_apply",
            serde_json::json!({
                "projectPath": proj_path.to_str().unwrap(),
                "envelope": {
                    "schemaVersion": CURRENT_SCHEMA_VERSION,
                    "projectId": "wrong-id",
                    "baseRevision": 0,
                    "clientId": "ag-1",
                    "clientEpoch": 1,
                    "sequenceNumber": 1,
                    "dryRun": false,
                    "transactionId": "tx-1",
                    "operations": []
                }
            }),
        )
        .await;
    assert_eq!(bad_apply["result"]["isError"], true);
    assert_eq!(
        bad_apply["result"]["structuredContent"]["code"],
        "PROJECT_ID_MISMATCH"
    );

    // Server is alive and executes subsequent capabilities call
    let alive = client
        .execute("tinixcut_capabilities", serde_json::json!({}))
        .await;
    assert_eq!(alive["result"]["isError"], false);
}

#[tokio::test]
async fn test_p15_ac24_authoritative_write_gate_concurrency() {
    let dir = test_dir("concurrency");
    let (proj_path, project) = create_sample_project(&dir, "conc_proj");

    let server = AutomationOperations::new();

    // Spawn 5 concurrent apply requests against the same project file
    let mut handles = Vec::new();
    for i in 1..=5 {
        let s = server.clone();
        let path_str = proj_path.to_str().unwrap().to_string();
        let pid = project.project_id.clone();
        let handle = tokio::spawn(async move {
            let mut c = OperationsHarness::new(s).await;
            let env = CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: pid,
                base_revision: 0,
                client_id: Some(format!("client-{}", i)),
                client_epoch: Some(1),
                sequence_number: Some(1),
                dry_run: false,
                transaction_id: Some(format!("tx-{}", i)),
                operations: vec![OperationEnvelope {
                    operation_id: format!("op-{}", i),
                    operation: CommandOperation::TrackCreate {
                        track_id: Some(format!("track_conc_{}", i)),
                        name: format!("Track {}", i),
                        kind: TrackKind::Video,
                    },
                }],
            };
            c.execute(
                "tinixcut_apply",
                serde_json::json!({
                    "projectPath": path_str,
                    "envelope": env
                }),
            )
            .await
        });
        handles.push(handle);
    }

    for h in handles {
        let val = h.await.unwrap();
        // Each call was processed safely without corrupting the project file or panicking
        assert!(val.get("result").is_some());
    }

    // Verify project file is intact and valid
    let loaded = load_project(&proj_path).expect("Project file must not be corrupted");
    assert!(loaded.revision >= 1);
}

#[tokio::test]
async fn test_p15_ac25_stale_revision_rejection() {
    let dir = test_dir("stale_rev");
    let (proj_path, project) = create_sample_project(&dir, "stale_proj");

    let server = AutomationOperations::new();
    let mut client = OperationsHarness::new(server).await;

    let env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: 999, // Current is 0
        client_id: Some("agent-stale".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(1),
        dry_run: false,
        transaction_id: Some("tx-stale-1".to_string()),
        operations: vec![],
    };

    let res = client
        .execute(
            "tinixcut_apply",
            serde_json::json!({
                "projectPath": proj_path.to_str().unwrap(),
                "envelope": env
            }),
        )
        .await;

    assert_eq!(res["result"]["isError"], true);
    assert_eq!(res["result"]["structuredContent"]["code"], "STALE_REVISION");
    assert_eq!(res["result"]["structuredContent"]["details"]["expected"], 0);
    assert_eq!(
        res["result"]["structuredContent"]["details"]["received"],
        999
    );
}

#[tokio::test]
async fn test_p15_ac26_multi_op_atomicity() {
    let dir = test_dir("multi_op");
    let (proj_path, project) = create_sample_project(&dir, "multi_proj");

    let server = AutomationOperations::new();
    let mut client = OperationsHarness::new(server).await;

    // Operation 1 is valid (create track), Operation 2 is invalid (create duplicate track id)
    let op1 = OperationEnvelope {
        operation_id: "op-valid-1".to_string(),
        operation: CommandOperation::TrackCreate {
            track_id: Some("track_v2".to_string()),
            name: "Track 2".to_string(),
            kind: TrackKind::Video,
        },
    };
    let op2 = OperationEnvelope {
        operation_id: "op-invalid-2".to_string(),
        operation: CommandOperation::TrackDelete {
            track_id: "non_existent_track_999".to_string(),
        },
    };

    let env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: 0,
        client_id: Some("agent-m".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(1),
        dry_run: false,
        transaction_id: Some("tx-multi-1".to_string()),
        operations: vec![op1, op2],
    };

    let res = client
        .execute(
            "tinixcut_apply",
            serde_json::json!({
                "projectPath": proj_path.to_str().unwrap(),
                "envelope": env
            }),
        )
        .await;

    assert_eq!(res["result"]["isError"], true);
    assert_eq!(
        res["result"]["structuredContent"]["code"],
        "OPERATION_VALIDATION_FAILED"
    );

    // Atomicity check: reload from disk, verify track_v2 was NOT created and revision is still 0
    let reloaded = load_project(&proj_path).unwrap();
    assert_eq!(reloaded.revision, 0);
    assert_eq!(reloaded.tracks.len(), 1);
    assert_eq!(reloaded.command_ledger.len(), 0);
}

#[tokio::test]
async fn test_p15_ac27_save_reopen_parity() {
    let dir = test_dir("reopen_parity");
    let (proj_path, project) = create_sample_project(&dir, "parity_proj");

    let server = AutomationOperations::new();
    let mut client = OperationsHarness::new(server).await;

    let env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: 0,
        client_id: Some("agent-p".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(1),
        dry_run: false,
        transaction_id: Some("tx-p-1".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op-p-1".to_string(),
            operation: CommandOperation::TrackCreate {
                track_id: Some("track_v2".to_string()),
                name: "Track 2".to_string(),
                kind: TrackKind::Video,
            },
        }],
    };

    let res = client
        .execute(
            "tinixcut_apply",
            serde_json::json!({
                "projectPath": proj_path.to_str().unwrap(),
                "envelope": env
            }),
        )
        .await;
    assert_eq!(res["result"]["isError"], false);

    // Reopen directly with domain persistence and verify all domain structures
    let reopened = load_project(&proj_path).expect("Must reopen with standard persistence");
    assert_eq!(reopened.schema_version, CURRENT_SCHEMA_VERSION);
    assert_eq!(reopened.revision, 1);
    assert_eq!(reopened.tracks.len(), 2);
    assert_eq!(reopened.command_ledger.len(), 1);
}

// ===========================================================================
// P15-AC-28 & P15-AC-29: Explicit Save / Save-As & Statelessness
// ===========================================================================

#[tokio::test]
async fn test_p15_ac28_ac29_explicit_save_as_and_stateless_save() {
    let dir = test_dir("save_as");
    let (proj_path, _) = create_sample_project(&dir, "source_proj");
    let target_path = dir.join("target_copy.tcproj");

    let server = AutomationOperations::new();
    let mut client = OperationsHarness::new(server).await;

    // AC-28: Save to targetPath
    let res = client
        .execute(
            "tinixcut_save",
            serde_json::json!({
                "projectPath": proj_path.to_str().unwrap(),
                "targetPath": target_path.to_str().unwrap(),
            }),
        )
        .await;
    assert_eq!(res["result"]["isError"], false);
    assert_eq!(
        res["result"]["structuredContent"]["savedPath"],
        target_path.to_string_lossy().to_string()
    );

    assert!(target_path.exists(), "Target project file must exist");
    let copy_proj = load_project(&target_path).unwrap();
    assert_eq!(copy_proj.name, "source_proj");

    // AC-29: Save without targetPath overwrites projectPath cleanly
    let res_overwrite = client
        .execute(
            "tinixcut_save",
            serde_json::json!({
                "projectPath": target_path.to_str().unwrap(),
            }),
        )
        .await;
    assert_eq!(res_overwrite["result"]["isError"], false);
}

// ===========================================================================
// P15-AC-30 & P15-AC-31: Export Tools
// ===========================================================================

#[tokio::test]
async fn test_p15_ac30_ac31_export_start_status_cancel() {
    let dir = test_dir("export");
    let (proj_path, _) = create_sample_project(&dir, "exp_proj");
    let out_file = dir.join("out.mp4");

    let server = AutomationOperations::new();
    let mut client = OperationsHarness::new(server).await;

    // 1. AC-30: export_start returns jobId and status
    let res = client
        .execute(
            "tinixcut_export_start",
            serde_json::json!({
                "projectPath": proj_path.to_str().unwrap(),
                "outputPath": out_file.to_str().unwrap(),
            }),
        )
        .await;
    assert_eq!(res["result"]["isError"], false);
    let job_id = res["result"]["structuredContent"]["jobId"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(job_id.starts_with("export_"));

    // 2. AC-31: export_status returns current progress
    let status_res = client
        .execute(
            "tinixcut_export_status",
            serde_json::json!({
                "jobId": job_id
            }),
        )
        .await;
    assert_eq!(status_res["result"]["isError"], false);
    assert_eq!(status_res["result"]["structuredContent"]["jobId"], job_id);

    // 3. AC-31: export_cancel cancels job
    let cancel_res = client
        .execute(
            "tinixcut_export_cancel",
            serde_json::json!({
                "jobId": job_id
            }),
        )
        .await;
    assert_eq!(cancel_res["result"]["isError"], false);
    assert_eq!(cancel_res["result"]["structuredContent"]["cancelled"], true);
}

#[tokio::test]
async fn test_p15_ac31_explicit_cancel_cleans_active_render_artifacts() {
    let dir = test_dir("explicit_cancel_cleanup");
    let (proj_path, _, _) = create_sample_project_with_media(&dir, "cancel_cleanup_proj", 60);
    let out_file = dir.join("cancel_cleanup.mp4");
    fs::write(&out_file, b"PRE_EXISTING_VALID_CONTENT").unwrap();

    let server = AutomationOperations::new();
    let mut client = OperationsHarness::new(server).await;
    let start_res = client
        .execute(
            "tinixcut_export_start",
            serde_json::json!({
                "projectPath": proj_path.to_str().unwrap(),
                "outputPath": out_file.to_str().unwrap(),
            }),
        )
        .await;
    assert_eq!(start_res["result"]["isError"], false);
    let job_id = start_res["result"]["structuredContent"]["jobId"]
        .as_str()
        .unwrap()
        .to_string();

    // Wait until the renderer has created its temporary output so this exercises
    // cancellation during an active FFmpeg render rather than a queued job.
    let temp_output_exists = (0..500).any(|_| {
        let exists = fs::read_dir(&dir).unwrap().flatten().any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .contains(".tc_render.tmp.mp4")
        });
        if !exists {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        exists
    });
    assert!(
        temp_output_exists,
        "Export did not reach active rendering before the timeout"
    );

    let cancel_res = client
        .execute(
            "tinixcut_export_cancel",
            serde_json::json!({ "jobId": job_id }),
        )
        .await;
    assert_eq!(cancel_res["result"]["isError"], false);
    assert_eq!(cancel_res["result"]["structuredContent"]["cancelled"], true);

    let status_res = client
        .execute(
            "tinixcut_export_status",
            serde_json::json!({ "jobId": job_id }),
        )
        .await;
    assert_eq!(status_res["result"]["isError"], false);
    assert_eq!(
        status_res["result"]["structuredContent"]["status"],
        "cancelled"
    );

    // Cancellation is reported synchronously; wait for the renderer thread's
    // RAII guards and partial output cleanup to finish before checking artifacts.
    let cleanup_completed = (0..250).any(|_| {
        let entries: Vec<_> = fs::read_dir(&dir).unwrap().flatten().collect();
        let has_render_temp = entries.iter().any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .contains(".tc_render.tmp.mp4")
        });
        let has_automation_temp = entries
            .iter()
            .any(|entry| entry.file_name().to_string_lossy().starts_with("tc_auto_"));
        if has_render_temp || has_automation_temp {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        !has_render_temp && !has_automation_temp
    });
    assert!(
        cleanup_completed,
        "Cancelled export artifacts were not cleaned up"
    );
    assert_eq!(fs::read(&out_file).unwrap(), b"PRE_EXISTING_VALID_CONTENT");
}

#[tokio::test]
async fn test_p15_ac31_export_snapshot_isolation() {
    let dir = test_dir("snapshot_isolation");
    let (proj_path, project, _) = create_sample_project_with_media(&dir, "snap_iso_proj", 2);
    let out_file = dir.join("isolated_export.mp4");

    let server = AutomationOperations::new();
    let mut client = OperationsHarness::new(server).await;

    // 1. Start export at revision 0
    let res = client
        .execute(
            "tinixcut_export_start",
            serde_json::json!({
                "projectPath": proj_path.to_str().unwrap(),
                "outputPath": out_file.to_str().unwrap(),
            }),
        )
        .await;
    assert_eq!(res["result"]["isError"], false);
    let job_id = res["result"]["structuredContent"]["jobId"]
        .as_str()
        .unwrap()
        .to_string();

    // 2. Immediately mutate project on disk to revision 1 (add a second track) while export is running
    let env_mut = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: 0,
        client_id: Some("agent-mut".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(1),
        dry_run: false,
        transaction_id: Some("tx-mut-1".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op-mut-1".to_string(),
            operation: CommandOperation::TrackCreate {
                track_id: Some("track_v2_isolated".to_string()),
                name: "Track 2 Added Concurrently".to_string(),
                kind: TrackKind::Video,
            },
        }],
    };
    let apply_res = client
        .execute(
            "tinixcut_apply",
            serde_json::json!({
                "projectPath": proj_path.to_str().unwrap(),
                "envelope": env_mut
            }),
        )
        .await;
    assert_eq!(apply_res["result"]["isError"], false);
    let receipt: CommandReceipt =
        serde_json::from_value(apply_res["result"]["structuredContent"].clone()).unwrap();
    assert_eq!(receipt.new_revision, 1);

    // 3. Confirm disk project is now at revision 1 with 2 tracks
    let disk_proj = load_project(&proj_path).unwrap();
    assert_eq!(disk_proj.revision, 1);
    assert_eq!(disk_proj.tracks.len(), 2);

    // 4. Poll export job until completed
    let mut finished = false;
    for _ in 0..100 {
        let status_res = client
            .execute(
                "tinixcut_export_status",
                serde_json::json!({ "jobId": job_id }),
            )
            .await;
        assert_eq!(status_res["result"]["isError"], false);
        let status = status_res["result"]["structuredContent"]["status"]
            .as_str()
            .unwrap();
        if status == "completed" {
            finished = true;
            break;
        } else if status == "failed" {
            panic!("Export failed unexpectedly: {:?}", status_res);
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    assert!(finished, "Export job must complete cleanly");
    assert!(out_file.exists(), "Exported MP4 must exist on disk");
}

// ===========================================================================
// P15-AC-35 through P15-AC-39: Shutdown, Security, Architecture & Replay
// ===========================================================================

#[tokio::test]
async fn test_p15_ac35_shutdown_job_cleanup() {
    let server = AutomationOperations::new();
    let jid = {
        let mut jm = server.jobs.lock().unwrap();
        let (id, _) = jm.submit_job(
            "export_test_cleanup".to_string(),
            tinixcut_lib::modules::job::JobKind::Export,
            "k1".to_string(),
            100,
        );
        let _ = jm.start_job(&id);
        id
    };

    // Call shutdown_and_cleanup
    server.shutdown_and_cleanup();

    // Verify job is now cancelled
    let jm = server.jobs.lock().unwrap();
    let job = jm.get_job(&jid).unwrap();
    assert_eq!(job.status, tinixcut_lib::modules::job::JobStatus::Cancelled);
}

#[tokio::test]
async fn test_p15_ac36_security_boundary_no_unsafe_tools() {
    let tools = AutomationOperations::command_schemas();

    let forbidden_substrings = [
        "shell",
        "exec",
        "cmd",
        "process",
        "fs",
        "browse",
        "file_delete",
        "ffmpeg",
    ];
    for tool in tools {
        let name = tool["name"].as_str().unwrap().to_lowercase();
        for forbidden in forbidden_substrings {
            assert!(
                !name.contains(forbidden),
                "Tool {} violates security boundary (contains '{}')",
                name,
                forbidden
            );
        }
    }
}

#[tokio::test]
async fn test_p15_ac38_project_schema_unchanged() {
    let dir = test_dir("schema_check");
    let (proj_path, project) = create_sample_project(&dir, "schema_proj");

    let server = AutomationOperations::new();
    let mut client = OperationsHarness::new(server).await;

    let env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: 0,
        client_id: Some("ag-s".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(1),
        dry_run: false,
        transaction_id: Some("tx-s-1".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op-s-1".to_string(),
            operation: CommandOperation::TrackCreate {
                track_id: Some("track_v2".to_string()),
                name: "Track 2".to_string(),
                kind: TrackKind::Video,
            },
        }],
    };

    client
        .execute(
            "tinixcut_apply",
            serde_json::json!({
                "projectPath": proj_path.to_str().unwrap(),
                "envelope": env
            }),
        )
        .await;

    let raw_text = fs::read_to_string(&proj_path).unwrap();
    let raw_json: serde_json::Value = serde_json::from_str(&raw_text).unwrap();

    // shared operations writes must preserve the current project schema without adding shared operations metadata.
    assert_eq!(raw_json["schema_version"], CURRENT_SCHEMA_VERSION);
    // Verify zero shared operations-specific metadata fields in project root
    assert!(raw_json.get("mcp").is_none());
    assert!(raw_json.get("mcpSession").is_none());
    assert!(raw_json.get("serverHandles").is_none());
}

#[tokio::test]
async fn test_p15_ac39_restart_replay_consistency() {
    let dir = test_dir("restart_replay");
    let (proj_path, project) = create_sample_project(&dir, "restart_proj");

    let op = OperationEnvelope {
        operation_id: "op-restart-1".to_string(),
        operation: CommandOperation::TrackCreate {
            track_id: Some("track_v2".to_string()),
            name: "Track 2".to_string(),
            kind: TrackKind::Video,
        },
    };

    let env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: 0,
        client_id: Some("agent-restart".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(1),
        dry_run: false,
        transaction_id: Some("tx-restart-1".to_string()),
        operations: vec![op],
    };

    // Server Instance 1 executes apply
    {
        let server1 = AutomationOperations::new();
        let mut client1 = OperationsHarness::new(server1).await;
        let res = client1
            .execute(
                "tinixcut_apply",
                serde_json::json!({
                    "projectPath": proj_path.to_str().unwrap(),
                    "envelope": env
                }),
            )
            .await;
        assert_eq!(res["result"]["isError"], false);
    } // server1 and client1 are dropped

    // Server Instance 2 starts up fresh with zero shared in-memory state
    {
        let server2 = AutomationOperations::new();
        let mut client2 = OperationsHarness::new(server2).await;
        // Re-submit identical mutation to Server 2
        let replay_res = client2
            .execute(
                "tinixcut_apply",
                serde_json::json!({
                    "projectPath": proj_path.to_str().unwrap(),
                    "envelope": env
                }),
            )
            .await;
        assert_eq!(replay_res["result"]["isError"], false);
        let receipt: CommandReceipt =
            serde_json::from_value(replay_res["result"]["structuredContent"].clone()).unwrap();
        assert_eq!(
            receipt.new_revision, 1,
            "Must recognize deduplicated replay across server restart"
        );
    }
}

#[tokio::test]
async fn test_pre_phase15_command_operation_fingerprint_compatibility_and_replay() {
    use tinixcut_lib::modules::command::compute_operation_fingerprint;
    use tinixcut_lib::modules::domain::CommandLedgerEntry;

    let dir = test_dir("pre15_compat");
    let proj_path = dir.join("pre15_proj.tcproj");

    // Pre-Phase15 logical operation:
    let op = CommandOperation::TrackCreate {
        track_id: Some("trk_pre15_1".to_string()),
        name: "Legacy Track".to_string(),
        kind: TrackKind::Video,
    };
    let pre15_fp = compute_operation_fingerprint(&op);
    assert!(pre15_fp.starts_with("fp_sha256_"));

    // Construct project with pre-Phase15 ledger entry
    let mut project = Project::new("Pre-Phase15 Project".to_string());
    project.project_id = "proj_pre15_compat".to_string();
    project.revision = 1;
    project.client_registry.clients.insert(
        "agent-pre15".to_string(),
        tinixcut_lib::modules::domain::ClientRecord {
            client_epoch: 1,
            highest_sequence: 1,
            last_active_revision: 1,
            registered_at_revision: 1,
        },
    );
    project.command_ledger.push(CommandLedgerEntry {
        operation_id: "op_pre15_1".to_string(),
        transaction_id: Some("tx_pre15_1".to_string()),
        client_id: Some("agent-pre15".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(1),
        fingerprint: pre15_fp.clone(),
        status: "applied".to_string(),
        applied_at_revision: 1,
        affected_item_id: None,
        affected_track_id: Some("trk_pre15_1".to_string()),
    });
    project
        .tracks
        .push(Track::new("Legacy Track".to_string(), TrackKind::Video));
    save_project_atomic(&project, &proj_path).expect("Save pre-Phase15 project");

    // Load with current code
    let loaded = load_project(&proj_path).expect("Load pre-Phase15 project");
    assert_eq!(loaded.command_ledger.len(), 1);
    assert_eq!(loaded.command_ledger[0].fingerprint, pre15_fp);

    // Resend exact same logical structured operation identity/payload via shared operations
    let server = AutomationOperations::new();
    let mut client = OperationsHarness::new(server).await;

    let replay_env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: "proj_pre15_compat".to_string(),
        base_revision: 1,
        client_id: Some("agent-pre15".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(1),
        dry_run: false,
        transaction_id: Some("tx_pre15_1".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op_pre15_1".to_string(),
            operation: op.clone(),
        }],
    };

    let replay_res = client
        .execute(
            "tinixcut_apply",
            serde_json::json!({
                "projectPath": proj_path.to_str().unwrap(),
                "envelope": replay_env
            }),
        )
        .await;

    assert_eq!(
        replay_res["result"]["isError"], false,
        "Must NOT fail with collision or error: {:?}",
        replay_res
    );
    let receipt: CommandReceipt =
        serde_json::from_value(replay_res["result"]["structuredContent"].clone()).unwrap();
    assert_eq!(receipt.new_revision, 1, "Must return existing revision");
    assert_eq!(receipt.operations_applied, 0, "Must be deduplicated no-op");
    assert_eq!(receipt.receipts[0].status, "deduplicated_noop");

    // Also test pre-Phase15 JSONL wire format payload deserialization
    let pre15_json = r#"{
        "schema_version": 3,
        "project_id": "proj_pre15_compat",
        "base_revision": 1,
        "client_id": "agent-pre15",
        "client_epoch": 1,
        "sequence_number": 1,
        "dry_run": false,
        "transaction_id": "tx_pre15_1",
        "operations": [
            {
                "operation_id": "op_pre15_1",
                "operation": {
                    "type": "TrackCreate",
                    "payload": {
                        "track_id": "trk_pre15_1",
                        "name": "Legacy Track",
                        "kind": "Video"
                    }
                }
            }
        ]
    }"#;
    let deserialized: CommandEnvelope =
        serde_json::from_str(pre15_json).expect("pre-Phase15 JSON must deserialize cleanly");
    assert_eq!(deserialized.schema_version, 3);
    assert_eq!(deserialized.project_id, "proj_pre15_compat");
    assert_eq!(deserialized.operations.len(), 1);
    assert_eq!(deserialized.operations[0].operation_id, "op_pre15_1");
    let fp2 = compute_operation_fingerprint(&deserialized.operations[0].operation);
    assert_eq!(
        fp2, pre15_fp,
        "Fingerprint of deserialized operation must match pre-Phase15 fingerprint exactly"
    );
}

// ===========================================================================
// P15-AC-01, AC-02, AC-32, AC-33, AC-34, AC-40: Real Subprocess Qualification
// ===========================================================================

#[tokio::test]
async fn test_p58a_destructive_apply_requires_prior_identical_dry_run_when_gate_is_on() {
    let dir = test_dir("p58a_gate");
    let (project_path, project) = create_sample_project(&dir, "gate_project");
    let server = AutomationOperations::new().with_destructive_confirmation(true);
    let mut client = OperationsHarness::new(server).await;
    let path = project_path.to_str().unwrap();

    let envelope = |base: u64, seq: u64, tx: &str, ops: serde_json::Value| {
        serde_json::json!({
            "schemaVersion": CURRENT_SCHEMA_VERSION,
            "projectId": project.project_id,
            "baseRevision": base,
            "clientId": "p58a-test",
            "clientEpoch": project.client_registry.current_epoch,
            "sequenceNumber": seq,
            "dryRun": false,
            "transactionId": tx,
            "operations": ops
        })
    };
    let create_ops = serde_json::json!([{ "operationId": "op_create", "operation": {
        "type": "TrackCreate",
        "payload": { "track_id": "trk_p58a", "name": "Scratch", "kind": "Video" } } }]);
    let delete_ops = serde_json::json!([{ "operationId": "op_delete", "operation": {
        "type": "TrackDelete", "payload": { "track_id": "trk_p58a" } } }]);

    // Non-destructive edits are never gated.
    let created = client
        .execute("tinixcut_apply", serde_json::json!({
            "projectPath": path, "envelope": envelope(project.revision, 1, "p58a-create", create_ops) }))
        .await;
    assert_eq!(created["result"]["isError"], false, "{created}");

    // Destructive apply without a dry run is refused and changes nothing.
    let refused = client
        .execute("tinixcut_apply", serde_json::json!({
            "projectPath": path, "envelope": envelope(project.revision + 1, 2, "p58a-del-1", delete_ops.clone()) }))
        .await;
    assert_eq!(refused["result"]["isError"], true, "{refused}");
    assert_eq!(
        refused["result"]["structuredContent"]["code"], "DESTRUCTIVE_CONFIRMATION_REQUIRED",
        "{refused}"
    );
    assert_eq!(
        load_project(&project_path).unwrap().revision,
        project.revision + 1
    );

    // A dry run of a different envelope does not unlock it.
    let other = client
        .execute("tinixcut_dry_run", serde_json::json!({
            "projectPath": path, "envelope": envelope(project.revision + 1, 2, "p58a-other", serde_json::json!([])) }))
        .await;
    assert_eq!(other["result"]["isError"], false, "{other}");
    let still_refused = client
        .execute("tinixcut_apply", serde_json::json!({
            "projectPath": path, "envelope": envelope(project.revision + 1, 2, "p58a-del-2", delete_ops.clone()) }))
        .await;
    assert_eq!(still_refused["result"]["isError"], true, "{still_refused}");

    // The identical envelope after its own dry run is accepted, once.
    let dry = client
        .execute("tinixcut_dry_run", serde_json::json!({
            "projectPath": path, "envelope": envelope(project.revision + 1, 2, "p58a-del-3", delete_ops.clone()) }))
        .await;
    assert_eq!(dry["result"]["isError"], false, "{dry}");
    let applied = client
        .execute("tinixcut_apply", serde_json::json!({
            "projectPath": path, "envelope": envelope(project.revision + 1, 2, "p58a-del-3", delete_ops) }))
        .await;
    assert_eq!(applied["result"]["isError"], false, "{applied}");
    assert_eq!(
        load_project(&project_path).unwrap().revision,
        project.revision + 2
    );
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn test_p58b_lut_probe_returns_a_clip_reference_that_applies() {
    let dir = test_dir("p58b_probe");
    let (project_path, project) = create_sample_project(&dir, "lut_probe_project");
    let mut cube = String::from(
        "TITLE \"Probe\"
LUT_3D_SIZE 2
",
    );
    for b in 0..2 {
        for g in 0..2 {
            for r in 0..2 {
                cube.push_str(&format!(
                    "{r} {g} {b}
"
                ));
            }
        }
    }
    let lut_path = dir.join("probe.cube");
    fs::write(&lut_path, cube).unwrap();
    let mut client = OperationsHarness::new(AutomationOperations::new()).await;

    let probe = client
        .execute(
            "tinixcut_lut_probe",
            serde_json::json!({ "lutPath": lut_path.to_str().unwrap() }),
        )
        .await;
    assert_eq!(probe["result"]["isError"], false, "{probe}");
    let report = &probe["result"]["structuredContent"];
    assert_eq!(report["lutSize"], 2);
    assert_eq!(report["clipLut"]["name"], "Probe");
    assert_eq!(report["clipLut"]["sha256"].as_str().unwrap().len(), 64);

    let missing = client
        .execute(
            "tinixcut_lut_probe",
            serde_json::json!({ "lutPath": dir.join("nope.cube").to_str().unwrap() }),
        )
        .await;
    assert_eq!(missing["result"]["isError"], true);
    assert_eq!(
        missing["result"]["structuredContent"]["code"],
        "LUT_PROBE_FAILED"
    );
    assert_eq!(
        load_project(&project_path).unwrap().revision,
        project.revision
    );
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn test_p58d_frame_scopes_reports_export_frame_statistics_read_only() {
    let dir = test_dir("p58d_scopes");
    let (project_path, project, _media) =
        create_sample_project_with_media(&dir, "scopes_project", 2);
    let mut client = OperationsHarness::new(AutomationOperations::new()).await;
    let empty = client
        .execute(
            "tinixcut_frame_scopes",
            serde_json::json!({ "projectPath": project_path.to_str().unwrap(), "frame": 0 }),
        )
        .await;
    assert_eq!(empty["result"]["isError"], false, "{empty}");
    let report = &empty["result"]["structuredContent"];
    assert_eq!(report["excludesTextOverlays"], true);
    // The sample clip is solid green (0,128,0): channel means and luma follow from that.
    let means = report["channelMeans"].as_array().unwrap();
    assert!(
        means[0].as_f64().unwrap() < 4.0 && means[2].as_f64().unwrap() < 4.0,
        "{report}"
    );
    assert!((means[1].as_f64().unwrap() - 126.0).abs() < 8.0, "{report}");
    assert!(
        (report["lumaMean"].as_f64().unwrap() - 90.0).abs() < 8.0,
        "{report}"
    );
    assert_eq!(
        report["lumaMin"], report["lumaMax"],
        "a solid frame has one luma level"
    );
    assert_eq!(report["blackClipShare"], 0.0);
    assert_eq!(
        report["lumaHistogram16"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .sum::<u64>(),
        report["width"].as_u64().unwrap() * report["height"].as_u64().unwrap()
    );

    let missing = client
        .execute(
            "tinixcut_frame_scopes",
            serde_json::json!({ "projectPath": dir.join("nope.tcproj").to_str().unwrap(), "frame": 0 }),
        )
        .await;
    assert_eq!(missing["result"]["isError"], true);
    assert_eq!(
        load_project(&project_path).unwrap().revision,
        project.revision
    );
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn test_p58e_project_chapters_lists_markers_and_reports_violations_without_changing_them() {
    let dir = test_dir("p58e_chapters");
    let (project_path, mut project) = create_sample_project(&dir, "chapters_project");
    let rate = project.settings.frame_rate;
    for (index, (label, seconds)) in [("Intro", 0i64), ("Middle", 30), ("Close", 36)]
        .iter()
        .enumerate()
    {
        project
            .markers
            .push(tinixcut_lib::modules::domain::ProjectMarker {
                marker_id: format!("mrk_{index}"),
                label: (*label).into(),
                position: RationalTime::new(seconds * 30, rate),
            });
    }
    project.revision += 1;
    save_project_atomic(&project, &project_path).unwrap();

    let mut client = OperationsHarness::new(AutomationOperations::new()).await;
    let result = client
        .execute(
            "tinixcut_project_chapters",
            serde_json::json!({ "projectPath": project_path.to_str().unwrap() }),
        )
        .await;
    assert_eq!(result["result"]["isError"], false, "{result}");
    let report = &result["result"]["structuredContent"];
    assert_eq!(report["text"], "00:00 Intro\n00:30 Middle\n00:36 Close");
    let violations = report["violations"].as_array().unwrap();
    assert!(
        violations
            .iter()
            .any(|v| v.as_str().unwrap().contains("lasts 6 s")),
        "{report}"
    );
    assert_eq!(report["chapters"].as_array().unwrap().len(), 3);
    assert_eq!(load_project(&project_path).unwrap().markers.len(), 3);

    let cli = tinixcut_lib::modules::automation::AutomationAdapter::handle_cli(&[
        "chapters".to_string(),
        project_path.to_string_lossy().to_string(),
    ])
    .unwrap();
    assert!(cli.contains("00:30 Middle"), "{cli}");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn cli_real_media_export_and_active_analysis_cancellation() {
    use std::time::{Duration, Instant};
    let dir = test_dir("cli_real_media");
    let (project_path, _, media) = create_sample_project_with_media(&dir, "cli_real", 4);
    assert!(media.is_file());
    let mut child = Command::new(env!("CARGO_BIN_EXE_tinixcut"))
        .arg("api-session").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    fn send(input: &mut ChildStdin, value: serde_json::Value) { writeln!(input, "{value}").unwrap(); input.flush().unwrap(); }
    fn receive(output: &mut BufReader<ChildStdout>) -> serde_json::Value { let mut line = String::new(); assert!(output.read_line(&mut line).unwrap() > 0); serde_json::from_str(&line).unwrap() }
    let dest = dir.join("render.mp4");
    send(&mut input, serde_json::json!({"id":1,"command":"export_start","arguments":{"projectPath":project_path,"outputPath":dest}}));
    let start = receive(&mut output); assert_eq!(start["result"]["isError"], false, "{start}");
    let job = start["result"]["structuredContent"]["jobId"].as_str().unwrap();
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        send(&mut input, serde_json::json!({"id":2,"command":"export_status","arguments":{"jobId":job}}));
        let response = receive(&mut output);
        let status = response["result"]["structuredContent"]["status"].as_str().unwrap();
        if status == "completed" { break; }
        assert!(status == "running" || status == "queued", "{response}");
        assert!(Instant::now() < deadline); std::thread::sleep(Duration::from_millis(100));
    }
    let decode = tinixcut_lib::modules::media::resolver::MediaBinaryResolver::command("ffmpeg").unwrap().args(["-v", "error", "-i", dest.to_str().unwrap(), "-f", "null", "-"]).output().unwrap();
    assert!(decode.status.success(), "{}", String::from_utf8_lossy(&decode.stderr));
    send(&mut input, serde_json::json!({"id":3,"command":"media_scene_candidates","arguments":{"mediaPath":media,"threshold":0.314159}}));
    std::thread::sleep(Duration::from_millis(50));
    send(&mut input, serde_json::json!({"id":4,"command":"cancel","arguments":{"id":3}}));
    let first = receive(&mut output); let second = receive(&mut output);
    let cancel = if first["id"] == 4 { &first } else { &second };
    let analysis = if first["id"] == 3 { &first } else { &second };
    assert_eq!(cancel["result"]["cancelled"], true, "{cancel} {analysis}");
    assert_eq!(analysis["result"]["isError"], true, "{analysis}");
    assert_eq!(analysis["result"]["structuredContent"]["code"], "OPERATION_CANCELLED", "{analysis}");
    drop(input); assert!(child.wait().unwrap().success());
}

#[tokio::test]
async fn cli_operation_success_parity_for_media_and_edit_plans() {
    let dir = test_dir("cli_read_plans");
    let (path, mut project, media) = create_sample_project_with_media(&dir, "read_plans", 4);
    let rate = FrameRate::FPS_30;
    project.tracks[0].items[0].timeline_range = TimeRange::new(RationalTime::new(0, rate), RationalTime::new(60, rate));
    project.tracks[0].items[0].source_range = TimeRange::new(RationalTime::new(0, rate), RationalTime::new(60, rate));
    let second = TimelineItem::new(Some(project.assets[0].asset_id.clone()), "Second".into(), ItemKind::Video,
        TimeRange::new(RationalTime::new(60, rate), RationalTime::new(60, rate)),
        TimeRange::new(RationalTime::new(0, rate), RationalTime::new(60, rate))).unwrap();
    let second_id = second.item_id.clone();
    project.tracks[0].items.push(second);
    let range = TimeRange::new(RationalTime::new(0, rate), RationalTime::new(60, rate));
    let mut caption = TimelineItem::new(None, "Caption".into(), ItemKind::Text, range.clone(), range).unwrap();
    caption.text_content = Some("Visit Da Nang".into());
    let caption_id = caption.item_id.clone();
    let mut captions = Track::new("Captions".into(), TrackKind::Video); captions.items.push(caption);
    project.tracks.push(captions); save_project_atomic(&project, &path).unwrap();
    let mut cli = Command::new(env!("CARGO_BIN_EXE_tinixcut")).env("TINIX_TEMPLATE_CATALOG_DIR", dir.join("catalog-cli")).arg("api-session").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let mut input = cli.stdin.take().unwrap();
    let mut output = BufReader::new(cli.stdout.take().unwrap());
    let mut operations = OperationsHarness::new(AutomationOperations::new()).await;
    let item = &project.tracks[0].items[0].item_id;
    let asset = &project.assets[0].asset_id;
    let scene_source = tinixcut_lib::modules::media::visual_scene::VisualSceneSource {
        asset_id: project.assets[0].asset_id.clone(),
        asset_name: project.assets[0].name.clone(),
        path: PathBuf::from(&project.assets[0].path),
    };
    let scene_report = serde_json::json!({
        "schemaVersion": tinixcut_lib::modules::media::visual_scene::VISUAL_SCENE_SCHEMA_VERSION,
        "algorithmVersion": tinixcut_lib::modules::media::visual_scene::VISUAL_SCENE_ALGORITHM_VERSION,
        "provider": "lm-studio-local",
        "model": "google/gemma-4-e4b",
        "query": "travel scenes",
        "sourceFingerprint": tinixcut_lib::modules::media::visual_scene::aggregate_source_fingerprint(&[scene_source]).unwrap(),
        "configFingerprint": "0".repeat(64),
        "truncated": false,
        "scenes": [
            {"sceneId":"scene-01","assetId":project.assets[0].asset_id,"assetName":project.assets[0].name,"sourceStartMicros":0,"sourceEndMicros":1000000,"sampledAtMicros":500000,"caption":"Opening view","tags":["street"],"relevanceScore":0.7,"rationale":"Buildings are visible."},
            {"sceneId":"scene-02","assetId":project.assets[0].asset_id,"assetName":project.assets[0].name,"sourceStartMicros":1000000,"sourceEndMicros":2000000,"sampledAtMicros":1500000,"caption":"Lantern street","tags":["lanterns"],"relevanceScore":0.9,"rationale":"Lanterns and wet pavement are visible."}
        ],
        "warnings": []
    });
    let scene_draft_args = serde_json::json!({
        "projectPath": path,
        "goal": "Put the lantern scene first",
        "report": scene_report,
        "selectedSceneIds": ["scene-02", "scene-01"],
        "targetDurationFrames": 60,
        "exactDuration": true
    });
    let cases = [
        ("project_compare", serde_json::json!({"leftProjectPath":path,"leftExpectedRevision":project.revision,"rightProjectPath":path,"rightExpectedRevision":project.revision})),
        ("media_probe", serde_json::json!({"mediaPath":media})),
        ("edit_plan_reorder", serde_json::json!({"projectPath":path,"goal":"Reorder","trackId":project.tracks[0].track_id,"itemIds":[second_id,item],"plannerProvider":"caller-order-v1"})),
        ("edit_plan_text_replace", serde_json::json!({"projectPath":path,"goal":"Replace","itemIds":[caption_id],"searchText":"Da Nang","replacementText":"Hoi An"})),
        ("audio_align", serde_json::json!({"projectPath":path,"referenceItemId":item,"itemIds":[second_id]})),

        ("frame_scopes", serde_json::json!({"projectPath":path,"frame":0})),
        ("edit_plan_trim", serde_json::json!({"projectPath":path,"goal":"Trim","itemId":item,"targetDurationFrames":30})),
        ("edit_plan_volume", serde_json::json!({"projectPath":path,"goal":"Set volume","itemId":item,"volume":0.6})),
        ("edit_plan_draft", serde_json::json!({"projectPath":path,"goal":"Make a cut","mediaIds":[asset],"targetDurationFrames":60})),
        ("edit_plan_scene_draft", scene_draft_args),
        ("media_scene_candidates", serde_json::json!({"mediaPath":media,"threshold":0.4})),
    ];
    for (name, args) in cases {
        writeln!(input, "{}", serde_json::json!({"id":1,"command":name,"arguments":args})).unwrap(); input.flush().unwrap();
        let mut line = String::new(); output.read_line(&mut line).unwrap();
        let actual: serde_json::Value = serde_json::from_str(&line).unwrap();
        let expected = operations.execute(name, args).await;
        assert_eq!(actual["result"]["isError"], false, "{name}: {actual}");
        assert_eq!(expected["result"]["isError"], false, "{name}: {expected}");
        if name == "edit_plan_scene_draft" {
            let left = &actual["result"]["structuredContent"];
            let right = &expected["result"]["structuredContent"];
            assert_eq!(left["provider"], right["provider"]);
            assert_eq!(left["validation"]["valid"], right["validation"]["valid"]);
            assert_eq!(left["validation"]["issues"], right["validation"]["issues"]);
            assert_eq!(
                left["validation"]["diff"]["operationsEvaluated"],
                right["validation"]["diff"]["operationsEvaluated"]
            );
            let mut left_plan = left["plan"].clone();
            let mut right_plan = right["plan"].clone();
            for plan in [&mut left_plan, &mut right_plan] {
                for operation in plan["operations"].as_array_mut().unwrap() {
                    operation.as_object_mut().unwrap().remove("operationId");
                    let payload = operation["operation"]["payload"].as_object_mut().unwrap();
                    payload.remove("track_id");
                    payload.remove("item_id");
                }
            }
            assert_eq!(left_plan, right_plan, "{name}");
        } else if name == "audio_align" {
            let left = &actual["result"]["structuredContent"];
            let right = &expected["result"]["structuredContent"];
            assert_eq!(left["referenceItemId"], right["referenceItemId"]);
            assert_eq!(left["proposals"].as_array().unwrap().len(), right["proposals"].as_array().unwrap().len());
            for (left, right) in left["proposals"].as_array().unwrap().iter().zip(right["proposals"].as_array().unwrap()) {
                for field in ["itemId", "currentStartFrame", "proposedStartFrame", "deltaFrames", "confident"] {
                    assert_eq!(left[field], right[field], "{field}");
                }
                for field in ["score", "peakRatio", "offsetSeconds"] {
                    let delta = (left[field].as_f64().unwrap() - right[field].as_f64().unwrap()).abs();
                    assert!(delta <= 1e-9, "{field} differs by {delta}");
                }
            }
        } else {
            assert_eq!(actual["result"], expected["result"], "{name}");
        }
    }
    let cli_project = dir.join("template-cli.tcproj");
    let operations_project = dir.join("template-operations.tcproj");
    fs::copy(&path, &cli_project).unwrap(); fs::copy(&path, &operations_project).unwrap();
    let draft_file = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("artifacts/animated-text-templates/phase-35/draft_vietnamese_v1.json");
    let mut draft: serde_json::Value = serde_json::from_slice(&fs::read(draft_file).unwrap()).unwrap();
    draft["templateId"] = serde_json::json!(format!("parity_template_{}", next_id()));
    let mut args = serde_json::json!({"projectPath":cli_project,"itemId":caption_id,"draft":draft,"clientId":"parity-template","clientEpoch":1,"sequenceNumber":1,"transactionId":"parity-template","baseRevision":0});
    writeln!(input, "{}", serde_json::json!({"id":1,"command":"template_apply","arguments":args})).unwrap(); input.flush().unwrap();
    let mut line=String::new(); output.read_line(&mut line).unwrap();
    let actual: serde_json::Value = serde_json::from_str(&line).unwrap();
    args["projectPath"] = serde_json::json!(operations_project);
    let expected = operations.execute("template_apply", args).await;
    assert_eq!(actual["result"]["isError"], false, "{actual}");
    assert_eq!(expected["result"]["isError"], false, "{expected}");
    let left=load_project(&cli_project).unwrap(); let right=load_project(&operations_project).unwrap();
    assert_eq!(left.revision, right.revision); assert_eq!(left.tracks, right.tracks);
    assert!(left.tracks[1].items[0].template_instance.is_some());
    drop(input);
    assert!(cli.wait().unwrap().success());
    fs::remove_dir_all(dir).unwrap();
}
