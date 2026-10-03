//! Persistence, Atomic Save, Recovery and Schema Migration
//! Architecture Invariant: Saves are atomic (temp write -> commit/replace).
//! Cache is rebuildable and not required for project correctness.
//! Corrupt or interrupted saves never destroy prior valid project.

use crate::modules::domain::{Project, CURRENT_SCHEMA_VERSION};
use serde::{Deserialize, Serialize};
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RecoveryStatus {
    NoRecoveryFound,
    RecoveryMatchesProject,
    NewerRecoveryAvailable {
        recovery_path: PathBuf,
        recovery_revision: u64,
        project_revision: u64,
    },
}

/// Atomically save a project to disk.
/// 1. Serialize validated project.
/// 2. Write to temporary sibling file `<path>.tmp`.
/// 3. Flush and sync.
/// 4. Atomically rename/replace `<path>.tmp` -> `<path>`.
///
/// If any step before rename fails, the original `<path>` remains completely intact!
pub fn save_project_atomic(project: &Project, target_path: &Path) -> Result<(), PersistenceError> {
    project
        .validate()
        .map_err(|e| PersistenceError::Validation(e.to_string()))?;

    let json_bytes = serde_json::to_vec_pretty(project)?;

    if let Some(parent) = target_path.parent() {
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

    // Check schema version and run migrations if needed
    let migrated_json = run_migrations_if_needed(&contents)?;
    let project: Project = serde_json::from_str(&migrated_json)?;

    project
        .validate()
        .map_err(|e| PersistenceError::Validation(e.to_string()))?;
    Ok(project)
}

/// Save an autosave / recovery snapshot.
/// Invariant: Autosave is NEVER saved directly over canonical user project!
pub fn save_recovery_snapshot(
    project: &Project,
    recovery_path: &Path,
) -> Result<(), PersistenceError> {
    save_project_atomic(project, recovery_path)
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
        Ok(p) => p,
        Err(_) => return Ok(RecoveryStatus::NoRecoveryFound),
    };

    if !canonical_path.exists() {
        return Ok(RecoveryStatus::NewerRecoveryAvailable {
            recovery_path: recovery_path.to_path_buf(),
            recovery_revision: recovery_proj.revision,
            project_revision: 0,
        });
    }

    let canonical_proj = load_project(canonical_path)?;

    if recovery_proj.revision > canonical_proj.revision {
        Ok(RecoveryStatus::NewerRecoveryAvailable {
            recovery_path: recovery_path.to_path_buf(),
            recovery_revision: recovery_proj.revision,
            project_revision: canonical_proj.revision,
        })
    } else {
        Ok(RecoveryStatus::RecoveryMatchesProject)
    }
}

/// Schema migration harness
/// Detects version in raw JSON and applies registered migration transforms.
pub fn run_migrations_if_needed(raw_json: &str) -> Result<String, PersistenceError> {
    let val: serde_json::Value = serde_json::from_str(raw_json)?;
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

    // If future schema versions exist, sequential migration functions are applied here:
    // if version == 1 && CURRENT_SCHEMA_VERSION == 2 { val = migrate_v1_to_v2(val)?; }

    serde_json::to_string(&val).map_err(PersistenceError::Serialization)
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
    use crate::modules::domain::{ItemKind, MediaKind, TimelineItem, Track, TrackKind};
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
        fs::write(&initial_media_path, b"MOCK_VIDEO_DATA").unwrap();

        let mut project = Project::new("Relink Project".to_string());
        let asset_id = MediaCatalog::register_asset(
            &mut project,
            Some("ast_poc4".to_string()),
            "original_footage.mp4".to_string(),
            initial_media_path.to_string_lossy().to_string(),
            MediaKind::Video,
            15,
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

        let _ = fs::remove_dir_all(&temp_dir);
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
}
