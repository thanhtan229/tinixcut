//! Persistence, Atomic Save, Recovery and Schema Migration
//! Architecture Invariant: Saves are atomic (temp write -> commit/replace).
//! Cache is rebuildable and not required for project correctness.
//! Corrupt or interrupted saves never destroy prior valid project.

use crate::modules::domain::{Project, TextStyle, CURRENT_SCHEMA_VERSION};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::fs::OpenOptions;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Project validation failed: {0}")]
    Validation(String),

    #[error("Schema version {found} is not supported. Maximum supported is {max_supported}")]
    UnsupportedSchemaVersion { found: u32, max_supported: u32 },

    #[error("Schema migration failed: {0}")]
    MigrationFailed(String),

    #[error("Invalid project file: {0}")]
    InvalidProject(String),

    #[error("Project revision conflict: expected disk revision {expected:?}, found {actual:?}; reload the project or save as a new file to preserve your edits")]
    RevisionConflict {
        expected: Option<u64>,
        actual: Option<u64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RecoveryStatus {
    NoRecoveryFound,
    RecoveryMatchesProject,
    NewerRecoveryAvailable {
        recovery_path: PathBuf,
        recovery_revision: u64,
        project_revision: u64,
        recovery_fingerprint: String,
    },
    RecoveryDiffersFromProject {
        recovery_path: PathBuf,
        recovery_revision: u64,
        project_revision: u64,
        recovery_fingerprint: String,
    },
    RecoveryUnreadable {
        recovery_path: PathBuf,
        reason: String,
    },
}

/// Unconditionally atomically save a project to disk.
///
/// Use `save_project_atomic_if_revision` for an existing authoritative project
/// so a stale in-memory snapshot cannot replace a newer disk revision.
/// 1. Serialize validated project.
/// 2. Write to temporary sibling file `<path>.tmp`.
/// 3. Flush and sync.
/// 4. Atomically rename/replace `<path>.tmp` -> `<path>`.
///
/// If any step before rename fails, the original `<path>` remains completely intact!
pub fn save_project_atomic(project: &Project, target_path: &Path) -> Result<(), PersistenceError> {
    let _write_lock = acquire_project_write_lock(target_path)?;
    save_project_atomic_unlocked(project, target_path)
}

/// Save only if the project's last observed disk revision is still current.
/// The lock covers both the comparison and atomic replacement, including across
/// cooperating TinixCut processes.
pub fn save_project_atomic_if_revision(
    project: &Project,
    target_path: &Path,
    expected_revision: Option<u64>,
) -> Result<(), PersistenceError> {
    let _write_lock = acquire_project_write_lock(target_path)?;
    let actual_revision = if target_path.exists() {
        Some(load_project(target_path)?.revision)
    } else {
        None
    };

    if actual_revision != expected_revision {
        return Err(PersistenceError::RevisionConflict {
            expected: expected_revision,
            actual: actual_revision,
        });
    }

    save_project_atomic_unlocked(project, target_path)
}

/// Remove a recovery/project file only while the caller's observed identity and revision remain current.
pub fn remove_project_if_revision(
    target_path: &Path,
    expected_project_id: &str,
    expected_revision: u64,
) -> Result<(), PersistenceError> {
    let _write_lock = acquire_project_write_lock(target_path)?;
    if !target_path.exists() {
        return Err(PersistenceError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Project file does not exist",
        )));
    }
    let current = load_project(target_path)?;
    if current.project_id != expected_project_id {
        return Err(PersistenceError::InvalidProject(
            "Project identity changed before removal".to_string(),
        ));
    }
    if current.revision != expected_revision {
        return Err(PersistenceError::RevisionConflict {
            expected: Some(expected_revision),
            actual: Some(current.revision),
        });
    }
    fs::remove_file(target_path)?;
    Ok(())
}

fn acquire_project_write_lock(target_path: &Path) -> Result<File, PersistenceError> {
    let canonical_target = canonical_project_path(target_path)?;

    let mut lock_path = OsString::from(canonical_target.as_os_str());
    lock_path.push(".tinixcut.lock");
    let lock_file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(PathBuf::from(lock_path))?;
    lock_file.lock()?;
    Ok(lock_file)
}

/// Resolve existing aliases to one path identity so Save As to the same project
/// cannot bypass the revision observed when that project was opened.
pub fn same_project_path(left: &Path, right: &Path) -> Result<bool, PersistenceError> {
    Ok(canonical_project_path(left)? == canonical_project_path(right)?)
}

fn canonical_project_path(path: &Path) -> Result<PathBuf, PersistenceError> {
    if path.exists() {
        return Ok(fs::canonicalize(path)?);
    }

    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let file_name = path.file_name().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Project path must include a file name",
        )
    })?;
    Ok(fs::canonicalize(parent)?.join(file_name))
}

fn save_project_atomic_unlocked(
    project: &Project,
    target_path: &Path,
) -> Result<(), PersistenceError> {
    project
        .validate()
        .map_err(|e| PersistenceError::Validation(e.to_string()))?;

    let json_bytes = serde_json::to_vec_pretty(project)?;

    if let Some(parent) = target_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }

    let tmp_path = target_path.with_extension("tcproj.tmp");

    // Write to temp file
    {
        let mut file = File::create(&tmp_path)?;
        file.write_all(&json_bytes)?;
        file.flush()?;
        file.sync_all()?;
    }

    // Atomic replace on Windows/POSIX
    // On Windows, rename will overwrite existing if using replace or remove first
    if target_path.exists() {
        let backup_path = target_path.with_extension("tcproj.bak");
        // Safe atomic switch via backup
        let _ = fs::remove_file(&backup_path);
        fs::rename(target_path, &backup_path)?;
        if let Err(e) = fs::rename(&tmp_path, target_path) {
            // Restore from backup if rename failed
            let _ = fs::rename(&backup_path, target_path);
            return Err(PersistenceError::Io(e));
        }
        let _ = fs::remove_file(&backup_path);
    } else {
        fs::rename(&tmp_path, target_path)?;
    }

    Ok(())
}

/// Load and validate a project from disk.
pub fn load_project(path: &Path) -> Result<Project, PersistenceError> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let clean_contents = contents.trim_start_matches('\u{feff}');

    // Check schema version and run migrations if needed
    let migrated_json = run_migrations_if_needed(clean_contents)?;
    let project: Project = serde_json::from_str(&migrated_json)?;

    project
        .validate()
        .map_err(|e| PersistenceError::Validation(e.to_string()))?;
    Ok(project)
}

/// Return the current revision for an existing project, or `None` for a new path.
pub fn load_project_revision_if_exists(path: &Path) -> Result<Option<u64>, PersistenceError> {
    if !path.exists() {
        return Ok(None);
    }
    load_project(path).map(|project| Some(project.revision))
}

/// Save an autosave / recovery snapshot.
/// Invariant: Autosave is NEVER saved directly over canonical user project!
pub fn save_recovery_snapshot(
    project: &Project,
    recovery_path: &Path,
) -> Result<(), PersistenceError> {
    save_project_atomic(project, recovery_path)
}

/// Save a recovery snapshot without allowing a stale writer to replace newer
/// content for the same project. The comparison and atomic replace share one lock.
pub fn save_recovery_snapshot_if_newer(
    project: &Project,
    recovery_path: &Path,
) -> Result<(), PersistenceError> {
    let _write_lock = acquire_project_write_lock(recovery_path)?;
    if recovery_path.exists() {
        let current = load_project(recovery_path)?;
        if current.project_id != project.project_id {
            return Err(PersistenceError::InvalidProject(
                "Recovery path belongs to a different project".to_string(),
            ));
        }
        if project.revision < current.revision {
            return Err(PersistenceError::RevisionConflict {
                expected: Some(project.revision),
                actual: Some(current.revision),
            });
        }
        if project.revision == current.revision {
            if project == &current {
                return Ok(());
            }
            return Err(PersistenceError::InvalidProject(
                "Different recovery content already exists at the same revision".to_string(),
            ));
        }
    }
    save_project_atomic_unlocked(project, recovery_path)
}

/// Detect whether a newer recovery file exists compared to canonical project.
pub fn check_recovery_status(
    canonical_path: &Path,
    recovery_path: &Path,
) -> Result<RecoveryStatus, PersistenceError> {
    if !recovery_path.exists() {
        return Ok(RecoveryStatus::NoRecoveryFound);
    }

    let recovery_proj = match load_project(recovery_path) {
        Ok(project) => project,
        Err(error) => {
            return Ok(RecoveryStatus::RecoveryUnreadable {
                recovery_path: recovery_path.to_path_buf(),
                reason: error.to_string(),
            })
        }
    };

    if !canonical_path.exists() {
        return Ok(RecoveryStatus::NewerRecoveryAvailable {
            recovery_path: recovery_path.to_path_buf(),
            recovery_revision: recovery_proj.revision,
            project_revision: 0,
            recovery_fingerprint: recovery_snapshot_fingerprint(&recovery_proj)?,
        });
    }

    let canonical_proj = load_project(canonical_path)?;

    if recovery_proj.project_id != canonical_proj.project_id {
        return Ok(RecoveryStatus::NoRecoveryFound);
    }

    let mut recovery_content = recovery_proj.clone();
    let mut canonical_content = canonical_proj.clone();
    recovery_content.revision = 0;
    canonical_content.revision = 0;
    if recovery_content == canonical_content {
        Ok(RecoveryStatus::RecoveryMatchesProject)
    } else if recovery_proj.revision > canonical_proj.revision {
        Ok(RecoveryStatus::NewerRecoveryAvailable {
            recovery_path: recovery_path.to_path_buf(),
            recovery_revision: recovery_proj.revision,
            project_revision: canonical_proj.revision,
            recovery_fingerprint: recovery_snapshot_fingerprint(&recovery_proj)?,
        })
    } else {
        Ok(RecoveryStatus::RecoveryDiffersFromProject {
            recovery_path: recovery_path.to_path_buf(),
            recovery_revision: recovery_proj.revision,
            project_revision: canonical_proj.revision,
            recovery_fingerprint: recovery_snapshot_fingerprint(&recovery_proj)?,
        })
    }
}

/// Delete the recovery copy of a saved project after the user chose the saved version.
///
/// Without this, a recovery copy with a higher revision than the reopened project makes every
/// later recovery save fail with a revision conflict. The revision and fingerprint must still
/// match what the user reviewed, so a copy written meanwhile is never deleted by mistake.
pub fn discard_saved_recovery(
    recovery_path: &Path,
    expected_recovery_revision: u64,
    expected_recovery_fingerprint: &str,
) -> Result<(), PersistenceError> {
    let _write_lock = acquire_project_write_lock(recovery_path)?;
    if !recovery_path.exists() {
        return Ok(());
    }
    let current = load_project(recovery_path)?;
    if current.revision != expected_recovery_revision
        || recovery_snapshot_fingerprint(&current)? != expected_recovery_fingerprint
    {
        return Err(PersistenceError::InvalidProject(
            "The recovery snapshot changed after it was reviewed; it was not discarded".to_string(),
        ));
    }
    fs::remove_file(recovery_path)?;
    Ok(())
}

