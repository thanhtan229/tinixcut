//! Agent Template Generation & Refinement Service (Phase 35)
//!
//! Provides the canonical typed declarative template-draft schema, validation,
//! normalization, shared-compositor preview, atomic apply transactions, and
//! catalog persistence for external CLI/MCP coding agents.
//!
//! Architecture Invariants:
//! - Automation/agent is a client of the editor, NOT mouse/keyboard simulation and NOT a second reducer.
//! - All mutations route exclusively through the canonical CommandService.
//! - Strict schema validation with unknown fields and malicious expressions rejected before mutation.
//! - Revision conflict / stale checks prevent silent overwrites.
//! - Preview and export consume identical normalized composition semantics.

use crate::modules::command::{
    CommandEnvelope, CommandError, CommandOperation, CommandReceipt, CommandService,
    OperationEnvelope,
};
use crate::modules::composition::CompositionPlan;
use crate::modules::domain::id::is_valid_id;
use crate::modules::domain::template::{
    apply_property_to_layer, ResponsivePolicy, TemplateDefinition, TemplateInstance, TemplateLayer,
    TemplateLayerKind, TemplateOverride, TemplateParameterBinding, TemplateParameterDefinition,
    TemplateResourceManifest, MAX_TEMPLATE_GRAPHEMES_PER_SLOT, MAX_TEMPLATE_LAYERS,
    MAX_TEMPLATE_TEXT_SLOTS,
};
use crate::modules::domain::text::grapheme_count;
use crate::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use crate::modules::domain::{is_hex_color, DomainError, ItemKind, Project, TimelineItem};
use crate::modules::media::text_overlay::render_frame_overlay_buffer_at;
use crate::modules::media::text_renderer_poc::RgbaBuffer;
use crate::modules::persistence::{load_project, save_project_atomic};
use crate::modules::templates::catalog::{TemplateCatalog, TemplateCatalogError};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use thiserror::Error;

/// Maximum payload size allowed for a template draft JSON: 1 MB.
pub const MAX_TEMPLATE_DRAFT_JSON_BYTES: usize = 1_048_576;

/// Maximum repair/refinement iterations permitted before failing the budget.
pub const MAX_REPAIR_ITERATIONS: u32 = 5;

/// Current supported template draft schema version.
pub const CURRENT_TEMPLATE_DRAFT_SCHEMA_VERSION: u32 = 1;

fn default_schema_version() -> u32 {
    CURRENT_TEMPLATE_DRAFT_SCHEMA_VERSION
}

fn default_version_one() -> u32 {
    1
}

fn default_iteration_one() -> u32 {
    1
}

fn default_timestamp() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn default_template_duration() -> RationalTime {
    RationalTime::new(90, FrameRate::FPS_30)
}

#[derive(Debug, Error)]
pub enum AgentTemplateError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Validation failed with {count} issue(s): {first_error}")]
    Validation {
        count: usize,
        first_error: String,
        report: TemplateValidationReport,
    },

    #[error("Draft payload exceeds size limit of {max_bytes} bytes: found {found_bytes}")]
    PayloadOversized {
        max_bytes: usize,
        found_bytes: usize,
    },

    #[error("Repair iteration budget exceeded: maximum is {max_iterations}, requested iteration was {requested}")]
    RepairBudgetExceeded { max_iterations: u32, requested: u32 },

    #[error("Catalog error: {0}")]
    Catalog(#[from] TemplateCatalogError),

    #[error("Command execution error: {0}")]
    Command(#[from] CommandError),

    #[error("Item '{0}' not found in project")]
    ItemNotFound(String),

    #[error("Item '{0}' is not a text item and cannot receive a template")]
    NotTextItem(String),

    #[error("Domain error: {0}")]
    Domain(#[from] DomainError),

    #[error("Preview render error: {0}")]
    Preview(String),

    #[error("Security violation: {0}")]
    Security(String),

    #[error("Project load error: {0}")]
    ProjectLoad(String),
}

/// A declared slot within a template draft (for agent authoring clarity).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct TemplateDraftSlot {
    pub slot_key: String,
    pub name: String,
    pub slot_type: String, // "text" | "image" | "shape"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub required: bool,
}

/// Provenance metadata tracking author, agent version, prompt, iteration, and approximations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct TemplateDraftProvenance {
    pub author_type: String, // "agent" | "user" | "import"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_draft_id: Option<String>,
    #[serde(default = "default_iteration_one")]
    pub iteration: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unsupported_approximations: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_missing_resources: Vec<String>,
    #[serde(default = "default_timestamp")]
    pub created_at: String,
}

impl Default for TemplateDraftProvenance {
    fn default() -> Self {
        Self {
            author_type: "agent".to_string(),
            agent_id: Some("tinix-agent-cli".to_string()),
            agent_version: Some("1.0.0".to_string()),
            prompt: None,
            reference_summary: None,
            parent_draft_id: None,
            iteration: 1,
            unsupported_approximations: Vec::new(),
            required_missing_resources: Vec::new(),
            created_at: default_timestamp(),
        }
    }
}

