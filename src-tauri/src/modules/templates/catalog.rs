//! File-based atomic catalog for bundled and user template definitions.
//! Architecture Invariant: Version immutability; atomic file replacement;
//! deleting a catalog item never invalidates or deletes project-used assets.

use crate::modules::domain::id::is_valid_id;
use crate::modules::domain::template::TemplateDefinition;
use crate::modules::templates::curated::{curated_definitions, resolve_curated_definition};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TemplateCatalogError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Template validation failed: {0}")]
    Validation(String),

    #[error("Cannot overwrite immutable definition '{id}' version {version}. Bump version to publish updates.")]
    ImmutableVersionConflict { id: String, version: u32 },

    #[error("Definition '{id}' version {version} not found in user library")]
    NotFound { id: String, version: u32 },

    #[error("Security violation: {0}")]
    Security(String),
}

#[derive(Debug, Clone)]
pub struct TemplateCatalog {
    pub library_dir: PathBuf,
}

impl TemplateCatalog {
    pub fn new(library_dir: PathBuf) -> Self {
        Self { library_dir }
    }

    pub fn default_catalog() -> Self {
        if let Some(override_dir) = std::env::var_os("TINIX_TEMPLATE_CATALOG_DIR") {
            return Self::new(PathBuf::from(override_dir));
        }
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("TinixCut")
            .join("templates");
        Self::new(base)
    }

    pub fn definitions_dir(&self) -> PathBuf {
        self.library_dir.join("definitions")
    }

    pub fn resources_dir(&self) -> PathBuf {
        self.library_dir.join("resources")
    }

    pub fn ensure_dirs(&self) -> Result<(), TemplateCatalogError> {
        fs::create_dir_all(self.definitions_dir())?;
        fs::create_dir_all(self.resources_dir())?;
        Ok(())
    }

    pub fn definition_file_path(
        &self,
        id: &str,
        version: u32,
    ) -> Result<PathBuf, TemplateCatalogError> {
        if !is_valid_id(id) {
            return Err(TemplateCatalogError::Security(format!(
                "Invalid template ID '{}' for file path",
                id
            )));
        }
        Ok(self.definitions_dir().join(format!("{id}_v{version}.json")))
    }

    /// Lists all definitions: bundled native definitions plus user definitions from disk.
    pub fn list_all(&self) -> Vec<TemplateDefinition> {
        let mut list = curated_definitions();

        if let Ok(entries) = fs::read_dir(self.definitions_dir()) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(def) = serde_json::from_str::<TemplateDefinition>(&content) {
                            if def.validate().is_ok() {
                                // If a user definition has the same id and version as a bundled one,
                                // the user definition does not shadow bundled ones if invalid.
                                if !list
                                    .iter()
                                    .any(|b| b.id == def.id && b.version == def.version)
                                {
                                    list.push(def);
                                }
                            }
                        }
                    }
                }
            }
        }

        list
    }

    /// Resolves a template definition by exact id and version.
    pub fn get(&self, id: &str, version: u32) -> Option<TemplateDefinition> {
        if !is_valid_id(id) {
            return None;
        }
        if let Ok(user_path) = self.definition_file_path(id, version) {
            if user_path.exists() {
                if let Ok(content) = fs::read_to_string(&user_path) {
                    if let Ok(def) = serde_json::from_str::<TemplateDefinition>(&content) {
                        if def.validate().is_ok() {
                            return Some(def);
                        }
                    }
                }
            }
        }
        resolve_curated_definition(id, version)
    }

    /// Atomically saves a user definition.
    /// Invariant: Once saved, `<id>_v<version>` is immutable! If it exists with different
    /// content, an error is returned.
    pub fn save_user_definition(
        &self,
        definition: &TemplateDefinition,
    ) -> Result<(), TemplateCatalogError> {
        if !is_valid_id(&definition.id) {
            return Err(TemplateCatalogError::Security(format!(
                "Invalid template ID '{}'",
                definition.id
            )));
        }

        definition
            .validate()
            .map_err(|e| TemplateCatalogError::Validation(e.to_string()))?;

        if let Some(bundled) = resolve_curated_definition(&definition.id, definition.version) {
            if &bundled != definition {
                return Err(TemplateCatalogError::ImmutableVersionConflict {
                    id: definition.id.clone(),
                    version: definition.version,
                });
            }
        }

        self.ensure_dirs()?;
        let target_path = self.definition_file_path(&definition.id, definition.version)?;

        if target_path.exists() {
            let existing_content = fs::read_to_string(&target_path)?;
            if let Ok(existing_def) = serde_json::from_str::<TemplateDefinition>(&existing_content)
            {
                if &existing_def != definition {
                    return Err(TemplateCatalogError::ImmutableVersionConflict {
                        id: definition.id.clone(),
                        version: definition.version,
                    });
                } else {
                    // Exact same content already stored
                    return Ok(());
                }
            }
        }

        let json_bytes = serde_json::to_vec_pretty(definition)?;
        let tmp_path = target_path.with_extension(format!("tmp.{}", uuid::Uuid::new_v4().simple()));

        let write_res = (|| -> Result<(), std::io::Error> {
            let mut file = File::create(&tmp_path)?;
            file.write_all(&json_bytes)?;
            file.flush()?;
            file.sync_all()?;
            fs::rename(&tmp_path, &target_path)?;
            Ok(())
        })();

        if let Err(e) = write_res {
            let _ = fs::remove_file(&tmp_path);
            return Err(e.into());
        }

        Ok(())
    }

    /// Deletes a user definition file.
    /// Note: Does not delete resources (which may be referenced by projects).
    pub fn delete_user_definition(
        &self,
        id: &str,
        version: u32,
    ) -> Result<bool, TemplateCatalogError> {
        if !is_valid_id(id) {
            return Err(TemplateCatalogError::Security(format!(
                "Invalid template ID '{}'",
                id
            )));
        }
        let path = self.definition_file_path(id, version)?;
        if path.exists() {
            fs::remove_file(path)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
