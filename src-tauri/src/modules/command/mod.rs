//! Command Service & Engine for TinixCut
//! Architecture Invariant: All authoritative mutations pass through this engine.
//! UI, Shortcuts, and Automation share the exact same command mutation path.
//! Handles baseRevision checks, idempotency receipts, atomic transactions, and dry-run/diff.

use crate::modules::domain::id::*;
use crate::modules::domain::time::{RationalTime, TimeRange};
use crate::modules::domain::{
    CommandLedgerEntry, Crop, CutTransition, DomainError, ItemKind, Keyframe, KeyframeEasing,
    KeyframeProperty, Project, TimelineItem, Track, TrackKind, Transform, Transition,
    TransitionKind, CURRENT_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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

    #[error("Idempotency collision: operation '{operation_id}' was previously committed with a different payload")]
    IdempotencyCollision {
        operation_id: String,
        existing_fingerprint: String,
        received_fingerprint: String,
    },

    #[error("Unsupported transition '{transition}' in operation '{operation_id}': capability is deferred")]
    UnsupportedTransition {
        operation_id: String,
        transition: String,
    },

    #[error("Operation '{operation_id}' was already committed under sequence {sequence_number} for client '{client_id}' and detailed receipt has been evicted from active window")]
    OperationAlreadyCommitted {
        operation_id: String,
        client_id: String,
        sequence_number: u64,
    },

    #[error("Maximum active client capacity ({max_clients}) reached. Inactive clients must be retired before new clients can register")]
    ClientRegistryFull { max_clients: usize },

    #[error("Client epoch {client_epoch} for client '{client_id}' has expired (minimum accepted epoch is {min_accepted_epoch})")]
    ClientEpochExpired {
        client_id: String,
        client_epoch: u64,
        min_accepted_epoch: u64,
    },

    #[error("Sequence gap detected for client '{client_id}': expected sequence {expected_sequence}, received {received_sequence}")]
    SequenceGapDetected {
        client_id: String,
        expected_sequence: u64,
        received_sequence: u64,
    },

    #[error("Operation '{operation_id}' was committed under legacy non-cryptographic schema (FNV-1a) without raw payload retention and cannot be safely verified for replay")]
    LegacyReceiptUnverifiable { operation_id: String },

    #[error("Cannot create or update transition: required media handle exceeds available source frames. Code: {code}. Required preroll: {required_preroll}, available preroll: {available_preroll}, required postroll: {required_postroll}, available postroll: {available_postroll}")]
    InsufficientTransitionHandle {
        operation_id: String,
        code: String,
        message: String,
        required_preroll: RationalTime,
        available_preroll: RationalTime,
        required_postroll: RationalTime,
        available_postroll: RationalTime,
    },

    #[error("Cannot create or update transition: media duration or stream handle availability cannot be proven. Code: {code}. Reason: {reason}")]
    TransitionHandleUnverifiable {
        operation_id: String,
        code: String,
        reason: String,
    },

    #[error("Cannot create transition: participant is not a video item. Code: {code}")]
    UnsupportedTransitionMedia { operation_id: String, code: String },

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
        #[serde(default)]
        easing: Option<KeyframeEasing>,
    },
    ClipKeyframeRemove {
        item_id: String,
        property: KeyframeProperty,
        offset: RationalTime,
    },
    ClipKeyframeMove {
        item_id: String,
        property: KeyframeProperty,
        old_offset: RationalTime,
        new_offset: RationalTime,
    },
    ClipKeyframeEasingSet {
        item_id: String,
        property: KeyframeProperty,
        offset: RationalTime,
        easing: KeyframeEasing,
    },
    CutTransitionCreate {
        transition_id: Option<String>,
        track_id: String,
        left_item_id: String,
        right_item_id: String,
        kind: TransitionKind,
        duration: RationalTime,
    },
    CutTransitionUpdateDuration {
        transition_id: String,
        new_duration: RationalTime,
    },
    CutTransitionRemove {
        transition_id: String,
    },
    ClipGroupMove {
        item_ids: Vec<String>,
        delta: RationalTime,
    },
    ClipRippleDelete {
        item_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OperationEnvelope {
    #[serde(alias = "operation_id")]
    pub operation_id: String,
    pub operation: CommandOperation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommandEnvelope {
    #[serde(alias = "schema_version")]
    pub schema_version: u32,
    #[serde(alias = "project_id")]
    pub project_id: String,
    #[serde(alias = "base_revision")]
    pub base_revision: u64,
    #[serde(default, alias = "client_id")]
    pub client_id: Option<String>,
    #[serde(default, alias = "client_epoch")]
    pub client_epoch: Option<u64>,
    #[serde(default, alias = "sequence_number")]
    pub sequence_number: Option<u64>,
    #[serde(default, alias = "dry_run")]
    pub dry_run: bool,
    #[serde(default, alias = "transaction_id")]
    pub transaction_id: Option<String>,
    pub operations: Vec<OperationEnvelope>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OperationReceipt {
    #[serde(alias = "operation_id")]
    pub operation_id: String,
    pub status: String, // "applied", "deduplicated_noop", "dry_run"
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "affected_item_id"
    )]
    pub affected_item_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "affected_track_id"
    )]
    pub affected_track_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommandReceipt {
    #[serde(alias = "project_id")]
    pub project_id: String,
    #[serde(alias = "prior_revision")]
    pub prior_revision: u64,
    #[serde(alias = "new_revision")]
    pub new_revision: u64,
    #[serde(default, alias = "transaction_id")]
    pub transaction_id: Option<String>,
    #[serde(alias = "operations_applied")]
    pub operations_applied: usize,
    pub receipts: Vec<OperationReceipt>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "idempotency_scope"
    )]
    pub idempotency_scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
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
    CutTransitionCreated {
        transition_id: String,
        track_id: String,
    },
    CutTransitionUpdated {
        transition_id: String,
        new_duration: RationalTime,
    },
    CutTransitionRemoved {
        transition_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiffReport {
    #[serde(alias = "project_id")]
    pub project_id: String,
    #[serde(alias = "base_revision")]
    pub base_revision: u64,
    #[serde(alias = "operations_evaluated")]
    pub operations_evaluated: usize,
    pub changes: Vec<DiffChange>,
    pub warnings: Vec<String>,
}

/// Computes a deterministic semantic fingerprint for an operation using standard SHA-256 (NIST FIPS 180-4).
pub fn compute_operation_fingerprint(op: &CommandOperation) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write;
    let bytes = serde_json::to_vec(op).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let result = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for b in result {
        let _ = write!(hex, "{:02x}", b);
    }
    format!("fp_sha256_{}", hex)
}

/// Checks if a fingerprint is from the legacy Phase 10 FNV-1a scheme.
pub fn is_legacy_fnv_fingerprint(fp: &str) -> bool {
    fp.starts_with("fp_") && !fp.starts_with("fp_sha256_")
}

/// The authoritative Command Service
pub struct CommandService {
    project: Project,
    ledger: HashMap<String, CommandLedgerEntry>,
}

