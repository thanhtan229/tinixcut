#![allow(clippy::result_large_err)]

pub mod modules;

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
use crate::modules::export::{ExportEngine, ExportPreset, ExportProfile, ExportReport};
use crate::modules::history::{HistoryEntry, HistoryManager};
use crate::modules::job::{JobKind, JobManager, JobStatus};
use crate::modules::media::cache::CacheManager;
use crate::modules::media::captions::{
    parse_caption_bytes, serialize_captions, CaptionCue, CaptionFormat, MAX_CAPTION_FILE_BYTES,
};
use crate::modules::media::ffmpeg::FFmpegService;
use crate::modules::media::resolver::MediaBinaryResolver;
use crate::modules::media::transcription::{TranscriptionBoundary, TranscriptionResult};
use crate::modules::media::waveform::WaveformStatus;
use crate::modules::persistence::{
    check_recovery_status, load_project, save_project_atomic, RecoveryStatus,
};
use crate::modules::preview::{
    BenchmarkReport, NativeDecodedPreviewBackend, PreviewBackend, WebViewProxyPreviewBackend,
};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
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
    caption_reports: std::collections::HashMap<String, AutoCaptionReport>,
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
            caption_reports: std::collections::HashMap::new(),
            current_file_path: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AutoCaptionRequest {
    item_id: String,
    whisper_cli_path: String,
    model_path: String,
    language: Option<String>,
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
}

#[derive(Debug, Clone)]
struct AutoCaptionTarget {
    item: crate::modules::domain::TimelineItem,
    asset_id: String,
    asset_path: String,
}

pub struct AppState(pub Arc<Mutex<EditorEngine>>);

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

