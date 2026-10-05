#![allow(clippy::result_large_err)]

pub mod modules;

use crate::modules::command::{
    CommandEnvelope, CommandOperation, CommandReceipt, CommandService, DiffReport,
    OperationEnvelope,
};
use crate::modules::composition::{CompositionPlan, PreviewFrameState};
use crate::modules::domain::time::RationalTime;
use crate::modules::domain::{Asset, MediaKind, Project, CURRENT_SCHEMA_VERSION};
use crate::modules::export::{ExportEngine, ExportProfile, ExportReport};
use crate::modules::history::{HistoryEntry, HistoryManager};
use crate::modules::job::{JobKind, JobManager, JobStatus};
use crate::modules::media::cache::CacheManager;
use crate::modules::media::ffmpeg::FFmpegService;
use crate::modules::media::waveform::WaveformStatus;
use crate::modules::persistence::{
    check_recovery_status, load_project, save_project_atomic, RecoveryStatus,
};
use crate::modules::preview::{
    BenchmarkReport, NativeDecodedPreviewBackend, PreviewBackend, WebViewProxyPreviewBackend,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
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
    pub cache: CacheManager,
    pub jobs: JobManager,
    pub current_file_path: Option<PathBuf>,
}

impl EditorEngine {
    pub fn new(project: Project) -> Self {
        let cache_dir = std::env::temp_dir().join("tinixcut_cache");
        Self {
            service: CommandService::new(project),
            history: HistoryManager::new(100),
            cache: CacheManager::new(cache_dir),
            jobs: JobManager::new(4),
            current_file_path: None,
        }
    }
}

pub struct AppState(pub Arc<Mutex<EditorEngine>>);

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
    mut envelope: CommandEnvelope,
) -> Result<CommandReceipt, String> {
    let mut engine = state.0.lock().map_err(|e| e.to_string())?;
    let prior_snapshot = engine.service.project().clone();
    let is_dry_run = envelope.dry_run;

    // Desktop GUI automatically routes edits through client_id: "gui" (ADR-001 v3 Section 6.2)
    if envelope.client_id.is_none() {
        let cur_epoch = engine.service.project().client_registry.current_epoch;
        let next_seq = engine
            .service
            .project()
            .client_registry
            .clients
            .get("gui")
            .map(|c| c.highest_sequence + 1)
            .unwrap_or(1);
        envelope.client_id = Some("gui".to_string());
        envelope.client_epoch = Some(cur_epoch);
        envelope.sequence_number = Some(next_seq);
    }

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

    let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

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

    let cur_epoch = engine.service.project().client_registry.current_epoch;
    let next_seq = engine
        .service
        .project()
        .client_registry
        .clients
        .get("gui")
        .map(|c| c.highest_sequence + 1)
        .unwrap_or(1);

    let envelope = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: engine.service.project().project_id.clone(),
        base_revision: engine.service.project().revision,
        client_id: Some("gui".to_string()),
        client_epoch: Some(cur_epoch),
        sequence_number: Some(next_seq),
        dry_run: false,
        transaction_id: Some(format!("tx_import_{}", uuid::Uuid::new_v4().simple())),
        operations: vec![OperationEnvelope {
            operation_id: format!("op_import_{}", uuid::Uuid::new_v4().simple()),
            operation: op,
        }],
    };

    let prior_snapshot = engine.service.project().clone();
    let _receipt = engine
        .service
        .execute(envelope)
        .map_err(|e| e.to_string())?;
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

#[tauri::command]
fn preview_sample_at(state: State<'_, AppState>, frame: i64) -> Result<PreviewFrameState, String> {
    let engine = state.0.lock().map_err(|e| e.to_string())?;
    let project = engine.service.project();
    let plan = CompositionPlan::from_project(project).map_err(|e| e.to_string())?;
    let timestamp = RationalTime::new(frame, project.settings.frame_rate);
    Ok(plan.sample_preview_at(timestamp))
}

