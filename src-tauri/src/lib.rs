#![allow(clippy::result_large_err)]

pub mod modules;

use crate::modules::app_settings::{
    load_settings, record_recent_project, save_settings, settings_path, AppSettings,
};
use crate::modules::automation::edit_plan::{
    build_clip_reorder_plan, build_clip_trim_plan,
    build_deterministic_video_draft_with_supporting_media, build_local_ai_clip_reorder_plan,
    build_local_ai_video_draft, parse_draft_caption_file, validate_edit_plan, EditPlanDraftReceipt,
    LOCAL_LM_STUDIO_PROVIDER,
};
use crate::modules::automation::{ExportJobStatusReport, ExportStartReceipt};
use crate::modules::command::{
    CommandEnvelope, CommandOperation, CommandReceipt, CommandService, DiffReport,
    OperationEnvelope,
};
use crate::modules::composition::{CompositionPlan, PreviewFrameState};
use crate::modules::domain::id::{generate_item_id, generate_operation_id, generate_track_id};
use crate::modules::domain::time::{FrameRate, RationalTime};
use crate::modules::domain::{
    Asset, AssetDimensions, ItemKind, MediaKind, Project, TrackKind, CURRENT_SCHEMA_VERSION,
};
use crate::modules::export::{
    ExportEngine, ExportPreflightReport, ExportPreset, ExportProfile, ExportReport,
};
use crate::modules::history::{HistoryEntry, HistoryManager};
use crate::modules::job::{JobKind, JobManager, JobStatus};
use crate::modules::media::cache::CacheManager;
use crate::modules::media::captions::{
    parse_caption_bytes, serialize_captions, CaptionCue, CaptionFormat, MAX_CAPTION_FILE_BYTES,
};
use crate::modules::media::collect::{collect_project_media, CollectedMediaReceipt};
use crate::modules::media::ffmpeg::{FFmpegService, MediaMetadata};
use crate::modules::media::resolver::MediaBinaryResolver;
use crate::modules::media::transcription::{
    TranscriptSourceEntry, TranscriptionBoundary, TranscriptionResult,
};
use crate::modules::media::visual_scene::{
    aggregate_source_fingerprint, analyze_visual_scenes, config_fingerprint,
    VisualSceneAnalysisReport, VisualSceneSource, MAX_SCENES_PER_REQUEST, MAX_SELECTED_ASSETS,
};
use crate::modules::media::waveform::WaveformStatus;
use crate::modules::persistence::{
    check_recovery_status, load_project, load_project_revision_if_exists,
    recovery_snapshot_fingerprint, remove_project_if_revision, same_project_path,
    save_project_atomic_if_revision, save_recovery_snapshot_if_newer, RecoveryStatus,
};
use crate::modules::preview::{
    BenchmarkReport, NativeDecodedPreviewBackend, PreviewBackend, WebViewProxyPreviewBackend,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Manager, State};

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
    caption_reports: std::collections::HashMap<String, AutoCaptionReport>,
    visual_scene_reports: std::collections::HashMap<String, VisualSceneAnalysisJobReport>,
    pub current_file_path: Option<PathBuf>,
    pub persisted_revision: Option<u64>,
    current_unsaved_recovery_path: Option<PathBuf>,
}

impl EditorEngine {
    pub fn new(project: Project) -> Self {
        let cache_dir = std::env::temp_dir().join("tinixcut_cache");
        Self {
            service: CommandService::new(project),
            history: HistoryManager::new(100),
            cache: CacheManager::new(cache_dir),
            jobs: JobManager::new(4),
            caption_reports: std::collections::HashMap::new(),
            visual_scene_reports: std::collections::HashMap::new(),
            current_file_path: None,
            persisted_revision: None,
            current_unsaved_recovery_path: None,
        }
    }
}

fn install_saved_recovery_project(
    engine: &mut EditorEngine,
    project: Project,
    canonical_path: PathBuf,
    persisted_revision: u64,
) {
    engine.service = CommandService::new(project);
    engine.history = HistoryManager::new(100);
    engine.caption_reports.clear();
    engine.current_file_path = Some(canonical_path);
    engine.persisted_revision = Some(persisted_revision);
    // The unsaved recovery path belongs to the previously open project. Keep
    // its file intact; it must not be removed by a later save of this project.
    engine.current_unsaved_recovery_path = None;
}

fn cleanup_current_unsaved_recovery_after_save(engine: &mut EditorEngine) -> Result<(), String> {
    if let Some(recovery_path) = engine.current_unsaved_recovery_path.take() {
        if let Err(error) = fs::remove_file(&recovery_path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                engine.current_unsaved_recovery_path = Some(recovery_path);
                return Err(error.to_string());
            }
        }
    }
    Ok(())
}