/// Canonical, typed declarative template draft.
/// Rejects unknown fields, scripts, shell paths, or arbitrary expressions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct TemplateDraft {
    pub draft_id: String,
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub template_id: String,
    #[serde(default = "default_version_one")]
    pub template_version: u32,
    pub name: String,
    pub category: String,
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slots: Vec<TemplateDraftSlot>,
    #[serde(default)]
    pub parameters: Vec<TemplateParameterDefinition>,
    #[serde(default)]
    pub bindings: Vec<TemplateParameterBinding>,
    pub layers: Vec<TemplateLayer>,
    #[serde(default)]
    pub resources: TemplateResourceManifest,
    #[serde(default)]
    pub responsive_policy: ResponsivePolicy,
    #[serde(default = "default_template_duration")]
    pub default_duration: RationalTime,
    pub provenance: TemplateDraftProvenance,
    #[serde(default)]
    pub user_overrides: Vec<TemplateOverride>,
}

impl TemplateDraft {
    pub fn to_template_definition(&self) -> TemplateDefinition {
        TemplateDefinition {
            id: self.template_id.clone(),
            version: self.template_version,
            name: self.name.clone(),
            category: self.category.clone(),
            description: self.description.clone(),
            tags: self.tags.clone(),
            parameters: self.parameters.clone(),
            bindings: self.bindings.clone(),
            layers: self.layers.clone(),
            resources: self.resources.clone(),
            responsive_policy: self.responsive_policy.clone(),
            default_duration: self.default_duration,
        }
    }

