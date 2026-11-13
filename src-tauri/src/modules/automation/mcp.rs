//! Model Context Protocol (MCP) Automation Adapter for TinixCut
//!
//! Architecture Invariant: MCP is strictly a thin external transport adapter.
//! It contains zero second reducers, zero second command registries, zero alternate persistence formats,
//! and zero independent idempotency ledgers. All mutations route through the canonical CommandService.
//!
//! Governed by ADR-005 and ADR-001 v3.

use crate::modules::automation::edit_plan::{
    build_deterministic_video_draft_with_supporting_media, parse_draft_caption_file,
    validate_edit_plan, EditPlan, EditPlanDraftReceipt, EditPlanValidation,
};
use crate::modules::automation::workflow_manifest::WorkflowManifest;
use crate::modules::automation::{
    AutomationAdapter, CapabilitiesReport, ExportJobStatusReport, ExportStartReceipt,
    ProjectInspectReport,
};
use crate::modules::command::{
    CommandEnvelope, CommandError, CommandReceipt, CommandService, DiffReport,
};
use crate::modules::domain::Project;
use crate::modules::export::{ExportEngine, ExportError, ExportProfile};
use crate::modules::job::{JobKind, JobManager, JobStatus};
use crate::modules::media::ffmpeg::FFmpegService;
use crate::modules::persistence::{
    load_project, load_project_revision_if_exists, same_project_path,
    save_project_atomic_if_revision, PersistenceError,
};
use rmcp::{
    handler::server::wrapper::Parameters,
    model::CallToolResult,
    schemars::{self, JsonSchema, Schema, SchemaGenerator},
    serde::{Deserialize, Serialize},
    tool, tool_handler, tool_router,
};
use sha2::{Digest, Sha256};
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// JSON Schema Implementations for Shared Domain / Command Types
// ---------------------------------------------------------------------------

impl JsonSchema for CommandEnvelope {
    fn schema_name() -> Cow<'static, str> {
        "CommandEnvelope".into()
    }
    fn schema_id() -> Cow<'static, str> {
        "tinixcut::CommandEnvelope".into()
    }
    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        schemars::json_schema!({
            "type": "object",
            "required": ["schemaVersion", "projectId", "baseRevision", "operations"],
            "properties": {
                "schemaVersion": { "type": "integer" },
                "projectId": { "type": "string" },
                "baseRevision": { "type": "integer", "minimum": 0 },
                "clientId": { "type": ["string", "null"] },
                "clientEpoch": { "type": ["integer", "null"], "minimum": 0 },
                "sequenceNumber": { "type": ["integer", "null"], "minimum": 0 },
                "dryRun": { "type": "boolean" },
                "transactionId": { "type": ["string", "null"] },
                "operations": {
                    "type": "array",
                    "items": { "type": "object" }
                }
            }
        })
    }
}

impl JsonSchema for CommandReceipt {
    fn schema_name() -> Cow<'static, str> {
        "CommandReceipt".into()
    }
    fn schema_id() -> Cow<'static, str> {
        "tinixcut::CommandReceipt".into()
    }
    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        schemars::json_schema!({
            "type": "object",
            "required": ["projectId", "priorRevision", "newRevision", "operationsApplied", "receipts"],
            "properties": {
                "projectId": { "type": "string" },
                "priorRevision": { "type": "integer" },
                "newRevision": { "type": "integer" },
                "transactionId": { "type": ["string", "null"] },
                "operationsApplied": { "type": "integer" },
                "receipts": {
                    "type": "array",
                    "items": { "type": "object" }
                },
                "idempotencyScope": { "type": ["string", "null"] }
            }
        })
    }
}

impl JsonSchema for DiffReport {
    fn schema_name() -> Cow<'static, str> {
        "DiffReport".into()
    }
    fn schema_id() -> Cow<'static, str> {
        "tinixcut::DiffReport".into()
    }
    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        schemars::json_schema!({
            "type": "object",
            "required": ["projectId", "baseRevision", "operationsEvaluated", "changes", "warnings"],
            "properties": {
                "projectId": { "type": "string" },
                "baseRevision": { "type": "integer" },
                "operationsEvaluated": { "type": "integer" },
                "changes": {
                    "type": "array",
                    "items": { "type": "object" }
                },
                "warnings": {
                    "type": "array",
                    "items": { "type": "string" }
                }
            }
        })
    }
}

impl JsonSchema for ProjectInspectReport {
    fn schema_name() -> Cow<'static, str> {
        "ProjectInspectReport".into()
    }
    fn schema_id() -> Cow<'static, str> {
        "tinixcut::ProjectInspectReport".into()
    }
    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        schemars::json_schema!({
            "type": "object",
            "required": [
                "projectId", "name", "revision", "durationFrames", "durationSeconds",
                "frameRate", "resolution", "tracksCount", "clipsCount", "assetsCount",
                "tracks", "assets", "supportedOperations", "supportedTransitions", "schemaVersion"
            ],
            "properties": {
                "projectId": { "type": "string" },
                "name": { "type": "string" },
                "revision": { "type": "integer" },
                "durationFrames": { "type": "integer" },
                "durationSeconds": { "type": "number" },
                "frameRate": { "type": "object" },
                "resolution": { "type": "array", "items": { "type": "integer" } },
                "tracksCount": { "type": "integer" },
                "clipsCount": { "type": "integer" },
                "assetsCount": { "type": "integer" },
                "transitionsCount": { "type": "integer" },
                "tracks": { "type": "array", "items": { "type": "object" } },
                "assets": { "type": "array", "items": { "type": "object" } },
                "supportedOperations": { "type": "array", "items": { "type": "string" } },
                "supportedTransitions": { "type": "array", "items": { "type": "string" } },
                "schemaVersion": { "type": "integer" },
                "supportedKeyframeProperties": { "type": "array", "items": { "type": "string" } },
                "supportedKeyframeEasings": { "type": "array", "items": { "type": "string" } },
                "customCubicBounds": { "type": "array", "items": { "type": "number" } },
                "supportedAnimationUnits": { "type": "array", "items": { "type": "string" } },
                "supportedStaggerOrders": { "type": "array", "items": { "type": "string" } },
                "supportedLoopModes": { "type": "array", "items": { "type": "string" } },
                "supportedDurationConflictPolicies": { "type": "array", "items": { "type": "string" } }
            }
        })
    }
}

impl JsonSchema for CapabilitiesReport {
    fn schema_name() -> Cow<'static, str> {
        "CapabilitiesReport".into()
    }
    fn schema_id() -> Cow<'static, str> {
        "tinixcut::CapabilitiesReport".into()
    }
    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        schemars::json_schema!({
            "type": "object",
            "required": [
                "supportedOperations", "supportedTransitions", "supportedExportCodecs",
                "protocolVersion", "schemaVersion"
            ],
            "properties": {
                "supportedOperations": { "type": "array", "items": { "type": "string" } },
                "supportedTransitions": { "type": "array", "items": { "type": "string" } },
                "supportedExportCodecs": { "type": "array", "items": { "type": "string" } },
                "protocolVersion": { "type": "integer" },
                "schemaVersion": { "type": "integer" },
                "supportedKeyframeProperties": { "type": "array", "items": { "type": "string" } },
                "supportedKeyframeEasings": { "type": "array", "items": { "type": "string" } },
                "customCubicBounds": { "type": "array", "items": { "type": "number" } },
                "supportedAnimationUnits": { "type": "array", "items": { "type": "string" } },
                "supportedStaggerOrders": { "type": "array", "items": { "type": "string" } },
                "supportedLoopModes": { "type": "array", "items": { "type": "string" } },
                "supportedDurationConflictPolicies": { "type": "array", "items": { "type": "string" } }
            }
        })
    }
}

impl JsonSchema for ExportStartReceipt {
    fn schema_name() -> Cow<'static, str> {
        "ExportStartReceipt".into()
    }
    fn schema_id() -> Cow<'static, str> {
        "tinixcut::ExportStartReceipt".into()
    }
    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        schemars::json_schema!({
            "type": "object",
            "required": ["jobId", "status", "outputPath"],
            "properties": {
                "jobId": { "type": "string" },
                "status": { "type": "string" },
                "outputPath": { "type": "string" }
            }
        })
    }
}

impl JsonSchema for ExportJobStatusReport {
    fn schema_name() -> Cow<'static, str> {
        "ExportJobStatusReport".into()
    }
    fn schema_id() -> Cow<'static, str> {
        "tinixcut::ExportJobStatusReport".into()
    }
    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        schemars::json_schema!({
            "type": "object",
            "required": ["jobId", "status", "progress"],
            "properties": {
                "jobId": { "type": "string" },
                "status": { "type": "string" },
                "progress": { "type": "number" },
                "error": { "type": ["string", "null"] }
            }
        })
    }
}

impl JsonSchema for ExportProfile {
    fn schema_name() -> Cow<'static, str> {
        "ExportProfile".into()
    }
    fn schema_id() -> Cow<'static, str> {
        "tinixcut::ExportProfile".into()
    }
    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        schemars::json_schema!({
            "type": "object",
            "properties": {
                "width": { "type": "integer", "minimum": 1 },
                "height": { "type": "integer", "minimum": 1 },
                "frameRate": {
                    "type": "object",
                    "properties": {
                        "numerator": { "type": "integer" },
                        "denominator": { "type": "integer" }
                    }
                },
                "crf": { "type": "integer" },
                "preset": { "type": "string" },
                "videoCodec": { "type": "string" },
                "audioCodec": { "type": "string" },
                "audioBitrate": { "type": "string" },
                "audioSampleRate": { "type": "integer" }
            }
        })
    }
}

impl JsonSchema for crate::modules::templates::agent::TemplateDraft {
    fn schema_name() -> Cow<'static, str> {
        "TemplateDraft".into()
    }
    fn schema_id() -> Cow<'static, str> {
        "tinixcut::TemplateDraft".into()
    }
    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        schemars::json_schema!({
            "type": "object",
            "required": ["draftId", "templateId", "name", "category", "description", "layers", "provenance"],
            "properties": {
                "draftId": { "type": "string" },
                "schemaVersion": { "type": "integer" },
                "templateId": { "type": "string" },
                "templateVersion": { "type": "integer", "minimum": 1 },
                "name": { "type": "string" },
                "category": { "type": "string" },
                "description": { "type": "string" },
                "tags": { "type": "array", "items": { "type": "string" } },
                "slots": { "type": "array", "items": { "type": "object" } },
                "parameters": { "type": "array", "items": { "type": "object" } },
                "bindings": { "type": "array", "items": { "type": "object" } },
                "layers": { "type": "array", "items": { "type": "object" } },
                "resources": { "type": "object" },
                "responsivePolicy": { "type": "object" },
                "defaultDuration": { "type": "object" },
                "provenance": { "type": "object" },
                "userOverrides": { "type": "array", "items": { "type": "object" } }
            }
        })
    }
}

impl JsonSchema for crate::modules::templates::agent::TemplateValidationReport {
    fn schema_name() -> Cow<'static, str> {
        "TemplateValidationReport".into()
    }
    fn schema_id() -> Cow<'static, str> {
        "tinixcut::TemplateValidationReport".into()
    }
    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        schemars::json_schema!({
            "type": "object",
            "required": ["valid", "schemaVersion", "issues", "missingResources", "unsupportedOptions"],
            "properties": {
                "valid": { "type": "boolean" },
                "schemaVersion": { "type": "integer" },
                "issues": { "type": "array", "items": { "type": "object" } },
                "missingResources": { "type": "array", "items": { "type": "string" } },
                "unsupportedOptions": { "type": "array", "items": { "type": "string" } },
                "normalizedDraft": { "type": ["object", "null"] }
            }
        })
    }
}

impl JsonSchema for crate::modules::templates::agent::TemplatePreviewReceipt {
    fn schema_name() -> Cow<'static, str> {
        "TemplatePreviewReceipt".into()
    }
    fn schema_id() -> Cow<'static, str> {
        "tinixcut::TemplatePreviewReceipt".into()
    }
    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        schemars::json_schema!({
            "type": "object",
            "required": ["outputPath", "width", "height", "timeSeconds", "hasVisibleContent", "nonZeroPixels"],
            "properties": {
                "outputPath": { "type": "string" },
                "width": { "type": "integer" },
                "height": { "type": "integer" },
                "timeSeconds": { "type": "number" },
                "hasVisibleContent": { "type": "boolean" },
                "nonZeroPixels": { "type": "integer" }
            }
        })
    }
}

impl JsonSchema for crate::modules::templates::agent::TemplateApplyReceipt {
    fn schema_name() -> Cow<'static, str> {
        "TemplateApplyReceipt".into()
    }
    fn schema_id() -> Cow<'static, str> {
        "tinixcut::TemplateApplyReceipt".into()
    }
    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        schemars::json_schema!({
            "type": "object",
            "required": ["receipt", "appliedDraftId", "templateId", "templateVersion", "revision"],
            "properties": {
                "receipt": { "type": "object" },
                "appliedDraftId": { "type": "string" },
                "templateId": { "type": "string" },
                "templateVersion": { "type": "integer" },
                "revision": { "type": "integer" }
            }
        })
    }
}

impl JsonSchema for crate::modules::templates::agent::TemplateSaveReceipt {
    fn schema_name() -> Cow<'static, str> {
        "TemplateSaveReceipt".into()
    }
    fn schema_id() -> Cow<'static, str> {
        "tinixcut::TemplateSaveReceipt".into()
    }
    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        schemars::json_schema!({
            "type": "object",
            "required": ["templateId", "version", "savedPath"],
            "properties": {
                "templateId": { "type": "string" },
                "version": { "type": "integer" },
                "savedPath": { "type": "string" }
            }
        })
    }
}

