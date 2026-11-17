//! Export Foundation module
//! Delivers a safe, deterministic export workflow for the supported semantic subset.
//! Architecture Invariants:
//! - Snapshot-isolated: export starts from a specific project revision; later edits do not change the in-flight render.
//! - Safe output promotion: renders to a temporary file and atomically replaces only upon success.
//! - Cancellable and non-supersedable job lifecycle.
//! - Preserves pre-existing valid output on failure or cancellation.

pub mod automation;
pub use automation::{
    escape_ffmpeg_filter_path, AutomationArtifactsGuard, ExportAutomationSchedule,
};

fn remove_silent_video_audio_layers(
    plan: &mut CompositionPlan,
    silent_video_item_ids: &std::collections::HashSet<String>,
) {
    plan.audio_layers
        .retain(|audio| !silent_video_item_ids.contains(&audio.item_id));
}

use crate::modules::composition::{CompositionPlan, TextOverlayInput};
use crate::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use crate::modules::domain::{Project, ProjectSettings};
use crate::modules::media::ffmpeg::FFmpegService;
use crate::modules::media::loudness::{self, LoudnessOutcome, LoudnessTarget};
use crate::modules::media::resolver::MediaBinaryResolver;
use crate::modules::media::text_overlay;
use crate::modules::media::text_renderer_poc::RgbaBuffer;
use schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportError {
    #[error("Unsupported export profile: {0}")]
    UnsupportedProfile(String),

    #[error("Render execution failed: {0}")]
    RenderFailed(String),

    #[error("Export operation cancelled by user or job manager")]
    Cancelled,

    #[error("IO error during export promotion: {0}")]
    IoError(String),

    #[error("Composition error: {0}")]
    Composition(String),
}

/// Maximum number of rendered frames allowed for animated text sequences in export (Finding 7).
pub const MAX_ANIMATED_SEQUENCE_FRAMES: i64 = 10_000;
/// Maximum aggregate uncompressed/PNG byte quota for materialized overlay sequences (2 GB) (Finding 7).
pub const MAX_ANIMATED_SEQUENCE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

fn default_export_audio_sample_rate() -> u32 {
    48000
}

/// Named, output-only quality choices for MP4 rendering.
///
/// Every preset inherits the project canvas and frame rate. This keeps export
/// composition aligned with the canonical project semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExportPreset {
    #[default]
    Balanced,
    HighQuality,
    SmallerFile,
}

impl ExportPreset {
    pub fn profile_for(self, settings: &ProjectSettings) -> ExportProfile {
        let mut profile = ExportProfile::from_project_settings(settings);
        match self {
            Self::Balanced => {}
            Self::HighQuality => {
                profile.crf = 18;
                profile.preset = "slow".to_string();
                profile.audio_bitrate = "256k".to_string();
            }
            Self::SmallerFile => {
                profile.crf = 28;
                profile.audio_bitrate = "128k".to_string();
            }
        }
        profile
    }
}

/// Bounded output profile for deterministic MP4 export
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportProfile {
    pub width: u32,
    pub height: u32,
    pub frame_rate: FrameRate,
    pub crf: u32,
    pub preset: String,
    pub video_codec: String,
    pub audio_codec: String,
    pub audio_bitrate: String,
    #[serde(default = "default_export_audio_sample_rate")]
    pub audio_sample_rate: u32,
    /// Optional integrated-loudness target; `None` leaves the mix exactly as rendered.
    #[serde(default)]
    pub loudness: Option<LoudnessTarget>,
    /// Optional timeline range to export (an In/Out selection); `None` exports everything.
    #[serde(default)]
    pub range: Option<ExportRange>,
}

/// Half-open frame range `[start_frame, end_frame)` at the project frame rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportRange {
    pub start_frame: i64,
    pub end_frame: i64,
}

impl ExportRange {
    pub fn frames(&self) -> i64 {
        self.end_frame - self.start_frame
    }
}

impl Default for ExportProfile {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            frame_rate: FrameRate::FPS_30,
            crf: 23,
            preset: "fast".to_string(),
            video_codec: "libx264".to_string(),
            audio_codec: "aac".to_string(),
            audio_bitrate: "192k".to_string(),
            audio_sample_rate: 48000,
            loudness: None,
            range: None,
        }
    }
}

impl ExportProfile {
    /// Derive the default export target from the canonical project canvas.
    pub fn from_project_settings(settings: &ProjectSettings) -> Self {
        Self {
            width: settings.width,
            height: settings.height,
            frame_rate: settings.frame_rate,
            audio_sample_rate: settings.sample_rate,
            ..Self::default()
        }
    }

    pub fn validate(&self) -> Result<(), ExportError> {
        let supported_codecs = ["auto", "libx264", "h264_nvenc", "h264_qsv", "h264_amf"];
        if !supported_codecs.contains(&self.video_codec.as_str()) {
            return Err(ExportError::UnsupportedProfile(format!(
                "Video codec '{}' is not supported in v1 (only auto, libx264 and standard hardware codecs are supported)",
                self.video_codec
            )));
        }
        if self.audio_codec != "aac" {
            return Err(ExportError::UnsupportedProfile(format!(
                "Audio codec '{}' is not supported in v1 (only aac is supported)",
                self.audio_codec
            )));
        }
        if self.audio_sample_rate != 48000 && self.audio_sample_rate != 44100 {
            return Err(ExportError::UnsupportedProfile(format!(
                "Audio sample rate {} is not supported (only 48000 and 44100 Hz are supported)",
                self.audio_sample_rate
            )));
        }
        if self.width == 0 || self.height == 0 {
            return Err(ExportError::UnsupportedProfile(
                "Export dimensions must be positive integers".to_string(),
            ));
        }
        if let Some(target) = &self.loudness {
            target.validate().map_err(ExportError::UnsupportedProfile)?;
        }
        if let Some(range) = &self.range {
            if range.start_frame < 0 || range.end_frame <= range.start_frame {
                return Err(ExportError::UnsupportedProfile(
                    "Export range must satisfy 0 <= start < end".to_string(),
                ));
            }
        }
        if self.crf > 51 {
            return Err(ExportError::UnsupportedProfile(
                "CRF must be between 0 and 51".to_string(),
            ));
        }
        Ok(())
    }

    /// The current composition renderer always renders the canonical project
    /// canvas and frame rate. Reject contradictory profiles instead of
    /// accepting output settings that the renderer cannot honor.
    pub fn validate_for_project(&self, settings: &ProjectSettings) -> Result<(), ExportError> {
        self.validate()?;
        if self.width != settings.width || self.height != settings.height {
            return Err(ExportError::UnsupportedProfile(format!(
                "Export dimensions {}x{} must match the project canvas {}x{}",
                self.width, self.height, settings.width, settings.height
            )));
        }
        if self.frame_rate != settings.frame_rate {
            return Err(ExportError::UnsupportedProfile(
                "Export frame rate must match the project frame rate".to_string(),
            ));
        }
        Ok(())
    }
}

/// Report returned upon successful export completion
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportReport {
    pub output_path: PathBuf,
    pub duration_secs: f64,
    pub file_size_bytes: u64,
    pub render_time_ms: u128,
    #[serde(default)]
    pub encoder_used: Option<String>,
    #[serde(default)]
    pub attempted_encoders: Vec<String>,
    /// Present when the profile asked for a loudness target.
    #[serde(default)]
    pub loudness: Option<LoudnessOutcome>,
}

/// Read-only snapshot of export constraints shown before a render starts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExportPreflightReport {
    pub project_id: String,
    pub project_revision: u64,
    pub output_path: PathBuf,
    pub output_exists: bool,
    pub available_disk_bytes: u64,
    pub source_count: usize,
    pub duration_secs: f64,
    pub width: u32,
    pub height: u32,
    pub frame_rate_numerator: u32,
    pub frame_rate_denominator: u32,
    pub warnings: Vec<String>,
}

