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
    TemplateParameterValue, TemplatePropertyPath, TemplateResourceManifest,
    MAX_TEMPLATE_GRAPHEMES_PER_SLOT, MAX_TEMPLATE_LAYERS, MAX_TEMPLATE_TEXT_SLOTS,
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

/// Maximum preview resolution bounds to prevent memory exhaustion
pub const MAX_PREVIEW_WIDTH: u32 = 3840;
pub const MAX_PREVIEW_HEIGHT: u32 = 2160;
pub const DEFAULT_PREVIEW_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

#[derive(Debug, Error)]
#[allow(clippy::large_enum_variant)]
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

    #[error("Preview render cancelled")]
    Cancelled,

    #[error("Preview render timed out after {0:?}")]
    Timeout(std::time::Duration),

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
#[derive(Debug, Clone, PartialEq, Serialize)]
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct TemplateDraftHelper {
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

impl<'de> Deserialize<'de> for TemplateDraft {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        validate_raw_draft_json_tree(&value).map_err(serde::de::Error::custom)?;
        let helper = TemplateDraftHelper::deserialize(value).map_err(serde::de::Error::custom)?;
        Ok(TemplateDraft {
            draft_id: helper.draft_id,
            schema_version: helper.schema_version,
            template_id: helper.template_id,
            template_version: helper.template_version,
            name: helper.name,
            category: helper.category,
            description: helper.description,
            tags: helper.tags,
            slots: helper.slots,
            parameters: helper.parameters,
            bindings: helper.bindings,
            layers: helper.layers,
            resources: helper.resources,
            responsive_policy: helper.responsive_policy,
            default_duration: helper.default_duration,
            provenance: helper.provenance,
            user_overrides: helper.user_overrides,
        })
    }
}

/// Recursively validate raw fill values (solid or gradient), rejecting unknown fields and unauthorized enums.
pub fn validate_raw_fill_value(fill: &serde_json::Value, location: &str) -> Result<(), String> {
    if fill.is_null() {
        return Ok(());
    }
    let fill_obj = fill
        .as_object()
        .ok_or_else(|| format!("{location} must be an object"))?;

    let kind_val = fill_obj
        .get("kind")
        .ok_or_else(|| format!("{location} missing 'kind' field"))?;
    let kind = kind_val
        .as_str()
        .ok_or_else(|| format!("{location}.kind must be a string"))?;

    match kind.to_ascii_lowercase().as_str() {
        "solid" => {
            const SOLID_KEYS: &[&str] = &["kind", "color"];
            for k in fill_obj.keys() {
                if !SOLID_KEYS.contains(&k.as_str()) {
                    return Err(format!("Unknown field '{k}' in {location}"));
                }
            }
            if let Some(col) = fill_obj.get("color") {
                if !col.is_string() {
                    return Err(format!("{location}.color must be a string"));
                }
            }
        }
        "gradient" => {
            const GRADIENT_KEYS: &[&str] = &[
                "kind",
                "gradientKind",
                "gradient_kind",
                "angleDeg",
                "angle_deg",
                "radius",
                "space",
                "stops",
            ];
            for k in fill_obj.keys() {
                if !GRADIENT_KEYS.contains(&k.as_str()) {
                    return Err(format!("Unknown field '{k}' in {location}"));
                }
            }
            if let Some(gk) = fill_obj
                .get("gradientKind")
                .or_else(|| fill_obj.get("gradient_kind"))
            {
                let gk_str = gk
                    .as_str()
                    .ok_or_else(|| format!("{location}.gradient_kind must be a string"))?;
                if !["linear", "radial", "Linear", "Radial"].contains(&gk_str) {
                    return Err(format!("Invalid gradient_kind '{gk_str}' in {location}"));
                }
            }
            if let Some(sp) = fill_obj.get("space") {
                let sp_str = sp
                    .as_str()
                    .ok_or_else(|| format!("{location}.space must be a string"))?;
                if !["all", "character", "All", "Character", "ALL", "CHARACTER"].contains(&sp_str) {
                    return Err(format!("Invalid gradient space '{sp_str}' in {location}"));
                }
            }
            if let Some(stops) = fill_obj.get("stops") {
                let stops_arr = stops
                    .as_array()
                    .ok_or_else(|| format!("{location}.stops must be an array"))?;
                const STOP_KEYS: &[&str] = &["offset", "color"];
                for (si, st) in stops_arr.iter().enumerate() {
                    let st_obj = st
                        .as_object()
                        .ok_or_else(|| format!("{location}.stops[{si}] must be an object"))?;
                    for k in st_obj.keys() {
                        if !STOP_KEYS.contains(&k.as_str()) {
                            return Err(format!("Unknown field '{k}' in {location}.stops[{si}]"));
                        }
                    }
                }
            }
        }
        unknown => {
            return Err(format!("Unknown fill kind '{unknown}' in {location}"));
        }
    }
    Ok(())
}

