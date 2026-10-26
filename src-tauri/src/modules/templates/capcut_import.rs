//! Phase 34 — CapCut Template Inspection & Conversion Engine
//!
//! Provides read-only inspection, budget enforcement, resource resolution,
//! family profile detection, effective style/animation mapping, and atomic commit paths
//! for converting CapCut desktop project templates into Tinix native templates.
//!
//! Architectural Invariants:
//! - Strictly read-only inspect: no CapCut runtime, no Lua/JS execution, no mouse automation.
//! - Bounded budgets: streaming reads, directory depth/entry limits, zip expansion limits, cancellation propagation.
//! - Clear report statuses: FULL_CONVERSION, PARTIAL_CONVERSION, UNSUPPORTED_SOURCE,
//!   MISSING_RESOURCES, BAKED_MEDIA_ONLY.
//! - Single-owner motion: animation assigned to exactly one owner (never both layer.motion and text_animation).
//! - Stale revision protection & transactional catalog commit with rollback.
//! - Reference video (MP4) is diagnostics / baked media only; never editable template fallback.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::modules::command::{
    CommandEnvelope, CommandOperation, CommandReceipt, CommandService, OperationEnvelope,
};
use crate::modules::domain::id::is_valid_id;
use crate::modules::domain::template::{
    is_safe_resource_filename, ResponsivePolicy, TemplateDefinition, TemplateInstance,
    TemplateLayer, TemplateLayerKind, TemplateParameterBinding, TemplateParameterDefinition,
    TemplateParameterType, TemplateParameterValue, TemplatePropertyPath, TemplateResourceKind,
    TemplateResourceManifest, TemplateResourceReference, TemplateShapeKind, TemplateShapeLayerData,
    TemplateTextLayerData,
};
use crate::modules::domain::text::{
    grapheme_count, AnimationTransform, AnimationUnit, DurationConflictPolicy, FontReference,
    FontStyle, GradientStop, LoopMode, RichTextSpan, StaggerOrder, StrokeAlign, TextAnimation,
    TextAnimationSegment, TextFill, TextLayout, TextRichStyle, TextShadow, TextSpanStyle,
    TextStroke,
};
use crate::modules::domain::time::{FrameRate, RationalTime};
use crate::modules::domain::{
    KeyframeEasing, TextAlignment, TextAnchor, TextStyle, Transform, CURRENT_SCHEMA_VERSION,
};
use crate::modules::history::{HistoryEntry, HistoryManager};
use crate::modules::templates::catalog::TemplateCatalog;
use crate::modules::templates::curated::resolve_curated_definition;

/// Maximum size allowed for draft JSON content (50 MB).
pub const MAX_DRAFT_JSON_BYTES: u64 = 50 * 1024 * 1024;

/// Maximum size allowed for effectStyle.json (5 MB).
pub const MAX_EFFECT_STYLE_BYTES: u64 = 5 * 1024 * 1024;

/// Maximum entries allowed in a template ZIP archive.
pub const MAX_ZIP_ENTRIES: usize = 1000;

/// Maximum single uncompressed entry size in a template ZIP archive (50 MB).
pub const MAX_ZIP_SINGLE_ENTRY_BYTES: u64 = 50 * 1024 * 1024;

/// Maximum total uncompressed bytes allowed in a template ZIP archive (250 MB).
pub const MAX_ZIP_UNCOMPRESSED_BYTES: u64 = 250 * 1024 * 1024;

/// Maximum allowed ZIP compression ratio (zip bomb protection).
pub const MAX_ZIP_EXPANSION_RATIO: u64 = 50;

/// Maximum directory traversal depth when discovering draft files.
pub const MAX_DIR_SCAN_DEPTH: usize = 8;

/// Maximum total entries scanned during directory traversal.
pub const MAX_DIR_SCAN_ENTRIES: usize = 2000;

/// Maximum bytes read during streaming hash computation (500 MB).
pub const MAX_STREAM_HASH_BYTES: u64 = 500 * 1024 * 1024;

/// Chunk size for bounded streaming reads.
pub const HASH_CHUNK_SIZE: usize = 64 * 1024;

/// Maximum aggregate inspection hash/read budget across entire inspection (500 MB).
pub const MAX_INSPECTION_READ_BYTES: u64 = 500 * 1024 * 1024;

/// Maximum number of active staged ZIP directories retained in temp cache.
pub const MAX_STAGING_SESSIONS: usize = 5;

/// Maximum total aggregate cache size for staged ZIP directories (200 MB).
pub const MAX_STAGING_CACHE_BYTES: u64 = 200 * 1024 * 1024;

/// Known animation resource ID for the sample family.
pub const KNOWN_SAMPLE_ANIM_RESOURCE_ID: &str = "7498202895196736769";

/// Known wrapper resource ID for the sample family.
pub const KNOWN_SAMPLE_WRAPPER_RESOURCE_ID: &str = "7522343677012774197";

/// Known SHA-256 hash of the Lua AnimScript in the sample family (resource 7498202895196736769).
pub const KNOWN_SAMPLE_LUA_SHA256: &str =
    "C9C796C46A4258B129DFA28484E2A1382FE1710BA787023E8B1B4DC51822F518";

static INSPECTION_TOKENS: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();

fn get_tokens_map() -> &'static Mutex<HashMap<String, Arc<AtomicBool>>> {
    INSPECTION_TOKENS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Creates a new inspection cancellation token registered in the process.
pub fn create_inspection_cancellation_token() -> (String, Arc<AtomicBool>) {
    let token = format!("capcut_inspect_{}", uuid::Uuid::new_v4());
    let flag = Arc::new(AtomicBool::new(false));
    let mut map = get_tokens_map().lock().unwrap();
    map.insert(token.clone(), flag.clone());
    (token, flag)
}

/// Retrieves an active cancellation token if present.
pub fn get_inspection_cancellation_token(token: &str) -> Option<Arc<AtomicBool>> {
    let map = get_tokens_map().lock().unwrap();
    map.get(token).cloned()
}

/// Sets the cancellation flag for a given token. Returns true if found.
pub fn cancel_inspection(token: &str) -> bool {
    let map = get_tokens_map().lock().unwrap();
    if let Some(flag) = map.get(token) {
        flag.store(true, Ordering::SeqCst);
        true
    } else {
        false
    }
}

/// Unregisters a cancellation token after inspection finishes.
pub fn remove_inspection_cancellation_token(token: &str) {
    let mut map = get_tokens_map().lock().unwrap();
    map.remove(token);
}

/// Registry record for a staged template conversion.
#[derive(Debug, Clone)]
pub struct StagedConversionRecord {
    pub token: String,
    pub source_path: PathBuf,
    pub source_sha256: String,
    pub resource_fingerprints: HashMap<PathBuf, String>,
    pub scratch_dir: Option<PathBuf>,
    pub created_at: std::time::Instant,
    pub last_receipt: Option<CommandReceipt>,
    pub committed_project_revision: Option<u64>,
    pub committed_project_id: Option<String>,
    pub committed_request_fingerprint: Option<String>,
}

static STAGING_REGISTRY: OnceLock<Mutex<HashMap<String, StagedConversionRecord>>> = OnceLock::new();

fn get_staging_registry() -> &'static Mutex<HashMap<String, StagedConversionRecord>> {
    STAGING_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Registers a staged template conversion record and returns its token.
pub fn register_staged_conversion(
    source_path: PathBuf,
    source_sha256: String,
    resource_fingerprints: HashMap<PathBuf, String>,
    scratch_dir: Option<PathBuf>,
) -> String {
    let token = format!("capcut_stage_{}", uuid::Uuid::new_v4());
    let record = StagedConversionRecord {
        token: token.clone(),
        source_path,
        source_sha256,
        resource_fingerprints,
        scratch_dir,
        created_at: std::time::Instant::now(),
        last_receipt: None,
        committed_project_revision: None,
        committed_project_id: None,
        committed_request_fingerprint: None,
    };
    let mut reg = get_staging_registry().lock().unwrap();
    reg.retain(|_, rec| rec.created_at.elapsed() <= std::time::Duration::from_secs(300));
    reg.insert(token.clone(), record);
    token
}

/// Retrieves a staged conversion record if valid and unexpired.
pub fn get_staged_conversion(token: &str) -> Option<StagedConversionRecord> {
    let reg = get_staging_registry().lock().unwrap();
    reg.get(token).cloned()
}

/// Removes a staged conversion record and cleans up any associated scratch dir.
pub fn remove_staged_conversion(token: &str) {
    let mut reg = get_staging_registry().lock().unwrap();
    if let Some(record) = reg.remove(token) {
        if let Some(scratch) = record.scratch_dir {
            if scratch.exists() {
                let _ = fs::remove_dir_all(scratch);
            }
        }
    }
}

/// Verifies whether the CapCut draft version string belongs to the supported major series.
pub fn is_supported_draft_version(version: Option<&str>) -> bool {
    if let Some(v) = version {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            return false;
        }
        if let Some(major_str) = trimmed.split('.').next() {
            if let Ok(major) = major_str.parse::<u32>() {
                return (6..=16).contains(&major) || major == 185;
            }
        }
    }
    false
}

#[derive(Debug, thiserror::Error)]
pub enum CapCutImportError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON deserialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Budget exceeded: {0}")]
    BudgetExceeded(String),

    #[error("Security violation: {0}")]
    SecurityViolation(String),

    #[error("Unsupported source: {0}")]
    UnsupportedSource(String),

    #[error("Missing resources: {0:?}")]
    MissingResources(Vec<String>),

    #[error("Stale source revision: expected SHA-256 {expected}, got {actual}")]
    StaleSourceRevision { expected: String, actual: String },

    #[error("Stale revision: {0}")]
    StaleRevision(String),

    #[error("Command execution error: {0}")]
    Command(String),

    #[error("Operation cancelled by caller")]
    Cancelled,

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Zip archive error: {0}")]
    Zip(String),
}

/// Conversion compatibility status per Plan 34.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CapCutConversionStatus {
    FullConversion,
    PartialConversion,
    UnsupportedSource,
    MissingResources,
    BakedMediaOnly,
}

impl std::fmt::Display for CapCutConversionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FullConversion => write!(f, "FULL_CONVERSION"),
            Self::PartialConversion => write!(f, "PARTIAL_CONVERSION"),
            Self::UnsupportedSource => write!(f, "UNSUPPORTED_SOURCE"),
            Self::MissingResources => write!(f, "MISSING_RESOURCES"),
            Self::BakedMediaOnly => write!(f, "BAKED_MEDIA_ONLY"),
        }
    }
}

/// Resource availability lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResourceAvailabilityStatus {
    Available,
    Missing,
    Corrupt,
    Relocated,
}

/// Diagnostic metadata for a single referenced resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapCutResourceDiagnostic {
    pub resource_id: String,
    pub panel: String,
    pub original_path: String,
    pub resolved_path: Option<String>,
    pub status: ResourceAvailabilityStatus,
    pub sha256: Option<String>,
    pub byte_size: Option<u64>,
}

/// Diagnostic metadata for reference baked video.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapCutReferenceVideoDiagnostic {
    pub path: String,
    pub sha256: Option<String>,
    pub found: bool,
    pub note: String,
}

/// Detail itemizing exact loss or approximation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapCutLossDetail {
    pub component: String,
    pub original: String,
    pub treatment: String,
    pub impact: String,
}

/// Recognized template families.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CapCutFamilyProfile {
    StaggeredCharacterIn,
    VlogTitleMultiLayer,
    TextWithBackgroundCard,
    Unknown,
}

/// Extracted source profile metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapCutSourceProfile {
    pub draft_path: String,
    pub draft_sha256: String,
    pub app_version: Option<String>,
    pub fps: f64,
    pub duration_us: i64,
    pub duration_seconds: f64,
    pub canvas_width: u32,
    pub canvas_height: u32,
    pub template_name: Option<String>,
    pub template_id: Option<String>,
    pub template_resource_id: Option<String>,
    pub animation_resource_id: Option<String>,
    pub animation_duration_us: Option<i64>,
    pub animation_sha256: Option<String>,
    pub effect_style_resource_id: Option<String>,
    pub effect_style_path: Option<String>,
    pub font_resource_id: Option<String>,
    pub font_name: Option<String>,
    pub font_path: Option<String>,
    pub text_content: Option<String>,
    pub layers_count: usize,
    pub resources: Vec<CapCutResourceDiagnostic>,
    pub missing_resources: Vec<String>,
    pub reference_video: Option<CapCutReferenceVideoDiagnostic>,
    pub recognized_family: CapCutFamilyProfile,
}

/// Full conversion inspection and report payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapCutConversionReport {
    pub status: CapCutConversionStatus,
    pub source_path: String,
    pub source_sha256: String,
    pub template_name: String,
    pub recognized_profile: String,
    pub imported_elements: Vec<String>,
    pub loss_details: Vec<CapCutLossDetail>,
    pub warnings: Vec<String>,
    pub converted_definition: Option<TemplateDefinition>,
    pub preview_instance: Option<TemplateInstance>,
    pub reference_video: Option<CapCutReferenceVideoDiagnostic>,
    #[serde(default)]
    pub staging_token: Option<String>,
}

/// Top-level result combining source profile and conversion report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapCutInspectionResult {
    pub profile: CapCutSourceProfile,
    pub report: CapCutConversionReport,
    pub converted_definition: Option<TemplateDefinition>,
}