    pub fn to_instance(&self) -> TemplateInstance {
        let def = self.to_template_definition();
        let mut inst = def.to_instance();
        inst.overrides = self.user_overrides.clone();
        for ovr in &self.user_overrides {
            if let Some(layer) = inst
                .layers
                .iter_mut()
                .find(|l| l.layer_id == ovr.target_layer_id)
            {
                apply_property_to_layer(layer, ovr.property, &ovr.value);
            }
        }
        inst
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValidationSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssue {
    pub severity: ValidationSeverity,
    pub path: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateValidationReport {
    pub valid: bool,
    pub schema_version: u32,
    pub issues: Vec<ValidationIssue>,
    pub missing_resources: Vec<String>,
    pub unsupported_options: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized_draft: Option<TemplateDraft>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateCapabilitiesReport {
    pub schema_version: u32,
    pub max_layers: usize,
    pub max_text_slots: usize,
    pub max_graphemes_per_slot: usize,
    pub max_repair_iterations: u32,
    pub supported_layer_types: Vec<String>,
    pub supported_shape_kinds: Vec<String>,
    pub supported_parameter_types: Vec<String>,
    pub supported_property_paths: Vec<String>,
    pub supported_animation_presets: Vec<String>,
    pub supported_animation_units: Vec<String>,
    pub supported_stagger_orders: Vec<String>,
    pub supported_loop_modes: Vec<String>,
    pub supported_duration_conflict_policies: Vec<String>,
    pub supported_units: Vec<String>,
    pub built_in_primitives: Vec<String>,
    pub draft_json_schema: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePreviewReceipt {
    pub output_path: String,
    pub width: u32,
    pub height: u32,
    pub time_seconds: f64,
    pub has_visible_content: bool,
    pub non_zero_pixels: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateApplyReceipt {
    pub receipt: CommandReceipt,
    pub applied_draft_id: String,
    pub template_id: String,
    pub template_version: u32,
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateSaveReceipt {
    pub template_id: String,
    pub version: u32,
    pub saved_path: String,
}

pub struct AgentTemplateService;

impl AgentTemplateService {
    /// Return comprehensive capability surface and draft JSON schema.
    pub fn capabilities() -> TemplateCapabilitiesReport {
        TemplateCapabilitiesReport {
            schema_version: CURRENT_TEMPLATE_DRAFT_SCHEMA_VERSION,
            max_layers: MAX_TEMPLATE_LAYERS,
            max_text_slots: MAX_TEMPLATE_TEXT_SLOTS,
            max_graphemes_per_slot: MAX_TEMPLATE_GRAPHEMES_PER_SLOT,
            max_repair_iterations: MAX_REPAIR_ITERATIONS,
            supported_layer_types: vec![
                "Text".to_string(),
                "Shape".to_string(),
                "Image".to_string(),
            ],
            supported_shape_kinds: vec![
                "Rectangle".to_string(),
                "RoundedRectangle".to_string(),
                "Ellipse".to_string(),
                "Circle".to_string(),
                "Line".to_string(),
            ],
            supported_parameter_types: vec![
                "Text".to_string(),
                "Color".to_string(),
                "Number".to_string(),
                "Boolean".to_string(),
                "Enum".to_string(),
            ],
            supported_property_paths: vec![
                "TextContent".to_string(),
                "FontColor".to_string(),
                "FontSize".to_string(),
                "FontFamily".to_string(),
                "BackgroundColor".to_string(),
                "PositionX".to_string(),
                "PositionY".to_string(),
                "ScaleX".to_string(),
                "ScaleY".to_string(),
                "Rotation".to_string(),
                "Opacity".to_string(),
                "FillColor".to_string(),
                "StrokeColor".to_string(),
                "StrokeWidth".to_string(),
                "CornerRadius".to_string(),
                "Width".to_string(),
                "Height".to_string(),
                "Visible".to_string(),
            ],
            supported_animation_presets: vec![
                "FadeIn".to_string(),
                "FadeOut".to_string(),
                "SlideInLeft".to_string(),
                "SlideInRight".to_string(),
                "PopIn".to_string(),
                "Typewriter".to_string(),
                "Wiggle".to_string(),
                "Pulse".to_string(),
            ],
            supported_animation_units: vec![
                "All".to_string(),
                "Grapheme".to_string(),
                "Word".to_string(),
                "Line".to_string(),
            ],
            supported_stagger_orders: vec![
                "Forward".to_string(),
                "Backward".to_string(),
                "CenterOut".to_string(),
                "Random".to_string(),
            ],
            supported_loop_modes: vec![
                "None".to_string(),
                "Repeat".to_string(),
                "PingPong".to_string(),
            ],
            supported_duration_conflict_policies: vec![
                "Reject".to_string(),
                "ProportionalScale".to_string(),
                "ClampOut".to_string(),
            ],
            supported_units: vec![
                "pixels".to_string(),
                "normalized_0_1".to_string(),
                "degrees".to_string(),
                "rational_time".to_string(),
                "seconds".to_string(),
                "hex_color_#RRGGBB_#RRGGBBAA".to_string(),
            ],
            built_in_primitives: vec![
                "headline-bold".to_string(),
                "subtle-lower-third".to_string(),
                "retro-neon".to_string(),
                "cyber-frame".to_string(),
                "minimal-clean".to_string(),
                "speech-bubble".to_string(),
                "tag-pill".to_string(),
                "split-accent".to_string(),
                "gradient-wave".to_string(),
                "typewriter-box".to_string(),
                "card-slide".to_string(),
                "quote-frame".to_string(),
            ],
            draft_json_schema: Self::draft_json_schema(),
        }
    }

    /// Machine-readable JSON Schema 2020-12 representation of TemplateDraft.
    pub fn draft_json_schema() -> serde_json::Value {
        serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "title": "TinixCutTemplateDraft",
            "type": "object",
            "additionalProperties": false,
            "required": [
                "draftId",
                "templateId",
                "name",
                "category",
                "description",
                "layers",
                "provenance"
            ],
            "properties": {
                "draftId": { "type": "string" },
                "schemaVersion": { "type": "integer", "const": 1 },
                "templateId": { "type": "string", "pattern": "^[a-zA-Z0-9_-]+$" },
                "templateVersion": { "type": "integer", "minimum": 1 },
                "name": { "type": "string", "minLength": 1 },
                "category": { "type": "string", "minLength": 1 },
                "description": { "type": "string" },
                "tags": { "type": "array", "items": { "type": "string" } },
                "slots": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "required": ["slotKey", "name", "slotType"],
                        "properties": {
                            "slotKey": { "type": "string" },
                            "name": { "type": "string" },
                            "slotType": { "type": "string", "enum": ["text", "image", "shape"] },
                            "description": { "type": "string" },
                            "required": { "type": "boolean" }
                        }
                    }
                },
                "parameters": { "type": "array", "items": { "type": "object" } },
                "bindings": { "type": "array", "items": { "type": "object" } },
                "layers": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": MAX_TEMPLATE_LAYERS,
                    "items": { "type": "object" }
                },
                "resources": {
                    "type": "object",
                    "properties": {
                        "resources": { "type": "array", "items": { "type": "object" } }
                    }
                },
                "responsivePolicy": { "type": "object" },
                "defaultDuration": { "type": "object" },
                "provenance": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["authorType"],
                    "properties": {
                        "authorType": { "type": "string", "enum": ["agent", "user", "import"] },
                        "agentId": { "type": "string" },
                        "agentVersion": { "type": "string" },
                        "prompt": { "type": "string" },
                        "referenceSummary": { "type": "string" },
                        "parentDraftId": { "type": "string" },
                        "iteration": { "type": "integer", "minimum": 1, "maximum": MAX_REPAIR_ITERATIONS },
                        "unsupportedApproximations": { "type": "array", "items": { "type": "string" } },
                        "requiredMissingResources": { "type": "array", "items": { "type": "string" } },
                        "createdAt": { "type": "string" }
                    }
                },
                "userOverrides": { "type": "array", "items": { "type": "object" } }
            }
        })
    }

    /// Perform a deep security and expression check on raw JSON text.
    pub fn scan_malicious_content(content: &str) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        let suspicious_tokens = [
            (
                "<script",
                "MALICIOUS_SCRIPT_TAG",
                "Draft contains HTML/XML script tags",
            ),
            (
                "</script",
                "MALICIOUS_SCRIPT_TAG",
                "Draft contains HTML/XML script tags",
            ),
            (
                "javascript:",
                "MALICIOUS_JS_URI",
                "Draft contains javascript: URI scheme",
            ),
            (
                "powershell",
                "MALICIOUS_SHELL_COMMAND",
                "Draft contains powershell reference",
            ),
            (
                "cmd.exe",
                "MALICIOUS_SHELL_COMMAND",
                "Draft contains cmd.exe reference",
            ),
            (
                "/bin/sh",
                "MALICIOUS_SHELL_COMMAND",
                "Draft contains shell execution command",
            ),
            (
                "/bin/bash",
                "MALICIOUS_SHELL_COMMAND",
                "Draft contains bash execution command",
            ),
            (
                "rm -rf",
                "MALICIOUS_SHELL_COMMAND",
                "Draft contains destructive command",
            ),
            (
                "eval(",
                "MALICIOUS_EXPRESSION",
                "Draft contains arbitrary eval() expression",
            ),
            (
                "SELECT ",
                "MALICIOUS_SQL_INJECTION",
                "Draft contains SQL query pattern",
            ),
            (
                "DROP TABLE",
                "MALICIOUS_SQL_INJECTION",
                "Draft contains SQL drop pattern",
            ),
            (
                "gl_FragColor",
                "UNSUPPORTED_NATIVE_SHADER",
                "Draft contains arbitrary GLSL shader instructions",
            ),
            (
                "varying ",
                "UNSUPPORTED_NATIVE_SHADER",
                "Draft contains native shader keywords",
            ),
            (
                "uniform ",
                "UNSUPPORTED_NATIVE_SHADER",
                "Draft contains native shader keywords",
            ),
            (
                "ffmpeg -",
                "UNSUPPORTED_FFMPEG_SHELL",
                "Draft contains arbitrary FFmpeg shell commands",
            ),
            (
                "-filter_complex",
                "UNSUPPORTED_FFMPEG_SHELL",
                "Draft contains raw FFmpeg filtergraph strings",
            ),
            (
                "data:text/html",
                "MALICIOUS_HTML_PAYLOAD",
                "Draft contains data:text/html URI payload",
            ),
        ];

        let lower = content.to_lowercase();
        for (token, code, desc) in suspicious_tokens {
            if lower.contains(&token.to_lowercase()) {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    path: "root".to_string(),
                    code: code.to_string(),
                    message: format!("{}: rejected before mutation", desc),
                });
            }
        }
        issues
    }

    /// Full validation and normalization of a TemplateDraft.
    pub fn validate_draft(
        draft: &TemplateDraft,
        raw_json: Option<&str>,
        catalog: Option<&TemplateCatalog>,
    ) -> TemplateValidationReport {
        let mut issues = Vec::new();
        let mut missing_resources = Vec::new();
        let mut unsupported_options = Vec::new();

        // 1. Raw JSON size and security scan
        if let Some(raw) = raw_json {
            if raw.len() > MAX_TEMPLATE_DRAFT_JSON_BYTES {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    path: "payload".to_string(),
                    code: "DRAFT_OVERSIZED".to_string(),
                    message: format!(
                        "Draft payload size ({} bytes) exceeds limit of {} bytes",
                        raw.len(),
                        MAX_TEMPLATE_DRAFT_JSON_BYTES
                    ),
                });
            }
            let mut sec_issues = Self::scan_malicious_content(raw);
            issues.append(&mut sec_issues);
        }

        // 2. Identification checks
        if draft.draft_id.trim().is_empty() {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Error,
                path: "draftId".to_string(),
                code: "EMPTY_DRAFT_ID".to_string(),
                message: "draftId cannot be empty".to_string(),
            });
        }
        if !is_valid_id(&draft.template_id) {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Error,
                path: "templateId".to_string(),
                code: "INVALID_TEMPLATE_ID".to_string(),
                message: format!(
                    "templateId '{}' is not a valid identifier",
                    draft.template_id
                ),
            });
        }
        if draft.name.trim().is_empty() {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Error,
                path: "name".to_string(),
                code: "EMPTY_NAME".to_string(),
                message: "Template name cannot be empty".to_string(),
            });
        }
        if draft.template_version == 0 {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Error,
                path: "templateVersion".to_string(),
                code: "INVALID_VERSION".to_string(),
                message: "Template version must be positive (>= 1)".to_string(),
            });
        }

        // 3. Provenance and repair budget
        if draft.provenance.iteration > MAX_REPAIR_ITERATIONS {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Error,
                path: "provenance.iteration".to_string(),
                code: "REPAIR_BUDGET_EXCEEDED".to_string(),
                message: format!(
                    "Repair iteration {} exceeds maximum allowed budget ({})",
                    draft.provenance.iteration, MAX_REPAIR_ITERATIONS
                ),
            });
        }
        for approx in &draft.provenance.unsupported_approximations {
            unsupported_options.push(approx.clone());
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Warning,
                path: "provenance.unsupportedApproximations".to_string(),
                code: "UNSUPPORTED_APPROXIMATION".to_string(),
                message: format!("Option '{}' is approximated by editor primitives", approx),
            });
        }
        for missing in &draft.provenance.required_missing_resources {
            missing_resources.push(missing.clone());
        }

        // 4. Layer count and text slot limits
        if draft.layers.is_empty() {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Error,
                path: "layers".to_string(),
                code: "TEMPLATE_EMPTY".to_string(),
                message: "Template must declare at least one layer".to_string(),
            });
        }
        if draft.layers.len() > MAX_TEMPLATE_LAYERS {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Error,
                path: "layers".to_string(),
                code: "EXCEEDS_LAYER_LIMIT".to_string(),
                message: format!(
                    "Template declares {} layers, exceeding limit of {}",
                    draft.layers.len(),
                    MAX_TEMPLATE_LAYERS
                ),
            });
        }

        let mut layer_ids = HashSet::new();
        let mut text_slots = 0;
        let mut primary_count = 0;

        for (idx, layer) in draft.layers.iter().enumerate() {
            let path_prefix = format!("layers[{}]", idx);
            if !is_valid_id(&layer.layer_id) {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    path: format!("{}.layerId", path_prefix),
                    code: "INVALID_LAYER_ID".to_string(),
                    message: format!("Invalid layer ID '{}'", layer.layer_id),
                });
            } else if !layer_ids.insert(layer.layer_id.as_str()) {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    path: format!("{}.layerId", path_prefix),
                    code: "DUPLICATE_LAYER_ID".to_string(),
                    message: format!("Duplicate layer ID '{}'", layer.layer_id),
                });
            }

            if layer.name.trim().is_empty() {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    path: format!("{}.name", path_prefix),
                    code: "EMPTY_LAYER_NAME".to_string(),
                    message: "Layer name cannot be empty".to_string(),
                });
            }

            if !layer.opacity.is_finite() || !(0.0..=1.0).contains(&layer.opacity) {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    path: format!("{}.opacity", path_prefix),
                    code: "INVALID_OPACITY".to_string(),
                    message: "Layer opacity must be between 0.0 and 1.0".to_string(),
                });
            }

            // Layer-type specific validation
            match &layer.kind {
                TemplateLayerKind::Text(t) => {
                    text_slots += 1;
                    if t.is_primary {
                        primary_count += 1;
                    }
                    if let Some(text) = &t.text_content {
                        let count = grapheme_count(text);
                        if count > MAX_TEMPLATE_GRAPHEMES_PER_SLOT {
                            issues.push(ValidationIssue {
                                severity: ValidationSeverity::Error,
                                path: format!("{}.kind.data.textContent", path_prefix),
                                code: "EXCEEDS_GRAPHEME_LIMIT".to_string(),
                                message: format!(
                                    "Text slot length ({} graphemes) exceeds limit of {}",
                                    count, MAX_TEMPLATE_GRAPHEMES_PER_SLOT
                                ),
                            });
                        }
                    }

                    // Check font availability and approximations
                    if let Some(style) = &t.text_style {
                        if !style.font_color.is_empty() && !is_hex_color(&style.font_color) {
                            issues.push(ValidationIssue {
                                severity: ValidationSeverity::Error,
                                path: format!("{}.kind.data.textStyle.fontColor", path_prefix),
                                code: "INVALID_HEX_COLOR".to_string(),
                                message: format!("Invalid hex color '{}'", style.font_color),
                            });
                        }
                    }
                    if let Some(rich) = &t.text_rich {
                        if let Some(font) = &rich.font {
                            let standard_fonts = [
                                "Inter",
                                "Arial",
                                "Roboto",
                                "Segoe UI",
                                "Tahoma",
                                "Times New Roman",
                                "Helvetica",
                                "sans-serif",
                                "serif",
                            ];
                            if !standard_fonts
                                .iter()
                                .any(|f| f.eq_ignore_ascii_case(&font.family))
                                && font.asset_id.is_none()
                            {
                                unsupported_options.push(format!("Font family '{}'", font.family));
                                issues.push(ValidationIssue {
                                    severity: ValidationSeverity::Warning,
                                    path: format!("{}.kind.data.textRich.font.family", path_prefix),
                                    code: "FONT_APPROXIMATION".to_string(),
                                    message: format!(
                                        "Non-standard font '{}' will fallback to system typography",
                                        font.family
                                    ),
                                });
                            }
                        }
                        if let Err(e) = rich.validate(t.text_content.as_deref().unwrap_or("")) {
                            issues.push(ValidationIssue {
                                severity: ValidationSeverity::Error,
                                path: format!("{}.kind.data.textRich", path_prefix),
                                code: "INVALID_RICH_STYLE".to_string(),
                                message: e.to_string(),
                            });
                        }
                    }
                }
                TemplateLayerKind::Shape(s) => {
                    if let Err(e) = s.validate() {
                        issues.push(ValidationIssue {
                            severity: ValidationSeverity::Error,
                            path: format!("{}.kind.data", path_prefix),
                            code: "INVALID_SHAPE".to_string(),
                            message: e.to_string(),
                        });
                    }
                }
                TemplateLayerKind::Image(img) => {
                    if let Err(e) = img.validate() {
                        issues.push(ValidationIssue {
                            severity: ValidationSeverity::Error,
                            path: format!("{}.kind.data", path_prefix),
                            code: "INVALID_IMAGE_LAYER".to_string(),
                            message: e.to_string(),
                        });
                    }
                    if let Some(aid) = &img.asset_id {
                        if !draft
                            .resources
                            .resources
                            .iter()
                            .any(|r| &r.resource_id == aid)
                        {
                            missing_resources.push(aid.clone());
                            issues.push(ValidationIssue {
                                severity: ValidationSeverity::Warning,
                                path: format!("{}.kind.data.assetId", path_prefix),
                                code: "UNRESOLVED_IMAGE_ASSET".to_string(),
                                message: format!(
                                    "Image layer references unmanifested asset '{}'",
                                    aid
                                ),
                            });
                        }
                    }
                }
            }
        }

        if text_slots > MAX_TEMPLATE_TEXT_SLOTS {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Error,
                path: "layers".to_string(),
                code: "EXCEEDS_SLOT_LIMIT".to_string(),
                message: format!(
                    "Template declares {} text slots, exceeding limit of {}",
                    text_slots, MAX_TEMPLATE_TEXT_SLOTS
                ),
            });
        }
        if primary_count > 1 {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Error,
                path: "layers".to_string(),
                code: "MULTIPLE_PRIMARY_SLOTS".to_string(),
                message: format!(
                    "Template declares {} primary text slots; at most one is allowed",
                    primary_count
                ),
            });
        }

        // 5. Parameters & Bindings validation
        let mut param_map = HashMap::new();
        for (p_idx, param) in draft.parameters.iter().enumerate() {
            if let Err(e) = param.validate() {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    path: format!("parameters[{}]", p_idx),
                    code: "INVALID_PARAMETER".to_string(),
                    message: e.to_string(),
                });
            } else if param_map
                .insert(param.key.as_str(), &param.param_type)
                .is_some()
            {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    path: format!("parameters[{}].key", p_idx),
                    code: "DUPLICATE_PARAMETER_KEY".to_string(),
                    message: format!("Duplicate parameter key '{}'", param.key),
                });
            }
        }

        for (b_idx, binding) in draft.bindings.iter().enumerate() {
            if let Err(e) = binding.validate(&param_map, &layer_ids) {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    path: format!("bindings[{}]", b_idx),
                    code: "INVALID_BINDING".to_string(),
                    message: e.to_string(),
                });
            }
        }

        // 6. Resources validation and catalog existence check
        for (r_idx, res) in draft.resources.resources.iter().enumerate() {
            if let Err(e) = res.validate() {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    path: format!("resources.resources[{}]", r_idx),
                    code: "INVALID_RESOURCE_REFERENCE".to_string(),
                    message: e.to_string(),
                });
            } else if let Some(cat) = catalog {
                let target_file = cat.resources_dir().join(&res.file_name);
                if !target_file.exists() && res.required {
                    missing_resources.push(res.resource_id.clone());
                    issues.push(ValidationIssue {
                        severity: ValidationSeverity::Warning,
                        path: format!("resources.resources[{}].fileName", r_idx),
                        code: "MISSING_CATALOG_RESOURCE".to_string(),
                        message: format!(
                            "Required resource '{}' ({}) is missing from catalog directory",
                            res.resource_id, res.file_name
                        ),
                    });
                }
            }
        }

        // 7. Responsive policy and duration
        if let Err(e) = draft.responsive_policy.validate() {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Error,
                path: "responsivePolicy".to_string(),
                code: "INVALID_RESPONSIVE_POLICY".to_string(),
                message: e.to_string(),
            });
        }
        if draft.default_duration.value <= 0 {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Error,
                path: "defaultDuration".to_string(),
                code: "NON_POSITIVE_DURATION".to_string(),
                message: "defaultDuration must be strictly positive".to_string(),
            });
        }

        let is_valid = !issues
            .iter()
            .any(|i| i.severity == ValidationSeverity::Error);
        let normalized = if is_valid {
            let mut norm = draft.clone();
            // Normalize default duration to 30fps if needed
            if norm.default_duration.value <= 0 {
                norm.default_duration = default_template_duration();
            }
            Some(norm)
        } else {
            None
        };

        missing_resources.sort();
        missing_resources.dedup();
        unsupported_options.sort();
        unsupported_options.dedup();

        TemplateValidationReport {
            valid: is_valid,
            schema_version: CURRENT_TEMPLATE_DRAFT_SCHEMA_VERSION,
            issues,
            missing_resources,
            unsupported_options,
            normalized_draft: normalized,
        }
    }

    /// Render a preview frame overlay of the draft at specified time using the shared compositor.
    pub fn preview_draft(
        draft: &TemplateDraft,
        time_seconds: f64,
        width: Option<u32>,
        height: Option<u32>,
    ) -> Result<RgbaBuffer, AgentTemplateError> {
        let w = width
            .unwrap_or(draft.responsive_policy.reference_width)
            .max(320);
        let h = height
            .unwrap_or(draft.responsive_policy.reference_height)
            .max(180);
        let fps = FrameRate::FPS_30;

        let mut project = Project::new("TemplatePreviewStage".to_string());
        project.settings.width = w;
        project.settings.height = h;
        project.settings.frame_rate = fps;
        project.tracks.push(crate::modules::domain::Track::new(
            "Preview Track".to_string(),
            crate::modules::domain::TrackKind::Video,
        ));

        let instance = draft.to_instance();
        let dur = draft.default_duration.rescaled_to(fps);
        let mut item = TimelineItem::new(
            None,
            "PreviewDraftClip".to_string(),
            ItemKind::Text,
            TimeRange::new(RationalTime::zero(fps), dur),
            TimeRange::new(RationalTime::zero(fps), dur),
        )?;

        // Attach primary text layer properties if present
        if let Some(primary) = draft.layers.iter().find(|l| {
            if let TemplateLayerKind::Text(t) = &l.kind {
                t.is_primary
            } else {
                false
            }
        }) {
            if let TemplateLayerKind::Text(t) = &primary.kind {
                if let Some(txt) = &t.text_content {
                    item.text_content = Some(txt.clone());
                }
                if let Some(st) = &t.text_style {
                    item.text_style = Some(st.clone());
                }
                if let Some(rich) = &t.text_rich {
                    item.text_rich = Some(rich.clone());
                }
                if let Some(anim) = &t.text_animation {
                    item.text_animation = Some(anim.clone());
                }
            }
        }

        item.template_instance = Some(instance);
        item.validate()
            .map_err(|e| AgentTemplateError::Preview(e.to_string()))?;

        project.tracks[0].items.push(item);

        let plan = CompositionPlan::from_project(&project).map_err(|e| {
            AgentTemplateError::Preview(format!("Failed to build CompositionPlan: {}", e))
        })?;

        let target_layer = plan.visual_layers.first().ok_or_else(|| {
            AgentTemplateError::Preview("No visual layer produced in plan".to_string())
        })?;

        let sample_time = RationalTime::from_seconds(time_seconds, fps);
        let buf = render_frame_overlay_buffer_at(&project, &plan, target_layer, sample_time)
            .map_err(|e| AgentTemplateError::Preview(format!("Compositor error: {}", e)))?
            .ok_or_else(|| {
                AgentTemplateError::Preview("Compositor returned empty frame".to_string())
            })?;

        Ok(buf)
    }

    /// Render preview frame and persist to PNG file.
    pub fn preview_draft_to_png(
        draft: &TemplateDraft,
        output_path: &Path,
        time_seconds: f64,
        width: Option<u32>,
        height: Option<u32>,
    ) -> Result<TemplatePreviewReceipt, AgentTemplateError> {
        let buf = Self::preview_draft(draft, time_seconds, width, height)?;
        let png_bytes = buf.to_png_bytes();

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(output_path, &png_bytes)?;

        let mut non_zero_pixels = 0;
        for y in 0..buf.height {
            for x in 0..buf.width {
                let [_, _, _, a] = buf.get_pixel(x, y);
                if a > 0 {
                    non_zero_pixels += 1;
                }
            }
        }

        Ok(TemplatePreviewReceipt {
            output_path: output_path.to_string_lossy().to_string(),
            width: buf.width,
            height: buf.height,
            time_seconds,
            has_visible_content: non_zero_pixels > 0,
            non_zero_pixels,
        })
    }

    /// Save the validated template draft into the template catalog.
    pub fn save_draft_to_catalog(
        draft: &TemplateDraft,
        catalog: &TemplateCatalog,
        overwrite: bool,
    ) -> Result<TemplateSaveReceipt, AgentTemplateError> {
        let report = Self::validate_draft(draft, None, Some(catalog));
        if !report.valid {
            let first_err = report
                .issues
                .iter()
                .find(|i| i.severity == ValidationSeverity::Error)
                .map(|i| i.message.clone())
                .unwrap_or_else(|| "Validation error".to_string());
            return Err(AgentTemplateError::Validation {
                count: report.issues.len(),
                first_error: first_err,
                report,
            });
        }

        let def = draft.to_template_definition();
        catalog.ensure_dirs()?;

        let dest_file = catalog.definition_file_path(&def.id, def.version)?;
        if dest_file.exists() && !overwrite {
            return Err(AgentTemplateError::Catalog(
                TemplateCatalogError::ImmutableVersionConflict {
                    id: def.id.clone(),
                    version: def.version,
                },
            ));
        }

        catalog.save_user_definition(&def)?;

        Ok(TemplateSaveReceipt {
            template_id: def.id,
            version: def.version,
            saved_path: dest_file.to_string_lossy().to_string(),
        })
    }

    /// Apply template draft directly to a project text item as an atomic CommandService transaction.
    /// Preserves ADR-001 idempotency and durable atomic persistence.
    pub fn apply_draft_to_project(
        project_path: &Path,
        item_id: &str,
        draft: &TemplateDraft,
        client_id: Option<String>,
        client_epoch: Option<u64>,
        sequence_number: Option<u64>,
        transaction_id: Option<String>,
        base_revision: Option<u64>,
        catalog: &TemplateCatalog,
    ) -> Result<TemplateApplyReceipt, AgentTemplateError> {
        // 1. Validate draft before applying
        let report = Self::validate_draft(draft, None, Some(catalog));
        if !report.valid {
            let first_err = report
                .issues
                .iter()
                .find(|i| i.severity == ValidationSeverity::Error)
                .map(|i| i.message.clone())
                .unwrap_or_else(|| "Validation error".to_string());
            return Err(AgentTemplateError::Validation {
                count: report.issues.len(),
                first_error: first_err,
                report,
            });
        }

        // 2. Ensure template definition exists in the catalog so TemplateApply can resolve it
        let def = draft.to_template_definition();
        catalog.ensure_dirs()?;
        let def_path = catalog.definition_file_path(&def.id, def.version)?;
        if !def_path.exists() {
            catalog.save_user_definition(&def)?;
        }

        // 3. Load project
        let project = load_project(project_path)
            .map_err(|e| AgentTemplateError::ProjectLoad(e.to_string()))?;

        // 4. Verify item exists and is text
        let mut target_found = false;
        for track in &project.tracks {
            if let Some(item) = track.items.iter().find(|i| i.item_id == item_id) {
                target_found = true;
                if item.kind != ItemKind::Text {
                    return Err(AgentTemplateError::NotTextItem(item_id.to_string()));
                }
                break;
            }
        }
        if !target_found {
            return Err(AgentTemplateError::ItemNotFound(item_id.to_string()));
        }

        // 5. Construct CommandEnvelope with deterministic operation IDs for idempotency
        let cid = client_id.unwrap_or_else(|| "agent_template".to_string());
        let cepoch = client_epoch.unwrap_or(project.client_registry.current_epoch);
        let seq = sequence_number.unwrap_or_else(|| {
            project
                .client_registry
                .clients
                .get(&cid)
                .map(|c| c.highest_sequence + 1)
                .unwrap_or(1)
        });
        let tx_id =
            transaction_id.unwrap_or_else(|| format!("tx_{}", uuid::Uuid::new_v4().simple()));
        let base_tx = if tx_id.len() > 40 {
            &tx_id[..40]
        } else {
            &tx_id
        };

        let mut operations = Vec::new();

        let op0 = CommandOperation::TemplateApply {
            item_id: item_id.to_string(),
            template_id: draft.template_id.clone(),
            template_version: draft.template_version,
            initial_parameters: None,
        };
        operations.push(OperationEnvelope {
            operation_id: format!("{}_op_0_apply", base_tx),
            operation: op0,
        });

        // Operation 2: Apply primary text contents if specified
        if let Some(primary) = draft.layers.iter().find(|l| {
            if let TemplateLayerKind::Text(t) = &l.kind {
                t.is_primary
            } else {
                false
            }
        }) {
            if let TemplateLayerKind::Text(t) = &primary.kind {
                if let Some(rich) = &t.text_rich {
                    operations.push(OperationEnvelope {
                        operation_id: format!("{}_op_1_rich", base_tx),
                        operation: CommandOperation::ClipTextRichSet {
                            item_id: item_id.to_string(),
                            rich: rich.clone(),
                        },
                    });
                }
                if let Some(anim) = &t.text_animation {
                    operations.push(OperationEnvelope {
                        operation_id: format!("{}_op_2_anim", base_tx),
                        operation: CommandOperation::ClipTextAnimationSet {
                            item_id: item_id.to_string(),
                            animation: Some(anim.clone()),
                        },
                    });
                }
            }
        }

        // Operation 3: Batch apply overrides if any
        if !draft.user_overrides.is_empty() {
            let mut ovr_params = HashMap::new();
            for ovr in &draft.user_overrides {
                // If override targets a bound parameter key
                if let Some(binding) = draft.bindings.iter().find(|b| {
                    b.target_layer_id == ovr.target_layer_id && b.target_property == ovr.property
                }) {
                    ovr_params.insert(binding.parameter_key.clone(), ovr.value.clone());
                }
            }
            if !ovr_params.is_empty() {
                operations.push(OperationEnvelope {
                    operation_id: format!("{}_op_3_overrides", base_tx),
                    operation: CommandOperation::TemplateParametersBatchSet {
                        item_id: item_id.to_string(),
                        parameters: ovr_params,
                    },
                });
            }
        }

        let envelope = CommandEnvelope {
            schema_version: crate::modules::domain::CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision: base_revision.unwrap_or(project.revision),
            client_id: Some(cid),
            client_epoch: Some(cepoch),
            sequence_number: Some(seq),
            dry_run: false,
            transaction_id: Some(tx_id),
            operations,
        };

        let mut service = CommandService::new(project);
        let receipt = service.execute(envelope)?;

        // Durably persist atomically to disk
        save_project_atomic(service.project(), project_path)
            .map_err(|e| AgentTemplateError::ProjectLoad(e.to_string()))?;

        Ok(TemplateApplyReceipt {
            receipt: receipt.clone(),
            applied_draft_id: draft.draft_id.clone(),
            template_id: draft.template_id.clone(),
            template_version: draft.template_version,
            revision: receipt.new_revision,
        })
    }
}
