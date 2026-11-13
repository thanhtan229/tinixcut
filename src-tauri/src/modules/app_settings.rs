use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const APP_SETTINGS_SCHEMA_VERSION: u32 = 1;
const MAX_RECENT_PROJECTS: usize = 20;
const MAX_SETTING_STRING_LENGTH: usize = 4096;
const MAX_SHORTCUT_ACTIONS: usize = 64;
const MAX_CHORDS_PER_ACTION: usize = 4;
const MAX_CHORD_LENGTH: usize = 32;

/// Whether the preview switches decodable video to seek-optimized proxies.
/// `Off` keeps proxies as a codec-compatibility fallback only, so enabling the
/// background encoding and its disk use is an explicit user choice.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PreviewProxyMode {
    #[default]
    Off,
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub schema_version: u32,
    pub whisper_cli_path: Option<String>,
    pub whisper_model_path: Option<String>,
    pub transcription_language: String,
    pub recent_projects: Vec<String>,
    pub legacy_browser_settings_migrated: bool,
    /// Added within schema v1; files written before it load as `Off`.
    #[serde(default)]
    pub preview_proxy_mode: PreviewProxyMode,
    /// Keyboard shortcut overrides: action id -> chords. Actions not listed keep the built-in
    /// chords; conflicts are rejected by the editor before they are saved.
    #[serde(default)]
    pub shortcut_overrides: BTreeMap<String, Vec<String>>,
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
            preview_proxy_mode: PreviewProxyMode::Off,
            shortcut_overrides: BTreeMap::new(),
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
        if self.shortcut_overrides.len() > MAX_SHORTCUT_ACTIONS {
            return Err("Too many shortcut overrides".to_string());
        }
        for (action, chords) in &self.shortcut_overrides {
            if action.is_empty()
                || action.len() > MAX_CHORD_LENGTH
                || !action
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                return Err(format!("Invalid shortcut action id '{action}'"));
            }
            if chords.is_empty()
                || chords.len() > MAX_CHORDS_PER_ACTION
                || chords
                    .iter()
                    .any(|chord| chord.is_empty() || chord.len() > MAX_CHORD_LENGTH)
            {
                return Err(format!(
                    "Shortcut '{action}' needs 1 to {MAX_CHORDS_PER_ACTION} chords of at most {MAX_CHORD_LENGTH} characters"
                ));
            }
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
            preview_proxy_mode: PreviewProxyMode::Auto,
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
    fn shortcut_overrides_default_to_empty_and_are_validated() {
        let legacy: AppSettings = serde_json::from_str(
            r#"{"schemaVersion":1,"whisperCliPath":null,"whisperModelPath":null,"transcriptionLanguage":"auto","recentProjects":[],"legacyBrowserSettingsMigrated":false}"#,
        )
        .unwrap();
        assert!(legacy.shortcut_overrides.is_empty());
        let mut settings = AppSettings::default();
        settings
            .shortcut_overrides
            .insert("play_pause".into(), vec!["Space".into()]);
        assert!(settings.validate().is_ok());
        settings
            .shortcut_overrides
            .insert("bad action".into(), vec!["X".into()]);
        assert!(settings.validate().is_err());
        settings.shortcut_overrides.remove("bad action");
        settings
            .shortcut_overrides
            .insert("zoom_in".into(), Vec::new());
        assert!(settings.validate().is_err());
        settings
            .shortcut_overrides
            .insert("zoom_in".into(), vec!["".into()]);
        assert!(settings.validate().is_err());
    }

    #[test]
    fn settings_written_before_preview_proxy_mode_load_as_off() {
        let legacy = r#"{"schemaVersion":1,"whisperCliPath":null,"whisperModelPath":null,"transcriptionLanguage":"auto","recentProjects":[],"legacyBrowserSettingsMigrated":true}"#;
        let settings: AppSettings = serde_json::from_str(legacy).unwrap();
        assert_eq!(settings.preview_proxy_mode, PreviewProxyMode::Off);
        assert!(settings.validate().is_ok());
        assert!(serde_json::to_string(&settings)
            .unwrap()
            .contains(r#""previewProxyMode":"off""#));
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