// =========================================================================
// Raw Deserialization Types for CapCut Draft JSON
// =========================================================================

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawCapCutDraft {
    id: Option<String>,
    version: Option<serde_json::Value>,
    new_version: Option<String>,
    name: Option<String>,
    duration: Option<i64>,
    fps: Option<f64>,
    canvas_config: Option<RawCanvasConfig>,
    tracks: Option<Vec<RawTrack>>,
    materials: Option<RawMaterials>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawCanvasConfig {
    width: Option<u32>,
    height: Option<u32>,
    ratio: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawMaterials {
    texts: Option<Vec<RawTextMaterial>>,
    text_templates: Option<Vec<RawTextTemplate>>,
    material_animations: Option<Vec<RawMaterialAnimation>>,
    effects: Option<Vec<RawEffect>>,
    videos: Option<Vec<RawVideoMaterial>>,
    images: Option<Vec<serde_json::Value>>,
    stickers: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawTextMaterial {
    id: String,
    name: Option<String>,
    content: Option<String>,
    background_color: Option<String>,
    background_round_radius: Option<f32>,
    background_height: Option<f32>,
    background_width: Option<f32>,
    background_style: Option<i32>,
    background_alpha: Option<f32>,
    border_color: Option<String>,
    border_width: Option<f32>,
    alignment: Option<i32>,
    font_size: Option<f32>,
    font_name: Option<String>,
    font_path: Option<String>,
    fonts: Option<Vec<RawFontEntry>>,
    underline: Option<bool>,
    text_curve: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawFontEntry {
    id: Option<String>,
    resource_id: Option<String>,
    path: Option<String>,
    title: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawParsedTextContent {
    text: String,
    styles: Option<Vec<RawParsedTextStyle>>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawParsedTextStyle {
    fill: Option<RawParsedFill>,
    font: Option<RawParsedFont>,
    size: Option<f32>,
    #[serde(rename = "effectStyle")]
    effect_style: Option<RawParsedEffectStyleRef>,
    range: Option<Vec<usize>>,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawParsedFill {
    alpha: Option<f32>,
    render_type: Option<String>,
    solid: Option<RawSolidColor>,
    content: Option<RawParsedFillContent>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawParsedFillContent {
    render_type: Option<String>,
    gradient: Option<RawGradient>,
    solid: Option<RawSolidColor>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawGradient {
    angle: Option<f32>,
    color: Option<Vec<[f64; 3]>>,
    alpha: Option<Vec<f64>>,
    percent: Option<Vec<f64>>,
    mode: Option<String>,
    style: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawSolidColor {
    alpha: Option<f32>,
    color: Option<[f64; 3]>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawParsedFont {
    id: Option<String>,
    path: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawParsedEffectStyleRef {
    id: Option<String>,
    path: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawTextTemplate {
    id: String,
    name: Option<String>,
    resource_id: Option<String>,
    effect_id: Option<String>,
    path: Option<String>,
    resources: Option<Vec<RawTemplateResource>>,
    text_info_resources: Option<Vec<RawTextInfoResource>>,
    non_text_info_resources: Option<Vec<RawNonTextInfoResource>>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawTemplateResource {
    panel: Option<String>,
    path: Option<String>,
    resource_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawTextInfoResource {
    id: Option<String>,
    text_material_id: Option<String>,
    extra_material_refs: Option<Vec<String>>,
    attach_info: Option<RawAttachInfo>,
    lyric_keyframes: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawNonTextInfoResource {
    id: Option<String>,
    resource_id: Option<String>,
    attach_info: Option<RawAttachInfo>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawAttachInfo {
    clip: Option<RawClip>,
    duration: Option<i64>,
    start_time: Option<i64>,
    original_size_width: Option<f32>,
    original_size_height: Option<f32>,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
struct RawClip {
    alpha: Option<f32>,
    rotation: Option<f32>,
    scale: Option<RawScale>,
    transform: Option<RawTransform>,
    flip: Option<RawFlip>,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
struct RawFlip {
    vertical: Option<bool>,
    horizontal: Option<bool>,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
struct RawScale {
    x: Option<f32>,
    y: Option<f32>,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
struct RawTransform {
    x: Option<f32>,
    y: Option<f32>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawMaterialAnimation {
    id: String,
    animations: Option<Vec<RawAnimationItem>>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawAnimationItem {
    id: Option<String>,
    #[serde(rename = "type")]
    anim_type: Option<String>,
    duration: Option<i64>,
    start: Option<i64>,
    path: Option<String>,
    resource_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawEffect {
    id: String,
    resource_id: Option<String>,
    path: Option<String>,
    #[serde(rename = "type")]
    effect_type: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawVideoMaterial {
    id: String,
    path: Option<String>,
    duration: Option<i64>,
    width: Option<u32>,
    height: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawTrack {
    id: String,
    #[serde(rename = "type")]
    track_type: Option<String>,
    segments: Option<Vec<RawTrackSegment>>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawTrackSegment {
    id: String,
    material_id: Option<String>,
    extra_material_refs: Option<Vec<String>>,
    target_timerange: Option<RawTimeRange>,
    clip: Option<RawClip>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawTimeRange {
    start: Option<i64>,
    duration: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawEffectStyleFile {
    fill: Option<RawParsedFill>,
    strokes: Option<Vec<RawEffectStroke>>,
    shadows: Option<Vec<RawEffectShadow>>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawEffectStroke {
    alpha: Option<f32>,
    enable: Option<bool>,
    width: Option<f32>,
    render_type: Option<String>,
    solid: Option<RawSolidColor>,
    content: Option<RawParsedFillContent>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawEffectShadow {
    alpha: Option<f32>,
    angle: Option<f32>,
    color: Option<String>,
    diffuse: Option<f32>,
    distance: Option<f32>,
    enable: Option<bool>,
    render_type: Option<String>,
    solid: Option<RawSolidColor>,
    content: Option<RawParsedFillContent>,
}

// =========================================================================
// Hash & Utility Helpers
// =========================================================================

pub fn compute_sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let result = hasher.finalize();
    result.iter().map(|b| format!("{b:02X}")).collect()
}

/// Streams SHA-256 calculation with byte budget, chunked reads, and cancellation checks.
pub fn compute_file_sha256_bounded(
    path: &Path,
    max_bytes: u64,
    cancel: Option<&AtomicBool>,
) -> Result<String, CapCutImportError> {
    if let Some(c) = cancel {
        if c.load(Ordering::Relaxed) {
            return Err(CapCutImportError::Cancelled);
        }
    }
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    if metadata.len() > max_bytes {
        return Err(CapCutImportError::BudgetExceeded(format!(
            "File '{}' size ({} bytes) exceeds maximum budget ({} bytes)",
            path.display(),
            metadata.len(),
            max_bytes
        )));
    }

    let mut reader = std::io::BufReader::new(file.take(max_bytes + 1));
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; HASH_CHUNK_SIZE];
    let mut total_read = 0u64;

    loop {
        if let Some(c) = cancel {
            if c.load(Ordering::Relaxed) {
                return Err(CapCutImportError::Cancelled);
            }
        }
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        total_read += n as u64;
        if total_read > max_bytes {
            return Err(CapCutImportError::BudgetExceeded(format!(
                "File '{}' streaming read exceeded maximum budget ({} bytes)",
                path.display(),
                max_bytes
            )));
        }
        hasher.update(&buffer[..n]);
    }

    let result = hasher.finalize();
    Ok(result.iter().map(|b| format!("{b:02X}")).collect())
}

pub fn compute_file_sha256(path: &Path) -> Result<String, CapCutImportError> {
    compute_file_sha256_bounded(path, MAX_STREAM_HASH_BYTES, None)
}

/// Computes SHA-256 with per-file byte limit and global aggregate inspection read limit.
pub fn compute_file_sha256_metered(
    path: &Path,
    max_file_bytes: u64,
    aggregate_counter: &std::sync::atomic::AtomicU64,
    max_aggregate_bytes: u64,
    cancel: Option<&AtomicBool>,
) -> Result<String, CapCutImportError> {
    if let Some(c) = cancel {
        if c.load(Ordering::Relaxed) {
            return Err(CapCutImportError::Cancelled);
        }
    }
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; HASH_CHUNK_SIZE];
    let mut bytes_read = 0u64;

    loop {
        if let Some(c) = cancel {
            if c.load(Ordering::Relaxed) {
                return Err(CapCutImportError::Cancelled);
            }
        }
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        bytes_read = bytes_read.saturating_add(n as u64);
        if bytes_read > max_file_bytes {
            return Err(CapCutImportError::BudgetExceeded(format!(
                "File '{}' size exceeded stream limit of {max_file_bytes} bytes",
                path.display()
            )));
        }

        let prev = aggregate_counter.fetch_add(n as u64, Ordering::Relaxed);
        if prev.saturating_add(n as u64) > max_aggregate_bytes {
            return Err(CapCutImportError::BudgetExceeded(format!(
                "Aggregate inspection read budget exceeded limit of {max_aggregate_bytes} bytes"
            )));
        }

        hasher.update(&buffer[..n]);
    }

    if let Some(c) = cancel {
        if c.load(Ordering::Relaxed) {
            return Err(CapCutImportError::Cancelled);
        }
    }

    let result = hasher.finalize();
    Ok(result.iter().map(|b| format!("{b:02X}")).collect())
}

/// Reads file bytes with per-file byte limit and global aggregate inspection read limit.
pub fn read_file_bytes_metered(
    path: &Path,
    max_file_bytes: u64,
    aggregate_counter: &std::sync::atomic::AtomicU64,
    max_aggregate_bytes: u64,
    cancel: Option<&AtomicBool>,
) -> Result<Vec<u8>, CapCutImportError> {
    if let Some(c) = cancel {
        if c.load(Ordering::Relaxed) {
            return Err(CapCutImportError::Cancelled);
        }
    }
    let mut file = File::open(path)?;
    let mut buf = Vec::new();
    let mut buffer = [0u8; 16384];

    loop {
        if let Some(c) = cancel {
            if c.load(Ordering::Relaxed) {
                return Err(CapCutImportError::Cancelled);
            }
        }
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        if (buf.len() + n) as u64 > max_file_bytes {
            return Err(CapCutImportError::BudgetExceeded(format!(
                "File '{}' size exceeded limit of {max_file_bytes} bytes",
                path.display()
            )));
        }
        let prev = aggregate_counter.fetch_add(n as u64, Ordering::Relaxed);
        if prev.saturating_add(n as u64) > max_aggregate_bytes {
            return Err(CapCutImportError::BudgetExceeded(format!(
                "Aggregate inspection read budget exceeded limit of {max_aggregate_bytes} bytes"
            )));
        }
        buf.extend_from_slice(&buffer[..n]);
    }

    if let Some(c) = cancel {
        if c.load(Ordering::Relaxed) {
            return Err(CapCutImportError::Cancelled);
        }
    }

    Ok(buf)
}

/// Reads file bytes with strict size budget and cancellation checks.
pub fn read_file_bytes_bounded(
    path: &Path,
    max_bytes: u64,
    cancel: Option<&AtomicBool>,
) -> Result<Vec<u8>, CapCutImportError> {
    if let Some(c) = cancel {
        if c.load(Ordering::Relaxed) {
            return Err(CapCutImportError::Cancelled);
        }
    }
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    if metadata.len() > max_bytes {
        return Err(CapCutImportError::BudgetExceeded(format!(
            "File '{}' size ({} bytes) exceeds budget of {max_bytes} bytes",
            path.display(),
            metadata.len()
        )));
    }
    let mut reader = std::io::BufReader::new(file.take(max_bytes + 1));
    let mut bytes = Vec::with_capacity(metadata.len().min(max_bytes) as usize);
    let mut buffer = [0u8; 16384];
    loop {
        if let Some(c) = cancel {
            if c.load(Ordering::Relaxed) {
                return Err(CapCutImportError::Cancelled);
            }
        }
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        if (bytes.len() + n) as u64 > max_bytes {
            return Err(CapCutImportError::BudgetExceeded(format!(
                "File '{}' exceeded read budget of {max_bytes} bytes",
                path.display()
            )));
        }
        bytes.extend_from_slice(&buffer[..n]);
    }
    Ok(bytes)
}

/// RAII guard for temporary scratch directory that deletes directory on Drop unless disarmed.
pub struct ScratchDirGuard {
    pub path: PathBuf,
    pub active: bool,
}

impl ScratchDirGuard {
    pub fn new(path: PathBuf) -> Self {
        Self { path, active: true }
    }
    pub fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for ScratchDirGuard {
    fn drop(&mut self) {
        if self.active && self.path.exists() {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

static STAGED_ZIP_DIRS: OnceLock<Mutex<HashMap<PathBuf, std::time::Instant>>> = OnceLock::new();

fn get_staged_dirs() -> &'static Mutex<HashMap<PathBuf, std::time::Instant>> {
    STAGED_ZIP_DIRS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Registers an active extracted staging directory.
pub fn register_staged_dir(path: PathBuf) {
    if let Ok(mut map) = get_staged_dirs().lock() {
        map.insert(path, std::time::Instant::now());
    }
}

/// Explicitly cleans up and unregisters an extracted staging directory.
pub fn cleanup_staged_dir(path: &Path) {
    if let Ok(mut map) = get_staged_dirs().lock() {
        map.remove(path);
    }
    if path.exists() {
        let _ = fs::remove_dir_all(path);
    }
}

/// Cleans up any staged directories older than `max_age`.
pub fn cleanup_expired_staged_dirs(max_age: std::time::Duration) {
    let now = std::time::Instant::now();
    let mut to_remove = Vec::new();
    if let Ok(mut map) = get_staged_dirs().lock() {
        for (path, created) in map.iter() {
            if now.duration_since(*created) >= max_age {
                to_remove.push(path.clone());
            }
        }
        for p in &to_remove {
            map.remove(p);
        }
    }
    for p in to_remove {
        if p.exists() {
            let _ = fs::remove_dir_all(&p);
        }
    }
    enforce_staging_cache_quota(MAX_STAGING_CACHE_BYTES, MAX_STAGING_SESSIONS);
}

/// Enforces cache quota for extracted ZIP staging directories.
pub fn enforce_staging_cache_quota(max_bytes: u64, max_dirs: usize) {
    let tmp = std::env::temp_dir();
    if let Ok(entries) = fs::read_dir(&tmp) {
        let mut staged_dirs: Vec<(PathBuf, std::time::SystemTime, u64)> = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("tinix_capcut_zip_") {
                        let mtime = entry
                            .metadata()
                            .and_then(|m| m.modified())
                            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                        let dir_size = dir_size_bounded(&path, 500 * 1024 * 1024);
                        staged_dirs.push((path, mtime, dir_size));
                    }
                }
            }
        }
        staged_dirs.sort_by_key(|(_, mtime, _)| *mtime);

        let mut total_bytes: u64 = staged_dirs.iter().map(|(_, _, sz)| *sz).sum();
        let mut total_count = staged_dirs.len();

        for (dir, _, sz) in staged_dirs {
            if total_count <= max_dirs && total_bytes <= max_bytes {
                break;
            }
            let _ = fs::remove_dir_all(&dir);
            total_count = total_count.saturating_sub(1);
            total_bytes = total_bytes.saturating_sub(sz);
        }
    }
}

fn dir_size_bounded(dir: &Path, max_bytes: u64) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                if let Ok(m) = p.metadata() {
                    total = total.saturating_add(m.len());
                }
            } else if p.is_dir() {
                total = total.saturating_add(dir_size_bounded(&p, max_bytes));
            }
            if total > max_bytes {
                break;
            }
        }
    }
    total
}

fn rgb_to_hex(rgb: [f64; 3]) -> String {
    let r = (rgb[0] * 255.0).round().clamp(0.0, 255.0) as u8;
    let g = (rgb[1] * 255.0).round().clamp(0.0, 255.0) as u8;
    let b = (rgb[2] * 255.0).round().clamp(0.0, 255.0) as u8;
    format!("#{r:02X}{g:02X}{b:02X}")
}

fn rgba_to_hex(rgb: [f64; 3], alpha: f64) -> String {
    let r = (rgb[0] * 255.0).round().clamp(0.0, 255.0) as u8;
    let g = (rgb[1] * 255.0).round().clamp(0.0, 255.0) as u8;
    let b = (rgb[2] * 255.0).round().clamp(0.0, 255.0) as u8;
    let a = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
}

/// Helper to collect approved roots for resource resolution: draft_dir, selected_roots, and local CapCut cache if existing.
pub fn get_approved_roots(draft_dir: &Path, selected_roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(canon) = draft_dir.canonicalize() {
        roots.push(canon);
    } else {
        roots.push(draft_dir.to_path_buf());
    }
    for r in selected_roots {
        if let Ok(canon) = r.canonicalize() {
            roots.push(canon);
        } else {
            roots.push(r.clone());
        }
    }
    // Also include standard CapCut local cache if present on the system
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let capcut_cache = PathBuf::from(local_app_data).join("CapCut");
        if capcut_cache.is_dir() {
            if let Ok(canon) = capcut_cache.canonicalize() {
                roots.push(canon);
            } else {
                roots.push(capcut_cache);
            }
        }
    }
    roots
}

/// Normalizes path string and resolves against draft directory or selected roots.
pub fn resolve_resource_path(
    raw_path: &str,
    draft_dir: &Path,
    selected_roots: &[PathBuf],
) -> (Option<PathBuf>, ResourceAvailabilityStatus) {
    resolve_resource_path_bounded(raw_path, draft_dir, selected_roots, None)
        .unwrap_or((None, ResourceAvailabilityStatus::Missing))
}

/// Normalizes path string and resolves against draft directory or selected roots with cancellation and scan budgets.
/// Rejects UNC host paths, directory traversal, arbitrary host file access, and respects cancellation.
/// Cancellation propagates as `Err(CapCutImportError::Cancelled)` rather than being masked as `Missing`.
pub fn resolve_resource_path_bounded(
    raw_path: &str,
    draft_dir: &Path,
    selected_roots: &[PathBuf],
    cancel: Option<&AtomicBool>,
) -> Result<(Option<PathBuf>, ResourceAvailabilityStatus), CapCutImportError> {
    if let Some(c) = cancel {
        if c.load(Ordering::Relaxed) {
            return Err(CapCutImportError::Cancelled);
        }
    }

    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return Ok((None, ResourceAvailabilityStatus::Missing));
    }

    // Reject UNC / network host paths (e.g. \\host\share or //host/share)
    if trimmed.starts_with(r"\\") || trimmed.starts_with("//") {
        return Ok((None, ResourceAvailabilityStatus::Missing));
    }

    // Reject explicit directory traversal sequences
    if trimmed.contains("..") {
        return Ok((None, ResourceAvailabilityStatus::Missing));
    }

    let normalized = trimmed.replace('\\', "/");
    let candidate_path = PathBuf::from(&normalized);

    let approved_roots = get_approved_roots(draft_dir, selected_roots);

    let is_within_approved_roots = |p: &Path| -> bool {
        if let Ok(canon) = p.canonicalize() {
            approved_roots.iter().any(|root| canon.starts_with(root))
        } else {
            approved_roots.iter().any(|root| p.starts_with(root))
        }
    };

    // 1. Direct path exists - only allowed if within approved roots
    if (candidate_path.is_file() || candidate_path.is_dir())
        && is_within_approved_roots(&candidate_path)
    {
        return Ok((Some(candidate_path), ResourceAvailabilityStatus::Available));
    }

    // 2. Relative to draft_dir
    let rel_in_draft = draft_dir.join(&normalized);
    if (rel_in_draft.is_file() || rel_in_draft.is_dir()) && is_within_approved_roots(&rel_in_draft)
    {
        return Ok((Some(rel_in_draft), ResourceAvailabilityStatus::Relocated));
    }

    // 3. Match filename in draft_dir or selected_roots (bounded scan with cancellation)
    if let Some(file_name) = candidate_path.file_name() {
        let directly_in_draft = draft_dir.join(file_name);
        if directly_in_draft.is_file() || directly_in_draft.is_dir() {
            return Ok((
                Some(directly_in_draft),
                ResourceAvailabilityStatus::Relocated,
            ));
        }

        const MAX_SCAN_ENTRIES: usize = 128;
        for root in &approved_roots {
            if let Some(c) = cancel {
                if c.load(Ordering::Relaxed) {
                    return Err(CapCutImportError::Cancelled);
                }
            }

            let candidate = root.join(file_name);
            if candidate.is_file() || candidate.is_dir() {
                return Ok((Some(candidate), ResourceAvailabilityStatus::Relocated));
            }
            let rel_candidate = root.join(&normalized);
            if (rel_candidate.is_file() || rel_candidate.is_dir())
                && is_within_approved_roots(&rel_candidate)
            {
                return Ok((Some(rel_candidate), ResourceAvailabilityStatus::Relocated));
            }

            // Check immediate subdirectories in root with strict budget
            if let Ok(entries) = fs::read_dir(root) {
                let mut scan_count = 0;
                for entry in entries.flatten() {
                    scan_count += 1;
                    if scan_count > MAX_SCAN_ENTRIES {
                        break;
                    }
                    if let Some(c) = cancel {
                        if c.load(Ordering::Relaxed) {
                            return Err(CapCutImportError::Cancelled);
                        }
                    }
                    let sub = entry.path();
                    if sub.is_dir() {
                        let sub_cand = sub.join(file_name);
                        if sub_cand.is_file() || sub_cand.is_dir() {
                            return Ok((Some(sub_cand), ResourceAvailabilityStatus::Relocated));
                        }
                    }
                }
            }
        }
    }

    Ok((None, ResourceAvailabilityStatus::Missing))
}

// =========================================================================
// Safe Inspection Engine (Budgets, Sandboxing, Cancellation)
// =========================================================================

/// Inspects a CapCut project draft folder, draft JSON file, or ZIP archive safely.
pub fn inspect_draft(
    source_path: &Path,
    selected_roots: &[PathBuf],
    cancel: Option<&AtomicBool>,
) -> Result<CapCutInspectionResult, CapCutImportError> {
    if let Some(c) = cancel {
        if c.load(Ordering::Relaxed) {
            return Err(CapCutImportError::Cancelled);
        }
    }

    if source_path.is_dir() {
        let direct_draft = source_path.join("draft_content.json");
        if direct_draft.is_file() {
            return inspect_draft_file(&direct_draft, source_path, selected_roots, cancel);
        }

        let mut scanned_entries = 0usize;
        let found = find_draft_content_in_dir(source_path, 0, &mut scanned_entries, cancel)?;
        if let Some(path) = found {
            let parent = path.parent().unwrap_or(source_path);
            return inspect_draft_file(&path, parent, selected_roots, cancel);
        }

        return Err(CapCutImportError::UnsupportedSource(format!(
            "Directory '{}' does not contain a draft_content.json file",
            source_path.display()
        )));
    }

    let ext = source_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if matches!(
        ext.as_str(),
        "exe" | "msi" | "dmg" | "pkg" | "deb" | "rpm" | "bin" | "sh" | "bat" | "cmd"
    ) {
        return Err(CapCutImportError::UnsupportedSource(format!(
            "Binary installer file '{}' cannot be imported as a template",
            source_path.display()
        )));
    }

    if ext == "zip" {
        return inspect_zip_archive(source_path, selected_roots, cancel);
    }

    let parent = source_path.parent().unwrap_or_else(|| Path::new("."));
    inspect_draft_file(source_path, parent, selected_roots, cancel)
}

fn find_draft_content_in_dir(
    dir: &Path,
    current_depth: usize,
    scanned_entries: &mut usize,
    cancel: Option<&AtomicBool>,
) -> Result<Option<PathBuf>, CapCutImportError> {
    if let Some(c) = cancel {
        if c.load(Ordering::Relaxed) {
            return Err(CapCutImportError::Cancelled);
        }
    }
    if current_depth > MAX_DIR_SCAN_DEPTH {
        return Ok(None);
    }

    let entries = fs::read_dir(dir)?;
    for entry in entries.flatten() {
        if let Some(c) = cancel {
            if c.load(Ordering::Relaxed) {
                return Err(CapCutImportError::Cancelled);
            }
        }
        *scanned_entries += 1;
        if *scanned_entries > MAX_DIR_SCAN_ENTRIES {
            return Err(CapCutImportError::BudgetExceeded(format!(
                "Directory scan exceeded maximum entry budget of {MAX_DIR_SCAN_ENTRIES} entries"
            )));
        }

        let path = entry.path();
        if path.is_file() && path.file_name().and_then(|n| n.to_str()) == Some("draft_content.json")
        {
            return Ok(Some(path));
        } else if path.is_dir() {
            if let Some(found) =
                find_draft_content_in_dir(&path, current_depth + 1, scanned_entries, cancel)?
            {
                return Ok(Some(found));
            }
        }
    }
    Ok(None)
}

fn inspect_draft_file(
    draft_json_path: &Path,
    draft_dir: &Path,
    selected_roots: &[PathBuf],
    cancel: Option<&AtomicBool>,
) -> Result<CapCutInspectionResult, CapCutImportError> {
    if let Some(c) = cancel {
        if c.load(Ordering::Relaxed) {
            return Err(CapCutImportError::Cancelled);
        }
    }

    let bytes = read_file_bytes_bounded(draft_json_path, MAX_DRAFT_JSON_BYTES, cancel)?;
    let mut res = inspect_draft_bytes(&bytes, draft_json_path, draft_dir, selected_roots, cancel)?;
    let mut fps = HashMap::new();
    for r in &res.profile.resources {
        if let (Some(ref p_str), Some(ref sha)) = (&r.resolved_path, &r.sha256) {
            let p = PathBuf::from(p_str);
            if p.is_file() {
                fps.insert(p, sha.clone());
            }
        }
    }
    let staging_tok = register_staged_conversion(
        draft_json_path.to_path_buf(),
        res.profile.draft_sha256.clone(),
        fps,
        None,
    );
    res.report.staging_token = Some(staging_tok);
    Ok(res)
}

/// Canonicalizes archive path identities using Windows Win32 filename rules:
/// case-insensitivity, forward slash normalization, and stripping trailing dots and spaces
/// from each path segment to prevent collision/alias vulnerabilities.
pub fn normalize_windows_archive_identity(path: &str) -> String {
    let replaced = path.replace('\\', "/");
    let parts: Vec<&str> = replaced
        .split('/')
        .filter(|p| !p.is_empty() && *p != ".")
        .collect();

    let mut normalized_parts = Vec::with_capacity(parts.len());
    for part in parts {
        let trimmed = part.trim_end_matches(['.', ' ']);
        normalized_parts.push(trimmed.to_lowercase());
    }
    normalized_parts.join("/")
}

/// Inspects a ZIP archive safely with entry and compression budgets.
/// Resolves declared font/image resources from ZIP entries via isolated bounded extraction,
/// and defines the archive's SHA-256 as the consistent source identity.
pub fn inspect_zip_archive(
    zip_path: &Path,
    selected_roots: &[PathBuf],
    cancel: Option<&AtomicBool>,
) -> Result<CapCutInspectionResult, CapCutImportError> {
    if let Some(c) = cancel {
        if c.load(Ordering::Relaxed) {
            return Err(CapCutImportError::Cancelled);
        }
    }

    cleanup_expired_staged_dirs(std::time::Duration::from_secs(300));
    enforce_staging_cache_quota(MAX_STAGING_CACHE_BYTES, MAX_STAGING_SESSIONS);

    let archive_sha256 = compute_file_sha256_bounded(zip_path, MAX_STREAM_HASH_BYTES, cancel)?;

    let file = File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| CapCutImportError::Zip(format!("Failed to open zip archive: {e}")))?;

    if archive.len() > MAX_ZIP_ENTRIES {
        return Err(CapCutImportError::BudgetExceeded(format!(
            "ZIP contains {} entries, exceeding budget of {}",
            archive.len(),
            MAX_ZIP_ENTRIES
        )));
    }

    let session_uuid = uuid::Uuid::new_v4();
    let extract_dir = std::env::temp_dir().join(format!(
        "tinix_capcut_zip_{}_{}",
        std::process::id(),
        session_uuid
    ));
    fs::create_dir_all(&extract_dir)?;

    let mut guard = ScratchDirGuard::new(extract_dir.clone());

    let mut seen_normalized_paths = std::collections::HashSet::new();
    let mut total_actual_expanded: u64 = 0;
    let mut draft_file_path: Option<PathBuf> = None;

    for i in 0..archive.len() {
        if let Some(c) = cancel {
            if c.load(Ordering::Relaxed) {
                return Err(CapCutImportError::Cancelled);
            }
        }

        let mut entry = archive
            .by_index(i)
            .map_err(|e| CapCutImportError::Zip(format!("Failed to read zip entry: {e}")))?;

        let name = entry.name().to_string();
        // Path traversal validation
        if name.contains("..")
            || name.starts_with('/')
            || name.starts_with('\\')
            || name.contains(':')
        {
            return Err(CapCutImportError::SecurityViolation(format!(
                "Suspicious archive entry path '{name}'"
            )));
        }

        let windows_identity = normalize_windows_archive_identity(&name);
        if !seen_normalized_paths.insert(windows_identity.clone()) {
            return Err(CapCutImportError::SecurityViolation(format!(
                "Duplicate archive entry path: Duplicate or aliased archive entry '{name}' (canonical identity: '{windows_identity}')"
            )));
        }

        let outpath = extract_dir.join(&name);
        if entry.is_dir() {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                fs::create_dir_all(p)?;
            }
            let mut outfile = File::create(&outpath)?;
            let mut buffer = [0u8; 16384];
            let mut entry_actual_bytes: u64 = 0;

            loop {
                if let Some(c) = cancel {
                    if c.load(Ordering::Relaxed) {
                        return Err(CapCutImportError::Cancelled);
                    }
                }
                let n = entry.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                if let Some(c) = cancel {
                    if c.load(Ordering::Relaxed) {
                        return Err(CapCutImportError::Cancelled);
                    }
                }
                entry_actual_bytes = entry_actual_bytes.saturating_add(n as u64);
                total_actual_expanded = total_actual_expanded.saturating_add(n as u64);

                if entry_actual_bytes > MAX_ZIP_SINGLE_ENTRY_BYTES {
                    return Err(CapCutImportError::BudgetExceeded(format!(
                        "ZIP entry '{name}' decompressed size exceeded limit of {MAX_ZIP_SINGLE_ENTRY_BYTES} bytes"
                    )));
                }
                if total_actual_expanded > MAX_ZIP_UNCOMPRESSED_BYTES {
                    return Err(CapCutImportError::BudgetExceeded(format!(
                        "ZIP total decompressed size exceeded limit of {MAX_ZIP_UNCOMPRESSED_BYTES} bytes"
                    )));
                }

                // Check actual decompression ratio against declared compressed size
                let comp_size = entry.compressed_size();
                if comp_size > 0
                    && (entry_actual_bytes / comp_size.max(1)) > MAX_ZIP_EXPANSION_RATIO
                {
                    return Err(CapCutImportError::SecurityViolation(format!(
                        "Decompression expansion ratio for entry '{name}' exceeded {MAX_ZIP_EXPANSION_RATIO}x (zip bomb)"
                    )));
                }

                outfile.write_all(&buffer[..n])?;
            }
            outfile.flush()?;

            if name == "draft_content.json" || name.ends_with("/draft_content.json") {
                draft_file_path = Some(outpath);
            }
        }
    }

    let draft_path = draft_file_path.ok_or_else(|| {
        CapCutImportError::UnsupportedSource(
            "Archive does not contain a draft_content.json file".to_string(),
        )
    })?;

    let draft_bytes = read_file_bytes_bounded(&draft_path, MAX_DRAFT_JSON_BYTES, cancel)?;

    let mut combined_roots = vec![extract_dir.clone()];
    combined_roots.extend_from_slice(selected_roots);

    let inspect_result = inspect_draft_bytes_with_source_hash(
        &draft_bytes,
        zip_path,
        &extract_dir,
        &combined_roots,
        Some(archive_sha256.clone()),
        cancel,
    );

    match inspect_result {
        Ok(mut res) => {
            guard.disarm();
            register_staged_dir(extract_dir.clone());
            let mut fps = HashMap::new();
            for r in &res.profile.resources {
                if let (Some(ref p_str), Some(ref sha)) = (&r.resolved_path, &r.sha256) {
                    let p = PathBuf::from(p_str);
                    if p.is_file() {
                        fps.insert(p, sha.clone());
                    }
                }
            }
            let staging_tok = register_staged_conversion(
                zip_path.to_path_buf(),
                archive_sha256,
                fps,
                Some(extract_dir),
            );
            res.report.staging_token = Some(staging_tok);
            enforce_staging_cache_quota(MAX_STAGING_CACHE_BYTES, MAX_STAGING_SESSIONS);
            Ok(res)
        }
        Err(e) => {
            enforce_staging_cache_quota(MAX_STAGING_CACHE_BYTES, MAX_STAGING_SESSIONS);
            Err(e)
        }
    }
}

/// Core parser and mapper from raw bytes.
pub fn inspect_draft_bytes(
    bytes: &[u8],
    source_path: &Path,
    draft_dir: &Path,
    selected_roots: &[PathBuf],
    cancel: Option<&AtomicBool>,
) -> Result<CapCutInspectionResult, CapCutImportError> {
    inspect_draft_bytes_with_source_hash(
        bytes,
        source_path,
        draft_dir,
        selected_roots,
        None,
        cancel,
    )
}

/// Core parser and mapper from raw bytes with optional source SHA-256 override (e.g. for ZIP archives).
pub fn inspect_draft_bytes_with_source_hash(
    bytes: &[u8],
    source_path: &Path,
    draft_dir: &Path,
    selected_roots: &[PathBuf],
    source_hash_override: Option<String>,
    cancel: Option<&AtomicBool>,
) -> Result<CapCutInspectionResult, CapCutImportError> {
    if let Some(c) = cancel {
        if c.load(Ordering::Relaxed) {
            return Err(CapCutImportError::Cancelled);
        }
    }

    let aggregate_counter = std::sync::atomic::AtomicU64::new(bytes.len() as u64);
    if bytes.len() as u64 > MAX_INSPECTION_READ_BYTES {
        return Err(CapCutImportError::BudgetExceeded(format!(
            "Draft bytes exceeded inspection limit of {MAX_INSPECTION_READ_BYTES} bytes"
        )));
    }

    let draft_sha256 = source_hash_override.unwrap_or_else(|| compute_sha256_hex(bytes));

    let raw_draft: RawCapCutDraft = serde_json::from_slice(bytes).map_err(|e| {
        CapCutImportError::UnsupportedSource(format!(
            "Failed to parse draft JSON (file may be encrypted, corrupted, or incompatible): {e}"
        ))
    })?;

    let is_known_version = is_supported_draft_version(raw_draft.new_version.as_deref());

    let fps = raw_draft.fps.unwrap_or(30.0).max(1.0);
    let mut duration_us = raw_draft.duration.unwrap_or(0);
    let mut duration_seconds = if duration_us > 0 {
        duration_us as f64 / 1_000_000.0
    } else {
        0.0
    };

    let canvas_config = raw_draft.canvas_config.unwrap_or_default();
    let canvas_width = canvas_config.width.unwrap_or(1920).max(100);
    let canvas_height = canvas_config.height.unwrap_or(1080).max(100);

    let materials = raw_draft.materials.unwrap_or_default();
    let texts = materials.texts.unwrap_or_default();
    let text_templates = materials.text_templates.unwrap_or_default();
    let material_animations = materials.material_animations.unwrap_or_default();
    let videos = materials.videos.unwrap_or_default();
    let tracks = raw_draft.tracks.unwrap_or_default();

    // Reference baked video detection (diagnostics only)
    let mut ref_video_diag: Option<CapCutReferenceVideoDiagnostic> = None;
    if let Some(first_video) = videos.first() {
        if let Some(ref path_str) = first_video.path {
            let (resolved, _) =
                resolve_resource_path_bounded(path_str, draft_dir, selected_roots, cancel)?;
            let sha = if let Some(ref p) = resolved {
                if p.is_file() {
                    Some(compute_file_sha256_metered(
                        p,
                        MAX_STREAM_HASH_BYTES,
                        &aggregate_counter,
                        MAX_INSPECTION_READ_BYTES,
                        cancel,
                    )?)
                } else {
                    None
                }
            } else {
                None
            };
            let found = resolved.is_some();
            ref_video_diag = Some(CapCutReferenceVideoDiagnostic {
                path: path_str.clone(),
                sha256: sha,
                found,
                note: "Baked reference video retained for parity comparison only; never used as editable template fallback."
                    .to_string(),
            });
        }
    }

    let primary_template = text_templates.first();
    let template_name = primary_template
        .and_then(|t| t.name.clone())
        .or_else(|| raw_draft.name.clone())
        .unwrap_or_else(|| "Imported CapCut Template".to_string());
    let template_resource_id = primary_template.and_then(|t| t.resource_id.clone());

    let mut primary_text_mat_id: Option<String> = None;
    let mut extra_refs: Vec<String> = Vec::new();

    if let Some(tpl) = primary_template {
        if let Some(text_infos) = &tpl.text_info_resources {
            if let Some(first_info) = text_infos.first() {
                primary_text_mat_id = first_info.text_material_id.clone();
                if let Some(refs) = &first_info.extra_material_refs {
                    extra_refs.extend(refs.clone());
                }
            }
        }
    }

    let primary_text_material = if let Some(ref mid) = primary_text_mat_id {
        texts
            .iter()
            .find(|t| t.id == *mid)
            .or_else(|| texts.first())
    } else {
        texts.first()
    };

    let secondary_text_materials: Vec<&RawTextMaterial> = texts
        .iter()
        .filter(|t| Some(&t.id) != primary_text_material.map(|m| &m.id))
        .collect();

    let mut text_content_extracted: Option<String> = None;
    let mut parsed_styles: Vec<RawParsedTextStyle> = Vec::new();

    if let Some(tm) = primary_text_material {
        if let Some(ref content_str) = tm.content {
            if let Ok(parsed) = serde_json::from_str::<RawParsedTextContent>(content_str) {
                text_content_extracted = Some(parsed.text.clone());
                if let Some(s) = parsed.styles {
                    parsed_styles = s;
                }
            }
        }
    }

    // Resource diagnostics collection
    let mut resource_diagnostics: Vec<CapCutResourceDiagnostic> = Vec::new();
    let mut missing_resources: Vec<String> = Vec::new();

    let mut font_resource_id: Option<String> = None;
    let mut font_path: Option<String> = None;
    let mut font_name: Option<String> = None;
    let mut effect_style_resource_id: Option<String> = None;
    let mut effect_style_path: Option<String> = None;
    let mut animation_resource_id: Option<String> = None;
    let mut animation_duration_us: Option<i64> = None;
    let mut animation_sha256: Option<String> = None;

    struct CollectedAnimationSegment {
        resource_id: String,
        anim_type: String,
        duration_us: Option<i64>,
        start_us: Option<i64>,
        lua_sha256: Option<String>,
    }
    let mut collected_anim_segments: Vec<CollectedAnimationSegment> = Vec::new();

    if let Some(tpl) = primary_template {
        if let Some(resources) = &tpl.resources {
            for r in resources {
                if let Some(c) = cancel {
                    if c.load(Ordering::Relaxed) {
                        return Err(CapCutImportError::Cancelled);
                    }
                }
                let panel = r.panel.clone().unwrap_or_default();
                let rid = r.resource_id.clone().unwrap_or_default();
                let rpath = r.path.clone().unwrap_or_default();

                let (resolved, status) =
                    resolve_resource_path_bounded(&rpath, draft_dir, selected_roots, cancel)?;
                let sha = if let Some(ref p) = resolved {
                    if p.is_file() {
                        Some(compute_file_sha256_metered(
                            p,
                            MAX_STREAM_HASH_BYTES,
                            &aggregate_counter,
                            MAX_INSPECTION_READ_BYTES,
                            cancel,
                        )?)
                    } else {
                        None
                    }
                } else {
                    None
                };
                let bsize = resolved
                    .as_ref()
                    .and_then(|p| fs::metadata(p).ok().map(|m| m.len()));

                if status == ResourceAvailabilityStatus::Missing {
                    missing_resources.push(format!("{panel}:{rid}"));
                }

                if panel == "font" || panel == "fonts" {
                    font_resource_id = Some(rid.clone());
                    font_path = resolved
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .or(Some(rpath.clone()));
                    font_name = resolved
                        .as_ref()
                        .and_then(|p| Path::new(p).file_name()?.to_str())
                        .or_else(|| Path::new(&rpath).file_name()?.to_str())
                        .map(|s| s.to_string());
                } else if panel == "effectStyle" || panel == "flower" || panel == "effect" {
                    effect_style_resource_id = Some(rid.clone());
                    effect_style_path = resolved
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .or(Some(rpath.clone()));
                }

                resource_diagnostics.push(CapCutResourceDiagnostic {
                    resource_id: rid,
                    panel,
                    original_path: rpath,
                    resolved_path: resolved.map(|p| p.display().to_string()),
                    status,
                    sha256: sha,
                    byte_size: bsize,
                });
            }
        }
    }

    // Check material animations for AnimScript
    for ma in &material_animations {
        if let Some(anims) = &ma.animations {
            for a in anims {
                let rid = a.resource_id.clone().unwrap_or_default();
                let rpath = a.path.clone().unwrap_or_default();

                let (resolved, status) =
                    resolve_resource_path_bounded(&rpath, draft_dir, selected_roots, cancel)?;

                let lua_file = resolved.as_ref().and_then(|p| {
                    if p.is_dir() {
                        let direct_lua = p.join("lua").join("AnimScript.lua");
                        if direct_lua.is_file() {
                            Some(direct_lua)
                        } else {
                            let root_lua = p.join("AnimScript.lua");
                            if root_lua.is_file() {
                                Some(root_lua)
                            } else {
                                None
                            }
                        }
                    } else if p.is_file() {
                        Some(p.clone())
                    } else {
                        None
                    }
                });

                let sha = if let Some(ref p) = lua_file {
                    Some(compute_file_sha256_metered(
                        p,
                        MAX_STREAM_HASH_BYTES,
                        &aggregate_counter,
                        MAX_INSPECTION_READ_BYTES,
                        cancel,
                    )?)
                } else {
                    None
                };
                let bsize = lua_file
                    .as_ref()
                    .and_then(|p| fs::metadata(p).ok().map(|m| m.len()));

                collected_anim_segments.push(CollectedAnimationSegment {
                    resource_id: rid.clone(),
                    anim_type: a.anim_type.clone().unwrap_or_else(|| "in".to_string()),
                    duration_us: a.duration,
                    start_us: a.start,
                    lua_sha256: sha.clone(),
                });

                if status == ResourceAvailabilityStatus::Missing {
                    missing_resources.push(format!("animation:{rid}"));
                }

                let anim_resolved_path = lua_file
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .or_else(|| resolved.as_ref().map(|p| p.display().to_string()));

                resource_diagnostics.push(CapCutResourceDiagnostic {
                    resource_id: rid,
                    panel: "animation".to_string(),
                    original_path: rpath,
                    resolved_path: anim_resolved_path,
                    status,
                    sha256: sha,
                    byte_size: bsize,
                });
            }
        }
    }

    // Also check direct font reference inside primary text material if not in text_templates
    if font_name.is_none() {
        if let Some(tm) = primary_text_material {
            if let Some(fonts) = &tm.fonts {
                if let Some(f0) = fonts.first() {
                    let rid = f0.resource_id.clone().unwrap_or_default();
                    let rpath = f0.path.clone().unwrap_or_default();
                    let (resolved, status) =
                        resolve_resource_path_bounded(&rpath, draft_dir, selected_roots, cancel)?;
                    font_resource_id = Some(rid.clone());
                    font_path = resolved
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .or(Some(rpath.clone()));
                    font_name = resolved
                        .as_ref()
                        .and_then(|p| Path::new(p).file_name()?.to_str())
                        .or_else(|| Path::new(&rpath).file_name()?.to_str())
                        .map(|s| s.to_string());
                    if status == ResourceAvailabilityStatus::Missing {
                        missing_resources.push(format!(
                            "font:{}",
                            font_name.as_deref().unwrap_or("unknown")
                        ));
                    }
                    let sha = if let Some(ref p) = resolved {
                        if p.is_file() {
                            Some(compute_file_sha256_metered(
                                p,
                                MAX_STREAM_HASH_BYTES,
                                &aggregate_counter,
                                MAX_INSPECTION_READ_BYTES,
                                cancel,
                            )?)
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    let bsize = resolved
                        .as_ref()
                        .and_then(|p| fs::metadata(p).ok().map(|m| m.len()));

                    resource_diagnostics.push(CapCutResourceDiagnostic {
                        resource_id: rid,
                        panel: "font".to_string(),
                        original_path: rpath,
                        resolved_path: resolved.map(|p| p.display().to_string()),
                        status,
                        sha256: sha,
                        byte_size: bsize,
                    });
                }
            }
        }
    }

    // Check direct effect style reference from parsed text styles or materials.effects if not found
    if effect_style_path.is_none() {
        if let Some(es) = parsed_styles.iter().find_map(|s| s.effect_style.as_ref()) {
            let rid = es.id.clone().unwrap_or_default();
            let rpath = es.path.clone().unwrap_or_default();
            let (resolved, _) =
                resolve_resource_path_bounded(&rpath, draft_dir, selected_roots, cancel)?;
            effect_style_resource_id = Some(rid);
            effect_style_path = resolved
                .as_ref()
                .map(|p| p.display().to_string())
                .or(Some(rpath));
        }
    }

    if effect_style_path.is_none() {
        if let Some(e) = materials.effects.as_ref().and_then(|ef| ef.first()) {
            let rid = e.resource_id.clone().unwrap_or_default();
            let rpath = e.path.clone().unwrap_or_default();
            let (resolved, _) =
                resolve_resource_path_bounded(&rpath, draft_dir, selected_roots, cancel)?;
            effect_style_resource_id = Some(rid);
            effect_style_path = resolved
                .as_ref()
                .map(|p| p.display().to_string())
                .or(Some(rpath));
        }
    }

    // Determine recognized family profile
    let mut recognized_family = CapCutFamilyProfile::Unknown;
    let mut warnings: Vec<String> = Vec::new();
    let mut loss_details: Vec<CapCutLossDetail> = Vec::new();
    let mut imported_elements: Vec<String> = Vec::new();

    // Check animation placement and segments semantics (Scope Option 1)
    let entrance_segments: Vec<&CollectedAnimationSegment> = collected_anim_segments
        .iter()
        .filter(|s| {
            s.anim_type.eq_ignore_ascii_case("in") || s.anim_type.eq_ignore_ascii_case("intro")
        })
        .collect();

    // Populate primary animation profile metadata deterministically (preferring entrance segment if available)
    if let Some(primary_in) = entrance_segments.first() {
        animation_resource_id = Some(primary_in.resource_id.clone());
        animation_duration_us = primary_in.duration_us;
        animation_sha256 = primary_in.lua_sha256.clone();
    } else if let Some(first_seg) = collected_anim_segments.first() {
        animation_resource_id = Some(first_seg.resource_id.clone());
        animation_duration_us = first_seg.duration_us;
        animation_sha256 = first_seg.lua_sha256.clone();
    }

    // Evaluate single-entrance Scope Option 1 constraint:
    // FULL conversion is allowed ONLY when exactly one qualified entrance exists (start=0, in/intro)
    // and no other out/loop/animation segments exist.
    let mut is_evidenced_anim_placement = false;

    if collected_anim_segments.len() > 1 {
        // Multiple animation segments present (e.g. out before in, loop before/after in, two in segments)
        let seg_descs: Vec<String> = collected_anim_segments
            .iter()
            .map(|s| {
                format!(
                    "{}:'{}'@{}us",
                    s.resource_id,
                    s.anim_type,
                    s.start_us.unwrap_or(0)
                )
            })
            .collect();
        loss_details.push(CapCutLossDetail {
            component: "AnimationSegments".to_string(),
            original: format!(
                "{} animation segment(s) present: [{}]",
                collected_anim_segments.len(),
                seg_descs.join(", ")
            ),
            treatment: "Scope Option 1 supports only a single entrance animation; multiple or compound animation segments (out/loop/extra in) are omitted deterministically and native motion is not mapped".to_string(),
            impact: "Compound animation omitted; layer rendered without native motion and downgraded to PartialConversion".to_string(),
        });
        is_evidenced_anim_placement = false;
    } else if collected_anim_segments.len() == 1 {
        let single = &collected_anim_segments[0];
        let a_start = single.start_us.unwrap_or(0);
        let is_in = single.anim_type.eq_ignore_ascii_case("in")
            || single.anim_type.eq_ignore_ascii_case("intro");

        if a_start != 0 {
            loss_details.push(CapCutLossDetail {
                component: "AnimationPlacement".to_string(),
                original: format!("animation.start: {} us", a_start),
                treatment:
                    "Non-zero animation entrance start offset not supported in sample motion profile"
                        .to_string(),
                impact: "Animation start offset omitted; downgraded to PartialConversion"
                    .to_string(),
            });
        }

        if !is_in {
            loss_details.push(CapCutLossDetail {
                component: "AnimationPlacement".to_string(),
                original: format!("animation.type: \"{}\"", single.anim_type),
                treatment: "Non-entrance animation type not supported in sample motion profile"
                    .to_string(),
                impact: format!(
                    "Animation type '{}' cannot be mapped to in_anim; downgraded to PartialConversion",
                    single.anim_type
                ),
            });
        }

        if a_start == 0 && is_in {
            is_evidenced_anim_placement = true;
        }
    }

    let has_sample_anim_id =
        animation_resource_id.as_deref() == Some(KNOWN_SAMPLE_ANIM_RESOURCE_ID);
    let has_sample_wrapper_id =
        template_resource_id.as_deref() == Some(KNOWN_SAMPLE_WRAPPER_RESOURCE_ID);

    // Read effective rich styles from effectStyle.json if available with bounded read
    let mut effective_rich_style = TextRichStyle::default();
    let mut effective_style_found = false;

    if let Some(ref es_path_str) = effect_style_path {
        let p = Path::new(es_path_str);
        let target_file = if p.is_dir() {
            p.join("effectStyle.json")
        } else {
            p.to_path_buf()
        };

        if target_file.is_file() {
            let bytes = read_file_bytes_metered(
                &target_file,
                MAX_EFFECT_STYLE_BYTES,
                &aggregate_counter,
                MAX_INSPECTION_READ_BYTES,
                cancel,
            )?;
            match std::str::from_utf8(&bytes) {
                Ok(style_content) => {
                    match serde_json::from_str::<RawEffectStyleFile>(style_content) {
                        Ok(raw_style) => {
                            effective_style_found = true;
                            map_effect_style_file_to_rich(
                                &raw_style,
                                &mut effective_rich_style,
                                &mut loss_details,
                            );
                            let fill_desc = match &effective_rich_style.fill {
                                TextFill::Solid { color } => format!("solid fill ({color})"),
                                TextFill::Gradient { .. } => "gradient fill".to_string(),
                            };
                            imported_elements.push(format!(
                                "Effective rich style: {fill_desc}, strokes, shadows"
                            ));
                            let sha = compute_file_sha256_metered(
                                &target_file,
                                MAX_STREAM_HASH_BYTES,
                                &aggregate_counter,
                                MAX_INSPECTION_READ_BYTES,
                                cancel,
                            )?;
                            let bsize = bytes.len() as u64;

                            // Ensure exact consumed effectStyle.json file is recorded in resource_diagnostics
                            // with its exact SHA-256 and byte size so staging fingerprints it!
                            if let Some(diag) = resource_diagnostics.iter_mut().find(|d| {
                                d.panel == "effectStyle"
                                    || d.panel == "flower"
                                    || d.panel == "effect"
                            }) {
                                diag.resolved_path = Some(target_file.display().to_string());
                                diag.sha256 = Some(sha);
                                diag.byte_size = Some(bsize);
                            } else {
                                resource_diagnostics.push(CapCutResourceDiagnostic {
                                    resource_id: effect_style_resource_id
                                        .clone()
                                        .unwrap_or_else(|| "effectStyle".to_string()),
                                    panel: "effectStyle".to_string(),
                                    original_path: es_path_str.clone(),
                                    resolved_path: Some(target_file.display().to_string()),
                                    status: ResourceAvailabilityStatus::Available,
                                    sha256: Some(sha),
                                    byte_size: Some(bsize),
                                });
                            }
                        }
                        Err(e) => {
                            loss_details.push(CapCutLossDetail {
                                component: "EffectStyle".to_string(),
                                original: format!(
                                    "Malformed effectStyle JSON in {}: {e}",
                                    target_file.display()
                                ),
                                treatment:
                                    "Failed to parse effectStyle.json; falling back to basic text material"
                                        .to_string(),
                                impact: "Rich effect style lost; downgraded from FullConversion"
                                    .to_string(),
                            });
                        }
                    }
                }
                Err(e) => {
                    loss_details.push(CapCutLossDetail {
                        component: "EffectStyle".to_string(),
                        original: format!("Malformed UTF-8 in {}: {e}", target_file.display()),
                        treatment:
                            "Invalid UTF-8 in effectStyle.json; falling back to basic text material"
                                .to_string(),
                        impact: "Rich effect style lost; downgraded from FullConversion"
                            .to_string(),
                    });
                }
            }
        }
    }

    // Fallback to text material styles if effectStyle.json was not loaded
    if !effective_style_found {
        if let Some(first_style) = parsed_styles.first() {
            map_parsed_text_style_to_rich(
                first_style,
                &mut effective_rich_style,
                &mut loss_details,
            );
            effective_style_found = true;
            imported_elements.push("Extracted style from draft text material content".to_string());
        }
    }

    let mut allow_sample_motion = false;

    let is_sample_family = has_sample_anim_id || has_sample_wrapper_id;

    if is_sample_family {
        recognized_family = CapCutFamilyProfile::StaggeredCharacterIn;

        if !has_sample_wrapper_id {
            warnings.push(
                "Wrapper template resource ID does not match known StaggeredCharacterIn wrapper profile"
                    .to_string(),
            );
            loss_details.push(CapCutLossDetail {
                component: "Wrapper".to_string(),
                original: template_resource_id
                    .clone()
                    .unwrap_or_else(|| "none".to_string()),
                treatment: "Wrapper resource unverified; downgraded to PartialConversion"
                    .to_string(),
                impact: "Nested wrapper hierarchy omitted".to_string(),
            });
        }

        if !has_sample_anim_id {
            warnings.push(
                "Animation resource ID is missing or does not match sample family".to_string(),
            );
            loss_details.push(CapCutLossDetail {
                component: "Animation".to_string(),
                original: animation_resource_id
                    .clone()
                    .unwrap_or_else(|| "none".to_string()),
                treatment: "Animation resource unverified; native motion omitted".to_string(),
                impact: "Layer rendered statically without animated motion".to_string(),
            });
        }

        if !is_known_version {
            warnings.push(format!(
                "CapCut draft version '{:?}' is unknown or unsupported",
                raw_draft.new_version
            ));
            loss_details.push(CapCutLossDetail {
                component: "Version".to_string(),
                original: raw_draft
                    .new_version
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
                treatment: "Draft version unverified; downgraded to PartialConversion".to_string(),
                impact: "Feature compatibility unverified".to_string(),
            });
        }

        match &animation_sha256 {
            Some(sha) if sha.eq_ignore_ascii_case(KNOWN_SAMPLE_LUA_SHA256) => {
                if has_sample_wrapper_id
                    && is_known_version
                    && effective_style_found
                    && is_evidenced_anim_placement
                {
                    allow_sample_motion = true;
                    imported_elements.push(
                        "Validated StaggeredCharacterIn drop-in animation (Lua fingerprint verified)"
                            .to_string(),
                    );
                } else {
                    allow_sample_motion = false;
                    if !is_evidenced_anim_placement {
                        // Loss detail already recorded under AnimationPlacement
                    } else {
                        loss_details.push(CapCutLossDetail {
                            component: "Animation".to_string(),
                            original: format!("AnimScript.lua (hash: {sha})"),
                            treatment:
                                "Wrapper, style, or version unverified; native motion omitted"
                                    .to_string(),
                            impact: "Layer rendered statically without animated motion".to_string(),
                        });
                    }
                }
            }
            Some(sha) => {
                allow_sample_motion = false;
                warnings.push(format!(
                    "AnimScript.lua SHA-256 '{sha}' differs from known sample fingerprint. Native Lua execution is prohibited."
                ));
                loss_details.push(CapCutLossDetail {
                    component: "Animation".to_string(),
                    original: format!("Custom AnimScript.lua (hash: {sha})"),
                    treatment: "Changed Lua fingerprint; script execution prohibited and unknown motion omitted"
                        .to_string(),
                    impact: "Layer rendered statically without animated motion".to_string(),
                });
            }
            None => {
                allow_sample_motion = false;
                warnings.push(
                    "AnimScript.lua is missing from resource directory; motion cannot be inferred."
                        .to_string(),
                );
                loss_details.push(CapCutLossDetail {
                    component: "Animation".to_string(),
                    original: "Missing AnimScript.lua".to_string(),
                    treatment: "Script missing; motion cannot be safely inferred and native execution is prohibited"
                        .to_string(),
                    impact: "Layer rendered statically without animated motion".to_string(),
                });
            }
        }
    } else if template_resource_id.as_deref() == Some("7487081445878435133") {
        recognized_family = CapCutFamilyProfile::VlogTitleMultiLayer;
        imported_elements.push("Vlog title multi-layer template".to_string());
        loss_details.push(CapCutLossDetail {
            component: "ProfileScopeGate".to_string(),
            original: "VlogTitleMultiLayer (7487081445878435133)".to_string(),
            treatment: "Non-sample profile downgraded to PartialConversion per Scope Option 1"
                .to_string(),
            impact: "Multi-layer structure preserved partially; full qualification reserved for evidenced sample"
                .to_string(),
        });
        if !is_known_version {
            loss_details.push(CapCutLossDetail {
                component: "Version".to_string(),
                original: raw_draft
                    .new_version
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
                treatment: "Draft version unverified".to_string(),
                impact: "Downgraded to PartialConversion".to_string(),
            });
        }
    } else if template_resource_id.as_deref() == Some("7571466006921596167") {
        recognized_family = CapCutFamilyProfile::TextWithBackgroundCard;
        imported_elements.push("Text with rounded background card".to_string());
        loss_details.push(CapCutLossDetail {
            component: "ProfileScopeGate".to_string(),
            original: "TextWithBackgroundCard (7571466006921596167)".to_string(),
            treatment: "Non-sample profile downgraded to PartialConversion per Scope Option 1"
                .to_string(),
            impact: "Card geometry converted; full qualification reserved for evidenced sample"
                .to_string(),
        });
    } else if primary_text_material
        .as_ref()
        .and_then(|m| m.background_style)
        .unwrap_or(0)
        > 0
    {
        recognized_family = CapCutFamilyProfile::TextWithBackgroundCard;
        allow_sample_motion = false;
        loss_details.push(CapCutLossDetail {
            component: "BackgroundCard".to_string(),
            original: format!(
                "generic background_style: {}",
                primary_text_material
                    .as_ref()
                    .unwrap()
                    .background_style
                    .unwrap()
            ),
            treatment: "Generic background style converted to basic shape card layer".to_string(),
            impact: "Advanced native shaders and effects omitted; basic shape geometry used"
                .to_string(),
        });
    }

    // Build verified resource manifest (fonts/images) with exact hashes and byte sizes
    let mut resource_manifest = TemplateResourceManifest::default();

    if let Some(ref f_name) = font_name {
        if let Some(ref f_path_str) = font_path {
            let f_path = Path::new(f_path_str);
            if f_path.is_file() {
                let f_bytes = fs::metadata(f_path).map(|m| m.len())?;
                let f_sha = compute_file_sha256_metered(
                    f_path,
                    MAX_STREAM_HASH_BYTES,
                    &aggregate_counter,
                    MAX_INSPECTION_READ_BYTES,
                    cancel,
                )?;

                let safe_f_name = f_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .filter(|n| is_safe_resource_filename(n))
                    .unwrap_or("font.ttf")
                    .to_string();

                let res_id = font_resource_id
                    .clone()
                    .unwrap_or_else(|| "font_primary".to_string());

                resource_manifest.resources.push(TemplateResourceReference {
                    resource_id: res_id.clone(),
                    kind: TemplateResourceKind::Font,
                    file_name: safe_f_name,
                    sha256: f_sha,
                    byte_size: f_bytes,
                    required: true,
                });

                effective_rich_style.font = Some(FontReference {
                    family: f_name.clone(),
                    weight: 700,
                    style: FontStyle::Normal,
                    asset_id: Some(res_id),
                });
            } else {
                let m_str = format!("font:{f_name}");
                if !missing_resources.iter().any(|r| r == &m_str) {
                    missing_resources.push(m_str);
                }
                loss_details.push(CapCutLossDetail {
                    component: "Font".to_string(),
                    original: f_name.clone(),
                    treatment: "Font file missing from disk/archive; declared as required missing resource without silent substitution"
                        .to_string(),
                    impact: "Font required for export; will fail export if absent".to_string(),
                });
                effective_rich_style.font = Some(FontReference {
                    family: f_name.clone(),
                    weight: 700,
                    style: FontStyle::Normal,
                    asset_id: None,
                });
            }
        }
    } else {
        effective_rich_style.font = None;
    }

    // Also resolve and manifest fonts for secondary text materials (deduplicated by resource_id)
    for sec_tm in &secondary_text_materials {
        if let Some(sec_fonts) = &sec_tm.fonts {
            for sf in sec_fonts {
                let s_rid = sf
                    .resource_id
                    .clone()
                    .unwrap_or_else(|| "font_secondary".to_string());
                let s_rpath = sf.path.clone().unwrap_or_default();
                let (resolved, status) =
                    resolve_resource_path_bounded(&s_rpath, draft_dir, selected_roots, cancel)?;
                let s_fname = resolved
                    .as_ref()
                    .and_then(|p| p.file_name()?.to_str())
                    .or_else(|| Path::new(&s_rpath).file_name()?.to_str())
                    .unwrap_or("font.ttf")
                    .to_string();

                if let Some(ref r_path) = resolved {
                    if r_path.is_file() {
                        if !resource_manifest
                            .resources
                            .iter()
                            .any(|r| r.resource_id == s_rid)
                        {
                            let f_bytes = fs::metadata(r_path).map(|m| m.len())?;
                            let f_sha = compute_file_sha256_metered(
                                r_path,
                                MAX_STREAM_HASH_BYTES,
                                &aggregate_counter,
                                MAX_INSPECTION_READ_BYTES,
                                cancel,
                            )?;

                            let safe_f_name = r_path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .filter(|n| is_safe_resource_filename(n))
                                .unwrap_or("font.ttf")
                                .to_string();

                            resource_manifest.resources.push(TemplateResourceReference {
                                resource_id: s_rid.clone(),
                                kind: TemplateResourceKind::Font,
                                file_name: safe_f_name,
                                sha256: f_sha,
                                byte_size: f_bytes,
                                required: true,
                            });
                        }
                    }
                } else if status == ResourceAvailabilityStatus::Missing {
                    let m_str = format!("font:{s_fname}");
                    if !missing_resources.iter().any(|r| r == &m_str) {
                        missing_resources.push(m_str);
                    }
                    loss_details.push(CapCutLossDetail {
                        component: "Secondary Font".to_string(),
                        original: s_fname.clone(),
                        treatment: "Secondary font file missing from disk/archive; declared as required missing resource without silent substitution"
                            .to_string(),
                        impact: "Font required for export; will fail export if absent".to_string(),
                    });
                }

                let sha = if let Some(ref p) = resolved {
                    if p.is_file() {
                        Some(compute_file_sha256_metered(
                            p,
                            MAX_STREAM_HASH_BYTES,
                            &aggregate_counter,
                            MAX_INSPECTION_READ_BYTES,
                            cancel,
                        )?)
                    } else {
                        None
                    }
                } else {
                    None
                };
                let bsize = resolved
                    .as_ref()
                    .and_then(|p| fs::metadata(p).ok().map(|m| m.len()));

                if !resource_diagnostics.iter().any(|d| d.resource_id == s_rid) {
                    resource_diagnostics.push(CapCutResourceDiagnostic {
                        resource_id: s_rid,
                        panel: "font".to_string(),
                        original_path: s_rpath,
                        resolved_path: resolved.map(|p| p.display().to_string()),
                        status,
                        sha256: sha,
                        byte_size: bsize,
                    });
                }
            }
        }
    }

    // Inspect decorations and unsupported semantics
    if let Some(tm) = primary_text_material {
        if tm.underline == Some(true) {
            loss_details.push(CapCutLossDetail {
                component: "Decoration".to_string(),
                original: "underline: true".to_string(),
                treatment: "Not supported by standard 2D text engine; flattened".to_string(),
                impact: "Text rendered without underline".to_string(),
            });
        }
        if tm.text_curve.is_some() {
            loss_details.push(CapCutLossDetail {
                component: "PathTypesetting".to_string(),
                original: "text_curve enabled".to_string(),
                treatment: "Curved text path omitted; mapped to horizontal layout".to_string(),
                impact: "Rendered along horizontal baseline".to_string(),
            });
        }
    }

    // Inspect secondary text, image/sticker assets, and wrapper hierarchy
    if !secondary_text_materials.is_empty() {
        loss_details.push(CapCutLossDetail {
            component: "SecondaryText".to_string(),
            original: format!("{} secondary text material(s)", secondary_text_materials.len()),
            treatment: "Secondary text mapped with baseline individual properties; complex animation omitted".to_string(),
            impact: "Multi-layer text structure present; full conversion gate restricted to single-layer evidenced sample".to_string(),
        });
    }

    let raw_images = materials.images.unwrap_or_default();
    if !raw_images.is_empty() {
        loss_details.push(CapCutLossDetail {
            component: "ImageMaterials".to_string(),
            original: format!("{} image asset(s)", raw_images.len()),
            treatment: "Image layers not mapped to animated text template".to_string(),
            impact: "Non-text image materials omitted from template definition".to_string(),
        });
    }

    let raw_stickers = materials.stickers.unwrap_or_default();
    if !raw_stickers.is_empty() {
        loss_details.push(CapCutLossDetail {
            component: "StickerMaterials".to_string(),
            original: format!("{} sticker asset(s)", raw_stickers.len()),
            treatment: "Sticker layers not mapped to animated text template".to_string(),
            impact: "Non-text sticker materials omitted from template definition".to_string(),
        });
    }

    if let Some(tpl) = primary_template {
        if let Some(non_text) = &tpl.non_text_info_resources {
            if !non_text.is_empty() {
                loss_details.push(CapCutLossDetail {
                    component: "WrapperAttachment".to_string(),
                    original: format!("{} non_text_info_resource(s)", non_text.len()),
                    treatment: "Non-text wrapper resources not supported; omitted from converted definition".to_string(),
                    impact: "Non-text wrapper resources omitted; downgraded to PartialConversion".to_string(),
                });
            }
        }
        if let Some(text_infos) = &tpl.text_info_resources {
            if text_infos.len() > 1 {
                loss_details.push(CapCutLossDetail {
                    component: "WrapperHierarchy".to_string(),
                    original: format!(
                        "{} text_info_resource wrapper attachments",
                        text_infos.len()
                    ),
                    treatment:
                        "Multiple wrapper attachments collapsed into template layer hierarchy"
                            .to_string(),
                    impact: "Complex nested wrapper hierarchy simplified".to_string(),
                });
            }

            for (idx, ti) in text_infos.iter().enumerate() {
                if let Some(attach) = &ti.attach_info {
                    let child_start = attach.start_time.unwrap_or(0);
                    if child_start != 0 {
                        loss_details.push(CapCutLossDetail {
                            component: "WrapperAttachment".to_string(),
                            original: format!(
                                "text_info_resources[{}].attach_info.start_time: {} us",
                                idx, child_start
                            ),
                            treatment:
                                "Non-zero child attachment start time not supported natively"
                                    .to_string(),
                            impact: "Layer start offset omitted; downgraded to PartialConversion"
                                .to_string(),
                        });
                    }

                    if let Some(child_dur) = attach.duration {
                        if child_dur > 0 && duration_us > 0 && child_dur != duration_us {
                            loss_details.push(CapCutLossDetail {
                                component: "WrapperAttachment".to_string(),
                                original: format!(
                                    "text_info_resources[{}].attach_info.duration: {} us (draft duration: {} us)",
                                    idx, child_dur, duration_us
                                ),
                                treatment: "Child attachment duration differs from draft duration".to_string(),
                                impact: "Layer duration mismatch omitted; downgraded to PartialConversion".to_string(),
                            });
                        } else if child_dur > 0 && duration_us == 0 {
                            duration_us = child_dur;
                            duration_seconds = duration_us as f64 / 1_000_000.0;
                        }
                    }

                    if let Some(clip) = &attach.clip {
                        if let Some(flip) = &clip.flip {
                            if flip.vertical.unwrap_or(false) || flip.horizontal.unwrap_or(false) {
                                loss_details.push(CapCutLossDetail {
                                    component: "WrapperAttachment".to_string(),
                                    original: format!(
                                        "text_info_resources[{}].attach_info.clip.flip (v={}, h={})",
                                        idx,
                                        flip.vertical.unwrap_or(false),
                                        flip.horizontal.unwrap_or(false)
                                    ),
                                    treatment: "Flipped child attachment clip not supported natively".to_string(),
                                    impact: "Layer flip omitted; downgraded to PartialConversion".to_string(),
                                });
                            }
                        }
                    }
                    if let Some(lks) = &ti.lyric_keyframes {
                        if !lks.is_empty() {
                            loss_details.push(CapCutLossDetail {
                                component: "WrapperAttachment".to_string(),
                                original: format!(
                                    "text_info_resources[{}].lyric_keyframes (count: {})",
                                    idx,
                                    lks.len()
                                ),
                                treatment: "Lyric keyframes on wrapper attachment not supported natively".to_string(),
                                impact: "Attachment lyric keyframes omitted; downgraded to PartialConversion".to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    if duration_us == 0 {
        duration_us = 5_000_000;
        duration_seconds = 5.0;
    }

    // Determine conversion status:
    // Check if source contains only baked video media and no editable text templates
    let has_editable_text = primary_text_material.is_some() || primary_template.is_some();
    let has_baked_video =
        !videos.is_empty() || ref_video_diag.as_ref().map(|v| v.found).unwrap_or(false);

    let status = if !has_editable_text && has_baked_video {
        imported_elements.push(
            "Reference baked video media detected; no editable text template present".to_string(),
        );
        CapCutConversionStatus::BakedMediaOnly
    } else if recognized_family == CapCutFamilyProfile::Unknown {
        CapCutConversionStatus::UnsupportedSource
    } else if !missing_resources.is_empty() {
        CapCutConversionStatus::MissingResources
    } else if recognized_family != CapCutFamilyProfile::StaggeredCharacterIn
        || !loss_details.is_empty()
        || !is_known_version
        || !allow_sample_motion
    {
        CapCutConversionStatus::PartialConversion
    } else {
        CapCutConversionStatus::FullConversion
    };

    let text_string = text_content_extracted.unwrap_or_else(|| "Tinix Template".to_string());

    let profile = CapCutSourceProfile {
        draft_path: source_path.display().to_string(),
        draft_sha256: draft_sha256.clone(),
        app_version: raw_draft.new_version,
        fps,
        duration_us,
        duration_seconds,
        canvas_width,
        canvas_height,
        template_name: Some(template_name.clone()),
        template_id: raw_draft.id,
        template_resource_id,
        animation_resource_id: animation_resource_id.clone(),
        animation_duration_us,
        animation_sha256,
        effect_style_resource_id,
        effect_style_path,
        font_resource_id,
        font_name,
        font_path,
        text_content: Some(text_string.clone()),
        layers_count: if recognized_family == CapCutFamilyProfile::TextWithBackgroundCard {
            2
        } else if !secondary_text_materials.is_empty() {
            1 + secondary_text_materials.len()
        } else {
            1
        },
        resources: resource_diagnostics,
        missing_resources,
        reference_video: ref_video_diag.clone(),
        recognized_family,
    };

    let converted_definition = if status != CapCutConversionStatus::UnsupportedSource
        && status != CapCutConversionStatus::BakedMediaOnly
    {
        Some(build_template_definition(
            &profile,
            primary_template,
            primary_text_material,
            &secondary_text_materials,
            &tracks,
            &text_string,
            &effective_rich_style,
            &parsed_styles,
            duration_us,
            fps,
            canvas_width,
            canvas_height,
            allow_sample_motion,
            resource_manifest,
        ))
    } else {
        None
    };

    let preview_instance = converted_definition.as_ref().map(|d| {
        let mut param_map = HashMap::new();
        for p in &d.parameters {
            param_map.insert(p.key.clone(), p.default_value.clone());
        }
        TemplateInstance {
            template_id: d.id.clone(),
            template_version: d.version,
            layers: d.layers.clone(),
            parameters: param_map.clone(),
            initial_parameters: param_map,
            overrides: Vec::new(),
            responsive_policy: Some(d.responsive_policy.clone()),
        }
    });

    // Final cancellation check before returning any converted definition / report
    if let Some(c) = cancel {
        if c.load(Ordering::Relaxed) {
            return Err(CapCutImportError::Cancelled);
        }
    }

    let report = CapCutConversionReport {
        status,
        source_path: source_path.display().to_string(),
        source_sha256: draft_sha256,
        template_name,
        recognized_profile: format!("{:?}", profile.recognized_family),
        imported_elements,
        loss_details,
        warnings,
        converted_definition: converted_definition.clone(),
        preview_instance,
        reference_video: ref_video_diag,
        staging_token: None,
    };

    Ok(CapCutInspectionResult {
        profile,
        report,
        converted_definition,
    })
}

// =========================================================================
// Style and Layer Mapping
// =========================================================================

fn map_effect_style_file_to_rich(
    style_file: &RawEffectStyleFile,
    rich: &mut TextRichStyle,
    loss_details: &mut Vec<CapCutLossDetail>,
) {
    if let Some(fill) = &style_file.fill {
        let solid_opt = fill
            .content
            .as_ref()
            .and_then(|c| c.solid.as_ref())
            .or_else(|| fill.solid.as_ref());

        let render_type = fill
            .content
            .as_ref()
            .and_then(|c| c.render_type.as_deref())
            .or_else(|| fill.render_type.as_deref())
            .unwrap_or("");

        let gradient_opt = fill.content.as_ref().and_then(|c| c.gradient.as_ref());

        if render_type == "gradient" || (render_type.is_empty() && gradient_opt.is_some()) {
            if let Some(grad) = gradient_opt {
                let angle_deg = grad.angle.unwrap_or(0.0);
                let colors = grad.color.clone().unwrap_or_default();
                let alphas = grad.alpha.clone().unwrap_or_default();
                let percents = grad.percent.clone().unwrap_or_default();

                let mut stops = Vec::new();
                for (i, c) in colors.iter().enumerate() {
                    let alpha_raw = alphas.get(i).copied();
                    let alpha_val = match alpha_raw {
                        Some(a) => {
                            if !a.is_finite() || !(0.0..=1.0).contains(&a) {
                                loss_details.push(CapCutLossDetail {
                                    component: "EffectStyleFill".to_string(),
                                    original: format!(
                                        "gradient stop {i} alpha '{a}' is unsupported or non-finite"
                                    ),
                                    treatment: "Unsupported gradient stop alpha value; downgraded to PartialConversion"
                                        .to_string(),
                                    impact: "Stop alpha defaulted to 1.0; FullConversion disallowed"
                                        .to_string(),
                                });
                                1.0
                            } else {
                                a
                            }
                        }
                        None => 1.0,
                    };
                    let hex = rgba_to_hex(*c, alpha_val);
                    let pct = percents.get(i).copied().unwrap_or(i as f64) as f32;
                    stops.push(GradientStop {
                        offset: pct.clamp(0.0, 1.0),
                        color: hex,
                    });
                }
                stops.sort_by(|a, b| a.offset.partial_cmp(&b.offset).unwrap());

                if !stops.is_empty() {
                    if stops.first().unwrap().offset > 0.0 {
                        let mut first = stops.first().unwrap().clone();
                        first.offset = 0.0;
                        stops.insert(0, first);
                    }
                    if stops.last().unwrap().offset < 1.0 {
                        let mut last = stops.last().unwrap().clone();
                        last.offset = 1.0;
                        stops.push(last);
                    } else if let Some(last) = stops.last_mut() {
                        last.offset = 1.0;
                    }

                    rich.fill = TextFill::Gradient {
                        gradient_kind: crate::modules::domain::text::GradientKind::Linear,
                        angle_deg,
                        radius: None,
                        space: crate::modules::domain::text::GradientSpace::All,
                        stops,
                    };
                } else {
                    loss_details.push(CapCutLossDetail {
                        component: "EffectStyleFill".to_string(),
                        original: "gradient fill with empty color stops".to_string(),
                        treatment: "Empty gradient stops; default fill retained".to_string(),
                        impact: "Effect style gradient lost; downgraded from FullConversion"
                            .to_string(),
                    });
                }
            } else {
                loss_details.push(CapCutLossDetail {
                    component: "EffectStyleFill".to_string(),
                    original: "render_type 'gradient' without gradient object".to_string(),
                    treatment: "Gradient definition missing; default fill retained".to_string(),
                    impact: "Effect style fill lost; downgraded from FullConversion".to_string(),
                });
            }
        } else if render_type == "solid" || (render_type.is_empty() && solid_opt.is_some()) {
            if let Some(solid) = solid_opt {
                if let Some(c) = solid.color {
                    let alpha_raw = solid.alpha.or(fill.alpha);
                    let alpha_val = match alpha_raw {
                        Some(a) => {
                            let a_f64 = a as f64;
                            if !a_f64.is_finite() || !(0.0..=1.0).contains(&a_f64) {
                                loss_details.push(CapCutLossDetail {
                                    component: "EffectStyleFill".to_string(),
                                    original: format!(
                                        "solid alpha '{a}' is unsupported or non-finite"
                                    ),
                                    treatment: "Unsupported solid fill alpha value; downgraded to PartialConversion"
                                        .to_string(),
                                    impact: "Alpha defaulted to 1.0; FullConversion disallowed"
                                        .to_string(),
                                });
                                1.0
                            } else {
                                a_f64
                            }
                        }
                        None => 1.0,
                    };
                    rich.fill = TextFill::Solid {
                        color: rgba_to_hex(c, alpha_val),
                    };
                } else {
                    loss_details.push(CapCutLossDetail {
                        component: "EffectStyleFill".to_string(),
                        original: "solid fill without color array".to_string(),
                        treatment: "Solid color missing; default fill retained".to_string(),
                        impact: "Effect style fill lost; downgraded from FullConversion"
                            .to_string(),
                    });
                }
            } else {
                loss_details.push(CapCutLossDetail {
                    component: "EffectStyleFill".to_string(),
                    original: "render_type 'solid' specified without solid color object"
                        .to_string(),
                    treatment: "Solid fill definition missing; default fill retained".to_string(),
                    impact: "Effect style fill lost; downgraded from FullConversion".to_string(),
                });
            }
        } else if !render_type.is_empty() {
            loss_details.push(CapCutLossDetail {
                component: "EffectStyleFill".to_string(),
                original: format!("unsupported fill render_type: '{render_type}'"),
                treatment: format!("Fill render_type '{render_type}' not supported natively"),
                impact: "Effect style fill lost; downgraded from FullConversion".to_string(),
            });
        }
    }

    if let Some(strokes) = &style_file.strokes {
        rich.strokes.clear();
        for st in strokes {
            if st.enable.unwrap_or(true) {
                let render_type = st
                    .content
                    .as_ref()
                    .and_then(|c| c.render_type.as_deref())
                    .or_else(|| st.render_type.as_deref())
                    .unwrap_or("solid");

                if render_type == "gradient" {
                    loss_details.push(CapCutLossDetail {
                        component: "EffectStyleStroke".to_string(),
                        original: "stroke render_type: 'gradient'".to_string(),
                        treatment: "Gradient stroke not supported natively; falling back to solid approximation"
                            .to_string(),
                        impact: "Stroke gradient lost; downgraded to PartialConversion".to_string(),
                    });
                } else if render_type != "solid" {
                    loss_details.push(CapCutLossDetail {
                        component: "EffectStyleStroke".to_string(),
                        original: format!("unsupported stroke render_type: '{render_type}'"),
                        treatment: format!("Stroke render_type '{render_type}' not supported natively"),
                        impact: "Unsupported stroke paint omitted or approximated; downgraded to PartialConversion"
                            .to_string(),
                    });
                }

                let color_rgb = st
                    .content
                    .as_ref()
                    .and_then(|c| c.solid.as_ref())
                    .and_then(|s| s.color)
                    .or_else(|| st.solid.as_ref().and_then(|s| s.color));

                if color_rgb.is_none() {
                    loss_details.push(CapCutLossDetail {
                        component: "EffectStyleStroke".to_string(),
                        original: "stroke solid color data missing".to_string(),
                        treatment: "Stroke solid color missing; default stroke color retained"
                            .to_string(),
                        impact: "Stroke color lost; downgraded to PartialConversion".to_string(),
                    });
                }

                let alpha_raw = st
                    .alpha
                    .or_else(|| {
                        st.content
                            .as_ref()
                            .and_then(|c| c.solid.as_ref())
                            .and_then(|s| s.alpha)
                    })
                    .or_else(|| st.solid.as_ref().and_then(|s| s.alpha));
                let alpha_val = match alpha_raw {
                    Some(a) => {
                        let a_f64 = a as f64;
                        if !a_f64.is_finite() || !(0.0..=1.0).contains(&a_f64) {
                            loss_details.push(CapCutLossDetail {
                                component: "EffectStyleStroke".to_string(),
                                original: format!("stroke alpha '{a}' is unsupported or non-finite"),
                                treatment: "Unsupported stroke alpha value; downgraded to PartialConversion"
                                    .to_string(),
                                impact: "Stroke alpha defaulted to 1.0; FullConversion disallowed"
                                    .to_string(),
                            });
                            1.0
                        } else {
                            a_f64
                        }
                    }
                    None => 1.0,
                };
                let color = color_rgb
                    .map(|rgb| rgba_to_hex(rgb, alpha_val))
                    .unwrap_or_else(|| {
                        rgba_to_hex([0xAD as f64 / 255.0, 0x51 as f64 / 255.0, 0.0], alpha_val)
                    });
                let width = st.width.unwrap_or(0.15) * 100.0;
                rich.strokes.push(TextStroke {
                    width: width.max(1.0),
                    color,
                    align: StrokeAlign::Outside,
                });
            }
        }
    }

    if let Some(shadows) = &style_file.shadows {
        rich.shadows.clear();
        for sh in shadows {
            if sh.enable.unwrap_or(true) {
                let render_type = sh
                    .content
                    .as_ref()
                    .and_then(|c| c.render_type.as_deref())
                    .or_else(|| sh.render_type.as_deref())
                    .unwrap_or("solid");

                if render_type == "gradient" {
                    loss_details.push(CapCutLossDetail {
                        component: "EffectStyleShadow".to_string(),
                        original: "shadow render_type: 'gradient'".to_string(),
                        treatment: "Gradient shadow not supported natively; falling back to solid approximation"
                            .to_string(),
                        impact: "Shadow gradient lost; downgraded to PartialConversion".to_string(),
                    });
                } else if render_type != "solid" {
                    loss_details.push(CapCutLossDetail {
                        component: "EffectStyleShadow".to_string(),
                        original: format!("unsupported shadow render_type: '{render_type}'"),
                        treatment: format!("Shadow render_type '{render_type}' not supported natively"),
                        impact: "Unsupported shadow paint omitted or approximated; downgraded to PartialConversion"
                            .to_string(),
                    });
                }

                let color_rgb = sh
                    .content
                    .as_ref()
                    .and_then(|c| c.solid.as_ref())
                    .and_then(|s| s.color)
                    .or_else(|| sh.solid.as_ref().and_then(|s| s.color));

                if color_rgb.is_none() {
                    loss_details.push(CapCutLossDetail {
                        component: "EffectStyleShadow".to_string(),
                        original: "shadow solid color data missing".to_string(),
                        treatment: "Shadow solid color missing; default shadow color retained"
                            .to_string(),
                        impact: "Shadow color lost; downgraded to PartialConversion".to_string(),
                    });
                }

                let alpha_raw = sh
                    .alpha
                    .or_else(|| {
                        sh.content
                            .as_ref()
                            .and_then(|c| c.solid.as_ref())
                            .and_then(|s| s.alpha)
                    })
                    .or_else(|| sh.solid.as_ref().and_then(|s| s.alpha));
                let alpha_val = match alpha_raw {
                    Some(a) => {
                        let a_f64 = a as f64;
                        if !a_f64.is_finite() || !(0.0..=1.0).contains(&a_f64) {
                            loss_details.push(CapCutLossDetail {
                                component: "EffectStyleShadow".to_string(),
                                original: format!("shadow alpha '{a}' is unsupported or non-finite"),
                                treatment: "Unsupported shadow alpha value; downgraded to PartialConversion"
                                    .to_string(),
                                impact: "Shadow alpha defaulted to 1.0; FullConversion disallowed"
                                    .to_string(),
                            });
                            1.0
                        } else {
                            a_f64
                        }
                    }
                    None => 1.0,
                };
                let color = color_rgb
                    .map(|rgb| rgba_to_hex(rgb, alpha_val))
                    .unwrap_or_else(|| {
                        rgba_to_hex([0xAD as f64 / 255.0, 0x51 as f64 / 255.0, 0.0], alpha_val)
                    });
                let raw_dist = sh.distance.unwrap_or(5.0);
                let dist = if raw_dist <= 1.0 {
                    raw_dist * 100.0
                } else {
                    raw_dist
                };
                let angle_deg = sh.angle.unwrap_or(-45.0);
                let angle_rad = angle_deg.to_radians();

                let offset_x = dist * angle_rad.cos();
                let offset_y = dist * angle_rad.sin();
                let blur = if let Some(d) = sh.diffuse {
                    if d > 0.0 && d <= 1.0 {
                        d * 50.0
                    } else {
                        d
                    }
                } else {
                    0.0
                };

                rich.shadows.push(TextShadow {
                    offset_x,
                    offset_y,
                    blur,
                    color,
                });
            }
        }
    }
}

fn map_parsed_text_style_to_rich(
    style: &RawParsedTextStyle,
    rich: &mut TextRichStyle,
    loss_details: &mut Vec<CapCutLossDetail>,
) {
    if let Some(fill) = &style.fill {
        if let Some(content) = &fill.content {
            if let Some(solid) = &content.solid {
                if let Some(c) = solid.color {
                    let alpha_raw = solid.alpha.or(fill.alpha);
                    let alpha_val = match alpha_raw {
                        Some(a) => {
                            let a_f64 = a as f64;
                            if !a_f64.is_finite() || !(0.0..=1.0).contains(&a_f64) {
                                loss_details.push(CapCutLossDetail {
                                    component: "EffectStyleFill".to_string(),
                                    original: format!(
                                        "draft text solid alpha '{a}' is unsupported or non-finite"
                                    ),
                                    treatment: "Unsupported solid fill alpha value; downgraded to PartialConversion"
                                        .to_string(),
                                    impact: "Alpha defaulted to 1.0; FullConversion disallowed"
                                        .to_string(),
                                });
                                1.0
                            } else {
                                a_f64
                            }
                        }
                        None => 1.0,
                    };
                    rich.fill = TextFill::Solid {
                        color: rgba_to_hex(c, alpha_val),
                    };
                }
            } else if let Some(grad) = &content.gradient {
                let angle_deg = grad.angle.unwrap_or(0.0);
                let colors = grad.color.clone().unwrap_or_default();
                let alphas = grad.alpha.clone().unwrap_or_default();
                let percents = grad.percent.clone().unwrap_or_default();

                let mut stops = Vec::new();
                for (i, c) in colors.iter().enumerate() {
                    let alpha_raw = alphas.get(i).copied();
                    let alpha_val = match alpha_raw {
                        Some(a) => {
                            if !a.is_finite() || !(0.0..=1.0).contains(&a) {
                                loss_details.push(CapCutLossDetail {
                                    component: "EffectStyleFill".to_string(),
                                    original: format!(
                                        "draft text gradient stop {i} alpha '{a}' is unsupported or non-finite"
                                    ),
                                    treatment: "Unsupported gradient stop alpha value; downgraded to PartialConversion"
                                        .to_string(),
                                    impact: "Stop alpha defaulted to 1.0; FullConversion disallowed"
                                        .to_string(),
                                });
                                1.0
                            } else {
                                a
                            }
                        }
                        None => 1.0,
                    };
                    let hex = rgba_to_hex(*c, alpha_val);
                    let pct = percents.get(i).copied().unwrap_or(i as f64) as f32;
                    stops.push(GradientStop {
                        offset: pct.clamp(0.0, 1.0),
                        color: hex,
                    });
                }
                stops.sort_by(|a, b| a.offset.partial_cmp(&b.offset).unwrap());

                if !stops.is_empty() {
                    if stops.first().unwrap().offset > 0.0 {
                        let mut first = stops.first().unwrap().clone();
                        first.offset = 0.0;
                        stops.insert(0, first);
                    }
                    if stops.last().unwrap().offset < 1.0 {
                        let mut last = stops.last().unwrap().clone();
                        last.offset = 1.0;
                        stops.push(last);
                    } else if let Some(last) = stops.last_mut() {
                        last.offset = 1.0;
                    }
                }

                rich.fill = TextFill::Gradient {
                    gradient_kind: crate::modules::domain::text::GradientKind::Linear,
                    angle_deg,
                    radius: None,
                    space: crate::modules::domain::text::GradientSpace::All,
                    stops,
                };
            }
        }
    }
    if let Some(sz) = style.size {
        if sz <= 25.0 {
            rich.font_size = Some((sz * 10.0).max(12.0));
        } else {
            rich.font_size = Some(sz);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_template_definition(
    profile: &CapCutSourceProfile,
    primary_tpl: Option<&RawTextTemplate>,
    primary_tm: Option<&RawTextMaterial>,
    secondary_tms: &[&RawTextMaterial],
    tracks: &[RawTrack],
    text_content: &str,
    rich_style: &TextRichStyle,
    parsed_styles: &[RawParsedTextStyle],
    duration_us: i64,
    fps: f64,
    canvas_width: u32,
    canvas_height: u32,
    allow_sample_motion: bool,
    resource_manifest: TemplateResourceManifest,
) -> TemplateDefinition {
    let rate = FrameRate::new((fps.round() as u32).max(1), 1);
    let total_frames = ((duration_us as f64 / 1_000_000.0) * fps).round().max(1.0) as i64;
    let default_dur = RationalTime::new(total_frames, rate);

    let anim_frames = profile
        .animation_duration_us
        .map(|d| ((d as f64 / 1_000_000.0) * fps).round().max(1.0) as i64)
        .unwrap_or(31);

    // Single-owner motion: animation is assigned to text_animation ONLY, layer.motion is None.
    let animation = if allow_sample_motion {
        Some(TextAnimation {
            unit: AnimationUnit::Grapheme,
            order: StaggerOrder::Forward,
            in_anim: Some(TextAnimationSegment {
                duration: RationalTime::new(anim_frames, rate),
                stagger_ratio: 0.70,
                easing: KeyframeEasing::CubicBezier {
                    x1: 0.166667,
                    y1: 0.166667,
                    x2: 0.14,
                    y2: 1.0,
                },
                from: AnimationTransform {
                    opacity: 0.0,
                    translate_x: 0.0,
                    translate_y: -60.0,
                    scale_x: 1.0,
                    scale_y: 1.0,
                    rotation: 0.0,
                },
                to: AnimationTransform {
                    opacity: 1.0,
                    translate_x: 0.0,
                    translate_y: 0.0,
                    scale_x: 1.0,
                    scale_y: 1.0,
                    rotation: 0.0,
                },
            }),
            loop_anim: None,
            out_anim: None,
            conflict_policy: DurationConflictPolicy::Reject,
            pivot_x: 0.5,
            pivot_y: 0.5,
            seed: None,
            loop_mode: LoopMode::None,
            phase_origin: None,
            phase_end: None,
        })
    } else {
        None
    };

    // Helper to extract transform components from a clip
    let extract_clip_transform = |clip: &RawClip| -> (f32, f32, f32, f32, f32, f32) {
        let tx = clip.transform.as_ref().and_then(|t| t.x).unwrap_or(0.0);
        let ty = clip.transform.as_ref().and_then(|t| t.y).unwrap_or(0.0);
        let sx = clip.scale.as_ref().and_then(|s| s.x).unwrap_or(1.0);
        let sy = clip.scale.as_ref().and_then(|s| s.y).unwrap_or(1.0);
        let rot = clip.rotation.unwrap_or(0.0);
        let alpha = clip.alpha.unwrap_or(1.0);
        (tx, ty, sx, sy, rot, alpha)
    };

    // Extract parent track segment transform
    let mut parent_tx = 0.0f32;
    let mut parent_ty = 0.0f32;
    let mut parent_sx = 1.0f32;
    let mut parent_sy = 1.0f32;
    let mut parent_rot = 0.0f32;
    let mut parent_alpha = 1.0f32;

    let target_ids: Vec<&str> = [
        primary_tm.map(|t| t.id.as_str()),
        primary_tpl.map(|t| t.id.as_str()),
    ]
    .into_iter()
    .flatten()
    .collect();

    for tr in tracks {
        if let Some(segs) = &tr.segments {
            for seg in segs {
                let matches_mat = seg
                    .material_id
                    .as_deref()
                    .map(|id| target_ids.contains(&id))
                    .unwrap_or(false);
                let matches_extra = seg
                    .extra_material_refs
                    .as_ref()
                    .map(|refs| refs.iter().any(|r| target_ids.contains(&r.as_str())))
                    .unwrap_or(false);
                if matches_mat || matches_extra {
                    if let Some(clip) = &seg.clip {
                        let (tx, ty, sx, sy, rot, alpha) = extract_clip_transform(clip);
                        parent_tx = tx;
                        parent_ty = ty;
                        parent_sx = sx;
                        parent_sy = sy;
                        parent_rot = rot;
                        parent_alpha = alpha;
                        break;
                    }
                }
            }
        }
    }

    // Extract child attach_info clip transform from primary_tpl
    let mut child_tx = 0.0f32;
    let mut child_ty = 0.0f32;
    let mut child_sx = 1.0f32;
    let mut child_sy = 1.0f32;
    let mut child_rot = 0.0f32;
    let mut child_alpha = 1.0f32;

    if let Some(tpl) = primary_tpl {
        if let Some(text_infos) = &tpl.text_info_resources {
            let matching_info = primary_tm
                .and_then(|tm| {
                    text_infos
                        .iter()
                        .find(|ti| ti.text_material_id.as_deref() == Some(&tm.id))
                })
                .or_else(|| text_infos.first());
            if let Some(info) = matching_info {
                if let Some(attach) = &info.attach_info {
                    if let Some(clip) = &attach.clip {
                        let (tx, ty, sx, sy, rot, alpha) = extract_clip_transform(clip);
                        child_tx = tx;
                        child_ty = ty;
                        child_sx = sx;
                        child_sy = sy;
                        child_rot = rot;
                        child_alpha = alpha;
                    }
                }
            }
        }
    }

    // Combine parent and child transforms as plan specifies
    let primary_transform = Transform {
        position_x: (parent_tx + child_tx) * (canvas_width as f32 / 2.0),
        position_y: (parent_ty + child_ty) * (canvas_height as f32 / 2.0),
        scale_x: parent_sx * child_sx,
        scale_y: parent_sy * child_sy,
        rotation: parent_rot + child_rot,
    };
    let primary_opacity = parent_alpha * child_alpha;

    let primary_alignment = match primary_tm.and_then(|m| m.alignment) {
        Some(0) => TextAlignment::Left,
        Some(2) => TextAlignment::Right,
        _ => TextAlignment::Center,
    };

    // Calculate font size dynamically from style / material (no hardcoded 152.0)
    let extracted_font_size = parsed_styles
        .first()
        .and_then(|s| s.size)
        .or_else(|| primary_tm.and_then(|m| m.font_size))
        .map(|sz| {
            if sz <= 25.0 {
                (sz * 10.0).max(12.0)
            } else {
                sz
            }
        })
        .unwrap_or(72.0);

    let mut clean_rich = rich_style.clone();
    clean_rich.font_size = Some(extracted_font_size);
    clean_rich.layout = TextLayout {
        alignment: primary_alignment,
        wrap_mode: crate::modules::domain::text::TextWrapMode::None,
        line_height: 1.2,
        letter_spacing: 0.0,
        max_width: None,
        min_font_size: None,
        max_font_size: None,
    };

    // Map style spans if multiple ranges exist
    if parsed_styles.len() > 1 {
        let text_graphemes = grapheme_count(text_content);
        for s in parsed_styles {
            if let Some(range) = &s.range {
                if range.len() >= 2 && range[0] < range[1] && range[1] <= text_graphemes {
                    let mut span_style = TextSpanStyle::default();
                    if let Some(sz) = s.size {
                        let span_size = if sz <= 25.0 {
                            (sz * 10.0).max(12.0)
                        } else {
                            sz
                        };
                        span_style.font_size = Some(span_size);
                    }
                    if let Some(fill) = &s.fill {
                        if let Some(content) = &fill.content {
                            if let Some(solid) = &content.solid {
                                if let Some(c) = solid.color {
                                    span_style.fill = Some(TextFill::Solid {
                                        color: rgb_to_hex(c),
                                    });
                                }
                            }
                        }
                    }
                    clean_rich.spans.push(RichTextSpan {
                        start: range[0] as u32,
                        end: range[1] as u32,
                        style: span_style,
                    });
                }
            }
        }
    }

    let primary_layer = TemplateLayer {
        layer_id: "lyr_primary_text".to_string(),
        name: "Primary Text".to_string(),
        z_index: 10,
        visible: true,
        opacity: primary_opacity,
        transform: primary_transform.clone(),
        anchor: TextAnchor::Center,
        pivot: (0.5, 0.5),
        motion: None, // Single-owner motion: Layer motion is strictly None!
        kind: TemplateLayerKind::Text(TemplateTextLayerData {
            is_primary: true,
            slot_key: Some("title".to_string()),
            text_content: Some(text_content.to_string()),
            text_style: Some(TextStyle {
                font_family: clean_rich
                    .font
                    .as_ref()
                    .map(|f| f.family.clone())
                    .or_else(|| profile.font_name.clone())
                    .unwrap_or_else(|| "MissingFont".to_string()),
                font_size: extracted_font_size,
                font_color: "#FFE35F".to_string(),
                font_weight: 700,
                line_height: 1.2,
                alignment: primary_alignment,
                anchor: TextAnchor::Center,
                background_color: None,
                background_opacity: 0.0,
                padding_x: 0.0,
                padding_y: 0.0,
                accent_color: None,
            }),
            text_rich: Some(clean_rich),
            text_animation: animation, // Motion owned exclusively by text animation
        }),
    };

    let mut layers = vec![primary_layer];
    let mut parameters = vec![TemplateParameterDefinition {
        key: "title".to_string(),
        name: "Title Text".to_string(),
        param_type: TemplateParameterType::Text,
        default_value: TemplateParameterValue::Text(text_content.to_string()),
        description: Some("Editable title text".to_string()),
    }];
    let mut bindings = vec![TemplateParameterBinding {
        parameter_key: "title".to_string(),
        target_layer_id: "lyr_primary_text".to_string(),
        target_property: TemplatePropertyPath::TextContent,
    }];

    // Multi-layer mapping for VlogTitleMultiLayer or multiple text materials
    for (idx, sec_tm) in secondary_tms.iter().enumerate() {
        let sec_num = idx + 1;
        let sec_layer_id = format!("lyr_secondary_text_{sec_num}");
        let sec_slot_key = format!("subtitle_{sec_num}");

        let mut sec_text = "Subtitle".to_string();
        let mut sec_fill_color = "#FFFFFF".to_string();
        let sec_rich_spans = Vec::new();

        if let Some(content_str) = &sec_tm.content {
            if let Ok(parsed) = serde_json::from_str::<RawParsedTextContent>(content_str) {
                sec_text = parsed.text;
                if let Some(styles) = parsed.styles {
                    for s in &styles {
                        if let Some(fill) = &s.fill {
                            if let Some(content) = &fill.content {
                                if let Some(solid) = &content.solid {
                                    if let Some(c) = solid.color {
                                        sec_fill_color = rgb_to_hex(c);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let sec_align = match sec_tm.alignment {
            Some(0) => TextAlignment::Left,
            Some(2) => TextAlignment::Right,
            _ => TextAlignment::Center,
        };

        let sec_font_size = sec_tm
            .font_size
            .map(|sz| {
                if sz <= 25.0 {
                    (sz * 10.0).max(12.0)
                } else {
                    sz
                }
            })
            .unwrap_or(48.0);

        let sec_font_name = sec_tm
            .fonts
            .as_ref()
            .and_then(|f| f.first())
            .and_then(|fi| {
                fi.path
                    .as_deref()
                    .and_then(|p| Path::new(p).file_name())
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string())
                    .or_else(|| fi.resource_id.clone())
            })
            .or_else(|| profile.font_name.clone());

        let sec_asset_id = sec_tm
            .fonts
            .as_ref()
            .and_then(|f| f.first())
            .and_then(|fi| fi.resource_id.as_ref())
            .and_then(|rid| {
                if resource_manifest
                    .resources
                    .iter()
                    .any(|r| &r.resource_id == rid)
                {
                    Some(rid.clone())
                } else {
                    None
                }
            });

        let sec_font_ref = sec_font_name.as_ref().map(|fname| FontReference {
            family: fname.clone(),
            weight: 400,
            style: FontStyle::Normal,
            asset_id: sec_asset_id,
        });

        let sec_rich = TextRichStyle {
            font: sec_font_ref.clone(),
            font_size: Some(sec_font_size),
            layout: TextLayout {
                alignment: sec_align,
                wrap_mode: crate::modules::domain::text::TextWrapMode::None,
                line_height: 1.2,
                letter_spacing: 0.0,
                max_width: None,
                min_font_size: None,
                max_font_size: None,
            },
            fill: TextFill::Solid {
                color: sec_fill_color.clone(),
            },
            strokes: Vec::new(),
            shadows: Vec::new(),
            background: None,
            spans: sec_rich_spans,
        };

        // Combine parent and child transform for secondary text
        let mut sec_parent_tx = 0.0f32;
        let mut sec_parent_ty = 0.0f32;
        let mut sec_parent_sx = 1.0f32;
        let mut sec_parent_sy = 1.0f32;
        let mut sec_parent_rot = 0.0f32;
        let mut sec_parent_alpha = 1.0f32;

        for tr in tracks {
            if let Some(segs) = &tr.segments {
                for seg in segs {
                    let matches_mat = seg.material_id.as_deref() == Some(&sec_tm.id);
                    let matches_extra = seg
                        .extra_material_refs
                        .as_ref()
                        .map(|refs| refs.iter().any(|r| r == &sec_tm.id))
                        .unwrap_or(false);
                    if matches_mat || matches_extra {
                        if let Some(clip) = &seg.clip {
                            let (tx, ty, sx, sy, rot, alpha) = extract_clip_transform(clip);
                            sec_parent_tx = tx;
                            sec_parent_ty = ty;
                            sec_parent_sx = sx;
                            sec_parent_sy = sy;
                            sec_parent_rot = rot;
                            sec_parent_alpha = alpha;
                            break;
                        }
                    }
                }
            }
        }

        let mut sec_child_tx = 0.0f32;
        let mut sec_child_ty = 0.0f32;
        let mut sec_child_sx = 1.0f32;
        let mut sec_child_sy = 1.0f32;
        let mut sec_child_rot = 0.0f32;
        let mut sec_child_alpha = 1.0f32;

        if let Some(tpl) = primary_tpl {
            if let Some(text_infos) = &tpl.text_info_resources {
                if let Some(info) = text_infos
                    .iter()
                    .find(|ti| ti.text_material_id.as_deref() == Some(&sec_tm.id))
                {
                    if let Some(attach) = &info.attach_info {
                        if let Some(clip) = &attach.clip {
                            let (tx, ty, sx, sy, rot, alpha) = extract_clip_transform(clip);
                            sec_child_tx = tx;
                            sec_child_ty = ty;
                            sec_child_sx = sx;
                            sec_child_sy = sy;
                            sec_child_rot = rot;
                            sec_child_alpha = alpha;
                        }
                    }
                }
            }
        }

        let sec_transform = Transform {
            position_x: (sec_parent_tx + sec_child_tx) * (canvas_width as f32 / 2.0),
            position_y: (sec_parent_ty + sec_child_ty) * (canvas_height as f32 / 2.0),
            scale_x: sec_parent_sx * sec_child_sx,
            scale_y: sec_parent_sy * sec_child_sy,
            rotation: sec_parent_rot + sec_child_rot,
        };
        let sec_opacity = sec_parent_alpha * sec_child_alpha;

        layers.push(TemplateLayer {
            layer_id: sec_layer_id.clone(),
            name: format!("Secondary Text {sec_num}"),
            z_index: 10 + sec_num as i32,
            visible: true,
            opacity: sec_opacity,
            transform: sec_transform,
            anchor: TextAnchor::Center,
            pivot: (0.5, 0.5),
            motion: None,
            kind: TemplateLayerKind::Text(TemplateTextLayerData {
                is_primary: false,
                slot_key: Some(sec_slot_key.clone()),
                text_content: Some(sec_text.clone()),
                text_style: Some(TextStyle {
                    font_family: sec_font_name.unwrap_or_else(|| "SecondaryFont".to_string()),
                    font_size: sec_font_size,
                    font_color: sec_fill_color,
                    font_weight: 400,
                    line_height: 1.2,
                    alignment: sec_align,
                    anchor: TextAnchor::Center,
                    background_color: None,
                    background_opacity: 0.0,
                    padding_x: 0.0,
                    padding_y: 0.0,
                    accent_color: None,
                }),
                text_rich: Some(sec_rich),
                text_animation: None,
            }),
        });

        parameters.push(TemplateParameterDefinition {
            key: sec_slot_key.clone(),
            name: format!("Subtitle {sec_num}"),
            param_type: TemplateParameterType::Text,
            default_value: TemplateParameterValue::Text(sec_text),
            description: Some(format!("Editable subtitle text {sec_num}")),
        });

        bindings.push(TemplateParameterBinding {
            parameter_key: sec_slot_key,
            target_layer_id: sec_layer_id,
            target_property: TemplatePropertyPath::TextContent,
        });
    }

    // Dynamic background card (responsive, non-fixed 800x240)
    if profile.recognized_family == CapCutFamilyProfile::TextWithBackgroundCard {
        let tm = primary_tm;
        let bg_w = tm.and_then(|m| m.background_width).unwrap_or(0.0);
        let bg_h = tm.and_then(|m| m.background_height).unwrap_or(0.0);
        let bg_r = tm.and_then(|m| m.background_round_radius).unwrap_or(0.2);
        let bg_col = tm
            .and_then(|m| m.background_color.clone())
            .unwrap_or_else(|| "#1E1E1E".to_string());

        let card_w = if bg_w > 0.0 {
            (bg_w * canvas_width as f32).max(100.0)
        } else {
            let graphemes = grapheme_count(text_content);
            (graphemes as f32 * extracted_font_size * 0.75 + 100.0)
                .clamp(200.0, canvas_width as f32)
        };

        let card_h = if bg_h > 0.0 {
            (bg_h * canvas_height as f32).max(60.0)
        } else {
            (extracted_font_size * 1.6 + 40.0).clamp(80.0, canvas_height as f32)
        };

        let corner_radius = (bg_r * card_h).clamp(0.0, card_h / 2.0);

        layers.insert(
            0,
            TemplateLayer {
                layer_id: "lyr_card_bg".to_string(),
                name: "Backdrop Card".to_string(),
                z_index: 0,
                visible: true,
                opacity: 0.95,
                transform: primary_transform.clone(),
                anchor: TextAnchor::Center,
                pivot: (0.5, 0.5),
                motion: None,
                kind: TemplateLayerKind::Shape(TemplateShapeLayerData {
                    shape_kind: TemplateShapeKind::RoundedRectangle,
                    width: card_w,
                    height: card_h,
                    fill_color: Some(bg_col),
                    stroke_color: None,
                    stroke_width: 0.0,
                    corner_radius,
                }),
            },
        );
    }

    let sanitized_id = format!(
        "capcut_tpl_{}",
        profile.template_resource_id.as_deref().unwrap_or("import")
    );

    TemplateDefinition {
        id: sanitized_id,
        version: 1,
        name: profile
            .template_name
            .clone()
            .unwrap_or_else(|| "Imported Template".to_string()),
        category: "CapCut Import".to_string(),
        description: format!("Imported from CapCut draft '{}'", profile.draft_path),
        tags: vec!["capcut".to_string(), "imported".to_string()],
        parameters,
        bindings,
        layers,
        resources: resource_manifest,
        responsive_policy: ResponsivePolicy {
            reference_width: canvas_width,
            reference_height: canvas_height,
            auto_wrap: true,
            auto_fit: false,
            safe_margin: 80.0,
            ..Default::default()
        },
        default_duration: default_dur,
    }
}

// =========================================================================
// Transactional Catalog Commit & Project Apply
// =========================================================================

/// Atomically saves a converted definition to the template catalog and verifies source revision.
/// Implements full preflight checks, staging, and rollback on any failure.
pub fn save_converted_to_catalog(
    catalog: &TemplateCatalog,
    report: &CapCutConversionReport,
    asset_source_files: &[(String, PathBuf)],
    expected_source_hash: Option<(&Path, &str)>,
) -> Result<String, CapCutImportError> {
    // 1. Revision check
    if let Some((path, expected_hash)) = expected_source_hash {
        let current_hash = compute_file_sha256_bounded(path, MAX_STREAM_HASH_BYTES, None)?;
        if !current_hash.eq_ignore_ascii_case(expected_hash) {
            return Err(CapCutImportError::StaleSourceRevision {
                expected: expected_hash.to_string(),
                actual: current_hash,
            });
        }
    }

    let definition = report.converted_definition.as_ref().ok_or_else(|| {
        CapCutImportError::UnsupportedSource(
            "Cannot save to catalog: report does not contain a valid converted template definition"
                .to_string(),
        )
    })?;

    // 2. Preflight checks
    if !is_valid_id(&definition.id) {
        return Err(CapCutImportError::SecurityViolation(format!(
            "Invalid template ID '{}'",
            definition.id
        )));
    }
    definition
        .validate()
        .map_err(|e| CapCutImportError::Validation(e.to_string()))?;

    // Immutable curated conflict check
    if let Some(bundled) = resolve_curated_definition(&definition.id, definition.version) {
        if &bundled != definition {
            return Err(CapCutImportError::Validation(format!(
                "Cannot overwrite immutable curated template '{}:{}'",
                definition.id, definition.version
            )));
        }
    }

    // Preflight user definition path
    let def_target_path = catalog
        .definition_file_path(&definition.id, definition.version)
        .map_err(|e| CapCutImportError::Validation(e.to_string()))?;

    if def_target_path.exists() {
        if let Ok(content) = fs::read_to_string(&def_target_path) {
            if let Ok(existing_def) = serde_json::from_str::<TemplateDefinition>(&content) {
                if &existing_def != definition {
                    return Err(CapCutImportError::Validation(format!(
                        "Cannot overwrite immutable user template '{}:{}'",
                        definition.id, definition.version
                    )));
                }
            }
        }
    }

    let resources_dir = catalog.resources_dir();

    // Preflight: Resource manifest completeness and content integrity
    // Verify SHA-256 and byte size for EVERY manifest resource reference, including catalog-present files
    for res_ref in &definition.resources.resources {
        let incoming_asset = asset_source_files
            .iter()
            .find(|(fname, _)| fname == &res_ref.file_name);
        let catalog_file = resources_dir.join(&res_ref.file_name);
        let in_catalog = catalog_file.is_file();

        if let Some((_, source_path)) = incoming_asset {
            if !is_safe_resource_filename(&res_ref.file_name) {
                return Err(CapCutImportError::SecurityViolation(format!(
                    "Invalid resource filename '{}'",
                    res_ref.file_name
                )));
            }
            if !source_path.is_file() {
                return Err(CapCutImportError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Source asset file '{}' not found", source_path.display()),
                )));
            }
            let meta = fs::metadata(source_path)?;
            if meta.len() != res_ref.byte_size {
                return Err(CapCutImportError::Validation(format!(
                    "Resource byte size mismatch for incoming '{}': manifest declared {}, source file has {}",
                    res_ref.file_name, res_ref.byte_size, meta.len()
                )));
            }
            let computed_sha =
                compute_file_sha256_bounded(source_path, MAX_STREAM_HASH_BYTES, None)?;
            if !computed_sha.eq_ignore_ascii_case(&res_ref.sha256) {
                return Err(CapCutImportError::Validation(format!(
                    "Resource SHA-256 mismatch for incoming '{}': manifest declared {}, source file has {computed_sha}",
                    res_ref.file_name, res_ref.sha256
                )));
            }
        }

        if in_catalog {
            let meta = fs::metadata(&catalog_file)?;
            if meta.len() != res_ref.byte_size {
                return Err(CapCutImportError::Validation(format!(
                    "Catalog resource '{}' byte size mismatch: manifest declared {}, existing file has {}",
                    res_ref.file_name, res_ref.byte_size, meta.len()
                )));
            }
            let existing_sha =
                compute_file_sha256_bounded(&catalog_file, MAX_STREAM_HASH_BYTES, None)?;
            if !existing_sha.eq_ignore_ascii_case(&res_ref.sha256) {
                return Err(CapCutImportError::Validation(format!(
                    "Catalog resource '{}' content mismatch: expected SHA-256 {}, existing file has {existing_sha}",
                    res_ref.file_name, res_ref.sha256
                )));
            }
        }

        if incoming_asset.is_none() && !in_catalog {
            if res_ref.required {
                return Err(CapCutImportError::Validation(format!(
                    "Resource manifest incomplete: required resource '{}' ({}) is missing from asset sources and catalog",
                    res_ref.resource_id, res_ref.file_name
                )));
            }
        }
    }

    // Preflight all asset source files: every asset source file must exist and have safe name
    for (file_name, source_path) in asset_source_files {
        if !is_safe_resource_filename(file_name) {
            return Err(CapCutImportError::SecurityViolation(format!(
                "Invalid resource filename '{file_name}'"
            )));
        }
        if !source_path.is_file() {
            return Err(CapCutImportError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Source asset file '{}' not found", source_path.display()),
            )));
        }

        let computed_sha = compute_file_sha256_bounded(source_path, MAX_STREAM_HASH_BYTES, None)?;
        if let Some(res_ref) = definition
            .resources
            .resources
            .iter()
            .find(|r| &r.file_name == file_name)
        {
            if !computed_sha.eq_ignore_ascii_case(&res_ref.sha256) {
                return Err(CapCutImportError::Validation(format!(
                    "Resource SHA-256 mismatch for '{file_name}': manifest declared {}, source file has {computed_sha}",
                    res_ref.sha256
                )));
            }
        }

        // Destination filename / content conflict check:
        // If file already exists in catalog, its SHA must match the incoming asset
        let target_path = resources_dir.join(file_name);
        if target_path.exists() {
            let existing_sha =
                compute_file_sha256_bounded(&target_path, MAX_STREAM_HASH_BYTES, None)?;
            if !existing_sha.eq_ignore_ascii_case(&computed_sha) {
                return Err(CapCutImportError::Validation(format!(
                    "Destination resource filename conflict for '{file_name}': existing file in catalog has different SHA-256 ({existing_sha} vs incoming {computed_sha})"
                )));
            }
        }
    }

    // Ensure catalog directories
    catalog
        .ensure_dirs()
        .map_err(|e| CapCutImportError::Io(std::io::Error::other(e.to_string())))?;

    // 3. Staging and Rollback state tracking
    let mut staged_created_files: Vec<PathBuf> = Vec::new();
    let mut staged_backup_files: Vec<(PathBuf, Vec<u8>)> = Vec::new();
    let mut temp_paths: Vec<PathBuf> = Vec::new();

    let session_id = uuid::Uuid::new_v4();

    // Helper closure for rollback
    let rollback = |created: &[PathBuf], backups: &[(PathBuf, Vec<u8>)], temps: &[PathBuf]| {
        let mut errs = Vec::new();
        for t in temps {
            if t.exists() {
                if let Err(e) = fs::remove_file(t) {
                    errs.push(format!("Failed to clean temp file {}: {e}", t.display()));
                }
            }
        }
        for c in created {
            if c.exists() {
                if let Err(e) = fs::remove_file(c) {
                    errs.push(format!(
                        "Failed to remove created file {}: {e}",
                        c.display()
                    ));
                }
            }
        }
        for (path, bytes) in backups {
            if let Err(e) = fs::write(path, bytes) {
                errs.push(format!("Failed to restore backup {}: {e}", path.display()));
            }
        }
        if !errs.is_empty() {
            eprintln!("Rollback cleanup warnings: {}", errs.join("; "));
        }
    };

    // Copy each asset via staging temp file
    for (file_name, source_path) in asset_source_files {
        let target_path = resources_dir.join(file_name);
        let tmp_path = resources_dir.join(format!("{file_name}.stage_{session_id}"));
        temp_paths.push(tmp_path.clone());

        if let Err(e) = fs::copy(source_path, &tmp_path) {
            rollback(&staged_created_files, &staged_backup_files, &temp_paths);
            return Err(CapCutImportError::Io(e));
        }

        // Flush and sync staged asset file before commit
        if let Ok(f) = File::open(&tmp_path) {
            let _ = f.sync_all();
        }

        if target_path.exists() {
            match fs::read(&target_path) {
                Ok(bytes) => staged_backup_files.push((target_path.clone(), bytes)),
                Err(e) => {
                    rollback(&staged_created_files, &staged_backup_files, &temp_paths);
                    return Err(CapCutImportError::Io(e));
                }
            }
        } else {
            staged_created_files.push(target_path.clone());
        }

        if let Err(e) = fs::rename(&tmp_path, &target_path) {
            rollback(&staged_created_files, &staged_backup_files, &temp_paths);
            return Err(CapCutImportError::Io(e));
        }
    }

    // Stage definition write
    let tmp_def_path = def_target_path.with_extension(format!("stage_{session_id}.json"));
    temp_paths.push(tmp_def_path.clone());

    let def_json = match serde_json::to_string_pretty(definition) {
        Ok(j) => j,
        Err(e) => {
            rollback(&staged_created_files, &staged_backup_files, &temp_paths);
            return Err(CapCutImportError::Validation(e.to_string()));
        }
    };

    {
        let mut def_file = match File::create(&tmp_def_path) {
            Ok(f) => f,
            Err(e) => {
                rollback(&staged_created_files, &staged_backup_files, &temp_paths);
                return Err(CapCutImportError::Io(e));
            }
        };
        if let Err(e) = def_file.write_all(def_json.as_bytes()) {
            rollback(&staged_created_files, &staged_backup_files, &temp_paths);
            return Err(CapCutImportError::Io(e));
        }
        if let Err(e) = def_file.sync_all() {
            rollback(&staged_created_files, &staged_backup_files, &temp_paths);
            return Err(CapCutImportError::Io(e));
        }
    }

    if def_target_path.exists() {
        match fs::read(&def_target_path) {
            Ok(bytes) => staged_backup_files.push((def_target_path.clone(), bytes)),
            Err(e) => {
                rollback(&staged_created_files, &staged_backup_files, &temp_paths);
                return Err(CapCutImportError::Io(e));
            }
        }
    } else {
        staged_created_files.push(def_target_path.clone());
    }

    if let Err(e) = fs::rename(&tmp_def_path, &def_target_path) {
        rollback(&staged_created_files, &staged_backup_files, &temp_paths);
        return Err(CapCutImportError::Io(e));
    }

    // Clean up temporary files
    for t in &temp_paths {
        if t.exists() {
            let _ = fs::remove_file(t);
        }
    }

    Ok(format!("{}_v{}", definition.id, definition.version))
}

/// Builds an atomic `CommandEnvelope` to apply an imported template and text variant to a timeline item.
pub fn build_atomic_apply_envelope(
    project_id: &str,
    base_revision: u64,
    item_id: &str,
    template_id: &str,
    template_version: u32,
    text_variant: Option<&str>,
) -> CommandEnvelope {
    let mut initial_parameters = HashMap::new();
    if let Some(variant) = text_variant {
        initial_parameters.insert(
            "title".to_string(),
            TemplateParameterValue::Text(variant.to_string()),
        );
    }

    let initial = if initial_parameters.is_empty() {
        None
    } else {
        Some(initial_parameters)
    };

    let operations = vec![OperationEnvelope {
        operation_id: format!("op_apply_{}", uuid::Uuid::new_v4()),
        operation: CommandOperation::TemplateApply {
            item_id: item_id.to_string(),
            template_id: template_id.to_string(),
            template_version,
            initial_parameters: initial,
        },
    }];

    let tx_id = format!("tx_capcut_apply_{}", uuid::Uuid::new_v4());

    CommandEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        transaction_id: Some(tx_id),
        project_id: project_id.to_string(),
        base_revision,
        client_id: None,
        client_epoch: None,
        sequence_number: None,
        dry_run: false,
        operations,
    }
}

/// Request parameters for staged template apply with mandatory guardrails.
pub struct ApplyTemplateGuardrailsRequest<'a> {
    pub staging_token: &'a str,
    pub expected_revision: u64,
    pub source_draft_path: &'a Path,
    pub expected_source_hash: &'a str,
    pub expected_resource_fingerprints: &'a HashMap<PathBuf, String>,
    pub item_id: &'a str,
    pub template_id: &'a str,
    pub template_version: u32,
    pub text_variant: Option<&'a str>,
    pub cancel: Option<&'a AtomicBool>,
}

/// Executes a template apply command through CommandService with full guardrails:
/// 1. Cancellation check before execution.
/// 2. Mandatory staging token validation and expiration check.
/// 3. Stable commit identity across duplicate retries (idempotent apply).
/// 4. Source draft hash verification against `expected_source_hash`.
/// 5. Consumed resource fingerprint verification against `expected_resource_fingerprints`.
/// 6. Project revision concurrency check against `expected_revision`.
/// 7. Cancellation re-check immediately before commit.
/// 8. Execution via CommandService and recording in HistoryManager.
/// 9. Automatic cleanup of staged extraction directories on successful commit.
fn apply_request_fingerprint(
    project_id: &str,
    request: &ApplyTemplateGuardrailsRequest<'_>,
) -> String {
    format!(
        "{}|{}|{}|{}|{}",
        project_id,
        request.item_id,
        request.template_id,
        request.template_version,
        request.text_variant.unwrap_or("")
    )
}

pub fn execute_staged_template_apply(
    service: &mut CommandService,
    mut history: Option<&mut HistoryManager>,
    request: ApplyTemplateGuardrailsRequest<'_>,
) -> Result<CommandReceipt, CapCutImportError> {
    // 1. Initial cancellation check
    if let Some(c) = request.cancel {
        if c.load(Ordering::Relaxed) {
            return Err(CapCutImportError::Cancelled);
        }
    }

    // 2. Validate staging token from registry
    let mut reg = get_staging_registry().lock().unwrap();
    let record = reg.get_mut(request.staging_token).ok_or_else(|| {
        CapCutImportError::Validation(format!(
            "Staging token '{}' not found or invalid",
            request.staging_token
        ))
    })?;

    // Check token expiration (300 seconds TTL)
    if record.created_at.elapsed() > std::time::Duration::from_secs(300) {
        return Err(CapCutImportError::Validation(format!(
            "Staging token '{}' has expired",
            request.staging_token
        )));
    }

    // 3. Stable commit identity across duplicate retries, bound to project + request
    let current_project_id = service.project().project_id.clone();
    let request_fingerprint = apply_request_fingerprint(&current_project_id, &request);
    if let Some(ref receipt) = record.last_receipt {
        if record.committed_project_id.as_deref() != Some(current_project_id.as_str())
            || record.committed_request_fingerprint.as_deref() != Some(request_fingerprint.as_str())
        {
            return Err(CapCutImportError::Validation(
                "Staging token was already committed for a different project or request"
                    .to_string(),
            ));
        }
        return Ok(receipt.clone());
    }

    // 4. Source draft stale check against the server-owned staging record
    if !record.source_path.exists() {
        return Err(CapCutImportError::Validation(format!(
            "Source draft file missing: {}",
            record.source_path.display()
        )));
    }
    if !request
        .source_draft_path
        .to_string_lossy()
        .eq_ignore_ascii_case(&record.source_path.to_string_lossy())
    {
        return Err(CapCutImportError::StaleRevision(
            "Stale source revision: staged source path does not match the inspected source"
                .to_string(),
        ));
    }
    if !request
        .expected_source_hash
        .eq_ignore_ascii_case(&record.source_sha256)
    {
        return Err(CapCutImportError::StaleRevision(
            "Stale source revision: caller hash does not match the inspected source snapshot"
                .to_string(),
        ));
    }
    let current_hash =
        compute_file_sha256_bounded(&record.source_path, MAX_STREAM_HASH_BYTES, request.cancel)?;
    if !current_hash.eq_ignore_ascii_case(&record.source_sha256) {
        return Err(CapCutImportError::StaleRevision(format!(
            "Stale source revision: expected SHA-256 {}, got {current_hash}",
            record.source_sha256
        )));
    }

    // 5. Consumed resources stale check against the server-owned record.
    for (res_p, expected_hash) in &record.resource_fingerprints {
        if !res_p.exists() {
            return Err(CapCutImportError::Validation(format!(
                "Consumed resource missing: {}",
                res_p.display()
            )));
        }
        let cur_hash = compute_file_sha256_bounded(res_p, MAX_STREAM_HASH_BYTES, request.cancel)?;
        if !cur_hash.eq_ignore_ascii_case(expected_hash) {
            return Err(CapCutImportError::StaleRevision(format!(
                "Consumed resource modified: {} expected SHA-256 {expected_hash}, got {cur_hash}",
                res_p.display()
            )));
        }
        if let Some(caller_hash) = request.expected_resource_fingerprints.get(res_p) {
            if !caller_hash.eq_ignore_ascii_case(expected_hash) {
                return Err(CapCutImportError::Validation(
                    "Caller resource fingerprints do not match the inspected snapshot".to_string(),
                ));
            }
        }
    }

    // 5b. Verify any additional caller-declared consumed resources as well.
    for (res_p, expected_hash) in request.expected_resource_fingerprints {
        if !record.resource_fingerprints.contains_key(res_p) {
            if !res_p.exists() {
                return Err(CapCutImportError::Validation(format!(
                    "Consumed resource missing: {}",
                    res_p.display()
                )));
            }
            let cur_hash =
                compute_file_sha256_bounded(res_p, MAX_STREAM_HASH_BYTES, request.cancel)?;
            if !cur_hash.eq_ignore_ascii_case(expected_hash) {
                return Err(CapCutImportError::StaleRevision(format!(
                    "Consumed resource modified: {} expected SHA-256 {expected_hash}, got {cur_hash}",
                    res_p.display()
                )));
            }
        }
    }

    // 6. Project revision stale check
    let project_id = service.project().project_id.clone();
    let base_revision = service.project().revision;
    let pre_project = service.project().clone();

    if base_revision != request.expected_revision {
        return Err(CapCutImportError::StaleRevision(format!(
            "Stale project revision: expected {}, current is {base_revision}",
            request.expected_revision
        )));
    }

    // 7. Recheck cancellation immediately before commit
    if let Some(c) = request.cancel {
        if c.load(Ordering::Relaxed) {
            return Err(CapCutImportError::Cancelled);
        }
    }

    // 8. Execute atomic apply through CommandService
    let envelope = build_atomic_apply_envelope(
        &project_id,
        base_revision,
        request.item_id,
        request.template_id,
        request.template_version,
        request.text_variant,
    );

    let receipt = service
        .execute(envelope)
        .map_err(|e| CapCutImportError::Command(format!("{e:?}")))?;

    if let Some(hist) = history.as_mut() {
        hist.record_edit(HistoryEntry {
            description: format!("Apply CapCut Template {}", request.template_id),
            transaction_id: receipt.transaction_id.clone(),
            prior_snapshot: pre_project,
            resulting_snapshot: service.project().clone(),
        });
    }

    // 9. Store receipt in staging record for retry idempotency
    record.last_receipt = Some(receipt.clone());
    record.committed_project_revision = Some(service.project().revision);
    record.committed_project_id = Some(current_project_id);
    record.committed_request_fingerprint = Some(request_fingerprint);

    // 10. Clean up scratch extraction directory on success
    if let Some(ref scratch) = record.scratch_dir {
        if scratch.exists() {
            let _ = fs::remove_dir_all(scratch);
        }
    }

    Ok(receipt)
}
