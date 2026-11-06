use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const APP_SETTINGS_SCHEMA_VERSION: u32 = 1;
const MAX_RECENT_PROJECTS: usize = 20;
const MAX_SETTING_STRING_LENGTH: usize = 4096;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub schema_version: u32,
    pub whisper_cli_path: Option<String>,
    pub whisper_model_path: Option<String>,
    pub transcription_language: String,
    pub recent_projects: Vec<String>,
    pub legacy_browser_settings_migrated: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: APP_SETTINGS_SCHEMA_VERSION,
            whisper_cli_path: None,
            whisper_model_path: None,
            transcription_language: "auto".to_string(),
            recent_projects: Vec::new(),
            legacy_browser_settings_migrated: false,
        }
    }
}

impl AppSettings {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != APP_SETTINGS_SCHEMA_VERSION {
            return Err(format!(
                "Unsupported app settings schema version {} (supported: {})",
                self.schema_version, APP_SETTINGS_SCHEMA_VERSION
            ));
        }
        for (name, value) in [
            ("Whisper CLI path", self.whisper_cli_path.as_deref()),
            ("Whisper model path", self.whisper_model_path.as_deref()),
        ] {
            if value.is_some_and(|value| value.len() > MAX_SETTING_STRING_LENGTH) {
                return Err(format!("{name} exceeds the maximum supported length"));
            }
        }
        if !matches!(self.transcription_language.as_str(), "auto" | "vi" | "en") {
            return Err("Transcription language must be auto, vi, or en".to_string());
        }
        if self.recent_projects.len() > MAX_RECENT_PROJECTS
            || self
                .recent_projects
                .iter()
                .any(|path| path.is_empty() || path.len() > MAX_SETTING_STRING_LENGTH)
        {
            return Err(format!(
                "Recent projects must contain at most {MAX_RECENT_PROJECTS} valid paths"
            ));
        }
        Ok(())
    }
}

pub fn settings_path(config_dir: &Path) -> PathBuf {
    config_dir.join("settings.json")
}

pub fn load_settings(path: &Path) -> Result<AppSettings, String> {
    let backup = path.with_extension("json.bak");
    let source = if path.exists() {
        path
    } else if backup.exists() {
        &backup
    } else {
        return Ok(AppSettings::default());
    };
    let mut file = File::open(source).map_err(|error| {
        format!(
            "Could not open app settings at {}: {error}",
            source.display()
        )
    })?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).map_err(|error| {
        format!(
            "Could not read app settings at {}: {error}",
            source.display()
        )
    })?;
    let settings: AppSettings = serde_json::from_str(&contents).map_err(|error| {
        format!(
            "App settings at {} are invalid; the file was preserved. Repair or move it before saving new settings: {error}",
            source.display()
        )
    })?;
    settings.validate().map_err(|error| {
        format!(
            "App settings at {} are invalid; the file was preserved: {error}",
            source.display()
        )
    })?;
    Ok(settings)
}

pub fn save_settings(settings: &AppSettings, path: &Path) -> Result<(), String> {
    settings.validate()?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| "App settings path must have a parent directory".to_string())?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "Could not create app settings directory {}: {error}",
            parent.display()
        )
    })?;

    let bytes = serde_json::to_vec_pretty(settings)
        .map_err(|error| format!("Could not encode app settings: {error}"))?;
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4().simple()));
    let write_result = (|| -> Result<(), String> {
        let mut file = File::create(&temporary)
            .map_err(|error| format!("Could not create temporary app settings: {error}"))?;
        file.write_all(&bytes)
            .and_then(|_| file.flush())
            .and_then(|_| file.sync_all())
            .map_err(|error| format!("Could not write app settings: {error}"))?;

        if path.exists() {
            let backup = path.with_extension("json.bak");
            let _ = fs::remove_file(&backup);
            fs::rename(path, &backup)
                .map_err(|error| format!("Could not preserve previous app settings: {error}"))?;
            if let Err(error) = fs::rename(&temporary, path) {
                let _ = fs::rename(&backup, path);
                return Err(format!("Could not replace app settings: {error}"));
            }
            let _ = fs::remove_file(backup);
        } else {
            fs::rename(&temporary, path)
                .map_err(|error| format!("Could not install app settings: {error}"))?;
        }
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    write_result
}

pub fn record_recent_project(settings: &mut AppSettings, project_path: &str) -> Result<(), String> {
    let path = project_path.trim();
    if path.is_empty() || path.len() > MAX_SETTING_STRING_LENGTH {
        return Err("Project path is not valid for the recent-project list".to_string());
    }
    settings.recent_projects.retain(|recent| recent != path);
    settings.recent_projects.insert(0, path.to_string());
    settings.recent_projects.truncate(MAX_RECENT_PROJECTS);
    settings.validate()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_corrupt_file_is_preserved() {
        let root = std::env::temp_dir().join(format!(
            "tinixcut_app_settings_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let path = settings_path(&root);
        let settings = AppSettings {
            whisper_cli_path: Some("C:/tools/whisper-cli.exe".to_string()),
            whisper_model_path: Some("C:/models/ggml-base.bin".to_string()),
            transcription_language: "vi".to_string(),
            recent_projects: vec!["C:/projects/test.tinix".to_string()],
            ..AppSettings::default()
        };

        assert_eq!(load_settings(&path).unwrap(), AppSettings::default());
        save_settings(&settings, &path).unwrap();
        assert_eq!(load_settings(&path).unwrap(), settings);

        let backup = path.with_extension("json.bak");
        fs::rename(&path, &backup).unwrap();
        assert_eq!(load_settings(&path).unwrap(), settings);
        fs::rename(&backup, &path).unwrap();

        fs::write(&path, b"{broken").unwrap();
        let error = load_settings(&path).unwrap_err();
        assert!(error.contains("the file was preserved"));
        assert_eq!(fs::read(&path).unwrap(), b"{broken");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn settings_reject_unknown_language_and_oversized_recent_list() {
        let mut settings = AppSettings::default();
        settings.transcription_language = "fr".to_string();
        assert!(settings.validate().is_err());

        settings.transcription_language = "auto".to_string();
        settings.recent_projects = (0..=MAX_RECENT_PROJECTS)
            .map(|index| format!("project-{index}.tinix"))
            .collect();
        assert!(settings.validate().is_err());
    }

    #[test]
    fn recent_projects_move_to_front_and_remain_bounded() {
        let mut settings = AppSettings::default();
        record_recent_project(&mut settings, "first.tinix").unwrap();
        record_recent_project(&mut settings, "second.tinix").unwrap();
        record_recent_project(&mut settings, "first.tinix").unwrap();
        assert_eq!(settings.recent_projects[0], "first.tinix");
        assert_eq!(settings.recent_projects[1], "second.tinix");

        for index in 0..MAX_RECENT_PROJECTS + 3 {
            record_recent_project(&mut settings, &format!("project-{index}.tinix")).unwrap();
        }
        assert_eq!(settings.recent_projects.len(), MAX_RECENT_PROJECTS);
    }
}
