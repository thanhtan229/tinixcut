//! Durable, non-sensitive preconditions for resuming an automation workflow.

use crate::modules::automation::edit_plan::{
    validate_edit_plan, EditPlan, EDIT_PLAN_SCHEMA_VERSION,
};
use crate::modules::domain::Project;
use crate::modules::media::ffmpeg::FFmpegService;
use schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const WORKFLOW_MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowInputFingerprint {
    pub asset_id: String,
    /// Stable source identity based on canonical path, size and modified time; raw paths are not stored.
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowManifest {
    pub schema_version: u32,
    pub workflow_id: String,
    pub project_id: String,
    pub base_revision: u64,
    pub plan_schema_version: u32,
    pub plan_sha256: String,
    pub operation_ids: Vec<String>,
    pub input_fingerprints: Vec<WorkflowInputFingerprint>,
    pub artifact_paths: Vec<String>,
    pub state: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCheckpointState {
    Applied,
    Saved,
    Exported,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowCheckpoint {
    pub schema_version: u32,
    pub workflow_id: String,
    pub state: WorkflowCheckpointState,
    pub project_id: String,
    pub base_revision: u64,
    pub project_revision: u64,
    pub plan_sha256: String,
    pub operation_ids: Vec<String>,
    pub artifact_path: Option<String>,
    pub artifact_sha256: Option<String>,
    pub artifact_size_bytes: Option<u64>,
    pub video_codec: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_millis: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowCheckpointReceipt {
    pub checkpoint_path: String,
    pub checkpoint: WorkflowCheckpoint,
    pub already_recorded: bool,
}

impl WorkflowManifest {
    pub fn create(
        project: &Project,
        plan: &EditPlan,
        artifact_paths: &[String],
    ) -> Result<Self, String> {
        if plan.project_id != project.project_id || plan.base_revision != project.revision {
            return Err("Edit plan does not match the current project revision".to_string());
        }
        let validation = validate_edit_plan(project, plan);
        if !validation.valid {
            return Err(format!(
                "Cannot create a workflow manifest for an invalid edit plan: {}",
                validation.issues.join("; ")
            ));
        }
        let plan_sha256 = plan_fingerprint(plan)?;
        let operation_ids = plan_operation_ids(plan);
        if operation_ids.iter().any(|id| id.trim().is_empty())
            || operation_ids.iter().collect::<HashSet<_>>().len() != operation_ids.len()
        {
            return Err("Edit plan operation IDs must be non-empty and unique".to_string());
        }
        let mut input_fingerprints = Vec::with_capacity(plan.referenced_media_ids.len());
        let mut seen_assets = HashSet::new();
        for asset_id in &plan.referenced_media_ids {
            if !seen_assets.insert(asset_id) {
                return Err("Edit plan referenced media IDs must be unique".to_string());
            }
            let asset = project
                .assets
                .iter()
                .find(|asset| asset.asset_id == *asset_id)
                .ok_or_else(|| format!("Referenced media does not exist: {asset_id}"))?;
            let sha256 = FFmpegService::source_fingerprint(Path::new(&asset.path))
                .map_err(|error| format!("Could not fingerprint media {asset_id}: {error}"))?;
            input_fingerprints.push(WorkflowInputFingerprint {
                asset_id: asset_id.clone(),
                fingerprint: sha256,
            });
        }
        validate_artifact_paths(artifact_paths)?;
        Ok(Self {
            schema_version: WORKFLOW_MANIFEST_SCHEMA_VERSION,
            workflow_id: uuid::Uuid::new_v4().to_string(),
            project_id: project.project_id.clone(),
            base_revision: project.revision,
            plan_schema_version: plan.schema_version,
            plan_sha256,
            operation_ids,
            input_fingerprints,
            artifact_paths: artifact_paths.to_vec(),
            state: "planned".to_string(),
        })
    }

    /// Resume is allowed only while the planned project revision and every input still match.
    /// This prevents a retry from applying the same plan on top of a changed project.
    pub fn validate_resume(&self, project: &Project, plan: &EditPlan) -> Result<(), String> {
        if self.schema_version != WORKFLOW_MANIFEST_SCHEMA_VERSION
            || self.plan_schema_version != EDIT_PLAN_SCHEMA_VERSION
            || self.state != "planned"
        {
            return Err(
                "Workflow manifest schema, plan version, or state is unsupported".to_string(),
            );
        }
        if self.project_id != project.project_id || self.base_revision != project.revision {
            return Err("Workflow manifest is stale for the current project revision".to_string());
        }
        if plan.project_id != self.project_id || plan.base_revision != self.base_revision {
            return Err("Workflow plan is stale for the manifest revision".to_string());
        }
        if self.plan_sha256.len() != 64
            || !self
                .plan_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || self.operation_ids.iter().any(|id| id.trim().is_empty())
            || self.operation_ids.iter().collect::<HashSet<_>>().len() != self.operation_ids.len()
        {
            return Err("Workflow manifest contains invalid plan identity data".to_string());
        }
        if self.plan_schema_version != plan.schema_version
            || self.plan_sha256 != plan_fingerprint(plan)?
            || self.operation_ids != plan_operation_ids(plan)
        {
            return Err(
                "Workflow plan content or operation IDs do not match the manifest".to_string(),
            );
        }
        let expected = Self::create(project, plan, &self.artifact_paths)?;
        if self.input_fingerprints != expected.input_fingerprints {
            return Err("Workflow media inputs changed since planning".to_string());
        }
        Ok(())
    }

    /// Reconcile a completed automation step against the saved project and, for export, the actual
    /// media file. Checkpoints are immutable sidecars; retries are idempotent only for identical
    /// evidence and never overwrite an earlier result.
    pub fn record_checkpoint(
        &self,
        manifest_path: &Path,
        project: &Project,
        plan: &EditPlan,
        state: WorkflowCheckpointState,
        artifact_path: Option<&Path>,
    ) -> Result<WorkflowCheckpointReceipt, String> {
        self.validate_plan_identity(project, plan)?;
        self.validate_current_inputs(project)?;
        let checkpoint_dir = checkpoint_dir(manifest_path)?;

        match state {
            WorkflowCheckpointState::Applied => {
                if project.revision <= self.base_revision {
                    return Err("Applied checkpoint requires a project revision newer than the planned revision".into());
                }
                let applied_entries = project
                    .command_ledger
                    .iter()
                    .filter(|entry| self.operation_ids.contains(&entry.operation_id))
                    .collect::<Vec<_>>();
                let applied_revisions = applied_entries
                    .iter()
                    .map(|entry| entry.applied_at_revision)
                    .collect::<HashSet<_>>();
                if self.operation_ids.iter().any(|id| {
                    !applied_entries.iter().any(|entry| {
                        entry.operation_id == *id && entry.applied_at_revision > self.base_revision
                    })
                }) || applied_entries.len() != self.operation_ids.len()
                    || applied_revisions.len() != 1
                {
                    return Err("Applied checkpoint requires every planned operation in the durable command ledger".into());
                }
            }
            WorkflowCheckpointState::Saved => {
                let applied = read_checkpoint(&checkpoint_dir, WorkflowCheckpointState::Applied)?
                    .ok_or_else(|| {
                    "Saved checkpoint requires a recorded applied checkpoint".to_string()
                })?;
                self.validate_checkpoint_identity(&applied, WorkflowCheckpointState::Applied)?;
                if applied.project_revision != project.revision {
                    return Err(
                        "Saved project revision does not match the applied checkpoint".into(),
                    );
                }
            }
            WorkflowCheckpointState::Exported => {
                let saved = read_checkpoint(&checkpoint_dir, WorkflowCheckpointState::Saved)?
                    .ok_or_else(|| {
                        "Export checkpoint requires a recorded saved checkpoint".to_string()
                    })?;
                self.validate_checkpoint_identity(&saved, WorkflowCheckpointState::Saved)?;
                if saved.project_revision != project.revision {
                    return Err(
                        "Exported project revision does not match the saved checkpoint".into(),
                    );
                }
            }
        }

        let mut checkpoint = WorkflowCheckpoint {
            schema_version: 1,
            workflow_id: self.workflow_id.clone(),
            state,
            project_id: project.project_id.clone(),
            base_revision: self.base_revision,
            project_revision: project.revision,
            plan_sha256: self.plan_sha256.clone(),
            operation_ids: self.operation_ids.clone(),
            artifact_path: None,
            artifact_sha256: None,
            artifact_size_bytes: None,
            video_codec: None,
            width: None,
            height: None,
            duration_millis: None,
        };
        if state == WorkflowCheckpointState::Exported {
            let artifact = artifact_path
                .ok_or_else(|| "Exported checkpoint requires an artifact path".to_string())?;
            let artifact_text = artifact.to_string_lossy().to_string();
            if !self
                .artifact_paths
                .iter()
                .any(|path| path == &artifact_text)
            {
                return Err(
                    "Exported artifact path is not declared in the workflow manifest".into(),
                );
            }
            let metadata = FFmpegService::probe_file(artifact)
                .map_err(|error| format!("Could not verify exported artifact: {error}"))?;
            let video_codec = metadata
                .video_codec
                .filter(|codec| !codec.trim().is_empty())
                .ok_or_else(|| "Exported artifact has no video stream".to_string())?;
            if !metadata.duration_seconds.is_finite() || metadata.duration_seconds <= 0.0 {
                return Err("Exported artifact has an invalid duration".into());
            }
            let artifact_size = fs::metadata(artifact)
                .map_err(|error| error.to_string())?
                .len();
            checkpoint.artifact_sha256 = Some(sha256_file(artifact)?);
            checkpoint.artifact_size_bytes = Some(artifact_size);
            checkpoint.artifact_path = Some(artifact_text);
            checkpoint.video_codec = Some(video_codec);
            checkpoint.width = Some(metadata.width);
            checkpoint.height = Some(metadata.height);
            checkpoint.duration_millis = Some((metadata.duration_seconds * 1_000.0).round() as u64);
        } else if artifact_path.is_some() {
            return Err("Artifact path is only valid for an exported checkpoint".into());
        }

        let checkpoint_path = save_checkpoint_idempotently(&checkpoint_dir, &checkpoint)?;
        Ok(checkpoint_path)
    }

    fn validate_plan_identity(&self, project: &Project, plan: &EditPlan) -> Result<(), String> {
        if self.schema_version != WORKFLOW_MANIFEST_SCHEMA_VERSION
            || self.plan_schema_version != plan.schema_version
            || self.project_id != project.project_id
            || plan.project_id != self.project_id
            || plan.base_revision != self.base_revision
            || self.plan_sha256 != plan_fingerprint(plan)?
            || self.operation_ids != plan_operation_ids(plan)
        {
            return Err(
                "Workflow checkpoint does not match the manifest, plan, or project identity".into(),
            );
        }
        Ok(())
    }

    fn validate_current_inputs(&self, project: &Project) -> Result<(), String> {
        for expected in &self.input_fingerprints {
            let asset = project
                .assets
                .iter()
                .find(|asset| asset.asset_id == expected.asset_id)
                .ok_or_else(|| format!("Workflow input asset is missing: {}", expected.asset_id))?;
            let actual =
                FFmpegService::source_fingerprint(Path::new(&asset.path)).map_err(|error| {
                    format!(
                        "Could not recheck workflow input {}: {error}",
                        expected.asset_id
                    )
                })?;
            if actual != expected.fingerprint {
                return Err(format!(
                    "Workflow input changed before checkpoint: {}",
                    expected.asset_id
                ));
            }
        }
        Ok(())
    }

    fn validate_checkpoint_identity(
        &self,
        checkpoint: &WorkflowCheckpoint,
        expected_state: WorkflowCheckpointState,
    ) -> Result<(), String> {
        if checkpoint.schema_version != 1
            || checkpoint.workflow_id != self.workflow_id
            || checkpoint.state != expected_state
            || checkpoint.project_id != self.project_id
            || checkpoint.base_revision != self.base_revision
            || checkpoint.plan_sha256 != self.plan_sha256
            || checkpoint.operation_ids != self.operation_ids
        {
            return Err("Prior workflow checkpoint identity does not match this manifest".into());
        }
        Ok(())
    }

    /// Persist without overwriting an existing manifest. The temporary file is written and
    /// flushed first, then hard-linked into place on the same volume for atomic no-clobber.
    pub fn save_new(&self, path: &Path) -> Result<PathBuf, String> {
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let name = path
            .file_name()
            .ok_or_else(|| "Manifest path must include a file name".to_string())?;
        let temp_path = parent.join(format!(
            ".{}.{}.tmp",
            name.to_string_lossy(),
            uuid::Uuid::new_v4().simple()
        ));
        let bytes = serde_json::to_vec_pretty(self).map_err(|error| error.to_string())?;
        let write_result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp_path)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::hard_link(&temp_path, path)?;
            Ok::<_, std::io::Error>(())
        })();
        let _ = fs::remove_file(&temp_path);
        write_result.map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                format!(
                    "Workflow manifest already exists; refusing to overwrite: {}",
                    path.display()
                )
            } else {
                format!("Could not persist workflow manifest: {error}")
            }
        })?;
        Ok(path.to_path_buf())
    }
}