impl JsonSchema for crate::modules::templates::agent::TemplateCapabilitiesReport {
    fn schema_name() -> Cow<'static, str> {
        "TemplateCapabilitiesReport".into()
    }
    fn schema_id() -> Cow<'static, str> {
        "tinixcut::TemplateCapabilitiesReport".into()
    }
    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        schemars::json_schema!({
            "type": "object",
            "required": ["schemaVersion", "maxLayers", "maxTextSlots", "maxGraphemesPerSlot", "maxRepairIterations"],
            "properties": {
                "schemaVersion": { "type": "integer" },
                "maxLayers": { "type": "integer" },
                "maxTextSlots": { "type": "integer" },
                "maxGraphemesPerSlot": { "type": "integer" },
                "maxRepairIterations": { "type": "integer" },
                "supportedLayerTypes": { "type": "array", "items": { "type": "string" } },
                "supportedShapeKinds": { "type": "array", "items": { "type": "string" } },
                "supportedParameterTypes": { "type": "array", "items": { "type": "string" } },
                "supportedPropertyPaths": { "type": "array", "items": { "type": "string" } },
                "supportedAnimationPresets": { "type": "array", "items": { "type": "string" } },
                "supportedAnimationUnits": { "type": "array", "items": { "type": "string" } },
                "supportedStaggerOrders": { "type": "array", "items": { "type": "string" } },
                "supportedLoopModes": { "type": "array", "items": { "type": "string" } },
                "supportedDurationConflictPolicies": { "type": "array", "items": { "type": "string" } },
                "supportedUnits": { "type": "array", "items": { "type": "string" } },
                "builtInPrimitives": { "type": "array", "items": { "type": "string" } },
                "draftJsonSchema": { "type": "object" }
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Tool Argument DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InspectArgs {
    pub project_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCreateArgs {
    pub project_path: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCreateReceipt {
    pub project_path: String,
    pub project_id: String,
    pub name: String,
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectVariantArgs {
    pub source_project_path: String,
    pub destination_project_path: String,
    pub expected_revision: u64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectVariantReceipt {
    pub source_project_path: String,
    pub destination_project_path: String,
    pub source_project_id: String,
    pub source_revision: u64,
    pub project_id: String,
    pub name: String,
    pub revision: u64,
    pub assets_count: usize,
    pub tracks_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCompareArgs {
    pub left_project_path: String,
    pub left_expected_revision: u64,
    pub right_project_path: String,
    pub right_expected_revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCompareEndpoint {
    pub project_id: String,
    pub name: String,
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEntityChangeSet {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub modified: Vec<ProjectEntityModification>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEntityModification {
    pub entity_id: String,
    pub changed_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCompareReport {
    pub left: ProjectCompareEndpoint,
    pub right: ProjectCompareEndpoint,
    pub content_equivalent: bool,
    pub settings_changed_fields: Vec<String>,
    pub assets: ProjectEntityChangeSet,
    pub tracks: ProjectEntityChangeSet,
    pub items: ProjectEntityChangeSet,
    pub transitions: ProjectEntityChangeSet,
    pub audio_transitions: ProjectEntityChangeSet,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MediaProbeArgs {
    pub media_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MediaSceneCandidatesArgs {
    pub media_path: String,
    #[serde(default)]
    pub threshold: Option<f32>,
    #[serde(default)]
    pub max_duration_seconds: Option<u32>,
    #[serde(default)]
    pub max_candidates: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VisualSceneSourceArgs {
    /// Caller-selected media identity used to map results back; the model does not receive this ID.
    pub asset_id: String,
    /// Explicit local input path; this value is never sent to the vision model or returned in results.
    pub media_path: String,
    #[serde(default)]
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MediaVisualScenesArgs {
    pub media: Vec<VisualSceneSourceArgs>,
    pub query: String,
    pub model: String,
    #[serde(default)]
    pub max_scenes: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MediaTranscriptArgs {
    pub media_path: String,
    pub whisper_cli_path: String,
    pub model_path: String,
    #[serde(default)]
    pub language: Option<String>,
}

impl JsonSchema for crate::modules::media::transcription::TranscriptAnalysisReport {
    fn schema_name() -> Cow<'static, str> {
        "TranscriptAnalysisReport".into()
    }
    fn schema_id() -> Cow<'static, str> {
        "tinixcut::TranscriptAnalysisReport".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        schemars::json_schema!({
            "type": "object",
            "required": ["schemaVersion", "algorithmVersion", "sourceFingerprint", "configFingerprint", "analyzedDurationMicros", "result"],
            "properties": {
                "schemaVersion": {"type": "integer"},
                "algorithmVersion": {"type": "string"},
                "sourceFingerprint": {"type": "string"},
                "configFingerprint": {"type": "string"},
                "analyzedDurationMicros": {"type": "integer", "minimum": 0},
                "result": {
                    "type": "object",
                    "required": ["language", "segments"],
                    "properties": {
                        "language": {"type": "string"},
                        "segments": {"type": "array", "items": {
                            "type": "object",
                            "required": ["start", "duration", "text"],
                            "properties": {
                                "start": {"type": "object"},
                                "duration": {"type": "object"},
                                "text": {"type": "string"}
                            }
                        }}
                    }
                }
            }
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaProbeStreamReport {
    pub codec_type: String,
    pub codec_name: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_seconds: Option<f64>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaProbeReport {
    pub media_path: String,
    pub file_size_bytes: u64,
    pub duration_seconds: f64,
    pub width: u32,
    pub height: u32,
    pub frame_rate_numerator: u32,
    pub frame_rate_denominator: u32,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub streams: Vec<MediaProbeStreamReport>,
}

impl JsonSchema for MediaProbeReport {
    fn schema_name() -> Cow<'static, str> {
        "MediaProbeReport".into()
    }
    fn schema_id() -> Cow<'static, str> {
        "tinixcut::MediaProbeReport".into()
    }
    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        schemars::json_schema!({
            "type": "object",
            "required": ["mediaPath", "fileSizeBytes", "durationSeconds", "width", "height", "frameRateNumerator", "frameRateDenominator", "streams"],
            "properties": {
                "mediaPath": { "type": "string" },
                "fileSizeBytes": { "type": "integer", "minimum": 0 },
                "durationSeconds": { "type": "number", "minimum": 0 },
                "width": { "type": "integer", "minimum": 0 },
                "height": { "type": "integer", "minimum": 0 },
                "frameRateNumerator": { "type": "integer", "minimum": 0 },
                "frameRateDenominator": { "type": "integer", "minimum": 1 },
                "videoCodec": { "type": ["string", "null"] },
                "audioCodec": { "type": ["string", "null"] },
                "sampleRate": { "type": ["integer", "null"] },
                "channels": { "type": ["integer", "null"] },
                "streams": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["codecType", "codecName"],
                        "properties": {
                            "codecType": { "type": "string" },
                            "codecName": { "type": "string" },
                            "width": { "type": ["integer", "null"] },
                            "height": { "type": ["integer", "null"] },
                            "durationSeconds": { "type": ["number", "null"] },
                            "sampleRate": { "type": ["integer", "null"] },
                            "channels": { "type": ["integer", "null"] }
                        }
                    }
                }
            }
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct CapabilitiesArgs {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DryRunArgs {
    pub project_path: String,
    pub envelope: CommandEnvelope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditPlanValidateArgs {
    pub project_path: String,
    pub plan: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EditPlanDraftArgs {
    pub project_path: String,
    pub goal: String,
    pub media_ids: Vec<String>,
    #[serde(default)]
    pub target_duration_frames: Option<u64>,
    #[serde(default)]
    pub audio_asset_id: Option<String>,
    #[serde(default)]
    pub subtitle_file_path: Option<String>,
    #[serde(default)]
    pub planner_provider: Option<String>,
    #[serde(default)]
    pub local_model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EditPlanSceneDraftArgs {
    pub project_path: String,
    pub goal: String,
    pub report: crate::modules::media::visual_scene::VisualSceneAnalysisReport,
    /// Explicit scene IDs in the intended timeline order.
    pub selected_scene_ids: Vec<String>,
    #[serde(default)]
    pub target_duration_frames: Option<u64>,
    #[serde(default)]
    pub exact_duration: bool,
    #[serde(default)]
    pub must_keep_scene_ids: Vec<String>,
    #[serde(default)]
    pub must_omit_scene_ids: Vec<String>,
    #[serde(default)]
    pub allowed_asset_ids: Vec<String>,
    #[serde(default)]
    pub target_canvas: Option<crate::modules::automation::edit_plan::SceneCanvasPreset>,
    #[serde(default)]
    pub opening_title: Option<String>,
    #[serde(default)]
    pub title_template: Option<crate::modules::automation::edit_plan::SceneTitleTemplate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EditPlanSceneSelectionSuggestArgs {
    pub project_path: String,
    pub goal: String,
    pub report: crate::modules::media::visual_scene::VisualSceneAnalysisReport,
    #[serde(default)]
    pub target_duration_seconds: Option<u32>,
    #[serde(default)]
    pub must_keep_scene_ids: Vec<String>,
    #[serde(default)]
    pub must_omit_scene_ids: Vec<String>,
    #[serde(default)]
    pub allowed_asset_ids: Vec<String>,
    #[serde(default)]
    pub caption_intent: Option<String>,
    #[serde(default)]
    pub audio_intent: Option<String>,
    #[serde(default)]
    pub suggest_opening_title: bool,
    /// Optional source-timed local transcript excerpts. The native boundary checks source fingerprints and scene overlap before model use.
    #[serde(default)]
    pub transcript_evidence: Vec<crate::modules::automation::edit_plan::TranscriptSceneEvidence>,
    pub local_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EditPlanReorderArgs {
    pub project_path: String,
    pub goal: String,
    pub track_id: String,
    /// For caller-order-v1 this is the desired order; for lm-studio-local it is the selected set.
    pub item_ids: Vec<String>,
    #[serde(default)]
    pub planner_provider: Option<String>,
    #[serde(default)]
    pub local_model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EditPlanTrimArgs {
    pub project_path: String,
    pub goal: String,
    pub item_id: String,
    pub target_duration_frames: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EditPlanVolumeArgs {
    pub project_path: String,
    pub goal: String,
    pub item_id: String,
    /// Linear gain: 0.0 is silent, 1.0 is unchanged, and 2.0 is twice the amplitude.
    pub volume: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EditPlanCanvasArgs {
    pub project_path: String,
    pub goal: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EditPlanTextReplaceArgs {
    pub project_path: String,
    pub goal: String,
    pub item_ids: Vec<String>,
    pub search_text: String,
    pub replacement_text: String,
}

impl JsonSchema for EditPlanDraftReceipt {
    fn schema_name() -> Cow<'static, str> {
        "EditPlanDraftReceipt".into()
    }
    fn schema_id() -> Cow<'static, str> {
        "tinixcut::EditPlanDraftReceipt".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        schemars::json_schema!({
            "type": "object",
            "required": ["provider", "plan", "validation"],
            "properties": {
                "provider": { "type": "string" },
                "plan": { "type": "object" },
                "validation": { "type": "object" }
            }
        })
    }
}

impl JsonSchema for EditPlanValidateArgs {
    fn schema_name() -> Cow<'static, str> {
        "EditPlanValidateArgs".into()
    }
    fn schema_id() -> Cow<'static, str> {
        "tinixcut::EditPlanValidateArgs".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        schemars::json_schema!({
            "type": "object",
            "required": ["projectPath", "plan"],
            "properties": {
                "projectPath": { "type": "string" },
                "plan": {
                    "type": "object",
                    "required": ["schemaVersion", "goal", "projectId", "baseRevision", "operations"],
                    "properties": {
                        "schemaVersion": { "type": "integer", "const": 1 },
                        "goal": { "type": "string", "minLength": 1 },
                        "projectId": { "type": "string" },
                        "baseRevision": { "type": "integer", "minimum": 0 },
                        "referencedMediaIds": { "type": "array", "items": { "type": "string" } },
                        "assumptions": { "type": "array", "items": { "type": "string" } },
                        "warnings": { "type": "array", "items": { "type": "string" } },
                        "operations": {
                            "type": "array",
                            "minItems": 1,
                            "items": {
                                "type": "object",
                                "required": ["operationId", "operation"],
                                "properties": {
                                    "operationId": { "type": "string" },
                                    "operation": {
                                        "type": "object",
                                        "required": ["type", "payload"],
                                        "properties": {
                                            "type": { "type": "string" },
                                            "payload": { "type": "object" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        })
    }
}

impl JsonSchema for EditPlanValidation {
    fn schema_name() -> Cow<'static, str> {
        "EditPlanValidation".into()
    }
    fn schema_id() -> Cow<'static, str> {
        "tinixcut::EditPlanValidation".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        schemars::json_schema!({
            "type": "object",
            "required": ["valid", "issues"],
            "properties": {
                "valid": { "type": "boolean" },
                "issues": { "type": "array", "items": { "type": "string" } },
                "diff": { "type": ["object", "null"] }
            }
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApplyArgs {
    pub project_path: String,
    pub envelope: CommandEnvelope,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SaveArgs {
    pub project_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SaveReceipt {
    pub saved_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExportStartArgs {
    pub project_path: String,
    pub output_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<ExportProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExportPreflightArgs {
    pub project_path: String,
    pub output_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<ExportProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowManifestCreateArgs {
    pub project_path: String,
    pub manifest_path: String,
    pub plan: serde_json::Value,
    #[serde(default)]
    pub artifact_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowManifestCreateReceipt {
    pub manifest_path: String,
    pub manifest: WorkflowManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowManifestValidateArgs {
    pub project_path: String,
    pub manifest_path: String,
    pub plan: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowManifestValidateReceipt {
    pub valid: bool,
    pub workflow_id: String,
    pub state: String,
    pub project_revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowManifestCheckpointArgs {
    pub project_path: String,
    pub manifest_path: String,
    pub plan: serde_json::Value,
    pub state: crate::modules::automation::workflow_manifest::WorkflowCheckpointState,
    #[serde(default)]
    pub artifact_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExportStatusArgs {
    pub job_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExportCancelArgs {
    pub job_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExportCancelReceipt {
    pub cancelled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TemplateValidateArgs {
    pub draft: crate::modules::templates::agent::TemplateDraft,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePreviewArgs {
    pub draft: crate::modules::templates::agent::TemplateDraft,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancel_requested: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TemplateApplyArgs {
    pub project_path: String,
    pub item_id: String,
    pub draft: crate::modules::templates::agent::TemplateDraft,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_epoch: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sequence_number: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_revision: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TemplateSaveArgs {
    pub draft: crate::modules::templates::agent::TemplateDraft,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overwrite: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StructuredError {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl StructuredError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(
        code: impl Into<String>,
        message: impl Into<String>,
        details: serde_json::Value,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: Some(details),
        }
    }
}

fn tool_error(err: StructuredError) -> CallToolResult {
    CallToolResult::structured_error(serde_json::to_value(err).unwrap_or_default())
}

fn tool_ok<T: Serialize>(value: &T) -> CallToolResult {
    CallToolResult::structured(serde_json::to_value(value).unwrap_or_default())
}

fn serialized_object<T: Serialize>(value: &T) -> serde_json::Value {
    serde_json::to_value(value).unwrap_or(serde_json::Value::Null)
}

fn with_track_id<T: Serialize>(value: &T, track_id: &str) -> serde_json::Value {
    let mut serialized = serialized_object(value);
    if let serde_json::Value::Object(fields) = &mut serialized {
        fields.insert(
            "track_id".to_string(),
            serde_json::Value::String(track_id.to_string()),
        );
    }
    serialized
}

fn changed_object_fields(before: &serde_json::Value, after: &serde_json::Value) -> Vec<String> {
    let (Some(before), Some(after)) = (before.as_object(), after.as_object()) else {
        return (before != after)
            .then(|| "value".to_string())
            .into_iter()
            .collect();
    };
    let keys = before
        .keys()
        .chain(after.keys())
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    keys.into_iter()
        .filter(|key| before.get(key) != after.get(key))
        .collect()
}

fn compare_entity_maps(
    before: std::collections::BTreeMap<String, serde_json::Value>,
    after: std::collections::BTreeMap<String, serde_json::Value>,
) -> ProjectEntityChangeSet {
    let before_ids = before
        .keys()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let after_ids = after
        .keys()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let added = after_ids.difference(&before_ids).cloned().collect();
    let removed = before_ids.difference(&after_ids).cloned().collect();
    let modified = before_ids
        .intersection(&after_ids)
        .filter_map(|entity_id| {
            let fields = changed_object_fields(&before[entity_id], &after[entity_id]);
            (!fields.is_empty()).then(|| ProjectEntityModification {
                entity_id: entity_id.clone(),
                changed_fields: fields,
            })
        })
        .collect();
    ProjectEntityChangeSet {
        added,
        removed,
        modified,
    }
}

fn entity_changes_are_empty(changes: &ProjectEntityChangeSet) -> bool {
    changes.added.is_empty() && changes.removed.is_empty() && changes.modified.is_empty()
}

pub(crate) fn project_compare_report(left: &Project, right: &Project) -> ProjectCompareReport {
    let settings_changed_fields = changed_object_fields(
        &serialized_object(&left.settings),
        &serialized_object(&right.settings),
    );
    let assets = compare_entity_maps(
        left.assets
            .iter()
            .map(|asset| (asset.asset_id.clone(), serialized_object(asset)))
            .collect(),
        right
            .assets
            .iter()
            .map(|asset| (asset.asset_id.clone(), serialized_object(asset)))
            .collect(),
    );
    let tracks = compare_entity_maps(
        left.tracks
            .iter()
            .map(|track| {
                let mut value = serialized_object(track);
                if let serde_json::Value::Object(fields) = &mut value {
                    fields.remove("items");
                    fields.remove("transitions");
                    fields.remove("audio_transitions");
                }
                (track.track_id.clone(), value)
            })
            .collect(),
        right
            .tracks
            .iter()
            .map(|track| {
                let mut value = serialized_object(track);
                if let serde_json::Value::Object(fields) = &mut value {
                    fields.remove("items");
                    fields.remove("transitions");
                    fields.remove("audio_transitions");
                }
                (track.track_id.clone(), value)
            })
            .collect(),
    );
    let collect_entities = |project: &Project, kind: &str| {
        project
            .tracks
            .iter()
            .flat_map(|track| {
                let track_id = track.track_id.clone();
                match kind {
                    "items" => track
                        .items
                        .iter()
                        .map(|item| (item.item_id.clone(), with_track_id(item, &track_id)))
                        .collect::<Vec<_>>(),
                    "transitions" => track
                        .transitions
                        .iter()
                        .map(|transition| {
                            (
                                transition.transition_id.clone(),
                                with_track_id(transition, &track_id),
                            )
                        })
                        .collect(),
                    "audio_transitions" => track
                        .audio_transitions
                        .iter()
                        .map(|transition| {
                            (
                                transition.transition_id.clone(),
                                with_track_id(transition, &track_id),
                            )
                        })
                        .collect(),
                    _ => Vec::new(),
                }
            })
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    let items = compare_entity_maps(
        collect_entities(left, "items"),
        collect_entities(right, "items"),
    );
    let transitions = compare_entity_maps(
        collect_entities(left, "transitions"),
        collect_entities(right, "transitions"),
    );
    let audio_transitions = compare_entity_maps(
        collect_entities(left, "audio_transitions"),
        collect_entities(right, "audio_transitions"),
    );
    let content_equivalent = settings_changed_fields.is_empty()
        && entity_changes_are_empty(&assets)
        && entity_changes_are_empty(&tracks)
        && entity_changes_are_empty(&items)
        && entity_changes_are_empty(&transitions)
        && entity_changes_are_empty(&audio_transitions);
    ProjectCompareReport {
        left: ProjectCompareEndpoint {
            project_id: left.project_id.clone(),
            name: left.name.clone(),
            revision: left.revision,
        },
        right: ProjectCompareEndpoint {
            project_id: right.project_id.clone(),
            name: right.name.clone(),
            revision: right.revision,
        },
        content_equivalent,
        settings_changed_fields,
        assets,
        tracks,
        items,
        transitions,
        audio_transitions,
    }
}

fn map_project_save_error(error: PersistenceError, action: &str) -> StructuredError {
    match error {
        PersistenceError::RevisionConflict { expected, actual } => {
            StructuredError::with_details(
                "PROJECT_REVISION_CONFLICT",
                format!(
                    "Cannot {action}: expected disk revision {expected:?}, found {actual:?}; reload the project or save as a new file to preserve your edits"
                ),
                serde_json::json!({
                    "expectedRevision": expected,
                    "actualRevision": actual,
                }),
            )
        }
        other => StructuredError::new(
            "PROJECT_SAVE_FAILED",
            format!("Failed to {action}: {other}"),
        ),
    }
}

pub fn map_command_error(err: CommandError) -> StructuredError {
    match err {
        CommandError::StaleRevision { expected, received } => StructuredError::with_details(
            "STALE_REVISION",
            format!(
                "Stale revision conflict: current project revision is {expected}, but baseRevision was {received}"
            ),
            serde_json::json!({ "expected": expected, "received": received }),
        ),
        CommandError::IdempotencyCollision {
            operation_id,
            existing_fingerprint,
            received_fingerprint,
        } => StructuredError::with_details(
            "IDEMPOTENCY_COLLISION",
            format!(
                "Idempotency collision: operation '{operation_id}' was previously committed with a different payload"
            ),
            serde_json::json!({
                "operationId": operation_id,
                "existingFingerprint": existing_fingerprint,
                "receivedFingerprint": received_fingerprint
            }),
        ),
        CommandError::OperationAlreadyCommitted {
            operation_id,
            client_id,
            sequence_number,
        } => StructuredError::with_details(
            "OPERATION_ALREADY_COMMITTED",
            format!(
                "Operation '{operation_id}' was already committed under sequence {sequence_number} for client '{client_id}'"
            ),
            serde_json::json!({
                "operationId": operation_id,
                "clientId": client_id,
                "sequenceNumber": sequence_number
            }),
        ),
        CommandError::ClientEpochExpired {
            client_id,
            client_epoch,
            min_accepted_epoch,
        } => StructuredError::with_details(
            "CLIENT_EPOCH_EXPIRED",
            format!(
                "Client epoch {client_epoch} for client '{client_id}' has expired (minimum accepted epoch is {min_accepted_epoch})"
            ),
            serde_json::json!({
                "clientId": client_id,
                "clientEpoch": client_epoch,
                "minAcceptedEpoch": min_accepted_epoch
            }),
        ),
        CommandError::SequenceGapDetected {
            client_id,
            expected_sequence,
            received_sequence,
        } => StructuredError::with_details(
            "SEQUENCE_GAP_DETECTED",
            format!(
                "Sequence gap detected for client '{client_id}': expected sequence {expected_sequence}, received {received_sequence}"
            ),
            serde_json::json!({
                "clientId": client_id,
                "expectedSequence": expected_sequence,
                "receivedSequence": received_sequence
            }),
        ),
        CommandError::ProjectIdMismatch { expected, received } => StructuredError::with_details(
            "PROJECT_ID_MISMATCH",
            format!("Project ID mismatch: expected {expected}, received {received}"),
            serde_json::json!({ "expected": expected, "received": received }),
        ),
        CommandError::SchemaVersionMismatch { expected, received } => {
            StructuredError::with_details(
                "SCHEMA_VERSION_MISMATCH",
                format!("Schema version mismatch: expected {expected}, received {received}"),
                serde_json::json!({ "expected": expected, "received": received }),
            )
        }
        CommandError::OperationValidationFailed {
            operation_id,
            message,
        } => StructuredError::with_details(
            "OPERATION_VALIDATION_FAILED",
            format!("Operation '{operation_id}' validation failed: {message}"),
            serde_json::json!({ "operationId": operation_id, "message": message }),
        ),
        CommandError::InsufficientTransitionHandle {
            operation_id,
            code,
            message,
            required_preroll,
            available_preroll,
            required_postroll,
            available_postroll,
        } => StructuredError::with_details(
            "INSUFFICIENT_TRANSITION_HANDLE",
            format!(
                "Cannot create or update transition: required media handle exceeds available source frames. Code: {code}"
            ),
            serde_json::json!({
                "operationId": operation_id,
                "code": code,
                "message": message,
                "requiredPreroll": required_preroll,
                "availablePreroll": available_preroll,
                "requiredPostroll": required_postroll,
                "availablePostroll": available_postroll
            }),
        ),
        CommandError::TransitionHandleUnverifiable {
            operation_id,
            code,
            reason,
        } => StructuredError::with_details(
            "TRANSITION_HANDLE_UNVERIFIABLE",
            format!("Transition handle unverifiable: {reason}"),
            serde_json::json!({ "operationId": operation_id, "code": code, "reason": reason }),
        ),
        CommandError::UnsupportedTransitionMedia { operation_id, code } => {
            StructuredError::with_details(
                "UNSUPPORTED_TRANSITION_MEDIA",
                format!("Cannot create transition: participant is not a video item. Code: {code}"),
                serde_json::json!({ "operationId": operation_id, "code": code }),
            )
        }
        CommandError::UnsupportedTransition {
            operation_id,
            transition,
        } => StructuredError::with_details(
            "UNSUPPORTED_TRANSITION",
            format!("Unsupported transition '{transition}' in operation '{operation_id}'"),
            serde_json::json!({ "operationId": operation_id, "transition": transition }),
        ),
        CommandError::ClientRegistryFull { max_clients } => StructuredError::with_details(
            "CLIENT_REGISTRY_FULL",
            format!("Maximum active client capacity ({max_clients}) reached"),
            serde_json::json!({ "maxClients": max_clients }),
        ),
        CommandError::LegacyReceiptUnverifiable { operation_id } => StructuredError::with_details(
            "LEGACY_RECEIPT_UNVERIFIABLE",
            format!(
                "Operation '{operation_id}' committed under legacy non-cryptographic schema cannot be safely verified"
            ),
            serde_json::json!({ "operationId": operation_id }),
        ),
        other => StructuredError::new("COMMAND_FAILED", other.to_string()),
    }
}

// ---------------------------------------------------------------------------
// TinixCut MCP Server Implementation
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Worker Concurrency and Capacity Management
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum CapacityError {
    AlreadyActive(String),
    Full,
}

#[derive(Default)]
pub struct WorkerCapacityState {
    pub max_workers: usize,
    pub active_job_ids: std::collections::HashSet<String>,
    pub live_workers: usize,
    pub peak_live_workers: usize,
    pub job_live_counts: std::collections::HashMap<String, usize>,
    pub job_peak_counts: std::collections::HashMap<String, usize>,
}

#[derive(Clone)]
pub struct PreviewWorkerCapacity {
    inner: Arc<std::sync::Mutex<WorkerCapacityState>>,
}

impl PreviewWorkerCapacity {
    pub fn new(max_workers: usize) -> Self {
        Self {
            inner: Arc::new(std::sync::Mutex::new(WorkerCapacityState {
                max_workers,
                active_job_ids: std::collections::HashSet::new(),
                live_workers: 0,
                peak_live_workers: 0,
                job_live_counts: std::collections::HashMap::new(),
                job_peak_counts: std::collections::HashMap::new(),
            })),
        }
    }

    /// Try to acquire capacity for `job_id`.
    /// Returns:
    /// - Err(CapacityError::AlreadyActive) if a worker is already running for this job_id
    /// - Err(CapacityError::Full) if total active workers >= max_workers
    /// - Ok(WorkerCapacityPermit) if permit is successfully acquired
    pub fn try_acquire(&self, job_id: &str) -> Result<WorkerCapacityPermit, CapacityError> {
        let mut state = self.inner.lock().unwrap();
        if state.active_job_ids.contains(job_id) {
            return Err(CapacityError::AlreadyActive(job_id.to_string()));
        }
        if state.live_workers >= state.max_workers {
            return Err(CapacityError::Full);
        }

        state.active_job_ids.insert(job_id.to_string());
        state.live_workers += 1;
        if state.live_workers > state.peak_live_workers {
            state.peak_live_workers = state.live_workers;
        }
        let new_count = {
            let count = state.job_live_counts.entry(job_id.to_string()).or_insert(0);
            *count += 1;
            *count
        };
        let job_peak = state.job_peak_counts.entry(job_id.to_string()).or_insert(0);
        if new_count > *job_peak {
            *job_peak = new_count;
        }

        Ok(WorkerCapacityPermit {
            tracker: Arc::clone(&self.inner),
            job_id: job_id.to_string(),
            active: true,
        })
    }

    pub fn live_workers(&self) -> usize {
        self.inner.lock().unwrap().live_workers
    }

    pub fn peak_live_workers(&self) -> usize {
        self.inner.lock().unwrap().peak_live_workers
    }

    pub fn live_for_job(&self, job_id: &str) -> usize {
        let state = self.inner.lock().unwrap();
        state.job_live_counts.get(job_id).copied().unwrap_or(0)
    }

    pub fn peak_for_job(&self, job_id: &str) -> usize {
        let state = self.inner.lock().unwrap();
        state.job_peak_counts.get(job_id).copied().unwrap_or(0)
    }

    pub fn is_active(&self, job_id: &str) -> bool {
        self.inner.lock().unwrap().active_job_ids.contains(job_id)
    }
}

pub struct WorkerCapacityPermit {
    tracker: Arc<std::sync::Mutex<WorkerCapacityState>>,
    job_id: String,
    active: bool,
}

impl Drop for WorkerCapacityPermit {
    fn drop(&mut self) {
        if self.active {
            if let Ok(mut state) = self.tracker.lock() {
                state.active_job_ids.remove(&self.job_id);
                if state.live_workers > 0 {
                    state.live_workers -= 1;
                }
                if let Some(count) = state.job_live_counts.get_mut(&self.job_id) {
                    if *count > 0 {
                        *count -= 1;
                    }
                }
            }
            self.active = false;
        }
    }
}

#[derive(Clone)]
pub struct TinixCutMcpServer {
    pub jobs: Arc<Mutex<JobManager>>,
    pub write_gate: Arc<tokio::sync::Mutex<()>>,
    pub worker_capacity: Arc<PreviewWorkerCapacity>,
    /// When true, `tinixcut_apply` refuses envelopes with destructive operations unless an
    /// identical envelope was accepted by `tinixcut_dry_run` earlier in this server session.
    pub require_destructive_confirmation: bool,
    confirmed_dry_runs: Arc<Mutex<std::collections::HashSet<String>>>,
}

/// Environment switch for the opt-in destructive-edit confirmation gate.
pub const REQUIRE_DESTRUCTIVE_CONFIRMATION_ENV: &str =
    "TINIXCUT_MCP_REQUIRE_DESTRUCTIVE_CONFIRMATION";

/// Identity of an envelope for dry-run confirmation: project, base revision and operations.
fn dry_run_confirmation_key(project_path: &str, envelope: &CommandEnvelope) -> String {
    let operations = serde_json::to_string(&envelope.operations).unwrap_or_default();
    format!(
        "{}|{}|{}|{}",
        project_path, envelope.project_id, envelope.base_revision, operations
    )
}

impl Default for TinixCutMcpServer {
    fn default() -> Self {
        Self::new()
    }
}

impl TinixCutMcpServer {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(Mutex::new(JobManager::new(4))),
            write_gate: Arc::new(tokio::sync::Mutex::new(())),
            worker_capacity: Arc::new(PreviewWorkerCapacity::new(4)),
            require_destructive_confirmation: std::env::var(REQUIRE_DESTRUCTIVE_CONFIRMATION_ENV)
                .is_ok_and(|v| v == "1"),
            confirmed_dry_runs: Arc::new(Mutex::new(std::collections::HashSet::new())),
        }
    }

    /// Same server with the destructive-edit confirmation gate switched explicitly (tests).
    pub fn with_destructive_confirmation(mut self, required: bool) -> Self {
        self.require_destructive_confirmation = required;
        self
    }

    /// Cancel all active export jobs cleanly upon shutdown.
    pub fn shutdown_and_cleanup(&self) {
        let count = if let Ok(mut jm) = self.jobs.lock() {
            jm.cancel_all_active()
        } else {
            0
        };
        if count > 0 {
            // Give active export worker threads a bounded window to kill FFmpeg children and clean up temp files
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
    }
}

#[tool_router]
impl TinixCutMcpServer {
    #[tool(
        name = "tinixcut_project_create",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = false, open_world_hint = false),
        description = "Create and atomically persist a new empty TinixCut project at projectPath; refuses to overwrite an existing file",
        output_schema = rmcp::handler::server::tool::schema_for_type::<ProjectCreateReceipt>()
    )]
    async fn project_create(
        &self,
        Parameters(params): Parameters<ProjectCreateArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let _guard = self.write_gate.lock().await;
        if params.name.trim().is_empty() {
            return Ok(tool_error(StructuredError::new(
                "PROJECT_CREATE_INVALID",
                "Project name must contain at least one non-whitespace character",
            )));
        }

        let project_path = PathBuf::from(&params.project_path);
        if project_path.as_os_str().is_empty() {
            return Ok(tool_error(StructuredError::new(
                "PROJECT_CREATE_INVALID",
                "projectPath must name the destination project file",
            )));
        }

        let project = Project::new(params.name.trim().to_string());
        if let Err(error) = save_project_atomic_if_revision(&project, &project_path, None) {
            return Ok(tool_error(map_project_save_error(
                error,
                &format!("create project at {:?}", project_path),
            )));
        }

        Ok(tool_ok(&ProjectCreateReceipt {
            project_path: project_path.to_string_lossy().to_string(),
            project_id: project.project_id,
            name: project.name,
            revision: project.revision,
        }))
    }

    #[tool(
        name = "tinixcut_project_variant",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = false, open_world_hint = false),
        description = "Create a separate project variant from a persisted revision; refuses stale sources, same-path targets, and existing destinations",
        output_schema = rmcp::handler::server::tool::schema_for_type::<ProjectVariantReceipt>()
    )]
    async fn project_variant(
        &self,
        Parameters(params): Parameters<ProjectVariantArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let _guard = self.write_gate.lock().await;
        if params.name.trim().is_empty() {
            return Ok(tool_error(StructuredError::new(
                "PROJECT_VARIANT_INVALID",
                "Variant name must contain at least one non-whitespace character",
            )));
        }
        let source_path = PathBuf::from(&params.source_project_path);
        let destination_path = PathBuf::from(&params.destination_project_path);
        if source_path.as_os_str().is_empty() || destination_path.as_os_str().is_empty() {
            return Ok(tool_error(StructuredError::new(
                "PROJECT_VARIANT_INVALID",
                "Source and destination project paths are required",
            )));
        }
        if destination_path.exists() {
            return Ok(tool_error(StructuredError::new(
                "PROJECT_VARIANT_DESTINATION_EXISTS",
                "Variant destination already exists; choose a new path to preserve existing work",
            )));
        }
        match same_project_path(&source_path, &destination_path) {
            Ok(true) => {
                return Ok(tool_error(StructuredError::new(
                    "PROJECT_VARIANT_SAME_PATH",
                    "Variant destination must be a different project file from its source",
                )))
            }
            Ok(false) => {}
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "PROJECT_VARIANT_PATH_FAILED",
                    error.to_string(),
                )))
            }
        }

        let mut variant = match load_project(&source_path) {
            Ok(project) => project,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "PROJECT_LOAD_FAILED",
                    format!("Failed to load variant source: {error}"),
                )))
            }
        };
        if variant.revision != params.expected_revision {
            return Ok(tool_error(StructuredError::new(
                "PROJECT_REVISION_CONFLICT",
                format!(
                    "Variant source revision changed: expected {}, found {}",
                    params.expected_revision, variant.revision
                ),
            )));
        }

        let source_project_id = variant.project_id.clone();
        let source_revision = variant.revision;
        variant = match super::clone_project_as_variant(&variant, &params.name) {
            Ok(project) => project,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "PROJECT_VARIANT_INVALID",
                    error,
                )))
            }
        };
        if let Err(error) = save_project_atomic_if_revision(&variant, &destination_path, None) {
            return Ok(tool_error(map_project_save_error(
                error,
                &format!("create project variant at {:?}", destination_path),
            )));
        }

        Ok(tool_ok(&ProjectVariantReceipt {
            source_project_path: source_path.to_string_lossy().to_string(),
            destination_project_path: destination_path.to_string_lossy().to_string(),
            source_project_id,
            source_revision,
            project_id: variant.project_id,
            name: variant.name,
            revision: variant.revision,
            assets_count: variant.assets.len(),
            tracks_count: variant.tracks.len(),
        }))
    }

    #[tool(
        name = "tinixcut_project_compare",
        annotations(read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false),
        description = "Compare two persisted TinixCut revisions without exposing media paths or changing project data",
        output_schema = rmcp::handler::server::tool::schema_for_type::<ProjectCompareReport>()
    )]
    async fn project_compare(
        &self,
        Parameters(params): Parameters<ProjectCompareArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let left_path = PathBuf::from(&params.left_project_path);
        let right_path = PathBuf::from(&params.right_project_path);
        let left = match load_project(&left_path) {
            Ok(project) => project,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "PROJECT_LOAD_FAILED",
                    format!("Failed to load left project at {:?}: {error}", left_path),
                )))
            }
        };
        if left.revision != params.left_expected_revision {
            return Ok(tool_error(StructuredError::new(
                "PROJECT_REVISION_CONFLICT",
                format!(
                    "Left project revision changed: expected {}, found {}",
                    params.left_expected_revision, left.revision
                ),
            )));
        }
        let right = match load_project(&right_path) {
            Ok(project) => project,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "PROJECT_LOAD_FAILED",
                    format!("Failed to load right project at {:?}: {error}", right_path),
                )))
            }
        };
        if right.revision != params.right_expected_revision {
            return Ok(tool_error(StructuredError::new(
                "PROJECT_REVISION_CONFLICT",
                format!(
                    "Right project revision changed: expected {}, found {}",
                    params.right_expected_revision, right.revision
                ),
            )));
        }
        Ok(tool_ok(&project_compare_report(&left, &right)))
    }

    #[tool(
        name = "tinixcut_media_probe",
        annotations(read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false),
        description = "Read media metadata through the bundled ffprobe abstraction without changing any project",
        output_schema = rmcp::handler::server::tool::schema_for_type::<MediaProbeReport>()
    )]
    async fn media_probe(
        &self,
        Parameters(params): Parameters<MediaProbeArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let media_path = PathBuf::from(&params.media_path);
        if !media_path.is_file() {
            return Ok(tool_error(StructuredError::new(
                "MEDIA_PROBE_FAILED",
                format!(
                    "Media file does not exist or is not a file: {:?}",
                    media_path
                ),
            )));
        }
        let file_size_bytes = match std::fs::metadata(&media_path) {
            Ok(metadata) => metadata.len(),
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "MEDIA_PROBE_FAILED",
                    format!(
                        "Could not read media file metadata at {:?}: {}",
                        media_path, error
                    ),
                )));
            }
        };
        let metadata = match FFmpegService::probe_file(&media_path) {
            Ok(metadata) => metadata,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "MEDIA_PROBE_FAILED",
                    format!("Failed to probe media at {:?}: {}", media_path, error),
                )));
            }
        };

        Ok(tool_ok(&MediaProbeReport {
            media_path: media_path.to_string_lossy().to_string(),
            file_size_bytes,
            duration_seconds: metadata.duration_seconds,
            width: metadata.width,
            height: metadata.height,
            frame_rate_numerator: metadata.frame_rate.numerator,
            frame_rate_denominator: metadata.frame_rate.denominator,
            video_codec: metadata.video_codec,
            audio_codec: metadata.audio_codec,
            sample_rate: metadata.sample_rate,
            channels: metadata.channels,
            streams: metadata
                .streams
                .into_iter()
                .map(|stream| MediaProbeStreamReport {
                    codec_type: stream.codec_type,
                    codec_name: stream.codec_name,
                    width: stream.width,
                    height: stream.height,
                    duration_seconds: stream.duration_seconds,
                    sample_rate: stream.sample_rate,
                    channels: stream.channels,
                })
                .collect(),
        }))
    }

    #[tool(
        name = "tinixcut_edit_plan_validate",
        annotations(read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false),
        description = "Validate a versioned edit plan against the current project revision and the canonical CommandService dry-run; never writes project state",
        output_schema = rmcp::handler::server::tool::schema_for_type::<EditPlanValidation>()
    )]
    async fn edit_plan_validate(
        &self,
        Parameters(params): Parameters<EditPlanValidateArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let plan: EditPlan = match serde_json::from_value(params.plan) {
            Ok(plan) => plan,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "EDIT_PLAN_INVALID",
                    format!("Edit plan does not match the supported versioned schema: {error}"),
                )))
            }
        };
        let project_path = PathBuf::from(&params.project_path);
        let project = match load_project(&project_path) {
            Ok(project) => project,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "PROJECT_LOAD_FAILED",
                    format!("Failed to load project at {:?}: {}", project_path, error),
                )))
            }
        };
        Ok(tool_ok(&validate_edit_plan(&project, &plan)))
    }

    #[tool(
        name = "tinixcut_edit_plan_draft",
        annotations(read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false),
        description = "Create and validate a read-only draft from explicitly selected video assets. Defaults to deterministic offline planning; optionally set plannerProvider=lm-studio-local to send the goal and selected clip names/durations to LM Studio on 127.0.0.1:1234. No media bytes or paths are sent, and the project is not mutated.",
        output_schema = rmcp::handler::server::tool::schema_for_type::<EditPlanDraftReceipt>()
    )]
    async fn edit_plan_draft(
        &self,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
        Parameters(params): Parameters<EditPlanDraftArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let project_path = PathBuf::from(&params.project_path);
        let project = match load_project(&project_path) {
            Ok(project) => project,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "PROJECT_LOAD_FAILED",
                    format!("Failed to load project at {:?}: {}", project_path, error),
                )))
            }
        };
        let duration = params.target_duration_frames.unwrap_or(900);
        let captions = match params.subtitle_file_path.as_deref() {
            Some(path) => {
                match parse_draft_caption_file(Path::new(path), project.settings.frame_rate) {
                    Ok(captions) => captions,
                    Err(error) => {
                        return Ok(tool_error(StructuredError::new(
                            "EDIT_PLAN_DRAFT_FAILED",
                            error,
                        )))
                    }
                }
            }
            None => Vec::new(),
        };
        let receipt = match params
            .planner_provider
            .as_deref()
            .unwrap_or(crate::modules::automation::edit_plan::DETERMINISTIC_DRAFT_PROVIDER)
        {
            crate::modules::automation::edit_plan::DETERMINISTIC_DRAFT_PROVIDER => {
                let plan = match build_deterministic_video_draft_with_supporting_media(
                    &project,
                    &params.goal,
                    &params.media_ids,
                    duration,
                    params.audio_asset_id.as_deref(),
                    &captions,
                ) {
                    Ok(plan) => plan,
                    Err(error) => {
                        return Ok(tool_error(StructuredError::new(
                            "EDIT_PLAN_DRAFT_FAILED",
                            error,
                        )))
                    }
                };
                let validation = validate_edit_plan(&project, &plan);
                EditPlanDraftReceipt {
                    provider: crate::modules::automation::edit_plan::DETERMINISTIC_DRAFT_PROVIDER
                        .to_string(),
                    plan,
                    validation,
                }
            }
            crate::modules::automation::edit_plan::LOCAL_LM_STUDIO_PROVIDER => {
                let generation = crate::modules::automation::edit_plan::build_local_ai_video_draft(
                    &project,
                    &params.goal,
                    &params.media_ids,
                    duration,
                    params.audio_asset_id.as_deref(),
                    &captions,
                    params
                        .local_model
                        .as_deref()
                        .unwrap_or("google/gemma-4-e4b"),
                );
                let result = crate::modules::automation::edit_plan::wait_for_local_planner_request(
                    generation,
                    context.ct.cancelled(),
                )
                .await;
                match result {
                    Ok(receipt) => receipt,
                    Err(error) => {
                        let code = if error.contains("cancelled") {
                            "OPERATION_CANCELLED"
                        } else {
                            "EDIT_PLAN_DRAFT_FAILED"
                        };
                        return Ok(tool_error(StructuredError::new(code, error)));
                    }
                }
            }
            provider => {
                return Ok(tool_error(StructuredError::new(
                    "EDIT_PLAN_DRAFT_FAILED",
                    format!("Unsupported edit-plan provider: {provider}"),
                )))
            }
        };
        if !receipt.validation.valid {
            return Ok(tool_error(StructuredError::new(
                "EDIT_PLAN_DRAFT_INVALID",
                receipt.validation.issues.join("; "),
            )));
        }
        Ok(tool_ok(&receipt))
    }

    #[tool(
        name = "tinixcut_edit_plan_scene_draft",
        annotations(read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false),
        description = "Create a read-only revision-bound edit-plan preview from explicitly ordered scene IDs in a local visual analysis report. Optional allowedAssetIds restricts all selected scenes to those source assets. Optional targetDurationFrames can be advisory or exactDuration; exact trims only selected source tails and returns an error when source capacity cannot satisfy the target. Optional targetCanvas adds a ProjectSettingsSet operation. Supply openingTitle together with titleTemplate to add a built-in styled title overlay; they are validated as a pair. Revalidates current media and fingerprints; applying remains a separate Command Service operation.",
        output_schema = rmcp::handler::server::tool::schema_for_type::<EditPlanDraftReceipt>()
    )]
    async fn edit_plan_scene_draft(
        &self,
        Parameters(params): Parameters<EditPlanSceneDraftArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let project_path = PathBuf::from(&params.project_path);
        let project = match load_project(&project_path) {
            Ok(project) => project,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "PROJECT_LOAD_FAILED",
                    format!("Failed to load project at {:?}: {}", project_path, error),
                )))
            }
        };
        match crate::modules::automation::edit_plan::build_visual_scene_edit_plan_with_constraints(
            &project,
            &params.goal,
            &params.report,
            &params.selected_scene_ids,
            params.target_duration_frames,
            params.exact_duration,
            crate::modules::automation::edit_plan::SceneSelectionConstraints {
                allowed_asset_ids: params.allowed_asset_ids,
                must_keep_scene_ids: params.must_keep_scene_ids,
                must_omit_scene_ids: params.must_omit_scene_ids,
                caption_intent: None,
                audio_intent: None,
                suggest_opening_title: false,
            },
            params.target_canvas,
            params.opening_title.as_deref(),
            params.title_template,
        ) {
            Ok(receipt) => Ok(tool_ok(&receipt)),
            Err(error) => Ok(tool_error(StructuredError::new(
                "SCENE_EDIT_PLAN_FAILED",
                error,
            ))),
        }
    }

    #[tool(
        name = "tinixcut_edit_plan_scene_selection_suggest",
        annotations(read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false),
        description = "Ask the explicitly selected local LM Studio model for a review-only subset and order of scene IDs from a supplied visual scene report. Optional allowedAssetIds limits which source scenes are sent to the model. Optional transcriptEvidence supplies source-timed local ASR excerpts; TinixCut validates current source fingerprints and includes only excerpts overlapping allowed scenes. Asset IDs, names, paths and fingerprints are never sent to the model. Optional captionIntent and audioIntent guide ranking only; they do not create subtitle or audio edits. Optional suggestOpeningTitle requests a bounded human-review suggestion. Validates the project/report and applies no edits.",
        output_schema = rmcp::handler::server::tool::schema_for_type::<crate::modules::automation::edit_plan::SceneSelectionSuggestion>()
    )]
    async fn edit_plan_scene_selection_suggest(
        &self,
        Parameters(params): Parameters<EditPlanSceneSelectionSuggestArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let project_path = PathBuf::from(&params.project_path);
        let project = match load_project(&project_path) {
            Ok(project) => project,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "PROJECT_LOAD_FAILED",
                    format!("Failed to load project at {:?}: {}", project_path, error),
                )))
            }
        };
        let suggestion =
            match crate::modules::automation::edit_plan::suggest_visual_scene_selection_with_constraints_and_transcript(
                &project,
                &params.goal,
                &params.report,
                params.target_duration_seconds,
                &params.local_model,
                crate::modules::automation::edit_plan::SceneSelectionConstraints {
                    allowed_asset_ids: params.allowed_asset_ids,
                    must_keep_scene_ids: params.must_keep_scene_ids,
                    must_omit_scene_ids: params.must_omit_scene_ids,
                    caption_intent: params.caption_intent,
                    audio_intent: params.audio_intent,
                    suggest_opening_title: params.suggest_opening_title,
                },
                params.transcript_evidence,
            )
            .await
            {
                Ok(suggestion) => suggestion,
                Err(error) => {
                    return Ok(tool_error(StructuredError::new(
                        "SCENE_SELECTION_SUGGESTION_FAILED",
                        error,
                    )))
                }
            };
        let current_project = match load_project(&project_path) {
            Ok(project) => project,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "PROJECT_RELOAD_FAILED",
                    format!("Failed to recheck project after local planning: {error}"),
                )))
            }
        };
        if current_project.project_id != project.project_id
            || current_project.revision != project.revision
        {
            return Ok(tool_error(StructuredError::new(
                "PROJECT_REVISION_CHANGED",
                "Project changed while the scene suggestion was generated; retry with a fresh report.",
            )));
        }
        Ok(tool_ok(&suggestion))
    }

    #[tool(
        name = "tinixcut_edit_plan_reorder",
        annotations(read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false),
        description = "Create a revision-checked preview plan to reorder every clip on a contiguous transition-free video track; supports caller order or explicit local AI planning",
        output_schema = rmcp::handler::server::tool::schema_for_type::<EditPlanDraftReceipt>()
    )]
    async fn edit_plan_reorder(
        &self,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
        Parameters(params): Parameters<EditPlanReorderArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let project_path = PathBuf::from(&params.project_path);
        let project = match load_project(&project_path) {
            Ok(project) => project,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "PROJECT_LOAD_FAILED",
                    format!("Failed to load project at {:?}: {}", project_path, error),
                )))
            }
        };
        let provider = params
            .planner_provider
            .as_deref()
            .unwrap_or(crate::modules::automation::edit_plan::CALLER_ORDER_PROVIDER);
        let receipt = match provider {
            crate::modules::automation::edit_plan::CALLER_ORDER_PROVIDER => {
                let plan = match crate::modules::automation::edit_plan::build_clip_reorder_plan(
                    &project,
                    &params.goal,
                    &params.track_id,
                    &params.item_ids,
                    &params.item_ids,
                ) {
                    Ok(plan) => plan,
                    Err(error) => {
                        return Ok(tool_error(StructuredError::new(
                            "EDIT_PLAN_REORDER_FAILED",
                            error,
                        )))
                    }
                };
                let validation = validate_edit_plan(&project, &plan);
                EditPlanDraftReceipt {
                    provider: provider.to_string(),
                    plan,
                    validation,
                }
            }
            crate::modules::automation::edit_plan::LOCAL_LM_STUDIO_PROVIDER => {
                let generation =
                    crate::modules::automation::edit_plan::build_local_ai_clip_reorder_plan(
                        &project,
                        &params.goal,
                        &params.track_id,
                        &params.item_ids,
                        params
                            .local_model
                            .as_deref()
                            .unwrap_or("google/gemma-4-e4b"),
                    );
                match crate::modules::automation::edit_plan::wait_for_local_planner_request(
                    generation,
                    context.ct.cancelled(),
                )
                .await
                {
                    Ok(receipt) => receipt,
                    Err(error) => {
                        let code = if error.contains("cancelled") {
                            "OPERATION_CANCELLED"
                        } else {
                            "EDIT_PLAN_REORDER_FAILED"
                        };
                        return Ok(tool_error(StructuredError::new(code, error)));
                    }
                }
            }
            unsupported => {
                return Ok(tool_error(StructuredError::new(
                    "EDIT_PLAN_REORDER_FAILED",
                    format!("Unsupported reorder planner provider: {unsupported}"),
                )))
            }
        };
        if !receipt.validation.valid {
            return Ok(tool_error(StructuredError::new(
                "EDIT_PLAN_REORDER_INVALID",
                receipt.validation.issues.join("; "),
            )));
        }
        Ok(tool_ok(&receipt))
    }

    #[tool(
        name = "tinixcut_edit_plan_trim",
        annotations(read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false),
        description = "Create a revision-bound preview plan to shorten the tail of one video clip; preserves its source in-point and clip speed and never mutates the project",
        output_schema = rmcp::handler::server::tool::schema_for_type::<EditPlanDraftReceipt>()
    )]
    async fn edit_plan_trim(
        &self,
        Parameters(params): Parameters<EditPlanTrimArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let project_path = PathBuf::from(&params.project_path);
        let project = match load_project(&project_path) {
            Ok(project) => project,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "PROJECT_LOAD_FAILED",
                    format!("Failed to load project at {:?}: {}", project_path, error),
                )))
            }
        };
        let plan = match crate::modules::automation::edit_plan::build_clip_trim_plan(
            &project,
            &params.goal,
            &params.item_id,
            params.target_duration_frames,
        ) {
            Ok(plan) => plan,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "EDIT_PLAN_TRIM_FAILED",
                    error,
                )))
            }
        };
        let validation = validate_edit_plan(&project, &plan);
        if !validation.valid {
            return Ok(tool_error(StructuredError::new(
                "EDIT_PLAN_TRIM_INVALID",
                validation.issues.join("; "),
            )));
        }
        Ok(tool_ok(&EditPlanDraftReceipt {
            provider: "caller-trim-v1".to_string(),
            plan,
            validation,
        }))
    }

    #[tool(
        name = "tinixcut_edit_plan_volume",
        annotations(read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false),
        description = "Create a revision-bound preview plan to set one audio or video clip's linear volume; never mutates the project",
        output_schema = rmcp::handler::server::tool::schema_for_type::<EditPlanDraftReceipt>()
    )]
    async fn edit_plan_volume(
        &self,
        Parameters(params): Parameters<EditPlanVolumeArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let project_path = PathBuf::from(&params.project_path);
        let project = match load_project(&project_path) {
            Ok(project) => project,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "PROJECT_LOAD_FAILED",
                    format!("Failed to load project at {:?}: {}", project_path, error),
                )))
            }
        };
        let plan = match crate::modules::automation::edit_plan::build_clip_volume_plan(
            &project,
            &params.goal,
            &params.item_id,
            params.volume,
        ) {
            Ok(plan) => plan,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "EDIT_PLAN_VOLUME_FAILED",
                    error,
                )))
            }
        };
        let validation = validate_edit_plan(&project, &plan);
        if !validation.valid {
            return Ok(tool_error(StructuredError::new(
                "EDIT_PLAN_VOLUME_INVALID",
                validation.issues.join("; "),
            )));
        }
        Ok(tool_ok(&EditPlanDraftReceipt {
            provider: "caller-volume-v1".to_string(),
            plan,
            validation,
        }))
    }

    #[tool(
        name = "tinixcut_edit_plan_canvas",
        annotations(read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false),
        description = "Create a revision-bound preview plan to change canvas dimensions; does not automatically reposition or crop existing timeline items",
        output_schema = rmcp::handler::server::tool::schema_for_type::<EditPlanDraftReceipt>()
    )]
    async fn edit_plan_canvas(
        &self,
        Parameters(params): Parameters<EditPlanCanvasArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let project_path = PathBuf::from(&params.project_path);
        let project = match load_project(&project_path) {
            Ok(project) => project,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "PROJECT_LOAD_FAILED",
                    format!("Failed to load project at {:?}: {}", project_path, error),
                )))
            }
        };
        let plan = match crate::modules::automation::edit_plan::build_canvas_resize_plan(
            &project,
            &params.goal,
            params.width,
            params.height,
        ) {
            Ok(plan) => plan,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "EDIT_PLAN_CANVAS_FAILED",
                    error,
                )))
            }
        };
        let validation = validate_edit_plan(&project, &plan);
        if !validation.valid {
            return Ok(tool_error(StructuredError::new(
                "EDIT_PLAN_CANVAS_INVALID",
                validation.issues.join("; "),
            )));
        }
        Ok(tool_ok(&EditPlanDraftReceipt {
            provider: "caller-canvas-v1".to_string(),
            plan,
            validation,
        }))
    }

    #[tool(
        name = "tinixcut_edit_plan_text_replace",
        annotations(read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false),
        description = "Create a revision-bound review plan that searches explicitly selected caption items for a literal phrase and replaces it; never mutates the project",
        output_schema = rmcp::handler::server::tool::schema_for_type::<EditPlanDraftReceipt>()
    )]
    async fn edit_plan_text_replace(
        &self,
        Parameters(params): Parameters<EditPlanTextReplaceArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let project_path = PathBuf::from(&params.project_path);
        let project = match load_project(&project_path) {
            Ok(project) => project,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "PROJECT_LOAD_FAILED",
                    format!("Failed to load project at {:?}: {}", project_path, error),
                )))
            }
        };
        let plan = match crate::modules::automation::edit_plan::build_text_replace_plan(
            &project,
            &params.goal,
            &params.item_ids,
            &params.search_text,
            &params.replacement_text,
        ) {
            Ok(plan) => plan,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "EDIT_PLAN_TEXT_REPLACE_FAILED",
                    error,
                )))
            }
        };
        let validation = validate_edit_plan(&project, &plan);
        if !validation.valid {
            return Ok(tool_error(StructuredError::new(
                "EDIT_PLAN_TEXT_REPLACE_INVALID",
                validation.issues.join("; "),
            )));
        }
        Ok(tool_ok(&EditPlanDraftReceipt {
            provider: "caller-text-replace-v1".to_string(),
            plan,
            validation,
        }))
    }

    #[tool(
        name = "tinixcut_media_scene_candidates",
        annotations(read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false),
        description = "Find bounded scene-boundary candidates in a media file without changing a project; supports request cancellation",
        output_schema = rmcp::handler::server::tool::schema_for_type::<crate::modules::media::scene::SceneAnalysisReport>()
    )]
    async fn media_scene_candidates(
        &self,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
        Parameters(params): Parameters<MediaSceneCandidatesArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let source = PathBuf::from(&params.media_path);
        let threshold = params.threshold.unwrap_or(0.3);
        let max_duration = params.max_duration_seconds.unwrap_or(300);
        let max_candidates = params.max_candidates.unwrap_or(500);
        if !source.is_file() {
            return Ok(tool_error(StructuredError::new(
                "MEDIA_SCENE_ANALYSIS_INVALID",
                format!("Media file does not exist or is not a file: {:?}", source),
            )));
        }
        let source_fingerprint = match FFmpegService::source_fingerprint(&source) {
            Ok(value) => value,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "MEDIA_SCENE_ANALYSIS_FAILED",
                    error.to_string(),
                )))
            }
        };
        let config_fingerprint = FFmpegService::scene_analysis_config_fingerprint(
            threshold,
            max_duration,
            max_candidates,
        );
        let cache = crate::modules::media::cache::CacheManager::new(
            std::env::temp_dir().join("tinixcut_cache"),
        );
        if let Some(report) = cache.load_scene_analysis(&source_fingerprint, &config_fingerprint) {
            return Ok(tool_ok(&report));
        }

        let job_id = format!("scene_{}", uuid::Uuid::new_v4().simple());
        let permit = match self.worker_capacity.try_acquire(&job_id) {
            Ok(permit) => permit,
            Err(CapacityError::Full) => {
                return Ok(tool_error(StructuredError::new(
                    "ANALYSIS_CAPACITY",
                    "All media analysis workers are busy",
                )))
            }
            Err(CapacityError::AlreadyActive(_)) => {
                return Ok(tool_error(StructuredError::new(
                    "ANALYSIS_CAPACITY",
                    "Scene analysis job is already active",
                )))
            }
        };
        let cancel_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker_cancel = cancel_flag.clone();
        let worker_cache = cache;
        let task = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let report = FFmpegService::analyze_scene_candidates(
                &source,
                threshold,
                max_duration,
                max_candidates,
                &worker_cancel,
            )?;
            let _ = worker_cache.save_scene_analysis(&report);
            Ok::<_, crate::modules::media::MediaError>(report)
        });
        tokio::select! {
            result = task => match result {
                Ok(Ok(report)) => Ok(tool_ok(&report)),
                Ok(Err(crate::modules::media::MediaError::Cancelled)) => Ok(tool_error(StructuredError::new("OPERATION_CANCELLED", "Scene analysis was cancelled"))),
                Ok(Err(error)) => Ok(tool_error(StructuredError::new("MEDIA_SCENE_ANALYSIS_FAILED", error.to_string()))),
                Err(error) => Ok(tool_error(StructuredError::new("MEDIA_SCENE_ANALYSIS_FAILED", error.to_string()))),
            },
            _ = context.ct.cancelled() => {
                cancel_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(tool_error(StructuredError::new("OPERATION_CANCELLED", "Scene analysis was cancelled")))
            }
        }
    }

    #[tool(
        name = "tinixcut_media_visual_scenes",
        annotations(read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false),
        description = "Describe and rank bounded representative frames from explicitly selected local videos using a local LM Studio vision model. Read-only; media paths are not sent to the model or returned.",
        output_schema = rmcp::handler::server::tool::schema_for_type::<crate::modules::media::visual_scene::VisualSceneAnalysisReport>()
    )]
    async fn media_visual_scenes(
        &self,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
        Parameters(params): Parameters<MediaVisualScenesArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let max_scenes = params.max_scenes.unwrap_or(16);
        let sources = params
            .media
            .iter()
            .map(
                |source| crate::modules::media::visual_scene::VisualSceneSource {
                    asset_id: source.asset_id.clone(),
                    asset_name: source
                        .display_name
                        .clone()
                        .unwrap_or_else(|| source.asset_id.clone()),
                    path: PathBuf::from(&source.media_path),
                },
            )
            .collect::<Vec<_>>();
        let source_fingerprint =
            match crate::modules::media::visual_scene::aggregate_source_fingerprint(&sources) {
                Ok(value) => value,
                Err(error) => {
                    return Ok(tool_error(StructuredError::new(
                        "VISUAL_SCENE_ANALYSIS_INVALID",
                        error,
                    )))
                }
            };
        let config_fingerprint = crate::modules::media::visual_scene::config_fingerprint(
            &params.model,
            &params.query,
            max_scenes,
        );
        let cache = crate::modules::media::cache::CacheManager::new(
            std::env::temp_dir().join("tinixcut_cache"),
        );
        if let Some(report) =
            cache.load_visual_scene_analysis(&source_fingerprint, &config_fingerprint)
        {
            return Ok(tool_ok(&report));
        }
        let job_id = format!("visual_scene_{}", uuid::Uuid::new_v4().simple());
        let permit = match self.worker_capacity.try_acquire(&job_id) {
            Ok(permit) => permit,
            Err(CapacityError::Full | CapacityError::AlreadyActive(_)) => {
                return Ok(tool_error(StructuredError::new(
                    "ANALYSIS_CAPACITY",
                    "All media analysis workers are busy",
                )))
            }
        };
        let cancel_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel_flag);
        let worker_cache = cache;
        let query = params.query;
        let model = params.model;
        let task = tokio::spawn(async move {
            let _permit = permit;
            let report = crate::modules::media::visual_scene::analyze_visual_scenes(
                &sources,
                &query,
                &model,
                max_scenes,
                worker_cancel,
            )
            .await?;
            let _ = worker_cache.save_visual_scene_analysis(&report);
            Ok::<_, String>(report)
        });
        tokio::select! {
            result = task => match result {
                Ok(Ok(report)) => Ok(tool_ok(&report)),
                Ok(Err(error)) => Ok(tool_error(StructuredError::new("VISUAL_SCENE_ANALYSIS_FAILED", error))),
                Err(error) => Ok(tool_error(StructuredError::new("VISUAL_SCENE_ANALYSIS_FAILED", error.to_string()))),
            },
            _ = context.ct.cancelled() => {
                cancel_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(tool_error(StructuredError::new("OPERATION_CANCELLED", "Visual scene analysis was cancelled")))
            }
        }
    }

    #[tool(
        name = "tinixcut_media_transcript",
        annotations(read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false),
        description = "Transcribe an audio or video file with an explicitly supplied local Whisper CLI and model; returns source-time segments without modifying any project and supports cancellation",
        output_schema = rmcp::handler::server::tool::schema_for_type::<crate::modules::media::transcription::TranscriptAnalysisReport>()
    )]
    async fn media_transcript(
        &self,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
        Parameters(params): Parameters<MediaTranscriptArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let media_path = PathBuf::from(&params.media_path);
        let cli_path = PathBuf::from(&params.whisper_cli_path);
        let model_path = PathBuf::from(&params.model_path);
        if !media_path.is_file() {
            return Ok(tool_error(StructuredError::new(
                "MEDIA_TRANSCRIPT_INVALID",
                "Media file does not exist or is not a file",
            )));
        }
        if !cli_path.is_file() {
            return Ok(tool_error(StructuredError::new(
                "WHISPER_CLI_MISSING",
                format!(
                    "Whisper CLI executable was not found: {}",
                    cli_path.display()
                ),
            )));
        }
        if !model_path.is_file() {
            return Ok(tool_error(StructuredError::new(
                "WHISPER_MODEL_MISSING",
                format!("Whisper model was not found: {}", model_path.display()),
            )));
        }
        let source_fingerprint = match FFmpegService::source_fingerprint(&media_path) {
            Ok(value) => value,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "MEDIA_TRANSCRIPT_FAILED",
                    error.to_string(),
                )))
            }
        };
        let model_fingerprint = match FFmpegService::source_fingerprint(&model_path) {
            Ok(value) => value,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "WHISPER_MODEL_MISSING",
                    error.to_string(),
                )))
            }
        };
        let cli_fingerprint = match FFmpegService::source_fingerprint(&cli_path) {
            Ok(value) => value,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "WHISPER_CLI_MISSING",
                    error.to_string(),
                )))
            }
        };
        let language = params.language.unwrap_or_else(|| "auto".to_string());
        let config_material = format!(
            "{}|{}|{}|{}",
            crate::modules::media::transcription::TRANSCRIPT_ANALYSIS_ALGORITHM_VERSION,
            model_fingerprint,
            cli_fingerprint,
            language
        );
        let config_fingerprint = Sha256::digest(config_material.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let cache = crate::modules::media::cache::CacheManager::new(
            std::env::temp_dir().join("tinixcut_cache"),
        );
        if let Some(report) =
            cache.load_transcript_analysis(&source_fingerprint, &config_fingerprint)
        {
            return Ok(tool_ok(&report));
        }
        let job_id = format!("transcript_{}", uuid::Uuid::new_v4().simple());
        let permit = match self.worker_capacity.try_acquire(&job_id) {
            Ok(permit) => permit,
            Err(CapacityError::Full) => {
                return Ok(tool_error(StructuredError::new(
                    "ANALYSIS_CAPACITY",
                    "All media analysis workers are busy",
                )))
            }
            Err(CapacityError::AlreadyActive(_)) => {
                return Ok(tool_error(StructuredError::new(
                    "ANALYSIS_CAPACITY",
                    "Transcript job is already active",
                )))
            }
        };
        let cancel_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker_cancel = cancel_flag.clone();
        let worker_cache = cache;
        let task = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let (result, analyzed_duration_micros) = crate::modules::media::transcription::TranscriptionBoundary::analyze_media_read_only(
                &media_path, &cli_path, &model_path, Some(&language), &worker_cancel,
            ).map_err(|error| error.to_string())?;
            let report = crate::modules::media::transcription::TranscriptAnalysisReport {
                schema_version:
                    crate::modules::media::transcription::TRANSCRIPT_ANALYSIS_SCHEMA_VERSION,
                algorithm_version:
                    crate::modules::media::transcription::TRANSCRIPT_ANALYSIS_ALGORITHM_VERSION
                        .to_string(),
                source_fingerprint,
                config_fingerprint,
                analyzed_duration_micros,
                result,
            };
            let _ = worker_cache.save_transcript_analysis(&report);
            Ok::<_, String>(report)
        });
        tokio::select! {
            result = task => match result {
                Ok(Ok(report)) => Ok(tool_ok(&report)),
                Ok(Err(error)) if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) => Ok(tool_error(StructuredError::new("OPERATION_CANCELLED", error))),
                Ok(Err(error)) => Ok(tool_error(StructuredError::new("MEDIA_TRANSCRIPT_FAILED", error))),
                Err(error) => Ok(tool_error(StructuredError::new("MEDIA_TRANSCRIPT_FAILED", error.to_string()))),
            },
            _ = context.ct.cancelled() => {
                cancel_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(tool_error(StructuredError::new("OPERATION_CANCELLED", "Transcript analysis was cancelled")))
            }
        }
    }

    #[tool(
        name = "tinixcut_inspect",
        annotations(read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false),
        description = "Inspect an existing TinixCut project file and return timeline metadata, tracks, assets, and revision",
        output_schema = rmcp::handler::server::tool::schema_for_type::<ProjectInspectReport>()
    )]
    async fn inspect(
        &self,
        Parameters(params): Parameters<InspectArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let project_path = PathBuf::from(&params.project_path);
        let project = match load_project(&project_path) {
            Ok(p) => p,
            Err(e) => {
                return Ok(tool_error(StructuredError::new(
                    "PROJECT_LOAD_FAILED",
                    format!("Failed to load project at {:?}: {}", project_path, e),
                )));
            }
        };
        let report = AutomationAdapter::inspect_project(&project);
        Ok(tool_ok(&report))
    }

    #[tool(
        name = "tinixcut_capabilities",
        annotations(read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false),
        description = "Return editor capabilities, supported operations, transitions, keyframing, and export formats",
        output_schema = rmcp::handler::server::tool::schema_for_type::<CapabilitiesReport>()
    )]
    async fn capabilities(
        &self,
        Parameters(_params): Parameters<CapabilitiesArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let report = CapabilitiesReport {
            supported_operations: AutomationAdapter::supported_operations(),
            supported_transitions: AutomationAdapter::supported_transitions(),
            supported_export_codecs: vec![
                "libx264".to_string(),
                "h264_nvenc".to_string(),
                "h264_qsv".to_string(),
                "h264_amf".to_string(),
            ],
            protocol_version: 1,
            schema_version: crate::modules::domain::CURRENT_SCHEMA_VERSION,
            supported_keyframe_properties: AutomationAdapter::supported_keyframe_properties(),
            supported_keyframe_easings: AutomationAdapter::supported_keyframe_easings(),
            custom_cubic_bounds: (0.0, 1.0),
            supported_animation_units: AutomationAdapter::supported_animation_units(),
            supported_stagger_orders: AutomationAdapter::supported_stagger_orders(),
            supported_loop_modes: AutomationAdapter::supported_loop_modes(),
            supported_duration_conflict_policies:
                AutomationAdapter::supported_duration_conflict_policies(),
            template_capabilities: Some(
                crate::modules::templates::agent::AgentTemplateService::capabilities(),
            ),
        };
        Ok(tool_ok(&report))
    }

    #[tool(
        name = "tinixcut_dry_run",
        annotations(read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false),
        description = "Perform a transactional dry run of a CommandEnvelope against a project file without modifying disk state",
        output_schema = rmcp::handler::server::tool::schema_for_type::<DiffReport>()
    )]
    async fn dry_run(
        &self,
        Parameters(params): Parameters<DryRunArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let project_path = PathBuf::from(&params.project_path);
        let project = match load_project(&project_path) {
            Ok(p) => p,
            Err(e) => {
                return Ok(tool_error(StructuredError::new(
                    "PROJECT_LOAD_FAILED",
                    format!("Failed to load project at {:?}: {}", project_path, e),
                )));
            }
        };
        if params.envelope.project_id != project.project_id {
            return Ok(tool_error(StructuredError::with_details(
                "PROJECT_ID_MISMATCH",
                format!(
                    "Project ID mismatch: expected {}, received {}",
                    project.project_id, params.envelope.project_id
                ),
                serde_json::json!({
                    "expected": project.project_id,
                    "received": params.envelope.project_id
                }),
            )));
        }
        let service = CommandService::new(project);
        match service.dry_run(&params.envelope) {
            Ok(diff) => {
                if let Ok(mut confirmed) = self.confirmed_dry_runs.lock() {
                    confirmed.insert(dry_run_confirmation_key(
                        &params.project_path,
                        &params.envelope,
                    ));
                }
                Ok(tool_ok(&diff))
            }
            Err(err) => Ok(tool_error(map_command_error(err))),
        }
    }

    #[tool(
        name = "tinixcut_apply",
        annotations(read_only_hint = false, destructive_hint = true, idempotent_hint = false, open_world_hint = false),
        description = "Execute a transactional CommandEnvelope mutation against a project file and atomically persist to disk",
        output_schema = rmcp::handler::server::tool::schema_for_type::<CommandReceipt>()
    )]
    async fn apply(
        &self,
        Parameters(params): Parameters<ApplyArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        // Enforce process-local authoritative-write serialization gate
        let _guard = self.write_gate.lock().await;

        let envelope = params.envelope;

        // Structured identity validation (ADR-001 v3 & ADR-005 Section 3.10)
        if envelope
            .client_id
            .as_ref()
            .is_none_or(|id| id.trim().is_empty() || id.len() > 64)
        {
            return Ok(tool_error(StructuredError::new(
                "OPERATION_VALIDATION_FAILED",
                "Missing or invalid client_id: caller must supply a non-empty client_id (<= 64 chars)",
            )));
        }

        if envelope.client_epoch.is_none() {
            return Ok(tool_error(StructuredError::new(
                "OPERATION_VALIDATION_FAILED",
                "Missing client_epoch: caller must supply client_epoch for durable idempotency",
            )));
        }

        if envelope.sequence_number.is_none() || envelope.sequence_number == Some(0) {
            return Ok(tool_error(StructuredError::new(
                "OPERATION_VALIDATION_FAILED",
                "Missing or invalid sequence_number: caller must supply positive sequence_number >= 1",
            )));
        }

        if envelope
            .transaction_id
            .as_ref()
            .is_none_or(|tid| tid.trim().is_empty())
        {
            return Ok(tool_error(StructuredError::new(
                "OPERATION_VALIDATION_FAILED",
                "Missing transaction_id: caller must supply transaction_id for atomic envelope",
            )));
        }

        let confirmation_key = dry_run_confirmation_key(&params.project_path, &envelope);
        if self.require_destructive_confirmation
            && envelope
                .operations
                .iter()
                .any(|op| op.operation.is_destructive())
        {
            let confirmed = self
                .confirmed_dry_runs
                .lock()
                .is_ok_and(|set| set.contains(&confirmation_key));
            if !confirmed {
                return Ok(tool_error(StructuredError::new(
                    "DESTRUCTIVE_CONFIRMATION_REQUIRED",
                    "This envelope deletes project content. Call tinixcut_dry_run with the identical envelope first, review the diff, then retry tinixcut_apply",
                )));
            }
        }

        let project_path = PathBuf::from(&params.project_path);
        let project = match load_project(&project_path) {
            Ok(p) => p,
            Err(e) => {
                return Ok(tool_error(StructuredError::new(
                    "PROJECT_LOAD_FAILED",
                    format!("Failed to load project at {:?}: {}", project_path, e),
                )));
            }
        };
        let expected_disk_revision = project.revision;

        if envelope.project_id != project.project_id {
            return Ok(tool_error(StructuredError::with_details(
                "PROJECT_ID_MISMATCH",
                format!(
                    "Project ID mismatch: expected {}, received {}",
                    project.project_id, envelope.project_id
                ),
                serde_json::json!({
                    "expected": project.project_id,
                    "received": envelope.project_id
                }),
            )));
        }

        let mut service = CommandService::new(project);
        let receipt = match service.execute(envelope) {
            Ok(r) => r,
            Err(err) => {
                return Ok(tool_error(map_command_error(err)));
            }
        };

        // Enforce durable atomic persistence before returning success
        if let Err(error) = save_project_atomic_if_revision(
            service.project(),
            &project_path,
            Some(expected_disk_revision),
        ) {
            return Ok(tool_error(map_project_save_error(
                error,
                "commit project update",
            )));
        }

        if let Ok(mut confirmed) = self.confirmed_dry_runs.lock() {
            confirmed.remove(&confirmation_key);
        }

        Ok(tool_ok(&receipt))
    }

    #[tool(
        name = "tinixcut_save",
        annotations(read_only_hint = false, destructive_hint = true, idempotent_hint = false, open_world_hint = false),
        description = "Atomically save or save-as an existing project file from disk to projectPath or targetPath",
        output_schema = rmcp::handler::server::tool::schema_for_type::<SaveReceipt>()
    )]
    async fn save(
        &self,
        Parameters(params): Parameters<SaveArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let _guard = self.write_gate.lock().await;

        let source_path = PathBuf::from(&params.project_path);
        let project = match load_project(&source_path) {
            Ok(p) => p,
            Err(e) => {
                return Ok(tool_error(StructuredError::new(
                    "PROJECT_LOAD_FAILED",
                    format!("Failed to load project at {:?}: {}", source_path, e),
                )));
            }
        };

        let dest_path = params
            .target_path
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| source_path.clone());
        let same_path = match same_project_path(&dest_path, &source_path) {
            Ok(same_path) => same_path,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "PROJECT_LOAD_FAILED",
                    format!("Failed to inspect save target {:?}: {}", dest_path, error),
                )));
            }
        };
        let expected_revision = if same_path {
            Some(project.revision)
        } else {
            match load_project_revision_if_exists(&dest_path) {
                Ok(revision) => revision,
                Err(error) => {
                    return Ok(tool_error(StructuredError::new(
                        "PROJECT_LOAD_FAILED",
                        format!("Failed to inspect save target {:?}: {}", dest_path, error),
                    )));
                }
            }
        };

        if let Err(error) = save_project_atomic_if_revision(&project, &dest_path, expected_revision)
        {
            return Ok(tool_error(map_project_save_error(
                error,
                &format!("save project to disk at {:?}", dest_path),
            )));
        }

        Ok(tool_ok(&SaveReceipt {
            saved_path: dest_path.to_string_lossy().to_string(),
        }))
    }

    #[tool(
        name = "tinixcut_workflow_manifest_create",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = false, open_world_hint = false),
        description = "Validate a revision-bound edit plan and persist a non-sensitive workflow manifest containing project revision, plan hash, operation IDs, media fingerprints and artifact paths. Refuses to overwrite an existing manifest; does not apply edits or store brief text, credentials, media bytes or source paths.",
        output_schema = rmcp::handler::server::tool::schema_for_type::<WorkflowManifestCreateReceipt>()
    )]
    async fn workflow_manifest_create(
        &self,
        Parameters(params): Parameters<WorkflowManifestCreateArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let project_path = PathBuf::from(&params.project_path);
        let project = match load_project(&project_path) {
            Ok(project) => project,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "PROJECT_LOAD_FAILED",
                    format!("Failed to load project snapshot: {error}"),
                )))
            }
        };
        let plan: EditPlan = match serde_json::from_value(params.plan) {
            Ok(plan) => plan,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "WORKFLOW_MANIFEST_INVALID",
                    format!("Edit plan payload is invalid: {error}"),
                )))
            }
        };
        let manifest = match WorkflowManifest::create(&project, &plan, &params.artifact_paths) {
            Ok(manifest) => manifest,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "WORKFLOW_MANIFEST_INVALID",
                    error,
                )))
            }
        };
        let manifest_path = PathBuf::from(&params.manifest_path);
        match manifest.save_new(&manifest_path) {
            Ok(path) => Ok(tool_ok(&WorkflowManifestCreateReceipt {
                manifest_path: path.to_string_lossy().to_string(),
                manifest,
            })),
            Err(error) => Ok(tool_error(StructuredError::new(
                "WORKFLOW_MANIFEST_PERSIST_FAILED",
                error,
            ))),
        }
    }

    #[tool(
        name = "tinixcut_workflow_manifest_validate",
        annotations(read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false),
        description = "Read a persisted workflow manifest and validate its planned state, project revision, exact edit plan identity and current media fingerprints before a client retries or resumes. This tool is read-only and never applies edits.",
        output_schema = rmcp::handler::server::tool::schema_for_type::<WorkflowManifestValidateReceipt>()
    )]
    async fn workflow_manifest_validate(
        &self,
        Parameters(params): Parameters<WorkflowManifestValidateArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let project_path = PathBuf::from(&params.project_path);
        let project = match load_project(&project_path) {
            Ok(project) => project,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "PROJECT_LOAD_FAILED",
                    format!("Failed to load project snapshot: {error}"),
                )))
            }
        };
        let manifest_path = PathBuf::from(&params.manifest_path);
        let manifest_bytes = match std::fs::read(&manifest_path) {
            Ok(bytes) => bytes,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "WORKFLOW_MANIFEST_LOAD_FAILED",
                    format!("Could not read workflow manifest: {error}"),
                )))
            }
        };
        let manifest: WorkflowManifest = match serde_json::from_slice(&manifest_bytes) {
            Ok(manifest) => manifest,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "WORKFLOW_MANIFEST_INVALID",
                    format!("Workflow manifest payload is invalid: {error}"),
                )))
            }
        };
        let plan: EditPlan = match serde_json::from_value(params.plan) {
            Ok(plan) => plan,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "WORKFLOW_MANIFEST_INVALID",
                    format!("Edit plan payload is invalid: {error}"),
                )))
            }
        };
        if let Err(error) = manifest.validate_resume(&project, &plan) {
            return Ok(tool_error(StructuredError::new(
                "WORKFLOW_RESUME_REJECTED",
                error,
            )));
        }
        Ok(tool_ok(&WorkflowManifestValidateReceipt {
            valid: true,
            workflow_id: manifest.workflow_id,
            state: manifest.state,
            project_revision: project.revision,
        }))
    }

    #[tool(
        name = "tinixcut_workflow_manifest_checkpoint",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = false, open_world_hint = false),
        description = "Record an immutable applied, saved, or exported checkpoint for a planned workflow. Applied is verified against the durable operation ledger; saved is verified from the current project file; exported is verified with ffprobe and a streaming SHA-256. Retries are idempotent only for identical evidence, and existing checkpoints are never overwritten.",
        output_schema = rmcp::handler::server::tool::schema_for_type::<crate::modules::automation::workflow_manifest::WorkflowCheckpointReceipt>()
    )]
    async fn workflow_manifest_checkpoint(
        &self,
        Parameters(params): Parameters<WorkflowManifestCheckpointArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let project_path = PathBuf::from(&params.project_path);
        let project = match load_project(&project_path) {
            Ok(project) => project,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "PROJECT_LOAD_FAILED",
                    format!("Could not load saved project for workflow checkpoint: {error}"),
                )))
            }
        };
        let manifest_path = PathBuf::from(&params.manifest_path);
        let manifest_bytes = match std::fs::read(&manifest_path) {
            Ok(bytes) => bytes,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "WORKFLOW_MANIFEST_LOAD_FAILED",
                    format!("Could not read workflow manifest: {error}"),
                )))
            }
        };
        let manifest: WorkflowManifest = match serde_json::from_slice(&manifest_bytes) {
            Ok(manifest) => manifest,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "WORKFLOW_MANIFEST_INVALID",
                    format!("Workflow manifest payload is invalid: {error}"),
                )))
            }
        };
        let plan: EditPlan = match serde_json::from_value(params.plan) {
            Ok(plan) => plan,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "WORKFLOW_MANIFEST_INVALID",
                    format!("Edit plan payload is invalid: {error}"),
                )))
            }
        };
        match manifest.record_checkpoint(
            &manifest_path,
            &project,
            &plan,
            params.state,
            params.artifact_path.as_deref().map(Path::new),
        ) {
            Ok(receipt) => Ok(tool_ok(&receipt)),
            Err(error) => Ok(tool_error(StructuredError::new(
                "WORKFLOW_CHECKPOINT_REJECTED",
                error,
            ))),
        }
    }

    #[tool(
        name = "tinixcut_export_preflight",
        annotations(read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false),
        description = "Read-only export readiness check bound to the saved project snapshot and output path. Validates the project-matching profile, compiles the canonical composition, probes referenced media, and reports existing-output and free-disk information without starting a job or changing the project.",
        output_schema = rmcp::handler::server::tool::schema_for_type::<crate::modules::export::ExportPreflightReport>()
    )]
    async fn export_preflight(
        &self,
        Parameters(params): Parameters<ExportPreflightArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let project_path = PathBuf::from(&params.project_path);
        let snapshot = match load_project(&project_path) {
            Ok(project) => project,
            Err(error) => {
                return Ok(tool_error(StructuredError::new(
                    "PROJECT_LOAD_FAILED",
                    format!("Could not load project for export preflight: {error}"),
                )))
            }
        };
        let profile = params
            .profile
            .unwrap_or_else(|| ExportProfile::from_project_settings(&snapshot.settings));
        match ExportEngine::preflight(&snapshot, Path::new(&params.output_path), &profile) {
            Ok(report) => Ok(tool_ok(&report)),
            Err(error) => Ok(tool_error(StructuredError::new(
                "EXPORT_PREFLIGHT_FAILED",
                error.to_string(),
            ))),
        }
    }

    #[tool(
        name = "tinixcut_export_start",
        annotations(read_only_hint = false, destructive_hint = true, idempotent_hint = false, open_world_hint = false),
        description = "Start an asynchronous export render job using JobManager and return a jobId",
        output_schema = rmcp::handler::server::tool::schema_for_type::<ExportStartReceipt>()
    )]
    async fn export_start(
        &self,
        Parameters(params): Parameters<ExportStartArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let project_path = PathBuf::from(&params.project_path);
        let snapshot = match load_project(&project_path) {
            Ok(p) => p,
            Err(e) => {
                return Ok(tool_error(StructuredError::new(
                    "PROJECT_LOAD_FAILED",
                    format!(
                        "Failed to load project snapshot at {:?}: {}",
                        project_path, e
                    ),
                )));
            }
        };

        let target_path = PathBuf::from(&params.output_path);
        let profile = params
            .profile
            .unwrap_or_else(|| ExportProfile::from_project_settings(&snapshot.settings));
        if let Err(error) = ExportEngine::preflight(&snapshot, &target_path, &profile) {
            return Ok(tool_error(StructuredError::new(
                "EXPORT_PREFLIGHT_FAILED",
                error.to_string(),
            )));
        }

        let dedupe_key = format!("export:{}:{}", params.project_path, params.output_path);
        let job_id = format!("export_{}", uuid::Uuid::new_v4().simple());

        let (jid, deduplicated) = {
            let mut jm = match self.jobs.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            jm.submit_job(job_id, JobKind::Export, dedupe_key, 100)
        };

        if deduplicated {
            return Ok(tool_ok(&ExportStartReceipt {
                job_id: jid,
                status: "running".to_string(),
                output_path: params.output_path,
            }));
        }

        let cancel_flag = {
            let mut jm = match self.jobs.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            match jm.start_job(&jid) {
                Ok(flag) => flag,
                Err(err) => {
                    return Ok(tool_error(StructuredError::new("EXPORT_FAILED", err)));
                }
            }
        };

        let prof = profile;
        let jobs_arc = Arc::clone(&self.jobs);
        let active_jid = jid.clone();

        std::thread::spawn(move || {
            let render_res =
                ExportEngine::render_snapshot(&snapshot, &target_path, &prof, Some(cancel_flag));
            if let Ok(mut jm) = jobs_arc.lock() {
                match render_res {
                    Ok(_) => jm.mark_completed(&active_jid),
                    Err(ExportError::Cancelled) => {}
                    Err(e) => jm.mark_failed(&active_jid, e.to_string()),
                }
            }
        });

        Ok(tool_ok(&ExportStartReceipt {
            job_id: jid,
            status: "running".to_string(),
            output_path: params.output_path,
        }))
    }

    #[tool(
        name = "tinixcut_export_status",
        annotations(read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false),
        description = "Query progress and execution status of an active or completed export job",
        output_schema = rmcp::handler::server::tool::schema_for_type::<ExportJobStatusReport>()
    )]
    async fn export_status(
        &self,
        Parameters(params): Parameters<ExportStatusArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let jm = match self.jobs.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let job = match jm.get_job(&params.job_id) {
            Some(j) => j,
            None => {
                return Ok(tool_error(StructuredError::new(
                    "JOB_NOT_FOUND",
                    format!("Job '{}' not found", params.job_id),
                )));
            }
        };
        let (status_str, err_str) = match &job.status {
            JobStatus::Queued => ("queued".to_string(), None),
            JobStatus::Running => ("running".to_string(), None),
            JobStatus::Completed => ("completed".to_string(), None),
            JobStatus::Cancelled => ("cancelled".to_string(), None),
            JobStatus::Failed(reason) => ("failed".to_string(), Some(reason.clone())),
        };
        Ok(tool_ok(&ExportJobStatusReport {
            job_id: job.id.clone(),
            status: status_str,
            progress: job.progress,
            error: err_str,
        }))
    }

    #[tool(
        name = "tinixcut_export_cancel",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = true, open_world_hint = false),
        description = "Cancel an in-progress export render job",
        output_schema = rmcp::handler::server::tool::schema_for_type::<ExportCancelReceipt>()
    )]
    async fn export_cancel(
        &self,
        Parameters(params): Parameters<ExportCancelArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let mut jm = match self.jobs.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let cancelled = jm.cancel_job(&params.job_id);
        Ok(tool_ok(&ExportCancelReceipt { cancelled }))
    }

    #[tool(
        name = "tinixcut_template_validate",
        annotations(read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false),
        description = "Validate a typed TemplateDraft against domain limits, security rules, and catalog resources",
        output_schema = rmcp::handler::server::tool::schema_for_type::<crate::modules::templates::agent::TemplateValidationReport>()
    )]
    async fn template_validate(
        &self,
        Parameters(params): Parameters<TemplateValidateArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let catalog = crate::modules::templates::catalog::TemplateCatalog::default_catalog();
        let report = crate::modules::templates::agent::AgentTemplateService::validate_draft(
            &params.draft,
            None,
            Some(&catalog),
        );
        Ok(tool_ok(&report))
    }

    #[tool(
        name = "tinixcut_template_preview",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = false, open_world_hint = false),
        description = "Render a preview frame of a TemplateDraft at time t using the shared compositor and return frame receipt",
        output_schema = rmcp::handler::server::tool::schema_for_type::<crate::modules::templates::agent::TemplatePreviewReceipt>()
    )]
    async fn template_preview(
        &self,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
        Parameters(params): Parameters<TemplatePreviewArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let time_sec = params.time_seconds.unwrap_or(1.0);
        let out_path = params.output_path.map(PathBuf::from).unwrap_or_else(|| {
            std::env::temp_dir().join(format!(
                "tinixcut_preview_{}.png",
                uuid::Uuid::new_v4().simple()
            ))
        });
        let timeout = params
            .timeout_ms
            .map(std::time::Duration::from_millis)
            .unwrap_or(crate::modules::templates::agent::DEFAULT_PREVIEW_TIMEOUT);

        let jid = params
            .job_id
            .unwrap_or_else(|| format!("preview_{}", uuid::Uuid::new_v4().simple()));

        // Fast path: if this exact job_id is currently being executed by a live worker, reject early
        if self.worker_capacity.is_active(&jid) {
            return Ok(tool_error(StructuredError::new(
                "DUPLICATE_ACTIVE_JOB",
                format!("Job '{}' is already running", jid),
            )));
        }

        // Reject duplicate active IDs or deduplicate
        {
            let mut jm = match self.jobs.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            if let Some(existing) = jm.get_job(&jid) {
                match existing.status {
                    crate::modules::job::JobStatus::Queued
                    | crate::modules::job::JobStatus::Running => {
                        return Ok(tool_error(StructuredError::new(
                            "DUPLICATE_ACTIVE_JOB",
                            format!(
                                "Job '{}' is already active (status: {:?})",
                                jid, existing.status
                            ),
                        )));
                    }
                    crate::modules::job::JobStatus::Cancelled => {
                        return Ok(tool_error(StructuredError::new(
                            "OPERATION_CANCELLED",
                            "Template preview render was cancelled (job already cancelled in JobManager)",
                        )));
                    }
                    _ => {}
                }
            }
            let (_submitted_id, is_deduped) = jm.submit_job(
                jid.clone(),
                crate::modules::job::JobKind::Thumbnail,
                format!("preview_{}", jid),
                1,
            );
            if is_deduped {
                return Ok(tool_error(StructuredError::new(
                    "DUPLICATE_ACTIVE_JOB",
                    format!("Job '{}' deduplicated to an already active job", jid),
                )));
            }
        }

        let start_wait = std::time::Instant::now();
        // Wait for concurrency capacity. NEVER spawn blocking render while queued or full!
        let (permit, cancel_flag) = loop {
            if context.ct.is_cancelled() || params.cancel_requested == Some(true) {
                return Ok(tool_error(StructuredError::new(
                    "OPERATION_CANCELLED",
                    "Template preview render was cancelled",
                )));
            }

            if start_wait.elapsed() >= timeout {
                if let Ok(mut jm) = self.jobs.lock() {
                    jm.mark_failed(&jid, "Capacity wait timeout".to_string());
                }
                return Ok(tool_error(StructuredError::new(
                    "OPERATION_TIMEOUT",
                    "Template preview render timed out waiting for concurrency capacity",
                )));
            }

            let jm_can_start = {
                let jm = match self.jobs.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                jm.can_start()
            };

            if jm_can_start {
                match self.worker_capacity.try_acquire(&jid) {
                    Ok(permit) => {
                        let mut jm = match self.jobs.lock() {
                            Ok(guard) => guard,
                            Err(poisoned) => poisoned.into_inner(),
                        };
                        if let Some(existing) = jm.get_job(&jid) {
                            if existing.status == crate::modules::job::JobStatus::Cancelled {
                                drop(permit);
                                return Ok(tool_error(StructuredError::new(
                                    "OPERATION_CANCELLED",
                                    "Template preview render was cancelled",
                                )));
                            }
                        }
                        match jm.start_job(&jid) {
                            Ok(flag) => {
                                break (permit, flag);
                            }
                            Err(err) => {
                                drop(permit);
                                if err.contains("Cannot start cancelled job") {
                                    return Ok(tool_error(StructuredError::new(
                                        "OPERATION_CANCELLED",
                                        "Template preview render was cancelled",
                                    )));
                                }
                                if err.contains("Job is already running") {
                                    return Ok(tool_error(StructuredError::new(
                                        "DUPLICATE_ACTIVE_JOB",
                                        format!("Job '{}' is already running", jid),
                                    )));
                                }
                                // Wait briefly for a running job slot to free up or for cancellation
                            }
                        }
                    }
                    Err(CapacityError::AlreadyActive(active_id)) => {
                        return Ok(tool_error(StructuredError::new(
                            "DUPLICATE_ACTIVE_JOB",
                            format!("Job '{}' is already running", active_id),
                        )));
                    }
                    Err(CapacityError::Full) => {}
                }
            }

            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_millis(5)) => {},
                _ = context.ct.cancelled() => {},
            }
        };

        if params.cancel_requested == Some(true) || context.ct.is_cancelled() {
            cancel_flag.store(true, std::sync::atomic::Ordering::SeqCst);
        }

        // Connect request cancellation token to worker cancel flag
        let ct_clone = context.ct.clone();
        let cancel_flag_clone = cancel_flag.clone();
        tokio::spawn(async move {
            ct_clone.cancelled().await;
            cancel_flag_clone.store(true, std::sync::atomic::Ordering::SeqCst);
        });

        // RAII guard to flip cancel_flag if the request future is cancelled or dropped by caller
        struct CancelOnDrop {
            flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
            completed: bool,
        }
        impl Drop for CancelOnDrop {
            fn drop(&mut self) {
                if !self.completed {
                    self.flag.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            }
        }

        let mut drop_guard = CancelOnDrop {
            flag: cancel_flag.clone(),
            completed: false,
        };

        let elapsed_wait = start_wait.elapsed();
        let remaining_timeout = timeout.saturating_sub(elapsed_wait);
        if remaining_timeout.is_zero() {
            drop_guard.completed = true;
            if let Ok(mut jm) = self.jobs.lock() {
                jm.mark_failed(&jid, "Timeout before render start".to_string());
            }
            drop(permit);
            return Ok(tool_error(StructuredError::new(
                "OPERATION_TIMEOUT",
                "Template preview render timed out before render start",
            )));
        }

        let draft = params.draft;
        let out_path_clone = out_path.clone();
        let cancel_clone = cancel_flag.clone();
        let width = params.width;
        let height = params.height;

        let jobs_arc_inner = std::sync::Arc::clone(&self.jobs);
        let active_jid_inner = jid.clone();
        let task = tokio::task::spawn_blocking(move || {
            // Worker-owned capacity permit holding until closure exit, independent of JobManager status.
            let _worker_permit = permit;
            let res = crate::modules::templates::agent::AgentTemplateService::preview_draft_to_temp_bounded(
                &draft,
                &out_path_clone,
                time_sec,
                width,
                height,
                Some(remaining_timeout),
                Some(&cancel_clone),
            );

            // Update JobManager status upon worker closure completion
            if let Ok(mut jm) = jobs_arc_inner.lock() {
                match &res {
                    Ok(_) => {
                        jm.mark_completed(&active_jid_inner);
                    }
                    Err(crate::modules::templates::agent::AgentTemplateError::Cancelled) => {
                        let _ = jm.cancel_job(&active_jid_inner);
                    }
                    Err(e) => {
                        jm.mark_failed(&active_jid_inner, e.to_string());
                    }
                }
            }

            res
        });

        let spawn_res = match tokio::time::timeout(remaining_timeout, task).await {
            Ok(join_res) => {
                drop_guard.completed = true;
                join_res.map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?
            }
            Err(_) => {
                drop_guard.completed = true;
                cancel_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                // DO NOT decrement capacity or mark job completed/failed in JobManager here.
                // The worker in spawn_blocking is still running and owns its capacity permit.
                // The worker will update JobManager and release capacity when its closure exits.
                return Ok(tool_error(StructuredError::new(
                    "OPERATION_TIMEOUT",
                    "Template preview render timed out before completion",
                )));
            }
        };

        let (mut temp_artifact, receipt) = match spawn_res {
            Ok((artifact, r)) => (artifact, r),
            Err(crate::modules::templates::agent::AgentTemplateError::Cancelled) => {
                return Ok(tool_error(StructuredError::new(
                    "OPERATION_CANCELLED",
                    "Template preview render was cancelled",
                )));
            }
            Err(crate::modules::templates::agent::AgentTemplateError::Timeout(_)) => {
                return Ok(tool_error(StructuredError::new(
                    "OPERATION_TIMEOUT",
                    "Template preview render timed out before completion",
                )));
            }
            Err(e) => {
                return Ok(tool_error(StructuredError::new(
                    "PREVIEW_RENDER_FAILED",
                    e.to_string(),
                )));
            }
        };

        // Final deadline and cancellation check in MCP immediately before publication/rename
        if context.ct.is_cancelled()
            || cancel_flag.load(std::sync::atomic::Ordering::SeqCst)
            || params.cancel_requested == Some(true)
        {
            return Ok(tool_error(StructuredError::new(
                "OPERATION_CANCELLED",
                "Template preview render was cancelled",
            )));
        }
        if start_wait.elapsed() >= timeout {
            return Ok(tool_error(StructuredError::new(
                "OPERATION_TIMEOUT",
                "Template preview render timed out before publication",
            )));
        }

        // Coordinate publication with response completion:
        // Response completion is already secured (outer timeout did not fire, render and pixel calculations succeeded).
        // Atomically publish temp_artifact to out_path.
        if let Err(e) = std::fs::rename(temp_artifact.path(), &out_path) {
            return Ok(tool_error(StructuredError::new(
                "PREVIEW_PUBLISH_FAILED",
                format!("Failed to atomically publish preview: {}", e),
            )));
        }
        temp_artifact.disarm();
        Ok(tool_ok(&receipt))
    }

    #[tool(
        name = "tinixcut_template_apply",
        annotations(read_only_hint = false, destructive_hint = true, idempotent_hint = false, open_world_hint = false),
        description = "Apply a validated TemplateDraft to a project text item as an atomic CommandService transaction and persist",
        output_schema = rmcp::handler::server::tool::schema_for_type::<crate::modules::templates::agent::TemplateApplyReceipt>()
    )]
    async fn template_apply(
        &self,
        Parameters(params): Parameters<TemplateApplyArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let _guard = self.write_gate.lock().await;

        if params
            .client_id
            .as_ref()
            .is_none_or(|id| id.trim().is_empty() || id.len() > 64)
        {
            return Ok(tool_error(StructuredError::new(
                "OPERATION_VALIDATION_FAILED",
                "Missing or invalid client_id: caller must supply a non-empty client_id (<= 64 chars)",
            )));
        }

        if params.client_epoch.is_none() {
            return Ok(tool_error(StructuredError::new(
                "OPERATION_VALIDATION_FAILED",
                "Missing client_epoch: caller must supply client_epoch for durable idempotency",
            )));
        }

        if params.sequence_number.is_none() || params.sequence_number == Some(0) {
            return Ok(tool_error(StructuredError::new(
                "OPERATION_VALIDATION_FAILED",
                "Missing or invalid sequence_number: caller must supply positive sequence_number >= 1",
            )));
        }

        if params
            .transaction_id
            .as_ref()
            .is_none_or(|tid| tid.trim().is_empty())
        {
            return Ok(tool_error(StructuredError::new(
                "OPERATION_VALIDATION_FAILED",
                "Missing transaction_id: caller must supply transaction_id for atomic envelope",
            )));
        }

        let project_path = PathBuf::from(&params.project_path);
        let catalog = crate::modules::templates::catalog::TemplateCatalog::default_catalog();

        match crate::modules::templates::agent::AgentTemplateService::apply_draft_to_project(
            &project_path,
            &params.item_id,
            &params.draft,
            params.client_id,
            params.client_epoch,
            params.sequence_number,
            params.transaction_id,
            params.base_revision,
            &catalog,
        ) {
            Ok(receipt) => Ok(tool_ok(&receipt)),
            Err(crate::modules::templates::agent::AgentTemplateError::Command(cmd_err)) => {
                Ok(tool_error(map_command_error(cmd_err)))
            }
            Err(crate::modules::templates::agent::AgentTemplateError::ProjectRevisionConflict {
                expected,
                actual,
            }) => Ok(tool_error(StructuredError::with_details(
                "PROJECT_REVISION_CONFLICT",
                format!(
                    "Cannot apply template: expected disk revision {expected:?}, found {actual:?}; reload the project or save as a new file to preserve your edits"
                ),
                serde_json::json!({
                    "expectedRevision": expected,
                    "actualRevision": actual,
                }),
            ))),
            Err(e) => Ok(tool_error(StructuredError::new(
                "TEMPLATE_APPLY_FAILED",
                e.to_string(),
            ))),
        }
    }

    #[tool(
        name = "tinixcut_template_save",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = false, open_world_hint = false),
        description = "Save a validated TemplateDraft into the template catalog as a reusable TemplateDefinition",
        output_schema = rmcp::handler::server::tool::schema_for_type::<crate::modules::templates::agent::TemplateSaveReceipt>()
    )]
    async fn template_save(
        &self,
        Parameters(params): Parameters<TemplateSaveArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let catalog = params
            .catalog_dir
            .map(PathBuf::from)
            .map(crate::modules::templates::catalog::TemplateCatalog::new)
            .unwrap_or_else(crate::modules::templates::catalog::TemplateCatalog::default_catalog);
        let overwrite = params.overwrite.unwrap_or(false);

        match crate::modules::templates::agent::AgentTemplateService::save_draft_to_catalog(
            &params.draft,
            &catalog,
            overwrite,
        ) {
            Ok(receipt) => Ok(tool_ok(&receipt)),
            Err(crate::modules::templates::agent::AgentTemplateError::Catalog(
                crate::modules::templates::catalog::TemplateCatalogError::ImmutableVersionConflict {
                    id,
                    version,
                },
            )) => Ok(tool_error(StructuredError::with_details(
                "IMMUTABLE_VERSION_CONFLICT",
                format!("Template '{}' version {} already exists", id, version),
                serde_json::json!({ "templateId": id, "version": version }),
            ))),
            Err(e) => Ok(tool_error(StructuredError::new(
                "TEMPLATE_SAVE_FAILED",
                e.to_string(),
            ))),
        }
    }
}

#[tool_handler(
    name = "tinixcut",
    version = "0.1.0",
    instructions = "TinixCut Automation-First Desktop Video Editor MCP Server"
)]
impl rmcp::ServerHandler for TinixCutMcpServer {}

/// Launch the MCP server over standard input and output streams.
pub async fn run_mcp_server() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let server = TinixCutMcpServer::new();
    let server_cleanup = server.clone();

    // Standard I/O transport
    let transport = (tokio::io::stdin(), tokio::io::stdout());
    let service = rmcp::serve_server(server, transport).await;

    match service {
        Ok(running) => {
            let _ = running.waiting().await;
        }
        Err(e) => {
            eprintln!("TinixCut MCP Server initialization error: {e}");
        }
    }

    // Ensure all active render jobs and processes are cancelled on EOF
    server_cleanup.shutdown_and_cleanup();
    Ok(())
}