pub fn recovery_snapshot_fingerprint(project: &Project) -> Result<String, PersistenceError> {
    use sha2::Digest;

    // Serialize through serde_json::Value so object keys are canonicalized
    // before hashing. Direct serialization of HashMap fields is order-unstable.
    let canonical = serde_json::to_value(project)?;
    let bytes = serde_json::to_vec(&canonical)?;
    Ok(sha2::Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// Schema migration harness
/// Detects version in raw JSON and applies registered migration transforms.
pub fn run_migrations_if_needed(raw_json: &str) -> Result<String, PersistenceError> {
    let clean_json = raw_json.trim_start_matches('\u{feff}');
    let mut val: serde_json::Value = serde_json::from_str(clean_json)?;
    let version = val
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            PersistenceError::InvalidProject("Missing schema_version field".to_string())
        })? as u32;

    if version > CURRENT_SCHEMA_VERSION {
        return Err(PersistenceError::UnsupportedSchemaVersion {
            found: version,
            max_supported: CURRENT_SCHEMA_VERSION,
        });
    }

    if version == 1 && CURRENT_SCHEMA_VERSION >= 2 {
        val = migrate_v1_to_v2(val)?;
    }

    if (version == 1 || version == 2) && CURRENT_SCHEMA_VERSION >= 3 {
        val = migrate_v2_to_v3(val)?;
    }

    if version <= 3 && CURRENT_SCHEMA_VERSION >= 4 {
        val = migrate_v3_to_v4(val)?;
    }

    if version <= 4 && CURRENT_SCHEMA_VERSION >= 5 {
        val = migrate_v4_to_v5(val)?;
    }
    if version <= 5 && CURRENT_SCHEMA_VERSION >= 6 {
        val = migrate_v5_to_v6(val)?;
    }
    if version <= 6 && CURRENT_SCHEMA_VERSION >= 7 {
        val = migrate_v6_to_v7(val)?;
    }
    if version <= 7 && CURRENT_SCHEMA_VERSION >= 8 {
        val = migrate_v7_to_v8(val)?;
    }
    if version <= 8 && CURRENT_SCHEMA_VERSION >= 9 {
        val = migrate_v8_to_v9(val)?;
    }
    if version <= 9 && CURRENT_SCHEMA_VERSION >= 10 {
        val = migrate_v9_to_v10(val)?;
    }
    if version <= 10 && CURRENT_SCHEMA_VERSION >= 11 {
        val = migrate_v10_to_v11(val)?;
    }
    if version <= 11 && CURRENT_SCHEMA_VERSION >= 12 {
        val = migrate_v11_to_v12(val)?;
    }
    if version <= 12 && CURRENT_SCHEMA_VERSION >= 13 {
        val = migrate_v12_to_v13(val)?;
    }
    if version <= 13 && CURRENT_SCHEMA_VERSION >= 14 {
        val = migrate_v13_to_v14(val)?;
    }
    if version <= 14 && CURRENT_SCHEMA_VERSION >= 15 {
        val = migrate_v14_to_v15(val)?;
    }
    if version <= 15 && CURRENT_SCHEMA_VERSION >= 16 {
        val = migrate_v15_to_v16(val)?;
    }
    if version <= 16 && CURRENT_SCHEMA_VERSION >= 17 {
        val = migrate_v16_to_v17(val)?;
    }
    if version <= 17 && CURRENT_SCHEMA_VERSION >= 18 {
        val = migrate_v17_to_v18(val)?;
    }
    if version <= 18 && CURRENT_SCHEMA_VERSION >= 19 {
        val = migrate_v18_to_v19(val)?;
    }

    serde_json::to_string(&val).map_err(PersistenceError::Serialization)
}