/// Recursively validate raw JSON tree of a template draft BEFORE typed deserialization.
/// Rejects unknown keys anywhere in the tree (including transform.unrecognized and text_rich.script),
/// oversized payload, or structural violations.
pub fn validate_raw_draft_json_tree(val: &serde_json::Value) -> Result<(), String> {
    let obj = val
        .as_object()
        .ok_or_else(|| "TemplateDraft must be a JSON object".to_string())?;

    let raw_str = val.to_string();
    if raw_str.len() > MAX_TEMPLATE_DRAFT_JSON_BYTES {
        return Err(format!(
            "Draft payload size ({} bytes) exceeds limit of {} bytes",
            raw_str.len(),
            MAX_TEMPLATE_DRAFT_JSON_BYTES
        ));
    }

    const ROOT_KEYS: &[&str] = &[
        "draftId",
        "draft_id",
        "schemaVersion",
        "schema_version",
        "templateId",
        "template_id",
        "templateVersion",
        "template_version",
        "name",
        "category",
        "description",
        "tags",
        "slots",
        "parameters",
        "bindings",
        "layers",
        "resources",
        "responsivePolicy",
        "responsive_policy",
        "defaultDuration",
        "default_duration",
        "provenance",
        "userOverrides",
        "user_overrides",
    ];
    for k in obj.keys() {
        if !ROOT_KEYS.contains(&k.as_str()) {
            return Err(format!("Unknown field '{}' in template draft root", k));
        }
    }

    if let Some(slots) = obj.get("slots").and_then(|v| v.as_array()) {
        const SLOT_KEYS: &[&str] = &[
            "slotKey",
            "slot_key",
            "name",
            "slotType",
            "slot_type",
            "description",
            "required",
        ];
        for (i, slot) in slots.iter().enumerate() {
            let s_obj = slot
                .as_object()
                .ok_or_else(|| format!("slots[{i}] must be an object"))?;
            for k in s_obj.keys() {
                if !SLOT_KEYS.contains(&k.as_str()) {
                    return Err(format!("Unknown field '{}' in slots[{i}]", k));
                }
            }
        }
    }

    if let Some(prov) = obj.get("provenance") {
        if let Some(p_obj) = prov.as_object() {
            const PROV_KEYS: &[&str] = &[
                "authorType",
                "author_type",
                "model",
                "prompt",
                "timestamp",
                "parentTemplateId",
                "parent_template_id",
                "parentTemplateVersion",
                "parent_template_version",
                "repairIteration",
                "repair_iteration",
                "agentId",
                "agent_id",
                "agentVersion",
                "agent_version",
                "referenceSummary",
                "reference_summary",
                "parentDraftId",
                "parent_draft_id",
                "iteration",
                "unsupportedApproximations",
                "unsupported_approximations",
                "requiredMissingResources",
                "required_missing_resources",
                "createdAt",
                "created_at",
            ];
            for k in p_obj.keys() {
                if !PROV_KEYS.contains(&k.as_str()) {
                    return Err(format!("Unknown field '{}' in provenance", k));
                }
            }
        }
    }

    if let Some(params) = obj.get("parameters").and_then(|v| v.as_array()) {
        const PARAM_KEYS: &[&str] = &[
            "key",
            "name",
            "paramType",
            "param_type",
            "defaultValue",
            "default_value",
            "description",
        ];
        for (i, p) in params.iter().enumerate() {
            let p_obj = p
                .as_object()
                .ok_or_else(|| format!("parameters[{i}] must be an object"))?;
            for k in p_obj.keys() {
                if !PARAM_KEYS.contains(&k.as_str()) {
                    return Err(format!("Unknown field '{}' in parameters[{i}]", k));
                }
            }
            if let Some(pt) = p_obj.get("paramType").or_else(|| p_obj.get("param_type")) {
                if let Some(pt_obj) = pt.as_object() {
                    const PT_KEYS: &[&str] = &["type", "min", "max", "step", "choices", "options"];
                    for k in pt_obj.keys() {
                        if !PT_KEYS.contains(&k.as_str()) {
                            return Err(format!(
                                "Unknown field '{}' in parameters[{}].paramType",
                                k, i
                            ));
                        }
                    }
                }
            }
            if let Some(dv) = p_obj
                .get("defaultValue")
                .or_else(|| p_obj.get("default_value"))
            {
                if let Some(dv_obj) = dv.as_object() {
                    const DV_KEYS: &[&str] = &["kind", "value"];
                    for k in dv_obj.keys() {
                        if !DV_KEYS.contains(&k.as_str()) {
                            return Err(format!(
                                "Unknown field '{}' in parameters[{}].defaultValue",
                                k, i
                            ));
                        }
                    }
                }
            }
        }
    }

    if let Some(bindings) = obj.get("bindings").and_then(|v| v.as_array()) {
        const BIND_KEYS: &[&str] = &[
            "parameterKey",
            "parameter_key",
            "targetLayerId",
            "target_layer_id",
            "targetProperty",
            "target_property",
        ];
        for (i, b) in bindings.iter().enumerate() {
            let b_obj = b
                .as_object()
                .ok_or_else(|| format!("bindings[{i}] must be an object"))?;
            for k in b_obj.keys() {
                if !BIND_KEYS.contains(&k.as_str()) {
                    return Err(format!("Unknown field '{}' in bindings[{i}]", k));
                }
            }
        }
    }

    if let Some(overrides) = obj
        .get("userOverrides")
        .or_else(|| obj.get("user_overrides"))
        .and_then(|v| v.as_array())
    {
        const OVR_KEYS: &[&str] = &["targetLayerId", "target_layer_id", "property", "value"];
        for (i, ovr) in overrides.iter().enumerate() {
            let o_obj = ovr
                .as_object()
                .ok_or_else(|| format!("userOverrides[{i}] must be an object"))?;
            for k in o_obj.keys() {
                if !OVR_KEYS.contains(&k.as_str()) {
                    return Err(format!("Unknown field '{}' in userOverrides[{i}]", k));
                }
            }
            if let Some(val) = o_obj.get("value") {
                if let Some(v_obj) = val.as_object() {
                    const VAL_KEYS: &[&str] = &["kind", "value"];
                    for k in v_obj.keys() {
                        if !VAL_KEYS.contains(&k.as_str()) {
                            return Err(format!(
                                "Unknown field '{}' in userOverrides[{}].value",
                                k, i
                            ));
                        }
                    }
                }
            }
        }
    }

    if let Some(res) = obj.get("resources").and_then(|v| v.as_object()) {
        const RES_CONTAINER_KEYS: &[&str] = &["resources"];
        for k in res.keys() {
            if !RES_CONTAINER_KEYS.contains(&k.as_str()) {
                return Err(format!("Unknown field '{}' in resources", k));
            }
        }
        if let Some(res_arr) = res.get("resources").and_then(|v| v.as_array()) {
            const RES_KEYS: &[&str] = &[
                "resourceId",
                "resource_id",
                "kind",
                "fileName",
                "file_name",
                "sha256",
                "sizeBytes",
                "size_bytes",
                "required",
                "license",
            ];
            for (i, r) in res_arr.iter().enumerate() {
                let r_obj = r
                    .as_object()
                    .ok_or_else(|| format!("resources.resources[{i}] must be an object"))?;
                for k in r_obj.keys() {
                    if !RES_KEYS.contains(&k.as_str()) {
                        return Err(format!("Unknown field '{}' in resources.resources[{i}]", k));
                    }
                }
            }
        }
    }

    if let Some(rp) = obj
        .get("responsivePolicy")
        .or_else(|| obj.get("responsive_policy"))
        .and_then(|v| v.as_object())
    {
        const RP_KEYS: &[&str] = &[
            "referenceWidth",
            "reference_width",
            "referenceHeight",
            "reference_height",
            "supportedAspectRatios",
            "supported_aspect_ratios",
            "autoWrap",
            "auto_wrap",
            "autoFit",
            "auto_fit",
            "safeMargin",
            "safe_margin",
        ];
        for k in rp.keys() {
            if !RP_KEYS.contains(&k.as_str()) {
                return Err(format!("Unknown field '{}' in responsivePolicy", k));
            }
        }
    }

    if let Some(dd) = obj
        .get("defaultDuration")
        .or_else(|| obj.get("default_duration"))
        .and_then(|v| v.as_object())
    {
        const DD_KEYS: &[&str] = &["value", "scale", "rate"];
        for k in dd.keys() {
            if !DD_KEYS.contains(&k.as_str()) {
                return Err(format!("Unknown field '{}' in defaultDuration", k));
            }
        }
        if let Some(rate) = dd.get("rate").and_then(|v| v.as_object()) {
            const RATE_KEYS: &[&str] = &["numerator", "denominator"];
            for k in rate.keys() {
                if !RATE_KEYS.contains(&k.as_str()) {
                    return Err(format!("Unknown field '{}' in defaultDuration.rate", k));
                }
            }
        }
    }

    let layers = obj
        .get("layers")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "TemplateDraft must contain 'layers' array".to_string())?;

    const LAYER_KEYS: &[&str] = &[
        "layerId",
        "layer_id",
        "name",
        "zIndex",
        "z_index",
        "visible",
        "opacity",
        "transform",
        "anchor",
        "pivot",
        "motion",
        "kind",
    ];

    for (l_idx, layer) in layers.iter().enumerate() {
        let l_obj = layer
            .as_object()
            .ok_or_else(|| format!("layers[{l_idx}] must be an object"))?;
        for k in l_obj.keys() {
            if !LAYER_KEYS.contains(&k.as_str()) {
                return Err(format!("Unknown field '{}' in layers[{l_idx}]", k));
            }
        }

        // Transform
        if let Some(tr) = l_obj.get("transform") {
            let tr_obj = tr
                .as_object()
                .ok_or_else(|| format!("layers[{l_idx}].transform must be an object"))?;
            const TR_KEYS: &[&str] = &[
                "positionX",
                "position_x",
                "positionY",
                "position_y",
                "scaleX",
                "scale_x",
                "scaleY",
                "scale_y",
                "rotation",
            ];
            for k in tr_obj.keys() {
                if !TR_KEYS.contains(&k.as_str()) {
                    return Err(format!("Unknown field '{}' in transform", k));
                }
            }
        }

        // Motion
        if let Some(motion) = l_obj.get("motion") {
            if !motion.is_null() {
                let m_obj = motion
                    .as_object()
                    .ok_or_else(|| format!("layers[{l_idx}].motion must be an object"))?;
                const MOTION_KEYS: &[&str] = &[
                    "unit",
                    "order",
                    "seed",
                    "pivotX",
                    "pivot_x",
                    "pivotY",
                    "pivot_y",
                    "inAnim",
                    "in_anim",
                    "loopAnim",
                    "loop_anim",
                    "outAnim",
                    "out_anim",
                    "loopMode",
                    "loop_mode",
                    "conflictPolicy",
                    "conflict_policy",
                    "phaseOrigin",
                    "phase_origin",
                    "phaseEnd",
                    "phase_end",
                ];
                for k in m_obj.keys() {
                    if !MOTION_KEYS.contains(&k.as_str()) {
                        return Err(format!("Unknown field '{}' in motion", k));
                    }
                }
                for anim_key in &[
                    "inAnim",
                    "in_anim",
                    "loopAnim",
                    "loop_anim",
                    "outAnim",
                    "out_anim",
                ] {
                    if let Some(anim) = m_obj.get(*anim_key) {
                        if !anim.is_null() {
                            let a_obj = anim.as_object().ok_or_else(|| {
                                format!("layers[{l_idx}].motion.{} must be an object", anim_key)
                            })?;
                            const SEG_KEYS: &[&str] = &[
                                "preset",
                                "duration",
                                "easing",
                                "stagger_ratio",
                                "staggerRatio",
                                "from",
                                "to",
                                "unit",
                                "staggerMs",
                                "stagger_ms",
                                "staggerOrder",
                                "stagger_order",
                                "loopMode",
                                "loop_mode",
                                "conflictPolicy",
                                "conflict_policy",
                                "keyframes",
                                "customCubic",
                                "custom_cubic",
                            ];
                            for k in a_obj.keys() {
                                if !SEG_KEYS.contains(&k.as_str()) {
                                    return Err(format!("Unknown field '{}' in motion segment", k));
                                }
                            }
                            for ft in &["from", "to"] {
                                if let Some(from_to) = a_obj.get(*ft) {
                                    if !from_to.is_null() {
                                        let ft_obj = from_to
                                            .as_object()
                                            .ok_or("from/to must be an object")?;
                                        const FT_KEYS: &[&str] = &[
                                            "translate_x",
                                            "translateX",
                                            "translate_y",
                                            "translateY",
                                            "scale_x",
                                            "scaleX",
                                            "scale_y",
                                            "scaleY",
                                            "rotation",
                                            "opacity",
                                        ];
                                        for k in ft_obj.keys() {
                                            if !FT_KEYS.contains(&k.as_str()) {
                                                return Err(format!(
                                                    "Unknown field '{}' in motion from/to",
                                                    k
                                                ));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Kind
        let kind_val = l_obj
            .get("kind")
            .ok_or_else(|| format!("layers[{l_idx}] missing 'kind'"))?;
        let k_obj = kind_val
            .as_object()
            .ok_or_else(|| format!("layers[{l_idx}].kind must be an object"))?;

        if let Some(t_val) = k_obj.get("type").and_then(|v| v.as_str()) {
            const KIND_CONTAINER_KEYS: &[&str] = &["type", "data"];
            for k in k_obj.keys() {
                if !KIND_CONTAINER_KEYS.contains(&k.as_str()) {
                    return Err(format!("Unknown field '{}' in kind", k));
                }
            }
            let data = k_obj
                .get("data")
                .ok_or_else(|| format!("layers[{l_idx}].kind missing 'data'"))?;
            let d_obj = data
                .as_object()
                .ok_or_else(|| format!("layers[{l_idx}].kind.data must be an object"))?;
            match t_val {
                "Text" => validate_text_layer_data(d_obj, l_idx)?,
                "Shape" => validate_shape_layer_data(d_obj, l_idx)?,
                "Image" => validate_image_layer_data(d_obj, l_idx)?,
                other => return Err(format!("Unknown layer type '{}'", other)),
            }
        } else if let Some(t_data) = k_obj.get("Text") {
            let d_obj = t_data
                .as_object()
                .ok_or_else(|| format!("layers[{l_idx}].kind.Text must be an object"))?;
            validate_text_layer_data(d_obj, l_idx)?;
        } else if let Some(s_data) = k_obj.get("Shape") {
            let d_obj = s_data
                .as_object()
                .ok_or_else(|| format!("layers[{l_idx}].kind.Shape must be an object"))?;
            validate_shape_layer_data(d_obj, l_idx)?;
        } else if let Some(i_data) = k_obj.get("Image") {
            let d_obj = i_data
                .as_object()
                .ok_or_else(|| format!("layers[{l_idx}].kind.Image must be an object"))?;
            validate_image_layer_data(d_obj, l_idx)?;
        } else {
            return Err(format!("Invalid layer kind structure in layer {}", l_idx));
        }
    }

    Ok(())
}

fn validate_text_layer_data(
    d_obj: &serde_json::Map<String, serde_json::Value>,
    l_idx: usize,
) -> Result<(), String> {
    const TEXT_DATA_KEYS: &[&str] = &[
        "isPrimary",
        "is_primary",
        "slotKey",
        "slot_key",
        "textContent",
        "text_content",
        "textStyle",
        "text_style",
        "textRich",
        "text_rich",
        "textAnimation",
        "text_animation",
    ];
    for k in d_obj.keys() {
        if !TEXT_DATA_KEYS.contains(&k.as_str()) {
            return Err(format!("Unknown field '{}' in layer[{l_idx}].kind.data", k));
        }
    }

    // textStyle
    if let Some(ts) = d_obj.get("textStyle").or_else(|| d_obj.get("text_style")) {
        if !ts.is_null() {
            let ts_obj = ts
                .as_object()
                .ok_or_else(|| format!("text_style in layer {l_idx} must be an object"))?;
            const TS_KEYS: &[&str] = &[
                "fontFamily",
                "font_family",
                "fontSize",
                "font_size",
                "fontWeight",
                "font_weight",
                "fontColor",
                "font_color",
                "accentColor",
                "accent_color",
                "backgroundColor",
                "background_color",
                "backgroundOpacity",
                "background_opacity",
                "paddingX",
                "padding_x",
                "paddingY",
                "padding_y",
                "alignment",
                "lineHeight",
                "line_height",
                "anchor",
            ];
            for k in ts_obj.keys() {
                if !TS_KEYS.contains(&k.as_str()) {
                    return Err(format!("Unknown field '{}' in text_style", k));
                }
            }
        }
    }

    // textRich
    if let Some(tr) = d_obj.get("textRich").or_else(|| d_obj.get("text_rich")) {
        if !tr.is_null() {
            let tr_obj = tr
                .as_object()
                .ok_or_else(|| format!("text_rich in layer {l_idx} must be an object"))?;
            const TR_KEYS: &[&str] = &[
                "font",
                "fontSize",
                "font_size",
                "layout",
                "fill",
                "strokes",
                "shadows",
                "background",
                "spans",
            ];
            for k in tr_obj.keys() {
                if !TR_KEYS.contains(&k.as_str()) {
                    return Err(format!("Unknown field '{}' in text_rich", k));
                }
            }

            // font
            if let Some(font) = tr_obj.get("font") {
                if !font.is_null() {
                    let f_obj = font.as_object().ok_or("text_rich.font must be an object")?;
                    const F_KEYS: &[&str] = &["family", "weight", "style", "assetId", "asset_id"];
                    for k in f_obj.keys() {
                        if !F_KEYS.contains(&k.as_str()) {
                            return Err(format!("Unknown field '{}' in text_rich.font", k));
                        }
                    }
                }
            }

            // layout
            if let Some(layout) = tr_obj.get("layout") {
                if !layout.is_null() {
                    let lay_obj = layout
                        .as_object()
                        .ok_or("text_rich.layout must be an object")?;
                    const LAY_KEYS: &[&str] = &[
                        "maxWidth",
                        "max_width",
                        "alignment",
                        "lineHeight",
                        "line_height",
                        "letterSpacing",
                        "letter_spacing",
                        "wrapMode",
                        "wrap_mode",
                        "minFontSize",
                        "min_font_size",
                        "maxFontSize",
                        "max_font_size",
                    ];
                    for k in lay_obj.keys() {
                        if !LAY_KEYS.contains(&k.as_str()) {
                            return Err(format!("Unknown field '{}' in text_rich.layout", k));
                        }
                    }
                }
            }

            // fill
            if let Some(fill) = tr_obj.get("fill") {
                validate_raw_fill_value(fill, "text_rich.fill")?;
            }

            // strokes
            if let Some(strokes) = tr_obj.get("strokes").and_then(|v| v.as_array()) {
                const STROKE_KEYS: &[&str] = &["color", "width", "align"];
                for (si, st) in strokes.iter().enumerate() {
                    let st_obj = st
                        .as_object()
                        .ok_or_else(|| format!("strokes[{si}] must be an object"))?;
                    for k in st_obj.keys() {
                        if !STROKE_KEYS.contains(&k.as_str()) {
                            return Err(format!("Unknown field '{}' in strokes[{si}]", k));
                        }
                    }
                }
            }

            // shadows
            if let Some(shadows) = tr_obj.get("shadows").and_then(|v| v.as_array()) {
                const SHADOW_KEYS: &[&str] = &[
                    "color", "offsetX", "offset_x", "offsetY", "offset_y", "blur",
                ];
                for (si, sh) in shadows.iter().enumerate() {
                    let sh_obj = sh
                        .as_object()
                        .ok_or_else(|| format!("shadows[{si}] must be an object"))?;
                    for k in sh_obj.keys() {
                        if !SHADOW_KEYS.contains(&k.as_str()) {
                            return Err(format!("Unknown field '{}' in shadows[{si}]", k));
                        }
                    }
                }
            }

            // background
            if let Some(bg) = tr_obj.get("background") {
                if !bg.is_null() {
                    let bg_obj = bg
                        .as_object()
                        .ok_or("text_rich.background must be an object")?;
                    const BG_KEYS: &[&str] = &[
                        "fill",
                        "paddingX",
                        "padding_x",
                        "paddingY",
                        "padding_y",
                        "cornerRadius",
                        "corner_radius",
                        "opacity",
                    ];
                    for k in bg_obj.keys() {
                        if !BG_KEYS.contains(&k.as_str()) {
                            return Err(format!("Unknown field '{}' in text_rich.background", k));
                        }
                    }
                    if let Some(bg_fill) = bg_obj.get("fill") {
                        validate_raw_fill_value(bg_fill, "text_rich.background.fill")?;
                    }
                }
            }

            // spans
            if let Some(spans) = tr_obj.get("spans").and_then(|v| v.as_array()) {
                const SPAN_KEYS: &[&str] = &[
                    "start",
                    "end",
                    "style",
                    "font",
                    "fontSize",
                    "font_size",
                    "fill",
                    "strokes",
                    "shadows",
                    "background",
                    "letterSpacing",
                    "letter_spacing",
                ];
                for (si, sp) in spans.iter().enumerate() {
                    let sp_obj = sp
                        .as_object()
                        .ok_or_else(|| format!("spans[{si}] must be an object"))?;
                    for k in sp_obj.keys() {
                        if !SPAN_KEYS.contains(&k.as_str()) {
                            return Err(format!("Unknown field '{}' in spans[{si}]", k));
                        }
                    }
                    if let Some(style) = sp_obj.get("style") {
                        if !style.is_null() {
                            let st_obj = style
                                .as_object()
                                .ok_or_else(|| format!("spans[{si}].style must be an object"))?;
                            const STYLE_KEYS: &[&str] = &[
                                "fill",
                                "font",
                                "fontSize",
                                "font_size",
                                "letterSpacing",
                                "letter_spacing",
                            ];
                            for k in st_obj.keys() {
                                if !STYLE_KEYS.contains(&k.as_str()) {
                                    return Err(format!(
                                        "Unknown field '{}' in spans[{si}].style",
                                        k
                                    ));
                                }
                            }
                            if let Some(s_fill) = st_obj.get("fill") {
                                validate_raw_fill_value(
                                    s_fill,
                                    &format!("spans[{si}].style.fill"),
                                )?;
                            }
                            if let Some(font) = st_obj.get("font") {
                                if !font.is_null() {
                                    let f_obj = font.as_object().ok_or_else(|| {
                                        format!("spans[{si}].style.font must be an object")
                                    })?;
                                    const F_KEYS: &[&str] =
                                        &["family", "weight", "style", "assetId", "asset_id"];
                                    for k in f_obj.keys() {
                                        if !F_KEYS.contains(&k.as_str()) {
                                            return Err(format!(
                                                "Unknown field '{}' in spans[{si}].style.font",
                                                k
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if let Some(s_fill) = sp_obj.get("fill") {
                        validate_raw_fill_value(s_fill, &format!("spans[{si}].fill"))?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn validate_shape_layer_data(
    d_obj: &serde_json::Map<String, serde_json::Value>,
    l_idx: usize,
) -> Result<(), String> {
    const SHAPE_KEYS: &[&str] = &[
        "shapeKind",
        "shape_kind",
        "width",
        "height",
        "fillColor",
        "fill_color",
        "strokeColor",
        "stroke_color",
        "strokeWidth",
        "stroke_width",
        "cornerRadius",
        "corner_radius",
    ];
    for k in d_obj.keys() {
        if !SHAPE_KEYS.contains(&k.as_str()) {
            return Err(format!("Unknown field '{}' in layer[{l_idx}].kind.data", k));
        }
    }
    Ok(())
}

fn validate_image_layer_data(
    d_obj: &serde_json::Map<String, serde_json::Value>,
    l_idx: usize,
) -> Result<(), String> {
    const IMAGE_KEYS: &[&str] = &[
        "assetId",
        "asset_id",
        "assetName",
        "asset_name",
        "assetHash",
        "asset_hash",
        "width",
        "height",
        "placementMode",
        "placement_mode",
    ];
    for k in d_obj.keys() {
        if !IMAGE_KEYS.contains(&k.as_str()) {
            return Err(format!("Unknown field '{}' in layer[{l_idx}].kind.data", k));
        }
    }
    Ok(())
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
            if let Some(binding) = self.bindings.iter().find(|b| {
                b.target_layer_id == ovr.target_layer_id && b.target_property == ovr.property
            }) {
                inst.parameters
                    .insert(binding.parameter_key.clone(), ovr.value.clone());
            }
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

/// An owned temporary preview artifact that cleans itself up on Drop unless explicitly disarmed.
#[derive(Debug)]
pub struct TempPreviewArtifact {
    path: std::path::PathBuf,
    disarmed: bool,
}

impl TempPreviewArtifact {
    pub fn new(path: std::path::PathBuf) -> Self {
        Self {
            path,
            disarmed: false,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn disarm(&mut self) {
        self.disarmed = true;
    }
}

impl Drop for TempPreviewArtifact {
    fn drop(&mut self) {
        if !self.disarmed && self.path.exists() {
            let _ = fs::remove_file(&self.path);
        }
    }
}

impl AsRef<Path> for TempPreviewArtifact {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

pub type TestPathHook = Option<std::sync::Arc<dyn Fn(&Path) + Send + Sync + 'static>>;

static WORKER_EXECUTION_HOOK: std::sync::RwLock<TestPathHook> = std::sync::RwLock::new(None);

static PRE_DELIVERY_HOOK: std::sync::RwLock<TestPathHook> = std::sync::RwLock::new(None);

pub struct AgentTemplateService;

impl AgentTemplateService {
    pub fn set_worker_execution_hook(hook: TestPathHook) {
        if let Ok(mut w) = WORKER_EXECUTION_HOOK.write() {
            *w = hook;
        }
    }

    pub fn set_pre_delivery_hook(hook: TestPathHook) {
        if let Ok(mut w) = PRE_DELIVERY_HOOK.write() {
            *w = hook;
        }
    }
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

    /// Recursively validate the raw JSON tree before typed deserialization.
    /// Rejects unknown keys anywhere (e.g. transform.unrecognized, text_rich.script),
    /// schemaVersion != 1, payload oversized, and script/shell tokens.
    pub fn validate_raw_json_tree(val: &serde_json::Value, issues: &mut Vec<ValidationIssue>) {
        fn check_allowed(
            obj: &serde_json::Map<String, serde_json::Value>,
            allowed: &[&str],
            path: &str,
            issues: &mut Vec<ValidationIssue>,
        ) {
            for k in obj.keys() {
                if !allowed.contains(&k.as_str()) {
                    issues.push(ValidationIssue {
                        severity: ValidationSeverity::Error,
                        path: if path.is_empty() {
                            k.clone()
                        } else {
                            format!("{}.{}", path, k)
                        },
                        code: "UNKNOWN_FIELD".to_string(),
                        message: format!(
                            "Unknown field '{}' at {}",
                            k,
                            if path.is_empty() { "root" } else { path }
                        ),
                    });
                }
            }
        }

        let root_obj = match val.as_object() {
            Some(obj) => obj,
            None => {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    path: "root".to_string(),
                    code: "INVALID_JSON_OBJECT".to_string(),
                    message: "Template draft must be a JSON object".to_string(),
                });
                return;
            }
        };

        check_allowed(
            root_obj,
            &[
                "draftId",
                "draft_id",
                "schemaVersion",
                "schema_version",
                "templateId",
                "template_id",
                "templateVersion",
                "template_version",
                "name",
                "category",
                "description",
                "tags",
                "slots",
                "parameters",
                "bindings",
                "layers",
                "resources",
                "responsivePolicy",
                "responsive_policy",
                "defaultDuration",
                "default_duration",
                "provenance",
                "userOverrides",
                "user_overrides",
            ],
            "",
            issues,
        );

        // Schema version check on raw JSON
        if let Some(sv) = root_obj
            .get("schemaVersion")
            .or_else(|| root_obj.get("schema_version"))
        {
            if sv.as_u64() != Some(CURRENT_TEMPLATE_DRAFT_SCHEMA_VERSION as u64) {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    path: "schemaVersion".to_string(),
                    code: "UNSUPPORTED_SCHEMA_VERSION".to_string(),
                    message: format!(
                        "Unsupported template draft schemaVersion {:?}; expected {}",
                        sv, CURRENT_TEMPLATE_DRAFT_SCHEMA_VERSION
                    ),
                });
            }
        } else {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Error,
                path: "schemaVersion".to_string(),
                code: "MISSING_SCHEMA_VERSION".to_string(),
                message: "Missing schemaVersion in template draft".to_string(),
            });
        }

        // Validate layers
        if let Some(layers) = root_obj.get("layers").and_then(|l| l.as_array()) {
            for (idx, layer) in layers.iter().enumerate() {
                let layer_path = format!("layers[{}]", idx);
                if let Some(layer_obj) = layer.as_object() {
                    check_allowed(
                        layer_obj,
                        &[
                            "layerId",
                            "layer_id",
                            "name",
                            "zIndex",
                            "z_index",
                            "visible",
                            "opacity",
                            "transform",
                            "anchor",
                            "pivot",
                            "motion",
                            "kind",
                        ],
                        &layer_path,
                        issues,
                    );

                    // Transform
                    if let Some(t_obj) = layer_obj.get("transform").and_then(|t| t.as_object()) {
                        let t_path = format!("{}.transform", layer_path);
                        check_allowed(
                            t_obj,
                            &[
                                "positionX",
                                "position_x",
                                "positionY",
                                "position_y",
                                "scaleX",
                                "scale_x",
                                "scaleY",
                                "scale_y",
                                "rotation",
                            ],
                            &t_path,
                            issues,
                        );
                    }

                    // Motion
                    if let Some(m_obj) = layer_obj.get("motion").and_then(|m| m.as_object()) {
                        let m_path = format!("{}.motion", layer_path);
                        check_allowed(
                            m_obj,
                            &[
                                "unit",
                                "order",
                                "seed",
                                "pivotX",
                                "pivot_x",
                                "pivotY",
                                "pivot_y",
                                "inAnim",
                                "in_anim",
                                "loopAnim",
                                "loop_anim",
                                "outAnim",
                                "out_anim",
                                "loopMode",
                                "loop_mode",
                                "conflictPolicy",
                                "conflict_policy",
                                "phaseOrigin",
                                "phase_origin",
                                "phaseEnd",
                                "phase_end",
                            ],
                            &m_path,
                            issues,
                        );
                    }

                    // Kind
                    if let Some(k_obj) = layer_obj.get("kind").and_then(|k| k.as_object()) {
                        let k_path = format!("{}.kind", layer_path);
                        check_allowed(k_obj, &["type", "data"], &k_path, issues);
                        if let Some(data_obj) = k_obj.get("data").and_then(|d| d.as_object()) {
                            let d_path = format!("{}.data", k_path);
                            let kind_type =
                                k_obj.get("type").and_then(|t| t.as_str()).unwrap_or("");
                            if kind_type.eq_ignore_ascii_case("text") {
                                check_allowed(
                                    data_obj,
                                    &[
                                        "isPrimary",
                                        "is_primary",
                                        "slotKey",
                                        "slot_key",
                                        "textContent",
                                        "text_content",
                                        "textStyle",
                                        "text_style",
                                        "textRich",
                                        "text_rich",
                                        "textAnimation",
                                        "text_animation",
                                    ],
                                    &d_path,
                                    issues,
                                );
                                // TextRich
                                if let Some(rich_obj) = data_obj
                                    .get("textRich")
                                    .or_else(|| data_obj.get("text_rich"))
                                    .and_then(|r| r.as_object())
                                {
                                    let rich_path = format!("{}.textRich", d_path);
                                    check_allowed(
                                        rich_obj,
                                        &[
                                            "font",
                                            "fontSize",
                                            "font_size",
                                            "layout",
                                            "fill",
                                            "strokes",
                                            "shadows",
                                            "background",
                                            "spans",
                                            "style",
                                        ],
                                        &rich_path,
                                        issues,
                                    );
                                    if let Some(fill) = rich_obj.get("fill") {
                                        if let Err(e) = validate_raw_fill_value(
                                            fill,
                                            &format!("{}.fill", rich_path),
                                        ) {
                                            issues.push(ValidationIssue {
                                                severity: ValidationSeverity::Error,
                                                path: format!("{}.fill", rich_path),
                                                code: "UNKNOWN_FIELD".to_string(),
                                                message: e,
                                            });
                                        }
                                    }
                                    if let Some(bg_obj) =
                                        rich_obj.get("background").and_then(|b| b.as_object())
                                    {
                                        let bg_path = format!("{}.background", rich_path);
                                        check_allowed(
                                            bg_obj,
                                            &[
                                                "fill",
                                                "paddingX",
                                                "padding_x",
                                                "paddingY",
                                                "padding_y",
                                                "cornerRadius",
                                                "corner_radius",
                                                "opacity",
                                            ],
                                            &bg_path,
                                            issues,
                                        );
                                        if let Some(bg_fill) = bg_obj.get("fill") {
                                            if let Err(e) = validate_raw_fill_value(
                                                bg_fill,
                                                &format!("{}.fill", bg_path),
                                            ) {
                                                issues.push(ValidationIssue {
                                                    severity: ValidationSeverity::Error,
                                                    path: format!("{}.fill", bg_path),
                                                    code: "UNKNOWN_FIELD".to_string(),
                                                    message: e,
                                                });
                                            }
                                        }
                                    }
                                    if let Some(spans_arr) =
                                        rich_obj.get("spans").and_then(|s| s.as_array())
                                    {
                                        for (si, sp) in spans_arr.iter().enumerate() {
                                            let sp_path = format!("{}.spans[{}]", rich_path, si);
                                            if let Some(sp_obj) = sp.as_object() {
                                                check_allowed(
                                                    sp_obj,
                                                    &[
                                                        "start",
                                                        "end",
                                                        "style",
                                                        "font",
                                                        "fontSize",
                                                        "font_size",
                                                        "fill",
                                                        "strokes",
                                                        "shadows",
                                                        "background",
                                                        "letterSpacing",
                                                        "letter_spacing",
                                                    ],
                                                    &sp_path,
                                                    issues,
                                                );
                                                if let Some(style_obj) = sp_obj
                                                    .get("style")
                                                    .and_then(|st| st.as_object())
                                                {
                                                    let st_path = format!("{}.style", sp_path);
                                                    check_allowed(
                                                        style_obj,
                                                        &[
                                                            "fill",
                                                            "font",
                                                            "fontSize",
                                                            "font_size",
                                                            "letterSpacing",
                                                            "letter_spacing",
                                                        ],
                                                        &st_path,
                                                        issues,
                                                    );
                                                    if let Some(s_fill) = style_obj.get("fill") {
                                                        if let Err(e) = validate_raw_fill_value(
                                                            s_fill,
                                                            &format!("{}.fill", st_path),
                                                        ) {
                                                            issues.push(ValidationIssue {
                                                                severity: ValidationSeverity::Error,
                                                                path: format!("{}.fill", st_path),
                                                                code: "UNKNOWN_FIELD".to_string(),
                                                                message: e,
                                                            });
                                                        }
                                                    }
                                                    if let Some(f_obj) = style_obj
                                                        .get("font")
                                                        .and_then(|f| f.as_object())
                                                    {
                                                        check_allowed(
                                                            f_obj,
                                                            &[
                                                                "family", "weight", "style",
                                                                "assetId", "asset_id",
                                                            ],
                                                            &format!("{}.font", st_path),
                                                            issues,
                                                        );
                                                    }
                                                }
                                                if let Some(s_fill) = sp_obj.get("fill") {
                                                    if let Err(e) = validate_raw_fill_value(
                                                        s_fill,
                                                        &format!("{}.fill", sp_path),
                                                    ) {
                                                        issues.push(ValidationIssue {
                                                            severity: ValidationSeverity::Error,
                                                            path: format!("{}.fill", sp_path),
                                                            code: "UNKNOWN_FIELD".to_string(),
                                                            message: e,
                                                        });
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                // TextStyle
                                if let Some(style_obj) = data_obj
                                    .get("textStyle")
                                    .or_else(|| data_obj.get("text_style"))
                                    .and_then(|s| s.as_object())
                                {
                                    let style_path = format!("{}.textStyle", d_path);
                                    check_allowed(
                                        style_obj,
                                        &[
                                            "fontFamily",
                                            "font_family",
                                            "fontSize",
                                            "font_size",
                                            "fontWeight",
                                            "font_weight",
                                            "fontColor",
                                            "font_color",
                                            "accentColor",
                                            "accent_color",
                                            "backgroundColor",
                                            "background_color",
                                            "backgroundOpacity",
                                            "background_opacity",
                                            "paddingX",
                                            "padding_x",
                                            "paddingY",
                                            "padding_y",
                                            "alignment",
                                            "lineHeight",
                                            "line_height",
                                            "anchor",
                                        ],
                                        &style_path,
                                        issues,
                                    );
                                }
                                // TextAnimation
                                if let Some(anim_obj) = data_obj
                                    .get("textAnimation")
                                    .or_else(|| data_obj.get("text_animation"))
                                    .and_then(|a| a.as_object())
                                {
                                    let anim_path = format!("{}.textAnimation", d_path);
                                    check_allowed(
                                        anim_obj,
                                        &[
                                            "unit",
                                            "order",
                                            "seed",
                                            "pivotX",
                                            "pivot_x",
                                            "pivotY",
                                            "pivot_y",
                                            "inAnim",
                                            "in_anim",
                                            "loopAnim",
                                            "loop_anim",
                                            "outAnim",
                                            "out_anim",
                                            "loopMode",
                                            "loop_mode",
                                            "conflictPolicy",
                                            "conflict_policy",
                                            "phaseOrigin",
                                            "phase_origin",
                                            "phaseEnd",
                                            "phase_end",
                                        ],
                                        &anim_path,
                                        issues,
                                    );
                                }
                            } else if kind_type.eq_ignore_ascii_case("shape") {
                                check_allowed(
                                    data_obj,
                                    &[
                                        "shapeKind",
                                        "shape_kind",
                                        "width",
                                        "height",
                                        "fillColor",
                                        "fill_color",
                                        "strokeColor",
                                        "stroke_color",
                                        "strokeWidth",
                                        "stroke_width",
                                        "cornerRadius",
                                        "corner_radius",
                                    ],
                                    &d_path,
                                    issues,
                                );
                            } else if kind_type.eq_ignore_ascii_case("image") {
                                check_allowed(
                                    data_obj,
                                    &[
                                        "assetId", "asset_id", "width", "height", "fitMode",
                                        "fit_mode", "crop",
                                    ],
                                    &d_path,
                                    issues,
                                );
                            }
                        }
                    }
                }
            }
        }

        // Validate userOverrides
        if let Some(ovrs) = root_obj
            .get("userOverrides")
            .or_else(|| root_obj.get("user_overrides"))
            .and_then(|o| o.as_array())
        {
            for (idx, ovr) in ovrs.iter().enumerate() {
                let o_path = format!("userOverrides[{}]", idx);
                if let Some(ovr_obj) = ovr.as_object() {
                    check_allowed(
                        ovr_obj,
                        &["targetLayerId", "target_layer_id", "property", "value"],
                        &o_path,
                        issues,
                    );
                }
            }
        }

        // Validate provenance
        if let Some(p_obj) = root_obj.get("provenance").and_then(|p| p.as_object()) {
            check_allowed(
                p_obj,
                &[
                    "authorType",
                    "author_type",
                    "agentId",
                    "agent_id",
                    "agentVersion",
                    "agent_version",
                    "prompt",
                    "referenceSummary",
                    "reference_summary",
                    "parentDraftId",
                    "parent_draft_id",
                    "iteration",
                    "unsupportedApproximations",
                    "unsupported_approximations",
                    "requiredMissingResources",
                    "required_missing_resources",
                    "createdAt",
                    "created_at",
                ],
                "provenance",
                issues,
            );
        }

        // Validate slots
        if let Some(slots) = root_obj.get("slots").and_then(|s| s.as_array()) {
            for (idx, s) in slots.iter().enumerate() {
                let s_path = format!("slots[{}]", idx);
                if let Some(s_obj) = s.as_object() {
                    check_allowed(
                        s_obj,
                        &[
                            "slotKey",
                            "slot_key",
                            "name",
                            "slotType",
                            "slot_type",
                            "description",
                            "required",
                        ],
                        &s_path,
                        issues,
                    );
                }
            }
        }

        // Validate parameters
        if let Some(params) = root_obj.get("parameters").and_then(|p| p.as_array()) {
            for (idx, p) in params.iter().enumerate() {
                let p_path = format!("parameters[{}]", idx);
                if let Some(p_obj) = p.as_object() {
                    check_allowed(
                        p_obj,
                        &[
                            "key",
                            "name",
                            "paramType",
                            "param_type",
                            "defaultValue",
                            "default_value",
                            "description",
                            "category",
                        ],
                        &p_path,
                        issues,
                    );
                }
            }
        }

        // Validate bindings
        if let Some(bindings) = root_obj.get("bindings").and_then(|b| b.as_array()) {
            for (idx, b) in bindings.iter().enumerate() {
                let b_path = format!("bindings[{}]", idx);
                if let Some(b_obj) = b.as_object() {
                    check_allowed(
                        b_obj,
                        &[
                            "parameterKey",
                            "parameter_key",
                            "targetLayerId",
                            "target_layer_id",
                            "targetProperty",
                            "target_property",
                        ],
                        &b_path,
                        issues,
                    );
                }
            }
        }

        // Validate resources
        if let Some(res_obj) = root_obj.get("resources").and_then(|r| r.as_object()) {
            check_allowed(res_obj, &["resources"], "resources", issues);
            if let Some(res_arr) = res_obj.get("resources").and_then(|r| r.as_array()) {
                for (idx, r) in res_arr.iter().enumerate() {
                    let r_path = format!("resources.resources[{}]", idx);
                    if let Some(ro) = r.as_object() {
                        check_allowed(
                            ro,
                            &[
                                "resourceId",
                                "resource_id",
                                "kind",
                                "fileName",
                                "file_name",
                                "source",
                                "checksumSha256",
                                "checksum_sha256",
                                "required",
                                "description",
                            ],
                            &r_path,
                            issues,
                        );
                    }
                }
            }
        }

        // Validate responsivePolicy
        if let Some(rp_obj) = root_obj
            .get("responsivePolicy")
            .or_else(|| root_obj.get("responsive_policy"))
            .and_then(|r| r.as_object())
        {
            check_allowed(
                rp_obj,
                &[
                    "referenceWidth",
                    "reference_width",
                    "referenceHeight",
                    "reference_height",
                    "supportedAspectRatios",
                    "supported_aspect_ratios",
                    "autoWrap",
                    "auto_wrap",
                    "autoFit",
                    "auto_fit",
                    "safeMargin",
                    "safe_margin",
                ],
                "responsivePolicy",
                issues,
            );
        }

        // Validate defaultDuration
        if let Some(dd_obj) = root_obj
            .get("defaultDuration")
            .or_else(|| root_obj.get("default_duration"))
            .and_then(|d| d.as_object())
        {
            check_allowed(dd_obj, &["value", "rate"], "defaultDuration", issues);
        }
    }

    /// Validate a raw JSON Value directly before typed deserialization.
    /// This is the primary boundary entry point for MCP, JSONL, CLI, and Tauri.
    pub fn validate_raw_draft(
        val: &serde_json::Value,
        catalog: Option<&TemplateCatalog>,
    ) -> TemplateValidationReport {
        let raw_str = serde_json::to_string(val).unwrap_or_default();
        let mut issues = Vec::new();

        if raw_str.len() > MAX_TEMPLATE_DRAFT_JSON_BYTES {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Error,
                path: "payload".to_string(),
                code: "DRAFT_OVERSIZED".to_string(),
                message: format!(
                    "Draft payload size ({} bytes) exceeds limit of {} bytes",
                    raw_str.len(),
                    MAX_TEMPLATE_DRAFT_JSON_BYTES
                ),
            });
        }
        let mut sec_issues = Self::scan_malicious_content(&raw_str);
        issues.append(&mut sec_issues);

        Self::validate_raw_json_tree(val, &mut issues);

        match serde_json::from_value::<TemplateDraft>(val.clone()) {
            Ok(draft) => {
                let mut report = Self::validate_draft(&draft, Some(&raw_str), catalog);
                issues.append(&mut report.issues);
                issues.sort_by(|a, b| a.path.cmp(&b.path));
                issues.dedup_by(|a, b| a.path == b.path && a.code == b.code);
                let valid = !issues
                    .iter()
                    .any(|i| i.severity == ValidationSeverity::Error);
                report.valid = valid;
                report.issues = issues;
                if !valid {
                    report.normalized_draft = None;
                }
                report
            }
            Err(err) => {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    path: "draft".to_string(),
                    code: "UNKNOWN_FIELD".to_string(),
                    message: format!("Failed to parse TemplateDraft: {err}"),
                });
                TemplateValidationReport {
                    valid: false,
                    schema_version: CURRENT_TEMPLATE_DRAFT_SCHEMA_VERSION,
                    issues,
                    missing_resources: Vec::new(),
                    unsupported_options: Vec::new(),
                    normalized_draft: None,
                }
            }
        }
    }

    /// Run raw JSON tree validation directly and return a TemplateValidationReport.
    pub fn validate_raw_json_tree_report(val: &serde_json::Value) -> TemplateValidationReport {
        let mut issues = Vec::new();
        Self::validate_raw_json_tree(val, &mut issues);
        let valid = !issues
            .iter()
            .any(|i| i.severity == ValidationSeverity::Error);
        TemplateValidationReport {
            valid,
            schema_version: CURRENT_TEMPLATE_DRAFT_SCHEMA_VERSION,
            issues,
            missing_resources: Vec::new(),
            unsupported_options: Vec::new(),
            normalized_draft: None,
        }
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
        let serialized_raw;
        let effective_raw = match raw_json {
            Some(r) => r,
            None => {
                serialized_raw = serde_json::to_string(draft).unwrap_or_default();
                &serialized_raw
            }
        };

        if effective_raw.len() > MAX_TEMPLATE_DRAFT_JSON_BYTES {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Error,
                path: "payload".to_string(),
                code: "DRAFT_OVERSIZED".to_string(),
                message: format!(
                    "Draft payload size ({} bytes) exceeds limit of {} bytes",
                    effective_raw.len(),
                    MAX_TEMPLATE_DRAFT_JSON_BYTES
                ),
            });
        }
        let mut sec_issues = Self::scan_malicious_content(effective_raw);
        issues.append(&mut sec_issues);

        if let Ok(raw_val) = serde_json::from_str::<serde_json::Value>(effective_raw) {
            Self::validate_raw_json_tree(&raw_val, &mut issues);
        }

        // 2. Identification checks
        if draft.schema_version != CURRENT_TEMPLATE_DRAFT_SCHEMA_VERSION {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Error,
                path: "schemaVersion".to_string(),
                code: "UNSUPPORTED_SCHEMA_VERSION".to_string(),
                message: format!(
                    "Unsupported template draft schemaVersion {}; expected {}",
                    draft.schema_version, CURRENT_TEMPLATE_DRAFT_SCHEMA_VERSION
                ),
            });
        }
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

        // 8. Validate user overrides
        for (o_idx, ovr) in draft.user_overrides.iter().enumerate() {
            if !layer_ids.contains(ovr.target_layer_id.as_str()) {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    path: format!("userOverrides[{}].targetLayerId", o_idx),
                    code: "UNKNOWN_OVERRIDE_TARGET_LAYER".to_string(),
                    message: format!(
                        "Override references unknown target layer '{}'",
                        ovr.target_layer_id
                    ),
                });
            }
            let compatible = matches!(
                (ovr.property, &ovr.value),
                (
                    TemplatePropertyPath::TextContent | TemplatePropertyPath::FontFamily,
                    TemplateParameterValue::Text(_),
                ) | (
                    TemplatePropertyPath::FontColor
                        | TemplatePropertyPath::BackgroundColor
                        | TemplatePropertyPath::FillColor
                        | TemplatePropertyPath::StrokeColor,
                    TemplateParameterValue::Color(_),
                ) | (
                    TemplatePropertyPath::FontSize
                        | TemplatePropertyPath::PositionX
                        | TemplatePropertyPath::PositionY
                        | TemplatePropertyPath::ScaleX
                        | TemplatePropertyPath::ScaleY
                        | TemplatePropertyPath::Rotation
                        | TemplatePropertyPath::Opacity
                        | TemplatePropertyPath::StrokeWidth
                        | TemplatePropertyPath::CornerRadius
                        | TemplatePropertyPath::Width
                        | TemplatePropertyPath::Height,
                    TemplateParameterValue::Number(_),
                ) | (
                    TemplatePropertyPath::Visible,
                    TemplateParameterValue::Boolean(_)
                )
            );
            if !compatible {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    path: format!("userOverrides[{}].value", o_idx),
                    code: "INCOMPATIBLE_OVERRIDE_VALUE".to_string(),
                    message: format!(
                        "Property '{:?}' is not compatible with override value {:?}",
                        ovr.property, ovr.value
                    ),
                });
            }
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
        Self::preview_draft_bounded(
            draft,
            time_seconds,
            width,
            height,
            Some(DEFAULT_PREVIEW_TIMEOUT),
            None,
        )
    }

    /// Render a bounded, cancellable preview frame overlay with timeout and dimension clamping.
    pub fn preview_draft_bounded(
        draft: &TemplateDraft,
        time_seconds: f64,
        width: Option<u32>,
        height: Option<u32>,
        timeout: Option<std::time::Duration>,
        cancel_flag: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<RgbaBuffer, AgentTemplateError> {
        let start = std::time::Instant::now();
        let check_deadline = || -> Result<(), AgentTemplateError> {
            if let Some(flag) = cancel_flag {
                if flag.load(std::sync::atomic::Ordering::SeqCst) {
                    return Err(AgentTemplateError::Cancelled);
                }
            }
            if let Some(limit) = timeout {
                if start.elapsed() >= limit {
                    return Err(AgentTemplateError::Timeout(limit));
                }
            }
            Ok(())
        };

        check_deadline()?;

        // Validate draft before allocating framebuffers or rendering
        let report = Self::validate_draft(draft, None, None);
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

        check_deadline()?;

        let w = width
            .unwrap_or(draft.responsive_policy.reference_width)
            .clamp(320, MAX_PREVIEW_WIDTH);
        let h = height
            .unwrap_or(draft.responsive_policy.reference_height)
            .clamp(180, MAX_PREVIEW_HEIGHT);
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

        check_deadline()?;

        let plan = CompositionPlan::from_project(&project).map_err(|e| {
            AgentTemplateError::Preview(format!("Failed to build CompositionPlan: {}", e))
        })?;

        let target_layer = plan.visual_layers.first().ok_or_else(|| {
            AgentTemplateError::Preview("No visual layer produced in plan".to_string())
        })?;

        check_deadline()?;

        let sample_time = RationalTime::from_seconds(time_seconds, fps);
        let buf = render_frame_overlay_buffer_at(&project, &plan, target_layer, sample_time)
            .map_err(|e| AgentTemplateError::Preview(format!("Compositor error: {}", e)))?
            .ok_or_else(|| {
                AgentTemplateError::Preview("Compositor returned empty frame".to_string())
            })?;

        check_deadline()?;

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
        Self::preview_draft_to_png_bounded(
            draft,
            output_path,
            time_seconds,
            width,
            height,
            Some(DEFAULT_PREVIEW_TIMEOUT),
            None,
        )
    }

    /// Render preview frame and write to an operation-owned temporary file with bounded timeout and cancellation.
    /// Computes pixel scan and receipt BEFORE publication. Does not modify output_path.
    pub fn preview_draft_to_temp_bounded(
        draft: &TemplateDraft,
        output_path: &Path,
        time_seconds: f64,
        width: Option<u32>,
        height: Option<u32>,
        timeout: Option<std::time::Duration>,
        cancel_flag: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<(TempPreviewArtifact, TemplatePreviewReceipt), AgentTemplateError> {
        let start = std::time::Instant::now();
        let parent = output_path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)?;
        let temp_filename = format!(
            ".tmp_preview_{}_{}.png",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        );
        let temp_path = parent.join(temp_filename);
        let temp_artifact = TempPreviewArtifact::new(temp_path);

        let check_deadline = || -> Result<(), AgentTemplateError> {
            if let Some(flag) = cancel_flag {
                if flag.load(std::sync::atomic::Ordering::SeqCst) {
                    return Err(AgentTemplateError::Cancelled);
                }
            }
            if let Some(limit) = timeout {
                if start.elapsed() >= limit {
                    return Err(AgentTemplateError::Timeout(limit));
                }
            }
            Ok(())
        };

        check_deadline()?;

        if let Ok(r) = WORKER_EXECUTION_HOOK.read() {
            if let Some(hook) = r.as_ref() {
                hook(output_path);
            }
        }

        check_deadline()?;

        let buf =
            Self::preview_draft_bounded(draft, time_seconds, width, height, timeout, cancel_flag)?;

        check_deadline()?;

        let png_bytes = buf.to_png_bytes();

        check_deadline()?;

        fs::write(temp_artifact.path(), &png_bytes)?;

        check_deadline()?;

        // Compute pixel scan and receipt data BEFORE publication
        let mut non_zero_pixels = 0;
        for y in 0..buf.height {
            for x in 0..buf.width {
                let [_, _, _, a] = buf.get_pixel(x, y);
                if a > 0 {
                    non_zero_pixels += 1;
                }
            }
        }

        let receipt = TemplatePreviewReceipt {
            output_path: output_path.to_string_lossy().to_string(),
            width: buf.width,
            height: buf.height,
            time_seconds,
            has_visible_content: non_zero_pixels > 0,
            non_zero_pixels,
        };

        // Final deadline check before returning artifact
        check_deadline()?;

        if let Ok(r) = PRE_DELIVERY_HOOK.read() {
            if let Some(hook) = r.as_ref() {
                hook(temp_artifact.path());
            }
        }

        check_deadline()?;

        Ok((temp_artifact, receipt))
    }

    /// Render preview frame and persist to PNG file with bounded timeout, cancellation, and atomic publish.
    /// Existing output file bytes are preserved if cancel, timeout, or write error occurs.
    pub fn preview_draft_to_png_bounded(
        draft: &TemplateDraft,
        output_path: &Path,
        time_seconds: f64,
        width: Option<u32>,
        height: Option<u32>,
        timeout: Option<std::time::Duration>,
        cancel_flag: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<TemplatePreviewReceipt, AgentTemplateError> {
        let start = std::time::Instant::now();
        let (mut temp_artifact, receipt) = Self::preview_draft_to_temp_bounded(
            draft,
            output_path,
            time_seconds,
            width,
            height,
            timeout,
            cancel_flag,
        )?;

        // Final deadline and cancellation check before publication
        if let Some(flag) = cancel_flag {
            if flag.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(AgentTemplateError::Cancelled);
            }
        }
        if let Some(limit) = timeout {
            if start.elapsed() >= limit {
                return Err(AgentTemplateError::Timeout(limit));
            }
        }

        // Atomically publish temp_path to output_path with replace-existing semantics.
        // No backup-then-move window: destination is never left missing for concurrent readers.
        // If rename fails (e.g. locked destination with sharing violation), temp_artifact cleans temp_path
        // and destination remains untouched.
        fs::rename(temp_artifact.path(), output_path)?;
        temp_artifact.disarm();

        Ok(receipt)
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

    /// Construct CommandOperation envelopes to apply a TemplateDraft to a project text item.
    /// Preserves all userOverrides (both bound parameters via TemplateParametersBatchSet
    /// and unbound layer properties via TemplateLayerUpdate).
    pub fn build_apply_operations(
        draft: &TemplateDraft,
        item_id: &str,
        base_tx: &str,
    ) -> Vec<OperationEnvelope> {
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

        // Operation 3: Apply overrides (both bound and unbound)
        if !draft.user_overrides.is_empty() {
            let mut ovr_params = HashMap::new();
            let mut unbound_overrides: Vec<&TemplateOverride> = Vec::new();

            for ovr in &draft.user_overrides {
                if let Some(binding) = draft.bindings.iter().find(|b| {
                    b.target_layer_id == ovr.target_layer_id && b.target_property == ovr.property
                }) {
                    ovr_params.insert(binding.parameter_key.clone(), ovr.value.clone());
                } else {
                    unbound_overrides.push(ovr);
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

            // Group unbound overrides by target_layer_id
            let mut layer_groups: HashMap<&str, Vec<&TemplateOverride>> = HashMap::new();
            for ovr in &unbound_overrides {
                layer_groups
                    .entry(&ovr.target_layer_id)
                    .or_default()
                    .push(ovr);
            }

            let resolved_instance = draft.to_instance();
            for (target_lid, _ovrs) in layer_groups {
                if let Some(resolved_layer) = resolved_instance
                    .layers
                    .iter()
                    .find(|l| l.layer_id == target_lid)
                {
                    operations.push(OperationEnvelope {
                        operation_id: format!("{}_op_unbound_layer_{}", base_tx, target_lid),
                        operation: CommandOperation::TemplateLayerUpdate {
                            item_id: item_id.to_string(),
                            layer_id: target_lid.to_string(),
                            layer: resolved_layer.clone(),
                        },
                    });
                }
            }
        }

        operations
    }

    /// Apply template draft directly to a project text item as an atomic CommandService transaction.
    /// Preserves ADR-001 idempotency and durable atomic persistence.
    /// Strictly validates all preconditions before catalog publication; rolls back catalog on any commit failure.
    #[allow(clippy::too_many_arguments)]
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

        // 2. Reject if required resources are missing
        for res_id in &report.missing_resources {
            if draft
                .resources
                .resources
                .iter()
                .any(|r| r.required && &r.resource_id == res_id)
            {
                return Err(AgentTemplateError::Validation {
                    count: 1,
                    first_error: format!("Required resource '{}' is missing", res_id),
                    report,
                });
            }
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

        // 5. Check staged base revision and client registry preconditions BEFORE touching catalog
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

        // Pre-validate client epoch
        if cepoch < project.client_registry.current_epoch {
            return Err(AgentTemplateError::Command(
                CommandError::ClientEpochExpired {
                    client_id: cid.clone(),
                    client_epoch: cepoch,
                    min_accepted_epoch: project.client_registry.current_epoch,
                },
            ));
        }

        // Pre-validate sequence gap & replay
        if let Some(client_rec) = project.client_registry.clients.get(&cid) {
            if seq > client_rec.highest_sequence + 1 {
                return Err(AgentTemplateError::Command(
                    CommandError::SequenceGapDetected {
                        client_id: cid.clone(),
                        expected_sequence: client_rec.highest_sequence + 1,
                        received_sequence: seq,
                    },
                ));
            }
        } else if seq != 1 {
            return Err(AgentTemplateError::Command(
                CommandError::SequenceGapDetected {
                    client_id: cid.clone(),
                    expected_sequence: 1,
                    received_sequence: seq,
                },
            ));
        }

        if let Some(rev) = base_revision {
            let is_replayed = project
                .client_registry
                .clients
                .get(&cid)
                .map(|c| seq <= c.highest_sequence)
                .unwrap_or(false);
            if !is_replayed && rev != project.revision {
                return Err(AgentTemplateError::Command(CommandError::StaleRevision {
                    expected: project.revision,
                    received: rev,
                }));
            }
        }

        // 6. Check catalog conflict beforehand
        let def = draft.to_template_definition();
        catalog.ensure_dirs()?;
        let existed_in_catalog = catalog.get(&def.id, def.version);
        if let Some(ref existing_def) = existed_in_catalog {
            if existing_def != &def {
                return Err(AgentTemplateError::Catalog(
                    TemplateCatalogError::ImmutableVersionConflict {
                        id: def.id.clone(),
                        version: def.version,
                    },
                ));
            }
        }
        let def_existed_before = existed_in_catalog.is_some();

        // Stage in memory during transaction so CommandOperation::TemplateApply can resolve definition
        // without prematurely writing to catalog directory on disk
        TemplateCatalog::stage_definition(def.clone());
        struct StagedGuard {
            id: String,
            version: u32,
        }
        impl Drop for StagedGuard {
            fn drop(&mut self) {
                TemplateCatalog::unstage_definition(&self.id, self.version);
            }
        }
        let _staged_guard = StagedGuard {
            id: def.id.clone(),
            version: def.version,
        };

        // 7. Construct CommandEnvelope with deterministic operation IDs for idempotency
        let tx_id =
            transaction_id.unwrap_or_else(|| format!("tx_{}", uuid::Uuid::new_v4().simple()));
        let base_tx = if tx_id.len() > 40 {
            &tx_id[..40]
        } else {
            &tx_id
        };

        let operations = Self::build_apply_operations(draft, item_id, base_tx);

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

        let prior_project = project.clone();
        let mut service = CommandService::new(project);
        let receipt = service.execute(envelope)?;

        if let Err(e) = save_project_atomic(service.project(), project_path) {
            return Err(AgentTemplateError::ProjectLoad(e.to_string()));
        }

        // Publish to catalog on disk ONLY after command transaction and persistence succeed
        if !def_existed_before {
            if let Err(e) = catalog.save_user_definition(&def) {
                // Rollback project file to preserve consistency if catalog publish fails
                let _ = save_project_atomic(&prior_project, project_path);
                return Err(AgentTemplateError::Catalog(e));
            }
        }

        Ok(TemplateApplyReceipt {
            receipt: receipt.clone(),
            applied_draft_id: draft.draft_id.clone(),
            template_id: draft.template_id.clone(),
            template_version: draft.template_version,
            revision: receipt.new_revision,
        })
    }
}
