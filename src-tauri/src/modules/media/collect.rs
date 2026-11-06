use crate::modules::automation::clone_project_as_variant;
use crate::modules::command::{
    CommandEnvelope, CommandOperation, CommandService, OperationEnvelope,
};
use crate::modules::domain::{Project, CURRENT_SCHEMA_VERSION};
use crate::modules::persistence::save_project_atomic_if_revision;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectedMediaReceipt {
    pub project_path: String,
    pub media_directory: String,
    pub project: Project,
    pub copied_assets: usize,
    pub total_bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CollectionManifest {
    schema_version: u32,
    project_id: String,
    source_project_id: String,
    assets: Vec<CollectionManifestAsset>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CollectionManifestAsset {
    asset_id: String,
    original_name: String,
    relative_path: String,
    size_bytes: u64,
    sha256: String,
}

pub fn collect_project_media(
    source: &Project,
    destination_directory: &Path,
    is_allowed: impl Fn(&Path) -> bool,
) -> Result<CollectedMediaReceipt, String> {
    let parent = absolute_destination(destination_directory)?;
    if !parent.is_dir() {
        return Err(format!(
            "Collection destination directory does not exist: {}",
            parent.display()
        ));
    }
    if !is_allowed(&parent) {
        return Err(format!(
            "Collect media requires explicit destination directory access: {}",
            parent.display()
        ));
    }
    let stem = safe_project_stem(&source.name);
    let project_path = parent.join(format!("{stem}.tinix"));
    if !project_path.extension().is_some_and(|extension| {
        matches!(
            extension.to_string_lossy().to_ascii_lowercase().as_str(),
            "tinix" | "tcproj"
        )
    }) {
        return Err("Collection destination must use the .tinix or .tcproj extension".to_string());
    }
    if project_path.exists() {
        return Err(format!(
            "Collection project already exists: {}",
            project_path.display()
        ));
    }
    if !is_allowed(&project_path) {
        return Err(format!(
            "Collect media requires explicit project file access: {}",
            project_path.display()
        ));
    }

    let media_directory = parent.join(format!("{stem}_media"));
    if media_directory.exists() {
        return Err(format!(
            "Collection media directory already exists: {}",
            media_directory.display()
        ));
    }
    if !is_allowed(&media_directory) {
        return Err(format!(
            "Collect media requires explicit access to the media directory: {}",
            media_directory.display()
        ));
    }

    let mut total_bytes = 0_u64;
    let mut source_paths = Vec::with_capacity(source.assets.len());
    for asset in &source.assets {
        let original = PathBuf::from(&asset.path);
        let canonical = fs::canonicalize(&original).map_err(|error| {
            format!(
                "Cannot collect offline or unreadable media '{}': {error}",
                original.display()
            )
        })?;
        if !canonical.is_file() {
            return Err(format!(
                "Media source is not a file: {}",
                canonical.display()
            ));
        }
        if !is_allowed(&canonical) {
            return Err(format!(
                "Collect media requires explicit source file access: {}",
                canonical.display()
            ));
        }
        let size = fs::metadata(&canonical)
            .map_err(|error| format!("Could not inspect '{}': {error}", canonical.display()))?
            .len();
        total_bytes = total_bytes
            .checked_add(size)
            .ok_or_else(|| "Total media size exceeds supported collection size".to_string())?;
        source_paths.push((asset.asset_id.clone(), asset.name.clone(), canonical, size));
    }

    let required_bytes = total_bytes
        .checked_add(1024 * 1024)
        .ok_or_else(|| "Total media size exceeds supported collection size".to_string())?;
    let available = available_space(&parent)?;
    if required_bytes > available {
        return Err(format!(
            "Not enough free space to collect media: need approximately {required_bytes} bytes including manifest overhead, have {available} bytes"
        ));
    }

    let mut variant = clone_project_as_variant(source, &format!("{} (Collected)", source.name))?;

    let staging = parent.join(format!(
        ".tinixcut-collect-{}",
        uuid::Uuid::new_v4().simple()
    ));
    fs::create_dir(&staging).map_err(|error| {
        format!(
            "Could not create collection staging directory '{}': {error}",
            staging.display()
        )
    })?;

    let result = (|| {
        let mut manifest_assets = Vec::with_capacity(source_paths.len());
        let mut copied_paths = Vec::with_capacity(source_paths.len());
        for (asset_id, name, source_path, expected_size) in &source_paths {
            let filename = source_path.file_name().ok_or_else(|| {
                format!("Media source has no filename: {}", source_path.display())
            })?;
            let safe_id = asset_id
                .chars()
                .map(|character| {
                    if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                        character
                    } else {
                        '_'
                    }
                })
                .collect::<String>();
            let relative_name = format!("{safe_id}_{}", filename.to_string_lossy());
            let staged_path = staging.join(&relative_name);
            if staged_path.exists() {
                return Err(format!("Collection filename collision: {relative_name}"));
            }
            fs::copy(source_path, &staged_path).map_err(|error| {
                format!(
                    "Could not copy '{}' into collection: {error}",
                    source_path.display()
                )
            })?;
            let copied_size = fs::metadata(&staged_path)
                .map_err(|error| {
                    format!(
                        "Could not verify copied file '{}': {error}",
                        staged_path.display()
                    )
                })?
                .len();
            if copied_size != *expected_size {
                return Err(format!(
                    "Copied media size mismatch for '{}': expected {expected_size}, got {copied_size}",
                    source_path.display()
                ));
            }
            let digest = sha256_file(&staged_path)?;
            manifest_assets.push(CollectionManifestAsset {
                asset_id: asset_id.clone(),
                original_name: name.clone(),
                relative_path: relative_name.clone(),
                size_bytes: copied_size,
                sha256: digest,
            });
            copied_paths.push((asset_id.clone(), media_directory.join(relative_name)));
        }

        let manifest = CollectionManifest {
            schema_version: 1,
            project_id: variant.project_id.clone(),
            source_project_id: source.project_id.clone(),
            assets: manifest_assets,
        };
        let manifest_bytes = serde_json::to_vec_pretty(&manifest)
            .map_err(|error| format!("Could not serialize collection manifest: {error}"))?;
        let mut manifest_file = File::create(staging.join("manifest.json"))
            .map_err(|error| format!("Could not create collection manifest: {error}"))?;
        manifest_file
            .write_all(&manifest_bytes)
            .map_err(|error| format!("Could not write collection manifest: {error}"))?;
        manifest_file
            .sync_all()
            .map_err(|error| format!("Could not flush collection manifest: {error}"))?;
        drop(manifest_file);

        fs::rename(&staging, &media_directory).map_err(|error| {
            format!(
                "Could not publish collected media directory '{}': {error}",
                media_directory.display()
            )
        })?;

        let operations = copied_paths
            .iter()
            .enumerate()
            .map(|(index, (asset_id, path))| OperationEnvelope {
                operation_id: format!("op_collect_{index}_{}", uuid::Uuid::new_v4().simple()),
                operation: CommandOperation::AssetRelink {
                    asset_id: asset_id.clone(),
                    path: path.to_string_lossy().to_string(),
                },
            })
            .collect::<Vec<_>>();
        if !operations.is_empty() {
            let project_id = variant.project_id.clone();
            let mut service = CommandService::new(variant);
            service
                .execute(CommandEnvelope {
                    schema_version: CURRENT_SCHEMA_VERSION,
                    project_id,
                    base_revision: 0,
                    client_id: None,
                    client_epoch: None,
                    sequence_number: None,
                    dry_run: false,
                    transaction_id: Some(format!("tx_collect_{}", uuid::Uuid::new_v4().simple())),
                    operations,
                })
                .map_err(|error| format!("Collected project media transaction failed: {error}"))?;
            variant = service.project().clone();
        }

        save_project_atomic_if_revision(&variant, &project_path, None)
            .map_err(|error| format!("Could not save collected project: {error}"))?;
        Ok(CollectedMediaReceipt {
            project_path: project_path.to_string_lossy().to_string(),
            media_directory: media_directory.to_string_lossy().to_string(),
            project: variant,
            copied_assets: source_paths.len(),
            total_bytes,
        })
    })();

    match result {
        Ok(receipt) => Ok(receipt),
        Err(error) => {
            let mut cleanup_errors = Vec::new();
            if staging.exists() {
                if let Err(cleanup_error) = fs::remove_dir_all(&staging) {
                    cleanup_errors.push(format!("staging cleanup failed: {cleanup_error}"));
                }
            }
            if media_directory.exists() && !project_path.exists() {
                if let Err(cleanup_error) = fs::remove_dir_all(&media_directory) {
                    cleanup_errors.push(format!("published media cleanup failed: {cleanup_error}"));
                }
            }
            if cleanup_errors.is_empty() {
                Err(error)
            } else {
                Err(format!("{error}; {}", cleanup_errors.join("; ")))
            }
        }
    }
}

fn safe_project_stem(project_name: &str) -> String {
    let mut stem = project_name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    while stem.starts_with('_') {
        stem.remove(0);
    }
    while stem.ends_with('_') {
        stem.pop();
    }
    if stem.is_empty() {
        stem = "project".to_string();
    }
    format!("{stem}_collected")
}

fn absolute_destination(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|current| current.join(path))
            .map_err(|error| format!("Could not resolve collection destination: {error}"))
    }
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path)
        .map_err(|error| format!("Could not read copied media '{}': {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|error| {
            format!("Could not hash copied media '{}': {error}", path.display())
        })?;
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

