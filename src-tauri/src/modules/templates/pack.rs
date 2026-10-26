//! Portable `.tinixtpl` Template Pack Format & Service (Phase 32)
//!
//! Provides secure export and import of `.tinixtpl` packages.
//! Hardened against:
//! - Zip bombs (uncompressed size & entry count limits)
//! - Path traversal attacks (relative paths, "..", absolute roots, colons, symlinks)
//! - Corrupted resource hashes (SHA-256 verification)
//! - Version conflicts & partial installs (staged atomic transactions)
//! - Machine cache path leakage (portable, relative paths only)

use crate::modules::domain::id::is_valid_id;
use crate::modules::domain::template::TemplateDefinition;
use crate::modules::templates::catalog::{TemplateCatalog, TemplateCatalogError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use thiserror::Error;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

/// Maximum pack archive file size: 50 MB
pub const MAX_PACK_FILE_SIZE: u64 = 50 * 1024 * 1024;

/// Maximum aggregate uncompressed size: 150 MB (Zip-bomb defense)
pub const MAX_PACK_UNCOMPRESSED_SIZE: u64 = 150 * 1024 * 1024;

/// Maximum entries allowed in a single pack archive
pub const MAX_PACK_ENTRY_COUNT: usize = 200;

#[derive(Debug, Error)]
pub enum TemplatePackError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Zip archive error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Template pack validation error: {0}")]
    Validation(String),

    #[error("Security violation: {0}")]
    Security(String),

    #[error("Catalog error: {0}")]
    Catalog(#[from] TemplateCatalogError),

    #[error("Corrupt resource '{resource_id}': expected SHA-256 {expected}, found {found}")]
    HashMismatch {
        resource_id: String,
        expected: String,
        found: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplatePackManifest {
    pub pack_id: String,
    pub pack_version: u32,
    pub name: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub template_ids: Vec<String>,
    pub created_at: String,
}

impl TemplatePackManifest {
    pub fn validate(&self) -> Result<(), TemplatePackError> {
        if !is_valid_id(&self.pack_id) {
            return Err(TemplatePackError::Validation(format!(
                "Invalid pack_id '{}'",
                self.pack_id
            )));
        }
        if self.pack_version == 0 {
            return Err(TemplatePackError::Validation(
                "Pack version must be positive".to_string(),
            ));
        }
        if self.name.trim().is_empty() {
            return Err(TemplatePackError::Validation(
                "Pack name cannot be empty".to_string(),
            ));
        }
        if self.template_ids.is_empty() {
            return Err(TemplatePackError::Validation(
                "Pack must declare at least one template".to_string(),
            ));
        }
        Ok(())
    }
}

pub struct TemplatePackService;

impl TemplatePackService {
    /// Exports a set of template definitions and their associated resources into a `.tinixtpl` archive.
    pub fn export_pack(
        manifest: &TemplatePackManifest,
        definitions: &[TemplateDefinition],
        resource_bytes: &HashMap<String, Vec<u8>>, // resource_id -> data
        output_path: &Path,
    ) -> Result<(), TemplatePackError> {
        manifest.validate()?;
        if definitions.is_empty() {
            return Err(TemplatePackError::Validation(
                "Cannot export empty pack".to_string(),
            ));
        }

        for def in definitions {
            def.validate()
                .map_err(|e| TemplatePackError::Validation(e.to_string()))?;
        }

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let tmp_output = output_path.with_extension("tinixtpl.tmp");
        let file = File::create(&tmp_output)?;
        let mut zip = ZipWriter::new(file);

        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        // 1. Write manifest.json
        let manifest_bytes = serde_json::to_vec_pretty(manifest)?;
        zip.start_file("manifest.json", options)?;
        zip.write_all(&manifest_bytes)?;

        // 2. Write definitions/<id>_v<version>.json
        for def in definitions {
            let def_bytes = serde_json::to_vec_pretty(def)?;
            let entry_name = format!("definitions/{}_v{}.json", def.id, def.version);
            zip.start_file(entry_name, options)?;
            zip.write_all(&def_bytes)?;
        }

        // 3. Write resources/<file_name>
        for def in definitions {
            for res in &def.resources.resources {
                let data = resource_bytes
                    .get(&res.file_name)
                    .or_else(|| resource_bytes.get(&res.resource_id));
                if let Some(bytes) = data {
                    let entry_name = format!("resources/{}", res.file_name);
                    zip.start_file(entry_name, options)?;
                    zip.write_all(bytes)?;
                } else if res.required {
                    return Err(TemplatePackError::Validation(format!(
                        "Declared required resource '{}' not provided for export",
                        res.file_name
                    )));
                }
            }
        }

        zip.finish()?;

        // Atomic commit
        if output_path.exists() {
            fs::remove_file(output_path)?;
        }
        fs::rename(&tmp_output, output_path)?;

        Ok(())
    }

    /// Securely imports a `.tinixtpl` archive into the target catalog.
    /// Performs staged extraction and full validation before committing atomically.
    pub fn import_pack(
        pack_path: &Path,
        catalog: &TemplateCatalog,
    ) -> Result<TemplatePackManifest, TemplatePackError> {
        catalog.ensure_dirs()?;

        let meta = fs::metadata(pack_path)?;
        if meta.len() > MAX_PACK_FILE_SIZE {
            return Err(TemplatePackError::Security(format!(
                "Pack file size ({} bytes) exceeds limit ({} bytes)",
                meta.len(),
                MAX_PACK_FILE_SIZE
            )));
        }

        let file = File::open(pack_path)?;
        let mut archive = ZipArchive::new(file)?;

        if archive.len() > MAX_PACK_ENTRY_COUNT {
            return Err(TemplatePackError::Security(format!(
                "Pack entry count ({}) exceeds limit ({})",
                archive.len(),
                MAX_PACK_ENTRY_COUNT
            )));
        }

        // Staged temporary directory for atomic import
        let staging_id = uuid::Uuid::new_v4().to_string();
        let staging_dir = catalog.library_dir.join(format!("staging_{staging_id}"));
        fs::create_dir_all(&staging_dir)?;

        let import_result = (|| {
            let mut total_uncompressed_bytes: u64 = 0;
            let mut manifest: Option<TemplatePackManifest> = None;
            let mut definitions: Vec<TemplateDefinition> = Vec::new();
            let mut seen_entries = std::collections::HashSet::new();

            for i in 0..archive.len() {
                let mut zip_file = archive.by_index(i)?;
                let raw_name = zip_file.name().to_string();

                // Symlink defense
                let is_symlink = zip_file
                    .unix_mode()
                    .map(|m| m & 0o170000 == 0o120000)
                    .unwrap_or(false);
                if is_symlink {
                    return Err(TemplatePackError::Security(format!(
                        "Zip symlink entries are prohibited: '{raw_name}'"
                    )));
                }

                // Path traversal defenses:
                // - Disallow Windows backslashes in archive path names
                // - Disallow path segments containing ".."
                // - Disallow absolute paths or Windows drive prefixes
                // - Disallow null bytes
                if raw_name.contains('\\')
                    || raw_name.contains("..")
                    || raw_name.starts_with('/')
                    || raw_name.contains(':')
                    || raw_name.contains('\0')
                {
                    return Err(TemplatePackError::Security(format!(
                        "Potentially malicious entry path: '{raw_name}'"
                    )));
                }

                // Duplicate normalized entry path defense (collapse repeated slashes and normalize case)
                let norm_path = raw_name
                    .replace('\\', "/")
                    .split('/')
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join("/")
                    .to_lowercase();
                if !seen_entries.insert(norm_path) {
                    return Err(TemplatePackError::Security(format!(
                        "Duplicate entry path in pack: '{raw_name}'"
                    )));
                }

                if zip_file.is_dir() {
                    continue;
                }

                let remaining_budget =
                    MAX_PACK_UNCOMPRESSED_SIZE.saturating_sub(total_uncompressed_bytes);
                let mut take_reader = (&mut zip_file).take(remaining_budget + 1);
                let mut content = Vec::new();
                let bytes_read = take_reader.read_to_end(&mut content)?;

                if bytes_read as u64 > remaining_budget {
                    return Err(TemplatePackError::Security(format!(
                        "Aggregate uncompressed pack size exceeds limit ({} bytes)",
                        MAX_PACK_UNCOMPRESSED_SIZE
                    )));
                }
                total_uncompressed_bytes += bytes_read as u64;

                let target_path = staging_dir.join(&raw_name);
                if let Some(parent) = target_path.parent() {
                    fs::create_dir_all(parent)?;
                }

                let mut out_file = File::create(&target_path)?;
                out_file.write_all(&content)?;

                if raw_name == "manifest.json" {
                    let m: TemplatePackManifest = serde_json::from_slice(&content)?;
                    m.validate()?;
                    manifest = Some(m);
                } else if raw_name.starts_with("definitions/") && raw_name.ends_with(".json") {
                    let def: TemplateDefinition = serde_json::from_slice(&content)?;
                    def.validate()
                        .map_err(|e| TemplatePackError::Validation(e.to_string()))?;
                    definitions.push(def);
                }
            }

            let manifest = manifest.ok_or_else(|| {
                TemplatePackError::Validation("Missing manifest.json in pack".to_string())
            })?;

            if definitions.is_empty() {
                return Err(TemplatePackError::Validation(
                    "No valid definitions found in pack".to_string(),
                ));
            }

            // Verify manifest template_ids match definitions & check duplicate definitions
            let mut seen_def_ids = std::collections::HashSet::new();
            for def in &definitions {
                if !seen_def_ids.insert((def.id.clone(), def.version)) {
                    return Err(TemplatePackError::Validation(format!(
                        "Duplicate template definition '{}' v{} in pack",
                        def.id, def.version
                    )));
                }
                if !manifest.template_ids.contains(&def.id) {
                    return Err(TemplatePackError::Validation(format!(
                        "Pack contains definition '{}' not declared in manifest template_ids",
                        def.id
                    )));
                }
            }
            for declared_id in &manifest.template_ids {
                if !definitions.iter().any(|d| &d.id == declared_id) {
                    return Err(TemplatePackError::Validation(format!(
                        "Manifest declares template '{}' but definition is missing from pack",
                        declared_id
                    )));
                }
            }

            // Gather all declared resource filenames
            let mut declared_resource_filenames = std::collections::HashSet::new();
            for def in &definitions {
                for res in &def.resources.resources {
                    if !crate::modules::domain::template::is_safe_resource_filename(&res.file_name)
                    {
                        return Err(TemplatePackError::Security(format!(
                            "Resource file name '{}' is not safe",
                            res.file_name
                        )));
                    }
                    declared_resource_filenames.insert(res.file_name.clone());
                }
            }

            // Reject undeclared files under resources/
            let staged_resources = staging_dir.join("resources");
            if staged_resources.exists() {
                for entry in fs::read_dir(&staged_resources)?.flatten() {
                    let file_name = entry.file_name().to_string_lossy().to_string();
                    if !declared_resource_filenames.contains(&file_name) {
                        return Err(TemplatePackError::Security(format!(
                            "Undeclared resource file found in pack: '{}'",
                            file_name
                        )));
                    }
                }
            }

            // Verify resource integrity for all declared resources in definitions
            for def in &definitions {
                for res in &def.resources.resources {
                    let staged_res_path = staging_dir.join("resources").join(&res.file_name);
                    if staged_res_path.exists() {
                        let res_bytes = fs::read(&staged_res_path)?;
                        let hash = crate::modules::media::text_renderer_poc::compute_sha256_hex(
                            &res_bytes,
                        );
                        if !hash.eq_ignore_ascii_case(&res.sha256) {
                            return Err(TemplatePackError::HashMismatch {
                                resource_id: res.resource_id.clone(),
                                expected: res.sha256.clone(),
                                found: hash,
                            });
                        }
                    } else if res.required {
                        return Err(TemplatePackError::Validation(format!(
                            "Required resource '{}' missing from pack",
                            res.file_name
                        )));
                    }
                }
            }

            // 1. Preflight phase: Check all conflicts BEFORE modifying catalog state
            // Check version immutability conflicts against target catalog
            for def in &definitions {
                if let Some(existing) = catalog.get(&def.id, def.version) {
                    if &existing != def {
                        return Err(TemplatePackError::Validation(format!(
                            "Immutable conflict: template '{}' v{} already exists with different definition",
                            def.id, def.version
                        )));
                    }
                }
            }

            // Preflight resource conflicts against target catalog
            if staged_resources.exists() {
                for entry in fs::read_dir(&staged_resources)?.flatten() {
                    let src = entry.path();
                    if src.is_file() {
                        let file_name = entry.file_name();
                        let dst = catalog.resources_dir().join(&file_name);
                        if dst.exists() {
                            let src_bytes = fs::read(&src)?;
                            let src_hash =
                                crate::modules::media::text_renderer_poc::compute_sha256_hex(
                                    &src_bytes,
                                );
                            let existing_bytes = fs::read(&dst)?;
                            let existing_hash =
                                crate::modules::media::text_renderer_poc::compute_sha256_hex(
                                    &existing_bytes,
                                );
                            if !existing_hash.eq_ignore_ascii_case(&src_hash) {
                                return Err(TemplatePackError::Validation(format!(
                                    "Conflicting resource '{:?}' already exists in catalog with different content",
                                    file_name
                                )));
                            }
                        }
                    }
                }
            }

            // 2. Transactional commit phase: temp files + controlled promotion with rollback tracking
            let mut all_tracked_paths: Vec<std::path::PathBuf> = Vec::new();
            let mut pending_promotions: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();

            let commit_res = (|| -> Result<(), TemplatePackError> {
                // Stage resources to unique temp files
                if staged_resources.exists() {
                    for entry in fs::read_dir(&staged_resources)?.flatten() {
                        let src = entry.path();
                        if src.is_file() {
                            let file_name = entry.file_name();
                            let dst = catalog.resources_dir().join(&file_name);
                            if !dst.exists() {
                                let tmp_name = format!(
                                    "{}.tmp.{}",
                                    file_name.to_string_lossy(),
                                    uuid::Uuid::new_v4().simple()
                                );
                                let tmp = catalog.resources_dir().join(tmp_name);
                                all_tracked_paths.push(tmp.clone());
                                all_tracked_paths.push(dst.clone());
                                fs::copy(&src, &tmp)?;
                                pending_promotions.push((tmp, dst));
                            }
                        }
                    }
                }

                // Stage definitions to unique temp files
                for def in &definitions {
                    let def_path = catalog.definition_file_path(&def.id, def.version)?;
                    if !def_path.exists() {
                        let tmp_name = format!(
                            "{}_{}.tmp.{}",
                            def.id,
                            def.version,
                            uuid::Uuid::new_v4().simple()
                        );
                        let tmp = catalog.definitions_dir().join(tmp_name);
                        all_tracked_paths.push(tmp.clone());
                        all_tracked_paths.push(def_path.clone());

                        let json_bytes = serde_json::to_vec_pretty(def)?;
                        let mut file = File::create(&tmp)?;
                        file.write_all(&json_bytes)?;
                        file.flush()?;
                        file.sync_all()?;

                        pending_promotions.push((tmp, def_path));
                    }
                }

                // Controlled promotion: promote all temp files to their final destinations
                for (tmp, target) in &pending_promotions {
                    fs::rename(tmp, target)?;
                }

                Ok(())
            })();

            if let Err(err) = commit_res {
                // Rollback: delete all newly created temp files and promoted targets
                let mut cleanup_errors = Vec::new();
                for tracked in &all_tracked_paths {
                    if tracked.exists() {
                        if let Err(e) = fs::remove_file(tracked) {
                            cleanup_errors.push(format!(
                                "Failed to clean up '{}': {}",
                                tracked.display(),
                                e
                            ));
                        }
                    }
                }
                if !cleanup_errors.is_empty() {
                    return Err(TemplatePackError::Validation(format!(
                        "Pack import failed: {}. Rollback cleanup errors: {}",
                        err,
                        cleanup_errors.join("; ")
                    )));
                }
                return Err(err);
            }

            Ok(manifest)
        })();

        // Clean up staging directory unconditionally
        let _ = fs::remove_dir_all(&staging_dir);

        import_result
    }
}
