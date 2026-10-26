//! Headless Automation Surface for TinixCut
//! Architecture Invariant: Automation is a client of the editor, NOT UI mouse/keyboard simulation
//! and NOT a second project reducer. It routes exclusively through the Command Service.

pub mod mcp;

use crate::modules::command::{
    CommandEnvelope, CommandError, CommandReceipt, CommandService, DiffReport,
};
use crate::modules::domain::time::{FrameRate, RationalTime};
use crate::modules::domain::{MediaKind, Project, TrackKind};
use crate::modules::export::{ExportEngine, ExportError, ExportProfile, ExportReport};
use crate::modules::job::{JobKind, JobManager, JobStatus};
use crate::modules::persistence::{load_project, save_project_atomic};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutomationError {
    #[error("No project is currently open in the session")]
    ProjectNotOpen,

    #[error("Failed to load project: {0}")]
    ProjectLoad(String),

    #[error("Command execution error: {0}")]
    Command(#[from] CommandError),

    #[error("Export render error: {0}")]
    Export(String),

    #[error("Job not found: {0}")]
    JobNotFound(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("CLI argument error: {0}")]
    Cli(String),
}

impl AutomationError {
    pub fn error_code(&self) -> &'static str {
        match self {
            AutomationError::ProjectNotOpen => "PROJECT_NOT_OPEN",
            AutomationError::ProjectLoad(_) => "PROJECT_LOAD_FAILED",
            AutomationError::Command(CommandError::StaleRevision { .. }) => "STALE_REVISION",
            AutomationError::Command(CommandError::IdempotencyCollision { .. }) => {
                "IDEMPOTENCY_COLLISION"
            }
            AutomationError::Command(CommandError::SchemaVersionMismatch { .. }) => {
                "SCHEMA_VERSION_MISMATCH"
            }
            AutomationError::Command(CommandError::ProjectIdMismatch { .. }) => {
                "PROJECT_ID_MISMATCH"
            }
            AutomationError::Command(CommandError::UnsupportedTransition { .. }) => {
                "UNSUPPORTED_TRANSITION"
            }
            AutomationError::Command(CommandError::OperationValidationFailed { .. }) => {
                "OPERATION_VALIDATION_FAILED"
            }
            AutomationError::Command(CommandError::OperationAlreadyCommitted { .. }) => {
                "OPERATION_ALREADY_COMMITTED"
            }
            AutomationError::Command(CommandError::ClientRegistryFull { .. }) => {
                "CLIENT_REGISTRY_FULL"
            }
            AutomationError::Command(CommandError::ClientEpochExpired { .. }) => {
                "CLIENT_EPOCH_EXPIRED"
            }
            AutomationError::Command(CommandError::SequenceGapDetected { .. }) => {
                "SEQUENCE_GAP_DETECTED"
            }
            AutomationError::Command(CommandError::LegacyReceiptUnverifiable { .. }) => {
                "LEGACY_RECEIPT_UNVERIFIABLE"
            }
            AutomationError::Command(CommandError::InsufficientTransitionHandle { .. }) => {
                "INSUFFICIENT_TRANSITION_HANDLE"
            }
            AutomationError::Command(CommandError::TransitionHandleUnverifiable { .. }) => {
                "TRANSITION_HANDLE_UNVERIFIABLE"
            }
            AutomationError::Command(CommandError::UnsupportedTransitionMedia { .. }) => {
                "UNSUPPORTED_TRANSITION_MEDIA"
            }
            AutomationError::Command(_) => "COMMAND_FAILED",
            AutomationError::Export(_) => "EXPORT_FAILED",
            AutomationError::JobNotFound(_) => "JOB_NOT_FOUND",
            AutomationError::Serialization(_) => "SERIALIZATION_ERROR",
            AutomationError::Cli(_) => "CLI_ERROR",
        }
    }
}