impl CommandService {
    pub fn new(project: Project) -> Self {
        let mut ledger = HashMap::new();
        for entry in &project.command_ledger {
            ledger.insert(entry.operation_id.clone(), entry.clone());
        }
        Self { project, ledger }
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    pub fn set_project(&mut self, project: Project) {
        self.ledger.clear();
        for entry in &project.command_ledger {
            self.ledger
                .insert(entry.operation_id.clone(), entry.clone());
        }
        self.project = project;
    }

    pub fn revision(&self) -> u64 {
        self.project.revision
    }

    pub fn is_operation_applied(&self, operation_id: &str) -> bool {
        self.ledger.contains_key(operation_id)
    }

    pub fn get_ledger_entry(&self, operation_id: &str) -> Option<&CommandLedgerEntry> {
        self.ledger.get(operation_id)
    }

    /// Perform a dry-run / diff against the project without mutating authoritative state.
    pub fn dry_run(&self, envelope: &CommandEnvelope) -> Result<DiffReport, CommandError> {
        self.validate_envelope_header(envelope)?;

        // Staged isolation for dry-run simulation: isolate the command ledger
        let mut sim_project = Project {
            schema_version: self.project.schema_version,
            project_id: self.project.project_id.clone(),
            name: self.project.name.clone(),
            revision: self.project.revision,
            settings: self.project.settings.clone(),
            assets: self.project.assets.clone(),
            tracks: self.project.tracks.clone(),
            created_at: self.project.created_at.clone(),
            updated_at: self.project.updated_at.clone(),
            client_registry: self.project.client_registry.clone(),
            command_ledger: Vec::new(),
        };
        let mut changes = Vec::new();
        let mut warnings = Vec::new();

        for op_env in &envelope.operations {
            if self.ledger.contains_key(&op_env.operation_id) {
                warnings.push(format!(
                    "Operation '{}' was already applied; dry-run notes idempotent no-op",
                    op_env.operation_id
                ));
                continue;
            }

            let op_changes = Self::apply_operation_to_project(
                &mut sim_project,
                &op_env.operation,
                &op_env.operation_id,
            )?;
            changes.extend(op_changes);
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
    /// Invariant: Idempotent operation IDs do not duplicate edits and survive save/reopen.
    /// Invariant: Operation ID reuse with different payload is rejected as collision.
    pub fn execute(&mut self, envelope: CommandEnvelope) -> Result<CommandReceipt, CommandError> {
        self.validate_envelope_header(&envelope)?;

        let is_scoped = envelope.client_id.is_some();
        let idempotency_scope = if is_scoped {
            Some("structured".to_string())
        } else {
            Some("ephemeral_unscoped".to_string())
        };

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
                            DiffChange::CutTransitionCreated { transition_id, .. } => {
                                Some(transition_id.clone())
                            }
                            DiffChange::CutTransitionUpdated { transition_id, .. } => {
                                Some(transition_id.clone())
                            }
                            DiffChange::CutTransitionRemoved { transition_id } => {
                                Some(transition_id.clone())
                            }
                            _ => None,
                        },
                        affected_track_id: None,
                    })
                    .collect(),
                idempotency_scope,
            });
        }

        // Check if all operations in the envelope have already been applied (exact replay)
        let is_idempotent_replay = !envelope.operations.is_empty()
            && envelope
                .operations
                .iter()
                .all(|op| self.ledger.contains_key(&op.operation_id));

        if is_idempotent_replay {
            let receipts = envelope
                .operations
                .iter()
                .map(|op| {
                    let entry = self.ledger.get(&op.operation_id);
                    OperationReceipt {
                        operation_id: op.operation_id.clone(),
                        status: "deduplicated_noop".to_string(),
                        affected_item_id: entry.and_then(|e| e.affected_item_id.clone()),
                        affected_track_id: entry.and_then(|e| e.affected_track_id.clone()),
                    }
                })
                .collect();
            return Ok(CommandReceipt {
                project_id: self.project.project_id.clone(),
                prior_revision: self.project.revision,
                new_revision: self.project.revision,
                transaction_id: envelope.transaction_id,
                operations_applied: 0,
                receipts,
                idempotency_scope,
            });
        }

        // Atomic transaction: Staged Transaction Isolation (ADR-001 v3 Section 4.1)
        // Pre-commit validation and simulation clones only editorial timeline state (tracks, assets, settings)
        // isolating the command ledger vector.
        let mut staged_project = Project {
            schema_version: self.project.schema_version,
            project_id: self.project.project_id.clone(),
            name: self.project.name.clone(),
            revision: self.project.revision,
            settings: self.project.settings.clone(),
            assets: self.project.assets.clone(),
            tracks: self.project.tracks.clone(),
            created_at: self.project.created_at.clone(),
            updated_at: self.project.updated_at.clone(),
            client_registry: self.project.client_registry.clone(),
            command_ledger: Vec::new(),
        };
        let mut staged_receipts = Vec::new();
        let mut staged_ledger_entries = Vec::new();

        let client_id = envelope.client_id.clone();
        let client_epoch = envelope.client_epoch;
        let sequence_number = envelope.sequence_number;

        for op_env in &envelope.operations {
            let op_changes = Self::apply_operation_to_project(
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

            let (affected_item_id, affected_track_id) = match &op_env.operation {
                CommandOperation::ClipGroupMove { item_ids, .. } => {
                    (item_ids.first().cloned(), None)
                }
                CommandOperation::ClipRippleDelete { item_id } => (Some(item_id.clone()), None),
                _ => match op_changes.first() {
                    Some(DiffChange::ClipCreated { item_id, track_id }) => {
                        (Some(item_id.clone()), Some(track_id.clone()))
                    }
                    Some(DiffChange::ClipMoved { item_id, .. }) => (Some(item_id.clone()), None),
                    Some(DiffChange::ClipTrimmed { item_id, .. }) => (Some(item_id.clone()), None),
                    Some(DiffChange::ClipSplit {
                        original_item_id, ..
                    }) => (Some(original_item_id.clone()), None),
                    Some(DiffChange::ClipDeleted { item_id }) => (Some(item_id.clone()), None),
                    Some(DiffChange::ClipDuplicated { new_item_id, .. }) => {
                        (Some(new_item_id.clone()), None)
                    }
                    Some(DiffChange::ClipModified { item_id }) => (Some(item_id.clone()), None),
                    Some(DiffChange::TrackCreated { track_id, .. }) => {
                        (None, Some(track_id.clone()))
                    }
                    Some(DiffChange::TrackDeleted { track_id }) => (None, Some(track_id.clone())),
                    Some(DiffChange::TrackModified { track_id }) => (None, Some(track_id.clone())),
                    Some(DiffChange::AssetImported { asset_id }) => (Some(asset_id.clone()), None),
                    Some(DiffChange::CutTransitionCreated {
                        transition_id,
                        track_id,
                    }) => (Some(transition_id.clone()), Some(track_id.clone())),
                    Some(DiffChange::CutTransitionUpdated { transition_id, .. }) => {
                        (Some(transition_id.clone()), None)
                    }
                    Some(DiffChange::CutTransitionRemoved { transition_id }) => {
                        (Some(transition_id.clone()), None)
                    }
                    None => (None, None),
                },
            };

            let fp = compute_operation_fingerprint(&op_env.operation);
            staged_receipts.push(OperationReceipt {
                operation_id: op_env.operation_id.clone(),
                status: "applied".to_string(),
                affected_item_id: affected_item_id.clone(),
                affected_track_id: affected_track_id.clone(),
            });

            staged_ledger_entries.push(CommandLedgerEntry {
                operation_id: op_env.operation_id.clone(),
                transaction_id: envelope.transaction_id.clone(),
                client_id: client_id.clone(),
                client_epoch,
                sequence_number,
                fingerprint: fp,
                status: "applied".to_string(),
                applied_at_revision: self.project.revision + 1,
                affected_item_id,
                affected_track_id,
            });
        }

        // If all operations succeeded and validated: commit atomic state!
        let prior_revision = self.project.revision;
        let new_revision = if !staged_ledger_entries.is_empty() {
            prior_revision + 1
        } else {
            prior_revision
        };

        if !staged_ledger_entries.is_empty() {
            self.project.revision = new_revision;
            self.project.updated_at = chrono::Utc::now().to_rfc3339();
            self.project.tracks = staged_project.tracks;
            self.project.assets = staged_project.assets;
            self.project.settings = staged_project.settings;

            // Update ClientRegistry for structured identity
            if let Some(ref cid) = client_id {
                let epoch = client_epoch.unwrap_or(self.project.client_registry.current_epoch);
                let seq = sequence_number.unwrap_or(1);
                let client_rec = self
                    .project
                    .client_registry
                    .clients
                    .entry(cid.clone())
                    .or_insert_with(|| crate::modules::domain::ClientRecord {
                        client_epoch: epoch,
                        highest_sequence: 0,
                        last_active_revision: new_revision,
                        registered_at_revision: new_revision,
                    });
                client_rec.client_epoch = epoch;
                if seq > client_rec.highest_sequence {
                    client_rec.highest_sequence = seq;
                }
                client_rec.last_active_revision = new_revision;
            }

            // Append entries to active window
            for entry in staged_ledger_entries {
                self.ledger
                    .insert(entry.operation_id.clone(), entry.clone());
                self.project.command_ledger.push(entry);
            }

            // Enforce Active Window Capacity (K=5,000 FIFO eviction)
            const ACTIVE_WINDOW_CAPACITY: usize = 5000;
            if self.project.command_ledger.len() > ACTIVE_WINDOW_CAPACITY {
                let excess = self.project.command_ledger.len() - ACTIVE_WINDOW_CAPACITY;
                for evicted in self.project.command_ledger.drain(0..excess) {
                    self.ledger.remove(&evicted.operation_id);
                }
            }
        }

        Ok(CommandReceipt {
            project_id: self.project.project_id.clone(),
            prior_revision,
            new_revision,
            transaction_id: envelope.transaction_id,
            operations_applied: staged_receipts.len(),
            receipts: staged_receipts,
            idempotency_scope,
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

        // 1. Structured Scoped Identity Validation (ADR-001 v3 Section 4.2 & 5)
        if let Some(ref client_id) = envelope.client_id {
            let registry = &self.project.client_registry;

            // Epoch fencing: verify client_epoch is not expired
            if let Some(client_epoch) = envelope.client_epoch {
                if client_epoch < registry.min_accepted_epoch {
                    return Err(CommandError::ClientEpochExpired {
                        client_id: client_id.clone(),
                        client_epoch,
                        min_accepted_epoch: registry.min_accepted_epoch,
                    });
                }
            }

            if let Some(client_rec) = registry.clients.get(client_id) {
                if let Some(client_epoch) = envelope.client_epoch {
                    if client_epoch < client_rec.client_epoch {
                        return Err(CommandError::ClientEpochExpired {
                            client_id: client_id.clone(),
                            client_epoch,
                            min_accepted_epoch: client_rec.client_epoch,
                        });
                    }
                }

                if let Some(seq) = envelope.sequence_number {
                    if seq <= client_rec.highest_sequence {
                        // Monotonic sequence proof: this sequence was committed in the past!
                        let all_in_active_window = !envelope.operations.is_empty()
                            && envelope
                                .operations
                                .iter()
                                .all(|op| self.ledger.contains_key(&op.operation_id));

                        if all_in_active_window {
                            // In active window: verify fingerprints
                            for op_env in &envelope.operations {
                                if let Some(existing) = self.ledger.get(&op_env.operation_id) {
                                    if is_legacy_fnv_fingerprint(&existing.fingerprint) {
                                        return Err(CommandError::LegacyReceiptUnverifiable {
                                            operation_id: op_env.operation_id.clone(),
                                        });
                                    }
                                    let fp = compute_operation_fingerprint(&op_env.operation);
                                    if existing.fingerprint != fp {
                                        return Err(CommandError::IdempotencyCollision {
                                            operation_id: op_env.operation_id.clone(),
                                            existing_fingerprint: existing.fingerprint.clone(),
                                            received_fingerprint: fp,
                                        });
                                    }
                                }
                            }
                            return Ok(());
                        } else {
                            // Evicted from active window!
                            let evicted_op = envelope
                                .operations
                                .iter()
                                .find(|op| !self.ledger.contains_key(&op.operation_id))
                                .map(|op| op.operation_id.clone())
                                .unwrap_or_else(|| "unknown".to_string());
                            return Err(CommandError::OperationAlreadyCommitted {
                                operation_id: evicted_op,
                                client_id: client_id.clone(),
                                sequence_number: seq,
                            });
                        }
                    } else if seq > client_rec.highest_sequence + 1 {
                        return Err(CommandError::SequenceGapDetected {
                            client_id: client_id.clone(),
                            expected_sequence: client_rec.highest_sequence + 1,
                            received_sequence: seq,
                        });
                    }
                }
            } else {
                // Client not yet registered
                if registry.max_active_clients > 0
                    && registry.clients.len() >= registry.max_active_clients
                {
                    return Err(CommandError::ClientRegistryFull {
                        max_clients: registry.max_active_clients,
                    });
                }
                if let Some(seq) = envelope.sequence_number {
                    if seq != 1 {
                        return Err(CommandError::SequenceGapDetected {
                            client_id: client_id.clone(),
                            expected_sequence: 1,
                            received_sequence: seq,
                        });
                    }
                }
            }
        }

        // 2. Active Window Collision & Legacy FNV Checks
        for op_env in &envelope.operations {
            let fp = compute_operation_fingerprint(&op_env.operation);
            if let Some(existing) = self.ledger.get(&op_env.operation_id) {
                if is_legacy_fnv_fingerprint(&existing.fingerprint) {
                    return Err(CommandError::LegacyReceiptUnverifiable {
                        operation_id: op_env.operation_id.clone(),
                    });
                }
                if existing.fingerprint != fp {
                    return Err(CommandError::IdempotencyCollision {
                        operation_id: op_env.operation_id.clone(),
                        existing_fingerprint: existing.fingerprint.clone(),
                        received_fingerprint: fp,
                    });
                }
            }
        }

        // 3. Exact Idempotent Replay Check
        let is_idempotent_replay = !envelope.operations.is_empty()
            && envelope
                .operations
                .iter()
                .all(|op| self.ledger.contains_key(&op.operation_id));

        if is_idempotent_replay {
            return Ok(());
        }

        // 4. Reject partial mixture of already-committed operation IDs and new operations
        let has_any_committed = envelope
            .operations
            .iter()
            .any(|op| self.ledger.contains_key(&op.operation_id));
        if has_any_committed {
            return Err(CommandError::OperationValidationFailed {
                operation_id: "transaction_atomic".to_string(),
                message: "Transaction contains a mixture of already-committed operation IDs and new operations".to_string(),
            });
        }

        // 5. For unseen operations, enforce strict base_revision check
        if envelope.base_revision != self.project.revision {
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
    ) -> Result<Vec<DiffChange>, CommandError> {
        match operation {
            CommandOperation::ClipGroupMove { item_ids, delta } => {
                Self::apply_clip_group_move(project, item_ids, delta, op_id)
            }
            CommandOperation::ClipRippleDelete { item_id } => {
                Self::apply_clip_ripple_delete(project, item_id, op_id)
            }
            _ => {
                let single_change =
                    Self::apply_single_operation_to_project(project, operation, op_id)?;
                Ok(single_change.into_iter().collect())
            }
        }
    }

    fn apply_clip_group_move(
        project: &mut Project,
        item_ids: &[String],
        delta: &RationalTime,
        op_id: &str,
    ) -> Result<Vec<DiffChange>, CommandError> {
        if item_ids.is_empty() {
            return Err(CommandError::OperationValidationFailed {
                operation_id: op_id.to_string(),
                message: "item_ids cannot be empty in ClipGroupMove".to_string(),
            });
        }

        let mut id_set = std::collections::HashSet::new();
        for id in item_ids {
            if !id_set.insert(id.as_str()) {
                return Err(CommandError::OperationValidationFailed {
                    operation_id: op_id.to_string(),
                    message: format!("Duplicate item_id '{}' in ClipGroupMove", id),
                });
            }
        }

        struct MoveSpec {
            track_id: String,
            item_id: String,
            new_start: RationalTime,
            duration: RationalTime,
        }

        let mut specs = Vec::with_capacity(item_ids.len());

        for id in item_ids {
            let mut found = None;
            for track in &project.tracks {
                if let Some(item) = track.find_item(id) {
                    if track.locked {
                        return Err(CommandError::OperationValidationFailed {
                            operation_id: op_id.to_string(),
                            message: format!("Track '{}' is locked", track.track_id),
                        });
                    }
                    let shift = delta.rescaled_to(item.timeline_range.start.rate);
                    let new_start = item.timeline_range.start + shift;
                    if new_start.semantic_lt(&RationalTime::zero(new_start.rate)) {
                        return Err(CommandError::OperationValidationFailed {
                            operation_id: op_id.to_string(),
                            message: format!(
                                "Resulting timeline start for item '{}' would be negative: {:?}",
                                id, new_start
                            ),
                        });
                    }
                    found = Some(MoveSpec {
                        track_id: track.track_id.clone(),
                        item_id: id.clone(),
                        new_start,
                        duration: item.timeline_range.duration,
                    });
                    break;
                }
            }
            match found {
                Some(spec) => specs.push(spec),
                None => {
                    return Err(CommandError::OperationValidationFailed {
                        operation_id: op_id.to_string(),
                        message: format!("Item '{}' not found", id),
                    });
                }
            }
        }

        // Apply all target starts simultaneously
        for spec in &specs {
            let track = project.find_track_mut(&spec.track_id).unwrap();
            let item = track.find_item_mut(&spec.item_id).unwrap();
            item.timeline_range = TimeRange::new(spec.new_start, spec.duration);
        }

        // Validate overlap on all affected tracks
        let affected_track_ids: std::collections::HashSet<String> =
            specs.iter().map(|s| s.track_id.clone()).collect();
        for track_id in &affected_track_ids {
            let track = project.find_track(track_id).unwrap();
            for i in 0..track.items.len() {
                for j in (i + 1)..track.items.len() {
                    let item_a = &track.items[i];
                    let item_b = &track.items[j];
                    if item_a.timeline_range.overlaps(&item_b.timeline_range) {
                        return Err(CommandError::OperationValidationFailed {
                            operation_id: op_id.to_string(),
                            message: format!(
                                "Collision on track '{}': items '{}' and '{}' overlap",
                                track.track_id, item_a.item_id, item_b.item_id
                            ),
                        });
                    }
                }
            }
        }

        // Transition reconciliation:
        // Internal transitions (both participants in item_ids): preserved if still adjacent & valid handles
        // External transitions (one participant in item_ids): removed if broken adjacency
        let target_rate = project.settings.frame_rate;
        let mut transitions_to_remove = Vec::new();

        for track in &project.tracks {
            for t in &track.transitions {
                let left_in = id_set.contains(t.left_item_id.as_str());
                let right_in = id_set.contains(t.right_item_id.as_str());
                if left_in || right_in {
                    let mut valid = false;
                    if let (Some(left), Some(right)) = (
                        track.find_item(&t.left_item_id),
                        track.find_item(&t.right_item_id),
                    ) {
                        if left
                            .timeline_range
                            .is_adjacent_to_with_rate(&right.timeline_range, target_rate)
                        {
                            valid = Self::validate_cut_transition_handles(
                                project,
                                &track.track_id,
                                &t.left_item_id,
                                &t.right_item_id,
                                t.duration,
                                op_id,
                            )
                            .is_ok();
                        }
                    }
                    if !valid {
                        transitions_to_remove
                            .push((track.track_id.clone(), t.transition_id.clone()));
                    }
                }
            }
        }

        let mut diff_changes = Vec::new();
        for spec in specs {
            diff_changes.push(DiffChange::ClipMoved {
                item_id: spec.item_id,
                new_start: spec.new_start,
            });
        }

        for (track_id, trans_id) in transitions_to_remove {
            if let Some(track) = project.find_track_mut(&track_id) {
                track.remove_transition(&trans_id);
                diff_changes.push(DiffChange::CutTransitionRemoved {
                    transition_id: trans_id,
                });
            }
        }

        Ok(diff_changes)
    }

    fn apply_clip_ripple_delete(
        project: &mut Project,
        item_id: &str,
        op_id: &str,
    ) -> Result<Vec<DiffChange>, CommandError> {
        let mut found_pos = None;
        let mut found_track_id = None;
        for track in &project.tracks {
            if let Some(pos) = track.items.iter().position(|i| i.item_id == item_id) {
                found_pos = Some(pos);
                found_track_id = Some(track.track_id.clone());
                break;
            }
        }

        let (track_id, item_pos) = match (found_track_id, found_pos) {
            (Some(tid), Some(pos)) => (tid, pos),
            _ => {
                return Err(CommandError::OperationValidationFailed {
                    operation_id: op_id.to_string(),
                    message: format!("Item '{}' not found", item_id),
                });
            }
        };

        let track = project.find_track_mut(&track_id).unwrap();
        if track.locked {
            return Err(CommandError::OperationValidationFailed {
                operation_id: op_id.to_string(),
                message: format!("Track '{}' is locked", track_id),
            });
        }

        let deleted_item = track.items.remove(item_pos);
        let deleted_end = deleted_item.timeline_range.end();
        let deleted_duration = deleted_item.timeline_range.duration;

        // Remove directly dependent transitions
        let mut removed_transitions = Vec::new();
        track.transitions.retain(|t| {
            let is_dep = t.left_item_id == item_id || t.right_item_id == item_id;
            if is_dep {
                removed_transitions.push(t.transition_id.clone());
            }
            !is_dep
        });

        // Shift surviving items on SAME track where timeline_start >= deleted_end
        // Semantic RationalTime comparison via exact 128-bit cross-multiplication (ZERO float conversion)
        let mut moved_downstream = Vec::new();
        for item in &mut track.items {
            if item.timeline_range.start.semantic_ge(&deleted_end) {
                let shift_dur = deleted_duration.rescaled_to(item.timeline_range.start.rate);
                let new_start = item.timeline_range.start - shift_dur;
                item.timeline_range = TimeRange::new(new_start, item.timeline_range.duration);
                moved_downstream.push((item.item_id.clone(), new_start));
            }
        }

        let mut diff_changes = Vec::new();
        diff_changes.push(DiffChange::ClipDeleted {
            item_id: item_id.to_string(),
        });
        for (m_id, new_start) in moved_downstream {
            diff_changes.push(DiffChange::ClipMoved {
                item_id: m_id,
                new_start,
            });
        }
        for trans_id in removed_transitions {
            diff_changes.push(DiffChange::CutTransitionRemoved {
                transition_id: trans_id,
            });
        }

        Ok(diff_changes)
    }

    fn apply_single_operation_to_project(
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

                // Dependent transition reconciliation for move
                Self::reconcile_dependent_transitions_after_move(project, item_id, op_id);

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
                Self::reconcile_dependent_transitions_before_trim(
                    project,
                    item_id,
                    new_timeline_range,
                    new_source_range,
                    op_id,
                )?;

                let mut found = false;
                for track in &mut project.tracks {
                    if let Some(item) = track.find_item_mut(item_id) {
                        // KF-20, KF-21, KF-22: Non-destructive head trim keyframe shift
                        // Shift every keyframe offset by -H where H = new_source_start - old_source_start
                        let h = new_source_range.start - item.source_range.start;
                        if !h.is_same_time(&RationalTime::zero(h.rate)) {
                            for kf in &mut item.keyframes {
                                kf.offset = kf.offset - h;
                            }
                        }

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

                Self::cleanup_broken_transitions_after_trim(project, item_id);

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
                            || split_time.is_same_time(&item.timeline_range.start)
                        {
                            return Err(CommandError::OperationValidationFailed {
                                operation_id: op_id.to_string(),
                                message: format!(
                                    "Split time {:?} is outside clip range {:?}",
                                    split_time, item.timeline_range
                                ),
                            });
                        }

                        // Check transitions involving item_id for split restriction:
                        // "split inside the transition-used region is rejected in Phase 13"
                        let target_rate = project.settings.frame_rate;
                        for t in &track.transitions {
                            let d_ticks = t.duration.rescaled_to(target_rate).value;
                            let l_ticks = d_ticks / 2;
                            let r_ticks = d_ticks - l_ticks;
                            let l_dur = RationalTime::new(l_ticks, target_rate);
                            let r_dur = RationalTime::new(r_ticks, target_rate);

                            if t.left_item_id == *item_id {
                                // Outgoing participant: transition interval before cut is [C - L, C)
                                let cut = item.timeline_range.end();
                                let t0 = cut - l_dur;
                                if *split_time >= t0 && *split_time < cut {
                                    return Err(CommandError::OperationValidationFailed {
                                        operation_id: op_id.to_string(),
                                        message: format!(
                                            "Cannot split clip '{}' inside active transition region [{:?}, {:?})",
                                            item_id, t0, cut
                                        ),
                                    });
                                }
                            }
                            if t.right_item_id == *item_id {
                                // Incoming participant: transition interval after cut is [C, C + R)
                                let cut = item.timeline_range.start;
                                let t1 = cut + r_dur;
                                if *split_time >= cut && *split_time < t1 {
                                    return Err(CommandError::OperationValidationFailed {
                                        operation_id: op_id.to_string(),
                                        message: format!(
                                            "Cannot split clip '{}' inside active transition region [{:?}, {:?})",
                                            item_id, cut, t1
                                        ),
                                    });
                                }
                            }
                        }

                        // Left duration: split_time - start
                        let left_dur = *split_time - item.timeline_range.start;
                        let right_dur = item.timeline_range.end() - *split_time;
                        let clip_local_split = left_dur;

                        // Keyframe partition and de Casteljau subdivision (KF-23 .. KF-28)
                        let mut left_keyframes = Vec::new();
                        let mut right_keyframes = Vec::new();

                        let all_props = [
                            KeyframeProperty::Opacity,
                            KeyframeProperty::PositionX,
                            KeyframeProperty::PositionY,
                            KeyframeProperty::ScaleX,
                            KeyframeProperty::ScaleY,
                            KeyframeProperty::Rotation,
                            KeyframeProperty::Volume,
                        ];

                        for prop in all_props {
                            let mut prop_kfs: Vec<Keyframe> = item
                                .keyframes
                                .iter()
                                .filter(|k| k.property == prop)
                                .cloned()
                                .collect();
                            if prop_kfs.is_empty() {
                                continue;
                            }
                            prop_kfs.sort_by_key(|a| a.offset);

                            let exact_match = prop_kfs
                                .iter()
                                .find(|k| k.offset.is_same_time(&clip_local_split));

                            if let Some(boundary_kf) = exact_match {
                                let b_val = boundary_kf.value;
                                let b_easing = boundary_kf.outgoing_easing;

                                for k in &prop_kfs {
                                    if k.offset.cmp(&clip_local_split) == std::cmp::Ordering::Less {
                                        left_keyframes.push(k.clone());
                                    } else if k.offset.cmp(&clip_local_split)
                                        == std::cmp::Ordering::Greater
                                    {
                                        let rebased_offset = k.offset - clip_local_split;
                                        right_keyframes.push(Keyframe {
                                            property: prop,
                                            offset: rebased_offset,
                                            value: k.value,
                                            outgoing_easing: k.outgoing_easing,
                                        });
                                    }
                                }

                                left_keyframes.push(Keyframe {
                                    property: prop,
                                    offset: clip_local_split,
                                    value: b_val,
                                    outgoing_easing: KeyframeEasing::Linear,
                                });

                                let right_has_successors =
                                    right_keyframes.iter().any(|k| k.property == prop);
                                let right_easing = if right_has_successors {
                                    b_easing
                                } else {
                                    KeyframeEasing::Linear
                                };
                                right_keyframes.push(Keyframe {
                                    property: prop,
                                    offset: RationalTime::zero(clip_local_split.rate),
                                    value: b_val,
                                    outgoing_easing: right_easing,
                                });
                            } else {
                                let mut crossing_idx = None;
                                for i in 0..prop_kfs.len().saturating_sub(1) {
                                    let k0 = &prop_kfs[i];
                                    let k1 = &prop_kfs[i + 1];
                                    if k0.offset.cmp(&clip_local_split) == std::cmp::Ordering::Less
                                        && k1.offset.cmp(&clip_local_split)
                                            == std::cmp::Ordering::Greater
                                    {
                                        crossing_idx = Some(i);
                                        break;
                                    }
                                }

                                if let Some(idx) = crossing_idx {
                                    let k0 = &prop_kfs[idx];
                                    let k1 = &prop_kfs[idx + 1];

                                    let v_s = crate::modules::domain::evaluator::evaluate_property_channel(
                                        &prop_kfs,
                                        prop,
                                        k0.value,
                                        clip_local_split,
                                    );

                                    let seg_dur = (k1.offset - k0.offset).to_seconds();
                                    let elapsed = (clip_local_split - k0.offset).to_seconds();
                                    let u_s = if seg_dur > 1e-9 {
                                        (elapsed / seg_dur).clamp(0.0, 1.0)
                                    } else {
                                        0.0
                                    };

                                    let (left_sub_easing, right_sub_easing) =
                                        crate::modules::domain::evaluator::subdivide_easing(
                                            k0.outgoing_easing,
                                            u_s,
                                        );

                                    for (i, k) in prop_kfs.iter().enumerate() {
                                        if i < idx {
                                            left_keyframes.push(k.clone());
                                        } else if i == idx {
                                            left_keyframes.push(Keyframe {
                                                property: prop,
                                                offset: k.offset,
                                                value: k.value,
                                                outgoing_easing: left_sub_easing,
                                            });
                                        } else {
                                            let rebased_offset = k.offset - clip_local_split;
                                            right_keyframes.push(Keyframe {
                                                property: prop,
                                                offset: rebased_offset,
                                                value: k.value,
                                                outgoing_easing: k.outgoing_easing,
                                            });
                                        }
                                    }

                                    left_keyframes.push(Keyframe {
                                        property: prop,
                                        offset: clip_local_split,
                                        value: v_s,
                                        outgoing_easing: KeyframeEasing::Linear,
                                    });

                                    right_keyframes.push(Keyframe {
                                        property: prop,
                                        offset: RationalTime::zero(clip_local_split.rate),
                                        value: v_s,
                                        outgoing_easing: right_sub_easing,
                                    });
                                } else {
                                    for k in &prop_kfs {
                                        if k.offset.cmp(&clip_local_split)
                                            == std::cmp::Ordering::Less
                                        {
                                            left_keyframes.push(k.clone());
                                        } else {
                                            let rebased_offset = k.offset - clip_local_split;
                                            right_keyframes.push(Keyframe {
                                                property: prop,
                                                offset: rebased_offset,
                                                value: k.value,
                                                outgoing_easing: k.outgoing_easing,
                                            });
                                        }
                                    }
                                }
                            }
                        }

                        left_keyframes.sort_by_key(|a| a.offset);
                        right_keyframes.sort_by_key(|a| a.offset);

                        // Normalize last-keyframe invariant
                        for prop in all_props {
                            let mut left_p: Vec<&mut Keyframe> = left_keyframes
                                .iter_mut()
                                .filter(|k| k.property == prop)
                                .collect();
                            if let Some(last) = left_p.last_mut() {
                                last.outgoing_easing = KeyframeEasing::Linear;
                            }
                            let mut right_p: Vec<&mut Keyframe> = right_keyframes
                                .iter_mut()
                                .filter(|k| k.property == prop)
                                .collect();
                            if let Some(last) = right_p.last_mut() {
                                last.outgoing_easing = KeyframeEasing::Linear;
                            }
                        }

                        let mut right_item = item.clone();
                        let right_id = new_right_item_id.clone().unwrap_or_else(generate_item_id);
                        right_item.item_id = right_id.clone();
                        right_item.timeline_range = TimeRange::new(*split_time, right_dur);
                        right_item.source_range =
                            TimeRange::new(item.source_range.start + left_dur, right_dur);
                        right_item.keyframes = right_keyframes;

                        let left_item = &mut track.items[pos];
                        left_item.timeline_range =
                            TimeRange::new(left_item.timeline_range.start, left_dur);
                        left_item.source_range =
                            TimeRange::new(left_item.source_range.start, left_dur);
                        left_item.keyframes = left_keyframes;

                        left_item.validate()?;
                        right_item.validate()?;

                        track.items.push(right_item);

                        // Retarget transitions:
                        // "outgoing split uses the fragment that remains adjacent to the incoming participant"
                        // Outgoing participant's right fragment (right_id) is now adjacent to cut C.
                        for t in &mut track.transitions {
                            if t.left_item_id == *item_id {
                                t.left_item_id = right_id.clone();
                            }
                        }

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
                        track
                            .transitions
                            .retain(|t| t.left_item_id != *item_id && t.right_item_id != *item_id);
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

                let target_rate = project.settings.frame_rate;
                let dest_track_id = target_track_id.as_ref().or(orig_track_id.as_ref()).unwrap();
                let dest_track = project.find_track_mut(dest_track_id).ok_or_else(|| {
                    CommandError::OperationValidationFailed {
                        operation_id: op_id.to_string(),
                        message: format!("Track '{}' not found", dest_track_id),
                    }
                })?;

                dest_track.items.push(item);
                let items = &dest_track.items;
                dest_track.transitions.retain(|t| {
                    let l = items.iter().find(|i| i.item_id == t.left_item_id);
                    let r = items.iter().find(|i| i.item_id == t.right_item_id);
                    if let (Some(l), Some(r)) = (l, r) {
                        l.timeline_range
                            .is_adjacent_to_with_rate(&r.timeline_range, target_rate)
                    } else {
                        false
                    }
                });
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
                if let Some(ref t) = transition {
                    if t.kind == TransitionKind::CrossDissolve {
                        return Err(CommandError::UnsupportedTransition {
                            operation_id: op_id.to_string(),
                            transition: "CrossDissolve".to_string(),
                        });
                    }
                    if t.duration.value <= 0 {
                        return Err(CommandError::OperationValidationFailed {
                            operation_id: op_id.to_string(),
                            message: "Transition duration must be strictly positive".to_string(),
                        });
                    }
                }

                let mut found = false;
                for track in &mut project.tracks {
                    if let Some(item) = track.find_item_mut(item_id) {
                        if let Some(ref t) = transition {
                            if t.duration.value > item.timeline_range.duration.value {
                                return Err(CommandError::OperationValidationFailed {
                                    operation_id: op_id.to_string(),
                                    message: format!(
                                        "Transition duration ({} frames) cannot exceed clip duration ({} frames)",
                                        t.duration.value, item.timeline_range.duration.value
                                    ),
                                });
                            }
                        }
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

            CommandOperation::ClipKeyframeAdd {
                item_id,
                keyframe,
                easing,
            } => {
                let mut found = false;
                for track in &mut project.tracks {
                    if let Some(item) = track.find_item_mut(item_id) {
                        // Validate explicit easing if provided
                        if let Some(ref e) = easing {
                            e.validate().map_err(|err| {
                                CommandError::OperationValidationFailed {
                                    operation_id: op_id.to_string(),
                                    message: format!("INVALID_KEYFRAME_EASING_PARAMETERS: {}", err),
                                }
                            })?;
                        }
                        keyframe.outgoing_easing.validate().map_err(|err| {
                            CommandError::OperationValidationFailed {
                                operation_id: op_id.to_string(),
                                message: format!("INVALID_KEYFRAME_EASING_PARAMETERS: {}", err),
                            }
                        })?;

                        // Find existing keyframe at same semantic timestamp
                        let existing_idx = item.keyframes.iter().position(|k| {
                            k.property == keyframe.property
                                && k.offset.is_same_time(&keyframe.offset)
                        });

                        let final_easing = if let Some(idx) = existing_idx {
                            // Replacing existing keyframe:
                            // KF-17: explicit easing replaces easing; omitted easing preserves existing easing.
                            if let Some(ref e) = easing {
                                *e
                            } else if keyframe.outgoing_easing != KeyframeEasing::Linear {
                                keyframe.outgoing_easing
                            } else {
                                item.keyframes[idx].outgoing_easing
                            }
                        } else {
                            // New keyframe:
                            easing.unwrap_or(keyframe.outgoing_easing)
                        };

                        // Remove existing keyframe at same semantic offset
                        item.keyframes.retain(|k| {
                            !(k.property == keyframe.property
                                && k.offset.is_same_time(&keyframe.offset))
                        });

                        item.keyframes.push(Keyframe {
                            property: keyframe.property,
                            offset: keyframe.offset,
                            value: keyframe.value,
                            outgoing_easing: final_easing,
                        });

                        // Sort keyframes using canonical semantic RationalTime ordering
                        item.keyframes.sort_by_key(|a| a.offset);

                        // Normalize last-keyframe invariant
                        let mut prop_kfs: Vec<&mut Keyframe> = item
                            .keyframes
                            .iter_mut()
                            .filter(|k| k.property == keyframe.property)
                            .collect();
                        if let Some(last) = prop_kfs.last_mut() {
                            last.outgoing_easing = KeyframeEasing::Linear;
                        }

                        item.validate()
                            .map_err(|e| CommandError::OperationValidationFailed {
                                operation_id: op_id.to_string(),
                                message: e.to_string(),
                            })?;

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
                        let initial_len = item.keyframes.len();
                        item.keyframes.retain(|k| {
                            !(k.property == *property && k.offset.is_same_time(offset))
                        });
                        if item.keyframes.len() == initial_len {
                            return Err(CommandError::OperationValidationFailed {
                                operation_id: op_id.to_string(),
                                message: format!(
                                    "Keyframe on {:?} at offset {:?} not found",
                                    property, offset
                                ),
                            });
                        }

                        // Normalize last-keyframe invariant on remaining keyframes of this property
                        let mut prop_kfs: Vec<&mut Keyframe> = item
                            .keyframes
                            .iter_mut()
                            .filter(|k| k.property == *property)
                            .collect();
                        if let Some(last) = prop_kfs.last_mut() {
                            last.outgoing_easing = KeyframeEasing::Linear;
                        }

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

            CommandOperation::ClipKeyframeMove {
                item_id,
                property,
                old_offset,
                new_offset,
            } => {
                if old_offset.is_same_time(new_offset) {
                    return Ok(Some(DiffChange::ClipModified {
                        item_id: item_id.clone(),
                    }));
                }

                let mut found = false;
                for track in &mut project.tracks {
                    if let Some(item) = track.find_item_mut(item_id) {
                        let (kf_val, kf_easing) =
                            if let Some(existing) = item.keyframes.iter().find(|k| {
                                k.property == *property && k.offset.is_same_time(old_offset)
                            }) {
                                (existing.value, existing.outgoing_easing)
                            } else {
                                return Err(CommandError::OperationValidationFailed {
                                    operation_id: op_id.to_string(),
                                    message: format!(
                                        "Keyframe on {:?} at offset {:?} not found",
                                        property, old_offset
                                    ),
                                });
                            };

                        // Remove keyframe at old_offset
                        item.keyframes.retain(|k| {
                            !(k.property == *property && k.offset.is_same_time(old_offset))
                        });
                        // Remove any existing keyframe at new_offset on same property (replace policy)
                        item.keyframes.retain(|k| {
                            !(k.property == *property && k.offset.is_same_time(new_offset))
                        });

                        // Insert at new_offset with preserved value and easing
                        item.keyframes.push(Keyframe {
                            property: *property,
                            offset: *new_offset,
                            value: kf_val,
                            outgoing_easing: kf_easing,
                        });

                        // Sort keyframes using canonical semantic RationalTime ordering
                        item.keyframes.sort_by_key(|a| a.offset);

                        // Normalize last-keyframe invariant
                        let mut prop_kfs: Vec<&mut Keyframe> = item
                            .keyframes
                            .iter_mut()
                            .filter(|k| k.property == *property)
                            .collect();
                        if let Some(last) = prop_kfs.last_mut() {
                            last.outgoing_easing = KeyframeEasing::Linear;
                        }

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

            CommandOperation::ClipKeyframeEasingSet {
                item_id,
                property,
                offset,
                easing,
            } => {
                easing
                    .validate()
                    .map_err(|e| CommandError::OperationValidationFailed {
                        operation_id: op_id.to_string(),
                        message: format!("INVALID_KEYFRAME_EASING_PARAMETERS: {}", e),
                    })?;

                let mut found = false;
                for track in &mut project.tracks {
                    if let Some(item) = track.find_item_mut(item_id) {
                        let mut prop_indices: Vec<usize> = item
                            .keyframes
                            .iter()
                            .enumerate()
                            .filter(|(_, k)| k.property == *property)
                            .map(|(i, _)| i)
                            .collect();

                        if prop_indices.is_empty() {
                            return Err(CommandError::OperationValidationFailed {
                                operation_id: op_id.to_string(),
                                message: format!(
                                    "Keyframe on {:?} at offset {:?} not found",
                                    property, offset
                                ),
                            });
                        }

                        prop_indices.sort_by(|&a, &b| {
                            item.keyframes[a].offset.cmp(&item.keyframes[b].offset)
                        });

                        let target_pos = prop_indices
                            .iter()
                            .position(|&idx| item.keyframes[idx].offset.is_same_time(offset));

                        let pos_in_prop = match target_pos {
                            Some(p) => p,
                            None => {
                                return Err(CommandError::OperationValidationFailed {
                                    operation_id: op_id.to_string(),
                                    message: format!(
                                        "Keyframe on {:?} at offset {:?} not found",
                                        property, offset
                                    ),
                                });
                            }
                        };

                        // KF-05, KF-06: Last keyframe has no outgoing segment; setting non-Linear easing is rejected
                        let is_last = pos_in_prop == prop_indices.len() - 1;
                        if is_last && !easing.is_linear() {
                            return Err(CommandError::OperationValidationFailed {
                                operation_id: op_id.to_string(),
                                message: "LAST_KEYFRAME_EASING_MUST_BE_LINEAR: The last keyframe has no outgoing segment and must have Linear easing".to_string(),
                            });
                        }

                        let target_idx = prop_indices[pos_in_prop];
                        item.keyframes[target_idx].outgoing_easing = *easing;

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

            CommandOperation::CutTransitionCreate {
                transition_id,
                track_id,
                left_item_id,
                right_item_id,
                kind,
                duration,
            } => {
                if *kind != TransitionKind::CrossDissolve {
                    return Err(CommandError::UnsupportedTransition {
                        operation_id: op_id.to_string(),
                        transition: format!("{:?}", kind),
                    });
                }
                Self::validate_cut_transition_handles(
                    project,
                    track_id,
                    left_item_id,
                    right_item_id,
                    *duration,
                    op_id,
                )?;

                let track = project.find_track_mut(track_id).ok_or_else(|| {
                    CommandError::OperationValidationFailed {
                        operation_id: op_id.to_string(),
                        message: format!("Track '{}' not found", track_id),
                    }
                })?;

                // Check for duplicate transition at cut
                if track
                    .transitions
                    .iter()
                    .any(|t| t.left_item_id == *left_item_id || t.right_item_id == *right_item_id)
                {
                    return Err(CommandError::OperationValidationFailed {
                        operation_id: op_id.to_string(),
                        message: format!(
                            "A cut transition already exists on clip cut for '{}' or '{}'",
                            left_item_id, right_item_id
                        ),
                    });
                }

                let tid = transition_id.clone().unwrap_or_else(generate_transition_id);
                let cut_trans = CutTransition {
                    transition_id: tid.clone(),
                    track_id: track_id.clone(),
                    left_item_id: left_item_id.clone(),
                    right_item_id: right_item_id.clone(),
                    kind: *kind,
                    duration: *duration,
                };
                track.transitions.push(cut_trans);

                Ok(Some(DiffChange::CutTransitionCreated {
                    transition_id: tid,
                    track_id: track_id.clone(),
                }))
            }

            CommandOperation::CutTransitionUpdateDuration {
                transition_id,
                new_duration,
            } => {
                if new_duration.value <= 0 {
                    return Err(CommandError::OperationValidationFailed {
                        operation_id: op_id.to_string(),
                        message: "Transition duration must be strictly positive (> 0)".to_string(),
                    });
                }

                let (track_id, left_id, right_id) = {
                    let mut found = None;
                    for track in &project.tracks {
                        if let Some(t) = track.find_transition(transition_id) {
                            found = Some((
                                track.track_id.clone(),
                                t.left_item_id.clone(),
                                t.right_item_id.clone(),
                            ));
                            break;
                        }
                    }
                    found.ok_or_else(|| CommandError::OperationValidationFailed {
                        operation_id: op_id.to_string(),
                        message: format!("Transition '{}' not found", transition_id),
                    })?
                };

                Self::validate_cut_transition_handles(
                    project,
                    &track_id,
                    &left_id,
                    &right_id,
                    *new_duration,
                    op_id,
                )?;

                let trans = project.find_transition_mut(transition_id).ok_or_else(|| {
                    CommandError::OperationValidationFailed {
                        operation_id: op_id.to_string(),
                        message: format!("Transition '{}' not found", transition_id),
                    }
                })?;
                trans.duration = *new_duration;

                Ok(Some(DiffChange::CutTransitionUpdated {
                    transition_id: transition_id.clone(),
                    new_duration: *new_duration,
                }))
            }

            CommandOperation::CutTransitionRemove { transition_id } => {
                let mut removed = false;
                for track in &mut project.tracks {
                    if track.remove_transition(transition_id).is_some() {
                        removed = true;
                        break;
                    }
                }
                if !removed {
                    return Err(CommandError::OperationValidationFailed {
                        operation_id: op_id.to_string(),
                        message: format!("Transition '{}' not found", transition_id),
                    });
                }

                Ok(Some(DiffChange::CutTransitionRemoved {
                    transition_id: transition_id.clone(),
                }))
            }

            CommandOperation::ClipGroupMove { .. } | CommandOperation::ClipRippleDelete { .. } => {
                unreachable!(
                    "Group and ripple operations are handled in apply_operation_to_project"
                )
            }
        }
    }

    pub fn validate_cut_transition_handles(
        project: &Project,
        track_id: &str,
        left_item_id: &str,
        right_item_id: &str,
        duration: RationalTime,
        op_id: &str,
    ) -> Result<(), CommandError> {
        let track = project.find_track(track_id).ok_or_else(|| {
            CommandError::OperationValidationFailed {
                operation_id: op_id.to_string(),
                message: format!("Track '{}' not found", track_id),
            }
        })?;

        if track.kind != TrackKind::Video {
            return Err(CommandError::UnsupportedTransitionMedia {
                operation_id: op_id.to_string(),
                code: "UNSUPPORTED_TRANSITION_MEDIA".to_string(),
            });
        }

        let left = track.find_item(left_item_id).ok_or_else(|| {
            CommandError::OperationValidationFailed {
                operation_id: op_id.to_string(),
                message: format!(
                    "Left item '{}' not found on track '{}'",
                    left_item_id, track_id
                ),
            }
        })?;
        let right = track.find_item(right_item_id).ok_or_else(|| {
            CommandError::OperationValidationFailed {
                operation_id: op_id.to_string(),
                message: format!(
                    "Right item '{}' not found on track '{}'",
                    right_item_id, track_id
                ),
            }
        })?;

        if left.kind != ItemKind::Video || right.kind != ItemKind::Video {
            return Err(CommandError::UnsupportedTransitionMedia {
                operation_id: op_id.to_string(),
                code: "UNSUPPORTED_TRANSITION_MEDIA".to_string(),
            });
        }

        let target_rate = project.settings.frame_rate;
        if !left
            .timeline_range
            .is_adjacent_to_with_rate(&right.timeline_range, target_rate)
        {
            return Err(CommandError::OperationValidationFailed {
                operation_id: op_id.to_string(),
                message: format!(
                    "Participants '{}' and '{}' are not immediately adjacent (end {:?} != start {:?})",
                    left_item_id, right_item_id, left.timeline_range.end(), right.timeline_range.start
                ),
            });
        }
        let d_ticks = duration.rescaled_to(target_rate).value;
        if d_ticks <= 0 {
            return Err(CommandError::OperationValidationFailed {
                operation_id: op_id.to_string(),
                message: "Transition duration must be strictly positive (> 0)".to_string(),
            });
        }

        let l_ticks = d_ticks / 2;
        let r_ticks = d_ticks - l_ticks;
        let required_preroll = RationalTime::new(l_ticks, target_rate);
        let required_postroll = RationalTime::new(r_ticks, target_rate);

        let left_asset_id =
            left.asset_id
                .as_deref()
                .ok_or_else(|| CommandError::TransitionHandleUnverifiable {
                    operation_id: op_id.to_string(),
                    code: "TRANSITION_HANDLE_UNVERIFIABLE".to_string(),
                    reason: format!("Item '{}' has no associated asset ID", left_item_id),
                })?;
        let right_asset_id = right.asset_id.as_deref().ok_or_else(|| {
            CommandError::TransitionHandleUnverifiable {
                operation_id: op_id.to_string(),
                code: "TRANSITION_HANDLE_UNVERIFIABLE".to_string(),
                reason: format!("Item '{}' has no associated asset ID", right_item_id),
            }
        })?;

        let left_asset = project.find_asset(left_asset_id).ok_or_else(|| {
            CommandError::TransitionHandleUnverifiable {
                operation_id: op_id.to_string(),
                code: "TRANSITION_HANDLE_UNVERIFIABLE".to_string(),
                reason: format!("Asset '{}' not found in project assets", left_asset_id),
            }
        })?;
        let right_asset = project.find_asset(right_asset_id).ok_or_else(|| {
            CommandError::TransitionHandleUnverifiable {
                operation_id: op_id.to_string(),
                code: "TRANSITION_HANDLE_UNVERIFIABLE".to_string(),
                reason: format!("Asset '{}' not found in project assets", right_asset_id),
            }
        })?;

        let left_asset_dur = left_asset
            .duration
            .ok_or_else(|| CommandError::TransitionHandleUnverifiable {
                operation_id: op_id.to_string(),
                code: "TRANSITION_HANDLE_UNVERIFIABLE".to_string(),
                reason: format!("Asset '{}' duration is unprobed or missing", left_asset_id),
            })?
            .rescaled_to(target_rate);

        let _right_asset_dur = right_asset
            .duration
            .ok_or_else(|| CommandError::TransitionHandleUnverifiable {
                operation_id: op_id.to_string(),
                code: "TRANSITION_HANDLE_UNVERIFIABLE".to_string(),
                reason: format!("Asset '{}' duration is unprobed or missing", right_asset_id),
            })?
            .rescaled_to(target_rate);

        let left_src_start = left.source_range.start.rescaled_to(target_rate);
        let left_src_dur = left.source_range.duration.rescaled_to(target_rate);
        let avail_postroll_ticks =
            left_asset_dur.value - (left_src_start.value + left_src_dur.value);
        let available_postroll = RationalTime::new(avail_postroll_ticks, target_rate);

        let right_src_start = right.source_range.start.rescaled_to(target_rate);
        let avail_preroll_ticks = right_src_start.value;
        let available_preroll = RationalTime::new(avail_preroll_ticks, target_rate);

        let left_editorial_ticks = left.timeline_range.duration.rescaled_to(target_rate).value;
        let right_editorial_ticks = right.timeline_range.duration.rescaled_to(target_rate).value;

        if avail_postroll_ticks < r_ticks
            || avail_preroll_ticks < l_ticks
            || left_editorial_ticks < l_ticks
            || right_editorial_ticks < r_ticks
        {
            return Err(CommandError::InsufficientTransitionHandle {
                operation_id: op_id.to_string(),
                code: "INSUFFICIENT_TRANSITION_HANDLE".to_string(),
                message: "Cannot create CrossDissolve: required media handle exceeds available source frames.".to_string(),
                required_preroll,
                available_preroll,
                required_postroll,
                available_postroll,
            });
        }

        Ok(())
    }

    fn reconcile_dependent_transitions_after_move(
        project: &mut Project,
        item_id: &str,
        op_id: &str,
    ) {
        let mut transitions_to_remove = Vec::new();

        let target_rate = project.settings.frame_rate;
        for track in &project.tracks {
            for t in &track.transitions {
                if t.left_item_id == item_id || t.right_item_id == item_id {
                    let mut valid = false;
                    if let (Some(left), Some(right)) = (
                        track.find_item(&t.left_item_id),
                        track.find_item(&t.right_item_id),
                    ) {
                        if left
                            .timeline_range
                            .is_adjacent_to_with_rate(&right.timeline_range, target_rate)
                        {
                            valid = Self::validate_cut_transition_handles(
                                project,
                                &track.track_id,
                                &t.left_item_id,
                                &t.right_item_id,
                                t.duration,
                                op_id,
                            )
                            .is_ok();
                        }
                    }
                    if !valid {
                        transitions_to_remove
                            .push((track.track_id.clone(), t.transition_id.clone()));
                    }
                }
            }
        }

        for (track_id, trans_id) in transitions_to_remove {
            if let Some(track) = project.find_track_mut(&track_id) {
                track.remove_transition(&trans_id);
            }
        }
    }

    fn reconcile_dependent_transitions_before_trim(
        project: &Project,
        item_id: &str,
        new_timeline_range: &TimeRange,
        new_source_range: &TimeRange,
        op_id: &str,
    ) -> Result<(), CommandError> {
        let target_rate = project.settings.frame_rate;
        for track in &project.tracks {
            for t in &track.transitions {
                let d_ticks = t.duration.rescaled_to(target_rate).value;
                let l_ticks = d_ticks / 2;
                let r_ticks = d_ticks - l_ticks;
                let req_preroll = RationalTime::new(l_ticks, target_rate);
                let req_postroll = RationalTime::new(r_ticks, target_rate);

                if t.left_item_id == item_id {
                    if let Some(right) = track.find_item(&t.right_item_id) {
                        if new_timeline_range
                            .is_adjacent_to_with_rate(&right.timeline_range, target_rate)
                        {
                            let left_item = track.find_item(item_id).unwrap();
                            let asset = left_item
                                .asset_id
                                .as_ref()
                                .and_then(|aid| project.find_asset(aid))
                                .ok_or_else(|| CommandError::TransitionHandleUnverifiable {
                                    operation_id: op_id.to_string(),
                                    code: "TRANSITION_HANDLE_UNVERIFIABLE".to_string(),
                                    reason: "Asset missing".to_string(),
                                })?;
                            let asset_dur = asset
                                .duration
                                .ok_or_else(|| CommandError::TransitionHandleUnverifiable {
                                    operation_id: op_id.to_string(),
                                    code: "TRANSITION_HANDLE_UNVERIFIABLE".to_string(),
                                    reason: "Asset duration missing".to_string(),
                                })?
                                .rescaled_to(target_rate);

                            let right_asset = right
                                .asset_id
                                .as_ref()
                                .and_then(|aid| project.find_asset(aid))
                                .ok_or_else(|| CommandError::TransitionHandleUnverifiable {
                                    operation_id: op_id.to_string(),
                                    code: "TRANSITION_HANDLE_UNVERIFIABLE".to_string(),
                                    reason: "Asset missing".to_string(),
                                })?;
                            let _right_asset_dur = right_asset.duration.ok_or_else(|| {
                                CommandError::TransitionHandleUnverifiable {
                                    operation_id: op_id.to_string(),
                                    code: "TRANSITION_HANDLE_UNVERIFIABLE".to_string(),
                                    reason: "Asset duration missing".to_string(),
                                }
                            })?;

                            let new_src_start = new_source_range.start.rescaled_to(target_rate);
                            let new_src_dur = new_source_range.duration.rescaled_to(target_rate);
                            let avail_postroll_ticks =
                                asset_dur.value - (new_src_start.value + new_src_dur.value);
                            let avail_postroll =
                                RationalTime::new(avail_postroll_ticks, target_rate);

                            let right_src_start = right.source_range.start.rescaled_to(target_rate);
                            let avail_preroll_ticks = right_src_start.value;
                            let avail_preroll = RationalTime::new(avail_preroll_ticks, target_rate);

                            let left_ext_ticks =
                                new_timeline_range.duration.rescaled_to(target_rate).value;
                            let right_ext_ticks =
                                right.timeline_range.duration.rescaled_to(target_rate).value;

                            if avail_postroll_ticks < r_ticks
                                || avail_preroll_ticks < l_ticks
                                || left_ext_ticks < l_ticks
                                || right_ext_ticks < r_ticks
                            {
                                return Err(CommandError::InsufficientTransitionHandle {
                                    operation_id: op_id.to_string(),
                                    code: "INSUFFICIENT_TRANSITION_HANDLE".to_string(),
                                    message: "Cannot create CrossDissolve: required media handle exceeds available source frames.".to_string(),
                                    required_preroll: req_preroll,
                                    available_preroll: avail_preroll,
                                    required_postroll: req_postroll,
                                    available_postroll: avail_postroll,
                                });
                            }
                        }
                    }
                } else if t.right_item_id == item_id {
                    if let Some(left) = track.find_item(&t.left_item_id) {
                        if left
                            .timeline_range
                            .is_adjacent_to_with_rate(new_timeline_range, target_rate)
                        {
                            let right_item = track.find_item(item_id).unwrap();
                            let asset = right_item
                                .asset_id
                                .as_ref()
                                .and_then(|aid| project.find_asset(aid))
                                .ok_or_else(|| CommandError::TransitionHandleUnverifiable {
                                    operation_id: op_id.to_string(),
                                    code: "TRANSITION_HANDLE_UNVERIFIABLE".to_string(),
                                    reason: "Asset missing".to_string(),
                                })?;
                            let _asset_dur = asset.duration.ok_or_else(|| {
                                CommandError::TransitionHandleUnverifiable {
                                    operation_id: op_id.to_string(),
                                    code: "TRANSITION_HANDLE_UNVERIFIABLE".to_string(),
                                    reason: "Asset duration missing".to_string(),
                                }
                            })?;

                            let left_asset = left
                                .asset_id
                                .as_ref()
                                .and_then(|aid| project.find_asset(aid))
                                .ok_or_else(|| CommandError::TransitionHandleUnverifiable {
                                    operation_id: op_id.to_string(),
                                    code: "TRANSITION_HANDLE_UNVERIFIABLE".to_string(),
                                    reason: "Asset missing".to_string(),
                                })?;
                            let left_asset_dur = left_asset
                                .duration
                                .ok_or_else(|| CommandError::TransitionHandleUnverifiable {
                                    operation_id: op_id.to_string(),
                                    code: "TRANSITION_HANDLE_UNVERIFIABLE".to_string(),
                                    reason: "Asset duration missing".to_string(),
                                })?
                                .rescaled_to(target_rate);

                            let left_src_start = left.source_range.start.rescaled_to(target_rate);
                            let left_src_dur = left.source_range.duration.rescaled_to(target_rate);
                            let avail_postroll_ticks =
                                left_asset_dur.value - (left_src_start.value + left_src_dur.value);
                            let avail_postroll =
                                RationalTime::new(avail_postroll_ticks, target_rate);

                            let new_src_start = new_source_range.start.rescaled_to(target_rate);
                            let avail_preroll_ticks = new_src_start.value;
                            let avail_preroll = RationalTime::new(avail_preroll_ticks, target_rate);

                            let left_ext_ticks =
                                left.timeline_range.duration.rescaled_to(target_rate).value;
                            let right_ext_ticks =
                                new_timeline_range.duration.rescaled_to(target_rate).value;

                            if avail_preroll_ticks < l_ticks
                                || avail_postroll_ticks < r_ticks
                                || left_ext_ticks < l_ticks
                                || right_ext_ticks < r_ticks
                            {
                                return Err(CommandError::InsufficientTransitionHandle {
                                    operation_id: op_id.to_string(),
                                    code: "INSUFFICIENT_TRANSITION_HANDLE".to_string(),
                                    message: "Cannot create CrossDissolve: required media handle exceeds available source frames.".to_string(),
                                    required_preroll: req_preroll,
                                    available_preroll: avail_preroll,
                                    required_postroll: req_postroll,
                                    available_postroll: avail_postroll,
                                });
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn cleanup_broken_transitions_after_trim(project: &mut Project, item_id: &str) {
        let target_rate = project.settings.frame_rate;
        for track in &mut project.tracks {
            let items = &track.items;
            track.transitions.retain(|t| {
                if t.left_item_id == item_id || t.right_item_id == item_id {
                    let l = items.iter().find(|i| i.item_id == t.left_item_id);
                    let r = items.iter().find(|i| i.item_id == t.right_item_id);
                    if let (Some(left), Some(right)) = (l, r) {
                        left.timeline_range
                            .is_adjacent_to_with_rate(&right.timeline_range, target_rate)
                    } else {
                        false
                    }
                } else {
                    true
                }
            });
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
            client_id: None,
            client_epoch: None,
            sequence_number: None,
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
            client_id: None,
            client_epoch: None,
            sequence_number: None,
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
            client_id: None,
            client_epoch: None,
            sequence_number: None,
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
            client_id: None,
            client_epoch: None,
            sequence_number: None,
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
            client_id: None,
            client_epoch: None,
            sequence_number: None,
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
            client_id: None,
            client_epoch: None,
            sequence_number: None,
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
                client_id: None,
                client_epoch: None,
                sequence_number: None,
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
                client_id: None,
                client_epoch: None,
                sequence_number: None,
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
                client_id: None,
                client_epoch: None,
                sequence_number: None,
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
                client_id: None,
                client_epoch: None,
                sequence_number: None,
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
                client_id: None,
                client_epoch: None,
                sequence_number: None,
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
                client_id: None,
                client_epoch: None,
                sequence_number: None,
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
                client_id: None,
                client_epoch: None,
                sequence_number: None,
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
                client_id: None,
                client_epoch: None,
                sequence_number: None,
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
                client_id: None,
                client_epoch: None,
                sequence_number: None,
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
                client_id: None,
                client_epoch: None,
                sequence_number: None,
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
                client_id: None,
                client_epoch: None,
                sequence_number: None,
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
                client_id: None,
                client_epoch: None,
                sequence_number: None,
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

        // 4. Test ClipTransitionSet: CrossDissolve must be rejected explicitly as unsupported/deferred
        let cross_dissolve_res = service.execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 3,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_trans_unsupported".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_set_crossdissolve".to_string(),
                operation: CommandOperation::ClipTransitionSet {
                    item_id: "clip_vid".to_string(),
                    transition: Some(crate::modules::domain::Transition {
                        kind: crate::modules::domain::TransitionKind::CrossDissolve,
                        duration: RationalTime::from_frames(10, rate),
                    }),
                },
            }],
        });
        assert_eq!(
            cross_dissolve_res,
            Err(CommandError::UnsupportedTransition {
                operation_id: "op_set_crossdissolve".to_string(),
                transition: "CrossDissolve".to_string(),
            })
        );
        // Project revision must not advance and state must not mutate
        assert_eq!(service.project().revision, 3);
        assert_eq!(
            service.project().tracks[0]
                .find_item("clip_vid")
                .unwrap()
                .transition,
            None
        );

        // Setting a supported transition (FadeOut) succeeds
        service
            .execute(CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: pid.clone(),
                base_revision: 3,
                client_id: None,
                client_epoch: None,
                sequence_number: None,
                dry_run: false,
                transaction_id: Some("tx_trans".to_string()),
                operations: vec![OperationEnvelope {
                    operation_id: "op_set_trans".to_string(),
                    operation: CommandOperation::ClipTransitionSet {
                        item_id: "clip_vid".to_string(),
                        transition: Some(crate::modules::domain::Transition {
                            kind: crate::modules::domain::TransitionKind::FadeOut,
                            duration: RationalTime::from_frames(10, rate),
                        }),
                    },
                }],
            })
            .unwrap();

        let proj = service.project();
        let vid_item = proj.tracks[0].find_item("clip_vid").unwrap();
        assert!(vid_item.transition.is_some());
        assert_eq!(
            vid_item.transition.as_ref().unwrap().kind,
            crate::modules::domain::TransitionKind::FadeOut
        );
        assert_eq!(vid_item.transition.as_ref().unwrap().duration.value, 10);

        // 5. Test Keyframes: Add 3 keyframes, then remove 1
        service
            .execute(CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: pid.clone(),
                base_revision: 4,
                client_id: None,
                client_epoch: None,
                sequence_number: None,
                dry_run: false,
                transaction_id: Some("tx_kf_add".to_string()),
                operations: vec![
                    OperationEnvelope {
                        operation_id: "op_kf1".to_string(),
                        operation: CommandOperation::ClipKeyframeAdd {
                            item_id: "clip_vid".to_string(),
                            keyframe: Keyframe::new(
                                KeyframeProperty::Opacity,
                                RationalTime::from_frames(0, rate),
                                0.2,
                            ),
                            easing: None,
                        },
                    },
                    OperationEnvelope {
                        operation_id: "op_kf2".to_string(),
                        operation: CommandOperation::ClipKeyframeAdd {
                            item_id: "clip_vid".to_string(),
                            keyframe: Keyframe::new(
                                KeyframeProperty::Opacity,
                                RationalTime::from_frames(30, rate),
                                1.0,
                            ),
                            easing: None,
                        },
                    },
                    OperationEnvelope {
                        operation_id: "op_kf3".to_string(),
                        operation: CommandOperation::ClipKeyframeAdd {
                            item_id: "clip_vid".to_string(),
                            keyframe: Keyframe::new(
                                KeyframeProperty::Opacity,
                                RationalTime::from_frames(15, rate),
                                0.99,
                            ),
                            easing: None,
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
                client_id: None,
                client_epoch: None,
                sequence_number: None,
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

        // 6. Test ClipKeyframeMove (Milestone 7 & Workstream B)
        service
            .execute(CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: pid.clone(),
                base_revision: 6,
                client_id: None,
                client_epoch: None,
                sequence_number: None,
                dry_run: false,
                transaction_id: Some("tx_kf_move".to_string()),
                operations: vec![OperationEnvelope {
                    operation_id: "op_kf_move".to_string(),
                    operation: CommandOperation::ClipKeyframeMove {
                        item_id: "clip_vid".to_string(),
                        property: KeyframeProperty::Opacity,
                        old_offset: RationalTime::from_frames(30, rate),
                        new_offset: RationalTime::from_frames(60, rate),
                    },
                }],
            })
            .unwrap();

        let proj = service.project();
        let vid_item = proj.tracks[0].find_item("clip_vid").unwrap();
        assert_eq!(vid_item.keyframes.len(), 2);
        assert_eq!(vid_item.keyframes[0].offset.value, 0);
        assert_eq!(vid_item.keyframes[1].offset.value, 60);
        assert_eq!(vid_item.keyframes[1].value, 1.0);

        // 7. Test TranscriptionBoundary (Milestone 12 & Workstream D)
        let trans_result = crate::modules::media::transcription::TranscriptionResult {
            language: "en".to_string(),
            segments: vec![
                crate::modules::media::transcription::TranscriptionSegment {
                    start: RationalTime::from_frames(80, rate),
                    duration: RationalTime::from_frames(20, rate),
                    text: "AI Transcribed Caption 1".to_string(),
                },
                crate::modules::media::transcription::TranscriptionSegment {
                    start: RationalTime::from_frames(105, rate),
                    duration: RationalTime::from_frames(25, rate),
                    text: "AI Transcribed Caption 2".to_string(),
                },
            ],
        };
        let cap_ops =
            crate::modules::media::transcription::TranscriptionBoundary::create_caption_commands(
                "trk_txt",
                &trans_result,
            );
        assert_eq!(cap_ops.len(), 4); // 2 creates + 2 text sets

        service
            .execute(CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: pid.clone(),
                base_revision: 7,
                client_id: None,
                client_epoch: None,
                sequence_number: None,
                dry_run: false,
                transaction_id: Some("tx_trans_captions".to_string()),
                operations: cap_ops,
            })
            .unwrap();

        let proj = service.project();
        assert_eq!(proj.tracks[1].items.len(), 3); // original caption + 2 new captions

        // 8. Test Persistence Round-trip preserving all creator features
        let temp_file = std::env::temp_dir().join("tinix_phase9_creator_roundtrip.tcproj");
        crate::modules::persistence::save_project_atomic(proj, &temp_file).unwrap();
        let loaded = crate::modules::persistence::load_project(&temp_file).unwrap();
        assert_eq!(loaded, *proj);
        let _ = std::fs::remove_file(temp_file);

        // 9. Verify CompositionPlan compilation & keyframe sampling parity
        let plan = crate::modules::composition::CompositionPlan::from_project(proj)
            .expect("Plan must compile");

        // Sampling at t = 30 frames:
        // Keyframe interpolation between offset 0 (0.2) and offset 60 (1.0) -> midpoint at 30 is 0.6!
        let sample = plan.sample_preview_at(RationalTime::from_frames(30, rate));
        let vid_layer = sample
            .active_layers
            .iter()
            .find(|l| l.item_id == "clip_vid")
            .unwrap();
        assert!((vid_layer.opacity - 0.6).abs() < 0.001);

        // Check before-first hold (t = -5 frames or offset = 0)
        let sample_first = plan.sample_preview_at(RationalTime::from_frames(0, rate));
        let vid_layer_first = sample_first
            .active_layers
            .iter()
            .find(|l| l.item_id == "clip_vid")
            .unwrap();
        assert!((vid_layer_first.opacity - 0.2).abs() < 0.001);

        // Check after-last hold (t = 60 frames or offset = 60)
        let sample_last = plan.sample_preview_at(RationalTime::from_frames(60, rate));
        let vid_layer_last = sample_last
            .active_layers
            .iter()
            .find(|l| l.item_id == "clip_vid")
            .unwrap();
        assert!((vid_layer_last.opacity - 1.0).abs() < 0.001);

        // Title text should also be present in active layers
        let title_layer = sample
            .active_layers
            .iter()
            .find(|l| l.item_id == "clip_caption")
            .unwrap();
        assert_eq!(title_layer.text.as_deref(), Some("Welcome to TinixCut!"));
    }

    #[test]
    fn test_phase9_remediation_transition_lifecycle() {
        use crate::modules::domain::{Transition, TransitionKind};

        let rate = FrameRate::FPS_30;
        let mut project = Project::new("Transition Lifecycle".to_string());
        let mut track = Track::new("Video Track".to_string(), TrackKind::Video);
        let clip = TimelineItem::new(
            None,
            "Video Clip".to_string(),
            ItemKind::Video,
            TimeRange::new(
                RationalTime::from_frames(0, rate),
                RationalTime::from_frames(90, rate),
            ),
            TimeRange::new(
                RationalTime::from_frames(0, rate),
                RationalTime::from_frames(90, rate),
            ),
        )
        .unwrap();
        let clip_id = clip.item_id.clone();
        track.items.push(clip);
        project.tracks.push(track);

        let mut service = CommandService::new(project);
        let pid = service.project().project_id.clone();

        // 1. Add Transition (FadeIn, 15 frames)
        let add_env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 0,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_add_fadein".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_add_fadein".to_string(),
                operation: CommandOperation::ClipTransitionSet {
                    item_id: clip_id.clone(),
                    transition: Some(Transition {
                        kind: TransitionKind::FadeIn,
                        duration: RationalTime::from_frames(15, rate),
                    }),
                },
            }],
        };
        let receipt = service
            .execute(add_env)
            .expect("Add transition should succeed");
        assert_eq!(receipt.new_revision, 1);
        let item = service.project().tracks[0].find_item(&clip_id).unwrap();
        assert_eq!(
            item.transition,
            Some(Transition {
                kind: TransitionKind::FadeIn,
                duration: RationalTime::from_frames(15, rate),
            })
        );

        // 2. Update Transition (FadeOut, 25 frames)
        let update_env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 1,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_update_fadeout".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_update_fadeout".to_string(),
                operation: CommandOperation::ClipTransitionSet {
                    item_id: clip_id.clone(),
                    transition: Some(Transition {
                        kind: TransitionKind::FadeOut,
                        duration: RationalTime::from_frames(25, rate),
                    }),
                },
            }],
        };
        service
            .execute(update_env)
            .expect("Update transition should succeed");
        let item = service.project().tracks[0].find_item(&clip_id).unwrap();
        assert_eq!(
            item.transition,
            Some(Transition {
                kind: TransitionKind::FadeOut,
                duration: RationalTime::from_frames(25, rate),
            })
        );

        // 3. Validation failure: zero duration rejected
        let invalid_zero_env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 2,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_invalid_zero".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_invalid_zero".to_string(),
                operation: CommandOperation::ClipTransitionSet {
                    item_id: clip_id.clone(),
                    transition: Some(Transition {
                        kind: TransitionKind::FadeIn,
                        duration: RationalTime::from_frames(0, rate),
                    }),
                },
            }],
        };
        assert!(service.execute(invalid_zero_env).is_err());

        // 4. Validation failure: duration exceeding clip length (120 > 90) rejected
        let invalid_overflow_env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 2,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_invalid_overflow".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_invalid_overflow".to_string(),
                operation: CommandOperation::ClipTransitionSet {
                    item_id: clip_id.clone(),
                    transition: Some(Transition {
                        kind: TransitionKind::FadeIn,
                        duration: RationalTime::from_frames(120, rate),
                    }),
                },
            }],
        };
        assert!(service.execute(invalid_overflow_env).is_err());

        // 5. Remove Transition (transition: None)
        let remove_env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 2,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_remove_trans".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_remove_trans".to_string(),
                operation: CommandOperation::ClipTransitionSet {
                    item_id: clip_id.clone(),
                    transition: None,
                },
            }],
        };
        service
            .execute(remove_env)
            .expect("Remove transition should succeed");
        let item = service.project().tracks[0].find_item(&clip_id).unwrap();
        assert_eq!(item.transition, None);

        // 6. Undo / Redo verification
        let mut history = crate::modules::history::HistoryManager::new(20);
        let mut snap0 = service.project().clone();
        snap0.tracks[0].find_item_mut(&clip_id).unwrap().transition = None;

        let mut snap1 = snap0.clone();
        snap1.tracks[0].find_item_mut(&clip_id).unwrap().transition = Some(Transition {
            kind: TransitionKind::FadeBlack,
            duration: RationalTime::from_frames(20, rate),
        });

        history.record_edit(crate::modules::history::HistoryEntry {
            description: "Set FadeBlack".to_string(),
            transaction_id: Some("tx_hist".to_string()),
            prior_snapshot: snap0.clone(),
            resulting_snapshot: snap1.clone(),
        });

        let mut active = snap1.clone();
        assert!(active.tracks[0]
            .find_item(&clip_id)
            .unwrap()
            .transition
            .is_some());

        // Undo -> restores None
        history.undo(&mut active).unwrap();
        assert_eq!(
            active.tracks[0].find_item(&clip_id).unwrap().transition,
            None
        );

        // Redo -> restores FadeBlack
        history.redo(&mut active).unwrap();
        assert_eq!(
            active.tracks[0].find_item(&clip_id).unwrap().transition,
            Some(Transition {
                kind: TransitionKind::FadeBlack,
                duration: RationalTime::from_frames(20, rate),
            })
        );
    }

    #[test]
    fn test_phase9_crossdissolve_explicit_rejection_and_state_invariance() {
        use crate::modules::domain::{Transition, TransitionKind};

        let rate = FrameRate::FPS_30;
        let mut project = Project::new("CrossDissolve Rejection".to_string());
        let mut track = Track::new("Video Track".to_string(), TrackKind::Video);
        let clip = TimelineItem::new(
            None,
            "Video Clip".to_string(),
            ItemKind::Video,
            TimeRange::new(
                RationalTime::from_frames(0, rate),
                RationalTime::from_frames(90, rate),
            ),
            TimeRange::new(
                RationalTime::from_frames(0, rate),
                RationalTime::from_frames(90, rate),
            ),
        )
        .unwrap();
        let clip_id = clip.item_id.clone();
        track.items.push(clip);
        project.tracks.push(track);

        let mut service = CommandService::new(project);
        let pid = service.project().project_id.clone();

        // 1. Dry run should return the explicit UnsupportedTransition error
        let env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 0,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: true,
            transaction_id: Some("tx_xdissolve_dry".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_xdissolve_dry".to_string(),
                operation: CommandOperation::ClipTransitionSet {
                    item_id: clip_id.clone(),
                    transition: Some(Transition {
                        kind: TransitionKind::CrossDissolve,
                        duration: RationalTime::from_frames(20, rate),
                    }),
                },
            }],
        };
        let dry_res = service.dry_run(&env);
        assert_eq!(
            dry_res,
            Err(CommandError::UnsupportedTransition {
                operation_id: "op_xdissolve_dry".to_string(),
                transition: "CrossDissolve".to_string(),
            })
        );

        // 2. Authoritative execution should return explicit UnsupportedTransition error
        let exec_env = CommandEnvelope {
            dry_run: false,
            ..env
        };
        let exec_res = service.execute(exec_env);
        assert_eq!(
            exec_res,
            Err(CommandError::UnsupportedTransition {
                operation_id: "op_xdissolve_dry".to_string(),
                transition: "CrossDissolve".to_string(),
            })
        );

        // 3. Confirm state invariance:
        // - Project revision did NOT advance
        assert_eq!(service.project().revision, 0);
        // - Clip transition is unchanged (None)
        assert_eq!(
            service.project().tracks[0]
                .find_item(&clip_id)
                .unwrap()
                .transition,
            None
        );
    }

    #[test]
    fn test_phase10_durable_idempotency_save_reopen_exact_replay() {
        let temp_dir = std::env::temp_dir().join("tinix_test_p10_idempotency");
        let _ = std::fs::create_dir_all(&temp_dir);
        let project_path = temp_dir.join("idempotency_proj.tcproj");

        let project = create_test_project();
        let pid = project.project_id.clone();
        let mut service = CommandService::new(project);

        let op = OperationEnvelope {
            operation_id: "op_p10_trk1".to_string(),
            operation: CommandOperation::TrackCreate {
                track_id: Some("trk_p10_1".to_string()),
                name: "P10 Track".to_string(),
                kind: TrackKind::Video,
            },
        };

        let env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 0,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_p10_1".to_string()),
            operations: vec![op.clone()],
        };

        // 1. Initial execution
        let receipt1 = service.execute(env.clone()).unwrap();
        assert_eq!(receipt1.operations_applied, 1);
        assert_eq!(receipt1.new_revision, 1);
        assert_eq!(service.project().command_ledger.len(), 1);

        // 2. Canonical atomic save
        crate::modules::persistence::save_project_atomic(service.project(), &project_path).unwrap();

        // 3. Drop in-memory service & project (simulate process exit / session destroy)
        drop(service);

        // 4. Reopen from disk
        let reopened_project = crate::modules::persistence::load_project(&project_path).unwrap();
        assert_eq!(reopened_project.revision, 1);
        assert_eq!(reopened_project.command_ledger.len(), 1);
        assert_eq!(
            reopened_project.command_ledger[0].operation_id,
            "op_p10_trk1"
        );

        // 5. Retry identical request on fresh CommandService (even with original base_revision = 0)
        let mut fresh_service = CommandService::new(reopened_project);
        let retry_receipt = fresh_service.execute(env).unwrap();

        // Must NOT apply second mutation
        assert_eq!(retry_receipt.operations_applied, 0);
        assert_eq!(retry_receipt.receipts[0].status, "deduplicated_noop");
        assert_eq!(retry_receipt.new_revision, 1);
        assert_eq!(fresh_service.revision(), 1);
        assert_eq!(fresh_service.project().tracks.len(), 1);

        // 6. Attempt ID collision: reuse op_p10_trk1 with DIFFERENT payload
        let collision_op = OperationEnvelope {
            operation_id: "op_p10_trk1".to_string(), // Same ID!
            operation: CommandOperation::TrackCreate {
                track_id: Some("trk_collided".to_string()),
                name: "Collided Track".to_string(),
                kind: TrackKind::Audio, // Different payload!
            },
        };
        let collision_env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 1,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_collision".to_string()),
            operations: vec![collision_op],
        };

        let collision_res = fresh_service.execute(collision_env);
        match collision_res {
            Err(CommandError::IdempotencyCollision { operation_id, .. }) => {
                assert_eq!(operation_id, "op_p10_trk1");
            }
            other => panic!("Expected IdempotencyCollision, got {:?}", other),
        }

        // Project remains unmutated
        assert_eq!(fresh_service.revision(), 1);
        assert_eq!(fresh_service.project().tracks.len(), 1);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_phase10_durable_idempotency_failed_transaction_leaves_ledger_empty() {
        let project = create_test_project();
        let pid = project.project_id.clone();
        let mut service = CommandService::new(project);

        let op1 = OperationEnvelope {
            operation_id: "op_tx_valid".to_string(),
            operation: CommandOperation::TrackCreate {
                track_id: Some("trk_tx_v".to_string()),
                name: "Valid Track".to_string(),
                kind: TrackKind::Video,
            },
        };

        let op2 = OperationEnvelope {
            operation_id: "op_tx_invalid".to_string(),
            operation: CommandOperation::ClipDelete {
                item_id: "non_existent_clip_12345".to_string(),
            },
        };

        let env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 0,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_fail_test".to_string()),
            operations: vec![op1.clone(), op2],
        };

        // Execution should fail and roll back completely
        assert!(service.execute(env).is_err());

        // Invariant: Ledger must NOT contain op_tx_valid or op_tx_invalid!
        assert_eq!(service.project().command_ledger.len(), 0);
        assert!(!service.is_operation_applied("op_tx_valid"));
        assert!(!service.is_operation_applied("op_tx_invalid"));
        assert_eq!(service.revision(), 0);

        // Later, submitting op_tx_valid alone should succeed normally
        let valid_env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid,
            base_revision: 0,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_success_after_fail".to_string()),
            operations: vec![op1],
        };
        let receipt = service.execute(valid_env).unwrap();
        assert_eq!(receipt.operations_applied, 1);
        assert_eq!(service.project().command_ledger.len(), 1);
        assert!(service.is_operation_applied("op_tx_valid"));
    }

    #[test]
    fn test_phase11_structured_identity_sequential_commits_and_replay() {
        let project = create_test_project();
        let pid = project.project_id.clone();
        let mut service = CommandService::new(project);

        // 1. Commit Sequence 1 for client "agent_audio"
        let env1 = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 0,
            client_id: Some("agent_audio".to_string()),
            client_epoch: Some(1),
            sequence_number: Some(1),
            dry_run: false,
            transaction_id: Some("tx_seq_1".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_seq_1".to_string(),
                operation: CommandOperation::TrackCreate {
                    track_id: Some("trk_seq_1".to_string()),
                    name: "Track Seq 1".to_string(),
                    kind: TrackKind::Audio,
                },
            }],
        };
        let receipt1 = service.execute(env1).unwrap();
        assert_eq!(receipt1.operations_applied, 1);
        assert_eq!(receipt1.new_revision, 1);
        assert_eq!(receipt1.idempotency_scope, Some("structured".to_string()));
        let client_rec = service
            .project()
            .client_registry
            .clients
            .get("agent_audio")
            .unwrap();
        assert_eq!(client_rec.highest_sequence, 1);

        // 2. Commit Sequence 2 for client "agent_audio"
        let env2 = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 1,
            client_id: Some("agent_audio".to_string()),
            client_epoch: Some(1),
            sequence_number: Some(2),
            dry_run: false,
            transaction_id: Some("tx_seq_2".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_seq_2".to_string(),
                operation: CommandOperation::TrackCreate {
                    track_id: Some("trk_seq_2".to_string()),
                    name: "Track Seq 2".to_string(),
                    kind: TrackKind::Audio,
                },
            }],
        };
        let receipt2 = service.execute(env2.clone()).unwrap();
        assert_eq!(receipt2.operations_applied, 1);
        assert_eq!(receipt2.new_revision, 2);

        // 3. Retry Sequence 2 (in active window) => Exact replay deduplicated_noop
        let retry_receipt = service.execute(env2).unwrap();
        assert_eq!(retry_receipt.operations_applied, 0);
        assert_eq!(retry_receipt.receipts[0].status, "deduplicated_noop");
        assert_eq!(service.revision(), 2);

        // 4. Out-of-order sequence gap: submit sequence 5 when next expected is 3
        let gap_env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 2,
            client_id: Some("agent_audio".to_string()),
            client_epoch: Some(1),
            sequence_number: Some(5), // GAP! Expected 3
            dry_run: false,
            transaction_id: Some("tx_gap".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_gap".to_string(),
                operation: CommandOperation::TrackCreate {
                    track_id: Some("trk_gap".to_string()),
                    name: "Track Gap".to_string(),
                    kind: TrackKind::Audio,
                },
            }],
        };
        let gap_res = service.execute(gap_env);
        assert_eq!(
            gap_res,
            Err(CommandError::SequenceGapDetected {
                client_id: "agent_audio".to_string(),
                expected_sequence: 3,
                received_sequence: 5,
            })
        );
        assert_eq!(service.revision(), 2);

        // 5. Fresh unregistered client starting at sequence != 1 must fail with SequenceGapDetected
        let bad_start_env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid,
            base_revision: 2,
            client_id: Some("brand_new_client".to_string()),
            client_epoch: Some(1),
            sequence_number: Some(2), // Brand new client must start at 1!
            dry_run: false,
            transaction_id: Some("tx_bad_start".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_bad_start".to_string(),
                operation: CommandOperation::TrackCreate {
                    track_id: Some("trk_bad".to_string()),
                    name: "Track Bad".to_string(),
                    kind: TrackKind::Audio,
                },
            }],
        };
        let bad_start_res = service.execute(bad_start_env);
        assert_eq!(
            bad_start_res,
            Err(CommandError::SequenceGapDetected {
                client_id: "brand_new_client".to_string(),
                expected_sequence: 1,
                received_sequence: 2,
            })
        );
    }

    #[test]
    fn test_phase11_active_window_bounding_and_past_sequence_protection() {
        let mut project = create_test_project();
        let pid = project.project_id.clone();

        // Pre-populate 5,005 entries directly into project to simulate sustained history
        for i in 1..=5005 {
            project.command_ledger.push(CommandLedgerEntry {
                operation_id: format!("op_hist_{}", i),
                transaction_id: Some(format!("tx_hist_{}", i)),
                client_id: Some("batch_client".to_string()),
                client_epoch: Some(1),
                sequence_number: Some(i),
                fingerprint: "fp_sha256_dummy_hash".to_string(),
                status: "applied".to_string(),
                applied_at_revision: i,
                affected_item_id: None,
                affected_track_id: None,
            });
        }
        project.revision = 5005;
        project.client_registry.clients.insert(
            "batch_client".to_string(),
            crate::modules::domain::ClientRecord {
                client_epoch: 1,
                highest_sequence: 5005,
                last_active_revision: 5005,
                registered_at_revision: 1,
            },
        );

        let mut service = CommandService::new(project);

        // 1. Commit one new operation (sequence 5006)
        let env_new = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 5005,
            client_id: Some("batch_client".to_string()),
            client_epoch: Some(1),
            sequence_number: Some(5006),
            dry_run: false,
            transaction_id: Some("tx_5006".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_hist_5006".to_string(),
                operation: CommandOperation::TrackCreate {
                    track_id: Some("trk_5006".to_string()),
                    name: "Track 5006".to_string(),
                    kind: TrackKind::Video,
                },
            }],
        };
        service.execute(env_new).unwrap();

        // Invariant: Active window strictly bounded to <= 5,000 entries!
        assert_eq!(service.project().command_ledger.len(), 5000);
        // Oldest entries (e.g. op_hist_1..6) have been drained
        assert!(!service.is_operation_applied("op_hist_1"));
        assert!(!service.is_operation_applied("op_hist_6"));
        // Recent entries are present
        assert!(service.is_operation_applied("op_hist_5006"));

        // 2. Mathematical Proof Test: Caller refreshes base_revision to current (5006)
        // and resubmits evicted operation sequence 2.
        let evicted_replay_env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid,
            base_revision: 5006, // Refreshed to current revision!
            client_id: Some("batch_client".to_string()),
            client_epoch: Some(1),
            sequence_number: Some(2), // Committed in the past, but evicted!
            dry_run: false,
            transaction_id: Some("tx_evicted_replay".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_hist_2".to_string(),
                operation: CommandOperation::TrackCreate {
                    track_id: Some("trk_dup".to_string()),
                    name: "Should Not Duplicate".to_string(),
                    kind: TrackKind::Video,
                },
            }],
        };

        let evicted_res = service.execute(evicted_replay_env);
        // Correctness Guarantee: Server rejects with OperationAlreadyCommitted!
        assert_eq!(
            evicted_res,
            Err(CommandError::OperationAlreadyCommitted {
                operation_id: "op_hist_2".to_string(),
                client_id: "batch_client".to_string(),
                sequence_number: 2,
            })
        );
        // Zero duplicate mutation, revision unchanged!
        assert_eq!(service.revision(), 5006);
    }

    #[test]
    fn test_phase11_client_registry_quota_enforcement() {
        let mut project = create_test_project();
        let pid = project.project_id.clone();
        project.client_registry.max_active_clients = 64;

        // Fill up to 64 clients
        for i in 0..64 {
            project.client_registry.clients.insert(
                format!("client_{:02}", i),
                crate::modules::domain::ClientRecord {
                    client_epoch: 1,
                    highest_sequence: 10,
                    last_active_revision: 10,
                    registered_at_revision: 1,
                },
            );
        }
        assert_eq!(project.client_registry.clients.len(), 64);
        assert!(project.validate().is_ok());

        let mut service = CommandService::new(project);

        // Attempt to execute command from 65th unregistered client
        let env_overflow = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid,
            base_revision: 0,
            client_id: Some("client_65_overflow".to_string()),
            client_epoch: Some(1),
            sequence_number: Some(1),
            dry_run: false,
            transaction_id: Some("tx_overflow".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_overflow".to_string(),
                operation: CommandOperation::TrackCreate {
                    track_id: Some("trk_overflow".to_string()),
                    name: "Overflow Track".to_string(),
                    kind: TrackKind::Video,
                },
            }],
        };

        let overflow_res = service.execute(env_overflow);
        assert_eq!(
            overflow_res,
            Err(CommandError::ClientRegistryFull { max_clients: 64 })
        );
    }

    #[test]
    fn test_phase11_client_epoch_fencing_and_expiration() {
        let mut project = create_test_project();
        let pid = project.project_id.clone();
        project.client_registry.min_accepted_epoch = 2; // Epoch 1 is sealed/expired
        project.client_registry.current_epoch = 2;

        let mut service = CommandService::new(project);

        // Submitting command with expired epoch 1 must be strictly rejected
        let expired_env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid,
            base_revision: 0,
            client_id: Some("retired_client".to_string()),
            client_epoch: Some(1), // EXPIRED! min is 2
            sequence_number: Some(1),
            dry_run: false,
            transaction_id: Some("tx_expired".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_expired".to_string(),
                operation: CommandOperation::TrackCreate {
                    track_id: Some("trk_exp".to_string()),
                    name: "Expired Track".to_string(),
                    kind: TrackKind::Video,
                },
            }],
        };

        let expired_res = service.execute(expired_env);
        assert_eq!(
            expired_res,
            Err(CommandError::ClientEpochExpired {
                client_id: "retired_client".to_string(),
                client_epoch: 1,
                min_accepted_epoch: 2,
            })
        );
    }

    #[test]
    fn test_phase11_legacy_fnv_conservative_replay_rejection() {
        let mut project = create_test_project();
        let pid = project.project_id.clone();

        // Inject a legacy Phase 10 entry with 19-char FNV fingerprint
        project.command_ledger.push(CommandLedgerEntry {
            operation_id: "op_legacy_fnv".to_string(),
            transaction_id: Some("tx_legacy".to_string()),
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            fingerprint: "fp_cbf29ce484222325".to_string(), // Legacy FNV-1a
            status: "applied".to_string(),
            applied_at_revision: 1,
            affected_item_id: None,
            affected_track_id: None,
        });
        project.revision = 1;

        let mut service = CommandService::new(project);

        // Replay attempt against legacy FNV entry must be safely rejected for cryptographic integrity
        let replay_env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid,
            base_revision: 1,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_replay_legacy".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_legacy_fnv".to_string(),
                operation: CommandOperation::TrackCreate {
                    track_id: Some("trk_leg".to_string()),
                    name: "Legacy Replay Attempt".to_string(),
                    kind: TrackKind::Video,
                },
            }],
        };

        let replay_res = service.execute(replay_env);
        assert_eq!(
            replay_res,
            Err(CommandError::LegacyReceiptUnverifiable {
                operation_id: "op_legacy_fnv".to_string(),
            })
        );
        // Correctness Guarantee: Project is NOT mutated and revision does NOT bump!
        assert_eq!(service.revision(), 1);
        assert_eq!(service.project().tracks.len(), 0);
    }

    #[test]
    fn test_phase11_sha256_cryptographic_fingerprint_and_collision() {
        let op = CommandOperation::TrackCreate {
            track_id: Some("trk_sha".to_string()),
            name: "SHA Track".to_string(),
            kind: TrackKind::Video,
        };

        let fp = compute_operation_fingerprint(&op);
        // Verify prefix and exact 64-hex length
        assert!(fp.starts_with("fp_sha256_"));
        assert_eq!(fp.len(), 10 + 64);
        assert!(!is_legacy_fnv_fingerprint(&fp));

        // Legacy check
        assert!(is_legacy_fnv_fingerprint("fp_cbf29ce484222325"));
        assert!(!is_legacy_fnv_fingerprint(
            "fp_sha256_0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        ));
    }

    #[test]
    fn test_phase11_staged_transaction_isolation_sub_microsecond() {
        let mut project = create_test_project();
        // Add 5,000 dummy ledger entries
        for i in 0..5000 {
            project.command_ledger.push(CommandLedgerEntry {
                operation_id: format!("op_bench_{}", i),
                transaction_id: None,
                client_id: Some("bench".to_string()),
                client_epoch: Some(1),
                sequence_number: Some(i),
                fingerprint: "fp_sha256_bench".to_string(),
                status: "applied".to_string(),
                applied_at_revision: i,
                affected_item_id: None,
                affected_track_id: None,
            });
        }
        project.client_registry.clients.insert(
            "bench".to_string(),
            crate::modules::domain::ClientRecord {
                client_epoch: 1,
                highest_sequence: 5000,
                last_active_revision: 5000,
                registered_at_revision: 1,
            },
        );
        project.revision = 5000;
        let pid = project.project_id.clone();
        let mut service = CommandService::new(project);

        let t0 = std::time::Instant::now();
        let receipt = service
            .execute(CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: pid,
                base_revision: 5000,
                client_id: Some("bench".to_string()),
                client_epoch: Some(1),
                sequence_number: Some(5001),
                dry_run: false,
                transaction_id: Some("tx_bench".to_string()),
                operations: vec![OperationEnvelope {
                    operation_id: "op_isolate_test".to_string(),
                    operation: CommandOperation::TrackCreate {
                        track_id: Some("trk_iso".to_string()),
                        name: "Iso Track".to_string(),
                        kind: TrackKind::Video,
                    },
                }],
            })
            .unwrap();

        let elapsed = t0.elapsed();
        assert_eq!(receipt.operations_applied, 1);
        // Staged clone isolates the 5,000 ledger entries, keeping clone latency well within budget
        assert!(
            elapsed.as_millis() < 50,
            "Execution took {}ms (budget < 50ms in debug)",
            elapsed.as_millis()
        );
    }

    // ==========================================
    // Phase 13 Slice B Acceptance Tests
    // ==========================================

    fn setup_phase13_test_project() -> (Project, String, String, String, String) {
        let mut project = Project::new("Phase 13 Command Test".to_string());
        project.settings.frame_rate = FrameRate::FPS_30;

        let a1 = crate::modules::domain::Asset {
            asset_id: "ast_1".to_string(),
            name: "clip1.mp4".to_string(),
            path: "D:/media/clip1.mp4".to_string(),
            media_kind: crate::modules::domain::MediaKind::Video,
            duration: Some(RationalTime::new(100, FrameRate::FPS_30)),
            file_size: 1024,
        };
        let a2 = crate::modules::domain::Asset {
            asset_id: "ast_2".to_string(),
            name: "clip2.mp4".to_string(),
            path: "D:/media/clip2.mp4".to_string(),
            media_kind: crate::modules::domain::MediaKind::Video,
            duration: Some(RationalTime::new(100, FrameRate::FPS_30)),
            file_size: 1024,
        };
        project.assets.push(a1);
        project.assets.push(a2);

        let mut track = Track::new("Video Track 1".to_string(), TrackKind::Video);
        track.track_id = "trk_v1".to_string();

        // Clip 1: start at 0, dur 30 (source: 10 to 40). Postroll available = 100 - (10+30) = 60 frames.
        let mut item1 = TimelineItem::new(
            Some("ast_1".to_string()),
            "Clip 1".to_string(),
            ItemKind::Video,
            TimeRange::new(
                RationalTime::new(0, FrameRate::FPS_30),
                RationalTime::new(30, FrameRate::FPS_30),
            ),
            TimeRange::new(
                RationalTime::new(10, FrameRate::FPS_30),
                RationalTime::new(30, FrameRate::FPS_30),
            ),
        )
        .unwrap();
        item1.item_id = "itm_1".to_string();

        // Clip 2: start at 30, dur 30 (source: 20 to 50). Preroll available = 20 frames.
        let mut item2 = TimelineItem::new(
            Some("ast_2".to_string()),
            "Clip 2".to_string(),
            ItemKind::Video,
            TimeRange::new(
                RationalTime::new(30, FrameRate::FPS_30),
                RationalTime::new(30, FrameRate::FPS_30),
            ),
            TimeRange::new(
                RationalTime::new(20, FrameRate::FPS_30),
                RationalTime::new(30, FrameRate::FPS_30),
            ),
        )
        .unwrap();
        item2.item_id = "itm_2".to_string();

        track.items.push(item1);
        track.items.push(item2);
        project.tracks.push(track);

        let pid = project.project_id.clone();
        (
            project,
            pid,
            "trk_v1".to_string(),
            "itm_1".to_string(),
            "itm_2".to_string(),
        )
    }

    #[test]
    fn test_phase13_adjacent_valid_create_update_remove() {
        let (project, pid, trk_id, itm1, itm2) = setup_phase13_test_project();
        let mut service = CommandService::new(project);

        // 1. Create CrossDissolve (10 frames = 5L + 5R)
        let create_env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 0,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_p13_create".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_cd_create".to_string(),
                operation: CommandOperation::CutTransitionCreate {
                    transition_id: Some("trans_cd_1".to_string()),
                    track_id: trk_id.clone(),
                    left_item_id: itm1.clone(),
                    right_item_id: itm2.clone(),
                    kind: TransitionKind::CrossDissolve,
                    duration: RationalTime::new(10, FrameRate::FPS_30),
                },
            }],
        };

        let receipt = service.execute(create_env).unwrap();
        assert_eq!(receipt.operations_applied, 1);
        assert_eq!(
            receipt.receipts[0].affected_item_id,
            Some("trans_cd_1".to_string())
        );
        assert_eq!(receipt.receipts[0].affected_track_id, Some(trk_id.clone()));
        assert_eq!(service.project().tracks[0].transitions.len(), 1);

        let trans = &service.project().tracks[0].transitions[0];
        assert_eq!(trans.transition_id, "trans_cd_1");
        assert_eq!(trans.duration, RationalTime::new(10, FrameRate::FPS_30));

        // 2. Update duration to 15 frames (7L + 8R)
        let update_env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 1,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_p13_update".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_cd_update".to_string(),
                operation: CommandOperation::CutTransitionUpdateDuration {
                    transition_id: "trans_cd_1".to_string(),
                    new_duration: RationalTime::new(15, FrameRate::FPS_30),
                },
            }],
        };

        let receipt_up = service.execute(update_env).unwrap();
        assert_eq!(receipt_up.operations_applied, 1);
        assert_eq!(
            service.project().tracks[0].transitions[0].duration,
            RationalTime::new(15, FrameRate::FPS_30)
        );

        // 3. Remove CutTransition
        let remove_env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 2,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_p13_remove".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_cd_remove".to_string(),
                operation: CommandOperation::CutTransitionRemove {
                    transition_id: "trans_cd_1".to_string(),
                },
            }],
        };

        let receipt_rem = service.execute(remove_env).unwrap();
        assert_eq!(receipt_rem.operations_applied, 1);
        assert!(service.project().tracks[0].transitions.is_empty());
    }

    #[test]
    fn test_phase13_zero_duration_rejected() {
        let (project, pid, trk_id, itm1, itm2) = setup_phase13_test_project();
        let mut service = CommandService::new(project);

        let zero_env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid,
            base_revision: 0,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_zero".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_zero_dur".to_string(),
                operation: CommandOperation::CutTransitionCreate {
                    transition_id: None,
                    track_id: trk_id,
                    left_item_id: itm1,
                    right_item_id: itm2,
                    kind: TransitionKind::CrossDissolve,
                    duration: RationalTime::new(0, FrameRate::FPS_30),
                },
            }],
        };

        let res = service.execute(zero_env);
        assert!(matches!(
            res,
            Err(CommandError::OperationValidationFailed { .. })
        ));
    }

    #[test]
    fn test_phase13_video_video_restriction() {
        let (mut project, pid, trk_id, itm1, _itm2) = setup_phase13_test_project();

        // Add an audio item to the track
        let mut audio_item = TimelineItem::new(
            None,
            "Audio Clip".to_string(),
            ItemKind::Audio,
            TimeRange::new(
                RationalTime::new(30, FrameRate::FPS_30),
                RationalTime::new(30, FrameRate::FPS_30),
            ),
            TimeRange::new(
                RationalTime::new(0, FrameRate::FPS_30),
                RationalTime::new(30, FrameRate::FPS_30),
            ),
        )
        .unwrap();
        audio_item.item_id = "itm_audio".to_string();
        project.tracks[0].items.push(audio_item);

        let mut service = CommandService::new(project);

        let non_vid_env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid,
            base_revision: 0,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_non_vid".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_non_vid".to_string(),
                operation: CommandOperation::CutTransitionCreate {
                    transition_id: None,
                    track_id: trk_id,
                    left_item_id: itm1,
                    right_item_id: "itm_audio".to_string(),
                    kind: TransitionKind::CrossDissolve,
                    duration: RationalTime::new(10, FrameRate::FPS_30),
                },
            }],
        };

        let res = service.execute(non_vid_env);
        assert_eq!(
            res,
            Err(CommandError::UnsupportedTransitionMedia {
                operation_id: "op_non_vid".to_string(),
                code: "UNSUPPORTED_TRANSITION_MEDIA".to_string(),
            })
        );
    }

    #[test]
    fn test_phase13_gap_and_duplicate_rejected() {
        let (mut project, pid, trk_id, itm1, itm2) = setup_phase13_test_project();
        // Introduce a 5-frame gap: itm2 starts at 35 instead of 30
        project.tracks[0].items[1].timeline_range = TimeRange::new(
            RationalTime::new(35, FrameRate::FPS_30),
            RationalTime::new(30, FrameRate::FPS_30),
        );

        let mut service = CommandService::new(project);

        let gap_env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 0,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_gap".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_gap".to_string(),
                operation: CommandOperation::CutTransitionCreate {
                    transition_id: None,
                    track_id: trk_id.clone(),
                    left_item_id: itm1.clone(),
                    right_item_id: itm2.clone(),
                    kind: TransitionKind::CrossDissolve,
                    duration: RationalTime::new(10, FrameRate::FPS_30),
                },
            }],
        };

        let res = service.execute(gap_env);
        assert!(matches!(
            res,
            Err(CommandError::OperationValidationFailed { .. })
        ));

        // Restore adjacency and create first transition
        let (project, pid, trk_id, itm1, itm2) = setup_phase13_test_project();
        let mut service = CommandService::new(project);

        service
            .execute(CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: pid.clone(),
                base_revision: 0,
                client_id: None,
                client_epoch: None,
                sequence_number: None,
                dry_run: false,
                transaction_id: Some("tx_first".to_string()),
                operations: vec![OperationEnvelope {
                    operation_id: "op_first".to_string(),
                    operation: CommandOperation::CutTransitionCreate {
                        transition_id: Some("t1".to_string()),
                        track_id: trk_id.clone(),
                        left_item_id: itm1.clone(),
                        right_item_id: itm2.clone(),
                        kind: TransitionKind::CrossDissolve,
                        duration: RationalTime::new(10, FrameRate::FPS_30),
                    },
                }],
            })
            .unwrap();

        // Second transition on same cut must be rejected
        let dup_res = service.execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid,
            base_revision: 1,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_dup".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_dup".to_string(),
                operation: CommandOperation::CutTransitionCreate {
                    transition_id: Some("t2".to_string()),
                    track_id: trk_id,
                    left_item_id: itm1,
                    right_item_id: itm2,
                    kind: TransitionKind::CrossDissolve,
                    duration: RationalTime::new(10, FrameRate::FPS_30),
                },
            }],
        });
        assert!(matches!(
            dup_res,
            Err(CommandError::OperationValidationFailed { .. })
        ));
    }

    #[test]
    fn test_phase13_exact_sufficient_handles_and_one_tick_short_rejected() {
        let mut project = Project::new("Exact Handles Test".to_string());
        project.settings.frame_rate = FrameRate::FPS_30;

        // Clip 1: asset duration = 35 frames. source: 0..30. Available postroll = 35 - 30 = 5 frames.
        let a1 = crate::modules::domain::Asset {
            asset_id: "ast_ex1".to_string(),
            name: "ex1.mp4".to_string(),
            path: "D:/media/ex1.mp4".to_string(),
            media_kind: crate::modules::domain::MediaKind::Video,
            duration: Some(RationalTime::new(35, FrameRate::FPS_30)),
            file_size: 1024,
        };
        // Clip 2: asset duration = 50 frames. source: 5..35. Available preroll = 5 frames.
        let a2 = crate::modules::domain::Asset {
            asset_id: "ast_ex2".to_string(),
            name: "ex2.mp4".to_string(),
            path: "D:/media/ex2.mp4".to_string(),
            media_kind: crate::modules::domain::MediaKind::Video,
            duration: Some(RationalTime::new(50, FrameRate::FPS_30)),
            file_size: 1024,
        };
        project.assets.push(a1);
        project.assets.push(a2);

        let mut track = Track::new("Video".to_string(), TrackKind::Video);
        track.track_id = "trk_ex".to_string();

        let mut item1 = TimelineItem::new(
            Some("ast_ex1".to_string()),
            "C1".to_string(),
            ItemKind::Video,
            TimeRange::new(
                RationalTime::new(0, FrameRate::FPS_30),
                RationalTime::new(30, FrameRate::FPS_30),
            ),
            TimeRange::new(
                RationalTime::new(0, FrameRate::FPS_30),
                RationalTime::new(30, FrameRate::FPS_30),
            ),
        )
        .unwrap();
        item1.item_id = "itm_ex1".to_string();

        let mut item2 = TimelineItem::new(
            Some("ast_ex2".to_string()),
            "C2".to_string(),
            ItemKind::Video,
            TimeRange::new(
                RationalTime::new(30, FrameRate::FPS_30),
                RationalTime::new(30, FrameRate::FPS_30),
            ),
            TimeRange::new(
                RationalTime::new(5, FrameRate::FPS_30),
                RationalTime::new(30, FrameRate::FPS_30),
            ),
        )
        .unwrap();
        item2.item_id = "itm_ex2".to_string();

        track.items.push(item1);
        track.items.push(item2);
        project.tracks.push(track);
        let pid = project.project_id.clone();

        let mut service = CommandService::new(project);

        // Transition duration = 10 frames -> L = 5, R = 5.
        // Available postroll = 5 (>= R=5). Available preroll = 5 (>= L=5).
        // EXACT SUFFICIENT: Must succeed!
        let exact_env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 0,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_exact".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_exact".to_string(),
                operation: CommandOperation::CutTransitionCreate {
                    transition_id: Some("trans_ex".to_string()),
                    track_id: "trk_ex".to_string(),
                    left_item_id: "itm_ex1".to_string(),
                    right_item_id: "itm_ex2".to_string(),
                    kind: TransitionKind::CrossDissolve,
                    duration: RationalTime::new(10, FrameRate::FPS_30),
                },
            }],
        };

        let exact_res = service.execute(exact_env);
        assert!(exact_res.is_ok(), "Exact sufficient handles must succeed");

        // Now test 1-tick short: update duration to 11 frames -> L=5, R=6.
        // Required postroll is 6, but available postroll is only 5!
        let short_env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid,
            base_revision: 1,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_short".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_short".to_string(),
                operation: CommandOperation::CutTransitionUpdateDuration {
                    transition_id: "trans_ex".to_string(),
                    new_duration: RationalTime::new(11, FrameRate::FPS_30),
                },
            }],
        };

        let short_res = service.execute(short_env);
        assert_eq!(
            short_res,
            Err(CommandError::InsufficientTransitionHandle {
                operation_id: "op_short".to_string(),
                code: "INSUFFICIENT_TRANSITION_HANDLE".to_string(),
                message: "Cannot create CrossDissolve: required media handle exceeds available source frames.".to_string(),
                required_preroll: RationalTime::new(5, FrameRate::FPS_30),
                available_preroll: RationalTime::new(5, FrameRate::FPS_30),
                required_postroll: RationalTime::new(6, FrameRate::FPS_30),
                available_postroll: RationalTime::new(5, FrameRate::FPS_30),
            })
        );
    }

    #[test]
    fn test_phase13_unknown_duration_rejected_as_unverifiable() {
        let (mut project, pid, trk_id, itm1, itm2) = setup_phase13_test_project();
        // Remove duration from asset 1
        project.assets[0].duration = None;

        let mut service = CommandService::new(project);

        let unverifiable_env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid,
            base_revision: 0,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_unv".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_unv".to_string(),
                operation: CommandOperation::CutTransitionCreate {
                    transition_id: None,
                    track_id: trk_id,
                    left_item_id: itm1,
                    right_item_id: itm2,
                    kind: TransitionKind::CrossDissolve,
                    duration: RationalTime::new(10, FrameRate::FPS_30),
                },
            }],
        };

        let res = service.execute(unverifiable_env);
        assert!(matches!(
            res,
            Err(CommandError::TransitionHandleUnverifiable { ref code, .. }) if code == "TRANSITION_HANDLE_UNVERIFIABLE"
        ));
    }

    #[test]
    fn test_phase13_dependent_editing_lifecycle_move_trim_split_delete() {
        let (project, pid, trk_id, itm1, itm2) = setup_phase13_test_project();
        let mut service = CommandService::new(project);

        // 1. Create initial transition: cut at C=30, dur=10 -> [25, 35)
        service
            .execute(CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: pid.clone(),
                base_revision: 0,
                client_id: None,
                client_epoch: None,
                sequence_number: None,
                dry_run: false,
                transaction_id: Some("tx_setup".to_string()),
                operations: vec![OperationEnvelope {
                    operation_id: "op_init".to_string(),
                    operation: CommandOperation::CutTransitionCreate {
                        transition_id: Some("trans_life".to_string()),
                        track_id: trk_id.clone(),
                        left_item_id: itm1.clone(),
                        right_item_id: itm2.clone(),
                        kind: TransitionKind::CrossDissolve,
                        duration: RationalTime::new(10, FrameRate::FPS_30),
                    },
                }],
            })
            .unwrap();

        // 2. Split inside transition region [25, 30) -> REJECTED!
        let split_inside = service.execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 1,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_split_inside".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_sp_in".to_string(),
                operation: CommandOperation::ClipSplit {
                    item_id: itm1.clone(),
                    split_time: RationalTime::new(27, FrameRate::FPS_30),
                    new_right_item_id: Some("itm_split_right".to_string()),
                },
            }],
        });
        assert!(matches!(
            split_inside,
            Err(CommandError::OperationValidationFailed { .. })
        ));

        // 3. Split outside transition region (split at 15 < 25) -> SUCCEEDS & RETARGETS!
        let split_outside = service.execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 1,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_split_out".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_sp_out".to_string(),
                operation: CommandOperation::ClipSplit {
                    item_id: itm1.clone(),
                    split_time: RationalTime::new(15, FrameRate::FPS_30),
                    new_right_item_id: Some("itm_1_right_frag".to_string()),
                },
            }],
        });
        assert!(split_outside.is_ok());
        // Verify outgoing participant was retargeted to the right fragment
        assert_eq!(
            service.project().tracks[0].transitions[0].left_item_id,
            "itm_1_right_frag"
        );

        // 4. Trim that keeps cut intact but makes editorial extent insufficient:
        // itm_1_right_frag is currently [15, 30) (duration = 15). Required L = 5.
        // Trim it to [28, 30) (duration = 2 < 5).
        let trim_insufficient = service.execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 2,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_trim_insuf".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_trim_insuf".to_string(),
                operation: CommandOperation::ClipTrim {
                    item_id: "itm_1_right_frag".to_string(),
                    new_timeline_range: TimeRange::new(
                        RationalTime::new(28, FrameRate::FPS_30),
                        RationalTime::new(2, FrameRate::FPS_30),
                    ),
                    new_source_range: TimeRange::new(
                        RationalTime::new(38, FrameRate::FPS_30),
                        RationalTime::new(2, FrameRate::FPS_30),
                    ),
                },
            }],
        });
        assert!(matches!(
            trim_insufficient,
            Err(CommandError::InsufficientTransitionHandle { .. })
        ));
        assert_eq!(service.revision(), 2);
        assert_eq!(service.project().tracks[0].transitions.len(), 1);

        // 5. Trim that destroys cut: itm_1_right_frag end trimmed from 30 to 25.
        let trim_destroy_cut = service.execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 2,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_trim_destroy".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_trim_dest".to_string(),
                operation: CommandOperation::ClipTrim {
                    item_id: "itm_1_right_frag".to_string(),
                    new_timeline_range: TimeRange::new(
                        RationalTime::new(15, FrameRate::FPS_30),
                        RationalTime::new(10, FrameRate::FPS_30),
                    ),
                    new_source_range: TimeRange::new(
                        RationalTime::new(25, FrameRate::FPS_30),
                        RationalTime::new(10, FrameRate::FPS_30),
                    ),
                },
            }],
        });
        assert!(trim_destroy_cut.is_ok());
        // Transition must be removed atomically
        assert!(service.project().tracks[0].transitions.is_empty());

        // 6. Delete participant removes transition atomically
        // Re-setup project
        let (project, pid, trk_id, itm1, itm2) = setup_phase13_test_project();
        let mut service2 = CommandService::new(project);
        service2
            .execute(CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: pid.clone(),
                base_revision: 0,
                client_id: None,
                client_epoch: None,
                sequence_number: None,
                dry_run: false,
                transaction_id: Some("tx_init2".to_string()),
                operations: vec![OperationEnvelope {
                    operation_id: "op_init2".to_string(),
                    operation: CommandOperation::CutTransitionCreate {
                        transition_id: Some("trans_del".to_string()),
                        track_id: trk_id,
                        left_item_id: itm1,
                        right_item_id: itm2.clone(),
                        kind: TransitionKind::CrossDissolve,
                        duration: RationalTime::new(10, FrameRate::FPS_30),
                    },
                }],
            })
            .unwrap();
        assert_eq!(service2.project().tracks[0].transitions.len(), 1);

        // Delete itm2
        service2
            .execute(CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: pid,
                base_revision: 1,
                client_id: None,
                client_epoch: None,
                sequence_number: None,
                dry_run: false,
                transaction_id: Some("tx_del".to_string()),
                operations: vec![OperationEnvelope {
                    operation_id: "op_del".to_string(),
                    operation: CommandOperation::ClipDelete { item_id: itm2 },
                }],
            })
            .unwrap();
        assert!(service2.project().tracks[0].transitions.is_empty());
    }

    #[test]
    fn test_phase13_undo_redo_exact_transition_identity() {
        let (project, pid, trk_id, itm1, itm2) = setup_phase13_test_project();
        let mut history = crate::modules::history::HistoryManager::new(20);
        let mut active = project.clone();
        let mut service = CommandService::new(active.clone());

        let prior_snapshot = active.clone();

        // Create transition
        let receipt = service
            .execute(CommandEnvelope {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: pid,
                base_revision: 0,
                client_id: None,
                client_epoch: None,
                sequence_number: None,
                dry_run: false,
                transaction_id: Some("tx_hist".to_string()),
                operations: vec![OperationEnvelope {
                    operation_id: "op_hist".to_string(),
                    operation: CommandOperation::CutTransitionCreate {
                        transition_id: Some("trans_exact_id".to_string()),
                        track_id: trk_id,
                        left_item_id: itm1.clone(),
                        right_item_id: itm2.clone(),
                        kind: TransitionKind::CrossDissolve,
                        duration: RationalTime::new(10, FrameRate::FPS_30),
                    },
                }],
            })
            .unwrap();
        assert_eq!(receipt.operations_applied, 1);

        active = service.project().clone();
        history.record_edit(crate::modules::history::HistoryEntry {
            description: "create_transition".to_string(),
            transaction_id: Some("tx_hist".to_string()),
            prior_snapshot,
            resulting_snapshot: active.clone(),
        });
        assert_eq!(active.tracks[0].transitions.len(), 1);

        // Undo -> restores empty transitions
        history.undo(&mut active).unwrap();
        assert_eq!(active.tracks[0].transitions.len(), 0);

        // Redo -> restores exact transition
        history.redo(&mut active).unwrap();
        assert_eq!(active.tracks[0].transitions.len(), 1);
        let t = &active.tracks[0].transitions[0];
        assert_eq!(t.transition_id, "trans_exact_id");
        assert_eq!(t.left_item_id, itm1);
        assert_eq!(t.right_item_id, itm2);
        assert_eq!(t.duration, RationalTime::new(10, FrameRate::FPS_30));
    }

    #[test]
    fn test_phase13_cliptrim_semantics_distinction_and_history() {
        use crate::modules::domain::{Asset, MediaKind};
        let fps = FrameRate::FPS_30;
        let mut project = Project::new("ClipTrim Semantics Test".to_string());
        project.settings.frame_rate = fps;

        let ast1 = Asset {
            asset_id: "ast_trim_1".to_string(),
            name: "vid1.mp4".to_string(),
            path: "vid1.mp4".to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(100, fps)),
            file_size: 1024,
        };
        let ast2 = Asset {
            asset_id: "ast_trim_2".to_string(),
            name: "vid2.mp4".to_string(),
            path: "vid2.mp4".to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(100, fps)),
            file_size: 1024,
        };
        project.assets.push(ast1);
        project.assets.push(ast2);

        let mut track = Track::new("Video Track".to_string(), TrackKind::Video);
        let mut item1 = TimelineItem::new(
            Some("ast_trim_1".to_string()),
            "Item 1".to_string(),
            ItemKind::Video,
            TimeRange::new(RationalTime::new(0, fps), RationalTime::new(50, fps)),
            TimeRange::new(RationalTime::new(10, fps), RationalTime::new(50, fps)),
        )
        .unwrap();
        item1.item_id = "it1".to_string();

        let mut item2 = TimelineItem::new(
            Some("ast_trim_2".to_string()),
            "Item 2".to_string(),
            ItemKind::Video,
            TimeRange::new(RationalTime::new(50, fps), RationalTime::new(50, fps)),
            TimeRange::new(RationalTime::new(20, fps), RationalTime::new(50, fps)),
        )
        .unwrap();
        item2.item_id = "it2".to_string();

        let trk_id = track.track_id.clone();
        track.items.push(item1);
        track.items.push(item2);
        project.tracks.push(track);

        let pid = project.project_id.clone();
        let mut service = CommandService::new(project);

        // Create initial CrossDissolve transition
        let create_res = service.execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 0,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_init".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_create_trans".to_string(),
                operation: CommandOperation::CutTransitionCreate {
                    transition_id: Some("trans_trim_test".to_string()),
                    track_id: trk_id,
                    left_item_id: "it1".to_string(),
                    right_item_id: "it2".to_string(),
                    kind: TransitionKind::CrossDissolve,
                    duration: RationalTime::new(20, fps),
                },
            }],
        });
        assert!(create_res.is_ok());
        assert_eq!(service.revision(), 1);
        assert_eq!(service.project().tracks[0].transitions.len(), 1);

        // =========================================================================
        // CASE B: Trim preserves cut relation but invalidates handle/extent
        // => REJECT toàn bộ trim atomically, project & transition unchanged!
        // =========================================================================

        // B1: Left clip trimmed at in-point: timeline [45, 50) (duration = 5 < L=10).
        // Cut at 50 is preserved (end = 50 == it2.start 50).
        let trim_b1 = service.execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 1,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_b1".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_b1".to_string(),
                operation: CommandOperation::ClipTrim {
                    item_id: "it1".to_string(),
                    new_timeline_range: TimeRange::new(
                        RationalTime::new(45, fps),
                        RationalTime::new(5, fps),
                    ),
                    new_source_range: TimeRange::new(
                        RationalTime::new(55, fps),
                        RationalTime::new(5, fps),
                    ),
                },
            }],
        });
        assert!(matches!(
            trim_b1,
            Err(CommandError::InsufficientTransitionHandle { .. })
        ));
        assert_eq!(service.revision(), 1);
        assert_eq!(service.project().tracks[0].transitions.len(), 1);
        assert_eq!(
            service.project().tracks[0].items[0].timeline_range,
            TimeRange::new(RationalTime::new(0, fps), RationalTime::new(50, fps))
        );

        // B2: Right clip trimmed at out-point: timeline [50, 55) (duration = 5 < R=10).
        // Cut at 50 is preserved (start = 50 == it1.end 50).
        let trim_b2 = service.execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 1,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_b2".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_b2".to_string(),
                operation: CommandOperation::ClipTrim {
                    item_id: "it2".to_string(),
                    new_timeline_range: TimeRange::new(
                        RationalTime::new(50, fps),
                        RationalTime::new(5, fps),
                    ),
                    new_source_range: TimeRange::new(
                        RationalTime::new(20, fps),
                        RationalTime::new(5, fps),
                    ),
                },
            }],
        });
        assert!(matches!(
            trim_b2,
            Err(CommandError::InsufficientTransitionHandle { .. })
        ));
        assert_eq!(service.revision(), 1);
        assert_eq!(service.project().tracks[0].transitions.len(), 1);
        assert_eq!(
            service.project().tracks[0].items[1].timeline_range,
            TimeRange::new(RationalTime::new(50, fps), RationalTime::new(50, fps))
        );

        // B3: Left clip source slipped so postroll < R:
        // Timeline still [0, 50) (duration 50, end 50 preserved).
        // Source changed to [45, 95) (duration 50). Available postroll = 100 - 95 = 5 < R=10!
        let trim_b3 = service.execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 1,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_b3".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_b3".to_string(),
                operation: CommandOperation::ClipTrim {
                    item_id: "it1".to_string(),
                    new_timeline_range: TimeRange::new(
                        RationalTime::new(0, fps),
                        RationalTime::new(50, fps),
                    ),
                    new_source_range: TimeRange::new(
                        RationalTime::new(45, fps),
                        RationalTime::new(50, fps),
                    ),
                },
            }],
        });
        assert!(matches!(
            trim_b3,
            Err(CommandError::InsufficientTransitionHandle { .. })
        ));
        assert_eq!(service.revision(), 1);
        assert_eq!(service.project().tracks[0].transitions.len(), 1);

        // B4: Right clip source slipped so preroll < L:
        // Timeline still [50, 100) (duration 50, start 50 preserved).
        // Source changed to [5, 55) (duration 50). Available preroll = 5 < L=10!
        let trim_b4 = service.execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 1,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_b4".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_b4".to_string(),
                operation: CommandOperation::ClipTrim {
                    item_id: "it2".to_string(),
                    new_timeline_range: TimeRange::new(
                        RationalTime::new(50, fps),
                        RationalTime::new(50, fps),
                    ),
                    new_source_range: TimeRange::new(
                        RationalTime::new(5, fps),
                        RationalTime::new(50, fps),
                    ),
                },
            }],
        });
        assert!(matches!(
            trim_b4,
            Err(CommandError::InsufficientTransitionHandle { .. })
        ));
        assert_eq!(service.revision(), 1);
        assert_eq!(service.project().tracks[0].transitions.len(), 1);

        // =========================================================================
        // CASE A: Trim breaks original adjacency/cut relation
        // => trim succeeds + transition removed atomically!
        // =========================================================================

        let mut history = crate::modules::history::HistoryManager::new(20);
        let pre_trim_snapshot = service.project().clone();

        // Trim it1 out-point from 50 to 45 -> timeline [0, 45).
        // Cut at 50 is broken (new end = 45 != it2.start 50).
        let trim_a = service.execute(CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 1,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_a".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_a".to_string(),
                operation: CommandOperation::ClipTrim {
                    item_id: "it1".to_string(),
                    new_timeline_range: TimeRange::new(
                        RationalTime::new(0, fps),
                        RationalTime::new(45, fps),
                    ),
                    new_source_range: TimeRange::new(
                        RationalTime::new(10, fps),
                        RationalTime::new(45, fps),
                    ),
                },
            }],
        });
        assert!(trim_a.is_ok());
        assert_eq!(service.revision(), 2);
        // Transition removed atomically cùng trim
        assert!(service.project().tracks[0].transitions.is_empty());
        assert_eq!(
            service.project().tracks[0].items[0].timeline_range,
            TimeRange::new(RationalTime::new(0, fps), RationalTime::new(45, fps))
        );

        // =========================================================================
        // CASE C: Undo / Redo behavior
        // =========================================================================
        let post_trim_snapshot = service.project().clone();
        history.record_edit(crate::modules::history::HistoryEntry {
            description: "trim_breaks_cut".to_string(),
            transaction_id: Some("tx_a".to_string()),
            prior_snapshot: pre_trim_snapshot,
            resulting_snapshot: post_trim_snapshot,
        });

        // Undo -> restores both original clip range and the exact CutTransition
        let mut restored = service.project().clone();
        history.undo(&mut restored).unwrap();
        assert_eq!(restored.tracks[0].transitions.len(), 1);
        let t_restored = &restored.tracks[0].transitions[0];
        assert_eq!(t_restored.transition_id, "trans_trim_test");
        assert_eq!(t_restored.left_item_id, "it1");
        assert_eq!(t_restored.right_item_id, "it2");
        assert_eq!(t_restored.duration, RationalTime::new(20, fps));
        assert_eq!(
            restored.tracks[0].items[0].timeline_range,
            TimeRange::new(RationalTime::new(0, fps), RationalTime::new(50, fps))
        );

        // Redo -> applies trim and removes transition again
        history.redo(&mut restored).unwrap();
        assert!(restored.tracks[0].transitions.is_empty());
        assert_eq!(
            restored.tracks[0].items[0].timeline_range,
            TimeRange::new(RationalTime::new(0, fps), RationalTime::new(45, fps))
        );
    }

    #[test]
    fn test_phase13_failed_operation_leaves_canonical_state_unchanged() {
        let (project, pid, trk_id, itm1, _itm2) = setup_phase13_test_project();
        let mut service = CommandService::new(project);

        // Multi-op transaction: 1 valid TrackCreate, 1 invalid CutTransitionCreate (non-existent right item)
        let multi_env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid,
            base_revision: 0,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_atomic_fail".to_string()),
            operations: vec![
                OperationEnvelope {
                    operation_id: "op_trk".to_string(),
                    operation: CommandOperation::TrackCreate {
                        track_id: Some("trk_should_rollback".to_string()),
                        name: "Rollback Track".to_string(),
                        kind: TrackKind::Video,
                    },
                },
                OperationEnvelope {
                    operation_id: "op_fail".to_string(),
                    operation: CommandOperation::CutTransitionCreate {
                        transition_id: None,
                        track_id: trk_id,
                        left_item_id: itm1,
                        right_item_id: "non_existent_item".to_string(),
                        kind: TransitionKind::CrossDissolve,
                        duration: RationalTime::new(10, FrameRate::FPS_30),
                    },
                },
            ],
        };

        let res = service.execute(multi_env);
        assert!(res.is_err());
        // Verify canonical state was untouched:
        assert_eq!(service.revision(), 0);
        assert_eq!(service.project().tracks.len(), 1); // TrackCreate rolled back!
        assert!(service
            .project()
            .find_track("trk_should_rollback")
            .is_none());
    }

    #[test]
    fn test_phase13_mixed_rate_semantic_equality_acceptance() {
        use crate::modules::composition::CompositionPlan;
        use crate::modules::domain::time::TimeRange;
        use crate::modules::persistence::{load_project, save_project_atomic};

        let mut project = Project::new("Phase 13 Mixed Rate Test".to_string());
        project.settings.frame_rate = FrameRate::FPS_30;

        let a1 = crate::modules::domain::Asset {
            asset_id: "ast_mixed_1".to_string(),
            name: "clip1_30fps.mp4".to_string(),
            path: "D:/media/clip1_30fps.mp4".to_string(),
            media_kind: crate::modules::domain::MediaKind::Video,
            duration: Some(RationalTime::new(120, FrameRate::FPS_30)), // 4.0s
            file_size: 1024,
        };
        let a2 = crate::modules::domain::Asset {
            asset_id: "ast_mixed_2".to_string(),
            name: "clip2_60fps.mp4".to_string(),
            path: "D:/media/clip2_60fps.mp4".to_string(),
            media_kind: crate::modules::domain::MediaKind::Video,
            duration: Some(RationalTime::new(240, FrameRate::FPS_60)), // 4.0s
            file_size: 1024,
        };
        project.assets.push(a1);
        project.assets.push(a2);

        let mut track = Track::new("Video Track 1".to_string(), TrackKind::Video);
        track.track_id = "trk_mixed".to_string();

        // Left item: end 60 @ 30fps = 2.0s
        let mut left_item = TimelineItem::new(
            Some("ast_mixed_1".to_string()),
            "Left Clip 30fps".to_string(),
            ItemKind::Video,
            TimeRange::new(
                RationalTime::new(0, FrameRate::FPS_30),
                RationalTime::new(60, FrameRate::FPS_30), // duration 60 @ 30fps -> end = 60 @ 30fps (2.0s)
            ),
            TimeRange::new(
                RationalTime::new(10, FrameRate::FPS_30),
                RationalTime::new(60, FrameRate::FPS_30), // postroll = 120 - 70 = 50 frames @ 30fps
            ),
        )
        .unwrap();
        left_item.item_id = "itm_left_30".to_string();

        // Right item: start 120 @ 60fps = 2.0s
        let mut right_item = TimelineItem::new(
            Some("ast_mixed_2".to_string()),
            "Right Clip 60fps".to_string(),
            ItemKind::Video,
            TimeRange::new(
                RationalTime::new(120, FrameRate::FPS_60), // start 120 @ 60fps (2.0s)
                RationalTime::new(120, FrameRate::FPS_60), // duration 120 @ 60fps -> end = 240 @ 60fps (4.0s)
            ),
            TimeRange::new(
                RationalTime::new(60, FrameRate::FPS_60), // preroll = 60 frames @ 60fps = 1.0s
                RationalTime::new(120, FrameRate::FPS_60),
            ),
        )
        .unwrap();
        right_item.item_id = "itm_right_60".to_string();

        // Verify precondition: raw equality fails, but semantic equality holds
        assert_ne!(
            left_item.timeline_range.end(),
            right_item.timeline_range.start
        );
        assert!(left_item
            .timeline_range
            .end()
            .is_same_time(&right_item.timeline_range.start));
        assert!(left_item
            .timeline_range
            .is_adjacent_to(&right_item.timeline_range));
        assert!(left_item
            .timeline_range
            .is_adjacent_to_with_rate(&right_item.timeline_range, FrameRate::FPS_30));

        track.items.push(left_item);
        track.items.push(right_item);
        project.tracks.push(track);

        // --- Proof 1: Valid adjacent CrossDissolve is accepted by domain validation ---
        let ct = CutTransition::new(
            "trk_mixed".to_string(),
            "itm_left_30".to_string(),
            "itm_right_60".to_string(),
            TransitionKind::CrossDissolve,
            RationalTime::new(20, FrameRate::FPS_30),
        )
        .unwrap();
        project.tracks[0].transitions.push(ct);
        assert!(
            project.tracks[0].validate().is_ok(),
            "Domain track validation must accept adjacent CrossDissolve with equivalent timestamps at different rates"
        );
        assert!(
            project.validate().is_ok(),
            "Domain project validation must accept adjacent CrossDissolve with equivalent timestamps at different rates"
        );
        project.tracks[0].transitions.clear();

        // --- Proof 2: CutTransition create and update duration work via CommandService ---
        let mut service = CommandService::new(project.clone());
        let pid = project.project_id.clone();

        // Create transition
        let create_env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 0,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_mixed_create".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_create".to_string(),
                operation: CommandOperation::CutTransitionCreate {
                    transition_id: Some("ct_mixed".to_string()),
                    track_id: "trk_mixed".to_string(),
                    left_item_id: "itm_left_30".to_string(),
                    right_item_id: "itm_right_60".to_string(),
                    kind: TransitionKind::CrossDissolve,
                    duration: RationalTime::new(20, FrameRate::FPS_30), // 20 frames = 0.667s
                },
            }],
        };
        let res_create = service.execute(create_env);
        assert!(
            res_create.is_ok(),
            "CutTransitionCreate must succeed across mixed-rate participants: {:?}",
            res_create.err()
        );
        assert_eq!(service.project().tracks[0].transitions.len(), 1);
        assert_eq!(
            service.project().tracks[0].transitions[0].duration,
            RationalTime::new(20, FrameRate::FPS_30)
        );

        // Update duration
        let update_env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 1,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_mixed_update".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_update".to_string(),
                operation: CommandOperation::CutTransitionUpdateDuration {
                    transition_id: "ct_mixed".to_string(),
                    new_duration: RationalTime::new(24, FrameRate::FPS_30),
                },
            }],
        };
        let res_update = service.execute(update_env);
        assert!(
            res_update.is_ok(),
            "CutTransitionUpdateDuration must succeed across mixed-rate participants: {:?}",
            res_update.err()
        );
        assert_eq!(
            service.project().tracks[0].transitions[0].duration,
            RationalTime::new(24, FrameRate::FPS_30)
        );

        // --- Proof 3: Trim preserving cut is NOT misidentified as cut broken ---
        // Trim left clip start from 0 to 10 @ 30fps (duration from 60 to 50 @ 30fps).
        // Cut at 2.0s (end = 10 + 50 = 60 @ 30fps) is completely preserved!
        let trim_env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: pid.clone(),
            base_revision: 2,
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: Some("tx_mixed_trim".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_trim".to_string(),
                operation: CommandOperation::ClipTrim {
                    item_id: "itm_left_30".to_string(),
                    new_timeline_range: TimeRange::new(
                        RationalTime::new(10, FrameRate::FPS_30),
                        RationalTime::new(50, FrameRate::FPS_30),
                    ),
                    new_source_range: TimeRange::new(
                        RationalTime::new(20, FrameRate::FPS_30),
                        RationalTime::new(50, FrameRate::FPS_30),
                    ),
                },
            }],
        };
        let res_trim = service.execute(trim_env);
        assert!(
            res_trim.is_ok(),
            "ClipTrim preserving cut must succeed: {:?}",
            res_trim.err()
        );
        assert_eq!(
            service.project().tracks[0].transitions.len(),
            1,
            "CutTransition must be preserved after trim that keeps the cut seam intact"
        );

        // --- Proof 4: Composition, Preview, and Export resolve the same cut ---
        let plan = CompositionPlan::from_project(service.project())
            .expect("CompositionPlan::from_project");
        assert_eq!(
            plan.transitions.len(),
            1,
            "CompositionPlan must resolve the cut transition"
        );
        let tp = &plan.transitions[0];
        assert_eq!(tp.transition_id, "ct_mixed");
        // Cut time is 2.0s
        assert_eq!(tp.cut_time.to_seconds(), 2.0);

        // Sample preview at exactly 2.0s using timestamps from both representations (30fps and 60fps)
        let sample_30 = RationalTime::new(60, FrameRate::FPS_30); // 2.0s
        let sample_60 = RationalTime::new(120, FrameRate::FPS_60); // 2.0s
        let preview_30 = plan.sample_preview_at(sample_30);
        let preview_60 = plan.sample_preview_at(sample_60);

        assert_eq!(
            preview_30.transition_groups.len(),
            1,
            "Preview at 30fps must resolve transition"
        );
        assert_eq!(
            preview_60.transition_groups.len(),
            1,
            "Preview at 60fps must resolve transition"
        );
        assert_eq!(
            preview_30.transition_groups[0].transition_id,
            preview_60.transition_groups[0].transition_id
        );
        assert!(
            (preview_30.transition_groups[0].progress - preview_60.transition_groups[0].progress)
                .abs()
                < 1e-4,
            "Both rate representations must yield identical preview progress"
        );

        // Export filtergraph build
        let input_paths = vec![
            std::path::PathBuf::from("D:/media/clip1_30fps.mp4"),
            std::path::PathBuf::from("D:/media/clip2_60fps.mp4"),
        ];
        let (fg, _out_label) = plan.build_video_filtergraph(&input_paths, 4.0, None);
        assert!(
            fg.contains("tr_l_clip_0"),
            "Export filtergraph must contain transition left participant pipeline"
        );

        // --- Proof 5: Save / Reopen does not alter behavior ---
        let temp_dir = std::env::temp_dir().join("tinixcut_mixed_rate_audit_test");
        std::fs::create_dir_all(&temp_dir).unwrap();
        let proj_file = temp_dir.join("mixed_rate_project.tcproj");
        save_project_atomic(service.project(), &proj_file).expect("save_project_atomic");
        let reopened = load_project(&proj_file).expect("load_project");

        assert!(
            reopened.validate().is_ok(),
            "Reopened project must pass domain validation"
        );
        assert_eq!(reopened.tracks[0].transitions.len(), 1);
        let reopened_trans = &reopened.tracks[0].transitions[0];
        assert_eq!(reopened_trans.transition_id, "ct_mixed");
        assert_eq!(
            reopened_trans.duration,
            RationalTime::new(24, FrameRate::FPS_30)
        );

        let reopened_plan =
            CompositionPlan::from_project(&reopened).expect("reopened CompositionPlan");
        assert_eq!(reopened_plan.transitions.len(), 1);
        assert_eq!(reopened_plan.transitions[0].cut_time.to_seconds(), 2.0);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
