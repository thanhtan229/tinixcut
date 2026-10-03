pub mod modules;

use crate::modules::command::{
    CommandEnvelope, CommandOperation, CommandReceipt, CommandService, DiffReport,
    OperationEnvelope,
};
use crate::modules::domain::{Asset, MediaKind, Project, CURRENT_SCHEMA_VERSION};
use crate::modules::export::{ExportEngine, ExportProfile, ExportReport};
use crate::modules::history::{HistoryEntry, HistoryManager};
use crate::modules::media::ffmpeg::FFmpegService;
use crate::modules::persistence::{
    check_recovery_status, load_project, save_project_atomic, RecoveryStatus,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthStatus {
    pub status: String,
    pub version: String,
    pub platform: String,
    pub schema_version: u32,
}

pub fn get_health_status() -> HealthStatus {
    HealthStatus {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        platform: std::env::consts::OS.to_string(),
        schema_version: 1,
    }
}

pub struct EditorEngine {
    pub service: CommandService,
    pub history: HistoryManager,
    pub current_file_path: Option<PathBuf>,
}

impl EditorEngine {
    pub fn new(project: Project) -> Self {
        Self {
            service: CommandService::new(project),
            history: HistoryManager::new(100),
            current_file_path: None,
        }
    }
}

pub struct AppState(pub Mutex<EditorEngine>);

#[tauri::command]
fn app_health_check() -> HealthStatus {
    get_health_status()
}

#[tauri::command]
fn project_new(state: State<'_, AppState>, name: String) -> Result<Project, String> {
    let mut engine = state.0.lock().map_err(|e| e.to_string())?;
    let proj = Project::new(name);
    engine.service = CommandService::new(proj.clone());
    engine.history = HistoryManager::new(100);
    engine.current_file_path = None;
    Ok(proj)
}

#[tauri::command]
fn project_get_current(state: State<'_, AppState>) -> Result<Project, String> {
    let engine = state.0.lock().map_err(|e| e.to_string())?;
    Ok(engine.service.project().clone())
}

#[tauri::command]
fn project_execute_command(
    state: State<'_, AppState>,
    envelope: CommandEnvelope,
) -> Result<CommandReceipt, String> {
    let mut engine = state.0.lock().map_err(|e| e.to_string())?;
    let prior_snapshot = engine.service.project().clone();
    let is_dry_run = envelope.dry_run;

    let receipt = engine
        .service
        .execute(envelope)
        .map_err(|e| e.to_string())?;

    if !is_dry_run && receipt.operations_applied > 0 {
        let resulting_snapshot = engine.service.project().clone();
        engine.history.record_edit(HistoryEntry {
            description: format!(
                "Transaction {}",
                receipt.transaction_id.as_deref().unwrap_or("edit")
            ),
            transaction_id: receipt.transaction_id.clone(),
            prior_snapshot,
            resulting_snapshot,
        });
    }

    Ok(receipt)
}

#[tauri::command]
fn project_dry_run(
    state: State<'_, AppState>,
    envelope: CommandEnvelope,
) -> Result<DiffReport, String> {
    let engine = state.0.lock().map_err(|e| e.to_string())?;
    engine.service.dry_run(&envelope).map_err(|e| e.to_string())
}

#[tauri::command]
fn project_save(state: State<'_, AppState>, path: Option<String>) -> Result<String, String> {
    let mut engine = state.0.lock().map_err(|e| e.to_string())?;
    let target = if let Some(p) = path {
        let pbuf = PathBuf::from(p);
        engine.current_file_path = Some(pbuf.clone());
        pbuf
    } else if let Some(ref p) = engine.current_file_path {
        p.clone()
    } else {
        return Err("No path specified for project save".to_string());
    };

    save_project_atomic(engine.service.project(), &target).map_err(|e| e.to_string())?;
    Ok(target.to_string_lossy().to_string())
}

#[tauri::command]
fn project_open(state: State<'_, AppState>, path: String) -> Result<Project, String> {
    let mut engine = state.0.lock().map_err(|e| e.to_string())?;
    let pbuf = PathBuf::from(&path);
    let proj = load_project(&pbuf).map_err(|e| e.to_string())?;
    engine.service = CommandService::new(proj.clone());
    engine.history = HistoryManager::new(100);
    engine.current_file_path = Some(pbuf);
    Ok(proj)
}

#[tauri::command]
fn project_check_recovery(path: String) -> Result<RecoveryStatus, String> {
    let canonical = PathBuf::from(&path);
    let recovery = canonical.with_extension("tcproj.recovery");
    check_recovery_status(&canonical, &recovery).map_err(|e| e.to_string())
}

#[tauri::command]
fn project_undo(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let mut engine = state.0.lock().map_err(|e| e.to_string())?;
    let mut current = engine.service.project().clone();
    let desc = engine.history.undo(&mut current);
    if desc.is_some() {
        engine.service.set_project(current);
    }
    Ok(desc)
}

#[tauri::command]
fn project_redo(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let mut engine = state.0.lock().map_err(|e| e.to_string())?;
    let mut current = engine.service.project().clone();
    let desc = engine.history.redo(&mut current);
    if desc.is_some() {
        engine.service.set_project(current);
    }
    Ok(desc)
}

#[tauri::command]
fn project_export(
    state: State<'_, AppState>,
    output_path: String,
    profile: Option<ExportProfile>,
) -> Result<ExportReport, String> {
    let engine = state.0.lock().map_err(|e| e.to_string())?;
    let snapshot = engine.service.project();
    let target = PathBuf::from(&output_path);
    let prof = profile.unwrap_or_default();
    ExportEngine::render_snapshot(snapshot, &target, &prof, None).map_err(|e| e.to_string())
}

#[tauri::command]
fn media_import(state: State<'_, AppState>, file_path: String) -> Result<Asset, String> {
    let path = Path::new(&file_path);
    if !path.exists() {
        return Err(format!("Media file not found at path: {}", file_path));
    }

    let file_name = path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "Imported Media".to_string());

    let file_size = std::fs::metadata(path)
        .map(|m| m.len())
        .unwrap_or(0);

    // Determine media kind
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    let media_kind = match ext.as_str() {
        "mp3" | "wav" | "aac" | "flac" | "ogg" | "m4a" => MediaKind::Audio,
        "png" | "jpg" | "jpeg" | "webp" | "bmp" => MediaKind::Image,
        _ => {
            // Default to probing via ffprobe
            match FFmpegService::probe_file(path) {
                Ok(meta) => {
                    if meta.video_codec.is_some() {
                        MediaKind::Video
                    } else if meta.audio_codec.is_some() {
                        MediaKind::Audio
                    } else {
                        MediaKind::Video
                    }
                }
                Err(_) => MediaKind::Video,
            }
        }
    };

    let mut engine = state.0.lock().map_err(|e| e.to_string())?;
    let asset_id = format!("ast_{}", uuid::Uuid::new_v4().simple());
    let op = CommandOperation::AssetImport {
        asset_id: Some(asset_id.clone()),
        name: file_name,
        path: file_path.clone(),
        media_kind,
        file_size,
    };

    let envelope = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: engine.service.project().project_id.clone(),
        base_revision: engine.service.project().revision,
        dry_run: false,
        transaction_id: Some(format!("tx_import_{}", uuid::Uuid::new_v4().simple())),
        operations: vec![OperationEnvelope {
            operation_id: format!("op_import_{}", uuid::Uuid::new_v4().simple()),
            operation: op,
        }],
    };

    let prior_snapshot = engine.service.project().clone();
    let _receipt = engine.service.execute(envelope).map_err(|e| e.to_string())?;
    let resulting_snapshot = engine.service.project().clone();

    engine.history.record_edit(HistoryEntry {
        description: format!("Import media {}", file_path),
        transaction_id: Some(format!("tx_import_{}", asset_id)),
        prior_snapshot,
        resulting_snapshot: resulting_snapshot.clone(),
    });

    let asset = resulting_snapshot
        .assets
        .into_iter()
        .find(|a| a.asset_id == asset_id)
        .ok_or_else(|| "Failed to find imported asset in project".to_string())?;

    Ok(asset)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let initial_project = Project::new("Untitled Project".to_string());
    let state = AppState(Mutex::new(EditorEngine::new(initial_project)));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            app_health_check,
            project_new,
            project_get_current,
            project_execute_command,
            project_dry_run,
            project_save,
            project_open,
            project_check_recovery,
            project_undo,
            project_redo,
            project_export,
            media_import
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::get_health_status;

    #[test]
    fn test_app_health_status() {
        let health = get_health_status();
        assert_eq!(health.status, "healthy");
        assert_eq!(health.schema_version, 1);
        assert!(!health.version.is_empty());
        assert!(!health.platform.is_empty());
    }
}
