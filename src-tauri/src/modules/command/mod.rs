//! Command Service & Engine for TinixCut
//! Architecture Invariant: All authoritative mutations pass through this engine.
//! UI, Shortcuts, and Automation share the exact same command mutation path.
//! Handles baseRevision checks, idempotency receipts, atomic transactions, and dry-run/diff.

use crate::modules::domain::id::*;
use crate::modules::domain::time::{RationalTime, TimeRange};
use crate::modules::domain::{
    Crop, DomainError, ItemKind, Keyframe, KeyframeProperty, Project, TimelineItem, Track,
    TrackKind, Transform, Transition, CURRENT_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandError {
    #[error("Schema version mismatch: expected {expected}, received {received}")]
    SchemaVersionMismatch { expected: u32, received: u32 },

    #[error("Project ID mismatch: expected {expected}, received {received}")]
    ProjectIdMismatch { expected: String, received: String },

    #[error("Stale revision conflict: current project revision is {expected}, but baseRevision was {received}")]
    StaleRevision { expected: u64, received: u64 },

    #[error("Operation '{operation_id}' validation failed: {message}")]
    OperationValidationFailed {
        operation_id: String,
        message: String,
    },

    #[error("Domain error: {0}")]
    Domain(#[from] DomainError),

    #[error("Transaction '{0}' failed and was rolled back completely")]
    TransactionRolledBack(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "payload")]
pub enum CommandOperation {
    TrackCreate {
        track_id: Option<String>,
        name: String,
        kind: TrackKind,
    },
    TrackDelete {
        track_id: String,
    },
    ClipCreate {
        track_id: String,
        item_id: Option<String>,
        name: String,
        kind: ItemKind,
        asset_id: Option<String>,
        timeline_start: RationalTime,
        duration: RationalTime,
        source_start: Option<RationalTime>,
    },
    ClipMove {
        item_id: String,
        target_track_id: Option<String>,
        target_start: RationalTime,
    },
    ClipTrim {
        item_id: String,
        new_timeline_range: TimeRange,
        new_source_range: TimeRange,
    },
    ClipSplit {
        item_id: String,
        split_time: RationalTime,
        new_right_item_id: Option<String>,
    },
    ClipDelete {
        item_id: String,
    },
    ClipDuplicate {
        item_id: String,
        target_start: Option<RationalTime>,
        target_track_id: Option<String>,
        new_item_id: Option<String>,
    },
    ClipTransformSet {
        item_id: String,
        transform: Transform,
    },
    ClipCropSet {
        item_id: String,
        crop: Crop,
    },
    ClipOpacitySet {
        item_id: String,
        opacity: f32,
    },
    TrackReorder {
        track_id: String,
        new_index: usize,
    },
    TrackMuteSet {
        track_id: String,
        muted: bool,
    },
    ClipVolumeSet {
        item_id: String,
        volume: f32,
    },
    AssetImport {
        asset_id: Option<String>,
        name: String,
        path: String,
        media_kind: crate::modules::domain::MediaKind,
        file_size: u64,
    },
    ClipTextSet {
        item_id: String,
        text: String,
    },
    ClipAudioFadeSet {
        item_id: String,
        fade_in: Option<RationalTime>,
        fade_out: Option<RationalTime>,
    },
    ClipTransitionSet {
        item_id: String,
        transition: Option<Transition>,
    },
    ClipKeyframeAdd {
        item_id: String,
        keyframe: Keyframe,
    },
    ClipKeyframeRemove {
        item_id: String,
        property: KeyframeProperty,
        offset: RationalTime,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OperationEnvelope {
    pub operation_id: String,
    pub operation: CommandOperation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommandEnvelope {
    pub schema_version: u32,
    pub project_id: String,
    pub base_revision: u64,
    pub dry_run: bool,
    pub transaction_id: Option<String>,
    pub operations: Vec<OperationEnvelope>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OperationReceipt {
    pub operation_id: String,
    pub status: String, // "applied", "deduplicated_noop", "dry_run"
    pub affected_item_id: Option<String>,
    pub affected_track_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommandReceipt {
    pub project_id: String,
    pub prior_revision: u64,
    pub new_revision: u64,
    pub transaction_id: Option<String>,
    pub operations_applied: usize,
    pub receipts: Vec<OperationReceipt>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DiffChange {
    TrackCreated {
        track_id: String,
        name: String,
    },
    TrackDeleted {
        track_id: String,
    },
    TrackModified {
        track_id: String,
    },
    ClipCreated {
        item_id: String,
        track_id: String,
    },
    ClipMoved {
        item_id: String,
        new_start: RationalTime,
    },
    ClipTrimmed {
        item_id: String,
        duration: RationalTime,
    },
    ClipSplit {
        original_item_id: String,
        new_item_id: String,
        split_time: RationalTime,
    },
    ClipDeleted {
        item_id: String,
    },
    ClipDuplicated {
        original_item_id: String,
        new_item_id: String,
    },
    ClipModified {
        item_id: String,
    },
    AssetImported {
        asset_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiffReport {
    pub project_id: String,
    pub base_revision: u64,
    pub operations_evaluated: usize,
    pub changes: Vec<DiffChange>,
    pub warnings: Vec<String>,
}

/// The authoritative Command Service
pub struct CommandService {
    project: Project,
    applied_operation_ids: HashSet<String>,
}

impl CommandService {
    pub fn new(project: Project) -> Self {
        Self {
            project,
            applied_operation_ids: HashSet::new(),
        }
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    pub fn set_project(&mut self, project: Project) {
        self.project = project;
    }

    pub fn revision(&self) -> u64 {
        self.project.revision
    }

    /// Perform a dry-run / diff against the project without mutating authoritative state.
    pub fn dry_run(&self, envelope: &CommandEnvelope) -> Result<DiffReport, CommandError> {
        self.validate_envelope_header(envelope)?;

        // Clone project state to run simulation
        let mut sim_project = self.project.clone();
        let mut changes = Vec::new();
        let mut warnings = Vec::new();

        for op_env in &envelope.operations {
            if self.applied_operation_ids.contains(&op_env.operation_id) {
                warnings.push(format!(
                    "Operation '{}' was already applied; dry-run notes idempotent no-op",
                    op_env.operation_id
                ));
                continue;
            }

            let change = Self::apply_operation_to_project(
                &mut sim_project,
                &op_env.operation,
                &op_env.operation_id,
            )?;
            if let Some(ch) = change {
                changes.push(ch);
            }
        }

        // Validate final simulated state
        sim_project
            .validate()
            .map_err(|e| CommandError::OperationValidationFailed {
                operation_id: "final_simulation".to_string(),
                message: e.to_string(),
            })?;

        Ok(DiffReport {
            project_id: self.project.project_id.clone(),
            base_revision: self.project.revision,
            operations_evaluated: envelope.operations.len(),
            changes,
            warnings,
        })
    }

    /// Authoritatively execute a command envelope.
    /// Invariant: Atomic transactions — if any operation fails, whole transaction rolls back!
    /// Invariant: Stale revision is rejected.
    /// Invariant: Idempotent operation IDs do not duplicate edits.
    pub fn execute(&mut self, envelope: CommandEnvelope) -> Result<CommandReceipt, CommandError> {
        self.validate_envelope_header(&envelope)?;

        if envelope.dry_run {
            let diff = self.dry_run(&envelope)?;
            return Ok(CommandReceipt {
                project_id: self.project.project_id.clone(),
                prior_revision: self.project.revision,
                new_revision: self.project.revision,
                transaction_id: envelope.transaction_id,
                operations_applied: 0,
                receipts: diff
                    .changes
                    .into_iter()
                    .map(|c| OperationReceipt {
                        operation_id: "dry_run".to_string(),
                        status: "dry_run".to_string(),
                        affected_item_id: match &c {
                            DiffChange::ClipCreated { item_id, .. } => Some(item_id.clone()),
                            DiffChange::ClipMoved { item_id, .. } => Some(item_id.clone()),
                            DiffChange::ClipTrimmed { item_id, .. } => Some(item_id.clone()),
                            DiffChange::ClipSplit { new_item_id, .. } => Some(new_item_id.clone()),
                            DiffChange::ClipDeleted { item_id } => Some(item_id.clone()),
                            DiffChange::ClipDuplicated { new_item_id, .. } => {
                                Some(new_item_id.clone())
                            }
                            DiffChange::ClipModified { item_id } => Some(item_id.clone()),
                            _ => None,
                        },
                        affected_track_id: None,
                    })
                    .collect(),
            });
        }

        // Atomic transaction: simulate on clone first
        let mut staged_project = self.project.clone();
        let mut staged_receipts = Vec::new();
        let mut non_deduped_applied_count = 0;
        let mut new_operation_ids = Vec::new();

        for op_env in &envelope.operations {
            if self.applied_operation_ids.contains(&op_env.operation_id) {
                // Idempotent retry: do not apply mutation again
                staged_receipts.push(OperationReceipt {
                    operation_id: op_env.operation_id.clone(),
                    status: "deduplicated_noop".to_string(),
                    affected_item_id: None,
                    affected_track_id: None,
                });
                continue;
            }

            let _ = Self::apply_operation_to_project(
                &mut staged_project,
                &op_env.operation,
                &op_env.operation_id,
            )?;
            staged_project
                .validate()
                .map_err(|e| CommandError::OperationValidationFailed {
                    operation_id: op_env.operation_id.clone(),
                    message: e.to_string(),
                })?;

            staged_receipts.push(OperationReceipt {
                operation_id: op_env.operation_id.clone(),
                status: "applied".to_string(),
                affected_item_id: None,
                affected_track_id: None,
            });
            non_deduped_applied_count += 1;
            new_operation_ids.push(op_env.operation_id.clone());
        }

        // If all operations succeeded and validated: commit atomic state!
        let prior_revision = self.project.revision;
        let new_revision = if non_deduped_applied_count > 0 {
            staged_project.revision = prior_revision + 1;
            staged_project.updated_at = chrono::Utc::now().to_rfc3339();
            prior_revision + 1
        } else {
            prior_revision
        };

        self.project = staged_project;
        for op_id in new_operation_ids {
            self.applied_operation_ids.insert(op_id);
        }

        Ok(CommandReceipt {
            project_id: self.project.project_id.clone(),
            prior_revision,
            new_revision,
            transaction_id: envelope.transaction_id,
            operations_applied: non_deduped_applied_count,
            receipts: staged_receipts,
        })
    }

    fn validate_envelope_header(&self, envelope: &CommandEnvelope) -> Result<(), CommandError> {
        if envelope.schema_version != CURRENT_SCHEMA_VERSION {
            return Err(CommandError::SchemaVersionMismatch {
                expected: CURRENT_SCHEMA_VERSION,
                received: envelope.schema_version,
            });
        }
        if envelope.project_id != self.project.project_id {
            return Err(CommandError::ProjectIdMismatch {
                expected: self.project.project_id.clone(),
                received: envelope.project_id.clone(),
            });
        }
        // If ALL operations in this envelope have already been applied, this is an idempotent retry
        // of a prior request. It is allowed even if the project revision has advanced.
        let is_idempotent_replay = !envelope.operations.is_empty()
            && envelope
                .operations
                .iter()
                .all(|op| self.applied_operation_ids.contains(&op.operation_id));

        if !is_idempotent_replay && envelope.base_revision != self.project.revision {
            return Err(CommandError::StaleRevision {
                expected: self.project.revision,
                received: envelope.base_revision,
            });
        }
        Ok(())
    }

    fn apply_operation_to_project(
        project: &mut Project,
        operation: &CommandOperation,
        op_id: &str,
    ) -> Result<Option<DiffChange>, CommandError> {
        match operation {
            CommandOperation::TrackCreate {
                track_id,
                name,
                kind,
            } => {
                let id = track_id.clone().unwrap_or_else(generate_track_id);
                let mut track = Track::new(name.clone(), *kind);
                track.track_id = id.clone();
                project.tracks.push(track);
                Ok(Some(DiffChange::TrackCreated {
                    track_id: id,
                    name: name.clone(),
                }))
            }

            CommandOperation::TrackDelete { track_id } => {
                let idx = project
                    .tracks
                    .iter()
                    .position(|t| &t.track_id == track_id)
                    .ok_or_else(|| CommandError::OperationValidationFailed {
                        operation_id: op_id.to_string(),
                        message: format!("Track '{}' not found", track_id),
                    })?;
                project.tracks.remove(idx);
                Ok(Some(DiffChange::TrackDeleted {
                    track_id: track_id.clone(),
                }))
            }

            CommandOperation::ClipCreate {
                track_id,
                item_id,
                name,
                kind,
                asset_id,
                timeline_start,
                duration,
                source_start,
            } => {
                let track = project.find_track_mut(track_id).ok_or_else(|| {
                    CommandError::OperationValidationFailed {
                        operation_id: op_id.to_string(),
                        message: format!("Track '{}' not found", track_id),
                    }
                })?;

                let s_start = source_start.unwrap_or_else(|| RationalTime::zero(duration.rate));
                let t_range = TimeRange::new(*timeline_start, *duration);
                let s_range = TimeRange::new(s_start, *duration);

                let mut item =
                    TimelineItem::new(asset_id.clone(), name.clone(), *kind, t_range, s_range)?;
                if let Some(id) = item_id {
                    item.item_id = id.clone();
                }

                let final_id = item.item_id.clone();
                track.items.push(item);
                Ok(Some(DiffChange::ClipCreated {
                    item_id: final_id,
                    track_id: track_id.clone(),
                }))
            }

            CommandOperation::ClipMove {
                item_id,
                target_track_id,
                target_start,
            } => {
                // Remove item from source track
                let mut found_item = None;
                for track in &mut project.tracks {
                    if let Some(pos) = track.items.iter().position(|i| &i.item_id == item_id) {
                        found_item = Some((track.track_id.clone(), track.items.remove(pos)));
                        break;
                    }
                }

                let (src_track_id, mut item) =
                    found_item.ok_or_else(|| CommandError::OperationValidationFailed {
                        operation_id: op_id.to_string(),
                        message: format!("Item '{}' not found", item_id),
                    })?;

                // Update timeline range
                let new_range = TimeRange::new(*target_start, item.timeline_range.duration);
                item.timeline_range = new_range;

                let dest_track_id = target_track_id.as_ref().unwrap_or(&src_track_id);
                let dest_track = project.find_track_mut(dest_track_id).ok_or_else(|| {
                    CommandError::OperationValidationFailed {
                        operation_id: op_id.to_string(),
                        message: format!("Destination track '{}' not found", dest_track_id),
                    }
                })?;

                dest_track.items.push(item);
                Ok(Some(DiffChange::ClipMoved {
                    item_id: item_id.clone(),
                    new_start: *target_start,
                }))
            }

            CommandOperation::ClipTrim {
                item_id,
                new_timeline_range,
                new_source_range,
            } => {
                let mut found = false;
                for track in &mut project.tracks {
                    if let Some(item) = track.find_item_mut(item_id) {
                        item.timeline_range = *new_timeline_range;
                        item.source_range = *new_source_range;
                        item.validate()?;
                        found = true;
                        break;
                    }
                }

                if !found {
                    return Err(CommandError::OperationValidationFailed {
                        operation_id: op_id.to_string(),
                        message: format!("Item '{}' not found", item_id),
                    });
                }

                Ok(Some(DiffChange::ClipTrimmed {
                    item_id: item_id.clone(),
                    duration: new_timeline_range.duration,
                }))
            }

            CommandOperation::ClipSplit {
                item_id,
                split_time,
                new_right_item_id,
            } => {
                // Find clip
                let mut split_result = None;
                for track in &mut project.tracks {
                    if let Some(pos) = track.items.iter().position(|i| &i.item_id == item_id) {
                        let item = &track.items[pos];
                        if !item.timeline_range.contains_time(*split_time)
                            || *split_time == item.timeline_range.start
                        {
                            return Err(CommandError::OperationValidationFailed {
                                operation_id: op_id.to_string(),
                                message: format!(
                                    "Split time {:?} is outside clip range {:?}",
                                    split_time, item.timeline_range
                                ),
                            });
                        }

                        // Left duration: split_time - start
                        let left_dur = *split_time - item.timeline_range.start;
                        let right_dur = item.timeline_range.end() - *split_time;

                        let mut right_item = item.clone();
                        let right_id = new_right_item_id.clone().unwrap_or_else(generate_item_id);
                        right_item.item_id = right_id.clone();
                        right_item.timeline_range = TimeRange::new(*split_time, right_dur);
                        right_item.source_range =
                            TimeRange::new(item.source_range.start + left_dur, right_dur);

                        let left_item = &mut track.items[pos];
                        left_item.timeline_range =
                            TimeRange::new(left_item.timeline_range.start, left_dur);
                        left_item.source_range =
                            TimeRange::new(left_item.source_range.start, left_dur);

                        track.items.push(right_item);
                        split_result = Some(right_id);
                        break;
                    }
                }

                let right_id =
                    split_result.ok_or_else(|| CommandError::OperationValidationFailed {
                        operation_id: op_id.to_string(),
                        message: format!("Item '{}' not found", item_id),
                    })?;

                Ok(Some(DiffChange::ClipSplit {
                    original_item_id: item_id.clone(),
                    new_item_id: right_id,
                    split_time: *split_time,
                }))
            }

            CommandOperation::ClipDelete { item_id } => {
                let mut found = false;
                for track in &mut project.tracks {
                    if let Some(pos) = track.items.iter().position(|i| &i.item_id == item_id) {
                        track.items.remove(pos);
                        found = true;
                        break;
                    }
                }

                if !found {
                    return Err(CommandError::OperationValidationFailed {
                        operation_id: op_id.to_string(),
                        message: format!("Item '{}' not found", item_id),
                    });
                }

                Ok(Some(DiffChange::ClipDeleted {
                    item_id: item_id.clone(),
                }))
            }

            CommandOperation::ClipDuplicate {
                item_id,
                target_start,
                target_track_id,
                new_item_id,
            } => {
                let mut original_item = None;
                let mut orig_track_id = None;
                for track in &project.tracks {
                    if let Some(item) = track.find_item(item_id) {
                        original_item = Some(item.clone());
                        orig_track_id = Some(track.track_id.clone());
                        break;
                    }
                }

                let mut item =
                    original_item.ok_or_else(|| CommandError::OperationValidationFailed {
                        operation_id: op_id.to_string(),
                        message: format!("Item '{}' not found", item_id),
                    })?;

                let id = new_item_id.clone().unwrap_or_else(generate_item_id);
                item.item_id = id.clone();

                if let Some(start) = target_start {
                    item.timeline_range = TimeRange::new(*start, item.timeline_range.duration);
                } else {
                    // Place immediately after original
                    item.timeline_range =
                        TimeRange::new(item.timeline_range.end(), item.timeline_range.duration);
                }

                let dest_track_id = target_track_id.as_ref().or(orig_track_id.as_ref()).unwrap();
                let dest_track = project.find_track_mut(dest_track_id).ok_or_else(|| {
                    CommandError::OperationValidationFailed {
                        operation_id: op_id.to_string(),
                        message: format!("Track '{}' not found", dest_track_id),
                    }
                })?;

                dest_track.items.push(item);
                Ok(Some(DiffChange::ClipDuplicated {
                    original_item_id: item_id.clone(),
                    new_item_id: id,
                }))
            }

            CommandOperation::ClipTransformSet { item_id, transform } => {
                let mut found = false;
                for track in &mut project.tracks {
                    if let Some(item) = track.find_item_mut(item_id) {
                        item.transform = transform.clone();
                        found = true;
                        break;
                    }
                }
                if !found {
                    return Err(CommandError::OperationValidationFailed {
                        operation_id: op_id.to_string(),
                        message: format!("Item '{}' not found", item_id),
                    });
                }
                Ok(Some(DiffChange::ClipModified {
                    item_id: item_id.clone(),
                }))
            }

            CommandOperation::ClipCropSet { item_id, crop } => {
                let mut found = false;
                for track in &mut project.tracks {
                    if let Some(item) = track.find_item_mut(item_id) {
                        item.crop = crop.clone();
                        found = true;
                        break;
                    }
                }
                if !found {
                    return Err(CommandError::OperationValidationFailed {
                        operation_id: op_id.to_string(),
                        message: format!("Item '{}' not found", item_id),
                    });
                }
                Ok(Some(DiffChange::ClipModified {
                    item_id: item_id.clone(),
                }))
            }

            CommandOperation::ClipOpacitySet { item_id, opacity } => {
                let mut found = false;
                for track in &mut project.tracks {
                    if let Some(item) = track.find_item_mut(item_id) {
                        item.opacity = opacity.clamp(0.0, 1.0);
                        found = true;
                        break;
                    }
                }
                if !found {
                    return Err(CommandError::OperationValidationFailed {
                        operation_id: op_id.to_string(),
                        message: format!("Item '{}' not found", item_id),
                    });
                }
                Ok(Some(DiffChange::ClipModified {
                    item_id: item_id.clone(),
                }))
            }

            CommandOperation::TrackReorder {
                track_id,
                new_index,
            } => {
                let pos = project
                    .tracks
                    .iter()
                    .position(|t| &t.track_id == track_id)
                    .ok_or_else(|| CommandError::OperationValidationFailed {
                        operation_id: op_id.to_string(),
                        message: format!("Track '{}' not found", track_id),
                    })?;
                let track = project.tracks.remove(pos);
                let target_pos = (*new_index).min(project.tracks.len());
                project.tracks.insert(target_pos, track);
                Ok(Some(DiffChange::TrackModified {
                    track_id: track_id.clone(),
                }))
            }

            CommandOperation::TrackMuteSet { track_id, muted } => {
                let track = project.find_track_mut(track_id).ok_or_else(|| {
                    CommandError::OperationValidationFailed {
                        operation_id: op_id.to_string(),
                        message: format!("Track '{}' not found", track_id),
                    }
                })?;
                track.muted = *muted;
                Ok(Some(DiffChange::TrackModified {
                    track_id: track_id.clone(),
                }))
            }

            CommandOperation::ClipVolumeSet { item_id, volume } => {
                let mut found = false;
                for track in &mut project.tracks {
                    if let Some(item) = track.find_item_mut(item_id) {
                        item.volume = volume.clamp(0.0, 2.0);
                        found = true;
                        break;
                    }
                }
                if !found {
                    return Err(CommandError::OperationValidationFailed {
                        operation_id: op_id.to_string(),
                        message: format!("Item '{}' not found", item_id),
                    });
                }
                Ok(Some(DiffChange::ClipModified {
                    item_id: item_id.clone(),
                }))
            }

            CommandOperation::AssetImport {
                asset_id,
                name,
                path,
                media_kind,
                file_size,
            } => {
                let id = crate::modules::media::MediaCatalog::register_asset(
                    project,
                    asset_id.clone(),
                    name.clone(),
                    path.clone(),
                    *media_kind,
                    *file_size,
                );
                Ok(Some(DiffChange::AssetImported { asset_id: id }))
            }

            CommandOperation::ClipTextSet { item_id, text } => {
                let mut found = false;
                for track in &mut project.tracks {
                    if let Some(item) = track.find_item_mut(item_id) {
                        item.text_content = Some(text.clone());
                        found = true;
                        break;
                    }
                }
                if !found {
                    return Err(CommandError::OperationValidationFailed {
                        operation_id: op_id.to_string(),
                        message: format!("Item '{}' not found", item_id),
                    });
                }
                Ok(Some(DiffChange::ClipModified {
                    item_id: item_id.clone(),
                }))
            }

            CommandOperation::ClipAudioFadeSet {
                item_id,
                fade_in,
                fade_out,
            } => {
                let mut found = false;
                for track in &mut project.tracks {
                    if let Some(item) = track.find_item_mut(item_id) {
                        item.fade_in = *fade_in;
                        item.fade_out = *fade_out;
                        found = true;
                        break;
                    }
                }
                if !found {
                    return Err(CommandError::OperationValidationFailed {
                        operation_id: op_id.to_string(),
                        message: format!("Item '{}' not found", item_id),
                    });
                }
                Ok(Some(DiffChange::ClipModified {
                    item_id: item_id.clone(),
                }))
            }

            CommandOperation::ClipTransitionSet {
                item_id,
                transition,
            } => {
                let mut found = false;
                for track in &mut project.tracks {
                    if let Some(item) = track.find_item_mut(item_id) {
                        item.transition = transition.clone();
                        found = true;
                        break;
                    }
                }
                if !found {
                    return Err(CommandError::OperationValidationFailed {
                        operation_id: op_id.to_string(),
                        message: format!("Item '{}' not found", item_id),
                    });
                }
                Ok(Some(DiffChange::ClipModified {
                    item_id: item_id.clone(),
                }))
            }

            CommandOperation::ClipKeyframeAdd { item_id, keyframe } => {
                let mut found = false;
                for track in &mut project.tracks {
                    if let Some(item) = track.find_item_mut(item_id) {
                        // Remove any existing keyframe on this property at the same offset
                        item.keyframes.retain(|k| {
                            !(k.property == keyframe.property
                                && k.offset.value == keyframe.offset.value)
                        });
                        item.keyframes.push(keyframe.clone());
                        item.keyframes.sort_by_key(|k| k.offset.value);
                        found = true;
                        break;
                    }
                }
                if !found {
                    return Err(CommandError::OperationValidationFailed {
                        operation_id: op_id.to_string(),
                        message: format!("Item '{}' not found", item_id),
                    });
                }
                Ok(Some(DiffChange::ClipModified {
                    item_id: item_id.clone(),
                }))
            }

            CommandOperation::ClipKeyframeRemove {
                item_id,
                property,
                offset,
            } => {
                let mut found = false;
                for track in &mut project.tracks {
                    if let Some(item) = track.find_item_mut(item_id) {
                        item.keyframes.retain(|k| {
                            !(k.property == *property && k.offset.value == offset.value)
                        });
                        found = true;
                        break;
                    }
                }
                if !found {
                    return Err(CommandError::OperationValidationFailed {
                        operation_id: op_id.to_string(),
                        message: format!("Item '{}' not found", item_id),
                    });
                }
                Ok(Some(DiffChange::ClipModified {
                    item_id: item_id.clone(),
                }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::domain::time::FrameRate;

    fn create_test_project() -> Project {
        Project::new("POC 3 Test Project".to_string())
    }

    #[test]
    fn test_poc3_dry_run_writes_nothing() {
        let project = create_test_project();
        let service = CommandService::new(project.clone());

        let envelope = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision: 0,
            dry_run: true,
            transaction_id: Some("tx_dry_run".to_string()),
            operations: vec![
                OperationEnvelope {
                    operation_id: "op_trk_1".to_string(),
                    operation: CommandOperation::TrackCreate {
                        track_id: Some("trk_v1".to_string()),
                        name: "Video 1".to_string(),
                        kind: TrackKind::Video,
                    },
                },
                OperationEnvelope {
                    operation_id: "op_clip_1".to_string(),
                    operation: CommandOperation::ClipCreate {
                        track_id: "trk_v1".to_string(),
                        item_id: Some("clip_1".to_string()),
                        name: "Clip 1".to_string(),
                        kind: ItemKind::Video,
                        asset_id: None,
                        timeline_start: RationalTime::from_frames(0, FrameRate::FPS_30),
                        duration: RationalTime::from_frames(30, FrameRate::FPS_30),
                        source_start: None,
                    },
                },
            ],
        };

        let diff = service.dry_run(&envelope).expect("dry-run should succeed");
        assert_eq!(diff.operations_evaluated, 2);
        assert_eq!(diff.changes.len(), 2);

        // Invariant: Authoritative state is untouched!
        assert_eq!(service.revision(), 0);
        assert_eq!(service.project().tracks.len(), 0);
    }

    #[test]
    fn test_poc3_stale_revision_rejected() {
        let project = create_test_project();
        let mut service = CommandService::new(project.clone());

        // Step 1: Apply first command to advance revision from 0 to 1
        let env1 = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision: 0,
            dry_run: false,
            transaction_id: None,
            operations: vec![OperationEnvelope {
                operation_id: "op_step1".to_string(),
                operation: CommandOperation::TrackCreate {
                    track_id: Some("trk_1".to_string()),
                    name: "Track 1".to_string(),
                    kind: TrackKind::Video,
                },
            }],
        };
        let receipt1 = service.execute(env1).expect("first command succeeds");
        assert_eq!(receipt1.new_revision, 1);
        assert_eq!(service.revision(), 1);

        // Step 2: Attempt to apply with stale baseRevision = 0
        let stale_env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision: 0, // STALE! Expected 1
            dry_run: false,
            transaction_id: None,
            operations: vec![OperationEnvelope {
                operation_id: "op_stale".to_string(),
                operation: CommandOperation::TrackCreate {
                    track_id: Some("trk_2".to_string()),
                    name: "Track 2".to_string(),
                    kind: TrackKind::Video,
                },
            }],
        };

        let err = service
            .execute(stale_env)
            .expect_err("stale revision must fail");
        assert!(matches!(
            err,
            CommandError::StaleRevision {
                expected: 1,
                received: 0
            }
        ));
        assert_eq!(service.revision(), 1);
        assert_eq!(service.project().tracks.len(), 1);
    }

    #[test]
    fn test_poc3_duplicate_operation_id_idempotency() {
        let project = create_test_project();
        let mut service = CommandService::new(project.clone());

        let op = OperationEnvelope {
            operation_id: "op_idempotent_test".to_string(),
            operation: CommandOperation::TrackCreate {
                track_id: Some("trk_uniq".to_string()),
                name: "Unique Track".to_string(),
                kind: TrackKind::Video,
            },
        };

        // First apply
        let env1 = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision: 0,
            dry_run: false,
            transaction_id: None,
            operations: vec![op.clone()],
        };
        let receipt1 = service.execute(env1).unwrap();
        assert_eq!(receipt1.operations_applied, 1);
        assert_eq!(receipt1.receipts[0].status, "applied");
        assert_eq!(service.revision(), 1);
        assert_eq!(service.project().tracks.len(), 1);

        // Retry same operation ID with updated base_revision = 1
        let env_retry = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision: 1,
            dry_run: false,
            transaction_id: None,
            operations: vec![op],
        };
        let receipt2 = service.execute(env_retry).unwrap();
        assert_eq!(receipt2.operations_applied, 0); // Not applied again!
        assert_eq!(receipt2.receipts[0].status, "deduplicated_noop");
        assert_eq!(receipt2.new_revision, 1); // Revision does NOT increment on no-op
        assert_eq!(service.project().tracks.len(), 1); // Track count remains 1
    }

    #[test]
    fn test_poc3_atomic_transaction_rollback() {
        let project = create_test_project();
        let mut service = CommandService::new(project.clone());

        // Multi-operation transaction: op1 and op2 are valid, op3 fails
        let env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: project.project_id.clone(),
            base_revision: 0,
            dry_run: false,
            transaction_id: Some("tx_atomic_test".to_string()),
            operations: vec![
                OperationEnvelope {
                    operation_id: "op_valid_1".to_string(),
                    operation: CommandOperation::TrackCreate {
                        track_id: Some("trk_tx".to_string()),
                        name: "Tx Track".to_string(),
                        kind: TrackKind::Video,
                    },
                },
                OperationEnvelope {
                    operation_id: "op_valid_2".to_string(),
                    operation: CommandOperation::ClipCreate {
                        track_id: "trk_tx".to_string(),
                        item_id: Some("clip_tx".to_string()),
                        name: "Tx Clip".to_string(),
                        kind: ItemKind::Video,
                        asset_id: None,
                        timeline_start: RationalTime::from_frames(0, FrameRate::FPS_30),
                        duration: RationalTime::from_frames(30, FrameRate::FPS_30),
                        source_start: None,
                    },
                },
                OperationEnvelope {
                    operation_id: "op_invalid_3".to_string(),
                    operation: CommandOperation::ClipSplit {
                        item_id: "non_existent_clip".to_string(), // MUST FAIL!
                        split_time: RationalTime::from_frames(15, FrameRate::FPS_30),
                        new_right_item_id: None,
                    },
                },
            ],
        };

        let result = service.execute(env);
        assert!(result.is_err());

        // Invariant: Total rollback! No tracks, no clips created!
        assert_eq!(service.revision(), 0);
        assert_eq!(service.project().tracks.len(), 0);
    }

    #[test]
    fn test_poc3_ui_and_automation_caller_equivalence() {
        let rate = FrameRate::FPS_30;

        // Caller 1 (Simulated UI): Creates track, creates clip, moves clip, trims clip, splits clip
        let mut ui_service = CommandService::new(create_test_project());
        let pid = ui_service.project().project_id.clone();

        ui_service
            .execute(CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: pid.clone(),
                base_revision: 0,
                dry_run: false,
                transaction_id: None,
                operations: vec![
                    OperationEnvelope {
                        operation_id: "ui_op_1".to_string(),
                        operation: CommandOperation::TrackCreate {
                            track_id: Some("trk_main".to_string()),
                            name: "Main Video".to_string(),
                            kind: TrackKind::Video,
                        },
                    },
                    OperationEnvelope {
                        operation_id: "ui_op_2".to_string(),
                        operation: CommandOperation::ClipCreate {
                            track_id: "trk_main".to_string(),
                            item_id: Some("clip_orig".to_string()),
                            name: "Original Clip".to_string(),
                            kind: ItemKind::Video,
                            asset_id: None,
                            timeline_start: RationalTime::from_frames(0, rate),
                            duration: RationalTime::from_frames(90, rate),
                            source_start: None,
                        },
                    },
                ],
            })
            .unwrap();

        // Move to start at frame 10
        ui_service
            .execute(CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: pid.clone(),
                base_revision: 1,
                dry_run: false,
                transaction_id: None,
                operations: vec![OperationEnvelope {
                    operation_id: "ui_op_3".to_string(),
                    operation: CommandOperation::ClipMove {
                        item_id: "clip_orig".to_string(),
                        target_track_id: None,
                        target_start: RationalTime::from_frames(10, rate),
                    },
                }],
            })
            .unwrap();

        // Split clip at frame 40 (between 10 and 100)
        ui_service
            .execute(CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: pid.clone(),
                base_revision: 2,
                dry_run: false,
                transaction_id: None,
                operations: vec![OperationEnvelope {
                    operation_id: "ui_op_4".to_string(),
                    operation: CommandOperation::ClipSplit {
                        item_id: "clip_orig".to_string(),
                        split_time: RationalTime::from_frames(40, rate),
                        new_right_item_id: Some("clip_split_right".to_string()),
                    },
                }],
            })
            .unwrap();

        // Caller 2 (Simulated Automation Agent):
        // Applies the exact same logical editorial sequence as a single batch transaction
        let mut auto_service = CommandService::new({
            let mut p = create_test_project();
            p.project_id = pid.clone();
            p
        });

        auto_service
            .execute(CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: pid.clone(),
                base_revision: 0,
                dry_run: false,
                transaction_id: Some("tx_auto_batch".to_string()),
                operations: vec![
                    OperationEnvelope {
                        operation_id: "auto_op_1".to_string(),
                        operation: CommandOperation::TrackCreate {
                            track_id: Some("trk_main".to_string()),
                            name: "Main Video".to_string(),
                            kind: TrackKind::Video,
                        },
                    },
                    OperationEnvelope {
                        operation_id: "auto_op_2".to_string(),
                        operation: CommandOperation::ClipCreate {
                            track_id: "trk_main".to_string(),
                            item_id: Some("clip_orig".to_string()),
                            name: "Original Clip".to_string(),
                            kind: ItemKind::Video,
                            asset_id: None,
                            timeline_start: RationalTime::from_frames(0, rate),
                            duration: RationalTime::from_frames(90, rate),
                            source_start: None,
                        },
                    },
                    OperationEnvelope {
                        operation_id: "auto_op_3".to_string(),
                        operation: CommandOperation::ClipMove {
                            item_id: "clip_orig".to_string(),
                            target_track_id: None,
                            target_start: RationalTime::from_frames(10, rate),
                        },
                    },
                    OperationEnvelope {
                        operation_id: "auto_op_4".to_string(),
                        operation: CommandOperation::ClipSplit {
                            item_id: "clip_orig".to_string(),
                            split_time: RationalTime::from_frames(40, rate),
                            new_right_item_id: Some("clip_split_right".to_string()),
                        },
                    },
                ],
            })
            .unwrap();

        // Verify equivalence of canonical resulting state:
        let ui_proj = ui_service.project();
        let auto_proj = auto_service.project();

        assert_eq!(ui_proj.tracks.len(), auto_proj.tracks.len());
        assert_eq!(
            ui_proj.tracks[0].items.len(),
            auto_proj.tracks[0].items.len()
        );

        let ui_track = &ui_proj.tracks[0];
        let auto_track = &auto_proj.tracks[0];

        let ui_left = ui_track.find_item("clip_orig").unwrap();
        let auto_left = auto_track.find_item("clip_orig").unwrap();
        assert_eq!(ui_left.timeline_range, auto_left.timeline_range);
        assert_eq!(ui_left.source_range, auto_left.source_range);

        let ui_right = ui_track.find_item("clip_split_right").unwrap();
        let auto_right = auto_track.find_item("clip_split_right").unwrap();
        assert_eq!(ui_right.timeline_range, auto_right.timeline_range);
        assert_eq!(ui_right.source_range, auto_right.source_range);

        // Verification of split arithmetic:
        // Left: start 10, dur 30 -> ends at 40
        assert_eq!(ui_left.timeline_range.start.value, 10);
        assert_eq!(ui_left.timeline_range.duration.value, 30);
        // Right: start 40, dur 60 -> ends at 100
        assert_eq!(ui_right.timeline_range.start.value, 40);
        assert_eq!(ui_right.timeline_range.duration.value, 60);
        // Total duration preserved: 30 + 60 = 90 frames!
        assert_eq!(
            ui_left.timeline_range.duration + ui_right.timeline_range.duration,
            RationalTime::from_frames(90, rate)
        );
    }

    #[test]
    fn test_phase5_core_timeline_editing_vertical_slice() {
        let rate = FrameRate::FPS_30;
        let mut service = CommandService::new(create_test_project());
        let pid = service.project().project_id.clone();

        // 1. Asset import
        service
            .execute(CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: pid.clone(),
                base_revision: 0,
                dry_run: false,
                transaction_id: Some("tx_import".to_string()),
                operations: vec![OperationEnvelope {
                    operation_id: "op_import_1".to_string(),
                    operation: CommandOperation::AssetImport {
                        asset_id: Some("ast_main_video".to_string()),
                        name: "camera_footage.mp4".to_string(),
                        path: "D:/media/camera_footage.mp4".to_string(),
                        media_kind: crate::modules::domain::MediaKind::Video,
                        file_size: 1024 * 1024 * 10,
                    },
                }],
            })
            .unwrap();

        // 2. Add video and audio tracks
        service
            .execute(CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: pid.clone(),
                base_revision: 1,
                dry_run: false,
                transaction_id: Some("tx_tracks".to_string()),
                operations: vec![
                    OperationEnvelope {
                        operation_id: "op_trk_v".to_string(),
                        operation: CommandOperation::TrackCreate {
                            track_id: Some("trk_v1".to_string()),
                            name: "Video 1".to_string(),
                            kind: TrackKind::Video,
                        },
                    },
                    OperationEnvelope {
                        operation_id: "op_trk_a".to_string(),
                        operation: CommandOperation::TrackCreate {
                            track_id: Some("trk_a1".to_string()),
                            name: "Audio 1".to_string(),
                            kind: TrackKind::Audio,
                        },
                    },
                ],
            })
            .unwrap();

        // 3. Add clips to tracks
        service
            .execute(CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: pid.clone(),
                base_revision: 2,
                dry_run: false,
                transaction_id: Some("tx_clips".to_string()),
                operations: vec![
                    OperationEnvelope {
                        operation_id: "op_c1".to_string(),
                        operation: CommandOperation::ClipCreate {
                            track_id: "trk_v1".to_string(),
                            item_id: Some("clip_vid_1".to_string()),
                            name: "Shot 1".to_string(),
                            kind: ItemKind::Video,
                            asset_id: Some("ast_main_video".to_string()),
                            timeline_start: RationalTime::from_frames(0, rate),
                            duration: RationalTime::from_frames(90, rate),
                            source_start: Some(RationalTime::zero(rate)),
                        },
                    },
                    OperationEnvelope {
                        operation_id: "op_c2".to_string(),
                        operation: CommandOperation::ClipCreate {
                            track_id: "trk_a1".to_string(),
                            item_id: Some("clip_aud_1".to_string()),
                            name: "Music 1".to_string(),
                            kind: ItemKind::Audio,
                            asset_id: None,
                            timeline_start: RationalTime::from_frames(0, rate),
                            duration: RationalTime::from_frames(180, rate),
                            source_start: Some(RationalTime::zero(rate)),
                        },
                    },
                ],
            })
            .unwrap();

        // 4. Split video clip at frame 30
        service
            .execute(CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: pid.clone(),
                base_revision: 3,
                dry_run: false,
                transaction_id: Some("tx_split".to_string()),
                operations: vec![OperationEnvelope {
                    operation_id: "op_split".to_string(),
                    operation: CommandOperation::ClipSplit {
                        item_id: "clip_vid_1".to_string(),
                        split_time: RationalTime::from_frames(30, rate),
                        new_right_item_id: Some("clip_vid_1_part2".to_string()),
                    },
                }],
            })
            .unwrap();

        // 5. Transform, Crop, Opacity, and Volume adjustments
        service
            .execute(CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: pid.clone(),
                base_revision: 4,
                dry_run: false,
                transaction_id: Some("tx_transforms".to_string()),
                operations: vec![
                    OperationEnvelope {
                        operation_id: "op_trans".to_string(),
                        operation: CommandOperation::ClipTransformSet {
                            item_id: "clip_vid_1".to_string(),
                            transform: Transform {
                                position_x: 10.0,
                                position_y: -20.0,
                                scale_x: 1.2,
                                scale_y: 1.2,
                                rotation: 5.0,
                            },
                        },
                    },
                    OperationEnvelope {
                        operation_id: "op_opac".to_string(),
                        operation: CommandOperation::ClipOpacitySet {
                            item_id: "clip_vid_1".to_string(),
                            opacity: 0.85,
                        },
                    },
                    OperationEnvelope {
                        operation_id: "op_vol".to_string(),
                        operation: CommandOperation::ClipVolumeSet {
                            item_id: "clip_aud_1".to_string(),
                            volume: 0.75,
                        },
                    },
                    OperationEnvelope {
                        operation_id: "op_mute".to_string(),
                        operation: CommandOperation::TrackMuteSet {
                            track_id: "trk_a1".to_string(),
                            muted: true,
                        },
                    },
                ],
            })
            .unwrap();

        // Verify committed state
        let proj = service.project();
        assert_eq!(proj.revision, 5);
        assert_eq!(proj.assets.len(), 1);
        assert_eq!(proj.tracks.len(), 2);
        assert_eq!(proj.tracks[0].items.len(), 2); // Split created 2 items
        assert_eq!(proj.tracks[1].items.len(), 1);

        let v1_part1 = proj.tracks[0].find_item("clip_vid_1").unwrap();
        assert_eq!(v1_part1.timeline_range.duration.value, 30);
        assert_eq!(v1_part1.transform.position_x, 10.0);
        assert_eq!(v1_part1.opacity, 0.85);

        let a1 = proj.tracks[1].find_item("clip_aud_1").unwrap();
        assert_eq!(a1.volume, 0.75);
        assert!(proj.tracks[1].muted);

        // Atomic persistence round-trip test for this real edited project:
        let temp_file = std::env::temp_dir().join("tinix_phase5_roundtrip.tcproj");
        crate::modules::persistence::save_project_atomic(proj, &temp_file).unwrap();
        let loaded = crate::modules::persistence::load_project(&temp_file).unwrap();
        assert_eq!(loaded, *proj);

        let _ = std::fs::remove_file(temp_file);
    }

    #[test]
    fn test_phase9_bounded_creator_feature_expansion() {
        let rate = FrameRate::FPS_30;
        let mut service = CommandService::new(create_test_project());
        let pid = service.project().project_id.clone();

        // 1. Create a track and items (video clip, audio clip, text clip)
        service
            .execute(CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: pid.clone(),
                base_revision: 0,
                dry_run: false,
                transaction_id: Some("tx_init".to_string()),
                operations: vec![
                    OperationEnvelope {
                        operation_id: "op_trkV".to_string(),
                        operation: CommandOperation::TrackCreate {
                            track_id: Some("trk_v".to_string()),
                            name: "Main Track".to_string(),
                            kind: TrackKind::Video,
                        },
                    },
                    OperationEnvelope {
                        operation_id: "op_trkTxt".to_string(),
                        operation: CommandOperation::TrackCreate {
                            track_id: Some("trk_txt".to_string()),
                            name: "Captions Track".to_string(),
                            kind: TrackKind::Video,
                        },
                    },
                    OperationEnvelope {
                        operation_id: "op_clipV".to_string(),
                        operation: CommandOperation::ClipCreate {
                            track_id: "trk_v".to_string(),
                            item_id: Some("clip_vid".to_string()),
                            name: "Video 1".to_string(),
                            kind: ItemKind::Video,
                            asset_id: None,
                            timeline_start: RationalTime::from_frames(0, rate),
                            duration: RationalTime::from_frames(90, rate),
                            source_start: Some(RationalTime::zero(rate)),
                        },
                    },
                    OperationEnvelope {
                        operation_id: "op_clipTxt".to_string(),
                        operation: CommandOperation::ClipCreate {
                            track_id: "trk_txt".to_string(),
                            item_id: Some("clip_caption".to_string()),
                            name: "Title".to_string(),
                            kind: ItemKind::Text,
                            asset_id: None,
                            timeline_start: RationalTime::from_frames(15, rate),
                            duration: RationalTime::from_frames(60, rate),
                            source_start: Some(RationalTime::zero(rate)),
                        },
                    },
                ],
            })
            .unwrap();

        // 2. Test ClipTextSet
        service
            .execute(CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: pid.clone(),
                base_revision: 1,
                dry_run: false,
                transaction_id: Some("tx_text".to_string()),
                operations: vec![OperationEnvelope {
                    operation_id: "op_set_text".to_string(),
                    operation: CommandOperation::ClipTextSet {
                        item_id: "clip_caption".to_string(),
                        text: "Welcome to TinixCut!".to_string(),
                    },
                }],
            })
            .unwrap();

        let proj = service.project();
        let cap_item = proj.tracks[1].find_item("clip_caption").unwrap();
        assert_eq!(
            cap_item.text_content.as_deref(),
            Some("Welcome to TinixCut!")
        );

        // 3. Test ClipAudioFadeSet
        service
            .execute(CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: pid.clone(),
                base_revision: 2,
                dry_run: false,
                transaction_id: Some("tx_fade".to_string()),
                operations: vec![OperationEnvelope {
                    operation_id: "op_set_fade".to_string(),
                    operation: CommandOperation::ClipAudioFadeSet {
                        item_id: "clip_vid".to_string(),
                        fade_in: Some(RationalTime::from_frames(15, rate)),
                        fade_out: Some(RationalTime::from_frames(30, rate)),
                    },
                }],
            })
            .unwrap();

        let proj = service.project();
        let vid_item = proj.tracks[0].find_item("clip_vid").unwrap();
        assert_eq!(vid_item.fade_in, Some(RationalTime::from_frames(15, rate)));
        assert_eq!(vid_item.fade_out, Some(RationalTime::from_frames(30, rate)));

        // 4. Test ClipTransitionSet
        service
            .execute(CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: pid.clone(),
                base_revision: 3,
                dry_run: false,
                transaction_id: Some("tx_trans".to_string()),
                operations: vec![OperationEnvelope {
                    operation_id: "op_set_trans".to_string(),
                    operation: CommandOperation::ClipTransitionSet {
                        item_id: "clip_vid".to_string(),
                        transition: Some(crate::modules::domain::Transition {
                            kind: crate::modules::domain::TransitionKind::CrossDissolve,
                            duration: RationalTime::from_frames(10, rate),
                        }),
                    },
                }],
            })
            .unwrap();

        let proj = service.project();
        let vid_item = proj.tracks[0].find_item("clip_vid").unwrap();
        assert!(vid_item.transition.is_some());
        assert_eq!(vid_item.transition.as_ref().unwrap().duration.value, 10);

        // 5. Test Keyframes: Add 3 keyframes, then remove 1
        service
            .execute(CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: pid.clone(),
                base_revision: 4,
                dry_run: false,
                transaction_id: Some("tx_kf_add".to_string()),
                operations: vec![
                    OperationEnvelope {
                        operation_id: "op_kf1".to_string(),
                        operation: CommandOperation::ClipKeyframeAdd {
                            item_id: "clip_vid".to_string(),
                            keyframe: Keyframe {
                                property: KeyframeProperty::Opacity,
                                offset: RationalTime::from_frames(0, rate),
                                value: 0.2,
                            },
                        },
                    },
                    OperationEnvelope {
                        operation_id: "op_kf2".to_string(),
                        operation: CommandOperation::ClipKeyframeAdd {
                            item_id: "clip_vid".to_string(),
                            keyframe: Keyframe {
                                property: KeyframeProperty::Opacity,
                                offset: RationalTime::from_frames(30, rate),
                                value: 1.0,
                            },
                        },
                    },
                    OperationEnvelope {
                        operation_id: "op_kf3".to_string(),
                        operation: CommandOperation::ClipKeyframeAdd {
                            item_id: "clip_vid".to_string(),
                            keyframe: Keyframe {
                                property: KeyframeProperty::Opacity,
                                offset: RationalTime::from_frames(15, rate),
                                value: 0.99,
                            },
                        },
                    },
                ],
            })
            .unwrap();

        let proj = service.project();
        let vid_item = proj.tracks[0].find_item("clip_vid").unwrap();
        assert_eq!(vid_item.keyframes.len(), 3);
        // Keyframes should be sorted by offset: 0, 15, 30
        assert_eq!(vid_item.keyframes[0].offset.value, 0);
        assert_eq!(vid_item.keyframes[1].offset.value, 15);
        assert_eq!(vid_item.keyframes[2].offset.value, 30);

        // Remove intermediate keyframe at offset 15
        service
            .execute(CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: pid.clone(),
                base_revision: 5,
                dry_run: false,
                transaction_id: Some("tx_kf_rem".to_string()),
                operations: vec![OperationEnvelope {
                    operation_id: "op_kf_del".to_string(),
                    operation: CommandOperation::ClipKeyframeRemove {
                        item_id: "clip_vid".to_string(),
                        property: KeyframeProperty::Opacity,
                        offset: RationalTime::from_frames(15, rate),
                    },
                }],
            })
            .unwrap();

        let proj = service.project();
        let vid_item = proj.tracks[0].find_item("clip_vid").unwrap();
        assert_eq!(vid_item.keyframes.len(), 2);
        assert_eq!(vid_item.keyframes[0].value, 0.2);
        assert_eq!(vid_item.keyframes[1].value, 1.0);

        // 6. Test Persistence Round-trip preserving all creator features
        let temp_file = std::env::temp_dir().join("tinix_phase9_creator_roundtrip.tcproj");
        crate::modules::persistence::save_project_atomic(proj, &temp_file).unwrap();
        let loaded = crate::modules::persistence::load_project(&temp_file).unwrap();
        assert_eq!(loaded, *proj);
        let _ = std::fs::remove_file(temp_file);

        // 7. Verify CompositionPlan compilation & keyframe sampling parity
        let plan = crate::modules::composition::CompositionPlan::from_project(proj)
            .expect("Plan must compile");

        // Sampling at t = 15 frames:
        // Keyframe interpolation between offset 0 (0.2) and offset 30 (1.0) -> midpoint is 0.6!
        let sample = plan.sample_preview_at(RationalTime::from_frames(15, rate));
        let vid_layer = sample
            .active_layers
            .iter()
            .find(|l| l.item_id == "clip_vid")
            .unwrap();
        assert!((vid_layer.opacity - 0.6).abs() < 0.001);

        // Title text should also be present in active layers
        let title_layer = sample
            .active_layers
            .iter()
            .find(|l| l.item_id == "clip_caption")
            .unwrap();
        assert_eq!(title_layer.text.as_deref(), Some("Welcome to TinixCut!"));
    }
}