/// Removes a disposable derived-artifact directory on every exit path.
struct TempDirGuard(PathBuf);

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Longest filter graph passed on the command line. Windows allows about 32 000 characters for
/// everything together; longer graphs (long keyframed motion makes long position expressions)
/// are read from a file with `-/filter_complex`.
const MAX_INLINE_FILTER_GRAPH_CHARS: usize = 6000;

/// Moves a long `-filter_complex` graph out of the argument list into `<dir>/filter_complex.txt`
/// and replaces it with `-/filter_complex <file>`. Returns whether it did. The graph text is
/// unchanged: FFmpeg parses the file with the same filter-graph syntax as the argument.
fn spill_long_filter_graph(args: &mut [String], dir: &Path) -> std::io::Result<bool> {
    let Some(position) = args.iter().position(|a| a == "-filter_complex") else {
        return Ok(false);
    };
    let Some(graph) = args.get(position + 1) else {
        return Ok(false);
    };
    if graph.len() <= MAX_INLINE_FILTER_GRAPH_CHARS {
        return Ok(false);
    }
    fs::create_dir_all(dir)?;
    let file = dir.join("filter_complex.txt");
    fs::write(&file, graph)?;
    args[position] = "-/filter_complex".to_string();
    args[position + 1] = file.to_string_lossy().into_owned();
    Ok(true)
}

fn remove_existing_output(path: &Path) {
    for _ in 0..10 {
        if !path.exists() {
            return;
        }
        if fs::remove_file(path).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
}

fn rename_with_retry(from: &Path, to: &Path) -> std::io::Result<()> {
    let mut last_error = None;
    for _ in 0..10 {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(error) => {
                last_error = Some(error);
                std::thread::sleep(Duration::from_millis(15));
            }
        }
    }
    Err(last_error.unwrap_or_else(|| std::io::Error::other("promotion retry exhausted")))
}

/// Export Engine coordinating snapshot freeze, FFmpeg execution, and atomic promotion
pub struct ExportEngine;

/// Length of the exported output: the selected range, or the whole timeline.
fn export_span_seconds(plan: &CompositionPlan, profile: &ExportProfile) -> f64 {
    match &profile.range {
        Some(range) => range.frames() as f64 / plan.frame_rate.as_f64(),
        None => plan.duration.to_seconds(),
    }
}

/// Applies the profile's loudness target to the rendered temporary file. Returns `None` when no
/// target was requested. A measurement or encode failure is an export error, never a silent skip.
fn apply_loudness(
    temp_output: &Path,
    profile: &ExportProfile,
    cancel_flag: Option<&Arc<AtomicBool>>,
) -> Result<Option<LoudnessOutcome>, ExportError> {
    let Some(target) = profile.loudness else {
        return Ok(None);
    };
    let cancelled = || cancel_flag.is_some_and(|f| f.load(Ordering::SeqCst));
    let fail = |message: String| {
        let _ = fs::remove_file(temp_output);
        ExportError::RenderFailed(message)
    };
    let mut outcome = LoudnessOutcome {
        target_lufs: target.integrated_lufs,
        true_peak_limit_db: target.true_peak_db,
        input_lufs: None,
        output_lufs: None,
        output_true_peak_db: None,
        normalized: false,
        note: None,
    };
    if cancelled() {
        let _ = fs::remove_file(temp_output);
        return Err(ExportError::Cancelled);
    }
    let measured = loudness::measure(temp_output).map_err(&fail)?;
    let Some(measured) = measured else {
        outcome.note = Some("The export has no audio; no loudness change was made".into());
        return Ok(Some(outcome));
    };
    outcome.input_lufs = Some(measured.integrated_lufs);
    outcome.output_lufs = Some(measured.integrated_lufs);
    outcome.output_true_peak_db = Some(measured.true_peak_db);
    if measured.integrated_lufs < loudness::SILENCE_FLOOR_LUFS {
        outcome.note = Some("The audio is silent or nearly silent; it was not amplified".into());
        return Ok(Some(outcome));
    }
    let within_tolerance = (measured.integrated_lufs - target.integrated_lufs as f64).abs()
        <= loudness::SKIP_TOLERANCE_LU
        && measured.true_peak_db <= target.true_peak_db as f64;
    if within_tolerance {
        outcome.note = Some("The mix already meets the target; it was left unchanged".into());
        return Ok(Some(outcome));
    }
    if cancelled() {
        let _ = fs::remove_file(temp_output);
        return Err(ExportError::Cancelled);
    }
    let normalized = temp_output.with_extension("loudnorm.mp4");
    let _ = fs::remove_file(&normalized);
    loudness::normalize(
        temp_output,
        &normalized,
        &target,
        &measured,
        &profile.audio_bitrate,
        profile.audio_sample_rate,
    )
    .map_err(|message| {
        let _ = fs::remove_file(&normalized);
        fail(message)
    })?;
    // Report what the delivered file actually measures, not what loudnorm aimed for.
    let after = loudness::measure(&normalized).map_err(|message| {
        let _ = fs::remove_file(&normalized);
        fail(message)
    })?;
    fs::rename(&normalized, temp_output).map_err(|e| {
        let _ = fs::remove_file(&normalized);
        fail(format!(
            "Could not replace the render with the normalized file: {e}"
        ))
    })?;
    outcome.normalized = true;
    if let Some(after) = after {
        outcome.output_lufs = Some(after.integrated_lufs);
        outcome.output_true_peak_db = Some(after.true_peak_db);
    }
    Ok(Some(outcome))
}

impl ExportEngine {
    /// Validate a frozen project snapshot and the selected destination before scheduling a render.
    pub fn preflight(
        snapshot: &Project,
        target_path: &Path,
        profile: &ExportProfile,
    ) -> Result<ExportPreflightReport, ExportError> {
        profile.validate_for_project(&snapshot.settings)?;
        if target_path.as_os_str().is_empty() || target_path.file_name().is_none() {
            return Err(ExportError::IoError(
                "Choose a valid export file path".to_string(),
            ));
        }
        let parent = target_path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        if !parent.is_dir() {
            return Err(ExportError::IoError(format!(
                "Export destination folder does not exist: {}",
                parent.display()
            )));
        }
        if target_path.is_dir() {
            return Err(ExportError::IoError(
                "Export destination points to a folder".to_string(),
            ));
        }

        let plan = CompositionPlan::from_project(snapshot)
            .map_err(|error| ExportError::Composition(error.to_string()))?;
        plan.verify_lut_files()
            .map_err(|error| ExportError::Composition(error.to_string()))?;
        let mut sources = HashMap::<PathBuf, String>::new();
        for layer in &plan.visual_layers {
            if layer.asset_id.is_some() && layer.asset_path.is_none() {
                return Err(ExportError::RenderFailed(format!(
                    "Video or image source for clip '{}' is unavailable",
                    layer.item_id
                )));
            }
            if let (Some(asset_id), Some(path)) = (&layer.asset_id, &layer.asset_path) {
                sources
                    .entry(path.clone())
                    .or_insert_with(|| asset_id.clone());
            }
        }
        for layer in &plan.audio_layers {
            if layer.asset_id.is_some() && layer.asset_path.is_none() {
                return Err(ExportError::RenderFailed(format!(
                    "Audio source for clip '{}' is unavailable",
                    layer.item_id
                )));
            }
            if let (Some(asset_id), Some(path)) = (&layer.asset_id, &layer.asset_path) {
                sources
                    .entry(path.clone())
                    .or_insert_with(|| asset_id.clone());
            }
        }
        for (path, asset_id) in &sources {
            if !path.is_file() {
                return Err(ExportError::RenderFailed(format!(
                    "Source media '{}' is offline or missing",
                    asset_id
                )));
            }
            FFmpegService::probe_file(path).map_err(|error| {
                ExportError::RenderFailed(format!(
                    "Source media '{}' cannot be read by the export toolchain: {error}",
                    asset_id
                ))
            })?;
        }

        let available_disk_bytes = crate::modules::media::collect::available_space(parent)
            .map_err(ExportError::IoError)?;
        let output_exists = target_path.exists();
        let mut warnings = Vec::new();
        if output_exists {
            warnings.push(
                "An existing output file will be replaced only after a successful render."
                    .to_string(),
            );
        }
        warnings.push(
            "Available disk space is a point-in-time value; final MP4 size depends on its content."
                .to_string(),
        );
        Ok(ExportPreflightReport {
            project_id: snapshot.project_id.clone(),
            project_revision: snapshot.revision,
            output_path: target_path.to_path_buf(),
            output_exists,
            available_disk_bytes,
            source_count: sources.len(),
            duration_secs: plan.duration.to_seconds(),
            width: profile.width,
            height: profile.height,
            frame_rate_numerator: profile.frame_rate.numerator,
            frame_rate_denominator: profile.frame_rate.denominator,
            warnings,
        })
    }

