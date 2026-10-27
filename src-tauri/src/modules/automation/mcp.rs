//! Model Context Protocol (MCP) Automation Adapter for TinixCut
//!
//! Architecture Invariant: MCP is strictly a thin external transport adapter.
//! It contains zero second reducers, zero second command registries, zero alternate persistence formats,
//! and zero independent idempotency ledgers. All mutations route through the canonical CommandService.
//!
//! Governed by ADR-005 and ADR-001 v3.

use crate::modules::automation::{
    AutomationAdapter, CapabilitiesReport, ExportJobStatusReport, ExportStartReceipt,
    ProjectInspectReport,
};
use crate::modules::command::{
    CommandEnvelope, CommandError, CommandReceipt, CommandService, DiffReport,
};
use crate::modules::export::{ExportEngine, ExportError, ExportProfile};
use crate::modules::job::{JobKind, JobManager, JobStatus};
use crate::modules::persistence::{load_project, save_project_atomic};
use rmcp::{
    handler::server::wrapper::Parameters,
    model::CallToolResult,
    schemars::{self, JsonSchema, Schema, SchemaGenerator},
    serde::{Deserialize, Serialize},
    tool, tool_handler, tool_router,
};
use std::borrow::Cow;
use std::path::PathBuf;
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct CapabilitiesArgs {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DryRunArgs {
    pub project_path: String,
    pub envelope: CommandEnvelope,
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
        }
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
        name = "tinixcut_inspect",
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
            Ok(diff) => Ok(tool_ok(&diff)),
            Err(err) => Ok(tool_error(map_command_error(err))),
        }
    }

    #[tool(
        name = "tinixcut_apply",
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
        if let Err(e) = save_project_atomic(service.project(), &project_path) {
            return Ok(tool_error(StructuredError::new(
                "PROJECT_SAVE_FAILED",
                format!("Failed to atomically save project to disk: {}", e),
            )));
        }

        Ok(tool_ok(&receipt))
    }

    #[tool(
        name = "tinixcut_save",
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
            .unwrap_or(source_path);

        if let Err(e) = save_project_atomic(&project, &dest_path) {
            return Ok(tool_error(StructuredError::new(
                "PROJECT_SAVE_FAILED",
                format!(
                    "Failed to atomically save project to disk at {:?}: {}",
                    dest_path, e
                ),
            )));
        }

        Ok(tool_ok(&SaveReceipt {
            saved_path: dest_path.to_string_lossy().to_string(),
        }))
    }

    #[tool(
        name = "tinixcut_export_start",
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

        let target_path = PathBuf::from(&params.output_path);
        let prof = params.profile.unwrap_or_default();
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
            Err(e) => Ok(tool_error(StructuredError::new(
                "TEMPLATE_APPLY_FAILED",
                e.to_string(),
            ))),
        }
    }

    #[tool(
        name = "tinixcut_template_save",
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