fn verify_recovery_fingerprint(project: &Project, expected: &str) -> Result<(), String> {
    let actual = recovery_snapshot_fingerprint(project).map_err(|error| error.to_string())?;
    if actual != expected {
        return Err(
            "The recovery snapshot content changed while the prompt was open; review it again"
                .to_string(),
        );
    }
    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AutoCaptionRequest {
    item_id: String,
    whisper_cli_path: String,
    model_path: String,
    language: Option<String>,
    #[serde(default)]
    analyze_only: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AutoCaptionReport {
    job_id: String,
    state: String,
    progress: f32,
    message: String,
    caption_count: Option<usize>,
    track_id: Option<String>,
    transcript: Option<Vec<TranscriptSourceEntry>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VisualSceneAnalysisRequest {
    asset_ids: Vec<String>,
    query: String,
    model: String,
    max_scenes: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct VisualSceneAnalysisJobReport {
    job_id: String,
    state: String,
    progress: f32,
    message: String,
    report: Option<VisualSceneAnalysisReport>,
}

#[derive(Debug, Clone)]
struct AutoCaptionTarget {
    item: crate::modules::domain::TimelineItem,
    asset_id: String,
    asset_path: String,
    analyze_only: bool,
}

pub struct AppState(
    pub Arc<Mutex<EditorEngine>>,
    pub Arc<Mutex<std::collections::HashMap<String, tokio::sync::oneshot::Sender<()>>>>,
    pub Arc<Mutex<JobManager>>,
);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoverySnapshotReceipt {
    project_revision: u64,
    recovery_revision: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryRestoreReceipt {
    project: Project,
    persisted_revision: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UnsavedRecoveryCandidate {
    project_id: String,
    name: String,
    revision: u64,
    updated_at_ms: u128,
    asset_count: usize,
    track_count: usize,
    clip_count: usize,
    preview_item_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UnsavedRecoveryReceipt {
    project_revision: u64,
    recovery_path: String,
}

fn unsaved_recovery_path(root: &Path, project_id: &str) -> Result<PathBuf, String> {
    if project_id.is_empty()
        || !project_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    {
        return Err("Invalid project ID for unsaved recovery storage".to_string());
    }
    Ok(root.join(format!("{project_id}.unsaved.tinix")))
}

fn list_unsaved_recovery_candidates(
    recovery_root: &Path,
) -> Result<Vec<UnsavedRecoveryCandidate>, String> {
    if !recovery_root.exists() {
        return Ok(Vec::new());
    }
    let mut candidates = Vec::new();
    for entry in fs::read_dir(recovery_root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if !path.is_file()
            || !path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().ends_with(".unsaved.tinix"))
        {
            continue;
        }
        let project = match load_project(&path) {
            Ok(project) => project,
            Err(_) => continue,
        };
        let updated_at_ms = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis())
            .unwrap_or_default();
        let asset_count = project.assets.len();
        let track_count = project.tracks.len();
        let clip_count = project.tracks.iter().map(|track| track.items.len()).sum();
        let preview_item_names = recovery_preview_item_names(&project);
        candidates.push(UnsavedRecoveryCandidate {
            project_id: project.project_id,
            name: project.name,
            revision: project.revision,
            updated_at_ms,
            asset_count,
            track_count,
            clip_count,
            preview_item_names,
        });
    }
    candidates.sort_by(|left, right| right.updated_at_ms.cmp(&left.updated_at_ms));
    Ok(candidates)
}

fn recovery_preview_item_names(project: &Project) -> Vec<String> {
    let item_names = project
        .tracks
        .iter()
        .flat_map(|track| track.items.iter().map(|item| item.name.as_str()));
    let asset_names = project.assets.iter().map(|asset| asset.name.as_str());
    let mut seen = std::collections::HashSet::new();
    item_names
        .chain(asset_names)
        .filter_map(|name| {
            let leaf = name.rsplit(['/', '\\']).next()?.trim();
            if leaf.is_empty() || !seen.insert(leaf.to_lowercase()) {
                return None;
            }
            Some(leaf.chars().take(64).collect::<String>())
        })
        .take(3)
        .collect()
}

fn save_unsaved_recovery_snapshot(
    project: &Project,
    path: &Path,
    expected_revision: u64,
) -> Result<(), String> {
    if project.revision != expected_revision {
        return Err(format!(
            "Project revision changed before recovery save (expected {expected_revision}, actual {})",
            project.revision
        ));
    }
    if path.exists() {
        let current = load_project(path).map_err(|error| error.to_string())?;
        if current.project_id != project.project_id {
            return Err("Unsaved recovery path belongs to a different project".to_string());
        }
        if current.revision > project.revision {
            return Err(format!(
                "A newer recovery revision {} already exists",
                current.revision
            ));
        }
        if current.revision == project.revision && current == *project {
            return Ok(());
        }
        if current.revision == project.revision {
            return Err(
                "A different recovery snapshot already exists at the same revision".to_string(),
            );
        }
    }
    let expected_disk_revision =
        load_project_revision_if_exists(path).map_err(|e| e.to_string())?;
    save_project_atomic_if_revision(project, path, expected_disk_revision)
        .map_err(|error| error.to_string())
}

fn prepare_recovery_snapshot(project: &Project, persisted_revision: u64) -> Project {
    let mut snapshot = project.clone();
    snapshot.revision = project.revision.max(persisted_revision.saturating_add(1));
    snapshot
}

#[derive(Debug)]
struct MediaImportDetails {
    media_kind: MediaKind,
    duration: Option<RationalTime>,
    dimensions: Option<AssetDimensions>,
}

fn inspect_media_for_import(
    file_name: &str,
    extension: &str,
    probe_result: Result<MediaMetadata, String>,
) -> Result<MediaImportDetails, String> {
    let is_image = matches!(extension, "png" | "jpg" | "jpeg" | "webp" | "bmp");
    let is_supported_media = is_image
        || matches!(
            extension,
            "mp4" | "mov" | "avi" | "mkv" | "mp3" | "wav" | "aac" | "flac" | "ogg" | "m4a"
        );
    if !is_supported_media {
        return Err(format!(
            "Unsupported media format for '{}': .{}",
            file_name,
            if extension.is_empty() {
                "(no extension)"
            } else {
                extension
            }
        ));
    }

    let probed = probe_result.map_err(|error| {
        format!(
            "Could not read media file '{}'. The file may be unsupported or corrupted: {}",
            file_name, error
        )
    })?;
    let has_video = probed
        .video_codec
        .as_deref()
        .is_some_and(|codec| !codec.is_empty());
    let has_audio = probed
        .audio_codec
        .as_deref()
        .is_some_and(|codec| !codec.is_empty());
    let media_kind = if is_image {
        if !has_video {
            return Err(format!(
                "Could not read image file '{}': no image/video stream was found",
                file_name
            ));
        }
        MediaKind::Image
    } else if has_video {
        MediaKind::Video
    } else if has_audio {
        MediaKind::Audio
    } else {
        return Err(format!(
            "Could not read media file '{}': no audio or video stream was found",
            file_name
        ));
    };

    let duration = media_duration_from_seconds(probed.duration_seconds);
    let dimensions = (media_kind == MediaKind::Video)
        .then(|| {
            (probed.width > 0 && probed.height > 0).then_some(AssetDimensions {
                width: probed.width,
                height: probed.height,
            })
        })
        .flatten();

    Ok(MediaImportDetails {
        media_kind,
        duration,
        dimensions,
    })
}

const PREVIEW_PROXY_HEIGHT: u32 = 720;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", content = "payload")]
enum PreviewVideoSourceStatus {
    Ready { path: String, is_proxy: bool },
    Pending,
    Unavailable { reason: String },
}

#[tauri::command]
fn app_health_check() -> HealthStatus {
    get_health_status()
}

/// Validates and hashes a `.cube` file, returning the reference a clip stores.
#[tauri::command]
fn lut_describe(path: String) -> Result<modules::domain::ClipLut, String> {
    modules::domain::ClipLut::from_file(std::path::Path::new(&path)).map_err(|e| e.to_string())
}

/// 3D LUT table for the preview shader. `data_base64` is little-endian f32 RGB triples,
/// red-fastest. The pinned content hash must still match, otherwise the call fails.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LutTable {
    size: u32,
    domain_min: [f32; 3],
    domain_max: [f32; 3],
    data_base64: String,
}

#[tauri::command]
fn lut_read_table(path: String, sha256: String) -> Result<LutTable, String> {
    use base64::Engine;
    let path = std::path::Path::new(&path);
    let actual = modules::media::lut::lut_file_sha256(path).map_err(|e| e.to_string())?;
    if !actual.eq_ignore_ascii_case(&sha256) {
        return Err("LUT file changed since it was added (content hash differs)".to_string());
    }
    let lut = modules::media::lut::Lut3D::from_file(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::with_capacity(lut.samples.len() * 12);
    for sample in &lut.samples {
        for value in sample {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    Ok(LutTable {
        size: lut.size,
        domain_min: lut.domain_min,
        domain_max: lut.domain_max,
        data_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
    })
}

#[tauri::command]
fn app_settings_get(app: AppHandle) -> Result<AppSettings, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("Could not resolve app settings directory: {error}"))?;
    load_settings(&settings_path(&config_dir))
}

#[tauri::command]
fn app_settings_save(app: AppHandle, settings: AppSettings) -> Result<AppSettings, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("Could not resolve app settings directory: {error}"))?;
    let path = settings_path(&config_dir);
    save_settings(&settings, &path)?;
    load_settings(&path)
}

#[tauri::command]
fn app_settings_add_recent_project(
    app: AppHandle,
    project_path: String,
) -> Result<AppSettings, String> {
    let canonical = fs::canonicalize(&project_path).map_err(|error| {
        format!(
            "Could not resolve recent project path '{}': {error}",
            project_path
        )
    })?;
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("Could not resolve app settings directory: {error}"))?;
    let path = settings_path(&config_dir);
    let mut settings = load_settings(&path)?;
    record_recent_project(&mut settings, &canonical.to_string_lossy())?;
    save_settings(&settings, &path)?;
    load_settings(&path)
}

#[tauri::command]
fn project_new(state: State<'_, AppState>, name: String) -> Result<Project, String> {
    let mut engine = state.0.lock().map_err(|e| e.to_string())?;
    let proj = Project::new(name);
    engine.service = CommandService::new(proj.clone());
    engine.history = HistoryManager::new(100);
    engine.caption_reports.clear();
    engine.current_file_path = None;
    engine.persisted_revision = None;
    engine.current_unsaved_recovery_path = None;
    Ok(proj)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CurrentProjectSession {
    pub project: Project,
    pub canonical_path: Option<String>,
    pub persisted_revision: Option<u64>,
}

pub fn project_get_current_session_from_engine(
    engine: &EditorEngine,
) -> Result<CurrentProjectSession, String> {
    Ok(CurrentProjectSession {
        project: engine.service.project().clone(),
        canonical_path: engine
            .current_file_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string()),
        persisted_revision: engine.persisted_revision,
    })
}

#[tauri::command]
fn project_get_current(state: State<'_, AppState>) -> Result<Project, String> {
    let engine = state.0.lock().map_err(|e| e.to_string())?;
    Ok(engine.service.project().clone())
}

#[tauri::command]
fn project_get_current_session(
    state: State<'_, AppState>,
) -> Result<CurrentProjectSession, String> {
    let engine = state.0.lock().map_err(|e| e.to_string())?;
    project_get_current_session_from_engine(&engine)
}

#[tauri::command]
async fn edit_plan_draft_preview(
    state: State<'_, AppState>,
    goal: String,
    media_ids: Vec<String>,
    target_duration_frames: u64,
    audio_asset_id: Option<String>,
    subtitle_file_path: Option<String>,
    planner_provider: Option<String>,
    local_model: Option<String>,
    request_id: Option<String>,
) -> Result<EditPlanDraftReceipt, String> {
    let project = state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .service
        .project()
        .clone();
    let captions = subtitle_file_path
        .as_deref()
        .map(|path| {
            parse_draft_caption_file(std::path::Path::new(path), project.settings.frame_rate)
        })
        .transpose()?
        .unwrap_or_default();
    let receipt = match planner_provider
        .as_deref()
        .unwrap_or("deterministic-mock-v1")
    {
        "deterministic-mock-v1" => {
            let plan = build_deterministic_video_draft_with_supporting_media(
                &project,
                &goal,
                &media_ids,
                target_duration_frames,
                audio_asset_id.as_deref(),
                &captions,
            )?;
            let validation = validate_edit_plan(&project, &plan);
            EditPlanDraftReceipt {
                provider: crate::modules::automation::edit_plan::DETERMINISTIC_DRAFT_PROVIDER
                    .to_string(),
                plan,
                validation,
            }
        }
        crate::modules::automation::edit_plan::LOCAL_LM_STUDIO_PROVIDER => {
            let generation = build_local_ai_video_draft(
                &project,
                &goal,
                &media_ids,
                target_duration_frames,
                audio_asset_id.as_deref(),
                &captions,
                local_model.as_deref().unwrap_or("google/gemma-4-e4b"),
            );
            if let Some(request_id) = request_id {
                if request_id.trim().is_empty() || request_id.len() > 128 {
                    return Err("Invalid local planner request ID".to_string());
                }
                let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel();
                {
                    let mut active = state
                        .1
                        .lock()
                        .map_err(|_| "Local planner cancellation registry is unavailable")?;
                    if active.contains_key(&request_id) {
                        return Err(
                            "A local planner request with this ID is already active".to_string()
                        );
                    }
                    active.insert(request_id.clone(), cancel_tx);
                }
                let result = crate::modules::automation::edit_plan::wait_for_local_planner_request(
                    generation, cancel_rx,
                )
                .await;
                state
                    .1
                    .lock()
                    .map_err(|_| "Local planner cancellation registry is unavailable")?
                    .remove(&request_id);
                result?
            } else {
                generation.await?
            }
        }
        provider => return Err(format!("Unsupported edit-plan provider: {provider}")),
    };
    let engine = state.0.lock().map_err(|e| e.to_string())?;
    let current_project = engine.service.project();
    if current_project.project_id != project.project_id
        || current_project.revision != project.revision
    {
        return Err(
            "Project changed while the draft was being generated; generate a fresh preview."
                .to_string(),
        );
    }
    let validation = validate_edit_plan(current_project, &receipt.plan);
    if !validation.valid {
        return Err(validation.issues.join("; "));
    }
    Ok(EditPlanDraftReceipt {
        provider: receipt.provider,
        plan: receipt.plan,
        validation,
    })
}

#[tauri::command]
fn edit_plan_scene_draft_preview(
    state: State<'_, AppState>,
    goal: String,
    report: crate::modules::media::visual_scene::VisualSceneAnalysisReport,
    selected_scene_ids: Vec<String>,
    target_duration_frames: Option<u64>,
    exact_duration: Option<bool>,
    must_keep_scene_ids: Vec<String>,
    must_omit_scene_ids: Vec<String>,
    allowed_asset_ids: Vec<String>,
    target_canvas: Option<crate::modules::automation::edit_plan::SceneCanvasPreset>,
    opening_title: Option<String>,
    title_template: Option<crate::modules::automation::edit_plan::SceneTitleTemplate>,
) -> Result<EditPlanDraftReceipt, String> {
    let project = state
        .0
        .lock()
        .map_err(|error| error.to_string())?
        .service
        .project()
        .clone();
    let receipt =
        crate::modules::automation::edit_plan::build_visual_scene_edit_plan_with_constraints(
            &project,
            &goal,
            &report,
            &selected_scene_ids,
            target_duration_frames,
            exact_duration.unwrap_or(false),
            crate::modules::automation::edit_plan::SceneSelectionConstraints {
                allowed_asset_ids,
                must_keep_scene_ids,
                must_omit_scene_ids,
                caption_intent: None,
                audio_intent: None,
                suggest_opening_title: false,
            },
            target_canvas,
            opening_title.as_deref(),
            title_template,
        )?;
    let engine = state.0.lock().map_err(|error| error.to_string())?;
    let current_project = engine.service.project();
    if current_project.project_id != project.project_id
        || current_project.revision != project.revision
    {
        return Err(
            "Project changed while the scene draft was being generated; generate a fresh preview."
                .to_string(),
        );
    }
    let validation = validate_edit_plan(current_project, &receipt.plan);
    if !validation.valid {
        return Err(validation.issues.join("; "));
    }
    Ok(EditPlanDraftReceipt {
        validation,
        ..receipt
    })
}

#[tauri::command]
async fn edit_plan_scene_selection_suggest(
    state: State<'_, AppState>,
    goal: String,
    report: crate::modules::media::visual_scene::VisualSceneAnalysisReport,
    target_duration_seconds: Option<u32>,
    must_keep_scene_ids: Vec<String>,
    must_omit_scene_ids: Vec<String>,
    allowed_asset_ids: Vec<String>,
    caption_intent: Option<String>,
    audio_intent: Option<String>,
    suggest_opening_title: bool,
    local_model: String,
) -> Result<crate::modules::automation::edit_plan::SceneSelectionSuggestion, String> {
    let project = state
        .0
        .lock()
        .map_err(|error| error.to_string())?
        .service
        .project()
        .clone();
    let suggestion =
        crate::modules::automation::edit_plan::suggest_visual_scene_selection_with_constraints(
            &project,
            &goal,
            &report,
            target_duration_seconds,
            &local_model,
            crate::modules::automation::edit_plan::SceneSelectionConstraints {
                allowed_asset_ids,
                must_keep_scene_ids,
                must_omit_scene_ids,
                caption_intent,
                audio_intent,
                suggest_opening_title,
            },
        )
        .await?;
    let engine = state.0.lock().map_err(|error| error.to_string())?;
    if engine.service.project().project_id != project.project_id
        || engine.service.project().revision != project.revision
    {
        return Err(
            "Project changed while the scene suggestion was generated; try again.".to_string(),
        );
    }
    Ok(suggestion)
}

#[tauri::command]
async fn edit_plan_reorder_preview(
    state: State<'_, AppState>,
    goal: String,
    track_id: String,
    item_ids: Vec<String>,
    ordered_item_ids: Vec<String>,
    planner_provider: Option<String>,
    local_model: Option<String>,
    request_id: Option<String>,
) -> Result<EditPlanDraftReceipt, String> {
    let project = state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .service
        .project()
        .clone();
    let provider = planner_provider.as_deref().unwrap_or("caller-order-v1");
    let receipt = match provider {
        "caller-order-v1" => {
            let plan =
                build_clip_reorder_plan(&project, &goal, &track_id, &item_ids, &ordered_item_ids)?;
            let validation = validate_edit_plan(&project, &plan);
            if !validation.valid {
                return Err(validation.issues.join("; "));
            }
            EditPlanDraftReceipt {
                provider: "caller-order-v1".to_string(),
                plan,
                validation,
            }
        }
        LOCAL_LM_STUDIO_PROVIDER => {
            let generation = build_local_ai_clip_reorder_plan(
                &project,
                &goal,
                &track_id,
                &item_ids,
                local_model.as_deref().unwrap_or("google/gemma-4-e4b"),
            );
            if let Some(request_id) = request_id {
                if request_id.trim().is_empty() || request_id.len() > 128 {
                    return Err("Invalid local planner request ID".to_string());
                }
                let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel();
                {
                    let mut active = state
                        .1
                        .lock()
                        .map_err(|_| "Local planner cancellation registry is unavailable")?;
                    if active.contains_key(&request_id) {
                        return Err(
                            "A local planner request with this ID is already active".to_string()
                        );
                    }
                    active.insert(request_id.clone(), cancel_tx);
                }
                let result = crate::modules::automation::edit_plan::wait_for_local_planner_request(
                    generation, cancel_rx,
                )
                .await;
                state
                    .1
                    .lock()
                    .map_err(|_| "Local planner cancellation registry is unavailable")?
                    .remove(&request_id);
                result?
            } else {
                generation.await?
            }
        }
        unsupported => return Err(format!("Unsupported reorder planner: {unsupported}")),
    };

    let engine = state.0.lock().map_err(|e| e.to_string())?;
    let current_project = engine.service.project();
    if current_project.project_id != project.project_id
        || current_project.revision != project.revision
    {
        return Err(
            "Project changed while the reorder plan was being generated; generate a fresh preview."
                .to_string(),
        );
    }
    let validation = validate_edit_plan(current_project, &receipt.plan);
    if !validation.valid {
        return Err(validation.issues.join("; "));
    }
    Ok(EditPlanDraftReceipt {
        provider: receipt.provider,
        plan: receipt.plan,
        validation,
    })
}

#[tauri::command]
async fn edit_plan_trim_preview(
    state: State<'_, AppState>,
    goal: String,
    item_id: String,
    target_duration_frames: u64,
) -> Result<EditPlanDraftReceipt, String> {
    let project = state
        .0
        .lock()
        .map_err(|error| error.to_string())?
        .service
        .project()
        .clone();
    let plan = build_clip_trim_plan(&project, &goal, &item_id, target_duration_frames)?;
    let validation = validate_edit_plan(&project, &plan);
    if !validation.valid {
        return Err(validation.issues.join("; "));
    }
    Ok(EditPlanDraftReceipt {
        provider: "caller-trim-v1".to_string(),
        plan,
        validation,
    })
}

#[tauri::command]
fn edit_plan_draft_cancel(state: State<'_, AppState>, request_id: String) -> Result<bool, String> {
    let cancel = state
        .1
        .lock()
        .map_err(|_| "Local planner cancellation registry is unavailable")?
        .remove(&request_id);
    Ok(cancel.is_some_and(|sender| sender.send(()).is_ok()))
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
fn project_save(
    app: AppHandle,
    state: State<'_, AppState>,
    path: Option<String>,
) -> Result<String, String> {
    let mut engine = state.0.lock().map_err(|e| e.to_string())?;
    let target = if let Some(p) = path {
        PathBuf::from(p)
    } else if let Some(ref p) = engine.current_file_path {
        p.clone()
    } else {
        return Err("No path specified for project save".to_string());
    };
    require_explicit_file_scope(
        &target,
        asset_scope_allows_path(&app.asset_protocol_scope(), &target),
        "Project save",
    )?;

    let same_path = match engine.current_file_path.as_deref() {
        Some(current_path) => {
            same_project_path(current_path, &target).map_err(|e| e.to_string())?
        }
        None => false,
    };
    let expected_revision = if same_path {
        engine.persisted_revision
    } else {
        load_project_revision_if_exists(&target).map_err(|e| e.to_string())?
    };
    save_project_atomic_if_revision(engine.service.project(), &target, expected_revision)
        .map_err(|e| e.to_string())?;
    engine.current_file_path = Some(target.clone());
    engine.persisted_revision = Some(engine.service.project().revision);
    if let Err(error) = cleanup_current_unsaved_recovery_after_save(&mut engine) {
        return Err(format!(
            "Project was saved to '{}', but its unsaved recovery copy could not be removed: {error}",
            target.display()
        ));
    }
    Ok(target.to_string_lossy().to_string())
}

#[tauri::command]
fn project_open(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<Project, String> {
    let pbuf = PathBuf::from(&path);
    require_explicit_file_scope(
        &pbuf,
        asset_scope_allows_path(&app.asset_protocol_scope(), &pbuf),
        "Project open",
    )?;
    let mut engine = state.0.lock().map_err(|e| e.to_string())?;
    let proj = load_project(&pbuf).map_err(|e| e.to_string())?;
    engine.service = CommandService::new(proj.clone());
    engine.history = HistoryManager::new(100);
    engine.caption_reports.clear();
    engine.current_file_path = Some(pbuf);
    engine.persisted_revision = Some(proj.revision);
    engine.current_unsaved_recovery_path = None;
    Ok(proj)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectVariantCreateReceipt {
    path: String,
    source_project_id: String,
    source_revision: u64,
    project: Project,
}

#[tauri::command]
fn project_variant_create(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    name: String,
    expected_project_id: String,
    expected_revision: u64,
) -> Result<ProjectVariantCreateReceipt, String> {
    let engine = state.0.lock().map_err(|e| e.to_string())?;
    let source = engine.service.project();
    if source.project_id != expected_project_id || source.revision != expected_revision {
        return Err("Project changed before variant creation; refresh and try again".to_string());
    }
    let path = PathBuf::from(path);
    require_explicit_file_scope(
        &path,
        asset_scope_allows_path(&app.asset_protocol_scope(), &path),
        "Project variant creation",
    )?;
    if path.exists() {
        return Err(format!(
            "Variant destination already exists: {}",
            path.display()
        ));
    }
    let source_project_id = source.project_id.clone();
    let source_revision = source.revision;
    let variant = crate::modules::automation::clone_project_as_variant(source, &name)?;
    save_project_atomic_if_revision(&variant, &path, None).map_err(|e| e.to_string())?;
    Ok(ProjectVariantCreateReceipt {
        path: path.to_string_lossy().to_string(),
        source_project_id,
        source_revision,
        project: variant,
    })
}

#[tauri::command]
fn project_collect_media(
    app: AppHandle,
    state: State<'_, AppState>,
    destination_directory: String,
    expected_project_id: String,
    expected_revision: u64,
) -> Result<CollectedMediaReceipt, String> {
    let engine = state.0.lock().map_err(|e| e.to_string())?;
    let source = engine.service.project();
    if source.project_id != expected_project_id || source.revision != expected_revision {
        return Err("Project changed before media collection; refresh and try again".to_string());
    }
    let target = PathBuf::from(destination_directory);
    require_explicit_file_scope(
        &target,
        asset_scope_allows_path(&app.asset_protocol_scope(), &target),
        "Media collection directory",
    )?;
    collect_project_media(source, &target, |candidate| {
        asset_scope_allows_path(&app.asset_protocol_scope(), candidate)
    })
}

#[tauri::command]
fn project_compare_current_with_path(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    expected_project_id: String,
    expected_revision: u64,
) -> Result<crate::modules::automation::mcp::ProjectCompareReport, String> {
    let engine = state.0.lock().map_err(|e| e.to_string())?;
    let current = engine.service.project();
    if current.project_id != expected_project_id || current.revision != expected_revision {
        return Err("Current project changed before comparison; refresh and try again".to_string());
    }
    let compare_path = Path::new(&path);
    require_explicit_file_scope(
        compare_path,
        asset_scope_allows_path(&app.asset_protocol_scope(), compare_path),
        "Project comparison",
    )?;
    let other = load_project(compare_path).map_err(|e| e.to_string())?;
    Ok(crate::modules::automation::mcp::project_compare_report(
        current, &other,
    ))
}

#[tauri::command]
fn project_check_recovery(app: AppHandle, path: String) -> Result<RecoveryStatus, String> {
    let canonical = PathBuf::from(&path);
    require_explicit_file_scope(
        &canonical,
        asset_scope_allows_path(&app.asset_protocol_scope(), &canonical),
        "Project recovery check",
    )?;
    let recovery = canonical.with_extension("tcproj.recovery");
    check_recovery_status(&canonical, &recovery).map_err(|e| e.to_string())
}

#[tauri::command]
fn project_save_recovery_snapshot(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    expected_revision: u64,
) -> Result<RecoverySnapshotReceipt, String> {
    let engine = state.0.lock().map_err(|e| e.to_string())?;
    let target = PathBuf::from(path);
    require_explicit_file_scope(
        &target,
        asset_scope_allows_path(&app.asset_protocol_scope(), &target),
        "Project recovery save",
    )?;
    let current_path = engine
        .current_file_path
        .as_deref()
        .ok_or_else(|| "Save the project before creating a recovery snapshot".to_string())?;
    if !same_project_path(current_path, &target).map_err(|error| error.to_string())? {
        return Err(
            "The active project changed before its recovery snapshot was saved".to_string(),
        );
    }

    let project = engine.service.project();
    if project.revision != expected_revision {
        return Err(format!(
            "Recovery snapshot revision changed (expected {expected_revision}, actual {})",
            project.revision
        ));
    }

    let persisted_revision = engine
        .persisted_revision
        .ok_or_else(|| "The active project has no persisted revision".to_string())?;
    let snapshot = prepare_recovery_snapshot(project, persisted_revision);
    let recovery_revision = snapshot.revision;
    let recovery_path = target.with_extension("tcproj.recovery");
    save_recovery_snapshot_if_newer(&snapshot, &recovery_path)
        .map_err(|error| error.to_string())?;

    Ok(RecoverySnapshotReceipt {
        project_revision: project.revision,
        recovery_revision,
    })
}

#[tauri::command]
fn project_restore_recovery(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    expected_project_revision: u64,
    expected_recovery_revision: u64,
    expected_recovery_fingerprint: String,
) -> Result<RecoveryRestoreReceipt, String> {
    let mut engine = state.0.lock().map_err(|e| e.to_string())?;
    let canonical = PathBuf::from(path);
    require_explicit_file_scope(
        &canonical,
        asset_scope_allows_path(&app.asset_protocol_scope(), &canonical),
        "Project recovery restore",
    )?;
    let recovery_path = canonical.with_extension("tcproj.recovery");
    let status =
        check_recovery_status(&canonical, &recovery_path).map_err(|error| error.to_string())?;
    match status {
        RecoveryStatus::NewerRecoveryAvailable {
            recovery_revision,
            project_revision,
            recovery_fingerprint,
            ..
        }
        | RecoveryStatus::RecoveryDiffersFromProject {
            recovery_revision,
            project_revision,
            recovery_fingerprint,
            ..
        } if recovery_revision == expected_recovery_revision
            && project_revision == expected_project_revision
            && recovery_fingerprint == expected_recovery_fingerprint => {}
        RecoveryStatus::NewerRecoveryAvailable { .. }
        | RecoveryStatus::RecoveryDiffersFromProject { .. } => {
            return Err(
                "The recovery snapshot changed. Reopen the recovery prompt and review it again."
                    .to_string(),
            );
        }
        _ => {
            return Err("No differing recovery snapshot is available for this project".to_string())
        }
    }

    let canonical_project = load_project(&canonical).map_err(|error| error.to_string())?;
    let mut recovered_project = load_project(&recovery_path).map_err(|error| error.to_string())?;
    if canonical_project.revision != expected_project_revision
        || recovered_project.revision != expected_recovery_revision
    {
        return Err(
            "Project or recovery snapshot changed while the recovery prompt was open".to_string(),
        );
    }
    verify_recovery_fingerprint(&recovered_project, &expected_recovery_fingerprint)?;
    if recovered_project.project_id != canonical_project.project_id {
        return Err("The recovery snapshot belongs to a different project".to_string());
    }
    recovered_project.revision = canonical_project
        .revision
        .max(recovered_project.revision)
        .checked_add(1)
        .ok_or_else(|| "Project revision cannot be advanced beyond u64::MAX".to_string())?;

    install_saved_recovery_project(
        &mut engine,
        recovered_project.clone(),
        canonical,
        canonical_project.revision,
    );

    Ok(RecoveryRestoreReceipt {
        project: recovered_project,
        persisted_revision: canonical_project.revision,
    })
}

#[tauri::command]
fn project_save_unsaved_recovery(
    app: AppHandle,
    state: State<'_, AppState>,
    expected_project_id: String,
    expected_revision: u64,
) -> Result<UnsavedRecoveryReceipt, String> {
    let mut engine = state.0.lock().map_err(|e| e.to_string())?;
    if engine.current_file_path.is_some() {
        return Err(
            "Project already has a canonical path; use its project recovery snapshot".to_string(),
        );
    }
    let project = engine.service.project();
    if project.project_id != expected_project_id || project.revision != expected_revision {
        return Err("Project changed before unsaved recovery could be saved".to_string());
    }
    let recovery_root = app
        .path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())?
        .join("recovery")
        .join("unsaved");
    fs::create_dir_all(&recovery_root).map_err(|error| error.to_string())?;
    let recovery_path = unsaved_recovery_path(&recovery_root, &project.project_id)?;
    save_unsaved_recovery_snapshot(project, &recovery_path, expected_revision)?;
    engine.current_unsaved_recovery_path = Some(recovery_path.clone());
    Ok(UnsavedRecoveryReceipt {
        project_revision: expected_revision,
        recovery_path: recovery_path.to_string_lossy().to_string(),
    })
}

#[tauri::command]
fn project_list_unsaved_recoveries(
    app: AppHandle,
) -> Result<Vec<UnsavedRecoveryCandidate>, String> {
    let recovery_root = app
        .path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())?
        .join("recovery")
        .join("unsaved");
    list_unsaved_recovery_candidates(&recovery_root)
}

#[tauri::command]
fn project_restore_unsaved_recovery(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
    expected_recovery_revision: u64,
    expected_current_project_id: String,
    expected_current_revision: u64,
) -> Result<Project, String> {
    let mut engine = state.0.lock().map_err(|e| e.to_string())?;
    restore_unsaved_recovery_engine(
        &mut engine,
        &app.path()
            .app_local_data_dir()
            .map_err(|error| error.to_string())?
            .join("recovery")
            .join("unsaved"),
        &project_id,
        expected_recovery_revision,
        &expected_current_project_id,
        expected_current_revision,
    )
}

fn restore_unsaved_recovery_engine(
    engine: &mut EditorEngine,
    recovery_root: &Path,
    project_id: &str,
    expected_recovery_revision: u64,
    expected_current_project_id: &str,
    expected_current_revision: u64,
) -> Result<Project, String> {
    let current = engine.service.project();
    if current.project_id != expected_current_project_id
        || current.revision != expected_current_revision
        || current.revision != 0
        || !current.assets.is_empty()
        || !current.tracks.is_empty()
    {
        return Err(
            "The open project changed; refusing to replace it with recovery data".to_string(),
        );
    }
    let path = unsaved_recovery_path(recovery_root, project_id)?;
    let recovered = load_project(&path).map_err(|error| error.to_string())?;
    if recovered.project_id != project_id || recovered.revision != expected_recovery_revision {
        return Err("The unsaved recovery snapshot changed; refresh the recovery list".to_string());
    }
    engine.service = CommandService::new(recovered.clone());
    engine.history = HistoryManager::new(100);
    engine.caption_reports.clear();
    engine.current_file_path = None;
    engine.persisted_revision = None;
    engine.current_unsaved_recovery_path = Some(path);
    Ok(recovered)
}

#[tauri::command]
fn project_discard_unsaved_recovery(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
    expected_recovery_revision: u64,
) -> Result<(), String> {
    let engine = state.0.lock().map_err(|e| e.to_string())?;
    let recovery_root = app
        .path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())?
        .join("recovery")
        .join("unsaved");
    let path = unsaved_recovery_path(&recovery_root, &project_id)?;
    if engine.current_unsaved_recovery_path.as_deref() == Some(path.as_path()) {
        return Err(
            "This recovery is currently open; save it or create a new project before discarding"
                .to_string(),
        );
    }
    remove_project_if_revision(&path, &project_id, expected_recovery_revision)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn project_undo(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let mut engine = state.0.lock().map_err(|e| e.to_string())?;
    Ok(undo_editor_engine(&mut engine))
}

#[tauri::command]
fn project_redo(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let mut engine = state.0.lock().map_err(|e| e.to_string())?;
    Ok(redo_editor_engine(&mut engine))
}

fn undo_editor_engine(engine: &mut EditorEngine) -> Option<String> {
    let mut current = engine.service.project().clone();
    let current_revision = current.revision;
    let client_registry = current.client_registry.clone();
    let command_ledger = current.command_ledger.clone();
    let desc = engine.history.undo(&mut current);
    if desc.is_some() {
        // History restores document content, but revisions identify committed states
        // and must remain monotonic for stale-writer and recovery compare-and-swap checks.
        current.revision = current_revision.saturating_add(1);
        current.client_registry = client_registry;
        current.command_ledger = command_ledger;
        engine.service.set_project(current);
    }
    desc
}

fn redo_editor_engine(engine: &mut EditorEngine) -> Option<String> {
    let mut current = engine.service.project().clone();
    let current_revision = current.revision;
    let client_registry = current.client_registry.clone();
    let command_ledger = current.command_ledger.clone();
    let desc = engine.history.redo(&mut current);
    if desc.is_some() {
        current.revision = current_revision.saturating_add(1);
        current.client_registry = client_registry;
        current.command_ledger = command_ledger;
        engine.service.set_project(current);
    }
    desc
}

#[tauri::command]
fn project_export_preflight(
    app: AppHandle,
    state: State<'_, AppState>,
    output_path: String,
    profile: Option<ExportProfile>,
    preset: Option<ExportPreset>,
) -> Result<ExportPreflightReport, String> {
    let snapshot = {
        let engine = state.0.lock().map_err(|error| error.to_string())?;
        engine.service.project().clone()
    };
    require_project_media_scope(&snapshot, |path| {
        asset_scope_allows_path(&app.asset_protocol_scope(), path)
    })?;
    let target = PathBuf::from(&output_path);
    require_explicit_file_scope(
        &target,
        asset_scope_allows_path(&app.asset_protocol_scope(), &target),
        "Export destination",
    )?;
    if profile.is_some() && preset.is_some() {
        return Err("Provide either an export profile or an export preset, not both".to_string());
    }
    let profile = match (profile, preset) {
        (Some(profile), None) => profile,
        (None, Some(preset)) => preset.profile_for(&snapshot.settings),
        (None, None) => ExportProfile::from_project_settings(&snapshot.settings),
        (Some(_), Some(_)) => unreachable!("validated above"),
    };
    ExportEngine::preflight(&snapshot, &target, &profile).map_err(|error| error.to_string())
}

#[tauri::command]
fn project_export(
    app: AppHandle,
    state: State<'_, AppState>,
    output_path: String,
    profile: Option<ExportProfile>,
    preset: Option<ExportPreset>,
) -> Result<ExportReport, String> {
    let engine = state.0.lock().map_err(|e| e.to_string())?;
    let snapshot = engine.service.project();
    require_project_media_scope(snapshot, |path| {
        asset_scope_allows_path(&app.asset_protocol_scope(), path)
    })?;
    require_explicit_file_scope(
        Path::new(&output_path),
        asset_scope_allows_path(&app.asset_protocol_scope(), Path::new(&output_path)),
        "Export destination",
    )?;
    let target = PathBuf::from(&output_path);
    if profile.is_some() && preset.is_some() {
        return Err("Provide either an export profile or an export preset, not both".to_string());
    }
    let prof = match (profile, preset) {
        (Some(profile), None) => profile,
        (None, Some(preset)) => preset.profile_for(&snapshot.settings),
        (None, None) => ExportProfile::from_project_settings(&snapshot.settings),
        (Some(_), Some(_)) => unreachable!("validated above"),
    };
    prof.validate_for_project(&snapshot.settings)
        .map_err(|error| error.to_string())?;
    ExportEngine::render_snapshot(snapshot, &target, &prof, None).map_err(|e| e.to_string())
}

#[tauri::command]
fn project_export_start(
    app: AppHandle,
    state: State<'_, AppState>,
    output_path: String,
    profile: Option<ExportProfile>,
    preset: Option<ExportPreset>,
) -> Result<ExportStartReceipt, String> {
    let snapshot = {
        let engine = state.0.lock().map_err(|error| error.to_string())?;
        engine.service.project().clone()
    };
    require_project_media_scope(&snapshot, |path| {
        asset_scope_allows_path(&app.asset_protocol_scope(), path)
    })?;
    let target = PathBuf::from(&output_path);
    require_explicit_file_scope(
        &target,
        asset_scope_allows_path(&app.asset_protocol_scope(), &target),
        "Export destination",
    )?;
    if profile.is_some() && preset.is_some() {
        return Err("Provide either an export profile or an export preset, not both".to_string());
    }
    let profile = match (profile, preset) {
        (Some(profile), None) => profile,
        (None, Some(preset)) => preset.profile_for(&snapshot.settings),
        (None, None) => ExportProfile::from_project_settings(&snapshot.settings),
        (Some(_), Some(_)) => unreachable!("validated above"),
    };
    profile
        .validate_for_project(&snapshot.settings)
        .map_err(|error| error.to_string())?;
    ExportEngine::preflight(&snapshot, &target, &profile).map_err(|error| error.to_string())?;

    let dedupe_key = format!(
        "ui-export:{}:{}:{}",
        snapshot.project_id,
        snapshot.revision,
        target.display()
    );
    let (job_id, cancel_flag) = {
        let mut jobs = state.2.lock().map_err(|error| error.to_string())?;
        if jobs.has_active_kind(JobKind::Export) {
            return Err(
                "An export is already running. Cancel or wait for it to finish.".to_string(),
            );
        }
        let job_id = format!("job_ui_export_{}", uuid::Uuid::new_v4().simple());
        let (job_id, _) = jobs.submit_job(job_id, JobKind::Export, dedupe_key, 100);
        let cancel_flag = jobs.start_job(&job_id)?;
        (job_id, cancel_flag)
    };

    let manager = Arc::clone(&state.2);
    let worker_job_id = job_id.clone();
    std::thread::spawn(move || {
        let progress_manager = Arc::clone(&manager);
        let progress_job_id = worker_job_id.clone();
        let result = ExportEngine::render_snapshot_with_progress(
            &snapshot,
            &target,
            &profile,
            Some(cancel_flag),
            move |progress| {
                if let Ok(mut jobs) = progress_manager.lock() {
                    jobs.update_progress(&progress_job_id, progress);
                }
            },
        );
        if let Ok(mut jobs) = manager.lock() {
            match result {
                Ok(_) => jobs.mark_completed(&worker_job_id),
                Err(crate::modules::export::ExportError::Cancelled) => {
                    jobs.cancel_job(&worker_job_id);
                }
                Err(error) => jobs.mark_failed(&worker_job_id, error.to_string()),
            }
        }
    });

    Ok(ExportStartReceipt {
        job_id,
        status: "running".to_string(),
        output_path,
    })
}

#[tauri::command]
fn project_export_status(
    state: State<'_, AppState>,
    job_id: String,
) -> Result<ExportJobStatusReport, String> {
    let jobs = state.2.lock().map_err(|error| error.to_string())?;
    let job = jobs
        .get_job(&job_id)
        .ok_or_else(|| format!("Export job '{job_id}' was not found"))?;
    let (status, error) = match &job.status {
        JobStatus::Queued => ("queued".to_string(), None),
        JobStatus::Running => ("running".to_string(), None),
        JobStatus::Completed => ("completed".to_string(), None),
        JobStatus::Cancelled => ("cancelled".to_string(), None),
        JobStatus::Failed(reason) => ("failed".to_string(), Some(reason.clone())),
    };
    Ok(ExportJobStatusReport {
        job_id: job.id.clone(),
        status,
        progress: job.progress,
        error,
    })
}

#[tauri::command]
fn project_export_cancel(state: State<'_, AppState>, job_id: String) -> Result<bool, String> {
    let mut jobs = state.2.lock().map_err(|error| error.to_string())?;
    Ok(jobs.request_cancel(&job_id))
}

#[tauri::command]
fn media_import(
    app: AppHandle,
    state: State<'_, AppState>,
    file_path: String,
) -> Result<Asset, String> {
    let path = Path::new(&file_path);
    require_explicit_file_scope(
        path,
        asset_scope_allows_path(&app.asset_protocol_scope(), path),
        "Media import",
    )?;
    if !path.exists() || !path.is_file() {
        return Err(format!("Media file not found at path: {}", file_path));
    }

    let file_name = path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "Imported Media".to_string());

    let file_size = std::fs::metadata(path)
        .map_err(|error| format!("Could not read media file '{}': {}", file_name, error))?
        .len();

    // Determine media kind from the readable streams before mutating the project.
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let details = inspect_media_for_import(
        &file_name,
        &ext,
        FFmpegService::probe_file(path).map_err(|error| error.to_string()),
    )?;

    let mut engine = state.0.lock().map_err(|e| e.to_string())?;
    let asset_id = format!("ast_{}", uuid::Uuid::new_v4().simple());
    let op = CommandOperation::AssetImport {
        asset_id: Some(asset_id.clone()),
        name: file_name,
        path: file_path.clone(),
        media_kind: details.media_kind,
        duration: details.duration,
        file_size,
        dimensions: details.dimensions,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct OfflineMediaAsset {
    asset_id: String,
    last_known_path: String,
    permission_required: bool,
}

fn offline_media_asset(asset: &Asset, scope_allows_path: bool) -> Option<OfflineMediaAsset> {
    match crate::modules::media::MediaCatalog::check_asset_status(asset) {
        crate::modules::media::AssetOnlineStatus::Online if scope_allows_path => None,
        crate::modules::media::AssetOnlineStatus::Online => Some(OfflineMediaAsset {
            asset_id: asset.asset_id.clone(),
            last_known_path: asset.path.clone(),
            permission_required: true,
        }),
        crate::modules::media::AssetOnlineStatus::Offline { last_known_path } => {
            Some(OfflineMediaAsset {
                asset_id: asset.asset_id.clone(),
                last_known_path,
                permission_required: false,
            })
        }
    }
}

fn require_explicit_file_scope(
    path: &Path,
    scope_allows_path: bool,
    operation: &str,
) -> Result<(), String> {
    if scope_allows_path {
        Ok(())
    } else {
        Err(format!(
            "{operation} requires explicit file access through TinixCut: {}",
            path.display()
        ))
    }
}

fn canonicalize_scope_candidate(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| format!("Could not resolve current directory: {error}"))?
            .join(path)
    };

    let mut ancestor = absolute;
    let mut missing_components = Vec::new();
    loop {
        match fs::symlink_metadata(&ancestor) {
            Ok(_) => {
                // Resolve the nearest existing ancestor so a missing output path below a
                // symlink/junction is checked against its real destination, not its alias.
                let mut resolved = fs::canonicalize(&ancestor).map_err(|error| {
                    format!(
                        "Could not resolve scoped path '{}': {error}",
                        path.display()
                    )
                })?;
                for component in missing_components.iter().rev() {
                    resolved.push(component);
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let component = ancestor.file_name().ok_or_else(|| {
                    format!(
                        "Could not resolve scoped path '{}': {error}",
                        path.display()
                    )
                })?;
                missing_components.push(component.to_os_string());
                if !ancestor.pop() {
                    return Err(format!(
                        "Could not resolve scoped path '{}': {error}",
                        path.display()
                    ));
                }
            }
            Err(error) => {
                return Err(format!(
                    "Could not inspect scoped path '{}': {error}",
                    path.display()
                ));
            }
        }
    }
}

fn asset_scope_allows_path(scope: &tauri::fs::Scope, path: &Path) -> bool {
    canonicalize_scope_candidate(path).is_ok_and(|candidate| scope.is_allowed(candidate))
}

fn require_project_media_scope(
    project: &Project,
    mut is_allowed: impl FnMut(&Path) -> bool,
) -> Result<(), String> {
    let referenced_asset_ids = project
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .filter_map(|item| item.asset_id.as_deref())
        .collect::<std::collections::HashSet<_>>();

    for asset in project
        .assets
        .iter()
        .filter(|asset| referenced_asset_ids.contains(asset.asset_id.as_str()))
    {
        let path = Path::new(&asset.path);
        // Preserve the normal missing-media/export diagnostic. The scope check protects only
        // existing files that the native renderer could otherwise read.
        if path.is_file() {
            require_explicit_file_scope(path, is_allowed(path), "Project media access")?;
        }
    }
    Ok(())
}

#[tauri::command]
fn media_list_offline_assets(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<OfflineMediaAsset>, String> {
    let asset_scope = app.asset_protocol_scope();
    let engine = state.0.lock().map_err(|e| e.to_string())?;
    Ok(engine
        .service
        .project()
        .assets
        .iter()
        .filter_map(|asset| offline_media_asset(asset, asset_scope.is_allowed(&asset.path)))
        .collect())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CaptionImportReport {
    format: CaptionFormat,
    track_id: String,
    cue_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CaptionExportReport {
    output_path: String,
    format: CaptionFormat,
    cue_count: usize,
}

#[tauri::command]
fn captions_import(
    app: AppHandle,
    state: State<'_, AppState>,
    file_path: String,
) -> Result<CaptionImportReport, String> {
    let path = Path::new(&file_path);
    require_explicit_file_scope(
        path,
        asset_scope_allows_path(&app.asset_protocol_scope(), path),
        "Caption import",
    )?;
    if !path.exists() || !path.is_file() {
        return Err(format!("Caption file not found at path: {}", file_path));
    }
    let format = CaptionFormat::from_path(path)?;
    let metadata =
        fs::metadata(path).map_err(|error| format!("Could not inspect caption file: {error}"))?;
    if metadata.len() > MAX_CAPTION_FILE_BYTES as u64 {
        return Err(format!(
            "Caption file is too large ({} bytes; limit is {} bytes)",
            metadata.len(),
            MAX_CAPTION_FILE_BYTES
        ));
    }
    let bytes = fs::read(path).map_err(|error| format!("Could not read caption file: {error}"))?;

    let mut engine = state.0.lock().map_err(|error| error.to_string())?;
    let frame_rate = engine.service.project().settings.frame_rate;
    let cues = parse_caption_bytes(&bytes, format, frame_rate)?;
    let track_id = generate_track_id();
    let transaction_id = format!("tx_import_captions_{}", uuid::Uuid::new_v4().simple());
    let mut operations = Vec::with_capacity(1 + cues.len() * 2);
    operations.push(OperationEnvelope {
        operation_id: generate_operation_id(),
        operation: CommandOperation::TrackCreate {
            track_id: Some(track_id.clone()),
            name: "Captions".to_string(),
            kind: TrackKind::Video,
        },
    });

    for (index, cue) in cues.iter().enumerate() {
        let item_id = generate_item_id();
        operations.push(OperationEnvelope {
            operation_id: generate_operation_id(),
            operation: CommandOperation::ClipCreate {
                track_id: track_id.clone(),
                item_id: Some(item_id.clone()),
                name: format!("Caption {}", index + 1),
                kind: ItemKind::Text,
                asset_id: None,
                timeline_start: cue.start,
                duration: cue.duration,
                source_start: Some(RationalTime::zero(frame_rate)),
            },
        });
        operations.push(OperationEnvelope {
            operation_id: generate_operation_id(),
            operation: CommandOperation::ClipTextSet {
                item_id,
                text: cue.text.clone(),
            },
        });
    }

    let cur_epoch = engine.service.project().client_registry.current_epoch;
    let next_seq = engine
        .service
        .project()
        .client_registry
        .clients
        .get("gui")
        .map(|client| client.highest_sequence + 1)
        .unwrap_or(1);
    let envelope = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: engine.service.project().project_id.clone(),
        base_revision: engine.service.project().revision,
        client_id: Some("gui".to_string()),
        client_epoch: Some(cur_epoch),
        sequence_number: Some(next_seq),
        dry_run: false,
        transaction_id: Some(transaction_id.clone()),
        operations,
    };
    let prior_snapshot = engine.service.project().clone();
    engine
        .service
        .execute(envelope)
        .map_err(|error| format!("Caption import command failed: {error}"))?;
    let resulting_snapshot = engine.service.project().clone();
    engine.history.record_edit(HistoryEntry {
        description: format!("Import captions {}", file_path),
        transaction_id: Some(transaction_id),
        prior_snapshot,
        resulting_snapshot,
    });

    Ok(CaptionImportReport {
        format,
        track_id,
        cue_count: cues.len(),
    })
}

#[tauri::command]
fn captions_auto_start(
    app: AppHandle,
    state: State<'_, AppState>,
    request: AutoCaptionRequest,
) -> Result<AutoCaptionReport, String> {
    let whisper_cli_path = PathBuf::from(request.whisper_cli_path.trim());
    let model_path = PathBuf::from(request.model_path.trim());
    if !whisper_cli_path.is_file() {
        return Err(format!(
            "Whisper CLI executable was not found: {}",
            whisper_cli_path.display()
        ));
    }
    if !model_path.is_file() {
        return Err(format!(
            "Whisper model was not found: {}",
            model_path.display()
        ));
    }

    let language = request
        .language
        .as_deref()
        .filter(|value| !value.trim().is_empty() && *value != "auto")
        .map(str::to_owned);
    let (job_id, target, audio_path, cancel_flag) = {
        let mut engine = state.0.lock().map_err(|error| error.to_string())?;
        if engine.jobs.has_active_kind(JobKind::Transcription) {
            return Err("Another automatic-caption job is already running. Cancel it or wait for it to finish.".to_string());
        }
        let item = engine
            .service
            .project()
            .tracks
            .iter()
            .flat_map(|track| track.items.iter())
            .find(|item| item.item_id == request.item_id)
            .cloned()
            .ok_or_else(|| format!("Selected clip '{}' no longer exists", request.item_id))?;
        if !matches!(item.kind, ItemKind::Video | ItemKind::Audio) {
            return Err("Select a video or audio clip before generating captions.".to_string());
        }
        let asset_id = item
            .asset_id
            .clone()
            .ok_or_else(|| "The selected clip has no source media.".to_string())?;
        let asset = engine
            .service
            .project()
            .assets
            .iter()
            .find(|asset| asset.asset_id == asset_id)
            .cloned()
            .ok_or_else(|| {
                "The selected clip's source media is missing from this project.".to_string()
            })?;
        if !matches!(asset.media_kind, MediaKind::Video | MediaKind::Audio) {
            return Err("The selected clip does not reference audio-capable media.".to_string());
        }
        if !Path::new(&asset.path).is_file() {
            return Err(format!("Source media is unavailable: {}", asset.path));
        }
        let asset_path = Path::new(&asset.path);
        require_explicit_file_scope(
            asset_path,
            asset_scope_allows_path(&app.asset_protocol_scope(), asset_path),
            "Automatic captions",
        )?;
        let job_id = format!("job_caption_{}", uuid::Uuid::new_v4().simple());
        let key = format!(
            "caption:{}:{}:{}:{}:{}",
            item.item_id,
            item.timeline_range.start,
            item.source_range.start,
            item.speed.as_f64(),
            language.as_deref().unwrap_or("auto")
        );
        let (job_id, deduped) = engine
            .jobs
            .submit_job(job_id, JobKind::Transcription, key, 100);
        if deduped {
            return Ok(auto_caption_status_for(&engine, &job_id));
        }
        let cancel_flag = engine.jobs.start_job(&job_id)?;
        engine.caption_reports.insert(
            job_id.clone(),
            AutoCaptionReport {
                job_id: job_id.clone(),
                state: "running".to_string(),
                progress: 0.05,
                message: "Preparing audio for local transcription…".to_string(),
                caption_count: None,
                track_id: None,
                transcript: None,
            },
        );
        (
            job_id.clone(),
            AutoCaptionTarget {
                item,
                asset_id,
                asset_path: asset.path.clone(),
                analyze_only: request.analyze_only,
            },
            engine.cache.transcription_audio_path(&job_id),
            cancel_flag,
        )
    };

    let app_state = Arc::clone(&state.0);
    let worker_job_id = job_id.clone();
    thread::spawn(move || {
        run_auto_caption_job(
            app_state,
            worker_job_id,
            target,
            audio_path,
            whisper_cli_path,
            model_path,
            language,
            cancel_flag,
        )
    });

    Ok(AutoCaptionReport {
        job_id,
        state: "running".to_string(),
        progress: 0.05,
        message: "Preparing audio for local transcription…".to_string(),
        caption_count: None,
        track_id: None,
        transcript: None,
    })
}

#[tauri::command]
fn captions_auto_status(
    state: State<'_, AppState>,
    job_id: String,
) -> Result<AutoCaptionReport, String> {
    let engine = state.0.lock().map_err(|error| error.to_string())?;
    if engine.jobs.get_job(&job_id).is_none() {
        return Err(format!("Caption job '{job_id}' was not found"));
    }
    Ok(auto_caption_status_for(&engine, &job_id))
}

#[tauri::command]
fn captions_auto_cancel(state: State<'_, AppState>, job_id: String) -> Result<(), String> {
    let mut engine = state.0.lock().map_err(|error| error.to_string())?;
    if !engine.jobs.cancel_job(&job_id) {
        return Err(format!("Caption job '{job_id}' was not found"));
    }
    engine.caption_reports.insert(
        job_id.clone(),
        AutoCaptionReport {
            job_id,
            state: "cancelled".to_string(),
            progress: 0.0,
            message: "Automatic-caption job cancelled.".to_string(),
            caption_count: None,
            track_id: None,
            transcript: None,
        },
    );
    Ok(())
}

fn auto_caption_status_for(engine: &EditorEngine, job_id: &str) -> AutoCaptionReport {
    if let Some(report) = engine.caption_reports.get(job_id) {
        return report.clone();
    }
    let job = engine.jobs.get_job(job_id).expect("checked by caller");
    let (state, message) = match &job.status {
        JobStatus::Queued => ("queued", "Waiting for an available worker."),
        JobStatus::Running => ("running", "Generating captions locally…"),
        JobStatus::Completed => ("completed", "Captions generated."),
        JobStatus::Failed(reason) => ("failed", reason.as_str()),
        JobStatus::Cancelled => ("cancelled", "Automatic-caption job cancelled."),
    };
    AutoCaptionReport {
        job_id: job_id.to_string(),
        state: state.to_string(),
        progress: job.progress,
        message: message.to_string(),
        caption_count: None,
        track_id: None,
        transcript: None,
    }
}

#[tauri::command]
fn visual_scene_analysis_start(
    app: AppHandle,
    state: State<'_, AppState>,
    request: VisualSceneAnalysisRequest,
) -> Result<VisualSceneAnalysisJobReport, String> {
    let max_scenes = request.max_scenes.unwrap_or(16);
    if request.asset_ids.is_empty()
        || request.asset_ids.len() > MAX_SELECTED_ASSETS
        || request
            .asset_ids
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != request.asset_ids.len()
    {
        return Err(format!(
            "Select 1 to {MAX_SELECTED_ASSETS} unique video assets."
        ));
    }
    if request.query.trim().is_empty() || request.query.len() > 4_000 {
        return Err("Enter a visual search query of 1 to 4000 bytes.".to_string());
    }
    if request.model.trim().is_empty() || request.model.len() > 160 {
        return Err("Enter a valid local vision model ID.".to_string());
    }
    if !(request.asset_ids.len()..=MAX_SCENES_PER_REQUEST).contains(&max_scenes) {
        return Err(format!(
            "Scene limit must be between {} and {MAX_SCENES_PER_REQUEST}.",
            request.asset_ids.len()
        ));
    }

    let (job_id, sources, cancel_flag) = {
        let mut engine = state.0.lock().map_err(|error| error.to_string())?;
        if engine.jobs.has_active_kind(JobKind::VisualAnalysis) {
            return Err(
                "Another visual analysis is already running. Cancel it or wait for it to finish."
                    .to_string(),
            );
        }
        let mut sources = Vec::with_capacity(request.asset_ids.len());
        for asset_id in &request.asset_ids {
            let asset = engine
                .service
                .project()
                .assets
                .iter()
                .find(|asset| asset.asset_id == *asset_id)
                .ok_or_else(|| "A selected media item is no longer in this project.".to_string())?;
            if asset.media_kind != MediaKind::Video {
                return Err("Visual scene analysis supports selected video files only.".to_string());
            }
            let path = PathBuf::from(&asset.path);
            if !path.is_file() {
                return Err(format!(
                    "Selected video '{}' is offline or missing.",
                    asset.name
                ));
            }
            require_explicit_file_scope(
                &path,
                asset_scope_allows_path(&app.asset_protocol_scope(), &path),
                "Local visual analysis",
            )?;
            sources.push(VisualSceneSource {
                asset_id: asset.asset_id.clone(),
                asset_name: asset.name.clone(),
                path,
            });
        }
        let source_key = aggregate_source_fingerprint(&sources)?;
        let config_key = config_fingerprint(&request.model, &request.query, max_scenes);
        if let Some(report) = engine
            .cache
            .load_visual_scene_analysis(&source_key, &config_key)
        {
            let cached_id = format!("visual_cached_{}", uuid::Uuid::new_v4().simple());
            let cached = VisualSceneAnalysisJobReport {
                job_id: cached_id.clone(),
                state: "completed".to_string(),
                progress: 1.0,
                message: "Loaded cached visual scene analysis.".to_string(),
                report: Some(report),
            };
            engine
                .visual_scene_reports
                .insert(cached_id, cached.clone());
            return Ok(cached);
        }
        let job_id = format!("job_visual_scene_{}", uuid::Uuid::new_v4().simple());
        let (job_id, deduped) = engine.jobs.submit_job(
            job_id,
            JobKind::VisualAnalysis,
            format!("visual:{source_key}:{config_key}"),
            80,
        );
        if deduped {
            return Ok(engine.visual_scene_reports.get(&job_id).cloned().unwrap_or(
                VisualSceneAnalysisJobReport {
                    job_id,
                    state: "running".to_string(),
                    progress: 0.05,
                    message: "Visual scene analysis is already running.".to_string(),
                    report: None,
                },
            ));
        }
        let cancel_flag = engine.jobs.start_job(&job_id)?;
        engine.visual_scene_reports.insert(
            job_id.clone(),
            VisualSceneAnalysisJobReport {
                job_id: job_id.clone(),
                state: "running".to_string(),
                progress: 0.05,
                message: "Sampling selected video scenes for local analysis.".to_string(),
                report: None,
            },
        );
        (job_id, sources, cancel_flag)
    };

    let shared_state = Arc::clone(&state.0);
    let worker_job_id = job_id.clone();
    let query = request.query;
    let model = request.model;
    thread::spawn(move || {
        let result = (|| -> Result<VisualSceneAnalysisReport, String> {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|error| format!("Could not start local analysis runtime: {error}"))?;
            runtime.block_on(analyze_visual_scenes(
                &sources,
                &query,
                &model,
                max_scenes,
                Arc::clone(&cancel_flag),
            ))
        })();
        if cancel_flag.load(Ordering::SeqCst) {
            return;
        }
        if let Ok(mut engine) = shared_state.lock() {
            if cancel_flag.load(Ordering::SeqCst) {
                return;
            }
            match result {
                Ok(report) => {
                    let _ = engine.cache.save_visual_scene_analysis(&report);
                    engine.jobs.mark_completed(&worker_job_id);
                    engine.visual_scene_reports.insert(
                        worker_job_id.clone(),
                        VisualSceneAnalysisJobReport {
                            job_id: worker_job_id,
                            state: "completed".to_string(),
                            progress: 1.0,
                            message: "Visual scene analysis completed.".to_string(),
                            report: Some(report),
                        },
                    );
                }
                Err(error) => {
                    engine.jobs.mark_failed(&worker_job_id, error.clone());
                    engine.visual_scene_reports.insert(
                        worker_job_id.clone(),
                        VisualSceneAnalysisJobReport {
                            job_id: worker_job_id,
                            state: "failed".to_string(),
                            progress: 0.0,
                            message: error,
                            report: None,
                        },
                    );
                }
            }
        }
    });

    Ok(VisualSceneAnalysisJobReport {
        job_id,
        state: "running".to_string(),
        progress: 0.05,
        message: "Sampling selected video scenes for local analysis.".to_string(),
        report: None,
    })
}

#[tauri::command]
fn visual_scene_analysis_status(
    state: State<'_, AppState>,
    job_id: String,
) -> Result<VisualSceneAnalysisJobReport, String> {
    let engine = state.0.lock().map_err(|error| error.to_string())?;
    engine
        .visual_scene_reports
        .get(&job_id)
        .cloned()
        .ok_or_else(|| format!("Visual scene analysis job '{job_id}' was not found."))
}

#[tauri::command]
fn visual_scene_analysis_cancel(state: State<'_, AppState>, job_id: String) -> Result<(), String> {
    let mut engine = state.0.lock().map_err(|error| error.to_string())?;
    if !engine.jobs.cancel_job(&job_id) {
        return Err(format!(
            "Visual scene analysis job '{job_id}' was not found."
        ));
    }
    engine.visual_scene_reports.insert(
        job_id.clone(),
        VisualSceneAnalysisJobReport {
            job_id,
            state: "cancelled".to_string(),
            progress: 0.0,
            message: "Visual scene analysis cancelled.".to_string(),
            report: None,
        },
    );
    Ok(())
}

fn run_auto_caption_job(
    state: Arc<Mutex<EditorEngine>>,
    job_id: String,
    target: AutoCaptionTarget,
    wav_path: PathBuf,
    whisper_cli_path: PathBuf,
    model_path: PathBuf,
    language: Option<String>,
    cancel_flag: Arc<std::sync::atomic::AtomicBool>,
) {
    let output_json_path = wav_path.with_extension("whisper.json");
    let result = (|| -> Result<(usize, String, Option<Vec<TranscriptSourceEntry>>, Option<String>), String> {
        let cache_identity = if target.analyze_only {
            Some(transcript_cache_identity(&target.asset_path, &whisper_cli_path, &model_path, language.as_deref())?)
        } else {
            None
        };
        if let Some((source_fingerprint, config_fingerprint)) = &cache_identity {
            update_auto_caption_progress(&state, &job_id, 0.08, "Checking saved source transcript…");
            let cached = state.lock().map_err(|error| error.to_string())?.cache
                .load_transcript_analysis(source_fingerprint, config_fingerprint);
            if let Some(report) = cached {
                let engine = state.lock().map_err(|error| error.to_string())?;
                let current_item = engine.service.project().tracks.iter().flat_map(|track| &track.items)
                    .find(|item| item.item_id == target.item.item_id)
                    .ok_or_else(|| "The selected clip changed or was deleted while transcript analysis ran.".to_string())?;
                if current_item != &target.item || engine.service.project().assets.iter()
                    .find(|asset| asset.asset_id == target.asset_id)
                    .map(|asset| asset.path.as_str()) != Some(target.asset_path.as_str()) {
                    return Err("The selected clip or its media reference changed during transcript analysis; no transcript was returned.".to_string());
                }
                let entries = TranscriptionBoundary::source_entries(
                    &report.result, current_item, engine.service.project().settings.frame_rate,
                );
                if !entries.iter().any(|entry| entry.timeline_start.is_some()) {
                    return Err("Saved transcript has no segments within the selected clip's source range.".to_string());
                }
                return Ok((entries.len(), String::new(), Some(entries), None));
            }
        }
        update_auto_caption_progress(&state, &job_id, 0.15, "Extracting mono audio…");
        let mut ffmpeg = MediaBinaryResolver::command("ffmpeg")
            .map_err(|error| format!("FFmpeg is unavailable for captions: {error}"))?;
        ffmpeg
            .arg("-v")
            .arg("error")
            .arg("-i")
            .arg(&target.asset_path)
            .arg("-vn")
            .arg("-ar")
            .arg("16000")
            .arg("-ac")
            .arg("1")
            .arg("-c:a")
            .arg("pcm_s16le")
            .arg("-y")
            .arg(&wav_path);
        run_child_cancellable(ffmpeg, &cancel_flag, "Audio extraction")?;

        update_auto_caption_progress(&state, &job_id, 0.5, "Transcribing with local Whisper…");
        let output_base = wav_path.with_extension("whisper");
        let _ = fs::remove_file(&output_json_path);
        let mut whisper = Command::new(&whisper_cli_path);
        whisper
            .arg("-m")
            .arg(&model_path)
            .arg("-f")
            .arg(&wav_path)
            .arg("-oj")
            .arg("-of")
            .arg(&output_base);
        if let Some(language) = &language {
            whisper.arg("-l").arg(language);
        }
        run_child_cancellable(whisper, &cancel_flag, "Whisper transcription")?;
        if cancel_flag.load(Ordering::SeqCst) {
            return Err("Automatic-caption job cancelled.".to_string());
        }

        update_auto_caption_progress(
            &state,
            &job_id,
            0.78,
            "Converting transcript to project captions…",
        );
        let json = fs::read(&output_json_path)
            .map_err(|error| format!("Whisper did not produce JSON output: {error}"))?;
        let result = TranscriptionBoundary::parse_whisper_json(
            &json,
            language.clone().unwrap_or_else(|| "auto".to_string()),
        )?;
        if target.analyze_only {
            let engine = state.lock().map_err(|error| error.to_string())?;
            let current_item = engine
                .service
                .project()
                .tracks
                .iter()
                .flat_map(|track| &track.items)
                .find(|item| item.item_id == target.item.item_id)
                .ok_or_else(|| {
                    "The selected clip changed or was deleted while transcript analysis ran."
                        .to_string()
                })?;
            if current_item != &target.item
                || engine
                    .service
                    .project()
                    .assets
                    .iter()
                    .find(|asset| asset.asset_id == target.asset_id)
                    .map(|asset| asset.path.as_str())
                    != Some(target.asset_path.as_str())
            {
                return Err("The selected clip or its media reference changed during transcript analysis; no transcript was applied.".to_string());
            }
            let entries = TranscriptionBoundary::source_entries(
                &result,
                current_item,
                engine.service.project().settings.frame_rate,
            );
            if !entries.iter().any(|entry| entry.timeline_start.is_some()) {
                return Err("Whisper returned no transcript segments within the selected clip's source range.".to_string());
            }
            if let Some((source_fingerprint, config_fingerprint)) = cache_identity {
                let latest_fingerprint = FFmpegService::source_fingerprint(Path::new(&target.asset_path))
                    .map_err(|error| format!("Could not recheck transcript source: {error}"))?;
                if latest_fingerprint != source_fingerprint {
                    return Err("Source media changed during transcript analysis; the result was not cached or returned.".to_string());
                }
                let duration_micros = FFmpegService::probe_file(Path::new(&target.asset_path))
                    .map_err(|error| format!("Could not inspect transcript duration: {error}"))?
                    .duration_seconds.mul_add(1_000_000.0, 0.5) as u64;
                let report = crate::modules::media::transcription::TranscriptAnalysisReport {
                    schema_version: crate::modules::media::transcription::TRANSCRIPT_ANALYSIS_SCHEMA_VERSION,
                    algorithm_version: crate::modules::media::transcription::TRANSCRIPT_ANALYSIS_ALGORITHM_VERSION.to_string(),
                    source_fingerprint,
                    config_fingerprint,
                    analyzed_duration_micros: duration_micros,
                    result: result.clone(),
                };
                let cache_warning = state.lock().map_err(|error| error.to_string())?.cache
                    .save_transcript_analysis(&report)
                    .err()
                    .map(|error| format!("Transcript was analyzed but could not be cached: {error}"));
                return Ok((entries.len(), String::new(), Some(entries), cache_warning));
            }
            return Ok((entries.len(), String::new(), Some(entries), None));
        }
        let (count, track_id) = commit_generated_captions(&state, &target, result)?;
        Ok((count, track_id, None, None))
    })();
    let _ = fs::remove_file(&output_json_path);
    let _ = fs::remove_file(&wav_path);

    let mut engine = match state.lock() {
        Ok(engine) => engine,
        Err(_) => return,
    };
    if cancel_flag.load(Ordering::SeqCst)
        || matches!(
            engine.jobs.get_job(&job_id).map(|job| &job.status),
            Some(JobStatus::Cancelled)
        )
    {
        return;
    }
    match result {
        Ok((caption_count, track_id, transcript, cache_warning)) => {
            engine.jobs.mark_completed(&job_id);
            engine.caption_reports.insert(
                job_id.clone(),
                AutoCaptionReport {
                    job_id,
                    state: "completed".to_string(),
                    progress: 1.0,
                    message: if target.analyze_only {
                        format!("Analyzed {caption_count} source transcript segment(s). The project was not changed.")
                    } else {
                        format!("Generated {caption_count} captions on a new track.")
                    } + cache_warning.as_deref().map(|warning| format!(" {warning}")).unwrap_or_default().as_str(),
                    caption_count: if target.analyze_only { None } else { Some(caption_count) },
                    track_id: if target.analyze_only { None } else { Some(track_id) },
                    transcript,
                },
            );
        }
        Err(error) => {
            engine.jobs.mark_failed(&job_id, error.clone());
            engine.caption_reports.insert(
                job_id.clone(),
                AutoCaptionReport {
                    job_id,
                    state: "failed".to_string(),
                    progress: 0.0,
                    message: error,
                    caption_count: None,
                    track_id: None,
                    transcript: None,
                },
            );
        }
    }
}

fn transcript_cache_identity(
    media_path: &str,
    whisper_cli_path: &Path,
    model_path: &Path,
    language: Option<&str>,
) -> Result<(String, String), String> {
    let source_fingerprint = FFmpegService::source_fingerprint(Path::new(media_path))
        .map_err(|error| format!("Could not fingerprint transcript source: {error}"))?;
    let cli_fingerprint = FFmpegService::source_fingerprint(whisper_cli_path)
        .map_err(|error| format!("Could not fingerprint Whisper CLI: {error}"))?;
    let model_fingerprint = FFmpegService::source_fingerprint(model_path)
        .map_err(|error| format!("Could not fingerprint Whisper model: {error}"))?;
    let config = format!(
        "{}|{}|{}|{}",
        crate::modules::media::transcription::TRANSCRIPT_ANALYSIS_ALGORITHM_VERSION,
        model_fingerprint,
        cli_fingerprint,
        language
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("auto"),
    );
    let config_fingerprint = Sha256::digest(config.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Ok((source_fingerprint, config_fingerprint))
}

fn update_auto_caption_progress(
    state: &Arc<Mutex<EditorEngine>>,
    job_id: &str,
    progress: f32,
    message: &str,
) {
    if let Ok(mut engine) = state.lock() {
        if matches!(
            engine.jobs.get_job(job_id).map(|job| &job.status),
            Some(JobStatus::Cancelled)
        ) {
            return;
        }
        engine.jobs.update_progress(job_id, progress);
        engine.caption_reports.insert(
            job_id.to_string(),
            AutoCaptionReport {
                job_id: job_id.to_string(),
                state: "running".to_string(),
                progress,
                message: message.to_string(),
                caption_count: None,
                track_id: None,
                transcript: None,
            },
        );
    }
}

fn run_child_cancellable(
    mut command: Command,
    cancel_flag: &std::sync::atomic::AtomicBool,
    label: &str,
) -> Result<(), String> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("{label} could not start: {error}"))?;
    loop {
        if cancel_flag.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Automatic-caption job cancelled.".to_string());
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("{label} could not be monitored: {error}"))?
        {
            if status.success() {
                return Ok(());
            }
            return Err(format!(
                "{label} failed with exit code {}.",
                status.code().unwrap_or(-1)
            ));
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn commit_generated_captions(
    state: &Arc<Mutex<EditorEngine>>,
    target: &AutoCaptionTarget,
    transcription: TranscriptionResult,
) -> Result<(usize, String), String> {
    let mut engine = state.lock().map_err(|error| error.to_string())?;
    let current_item = engine
        .service
        .project()
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .find(|item| item.item_id == target.item.item_id)
        .ok_or_else(|| {
            "The selected clip changed or was deleted while transcription ran.".to_string()
        })?;
    if current_item != &target.item
        || engine
            .service
            .project()
            .assets
            .iter()
            .find(|asset| asset.asset_id == target.asset_id)
            .map(|asset| asset.path.as_str())
            != Some(target.asset_path.as_str())
    {
        return Err("The selected clip or its media changed while transcription ran; no captions were added.".to_string());
    }

    let mapped = TranscriptionBoundary::map_to_timeline(
        &transcription,
        current_item,
        engine.service.project().settings.frame_rate,
    );
    if mapped.segments.is_empty() {
        return Err("Whisper returned no caption segments within the selected clip.".to_string());
    }
    let track_id = generate_track_id();
    let transaction_id = format!("tx_auto_captions_{}", uuid::Uuid::new_v4().simple());
    let mut operations = vec![OperationEnvelope {
        operation_id: generate_operation_id(),
        operation: CommandOperation::TrackCreate {
            track_id: Some(track_id.clone()),
            name: "Auto Captions".to_string(),
            kind: TrackKind::Video,
        },
    }];
    operations.extend(TranscriptionBoundary::create_caption_commands(
        &track_id, &mapped,
    ));
    let cur_epoch = engine.service.project().client_registry.current_epoch;
    let next_seq = engine
        .service
        .project()
        .client_registry
        .clients
        .get("gui")
        .map(|client| client.highest_sequence + 1)
        .unwrap_or(1);
    let envelope = CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        project_id: engine.service.project().project_id.clone(),
        base_revision: engine.service.project().revision,
        client_id: Some("gui".to_string()),
        client_epoch: Some(cur_epoch),
        sequence_number: Some(next_seq),
        dry_run: false,
        transaction_id: Some(transaction_id.clone()),
        operations,
    };
    let prior_snapshot = engine.service.project().clone();
    engine
        .service
        .execute(envelope)
        .map_err(|error| format!("Automatic-caption command failed: {error}"))?;
    let resulting_snapshot = engine.service.project().clone();
    engine.history.record_edit(HistoryEntry {
        description: "Generate automatic captions".to_string(),
        transaction_id: Some(transaction_id),
        prior_snapshot,
        resulting_snapshot,
    });
    Ok((mapped.segments.len(), track_id))
}

#[tauri::command]
fn captions_export(
    app: AppHandle,
    state: State<'_, AppState>,
    output_path: String,
) -> Result<CaptionExportReport, String> {
    let target = PathBuf::from(&output_path);
    require_explicit_file_scope(
        &target,
        asset_scope_allows_path(&app.asset_protocol_scope(), &target),
        "Caption export",
    )?;
    let format = CaptionFormat::from_path(&target)?;
    let engine = state.0.lock().map_err(|error| error.to_string())?;
    let frame_rate = engine.service.project().settings.frame_rate;
    let cues = engine
        .service
        .project()
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .filter(|item| item.kind == ItemKind::Text)
        .filter_map(|item| {
            item.text_content
                .as_ref()
                .filter(|text| !text.trim().is_empty())
                .map(|text| CaptionCue {
                    start: item.timeline_range.start.rescaled_to(frame_rate),
                    duration: item.timeline_range.duration.rescaled_to(frame_rate),
                    text: text.clone(),
                })
        })
        .collect::<Vec<_>>();
    let cue_count = cues.len();
    let contents = serialize_captions(&cues, format)?;
    write_caption_file_atomic(&target, &contents)?;

    Ok(CaptionExportReport {
        output_path,
        format,
        cue_count,
    })
}

fn write_caption_file_atomic(target: &Path, contents: &str) -> Result<(), String> {
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    let name = target
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "Caption output path has no valid file name".to_string())?;
    let temporary = parent.join(format!(".{name}.tinixcut.tmp"));
    let backup = parent.join(format!(".{name}.tinixcut.bak"));

    let _ = fs::remove_file(&temporary);
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|error| format!("Could not create temporary caption file: {error}"))?;
    file.write_all(contents.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("Could not flush temporary caption file: {error}"))?;
    drop(file);

    let had_target = target.exists();
    if had_target {
        let _ = fs::remove_file(&backup);
        fs::rename(target, &backup)
            .map_err(|error| format!("Could not stage existing caption file: {error}"))?;
    }
    if let Err(error) = fs::rename(&temporary, target) {
        if had_target {
            let _ = fs::rename(&backup, target);
        }
        return Err(format!("Could not replace caption file: {error}"));
    }
    if had_target {
        let _ = fs::remove_file(backup);
    }
    Ok(())
}

fn media_duration_from_seconds(seconds: f64) -> Option<RationalTime> {
    const NANOSECONDS_PER_SECOND: u32 = 1_000_000_000;
    if !seconds.is_finite() || seconds <= 0.0 {
        return None;
    }
    let ticks = (seconds * f64::from(NANOSECONDS_PER_SECOND)).floor();
    if ticks > i64::MAX as f64 {
        return None;
    }
    Some(RationalTime::new(
        ticks as i64,
        FrameRate::new(NANOSECONDS_PER_SECOND, 1),
    ))
}

/// YouTube chapter list from the live project's markers, with any rule violations.
#[tauri::command]
fn project_chapters(
    state: State<'_, AppState>,
) -> Result<modules::media::chapters::ChapterReport, String> {
    let engine = state.0.lock().map_err(|e| e.to_string())?;
    Ok(modules::media::chapters::project_chapters(
        engine.service.project(),
    ))
}

/// Scopes of the exported frame at `frame`, computed from the export composition of the live
/// (possibly unsaved) project. Rich-text overlays are not part of this frame.
#[tauri::command]
async fn frame_scopes(
    state: State<'_, AppState>,
    frame: i64,
) -> Result<modules::media::scopes::FrameScopes, String> {
    let (plan, timestamp) = {
        let engine = state.0.lock().map_err(|e| e.to_string())?;
        let project = engine.service.project();
        let plan = CompositionPlan::from_project(project).map_err(|e| e.to_string())?;
        (plan, RationalTime::new(frame, project.settings.frame_rate))
    };
    tauri::async_runtime::spawn_blocking(move || {
        plan.verify_lut_files().map_err(|e| e.to_string())?;
        let rgba = modules::composition::parity::render_export_frame_rgba(&plan, timestamp)?;
        modules::media::scopes::compute_scopes(&rgba, plan.width, plan.height)
    })
    .await
    .map_err(|e| format!("Scopes task failed: {e}"))?
}

#[tauri::command]
fn preview_sample_at(state: State<'_, AppState>, frame: i64) -> Result<PreviewFrameState, String> {
    let engine = state.0.lock().map_err(|e| e.to_string())?;
    let project = engine.service.project();
    let plan = CompositionPlan::from_project(project).map_err(|e| e.to_string())?;
    let timestamp = RationalTime::new(frame, project.settings.frame_rate);
    Ok(plan.sample_preview_at(timestamp))
}

/// Renders one canonical rich-text overlay through the shared Phase 30 renderer.
/// Returns `None` when the item is not an active rich-text layer at `frame`.
#[tauri::command]
fn preview_rich_text_overlay(
    state: State<'_, AppState>,
    item_id: String,
    frame: i64,
) -> Result<Option<crate::modules::media::text_overlay::PreviewRichTextOverlay>, String> {
    let engine = state.0.lock().map_err(|e| e.to_string())?;
    let project = engine.service.project();
    let timestamp = RationalTime::new(frame, project.settings.frame_rate);
    crate::modules::media::text_overlay::preview_overlay_at(project, &item_id, timestamp)
        .map_err(|error| error.to_string())
}

/// The asset protocol scope starts empty and only grows through user file choices, so the
/// WebView cannot load a derived proxy until this exact cache file is allowed. Granting the
/// single file (not the cache directory) keeps the scope minimal.
fn ready_preview_proxy(app: &AppHandle, proxy_path: &Path) -> PreviewVideoSourceStatus {
    match app.asset_protocol_scope().allow_file(proxy_path) {
        Ok(()) => PreviewVideoSourceStatus::Ready {
            path: proxy_path.to_string_lossy().to_string(),
            is_proxy: true,
        },
        Err(error) => PreviewVideoSourceStatus::Unavailable {
            reason: format!("Preview proxy could not be made readable by the preview: {error}"),
        },
    }
}

/// Returns an H.264/AAC seek-optimized preview proxy, generating it in the background.
/// Used both when the WebView cannot decode an original video and, when the user enables
/// preview proxies, for responsive seeking. The proxy is derived cache only: the project
/// and export pipeline keep using the original asset.
#[tauri::command]
fn media_get_compatible_preview_video(
    app: AppHandle,
    state: State<'_, AppState>,
    asset_id: String,
) -> Result<PreviewVideoSourceStatus, String> {
    let mut engine = state.0.lock().map_err(|e| e.to_string())?;
    let asset = match engine
        .service
        .project()
        .assets
        .iter()
        .find(|asset| asset.asset_id == asset_id)
    {
        Some(asset) => asset.clone(),
        None => {
            return Ok(PreviewVideoSourceStatus::Unavailable {
                reason: format!("Asset '{}' not found in project", asset_id),
            });
        }
    };

    if asset.media_kind != MediaKind::Video {
        return Ok(PreviewVideoSourceStatus::Unavailable {
            reason: format!("Asset '{}' is not a video", asset_id),
        });
    }

    let source_path = PathBuf::from(&asset.path);
    if !source_path.exists() {
        return Ok(PreviewVideoSourceStatus::Unavailable {
            reason: format!("Media file not found at path '{}'", asset.path),
        });
    }
    if let Err(reason) = require_explicit_file_scope(
        &source_path,
        asset_scope_allows_path(&app.asset_protocol_scope(), &source_path),
        "Preview media access",
    ) {
        return Ok(PreviewVideoSourceStatus::Unavailable { reason });
    }

    let source_fingerprint = match FFmpegService::source_fingerprint(&source_path) {
        Ok(fingerprint) => fingerprint,
        Err(error) => {
            return Ok(PreviewVideoSourceStatus::Unavailable {
                reason: format!("Could not read source media metadata: {error}"),
            });
        }
    };
    let proxy_path = engine
        .cache
        .proxy_path(&asset_id, &source_fingerprint, PREVIEW_PROXY_HEIGHT);
    if engine
        .cache
        .has_proxy(&asset_id, &source_fingerprint, PREVIEW_PROXY_HEIGHT)
    {
        return Ok(ready_preview_proxy(&app, &proxy_path));
    }

    let dedupe_key = format!(
        "preview-proxy:{}:{}:{}",
        asset_id, source_fingerprint, PREVIEW_PROXY_HEIGHT
    );
    let job_id_candidate = format!(
        "preview_proxy_{}_{}",
        asset_id,
        uuid::Uuid::new_v4().simple()
    );
    let (job_id, _is_deduped) =
        engine
            .jobs
            .submit_job(job_id_candidate, JobKind::Proxy, dedupe_key, 10);

    if let Some(job) = engine.jobs.get_job(&job_id) {
        match &job.status {
            JobStatus::Completed => {
                if engine
                    .cache
                    .has_proxy(&asset_id, &source_fingerprint, PREVIEW_PROXY_HEIGHT)
                {
                    return Ok(ready_preview_proxy(&app, &proxy_path));
                }
                return Ok(PreviewVideoSourceStatus::Unavailable {
                    reason: "Compatible preview generation completed without a usable file"
                        .to_string(),
                });
            }
            JobStatus::Failed(reason) => {
                return Ok(PreviewVideoSourceStatus::Unavailable {
                    reason: reason.clone(),
                });
            }
            JobStatus::Cancelled => {
                return Ok(PreviewVideoSourceStatus::Unavailable {
                    reason: "Compatible preview generation was cancelled".to_string(),
                });
            }
            JobStatus::Running => return Ok(PreviewVideoSourceStatus::Pending),
            // A job queued behind the concurrency limit is started by a later poll;
            // requiring a non-deduplicated submit here left it Pending forever.
            JobStatus::Queued if engine.jobs.can_start() => {
                let cancel_flag = engine.jobs.start_job(&job_id).map_err(|e| e.to_string())?;
                let state_arc = Arc::clone(&state.0);
                let job_id = job_id.clone();
                let output_path = proxy_path.clone();

                std::thread::spawn(move || {
                    if cancel_flag.load(Ordering::SeqCst) {
                        if let Ok(mut engine) = state_arc.lock() {
                            engine.jobs.cancel_job(&job_id);
                        }
                        return;
                    }

                    let result = FFmpegService::generate_proxy(
                        &source_path,
                        &output_path,
                        PREVIEW_PROXY_HEIGHT,
                    );

                    if let Ok(mut engine) = state_arc.lock() {
                        if cancel_flag.load(Ordering::SeqCst) {
                            engine.jobs.cancel_job(&job_id);
                        } else {
                            match result {
                                Ok(()) => engine.jobs.mark_completed(&job_id),
                                Err(error) => engine.jobs.mark_failed(&job_id, error.to_string()),
                            }
                        }
                    }
                });
                return Ok(PreviewVideoSourceStatus::Pending);
            }
            JobStatus::Queued => return Ok(PreviewVideoSourceStatus::Pending),
        }
    }

    Ok(PreviewVideoSourceStatus::Pending)
}

#[tauri::command]
fn preview_run_benchmark(
    app: AppHandle,
    state: State<'_, AppState>,
    duration_secs: Option<u32>,
) -> Result<Vec<BenchmarkReport>, String> {
    let engine = state.0.lock().map_err(|e| e.to_string())?;
    let project = engine.service.project();
    require_project_media_scope(project, |path| {
        asset_scope_allows_path(&app.asset_protocol_scope(), path)
    })?;
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
fn render_text_template_poc_preview(
    text: String,
    progress: f32,
) -> Result<crate::modules::media::text_renderer_poc::PreviewOverlayIpcResponse, String> {
    Ok(crate::modules::media::text_renderer_poc::invoke_poc_preview_ipc(&text, progress))
}

#[tauri::command]
fn media_get_waveform(
    app: AppHandle,
    state: State<'_, AppState>,
    asset_id: String,
) -> Result<WaveformStatus, String> {
    let mut engine = state.0.lock().map_err(|e| e.to_string())?;

    // Resolve the current source before consulting the derived cache so relinked or
    // replaced media cannot reuse waveform data generated for an older source.
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
    if let Err(reason) = require_explicit_file_scope(
        &source_path,
        asset_scope_allows_path(&app.asset_protocol_scope(), &source_path),
        "Waveform media access",
    ) {
        return Ok(WaveformStatus::Unavailable { reason });
    }

    let source_fingerprint = FFmpegService::source_fingerprint(&source_path)
        .map_err(|error| format!("Could not fingerprint media for waveform cache: {error}"))?;
    if let Some(data) = engine.cache.load_waveform(&asset_id, &source_fingerprint) {
        return Ok(WaveformStatus::Ready(data));
    }

    // 3. Route through JobManager with deduplication and bounded concurrency
    let dedupe_key = format!("waveform:{}:{}", asset_id, source_fingerprint);
    let job_id_candidate = format!("wf_{}_{}", asset_id, uuid::Uuid::new_v4().simple());
    let (job_id, is_deduped) =
        engine
            .jobs
            .submit_job(job_id_candidate, JobKind::Waveform, dedupe_key, 10);

    if let Some(job) = engine.jobs.get_job(&job_id) {
        match &job.status {
            JobStatus::Completed => {
                if let Some(data) = engine.cache.load_waveform(&asset_id, &source_fingerprint) {
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
                        let cache_out = engine.cache.waveform_path(&asset_id, &source_fingerprint);
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

#[tauri::command]
fn media_cache_status(
    state: State<'_, AppState>,
) -> Result<crate::modules::media::cache::MediaCacheStatus, String> {
    let engine = state.0.lock().map_err(|error| error.to_string())?;
    engine
        .cache
        .status()
        .map_err(|error| format!("Could not read media cache status: {error}"))
}

#[tauri::command]
fn media_cache_clear(
    state: State<'_, AppState>,
) -> Result<crate::modules::media::cache::MediaCacheStatus, String> {
    let engine = state.0.lock().map_err(|error| error.to_string())?;
    let active_jobs = engine.jobs.active_jobs_count();
    if active_jobs > 0 {
        return Err(format!("Wait for or cancel the {active_jobs} active media/export job(s) before clearing the derived cache."));
    }
    engine
        .cache
        .clear_cache()
        .map_err(|error| format!("Could not clear derived media cache: {error}"))?;
    engine
        .cache
        .status()
        .map_err(|error| format!("Cache was cleared, but status could not be read: {error}"))
}

#[tauri::command]
async fn media_analyze_audio_levels(
    app: AppHandle,
    state: State<'_, AppState>,
    asset_id: String,
) -> Result<crate::modules::media::ffmpeg::AudioLevelReport, String> {
    let source_path = {
        let engine = state.0.lock().map_err(|error| error.to_string())?;
        let asset = engine
            .service
            .project()
            .assets
            .iter()
            .find(|asset| asset.asset_id == asset_id)
            .ok_or_else(|| format!("Asset '{asset_id}' not found in project"))?;
        PathBuf::from(&asset.path)
    };
    if !source_path.exists() {
        return Err(format!(
            "Media file not found at path '{}'",
            source_path.display()
        ));
    }
    require_explicit_file_scope(
        &source_path,
        asset_scope_allows_path(&app.asset_protocol_scope(), &source_path),
        "Audio level analysis media access",
    )?;
    tokio::task::spawn_blocking(move || FFmpegService::analyze_audio_levels(&source_path))
        .await
        .map_err(|error| format!("Audio analysis worker failed: {error}"))?
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn template_list_definitions(
) -> Result<Vec<crate::modules::domain::template::TemplateDefinition>, String> {
    let catalog = crate::modules::templates::catalog::TemplateCatalog::default_catalog();
    Ok(catalog.list_all())
}

#[tauri::command]
fn template_get_definition(
    id: String,
    version: Option<u32>,
) -> Result<Option<crate::modules::domain::template::TemplateDefinition>, String> {
    let catalog = crate::modules::templates::catalog::TemplateCatalog::default_catalog();
    if let Some(v) = version {
        Ok(catalog.get(&id, v))
    } else {
        Ok(catalog
            .list_all()
            .into_iter()
            .filter(|d| d.id == id)
            .max_by_key(|d| d.version))
    }
}

#[tauri::command]
fn template_save_user_definition(
    definition: crate::modules::domain::template::TemplateDefinition,
) -> Result<String, String> {
    let catalog = crate::modules::templates::catalog::TemplateCatalog::default_catalog();
    catalog
        .save_user_definition(&definition)
        .map_err(|e| e.to_string())?;
    Ok(format!("{}_v{}", definition.id, definition.version))
}

#[tauri::command]
fn template_delete_user_definition(id: String, version: u32) -> Result<bool, String> {
    let catalog = crate::modules::templates::catalog::TemplateCatalog::default_catalog();
    catalog
        .delete_user_definition(&id, version)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn template_import_pack(
    app: AppHandle,
    pack_path: String,
) -> Result<crate::modules::templates::pack::TemplatePackManifest, String> {
    let path = Path::new(&pack_path);
    require_explicit_file_scope(
        path,
        asset_scope_allows_path(&app.asset_protocol_scope(), path),
        "Template pack import",
    )?;
    let catalog = crate::modules::templates::catalog::TemplateCatalog::default_catalog();
    crate::modules::templates::pack::TemplatePackService::import_pack(
        Path::new(&pack_path),
        &catalog,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn template_export_pack(
    app: AppHandle,
    manifest: crate::modules::templates::pack::TemplatePackManifest,
    definitions: Vec<crate::modules::domain::template::TemplateDefinition>,
    asset_files: Vec<(String, String)>,
    output_path: String,
) -> Result<(), String> {
    let mut resource_map = std::collections::HashMap::new();
    for (res_id, file_path) in asset_files {
        let path = Path::new(&file_path);
        require_explicit_file_scope(
            path,
            asset_scope_allows_path(&app.asset_protocol_scope(), path),
            "Template pack asset read",
        )?;
        let bytes = fs::read(&file_path)
            .map_err(|e| format!("Failed to read asset '{}': {e}", file_path))?;
        resource_map.insert(res_id, bytes);
    }
    require_explicit_file_scope(
        Path::new(&output_path),
        asset_scope_allows_path(&app.asset_protocol_scope(), Path::new(&output_path)),
        "Template pack export",
    )?;
    crate::modules::templates::pack::TemplatePackService::export_pack(
        &manifest,
        &definitions,
        &resource_map,
        Path::new(&output_path),
    )
    .map_err(|e| e.to_string())
}

/// SHA-256 for a local source asset used by template pack manifests.
#[tauri::command]
fn media_file_sha256(app: AppHandle, path: String) -> Result<String, String> {
    require_explicit_file_scope(
        Path::new(&path),
        asset_scope_allows_path(&app.asset_protocol_scope(), Path::new(&path)),
        "Media file hash",
    )?;
    let bytes = fs::read(&path).map_err(|e| format!("Failed to read '{path}': {e}"))?;
    Ok(crate::modules::media::text_renderer_poc::compute_sha256_hex(&bytes))
}

#[tauri::command]
fn capcut_create_inspection_token() -> Result<String, String> {
    let (token, _) =
        crate::modules::templates::capcut_import::create_inspection_cancellation_token();
    Ok(token)
}

#[tauri::command]
fn capcut_cancel_inspection(token: String) -> Result<bool, String> {
    Ok(crate::modules::templates::capcut_import::cancel_inspection(
        &token,
    ))
}

#[tauri::command]
fn capcut_inspect_template_draft(
    app: AppHandle,
    draft_path: String,
    selected_roots: Option<Vec<String>>,
    cancel_token: Option<String>,
) -> Result<crate::modules::templates::capcut_import::CapCutInspectionResult, String> {
    let draft = Path::new(&draft_path);
    require_explicit_file_scope(
        draft,
        asset_scope_allows_path(&app.asset_protocol_scope(), draft),
        "Template inspection",
    )?;
    let roots: Vec<std::path::PathBuf> = selected_roots
        .unwrap_or_default()
        .into_iter()
        .map(std::path::PathBuf::from)
        .collect();
    for root in &roots {
        require_explicit_file_scope(
            root,
            asset_scope_allows_path(&app.asset_protocol_scope(), root),
            "Template extraction root",
        )?;
    }

    let cancel_flag = cancel_token
        .as_deref()
        .and_then(crate::modules::templates::capcut_import::get_inspection_cancellation_token);

    let res = crate::modules::templates::capcut_import::inspect_draft(
        std::path::Path::new(&draft_path),
        &roots,
        cancel_flag.as_deref(),
    );

    if let Some(ref tok) = cancel_token {
        crate::modules::templates::capcut_import::remove_inspection_cancellation_token(tok);
    }

    res.map_err(|e| e.to_string())
}

#[tauri::command]
fn capcut_save_template_to_library(
    app: AppHandle,
    report: crate::modules::templates::capcut_import::CapCutConversionReport,
    asset_source_files: Vec<(String, String)>,
    expected_source_hash: Option<String>,
) -> Result<String, String> {
    require_explicit_file_scope(
        Path::new(&report.source_path),
        asset_scope_allows_path(&app.asset_protocol_scope(), Path::new(&report.source_path)),
        "Template source read",
    )?;
    for (_, path) in &asset_source_files {
        let path = Path::new(path);
        require_explicit_file_scope(
            path,
            asset_scope_allows_path(&app.asset_protocol_scope(), path),
            "Template asset read",
        )?;
    }
    let catalog = crate::modules::templates::catalog::TemplateCatalog::default_catalog();
    let assets: Vec<(String, std::path::PathBuf)> = asset_source_files
        .into_iter()
        .map(|(id, p)| (id, std::path::PathBuf::from(p)))
        .collect();
    let expected = expected_source_hash
        .as_deref()
        .map(|h| (std::path::Path::new(&report.source_path), h));

    crate::modules::templates::capcut_import::save_converted_to_catalog(
        &catalog, &report, &assets, expected,
    )
    .map_err(|e| e.to_string())
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
fn capcut_apply_template_to_project(
    state: tauri::State<'_, AppState>,
    staging_token: String,
    item_id: String,
    template_id: String,
    template_version: u32,
    text_variant: Option<String>,
    expected_revision: u64,
    source_draft_path: String,
    expected_source_hash: String,
    expected_resource_fingerprints: std::collections::HashMap<String, String>,
    cancel_token: Option<String>,
) -> Result<crate::modules::command::CommandReceipt, String> {
    let cancel_flag = cancel_token.as_ref().and_then(|tok| {
        crate::modules::templates::capcut_import::get_inspection_cancellation_token(tok)
    });

    let res_fps_map: std::collections::HashMap<std::path::PathBuf, String> =
        expected_resource_fingerprints
            .into_iter()
            .map(|(k, v)| (std::path::PathBuf::from(k), v))
            .collect();

    let draft_p = std::path::PathBuf::from(&source_draft_path);

    let req = crate::modules::templates::capcut_import::ApplyTemplateGuardrailsRequest {
        staging_token: &staging_token,
        item_id: &item_id,
        template_id: &template_id,
        template_version,
        text_variant: text_variant.as_deref(),
        expected_revision,
        expected_source_hash: &expected_source_hash,
        source_draft_path: &draft_p,
        expected_resource_fingerprints: &res_fps_map,
        cancel: cancel_flag.as_deref(),
    };

    let mut engine_guard = state.0.lock().map_err(|e| e.to_string())?;
    let engine = &mut *engine_guard;
    crate::modules::templates::capcut_import::execute_staged_template_apply(
        &mut engine.service,
        Some(&mut engine.history),
        req,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn template_draft_capabilities() -> crate::modules::templates::agent::TemplateCapabilitiesReport {
    crate::modules::templates::agent::AgentTemplateService::capabilities()
}

#[tauri::command]
fn template_draft_validate(
    draft: crate::modules::templates::agent::TemplateDraft,
) -> crate::modules::templates::agent::TemplateValidationReport {
    let catalog = crate::modules::templates::catalog::TemplateCatalog::default_catalog();
    crate::modules::templates::agent::AgentTemplateService::validate_draft(
        &draft,
        None,
        Some(&catalog),
    )
}

#[tauri::command]
fn template_draft_preview(
    draft: crate::modules::templates::agent::TemplateDraft,
    time_seconds: Option<f64>,
    output_path: Option<String>,
) -> Result<crate::modules::templates::agent::TemplatePreviewReceipt, String> {
    let t = time_seconds.unwrap_or(1.0);
    let out = output_path.map(PathBuf::from).unwrap_or_else(|| {
        std::env::temp_dir().join(format!(
            "tinixcut_preview_{}.png",
            uuid::Uuid::new_v4().simple()
        ))
    });
    crate::modules::templates::agent::AgentTemplateService::preview_draft_to_png(
        &draft, &out, t, None, None,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn template_draft_save(
    draft: crate::modules::templates::agent::TemplateDraft,
    overwrite: Option<bool>,
) -> Result<crate::modules::templates::agent::TemplateSaveReceipt, String> {
    let catalog = crate::modules::templates::catalog::TemplateCatalog::default_catalog();
    crate::modules::templates::agent::AgentTemplateService::save_draft_to_catalog(
        &draft,
        &catalog,
        overwrite.unwrap_or(false),
    )
    .map_err(|e| e.to_string())
}

pub fn template_draft_apply_engine(
    engine: &mut EditorEngine,
    item_id: &str,
    draft: &crate::modules::templates::agent::TemplateDraft,
    base_revision: Option<u64>,
) -> Result<crate::modules::templates::agent::TemplateApplyReceipt, String> {
    let catalog = crate::modules::templates::catalog::TemplateCatalog::default_catalog();

    // 1. Validate draft before mutating project or catalog
    let report = crate::modules::templates::agent::AgentTemplateService::validate_draft(
        draft,
        None,
        Some(&catalog),
    );
    if !report.valid {
        let first_err = report
            .issues
            .iter()
            .find(|i| i.severity == crate::modules::templates::agent::ValidationSeverity::Error)
            .map(|i| i.message.clone())
            .unwrap_or_else(|| "Validation error".to_string());
        return Err(format!("Validation failed: {first_err}"));
    }

    // 2. Validate target item exists and is text in the active project
    let current_project = engine.service.project();
    let mut target_found = false;
    for track in &current_project.tracks {
        if let Some(item) = track.items.iter().find(|i| i.item_id == item_id) {
            target_found = true;
            if item.kind != ItemKind::Text {
                return Err(format!(
                    "Item '{item_id}' is not a text item and cannot receive a template"
                ));
            }
            break;
        }
    }
    if !target_found {
        return Err(format!("Item '{item_id}' not found in project"));
    }

    // 3. Staged base revision check: reject candidate if base_revision is stale
    if let Some(rev) = base_revision {
        if rev != current_project.revision {
            return Err(format!(
                "Stale revision: expected {}, received {}",
                current_project.revision, rev
            ));
        }
    }

    // 4. Check catalog conflict beforehand WITHOUT irreversible publication
    let def = draft.to_template_definition();
    catalog.ensure_dirs().map_err(|e| e.to_string())?;
    let existed_in_catalog = catalog.get(&def.id, def.version);
    if let Some(ref existing) = existed_in_catalog {
        if existing != &def {
            return Err(format!(
                "Immutable version conflict: template '{}' v{} already exists with different definition",
                def.id, def.version
            ));
        }
    }
    let def_existed_before = existed_in_catalog.is_some();

    // Stage in memory during transaction so CommandOperation::TemplateApply can resolve definition
    // without prematurely writing to catalog directory on disk
    crate::modules::templates::catalog::TemplateCatalog::stage_definition(def.clone());
    struct StagedGuard {
        id: String,
        version: u32,
    }
    impl Drop for StagedGuard {
        fn drop(&mut self) {
            crate::modules::templates::catalog::TemplateCatalog::unstage_definition(
                &self.id,
                self.version,
            );
        }
    }
    let _staged_guard = StagedGuard {
        id: def.id.clone(),
        version: def.version,
    };

    // 5. Build envelope routed through GUI client registry
    let cid = "gui_agent".to_string();
    let cepoch = engine.service.project().client_registry.current_epoch;
    let seq = engine
        .service
        .project()
        .client_registry
        .clients
        .get(&cid)
        .map(|c| c.highest_sequence + 1)
        .unwrap_or(1);
    let tx_id = format!("tx_{}", uuid::Uuid::new_v4().simple());
    let base_tx = if tx_id.len() > 40 {
        &tx_id[..40]
    } else {
        &tx_id
    };

    let operations = crate::modules::templates::agent::AgentTemplateService::build_apply_operations(
        draft, item_id, base_tx,
    );

    let envelope = CommandEnvelope {
        schema_version: crate::modules::domain::CURRENT_SCHEMA_VERSION,
        project_id: engine.service.project().project_id.clone(),
        base_revision: base_revision.unwrap_or(engine.service.project().revision),
        client_id: Some(cid),
        client_epoch: Some(cepoch),
        sequence_number: Some(seq),
        dry_run: false,
        transaction_id: Some(tx_id.clone()),
        operations,
    };

    let prior_snapshot = engine.service.project().clone();
    let receipt = engine
        .service
        .execute(envelope)
        .map_err(|e| e.to_string())?;

    // 6. Propagate persistence failure atomically and rollback memory state
    if let Some(path) = &engine.current_file_path {
        let expected_revision = engine.persisted_revision;
        if let Err(e) =
            save_project_atomic_if_revision(engine.service.project(), path, expected_revision)
        {
            engine.service.set_project(prior_snapshot);
            return Err(format!("Persistence failure: {e}"));
        }
        engine.persisted_revision = Some(engine.service.project().revision);
    }

    // Publish to catalog on disk ONLY after command transaction and persistence succeed
    if !def_existed_before {
        if let Err(e) = catalog.save_user_definition(&def) {
            if let Some(path) = &engine.current_file_path {
                let committed_revision = engine.service.project().revision;
                if save_project_atomic_if_revision(&prior_snapshot, path, Some(committed_revision))
                    .is_ok()
                {
                    engine.persisted_revision = Some(prior_snapshot.revision);
                }
            }
            engine.service.set_project(prior_snapshot);
            return Err(format!("Catalog publish failure: {e}"));
        }
    }

    if receipt.operations_applied > 0 {
        let resulting_snapshot = engine.service.project().clone();
        engine.history.record_edit(HistoryEntry {
            description: format!(
                "Apply template '{}' v{}",
                draft.template_id, draft.template_version
            ),
            transaction_id: Some(tx_id),
            prior_snapshot,
            resulting_snapshot,
        });
    }

    Ok(crate::modules::templates::agent::TemplateApplyReceipt {
        receipt: receipt.clone(),
        applied_draft_id: draft.draft_id.clone(),
        template_id: draft.template_id.clone(),
        template_version: draft.template_version,
        revision: receipt.new_revision,
    })
}

#[tauri::command]
fn template_draft_apply(
    state: State<'_, AppState>,
    item_id: String,
    draft: crate::modules::templates::agent::TemplateDraft,
    base_revision: Option<u64>,
) -> Result<crate::modules::templates::agent::TemplateApplyReceipt, String> {
    let mut engine_guard = state.0.lock().map_err(|e| e.to_string())?;
    template_draft_apply_engine(&mut engine_guard, &item_id, &draft, base_revision)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    configure_webview2_profile();
    let initial_project = Project::new("Untitled Project".to_string());
    let state = AppState(
        Arc::new(Mutex::new(EditorEngine::new(initial_project))),
        Arc::new(Mutex::new(std::collections::HashMap::new())),
        Arc::new(Mutex::new(JobManager::new(1))),
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_persisted_scope::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            lut_describe,
            lut_read_table,
            frame_scopes,
            project_chapters,
            app_health_check,
            app_settings_get,
            app_settings_save,
            app_settings_add_recent_project,
            project_new,
            project_get_current,
            project_get_current_session,
            edit_plan_draft_preview,
            edit_plan_scene_draft_preview,
            edit_plan_scene_selection_suggest,
            edit_plan_reorder_preview,
            edit_plan_trim_preview,
            edit_plan_draft_cancel,
            project_execute_command,
            project_dry_run,
            project_save,
            project_open,
            project_variant_create,
            project_collect_media,
            project_compare_current_with_path,
            project_check_recovery,
            project_save_recovery_snapshot,
            project_restore_recovery,
            project_save_unsaved_recovery,
            project_list_unsaved_recoveries,
            project_restore_unsaved_recovery,
            project_discard_unsaved_recovery,
            project_undo,
            project_redo,
            project_export,
            project_export_preflight,
            project_export_start,
            project_export_status,
            project_export_cancel,
            media_import,
            media_list_offline_assets,
            captions_import,
            captions_export,
            captions_auto_start,
            captions_auto_status,
            captions_auto_cancel,
            visual_scene_analysis_start,
            visual_scene_analysis_status,
            visual_scene_analysis_cancel,
            media_get_waveform,
            media_cache_status,
            media_cache_clear,
            media_analyze_audio_levels,
            media_get_compatible_preview_video,
            preview_sample_at,
            preview_rich_text_overlay,
            preview_run_benchmark,
            parity_run_suite,
            render_text_template_poc_preview,
            template_list_definitions,
            template_get_definition,
            template_save_user_definition,
            template_delete_user_definition,
            template_import_pack,
            template_export_pack,
            media_file_sha256,
            capcut_create_inspection_token,
            capcut_cancel_inspection,
            capcut_inspect_template_draft,
            capcut_save_template_to_library,
            capcut_apply_template_to_project,
            template_draft_capabilities,
            template_draft_validate,
            template_draft_preview,
            template_draft_save,
            template_draft_apply
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(target_os = "windows")]
fn configure_webview2_profile() {
    if std::env::var_os("WEBVIEW2_USER_DATA_FOLDER").is_some() {
        return;
    }

    let local_app_data = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .expect("LOCALAPPDATA is required to configure the TinixCut WebView2 profile");
    let profile_root = local_app_data
        .join("com.tinixcut.editor")
        .join("webview-profile-v2");
    fs::create_dir_all(&profile_root)
        .expect("could not create the dedicated TinixCut WebView2 profile directory");
    std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", profile_root);
}

#[cfg(not(target_os = "windows"))]
fn configure_webview2_profile() {}

#[cfg(test)]
mod tests {
    use super::{
        canonicalize_scope_candidate, cleanup_current_unsaved_recovery_after_save,
        get_health_status, inspect_media_for_import, install_saved_recovery_project,
        list_unsaved_recovery_candidates, media_duration_from_seconds, offline_media_asset,
        prepare_recovery_snapshot, project_get_current_session_from_engine,
        recovery_snapshot_fingerprint, redo_editor_engine, require_explicit_file_scope,
        require_project_media_scope, restore_unsaved_recovery_engine,
        save_unsaved_recovery_snapshot, undo_editor_engine, unsaved_recovery_path,
        verify_recovery_fingerprint, EditorEngine,
    };
    use crate::modules::domain::time::{FrameRate, RationalTime, TimeRange};
    use crate::modules::domain::{Asset, ItemKind, Project, TimelineItem, Track, TrackKind};
    use crate::modules::domain::{AssetDimensions, MediaKind};
    use crate::modules::history::HistoryEntry;
    use crate::modules::media::ffmpeg::{FFmpegService, MediaMetadata};
    use crate::modules::persistence::{
        load_project, remove_project_if_revision, save_project_atomic,
    };
    use std::fs;
    use std::path::Path;

    fn probed_media(video: Option<&str>, audio: Option<&str>) -> MediaMetadata {
        MediaMetadata {
            duration_seconds: 2.0,
            width: 1280,
            height: 720,
            frame_rate: FrameRate::FPS_30,
            video_codec: video.map(str::to_string),
            audio_codec: audio.map(str::to_string),
            sample_rate: Some(48_000),
            channels: Some(2),
            streams: Vec::new(),
        }
    }

    #[test]
    fn test_app_health_status() {
        let health = get_health_status();
        assert_eq!(health.status, "healthy");
        assert_eq!(health.schema_version, 1);
        assert!(!health.version.is_empty());
        assert!(!health.platform.is_empty());
    }

    #[test]
    fn media_scope_denial_is_distinct_from_a_missing_media_file() {
        let root = std::env::temp_dir().join(format!(
            "tinixcut_asset_scope_test_{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("fixture.mp4");
        fs::write(&path, b"test media").unwrap();
        let asset = Asset {
            asset_id: "asset-scope-test".to_string(),
            name: "fixture.mp4".to_string(),
            path: path.to_string_lossy().to_string(),
            media_kind: MediaKind::Video,
            duration: None,
            file_size: 10,
            dimensions: None,
        };

        assert!(offline_media_asset(&asset, true).is_none());
        let denied = offline_media_asset(&asset, false).unwrap();
        assert!(denied.permission_required);
        assert_eq!(denied.last_known_path, asset.path);

        fs::remove_file(&path).unwrap();
        let missing = offline_media_asset(&asset, true).unwrap();
        assert!(!missing.permission_required);
        assert_eq!(missing.last_known_path, asset.path);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn native_file_reads_require_an_explicit_scope_grant() {
        let path = Path::new("selected-media.mp4");
        let denied = require_explicit_file_scope(path, false, "Export").unwrap_err();
        assert!(denied.contains("explicit file access"));
        assert!(denied.contains("selected-media.mp4"));
        require_explicit_file_scope(path, true, "Export").unwrap();
    }

    #[test]
    fn missing_scope_target_resolves_through_existing_junction_parent() {
        use std::os::windows::fs::symlink_dir;

        let root = std::env::temp_dir().join(format!(
            "tinixcut_scope_junction_test_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let granted = root.join("granted");
        let outside = root.join("outside");
        fs::create_dir_all(&granted).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let junction = granted.join("redirect");
        symlink_dir(&outside, &junction)
            .expect("test requires creating a temporary directory symlink");

        let missing_target = junction.join("new-export.mp4");
        let resolved = canonicalize_scope_candidate(&missing_target).unwrap();
        let expected = fs::canonicalize(&outside).unwrap().join("new-export.mp4");
        assert_eq!(resolved, expected);

        fs::remove_file(&junction.join("new-export.mp4")).ok();
        fs::remove_dir(&junction).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn export_scope_checks_only_existing_media_referenced_by_timeline() {
        let root = std::env::temp_dir().join(format!(
            "tinixcut_export_scope_test_{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("referenced.mp4");
        fs::write(&path, b"fixture").unwrap();

        let asset_id = "asset-export-scope".to_string();
        let asset = Asset {
            asset_id: asset_id.clone(),
            name: "referenced.mp4".to_string(),
            path: path.to_string_lossy().to_string(),
            media_kind: MediaKind::Video,
            duration: None,
            file_size: 7,
            dimensions: None,
        };
        let rate = FrameRate::FPS_30;
        let range = TimeRange::new(RationalTime::new(0, rate), RationalTime::new(30, rate));
        let item = TimelineItem::new(
            Some(asset_id),
            "Referenced clip".to_string(),
            ItemKind::Video,
            range,
            range,
        )
        .unwrap();
        let mut track = Track::new("Video".to_string(), TrackKind::Video);
        track.items.push(item);
        let mut project = Project::new("Scope test".to_string());
        project.assets.push(asset);
        project.tracks.push(track);

        assert!(require_project_media_scope(&project, |_| false).is_err());
        require_project_media_scope(&project, |_| true).unwrap();
        assert!(require_project_media_scope(&Project::new("Empty".to_string()), |_| false).is_ok());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recovery_snapshot_revision_stays_newer_than_the_saved_file_after_undo() {
        let mut project = Project::new("Recovery revision test".to_string());
        project.revision = 2;

        let snapshot = prepare_recovery_snapshot(&project, 4);

        assert_eq!(snapshot.revision, 5);
        assert_eq!(project.revision, 2);
        assert_eq!(snapshot.project_id, project.project_id);
    }

    #[test]
    fn unsaved_recovery_round_trips_and_rejects_stale_overwrite_or_discard() {
        let root = std::env::temp_dir().join(format!(
            "tinixcut_unsaved_recovery_test_{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&root).unwrap();
        let mut project = Project::new("Unsaved draft".to_string());
        project.revision = 3;
        let path = unsaved_recovery_path(&root, &project.project_id).unwrap();

        save_unsaved_recovery_snapshot(&project, &path, 3).unwrap();
        assert_eq!(load_project(&path).unwrap(), project);
        let listed = list_unsaved_recovery_candidates(&root).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].project_id, project.project_id);
        assert_eq!(listed[0].name, project.name);
        assert_eq!(listed[0].revision, 3);
        assert_eq!(listed[0].asset_count, 0);
        assert_eq!(listed[0].track_count, 0);
        assert_eq!(listed[0].clip_count, 0);
        assert!(listed[0].preview_item_names.is_empty());

        let mut divergent = project.clone();
        divergent.name = "Different branch at same revision".to_string();
        assert!(save_unsaved_recovery_snapshot(&divergent, &path, 3).is_err());
        assert_eq!(load_project(&path).unwrap(), project);

        let mut stale = project.clone();
        stale.revision = 2;
        assert!(save_unsaved_recovery_snapshot(&stale, &path, 2).is_err());
        assert_eq!(load_project(&path).unwrap(), project);
        assert!(remove_project_if_revision(&path, &project.project_id, 2).is_err());
        assert!(path.exists());
        remove_project_if_revision(&path, &project.project_id, 3).unwrap();
        assert!(!path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recovery_candidate_summary_shows_bounded_labels_without_file_paths() {
        let mut project = Project::new("Untitled Project".to_string());
        let rate = FrameRate::FPS_30;
        let duration = RationalTime::new(30, rate);
        let source_range = TimeRange::new(RationalTime::zero(rate), duration);
        let mut track = Track::new("Video 1".to_string(), TrackKind::Video);
        for (index, name) in [
            "C:\\private\\phase clip one.mp4",
            "clip two.mp4",
            "clip three.mp4",
            "clip four.mp4",
        ]
        .into_iter()
        .enumerate()
        {
            let timeline_range = TimeRange::new(
                RationalTime::new(index as i64 * duration.value, rate),
                duration,
            );
            track.items.push(
                TimelineItem::new(
                    None,
                    name.to_string(),
                    ItemKind::Video,
                    timeline_range,
                    source_range,
                )
                .unwrap(),
            );
        }
        project.tracks.push(track);
        project.assets.push(Asset {
            asset_id: "asset-summary".to_string(),
            name: "unused-source.mp4".to_string(),
            path: "C:\\private\\unused-source.mp4".to_string(),
            media_kind: MediaKind::Video,
            duration: None,
            file_size: 0,
            dimensions: None,
        });

        let root = std::env::temp_dir().join(format!(
            "tinixcut_recovery_summary_test_{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = unsaved_recovery_path(&root, &project.project_id).unwrap();
        save_unsaved_recovery_snapshot(&project, &path, project.revision).unwrap();
        let listed = list_unsaved_recovery_candidates(&root).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].asset_count, 1);
        assert_eq!(listed[0].track_count, 1);
        assert_eq!(listed[0].clip_count, 4);
        let labels = &listed[0].preview_item_names;
        assert_eq!(labels.len(), 3);
        assert_eq!(labels[0], "phase clip one.mp4");
        assert_eq!(labels[1], "clip two.mp4");
        assert_eq!(labels[2], "clip three.mp4");
        assert!(labels.iter().all(|label| !label.contains('\\')));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn undo_redo_recovery_snapshots_keep_monotonic_revisions() {
        let root = std::env::temp_dir().join(format!(
            "tinixcut_undo_recovery_revision_test_{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&root).unwrap();

        let original = Project::new("Original draft".to_string());
        let mut applied = original.clone();
        applied.name = "Applied draft".to_string();
        applied.revision = 1;
        applied.client_registry.current_epoch = 3;
        applied.client_registry.min_accepted_epoch = 2;
        applied.client_registry.clients.insert(
            "automation-client".to_string(),
            crate::modules::domain::ClientRecord {
                client_epoch: 3,
                highest_sequence: 7,
                last_active_revision: 1,
                registered_at_revision: 0,
            },
        );
        applied
            .command_ledger
            .push(crate::modules::domain::CommandLedgerEntry {
                operation_id: "operation-already-applied".to_string(),
                transaction_id: Some("transaction-1".to_string()),
                client_id: Some("automation-client".to_string()),
                client_epoch: Some(3),
                sequence_number: Some(7),
                fingerprint: "fp_sha256_phase48_test".to_string(),
                status: "applied".to_string(),
                applied_at_revision: 1,
                affected_item_id: None,
                affected_track_id: None,
            });

        let mut engine = EditorEngine::new(original.clone());
        engine.history.record_edit(HistoryEntry {
            description: "Rename draft".to_string(),
            transaction_id: Some("phase47-apply".to_string()),
            prior_snapshot: original,
            resulting_snapshot: applied.clone(),
        });
        engine.service.set_project(applied.clone());

        let path = unsaved_recovery_path(&root, &applied.project_id).unwrap();
        save_unsaved_recovery_snapshot(&applied, &path, 1).unwrap();

        assert_eq!(
            undo_editor_engine(&mut engine).as_deref(),
            Some("Rename draft")
        );
        let undone = engine.service.project().clone();
        assert_eq!(undone.name, "Original draft");
        assert_eq!(undone.revision, 2);
        assert_eq!(undone.client_registry, applied.client_registry);
        assert_eq!(undone.command_ledger, applied.command_ledger);
        assert!(engine
            .service
            .is_operation_applied("operation-already-applied"));
        save_unsaved_recovery_snapshot(&undone, &path, 2).unwrap();

        assert_eq!(
            redo_editor_engine(&mut engine).as_deref(),
            Some("Rename draft")
        );
        let redone = engine.service.project().clone();
        let mut expected_redone = applied.clone();
        expected_redone.revision = 3;
        assert_eq!(redone, expected_redone);
        save_unsaved_recovery_snapshot(&redone, &path, 3).unwrap();

        let stale_writer = undone;
        let error = save_unsaved_recovery_snapshot(&stale_writer, &path, 2).unwrap_err();
        assert!(error.contains("A newer recovery revision 3 already exists"));
        assert_eq!(load_project(&path).unwrap(), redone);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unsaved_recovery_path_rejects_path_traversal_project_ids() {
        let root = std::env::temp_dir();
        assert!(unsaved_recovery_path(&root, "../outside").is_err());
        assert!(unsaved_recovery_path(&root, "").is_err());
    }

    #[test]
    fn unsaved_recovery_restore_rehydrates_project_and_refuses_to_replace_edits() {
        let root = std::env::temp_dir().join(format!(
            "tinixcut_unsaved_restore_test_{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&root).unwrap();
        let mut recovered = Project::new("Recoverable unsaved edit".to_string());
        recovered.revision = 4;
        let recovery_path = unsaved_recovery_path(&root, &recovered.project_id).unwrap();
        save_unsaved_recovery_snapshot(&recovered, &recovery_path, 4).unwrap();

        let mut engine = EditorEngine::new(Project::new("Untitled".to_string()));
        let current_id = engine.service.project().project_id.clone();
        let restored = restore_unsaved_recovery_engine(
            &mut engine,
            &root,
            &recovered.project_id,
            4,
            &current_id,
            0,
        )
        .unwrap();
        assert_eq!(restored, recovered);
        assert_eq!(engine.service.project(), &recovered);
        assert!(engine.current_file_path.is_none());
        assert!(engine.persisted_revision.is_none());
        assert_eq!(
            engine.current_unsaved_recovery_path.as_deref(),
            Some(recovery_path.as_path())
        );

        let mut dirty_engine = EditorEngine::new(Project::new("Untitled".to_string()));
        let dirty_id = dirty_engine.service.project().project_id.clone();
        let mut dirty_project = dirty_engine.service.project().clone();
        dirty_project.revision = 1;
        dirty_engine.service.set_project(dirty_project.clone());
        let error = restore_unsaved_recovery_engine(
            &mut dirty_engine,
            &root,
            &recovered.project_id,
            4,
            &dirty_id,
            1,
        )
        .unwrap_err();
        assert!(error.contains("refusing to replace it"));
        assert_eq!(dirty_engine.service.project(), &dirty_project);

        let mut stale_engine = EditorEngine::new(Project::new("Untitled".to_string()));
        let stale_id = stale_engine.service.project().project_id.clone();
        assert!(restore_unsaved_recovery_engine(
            &mut stale_engine,
            &root,
            &recovered.project_id,
            3,
            &stale_id,
            0,
        )
        .unwrap_err()
        .contains("snapshot changed"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn engine_current_session_receipt_reflects_persisted_and_unsaved_state() {
        let engine = EditorEngine::new(Project::new("Untitled".to_string()));
        let unsaved_session = project_get_current_session_from_engine(&engine).unwrap();
        assert_eq!(unsaved_session.project.name, "Untitled");
        assert_eq!(unsaved_session.project.revision, 0);
        assert!(unsaved_session.canonical_path.is_none());
        assert!(unsaved_session.persisted_revision.is_none());

        let mut saved_engine = EditorEngine::new(Project::new("Saved".to_string()));
        let saved_path = std::path::PathBuf::from("projects").join("sample.tinix");
        let mut saved_project = Project::new("Saved".to_string());
        saved_project.revision = 5;
        install_saved_recovery_project(
            &mut saved_engine,
            saved_project.clone(),
            saved_path.clone(),
            4,
        );

        let saved_session = project_get_current_session_from_engine(&saved_engine).unwrap();
        assert_eq!(saved_session.project.name, "Saved");
        assert_eq!(saved_session.project.revision, 5);
        assert_eq!(
            saved_session.canonical_path.as_deref(),
            Some(saved_path.to_string_lossy().as_ref())
        );
        assert_eq!(saved_session.persisted_revision, Some(4));
    }

    #[test]
    fn restoring_saved_project_does_not_delete_previous_projects_unsaved_recovery() {
        let root = std::env::temp_dir().join(format!(
            "tinixcut_saved_restore_preserves_unsaved_{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&root).unwrap();
        let project_a = Project::new("Unsaved project A".to_string());
        let recovery_a = root.join("project-a.unsaved.tinix");
        save_unsaved_recovery_snapshot(&project_a, &recovery_a, project_a.revision).unwrap();

        let mut engine = EditorEngine::new(Project::new("Temporary project".to_string()));
        engine.current_unsaved_recovery_path = Some(recovery_a.clone());
        let project_b = Project::new("Saved project B".to_string());
        let saved_b = root.join("project-b.tinix");
        save_project_atomic(&project_b, &saved_b).unwrap();
        install_saved_recovery_project(&mut engine, project_b.clone(), saved_b.clone(), 0);

        assert!(engine.current_unsaved_recovery_path.is_none());
        let mut edited_b = project_b.clone();
        edited_b.revision = 1;
        edited_b.name = "Edited project B".to_string();
        save_project_atomic(&edited_b, &saved_b).unwrap();
        cleanup_current_unsaved_recovery_after_save(&mut engine).unwrap();

        assert_eq!(load_project(&recovery_a).unwrap(), project_a);
        assert_eq!(load_project(&saved_b).unwrap(), edited_b);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recovery_prompt_fingerprint_rejects_same_revision_content_replacement() {
        let original = Project::new("Recovery candidate".to_string());
        let fingerprint = recovery_snapshot_fingerprint(&original).unwrap();
        verify_recovery_fingerprint(&original, &fingerprint).unwrap();

        let mut replacement = original.clone();
        replacement.name = "Replacement at same revision".to_string();
        assert!(verify_recovery_fingerprint(&replacement, &fingerprint)
            .unwrap_err()
            .contains("content changed"));
    }

    #[test]
    fn media_probe_duration_keeps_subframe_precision_and_rejects_invalid_values() {
        let duration = media_duration_from_seconds(12.345_678_901).unwrap();
        assert_eq!(duration.rate, FrameRate::new(1_000_000_000, 1));
        assert_eq!(duration.value, 12_345_678_901);

        assert!(media_duration_from_seconds(0.0).is_none());
        assert!(media_duration_from_seconds(f64::NAN).is_none());
        assert!(media_duration_from_seconds(f64::INFINITY).is_none());
    }

    #[test]
    fn media_import_rejects_probe_errors_before_creating_an_asset() {
        let invalid_path = std::env::temp_dir().join(format!(
            "tinixcut_invalid_media_import_{}.mp4",
            uuid::Uuid::new_v4().simple()
        ));
        fs::write(&invalid_path, b"not an MP4 container").unwrap();
        let probe_result =
            FFmpegService::probe_file(&invalid_path).map_err(|error| error.to_string());
        let _ = fs::remove_file(&invalid_path);

        let error =
            inspect_media_for_import("invalid-media-import.mp4", "mp4", probe_result).unwrap_err();

        assert!(error.contains("invalid-media-import.mp4"));
        assert!(error.contains("unsupported or corrupted"));
        assert!(error.contains("ffprobe"));
    }

    #[test]
    fn media_import_requires_a_supported_extension_and_readable_stream() {
        let unsupported =
            inspect_media_for_import("clip.txt", "txt", Ok(probed_media(Some("h264"), None)))
                .unwrap_err();
        assert!(unsupported.contains("Unsupported media format"));

        let no_streams =
            inspect_media_for_import("empty.mp4", "mp4", Ok(probed_media(None, None))).unwrap_err();
        assert!(no_streams.contains("no audio or video stream"));

        let invalid_image =
            inspect_media_for_import("invalid.png", "png", Ok(probed_media(None, Some("aac"))))
                .unwrap_err();
        assert!(invalid_image.contains("no image/video stream"));
    }

    #[test]
    fn media_import_classifies_probe_results_and_keeps_video_dimensions() {
        let video = inspect_media_for_import(
            "clip.mp4",
            "mp4",
            Ok(probed_media(Some("h264"), Some("aac"))),
        )
        .unwrap();
        assert_eq!(video.media_kind, MediaKind::Video);
        assert_eq!(
            video.dimensions,
            Some(AssetDimensions {
                width: 1280,
                height: 720
            })
        );

        let audio = inspect_media_for_import(
            "sound.wav",
            "wav",
            Ok(probed_media(None, Some("pcm_s16le"))),
        )
        .unwrap();
        assert_eq!(audio.media_kind, MediaKind::Audio);
        assert_eq!(audio.dimensions, None);

        let image =
            inspect_media_for_import("still.png", "png", Ok(probed_media(Some("png"), None)))
                .unwrap();
        assert_eq!(image.media_kind, MediaKind::Image);
        assert_eq!(image.dimensions, None);
    }
}