    /// Ordered ladder candidates according to ADR-002: NVENC -> QSV -> AMF -> libx264
    pub fn get_encoder_candidates(requested: &str) -> Vec<&'static str> {
        match requested {
            "auto" | "h264_nvenc" => vec!["h264_nvenc", "h264_qsv", "h264_amf", "libx264"],
            "h264_qsv" => vec!["h264_qsv", "h264_amf", "libx264"],
            "h264_amf" => vec!["h264_amf", "libx264"],
            "libx264" => vec!["libx264"],
            _ => vec!["libx264"],
        }
    }

    /// Discriminate hardware/encoder initialization failures from I/O or syntax errors.
    /// Only genuine hardware/device/driver/encoder failures trigger ladder fallback.
    pub fn is_hardware_encoder_error(stderr: &str, codec: &str) -> bool {
        if codec == "libx264" {
            return false;
        }

        let lower = stderr.to_lowercase();

        // Non-hardware errors must NOT trigger fallback!
        if lower.contains("no such file or directory")
            || lower.contains("permission denied")
            || lower.contains("no space left on device")
            || lower.contains("matches no streams")
        {
            return false;
        }

        // Hardware encoder and driver failure signatures
        lower.contains("cannot load nvcuda.dll")
            || lower.contains("cannot load nvenvapi.dll")
            || lower.contains("no nvenc capable devices found")
            || lower.contains("device creation failed")
            || lower.contains("driver version insufficient")
            || lower.contains("function not implemented")
            || lower.contains("unknown encoder")
            || lower.contains("encoder not found")
            || lower.contains("error while opening encoder")
            || lower.contains("failed to open encoder")
            || lower.contains("initialization failed")
            || lower.contains("device not found")
            || lower.contains("selected device is not valid")
            || lower.contains("error creating amf context")
            || lower.contains("amf failed to initialize")
            || lower.contains("error initializing output stream")
            || lower.contains("no device available for encoder")
            || lower.contains("openencodesessionex failed")
            || lower.contains("nvcuda.dll")
            || lower.contains("nvenc")
            || lower.contains("qsv")
            || lower.contains("amf")
            || lower.contains("mfx")
            || lower.contains("unrecognized option 'c:v")
            || lower.contains("unrecognized option 'vcodec")
            || lower.contains("cannot load")
            || lower.contains("not supported")
    }

    fn render_failure_message(codec: &str, stderr: &str) -> String {
        let lower = stderr.to_lowercase();
        if lower.contains("no space left on device")
            || lower.contains("not enough space")
            || lower.contains("disk is full")
        {
            return format!(
                "Insufficient disk space to finish the export. Free up space and retry. FFmpeg encoder '{codec}' reported: {stderr}"
            );
        }

        format!(
            "FFmpeg render process failed with encoder '{codec}' (status error). Stderr: {stderr}"
        )
    }

    /// Replaces every reversed layer whose source range exceeds one reverse piece with a
    /// lossless, already reversed scratch file, and clears that layer's `reverse` flag.
    ///
    /// The scratch file starts at the layer's source start, so the layer then reads
    /// `[0, duration)` forward. Video and audio layers over the same range share one file.
    fn materialize_long_reverses(
        plan: &mut CompositionPlan,
        dir: &Path,
        cancel_flag: Option<&Arc<AtomicBool>>,
    ) -> Result<(), ExportError> {
        use crate::modules::media::ffmpeg::{ReverseOutput, REVERSE_SEGMENT_SECONDS};

        let needs = |reverse: bool, path: &Option<PathBuf>, range: &TimeRange| {
            reverse
                && path.is_some()
                && range.duration.to_seconds() > REVERSE_SEGMENT_SECONDS + 1e-9
        };
        let mut rendered: HashMap<(PathBuf, i64, i64), PathBuf> = HashMap::new();
        let mut render = |path: &Path, range: &TimeRange| -> Result<PathBuf, ExportError> {
            let key = (
                path.to_path_buf(),
                (range.start.to_seconds() * 1_000_000.0).round() as i64,
                (range.duration.to_seconds() * 1_000_000.0).round() as i64,
            );
            if let Some(existing) = rendered.get(&key) {
                return Ok(existing.clone());
            }
            fs::create_dir_all(dir).map_err(|error| {
                ExportError::IoError(format!(
                    "Failed to create reverse scratch directory: {error}"
                ))
            })?;
            let out = dir.join(format!("reverse_{}.mkv", rendered.len()));
            FFmpegService::render_reversed_range(
                path,
                range.start.to_seconds(),
                range.duration.to_seconds(),
                &out,
                ReverseOutput::Intermediate,
                cancel_flag.map(|flag| flag.as_ref()),
            )
            .map_err(|error| {
                if cancel_flag.is_some_and(|flag| flag.load(Ordering::SeqCst)) {
                    ExportError::Cancelled
                } else {
                    ExportError::RenderFailed(format!("Reversing a long clip failed: {error}"))
                }
            })?;
            rendered.insert(key, out.clone());
            Ok(out)
        };

        for layer in &mut plan.visual_layers {
            if layer.kind == crate::modules::domain::ItemKind::Video
                && needs(layer.reverse, &layer.asset_path, &layer.source_range)
            {
                let path = layer.asset_path.clone().unwrap_or_default();
                layer.asset_path = Some(render(&path, &layer.source_range)?);
                layer.source_range = TimeRange::new(
                    RationalTime::new(0, layer.source_range.start.rate),
                    layer.source_range.duration,
                );
                layer.reverse = false;
            }
        }
        for layer in &mut plan.audio_layers {
            if needs(layer.reverse, &layer.asset_path, &layer.source_range) {
                let path = layer.asset_path.clone().unwrap_or_default();
                layer.asset_path = Some(render(&path, &layer.source_range)?);
                layer.source_range = TimeRange::new(
                    RationalTime::new(0, layer.source_range.start.rate),
                    layer.source_range.duration,
                );
                layer.reverse = false;
            }
        }
        Ok(())
    }

    /// Renders a project snapshot to the target file safely and deterministically.
    /// Invariant: Edits made to other project instances do not affect this frozen snapshot.
    pub fn render_snapshot(
        snapshot: &Project,
        target_path: &Path,
        profile: &ExportProfile,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<ExportReport, ExportError> {
        Self::render_snapshot_with_progress(snapshot, target_path, profile, cancel_flag, |_| {})
    }

    /// Renders a frozen project snapshot while reporting monotonic progress.
    /// Progress is derived from FFmpeg's out-time and never predicts completion.
    pub fn render_snapshot_with_progress<F>(
        snapshot: &Project,
        target_path: &Path,
        profile: &ExportProfile,
        cancel_flag: Option<Arc<AtomicBool>>,
        mut on_progress: F,
    ) -> Result<ExportReport, ExportError>
    where
        F: FnMut(f32),
    {
        Self::preflight(snapshot, target_path, profile)?;
        on_progress(0.05);

        // Compile normalized CompositionPlan from the frozen project snapshot
        let mut plan = CompositionPlan::from_project(snapshot)
            .map_err(|e| ExportError::Composition(e.to_string()))?;
        if let Some(range) = &profile.range {
            let total = plan.duration.rescaled_to(plan.frame_rate).value;
            if range.end_frame > total {
                return Err(ExportError::UnsupportedProfile(format!(
                    "Export range ends at frame {} but the timeline has {} frames",
                    range.end_frame, total
                )));
            }
        }
        on_progress(0.12);

        // Video items are also represented as audio layers so clips with embedded
        // audio can be mixed. A silent video has no `[input:a]` stream, however,
        // and must not contribute an audio filter branch to the FFmpeg graph.
        let video_item_ids: std::collections::HashSet<_> = plan
            .visual_layers
            .iter()
            .filter(|layer| layer.kind == crate::modules::domain::ItemKind::Video)
            .map(|layer| layer.item_id.clone())
            .collect();
        let mut video_audio_presence = std::collections::HashMap::new();
        for audio in &plan.audio_layers {
            if !video_item_ids.contains(&audio.item_id) {
                continue;
            }
            let path = audio.asset_path.as_ref().ok_or_else(|| {
                ExportError::RenderFailed(format!(
                    "Video clip '{}' is missing its media path while checking audio streams",
                    audio.item_id
                ))
            })?;
            if !video_audio_presence.contains_key(path) {
                let metadata = FFmpegService::probe_file(path).map_err(|error| {
                    ExportError::RenderFailed(format!(
                        "Failed to inspect audio streams for video '{}': {error}",
                        path.display()
                    ))
                })?;
                video_audio_presence.insert(path.clone(), metadata.audio_codec.is_some());
            }
        }
        let silent_video_item_ids: std::collections::HashSet<_> = plan
            .audio_layers
            .iter()
            .filter(|audio| {
                video_item_ids.contains(&audio.item_id)
                    && audio
                        .asset_path
                        .as_ref()
                        .and_then(|path| video_audio_presence.get(path))
                        .is_some_and(|has_audio| !has_audio)
            })
            .map(|audio| audio.item_id.clone())
            .collect();
        remove_silent_video_audio_layers(&mut plan, &silent_video_item_ids);

        // Long reversed clips are reversed in pieces into scratch files and read from there,
        // so no filter in the export graph buffers more than one piece.
        let reverse_dir = target_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!(
                "tc_rev_{}_{}",
                std::process::id(),
                uuid::Uuid::new_v4().simple()
            ));
        let _reverse_dir_guard = TempDirGuard(reverse_dir.clone());
        Self::materialize_long_reverses(&mut plan, &reverse_dir, cancel_flag.as_ref())?;

        // Ducking is data derived from the mix: measure it once here so the envelope written
        // below and the preview evaluate the same curve.
        let ducking_curves =
            crate::modules::composition::ducking::compute_curves(&plan, cancel_flag.as_ref())
                .map_err(|message| {
                    if cancel_flag
                        .as_ref()
                        .is_some_and(|flag| flag.load(Ordering::SeqCst))
                    {
                        ExportError::Cancelled
                    } else {
                        ExportError::RenderFailed(format!("Ducking analysis failed: {message}"))
                    }
                })?;
        plan.attach_ducking_curves(ducking_curves);

        // 1. Establish temporary output path for safe promotion
        let temp_filename = format!(
            "{}.{}.tc_render.tmp.mp4",
            target_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy(),
            std::process::id()
        );
        let temp_output_path = target_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(temp_filename);

        // Ensure parent directory exists
        if let Some(parent) = temp_output_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| ExportError::IoError(format!("Failed to create parent dir: {e}")))?;
        }

        // Clean up any stale temp file
        let _ = fs::remove_file(&temp_output_path);

        let automation_dir = temp_output_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!(
                "tc_auto_{}_{}",
                std::process::id(),
                uuid::Uuid::new_v4().simple()
            ));
        let _automation_dir_guard = TempDirGuard(automation_dir.clone());
        let mut schedule = ExportAutomationSchedule::build(
            &plan,
            &automation_dir,
            profile.audio_sample_rate,
            cancel_flag.as_ref(),
        )?;
        on_progress(0.2);

        // Materialize rich-text overlays through the shared Phase 30 renderer.
        // These PNGs are disposable derived artifacts, never project state.
        let overlay_dir = automation_dir.join("text_overlays");
        let _overlay_guard = TempDirGuard(overlay_dir.clone());
        let mut overlays: Vec<TextOverlayInput> = Vec::new();
        if plan
            .visual_layers
            .iter()
            .any(|layer| layer.uses_shared_text_overlay())
        {
            fs::create_dir_all(&overlay_dir).map_err(|error| {
                ExportError::IoError(format!("Failed to create text overlay directory: {error}"))
            })?;
            // Aggregate budget is export-wide, not per layer.
            let mut total_seq_bytes: u64 = 0;
            for layer in &plan.visual_layers {
                if layer.item_id.contains("__layer_") {
                    continue;
                }
                if !layer.uses_shared_text_overlay() {
                    continue;
                }

                let has_animation = layer.text_animation.is_some()
                    || !layer.keyframes.is_empty()
                    || layer.template_instance.as_ref().is_some_and(|ti| {
                        ti.layers.iter().any(|l| {
                            l.motion.is_some()
                                || match &l.kind {
                                    crate::modules::domain::template::TemplateLayerKind::Text(
                                        t,
                                    ) => t.text_animation.is_some(),
                                    _ => false,
                                }
                        })
                    });
                if has_animation {
                    // Animated rich text or keyframed text: materialize per-frame PNG sequence over the clip window
                    let rate = plan.frame_rate;
                    let duration_ticks = layer.timeline_range.duration.rescaled_to(rate).value;
                    let num_frames = duration_ticks.max(1);

                    // Enforce documented frame quota (Finding 7)
                    if num_frames > MAX_ANIMATED_SEQUENCE_FRAMES {
                        return Err(ExportError::RenderFailed(format!(
                            "Animated text sequence for '{}' exceeds frame quota ({} > {})",
                            layer.name, num_frames, MAX_ANIMATED_SEQUENCE_FRAMES
                        )));
                    }

                    let seq_dir = overlay_dir.join(format!("{}_seq", layer.item_id));
                    fs::create_dir_all(&seq_dir).map_err(|error| {
                        ExportError::IoError(format!(
                            "Failed to create sequence directory: {error}"
                        ))
                    })?;

                    for frame_idx in 0..num_frames {
                        // Check cancellation flag per frame during sequence preparation (Finding 7)
                        if let Some(ref flag) = cancel_flag {
                            if flag.load(Ordering::SeqCst) {
                                return Err(ExportError::Cancelled);
                            }
                        }

                        let timestamp = layer
                            .timeline_range
                            .start
                            .add_exact(&RationalTime::from_frames(frame_idx, rate));
                        let buf = text_overlay::render_frame_overlay_buffer_at(
                            snapshot, &plan, layer, timestamp,
                        )
                        .map_err(|error| {
                            ExportError::RenderFailed(format!(
                                "Animated text overlay frame failed for '{}': {error}",
                                layer.name
                            ))
                        })?
                        .unwrap_or_else(|| RgbaBuffer::new(plan.width, plan.height));

                        let png_bytes = buf.to_png_bytes_compressed().map_err(|error| {
                            ExportError::RenderFailed(format!(
                                "Animated text overlay PNG encoding failed for '{}': {error}",
                                layer.name
                            ))
                        })?;
                        total_seq_bytes += png_bytes.len() as u64;

                        // Enforce documented byte quota (Finding 7)
                        if total_seq_bytes > MAX_ANIMATED_SEQUENCE_BYTES {
                            return Err(ExportError::RenderFailed(format!(
                                "Animated text sequence for '{}' exceeded byte quota ({} bytes > {} bytes)",
                                layer.name, total_seq_bytes, MAX_ANIMATED_SEQUENCE_BYTES
                            )));
                        }

                        let frame_path = seq_dir.join(format!("frame_{:06}.png", frame_idx));
                        fs::write(&frame_path, &png_bytes).map_err(|error| {
                            ExportError::IoError(format!(
                                "Failed to write sequence frame '{:?}': {error}",
                                frame_path
                            ))
                        })?;
                    }

                    overlays.push(TextOverlayInput {
                        item_id: layer.item_id.clone(),
                        path: seq_dir.join("frame_%06d.png"),
                        roi: [0, 0, plan.width, plan.height],
                    });
                } else {
                    match text_overlay::render_overlay_for_layer(
                        snapshot,
                        plan.width,
                        plan.height,
                        layer,
                        layer.opacity,
                    ) {
                        Ok(Some(overlay)) if overlay.has_visible_content => {
                            let path = overlay_dir.join(format!("{}.png", layer.item_id));
                            fs::write(&path, &overlay.roi_png_bytes).map_err(|error| {
                                ExportError::IoError(format!(
                                    "Failed to write text overlay '{}': {error}",
                                    layer.item_id
                                ))
                            })?;
                            overlays.push(TextOverlayInput {
                                item_id: layer.item_id.clone(),
                                path,
                                roi: overlay.roi,
                            });
                        }
                        Ok(_) => {}
                        Err(error) => {
                            return Err(ExportError::RenderFailed(format!(
                                "Rich text overlay failed for '{}': {error}",
                                layer.name
                            )));
                        }
                    }
                }
            }
        }

        plan.verify_lut_files()
            .map_err(|error| ExportError::Composition(error.to_string()))?;

        let candidates = Self::get_encoder_candidates(&profile.video_codec);
        let mut attempted_encoders: Vec<String> = Vec::new();
        let start_time = Instant::now();

        for (i, &candidate) in candidates.iter().enumerate() {
            // Check cancellation before launching candidate
            if let Some(ref flag) = cancel_flag {
                if flag.load(Ordering::SeqCst) {
                    let _ = fs::remove_file(&temp_output_path);
                    return Err(ExportError::Cancelled);
                }
            }

            attempted_encoders.push(candidate.to_string());

            // Generate FFmpeg command arguments from the CompositionPlan with candidate codec and automation schedule
            let (args, _) = plan.to_ffmpeg_export_args_with_overlays(
                &temp_output_path,
                profile.crf,
                candidate,
                Some(&schedule),
                &overlays,
            );
            let mut args = args;
            spill_long_filter_graph(&mut args, &automation_dir).map_err(|error| {
                ExportError::IoError(format!("Failed to write the filter graph file: {error}"))
            })?;
            let output_argument = args
                .pop()
                .expect("FFmpeg export arguments always end with an output path");
            if let Some(range) = &profile.range {
                // Output-side seek/limit: frames before the range are rendered but not
                // encoded, so picture and audio stay in sync and the result is frame exact.
                let rate = plan.frame_rate.as_f64();
                args.extend([
                    "-ss".to_string(),
                    format!("{:.6}", range.start_frame as f64 / rate),
                    "-t".to_string(),
                    format!("{:.6}", range.frames() as f64 / rate),
                ]);
            }
            args.extend([
                "-nostats".to_string(),
                "-stats_period".to_string(),
                "0.25".to_string(),
                "-progress".to_string(),
                "pipe:1".to_string(),
            ]);
            args.push(output_argument);

            // Spawn child FFmpeg process via centralized MediaBinaryResolver
            let mut cmd = match MediaBinaryResolver::command("ffmpeg") {
                Ok(c) => c,
                Err(e) => {
                    let _ = fs::remove_file(&temp_output_path);
                    return Err(ExportError::RenderFailed(format!(
                        "Failed to resolve FFmpeg binary: {e}"
                    )));
                }
            };

            let mut child: Child = cmd
                .args(&args)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| {
                    let _ = fs::remove_file(&temp_output_path);
                    ExportError::RenderFailed(format!("Failed to spawn FFmpeg process: {e}"))
                })?;

            // Drain stderr concurrently while FFmpeg runs. Reading it only after
            // wait/try_wait can deadlock when the OS pipe buffer fills.
            let mut stderr_reader = child.stderr.take().map(|mut err_pipe| {
                std::thread::spawn(move || {
                    use std::io::Read;
                    let mut stderr = String::new();
                    let _ = err_pipe.read_to_string(&mut stderr);
                    stderr
                })
            });
            let (progress_tx, progress_rx) = std::sync::mpsc::channel::<f64>();
            let mut progress_reader = child.stdout.take().map(|out_pipe| {
                std::thread::spawn(move || {
                    use std::io::{BufRead, BufReader};
                    let reader = BufReader::new(out_pipe);
                    for line in reader.lines().flatten() {
                        if let Some(value) = line
                            .strip_prefix("out_time_us=")
                            .or_else(|| line.strip_prefix("out_time_ms="))
                            .and_then(|value| value.parse::<f64>().ok())
                        {
                            // FFmpeg labels both fields in microseconds in supported builds.
                            let _ = progress_tx.send(value / 1_000_000.0);
                        }
                    }
                })
            });
            let mut cancelled = false;
            let mut render_success = false;
            let mut stderr_str = String::new();

            loop {
                let duration = export_span_seconds(&plan, profile).max(0.001);
                for out_time in progress_rx.try_iter() {
                    let encoder_fraction = (out_time / duration).clamp(0.0, 1.0);
                    let candidate_fraction =
                        (i as f64 + encoder_fraction) / candidates.len().max(1) as f64;
                    on_progress((0.2 + candidate_fraction as f32 * 0.75).min(0.95));
                }
                if let Some(ref flag) = cancel_flag {
                    if flag.load(Ordering::SeqCst) {
                        cancelled = true;
                        let _ = child.kill();
                        let _ = child.wait();
                        if let Some(reader) = stderr_reader.take() {
                            let _ = reader.join();
                        }
                        if let Some(reader) = progress_reader.take() {
                            let _ = reader.join();
                        }
                        break;
                    }
                }

                match child.try_wait() {
                    Ok(Some(status)) => {
                        if status.success() {
                            render_success = true;
                        }
                        if let Some(reader) = stderr_reader.take() {
                            stderr_str = reader
                                .join()
                                .unwrap_or_else(|_| "Failed to collect FFmpeg stderr".to_string());
                        }
                        if let Some(reader) = progress_reader.take() {
                            let _ = reader.join();
                        }
                        break;
                    }
                    Ok(None) => {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    Err(e) => {
                        let _ = child.kill();
                        let _ = child.wait();
                        if let Some(reader) = stderr_reader.take() {
                            let _ = reader.join();
                        }
                        if let Some(reader) = progress_reader.take() {
                            let _ = reader.join();
                        }
                        let _ = fs::remove_file(&temp_output_path);
                        return Err(ExportError::RenderFailed(format!(
                            "Error checking FFmpeg process: {e}"
                        )));
                    }
                }
            }

            if cancelled {
                let _ = fs::remove_file(&temp_output_path);
                return Err(ExportError::Cancelled);
            }

            if render_success {
                // Verify temporary file was written and is non-empty
                let metadata = fs::metadata(&temp_output_path).map_err(|e| {
                    ExportError::RenderFailed(format!(
                        "Rendered file is missing or inaccessible: {e}"
                    ))
                })?;

                if metadata.len() == 0 {
                    let _ = fs::remove_file(&temp_output_path);
                    return Err(ExportError::RenderFailed(
                        "Rendered file is empty (0 bytes)".to_string(),
                    ));
                }

                // Optional loudness step: runs on the temporary file, so a failure or a
                // cancellation leaves the destination untouched.
                let loudness_outcome =
                    apply_loudness(&temp_output_path, profile, cancel_flag.as_ref())?;
                let metadata = fs::metadata(&temp_output_path).map_err(|e| {
                    ExportError::RenderFailed(format!(
                        "Rendered file is missing or inaccessible: {e}"
                    ))
                })?;

                // Safe output promotion: atomically rename temp file to target output.
                // Windows can briefly hold a scan lock on a just-created file, so a
                // bounded retry prevents a spurious failure when re-exporting.
                remove_existing_output(target_path);
                rename_with_retry(&temp_output_path, target_path).map_err(|e| {
                    let _ = fs::remove_file(&temp_output_path);
                    ExportError::IoError(format!(
                        "Failed to promote temporary render file to destination: {e}"
                    ))
                })?;

                let render_time_ms = start_time.elapsed().as_millis();
                let file_size_bytes = metadata.len();
                let duration_secs = export_span_seconds(&plan, profile);

                schedule.guard.cleanup();
                on_progress(1.0);

                return Ok(ExportReport {
                    output_path: target_path.to_path_buf(),
                    duration_secs,
                    file_size_bytes,
                    render_time_ms,
                    encoder_used: Some(candidate.to_string()),
                    attempted_encoders,
                    loudness: loudness_outcome,
                });
            }

            // Render failed with this candidate, clean up temp file
            let _ = fs::remove_file(&temp_output_path);

            // Check if fallback to the next candidate in the ladder is permitted
            let has_next = i + 1 < candidates.len();
            if has_next && Self::is_hardware_encoder_error(&stderr_str, candidate) {
                eprintln!(
                    "[ExportEngine] Hardware encoder '{}' unavailable or failed; falling back to '{}'. Stderr: {}",
                    candidate,
                    candidates[i + 1],
                    stderr_str.lines().next().unwrap_or("")
                );
                continue;
            }

            // Non-hardware error or final fallback candidate exhausted: fail explicitly
            return Err(ExportError::RenderFailed(Self::render_failure_message(
                candidate,
                &stderr_str,
            )));
        }

        Err(ExportError::RenderFailed(
            "All encoder candidates exhausted without success".to_string(),
        ))
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::modules::domain::time::{FrameRate, RationalTime, TimeRange};
    use crate::modules::domain::{
        Asset, ItemKind, MediaKind, Project, ProjectSettings, TimelineItem, Track, TrackKind,
    };
    use std::fs;

    #[test]
    fn long_filter_graphs_are_read_from_a_file_and_short_ones_stay_inline() {
        let dir = std::env::temp_dir().join(format!("tinixcut_spill_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let long = format!("color=c=red:s=64x64:d=1[o];{}", "[o]null[o];".repeat(700));
        let mut args: Vec<String> = ["-i", "in.mp4", "-filter_complex", &long, "-map", "[o]"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!(spill_long_filter_graph(&mut args, &dir).unwrap());
        assert_eq!(args[2], "-/filter_complex");
        assert_eq!(
            fs::read_to_string(&args[3]).unwrap(),
            long,
            "graph text is unchanged"
        );
        assert_eq!(args.len(), 6, "only the graph argument was replaced");

        let mut short: Vec<String> = [
            "-filter_complex",
            "color=c=red:s=64x64:d=1[o]",
            "-map",
            "[o]",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let before = short.clone();
        assert!(!spill_long_filter_graph(&mut short, &dir).unwrap());
        assert_eq!(short, before);
        let _ = fs::remove_dir_all(&dir);
    }

    fn get_fixture_dir() -> PathBuf {
        let dir = std::env::temp_dir().join("tinixcut_test_fixtures_phase8");
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn default_profile_inherits_the_canonical_project_canvas() {
        let settings = ProjectSettings {
            width: 1080,
            height: 1920,
            canvas_background_color: "#000000".to_string(),
            frame_rate: FrameRate::FPS_60,
            sample_rate: 44_100,
            channels: 2,
        };

        let profile = ExportProfile::from_project_settings(&settings);

        assert_eq!(profile.width, 1080);
        assert_eq!(profile.height, 1920);
        assert_eq!(profile.frame_rate, FrameRate::FPS_60);
        assert_eq!(profile.audio_sample_rate, 44_100);
        assert!(profile.validate().is_ok());
    }

    #[test]
    fn export_preflight_binds_snapshot_and_reports_replacement_and_disk_space() {
        let directory = get_fixture_dir().join(format!("preflight_{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let output = directory.join("existing.mp4");
        fs::write(&output, b"prior output must remain untouched").unwrap();
        let project = Project::new("Preflight snapshot".to_string());
        let profile = ExportProfile::from_project_settings(&project.settings);

        let report = ExportEngine::preflight(&project, &output, &profile).unwrap();

        assert_eq!(report.project_id, project.project_id);
        assert_eq!(report.project_revision, project.revision);
        assert!(report.output_exists);
        assert!(report.available_disk_bytes > 0);
        assert!(report
            .warnings
            .iter()
            .any(|warning| warning.contains("replaced only after a successful render")));
        assert_eq!(
            fs::read(&output).unwrap(),
            b"prior output must remain untouched"
        );
        fs::remove_dir_all(directory).unwrap();
    }

    fn generate_synthetic_mp4(path: &Path, duration_secs: u32, color: &str) {
        if path.exists() {
            return;
        }
        if let Ok(mut cmd) = MediaBinaryResolver::command("ffmpeg") {
            let output = cmd
                .args([
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    &format!("color=c={}:s=640x360:r=30:d={}", color, duration_secs),
                    "-f",
                    "lavfi",
                    "-i",
                    &format!("sine=frequency=440:duration={}", duration_secs),
                    "-c:v",
                    "libx264",
                    "-pix_fmt",
                    "yuv420p",
                    "-c:a",
                    "aac",
                    &path.to_string_lossy(),
                ])
                .output();

            if let Ok(res) = output {
                if !res.status.success() {
                    eprintln!(
                        "FFmpeg synthetic generator warning: {}",
                        String::from_utf8_lossy(&res.stderr)
                    );
                }
            }
        }
    }

    fn generate_silent_synthetic_mp4(path: &Path) {
        if let Ok(mut cmd) = MediaBinaryResolver::command("ffmpeg") {
            let output = cmd
                .args([
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    "color=c=red:s=640x360:r=30:d=2",
                    "-c:v",
                    "libx264",
                    "-pix_fmt",
                    "yuv420p",
                    &path.to_string_lossy(),
                ])
                .output();
            assert!(output.is_ok_and(|result| result.status.success()));
        }
    }

    #[test]
    fn silent_video_export_does_not_reference_a_missing_audio_stream() {
        let test_dir = get_fixture_dir().join(format!("silent_video_{}", std::process::id()));
        fs::create_dir_all(&test_dir).unwrap();
        let source = test_dir.join("silent.mp4");
        let output = test_dir.join("silent_export.mp4");
        generate_silent_synthetic_mp4(&source);

        let mut project = Project::new("Silent video export".to_string());
        project.settings.width = 640;
        project.settings.height = 360;
        let asset_id = "silent-video".to_string();
        project.assets.push(Asset {
            asset_id: asset_id.clone(),
            name: "silent.mp4".to_string(),
            path: source.to_string_lossy().to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(60, FrameRate::FPS_30)),
            file_size: 100,
            dimensions: None,
        });
        let clip_range = TimeRange::new(
            RationalTime::new(0, FrameRate::FPS_30),
            RationalTime::new(60, FrameRate::FPS_30),
        );
        let mut track = Track::new("Video".to_string(), TrackKind::Video);
        track.items.push(
            TimelineItem::new(
                Some(asset_id),
                "Silent clip".to_string(),
                ItemKind::Video,
                clip_range,
                clip_range,
            )
            .unwrap(),
        );
        project.tracks.push(track);

        let profile = ExportProfile::from_project_settings(&project.settings);
        let mut progress = Vec::new();
        let report = ExportEngine::render_snapshot_with_progress(
            &project,
            &output,
            &profile,
            None,
            |value| progress.push(value),
        )
        .expect("silent video export should not require an audio stream");
        assert!(progress.windows(2).all(|pair| pair[0] <= pair[1]));
        assert_eq!(progress.last().copied(), Some(1.0));
        assert!(report.file_size_bytes > 0);
        let metadata = FFmpegService::probe_file(&output).expect("probe rendered MP4");
        assert!(metadata.video_codec.is_some());
        assert!(metadata.audio_codec.is_none());
        let decode = MediaBinaryResolver::command("ffmpeg")
            .expect("resolve packaged FFmpeg")
            .args(["-v", "error", "-i"])
            .arg(&output)
            .args(["-f", "null", "-"])
            .output()
            .expect("decode rendered MP4");
        assert!(
            decode.status.success(),
            "FFmpeg full decode failed: {}",
            String::from_utf8_lossy(&decode.stderr)
        );

        let _ = fs::remove_dir_all(test_dir);
    }

    #[test]
    fn test_export_profile_validation() {
        let mut profile = ExportProfile::default();
        assert!(profile.validate().is_ok());

        profile.video_codec = "prores".to_string();
        assert!(profile.validate().is_err());

        profile.video_codec = "libx264".to_string();
        profile.audio_codec = "mp3".to_string();
        assert!(profile.validate().is_err());

        profile.audio_codec = "aac".to_string();
        profile.width = 0;
        assert!(profile.validate().is_err());
    }

    #[test]
    fn social_export_presets_inherit_the_canonical_project_canvas() {
        let settings = ProjectSettings {
            width: 1080,
            height: 1920,
            canvas_background_color: "#000000".to_string(),
            frame_rate: FrameRate::FPS_60,
            sample_rate: 48000,
            channels: 2,
        };

        let balanced = ExportPreset::Balanced.profile_for(&settings);
        let high_quality = ExportPreset::HighQuality.profile_for(&settings);
        let smaller_file = ExportPreset::SmallerFile.profile_for(&settings);

        for profile in [&balanced, &high_quality, &smaller_file] {
            assert_eq!((profile.width, profile.height), (1080, 1920));
            assert_eq!(profile.frame_rate, FrameRate::FPS_60);
            assert_eq!(profile.audio_sample_rate, 48000);
            assert!(profile.validate_for_project(&settings).is_ok());
        }
        assert_eq!(balanced.crf, 23);
        assert_eq!(high_quality.crf, 18);
        assert_eq!(high_quality.audio_bitrate, "256k");
        assert_eq!(smaller_file.crf, 28);
        assert_eq!(smaller_file.audio_bitrate, "128k");
    }

    #[test]
    fn export_profile_rejects_a_canvas_or_frame_rate_that_cannot_be_rendered() {
        let settings = ProjectSettings {
            width: 1080,
            height: 1920,
            canvas_background_color: "#000000".to_string(),
            frame_rate: FrameRate::FPS_30,
            sample_rate: 48000,
            channels: 2,
        };
        let mut profile = ExportPreset::Balanced.profile_for(&settings);
        profile.width = 1920;
        assert!(profile.validate_for_project(&settings).is_err());

        profile.width = settings.width;
        profile.frame_rate = FrameRate::FPS_60;
        assert!(profile.validate_for_project(&settings).is_err());
    }

    #[test]
    fn test_export_snapshot_isolation_and_atomic_promotion() {
        let fixture_dir = get_fixture_dir();
        let test_dir = fixture_dir.join("isolation_sub");
        fs::create_dir_all(&test_dir).unwrap();
        let src_video = fixture_dir.join("input_source.mp4");
        generate_synthetic_mp4(&src_video, 3, "red");

        let mut project = Project::new("Export Test Project".to_string());
        project.settings = ProjectSettings {
            width: 640,
            height: 360,
            canvas_background_color: "#000000".to_string(),
            frame_rate: FrameRate::FPS_30,
            sample_rate: 48000,
            channels: 2,
        };

        let asset_id = "asset-src-1".to_string();
        project.assets.push(Asset {
            asset_id: asset_id.clone(),
            name: "input_source.mp4".to_string(),
            path: src_video.to_string_lossy().to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(90, FrameRate::FPS_30)),
            file_size: 1024,
            dimensions: None,
        });

        let mut track = Track::new("Video Track".to_string(), TrackKind::Video);
        let item = TimelineItem::new(
            Some(asset_id),
            "Clip 1".to_string(),
            ItemKind::Video,
            TimeRange::new(
                RationalTime::new(0, FrameRate::FPS_30),
                RationalTime::new(60, FrameRate::FPS_30), // 2.0 seconds
            ),
            TimeRange::new(
                RationalTime::new(0, FrameRate::FPS_30),
                RationalTime::new(60, FrameRate::FPS_30),
            ),
        )
        .unwrap();
        track.items.push(item);
        project.tracks.push(track);

        // 1. Take snapshot of project
        let frozen_snapshot = project.clone();

        // 2. Mutate active project in memory (simulating user continuing to edit)
        project.tracks.clear();
        project.name = "Mutated Project".to_string();

        let output_file = test_dir.join("exported_render.mp4");
        let _ = fs::remove_file(&output_file);

        let mut profile = ExportProfile::default();
        profile.width = 640;
        profile.height = 360;

        // 3. Render using frozen snapshot
        let report = ExportEngine::render_snapshot(&frozen_snapshot, &output_file, &profile, None)
            .expect("Render should succeed");

        // Verify output file exists and is promoted
        assert!(output_file.exists());
        assert_eq!(report.duration_secs, 2.0);
        assert!(report.file_size_bytes > 0);

        // Verify no temporary .tmp file left behind in test_dir
        let entries = fs::read_dir(&test_dir).unwrap();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            assert!(
                !name.contains(".tc_render.tmp"),
                "Temporary render file was not cleaned up: {}",
                name
            );
        }

        // 4. Repeat export to verify determinism
        let repeat_output = test_dir.join("repeat_render.mp4");
        let repeat_report =
            ExportEngine::render_snapshot(&frozen_snapshot, &repeat_output, &profile, None)
                .expect("Repeat render should succeed");

        assert_eq!(report.duration_secs, repeat_report.duration_secs);
        assert!(repeat_output.exists());
    }

    #[test]
    fn test_export_cancellation_safety_preserves_valid_output() {
        let fixture_dir = get_fixture_dir();
        let test_dir = fixture_dir.join("cancellation_sub");
        fs::create_dir_all(&test_dir).unwrap();
        let src_video = fixture_dir.join("input_source_cancel.mp4");
        generate_synthetic_mp4(&src_video, 4, "green");

        let mut project = Project::new("Cancel Test Project".to_string());
        project.settings = ProjectSettings {
            width: 640,
            height: 360,
            canvas_background_color: "#000000".to_string(),
            frame_rate: FrameRate::FPS_30,
            sample_rate: 48000,
            channels: 2,
        };

        let asset_id = "asset-cancel".to_string();
        project.assets.push(Asset {
            asset_id: asset_id.clone(),
            name: "input_source_cancel.mp4".to_string(),
            path: src_video.to_string_lossy().to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(120, FrameRate::FPS_30)),
            file_size: 1024,
            dimensions: None,
        });

        let mut track = Track::new("Video Track".to_string(), TrackKind::Video);
        let item = TimelineItem::new(
            Some(asset_id),
            "Clip 1".to_string(),
            ItemKind::Video,
            TimeRange::new(
                RationalTime::new(0, FrameRate::FPS_30),
                RationalTime::new(90, FrameRate::FPS_30),
            ),
            TimeRange::new(
                RationalTime::new(0, FrameRate::FPS_30),
                RationalTime::new(90, FrameRate::FPS_30),
            ),
        )
        .unwrap();
        track.items.push(item);
        project.tracks.push(track);

        let output_file = test_dir.join("cancelled_target.mp4");
        // Create an existing valid file at target_path
        fs::write(&output_file, b"PRE_EXISTING_VALID_CONTENT").unwrap();

        let cancel_flag = Arc::new(AtomicBool::new(false));
        // Flag set to cancel immediately
        cancel_flag.store(true, Ordering::SeqCst);

        let mut profile = ExportProfile::default();
        profile.width = 640;
        profile.height = 360;

        let result =
            ExportEngine::render_snapshot(&project, &output_file, &profile, Some(cancel_flag));

        // Verification of Gate requirements:
        // 1. Result must be Cancelled
        assert_eq!(result, Err(ExportError::Cancelled));

        // 2. Pre-existing valid file at target_path must be completely preserved!
        let content = fs::read(&output_file).unwrap();
        assert_eq!(content, b"PRE_EXISTING_VALID_CONTENT");

        // 3. No orphan .tmp file
        let entries = fs::read_dir(&test_dir).unwrap();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            assert!(
                !name.contains(".tc_render.tmp"),
                "Leftover temp file after cancellation: {}",
                name
            );
        }
    }

    /// POC 5 — Hardware vs Software Fallback qualification
    #[test]
    fn test_poc5_hardware_vs_software_fallback() {
        let fixture_dir = get_fixture_dir();
        let test_dir = fixture_dir.join("poc5_sub");
        fs::create_dir_all(&test_dir).unwrap();
        let src_video = fixture_dir.join("input_source_poc5.mp4");
        generate_synthetic_mp4(&src_video, 3, "magenta");

        let mut project = Project::new("POC 5 Fallback Project".to_string());
        project.settings = ProjectSettings {
            width: 640,
            height: 360,
            canvas_background_color: "#000000".to_string(),
            frame_rate: FrameRate::FPS_30,
            sample_rate: 48000,
            channels: 2,
        };

        let asset_id = "asset-poc5".to_string();
        project.assets.push(Asset {
            asset_id: asset_id.clone(),
            name: "input_source_poc5.mp4".to_string(),
            path: src_video.to_string_lossy().to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(90, FrameRate::FPS_30)),
            file_size: 1024,
            dimensions: None,
        });

        let mut track = Track::new("Video Track".to_string(), TrackKind::Video);
        let item = TimelineItem::new(
            Some(asset_id),
            "Clip 1".to_string(),
            ItemKind::Video,
            TimeRange::new(
                RationalTime::new(0, FrameRate::FPS_30),
                RationalTime::new(60, FrameRate::FPS_30),
            ),
            TimeRange::new(
                RationalTime::new(0, FrameRate::FPS_30),
                RationalTime::new(60, FrameRate::FPS_30),
            ),
        )
        .unwrap();
        track.items.push(item);
        project.tracks.push(track);

        let output_file = test_dir.join("poc5_fallback_render.mp4");
        let _ = fs::remove_file(&output_file);

        // Request hardware acceleration (e.g. h264_nvenc)
        let mut hw_profile = ExportProfile::default();
        hw_profile.width = 640;
        hw_profile.height = 360;
        hw_profile.video_codec = "h264_nvenc".to_string();

        // Render: Even if h264_nvenc is unavailable or GPU drivers are absent,
        // the engine must automatically fall back to software encoding (libx264)
        // and produce a valid output!
        let report = ExportEngine::render_snapshot(&project, &output_file, &hw_profile, None)
            .expect("Render with hardware fallback must succeed");

        assert!(output_file.exists());
        assert_eq!(report.duration_secs, 2.0);
        assert!(report.file_size_bytes > 0);
        assert!(report.encoder_used.is_some());
        assert!(!report.attempted_encoders.is_empty());
        assert_eq!(report.attempted_encoders[0], "h264_nvenc");

        let _ = fs::remove_file(output_file);
    }

    /// P12-AC-09: Automated test proving exact candidate ladder order NVENC -> QSV -> AMF -> libx264
    #[test]
    fn test_phase12_hardware_ladder_candidate_order() {
        let auto_ladder = ExportEngine::get_encoder_candidates("auto");
        assert_eq!(
            auto_ladder,
            vec!["h264_nvenc", "h264_qsv", "h264_amf", "libx264"],
            "ADR-002 requires exact ladder: NVENC -> QSV -> AMF -> libx264"
        );

        let nvenc_ladder = ExportEngine::get_encoder_candidates("h264_nvenc");
        assert_eq!(
            nvenc_ladder,
            vec!["h264_nvenc", "h264_qsv", "h264_amf", "libx264"]
        );

        let qsv_ladder = ExportEngine::get_encoder_candidates("h264_qsv");
        assert_eq!(qsv_ladder, vec!["h264_qsv", "h264_amf", "libx264"]);

        let amf_ladder = ExportEngine::get_encoder_candidates("h264_amf");
        assert_eq!(amf_ladder, vec!["h264_amf", "libx264"]);

        let sw_ladder = ExportEngine::get_encoder_candidates("libx264");
        assert_eq!(sw_ladder, vec!["libx264"]);
    }

    /// P12-AC-10: Automated test proving only encoder/device initialization unavailability triggers fallback
    #[test]
    fn test_phase12_hardware_error_discrimination() {
        // Genuine hardware/driver errors
        assert!(ExportEngine::is_hardware_encoder_error(
            "Cannot load nvcuda.dll",
            "h264_nvenc"
        ));
        assert!(ExportEngine::is_hardware_encoder_error(
            "No NVENC capable devices found",
            "h264_nvenc"
        ));
        assert!(ExportEngine::is_hardware_encoder_error(
            "Device creation failed",
            "h264_qsv"
        ));
        assert!(ExportEngine::is_hardware_encoder_error(
            "Error creating AMF context",
            "h264_amf"
        ));
        assert!(ExportEngine::is_hardware_encoder_error(
            "Unknown encoder 'h264_nvenc'",
            "h264_nvenc"
        ));

        // Software baseline never triggers hardware fallback
        assert!(!ExportEngine::is_hardware_encoder_error(
            "Error initializing output stream",
            "libx264"
        ));

        // Non-hardware I/O or syntax errors must NOT trigger fallback
        assert!(!ExportEngine::is_hardware_encoder_error(
            "No such file or directory: input.mp4",
            "h264_nvenc"
        ));
        assert!(!ExportEngine::is_hardware_encoder_error(
            "Permission denied when opening destination",
            "h264_nvenc"
        ));
        assert!(!ExportEngine::is_hardware_encoder_error(
            "No space left on device",
            "h264_qsv"
        ));
    }

    #[test]
    fn test_phase57_disk_full_error_is_actionable_and_keeps_encoder_detail() {
        let message = ExportEngine::render_failure_message(
            "h264_qsv",
            "Error writing trailer: No space left on device",
        );

        assert!(message.starts_with("Insufficient disk space to finish the export."));
        assert!(message.contains("Free up space and retry."));
        assert!(message.contains("No space left on device"));

        let generic = ExportEngine::render_failure_message("libx264", "invalid filter graph");
        assert!(generic.starts_with("FFmpeg render process failed with encoder 'libx264'"));
    }
}