fn plan_fingerprint(plan: &EditPlan) -> Result<String, String> {
    let bytes = serde_json::to_vec(plan).map_err(|error| error.to_string())?;
    let mut digest = Sha256::new();
    digest.update(bytes);
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn plan_operation_ids(plan: &EditPlan) -> Vec<String> {
    plan.operations
        .iter()
        .map(|operation| operation.operation_id.clone())
        .collect()
}

fn validate_artifact_paths(paths: &[String]) -> Result<(), String> {
    if paths.len() > 64 {
        return Err("A workflow manifest may reference at most 64 artifacts".to_string());
    }
    let mut seen = HashSet::new();
    for path in paths {
        if path.trim().is_empty() || path.len() > 4_096 || !seen.insert(path.to_lowercase()) {
            return Err(
                "Artifact paths must be non-empty, unique, and at most 4096 bytes".to_string(),
            );
        }
    }
    Ok(())
}

fn checkpoint_dir(manifest_path: &Path) -> Result<PathBuf, String> {
    let parent = manifest_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = manifest_path
        .file_name()
        .ok_or_else(|| "Manifest path must include a file name".to_string())?;
    Ok(parent.join(format!("{}.checkpoints", name.to_string_lossy())))
}

fn checkpoint_state_name(state: WorkflowCheckpointState) -> &'static str {
    match state {
        WorkflowCheckpointState::Applied => "applied",
        WorkflowCheckpointState::Saved => "saved",
        WorkflowCheckpointState::Exported => "exported",
    }
}