impl From<ExportError> for AutomationError {
    fn from(err: ExportError) -> Self {
        AutomationError::Export(err.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TrackSummary {
    pub track_id: String,
    pub name: String,
    pub kind: TrackKind,
    pub muted: bool,
    pub items_count: usize,
    #[serde(default)]
    pub transitions_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AssetSummary {
    pub asset_id: String,
    pub name: String,
    pub path: String,
    pub media_kind: MediaKind,
    pub duration: Option<RationalTime>,
}

/// Structured inspection report returned by the Inspect API
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInspectReport {
    pub project_id: String,
    pub name: String,
    pub revision: u64,
    pub duration_frames: i64,
    pub duration_seconds: f64,
    pub frame_rate: FrameRate,
    pub resolution: (u32, u32),
    pub tracks_count: usize,
    pub clips_count: usize,
    pub assets_count: usize,
    #[serde(default)]
    pub transitions_count: usize,
    pub tracks: Vec<TrackSummary>,
    pub assets: Vec<AssetSummary>,
    pub supported_operations: Vec<String>,
    pub supported_transitions: Vec<String>,
    #[serde(default = "default_schema_version_3")]
    pub schema_version: u32,
    #[serde(default)]
    pub supported_keyframe_properties: Vec<String>,
    #[serde(default)]
    pub supported_keyframe_easings: Vec<String>,
    #[serde(default = "default_custom_cubic_bounds")]
    pub custom_cubic_bounds: (f32, f32),
    #[serde(default)]
    pub supported_animation_units: Vec<String>,
    #[serde(default)]
    pub supported_stagger_orders: Vec<String>,
    #[serde(default)]
    pub supported_loop_modes: Vec<String>,
    #[serde(default)]
    pub supported_duration_conflict_policies: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_capabilities: Option<crate::modules::templates::agent::TemplateCapabilitiesReport>,
}

fn default_schema_version_3() -> u32 {
    crate::modules::domain::CURRENT_SCHEMA_VERSION
}

fn default_custom_cubic_bounds() -> (f32, f32) {
    (0.0, 1.0)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CapabilitiesReport {
    pub supported_operations: Vec<String>,
    pub supported_transitions: Vec<String>,
    pub supported_export_codecs: Vec<String>,
    pub protocol_version: u32,
    #[serde(default = "default_schema_version_3")]
    pub schema_version: u32,
    #[serde(default)]
    pub supported_keyframe_properties: Vec<String>,
    #[serde(default)]
    pub supported_keyframe_easings: Vec<String>,
    #[serde(default = "default_custom_cubic_bounds")]
    pub custom_cubic_bounds: (f32, f32),
    #[serde(default)]
    pub supported_animation_units: Vec<String>,
    #[serde(default)]
    pub supported_stagger_orders: Vec<String>,
    #[serde(default)]
    pub supported_loop_modes: Vec<String>,
    #[serde(default)]
    pub supported_duration_conflict_policies: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_capabilities: Option<crate::modules::templates::agent::TemplateCapabilitiesReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExportStartReceipt {
    pub job_id: String,
    pub status: String,
    pub output_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExportJobStatusReport {
    pub job_id: String,
    pub status: String, // "queued", "running", "completed", "failed", "cancelled"
    pub progress: f32,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Protocol Framing DTOs (JSONL)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolRequest {
    pub protocol_version: u32,
    pub request_id: String,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolSuccessResponse<T> {
    pub protocol_version: u32,
    pub request_id: String,
    pub ok: bool,
    pub result: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolErrorDetail {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolErrorResponse {
    pub protocol_version: u32,
    pub request_id: String,
    pub ok: bool,
    pub error: ProtocolErrorDetail,
}

// ---------------------------------------------------------------------------
// Headless Application Session
// ---------------------------------------------------------------------------

pub struct AutomationSession {
    pub service: Option<CommandService>,
    pub jobs: Arc<Mutex<JobManager>>,
    pub current_project_path: Option<PathBuf>,
}

impl Default for AutomationSession {
    fn default() -> Self {
        Self::new()
    }
}

impl AutomationSession {
    pub fn new() -> Self {
        Self {
            service: None,
            jobs: Arc::new(Mutex::new(JobManager::new(4))),
            current_project_path: None,
        }
    }

    pub fn open_project(&mut self, path: &Path) -> Result<ProjectInspectReport, AutomationError> {
        let project =
            load_project(path).map_err(|e| AutomationError::ProjectLoad(e.to_string()))?;
        let report = AutomationAdapter::inspect_project(&project);
        self.service = Some(CommandService::new(project));
        self.current_project_path = Some(path.to_path_buf());
        Ok(report)
    }

    pub fn close_project(&mut self) -> Result<(), AutomationError> {
        self.service = None;
        self.current_project_path = None;
        Ok(())
    }

    pub fn save_project(&mut self, target_path: Option<&Path>) -> Result<String, AutomationError> {
        let service = self
            .service
            .as_ref()
            .ok_or(AutomationError::ProjectNotOpen)?;
        let path = match target_path {
            Some(p) => p.to_path_buf(),
            None => self.current_project_path.clone().ok_or_else(|| {
                AutomationError::Cli("No path specified for project save".to_string())
            })?,
        };
        save_project_atomic(service.project(), &path)
            .map_err(|e| AutomationError::ProjectLoad(e.to_string()))?;
        self.current_project_path = Some(path.clone());
        Ok(path.to_string_lossy().to_string())
    }

    pub fn inspect(&self) -> Result<ProjectInspectReport, AutomationError> {
        let service = self
            .service
            .as_ref()
            .ok_or(AutomationError::ProjectNotOpen)?;
        Ok(AutomationAdapter::inspect_project(service.project()))
    }

    pub fn capabilities(&self) -> Result<CapabilitiesReport, AutomationError> {
        Ok(CapabilitiesReport {
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
        })
    }

    pub fn dry_run(&self, envelope: &CommandEnvelope) -> Result<DiffReport, AutomationError> {
        let service = self
            .service
            .as_ref()
            .ok_or(AutomationError::ProjectNotOpen)?;
        let diff = service.dry_run(envelope)?;
        Ok(diff)
    }

    pub fn apply(
        &mut self,
        mut envelope: CommandEnvelope,
        save_after: bool,
    ) -> Result<CommandReceipt, AutomationError> {
        let service = self
            .service
            .as_mut()
            .ok_or(AutomationError::ProjectNotOpen)?;

        // Stdio session automatically injects client_id: "session" (ADR-001 v3 Section 6.2)
        if envelope.client_id.is_none() {
            let cur_epoch = service.project().client_registry.current_epoch;
            let next_seq = service
                .project()
                .client_registry
                .clients
                .get("session")
                .map(|c| c.highest_sequence + 1)
                .unwrap_or(1);
            envelope.client_id = Some("session".to_string());
            envelope.client_epoch = Some(cur_epoch);
            envelope.sequence_number = Some(next_seq);
        }

        let receipt = service.execute(envelope)?;
        if save_after {
            if let Some(ref path) = self.current_project_path {
                save_project_atomic(service.project(), path)
                    .map_err(|e| AutomationError::ProjectLoad(e.to_string()))?;
            }
        }
        Ok(receipt)
    }

    pub fn export_start(
        &mut self,
        output_path: &Path,
        profile: Option<ExportProfile>,
    ) -> Result<ExportStartReceipt, AutomationError> {
        let service = self
            .service
            .as_ref()
            .ok_or(AutomationError::ProjectNotOpen)?;
        let snapshot = service.project().clone(); // Snapshot isolation

        let job_id = format!("job_export_{}", uuid::Uuid::new_v4().simple());
        let dedupe_key = format!("export:{}:{}", snapshot.project_id, snapshot.revision);

        let (jid, _) = {
            let mut jm = self.jobs.lock().unwrap();
            jm.submit_job(job_id.clone(), JobKind::Export, dedupe_key, 100)
        };

        let cancel_flag = {
            let mut jm = self.jobs.lock().unwrap();
            jm.start_job(&jid).map_err(AutomationError::Export)?
        };

        let target_path = output_path.to_path_buf();
        let prof = profile.unwrap_or_default();
        let jobs_arc = Arc::clone(&self.jobs);
        let active_jid = jid.clone();

        std::thread::spawn(move || {
            let render_res =
                ExportEngine::render_snapshot(&snapshot, &target_path, &prof, Some(cancel_flag));

            if let Ok(mut jm) = jobs_arc.lock() {
                match render_res {
                    Ok(_) => jm.mark_completed(&active_jid),
                    Err(ExportError::Cancelled) => {
                        // Already handled by cancellation
                    }
                    Err(e) => jm.mark_failed(&active_jid, e.to_string()),
                }
            }
        });

        Ok(ExportStartReceipt {
            job_id: jid,
            status: "running".to_string(),
            output_path: output_path.to_string_lossy().to_string(),
        })
    }

    pub fn export_status(&self, job_id: &str) -> Result<ExportJobStatusReport, AutomationError> {
        let jm = self.jobs.lock().unwrap();
        let job = jm
            .get_job(job_id)
            .ok_or_else(|| AutomationError::JobNotFound(job_id.to_string()))?;
        let (status_str, err_str) = match &job.status {
            JobStatus::Queued => ("queued".to_string(), None),
            JobStatus::Running => ("running".to_string(), None),
            JobStatus::Completed => ("completed".to_string(), None),
            JobStatus::Cancelled => ("cancelled".to_string(), None),
            JobStatus::Failed(reason) => ("failed".to_string(), Some(reason.clone())),
        };
        Ok(ExportJobStatusReport {
            job_id: job.id.clone(),
            status: status_str,
            progress: job.progress,
            error: err_str,
        })
    }

    pub fn export_cancel(&mut self, job_id: &str) -> Result<bool, AutomationError> {
        let mut jm = self.jobs.lock().unwrap();
        Ok(jm.cancel_job(job_id))
    }
}

// ---------------------------------------------------------------------------
// Method Dispatcher & Stdio Loop
// ---------------------------------------------------------------------------

pub fn dispatch_method(
    session: &mut AutomationSession,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, AutomationError> {
    match method {
        "session.open" => {
            let path_str = params.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
                AutomationError::Cli("Missing 'path' parameter in session.open".to_string())
            })?;
            let report = session.open_project(Path::new(path_str))?;
            serde_json::to_value(report).map_err(|e| AutomationError::Serialization(e.to_string()))
        }
        "session.close" => {
            session.close_project()?;
            Ok(serde_json::json!({ "closed": true }))
        }
        "project.inspect" => {
            let report = session.inspect()?;
            serde_json::to_value(report).map_err(|e| AutomationError::Serialization(e.to_string()))
        }
        "project.capabilities" => {
            let caps = session.capabilities()?;
            serde_json::to_value(caps).map_err(|e| AutomationError::Serialization(e.to_string()))
        }
        "project.save" => {
            let path_opt = params.get("path").and_then(|v| v.as_str()).map(Path::new);
            let saved = session.save_project(path_opt)?;
            Ok(serde_json::json!({ "savedPath": saved }))
        }
        "command.dryRun" => {
            let envelope: CommandEnvelope = if params.get("envelope").is_some() {
                serde_json::from_value(params.get("envelope").unwrap().clone())
            } else {
                serde_json::from_value(params)
            }
            .map_err(|e| AutomationError::Serialization(format!("Invalid CommandEnvelope: {e}")))?;

            let diff = session.dry_run(&envelope)?;
            serde_json::to_value(diff).map_err(|e| AutomationError::Serialization(e.to_string()))
        }
        "command.apply" => {
            let (envelope, save_after): (CommandEnvelope, bool) =
                if params.get("envelope").is_some() {
                    let env = serde_json::from_value(params.get("envelope").unwrap().clone())
                        .map_err(|e| {
                            AutomationError::Serialization(format!("Invalid CommandEnvelope: {e}"))
                        })?;
                    let save = params
                        .get("saveAfter")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    (env, save)
                } else {
                    let env = serde_json::from_value(params).map_err(|e| {
                        AutomationError::Serialization(format!("Invalid CommandEnvelope: {e}"))
                    })?;
                    (env, false)
                };

            let receipt = session.apply(envelope, save_after)?;
            serde_json::to_value(receipt).map_err(|e| AutomationError::Serialization(e.to_string()))
        }
        "export.start" => {
            let output_path = params
                .get("outputPath")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    AutomationError::Cli(
                        "Missing 'outputPath' parameter in export.start".to_string(),
                    )
                })?;
            let profile: Option<ExportProfile> = params
                .get("profile")
                .and_then(|p| serde_json::from_value(p.clone()).ok());
            let receipt = session.export_start(Path::new(output_path), profile)?;
            serde_json::to_value(receipt).map_err(|e| AutomationError::Serialization(e.to_string()))
        }
        "export.status" => {
            let job_id = params
                .get("jobId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    AutomationError::Cli("Missing 'jobId' parameter in export.status".to_string())
                })?;
            let status = session.export_status(job_id)?;
            serde_json::to_value(status).map_err(|e| AutomationError::Serialization(e.to_string()))
        }
        "export.cancel" => {
            let job_id = params
                .get("jobId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    AutomationError::Cli("Missing 'jobId' parameter in export.cancel".to_string())
                })?;
            let cancelled = session.export_cancel(job_id)?;
            Ok(serde_json::json!({ "cancelled": cancelled }))
        }
        "template.capabilities" => {
            let caps = crate::modules::templates::agent::AgentTemplateService::capabilities();
            serde_json::to_value(caps).map_err(|e| AutomationError::Serialization(e.to_string()))
        }
        "template.schema" => {
            Ok(crate::modules::templates::agent::AgentTemplateService::draft_json_schema())
        }
        "template.validate" => {
            let draft: crate::modules::templates::agent::TemplateDraft = if params
                .get("draft")
                .is_some()
            {
                serde_json::from_value(params.get("draft").unwrap().clone())
            } else {
                serde_json::from_value(params)
            }
            .map_err(|e| AutomationError::Serialization(format!("Invalid TemplateDraft: {e}")))?;
            let catalog = crate::modules::templates::catalog::TemplateCatalog::default_catalog();
            let report = crate::modules::templates::agent::AgentTemplateService::validate_draft(
                &draft,
                None,
                Some(&catalog),
            );
            serde_json::to_value(report).map_err(|e| AutomationError::Serialization(e.to_string()))
        }
        "template.preview" => {
            let draft: crate::modules::templates::agent::TemplateDraft = if params
                .get("draft")
                .is_some()
            {
                serde_json::from_value(params.get("draft").unwrap().clone())
            } else {
                serde_json::from_value(params.clone())
            }
            .map_err(|e| AutomationError::Serialization(format!("Invalid TemplateDraft: {e}")))?;
            let time_sec = params
                .get("timeSeconds")
                .and_then(|v| v.as_f64())
                .unwrap_or(1.0);
            let width = params
                .get("width")
                .and_then(|v| v.as_u64())
                .map(|w| w as u32);
            let height = params
                .get("height")
                .and_then(|v| v.as_u64())
                .map(|h| h as u32);
            let output_path = params
                .get("outputPath")
                .and_then(|v| v.as_str())
                .map(Path::new);

            if let Some(out_p) = output_path {
                let receipt =
                    crate::modules::templates::agent::AgentTemplateService::preview_draft_to_png(
                        &draft, out_p, time_sec, width, height,
                    )
                    .map_err(|e| AutomationError::Cli(format!("Preview failed: {e}")))?;
                serde_json::to_value(receipt)
                    .map_err(|e| AutomationError::Serialization(e.to_string()))
            } else {
                let buf = crate::modules::templates::agent::AgentTemplateService::preview_draft(
                    &draft, time_sec, width, height,
                )
                .map_err(|e| AutomationError::Cli(format!("Preview failed: {e}")))?;
                Ok(serde_json::json!({
                    "width": buf.width,
                    "height": buf.height,
                    "timeSeconds": time_sec,
                    "hasVisibleContent": true
                }))
            }
        }
        "template.apply" => {
            let project_path_str = params
                .get("projectPath")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AutomationError::Cli("Missing 'projectPath'".to_string()))?;
            let item_id = params
                .get("itemId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AutomationError::Cli("Missing 'itemId'".to_string()))?;
            let draft: crate::modules::templates::agent::TemplateDraft = serde_json::from_value(
                params
                    .get("draft")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
            )
            .map_err(|e| AutomationError::Serialization(format!("Invalid TemplateDraft: {e}")))?;

            let client_id = params
                .get("clientId")
                .and_then(|v| v.as_str())
                .map(ToString::to_string);
            let client_epoch = params.get("clientEpoch").and_then(|v| v.as_u64());
            let sequence_number = params.get("sequenceNumber").and_then(|v| v.as_u64());
            let transaction_id = params
                .get("transactionId")
                .and_then(|v| v.as_str())
                .map(ToString::to_string);
            let base_revision = params.get("baseRevision").and_then(|v| v.as_u64());

            let catalog = crate::modules::templates::catalog::TemplateCatalog::default_catalog();
            let receipt =
                crate::modules::templates::agent::AgentTemplateService::apply_draft_to_project(
                    Path::new(project_path_str),
                    item_id,
                    &draft,
                    client_id,
                    client_epoch,
                    sequence_number,
                    transaction_id,
                    base_revision,
                    &catalog,
                )
                .map_err(|e| match e {
                    crate::modules::templates::agent::AgentTemplateError::Command(cmd_err) => {
                        AutomationError::Command(cmd_err)
                    }
                    other => AutomationError::Cli(format!("Template apply failed: {other}")),
                })?;
            serde_json::to_value(receipt).map_err(|e| AutomationError::Serialization(e.to_string()))
        }
        "template.save" => {
            let draft: crate::modules::templates::agent::TemplateDraft = serde_json::from_value(
                params
                    .get("draft")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
            )
            .map_err(|e| AutomationError::Serialization(format!("Invalid TemplateDraft: {e}")))?;
            let catalog_dir = params
                .get("catalogDir")
                .and_then(|v| v.as_str())
                .map(PathBuf::from);
            let overwrite = params
                .get("overwrite")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let catalog = catalog_dir
                .map(crate::modules::templates::catalog::TemplateCatalog::new)
                .unwrap_or_else(
                    crate::modules::templates::catalog::TemplateCatalog::default_catalog,
                );
            let receipt =
                crate::modules::templates::agent::AgentTemplateService::save_draft_to_catalog(
                    &draft, &catalog, overwrite,
                )
                .map_err(|e| AutomationError::Cli(format!("Template save failed: {e}")))?;
            serde_json::to_value(receipt).map_err(|e| AutomationError::Serialization(e.to_string()))
        }
        other => Err(AutomationError::Cli(format!("Unknown method '{other}'"))),
    }
}

pub fn run_stdio_session<R: BufRead, W: Write, E: Write>(
    reader: R,
    mut writer: W,
    mut err_writer: E,
) {
    let mut session = AutomationSession::new();

    for line_result in reader.lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(e) => {
                let _ = writeln!(err_writer, "[tinixcut:session] Read error: {e}");
                break;
            }
        };

        let trimmed = line.trim().trim_start_matches('\u{feff}');
        if trimmed.is_empty() {
            continue;
        }

        let req: ProtocolRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                let err_resp = ProtocolErrorResponse {
                    protocol_version: 1,
                    request_id: "unknown".to_string(),
                    ok: false,
                    error: ProtocolErrorDetail {
                        code: "INVALID_JSON".to_string(),
                        message: format!("Malformed JSON request line: {e}"),
                        details: None,
                    },
                };
                if let Ok(json) = serde_json::to_string(&err_resp) {
                    let _ = writeln!(writer, "{json}");
                    let _ = writer.flush();
                }
                continue;
            }
        };

        if req.protocol_version != 1 {
            let err_resp = ProtocolErrorResponse {
                protocol_version: 1,
                request_id: req.request_id,
                ok: false,
                error: ProtocolErrorDetail {
                    code: "UNSUPPORTED_PROTOCOL_VERSION".to_string(),
                    message: format!(
                        "Protocol version {} is not supported (expected 1)",
                        req.protocol_version
                    ),
                    details: None,
                },
            };
            if let Ok(json) = serde_json::to_string(&err_resp) {
                let _ = writeln!(writer, "{json}");
                let _ = writer.flush();
            }
            continue;
        }

        let req_id = req.request_id.clone();
        let method = req.method.as_str();

        if method == "session.shutdown" {
            let success_resp = ProtocolSuccessResponse {
                protocol_version: 1,
                request_id: req_id,
                ok: true,
                result: serde_json::json!({ "shutdown": true }),
            };
            if let Ok(json) = serde_json::to_string(&success_resp) {
                let _ = writeln!(writer, "{json}");
                let _ = writer.flush();
            }
            break;
        }

        let dispatch_result = dispatch_method(&mut session, method, req.params);
        match dispatch_result {
            Ok(result_value) => {
                let success_resp = ProtocolSuccessResponse {
                    protocol_version: 1,
                    request_id: req_id,
                    ok: true,
                    result: result_value,
                };
                if let Ok(json) = serde_json::to_string(&success_resp) {
                    let _ = writeln!(writer, "{json}");
                    let _ = writer.flush();
                }
            }
            Err(err) => {
                let err_resp = ProtocolErrorResponse {
                    protocol_version: 1,
                    request_id: req_id,
                    ok: false,
                    error: ProtocolErrorDetail {
                        code: err.error_code().to_string(),
                        message: err.to_string(),
                        details: None,
                    },
                };
                if let Ok(json) = serde_json::to_string(&err_resp) {
                    let _ = writeln!(writer, "{json}");
                    let _ = writer.flush();
                }
            }
        }
    }
}

pub struct AutomationAdapter;

impl AutomationAdapter {
    pub fn supported_operations() -> Vec<String> {
        vec![
            "TrackCreate".to_string(),
            "TrackDelete".to_string(),
            "TrackReorder".to_string(),
            "TrackMuteSet".to_string(),
            "ClipCreate".to_string(),
            "ClipMove".to_string(),
            "ClipTrim".to_string(),
            "ClipSplit".to_string(),
            "ClipDelete".to_string(),
            "ClipDuplicate".to_string(),
            "ClipTransformSet".to_string(),
            "ClipCropSet".to_string(),
            "ClipPlacementModeSet".to_string(),
            "ClipOpacitySet".to_string(),
            "ClipVolumeSet".to_string(),
            "ClipTextSet".to_string(),
            "ClipTextStyleSet".to_string(),
            "TextTemplateApply".to_string(),
            "ClipTextTemplateReset".to_string(),
            "ClipAudioFadeSet".to_string(),
            "ClipTransitionSet".to_string(),
            "ClipKeyframeAdd".to_string(),
            "ClipKeyframeRemove".to_string(),
            "ClipKeyframeMove".to_string(),
            "ClipKeyframeEasingSet".to_string(),
            "AssetImport".to_string(),
            "ProjectSettingsSet".to_string(),
            "CanvasBackgroundSet".to_string(),
            "CutTransitionCreate".to_string(),
            "CutTransitionUpdateDuration".to_string(),
            "CutTransitionRemove".to_string(),
            "AudioCrossfadeCreate".to_string(),
            "AudioCrossfadeUpdateDuration".to_string(),
            "AudioCrossfadeRemove".to_string(),
            "ClipGroupMove".to_string(),
            "ClipRippleDelete".to_string(),
            "ClipTextRichSet".to_string(),
            "ClipTextSpansSet".to_string(),
            "ClipTextLayoutSet".to_string(),
            "ClipTextAppearanceSet".to_string(),
            "ClipTextFontSet".to_string(),
            "ClipTextAnimationSet".to_string(),
            "ClipTextRichReset".to_string(),
            "TemplateApply".to_string(),
            "TemplateTrackApply".to_string(),
            "TemplateLayerAdd".to_string(),
            "TemplateLayerRemove".to_string(),
            "TemplateLayerReorder".to_string(),
            "TemplateLayerUpdate".to_string(),
            "TemplateParameterSet".to_string(),
            "TemplateParametersBatchSet".to_string(),
            "TemplateReset".to_string(),
            "TemplateDetach".to_string(),
        ]
    }

    pub fn supported_transitions() -> Vec<String> {
        vec![
            "FadeIn".to_string(),
            "FadeOut".to_string(),
            "FadeBlack".to_string(),
            "CrossDissolve".to_string(),
        ]
    }

    pub fn supported_keyframe_properties() -> Vec<String> {
        vec![
            "Opacity".to_string(),
            "PositionX".to_string(),
            "PositionY".to_string(),
            "ScaleX".to_string(),
            "ScaleY".to_string(),
            "Rotation".to_string(),
            "Volume".to_string(),
        ]
    }

    pub fn supported_keyframe_easings() -> Vec<String> {
        vec![
            "Linear".to_string(),
            "EaseIn".to_string(),
            "EaseOut".to_string(),
            "EaseInOut".to_string(),
            "CubicBezier".to_string(),
        ]
    }

    pub fn supported_animation_units() -> Vec<String> {
        vec![
            "All".to_string(),
            "Grapheme".to_string(),
            "Word".to_string(),
            "Line".to_string(),
        ]
    }

    pub fn supported_stagger_orders() -> Vec<String> {
        vec![
            "Forward".to_string(),
            "Backward".to_string(),
            "CenterOut".to_string(),
            "Random".to_string(),
        ]
    }

    pub fn supported_loop_modes() -> Vec<String> {
        vec![
            "None".to_string(),
            "Repeat".to_string(),
            "PingPong".to_string(),
        ]
    }

    pub fn supported_duration_conflict_policies() -> Vec<String> {
        vec![
            "Reject".to_string(),
            "ProportionalScale".to_string(),
            "ClampOut".to_string(),
        ]
    }

    /// Inspect an in-memory Project
    pub fn inspect_project(project: &Project) -> ProjectInspectReport {
        let mut total_duration = RationalTime::zero(project.settings.frame_rate);
        let mut clips_count = 0;
        let mut transitions_count = 0;

        let tracks = project
            .tracks
            .iter()
            .map(|t| {
                clips_count += t.items.len();
                transitions_count += t.transitions.len();
                for item in &t.items {
                    if item.timeline_range.end().semantic_gt(&total_duration) {
                        total_duration = item
                            .timeline_range
                            .end()
                            .rescaled_to(project.settings.frame_rate);
                    }
                }
                TrackSummary {
                    track_id: t.track_id.clone(),
                    name: t.name.clone(),
                    kind: t.kind,
                    muted: t.muted,
                    items_count: t.items.len(),
                    transitions_count: t.transitions.len(),
                }
            })
            .collect();

        let assets = project
            .assets
            .iter()
            .map(|a| AssetSummary {
                asset_id: a.asset_id.clone(),
                name: a.name.clone(),
                path: a.path.clone(),
                media_kind: a.media_kind,
                duration: a.duration,
            })
            .collect();

        ProjectInspectReport {
            project_id: project.project_id.clone(),
            name: project.name.clone(),
            revision: project.revision,
            duration_frames: total_duration.value,
            duration_seconds: total_duration.to_seconds(),
            frame_rate: project.settings.frame_rate,
            resolution: (project.settings.width, project.settings.height),
            tracks_count: project.tracks.len(),
            clips_count,
            assets_count: project.assets.len(),
            transitions_count,
            tracks,
            assets,
            supported_operations: Self::supported_operations(),
            supported_transitions: Self::supported_transitions(),
            schema_version: project.schema_version,
            supported_keyframe_properties: Self::supported_keyframe_properties(),
            supported_keyframe_easings: Self::supported_keyframe_easings(),
            custom_cubic_bounds: (0.0, 1.0),
            supported_animation_units: Self::supported_animation_units(),
            supported_stagger_orders: Self::supported_stagger_orders(),
            supported_loop_modes: Self::supported_loop_modes(),
            supported_duration_conflict_policies: Self::supported_duration_conflict_policies(),
            template_capabilities: Some(
                crate::modules::templates::agent::AgentTemplateService::capabilities(),
            ),
        }
    }

    /// Headless Inspect API for project files
    pub fn inspect_file(project_path: &Path) -> Result<ProjectInspectReport, AutomationError> {
        let mut session = AutomationSession::new();
        session.open_project(project_path)
    }

    /// Headless Diff / Dry-run API for project files
    pub fn dry_run_file(
        project_path: &Path,
        envelope: &CommandEnvelope,
    ) -> Result<DiffReport, AutomationError> {
        let mut session = AutomationSession::new();
        session.open_project(project_path)?;
        session.dry_run(envelope)
    }

    /// Headless Apply API for project files
    pub fn apply_file(
        project_path: &Path,
        service: &mut CommandService,
        envelope: CommandEnvelope,
        save_after: bool,
    ) -> Result<CommandReceipt, AutomationError> {
        let receipt = service.execute(envelope)?;
        if save_after {
            save_project_atomic(service.project(), project_path)
                .map_err(|e| AutomationError::ProjectLoad(e.to_string()))?;
        }
        Ok(receipt)
    }

    /// Headless Render / Export API for project files
    pub fn render_file(
        project_path: &Path,
        output_path: &Path,
        profile: Option<ExportProfile>,
    ) -> Result<ExportReport, AutomationError> {
        let project =
            load_project(project_path).map_err(|e| AutomationError::ProjectLoad(e.to_string()))?;
        let prof = profile.unwrap_or_default();
        let report = ExportEngine::render_snapshot(&project, output_path, &prof, None)?;
        Ok(report)
    }

    /// CLI argument dispatcher for headless workflows
    pub fn handle_cli(args: &[String]) -> Result<String, AutomationError> {
        if args.is_empty() {
            return Err(AutomationError::Cli(
                "Usage: tinixcut [inspect|diff|apply|render|session] <args...>".to_string(),
            ));
        }

        match args[0].as_str() {
            "inspect" => {
                if args.len() < 2 {
                    return Err(AutomationError::Cli(
                        "Usage: tinixcut inspect <path>".to_string(),
                    ));
                }
                let report = Self::inspect_file(Path::new(&args[1]))?;
                serde_json::to_string_pretty(&report)
                    .map_err(|e| AutomationError::Serialization(e.to_string()))
            }
            "diff" => {
                if args.len() < 3 {
                    return Err(AutomationError::Cli(
                        "Usage: tinixcut diff <project_path> <envelope_json_path>".to_string(),
                    ));
                }
                let env_content = std::fs::read_to_string(&args[2])
                    .map_err(|e| AutomationError::Cli(format!("Cannot read envelope file: {e}")))?;
                let envelope: CommandEnvelope = serde_json::from_str(&env_content)
                    .map_err(|e| AutomationError::Serialization(e.to_string()))?;
                let diff = Self::dry_run_file(Path::new(&args[1]), &envelope)?;
                serde_json::to_string_pretty(&diff)
                    .map_err(|e| AutomationError::Serialization(e.to_string()))
            }
            "apply" => {
                if args.len() < 3 {
                    return Err(AutomationError::Cli(
                        "Usage: tinixcut apply <project_path> <envelope_json_path>".to_string(),
                    ));
                }
                let project_path = Path::new(&args[1]);
                let project = load_project(project_path)
                    .map_err(|e| AutomationError::ProjectLoad(e.to_string()))?;
                let mut service = CommandService::new(project);
                let env_content = std::fs::read_to_string(&args[2])
                    .map_err(|e| AutomationError::Cli(format!("Cannot read envelope file: {e}")))?;
                let mut envelope: CommandEnvelope = serde_json::from_str(&env_content)
                    .map_err(|e| AutomationError::Serialization(e.to_string()))?;
                if envelope.client_id.is_none() {
                    let cur_epoch = service.project().client_registry.current_epoch;
                    let next_seq = service
                        .project()
                        .client_registry
                        .clients
                        .get("cli")
                        .map(|c| c.highest_sequence + 1)
                        .unwrap_or(1);
                    envelope.client_id = Some("cli".to_string());
                    envelope.client_epoch = Some(cur_epoch);
                    envelope.sequence_number = Some(next_seq);
                }
                let receipt = Self::apply_file(project_path, &mut service, envelope, true)?;
                serde_json::to_string_pretty(&receipt)
                    .map_err(|e| AutomationError::Serialization(e.to_string()))
            }
            "template" => {
                if args.len() < 2 {
                    return Err(AutomationError::Cli(
                        "Usage: tinixcut template [capabilities|schema|validate|preview|apply|save] <args...>".to_string(),
                    ));
                }
                match args[1].as_str() {
                    "capabilities" => {
                        let caps = crate::modules::templates::agent::AgentTemplateService::capabilities();
                        serde_json::to_string_pretty(&caps)
                            .map_err(|e| AutomationError::Serialization(e.to_string()))
                    }
                    "schema" => {
                        let schema = crate::modules::templates::agent::AgentTemplateService::draft_json_schema();
                        serde_json::to_string_pretty(&schema)
                            .map_err(|e| AutomationError::Serialization(e.to_string()))
                    }
                    "validate" => {
                        if args.len() < 3 {
                            return Err(AutomationError::Cli(
                                "Usage: tinixcut template validate <draft_path> [--project <project_path>]".to_string(),
                            ));
                        }
                        let draft_path = Path::new(&args[2]);
                        let content = std::fs::read_to_string(draft_path)
                            .map_err(|e| AutomationError::Cli(format!("Cannot read draft file: {e}")))?;
                        let draft: crate::modules::templates::agent::TemplateDraft = serde_json::from_str(&content)
                            .map_err(|e| AutomationError::Serialization(format!("Invalid TemplateDraft JSON: {e}")))?;
                        let catalog = crate::modules::templates::catalog::TemplateCatalog::default_catalog();
                        let report = crate::modules::templates::agent::AgentTemplateService::validate_draft(&draft, Some(&content), Some(&catalog));
                        serde_json::to_string_pretty(&report)
                            .map_err(|e| AutomationError::Serialization(e.to_string()))
                    }
                    "preview" => {
                        if args.len() < 4 {
                            return Err(AutomationError::Cli(
                                "Usage: tinixcut template preview <draft_path> <output_png_path> [--time <sec>] [--width <w>] [--height <h>]".to_string(),
                            ));
                        }
                        let draft_path = Path::new(&args[2]);
                        let output_path = Path::new(&args[3]);
                        let content = std::fs::read_to_string(draft_path)
                            .map_err(|e| AutomationError::Cli(format!("Cannot read draft file: {e}")))?;
                        let draft: crate::modules::templates::agent::TemplateDraft = serde_json::from_str(&content)
                            .map_err(|e| AutomationError::Serialization(format!("Invalid TemplateDraft JSON: {e}")))?;

                        let mut time_sec = 1.0f64;
                        let mut width = None;
                        let mut height = None;
                        let mut i = 4;
                        while i < args.len() {
                            if args[i] == "--time" && i + 1 < args.len() {
                                time_sec = args[i + 1].parse().unwrap_or(1.0);
                                i += 2;
                            } else if args[i] == "--width" && i + 1 < args.len() {
                                width = args[i + 1].parse().ok();
                                i += 2;
                            } else if args[i] == "--height" && i + 1 < args.len() {
                                height = args[i + 1].parse().ok();
                                i += 2;
                            } else {
                                i += 1;
                            }
                        }
                        let receipt = crate::modules::templates::agent::AgentTemplateService::preview_draft_to_png(
                            &draft, output_path, time_sec, width, height,
                        ).map_err(|e| AutomationError::Cli(format!("Template preview error: {e}")))?;
                        serde_json::to_string_pretty(&receipt)
                            .map_err(|e| AutomationError::Serialization(e.to_string()))
                    }
                    "apply" => {
                        if args.len() < 5 {
                            return Err(AutomationError::Cli(
                                "Usage: tinixcut template apply <project_path> <item_id> <draft_path> [--client-id <id>] [--client-epoch <epoch>] [--sequence <seq>] [--transaction-id <tx>] [--base-revision <rev>]".to_string(),
                            ));
                        }
                        let project_path = Path::new(&args[2]);
                        let item_id = &args[3];
                        let draft_path = Path::new(&args[4]);
                        let content = std::fs::read_to_string(draft_path)
                            .map_err(|e| AutomationError::Cli(format!("Cannot read draft file: {e}")))?;
                        let draft: crate::modules::templates::agent::TemplateDraft = serde_json::from_str(&content)
                            .map_err(|e| AutomationError::Serialization(format!("Invalid TemplateDraft JSON: {e}")))?;

                        let mut client_id = None;
                        let mut client_epoch = None;
                        let mut sequence_number = None;
                        let mut transaction_id = None;
                        let mut base_revision = None;
                        let mut i = 5;
                        while i < args.len() {
                            if args[i] == "--client-id" && i + 1 < args.len() {
                                client_id = Some(args[i + 1].clone());
                                i += 2;
                            } else if args[i] == "--client-epoch" && i + 1 < args.len() {
                                client_epoch = args[i + 1].parse().ok();
                                i += 2;
                            } else if args[i] == "--sequence" && i + 1 < args.len() {
                                sequence_number = args[i + 1].parse().ok();
                                i += 2;
                            } else if args[i] == "--transaction-id" && i + 1 < args.len() {
                                transaction_id = Some(args[i + 1].clone());
                                i += 2;
                            } else if args[i] == "--base-revision" && i + 1 < args.len() {
                                base_revision = args[i + 1].parse().ok();
                                i += 2;
                            } else {
                                i += 1;
                            }
                        }
                        let catalog = crate::modules::templates::catalog::TemplateCatalog::default_catalog();
                        let receipt = crate::modules::templates::agent::AgentTemplateService::apply_draft_to_project(
                            project_path, item_id, &draft, client_id, client_epoch, sequence_number, transaction_id, base_revision, &catalog,
                        ).map_err(|e| match e {
                            crate::modules::templates::agent::AgentTemplateError::Command(cmd_err) => {
                                AutomationError::Command(cmd_err)
                            }
                            other => AutomationError::Cli(format!("Template apply error: {other}")),
                        })?;
                        serde_json::to_string_pretty(&receipt)
                            .map_err(|e| AutomationError::Serialization(e.to_string()))
                    }
                    "save" => {
                        if args.len() < 3 {
                            return Err(AutomationError::Cli(
                                "Usage: tinixcut template save <draft_path> [--catalog-dir <dir>] [--overwrite]".to_string(),
                            ));
                        }
                        let draft_path = Path::new(&args[2]);
                        let content = std::fs::read_to_string(draft_path)
                            .map_err(|e| AutomationError::Cli(format!("Cannot read draft file: {e}")))?;
                        let draft: crate::modules::templates::agent::TemplateDraft = serde_json::from_str(&content)
                            .map_err(|e| AutomationError::Serialization(format!("Invalid TemplateDraft JSON: {e}")))?;
                        let mut catalog_dir = None;
                        let mut overwrite = false;
                        let mut i = 3;
                        while i < args.len() {
                            if args[i] == "--catalog-dir" && i + 1 < args.len() {
                                catalog_dir = Some(PathBuf::from(&args[i + 1]));
                                i += 2;
                            } else if args[i] == "--overwrite" {
                                overwrite = true;
                                i += 1;
                            } else {
                                i += 1;
                            }
                        }
                        let catalog = catalog_dir
                            .map(crate::modules::templates::catalog::TemplateCatalog::new)
                            .unwrap_or_else(crate::modules::templates::catalog::TemplateCatalog::default_catalog);
                        let receipt = crate::modules::templates::agent::AgentTemplateService::save_draft_to_catalog(
                            &draft, &catalog, overwrite,
                        ).map_err(|e| AutomationError::Cli(format!("Template save error: {e}")))?;
                        serde_json::to_string_pretty(&receipt)
                            .map_err(|e| AutomationError::Serialization(e.to_string()))
                    }
                    other => Err(AutomationError::Cli(format!("Unknown template command '{}'", other))),
                }
            }
            "render" => {
                if args.len() < 3 {
                    return Err(AutomationError::Cli(
                        "Usage: tinixcut render <project_path> <output_path>".to_string(),
                    ));
                }
                let report = Self::render_file(Path::new(&args[1]), Path::new(&args[2]), None)?;
                serde_json::to_string_pretty(&report)
                    .map_err(|e| AutomationError::Serialization(e.to_string()))
            }
            "session" | "--stdio" => {
                let stdin = std::io::stdin().lock();
                let stdout = std::io::stdout();
                let stderr = std::io::stderr();
                run_stdio_session(stdin, stdout, stderr);
                Ok("Session ended".to_string())
            }
            "help" | "--help" | "-h" => Ok(
                "TinixCut Automation CLI\nUsage: tinixcut [inspect|diff|apply|render|session] <args...>\n\nCommands:\n  inspect <project_path>                     Inspect a project and output JSON summary\n  diff <project_path> <envelope_json_path>   Run a dry-run diff of commands against a project\n  apply <project_path> <envelope_json_path>  Apply commands transactionally to a project\n  render <project_path> <output_path>        Render project to MP4 headlessly\n  session, --stdio                           Start long-lived machine-readable JSONL stdio session\n  help, --help                               Show this help message".to_string()
            ),
            other => Err(AutomationError::Cli(format!("Unknown command '{}'", other))),
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::modules::command::{CommandOperation, OperationEnvelope};
    use crate::modules::domain::time::{FrameRate, RationalTime, TimeRange};
    use crate::modules::domain::{
        Asset, ItemKind, MediaKind, Project, ProjectSettings, TimelineItem, Track, TrackKind,
        Transition, TransitionKind, CURRENT_SCHEMA_VERSION,
    };
    use crate::modules::media::resolver::MediaBinaryResolver;
    use std::fs;
    use std::path::PathBuf;

    fn get_fixture_dir() -> PathBuf {
        let dir = std::env::temp_dir().join("tinixcut_test_fixtures_phase10");
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn generate_synthetic_mp4(path: &Path, duration_secs: u32, color: &str) {
        if path.exists() {
            return;
        }
        if let Ok(mut cmd) = MediaBinaryResolver::command("ffmpeg") {
            let output = cmd
                .args([
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    &format!("color=c={}:s=640x360:r=30:d={}", color, duration_secs),
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
                    &path.to_string_lossy(),
                ])
                .output();

            if let Ok(res) = output {
                if !res.status.success() {
                    eprintln!(
                        "FFmpeg synthetic generator warning: {}",
                        String::from_utf8_lossy(&res.stderr)
                    );
                }
            }
        }
    }

    /// Complete 10-step Automation E2E Acceptance scenario defined in PLAN.md Phase 10
    #[test]
    fn test_phase10_automation_surface_e2e_acceptance() {
        let fixture_dir = get_fixture_dir();
        let project_path = fixture_dir.join("automation_fixture.tcproj");
        let media_file = fixture_dir.join("automation_media.mp4");
        generate_synthetic_mp4(&media_file, 4, "yellow");

        // 1. Open fixture project (create and save initial project with media)
        let rate = FrameRate::FPS_30;
        let mut initial_project = Project::new("Automation Fixture Project".to_string());
        initial_project.settings = ProjectSettings {
            width: 640,
            height: 360,
            canvas_background_color: "#000000".to_string(),
            frame_rate: rate,
            sample_rate: 48000,
            channels: 2,
        };
        initial_project.assets.push(Asset {
            asset_id: "ast_auto_1".to_string(),
            name: "automation_media.mp4".to_string(),
            path: media_file.to_string_lossy().to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::from_frames(120, rate)),
            file_size: 2048,
            dimensions: None,
        });

        save_project_atomic(&initial_project, &project_path).unwrap();

        // 2. Inspect revision through Inspect API
        let inspect_report = AutomationAdapter::inspect_file(&project_path).unwrap();
        assert_eq!(inspect_report.revision, 0);
        assert_eq!(inspect_report.project_id, initial_project.project_id);
        assert_eq!(inspect_report.assets_count, 1);
        assert_eq!(inspect_report.tracks_count, 0);
        assert!(inspect_report
            .supported_operations
            .contains(&"ClipCreate".to_string()));
        for operation in [
            "ClipTextStyleSet",
            "TextTemplateApply",
            "ClipTextTemplateReset",
        ] {
            assert!(
                inspect_report
                    .supported_operations
                    .contains(&operation.to_string()),
                "Automation capability missing {operation}"
            );
        }

        // Prepare an automation plan: Create video track and place a clip
        let plan_envelope = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: initial_project.project_id.clone(),
            base_revision: 0,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_auto_01".to_string()),
            operations: vec![
                OperationEnvelope {
                    operation_id: "op_auto_trk1".to_string(),
                    operation: CommandOperation::TrackCreate {
                        track_id: Some("trk_auto_v".to_string()),
                        name: "Auto Video Track".to_string(),
                        kind: TrackKind::Video,
                    },
                },
                OperationEnvelope {
                    operation_id: "op_auto_clip1".to_string(),
                    operation: CommandOperation::ClipCreate {
                        track_id: "trk_auto_v".to_string(),
                        item_id: Some("clip_auto_1".to_string()),
                        name: "Auto Clip 1".to_string(),
                        kind: ItemKind::Video,
                        asset_id: Some("ast_auto_1".to_string()),
                        timeline_start: RationalTime::from_frames(0, rate),
                        duration: RationalTime::from_frames(60, rate), // 2.0s
                        source_start: Some(RationalTime::zero(rate)),
                    },
                },
            ],
        };

        // 3. Dry-run plan: check diff, confirm project file on disk is NOT modified
        let dry_run_envelope = CommandEnvelope {
            dry_run: true,
            ..plan_envelope.clone()
        };
        let diff_report =
            AutomationAdapter::dry_run_file(&project_path, &dry_run_envelope).unwrap();
        assert_eq!(diff_report.operations_evaluated, 2);
        assert_eq!(diff_report.changes.len(), 2);
        // Verify disk state remains at revision 0
        let unmutated = load_project(&project_path).unwrap();
        assert_eq!(unmutated.revision, 0);
        assert_eq!(unmutated.tracks.len(), 0);

        // 4. Apply plan
        let mut service = CommandService::new(unmutated);
        let receipt = AutomationAdapter::apply_file(
            &project_path,
            &mut service,
            plan_envelope.clone(),
            true, // persist after apply
        )
        .unwrap();

        assert_eq!(receipt.prior_revision, 0);
        assert_eq!(receipt.new_revision, 1);
        assert_eq!(receipt.operations_applied, 2);

        // 5. Save/reopen: confirm committed state and drop in-memory service
        drop(service);
        let reopened = load_project(&project_path).unwrap();
        assert_eq!(reopened.revision, 1);
        assert_eq!(reopened.tracks.len(), 1);
        assert_eq!(reopened.tracks[0].items.len(), 1);
        assert_eq!(reopened.tracks[0].items[0].item_id, "clip_auto_1");
        assert_eq!(reopened.command_ledger.len(), 2);

        // 6. Retry same operation ID after reopen: test durable idempotency deduplication across save/reopen boundary
        let mut reopened_service = CommandService::new(reopened);
        let retry_receipt = reopened_service.execute(plan_envelope.clone()).unwrap();
        assert_eq!(retry_receipt.operations_applied, 0);
        for r in &retry_receipt.receipts {
            assert_eq!(r.status, "deduplicated_noop");
        }

        // 7. Confirm no duplicate edit: revision and item count are unchanged
        assert_eq!(reopened_service.project().revision, 1);
        assert_eq!(reopened_service.project().tracks[0].items.len(), 1);

        // 7b. Attempt ID collision: reuse op_auto_clip1 with different payload
        let collision_envelope = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: initial_project.project_id.clone(),
            base_revision: 1,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_coll".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_auto_clip1".to_string(), // Same ID!
                operation: CommandOperation::TrackCreate {
                    track_id: Some("trk_coll".to_string()),
                    name: "Collided Track".to_string(),
                    kind: TrackKind::Audio,
                },
            }],
        };
        let coll_res = reopened_service.execute(collision_envelope);
        assert!(matches!(
            coll_res,
            Err(CommandError::IdempotencyCollision { .. })
        ));
        assert_eq!(reopened_service.project().revision, 1);

        // 8. Attempt stale base revision: submit unseen edit with baseRevision = 0 when project is at 1
        let stale_envelope = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: initial_project.project_id.clone(),
            base_revision: 0, // Stale! Current is 1
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_stale".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_stale_new".to_string(),
                operation: CommandOperation::ClipDelete {
                    item_id: "clip_auto_1".to_string(),
                },
            }],
        };

        // 9. Confirm explicit conflict: StaleRevision error returned deterministically
        let stale_result = reopened_service.execute(stale_envelope);
        assert_eq!(
            stale_result,
            Err(CommandError::StaleRevision {
                expected: 1,
                received: 0,
            })
        );

        // 10. Export through job API: render the applied project
        let output_render = fixture_dir.join("automation_rendered_output.mp4");
        let mut profile = ExportProfile::default();
        profile.width = 640;
        profile.height = 360;

        let export_report =
            AutomationAdapter::render_file(&project_path, &output_render, Some(profile)).unwrap();

        assert!(output_render.exists());
        assert_eq!(export_report.duration_secs, 2.0);
        assert!(export_report.file_size_bytes > 0);
    }

    #[test]
    fn test_phase10_automation_session_and_stdio_protocol() {
        let fixture_dir = get_fixture_dir();
        let project_path = fixture_dir.join("session_test.tcproj");
        let rate = FrameRate::FPS_30;
        let mut proj = Project::new("Session Test Proj".to_string());
        proj.settings.frame_rate = rate;
        save_project_atomic(&proj, &project_path).unwrap();

        let path_escaped = project_path.to_string_lossy().replace('\\', "/");
        let jsonl_input = format!(
            "{{\"protocolVersion\":1,\"requestId\":\"req-1\",\"method\":\"session.open\",\"params\":{{\"path\":\"{path_escaped}\"}}}}\n\
             {{\"protocolVersion\":1,\"requestId\":\"req-2\",\"method\":\"project.inspect\",\"params\":{{}}}}\n\
             {{\"protocolVersion\":1,\"requestId\":\"req-3\",\"method\":\"project.capabilities\",\"params\":{{}}}}\n\
             {{malformed json line here\n\
             {{\"protocolVersion\":1,\"requestId\":\"req-5\",\"method\":\"unknown.invalidMethod\",\"params\":{{}}}}\n\
             {{\"protocolVersion\":1,\"requestId\":\"req-6\",\"method\":\"session.shutdown\",\"params\":{{}}}}\n"
        );

        let input_cursor = std::io::Cursor::new(jsonl_input.into_bytes());
        let mut stdout_buf = Vec::new();
        let mut stderr_buf = Vec::new();

        run_stdio_session(input_cursor, &mut stdout_buf, &mut stderr_buf);

        let stdout_str = String::from_utf8(stdout_buf).unwrap();
        let lines: Vec<&str> = stdout_str.lines().collect();

        // 6 valid single-line JSON responses
        assert_eq!(lines.len(), 6, "Expected 6 JSON response lines");

        let resp1: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(resp1["requestId"], "req-1");
        assert_eq!(resp1["ok"], true);
        assert_eq!(resp1["result"]["name"], "Session Test Proj");

        let resp2: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(resp2["requestId"], "req-2");
        assert_eq!(resp2["ok"], true);
        assert_eq!(resp2["result"]["revision"], 0);

        let resp3: serde_json::Value = serde_json::from_str(lines[2]).unwrap();
        assert_eq!(resp3["requestId"], "req-3");
        assert_eq!(resp3["ok"], true);
        assert_eq!(resp3["result"]["protocolVersion"], 1);

        let resp4: serde_json::Value = serde_json::from_str(lines[3]).unwrap();
        assert_eq!(resp4["requestId"], "unknown");
        assert_eq!(resp4["ok"], false);
        assert_eq!(resp4["error"]["code"], "INVALID_JSON");

        let resp5: serde_json::Value = serde_json::from_str(lines[4]).unwrap();
        assert_eq!(resp5["requestId"], "req-5");
        assert_eq!(resp5["ok"], false);
        assert_eq!(resp5["error"]["code"], "CLI_ERROR");

        let resp6: serde_json::Value = serde_json::from_str(lines[5]).unwrap();
        assert_eq!(resp6["requestId"], "req-6");
        assert_eq!(resp6["ok"], true);
        assert_eq!(resp6["result"]["shutdown"], true);
    }

    #[test]
    fn test_phase10_export_automation_job_lifecycle() {
        let fixture_dir = get_fixture_dir();
        let project_path = fixture_dir.join("export_auto_test.tcproj");
        let media_file = fixture_dir.join("export_auto_media.mp4");
        generate_synthetic_mp4(&media_file, 2, "blue");

        let rate = FrameRate::FPS_30;
        let mut project = Project::new("Export Auto Test".to_string());
        project.settings.frame_rate = rate;
        project.assets.push(Asset {
            asset_id: "ast_exp_1".to_string(),
            name: "media.mp4".to_string(),
            path: media_file.to_string_lossy().to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::from_frames(60, rate)),
            file_size: 1024,
            dimensions: None,
        });
        let mut track = Track::new("V1".to_string(), TrackKind::Video);
        let time_range = TimeRange::new(
            RationalTime::from_frames(0, rate),
            RationalTime::from_frames(30, rate),
        );
        let item = TimelineItem::new(
            Some("ast_exp_1".to_string()),
            "Clip 1".to_string(),
            ItemKind::Video,
            time_range,
            time_range,
        )
        .unwrap();
        track.items.push(item);
        project.tracks.push(track);
        save_project_atomic(&project, &project_path).unwrap();

        let mut session = AutomationSession::new();
        session.open_project(&project_path).unwrap();

        let out_target = fixture_dir.join("export_job_output.mp4");
        let mut profile = ExportProfile::default();
        profile.width = 640;
        profile.height = 360;

        // 1. Start export job asynchronously
        let receipt = session.export_start(&out_target, Some(profile)).unwrap();
        assert_eq!(receipt.status, "running");
        assert!(!receipt.job_id.is_empty());

        // 2. Poll export status
        let mut completed = false;
        for _ in 0..100 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            let status = session.export_status(&receipt.job_id).unwrap();
            if status.status == "completed" {
                completed = true;
                break;
            }
        }
        assert!(completed, "Export job must complete successfully");
        assert!(out_target.exists());
    }

    #[test]
    fn test_phase13_automation_transition_capability_discovery_and_crossdissolve() {
        let fixture_dir = get_fixture_dir();
        let project_path = fixture_dir.join("automation_transition_test.tcproj");
        let rate = FrameRate::FPS_30;
        let mut project = Project::new("Transition Capability Test".to_string());
        project.settings.frame_rate = rate;
        project.assets.push(Asset {
            asset_id: "ast_t1".to_string(),
            name: "test.mp4".to_string(),
            path: "test.mp4".to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::from_frames(120, rate)),
            file_size: 1024,
            dimensions: None,
        });

        let mut track = Track::new("V1".to_string(), TrackKind::Video);
        let time_range = TimeRange::new(
            RationalTime::from_frames(0, rate),
            RationalTime::from_frames(60, rate),
        );
        let item = TimelineItem::new(
            Some("ast_t1".to_string()),
            "Clip 1".to_string(),
            ItemKind::Video,
            time_range,
            time_range,
        )
        .unwrap();
        let item_id = item.item_id.clone();
        track.items.push(item);
        project.tracks.push(track);

        save_project_atomic(&project, &project_path).unwrap();

        // 1. Capability discovery test: inspect report
        let report = AutomationAdapter::inspect_file(&project_path).unwrap();
        assert!(report.supported_transitions.contains(&"FadeIn".to_string()));
        assert!(report
            .supported_transitions
            .contains(&"FadeOut".to_string()));
        assert!(report
            .supported_transitions
            .contains(&"FadeBlack".to_string()));
        assert!(report
            .supported_transitions
            .contains(&"CrossDissolve".to_string()));
        assert!(report
            .supported_operations
            .contains(&"CutTransitionCreate".to_string()));

        // 2. Direct attempt to apply CrossDissolve via item-level command (MUST fail with UnsupportedTransition)
        let cross_dissolve_envelope = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision: 0,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_auto_crossdissolve".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_crossdissolve".to_string(),
                operation: CommandOperation::ClipTransitionSet {
                    item_id: item_id.clone(),
                    transition: Some(Transition {
                        kind: TransitionKind::CrossDissolve,
                        duration: RationalTime::from_frames(15, rate),
                    }),
                },
            }],
        };

        let mut service = CommandService::new(project);
        let result = AutomationAdapter::apply_file(
            &project_path,
            &mut service,
            cross_dissolve_envelope,
            true,
        );
        match result {
            Err(AutomationError::Command(CommandError::UnsupportedTransition {
                operation_id,
                transition,
            })) => {
                assert_eq!(operation_id, "op_crossdissolve");
                assert_eq!(transition, "CrossDissolve");
            }
            other => panic!(
                "Expected CommandError::UnsupportedTransition, got {:?}",
                other
            ),
        }

        // Verify disk state remains intact and revision 0
        let unmutated = load_project(&project_path).unwrap();
        assert_eq!(unmutated.revision, 0);
        assert!(unmutated.tracks[0].items[0].transition.is_none());

        // 3. Add adjacent clip and create CutTransition via automation
        let time_range_2 = TimeRange::new(
            RationalTime::from_frames(60, rate),
            RationalTime::from_frames(60, rate),
        );
        let item_2 = TimelineItem::new(
            Some("ast_t1".to_string()),
            "Clip 2".to_string(),
            ItemKind::Video,
            time_range_2,
            time_range_2,
        )
        .unwrap();
        let item_id_2 = item_2.item_id.clone();
        let track_id = service.project().tracks[0].track_id.clone();

        let add_clip_envelope = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: service.project().project_id.clone(),
            base_revision: service.revision(),
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_add_clip2".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_add_clip2".to_string(),
                operation: CommandOperation::ClipCreate {
                    track_id: track_id.clone(),
                    item_id: Some(item_id_2.clone()),
                    name: "Clip 2".to_string(),
                    kind: ItemKind::Video,
                    asset_id: Some("ast_t1".to_string()),
                    timeline_start: RationalTime::from_frames(60, rate),
                    duration: RationalTime::from_frames(60, rate),
                    source_start: Some(RationalTime::from_frames(60, rate)),
                },
            }],
        };
        AutomationAdapter::apply_file(&project_path, &mut service, add_clip_envelope, true)
            .unwrap();

        // 4. Apply CutTransitionCreate via automation
        let cut_trans_envelope = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: service.project().project_id.clone(),
            base_revision: service.revision(),
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_auto_cut_trans".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_cut_trans".to_string(),
                operation: CommandOperation::CutTransitionCreate {
                    track_id,
                    transition_id: Some("auto_tr1".to_string()),
                    kind: TransitionKind::CrossDissolve,
                    left_item_id: item_id,
                    right_item_id: item_id_2,
                    duration: RationalTime::from_frames(20, rate),
                },
            }],
        };

        // Test dry-run
        let diff = AutomationAdapter::dry_run_file(&project_path, &cut_trans_envelope).unwrap();
        assert_eq!(diff.changes.len(), 1);

        // Test apply and atomic save
        let receipt =
            AutomationAdapter::apply_file(&project_path, &mut service, cut_trans_envelope, true)
                .unwrap();
        assert_eq!(receipt.new_revision, 2);

        // Verify reloaded project has cut transition in Schema v2
        let reloaded = load_project(&project_path).unwrap();
        assert_eq!(
            reloaded.schema_version,
            crate::modules::domain::CURRENT_SCHEMA_VERSION
        );
        assert_eq!(reloaded.tracks[0].transitions.len(), 1);
        assert_eq!(reloaded.tracks[0].transitions[0].transition_id, "auto_tr1");
    }
}
