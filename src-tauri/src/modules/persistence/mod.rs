//! Persistence, Atomic Save, Recovery and Schema Migration
//! Architecture Invariant: Saves are atomic (temp write -> commit/replace).
//! Cache is rebuildable and not required for project correctness.
//! Corrupt or interrupted saves never destroy prior valid project.

use crate::modules::domain::{Project, TextStyle, CURRENT_SCHEMA_VERSION};
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
    let clean_contents = contents.trim_start_matches('\u{feff}');

    // Check schema version and run migrations if needed
    let migrated_json = run_migrations_if_needed(clean_contents)?;
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
        Asset, AssetDimensions, ClipSpeed, CutTransition, ItemKind, Keyframe, KeyframeProperty,
        MediaKind, PlacementMode, TextStyle, TimelineItem, Track, TrackKind, TransitionKind,
        VisualEffectPreset,
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
        fs::write(&initial_media_path, b"MOCK_VIDEO_DATA").unwrap();

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
        let loaded: Project =
            serde_json::from_value(migrated).expect("deserialize current project");

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
        let loaded: Project = serde_json::from_value(migrated).expect("deserialize v8 project");

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
        let loaded: Project = serde_json::from_value(migrated).expect("deserialize v8 project");

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
    fn test_schema_11_rejected() {
        let raw_json = r#"{"schema_version": 12, "name": "Future Project"}"#;
        let err = run_migrations_if_needed(raw_json).unwrap_err();
        match err {
            PersistenceError::UnsupportedSchemaVersion {
                found,
                max_supported,
            } => {
                assert_eq!(found, 12);
                assert_eq!(max_supported, CURRENT_SCHEMA_VERSION);
            }
            other => panic!("Expected UnsupportedSchemaVersion, got {:?}", other),
        }
    }
}