/// Migrates Schema v1 projects to Schema v2:
/// - Promotes `schema_version` to 2.
/// - Explicitly initializes `transitions: []` on all tracks.
/// - Preserves all single-clip Fade semantics (`fade_in`, `fade_out`, item-level `FadeIn`/`FadeOut`/`FadeBlack`) losslessly.
/// - Never invents `CrossDissolve`.
/// - Rejects malformed v1 projects containing unsupported `CrossDissolve` placeholders on items.
pub fn migrate_v1_to_v2(mut val: serde_json::Value) -> Result<serde_json::Value, PersistenceError> {
    let obj = val.as_object_mut().ok_or_else(|| {
        PersistenceError::MigrationFailed("Project JSON root must be an object".to_string())
    })?;

    if let Some(tracks_val) = obj.get_mut("tracks") {
        if let Some(tracks_arr) = tracks_val.as_array_mut() {
            for track_val in tracks_arr.iter_mut() {
                if let Some(track_obj) = track_val.as_object_mut() {
                    // Check items for malformed legacy item-level CrossDissolve placeholder
                    if let Some(items_val) = track_obj.get("items") {
                        if let Some(items_arr) = items_val.as_array() {
                            for item_val in items_arr {
                                if let Some(trans_val) = item_val.get("transition") {
                                    if !trans_val.is_null() {
                                        if let Some(kind_val) = trans_val.get("kind") {
                                            if kind_val.as_str() == Some("CrossDissolve") {
                                                return Err(PersistenceError::MigrationFailed(
                                                    "Malformed Schema v1 project: item-level CrossDissolve placeholder is unsupported in Schema v2 and cannot be migrated to cut transition".to_string()
                                                ));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // CD-09, CD-12: Initialize empty transitions array if absent or null
                    if !track_obj.contains_key("transitions")
                        || track_obj.get("transitions").is_some_and(|v| v.is_null())
                    {
                        track_obj.insert("transitions".to_string(), serde_json::json!([]));
                    }
                }
            }
        }
    }

    // CD-08: Bump schema version to 2
    obj.insert("schema_version".to_string(), serde_json::json!(2));

    Ok(val)
}

/// Migrates Schema v2 projects to Schema v3:
/// - Promotes `schema_version` to 3.
/// - Sets `outgoing_easing = "Linear"` for all existing keyframes that omit outgoing easing.
/// - Preserves all Phase-13 cut transitions, fades, project metadata, assets, track topology,
///   item ranges, transforms, and client registry/command ledger losslessly.
/// - Produces zero visual or timing change for valid existing projects.
pub fn migrate_v2_to_v3(mut val: serde_json::Value) -> Result<serde_json::Value, PersistenceError> {
    let obj = val.as_object_mut().ok_or_else(|| {
        PersistenceError::MigrationFailed("Project JSON root must be an object".to_string())
    })?;

    if let Some(tracks_val) = obj.get_mut("tracks") {
        if let Some(tracks_arr) = tracks_val.as_array_mut() {
            for track_val in tracks_arr.iter_mut() {
                if let Some(track_obj) = track_val.as_object_mut() {
                    if let Some(items_val) = track_obj.get_mut("items") {
                        if let Some(items_arr) = items_val.as_array_mut() {
                            for item_val in items_arr.iter_mut() {
                                if let Some(item_obj) = item_val.as_object_mut() {
                                    if let Some(kfs_val) = item_obj.get_mut("keyframes") {
                                        if let Some(kfs_arr) = kfs_val.as_array_mut() {
                                            for kf_val in kfs_arr.iter_mut() {
                                                if let Some(kf_obj) = kf_val.as_object_mut() {
                                                    if !kf_obj.contains_key("outgoing_easing")
                                                        || kf_obj
                                                            .get("outgoing_easing")
                                                            .is_some_and(|v| v.is_null())
                                                    {
                                                        kf_obj.insert(
                                                            "outgoing_easing".to_string(),
                                                            serde_json::json!("Linear"),
                                                        );
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
            }
        }
    }

    // Bump schema version to 3
    obj.insert("schema_version".to_string(), serde_json::json!(3));

    Ok(val)
}

/// Migrates Schema v3 projects to Schema v4 by adding an empty standalone-audio
/// crossfade collection to every track. Existing editorial data is unchanged.
pub fn migrate_v3_to_v4(mut val: serde_json::Value) -> Result<serde_json::Value, PersistenceError> {
    let obj = val.as_object_mut().ok_or_else(|| {
        PersistenceError::MigrationFailed("Project JSON root must be an object".to_string())
    })?;
    if let Some(tracks) = obj.get_mut("tracks").and_then(|value| value.as_array_mut()) {
        for track in tracks {
            if let Some(track) = track.as_object_mut() {
                if !track.contains_key("audio_transitions")
                    || track
                        .get("audio_transitions")
                        .is_some_and(serde_json::Value::is_null)
                {
                    track.insert("audio_transitions".to_string(), serde_json::json!([]));
                }
            }
        }
    }
    obj.insert("schema_version".to_string(), serde_json::json!(4));
    Ok(val)
}

/// Migrates Schema v4 projects to Schema v5 by initializing the declarative
/// text style fields for existing text items. Non-text items and all existing
/// editorial data are preserved.
pub fn migrate_v4_to_v5(mut val: serde_json::Value) -> Result<serde_json::Value, PersistenceError> {
    let obj = val.as_object_mut().ok_or_else(|| {
        PersistenceError::MigrationFailed("Project JSON root must be an object".to_string())
    })?;
    let default_style = serde_json::to_value(TextStyle::default())?;

    if let Some(tracks) = obj.get_mut("tracks").and_then(|value| value.as_array_mut()) {
        for track in tracks {
            if let Some(items) = track
                .as_object_mut()
                .and_then(|track| track.get_mut("items"))
                .and_then(|value| value.as_array_mut())
            {
                for item in items {
                    if let Some(item) = item.as_object_mut() {
                        if item.get("kind").and_then(|value| value.as_str()) == Some("Text") {
                            if !item.contains_key("text_style")
                                || item
                                    .get("text_style")
                                    .is_some_and(serde_json::Value::is_null)
                            {
                                item.insert("text_style".to_string(), default_style.clone());
                            }
                            if !item.contains_key("text_template") {
                                item.insert("text_template".to_string(), serde_json::Value::Null);
                            }
                        }
                    }
                }
            }
        }
    }

    obj.insert("schema_version".to_string(), serde_json::json!(5));
    Ok(val)
}

/// Migrates Schema v5 projects to Schema v6. Source dimensions and placement
/// policy are explicit so preview and export can share fit/fill semantics.
pub fn migrate_v5_to_v6(mut val: serde_json::Value) -> Result<serde_json::Value, PersistenceError> {
    let obj = val.as_object_mut().ok_or_else(|| {
        PersistenceError::MigrationFailed("Project JSON root must be an object".to_string())
    })?;

    if let Some(assets) = obj.get_mut("assets").and_then(|value| value.as_array_mut()) {
        for asset in assets {
            if let Some(asset) = asset.as_object_mut() {
                asset
                    .entry("dimensions".to_string())
                    .or_insert(serde_json::Value::Null);
            }
        }
    }

    if let Some(tracks) = obj.get_mut("tracks").and_then(|value| value.as_array_mut()) {
        for track in tracks {
            if let Some(items) = track
                .as_object_mut()
                .and_then(|track| track.get_mut("items"))
                .and_then(|value| value.as_array_mut())
            {
                for item in items {
                    if let Some(item) = item.as_object_mut() {
                        item.entry("placement_mode".to_string())
                            .or_insert(serde_json::json!("Fit"));
                    }
                }
            }
        }
    }

    obj.insert("schema_version".to_string(), serde_json::json!(6));
    Ok(val)
}

/// Migrates Schema v6 projects to Schema v7. Canvas background is persisted as
/// a validated application-owned #RRGGBB value for preview/export parity.
pub fn migrate_v6_to_v7(mut val: serde_json::Value) -> Result<serde_json::Value, PersistenceError> {
    let obj = val.as_object_mut().ok_or_else(|| {
        PersistenceError::MigrationFailed("Project JSON root must be an object".to_string())
    })?;

    let settings = obj
        .get_mut("settings")
        .and_then(|value| value.as_object_mut())
        .ok_or_else(|| {
            PersistenceError::MigrationFailed("Project settings must be an object".to_string())
        })?;
    settings
        .entry("canvas_background_color".to_string())
        .or_insert(serde_json::json!("#000000"));

    obj.insert("schema_version".to_string(), serde_json::json!(7));
    Ok(val)
}

/// Migrates Schema v7 projects to Schema v8 by assigning the neutral visual
/// effect preset to every existing timeline item. The new preset is a
/// presentation property only, so this migration preserves prior appearance.
pub fn migrate_v7_to_v8(mut val: serde_json::Value) -> Result<serde_json::Value, PersistenceError> {
    let obj = val.as_object_mut().ok_or_else(|| {
        PersistenceError::MigrationFailed("Project JSON root must be an object".to_string())
    })?;

    if let Some(tracks) = obj.get_mut("tracks").and_then(|value| value.as_array_mut()) {
        for track in tracks {
            if let Some(items) = track
                .as_object_mut()
                .and_then(|track| track.get_mut("items"))
                .and_then(|value| value.as_array_mut())
            {
                for item in items {
                    if let Some(item) = item.as_object_mut() {
                        item.entry("visual_effect".to_string())
                            .or_insert(serde_json::json!("None"));
                    }
                }
            }
        }
    }

    obj.insert("schema_version".to_string(), serde_json::json!(8));
    Ok(val)
}

/// Migrates Schema v8 projects to Schema v9 by assigning normal playback speed
/// to all existing clips. This keeps legacy timeline/source mappings unchanged.
pub fn migrate_v8_to_v9(mut val: serde_json::Value) -> Result<serde_json::Value, PersistenceError> {
    let obj = val.as_object_mut().ok_or_else(|| {
        PersistenceError::MigrationFailed("Project JSON root must be an object".to_string())
    })?;

    if let Some(tracks) = obj.get_mut("tracks").and_then(|value| value.as_array_mut()) {
        for track in tracks {
            if let Some(items) = track
                .get_mut("items")
                .and_then(|value| value.as_array_mut())
            {
                for item in items {
                    if let Some(item_obj) = item.as_object_mut() {
                        item_obj
                            .entry("speed".to_string())
                            .or_insert_with(|| serde_json::json!("Normal"));
                    }
                }
            }
        }
    }

    obj.insert("schema_version".to_string(), serde_json::json!(9));
    Ok(val)
}

/// Migrates Schema v9 projects to Schema v10. The rich text styling and
/// animation domain model is additive and backwards-compatible; existing
/// items retain legacy styling losslessly and without synthetic defaults.
pub fn migrate_v9_to_v10(
    mut val: serde_json::Value,
) -> Result<serde_json::Value, PersistenceError> {
    let obj = val.as_object_mut().ok_or_else(|| {
        PersistenceError::MigrationFailed("Project JSON root must be an object".to_string())
    })?;
    obj.insert("schema_version".to_string(), serde_json::json!(10));
    Ok(val)
}

/// Migrates Schema v10 projects to Schema v11. The layered template model
/// is additive and backwards-compatible; existing items retain legacy styling
/// and animation losslessly without synthetic defaults.
pub fn migrate_v10_to_v11(
    mut val: serde_json::Value,
) -> Result<serde_json::Value, PersistenceError> {
    let obj = val.as_object_mut().ok_or_else(|| {
        PersistenceError::MigrationFailed("Project JSON root must be an object".to_string())
    })?;
    obj.insert("schema_version".to_string(), serde_json::json!(11));
    Ok(val)
}

/// Adds an empty project-level marker list without changing existing timeline semantics.
pub fn migrate_v11_to_v12(
    mut val: serde_json::Value,
) -> Result<serde_json::Value, PersistenceError> {
    let obj = val.as_object_mut().ok_or_else(|| {
        PersistenceError::MigrationFailed("Project JSON root must be an object".to_string())
    })?;
    obj.entry("markers")
        .or_insert_with(|| serde_json::json!([]));
    obj.insert("schema_version".to_string(), serde_json::json!(12));
    Ok(val)
}

/// Preserves existing embedded-video sound by default while allowing edits to opt out.
pub fn migrate_v12_to_v13(
    mut val: serde_json::Value,
) -> Result<serde_json::Value, PersistenceError> {
    let obj = val.as_object_mut().ok_or_else(|| {
        PersistenceError::MigrationFailed("Project JSON root must be an object".to_string())
    })?;
    if let Some(tracks) = obj
        .get_mut("tracks")
        .and_then(serde_json::Value::as_array_mut)
    {
        for track in tracks {
            if let Some(items) = track
                .get_mut("items")
                .and_then(serde_json::Value::as_array_mut)
            {
                for item in items {
                    item.as_object_mut()
                        .ok_or_else(|| {
                            PersistenceError::MigrationFailed(
                                "Timeline item must be an object during v12 to v13 migration"
                                    .to_string(),
                            )
                        })?
                        .entry("embedded_audio_enabled")
                        .or_insert(serde_json::Value::Bool(true));
                }
            }
        }
    }
    obj.insert("schema_version".to_string(), serde_json::json!(13));
    Ok(val)
}

/// Adds track solo state without changing the audible result of legacy projects.
pub fn migrate_v13_to_v14(
    mut val: serde_json::Value,
) -> Result<serde_json::Value, PersistenceError> {
    let obj = val.as_object_mut().ok_or_else(|| {
        PersistenceError::MigrationFailed("Project JSON root must be an object".to_string())
    })?;
    if let Some(tracks) = obj
        .get_mut("tracks")
        .and_then(serde_json::Value::as_array_mut)
    {
        for track in tracks {
            track
                .as_object_mut()
                .ok_or_else(|| {
                    PersistenceError::MigrationFailed(
                        "Track must be an object during v13 to v14 migration".to_string(),
                    )
                })?
                .entry("solo")
                .or_insert(serde_json::Value::Bool(false));
        }
    }
    obj.insert("schema_version".to_string(), serde_json::json!(14));
    Ok(val)
}

/// Adds unity track gain while preserving each existing project's audible mix.
pub fn migrate_v14_to_v15(
    mut val: serde_json::Value,
) -> Result<serde_json::Value, PersistenceError> {
    let obj = val.as_object_mut().ok_or_else(|| {
        PersistenceError::MigrationFailed("Project JSON root must be an object".to_string())
    })?;
    if let Some(tracks) = obj
        .get_mut("tracks")
        .and_then(serde_json::Value::as_array_mut)
    {
        for track in tracks {
            track
                .as_object_mut()
                .ok_or_else(|| {
                    PersistenceError::MigrationFailed(
                        "Track must be an object during v14 to v15 migration".to_string(),
                    )
                })?
                .entry("volume")
                .or_insert(serde_json::json!(1.0));
        }
    }
    obj.insert("schema_version".to_string(), serde_json::json!(15));
    Ok(val)
}

/// Adds an explicit reciprocal link reference without linking legacy clips implicitly.
pub fn migrate_v15_to_v16(
    mut val: serde_json::Value,
) -> Result<serde_json::Value, PersistenceError> {
    let obj = val.as_object_mut().ok_or_else(|| {
        PersistenceError::MigrationFailed("Project JSON root must be an object".to_string())
    })?;
    if let Some(tracks) = obj
        .get_mut("tracks")
        .and_then(serde_json::Value::as_array_mut)
    {
        for track in tracks {
            let items = track
                .as_object_mut()
                .ok_or_else(|| {
                    PersistenceError::MigrationFailed(
                        "Track must be an object during v15 to v16 migration".to_string(),
                    )
                })?
                .entry("items")
                .or_insert_with(|| serde_json::json!([]));
            for item in items.as_array_mut().ok_or_else(|| {
                PersistenceError::MigrationFailed(
                    "Track items must be an array during v15 to v16 migration".to_string(),
                )
            })? {
                item.as_object_mut()
                    .ok_or_else(|| {
                        PersistenceError::MigrationFailed(
                            "Timeline item must be an object during v15 to v16 migration"
                                .to_string(),
                        )
                    })?
                    .entry("linked_item_id")
                    .or_insert(serde_json::Value::Null);
            }
        }
    }
    obj.insert("schema_version".to_string(), serde_json::json!(16));
    Ok(val)
}

/// Adds neutral SDR color controls to every timeline item.
pub fn migrate_v16_to_v17(
    mut val: serde_json::Value,
) -> Result<serde_json::Value, PersistenceError> {
    let obj = val.as_object_mut().ok_or_else(|| {
        PersistenceError::MigrationFailed("Project JSON root must be an object".to_string())
    })?;
    if let Some(tracks) = obj
        .get_mut("tracks")
        .and_then(serde_json::Value::as_array_mut)
    {
        for track in tracks {
            let items = track
                .as_object_mut()
                .ok_or_else(|| {
                    PersistenceError::MigrationFailed(
                        "Track must be an object during v16 to v17 migration".to_string(),
                    )
                })?
                .entry("items")
                .or_insert_with(|| serde_json::json!([]));
            for item in items.as_array_mut().ok_or_else(|| {
                PersistenceError::MigrationFailed(
                    "Track items must be an array during v16 to v17 migration".to_string(),
                )
            })? {
                item.as_object_mut()
                    .ok_or_else(|| PersistenceError::MigrationFailed("Timeline item must be an object during v16 to v17 migration".to_string()))?
                    .entry("color_adjustments")
                    .or_insert_with(|| serde_json::json!({"brightness":0.0,"contrast":1.0,"saturation":1.0,"temperature":0.0}));
            }
        }
    }
    obj.insert("schema_version".to_string(), serde_json::json!(17));
    Ok(val)
}

/// Migrates Schema v17 projects to Schema v18: adds the per-clip `color_lut` reference and
/// `chroma_key` settings (absent means none) and promotes `schema_version` to 18.
pub fn migrate_v17_to_v18(
    mut val: serde_json::Value,
) -> Result<serde_json::Value, PersistenceError> {
    let obj = val.as_object_mut().ok_or_else(|| {
        PersistenceError::MigrationFailed("Project JSON root must be an object".to_string())
    })?;
    if let Some(tracks) = obj
        .get_mut("tracks")
        .and_then(serde_json::Value::as_array_mut)
    {
        for track in tracks {
            let items = track
                .as_object_mut()
                .ok_or_else(|| {
                    PersistenceError::MigrationFailed(
                        "Track must be an object during v17 to v18 migration".to_string(),
                    )
                })?
                .entry("items")
                .or_insert_with(|| serde_json::json!([]));
            for item in items.as_array_mut().ok_or_else(|| {
                PersistenceError::MigrationFailed(
                    "Track items must be an array during v17 to v18 migration".to_string(),
                )
            })? {
                item.as_object_mut()
                    .ok_or_else(|| {
                        PersistenceError::MigrationFailed(
                            "Timeline item must be an object during v17 to v18 migration"
                                .to_string(),
                        )
                    })?
                    .entry("color_lut")
                    .or_insert(serde_json::Value::Null);
                item.as_object_mut()
                    .expect("timeline item object checked above")
                    .entry("chroma_key")
                    .or_insert(serde_json::Value::Null);
            }
        }
    }
    obj.insert("schema_version".to_string(), serde_json::json!(18));
    Ok(val)
}

/// Migrates Schema v18 projects to Schema v19: adds the per-clip `ducking` setting (absent
/// means none) and promotes `schema_version` to 19.
pub fn migrate_v18_to_v19(
    mut val: serde_json::Value,
) -> Result<serde_json::Value, PersistenceError> {
    let obj = val.as_object_mut().ok_or_else(|| {
        PersistenceError::MigrationFailed("Project JSON root must be an object".to_string())
    })?;
    if let Some(tracks) = obj
        .get_mut("tracks")
        .and_then(serde_json::Value::as_array_mut)
    {
        for track in tracks {
            let items = track
                .as_object_mut()
                .ok_or_else(|| {
                    PersistenceError::MigrationFailed(
                        "Track must be an object during v18 to v19 migration".to_string(),
                    )
                })?
                .entry("items")
                .or_insert_with(|| serde_json::json!([]));
            for item in items.as_array_mut().ok_or_else(|| {
                PersistenceError::MigrationFailed(
                    "Track items must be an array during v18 to v19 migration".to_string(),
                )
            })? {
                item.as_object_mut()
                    .ok_or_else(|| {
                        PersistenceError::MigrationFailed(
                            "Timeline item must be an object during v18 to v19 migration"
                                .to_string(),
                        )
                    })?
                    .entry("ducking")
                    .or_insert(serde_json::Value::Null);
                item.as_object_mut()
                    .expect("timeline item object checked above")
                    .entry("reverse")
                    .or_insert(serde_json::Value::Bool(false));
            }
        }
    }
    obj.insert("schema_version".to_string(), serde_json::json!(19));
    Ok(val)
}

/// Example migration for tests: Migrates hypothetical legacy format to v1
pub fn migrate_legacy_fixture(raw_json: &str) -> Result<String, PersistenceError> {
    let mut val: serde_json::Value = serde_json::from_str(raw_json)?;
    if let Some(obj) = val.as_object_mut() {
        if !obj.contains_key("schema_version") {
            obj.insert("schema_version".to_string(), serde_json::json!(1));
        }
        if !obj.contains_key("revision") {
            obj.insert("revision".to_string(), serde_json::json!(0));
        }
    }
    serde_json::to_string(&val).map_err(PersistenceError::Serialization)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::domain::time::{FrameRate, RationalTime, TimeRange};
    use crate::modules::domain::{
        Asset, AssetDimensions, ClientRecord, ClipSpeed, CutTransition, ItemKind, Keyframe,
        KeyframeProperty, MediaKind, PlacementMode, TextStyle, TimelineItem, Track, TrackKind,
        TransitionKind, VisualEffectPreset,
    };
    use crate::modules::history::{HistoryEntry, HistoryManager};
    use crate::modules::media::{AssetOnlineStatus, MediaCatalog};

    #[test]
    fn test_poc4_interrupted_save_preserves_valid_project() {
        let temp_dir = std::env::temp_dir().join("tinix_poc4_save_test");
        let _ = fs::create_dir_all(&temp_dir);
        let project_path = temp_dir.join("safe_project.tcproj");

        // 1. Save valid initial project
        let mut p1 = Project::new("Safe Initial Project".to_string());
        p1.revision = 1;
        save_project_atomic(&p1, &project_path).expect("initial save should succeed");

        let loaded1 = load_project(&project_path).expect("must load initial project");
        assert_eq!(loaded1.name, "Safe Initial Project");
        assert_eq!(loaded1.revision, 1);

        // 2. Simulate an interrupted/corrupt save by simulating crash/power loss during temp file creation
        let tmp_path = project_path.with_extension("tcproj.tmp");
        fs::write(&tmp_path, b"PARTIAL_CORRUPT_DATA_CRASH").expect("write temp crash file");

        // 3. Reopen canonical project: it MUST still load the original valid project!
        let loaded_after_crash =
            load_project(&project_path).expect("canonical file must survive crash");
        assert_eq!(loaded_after_crash.name, "Safe Initial Project");
        assert_eq!(loaded_after_crash.revision, 1);

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_poc4_cache_deletion_independence() {
        let temp_dir = std::env::temp_dir().join("tinix_poc4_cache_test");
        let _ = fs::create_dir_all(&temp_dir);
        let cache_dir = temp_dir.join(".tinix_cache");
        let _ = fs::create_dir_all(&cache_dir);
        fs::write(cache_dir.join("proxy_dummy.mp4"), b"proxy_data").unwrap();
        fs::write(cache_dir.join("thumb_dummy.jpg"), b"thumb_data").unwrap();

        let project_path = temp_dir.join("cache_test.tcproj");
        let p = Project::new("Cache Independence Project".to_string());
        save_project_atomic(&p, &project_path).unwrap();

        // Completely destroy cache directory
        fs::remove_dir_all(&cache_dir).unwrap();
        assert!(!cache_dir.exists());

        // Invariant: Project remains 100% valid and loadable!
        let loaded = load_project(&project_path).unwrap();
        assert_eq!(loaded.name, "Cache Independence Project");
        assert!(loaded.validate().is_ok());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_poc4_moved_media_and_relink() {
        let temp_dir = std::env::temp_dir().join("tinix_poc4_relink_test");
        let _ = fs::create_dir_all(&temp_dir);

        let initial_media_path = temp_dir.join("original_footage.mp4");
        let generated = crate::modules::media::resolver::MediaBinaryResolver::command("ffmpeg")
            .unwrap()
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc=duration=2:size=320x240:rate=30",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&initial_media_path)
            .status()
            .unwrap();
        assert!(generated.success());

        let mut project = Project::new("Relink Project".to_string());
        let asset_id = MediaCatalog::register_asset(
            &mut project,
            Some("ast_poc4".to_string()),
            "original_footage.mp4".to_string(),
            initial_media_path.to_string_lossy().to_string(),
            MediaKind::Video,
            None,
            15,
            None,
        );

        let mut track = Track::new("Video Track".to_string(), TrackKind::Video);
        let t_range = TimeRange::new(
            RationalTime::zero(FrameRate::FPS_30),
            RationalTime::from_frames(60, FrameRate::FPS_30),
        );
        let s_range = t_range;
        let clip = TimelineItem::new(
            Some(asset_id.clone()),
            "Original Clip".to_string(),
            ItemKind::Video,
            t_range,
            s_range,
        )
        .unwrap();
        track.items.push(clip);
        project.tracks.push(track);

        let proj_path = temp_dir.join("relink_project.tcproj");
        save_project_atomic(&project, &proj_path).unwrap();

        // 1. Move media to new location
        let moved_media_path = temp_dir.join("renamed_footage_moved.mp4");
        fs::rename(&initial_media_path, &moved_media_path).unwrap();

        // 2. Reopen project: media is offline, but clip is still on timeline!
        let mut loaded = load_project(&proj_path).unwrap();
        let status = MediaCatalog::check_asset_status(&loaded.assets[0]);
        assert!(matches!(status, AssetOnlineStatus::Offline { .. }));
        assert_eq!(loaded.tracks[0].items.len(), 1);
        assert_eq!(loaded.tracks[0].items[0].timeline_range.duration.value, 60);

        // 3. Relink asset to new path
        let relink_res = MediaCatalog::relink_asset(&mut loaded, &asset_id, &moved_media_path);
        assert!(relink_res.is_ok());

        // Verify media is now Online
        let status_after = MediaCatalog::check_asset_status(&loaded.assets[0]);
        assert_eq!(status_after, AssetOnlineStatus::Online);
        assert_eq!(loaded.assets[0].name, "renamed_footage_moved.mp4");

        // 4. Save and reopen again: all editorial state unchanged
        save_project_atomic(&loaded, &proj_path).unwrap();
        let reloaded = load_project(&proj_path).unwrap();
        assert_eq!(
            reloaded.tracks[0].items[0].timeline_range.duration.value,
            60
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_poc4_recovery_newer_detection() {
        let temp_dir = std::env::temp_dir().join("tinix_poc4_recovery_test");
        let _ = fs::create_dir_all(&temp_dir);

        let proj_path = temp_dir.join("user_saved.tcproj");
        let rec_path = temp_dir.join("user_saved.tcproj.recovery");

        let mut p_saved = Project::new("Recovery Test".to_string());
        p_saved.revision = 3;
        save_project_atomic(&p_saved, &proj_path).unwrap();

        let mut p_recovery = p_saved.clone();
        p_recovery.revision = 7; // Newer un-saved recovery!
        p_recovery.name = "Recovered edits".to_string();
        save_recovery_snapshot(&p_recovery, &rec_path).unwrap();

        let status = check_recovery_status(&proj_path, &rec_path).unwrap();
        match status {
            RecoveryStatus::NewerRecoveryAvailable {
                recovery_revision,
                project_revision,
                ..
            } => {
                assert_eq!(recovery_revision, 7);
                assert_eq!(project_revision, 3);
            }
            _ => panic!("Expected NewerRecoveryAvailable"),
        }

        let mut unrelated_recovery = Project::new("Different project".to_string());
        unrelated_recovery.revision = 8;
        save_recovery_snapshot(&unrelated_recovery, &rec_path).unwrap();
        assert_eq!(
            check_recovery_status(&proj_path, &rec_path).unwrap(),
            RecoveryStatus::NoRecoveryFound
        );

        let mut same_content = p_saved.clone();
        same_content.revision = 9;
        save_recovery_snapshot(&same_content, &rec_path).unwrap();
        assert_eq!(
            check_recovery_status(&proj_path, &rec_path).unwrap(),
            RecoveryStatus::RecoveryMatchesProject
        );

        let mut older_divergent = p_saved.clone();
        older_divergent.revision = 2;
        older_divergent.name = "Older recovery copy".to_string();
        save_recovery_snapshot(&older_divergent, &rec_path).unwrap();
        assert!(matches!(
            check_recovery_status(&proj_path, &rec_path).unwrap(),
            RecoveryStatus::RecoveryDiffersFromProject {
                recovery_revision: 2,
                project_revision: 3,
                ..
            }
        ));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn corrupt_recovery_is_reported_without_blocking_saved_project_or_changing_file() {
        let dir = std::env::temp_dir().join(format!(
            "tinixcut_corrupt_recovery_{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&dir).unwrap();
        let canonical = dir.join("saved.tinix");
        let recovery = dir.join("saved.tinix.recovery");
        save_project_atomic(&Project::new("Saved project".into()), &canonical).unwrap();
        let corrupt = b"{not a valid project";
        fs::write(&recovery, corrupt).unwrap();

        let status = check_recovery_status(&canonical, &recovery).unwrap();
        assert!(matches!(status, RecoveryStatus::RecoveryUnreadable { .. }));
        assert_eq!(fs::read(&recovery).unwrap(), corrupt);
        assert_eq!(load_project(&canonical).unwrap().name, "Saved project");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn recovery_snapshot_compare_and_swap_rejects_stale_or_conflicting_writer() {
        let dir = std::env::temp_dir().join(format!(
            "tinixcut_recovery_cas_{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("project.recovery");
        let mut current = Project::new("Current recovery".into());
        current.revision = 8;
        save_recovery_snapshot_if_newer(&current, &path).unwrap();

        let mut stale = current.clone();
        stale.revision = 7;
        stale.name = "Stale recovery".into();
        assert!(save_recovery_snapshot_if_newer(&stale, &path).is_err());

        let mut conflicting = current.clone();
        conflicting.name = "Different content at same revision".into();
        assert!(save_recovery_snapshot_if_newer(&conflicting, &path).is_err());
        assert_eq!(load_project(&path).unwrap(), current);

        save_recovery_snapshot_if_newer(&current, &path).unwrap();
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn discarding_a_stale_recovery_copy_unblocks_later_recovery_saves() {
        let dir = std::env::temp_dir().join(format!(
            "tinixcut_recovery_discard_{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&dir).unwrap();
        let recovery = dir.join("project.tcproj.recovery");
        let mut stale = Project::new("Stale".into());
        stale.revision = 6;
        save_recovery_snapshot_if_newer(&stale, &recovery).unwrap();
        let fingerprint = recovery_snapshot_fingerprint(&stale).unwrap();

        // Reopened at revision 1: the newer-revision recovery copy blocks every save.
        let mut reopened = stale.clone();
        reopened.revision = 1;
        reopened.name = "Edited after reopen".into();
        assert!(matches!(
            save_recovery_snapshot_if_newer(&reopened, &recovery),
            Err(PersistenceError::RevisionConflict { .. })
        ));

        // A wrong fingerprint or revision must not delete anything.
        assert!(discard_saved_recovery(&recovery, 6, "not-the-fingerprint").is_err());
        assert!(discard_saved_recovery(&recovery, 5, &fingerprint).is_err());
        assert!(recovery.exists());

        discard_saved_recovery(&recovery, 6, &fingerprint).unwrap();
        assert!(!recovery.exists());
        save_recovery_snapshot_if_newer(&reopened, &recovery).unwrap();
        // Discarding a missing copy is a no-op.
        fs::remove_file(&recovery).unwrap();
        discard_saved_recovery(&recovery, 6, &fingerprint).unwrap();
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn recovery_fingerprint_is_stable_across_loads_with_hash_map_registry() {
        let dir = std::env::temp_dir().join(format!(
            "tinixcut_recovery_fingerprint_{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("project.recovery");
        let mut project = Project::new("Fingerprint fixture".into());
        for client_id in ["client-z", "client-a", "client-m", "client-b"] {
            project.client_registry.clients.insert(
                client_id.to_string(),
                ClientRecord {
                    client_epoch: 1,
                    highest_sequence: 3,
                    last_active_revision: 4,
                    registered_at_revision: 0,
                },
            );
        }
        save_project_atomic(&project, &path).unwrap();
        let expected = recovery_snapshot_fingerprint(&project).unwrap();
        for _ in 0..8 {
            let loaded = load_project(&path).unwrap();
            assert_eq!(recovery_snapshot_fingerprint(&loaded).unwrap(), expected);
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn test_poc4_schema_migration_harness() {
        // Test 1: Unsupported future version is rejected
        let future_json = r#"{ "schema_version": 999, "name": "Future Project" }"#;
        let err = run_migrations_if_needed(future_json).expect_err("future version must fail");
        assert!(matches!(
            err,
            PersistenceError::UnsupportedSchemaVersion { found: 999, .. }
        ));

        // Test 2: Migration of legacy fixture to v1
        let legacy_json = r#"{ "project_id": "proj_legacy", "name": "Legacy Project" }"#;
        let migrated = migrate_legacy_fixture(legacy_json).unwrap();
        assert!(migrated.contains(r#""schema_version":1"#));
        assert!(migrated.contains(r#""revision":0"#));
    }

    #[test]
    fn test_poc4_history_undo_redo_preserves_state() {
        let mut history = HistoryManager::new(50);
        let mut p_initial = Project::new("History Base".to_string());
        p_initial.revision = 0;

        let mut p_after_add = p_initial.clone();
        p_after_add.revision = 1;
        let track = Track::new("Main Video".to_string(), TrackKind::Video);
        p_after_add.tracks.push(track);

        history.record_edit(HistoryEntry {
            description: "Add Video Track".to_string(),
            transaction_id: Some("tx_add_trk".to_string()),
            prior_snapshot: p_initial.clone(),
            resulting_snapshot: p_after_add.clone(),
        });

        let mut active = p_after_add.clone();
        assert_eq!(active.tracks.len(), 1);

        // Undo -> restores initial
        history.undo(&mut active).unwrap();
        assert_eq!(active.tracks.len(), 0);
        assert_eq!(active.revision, 0);

        // Redo -> restores after_add
        history.redo(&mut active).unwrap();
        assert_eq!(active.tracks.len(), 1);
        assert_eq!(active.revision, 1);
    }

    #[test]
    fn test_phase9_keyframe_and_transition_save_reopen_round_trip() {
        use crate::modules::composition::CompositionPlan;
        use crate::modules::domain::{Keyframe, KeyframeProperty, Transition, TransitionKind};

        let temp_dir = std::env::temp_dir().join("tinix_phase9_save_reopen_test");
        let _ = fs::create_dir_all(&temp_dir);
        let proj_path = temp_dir.join("phase9_keyframes_transitions.tcproj");

        let mut project = Project::new("Phase 9 Round Trip".to_string());
        let mut track = Track::new("Video Track".to_string(), TrackKind::Video);
        let rate = FrameRate::new(30, 1);
        let mut item = TimelineItem::new(
            Some("asset_vid".to_string()),
            "Video Clip".to_string(),
            ItemKind::Video,
            TimeRange::new(RationalTime::new(0, rate), RationalTime::new(300, rate)),
            TimeRange::new(RationalTime::new(0, rate), RationalTime::new(300, rate)),
        )
        .unwrap();

        // 1. Add keyframes across all supported animated properties
        let keyframe_specs = vec![
            (KeyframeProperty::PositionX, 0.0, 100.0),
            (KeyframeProperty::PositionY, 0.0, -50.0),
            (KeyframeProperty::ScaleX, 1.0, 2.0),
            (KeyframeProperty::ScaleY, 1.0, 2.0),
            (KeyframeProperty::Rotation, 0.0, 45.0),
            (KeyframeProperty::Opacity, 1.0, 0.2),
            (KeyframeProperty::Volume, 1.0, 0.5),
        ];

        for (prop, v_start, v_end) in &keyframe_specs {
            item.keyframes
                .push(Keyframe::new(*prop, RationalTime::new(0, rate), *v_start));
            item.keyframes
                .push(Keyframe::new(*prop, RationalTime::new(100, rate), *v_end));
        }

        // 2. Add transition
        item.transition = Some(Transition {
            kind: TransitionKind::FadeIn,
            duration: RationalTime::new(30, rate),
        });

        track.items.push(item);
        project.tracks.push(track);
        project.assets.push(crate::modules::domain::Asset {
            asset_id: "asset_vid".to_string(),
            name: "asset_vid.mp4".to_string(),
            path: "D:/dummy/asset_vid.mp4".to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(300, rate)),
            file_size: 1024,
            dimensions: None,
        });

        // Pre-save composition plan evaluation
        let plan_pre = CompositionPlan::from_project(&project).unwrap();
        let sample_time = RationalTime::new(50, rate);
        let pre_op = plan_pre.visual_layers[0].effective_opacity_at(sample_time);
        let pre_pos_x =
            plan_pre.visual_layers[0].evaluate_property(KeyframeProperty::PositionX, sample_time);
        let pre_pos_y =
            plan_pre.visual_layers[0].evaluate_property(KeyframeProperty::PositionY, sample_time);
        let pre_scale_x =
            plan_pre.visual_layers[0].evaluate_property(KeyframeProperty::ScaleX, sample_time);
        let pre_rot =
            plan_pre.visual_layers[0].evaluate_property(KeyframeProperty::Rotation, sample_time);
        let pre_vol = plan_pre.audio_layers[0].effective_volume_at(sample_time);

        // 3. Atomically save project
        save_project_atomic(&project, &proj_path).expect("save_project_atomic should succeed");

        // 4. Reopen and validate persistence
        let loaded = load_project(&proj_path).expect("load_project should succeed");
        assert_eq!(loaded.name, "Phase 9 Round Trip");
        assert_eq!(loaded.tracks.len(), 1);
        let loaded_item = &loaded.tracks[0].items[0];

        // Validate Transition persistence
        assert!(loaded_item.transition.is_some());
        let trans = loaded_item.transition.as_ref().unwrap();
        assert_eq!(trans.kind, TransitionKind::FadeIn);
        assert_eq!(trans.duration, RationalTime::new(30, rate));

        // Validate Keyframes persistence
        assert_eq!(loaded_item.keyframes.len(), keyframe_specs.len() * 2);
        for (prop, v_start, v_end) in &keyframe_specs {
            let kf0 = loaded_item
                .keyframes
                .iter()
                .find(|k| k.property == *prop && k.offset.value == 0)
                .expect("start keyframe must exist");
            assert_eq!(kf0.value, *v_start);

            let kf1 = loaded_item
                .keyframes
                .iter()
                .find(|k| k.property == *prop && k.offset.value == 100)
                .expect("end keyframe must exist");
            assert_eq!(kf1.value, *v_end);
        }

        // Validate Post-load composition plan evaluation matches pre-save exactly
        let plan_post = CompositionPlan::from_project(&loaded).unwrap();
        let post_op = plan_post.visual_layers[0].effective_opacity_at(sample_time);
        let post_pos_x =
            plan_post.visual_layers[0].evaluate_property(KeyframeProperty::PositionX, sample_time);
        let post_pos_y =
            plan_post.visual_layers[0].evaluate_property(KeyframeProperty::PositionY, sample_time);
        let post_scale_x =
            plan_post.visual_layers[0].evaluate_property(KeyframeProperty::ScaleX, sample_time);
        let post_rot =
            plan_post.visual_layers[0].evaluate_property(KeyframeProperty::Rotation, sample_time);
        let post_vol = plan_post.audio_layers[0].effective_volume_at(sample_time);

        assert_eq!(pre_op, post_op);
        assert_eq!(pre_pos_x, post_pos_x);
        assert_eq!(pre_pos_y, post_pos_y);
        assert_eq!(pre_scale_x, post_scale_x);
        assert_eq!(pre_rot, post_rot);
        assert_eq!(pre_vol, post_vol);

        // 5. Recovery snapshot round trip (C2)
        let rec_path = temp_dir.join("phase9_keyframes_transitions.tcproj.recovery");
        save_recovery_snapshot(&loaded, &rec_path).expect("save_recovery_snapshot should succeed");
        let rec_loaded = load_project(&rec_path).expect("loading recovery file should succeed");
        assert_eq!(
            rec_loaded.tracks[0].items[0].keyframes.len(),
            keyframe_specs.len() * 2
        );
        assert_eq!(
            rec_loaded.tracks[0].items[0]
                .transition
                .as_ref()
                .unwrap()
                .kind,
            TransitionKind::FadeIn
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_phase13_schema_v1_to_v2_migration_lossless_fades() {
        let v1_json = r#"{
            "schema_version": 1,
            "project_id": "proj_test_v1",
            "name": "Legacy V1 Project",
            "revision": 3,
            "settings": {
                "width": 1920,
                "height": 1080,
                "frame_rate": { "numerator": 30, "denominator": 1 },
                "sample_rate": 48000,
                "channels": 2
            },
            "assets": [],
            "tracks": [
                {
                    "track_id": "trk_v1",
                    "name": "Track 1",
                    "kind": "Video",
                    "muted": false,
                    "locked": false,
                    "items": [
                        {
                            "item_id": "clip_v1_1",
                            "name": "Clip with Fades",
                            "kind": "Video",
                            "timeline_range": {
                                "start": { "value": 0, "rate": { "numerator": 30, "denominator": 1 } },
                                "duration": { "value": 60, "rate": { "numerator": 30, "denominator": 1 } }
                            },
                            "source_range": {
                                "start": { "value": 0, "rate": { "numerator": 30, "denominator": 1 } },
                                "duration": { "value": 60, "rate": { "numerator": 30, "denominator": 1 } }
                            },
                            "transform": { "position_x": 0.0, "position_y": 0.0, "scale_x": 1.0, "scale_y": 1.0, "rotation": 0.0 },
                            "crop": { "top": 0.0, "bottom": 0.0, "left": 0.0, "right": 0.0 },
                            "opacity": 1.0,
                            "volume": 1.0,
                            "fade_in": { "value": 15, "rate": { "numerator": 30, "denominator": 1 } },
                            "fade_out": { "value": 15, "rate": { "numerator": 30, "denominator": 1 } },
                            "transition": {
                                "kind": "FadeIn",
                                "duration": { "value": 15, "rate": { "numerator": 30, "denominator": 1 } }
                            },
                            "keyframes": []
                        }
                    ]
                }
            ],
            "created_at": "2026-10-04T00:00:00Z",
            "updated_at": "2026-10-04T00:00:00Z"
        }"#;

        let migrated_str = run_migrations_if_needed(v1_json).expect("v1 migration should succeed");
        let project: Project =
            serde_json::from_str(&migrated_str).expect("migrated json should parse to Project");

        // Legacy projects migrate through schema v5 without changing editorial data.
        assert_eq!(project.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(project.tracks.len(), 1);

        // CD-09, CD-12: transitions initialized empty
        assert_eq!(project.tracks[0].transitions.len(), 0);

        // CD-10, CD-11: Item-level Fade semantics preserved losslessly
        let item = &project.tracks[0].items[0];
        assert_eq!(item.fade_in.unwrap().value, 15);
        assert_eq!(item.fade_out.unwrap().value, 15);
        assert!(item.transition.is_some());
        assert_eq!(
            item.transition.as_ref().unwrap().kind,
            TransitionKind::FadeIn
        );
        assert_eq!(item.transition.as_ref().unwrap().duration.value, 15);

        // Validation passes
        assert!(project.validate().is_ok());
    }

    #[test]
    fn test_phase18_schema_v1_v2_v3_migrate_with_empty_audio_crossfades() {
        let mut project = Project::new("Phase 18 migration fixture".to_string());
        project
            .tracks
            .push(Track::new("Audio".to_string(), TrackKind::Audio));
        let canonical = serde_json::to_value(project).unwrap();

        for version in 1..=3 {
            let mut legacy = canonical.clone();
            let root = legacy.as_object_mut().unwrap();
            root.insert("schema_version".to_string(), serde_json::json!(version));
            for track in root["tracks"].as_array_mut().unwrap() {
                track.as_object_mut().unwrap().remove("audio_transitions");
            }

            let migrated = run_migrations_if_needed(&legacy.to_string())
                .unwrap_or_else(|error| panic!("v{version} migration failed: {error}"));
            let loaded: Project = serde_json::from_str(&migrated).unwrap();
            assert_eq!(loaded.schema_version, CURRENT_SCHEMA_VERSION);
            assert_eq!(loaded.name, "Phase 18 migration fixture");
            assert_eq!(loaded.tracks.len(), 1);
            assert!(loaded.tracks[0].audio_transitions.is_empty());
            assert!(loaded.validate().is_ok());
        }
    }

    #[test]
    fn test_phase19_v4_text_items_receive_default_style_during_migration() {
        let rate = FrameRate::FPS_30;
        let mut project = Project::new("Phase 19 text migration".to_string());
        let mut track = Track::new("Text".to_string(), TrackKind::Video);
        let range = TimeRange::new(
            RationalTime::from_frames(0, rate),
            RationalTime::from_frames(30, rate),
        );
        let mut text_item = TimelineItem::new(
            None,
            "Legacy title".to_string(),
            ItemKind::Text,
            range,
            range,
        )
        .unwrap();
        text_item.text_content = Some("Legacy title".to_string());
        track.items.push(text_item);
        project.tracks.push(track);

        let mut legacy = serde_json::to_value(project).unwrap();
        let root = legacy.as_object_mut().unwrap();
        root.insert("schema_version".to_string(), serde_json::json!(4));
        let item = &mut root["tracks"][0]["items"][0];
        item.as_object_mut().unwrap().remove("text_style");
        item.as_object_mut().unwrap().remove("text_template");

        let migrated = run_migrations_if_needed(&legacy.to_string()).unwrap();
        let loaded: Project = serde_json::from_str(&migrated).unwrap();
        let item = &loaded.tracks[0].items[0];

        assert_eq!(loaded.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(
            item.text_style.as_ref().unwrap().font_family,
            TextStyle::default().font_family
        );
        assert_eq!(item.text_template, None);
        assert!(loaded.validate().is_ok());
    }

    #[test]
    fn test_phase18_audio_crossfade_save_reopen_round_trip() {
        use crate::modules::domain::AudioCrossfade;

        let temp_dir = std::env::temp_dir().join("tinix_p18_crossfade_persistence");
        let _ = fs::create_dir_all(&temp_dir);
        let project_path = temp_dir.join("audio_crossfade.tcproj");
        let rate = FrameRate::FPS_30;
        let mut project = Project::new("Phase 18 persistence".to_string());
        let mut track = Track::new("Audio".to_string(), TrackKind::Audio);

        for (index, start) in [0, 60].into_iter().enumerate() {
            let asset_id = format!("asset_audio_{index}");
            project.assets.push(crate::modules::domain::Asset {
                asset_id: asset_id.clone(),
                name: format!("{asset_id}.wav"),
                path: format!("D:/fixtures/{asset_id}.wav"),
                media_kind: MediaKind::Audio,
                duration: Some(RationalTime::new(130, rate)),
                file_size: 1,
                dimensions: None,
            });
            let timeline =
                TimeRange::new(RationalTime::new(start, rate), RationalTime::new(60, rate));
            let source = TimeRange::new(
                RationalTime::new(if index == 0 { 0 } else { 10 }, rate),
                RationalTime::new(60, rate),
            );
            track.items.push(
                TimelineItem::new(
                    Some(asset_id),
                    format!("Audio clip {index}"),
                    ItemKind::Audio,
                    timeline,
                    source,
                )
                .unwrap(),
            );
        }
        track.audio_transitions.push(AudioCrossfade {
            transition_id: "audio_xfade_persist".to_string(),
            track_id: track.track_id.clone(),
            left_item_id: track.items[0].item_id.clone(),
            right_item_id: track.items[1].item_id.clone(),
            duration: RationalTime::new(20, rate),
        });
        let expected = track.audio_transitions[0].clone();
        project.tracks.push(track);

        save_project_atomic(&project, &project_path).expect("save should succeed");
        let loaded = load_project(&project_path).expect("reopen should succeed");
        assert_eq!(loaded.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(loaded.tracks[0].audio_transitions, vec![expected]);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_phase13_schema_v2_save_and_reopen_exact_round_trip() {
        let temp_dir = std::env::temp_dir().join("tinix_p13_save_test");
        let _ = fs::create_dir_all(&temp_dir);
        let project_path = temp_dir.join("v2_round_trip.tcproj");

        let rate = FrameRate::FPS_30;
        let mut project = Project::new("Schema v2 Project".to_string());
        assert_eq!(project.schema_version, CURRENT_SCHEMA_VERSION);

        let mut track = Track::new("Video Track 1".to_string(), TrackKind::Video);
        let t1 = TimeRange::new(
            RationalTime::from_frames(0, rate),
            RationalTime::from_frames(60, rate),
        );
        let item1 = TimelineItem::new(None, "Clip 1".to_string(), ItemKind::Video, t1, t1).unwrap();
        let item1_id = item1.item_id.clone();

        let t2 = TimeRange::new(
            RationalTime::from_frames(60, rate),
            RationalTime::from_frames(60, rate),
        );
        let item2 = TimelineItem::new(None, "Clip 2".to_string(), ItemKind::Video, t2, t2).unwrap();
        let item2_id = item2.item_id.clone();

        track.items.push(item1);
        track.items.push(item2);

        let cut_trans = CutTransition::new(
            track.track_id.clone(),
            item1_id.clone(),
            item2_id.clone(),
            TransitionKind::CrossDissolve,
            RationalTime::from_frames(20, rate),
        )
        .unwrap();
        let trans_id = cut_trans.transition_id.clone();
        track.transitions.push(cut_trans);
        project.tracks.push(track);

        save_project_atomic(&project, &project_path).expect("v2 save should succeed");

        let loaded = load_project(&project_path).expect("v2 load should succeed");
        assert_eq!(loaded.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(loaded.tracks[0].transitions.len(), 1);

        let loaded_trans = &loaded.tracks[0].transitions[0];
        assert_eq!(loaded_trans.transition_id, trans_id);
        assert_eq!(loaded_trans.left_item_id, item1_id);
        assert_eq!(loaded_trans.right_item_id, item2_id);
        assert_eq!(loaded_trans.kind, TransitionKind::CrossDissolve);
        assert_eq!(loaded_trans.duration.value, 20);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_phase13_malformed_legacy_crossdissolve_rejected_on_migration() {
        let malformed_v1_json = r#"{
            "schema_version": 1,
            "project_id": "proj_malformed",
            "name": "Malformed Legacy",
            "tracks": [
                {
                    "track_id": "trk_1",
                    "name": "Track 1",
                    "kind": "Video",
                    "items": [
                        {
                            "item_id": "clip_bad",
                            "name": "Bad Clip",
                            "kind": "Video",
                            "transition": {
                                "kind": "CrossDissolve",
                                "duration": { "value": 30, "rate": { "numerator": 30, "denominator": 1 } }
                            }
                        }
                    ]
                }
            ]
        }"#;

        let res = run_migrations_if_needed(malformed_v1_json);
        assert!(matches!(res, Err(PersistenceError::MigrationFailed(_))));
    }

    #[test]
    fn test_phase13_migration_failure_leaves_source_file_safe() {
        let temp_dir = std::env::temp_dir().join("tinix_p13_safe_migration_test");
        let _ = fs::create_dir_all(&temp_dir);
        let project_path = temp_dir.join("corrupted_legacy.tcproj");

        let malformed_v1 = r#"{
            "schema_version": 1,
            "project_id": "proj_safe_test",
            "name": "Untouched Legacy",
            "tracks": [
                {
                    "track_id": "trk_1",
                    "name": "Track 1",
                    "kind": "Video",
                    "items": [
                        {
                            "item_id": "clip_bad",
                            "name": "Bad Clip",
                            "kind": "Video",
                            "transition": {
                                "kind": "CrossDissolve",
                                "duration": { "value": 30, "rate": { "numerator": 30, "denominator": 1 } }
                            }
                        }
                    ]
                }
            ]
        }"#;

        fs::write(&project_path, malformed_v1).unwrap();
        let res = load_project(&project_path);
        assert!(res.is_err());

        // Verify original file content is intact and was NOT overwritten
        let on_disk = fs::read_to_string(&project_path).unwrap();
        assert_eq!(on_disk, malformed_v1);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn v5_migration_preserves_dimensions_and_defaults_existing_clips_to_fit() {
        let rate = FrameRate::FPS_30;
        let mut project = Project::new("Phase 21 legacy project".to_string());
        project.assets.push(Asset {
            asset_id: "ast_vertical".to_string(),
            name: "vertical.mp4".to_string(),
            path: "D:/media/vertical.mp4".to_string(),
            media_kind: MediaKind::Video,
            duration: None,
            file_size: 1,
            dimensions: Some(AssetDimensions {
                width: 1080,
                height: 1920,
            }),
        });

        let mut track = Track::new("Video".to_string(), TrackKind::Video);
        track.items.push(
            TimelineItem::new(
                Some("ast_vertical".to_string()),
                "Vertical clip".to_string(),
                ItemKind::Video,
                TimeRange::new(
                    RationalTime::zero(rate),
                    RationalTime::from_frames(30, rate),
                ),
                TimeRange::new(
                    RationalTime::zero(rate),
                    RationalTime::from_frames(30, rate),
                ),
            )
            .expect("valid legacy clip"),
        );
        project.tracks.push(track);

        let mut v5 = serde_json::to_value(project).expect("serialize project");
        v5["schema_version"] = serde_json::json!(5);
        v5["assets"][0]
            .as_object_mut()
            .expect("asset object")
            .remove("dimensions");
        v5["tracks"][0]["items"][0]
            .as_object_mut()
            .expect("item object")
            .remove("placement_mode");

        let migrated_v6 = migrate_v5_to_v6(v5).expect("v5 to v6 migration");
        let migrated_v7 = migrate_v6_to_v7(migrated_v6).expect("v6 to v7 migration");
        let migrated_v8 = migrate_v7_to_v8(migrated_v7).expect("v7 to v8 migration");
        let migrated_v9 = migrate_v8_to_v9(migrated_v8).expect("v8 to v9 migration");
        let migrated_v10 = migrate_v9_to_v10(migrated_v9).expect("v9 to v10 migration");
        let migrated = migrate_v10_to_v11(migrated_v10).expect("v10 to v11 migration");
        let migrated_json =
            serde_json::to_string(&migrated).expect("serialize migrated v11 project");
        let migrated = run_migrations_if_needed(&migrated_json).expect("migrate v11 to current");
        let loaded: Project = serde_json::from_str(&migrated).expect("deserialize current project");

        assert_eq!(loaded.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(loaded.assets[0].dimensions, None);
        assert_eq!(loaded.tracks[0].items[0].placement_mode, PlacementMode::Fit);
        assert_eq!(loaded.tracks[0].items[0].speed, ClipSpeed::Normal);
        loaded.validate().expect("migrated project remains valid");
    }

    #[test]
    fn v6_migration_defaults_canvas_background_to_black() {
        let mut v6 = serde_json::to_value(Project::new("Phase 21 v6 project".to_string()))
            .expect("serialize project");
        v6["schema_version"] = serde_json::json!(6);
        v6["settings"]
            .as_object_mut()
            .expect("settings object")
            .remove("canvas_background_color");

        let migrated_v7 = migrate_v6_to_v7(v6).expect("v6 migration");
        let migrated_v8 = migrate_v7_to_v8(migrated_v7).expect("v7 to v8 migration");
        let migrated_v9 = migrate_v8_to_v9(migrated_v8).expect("v8 to v9 migration");
        let migrated_v10 = migrate_v9_to_v10(migrated_v9).expect("v9 to v10 migration");
        let migrated = migrate_v10_to_v11(migrated_v10).expect("v10 to v11 migration");
        let migrated_json =
            serde_json::to_string(&migrated).expect("serialize migrated v11 project");
        let migrated = run_migrations_if_needed(&migrated_json).expect("migrate v11 to current");
        let loaded: Project = serde_json::from_str(&migrated).expect("deserialize current project");

        assert_eq!(loaded.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(loaded.settings.canvas_background_color, "#000000");
        loaded.validate().expect("migrated project remains valid");
    }

    #[test]
    fn v7_migration_defaults_visual_effect_to_none() {
        let rate = FrameRate::FPS_30;
        let mut project = Project::new("Phase 22 v7 project".to_string());
        let mut track = Track::new("Video".to_string(), TrackKind::Video);
        track.items.push(
            TimelineItem::new(
                None,
                "Legacy clip".to_string(),
                ItemKind::Video,
                TimeRange::new(
                    RationalTime::zero(rate),
                    RationalTime::from_frames(30, rate),
                ),
                TimeRange::new(
                    RationalTime::zero(rate),
                    RationalTime::from_frames(30, rate),
                ),
            )
            .expect("valid legacy clip"),
        );
        project.tracks.push(track);

        let mut v7 = serde_json::to_value(project).expect("serialize project");
        v7["schema_version"] = serde_json::json!(7);
        v7["tracks"][0]["items"][0]
            .as_object_mut()
            .expect("item object")
            .remove("visual_effect");

        let migrated_v8 = migrate_v7_to_v8(v7).expect("v7 to v8 migration");
        let migrated_v9 = migrate_v8_to_v9(migrated_v8).expect("v8 to v9 migration");
        let migrated_v10 = migrate_v9_to_v10(migrated_v9).expect("v9 to v10 migration");
        let migrated = migrate_v10_to_v11(migrated_v10).expect("v10 to v11 migration");
        let migrated_json =
            serde_json::to_string(&migrated).expect("serialize migrated v11 project");
        let migrated = run_migrations_if_needed(&migrated_json).expect("migrate v11 to current");
        let loaded: Project = serde_json::from_str(&migrated).expect("deserialize current project");

        assert_eq!(loaded.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(
            loaded.tracks[0].items[0].visual_effect,
            VisualEffectPreset::None
        );
        loaded.validate().expect("migrated project remains valid");
    }

    #[test]
    fn test_v9_to_v10_migration_preserves_data_and_defaults_rich_text_to_none() {
        let rate = FrameRate::FPS_30;
        let mut project = Project::new("Phase 29 v9 fixture".to_string());
        let mut track = Track::new("Text Track".to_string(), TrackKind::Video);
        let mut text_item = TimelineItem::new(
            None,
            "Caption clip".to_string(),
            ItemKind::Text,
            TimeRange::new(
                RationalTime::zero(rate),
                RationalTime::from_frames(60, rate),
            ),
            TimeRange::new(
                RationalTime::zero(rate),
                RationalTime::from_frames(60, rate),
            ),
        )
        .expect("valid text clip");

        text_item.text_content = Some("Legacy text content".to_string());
        text_item.keyframes.push(Keyframe::new(
            KeyframeProperty::Opacity,
            RationalTime::zero(rate),
            1.0,
        ));
        track.items.push(text_item);
        project.tracks.push(track);

        let mut v9_val = serde_json::to_value(&project).expect("serialize v9 project");
        v9_val["schema_version"] = serde_json::json!(9);

        let raw_v9 = serde_json::to_string(&v9_val).expect("to string");
        let migrated_str = run_migrations_if_needed(&raw_v9).expect("run migration");
        let loaded: Project = serde_json::from_str(&migrated_str).expect("deserialize");

        assert_eq!(loaded.schema_version, CURRENT_SCHEMA_VERSION);
        let item = &loaded.tracks[0].items[0];
        assert_eq!(item.text_content.as_deref(), Some("Legacy text content"));
        assert!(item.text_rich.is_none());
        assert!(item.text_animation.is_none());
        assert_eq!(item.keyframes.len(), 1);
        loaded.validate().expect("migrated project validates");
    }

    #[test]
    fn test_future_schema_rejected() {
        // Schema 19 is CURRENT_SCHEMA_VERSION; future schema version 20 must be rejected.
        let raw_json = r#"{"schema_version": 20, "name": "Future Project"}"#;
        let err = run_migrations_if_needed(raw_json).unwrap_err();
        match err {
            PersistenceError::UnsupportedSchemaVersion {
                found,
                max_supported,
            } => {
                assert_eq!(found, 20);
                assert_eq!(max_supported, CURRENT_SCHEMA_VERSION);
            }
            other => panic!("Expected UnsupportedSchemaVersion, got {:?}", other),
        }
    }

    #[test]
    fn test_schema_17_migrates_to_18_with_empty_lut_and_preserves_items() {
        let raw = r#"{"schema_version": 17, "tracks": [{"items": [{"item_id": "itm_a", "color_adjustments": {"brightness":0.1,"contrast":1.0,"saturation":1.0,"temperature":0.0}}]}]}"#;
        let migrated: serde_json::Value =
            serde_json::from_str(&run_migrations_if_needed(raw).unwrap()).unwrap();
        assert_eq!(migrated["schema_version"], CURRENT_SCHEMA_VERSION);
        let item = &migrated["tracks"][0]["items"][0];
        assert!(item["ducking"].is_null());
        assert_eq!(item["reverse"], false);
        assert!(item["color_lut"].is_null());
        assert!(item.as_object().unwrap().contains_key("color_lut"));
        assert!(item["chroma_key"].is_null());
        assert_eq!(item["color_adjustments"]["brightness"], 0.1);
    }

    #[test]
    fn test_clip_lut_round_trips_through_serialization() {
        let lut = crate::modules::domain::ClipLut {
            name: "Warm".into(),
            path: "C:/luts/warm.cube".into(),
            sha256: "0f".repeat(32),
        };
        let json = serde_json::to_string(&lut).unwrap();
        let back: crate::modules::domain::ClipLut = serde_json::from_str(&json).unwrap();
        assert_eq!(back, lut);
    }

    #[test]
    fn test_schema_11_migrates_to_current_with_empty_markers() {
        let mut project = Project::new("marker migration".to_string());
        project.schema_version = 11;
        let mut value = serde_json::to_value(project).expect("serialize legacy project");
        value.as_object_mut().unwrap().remove("markers");
        let raw = serde_json::to_string(&value).expect("serialize v11 JSON");
        let migrated_json = run_migrations_if_needed(&raw).expect("run v11 to v12 migration");
        let migrated: serde_json::Value = serde_json::from_str(&migrated_json).unwrap();
        assert_eq!(migrated["schema_version"], CURRENT_SCHEMA_VERSION);
        assert_eq!(migrated["markers"], serde_json::json!([]));
        let loaded: Project =
            serde_json::from_value(migrated).expect("deserialize current project");
        assert_eq!(loaded.schema_version, CURRENT_SCHEMA_VERSION);
        assert!(loaded.markers.is_empty());
        loaded.validate().expect("current project validates");
    }

    #[test]
    fn test_schema_12_migrates_embedded_audio_to_enabled_by_default() {
        let rate = FrameRate::FPS_30;
        let mut project = Project::new("embedded audio migration".to_string());
        project.assets.push(Asset {
            asset_id: "asset_video".to_string(),
            name: "video.mp4".to_string(),
            path: "D:/media/video.mp4".to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::from_frames(30, rate)),
            file_size: 1,
            dimensions: None,
        });
        let mut track = Track::new("Video".to_string(), TrackKind::Video);
        track.items.push(
            TimelineItem::new(
                Some("asset_video".to_string()),
                "Video clip".to_string(),
                ItemKind::Video,
                TimeRange::new(
                    RationalTime::zero(rate),
                    RationalTime::from_frames(30, rate),
                ),
                TimeRange::new(
                    RationalTime::zero(rate),
                    RationalTime::from_frames(30, rate),
                ),
            )
            .expect("video item"),
        );
        project.tracks.push(track);
        let mut value = serde_json::to_value(project).expect("serialize fixture");
        value["schema_version"] = serde_json::json!(12);
        value["tracks"][0]["items"][0]
            .as_object_mut()
            .unwrap()
            .remove("embedded_audio_enabled");

        let migrated = run_migrations_if_needed(&serde_json::to_string(&value).unwrap())
            .expect("migrate v12 project");
        let loaded: Project =
            serde_json::from_str(&migrated).expect("deserialize migrated project");
        assert_eq!(loaded.schema_version, CURRENT_SCHEMA_VERSION);
        assert!(loaded.tracks[0].items[0].embedded_audio_enabled);
        loaded.validate().expect("migrated project validates");
    }

    #[test]
    fn test_schema_13_migrates_track_solo_to_off_by_default() {
        let mut project = Project::new("track solo migration".to_string());
        project
            .tracks
            .push(Track::new("Voice".to_string(), TrackKind::Audio));
        let mut value = serde_json::to_value(project).expect("serialize fixture");
        value["schema_version"] = serde_json::json!(13);
        value["tracks"][0].as_object_mut().unwrap().remove("solo");
        let migrated = run_migrations_if_needed(&serde_json::to_string(&value).unwrap())
            .expect("migrate v13 project");
        let loaded: Project =
            serde_json::from_str(&migrated).expect("deserialize migrated project");
        assert_eq!(loaded.schema_version, CURRENT_SCHEMA_VERSION);
        assert!(!loaded.tracks[0].solo);
        loaded.validate().expect("migrated project validates");
    }

    #[test]
    fn test_schema_14_migrates_track_gain_to_unity_by_default() {
        let mut project = Project::new("track gain migration".to_string());
        project
            .tracks
            .push(Track::new("Voice".to_string(), TrackKind::Audio));
        let mut value = serde_json::to_value(project).expect("serialize fixture");
        value["schema_version"] = serde_json::json!(14);
        value["tracks"][0].as_object_mut().unwrap().remove("volume");
        let migrated = run_migrations_if_needed(&serde_json::to_string(&value).unwrap())
            .expect("migrate v14 project");
        let loaded: Project =
            serde_json::from_str(&migrated).expect("deserialize migrated project");
        assert_eq!(loaded.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(loaded.tracks[0].volume, 1.0);
        loaded.validate().expect("migrated project validates");
    }

    #[test]
    fn test_schema_15_migrates_linked_item_ids_to_none() {
        let rate = crate::modules::domain::time::FrameRate::FPS_30;
        let mut project = Project::new("link migration".to_string());
        project.assets.push(crate::modules::domain::Asset {
            asset_id: "asset_v".to_string(),
            name: "video.mp4".to_string(),
            path: "D:/media/video.mp4".to_string(),
            media_kind: crate::modules::domain::MediaKind::Video,
            duration: Some(crate::modules::domain::time::RationalTime::from_frames(
                30, rate,
            )),
            file_size: 1,
            dimensions: None,
        });
        let mut video = crate::modules::domain::Track::new(
            "Video".to_string(),
            crate::modules::domain::TrackKind::Video,
        );
        video.items.push(
            crate::modules::domain::TimelineItem::new(
                Some("asset_v".to_string()),
                "Video".to_string(),
                crate::modules::domain::ItemKind::Video,
                crate::modules::domain::time::TimeRange::new(
                    crate::modules::domain::time::RationalTime::zero(rate),
                    crate::modules::domain::time::RationalTime::from_frames(30, rate),
                ),
                crate::modules::domain::time::TimeRange::new(
                    crate::modules::domain::time::RationalTime::zero(rate),
                    crate::modules::domain::time::RationalTime::from_frames(30, rate),
                ),
            )
            .expect("video item"),
        );
        project.tracks.push(video);
        let mut value = serde_json::to_value(project).expect("serialize v15 fixture");
        value["schema_version"] = serde_json::json!(15);
        value["tracks"][0]["items"][0]
            .as_object_mut()
            .unwrap()
            .remove("linked_item_id");

        let migrated = run_migrations_if_needed(&serde_json::to_string(&value).unwrap())
            .expect("migrate v15 project");
        let loaded: Project = serde_json::from_str(&migrated).expect("deserialize v16 project");
        assert_eq!(loaded.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(loaded.tracks[0].items[0].linked_item_id, None);
        assert_eq!(
            loaded.tracks[0].items[0].color_adjustments,
            Default::default()
        );
        loaded.validate().expect("migrated project validates");
    }

    #[test]
    fn test_schema_16_migrates_color_adjustments_to_neutral() {
        let mut project = Project::new("color migration".to_string());
        let mut track = crate::modules::domain::Track::new(
            "Video".to_string(),
            crate::modules::domain::TrackKind::Video,
        );
        track.items.push(
            crate::modules::domain::TimelineItem::new(
                None,
                "Generated".to_string(),
                crate::modules::domain::ItemKind::Image,
                crate::modules::domain::time::TimeRange::new(
                    crate::modules::domain::time::RationalTime::zero(
                        crate::modules::domain::time::FrameRate::FPS_30,
                    ),
                    crate::modules::domain::time::RationalTime::from_frames(
                        30,
                        crate::modules::domain::time::FrameRate::FPS_30,
                    ),
                ),
                crate::modules::domain::time::TimeRange::new(
                    crate::modules::domain::time::RationalTime::zero(
                        crate::modules::domain::time::FrameRate::FPS_30,
                    ),
                    crate::modules::domain::time::RationalTime::from_frames(
                        30,
                        crate::modules::domain::time::FrameRate::FPS_30,
                    ),
                ),
            )
            .unwrap(),
        );
        project.tracks.push(track);
        let mut value = serde_json::to_value(project).unwrap();
        value["schema_version"] = serde_json::json!(16);
        value["tracks"][0]["items"][0]
            .as_object_mut()
            .unwrap()
            .remove("color_adjustments");
        let migrated = run_migrations_if_needed(&serde_json::to_string(&value).unwrap()).unwrap();
        let loaded: Project = serde_json::from_str(&migrated).unwrap();
        assert_eq!(loaded.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(
            loaded.tracks[0].items[0].color_adjustments,
            Default::default()
        );
        loaded.validate().expect("migrated project validates");
    }

    #[test]
    fn test_revision_guard_rejects_stale_writer_and_preserves_newer_project() {
        let dir = std::env::temp_dir().join(format!(
            "tinixcut_revision_guard_{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("shared.tinix");
        let initial = Project::new("Initial".to_string());
        save_project_atomic(&initial, &path).unwrap();

        let mut mcp_update = initial.clone();
        mcp_update.revision = 1;
        mcp_update.name = "MCP update".to_string();
        save_project_atomic_if_revision(&mcp_update, &path, Some(0)).unwrap();

        let mut stale_ui_update = initial;
        stale_ui_update.revision = 1;
        stale_ui_update.name = "Stale UI update".to_string();
        let error = save_project_atomic_if_revision(&stale_ui_update, &path, Some(0))
            .expect_err("stale UI save must be rejected");
        assert!(matches!(
            error,
            PersistenceError::RevisionConflict {
                expected: Some(0),
                actual: Some(1)
            }
        ));

        let saved = load_project(&path).unwrap();
        assert_eq!(saved.revision, 1);
        assert_eq!(saved.name, "MCP update");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn test_revision_guard_serializes_concurrent_writers_across_file_handles() {
        use std::sync::{Arc, Barrier};

        let dir = std::env::temp_dir().join(format!(
            "tinixcut_revision_race_{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("shared.tinix");
        let initial = Project::new("Initial".to_string());
        save_project_atomic(&initial, &path).unwrap();

        let barrier = Arc::new(Barrier::new(3));
        let handles = ["Writer A", "Writer B"].map(|name| {
            let barrier = Arc::clone(&barrier);
            let path = path.clone();
            let mut project = initial.clone();
            project.revision = 1;
            project.name = name.to_string();
            std::thread::spawn(move || {
                barrier.wait();
                save_project_atomic_if_revision(&project, &path, Some(0))
            })
        });

        barrier.wait();
        let results = handles.map(|handle| handle.join().unwrap());
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, Err(PersistenceError::RevisionConflict { .. })))
                .count(),
            1
        );
        let saved = load_project(&path).unwrap();
        assert_eq!(saved.revision, 1);
        assert!(saved.name == "Writer A" || saved.name == "Writer B");
        fs::remove_dir_all(dir).unwrap();
    }
}
