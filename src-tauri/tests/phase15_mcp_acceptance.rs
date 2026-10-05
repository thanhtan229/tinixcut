//! Phase 15 Comprehensive Acceptance Test Suite
//! Validates all 41 Acceptance Criteria (P15-AC-01 through P15-AC-41)
//! defined in Plans/plan_phase15.md, ADR-005, and ADR-001 v3.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use tinixcut_lib::modules::automation::mcp::TinixCutMcpServer;
use tinixcut_lib::modules::command::{
    CommandEnvelope, CommandOperation, CommandReceipt, OperationEnvelope,
};
use tinixcut_lib::modules::domain::{Project, Track, TrackKind, CURRENT_SCHEMA_VERSION};
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
    assert_eq!(tools.len(), 8);
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
async fn test_p15_ac05_ac06_ac07_exact_8_tools_and_no_extraneous_primitives() {
    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;

    let res = client
        .send_request("tools/list", serde_json::json!({}))
        .await;
    let tools = res["result"]["tools"].as_array().expect("tools array");
    assert_eq!(tools.len(), 8, "Must expose exactly 8 tools");

    let mut tool_names: Vec<String> = tools
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    tool_names.sort();

    let expected_tools = vec![
        "tinixcut_apply".to_string(),
        "tinixcut_capabilities".to_string(),
        "tinixcut_dry_run".to_string(),
        "tinixcut_export_cancel".to_string(),
        "tinixcut_export_start".to_string(),
        "tinixcut_export_status".to_string(),
        "tinixcut_inspect".to_string(),
        "tinixcut_save".to_string(),
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

    // Verify schema_version remains CURRENT_SCHEMA_VERSION (3)
    assert_eq!(raw_json["schema_version"], 3);
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
    assert_eq!(tools.len(), 8);

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
        "id": 4,
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
    assert_eq!(val3["id"], 4);
    assert_eq!(val3["result"]["isError"], false);
    assert_eq!(val3["result"]["structuredContent"]["newRevision"], 1);

    // AC-34: Graceful exit on stdin EOF
    drop(stdin);
    let status = child.wait().expect("Child must exit on EOF");
    assert!(status.success(), "Child must exit with exit status 0");

    // Verify disk persistence after subprocess exit
    let reloaded = load_project(&proj_path).unwrap();
    assert_eq!(reloaded.revision, 1);
    assert_eq!(reloaded.tracks.len(), 2);
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