#[cfg(windows)]
pub(crate) fn available_space(path: &Path) -> Result<u64, String> {
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    #[link(name = "Kernel32")]
    extern "system" {
        fn GetDiskFreeSpaceExW(
            directory_name: *const u16,
            free_bytes_available: *mut u64,
            total_number_of_bytes: *mut u64,
            total_number_of_free_bytes: *mut u64,
        ) -> i32;
    }

    let mut wide = path.as_os_str().encode_wide().collect::<Vec<_>>();
    wide.push(0);
    let mut available = 0_u64;
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut available,
            ptr::null_mut(),
            ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(format!(
            "Could not preflight free disk space at '{}': {}",
            path.display(),
            std::io::Error::last_os_error()
        ));
    }
    Ok(available)
}

#[cfg(not(windows))]
pub(crate) fn available_space(_path: &Path) -> Result<u64, String> {
    Ok(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::domain::Project;
    use crate::modules::media::resolver::MediaBinaryResolver;

    fn make_video(path: &Path) {
        let result = MediaBinaryResolver::command("ffmpeg")
            .unwrap()
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc=duration=1:size=64x64:rate=30",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(path)
            .status()
            .unwrap();
        assert!(result.success());
    }

    #[test]
    fn collection_copies_sources_and_saves_independent_project_and_manifest() {
        let root = std::env::temp_dir().join(format!(
            "tinixcut_collect_{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&root).unwrap();
        let source_media = root.join("source.mp4");
        make_video(&source_media);
        let source_hash = sha256_file(&source_media).unwrap();
        let mut project = Project::new("Collect fixture".into());
        project.assets.push(crate::modules::domain::Asset {
            asset_id: "asset_video_1".into(),
            name: "source.mp4".into(),
            path: source_media.to_string_lossy().into_owned(),
            media_kind: crate::modules::domain::MediaKind::Video,
            duration: None,
            file_size: fs::metadata(&source_media).unwrap().len(),
            dimensions: None,
        });
        let receipt = collect_project_media(&project, &root, |_| true).unwrap();
        let destination = PathBuf::from(&receipt.project_path);

        assert_ne!(receipt.project.project_id, project.project_id);
        assert_eq!(receipt.project.revision, 1);
        assert_eq!(receipt.copied_assets, 1);
        assert_ne!(receipt.project.assets[0].path, project.assets[0].path);
        assert_eq!(sha256_file(&source_media).unwrap(), source_hash);
        let collected_path = PathBuf::from(&receipt.project.assets[0].path);
        assert_eq!(sha256_file(&collected_path).unwrap(), source_hash);
        assert!(destination.is_file());
        assert!(Path::new(&receipt.media_directory)
            .join("manifest.json")
            .is_file());
        let unavailable_original = source_media.with_extension("offline");
        fs::rename(&source_media, &unavailable_original).unwrap();
        let reopened = crate::modules::persistence::load_project(&destination).unwrap();
        assert_eq!(reopened.project_id, receipt.project.project_id);
        assert_eq!(reopened.assets[0].path, receipt.project.assets[0].path);
        assert!(Path::new(&reopened.assets[0].path).is_file());
        fs::rename(&unavailable_original, &source_media).unwrap();
        assert_eq!(sha256_file(&source_media).unwrap(), source_hash);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn collection_collision_and_offline_source_leave_no_partial_output() {
        let root = std::env::temp_dir().join(format!(
            "tinixcut_collect_collision_{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&root).unwrap();
        let destination = root.join("Collect_fixture_collected.tinix");
        let occupied_media = root.join("Collect_fixture_collected_media");
        fs::create_dir_all(&occupied_media).unwrap();
        let project = Project::new("Collect fixture".into());
        assert!(collect_project_media(&project, &root, |_| true).is_err());
        assert!(!destination.exists());
        fs::remove_dir(&occupied_media).unwrap();

        let mut project = Project::new("Offline fixture".into());
        project.assets.push(crate::modules::domain::Asset {
            asset_id: "asset_offline".into(),
            name: "missing.mp4".into(),
            path: root.join("missing.mp4").to_string_lossy().into_owned(),
            media_kind: crate::modules::domain::MediaKind::Video,
            duration: None,
            file_size: 0,
            dimensions: None,
        });
        assert!(collect_project_media(&project, &root, |_| true).is_err());
        assert!(!destination.exists());
        assert!(!root.join("Offline_fixture_collected_media").exists());
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn collection_command_validation_failure_rolls_back_copied_media_and_project() {
        let root = std::env::temp_dir().join(format!(
            "tinixcut_collect_rollback_{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&root).unwrap();
        let source_media = root.join("source.mp4");
        make_video(&source_media);
        let source_hash = sha256_file(&source_media).unwrap();
        let mut project = Project::new("Invalid collection fixture".into());
        project.assets.push(crate::modules::domain::Asset {
            asset_id: "asset_mismatched_kind".into(),
            name: "source.mp4".into(),
            path: source_media.to_string_lossy().into_owned(),
            media_kind: crate::modules::domain::MediaKind::Audio,
            duration: None,
            file_size: fs::metadata(&source_media).unwrap().len(),
            dimensions: None,
        });
        let destination = root.join("Invalid_collection_fixture_collected.tinix");

        let result = collect_project_media(&project, &root, |_| true);

        assert!(result.is_err());
        assert!(!destination.exists());
        assert!(!root
            .join("Invalid_collection_fixture_collected_media")
            .exists());
        assert_eq!(sha256_file(&source_media).unwrap(), source_hash);
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }
}
