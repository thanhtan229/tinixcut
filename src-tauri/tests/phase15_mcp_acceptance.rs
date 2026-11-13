//! Phase 15 Comprehensive Acceptance Test Suite
//! Validates all 41 Acceptance Criteria (P15-AC-01 through P15-AC-41)
//! defined in Plans/plan_phase15.md, ADR-005, and ADR-001 v3.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use tinixcut_lib::modules::automation::mcp::TinixCutMcpServer;
use tinixcut_lib::modules::command::{
    CommandEnvelope, CommandOperation, CommandReceipt, OperationEnvelope,
};
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    Asset, ClipSpeed, ItemKind, MediaKind, Project, TimelineItem, Track, TrackKind,
    CURRENT_SCHEMA_VERSION,
};
use tinixcut_lib::modules::persistence::{load_project, save_project_atomic};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader as TokioBufReader};

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

fn stdio_call_tool(
    stdin: &mut ChildStdin,
    reader: &mut BufReader<ChildStdout>,
    id: u64,
    name: &str,
    arguments: serde_json::Value,
) -> serde_json::Value {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": {
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientCapabilities": {}
            },
            "name": name,
            "arguments": arguments
        }
    });
    stdin
        .write_all(format!("{}\n", serde_json::to_string(&request).unwrap()).as_bytes())
        .unwrap();
    stdin.flush().unwrap();
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let response: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(response["id"], id, "MCP response ID must match: {response}");
    response
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
        let id = next_id();
        // Inject modern 2026-07-28 metadata into params if not present
        if let Some(map) = params.as_object_mut() {
            if !map.contains_key("_meta") {
                map.insert(
                    "_meta".to_string(),
                    serde_json::json!({
                        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                        "io.modelcontextprotocol/clientCapabilities": {},
                        "io.modelcontextprotocol/clientInfo": {
                            "name": "test-client",
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

// ===========================================================================
// P15-AC-01 through P15-AC-08: Transport, Discovery, Lifecycle, & Schemas
// ===========================================================================

#[tokio::test]
async fn test_p15_ac03_modern_protocol_lifecycle() {
    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;

    // 1. Call server/discover
    let disc_res = client
        .send_request("server/discover", serde_json::json!({}))
        .await;
    assert_eq!(disc_res["jsonrpc"], "2.0");
    assert!(
        disc_res.get("result").is_some(),
        "server/discover must succeed: {:?}",
        disc_res
    );
    let disc_result = &disc_res["result"];
    // Verify response advertises 2026-07-28
    let versions = disc_result["supportedVersions"]
        .as_array()
        .expect("supportedVersions array");
    assert!(
        versions.iter().any(|v| v.as_str() == Some("2026-07-28")),
        "Must advertise 2026-07-28 in supportedVersions: {:?}",
        versions
    );
    // Verify server identity/metadata
    assert!(disc_result["serverInfo"].is_object() || disc_result["capabilities"].is_object());
    // Verify tools support
    assert!(
        disc_result["capabilities"]["tools"].is_object(),
        "Must advertise tools capability"
    );

    // 2. Direct tools/list call with 2026-07-28 _meta without any initialize handshake
    let res = client
        .send_request("tools/list", serde_json::json!({}))
        .await;
    assert_eq!(res["jsonrpc"], "2.0");
    assert!(
        res.get("result").is_some(),
        "Must succeed without initialize"
    );
    let tools = res["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 35);
}

#[tokio::test]
async fn test_p15_ac04_legacy_compatibility_smoke() {
    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;

    // Legacy initialize handshake for 2025-11-25
    let init_req = serde_json::json!({
        "_meta": {},
        "protocolVersion": "2025-11-25",
        "capabilities": {},
        "clientInfo": { "name": "legacy-agent", "version": "1.0.0" }
    });
    let res = client.send_request("initialize", init_req).await;
    assert_eq!(res["jsonrpc"], "2.0");
    assert!(
        res.get("result").is_some(),
        "Legacy initialize must be accepted"
    );
    assert_eq!(res["result"]["protocolVersion"], "2025-11-25");
}

#[tokio::test]
async fn test_p15_ac05_ac06_ac07_current_tools_and_no_extraneous_primitives() {
    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;

    let res = client
        .send_request("tools/list", serde_json::json!({}))
        .await;
    let tools = res["result"]["tools"].as_array().expect("tools array");
    assert_eq!(tools.len(), 35, "Must expose exactly 35 tools");

    let mut tool_names: Vec<String> = tools
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    tool_names.sort();

    let expected_tools = vec![
        "tinixcut_apply".to_string(),
        "tinixcut_capabilities".to_string(),
        "tinixcut_dry_run".to_string(),
        "tinixcut_edit_plan_canvas".to_string(),
        "tinixcut_edit_plan_draft".to_string(),
        "tinixcut_edit_plan_reorder".to_string(),
        "tinixcut_edit_plan_scene_draft".to_string(),
        "tinixcut_edit_plan_scene_selection_suggest".to_string(),
        "tinixcut_edit_plan_text_replace".to_string(),
        "tinixcut_edit_plan_trim".to_string(),
        "tinixcut_edit_plan_validate".to_string(),
        "tinixcut_edit_plan_volume".to_string(),
        "tinixcut_export_cancel".to_string(),
        "tinixcut_export_preflight".to_string(),
        "tinixcut_export_start".to_string(),
        "tinixcut_export_status".to_string(),
        "tinixcut_frame_scopes".to_string(),
        "tinixcut_inspect".to_string(),
        "tinixcut_lut_probe".to_string(),
        "tinixcut_media_probe".to_string(),
        "tinixcut_media_scene_candidates".to_string(),
        "tinixcut_media_transcript".to_string(),
        "tinixcut_media_visual_scenes".to_string(),
        "tinixcut_project_chapters".to_string(),
        "tinixcut_project_compare".to_string(),
        "tinixcut_project_create".to_string(),
        "tinixcut_project_variant".to_string(),
        "tinixcut_save".to_string(),
        "tinixcut_template_apply".to_string(),
        "tinixcut_template_preview".to_string(),
        "tinixcut_template_save".to_string(),
        "tinixcut_template_validate".to_string(),
        "tinixcut_workflow_manifest_checkpoint".to_string(),
        "tinixcut_workflow_manifest_create".to_string(),
        "tinixcut_workflow_manifest_validate".to_string(),
    ];
    assert_eq!(tool_names, expected_tools);

    // Verify all tools have non-empty descriptions
    for tool in tools {
        let desc = tool["description"].as_str().unwrap();
        assert!(!desc.trim().is_empty());
    }

    // Verify zero extraneous primitives (Resources, Prompts, etc.)
    let res_resources = client
        .send_request("resources/list", serde_json::json!({}))
        .await;
    assert!(
        res_resources.get("error").is_some()
            || res_resources["result"]["resources"]
                .as_array()
                .map_or(false, |a| a.is_empty()),
        "Resources must not be exposed"
    );

    let res_prompts = client
        .send_request("prompts/list", serde_json::json!({}))
        .await;
    assert!(
        res_prompts.get("error").is_some()
            || res_prompts["result"]["prompts"]
                .as_array()
                .map_or(false, |a| a.is_empty()),
        "Prompts must not be exposed"
    );
}

#[tokio::test]
async fn test_p15_ac08_json_schema_2020_12_and_camel_case() {
    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;

    let res = client
        .send_request("tools/list", serde_json::json!({}))
        .await;
    let tools = res["result"]["tools"].as_array().unwrap();

    for tool in tools {
        let in_schema = &tool["inputSchema"];
        let out_schema = &tool["outputSchema"];

        assert_eq!(
            in_schema["$schema"], "https://json-schema.org/draft/2020-12/schema",
            "Tool {} input schema must be JSON Schema 2020-12",
            tool["name"]
        );
        assert_eq!(
            out_schema["$schema"], "https://json-schema.org/draft/2020-12/schema",
            "Tool {} output schema must be JSON Schema 2020-12",
            tool["name"]
        );

        // Verify camelCase properties in properties map
        if let Some(props) = in_schema.get("properties").and_then(|p| p.as_object()) {
            for key in props.keys() {
                assert!(
                    !key.contains('_'),
                    "Input schema property '{}' in tool {} must be camelCase",
                    key,
                    tool["name"]
                );
            }
        }
    }

    let plan_tool = tools
        .iter()
        .find(|tool| tool["name"] == "tinixcut_edit_plan_validate")
        .expect("versioned edit plan validator is discoverable");
    assert_eq!(
        plan_tool["inputSchema"]["properties"]["plan"]["properties"]["schemaVersion"]["const"],
        1
    );
    assert_eq!(
        plan_tool["inputSchema"]["properties"]["plan"]["properties"]["operations"]["minItems"],
        1
    );

    let scene_draft_tool = tools
        .iter()
        .find(|tool| tool["name"] == "tinixcut_edit_plan_scene_draft")
        .expect("scene draft planner is discoverable");
    let input_schema = &scene_draft_tool["inputSchema"];
    for (property, expected) in [
        (
            "targetCanvas",
            serde_json::json!([
                "landscape_16x9",
                "portrait_9x16",
                "square_1x1",
                "portrait_4x5"
            ]),
        ),
        (
            "titleTemplate",
            serde_json::json!([
                "clean_lower_third",
                "bold_headline",
                "center_pop",
                "subtitle_card",
                "quote_card",
                "call_to_action"
            ]),
        ),
    ] {
        let reference = input_schema["properties"][property]["anyOf"][0]["$ref"]
            .as_str()
            .expect("optional enum must refer to its schema definition");
        let definition_name = reference.rsplit('/').next().unwrap();
        assert_eq!(
            input_schema["$defs"][definition_name]["enum"], expected,
            "schema enum mismatch for {property}"
        );
    }
    assert!(scene_draft_tool["inputSchema"]["properties"]
        .get("openingTitle")
        .is_some());
    assert!(scene_draft_tool["inputSchema"]["properties"]
        .get("allowedAssetIds")
        .is_some());
    let scene_suggest_tool = tools
        .iter()
        .find(|tool| tool["name"] == "tinixcut_edit_plan_scene_selection_suggest")
        .expect("scene suggestion tool is discoverable");
    assert!(scene_suggest_tool["inputSchema"]["properties"]
        .get("captionIntent")
        .is_some());
    assert!(scene_suggest_tool["inputSchema"]["properties"]
        .get("audioIntent")
        .is_some());
    assert!(scene_suggest_tool["inputSchema"]["properties"]
        .get("suggestOpeningTitle")
        .is_some());
    assert!(scene_suggest_tool["inputSchema"]["properties"]
        .get("transcriptEvidence")
        .is_some());
    assert!(scene_suggest_tool["outputSchema"]["properties"]
        .get("openingTitleSuggestion")
        .is_some());
    assert!(scene_suggest_tool["outputSchema"]["properties"]
        .get("transcriptEvidenceSources")
        .is_some());
}

#[tokio::test]
async fn test_p40_mcp_scene_candidates_are_read_only_and_cached() {
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

    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;
    let project_path = dir.join("untouched.tinix");
    let created = client
        .call_tool(
            "tinixcut_project_create",
            serde_json::json!({ "projectPath": project_path.to_str().unwrap(), "name": "Read-only analysis check" }),
        )
        .await;
    assert_eq!(created["result"]["isError"], false);
    let before = client
        .call_tool(
            "tinixcut_inspect",
            serde_json::json!({ "projectPath": project_path.to_str().unwrap() }),
        )
        .await;
    let args = serde_json::json!({ "mediaPath": fixture.to_str().unwrap(), "threshold": 0.3 });
    let first = client
        .call_tool("tinixcut_media_scene_candidates", args.clone())
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
        .call_tool("tinixcut_media_scene_candidates", args)
        .await;
    assert_eq!(
        first["result"]["structuredContent"],
        second["result"]["structuredContent"]
    );
    let limited = client
        .call_tool(
            "tinixcut_media_scene_candidates",
            serde_json::json!({ "mediaPath": fixture.to_str().unwrap(), "threshold": 0.3, "maxCandidates": 1 }),
        )
        .await;
    let limited_report = &limited["result"]["structuredContent"];
    assert_eq!(limited_report["candidates"].as_array().unwrap().len(), 1);
    assert_eq!(limited_report["truncated"], true);
    let offline = client
        .call_tool(
            "tinixcut_media_scene_candidates",
            serde_json::json!({ "mediaPath": dir.join("missing.mp4").to_str().unwrap() }),
        )
        .await;
    assert_eq!(
        offline["result"]["structuredContent"]["code"],
        "MEDIA_SCENE_ANALYSIS_INVALID"
    );
    let duration_cap = client
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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

    let saturated_server = TinixCutMcpServer::new();
    let worker_permits: Vec<_> = (0..4)
        .map(|index| {
            saturated_server
                .worker_capacity
                .try_acquire(&format!("phase40-capacity-{index}"))
                .expect("fill the four-worker capacity")
        })
        .collect();
    let mut saturated_client = DuplexMcpClient::new(saturated_server).await;
    let saturated = saturated_client
        .call_tool(
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
    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;
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
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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
    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;
    let drafted = client
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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

    let mut client = DuplexMcpClient::new(TinixCutMcpServer::new()).await;
    let drafted = client
        .call_tool(
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
async fn test_p41_mcp_opt_in_local_ai_draft_is_validated_and_read_only() {
    let dir = test_dir("local_ai_draft");
    let (project_path, project, _) = create_sample_project_with_media(&dir, "local_ai_draft", 30);
    let selected_ids = project
        .assets
        .iter()
        .map(|asset| asset.asset_id.clone())
        .collect::<Vec<_>>();
    let client = DuplexMcpClient::new(TinixCutMcpServer::new()).await;
    let mut client = client;
    let response = client
        .call_tool(
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
        .call_tool(
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
#[ignore = "binds local LM Studio port 1234; run only when no local model server is active"]
async fn test_p41_mcp_local_ai_cancellation_drops_http_request_without_project_mutation() {
    use std::io::Read as StdRead;
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::time::Duration;

    let listener = TcpListener::bind("127.0.0.1:1234")
        .expect("Port 1234 must be free for the local cancellation mock");
    let (accepted_tx, accepted_rx) = mpsc::sync_channel(1);
    let mock = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut bytes = [0_u8; 8192];
        let _ = stream.read(&mut bytes);
        accepted_tx.send(()).unwrap();
        let mut closed_by_client = false;
        loop {
            match stream.read(&mut bytes) {
                Ok(0) => {
                    closed_by_client = true;
                    break;
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
        closed_by_client
    });

    let dir = test_dir("local_ai_cancel");
    let (project_path, project, _) = create_sample_project_with_media(&dir, "local_ai_cancel", 30);
    let selected_ids = project
        .assets
        .iter()
        .map(|asset| asset.asset_id.clone())
        .collect::<Vec<_>>();
    let mut client = DuplexMcpClient::new(TinixCutMcpServer::new()).await;
    let request_id = next_id();
    let call = serde_json::json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "tools/call",
        "params": {
            "name": "tinixcut_edit_plan_draft",
            "arguments": {
                "projectPath": project_path.to_string_lossy(),
                "goal": "Create a short local cancellation test draft",
                "mediaIds": selected_ids,
                "targetDurationFrames": 60,
                "plannerProvider": "lm-studio-local",
                "localModel": "cancellation-test-model"
            },
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientCapabilities": {},
                "io.modelcontextprotocol/clientInfo": {"name":"phase41-cancel-test","version":"1"}
            }
        }
    });
    client
        .writer
        .write_all(format!("{}\n", serde_json::to_string(&call).unwrap()).as_bytes())
        .await
        .unwrap();
    client.writer.flush().await.unwrap();
    tokio::task::spawn_blocking(move || accepted_rx.recv_timeout(Duration::from_secs(3)))
        .await
        .unwrap()
        .expect("Local planner request did not reach the mock endpoint");

    let cancelled = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/cancelled",
        "params": {"requestId": request_id, "reason": "phase41 acceptance cancellation"}
    });
    client
        .writer
        .write_all(format!("{}\n", serde_json::to_string(&cancelled).unwrap()).as_bytes())
        .await
        .unwrap();
    client.writer.flush().await.unwrap();

    let ping = tokio::time::timeout(
        Duration::from_secs(2),
        client.send_request("ping", serde_json::json!({})),
    )
    .await
    .expect("MCP server did not recover after cancelling local AI inference");
    assert_eq!(ping["id"], next_id() - 1);
    assert!(
        mock.join().unwrap(),
        "Cancellation must close the pending HTTP request"
    );
    assert_eq!(load_project(&project_path).unwrap(), project);
    fs::remove_dir_all(dir).unwrap();
}

// ===========================================================================
// P15-AC-09 through P15-AC-13: Read-Only Tools, Dry-Run & Ledger Isolation
// ===========================================================================

#[tokio::test]
async fn test_p15_ac09_ac10_inspect_success_and_error() {
    let dir = test_dir("inspect");
    let (proj_path, project) = create_sample_project(&dir, "inspect_proj");

    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;

    // AC-09: Inspect success
    let args = serde_json::json!({
        "projectPath": proj_path.to_str().unwrap()
    });
    let res = client.call_tool("tinixcut_inspect", args).await;
    assert_eq!(res["result"]["isError"], false);
    let content = &res["result"]["structuredContent"];
    assert_eq!(content["projectId"], project.project_id);
    assert_eq!(content["revision"], 0);
    assert_eq!(content["tracksCount"], 1);

    // AC-10: Inspect error on non-existent file
    let bad_args = serde_json::json!({
        "projectPath": dir.join("non_existent.tcproj").to_str().unwrap()
    });
    let bad_res = client.call_tool("tinixcut_inspect", bad_args).await;
    assert_eq!(bad_res["result"]["isError"], true);
    assert_eq!(
        bad_res["result"]["structuredContent"]["code"],
        "PROJECT_LOAD_FAILED"
    );
}

#[tokio::test]
async fn test_phase39_mcp_project_bootstrap_and_read_only_media_probe() {
    let dir = test_dir("phase39_project_bootstrap_probe");
    let project_path = dir.join("created_via_mcp.tcproj");
    let media_path = dir.join("probe_media.mp4");
    generate_synthetic_mp4(&media_path, 2, "blue");
    assert!(media_path.is_file(), "Synthetic media fixture must exist");

    let mut client = DuplexMcpClient::new(TinixCutMcpServer::new()).await;
    let create = client
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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
    let mut client = DuplexMcpClient::new(TinixCutMcpServer::new()).await;

    let variant_result = client
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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
    let mut client = DuplexMcpClient::new(TinixCutMcpServer::new()).await;

    let variant = client
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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
    let mut client = DuplexMcpClient::new(TinixCutMcpServer::new()).await;

    let branched = client
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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
    let mut client = DuplexMcpClient::new(TinixCutMcpServer::new()).await;

    let branched = client
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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
async fn test_p42_reorder_missing_media_fails_without_project_mutation_and_mcp_recovers() {
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

    let mut client = DuplexMcpClient::new(TinixCutMcpServer::new()).await;
    let rejected = client
        .call_tool(
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

    let discovery = client
        .send_request("tools/list", serde_json::json!({}))
        .await;
    assert!(
        discovery.get("error").is_none(),
        "MCP discovery failed after the planner error: {discovery:?}"
    );
    assert!(
        discovery["result"]["tools"].as_array().is_some(),
        "MCP returned no tools after the planner error: {discovery:?}"
    );
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
    let mut client = DuplexMcpClient::new(TinixCutMcpServer::new()).await;

    let variant = client
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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
    let mut client = DuplexMcpClient::new(TinixCutMcpServer::new()).await;

    let preview = client
        .call_tool(
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
        .call_tool(
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
    let mut client = DuplexMcpClient::new(TinixCutMcpServer::new()).await;

    let preview = client
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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
    let mut client = DuplexMcpClient::new(TinixCutMcpServer::new()).await;

    let preview = client
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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
async fn test_p45_mcp_scene_edit_plan_is_revision_bound_and_read_only() {
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
    let mut client = DuplexMcpClient::new(TinixCutMcpServer::new()).await;
    let invalid_constraints = client
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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
async fn test_p57_mcp_export_preflight_is_read_only_and_reports_replacement() {
    let dir = test_dir("p57_export_preflight");
    let (project_path, project) = create_sample_project(&dir, "preflight_project");
    let output_path = dir.join("existing-output.mp4");
    fs::write(
        &output_path,
        b"preserve this output until a render succeeds",
    )
    .unwrap();
    let mut client = DuplexMcpClient::new(TinixCutMcpServer::new()).await;

    let report = client
        .call_tool(
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
        .call_tool(
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
async fn test_p57_mcp_export_preflight_rejects_offline_media_without_mutation() {
    let dir = test_dir("p57_export_preflight_offline_media");
    let (project_path, project, media_path) =
        create_sample_project_with_media(&dir, "offline_preflight_project", 1);
    fs::remove_file(&media_path).expect("fixture media should be removable for offline test");
    let output_path = dir.join("must-not-be-created.mp4");
    let mut client = DuplexMcpClient::new(TinixCutMcpServer::new()).await;

    let report = client
        .call_tool(
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
        .call_tool(
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
async fn test_p56_mcp_workflow_checkpoints_cover_apply_save_and_export() {
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
    let mut client = DuplexMcpClient::new(TinixCutMcpServer::new()).await;

    let created = client
        .call_tool(
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
        .call_tool(
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
            .call_tool(
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
        .call_tool(
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
        .call_tool(
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
#[ignore = "requires an explicitly started LM Studio local API with google/gemma-4-e4b loaded"]
async fn test_p46_mcp_local_scene_selection_suggestion_is_read_only() {
    let dir = test_dir("p46_scene_selection");
    let (project_path, project, _media_path) =
        create_sample_project_with_media(&dir, "p46_scene_project", 30);
    let asset = project.assets.first().unwrap();
    let source_fingerprint =
        tinixcut_lib::modules::media::visual_scene::aggregate_source_fingerprint(&[
            tinixcut_lib::modules::media::visual_scene::VisualSceneSource {
                asset_id: asset.asset_id.clone(),
                asset_name: asset.name.clone(),
                path: PathBuf::from(&asset.path),
            },
        ])
        .unwrap();
    let report = serde_json::json!({
        "schemaVersion": tinixcut_lib::modules::media::visual_scene::VISUAL_SCENE_SCHEMA_VERSION,
        "algorithmVersion": tinixcut_lib::modules::media::visual_scene::VISUAL_SCENE_ALGORITHM_VERSION,
        "provider": "lm-studio-local",
        "model": "google/gemma-4-e4b",
        "query": "short travel scenes",
        "sourceFingerprint": source_fingerprint,
        "configFingerprint": "0".repeat(64),
        "truncated": false,
        "scenes": [
            {"sceneId":"scene-01","assetId":asset.asset_id,"assetName":asset.name,"sourceStartMicros":0,"sourceEndMicros":5000000,"sampledAtMicros":2500000,"caption":"People walking along the river at dawn","tags":["river","people"],"relevanceScore":0.8,"rationale":"Representative frame"},
            {"sceneId":"scene-02","assetId":asset.asset_id,"assetName":asset.name,"sourceStartMicros":10000000,"sourceEndMicros":15000000,"sampledAtMicros":12500000,"caption":"A lantern-lit street with wet pavement","tags":["street","lantern"],"relevanceScore":0.9,"rationale":"Representative frame"}
        ],
        "warnings": ["Model captions need human review."]
    });
    let mut client = DuplexMcpClient::new(TinixCutMcpServer::new()).await;
    let suggestion = client
        .call_tool(
            "tinixcut_edit_plan_scene_selection_suggest",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy(),
                "goal": "Create a calm 5-second travel montage that ends on the lantern street.",
                "report": report,
                "targetDurationSeconds": 5,
                "localModel": "google/gemma-4-e4b"
            }),
        )
        .await;
    assert_eq!(suggestion["result"]["isError"], false, "{suggestion}");
    let content = &suggestion["result"]["structuredContent"];
    assert_eq!(content["provider"], "lm-studio-local");
    assert_eq!(content["model"], "google/gemma-4-e4b");
    let ids = content["orderedSceneIds"].as_array().unwrap();
    assert!(!ids.is_empty());
    assert!(ids.iter().all(|id| id == "scene-01" || id == "scene-02"));
    assert_eq!(content["targetDurationSeconds"], 5);
    let unchanged = load_project(&project_path).unwrap();
    assert_eq!(unchanged.revision, project.revision);
    assert_eq!(unchanged.tracks, project.tracks);
    assert_eq!(unchanged.assets, project.assets);
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn test_p15_ac11_capabilities_report() {
    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;

    let res = client
        .call_tool("tinixcut_capabilities", serde_json::json!({}))
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

    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;

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

    let res = client.call_tool("tinixcut_dry_run", args).await;
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

    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;

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

    let res = client.call_tool("tinixcut_apply", args).await;
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
async fn test_p18_audio_crossfade_mcp_uses_command_service_and_idempotency() {
    let dir = test_dir("p18_audio_crossfade_mcp");
    let project_path = dir.join("audio_crossfade.tcproj");
    let rate = FrameRate::FPS_30;
    let mut project = Project::new("Phase 18 MCP".to_string());
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

    let mut client = DuplexMcpClient::new(TinixCutMcpServer::new()).await;
    let create_envelope = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: 0,
        client_id: Some("p18-agent".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(1),
        dry_run: false,
        transaction_id: Some("tx_p18_mcp_create".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op_p18_mcp_create".to_string(),
            operation: CommandOperation::AudioCrossfadeCreate {
                transition_id: Some("audio_xfade_mcp".to_string()),
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
        .call_tool("tinixcut_apply", create_args.clone())
        .await;
    assert_eq!(created["result"]["isError"], false, "{created:?}");
    let created_receipt: CommandReceipt =
        serde_json::from_value(created["result"]["structuredContent"].clone()).unwrap();
    assert_eq!(created_receipt.prior_revision, 0);
    assert_eq!(created_receipt.new_revision, 1);

    let replayed = client.call_tool("tinixcut_apply", create_args).await;
    assert_eq!(replayed["result"]["isError"], false, "{replayed:?}");
    let replay_receipt: CommandReceipt =
        serde_json::from_value(replayed["result"]["structuredContent"].clone()).unwrap();
    assert_eq!(replay_receipt.new_revision, 1);
    assert_eq!(replay_receipt.receipts[0].status, "deduplicated_noop");

    for (sequence_number, base_revision, operation_id, transaction_id, operation) in [
        (
            2,
            1,
            "op_p18_mcp_update",
            "tx_p18_mcp_update",
            CommandOperation::AudioCrossfadeUpdateDuration {
                transition_id: "audio_xfade_mcp".to_string(),
                new_duration: RationalTime::new(12, rate),
            },
        ),
        (
            3,
            2,
            "op_p18_mcp_remove",
            "tx_p18_mcp_remove",
            CommandOperation::AudioCrossfadeRemove {
                transition_id: "audio_xfade_mcp".to_string(),
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
            .call_tool(
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

    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;

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

    let res = client.call_tool("tinixcut_apply", args).await;

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

    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;

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
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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

    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;

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
        .call_tool(
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
        .call_tool(
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
    let col_res = client.call_tool("tinixcut_apply", serde_json::json!({ "projectPath": proj_path.to_str().unwrap(), "envelope": collision_env })).await;
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
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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
    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;

    // AC-22: Protocol error on unknown method
    let res = client
        .send_request("non_existent_method", serde_json::json!({}))
        .await;
    assert!(res.get("error").is_some());
    assert_eq!(res["error"]["code"], -32601);

    // AC-23: Domain error preserves structured format and server remains operational
    let dir = test_dir("err_boundary");
    let (proj_path, _) = create_sample_project(&dir, "err_proj");

    let bad_apply = client
        .call_tool(
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
        .call_tool("tinixcut_capabilities", serde_json::json!({}))
        .await;
    assert_eq!(alive["result"]["isError"], false);
}

#[tokio::test]
async fn test_p15_ac24_authoritative_write_gate_concurrency() {
    let dir = test_dir("concurrency");
    let (proj_path, project) = create_sample_project(&dir, "conc_proj");

    let server = TinixCutMcpServer::new();

    // Spawn 5 concurrent apply requests against the same project file
    let mut handles = Vec::new();
    for i in 1..=5 {
        let s = server.clone();
        let path_str = proj_path.to_str().unwrap().to_string();
        let pid = project.project_id.clone();
        let handle = tokio::spawn(async move {
            let mut c = DuplexMcpClient::new(s).await;
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
            c.call_tool(
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

    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;

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
        .call_tool(
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

    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;

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
        .call_tool(
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

    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;

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
        .call_tool(
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

    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;

    // AC-28: Save to targetPath
    let res = client
        .call_tool(
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
        .call_tool(
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

    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;

    // 1. AC-30: export_start returns jobId and status
    let res = client
        .call_tool(
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
        .call_tool(
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
        .call_tool(
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

    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;
    let start_res = client
        .call_tool(
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
        .call_tool(
            "tinixcut_export_cancel",
            serde_json::json!({ "jobId": job_id }),
        )
        .await;
    assert_eq!(cancel_res["result"]["isError"], false);
    assert_eq!(cancel_res["result"]["structuredContent"]["cancelled"], true);

    let status_res = client
        .call_tool(
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

    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;

    // 1. Start export at revision 0
    let res = client
        .call_tool(
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
        .call_tool(
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
            .call_tool(
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
    let server = TinixCutMcpServer::new();
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
    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;

    let res = client
        .send_request("tools/list", serde_json::json!({}))
        .await;
    let tools = res["result"]["tools"].as_array().unwrap();

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

    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;

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
        .call_tool(
            "tinixcut_apply",
            serde_json::json!({
                "projectPath": proj_path.to_str().unwrap(),
                "envelope": env
            }),
        )
        .await;

    let raw_text = fs::read_to_string(&proj_path).unwrap();
    let raw_json: serde_json::Value = serde_json::from_str(&raw_text).unwrap();

    // MCP writes must preserve the current project schema without adding MCP metadata.
    assert_eq!(raw_json["schema_version"], CURRENT_SCHEMA_VERSION);
    // Verify zero MCP-specific metadata fields in project root
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
        let server1 = TinixCutMcpServer::new();
        let mut client1 = DuplexMcpClient::new(server1).await;
        let res = client1
            .call_tool(
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
        let server2 = TinixCutMcpServer::new();
        let mut client2 = DuplexMcpClient::new(server2).await;
        // Re-submit identical mutation to Server 2
        let replay_res = client2
            .call_tool(
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

    // Resend exact same logical structured operation identity/payload via MCP
    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;

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
        .call_tool(
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

#[test]
fn test_p15_ac01_ac02_ac32_ac33_ac34_ac40_real_subprocess_e2e() {
    let bin = env!("CARGO_BIN_EXE_tinixcut");
    let dir = test_dir("subproc_e2e");
    let (proj_path, project) = create_sample_project(&dir, "subproc_proj");

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

    // 1. server/discover via real subprocess (verifying 2026-07-28 and capabilities)
    let req_disc = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "server/discover",
        "params": {
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientCapabilities": {},
                "io.modelcontextprotocol/clientInfo": {
                    "name": "subproc-agent",
                    "version": "1.0.0"
                }
            }
        }
    });
    stdin
        .write_all(format!("{}\n", serde_json::to_string(&req_disc).unwrap()).as_bytes())
        .unwrap();
    stdin.flush().unwrap();

    let mut line_disc = String::new();
    reader.read_line(&mut line_disc).unwrap();
    let val_disc: serde_json::Value =
        serde_json::from_str(&line_disc).expect("Subprocess stdout must be 100% valid JSON-RPC");
    assert_eq!(val_disc["id"], 1);
    let disc_res = &val_disc["result"];
    let versions = disc_res["supportedVersions"]
        .as_array()
        .expect("supportedVersions");
    assert!(
        versions.iter().any(|v| v.as_str() == Some("2026-07-28")),
        "Real subprocess server/discover must advertise 2026-07-28: {:?}",
        versions
    );
    assert!(
        disc_res["capabilities"]["tools"].is_object(),
        "Must advertise tools capability"
    );

    // 2. tools/list via real subprocess (modern path without initialize!)
    let req1 = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientCapabilities": {}
            }
        }
    });
    stdin
        .write_all(format!("{}\n", serde_json::to_string(&req1).unwrap()).as_bytes())
        .unwrap();
    stdin.flush().unwrap();

    let mut line1 = String::new();
    reader.read_line(&mut line1).unwrap();
    // AC-32: Stdout Protocol Purity (MUST parse as valid JSON-RPC)
    let val1: serde_json::Value =
        serde_json::from_str(&line1).expect("Subprocess stdout must be 100% valid JSON-RPC");
    assert_eq!(val1["id"], 2);
    let tools = val1["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 35);

    // 3. tinixcut_inspect via real subprocess
    let req2 = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientCapabilities": {}
            },
            "name": "tinixcut_inspect",
            "arguments": {
                "projectPath": proj_path.to_str().unwrap()
            }
        }
    });
    stdin
        .write_all(format!("{}\n", serde_json::to_string(&req2).unwrap()).as_bytes())
        .unwrap();
    stdin.flush().unwrap();

    let mut line2 = String::new();
    reader.read_line(&mut line2).unwrap();
    let val2: serde_json::Value = serde_json::from_str(&line2).unwrap();
    assert_eq!(val2["id"], 3);
    assert_eq!(
        val2["result"]["structuredContent"]["projectId"],
        project.project_id
    );

    let artifact_path = dir.join("stdio-export.mp4");
    let manifest_path = dir.join("stdio-workflow.json");
    let plan = tinixcut_lib::modules::automation::edit_plan::EditPlan {
        schema_version: 1,
        goal: "Create one video track through MCP stdio".to_string(),
        project_id: project.project_id.clone(),
        base_revision: project.revision,
        referenced_media_ids: Vec::new(),
        assumptions: Vec::new(),
        warnings: Vec::new(),
        operations: vec![OperationEnvelope {
            operation_id: "op-sub-1".to_string(),
            operation: CommandOperation::TrackCreate {
                track_id: Some("track_v2".to_string()),
                name: "Track 2 Subprocess".to_string(),
                kind: TrackKind::Video,
            },
        }],
    };
    let plan_json = serde_json::to_value(&plan).unwrap();
    let project_path = proj_path.to_string_lossy().to_string();
    let manifest_path_string = manifest_path.to_string_lossy().to_string();
    let artifact_path_string = artifact_path.to_string_lossy().to_string();
    let manifest = stdio_call_tool(
        &mut stdin,
        &mut reader,
        4,
        "tinixcut_workflow_manifest_create",
        serde_json::json!({
            "projectPath": project_path,
            "manifestPath": manifest_path_string,
            "plan": plan_json,
            "artifactPaths": [artifact_path_string]
        }),
    );
    assert_eq!(manifest["result"]["isError"], false, "{manifest}");

    // 4. tinixcut_apply via real subprocess
    let env = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: project.project_id.clone(),
        base_revision: 0,
        client_id: Some("agent-subproc".to_string()),
        client_epoch: Some(1),
        sequence_number: Some(1),
        dry_run: false,
        transaction_id: Some("tx-sub-1".to_string()),
        operations: vec![OperationEnvelope {
            operation_id: "op-sub-1".to_string(),
            operation: CommandOperation::TrackCreate {
                track_id: Some("track_v2".to_string()),
                name: "Track 2 Subprocess".to_string(),
                kind: TrackKind::Video,
            },
        }],
    };
    let req3 = serde_json::json!({
        "jsonrpc": "2.0",
            "id": 5,
        "method": "tools/call",
        "params": {
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientCapabilities": {}
            },
            "name": "tinixcut_apply",
            "arguments": {
                "projectPath": proj_path.to_str().unwrap(),
                "envelope": env
            }
        }
    });
    stdin
        .write_all(format!("{}\n", serde_json::to_string(&req3).unwrap()).as_bytes())
        .unwrap();
    stdin.flush().unwrap();

    let mut line3 = String::new();
    reader.read_line(&mut line3).unwrap();
    let val3: serde_json::Value = serde_json::from_str(&line3).unwrap();
    assert_eq!(val3["id"], 5);
    assert_eq!(val3["result"]["isError"], false);
    assert_eq!(val3["result"]["structuredContent"]["newRevision"], 1);

    for (id, state) in [(6, "applied"), (7, "saved")] {
        let checkpoint = stdio_call_tool(
            &mut stdin,
            &mut reader,
            id,
            "tinixcut_workflow_manifest_checkpoint",
            serde_json::json!({
                "projectPath": project_path,
                "manifestPath": manifest_path_string,
                "plan": plan_json,
                "state": state
            }),
        );
        assert_eq!(checkpoint["result"]["isError"], false, "{checkpoint}");
        assert_eq!(
            checkpoint["result"]["structuredContent"]["checkpoint"]["state"],
            state
        );
    }
    generate_synthetic_mp4(&artifact_path, 1, "blue");
    let exported = stdio_call_tool(
        &mut stdin,
        &mut reader,
        8,
        "tinixcut_workflow_manifest_checkpoint",
        serde_json::json!({
            "projectPath": project_path,
            "manifestPath": manifest_path_string,
            "plan": plan_json,
            "state": "exported",
            "artifactPath": artifact_path_string
        }),
    );
    assert_eq!(exported["result"]["isError"], false, "{exported}");
    assert_eq!(
        exported["result"]["structuredContent"]["checkpoint"]["state"],
        "exported"
    );

    // AC-34: Graceful exit on stdin EOF
    drop(stdin);
    let status = child.wait().expect("Child must exit on EOF");
    assert!(status.success(), "Child must exit with exit status 0");

    // Verify disk persistence after subprocess exit
    let reloaded = load_project(&proj_path).unwrap();
    assert_eq!(reloaded.revision, 1);
    assert_eq!(reloaded.tracks.len(), 2);

    // Restart a fresh MCP process and retry the same exported checkpoint. The durable sidecar
    // must return the original receipt without adding a second operation or replacing evidence.
    let mut restarted = Command::new(bin)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Restarted MCP process must start");
    let mut restarted_stdin = restarted.stdin.take().unwrap();
    let mut restarted_reader = BufReader::new(restarted.stdout.take().unwrap());
    let retry = stdio_call_tool(
        &mut restarted_stdin,
        &mut restarted_reader,
        1,
        "tinixcut_workflow_manifest_checkpoint",
        serde_json::json!({
            "projectPath": project_path,
            "manifestPath": manifest_path_string,
            "plan": plan_json,
            "state": "exported",
            "artifactPath": artifact_path_string
        }),
    );
    assert_eq!(retry["result"]["isError"], false, "{retry}");
    assert_eq!(
        retry["result"]["structuredContent"]["alreadyRecorded"],
        true
    );
    drop(restarted_stdin);
    assert!(restarted.wait().unwrap().success());
    let restarted_project = load_project(&proj_path).unwrap();
    assert_eq!(restarted_project.revision, 1);
    assert_eq!(restarted_project.tracks.len(), 2);
}

#[test]
fn test_p56_real_stdio_three_project_batch_isolates_collision_and_continues() {
    let bin = env!("CARGO_BIN_EXE_tinixcut");
    // This test is also useful when more than one Cargo invocation is active, so its fixture
    // directory must be unique across processes (the shared test_dir counter is process-local).
    let dir = std::env::temp_dir().join(format!(
        "tinixcut_p56_three_project_batch_{}_{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    fs::create_dir_all(&dir).expect("batch fixture directory must be created");
    let mut projects = Vec::new();
    for index in 0..3 {
        let name = format!("batch_project_{index}");
        let (project_path, project, _) = create_sample_project_with_media(&dir, &name, 1);
        projects.push((project_path, project));
    }

    // Project 1 has a pre-existing output. The batch client must skip only this item and
    // preserve the existing artifact while continuing with projects 0 and 2.
    let outputs = (0..3)
        .map(|index| dir.join(format!("batch_output_{index}.mp4")))
        .collect::<Vec<_>>();
    let preserved_collision = b"prior user output; batch must not replace it";
    fs::write(&outputs[1], preserved_collision).unwrap();
    let collision_bytes_before = fs::read(&outputs[1]).unwrap();

    let mut child = Command::new(bin)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("MCP stdio process must start");
    let mut stdin = child.stdin.take().unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());

    // One invalid item must return a structured error without terminating the process or
    // preventing later items from being processed.
    let invalid = stdio_call_tool(
        &mut stdin,
        &mut reader,
        1,
        "tinixcut_inspect",
        serde_json::json!({ "projectPath": dir.join("missing.tcproj") }),
    );
    assert_eq!(invalid["result"]["isError"], true, "{invalid}");

    let mut exported = 0;
    let mut collision_isolated = false;
    for (index, (project_path, project)) in projects.iter().enumerate() {
        let output_path = &outputs[index];
        let report = stdio_call_tool(
            &mut stdin,
            &mut reader,
            10 + (index as u64 * 10),
            "tinixcut_export_preflight",
            serde_json::json!({
                "projectPath": project_path,
                "outputPath": output_path,
            }),
        );
        assert_eq!(report["result"]["isError"], false, "{report}");
        let output_exists = report["result"]["structuredContent"]["outputExists"]
            .as_bool()
            .unwrap_or(false);

        if index == 1 {
            assert!(
                output_exists,
                "preflight must report the existing destination: {report}"
            );
            assert_eq!(fs::read(output_path).unwrap(), collision_bytes_before);
            collision_isolated = true;
            continue;
        }
        assert!(
            !output_exists,
            "each successful batch item has a unique output"
        );

        let manifest_path = dir.join(format!("batch_manifest_{index}.json"));
        let operation_id = format!("batch_track_{index}");
        let plan = tinixcut_lib::modules::automation::edit_plan::EditPlan {
            schema_version: 1,
            goal: format!("Batch acceptance project {index}"),
            project_id: project.project_id.clone(),
            base_revision: project.revision,
            referenced_media_ids: Vec::new(),
            assumptions: Vec::new(),
            warnings: Vec::new(),
            operations: vec![OperationEnvelope {
                operation_id: operation_id.clone(),
                operation: CommandOperation::TrackCreate {
                    track_id: Some(format!("batch_extra_track_{index}")),
                    name: format!("Batch Track {index}"),
                    kind: TrackKind::Video,
                },
            }],
        };
        let plan_json = serde_json::to_value(&plan).unwrap();
        let manifest = stdio_call_tool(
            &mut stdin,
            &mut reader,
            11 + (index as u64 * 10),
            "tinixcut_workflow_manifest_create",
            serde_json::json!({
                "projectPath": project_path,
                "manifestPath": manifest_path,
                "plan": plan_json,
                "artifactPaths": [output_path],
            }),
        );
        assert_eq!(manifest["result"]["isError"], false, "{manifest}");

        let apply = stdio_call_tool(
            &mut stdin,
            &mut reader,
            12 + (index as u64 * 10),
            "tinixcut_apply",
            serde_json::json!({
                "projectPath": project_path,
                "envelope": CommandEnvelope {
                    schema_version: CURRENT_SCHEMA_VERSION,
                    project_id: project.project_id.clone(),
                    base_revision: project.revision,
                    client_id: Some(format!("batch_client_{index}")),
                    client_epoch: Some(1),
                    sequence_number: Some(1),
                    dry_run: false,
                    transaction_id: Some(format!("batch_tx_{index}")),
                    operations: plan.operations.clone(),
                },
            }),
        );
        assert_eq!(apply["result"]["isError"], false, "{apply}");
        let applied = stdio_call_tool(
            &mut stdin,
            &mut reader,
            13 + (index as u64 * 10),
            "tinixcut_workflow_manifest_checkpoint",
            serde_json::json!({
                "projectPath": project_path,
                "manifestPath": manifest_path,
                "plan": plan_json,
                "state": "applied",
            }),
        );
        assert_eq!(applied["result"]["isError"], false, "{applied}");

        let save = stdio_call_tool(
            &mut stdin,
            &mut reader,
            14 + (index as u64 * 10),
            "tinixcut_save",
            serde_json::json!({ "projectPath": project_path }),
        );
        assert_eq!(save["result"]["isError"], false, "{save}");
        let saved = stdio_call_tool(
            &mut stdin,
            &mut reader,
            15 + (index as u64 * 10),
            "tinixcut_workflow_manifest_checkpoint",
            serde_json::json!({
                "projectPath": project_path,
                "manifestPath": manifest_path,
                "plan": plan_json,
                "state": "saved",
            }),
        );
        assert_eq!(saved["result"]["isError"], false, "{saved}");

        let start = stdio_call_tool(
            &mut stdin,
            &mut reader,
            16 + (index as u64 * 10),
            "tinixcut_export_start",
            serde_json::json!({ "projectPath": project_path, "outputPath": output_path }),
        );
        assert_eq!(start["result"]["isError"], false, "{start}");
        let job_id = start["result"]["structuredContent"]["jobId"]
            .as_str()
            .expect("export job id")
            .to_string();
        let mut completed = false;
        for poll in 0..120 {
            let status = stdio_call_tool(
                &mut stdin,
                &mut reader,
                17 + (index as u64 * 200) + poll,
                "tinixcut_export_status",
                serde_json::json!({ "jobId": job_id }),
            );
            assert_eq!(status["result"]["isError"], false, "{status}");
            match status["result"]["structuredContent"]["status"].as_str() {
                Some("completed") => {
                    completed = true;
                    break;
                }
                Some("failed") => panic!("batch export failed: {status}"),
                _ => std::thread::sleep(std::time::Duration::from_millis(100)),
            }
        }
        assert!(completed, "batch export did not finish for project {index}");
        let checkpoint = stdio_call_tool(
            &mut stdin,
            &mut reader,
            18 + (index as u64 * 200),
            "tinixcut_workflow_manifest_checkpoint",
            serde_json::json!({
                "projectPath": project_path,
                "manifestPath": manifest_path,
                "plan": plan_json,
                "state": "exported",
                "artifactPath": output_path,
            }),
        );
        assert_eq!(checkpoint["result"]["isError"], false, "{checkpoint}");
        assert!(output_path.exists());
        let persisted = load_project(project_path).unwrap();
        assert_eq!(persisted.revision, 1);
        assert_eq!(persisted.tracks.len(), project.tracks.len() + 1);
        exported += 1;
    }

    assert_eq!(exported, 2);
    assert!(collision_isolated);
    assert_eq!(fs::read(&outputs[1]).unwrap(), collision_bytes_before);
    drop(stdin);
    assert!(child.wait().unwrap().success());
}

#[test]
fn test_p56_real_stdio_batch_cancel_isolated_and_next_project_exports() {
    let bin = env!("CARGO_BIN_EXE_tinixcut");
    let dir = std::env::temp_dir().join(format!(
        "tinixcut_p56_cancel_batch_{}_{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    fs::create_dir_all(&dir).expect("batch cancellation fixture directory must be created");
    let (cancelled_project_path, cancelled_project, _) =
        create_sample_project_with_media(&dir, "cancelled_batch_item", 60);
    let (next_project_path, next_project, _) =
        create_sample_project_with_media(&dir, "next_batch_item", 1);
    let cancelled_output = dir.join("cancelled_output.mp4");
    let next_output = dir.join("next_output.mp4");

    let mut child = Command::new(bin)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("MCP stdio process must start");
    let mut stdin = child.stdin.take().unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());

    let start_cancelled = stdio_call_tool(
        &mut stdin,
        &mut reader,
        1,
        "tinixcut_export_start",
        serde_json::json!({
            "projectPath": cancelled_project_path,
            "outputPath": cancelled_output,
        }),
    );
    assert_eq!(
        start_cancelled["result"]["isError"], false,
        "{start_cancelled}"
    );
    let cancelled_job = start_cancelled["result"]["structuredContent"]["jobId"]
        .as_str()
        .expect("first batch export must return a job ID")
        .to_string();

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
        "first batch export must reach active rendering before cancellation"
    );

    let cancel = stdio_call_tool(
        &mut stdin,
        &mut reader,
        2,
        "tinixcut_export_cancel",
        serde_json::json!({ "jobId": cancelled_job }),
    );
    assert_eq!(cancel["result"]["isError"], false, "{cancel}");
    assert_eq!(cancel["result"]["structuredContent"]["cancelled"], true);
    let cancelled_status = stdio_call_tool(
        &mut stdin,
        &mut reader,
        3,
        "tinixcut_export_status",
        serde_json::json!({ "jobId": cancelled_job }),
    );
    assert_eq!(
        cancelled_status["result"]["structuredContent"]["status"],
        "cancelled"
    );

    let cleanup_completed = (0..250).any(|_| {
        let has_temp = fs::read_dir(&dir).unwrap().flatten().any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .contains(".tc_render.tmp.mp4")
        });
        if has_temp {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        !has_temp
    });
    assert!(
        cleanup_completed,
        "cancelled batch item must clean its temporary render"
    );
    assert!(
        !cancelled_output.exists(),
        "cancelled batch item must not promote output"
    );

    // The same MCP subprocess remains usable: process the next independent batch item.
    let next_start = stdio_call_tool(
        &mut stdin,
        &mut reader,
        4,
        "tinixcut_export_start",
        serde_json::json!({
            "projectPath": next_project_path,
            "outputPath": next_output,
        }),
    );
    assert_eq!(next_start["result"]["isError"], false, "{next_start}");
    let next_job = next_start["result"]["structuredContent"]["jobId"]
        .as_str()
        .expect("next batch export must return a job ID")
        .to_string();
    let mut next_completed = false;
    for poll in 0..120 {
        let status = stdio_call_tool(
            &mut stdin,
            &mut reader,
            5 + poll,
            "tinixcut_export_status",
            serde_json::json!({ "jobId": next_job }),
        );
        assert_eq!(status["result"]["isError"], false, "{status}");
        match status["result"]["structuredContent"]["status"].as_str() {
            Some("completed") => {
                next_completed = true;
                break;
            }
            Some("failed") | Some("cancelled") => panic!("next batch export failed: {status}"),
            _ => std::thread::sleep(std::time::Duration::from_millis(100)),
        }
    }
    assert!(
        next_completed,
        "next independent batch export did not complete"
    );
    assert!(next_output.is_file());

    let mut ffmpeg = tinixcut_lib::modules::media::resolver::MediaBinaryResolver::command("ffmpeg")
        .expect("bundled FFmpeg must resolve");
    let decode = ffmpeg
        .args(["-v", "error", "-i"])
        .arg(&next_output)
        .args(["-f", "null", "-"])
        .output()
        .expect("next batch output must full-decode");
    assert!(
        decode.status.success(),
        "next batch output failed FFmpeg decode: {}",
        String::from_utf8_lossy(&decode.stderr)
    );
    assert_eq!(
        load_project(&cancelled_project_path).unwrap(),
        cancelled_project
    );
    assert_eq!(load_project(&next_project_path).unwrap(), next_project);

    drop(stdin);
    assert!(child.wait().unwrap().success());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn test_p56_real_stdio_new_project_stale_revision_recovers_and_exports() {
    let bin = env!("CARGO_BIN_EXE_tinixcut");
    let dir = std::env::temp_dir().join(format!(
        "tinixcut_p56_new_project_{}_{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    fs::create_dir_all(&dir).expect("new-project fixture directory must be created");
    let project_path = dir.join("new_project.tcproj");
    let media_path = dir.join("new_project_media.mp4");
    let output_path = dir.join("new_project_export.mp4");
    let manifest_path = dir.join("new_project_workflow.json");
    generate_synthetic_mp4(&media_path, 1, "blue");
    assert!(media_path.is_file(), "disposable source media must exist");
    let media_size = fs::metadata(&media_path).unwrap().len();

    let mut child = Command::new(bin)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("MCP stdio process must start");
    let mut stdin = child.stdin.take().unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());

    let created = stdio_call_tool(
        &mut stdin,
        &mut reader,
        1,
        "tinixcut_project_create",
        serde_json::json!({ "projectPath": project_path, "name": "New project batch fixture" }),
    );
    assert_eq!(created["result"]["isError"], false, "{created}");
    let initial = load_project(&project_path).expect("MCP-created project must load");
    assert_eq!(initial.revision, 0);

    let import_operation = OperationEnvelope {
        operation_id: "new_project_asset_import".into(),
        operation: CommandOperation::AssetImport {
            asset_id: Some("new_project_asset".into()),
            name: "new_project_media.mp4".into(),
            path: media_path.to_string_lossy().into_owned(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(30, FrameRate::FPS_30)),
            file_size: media_size,
            dimensions: None,
        },
    };
    let imported = stdio_call_tool(
        &mut stdin,
        &mut reader,
        2,
        "tinixcut_apply",
        serde_json::json!({
            "projectPath": project_path,
            "envelope": CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: initial.project_id.clone(),
                base_revision: 0,
                client_id: Some("new-project-import-client".into()),
                client_epoch: Some(1),
                sequence_number: Some(1),
                dry_run: false,
                transaction_id: Some("new-project-import-transaction".into()),
                operations: vec![import_operation],
            },
        }),
    );
    assert_eq!(imported["result"]["isError"], false, "{imported}");
    let after_import = load_project(&project_path).expect("media import must persist");
    assert_eq!(after_import.revision, 1);
    assert_eq!(after_import.assets.len(), 1);

    // A client holding the pre-import revision must fail without mutating the project.
    let stale = stdio_call_tool(
        &mut stdin,
        &mut reader,
        3,
        "tinixcut_apply",
        serde_json::json!({
            "projectPath": project_path,
            "envelope": CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: after_import.project_id.clone(),
                base_revision: 0,
                client_id: Some("new-project-stale-client".into()),
                client_epoch: Some(1),
                sequence_number: Some(1),
                dry_run: false,
                transaction_id: Some("new-project-stale-transaction".into()),
                operations: vec![OperationEnvelope {
                    operation_id: "stale_track_operation".into(),
                    operation: CommandOperation::TrackCreate {
                        track_id: Some("must_not_exist".into()),
                        name: "Stale track".into(),
                        kind: TrackKind::Video,
                    },
                }],
            },
        }),
    );
    assert_eq!(stale["result"]["isError"], true, "{stale}");
    assert_eq!(
        stale["result"]["structuredContent"]["code"],
        "STALE_REVISION"
    );
    let unchanged = load_project(&project_path).unwrap();
    assert_eq!(unchanged.revision, 1);
    assert!(unchanged.tracks.is_empty());

    let plan = tinixcut_lib::modules::automation::edit_plan::EditPlan {
        schema_version: 1,
        goal: "Create a one-second video from a new MCP project".into(),
        project_id: unchanged.project_id.clone(),
        base_revision: unchanged.revision,
        referenced_media_ids: vec!["new_project_asset".into()],
        assumptions: Vec::new(),
        warnings: Vec::new(),
        operations: vec![
            OperationEnvelope {
                operation_id: "new_project_video_track".into(),
                operation: CommandOperation::TrackCreate {
                    track_id: Some("new_project_video_track".into()),
                    name: "Video".into(),
                    kind: TrackKind::Video,
                },
            },
            OperationEnvelope {
                operation_id: "new_project_video_clip".into(),
                operation: CommandOperation::ClipCreate {
                    track_id: "new_project_video_track".into(),
                    item_id: Some("new_project_video_clip".into()),
                    name: "One-second source".into(),
                    kind: ItemKind::Video,
                    asset_id: Some("new_project_asset".into()),
                    timeline_start: RationalTime::zero(FrameRate::FPS_30),
                    duration: RationalTime::new(30, FrameRate::FPS_30),
                    source_start: Some(RationalTime::zero(FrameRate::FPS_30)),
                },
            },
        ],
    };
    let plan_json = serde_json::to_value(&plan).unwrap();
    let manifest = stdio_call_tool(
        &mut stdin,
        &mut reader,
        4,
        "tinixcut_workflow_manifest_create",
        serde_json::json!({
            "projectPath": project_path,
            "manifestPath": manifest_path,
            "plan": plan_json,
            "artifactPaths": [output_path],
        }),
    );
    assert_eq!(manifest["result"]["isError"], false, "{manifest}");
    let applied = stdio_call_tool(
        &mut stdin,
        &mut reader,
        5,
        "tinixcut_apply",
        serde_json::json!({
            "projectPath": project_path,
            "envelope": CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: unchanged.project_id.clone(),
                base_revision: unchanged.revision,
                client_id: Some("new-project-edit-client".into()),
                client_epoch: Some(1),
                sequence_number: Some(1),
                dry_run: false,
                transaction_id: Some("new-project-edit-transaction".into()),
                operations: plan.operations.clone(),
            },
        }),
    );
    assert_eq!(applied["result"]["isError"], false, "{applied}");
    assert_eq!(load_project(&project_path).unwrap().revision, 2);
    let applied_checkpoint = stdio_call_tool(
        &mut stdin,
        &mut reader,
        6,
        "tinixcut_workflow_manifest_checkpoint",
        serde_json::json!({
            "projectPath": project_path,
            "manifestPath": manifest_path,
            "plan": plan_json,
            "state": "applied",
        }),
    );
    assert_eq!(
        applied_checkpoint["result"]["isError"], false,
        "{applied_checkpoint}"
    );

    let saved = stdio_call_tool(
        &mut stdin,
        &mut reader,
        7,
        "tinixcut_save",
        serde_json::json!({ "projectPath": project_path }),
    );
    assert_eq!(saved["result"]["isError"], false, "{saved}");
    let saved_checkpoint = stdio_call_tool(
        &mut stdin,
        &mut reader,
        8,
        "tinixcut_workflow_manifest_checkpoint",
        serde_json::json!({
            "projectPath": project_path,
            "manifestPath": manifest_path,
            "plan": plan_json,
            "state": "saved",
        }),
    );
    assert_eq!(
        saved_checkpoint["result"]["isError"], false,
        "{saved_checkpoint}"
    );

    let start = stdio_call_tool(
        &mut stdin,
        &mut reader,
        9,
        "tinixcut_export_start",
        serde_json::json!({ "projectPath": project_path, "outputPath": output_path }),
    );
    assert_eq!(start["result"]["isError"], false, "{start}");
    let job_id = start["result"]["structuredContent"]["jobId"]
        .as_str()
        .expect("new project export job ID")
        .to_string();
    let mut completed = false;
    for poll in 0..120 {
        let status = stdio_call_tool(
            &mut stdin,
            &mut reader,
            10 + poll,
            "tinixcut_export_status",
            serde_json::json!({ "jobId": job_id }),
        );
        assert_eq!(status["result"]["isError"], false, "{status}");
        match status["result"]["structuredContent"]["status"].as_str() {
            Some("completed") => {
                completed = true;
                break;
            }
            Some("failed") => panic!("new project export failed: {status}"),
            _ => std::thread::sleep(std::time::Duration::from_millis(100)),
        }
    }
    assert!(completed, "new project export must complete");
    let exported_checkpoint = stdio_call_tool(
        &mut stdin,
        &mut reader,
        131,
        "tinixcut_workflow_manifest_checkpoint",
        serde_json::json!({
            "projectPath": project_path,
            "manifestPath": manifest_path,
            "plan": plan_json,
            "state": "exported",
            "artifactPath": output_path,
        }),
    );
    assert_eq!(
        exported_checkpoint["result"]["isError"], false,
        "{exported_checkpoint}"
    );
    assert!(output_path.is_file());
    let mut decode = tinixcut_lib::modules::media::resolver::MediaBinaryResolver::command("ffmpeg")
        .expect("bundled FFmpeg must be available");
    let decoded = decode
        .args(["-v", "error", "-xerror", "-i"])
        .arg(&output_path)
        .args(["-f", "null", "-"])
        .output()
        .expect("full decode process must start");
    assert!(
        decoded.status.success(),
        "export full decode failed: {}",
        String::from_utf8_lossy(&decoded.stderr)
    );
    let persisted = load_project(&project_path).unwrap();
    assert_eq!(persisted.revision, 2);
    assert_eq!(persisted.assets.len(), 1);
    assert_eq!(persisted.tracks.len(), 1);
    assert_eq!(persisted.tracks[0].items.len(), 1);
    drop(stdin);
    assert!(child.wait().unwrap().success());
}

#[test]
fn test_p15_ac35_subprocess_shutdown_orphan_and_temp_cleanup() {
    let bin = env!("CARGO_BIN_EXE_tinixcut");
    let dir = test_dir("shutdown_cleanup_subproc");
    let (proj_path, _, _) = create_sample_project_with_media(&dir, "shutdown_proj", 10);
    let out_file = dir.join("shutdown_render.mp4");

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

    // 1. Discover
    let disc_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "server/discover",
        "params": {
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientCapabilities": {},
                "io.modelcontextprotocol/clientInfo": { "name": "test-agent", "version": "1.0.0" }
            }
        }
    });
    stdin
        .write_all(format!("{}\n", serde_json::to_string(&disc_req).unwrap()).as_bytes())
        .unwrap();
    stdin.flush().unwrap();
    let mut line1 = String::new();
    reader.read_line(&mut line1).unwrap();

    // 2. Start a 10s export
    let exp_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientCapabilities": {}
            },
            "name": "tinixcut_export_start",
            "arguments": {
                "projectPath": proj_path.to_str().unwrap(),
                "outputPath": out_file.to_str().unwrap()
            }
        }
    });
    stdin
        .write_all(format!("{}\n", serde_json::to_string(&exp_req).unwrap()).as_bytes())
        .unwrap();
    stdin.flush().unwrap();
    let mut line2 = String::new();
    reader.read_line(&mut line2).unwrap();
    let exp_val: serde_json::Value = serde_json::from_str(&line2).unwrap();
    assert_eq!(exp_val["id"], 2);
    assert_eq!(exp_val["result"]["isError"], false);

    // Give FFmpeg worker a moment to spawn and create temp render file
    std::thread::sleep(std::time::Duration::from_millis(150));

    // 3. Drop stdin while export is actively running to trigger clean MCP shutdown
    drop(stdin);

    // 4. Subprocess exits cleanly
    let status = child.wait().expect("Subprocess must exit on stdin EOF");
    assert!(status.success(), "Subprocess must exit with status 0");

    // 5. Verify no temporary render artifact files remain in directory
    let entries = fs::read_dir(&dir).unwrap();
    let tmp_files: Vec<_> = entries
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .contains(".tc_render.tmp.mp4")
        })
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert!(
        tmp_files.is_empty(),
        "All temporary render files must be cleaned up on shutdown! Found: {:?}",
        tmp_files
    );

    // 6. Prove directory is not locked by any orphaned FFmpeg process
    let lock_probe = dir.join("probe_lock.tmp");
    fs::write(&lock_probe, b"probe").expect("Directory must remain writable");
    let _ = fs::remove_file(&lock_probe);
}

#[tokio::test]
async fn test_p58a_every_tool_declares_safety_annotations() {
    let mut client = DuplexMcpClient::new(TinixCutMcpServer::new()).await;
    let res = client
        .send_request("tools/list", serde_json::json!({}))
        .await;
    let tools = res["result"]["tools"].as_array().expect("tools array");
    let read_only: std::collections::BTreeSet<&str> = [
        "project_compare",
        "project_chapters",
        "frame_scopes",
        "lut_probe",
        "media_probe",
        "edit_plan_validate",
        "edit_plan_draft",
        "edit_plan_scene_draft",
        "edit_plan_scene_selection_suggest",
        "edit_plan_reorder",
        "edit_plan_trim",
        "edit_plan_volume",
        "edit_plan_canvas",
        "edit_plan_text_replace",
        "media_scene_candidates",
        "media_visual_scenes",
        "media_transcript",
        "inspect",
        "capabilities",
        "dry_run",
        "workflow_manifest_validate",
        "export_preflight",
        "export_status",
        "template_validate",
    ]
    .into_iter()
    .collect();
    let destructive: std::collections::BTreeSet<&str> =
        ["apply", "save", "export_start", "template_apply"]
            .into_iter()
            .collect();
    for tool in tools {
        let name = tool["name"].as_str().unwrap();
        let short = name.trim_start_matches("tinixcut_");
        let a = &tool["annotations"];
        assert!(a.is_object(), "{name} has no annotations");
        assert_eq!(a["openWorldHint"], false, "{name}");
        assert_eq!(
            a["readOnlyHint"],
            read_only.contains(short),
            "{name} readOnlyHint"
        );
        assert_eq!(
            a["destructiveHint"],
            destructive.contains(short),
            "{name} destructiveHint"
        );
        if read_only.contains(short) {
            assert_eq!(
                a["idempotentHint"], true,
                "{name} read-only tools are idempotent"
            );
        }
    }
}

#[tokio::test]
async fn test_p58a_destructive_apply_requires_prior_identical_dry_run_when_gate_is_on() {
    let dir = test_dir("p58a_gate");
    let (project_path, project) = create_sample_project(&dir, "gate_project");
    let server = TinixCutMcpServer::new().with_destructive_confirmation(true);
    let mut client = DuplexMcpClient::new(server).await;
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
        .call_tool("tinixcut_apply", serde_json::json!({
            "projectPath": path, "envelope": envelope(project.revision, 1, "p58a-create", create_ops) }))
        .await;
    assert_eq!(created["result"]["isError"], false, "{created}");

    // Destructive apply without a dry run is refused and changes nothing.
    let refused = client
        .call_tool("tinixcut_apply", serde_json::json!({
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
        .call_tool("tinixcut_dry_run", serde_json::json!({
            "projectPath": path, "envelope": envelope(project.revision + 1, 2, "p58a-other", serde_json::json!([])) }))
        .await;
    assert_eq!(other["result"]["isError"], false, "{other}");
    let still_refused = client
        .call_tool("tinixcut_apply", serde_json::json!({
            "projectPath": path, "envelope": envelope(project.revision + 1, 2, "p58a-del-2", delete_ops.clone()) }))
        .await;
    assert_eq!(still_refused["result"]["isError"], true, "{still_refused}");

    // The identical envelope after its own dry run is accepted, once.
    let dry = client
        .call_tool("tinixcut_dry_run", serde_json::json!({
            "projectPath": path, "envelope": envelope(project.revision + 1, 2, "p58a-del-3", delete_ops.clone()) }))
        .await;
    assert_eq!(dry["result"]["isError"], false, "{dry}");
    let applied = client
        .call_tool("tinixcut_apply", serde_json::json!({
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
    let mut client = DuplexMcpClient::new(TinixCutMcpServer::new()).await;

    let probe = client
        .call_tool(
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
        .call_tool(
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
    let mut client = DuplexMcpClient::new(TinixCutMcpServer::new()).await;
    let empty = client
        .call_tool(
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
        .call_tool(
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

    let mut client = DuplexMcpClient::new(TinixCutMcpServer::new()).await;
    let result = client
        .call_tool(
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