#[tauri::command]
fn project_new(state: State<'_, AppState>, name: String) -> Result<Project, String> {
    let mut engine = state.0.lock().map_err(|e| e.to_string())?;
    let proj = Project::new(name);
    engine.service = CommandService::new(proj.clone());
    engine.history = HistoryManager::new(100);
    engine.caption_reports.clear();
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
    engine.caption_reports.clear();
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
    preset: Option<ExportPreset>,
) -> Result<ExportReport, String> {
    let engine = state.0.lock().map_err(|e| e.to_string())?;
    let snapshot = engine.service.project();
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

    let is_image = matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "bmp");
    let probed = (!is_image)
        .then(|| FFmpegService::probe_file(path).ok())
        .flatten();
    let media_kind = if is_image {
        MediaKind::Image
    } else if matches!(ext.as_str(), "mp3" | "wav" | "aac" | "flac" | "ogg" | "m4a") {
        MediaKind::Audio
    } else {
        match probed.as_ref() {
            Some(meta) if meta.video_codec.is_some() => MediaKind::Video,
            Some(meta) if meta.audio_codec.is_some() => MediaKind::Audio,
            _ => MediaKind::Video,
        }
    };
    let duration = probed
        .as_ref()
        .and_then(|metadata| media_duration_from_seconds(metadata.duration_seconds));
    let dimensions = (media_kind == MediaKind::Video)
        .then(|| {
            probed.as_ref().and_then(|metadata| {
                (metadata.width > 0 && metadata.height > 0).then_some(AssetDimensions {
                    width: metadata.width,
                    height: metadata.height,
                })
            })
        })
        .flatten();

    let mut engine = state.0.lock().map_err(|e| e.to_string())?;
    let asset_id = format!("ast_{}", uuid::Uuid::new_v4().simple());
    let op = CommandOperation::AssetImport {
        asset_id: Some(asset_id.clone()),
        name: file_name,
        path: file_path.clone(),
        media_kind,
        duration,
        file_size,
        dimensions,
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
    state: State<'_, AppState>,
    file_path: String,
) -> Result<CaptionImportReport, String> {
    let path = Path::new(&file_path);
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
            },
        );
        (
            job_id.clone(),
            AutoCaptionTarget {
                item,
                asset_id,
                asset_path: asset.path.clone(),
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
    }
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
    let result = (|| -> Result<(usize, String), String> {
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
        let (count, track_id) = commit_generated_captions(&state, &target, result)?;
        Ok((count, track_id))
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
        Ok((caption_count, track_id)) => {
            engine.jobs.mark_completed(&job_id);
            engine.caption_reports.insert(
                job_id.clone(),
                AutoCaptionReport {
                    job_id,
                    state: "completed".to_string(),
                    progress: 1.0,
                    message: format!("Generated {caption_count} captions on a new track."),
                    caption_count: Some(caption_count),
                    track_id: Some(track_id),
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
                },
            );
        }
    }
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
    state: State<'_, AppState>,
    output_path: String,
) -> Result<CaptionExportReport, String> {
    let target = PathBuf::from(&output_path);
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

/// Returns an H.264/AAC preview proxy after the WebView cannot decode an original video.
/// The proxy is derived cache only: the project and export pipeline keep using the original asset.
#[tauri::command]
fn media_get_compatible_preview_video(
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

    let proxy_path = engine.cache.proxy_path(&asset_id, PREVIEW_PROXY_HEIGHT);
    if engine.cache.has_proxy(&asset_id, PREVIEW_PROXY_HEIGHT) {
        return Ok(PreviewVideoSourceStatus::Ready {
            path: proxy_path.to_string_lossy().to_string(),
            is_proxy: true,
        });
    }

    let dedupe_key = format!("preview-proxy:{}:{}", asset_id, PREVIEW_PROXY_HEIGHT);
    let job_id_candidate = format!(
        "preview_proxy_{}_{}",
        asset_id,
        uuid::Uuid::new_v4().simple()
    );
    let (job_id, is_deduped) =
        engine
            .jobs
            .submit_job(job_id_candidate, JobKind::Proxy, dedupe_key, 10);

    if let Some(job) = engine.jobs.get_job(&job_id) {
        match &job.status {
            JobStatus::Completed => {
                if engine.cache.has_proxy(&asset_id, PREVIEW_PROXY_HEIGHT) {
                    return Ok(PreviewVideoSourceStatus::Ready {
                        path: proxy_path.to_string_lossy().to_string(),
                        is_proxy: true,
                    });
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
            JobStatus::Queued if !is_deduped && engine.jobs.can_start() => {
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
fn render_text_template_poc_preview(
    text: String,
    progress: f32,
) -> Result<crate::modules::media::text_renderer_poc::PreviewOverlayIpcResponse, String> {
    Ok(crate::modules::media::text_renderer_poc::invoke_poc_preview_ipc(&text, progress))
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
    pack_path: String,
) -> Result<crate::modules::templates::pack::TemplatePackManifest, String> {
    let catalog = crate::modules::templates::catalog::TemplateCatalog::default_catalog();
    crate::modules::templates::pack::TemplatePackService::import_pack(
        Path::new(&pack_path),
        &catalog,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn template_export_pack(
    manifest: crate::modules::templates::pack::TemplatePackManifest,
    definitions: Vec<crate::modules::domain::template::TemplateDefinition>,
    asset_files: Vec<(String, String)>,
    output_path: String,
) -> Result<(), String> {
    let mut resource_map = std::collections::HashMap::new();
    for (res_id, file_path) in asset_files {
        let bytes = fs::read(&file_path)
            .map_err(|e| format!("Failed to read asset '{}': {e}", file_path))?;
        resource_map.insert(res_id, bytes);
    }
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
fn media_file_sha256(path: String) -> Result<String, String> {
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
    draft_path: String,
    selected_roots: Option<Vec<String>>,
    cancel_token: Option<String>,
) -> Result<crate::modules::templates::capcut_import::CapCutInspectionResult, String> {
    let roots: Vec<std::path::PathBuf> = selected_roots
        .unwrap_or_default()
        .into_iter()
        .map(std::path::PathBuf::from)
        .collect();

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
    report: crate::modules::templates::capcut_import::CapCutConversionReport,
    asset_source_files: Vec<(String, String)>,
    expected_source_hash: Option<String>,
) -> Result<String, String> {
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
    crate::modules::templates::agent::AgentTemplateService::validate_draft(&draft, None, Some(&catalog))
}

#[tauri::command]
fn template_draft_preview(
    draft: crate::modules::templates::agent::TemplateDraft,
    time_seconds: Option<f64>,
    output_path: Option<String>,
) -> Result<crate::modules::templates::agent::TemplatePreviewReceipt, String> {
    let t = time_seconds.unwrap_or(1.0);
    let out = output_path
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join(format!("tinixcut_preview_{}.png", uuid::Uuid::new_v4().simple())));
    crate::modules::templates::agent::AgentTemplateService::preview_draft_to_png(&draft, &out, t, None, None)
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

#[tauri::command]
fn template_draft_apply(
    state: State<'_, AppState>,
    item_id: String,
    draft: crate::modules::templates::agent::TemplateDraft,
) -> Result<crate::modules::templates::agent::TemplateApplyReceipt, String> {
    let catalog = crate::modules::templates::catalog::TemplateCatalog::default_catalog();
    let mut engine_guard = state.0.lock().map_err(|e| e.to_string())?;
    let engine = &mut *engine_guard;
    let project_path = engine
        .current_file_path
        .clone()
        .unwrap_or_else(|| std::env::temp_dir().join("active_session.tcproj"));

    save_project_atomic(engine.service.project(), &project_path).map_err(|e| e.to_string())?;

    let receipt = crate::modules::templates::agent::AgentTemplateService::apply_draft_to_project(
        &project_path,
        &item_id,
        &draft,
        Some("gui_agent".to_string()),
        None,
        None,
        None,
        None,
        &catalog,
    )
    .map_err(|e| e.to_string())?;

    let reloaded = load_project(&project_path).map_err(|e| e.to_string())?;
    engine.service = CommandService::new(reloaded);

    Ok(receipt)
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
            captions_import,
            captions_export,
            captions_auto_start,
            captions_auto_status,
            captions_auto_cancel,
            media_get_waveform,
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

#[cfg(test)]
mod tests {
    use super::{get_health_status, media_duration_from_seconds};
    use crate::modules::domain::time::FrameRate;

    #[test]
    fn test_app_health_status() {
        let health = get_health_status();
        assert_eq!(health.status, "healthy");
        assert_eq!(health.schema_version, 1);
        assert!(!health.version.is_empty());
        assert!(!health.platform.is_empty());
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
}