fn read_checkpoint(
    directory: &Path,
    state: WorkflowCheckpointState,
) -> Result<Option<WorkflowCheckpoint>, String> {
    let path = directory.join(format!("{}.json", checkpoint_state_name(state)));
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| format!("Workflow checkpoint is invalid: {error}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("Could not read workflow checkpoint: {error}")),
    }
}

fn save_checkpoint_idempotently(
    directory: &Path,
    checkpoint: &WorkflowCheckpoint,
) -> Result<WorkflowCheckpointReceipt, String> {
    fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let path = directory.join(format!("{}.json", checkpoint_state_name(checkpoint.state)));
    let bytes = serde_json::to_vec_pretty(checkpoint).map_err(|error| error.to_string())?;
    if path.exists() {
        let existing: WorkflowCheckpoint =
            serde_json::from_slice(&fs::read(&path).map_err(|error| error.to_string())?)
                .map_err(|error| format!("Existing workflow checkpoint is invalid: {error}"))?;
        if existing != *checkpoint {
            return Err(
                "Workflow checkpoint already exists with different evidence; refusing to overwrite"
                    .into(),
            );
        }
        return Ok(WorkflowCheckpointReceipt {
            checkpoint_path: path.to_string_lossy().to_string(),
            checkpoint: existing,
            already_recorded: true,
        });
    }
    let name = path
        .file_name()
        .ok_or_else(|| "Checkpoint path must include a file name".to_string())?;
    let temp_path = directory.join(format!(
        ".{}.{}.tmp",
        name.to_string_lossy(),
        uuid::Uuid::new_v4().simple()
    ));
    let write_result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::hard_link(&temp_path, &path)?;
        Ok::<_, std::io::Error>(())
    })();
    let _ = fs::remove_file(&temp_path);
    match write_result {
        Ok(()) => Ok(WorkflowCheckpointReceipt {
            checkpoint_path: path.to_string_lossy().to_string(),
            checkpoint: checkpoint.clone(),
            already_recorded: false,
        }),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let existing: WorkflowCheckpoint = serde_json::from_slice(
                &fs::read(&path).map_err(|read_error| read_error.to_string())?,
            )
            .map_err(|parse_error| {
                format!("Existing workflow checkpoint is invalid: {parse_error}")
            })?;
            if existing == *checkpoint {
                Ok(WorkflowCheckpointReceipt {
                    checkpoint_path: path.to_string_lossy().to_string(),
                    checkpoint: existing,
                    already_recorded: true,
                })
            } else {
                Err("Workflow checkpoint already exists with different evidence; refusing to overwrite".into())
            }
        }
        Err(error) => Err(format!("Could not persist workflow checkpoint: {error}")),
    }
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file =
        fs::File::open(path).map_err(|error| format!("Could not open export artifact: {error}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("Could not hash export artifact: {error}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::automation::edit_plan::EditPlan;
    use crate::modules::command::{
        CommandEnvelope, CommandOperation, CommandService, OperationEnvelope,
    };
    use crate::modules::domain::{Project, TrackKind};
    use crate::modules::media::resolver::MediaBinaryResolver;
    use std::process::Stdio;

    fn plan_for(project: &Project) -> EditPlan {
        EditPlan {
            schema_version: EDIT_PLAN_SCHEMA_VERSION,
            goal: "Private brief text must not be persisted".to_string(),
            project_id: project.project_id.clone(),
            base_revision: project.revision,
            referenced_media_ids: Vec::new(),
            assumptions: Vec::new(),
            warnings: Vec::new(),
            operations: vec![OperationEnvelope {
                operation_id: "workflow_op_1".to_string(),
                operation: CommandOperation::TrackCreate {
                    track_id: Some("workflow_video".to_string()),
                    name: "Draft video".to_string(),
                    kind: TrackKind::Video,
                },
            }],
        }
    }

    #[test]
    fn manifest_stores_hash_and_operation_ids_without_brief_text() {
        let project = Project::new("Manifest test".to_string());
        let plan = plan_for(&project);
        let manifest =
            WorkflowManifest::create(&project, &plan, &["output.mp4".to_string()]).unwrap();
        manifest.validate_resume(&project, &plan).unwrap();
        let serialized = serde_json::to_string(&manifest).unwrap();
        assert!(serialized.contains("workflow_op_1"));
        assert!(serialized.contains("output.mp4"));
        assert!(!serialized.contains("Private brief"));
        assert_eq!(manifest.plan_schema_version, EDIT_PLAN_SCHEMA_VERSION);
        assert_eq!(manifest.state, "planned");
    }

    #[test]
    fn manifest_resume_rejects_changed_revision_and_manifest_save_refuses_overwrite() {
        let project = Project::new("Manifest test".to_string());
        let plan = plan_for(&project);
        let manifest = WorkflowManifest::create(&project, &plan, &[]).unwrap();
        let mut changed = project.clone();
        changed.revision += 1;
        assert!(manifest
            .validate_resume(&changed, &plan)
            .unwrap_err()
            .contains("stale"));
        let mut changed_plan = plan.clone();
        changed_plan.goal.push_str(" changed");
        assert!(manifest
            .validate_resume(&project, &changed_plan)
            .unwrap_err()
            .contains("content"));

        let root = std::env::temp_dir().join(format!(
            "tinixcut-workflow-manifest-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let path = root.join("workflow.json");
        manifest.save_new(&path).unwrap();
        assert!(manifest
            .save_new(&path)
            .unwrap_err()
            .contains("refusing to overwrite"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn checkpoints_are_ordered_idempotent_and_attest_the_export_artifact() {
        let root = std::env::temp_dir().join(format!(
            "tinixcut-workflow-checkpoint-{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&root).unwrap();
        let project_path = root.join("project.tinix");
        let manifest_path = root.join("workflow.json");
        let artifact_path = root.join("export.mp4");

        let project = Project::new("Workflow checkpoint test".to_string());
        crate::modules::persistence::save_project_atomic(&project, &project_path).unwrap();
        let plan = plan_for(&project);
        let manifest = WorkflowManifest::create(
            &project,
            &plan,
            &[artifact_path.to_string_lossy().to_string()],
        )
        .unwrap();
        manifest.save_new(&manifest_path).unwrap();

        assert!(manifest
            .record_checkpoint(
                &manifest_path,
                &project,
                &plan,
                WorkflowCheckpointState::Exported,
                Some(&artifact_path),
            )
            .unwrap_err()
            .contains("requires a recorded saved checkpoint"));

        let mut service = CommandService::new(project.clone());
        service
            .execute(CommandEnvelope {
                schema_version: crate::modules::domain::CURRENT_SCHEMA_VERSION,
                project_id: project.project_id.clone(),
                base_revision: project.revision,
                client_id: Some("workflow-checkpoint-test".to_string()),
                client_epoch: Some(1),
                sequence_number: Some(1),
                dry_run: false,
                transaction_id: Some("workflow-checkpoint-apply".to_string()),
                operations: plan.operations.clone(),
            })
            .unwrap();
        let applied_project = service.project().clone();
        let applied = manifest
            .record_checkpoint(
                &manifest_path,
                &applied_project,
                &plan,
                WorkflowCheckpointState::Applied,
                None,
            )
            .unwrap();
        assert_eq!(applied.checkpoint.state, WorkflowCheckpointState::Applied);
        assert!(!applied.already_recorded);
        let replay = manifest
            .record_checkpoint(
                &manifest_path,
                &applied_project,
                &plan,
                WorkflowCheckpointState::Applied,
                None,
            )
            .unwrap();
        assert!(replay.already_recorded);

        crate::modules::persistence::save_project_atomic(&applied_project, &project_path).unwrap();
        let saved_project = crate::modules::persistence::load_project(&project_path).unwrap();
        let saved = manifest
            .record_checkpoint(
                &manifest_path,
                &saved_project,
                &plan,
                WorkflowCheckpointState::Saved,
                None,
            )
            .unwrap();
        assert_eq!(saved.checkpoint.project_revision, saved_project.revision);

        let mut ffmpeg = MediaBinaryResolver::command("ffmpeg").unwrap();
        let output = ffmpeg
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=blue:s=160x90:r=30:d=1",
                "-an",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-movflags",
                "+faststart",
            ])
            .arg(&artifact_path)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .expect("app FFmpeg should create a deterministic checkpoint fixture");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );

        let exported = manifest
            .record_checkpoint(
                &manifest_path,
                &saved_project,
                &plan,
                WorkflowCheckpointState::Exported,
                Some(&artifact_path),
            )
            .unwrap();
        assert_eq!(exported.checkpoint.video_codec.as_deref(), Some("h264"));
        assert_eq!(exported.checkpoint.width, Some(160));
        assert_eq!(exported.checkpoint.height, Some(90));
        assert_eq!(exported.checkpoint.duration_millis, Some(1_000));
        assert!(exported.checkpoint.artifact_sha256.is_some());
        assert!(
            manifest
                .record_checkpoint(
                    &manifest_path,
                    &saved_project,
                    &plan,
                    WorkflowCheckpointState::Exported,
                    Some(&artifact_path),
                )
                .unwrap()
                .already_recorded
        );

        let mut ffmpeg = MediaBinaryResolver::command("ffmpeg").unwrap();
        let output = ffmpeg
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=red:s=160x90:r=30:d=1",
                "-an",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-movflags",
                "+faststart",
            ])
            .arg(&artifact_path)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .expect("app FFmpeg should create a second valid collision fixture");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(manifest
            .record_checkpoint(
                &manifest_path,
                &saved_project,
                &plan,
                WorkflowCheckpointState::Exported,
                Some(&artifact_path),
            )
            .unwrap_err()
            .contains("refusing to overwrite"));
        fs::remove_dir_all(root).unwrap();
    }
}