#[tauri::command]
fn preview_run_benchmark(
    state: State<'_, AppState>,
    duration_secs: Option<u32>,
) -> Result<Vec<BenchmarkReport>, String> {
    let engine = state.0.lock().map_err(|e| e.to_string())?;
    let project = engine.service.project();
    let dur = duration_secs.unwrap_or(30);

    let mut candidate_a = WebViewProxyPreviewBackend::new();
    let report_a = candidate_a
        .run_workload_benchmark(project, dur)
        .map_err(|e| e.to_string())?;

    let mut candidate_b = NativeDecodedPreviewBackend::new();
    let report_b = candidate_b
        .run_workload_benchmark(project, dur)
        .map_err(|e| e.to_string())?;

    Ok(vec![report_a, report_b])
}

#[tauri::command]
fn parity_run_suite() -> Result<crate::modules::composition::parity::ParitySuiteReport, String> {
    Ok(crate::modules::composition::parity::run_all_parity_fixtures())
}

#[tauri::command]
fn media_get_waveform(
    state: State<'_, AppState>,
    asset_id: String,
) -> Result<WaveformStatus, String> {
    let mut engine = state.0.lock().map_err(|e| e.to_string())?;

    // 1. Check cache first
    if let Some(data) = engine.cache.load_waveform(&asset_id) {
        return Ok(WaveformStatus::Ready(data));
    }

    // 2. Find asset in project
    let project = engine.service.project();
    let asset = match project.assets.iter().find(|a| a.asset_id == asset_id) {
        Some(a) => a.clone(),
        None => {
            return Ok(WaveformStatus::Unavailable {
                reason: format!("Asset '{}' not found in project", asset_id),
            });
        }
    };

    let source_path = PathBuf::from(&asset.path);
    if !source_path.exists() {
        return Ok(WaveformStatus::Unavailable {
            reason: format!("Media file not found at path '{}'", asset.path),
        });
    }

    // 3. Route through JobManager with deduplication and bounded concurrency
    let dedupe_key = format!("waveform:{}", asset_id);
    let job_id_candidate = format!("wf_{}_{}", asset_id, uuid::Uuid::new_v4().simple());
    let (job_id, is_deduped) =
        engine
            .jobs
            .submit_job(job_id_candidate, JobKind::Waveform, dedupe_key, 10);

    if let Some(job) = engine.jobs.get_job(&job_id) {
        match &job.status {
            JobStatus::Completed => {
                if let Some(data) = engine.cache.load_waveform(&asset_id) {
                    return Ok(WaveformStatus::Ready(data));
                }
            }
            JobStatus::Failed(reason) => {
                return Ok(WaveformStatus::Unavailable {
                    reason: reason.clone(),
                });
            }
            JobStatus::Cancelled => {
                return Ok(WaveformStatus::Unavailable {
                    reason: "Waveform extraction was cancelled".to_string(),
                });
            }
            JobStatus::Running => {
                return Ok(WaveformStatus::Pending {
                    progress: job.progress,
                });
            }
            JobStatus::Queued => {
                if !is_deduped && engine.jobs.can_start() {
                    if let Ok(cancel_flag) = engine.jobs.start_job(&job_id) {
                        let cache_out = engine.cache.waveform_path(&asset_id);
                        let state_arc = Arc::clone(&state.0);
                        let jid = job_id.clone();
                        let aid = asset_id.clone();

                        std::thread::spawn(move || {
                            if cancel_flag.load(Ordering::SeqCst) {
                                if let Ok(mut eng) = state_arc.lock() {
                                    eng.jobs.cancel_job(&jid);
                                }
                                return;
                            }

                            let result =
                                FFmpegService::extract_waveform(&source_path, &aid, 50, &cache_out);

                            if let Ok(mut eng) = state_arc.lock() {
                                if cancel_flag.load(Ordering::SeqCst) {
                                    eng.jobs.cancel_job(&jid);
                                } else {
                                    match result {
                                        Ok(_) => eng.jobs.mark_completed(&jid),
                                        Err(err) => eng.jobs.mark_failed(&jid, err.to_string()),
                                    }
                                }
                            }
                        });
                    }
                }
                return Ok(WaveformStatus::Pending { progress: 0.0 });
            }
        }
    }

    Ok(WaveformStatus::Pending { progress: 0.0 })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let initial_project = Project::new("Untitled Project".to_string());
    let state = AppState(Arc::new(Mutex::new(EditorEngine::new(initial_project))));

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
            media_import,
            media_get_waveform,
            preview_sample_at,
            preview_run_benchmark,
            parity_run_suite
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
