//! Versioned, reviewable edit plans validated by the canonical Command Service.

use crate::modules::command::{
    CommandEnvelope, CommandOperation, CommandService, DiffReport, OperationEnvelope,
};
use crate::modules::domain::time::{RationalTime, TimeRange};
use crate::modules::domain::{ItemKind, MediaKind, Project, TextAnchor, TextStyle, TrackKind};
use crate::modules::media::captions::{parse_caption_bytes, CaptionCue, CaptionFormat};
use crate::modules::media::visual_scene::{
    aggregate_source_fingerprint, VisualSceneAnalysisReport, VisualSceneSource,
    VISUAL_SCENE_ALGORITHM_VERSION, VISUAL_SCENE_SCHEMA_VERSION,
};
use schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};

pub const EDIT_PLAN_SCHEMA_VERSION: u32 = 1;
pub const DETERMINISTIC_DRAFT_PROVIDER: &str = "deterministic-mock-v1";
pub const CALLER_ORDER_PROVIDER: &str = "caller-order-v1";
pub const LOCAL_LM_STUDIO_PROVIDER: &str = "lm-studio-local";
pub const MAX_DRAFT_DURATION_FRAMES: u64 = 54_000;
const LOCAL_LM_STUDIO_CHAT_ENDPOINT: &str = "http://127.0.0.1:1234/v1/chat/completions";
const LOCAL_LM_STUDIO_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(90);
const MAX_TRANSCRIPT_EVIDENCE_SOURCES: usize = 8;
const MAX_TRANSCRIPT_EVIDENCE_SEGMENTS: usize = 512;
const MAX_TRANSCRIPT_EVIDENCE_BYTES: usize = 64 * 1024;
const MAX_TRANSCRIPT_SEGMENTS_PER_SCENE: usize = 24;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SceneSelectionSuggestion {
    pub provider: String,
    pub model: String,
    pub ordered_scene_ids: Vec<String>,
    pub rationale: String,
    pub selected_duration_micros: u64,
    pub target_duration_seconds: Option<u32>,
    #[serde(default)]
    pub opening_title_suggestion: Option<String>,
    #[serde(default)]
    pub constraints: SceneSelectionConstraints,
    #[serde(default)]
    pub transcript_evidence_sources: Vec<TranscriptEvidenceProvenance>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSceneEvidence {
    pub asset_id: String,
    pub source_fingerprint: String,
    pub analyzed_duration_micros: u64,
    pub segments: Vec<TranscriptSceneEvidenceSegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSceneEvidenceSegment {
    pub start_micros: u64,
    pub end_micros: u64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptEvidenceProvenance {
    pub asset_id: String,
    pub source_fingerprint: String,
    pub segment_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SceneSelectionConstraints {
    /// Empty means every source asset in the report is allowed.
    #[serde(default)]
    pub allowed_asset_ids: Vec<String>,
    #[serde(default)]
    pub must_keep_scene_ids: Vec<String>,
    #[serde(default)]
    pub must_omit_scene_ids: Vec<String>,
    /// Editorial guidance for scene ranking only; it does not create caption operations.
    #[serde(default)]
    pub caption_intent: Option<String>,
    /// Editorial guidance for scene ranking only; it does not add music or audio operations.
    #[serde(default)]
    pub audio_intent: Option<String>,
    /// Ask the local planner for a review-only opening title suggestion.
    #[serde(default)]
    pub suggest_opening_title: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SceneCanvasPreset {
    #[serde(rename = "landscape_16x9")]
    Landscape16x9,
    #[serde(rename = "portrait_9x16")]
    Portrait9x16,
    #[serde(rename = "square_1x1")]
    Square1x1,
    #[serde(rename = "portrait_4x5")]
    Portrait4x5,
}

impl SceneCanvasPreset {
    pub fn dimensions(self) -> (u32, u32) {
        match self {
            Self::Landscape16x9 => (1920, 1080),
            Self::Portrait9x16 => (1080, 1920),
            Self::Square1x1 => (1080, 1080),
            Self::Portrait4x5 => (1080, 1350),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SceneTitleTemplate {
    CleanLowerThird,
    BoldHeadline,
    CenterPop,
    SubtitleCard,
    QuoteCard,
    CallToAction,
}

impl SceneTitleTemplate {
    fn template_id(self) -> &'static str {
        match self {
            Self::CleanLowerThird => "clean-lower-third",
            Self::BoldHeadline => "bold-headline",
            Self::CenterPop => "center-pop",
            Self::SubtitleCard => "subtitle-card",
            Self::QuoteCard => "quote-card",
            Self::CallToAction => "call-to-action",
        }
    }
}

impl SceneSelectionConstraints {
    pub fn validate_intents(&self) -> Result<(), String> {
        for (label, value) in [
            ("Caption intent", self.caption_intent.as_deref()),
            ("Audio intent", self.audio_intent.as_deref()),
        ] {
            if value.is_some_and(|value| {
                value.trim().is_empty() || value.len() > 500 || value.chars().any(char::is_control)
            }) {
                return Err(format!(
                    "{label} must contain 1 to 500 UTF-8 bytes without control characters"
                ));
            }
        }
        Ok(())
    }

    pub fn validate_allowed_assets(&self, available_asset_ids: &[String]) -> Result<(), String> {
        let mut seen = std::collections::HashSet::new();
        if self.allowed_asset_ids.iter().any(|id| {
            id.trim().is_empty() || !available_asset_ids.contains(id) || !seen.insert(id.as_str())
        }) {
            return Err(
                "Allowed source asset IDs must be unique and present in the report".to_string(),
            );
        }
        Ok(())
    }

    fn allows_asset(&self, asset_id: &str) -> bool {
        self.allowed_asset_ids.is_empty() || self.allowed_asset_ids.iter().any(|id| id == asset_id)
    }

    pub fn validate(&self, available_ids: &[String]) -> Result<(), String> {
        let mut keep = std::collections::HashSet::new();
        if self.must_keep_scene_ids.iter().any(|id| {
            id.trim().is_empty() || !available_ids.contains(id) || !keep.insert(id.as_str())
        }) {
            return Err("Must-keep scene IDs must be unique and present in the report".to_string());
        }
        let mut omit = std::collections::HashSet::new();
        if self.must_omit_scene_ids.iter().any(|id| {
            id.trim().is_empty() || !available_ids.contains(id) || !omit.insert(id.as_str())
        }) {
            return Err("Must-omit scene IDs must be unique and present in the report".to_string());
        }
        if keep.iter().any(|id| omit.contains(id)) {
            return Err("A scene cannot be both must-keep and must-omit".to_string());
        }
        Ok(())
    }

    pub fn validate_selection(&self, selected_ids: &[String]) -> Result<(), String> {
        if self
            .must_keep_scene_ids
            .iter()
            .any(|id| !selected_ids.contains(id))
        {
            return Err("Local scene planner omitted a required must-keep scene".to_string());
        }
        if self
            .must_omit_scene_ids
            .iter()
            .any(|id| selected_ids.contains(id))
        {
            return Err("Local scene planner selected a must-omit scene".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LocalSceneSelection {
    selected_scene_ids: Vec<String>,
    rationale: String,
    #[serde(default)]
    opening_title_suggestion: Option<String>,
}

/// Ask the explicitly selected local model to choose and order scenes from a supplied report.
/// No project paths, asset names, or media bytes are sent; the result remains review-only.
pub async fn suggest_visual_scene_selection(
    project: &Project,
    goal: &str,
    report: &VisualSceneAnalysisReport,
    target_duration_seconds: Option<u32>,
    model: &str,
) -> Result<SceneSelectionSuggestion, String> {
    suggest_visual_scene_selection_with_constraints(
        project,
        goal,
        report,
        target_duration_seconds,
        model,
        SceneSelectionConstraints::default(),
    )
    .await
}

pub async fn suggest_visual_scene_selection_with_constraints(
    project: &Project,
    goal: &str,
    report: &VisualSceneAnalysisReport,
    target_duration_seconds: Option<u32>,
    model: &str,
    constraints: SceneSelectionConstraints,
) -> Result<SceneSelectionSuggestion, String> {
    suggest_visual_scene_selection_at(
        project,
        goal,
        report,
        target_duration_seconds,
        model,
        constraints,
        LOCAL_LM_STUDIO_CHAT_ENDPOINT,
        LOCAL_LM_STUDIO_TIMEOUT,
    )
    .await
}

pub async fn suggest_visual_scene_selection_with_constraints_and_transcript(
    project: &Project,
    goal: &str,
    report: &VisualSceneAnalysisReport,
    target_duration_seconds: Option<u32>,
    model: &str,
    constraints: SceneSelectionConstraints,
    transcript_evidence: Vec<TranscriptSceneEvidence>,
) -> Result<SceneSelectionSuggestion, String> {
    suggest_visual_scene_selection_at_with_transcript(
        project,
        goal,
        report,
        target_duration_seconds,
        model,
        constraints,
        transcript_evidence,
        LOCAL_LM_STUDIO_CHAT_ENDPOINT,
        LOCAL_LM_STUDIO_TIMEOUT,
    )
    .await
}

async fn suggest_visual_scene_selection_at(
    project: &Project,
    goal: &str,
    report: &VisualSceneAnalysisReport,
    target_duration_seconds: Option<u32>,
    model: &str,
    constraints: SceneSelectionConstraints,
    endpoint: &str,
    timeout: std::time::Duration,
) -> Result<SceneSelectionSuggestion, String> {
    suggest_visual_scene_selection_at_with_transcript(
        project,
        goal,
        report,
        target_duration_seconds,
        model,
        constraints,
        Vec::new(),
        endpoint,
        timeout,
    )
    .await
}

async fn suggest_visual_scene_selection_at_with_transcript(
    project: &Project,
    goal: &str,
    report: &VisualSceneAnalysisReport,
    target_duration_seconds: Option<u32>,
    model: &str,
    constraints: SceneSelectionConstraints,
    transcript_evidence: Vec<TranscriptSceneEvidence>,
    endpoint: &str,
    timeout: std::time::Duration,
) -> Result<SceneSelectionSuggestion, String> {
    if goal.trim().is_empty() || goal.len() > 4_000 {
        return Err("Goal must contain 1 to 4000 bytes".to_string());
    }
    if model.trim().is_empty() || model.len() > 160 {
        return Err("Enter a valid local model ID".to_string());
    }
    if target_duration_seconds.is_some_and(|seconds| seconds == 0 || seconds > 1_800) {
        return Err("Target duration must be between 1 and 1800 seconds".to_string());
    }
    constraints.validate_intents()?;
    if report.scenes.is_empty()
        || report.scenes.len() > crate::modules::media::visual_scene::MAX_SCENES_PER_REQUEST
    {
        return Err("Visual scene report must contain 1 to 32 scenes".to_string());
    }
    // Validate every report entry and its current source fingerprint before serializing any
    // report text for the local model. The first scene is enough to trigger report-wide checks.
    build_visual_scene_edit_plan(
        project,
        goal,
        report,
        std::slice::from_ref(&report.scenes[0].scene_id),
    )?;
    let available_asset_ids = report
        .scenes
        .iter()
        .map(|scene| scene.asset_id.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    constraints.validate_allowed_assets(&available_asset_ids)?;
    let allowed_ids = report
        .scenes
        .iter()
        .filter(|scene| constraints.allows_asset(&scene.asset_id))
        .map(|scene| scene.scene_id.clone())
        .collect::<Vec<_>>();
    if allowed_ids.is_empty() {
        return Err("Select at least one source asset containing an analyzed scene".to_string());
    }
    constraints.validate(&allowed_ids)?;
    let (transcript_by_scene, transcript_evidence_sources) =
        validate_transcript_scene_evidence(project, report, &constraints, transcript_evidence)?;
    let input_scenes = report
        .scenes
        .iter()
        .filter(|scene| constraints.allows_asset(&scene.asset_id))
        .map(|scene| {
            let mut value = serde_json::json!({
                "sceneId": scene.scene_id,
                "startMicros": scene.source_start_micros,
                "endMicros": scene.source_end_micros,
                "caption": scene.caption,
                "tags": scene.tags,
                "analysisRationale": scene.rationale,
            });
            if let Some(segments) = transcript_by_scene.get(&scene.scene_id) {
                value["transcriptEvidence"] = serde_json::json!(segments);
            }
            value
        })
        .collect::<Vec<_>>();
    let schema = serde_json::json!({
        "name": "scene_selection",
        "strict": true,
        "schema": {
            "type": "object",
            "properties": {
                "selectedSceneIds": {
                    "type": "array",
                    "items": { "type": "string", "enum": allowed_ids },
                    "minItems": 1,
                    "maxItems": allowed_ids.len(),
                },
                "rationale": { "type": "string" },
                "openingTitleSuggestion": { "type": ["string", "null"], "maxLength": 240 }
            },
            "required": ["selectedSceneIds", "rationale", "openingTitleSuggestion"],
            "additionalProperties": false
        }
    });
    let system_message = if transcript_by_scene.is_empty() {
        "Select a useful subset and order of the supplied scene IDs for the user's video brief. Use only supplied IDs, at most once each, and return at least one. Include every must-keep scene and exclude every must-omit scene. Consider the optional target duration using scene interval lengths. Caption and audio intent are selection guidance only: do not claim to create subtitles, narration, music, or audio edits. Do not invent visual facts; captions are unverified model suggestions. Explain selection briefly. If suggestOpeningTitle is true, also return a concise openingTitleSuggestion grounded only in the brief and supplied scene descriptions; otherwise return null. The title is only a suggestion for human review, must contain no control characters, and must be at most 240 UTF-8 bytes."
    } else {
        "Select a useful subset and order of the supplied scene IDs for the user's video brief. Use only supplied IDs, at most once each, and return at least one. Include every must-keep scene and exclude every must-omit scene. Consider the optional target duration using scene interval lengths. Caption and audio intent are selection guidance only: do not claim to create subtitles, narration, music, or audio edits. Do not invent visual facts; captions are unverified model suggestions. Transcript excerpts are unverified supplied text and may come from imperfect speech recognition; use them only as a ranking hint and never state them as verified fact. Explain selection briefly. If suggestOpeningTitle is true, also return a concise openingTitleSuggestion grounded only in the brief and supplied scene descriptions; otherwise return null. The title is only a suggestion for human review, must contain no control characters, and must be at most 240 UTF-8 bytes."
    };
    let mut model_constraints = constraints.clone();
    model_constraints.allowed_asset_ids.clear();
    let request = serde_json::json!({
        "model": model.trim(),
        "temperature": 0,
        "max_tokens": 2048,
        "response_format": { "type": "json_schema", "json_schema": schema },
        "messages": [
            {
                "role": "system",
                "content": system_message
            },
            {
                "role": "user",
                "content": serde_json::json!({
                    "brief": goal,
                    "targetDurationSeconds": target_duration_seconds,
                    "constraints": model_constraints,
                    "captionIntent": constraints.caption_intent,
                    "audioIntent": constraints.audio_intent,
                    "suggestOpeningTitle": constraints.suggest_opening_title,
                    "scenes": input_scenes,
                }).to_string()
            }
        ]
    });
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|error| format!("Unable to initialize local scene planner request: {error}"))?;
    let response = client
        .post(endpoint)
        .json(&request)
        .send()
        .await
        .map_err(|error| {
            if error.is_timeout() {
                format!(
                    "Local scene planner timed out after {:.1} seconds",
                    timeout.as_secs_f64()
                )
            } else {
                format!("Local LM Studio is unavailable at 127.0.0.1:1234: {error}")
            }
        })?
        .error_for_status()
        .map_err(|error| format!("Local scene planner request failed: {error}"))?;
    let completion: LocalChatCompletion = response
        .json()
        .await
        .map_err(|error| format!("Local scene planner returned an invalid response: {error}"))?;
    let choice = completion
        .choices
        .first()
        .ok_or_else(|| "Local scene planner returned no choices".to_string())?;
    let content = choice.message.content.as_str();
    let selection: LocalSceneSelection = serde_json::from_str(content)
        .map_err(|error| {
            format!(
                "Local scene planner returned invalid selection JSON: {error} (finish reason: {})",
                choice.finish_reason.as_deref().unwrap_or("unknown")
            )
        })?;
    if selection.selected_scene_ids.is_empty()
        || selection.selected_scene_ids.len() > allowed_ids.len()
        || selection.rationale.trim().is_empty()
        || selection.rationale.len() > 2_000
    {
        return Err(
            "Local scene planner returned an empty, oversized, or unexplained selection"
                .to_string(),
        );
    }
    let opening_title_suggestion = validate_opening_title_suggestion(
        selection.opening_title_suggestion,
        constraints.suggest_opening_title,
    )?;
    let mut seen = std::collections::HashSet::new();
    if selection
        .selected_scene_ids
        .iter()
        .any(|id| !allowed_ids.contains(id) || !seen.insert(id))
    {
        return Err("Local scene planner returned an unknown or duplicate scene ID".to_string());
    }
    constraints.validate_selection(&selection.selected_scene_ids)?;
    // Reuse Phase 45 validation: verify report/schema/source fingerprints and ensure the
    // selected source ranges produce a valid revision-bound edit plan before returning IDs.
    build_visual_scene_edit_plan(project, goal, report, &selection.selected_scene_ids)?;
    let selected_duration_micros =
        selection
            .selected_scene_ids
            .iter()
            .try_fold(0_u64, |total, id| {
                let scene = report
                    .scenes
                    .iter()
                    .find(|scene| scene.scene_id == *id)
                    .ok_or_else(|| {
                        "Local scene planner returned an unknown scene ID".to_string()
                    })?;
                total
                    .checked_add(scene.source_end_micros - scene.source_start_micros)
                    .ok_or_else(|| "Selected scene duration exceeds supported range".to_string())
            })?;
    Ok(SceneSelectionSuggestion {
        provider: LOCAL_LM_STUDIO_PROVIDER.to_string(),
        model: model.trim().to_string(),
        ordered_scene_ids: selection.selected_scene_ids,
        rationale: selection.rationale.trim().to_string(),
        selected_duration_micros,
        target_duration_seconds,
        opening_title_suggestion,
        constraints,
        transcript_evidence_sources,
    })
}

fn validate_transcript_scene_evidence(
    project: &Project,
    report: &VisualSceneAnalysisReport,
    constraints: &SceneSelectionConstraints,
    evidence: Vec<TranscriptSceneEvidence>,
) -> Result<
    (
        std::collections::HashMap<String, Vec<TranscriptSceneEvidenceSegment>>,
        Vec<TranscriptEvidenceProvenance>,
    ),
    String,
> {
    if evidence.len() > MAX_TRANSCRIPT_EVIDENCE_SOURCES {
        return Err("Transcript evidence supports at most 8 source assets".into());
    }
    let mut seen_assets = std::collections::HashSet::new();
    let mut total_segments = 0_usize;
    let mut total_bytes = 0_usize;
    let available_asset_ids = report
        .scenes
        .iter()
        .map(|scene| scene.asset_id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let mut validated = Vec::with_capacity(evidence.len());

    for source in evidence {
        if source.asset_id.trim().is_empty() || !seen_assets.insert(source.asset_id.clone()) {
            return Err("Transcript evidence asset IDs must be non-empty and unique".into());
        }
        if !available_asset_ids.contains(source.asset_id.as_str())
            || !constraints.allows_asset(&source.asset_id)
        {
            return Err(
                "Transcript evidence must belong to an allowed asset with analyzed scenes".into(),
            );
        }
        let asset = project
            .assets
            .iter()
            .find(|asset| asset.asset_id == source.asset_id)
            .ok_or_else(|| {
                "Transcript evidence references an unavailable project asset".to_string()
            })?;
        if asset.media_kind != MediaKind::Video {
            return Err("Transcript evidence source must be a video asset".into());
        }
        if source.source_fingerprint.len() != 64
            || !source
                .source_fingerprint
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("Transcript evidence source fingerprint is invalid".into());
        }
        let current_fingerprint = crate::modules::media::ffmpeg::FFmpegService::source_fingerprint(
            std::path::Path::new(&asset.path),
        )
        .map_err(|_| "Transcript evidence source is unavailable; analyze it again".to_string())?;
        if current_fingerprint != source.source_fingerprint {
            return Err(
                "Transcript evidence is stale for the current source; analyze it again".into(),
            );
        }
        if source.analyzed_duration_micros == 0 || source.segments.is_empty() {
            return Err(
                "Transcript evidence must include a positive analyzed duration and segments".into(),
            );
        }
        total_segments = total_segments.saturating_add(source.segments.len());
        if total_segments > MAX_TRANSCRIPT_EVIDENCE_SEGMENTS {
            return Err("Transcript evidence supports at most 512 segments".into());
        }
        for segment in &source.segments {
            if segment.start_micros >= segment.end_micros
                || segment.end_micros > source.analyzed_duration_micros
            {
                return Err(
                    "Transcript evidence segment is outside its analyzed source range".into(),
                );
            }
            if segment.text.trim().is_empty()
                || segment.text.len() > 2_000
                || segment.text.chars().any(char::is_control)
            {
                return Err("Transcript evidence text must be 1 to 2000 UTF-8 bytes without control characters".into());
            }
            total_bytes = total_bytes.saturating_add(segment.text.len());
            if total_bytes > MAX_TRANSCRIPT_EVIDENCE_BYTES {
                return Err("Transcript evidence supports at most 64 KiB of text".into());
            }
        }
        validated.push(source);
    }

    let mut by_scene = std::collections::HashMap::new();
    let mut used_segment_counts = std::collections::HashMap::<String, usize>::new();
    let mut prompt_bytes = 0_usize;
    for scene in report
        .scenes
        .iter()
        .filter(|scene| constraints.allows_asset(&scene.asset_id))
    {
        let Some(source) = validated
            .iter()
            .find(|source| source.asset_id == scene.asset_id)
        else {
            continue;
        };
        let mut overlapping = Vec::new();
        for segment in &source.segments {
            if segment.start_micros < scene.source_end_micros
                && scene.source_start_micros < segment.end_micros
            {
                overlapping.push(segment.clone());
                prompt_bytes = prompt_bytes.saturating_add(segment.text.len());
                if prompt_bytes > MAX_TRANSCRIPT_EVIDENCE_BYTES {
                    return Err(
                        "Overlapping transcript excerpts exceed the 64 KiB model-input limit"
                            .into(),
                    );
                }
            }
        }
        if overlapping.len() > MAX_TRANSCRIPT_SEGMENTS_PER_SCENE {
            return Err(
                "Transcript evidence supports at most 24 overlapping segments per scene".into(),
            );
        }
        if !overlapping.is_empty() {
            *used_segment_counts
                .entry(source.asset_id.clone())
                .or_default() += overlapping.len();
            by_scene.insert(scene.scene_id.clone(), overlapping);
        }
    }
    let provenance = validated
        .into_iter()
        .filter_map(|source| {
            used_segment_counts
                .get(&source.asset_id)
                .copied()
                .map(|segment_count| TranscriptEvidenceProvenance {
                    asset_id: source.asset_id,
                    source_fingerprint: source.source_fingerprint,
                    segment_count,
                })
        })
        .collect();
    Ok((by_scene, provenance))
}

fn validate_opening_title_suggestion(
    title: Option<String>,
    requested: bool,
) -> Result<Option<String>, String> {
    let Some(title) = title else {
        return Ok(None);
    };
    if !requested {
        return Err("Local scene planner returned an opening title that was not requested".into());
    }
    let title = title.trim();
    if title.is_empty() {
        return Ok(None);
    }
    if title.len() > 240 || title.chars().any(char::is_control) {
        return Err(
            "Opening title suggestion must be at most 240 UTF-8 bytes without control characters"
                .into(),
        );
    }
    Ok(Some(title.to_string()))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LocalMediaOrdering {
    ordered_media_ids: Vec<String>,
    rationale: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LocalClipOrdering {
    ordered_item_ids: Vec<String>,
    rationale: String,
}

#[derive(Debug, Clone)]
struct ReorderableClip {
    item_id: String,
    asset_id: String,
    name: String,
    timeline_start_frames: i64,
    duration_frames: i64,
}

#[derive(Debug, Deserialize)]
struct LocalChatCompletion {
    choices: Vec<LocalChatChoice>,
}

#[derive(Debug, Deserialize)]
struct LocalChatChoice {
    message: LocalChatMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LocalChatMessage {
    content: String,
}

/// Ask a user-started LM Studio server to choose an order for explicitly selected media.
/// Media bytes and source paths are never sent; the result still uses the canonical plan builder.
pub async fn build_local_ai_video_draft(
    project: &Project,
    goal: &str,
    media_ids: &[String],
    target_duration_frames: u64,
    audio_asset_id: Option<&str>,
    captions: &[CaptionCue],
    model: &str,
) -> Result<EditPlanDraftReceipt, String> {
    build_local_ai_video_draft_at(
        project,
        goal,
        media_ids,
        target_duration_frames,
        audio_asset_id,
        captions,
        model,
        LOCAL_LM_STUDIO_CHAT_ENDPOINT,
        LOCAL_LM_STUDIO_TIMEOUT,
    )
    .await
}

/// Build a reviewable plan that reorders every video clip on one contiguous, transition-free track.
/// Only clip order changes: the command service preserves each source range and duration.
pub fn build_clip_reorder_plan(
    project: &Project,
    goal: &str,
    track_id: &str,
    selected_item_ids: &[String],
    ordered_item_ids: &[String],
) -> Result<EditPlan, String> {
    if goal.trim().is_empty() {
        return Err("Goal must contain at least one non-whitespace character".to_string());
    }
    if goal.len() > 4000 {
        return Err("Goal must be 4000 bytes or shorter".to_string());
    }
    let clips = collect_reorderable_clips(project, track_id, selected_item_ids)?;
    validate_local_clip_order(ordered_item_ids, selected_item_ids)?;
    let current_order = clips
        .iter()
        .map(|clip| clip.item_id.clone())
        .collect::<Vec<_>>();
    if current_order == ordered_item_ids {
        return Err("Requested clip order does not change the current track".to_string());
    }

    let frame_rate = project.settings.frame_rate;
    let mut current_track_end = clips
        .last()
        .map(|clip| clip.timeline_start_frames + clip.duration_frames)
        .ok_or_else(|| "Selected video track has no clips".to_string())?;
    // Move the clips to a clear temporary range first. This avoids transient overlaps while
    // swapping positions through the canonical sequential CommandService transaction.
    current_track_end = current_track_end
        .checked_add(1)
        .ok_or_else(|| "Track duration exceeds the supported frame range".to_string())?;
    let mut operations = Vec::with_capacity(clips.len() * 2);
    for (index, clip) in clips.iter().enumerate() {
        operations.push(OperationEnvelope {
            operation_id: format!("edit_order_r{}_lift_{index}", project.revision),
            operation: CommandOperation::ClipMove {
                item_id: clip.item_id.clone(),
                target_track_id: Some(track_id.to_string()),
                target_start: RationalTime::new(current_track_end, frame_rate),
            },
        });
        current_track_end = current_track_end
            .checked_add(clip.duration_frames)
            .and_then(|frame| frame.checked_add(1))
            .ok_or_else(|| "Track duration exceeds the supported frame range".to_string())?;
    }

    let first_start = clips[0].timeline_start_frames;
    let clips_by_id = clips
        .iter()
        .map(|clip| (clip.item_id.as_str(), clip))
        .collect::<std::collections::HashMap<_, _>>();
    let mut target_start = first_start;
    for (index, item_id) in ordered_item_ids.iter().enumerate() {
        let clip = clips_by_id
            .get(item_id.as_str())
            .ok_or_else(|| format!("Selected clip does not exist on the track: {item_id}"))?;
        operations.push(OperationEnvelope {
            operation_id: format!("edit_order_r{}_place_{index}", project.revision),
            operation: CommandOperation::ClipMove {
                item_id: item_id.clone(),
                target_track_id: Some(track_id.to_string()),
                target_start: RationalTime::new(target_start, frame_rate),
            },
        });
        target_start = target_start
            .checked_add(clip.duration_frames)
            .ok_or_else(|| "Track duration exceeds the supported frame range".to_string())?;
    }

    let mut referenced_media_ids = clips
        .iter()
        .map(|clip| clip.asset_id.clone())
        .collect::<Vec<_>>();
    referenced_media_ids.sort();
    referenced_media_ids.dedup();
    Ok(EditPlan {
        schema_version: EDIT_PLAN_SCHEMA_VERSION,
        goal: goal.trim().to_string(),
        project_id: project.project_id.clone(),
        base_revision: project.revision,
        referenced_media_ids,
        assumptions: vec![
            "Every video clip on the selected track is reordered as one contiguous group.".to_string(),
            "Clip durations and source ranges are preserved; only their timeline positions change.".to_string(),
            "The plan was generated from the persisted project revision and must be regenerated after any revision change.".to_string(),
        ],
        warnings: vec![
            "The planner sees clip names and durations only; it does not inspect image or audio content.".to_string(),
        ],
        operations,
    })
}

/// Build a reviewable, revision-bound plan that shortens one existing video clip from its tail.
/// Source start, timeline start, speed, and all unrelated project items are preserved.
pub fn build_clip_trim_plan(
    project: &Project,
    goal: &str,
    item_id: &str,
    target_duration_frames: u64,
) -> Result<EditPlan, String> {
    if goal.trim().is_empty() || goal.len() > 4000 {
        return Err("Goal must contain 1 to 4000 bytes".to_string());
    }
    if target_duration_frames == 0 || target_duration_frames > MAX_DRAFT_DURATION_FRAMES {
        return Err(format!(
            "Target duration must be between 1 and {MAX_DRAFT_DURATION_FRAMES} frames"
        ));
    }
    let target_duration_frames = i64::try_from(target_duration_frames)
        .map_err(|_| "Target duration exceeds the supported frame range".to_string())?;
    let (track, item) = project
        .tracks
        .iter()
        .find_map(|track| track.find_item(item_id).map(|item| (track, item)))
        .ok_or_else(|| format!("Selected clip does not exist: {item_id}"))?;
    if track.locked {
        return Err("Cannot trim a clip on a locked track".to_string());
    }
    if item.kind != ItemKind::Video {
        return Err("Clip trim planning currently supports video clips only".to_string());
    }
    let asset_id = item
        .asset_id
        .as_deref()
        .ok_or_else(|| "Selected video clip has no linked media asset".to_string())?;
    let asset = project
        .assets
        .iter()
        .find(|asset| asset.asset_id == asset_id && asset.media_kind == MediaKind::Video)
        .ok_or_else(|| format!("Selected clip references missing video media: {asset_id}"))?;
    let old_duration = item
        .timeline_range
        .duration
        .rescaled_to(project.settings.frame_rate)
        .value;
    if target_duration_frames >= old_duration {
        return Err(format!(
            "Target duration must be shorter than the current {old_duration}-frame clip"
        ));
    }

    let timeline_duration = RationalTime::new(target_duration_frames, project.settings.frame_rate);
    let source_duration = item
        .speed
        .source_duration_for(timeline_duration, item.source_range.duration.rate)
        .map_err(|error| {
            format!("Target duration cannot be represented at the clip speed: {error}")
        })?;
    let new_timeline_range = TimeRange::new(item.timeline_range.start, timeline_duration);
    let new_source_range = TimeRange::new(item.source_range.start, source_duration);
    let has_later_clip = track.items.iter().any(|candidate| {
        candidate.item_id != item.item_id
            && candidate
                .timeline_range
                .start
                .semantic_ge(&item.timeline_range.end())
    });

    Ok(EditPlan {
        schema_version: EDIT_PLAN_SCHEMA_VERSION,
        goal: goal.trim().to_string(),
        project_id: project.project_id.clone(),
        base_revision: project.revision,
        referenced_media_ids: vec![asset.asset_id.clone()],
        assumptions: vec![
            "Only the selected video clip is changed; its timeline start and source in-point are preserved.".to_string(),
            format!("The clip tail is shortened from {old_duration} to {target_duration_frames} timeline frames."),
            "The plan is bound to the current project revision and must be regenerated after any project change.".to_string(),
        ],
        warnings: if has_later_clip {
            vec!["Shortening this clip may leave a gap before later clips; review the timeline before applying.".to_string()]
        } else {
            Vec::new()
        },
        operations: vec![OperationEnvelope {
            operation_id: format!("edit_trim_r{}_{}", project.revision, item.item_id),
            operation: CommandOperation::ClipTrim {
                item_id: item.item_id.clone(),
                new_timeline_range,
                new_source_range,
            },
        }],
    })
}

/// Build a revision-bound plan to adjust the clip's linear audio volume.
pub fn build_clip_volume_plan(
    project: &Project,
    goal: &str,
    item_id: &str,
    volume: f32,
) -> Result<EditPlan, String> {
    if goal.trim().is_empty() || goal.len() > 4000 {
        return Err("Goal must contain 1 to 4000 bytes".to_string());
    }
    if !volume.is_finite() || !(0.0..=2.0).contains(&volume) {
        return Err("Clip volume must be a finite linear value from 0.0 to 2.0".to_string());
    }
    let (track, item) = project
        .tracks
        .iter()
        .find_map(|track| track.find_item(item_id).map(|item| (track, item)))
        .ok_or_else(|| format!("Selected clip does not exist: {item_id}"))?;
    if track.locked {
        return Err("Cannot change volume on a locked track".to_string());
    }
    if !matches!(item.kind, ItemKind::Audio | ItemKind::Video) {
        return Err("Volume can only be changed for audio or video clips".to_string());
    }
    if (item.volume - volume).abs() < f32::EPSILON {
        return Err("Requested volume already matches the clip".to_string());
    }

    Ok(EditPlan {
        schema_version: EDIT_PLAN_SCHEMA_VERSION,
        goal: goal.trim().to_string(),
        project_id: project.project_id.clone(),
        base_revision: project.revision,
        referenced_media_ids: Vec::new(),
        assumptions: vec![
            format!(
                "Clip volume changes from {:.3} to {:.3} linear gain (1.0 is unity gain).",
                item.volume, volume
            ),
            "The plan is bound to the current project revision and must be regenerated after any project change.".to_string(),
        ],
        warnings: Vec::new(),
        operations: vec![OperationEnvelope {
            operation_id: format!("edit_volume_r{}_{}", project.revision, item.item_id),
            operation: CommandOperation::ClipVolumeSet {
                item_id: item.item_id.clone(),
                volume,
            },
        }],
    })
}

/// Build a revision-bound plan to replace literal text only in explicitly selected text items.
/// This lets an automation client search transcript/caption content without editing other cues.
pub fn build_text_replace_plan(
    project: &Project,
    goal: &str,
    item_ids: &[String],
    search_text: &str,
    replacement_text: &str,
) -> Result<EditPlan, String> {
    if goal.trim().is_empty() || goal.len() > 4000 {
        return Err("Goal must contain 1 to 4000 bytes".to_string());
    }
    if item_ids.is_empty() || item_ids.len() > 100 {
        return Err("Select between 1 and 100 text items".to_string());
    }
    if item_ids
        .iter()
        .collect::<std::collections::HashSet<_>>()
        .len()
        != item_ids.len()
    {
        return Err("Selected text item IDs must be unique".to_string());
    }
    if search_text.trim().is_empty() || search_text.len() > 1000 {
        return Err("Search text must contain 1 to 1000 bytes".to_string());
    }
    if replacement_text.len() > 4000 {
        return Err("Replacement text must be 4000 bytes or shorter".to_string());
    }

    let mut operations = Vec::new();
    let mut matched_items = 0usize;
    let mut occurrences = 0usize;
    for item_id in item_ids {
        let (track, item) = project
            .tracks
            .iter()
            .find_map(|track| track.find_item(item_id).map(|item| (track, item)))
            .ok_or_else(|| format!("Selected text item does not exist: {item_id}"))?;
        if track.locked {
            return Err(format!(
                "Cannot edit text on locked track: {}",
                track.track_id
            ));
        }
        if item.kind != ItemKind::Text {
            return Err(format!("Selected item is not text: {item_id}"));
        }
        let current_text = item
            .text_content
            .as_deref()
            .ok_or_else(|| format!("Selected text item has no caption text: {item_id}"))?;
        let match_count = current_text.matches(search_text).count();
        if match_count == 0 {
            continue;
        }
        let updated_text = current_text.replace(search_text, replacement_text);
        if updated_text.trim().is_empty() {
            return Err(format!(
                "Replacement would make the caption empty: {item_id}"
            ));
        }
        if updated_text.len() > 4000 {
            return Err(format!("Replacement caption exceeds 4000 bytes: {item_id}"));
        }
        matched_items += 1;
        occurrences += match_count;
        operations.push(OperationEnvelope {
            operation_id: format!("edit_text_r{}_{}", project.revision, item.item_id),
            operation: CommandOperation::ClipTextSet {
                item_id: item.item_id.clone(),
                text: updated_text,
            },
        });
    }
    if operations.is_empty() {
        return Err("Search text was not found in any selected text item".to_string());
    }

    Ok(EditPlan {
        schema_version: EDIT_PLAN_SCHEMA_VERSION,
        goal: goal.trim().to_string(),
        project_id: project.project_id.clone(),
        base_revision: project.revision,
        referenced_media_ids: Vec::new(),
        assumptions: vec![
            format!("Applied a case-sensitive literal replacement in {occurrences} occurrence(s) across {matched_items} selected caption(s)."),
            "Only the explicitly selected text item IDs were searched; unselected captions are untouched.".to_string(),
            "The plan is bound to the current project revision and must be regenerated after any project change.".to_string(),
        ],
        warnings: Vec::new(),
        operations,
    })
}

/// Build a revision-bound plan to change the project canvas dimensions.
/// Existing clip placement and crop values are intentionally left untouched.
pub fn build_canvas_resize_plan(
    project: &Project,
    goal: &str,
    width: u32,
    height: u32,
) -> Result<EditPlan, String> {
    if goal.trim().is_empty() || goal.len() > 4000 {
        return Err("Goal must contain 1 to 4000 bytes".to_string());
    }
    if width == 0 || height == 0 || width > 8192 || height > 8192 {
        return Err("Canvas dimensions must be between 1 and 8192 pixels".to_string());
    }
    if width % 2 != 0 || height % 2 != 0 {
        return Err("Canvas dimensions must be even for H.264 export".to_string());
    }
    if (width, height) == (project.settings.width, project.settings.height) {
        return Err("Requested canvas dimensions already match the project".to_string());
    }

    Ok(EditPlan {
        schema_version: EDIT_PLAN_SCHEMA_VERSION,
        goal: goal.trim().to_string(),
        project_id: project.project_id.clone(),
        base_revision: project.revision,
        referenced_media_ids: Vec::new(),
        assumptions: vec![
            format!(
                "Canvas changes from {}x{} to {width}x{height}.",
                project.settings.width, project.settings.height
            ),
            "Existing clip placement, crop, text and overlay geometry are not automatically reframed; review the composition after applying.".to_string(),
            "The plan is bound to the current project revision and must be regenerated after any project change.".to_string(),
        ],
        warnings: vec![
            "Changing canvas dimensions does not reposition or recrop existing clips and overlays.".to_string(),
        ],
        operations: vec![OperationEnvelope {
            operation_id: format!("edit_canvas_r{}_{width}x{height}", project.revision),
            operation: CommandOperation::ProjectSettingsSet { width, height },
        }],
    })
}

/// Ask an explicitly selected local LM Studio provider to order existing clips on a track.
/// The model returns only an exact permutation of the supplied item IDs.
pub async fn build_local_ai_clip_reorder_plan(
    project: &Project,
    goal: &str,
    track_id: &str,
    selected_item_ids: &[String],
    model: &str,
) -> Result<EditPlanDraftReceipt, String> {
    build_local_ai_clip_reorder_plan_at(
        project,
        goal,
        track_id,
        selected_item_ids,
        model,
        LOCAL_LM_STUDIO_CHAT_ENDPOINT,
        LOCAL_LM_STUDIO_TIMEOUT,
    )
    .await
}

async fn build_local_ai_clip_reorder_plan_at(
    project: &Project,
    goal: &str,
    track_id: &str,
    selected_item_ids: &[String],
    model: &str,
    endpoint: &str,
    timeout: std::time::Duration,
) -> Result<EditPlanDraftReceipt, String> {
    if goal.trim().is_empty() || goal.len() > 4000 {
        return Err("Goal must contain 1 to 4000 bytes".to_string());
    }
    if model.trim().is_empty() || model.len() > 160 {
        return Err("Enter a valid local model ID".to_string());
    }
    let clips = collect_reorderable_clips(project, track_id, selected_item_ids)?;
    let fps = project.settings.frame_rate.numerator as f64
        / project.settings.frame_rate.denominator as f64;
    let selected = clips
        .iter()
        .map(|clip| {
            if clip.name.len() > 512 {
                return Err(format!(
                    "Selected clip name is too long to send to the local planner: {}",
                    clip.item_id
                ));
            }
            Ok(serde_json::json!({
                "itemId": clip.item_id,
                "name": clip.name,
                "durationSeconds": clip.duration_frames as f64 / fps,
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let allowed_ids = selected_item_ids
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let schema = serde_json::json!({
        "name": "clip_order",
        "strict": true,
        "schema": {
            "type": "object",
            "properties": {
                "orderedItemIds": {
                    "type": "array",
                    "items": { "type": "string", "enum": allowed_ids }
                },
                "rationale": { "type": "string" }
            },
            "required": ["orderedItemIds", "rationale"],
            "additionalProperties": false
        }
    });
    let request = serde_json::json!({
        "model": model.trim(),
        "temperature": 0,
        "max_tokens": 512,
        "response_format": { "type": "json_schema", "json_schema": schema },
        "messages": [
            {
                "role": "system",
                "content": "Choose an order for the supplied existing video clips based on the user's editing request and the provided clip names and durations. Return every supplied itemId exactly once in orderedItemIds. Use only supplied IDs. Do not claim to inspect video or audio content or infer events from filenames. Keep rationale concise."
            },
            {
                "role": "user",
                "content": serde_json::json!({
                    "goal": goal,
                    "trackId": track_id,
                    "selectedClips": selected
                }).to_string()
            }
        ]
    });
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|error| format!("Unable to initialize local planner request: {error}"))?;
    let response = client
        .post(endpoint)
        .json(&request)
        .send()
        .await
        .map_err(|error| {
            if error.is_timeout() {
                format!(
                    "Local planner timed out after {:.1} seconds",
                    timeout.as_secs_f64()
                )
            } else {
                format!("Local LM Studio is unavailable at 127.0.0.1:1234: {error}")
            }
        })?
        .error_for_status()
        .map_err(|error| format!("Local planner request failed: {error}"))?;
    let completion: LocalChatCompletion = response
        .json()
        .await
        .map_err(|error| format!("Local planner returned an invalid response: {error}"))?;
    let content = completion
        .choices
        .first()
        .ok_or_else(|| "Local planner returned no choices".to_string())?
        .message
        .content
        .as_str();
    let choice = parse_local_clip_order(content, selected_item_ids)?;
    let mut plan = build_clip_reorder_plan(
        project,
        goal,
        track_id,
        selected_item_ids,
        &choice.ordered_item_ids,
    )?;
    plan.assumptions.push(format!(
        "Local model chose the clip order: {}",
        choice.rationale.trim()
    ));
    let validation = validate_edit_plan(project, &plan);
    if !validation.valid {
        return Err(format!(
            "Local planner produced an invalid edit plan: {}",
            validation.issues.join("; ")
        ));
    }
    Ok(EditPlanDraftReceipt {
        provider: format!("{LOCAL_LM_STUDIO_PROVIDER} ({})", model.trim()),
        plan,
        validation,
    })
}

fn collect_reorderable_clips(
    project: &Project,
    track_id: &str,
    selected_item_ids: &[String],
) -> Result<Vec<ReorderableClip>, String> {
    if selected_item_ids.len() < 2 || selected_item_ids.len() > 100 {
        return Err("Select between 2 and 100 clips from one video track".to_string());
    }
    let selected = selected_item_ids
        .iter()
        .collect::<std::collections::HashSet<_>>();
    if selected.len() != selected_item_ids.len() {
        return Err("Selected clip IDs must be unique".to_string());
    }
    let track = project
        .tracks
        .iter()
        .find(|track| track.track_id == track_id)
        .ok_or_else(|| format!("Video track does not exist: {track_id}"))?;
    if track.kind != TrackKind::Video || track.locked {
        return Err("Selected track must be an unlocked video track".to_string());
    }
    if !track.transitions.is_empty()
        || !track.audio_transitions.is_empty()
        || track.items.iter().any(|item| item.transition.is_some())
    {
        return Err("Tracks with transitions cannot be reordered by this planner".to_string());
    }
    if track.items.len() != selected_item_ids.len()
        || track
            .items
            .iter()
            .any(|item| !selected.contains(&item.item_id))
    {
        return Err(
            "Select every clip on the chosen track so unrelated clips are not moved".to_string(),
        );
    }
    let mut clips = track
        .items
        .iter()
        .map(|item| {
            if item.kind != ItemKind::Video {
                return Err("The chosen track must contain video clips only".to_string());
            }
            let asset_id = item
                .asset_id
                .as_deref()
                .ok_or_else(|| format!("Video clip has no source asset: {}", item.item_id))?;
            let asset = project
                .assets
                .iter()
                .find(|asset| asset.asset_id == asset_id)
                .ok_or_else(|| format!("Video source asset is missing: {asset_id}"))?;
            if asset.media_kind != MediaKind::Video || !std::path::Path::new(&asset.path).is_file()
            {
                return Err(format!(
                    "Video source asset is offline or invalid: {asset_id}"
                ));
            }
            let start = item
                .timeline_range
                .start
                .rescaled_to(project.settings.frame_rate)
                .value;
            let duration = item
                .timeline_range
                .duration
                .rescaled_to(project.settings.frame_rate)
                .value;
            if start < 0 || duration <= 0 {
                return Err(format!(
                    "Clip has an invalid timeline range: {}",
                    item.item_id
                ));
            }
            Ok(ReorderableClip {
                item_id: item.item_id.clone(),
                asset_id: asset_id.to_string(),
                name: item.name.clone(),
                timeline_start_frames: start,
                duration_frames: duration,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    clips.sort_by_key(|clip| clip.timeline_start_frames);
    let mut expected_start = clips
        .first()
        .map(|clip| clip.timeline_start_frames)
        .ok_or_else(|| "Selected video track has no clips".to_string())?;
    for clip in &clips {
        if clip.timeline_start_frames != expected_start {
            return Err(
                "Selected clips must form one contiguous, non-overlapping group".to_string(),
            );
        }
        expected_start = expected_start
            .checked_add(clip.duration_frames)
            .ok_or_else(|| "Track duration exceeds the supported frame range".to_string())?;
    }
    Ok(clips)
}

fn validate_local_clip_order(ordered: &[String], selected: &[String]) -> Result<(), String> {
    let selected_set = selected.iter().collect::<std::collections::HashSet<_>>();
    let ordered_set = ordered.iter().collect::<std::collections::HashSet<_>>();
    if ordered.len() != selected.len()
        || ordered_set.len() != ordered.len()
        || ordered_set != selected_set
    {
        return Err("Local planner must return every selected clip ID exactly once".to_string());
    }
    Ok(())
}

fn parse_local_clip_order(content: &str, selected: &[String]) -> Result<LocalClipOrdering, String> {
    let choice: LocalClipOrdering = serde_json::from_str(content)
        .map_err(|error| format!("Local planner returned invalid structured output: {error}"))?;
    validate_local_clip_order(&choice.ordered_item_ids, selected)?;
    if choice.rationale.trim().is_empty() || choice.rationale.len() > 2000 {
        return Err("Local planner must provide a concise non-empty rationale".to_string());
    }
    Ok(choice)
}

async fn build_local_ai_video_draft_at(
    project: &Project,
    goal: &str,
    media_ids: &[String],
    target_duration_frames: u64,
    audio_asset_id: Option<&str>,
    captions: &[CaptionCue],
    model: &str,
    endpoint: &str,
    timeout: std::time::Duration,
) -> Result<EditPlanDraftReceipt, String> {
    if goal.trim().is_empty() {
        return Err("Goal must contain at least one non-whitespace character".to_string());
    }
    if goal.len() > 4000 {
        return Err("Goal must be 4000 bytes or shorter".to_string());
    }
    if model.trim().is_empty() || model.len() > 160 {
        return Err("Enter a valid local model ID".to_string());
    }
    if media_ids.is_empty() || media_ids.len() > 100 {
        return Err("Select between 1 and 100 media assets in the intended order".to_string());
    }
    // Check all deterministic constraints before spending a local inference request.
    build_deterministic_video_draft_with_supporting_media(
        project,
        goal,
        media_ids,
        target_duration_frames,
        audio_asset_id,
        captions,
    )?;
    let fps = project.settings.frame_rate.numerator as f64
        / project.settings.frame_rate.denominator as f64;
    let media = media_ids
        .iter()
        .map(|id| {
            let asset = project
                .assets
                .iter()
                .find(|asset| asset.asset_id == *id)
                .ok_or_else(|| format!("Selected media does not exist in project: {id}"))?;
            if asset.media_kind != MediaKind::Video {
                return Err(format!("Selected media is not video: {id}"));
            }
            if !std::path::Path::new(&asset.path).is_file() {
                return Err(format!("Selected media is offline or missing: {id}"));
            }
            let frames = asset
                .duration
                .as_ref()
                .ok_or_else(|| format!("Selected media has no probed duration: {id}"))?
                .rescaled_to(project.settings.frame_rate)
                .value;
            if asset.name.len() > 512 {
                return Err(format!(
                    "Selected media name is too long to send to the local planner: {id}"
                ));
            }
            Ok(serde_json::json!({
                "mediaId": asset.asset_id,
                "name": asset.name,
                "durationSeconds": (frames.max(0) as f64 / fps),
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let ids = media_ids.iter().map(String::as_str).collect::<Vec<_>>();
    let schema = serde_json::json!({
        "name": "media_order",
        "strict": true,
        "schema": {
            "type": "object",
            "properties": {
                "orderedMediaIds": {
                    "type": "array",
                    "items": { "type": "string", "enum": ids }
                },
                "rationale": { "type": "string" }
            },
            "required": ["orderedMediaIds", "rationale"],
            "additionalProperties": false
        }
    });
    let request = serde_json::json!({
        "model": model.trim(),
        "temperature": 0,
        "max_tokens": 512,
        "response_format": { "type": "json_schema", "json_schema": schema },
        "messages": [
            {
                "role": "system",
                "content": "Choose a creative sequence for the supplied video clips. Return every supplied mediaId exactly once in orderedMediaIds. Use only the supplied IDs. Consider the user's goal and clip names/durations. Do not claim to inspect video content or infer events from filenames. Keep rationale concise."
            },
            {
                "role": "user",
                "content": serde_json::json!({
                    "goal": goal,
                    "targetDurationSeconds": target_duration_frames as f64 / fps,
                    "selectedVideo": media
                }).to_string()
            }
        ]
    });
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|error| format!("Unable to initialize local planner request: {error}"))?;
    let response = client
        .post(endpoint)
        .json(&request)
        .send()
        .await
        .map_err(|error| {
            if error.is_timeout() {
                format!(
                    "Local planner timed out after {:.1} seconds",
                    timeout.as_secs_f64()
                )
            } else {
                format!("Local LM Studio is unavailable at 127.0.0.1:1234: {error}")
            }
        })?
        .error_for_status()
        .map_err(|error| format!("Local planner request failed: {error}"))?;
    let completion: LocalChatCompletion = response
        .json()
        .await
        .map_err(|error| format!("Local planner returned an invalid response: {error}"))?;
    let content = completion
        .choices
        .first()
        .ok_or_else(|| "Local planner returned no choices".to_string())?
        .message
        .content
        .as_str();
    let choice = parse_local_media_order(content, media_ids)?;
    let mut plan = build_deterministic_video_draft_with_supporting_media(
        project,
        goal,
        &choice.ordered_media_ids,
        target_duration_frames,
        audio_asset_id,
        captions,
    )?;
    apply_local_planner_receipt_metadata(&mut plan, &choice.rationale);
    let validation = validate_edit_plan(project, &plan);
    if !validation.valid {
        return Err(format!(
            "Local planner produced an invalid edit plan: {}",
            validation.issues.join("; ")
        ));
    }
    Ok(EditPlanDraftReceipt {
        provider: format!("{LOCAL_LM_STUDIO_PROVIDER} ({})", model.trim()),
        plan,
        validation,
    })
}

fn apply_local_planner_receipt_metadata(plan: &mut EditPlan, rationale: &str) {
    plan.assumptions
        .retain(|item| !item.starts_with("Deterministic mock provider"));
    plan.warnings
        .retain(|warning| !warning.starts_with("The mock provider"));
    plan.assumptions.push(format!(
        "Local model selected the clip order: {}",
        rationale.trim()
    ));
    plan.warnings.push("The local model received only the goal and selected clip IDs, names, and probed durations; it did not inspect media bytes or scene content.".to_string());
}

pub(crate) async fn wait_for_local_planner_request<T, R, C>(
    request: R,
    cancelled: C,
) -> Result<T, String>
where
    R: std::future::Future<Output = Result<T, String>>,
    C: std::future::Future,
{
    tokio::select! {
        biased;
        _ = cancelled => Err("Local planner request was cancelled".to_string()),
        result = request => result,
    }
}

fn parse_local_media_order(
    content: &str,
    requested: &[String],
) -> Result<LocalMediaOrdering, String> {
    let choice: LocalMediaOrdering = serde_json::from_str(content)
        .map_err(|error| format!("Local planner returned invalid structured output: {error}"))?;
    let requested_set = requested.iter().collect::<std::collections::HashSet<_>>();
    let returned_set = choice
        .ordered_media_ids
        .iter()
        .collect::<std::collections::HashSet<_>>();
    if choice.ordered_media_ids.len() != requested.len()
        || returned_set.len() != choice.ordered_media_ids.len()
        || returned_set != requested_set
        || choice.rationale.trim().is_empty()
        || choice.rationale.len() > 2000
    {
        return Err(
            "Local planner must return each selected media ID exactly once and a concise rationale"
                .to_string(),
        );
    }
    Ok(choice)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EditPlan {
    pub schema_version: u32,
    pub goal: String,
    pub project_id: String,
    pub base_revision: u64,
    #[serde(default)]
    pub referenced_media_ids: Vec<String>,
    #[serde(default)]
    pub assumptions: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    pub operations: Vec<OperationEnvelope>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EditPlanValidation {
    pub valid: bool,
    pub issues: Vec<String>,
    pub diff: Option<DiffReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EditPlanDraftReceipt {
    pub provider: String,
    pub plan: EditPlan,
    pub validation: EditPlanValidation,
}

/// Build a review-only edit plan from explicit scene selections in a local visual report.
/// Every source is re-fingerprinted against the current project before source-time ranges are used.
pub fn build_visual_scene_edit_plan(
    project: &Project,
    goal: &str,
    report: &VisualSceneAnalysisReport,
    selected_scene_ids: &[String],
) -> Result<EditPlanDraftReceipt, String> {
    build_visual_scene_edit_plan_with_duration(
        project,
        goal,
        report,
        selected_scene_ids,
        None,
        false,
    )
}

fn schedule_exact_scene_frames(capacities: &[u64], target: u64) -> Result<Vec<u64>, String> {
    if capacities.is_empty() || capacities.iter().any(|frames| *frames == 0) {
        return Err(
            "Exact duration requires at least one positive source range per selected scene"
                .to_string(),
        );
    }
    let scene_count = capacities.len() as u64;
    let total_capacity = capacities.iter().try_fold(0_u64, |sum, value| {
        sum.checked_add(*value)
            .ok_or_else(|| "Selected scene duration exceeds the supported range".to_string())
    })?;
    if target < scene_count {
        return Err(format!(
            "Exact target {target} frames cannot retain one frame for each of {scene_count} selected scenes"
        ));
    }
    if target > total_capacity {
        return Err(format!(
            "Exact target {target} frames exceeds the selected source capacity of {total_capacity} frames; add scenes or use advisory duration"
        ));
    }

    let extra_target = target - scene_count;
    let extra_capacities = capacities
        .iter()
        .map(|frames| frames - 1)
        .collect::<Vec<_>>();
    let mut low = 0_u64;
    let mut high = extra_capacities.iter().copied().max().unwrap_or(0);
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        let allocated = extra_capacities
            .iter()
            .map(|capacity| (*capacity).min(middle))
            .sum::<u64>();
        if allocated <= extra_target {
            low = middle;
        } else {
            high = middle - 1;
        }
    }

    let mut result = extra_capacities
        .iter()
        .map(|capacity| 1 + (*capacity).min(low))
        .collect::<Vec<_>>();
    let mut remaining = target - result.iter().sum::<u64>();
    for (frames, capacity) in result.iter_mut().zip(capacities) {
        if remaining == 0 {
            break;
        }
        if *frames < *capacity {
            *frames += 1;
            remaining -= 1;
        }
    }
    if remaining != 0 || result.iter().sum::<u64>() != target {
        return Err("Exact scene duration allocation failed its frame-count invariant".to_string());
    }
    Ok(result)
}

/// Build a scene draft with an optional target. Exact mode trims selected scenes fairly
/// from their tails; advisory mode preserves the reviewed source ranges and reports the delta.
pub fn build_visual_scene_edit_plan_with_duration(
    project: &Project,
    goal: &str,
    report: &VisualSceneAnalysisReport,
    selected_scene_ids: &[String],
    target_duration_frames: Option<u64>,
    exact_duration: bool,
) -> Result<EditPlanDraftReceipt, String> {
    build_visual_scene_edit_plan_with_constraints(
        project,
        goal,
        report,
        selected_scene_ids,
        target_duration_frames,
        exact_duration,
        SceneSelectionConstraints::default(),
        None,
        None,
        None,
    )
}

pub fn build_visual_scene_edit_plan_with_constraints(
    project: &Project,
    goal: &str,
    report: &VisualSceneAnalysisReport,
    selected_scene_ids: &[String],
    target_duration_frames: Option<u64>,
    exact_duration: bool,
    constraints: SceneSelectionConstraints,
    target_canvas: Option<SceneCanvasPreset>,
    opening_title: Option<&str>,
    title_template: Option<SceneTitleTemplate>,
) -> Result<EditPlanDraftReceipt, String> {
    if goal.trim().is_empty() || goal.len() > 4_000 {
        return Err("Goal must contain 1 to 4000 bytes".to_string());
    }
    if report.schema_version != VISUAL_SCENE_SCHEMA_VERSION
        || report.algorithm_version != VISUAL_SCENE_ALGORITHM_VERSION
        || report.query.trim().is_empty()
        || report.model.trim().is_empty()
        || report.scenes.is_empty()
        || report.scenes.len() > crate::modules::media::visual_scene::MAX_SCENES_PER_REQUEST
    {
        return Err("Visual scene report is empty or uses an unsupported schema".to_string());
    }
    if selected_scene_ids.is_empty()
        || selected_scene_ids.len() > crate::modules::media::visual_scene::MAX_SCENES_PER_REQUEST
    {
        return Err("Select between 1 and 32 scenes for the draft".to_string());
    }
    let opening_title = opening_title
        .map(str::trim)
        .filter(|title| !title.is_empty());
    match (opening_title, title_template) {
        (Some(title), Some(_)) if title.len() <= 240 && !title.chars().any(char::is_control) => {}
        (Some(_), Some(_)) => {
            return Err(
                "Opening title must be at most 240 UTF-8 bytes and contain no control characters"
                    .into(),
            )
        }
        (None, None) => {}
        _ => return Err("Opening title text and template must be supplied together".into()),
    }
    let available_scene_ids = report
        .scenes
        .iter()
        .map(|scene| scene.scene_id.clone())
        .collect::<Vec<_>>();
    constraints.validate(&available_scene_ids)?;
    constraints.validate_selection(selected_scene_ids)?;

    let mut source_ids = std::collections::HashSet::new();
    let mut sources = Vec::new();
    let mut scene_ids = std::collections::HashSet::new();
    for scene in &report.scenes {
        if scene.scene_id.trim().is_empty()
            || !scene_ids.insert(scene.scene_id.as_str())
            || scene.scene_id.len() > 64
            || scene.asset_id.trim().is_empty()
            || scene.asset_id.len() > 128
            || scene.caption.trim().is_empty()
            || scene.caption.len() > 1_000
            || scene.rationale.len() > 1_000
            || scene.tags.len() > 12
            || scene
                .tags
                .iter()
                .any(|tag| tag.trim().is_empty() || tag.len() > 80)
            || scene.source_start_micros >= scene.source_end_micros
            || scene.sampled_at_micros < scene.source_start_micros
            || scene.sampled_at_micros >= scene.source_end_micros
            || !scene.relevance_score.is_finite()
            || !(0.0..=1.0).contains(&scene.relevance_score)
        {
            return Err(
                "Visual scene report contains an invalid or duplicate scene range".to_string(),
            );
        }
        if source_ids.insert(scene.asset_id.as_str()) {
            let asset = project
                .assets
                .iter()
                .find(|asset| asset.asset_id == scene.asset_id)
                .ok_or_else(|| {
                    format!(
                        "Visual scene source is not in the current project: {}",
                        scene.asset_id
                    )
                })?;
            if asset.media_kind != MediaKind::Video {
                return Err(format!(
                    "Visual scene source is not a video: {}",
                    scene.asset_id
                ));
            }
            if !std::path::Path::new(&asset.path).is_file() {
                return Err(format!(
                    "Visual scene source is offline or missing: {}",
                    scene.asset_id
                ));
            }
            sources.push(VisualSceneSource {
                asset_id: asset.asset_id.clone(),
                asset_name: asset.name.clone(),
                path: std::path::PathBuf::from(&asset.path),
            });
        }
    }
    if sources.is_empty()
        || sources.len() > crate::modules::media::visual_scene::MAX_SELECTED_ASSETS
    {
        return Err("Visual scene report must reference 1 to 8 video assets".to_string());
    }
    let available_asset_ids = source_ids
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>();
    constraints.validate_allowed_assets(&available_asset_ids)?;
    if report.scenes.iter().any(|scene| {
        constraints.must_keep_scene_ids.contains(&scene.scene_id)
            && !constraints.allows_asset(&scene.asset_id)
    }) {
        return Err("A must-keep scene belongs to a disallowed source asset".to_string());
    }
    if selected_scene_ids.iter().any(|id| {
        report
            .scenes
            .iter()
            .find(|scene| &scene.scene_id == id)
            .is_some_and(|scene| !constraints.allows_asset(&scene.asset_id))
    }) {
        return Err("Selected scene belongs to a disallowed source asset".to_string());
    }
    if aggregate_source_fingerprint(&sources)? != report.source_fingerprint {
        return Err(
            "Visual scene report is stale because a source file changed or was relinked"
                .to_string(),
        );
    }

    let selected = selected_scene_ids
        .iter()
        .map(|scene_id| {
            if scene_id.trim().is_empty() {
                return Err("Selected scene IDs must not be empty".to_string());
            }
            report
                .scenes
                .iter()
                .find(|scene| scene.scene_id == *scene_id)
                .ok_or_else(|| format!("Selected scene does not exist in the report: {scene_id}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if selected_scene_ids
        .iter()
        .collect::<std::collections::HashSet<_>>()
        .len()
        != selected_scene_ids.len()
    {
        return Err("Selected scene IDs must be unique".to_string());
    }

    let frame_rate = project.settings.frame_rate;
    let microsecond_rate = crate::modules::domain::time::FrameRate::new(1_000_000, 1);
    let mut total_frames = 0_u64;
    let mut clip_frames = Vec::with_capacity(selected.len());
    for scene in &selected {
        let asset = project
            .assets
            .iter()
            .find(|asset| asset.asset_id == scene.asset_id)
            .expect("selected scene assets were validated above");
        let source_start = i64::try_from(scene.source_start_micros)
            .map_err(|_| "Scene source start exceeds supported time range".to_string())?;
        let source_end = i64::try_from(scene.source_end_micros)
            .map_err(|_| "Scene source end exceeds supported time range".to_string())?;
        let start_frame = RationalTime::new(source_start, microsecond_rate)
            .rescaled_to(frame_rate)
            .value;
        let end_frame = RationalTime::new(source_end, microsecond_rate)
            .rescaled_to(frame_rate)
            .value;
        if start_frame < 0 || end_frame <= start_frame {
            return Err(format!(
                "Scene has no representable frames at the project frame rate: {}",
                scene.scene_id
            ));
        }
        let asset_frames = asset
            .duration
            .as_ref()
            .ok_or_else(|| format!("Scene source has no probed duration: {}", scene.asset_id))?
            .rescaled_to(frame_rate)
            .value;
        if end_frame > asset_frames {
            return Err(format!(
                "Scene source range exceeds current media duration: {}",
                scene.scene_id
            ));
        }
        let duration_frames = (end_frame - start_frame) as u64;
        total_frames = total_frames
            .checked_add(duration_frames)
            .ok_or_else(|| "Selected scenes exceed the supported draft duration".to_string())?;
        clip_frames.push((scene, start_frame as u64, duration_frames));
    }
    if total_frames == 0 || total_frames > MAX_DRAFT_DURATION_FRAMES {
        return Err(format!(
            "Selected scenes must total 1 to {MAX_DRAFT_DURATION_FRAMES} frames"
        ));
    }
    if let Some(target) = target_duration_frames {
        if target == 0 || target > MAX_DRAFT_DURATION_FRAMES {
            return Err(format!(
                "targetDurationFrames must be in 1..={MAX_DRAFT_DURATION_FRAMES}"
            ));
        }
        if exact_duration {
            let capacities = clip_frames
                .iter()
                .map(|(_, _, frames)| *frames)
                .collect::<Vec<_>>();
            let scheduled = schedule_exact_scene_frames(&capacities, target)?;
            for (clip, duration) in clip_frames.iter_mut().zip(scheduled) {
                clip.2 = duration;
            }
            total_frames = target;
        }
    } else if exact_duration {
        return Err("Exact duration mode requires a target duration".to_string());
    }

    let id_prefix = format!(
        "scene_draft_r{}_{}",
        project.revision,
        uuid::Uuid::new_v4().simple()
    );
    let track_id = format!("{id_prefix}_video");
    let mut operations = vec![OperationEnvelope {
        operation_id: format!("{id_prefix}_track"),
        operation: CommandOperation::TrackCreate {
            track_id: Some(track_id.clone()),
            name: "Scene Draft".to_string(),
            kind: TrackKind::Video,
        },
    }];
    if let Some(preset) = target_canvas {
        let (width, height) = preset.dimensions();
        if (width, height) != (project.settings.width, project.settings.height) {
            operations.insert(
                0,
                OperationEnvelope {
                    operation_id: format!("{id_prefix}_canvas"),
                    operation: CommandOperation::ProjectSettingsSet { width, height },
                },
            );
        }
    }
    let mut timeline_start = 0_u64;
    let mut referenced_media_ids = std::collections::BTreeSet::new();
    let mut assumptions = vec![
        format!("Selected scenes came from the '{}' local visual analysis report.", report.query.trim()),
        "Selected scene order is the caller's explicit order; clips use the complete detected source-time interval unless their final frame rounds to the project frame rate.".to_string(),
        "The report was checked against current source file fingerprints and this plan is bound to the current project revision.".to_string(),
    ];
    for (index, (scene, source_start, duration)) in clip_frames.into_iter().enumerate() {
        let asset = project
            .assets
            .iter()
            .find(|asset| asset.asset_id == scene.asset_id)
            .expect("selected scene assets were validated above");
        referenced_media_ids.insert(asset.asset_id.clone());
        let rationale = scene.rationale.trim();
        assumptions.push(format!(
            "{} at {:.2}s: {} (model ranking {:.2}; {}).",
            scene.scene_id,
            scene.sampled_at_micros as f64 / 1_000_000.0,
            scene.caption.trim(),
            scene.relevance_score,
            if rationale.is_empty() {
                "no rationale supplied"
            } else {
                rationale
            }
        ));
        operations.push(OperationEnvelope {
            operation_id: format!("{id_prefix}_clip_{index}"),
            operation: CommandOperation::ClipCreate {
                track_id: track_id.clone(),
                item_id: Some(format!("{id_prefix}_item_{index}")),
                name: asset.name.clone(),
                kind: ItemKind::Video,
                asset_id: Some(asset.asset_id.clone()),
                timeline_start: RationalTime::new(timeline_start as i64, frame_rate),
                duration: RationalTime::new(duration as i64, frame_rate),
                source_start: Some(RationalTime::new(source_start as i64, frame_rate)),
            },
        });
        timeline_start = timeline_start
            .checked_add(duration)
            .ok_or_else(|| "Draft timeline exceeds the supported frame range".to_string())?;
    }
    if let (Some(title), Some(template)) = (opening_title, title_template) {
        let title_track_id = format!("{id_prefix}_title_track");
        let title_item_id = format!("{id_prefix}_title");
        let title_duration =
            total_frames.min((frame_rate.numerator as u64 * 3) / frame_rate.denominator as u64);
        operations.extend([
            OperationEnvelope {
                operation_id: format!("{id_prefix}_title_track_create"),
                operation: CommandOperation::TrackCreate {
                    track_id: Some(title_track_id.clone()),
                    name: "Title Overlay".to_string(),
                    kind: TrackKind::Video,
                },
            },
            OperationEnvelope {
                operation_id: format!("{id_prefix}_title_clip_create"),
                operation: CommandOperation::ClipCreate {
                    track_id: title_track_id,
                    item_id: Some(title_item_id.clone()),
                    name: "Opening Title".to_string(),
                    kind: ItemKind::Text,
                    asset_id: None,
                    timeline_start: RationalTime::new(0, frame_rate),
                    duration: RationalTime::new(title_duration as i64, frame_rate),
                    source_start: Some(RationalTime::new(0, frame_rate)),
                },
            },
            OperationEnvelope {
                operation_id: format!("{id_prefix}_title_text"),
                operation: CommandOperation::ClipTextSet {
                    item_id: title_item_id.clone(),
                    text: title.to_string(),
                },
            },
            OperationEnvelope {
                operation_id: format!("{id_prefix}_title_template"),
                operation: CommandOperation::TextTemplateApply {
                    item_id: title_item_id,
                    template_id: template.template_id().to_string(),
                    template_version: 1,
                },
            },
        ]);
        assumptions.push(format!(
            "Opening title uses the selected built-in '{}' text template for {:.2} seconds.",
            template.template_id(),
            title_duration as f64 * frame_rate.denominator as f64 / frame_rate.numerator as f64
        ));
    }
    let actual_seconds =
        total_frames as f64 * frame_rate.denominator as f64 / frame_rate.numerator as f64;
    let duration_warnings = match (target_duration_frames, exact_duration) {
        (Some(target), true) => vec![format!(
            "Exact duration scheduler allocated exactly {target} frames ({actual_seconds:.3} seconds) without repeating scenes or changing speed."
        )],
        (Some(target), false) => {
            let target_seconds = target as f64 * frame_rate.denominator as f64
                / frame_rate.numerator as f64;
            vec![format!(
                "Advisory duration target is {target_seconds:.3} seconds; this draft is {actual_seconds:.3} seconds ({:+.3} seconds). Reviewed scene ranges were preserved.",
                actual_seconds - target_seconds
            )]
        }
        _ => Vec::new(),
    };
    let mut warnings = vec![
        "Scene descriptions and relevance rankings are model-generated and may be inaccurate; review each source interval before applying.".to_string(),
        format!("This draft contains {} selected scenes and lasts {:.2} seconds at the project frame rate.", selected.len(), actual_seconds),
    ];
    warnings.extend(duration_warnings);
    let plan = EditPlan {
        schema_version: EDIT_PLAN_SCHEMA_VERSION,
        goal: goal.trim().to_string(),
        project_id: project.project_id.clone(),
        base_revision: project.revision,
        referenced_media_ids: referenced_media_ids.into_iter().collect(),
        assumptions: std::mem::take(&mut assumptions),
        warnings,
        operations,
    };
    let validation = validate_edit_plan(project, &plan);
    if !validation.valid {
        return Err(format!(
            "Scene-guided draft is invalid: {}",
            validation.issues.join("; ")
        ));
    }
    Ok(EditPlanDraftReceipt {
        provider: "visual-scene-report-v1".to_string(),
        plan,
        validation,
    })
}

/// Make a repeatable rough-cut plan in the caller's specified media order.
/// This mock provider deliberately does not interpret natural-language intent.
pub fn build_deterministic_video_draft(
    project: &Project,
    goal: &str,
    media_ids: &[String],
    target_duration_frames: u64,
) -> Result<EditPlan, String> {
    build_deterministic_video_draft_with_supporting_media(
        project,
        goal,
        media_ids,
        target_duration_frames,
        None,
        &[],
    )
}

pub fn parse_draft_caption_file(
    path: &std::path::Path,
    frame_rate: crate::modules::domain::time::FrameRate,
) -> Result<Vec<CaptionCue>, String> {
    let format = CaptionFormat::from_path(path)?;
    let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
    if metadata.len() > crate::modules::media::captions::MAX_CAPTION_FILE_BYTES as u64 {
        return Err("Caption file exceeds the 10 MB draft limit".to_string());
    }
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    parse_caption_bytes(&bytes, format, frame_rate)
}

/// Extend the deterministic video rough cut with explicitly selected local audio and captions.
/// This mock still does not infer creative choices from the goal or analyze scene quality.
pub fn build_deterministic_video_draft_with_supporting_media(
    project: &Project,
    goal: &str,
    media_ids: &[String],
    target_duration_frames: u64,
    audio_asset_id: Option<&str>,
    captions: &[CaptionCue],
) -> Result<EditPlan, String> {
    if goal.trim().is_empty() {
        return Err("Goal must contain at least one non-whitespace character".to_string());
    }
    if media_ids.is_empty() || media_ids.len() > 100 {
        return Err("Select between 1 and 100 media assets in the intended order".to_string());
    }
    if target_duration_frames == 0 || target_duration_frames > MAX_DRAFT_DURATION_FRAMES {
        return Err(format!(
            "targetDurationFrames must be in 1..={MAX_DRAFT_DURATION_FRAMES} at the project frame rate"
        ));
    }
    if target_duration_frames < media_ids.len() as u64 {
        return Err(
            "Target duration must provide at least one frame per selected asset".to_string(),
        );
    }

    let mut seen = std::collections::HashSet::new();
    let assets = media_ids
        .iter()
        .map(|media_id| {
            if !seen.insert(media_id) {
                return Err(format!("Duplicate media ID in requested order: {media_id}"));
            }
            let asset = project
                .assets
                .iter()
                .find(|asset| asset.asset_id == *media_id)
                .ok_or_else(|| format!("Media ID does not exist in project: {media_id}"))?;
            if asset.media_kind != MediaKind::Video {
                return Err(format!(
                    "Deterministic video draft requires video media: {media_id}"
                ));
            }
            if !std::path::Path::new(&asset.path).is_file() {
                return Err(format!("Selected media is offline or missing: {media_id}"));
            }
            let duration = asset
                .duration
                .as_ref()
                .ok_or_else(|| format!("Selected media has no probed duration: {media_id}"))?
                .rescaled_to(project.settings.frame_rate)
                .value;
            if duration <= 0 {
                return Err(format!(
                    "Selected media has no usable video duration: {media_id}"
                ));
            }
            Ok((asset, duration as u64))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let total_source_frames = assets.iter().map(|(_, frames)| *frames).sum::<u64>();
    if total_source_frames < target_duration_frames {
        return Err(format!(
            "Selected video assets provide {} fewer source frames than the requested draft duration",
            target_duration_frames - total_source_frames
        ));
    }

    let audio_asset = audio_asset_id
        .map(|media_id| {
            let asset = project
                .assets
                .iter()
                .find(|asset| asset.asset_id == media_id)
                .ok_or_else(|| format!("Audio media ID does not exist in project: {media_id}"))?;
            if asset.media_kind != MediaKind::Audio {
                return Err(format!(
                    "Selected supporting media is not audio: {media_id}"
                ));
            }
            if !std::path::Path::new(&asset.path).is_file() {
                return Err(format!("Selected audio is offline or missing: {media_id}"));
            }
            let duration = asset
                .duration
                .as_ref()
                .ok_or_else(|| format!("Selected audio has no probed duration: {media_id}"))?
                .rescaled_to(project.settings.frame_rate)
                .value;
            if duration <= 0 {
                return Err(format!("Selected audio has no usable duration: {media_id}"));
            }
            Ok((asset, duration as u64))
        })
        .transpose()?;
    if captions.len() > 200 {
        return Err("A deterministic draft can include at most 200 subtitle cues".to_string());
    }

    let mut remaining = target_duration_frames;
    let mut timeline_start = 0_u64;
    let fps = project.settings.frame_rate;
    let operation_prefix = format!("draft_r{}", project.revision);
    let track_id = format!("draft_video_r{}", project.revision);
    let mut operations = vec![
        OperationEnvelope {
            operation_id: format!("{operation_prefix}_canvas_portrait"),
            operation: CommandOperation::ProjectSettingsSet {
                width: 1080,
                height: 1920,
            },
        },
        OperationEnvelope {
            operation_id: format!("{operation_prefix}_video_track"),
            operation: CommandOperation::TrackCreate {
                track_id: Some(track_id.clone()),
                name: "Draft Video".to_string(),
                kind: TrackKind::Video,
            },
        },
    ];

    for (index, (asset, available_frames)) in assets.iter().enumerate() {
        let remaining_assets = (assets.len() - index) as u64;
        let desired = remaining.div_ceil(remaining_assets);
        let future_capacity = assets[index + 1..]
            .iter()
            .map(|(_, frames)| *frames)
            .sum::<u64>();
        let minimum_needed_from_this = remaining.saturating_sub(future_capacity);
        let frames = desired.max(minimum_needed_from_this).min(*available_frames);
        if frames == 0 {
            return Err(format!(
                "Selected media has no frames available: {}",
                asset.asset_id
            ));
        }
        operations.push(OperationEnvelope {
            operation_id: format!("{operation_prefix}_clip_{index}"),
            operation: CommandOperation::ClipCreate {
                track_id: track_id.clone(),
                item_id: Some(format!("draft_r{}_clip_{index}", project.revision)),
                name: asset.name.clone(),
                kind: ItemKind::Video,
                asset_id: Some(asset.asset_id.clone()),
                timeline_start: RationalTime::new(timeline_start as i64, fps),
                duration: RationalTime::new(frames as i64, fps),
                source_start: Some(RationalTime::new(0, fps)),
            },
        });
        timeline_start += frames;
        remaining = remaining.saturating_sub(frames);
    }
    if remaining != 0 {
        return Err(format!(
            "Selected video assets provide {} fewer source frames than the requested draft duration",
            remaining
        ));
    }

    let mut referenced_media_ids = media_ids.to_vec();
    if let Some((asset, source_frames)) = audio_asset {
        referenced_media_ids.push(asset.asset_id.clone());
        let audio_track_id = format!("draft_audio_r{}", project.revision);
        operations.push(OperationEnvelope {
            operation_id: format!("{operation_prefix}_audio_track"),
            operation: CommandOperation::TrackCreate {
                track_id: Some(audio_track_id.clone()),
                name: "Draft Audio".to_string(),
                kind: TrackKind::Audio,
            },
        });
        operations.push(OperationEnvelope {
            operation_id: format!("{operation_prefix}_audio_bed"),
            operation: CommandOperation::ClipCreate {
                track_id: audio_track_id,
                item_id: Some(format!("draft_r{}_audio", project.revision)),
                name: asset.name.clone(),
                kind: ItemKind::Audio,
                asset_id: Some(asset.asset_id.clone()),
                timeline_start: RationalTime::zero(fps),
                duration: RationalTime::new(source_frames.min(target_duration_frames) as i64, fps),
                source_start: Some(RationalTime::zero(fps)),
            },
        });
    }

    if !captions.is_empty() {
        let subtitle_track_id = format!("draft_subtitles_r{}", project.revision);
        operations.push(OperationEnvelope {
            operation_id: format!("{operation_prefix}_subtitle_track"),
            operation: CommandOperation::TrackCreate {
                track_id: Some(subtitle_track_id.clone()),
                name: "Draft Subtitles".to_string(),
                kind: TrackKind::Video,
            },
        });
        let mut subtitle_style = TextStyle::default();
        subtitle_style.anchor = TextAnchor::BottomCenter;
        subtitle_style.font_size = 42.0;
        for (index, cue) in captions.iter().enumerate() {
            let start = cue.start.rescaled_to(fps).value.max(0) as u64;
            let duration = cue.duration.rescaled_to(fps).value.max(0) as u64;
            if start >= target_duration_frames || duration == 0 {
                continue;
            }
            let duration = duration.min(target_duration_frames - start);
            let item_id = format!("draft_r{}_subtitle_{index}", project.revision);
            operations.push(OperationEnvelope {
                operation_id: format!("{operation_prefix}_subtitle_clip_{index}"),
                operation: CommandOperation::ClipCreate {
                    track_id: subtitle_track_id.clone(),
                    item_id: Some(item_id.clone()),
                    name: format!("Subtitle {}", index + 1),
                    kind: ItemKind::Text,
                    asset_id: None,
                    timeline_start: RationalTime::new(start as i64, fps),
                    duration: RationalTime::new(duration as i64, fps),
                    source_start: Some(RationalTime::zero(fps)),
                },
            });
            operations.push(OperationEnvelope {
                operation_id: format!("{operation_prefix}_subtitle_text_{index}"),
                operation: CommandOperation::ClipTextSet {
                    item_id: item_id.clone(),
                    text: cue.text.clone(),
                },
            });
            operations.push(OperationEnvelope {
                operation_id: format!("{operation_prefix}_subtitle_style_{index}"),
                operation: CommandOperation::ClipTextStyleSet {
                    item_id,
                    style: subtitle_style.clone(),
                },
            });
        }
    }

    Ok(EditPlan {
        schema_version: EDIT_PLAN_SCHEMA_VERSION,
        goal: goal.trim().to_string(),
        project_id: project.project_id.clone(),
        base_revision: project.revision,
        referenced_media_ids,
        assumptions: vec![
            "Deterministic mock provider uses the requested media order and starts each source range at frame zero.".to_string(),
            "Each clip receives an even share of the target duration unless a shorter source requires redistribution.".to_string(),
            if audio_asset_id.is_some() {
                "The selected audio is placed once from source frame zero and is trimmed to the draft duration; no beat matching or ducking is inferred.".to_string()
            } else {
                "No audio asset was selected, so the draft contains no music bed.".to_string()
            },
            if captions.is_empty() {
                "No subtitle cues were supplied.".to_string()
            } else {
                "Subtitle cues come from the user-selected local SRT/VTT file and are clipped to the draft duration.".to_string()
            },
            "This draft adds a portrait 1080x1920 canvas and does not infer titles or scene choices from the goal.".to_string(),
        ],
        warnings: vec![
            "The mock provider does not interpret creative intent or analyze scene quality; review and edit the draft before export.".to_string(),
        ],
        operations,
    })
}

pub fn validate_edit_plan(project: &Project, plan: &EditPlan) -> EditPlanValidation {
    let mut issues = Vec::new();
    if plan.schema_version != EDIT_PLAN_SCHEMA_VERSION {
        issues.push(format!(
            "Unsupported edit-plan schema version {}; expected {}",
            plan.schema_version, EDIT_PLAN_SCHEMA_VERSION
        ));
    }
    if plan.goal.trim().is_empty() {
        issues.push("Goal must contain at least one non-whitespace character".to_string());
    }
    if plan.project_id != project.project_id {
        issues.push(format!(
            "Project ID mismatch: plan targets {}, current project is {}",
            plan.project_id, project.project_id
        ));
    }
    if plan.base_revision != project.revision {
        issues.push(format!(
            "Stale edit plan: base revision {} does not match current revision {}",
            plan.base_revision, project.revision
        ));
    }
    if plan.operations.is_empty() {
        issues.push("Edit plan must contain at least one operation".to_string());
    }

    let mut seen_media = std::collections::HashSet::new();
    for media_id in &plan.referenced_media_ids {
        if !seen_media.insert(media_id) {
            issues.push(format!("Duplicate referenced media ID: {media_id}"));
        } else if let Some(asset) = project
            .assets
            .iter()
            .find(|asset| asset.asset_id == *media_id)
        {
            if !std::path::Path::new(&asset.path).is_file() {
                issues.push(format!(
                    "Referenced media is offline or missing: {media_id}"
                ));
            }
        } else {
            issues.push(format!(
                "Referenced media ID does not exist in project: {media_id}"
            ));
        }
    }
    for operation in &plan.operations {
        if let CommandOperation::ClipCreate {
            asset_id: Some(asset_id),
            ..
        } = &operation.operation
        {
            if !seen_media.contains(asset_id) {
                issues.push(format!(
                    "ClipCreate media ID must be listed in referencedMediaIds: {asset_id}"
                ));
            }
            if let Some(asset) = project
                .assets
                .iter()
                .find(|asset| asset.asset_id == *asset_id)
            {
                if !std::path::Path::new(&asset.path).is_file() {
                    issues.push(format!(
                        "ClipCreate media is offline or missing: {asset_id}"
                    ));
                }
            }
        }
        if let CommandOperation::AssetImport { path, .. } = &operation.operation {
            if !std::path::Path::new(path).is_file() {
                issues.push(format!("AssetImport source is offline or missing: {path}"));
            }
        }
    }
    if !issues.is_empty() {
        return EditPlanValidation {
            valid: false,
            issues,
            diff: None,
        };
    }

    let envelope = CommandEnvelope {
        schema_version: crate::modules::domain::CURRENT_SCHEMA_VERSION,
        project_id: plan.project_id.clone(),
        base_revision: plan.base_revision,
        client_id: None,
        client_epoch: None,
        sequence_number: None,
        dry_run: true,
        transaction_id: Some(format!("edit-plan-preview-r{}", plan.base_revision)),
        operations: plan.operations.clone(),
    };
    match CommandService::new(project.clone()).dry_run(&envelope) {
        Ok(diff) => EditPlanValidation {
            valid: true,
            issues,
            diff: Some(diff),
        },
        Err(error) => EditPlanValidation {
            valid: false,
            issues: vec![error.to_string()],
            diff: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::command::CommandOperation;
    use crate::modules::domain::time::FrameRate;
    use crate::modules::domain::TrackKind;

    #[test]
    fn scene_canvas_presets_have_stable_wire_names_and_dimensions() {
        let presets = [
            (
                SceneCanvasPreset::Landscape16x9,
                "landscape_16x9",
                (1920, 1080),
            ),
            (
                SceneCanvasPreset::Portrait9x16,
                "portrait_9x16",
                (1080, 1920),
            ),
            (SceneCanvasPreset::Square1x1, "square_1x1", (1080, 1080)),
            (SceneCanvasPreset::Portrait4x5, "portrait_4x5", (1080, 1350)),
        ];
        for (preset, wire_name, dimensions) in presets {
            assert_eq!(
                serde_json::to_string(&preset).unwrap(),
                format!("\"{wire_name}\"")
            );
            assert_eq!(preset.dimensions(), dimensions);
        }
    }

    #[test]
    fn exact_scene_scheduler_hits_frame_target_and_respects_each_source_capacity() {
        let schedule = schedule_exact_scene_frames(&[90, 20, 60], 101).unwrap();
        assert_eq!(schedule.iter().sum::<u64>(), 101);
        assert_eq!(schedule, vec![41, 20, 40]);
        assert!(schedule
            .iter()
            .zip([90, 20, 60])
            .all(|(used, cap)| *used <= cap));
    }

    #[test]
    fn exact_scene_scheduler_reports_infeasible_targets() {
        assert!(schedule_exact_scene_frames(&[10, 10], 1)
            .unwrap_err()
            .contains("one frame"));
        assert!(schedule_exact_scene_frames(&[10, 10], 21)
            .unwrap_err()
            .contains("exceeds"));
        assert!(schedule_exact_scene_frames(&[0, 10], 5).is_err());
    }

    #[test]
    fn exact_scene_scheduler_exhaustively_preserves_capacity_and_fairness_in_small_cases() {
        fn visit(capacities: &mut Vec<u64>, remaining_slots: usize, schedules_checked: &mut usize) {
            if remaining_slots > 0 {
                for capacity in 1..=6 {
                    capacities.push(capacity);
                    visit(capacities, remaining_slots - 1, schedules_checked);
                    capacities.pop();
                }
                return;
            }

            let scene_count = capacities.len() as u64;
            let total_capacity = capacities.iter().sum::<u64>();
            for target in scene_count..=total_capacity {
                let schedule = schedule_exact_scene_frames(capacities, target).unwrap();
                assert_eq!(schedule.iter().sum::<u64>(), target);
                assert!(schedule
                    .iter()
                    .zip(capacities.iter())
                    .all(|(allocated, capacity)| allocated >= &1 && allocated <= capacity));

                let unsaturated = schedule
                    .iter()
                    .zip(capacities.iter())
                    .filter_map(|(allocated, capacity)| {
                        (allocated < capacity).then_some(*allocated)
                    })
                    .collect::<Vec<_>>();
                if let (Some(minimum), Some(maximum)) =
                    (unsaturated.iter().min(), unsaturated.iter().max())
                {
                    assert!(
                        maximum - minimum <= 1,
                        "non-saturated scenes should differ by at most one frame: capacities={capacities:?}, target={target}, schedule={schedule:?}"
                    );
                }
                *schedules_checked += 1;
            }
        }

        let mut schedules_checked = 0;
        for scene_count in 1..=4 {
            visit(
                &mut Vec::with_capacity(scene_count),
                scene_count,
                &mut schedules_checked,
            );
        }
        assert_eq!(schedules_checked, 16_329);
    }

    fn valid_plan(project: &Project) -> EditPlan {
        EditPlan {
            schema_version: EDIT_PLAN_SCHEMA_VERSION,
            goal: "Create a short draft".to_string(),
            project_id: project.project_id.clone(),
            base_revision: project.revision,
            referenced_media_ids: vec![],
            assumptions: vec!["Use one video track".to_string()],
            warnings: vec![],
            operations: vec![OperationEnvelope {
                operation_id: "op-track".to_string(),
                operation: CommandOperation::TrackCreate {
                    track_id: Some("video-1".to_string()),
                    name: "Video".to_string(),
                    kind: TrackKind::Video,
                },
            }],
        }
    }

    #[test]
    fn validates_plan_with_command_service_dry_run_without_mutating_project() {
        let project = Project::new("Plan test".to_string());
        let before = project.clone();
        let result = validate_edit_plan(&project, &valid_plan(&project));
        assert!(result.valid, "{result:?}");
        assert!(result.diff.is_some());
        assert_eq!(project, before);
    }

    #[test]
    fn rejects_unsupported_version_stale_revision_and_missing_media() {
        let project = Project::new("Plan test".to_string());
        let mut plan = valid_plan(&project);
        plan.schema_version += 1;
        plan.base_revision += 1;
        plan.referenced_media_ids.push("missing-media".to_string());
        let result = validate_edit_plan(&project, &plan);
        assert!(!result.valid);
        assert!(result.diff.is_none());
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.contains("Unsupported edit-plan schema")));
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.contains("Stale edit plan")));
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.contains("missing-media")));
    }

    #[test]
    fn rejects_offline_referenced_media() {
        let mut project = Project::new("Plan test".to_string());
        project.assets.push(crate::modules::domain::Asset {
            asset_id: "offline-asset".to_string(),
            name: "offline.mp4".to_string(),
            path: "Z:/missing/offline.mp4".to_string(),
            media_kind: crate::modules::domain::MediaKind::Video,
            duration: None,
            file_size: 0,
            dimensions: None,
        });
        let mut plan = valid_plan(&project);
        plan.referenced_media_ids.push("offline-asset".to_string());
        let result = validate_edit_plan(&project, &plan);
        assert!(!result.valid);
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.contains("offline or missing")));
    }

    #[test]
    fn clip_trim_plan_shortens_tail_at_speed_without_mutating_project() {
        let dir =
            std::env::temp_dir().join(format!("tinixcut_trim_plan_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let media_path = dir.join("source.mp4");
        std::fs::write(&media_path, b"trim planner fixture").unwrap();
        let mut project = Project::new("Trim plan test".to_string());
        project.assets.push(crate::modules::domain::Asset {
            asset_id: "video-source".to_string(),
            name: "source.mp4".to_string(),
            path: media_path.to_string_lossy().to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(300, FrameRate::FPS_30)),
            file_size: 20,
            dimensions: None,
        });
        let mut item = crate::modules::domain::TimelineItem::new(
            Some("video-source".to_string()),
            "Scene A".to_string(),
            ItemKind::Video,
            TimeRange::new(
                RationalTime::new(30, FrameRate::FPS_30),
                RationalTime::new(90, FrameRate::FPS_30),
            ),
            TimeRange::new(
                RationalTime::new(15, FrameRate::FPS_30),
                RationalTime::new(90, FrameRate::FPS_30),
            ),
        )
        .unwrap();
        item.speed = crate::modules::domain::ClipSpeed::Half;
        item.source_range.duration = RationalTime::new(45, FrameRate::FPS_30);
        let item_id = item.item_id.clone();
        let mut track = crate::modules::domain::Track::new("Video 1".to_string(), TrackKind::Video);
        track.items.push(item);
        project.tracks.push(track);
        let before = project.clone();

        let plan = build_clip_trim_plan(&project, "Shorten Scene A", &item_id, 60).unwrap();
        assert_eq!(project, before);
        assert_eq!(plan.base_revision, project.revision);
        assert_eq!(plan.referenced_media_ids, vec!["video-source"]);
        assert!(plan.warnings.is_empty());
        let CommandOperation::ClipTrim {
            item_id: planned_item_id,
            new_timeline_range,
            new_source_range,
        } = &plan.operations[0].operation
        else {
            panic!("expected ClipTrim operation")
        };
        assert_eq!(planned_item_id, &item_id);
        assert_eq!(new_timeline_range.start.value, 30);
        assert_eq!(new_timeline_range.duration.value, 60);
        assert_eq!(new_source_range.start.value, 15);
        assert_eq!(new_source_range.duration.value, 30);
        let validation = validate_edit_plan(&project, &plan);
        assert!(validation.valid, "{validation:?}");

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn clip_trim_plan_rejects_non_shortening_and_locked_tracks() {
        let mut project = Project::new("Trim validation test".to_string());
        project.assets.push(crate::modules::domain::Asset {
            asset_id: "asset".to_string(),
            name: "source.mp4".to_string(),
            path: "unused-test-path.mp4".to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(90, FrameRate::FPS_30)),
            file_size: 0,
            dimensions: None,
        });
        let mut track = crate::modules::domain::Track::new("Video 1".to_string(), TrackKind::Video);
        let item = crate::modules::domain::TimelineItem::new(
            Some("asset".to_string()),
            "Scene".to_string(),
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
        let item_id = item.item_id.clone();
        track.items.push(item);
        project.tracks.push(track);

        assert!(build_clip_trim_plan(&project, "Shorten", &item_id, 90)
            .unwrap_err()
            .contains("shorter than"));
        project.tracks[0].locked = true;
        assert!(build_clip_trim_plan(&project, "Shorten", &item_id, 60)
            .unwrap_err()
            .contains("locked track"));
    }

    #[test]
    fn deterministic_video_draft_is_repeatable_and_dry_run_valid() {
        let dir =
            std::env::temp_dir().join(format!("tinixcut_draft_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut project = Project::new("Draft test".to_string());
        for index in 0..2 {
            let path = dir.join(format!("source-{index}.mp4"));
            std::fs::write(&path, b"fixture-placeholder-for-planner-boundary-test").unwrap();
            project.assets.push(crate::modules::domain::Asset {
                asset_id: format!("video-{index}"),
                name: format!("source-{index}.mp4"),
                path: path.to_string_lossy().to_string(),
                media_kind: MediaKind::Video,
                duration: Some(RationalTime::new(300, FrameRate::FPS_30)),
                file_size: 48,
                dimensions: None,
            });
        }
        let media_ids = vec!["video-0".to_string(), "video-1".to_string()];
        let first = build_deterministic_video_draft(&project, "Reference 20s cut", &media_ids, 600)
            .unwrap();
        let second =
            build_deterministic_video_draft(&project, "Reference 20s cut", &media_ids, 600)
                .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.operations.len(), 4);
        assert_eq!(first.referenced_media_ids, media_ids);
        assert!(validate_edit_plan(&project, &first).valid);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn local_planner_receipt_does_not_keep_mock_provider_warning() {
        let dir = std::env::temp_dir().join(format!(
            "tinixcut_local_draft_metadata_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("source.mp4");
        std::fs::write(&path, b"disposable planner metadata fixture").unwrap();
        let mut project = Project::new("Local draft metadata test".to_string());
        project.assets.push(crate::modules::domain::Asset {
            asset_id: "video-1".to_string(),
            name: "source.mp4".to_string(),
            path: path.to_string_lossy().to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(300, FrameRate::FPS_30)),
            file_size: 35,
            dimensions: None,
        });
        let mut plan =
            build_deterministic_video_draft(&project, "A short cut", &["video-1".into()], 150)
                .unwrap();

        apply_local_planner_receipt_metadata(&mut plan, "Keep the sequence concise.");

        assert!(!plan
            .warnings
            .iter()
            .any(|warning| warning.contains("mock provider")));
        assert!(plan
            .warnings
            .iter()
            .any(|warning| warning.contains("did not inspect media bytes")));
        assert!(plan
            .assumptions
            .iter()
            .any(|item| item.contains("Local model selected the clip order")));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn deterministic_video_draft_rejects_missing_assets_and_empty_selection() {
        let project = Project::new("Draft test".to_string());
        assert!(
            build_deterministic_video_draft(&project, "Draft", &["missing".into()], 30)
                .unwrap_err()
                .contains("does not exist")
        );
        assert!(build_deterministic_video_draft(&project, "Draft", &[], 30)
            .unwrap_err()
            .contains("Select between"));
    }

    #[test]
    fn visual_scene_draft_revalidates_sources_and_maps_ranges_to_reviewable_clips() {
        let dir = std::env::temp_dir().join(format!(
            "tinixcut_visual_draft_test_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("visual-fixture.mp4");
        std::fs::write(&path, b"disposable video fixture placeholder").unwrap();
        let mut project = Project::new("Scene draft test".to_string());
        project.assets.push(crate::modules::domain::Asset {
            asset_id: "video-1".to_string(),
            name: "visual-fixture.mp4".to_string(),
            path: path.to_string_lossy().to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(30, FrameRate::new(1, 1))),
            file_size: 36,
            dimensions: None,
        });
        let source_fingerprint = aggregate_source_fingerprint(&[VisualSceneSource {
            asset_id: "video-1".to_string(),
            asset_name: "visual-fixture.mp4".to_string(),
            path: path.clone(),
        }])
        .unwrap();
        let scene = |id: &str, start: u64, end: u64, caption: &str| {
            crate::modules::media::visual_scene::VisualSceneDescription {
                scene_id: id.to_string(),
                asset_id: "video-1".to_string(),
                asset_name: "visual-fixture.mp4".to_string(),
                source_start_micros: start,
                source_end_micros: end,
                sampled_at_micros: start + (end - start) / 2,
                caption: caption.to_string(),
                tags: vec!["street".to_string()],
                relevance_score: 0.8,
                rationale: "Visible in the representative frame".to_string(),
            }
        };
        let mut report = VisualSceneAnalysisReport {
            schema_version: VISUAL_SCENE_SCHEMA_VERSION,
            algorithm_version: VISUAL_SCENE_ALGORITHM_VERSION.to_string(),
            provider: "lm-studio-local".to_string(),
            model: "google/gemma-4-e4b".to_string(),
            query: "street at night".to_string(),
            source_fingerprint,
            config_fingerprint: "0".repeat(64),
            truncated: false,
            scenes: vec![
                scene("scene-01", 0, 5_000_000, "Opening street"),
                scene("scene-02", 10_000_000, 15_000_000, "Lantern street"),
            ],
            warnings: vec![],
        };
        let before = project.clone();
        let receipt = build_visual_scene_edit_plan(
            &project,
            "Use lantern street first",
            &report,
            &["scene-02".to_string(), "scene-01".to_string()],
        )
        .unwrap();
        assert_eq!(receipt.provider, "visual-scene-report-v1");
        assert!(receipt.validation.valid);
        assert_eq!(receipt.plan.base_revision, project.revision);
        assert_eq!(receipt.plan.operations.len(), 3);
        assert!(matches!(
            receipt.plan.operations[0].operation,
            CommandOperation::TrackCreate { .. }
        ));
        match &receipt.plan.operations[1].operation {
            CommandOperation::ClipCreate {
                timeline_start,
                source_start,
                duration,
                ..
            } => {
                assert_eq!(timeline_start.value, 0);
                assert_eq!(source_start.as_ref().unwrap().value, 300);
                assert_eq!(duration.value, 150);
            }
            other => panic!("Expected first scene clip, got {other:?}"),
        }
        match &receipt.plan.operations[2].operation {
            CommandOperation::ClipCreate {
                timeline_start,
                source_start,
                duration,
                ..
            } => {
                assert_eq!(timeline_start.value, 150);
                assert_eq!(source_start.as_ref().unwrap().value, 0);
                assert_eq!(duration.value, 150);
            }
            other => panic!("Expected second scene clip, got {other:?}"),
        }
        assert!(receipt
            .plan
            .warnings
            .iter()
            .any(|warning| warning.contains("may be inaccurate")));
        assert_eq!(project, before);

        let mut project_25_fps = project.clone();
        project_25_fps.settings.frame_rate = FrameRate::new(25, 1);
        let exact_25_fps = build_visual_scene_edit_plan_with_duration(
            &project_25_fps,
            "Exact four-second draft at 25 fps",
            &report,
            &["scene-01".into(), "scene-02".into()],
            Some(100),
            true,
        )
        .unwrap();
        assert!(
            exact_25_fps.validation.valid,
            "{:?}",
            exact_25_fps.validation
        );
        let clip_durations = exact_25_fps
            .plan
            .operations
            .iter()
            .filter_map(|operation| match &operation.operation {
                CommandOperation::ClipCreate { duration, .. } => Some(duration.value),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(clip_durations, vec![50, 50]);
        assert_eq!(clip_durations.iter().sum::<i64>(), 100);

        let mut unequal_report = report.clone();
        unequal_report.scenes[1].source_end_micros = 14_000_000;
        unequal_report.scenes[1].sampled_at_micros = 12_000_000;
        for (frame_rate, target, expected_durations) in [
            (FrameRate::FPS_29_97, 269, vec![149, 120]),
            (FrameRate::FPS_23_976, 215, vec![119, 96]),
        ] {
            let mut non_integer_project = project.clone();
            non_integer_project.settings.frame_rate = frame_rate;
            let draft = build_visual_scene_edit_plan_with_duration(
                &non_integer_project,
                "Exact draft at a non-integer frame rate",
                &unequal_report,
                &["scene-01".into(), "scene-02".into()],
                Some(target),
                true,
            )
            .unwrap();
            assert!(draft.validation.valid, "{:?}", draft.validation);
            let clips = draft
                .plan
                .operations
                .iter()
                .filter_map(|operation| match &operation.operation {
                    CommandOperation::ClipCreate {
                        timeline_start,
                        duration,
                        ..
                    } => Some((*timeline_start, *duration)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let actual_durations = clips
                .iter()
                .map(|(_, duration)| duration.value)
                .collect::<Vec<_>>();
            assert_eq!(actual_durations, expected_durations);
            assert_eq!(actual_durations.iter().sum::<i64>(), target as i64);
            assert_eq!(clips[0].0.value, 0);
            assert_eq!(clips[1].0.value, expected_durations[0]);
            assert!(clips
                .iter()
                .all(|(start, duration)| start.rate == frame_rate && duration.rate == frame_rate));
        }

        for (frame_rate, target, expected_source_durations) in [
            (FrameRate::FPS_29_97, 300, vec![150, 120]),
            (FrameRate::FPS_23_976, 240, vec![120, 96]),
        ] {
            let mut non_integer_project = project.clone();
            non_integer_project.settings.frame_rate = frame_rate;
            let draft = build_visual_scene_edit_plan_with_duration(
                &non_integer_project,
                "Advisory target preserves reviewed scene ranges",
                &unequal_report,
                &["scene-01".into(), "scene-02".into()],
                Some(target),
                false,
            )
            .unwrap();
            assert!(draft.validation.valid, "{:?}", draft.validation);
            let clips = draft
                .plan
                .operations
                .iter()
                .filter_map(|operation| match &operation.operation {
                    CommandOperation::ClipCreate {
                        timeline_start,
                        duration,
                        ..
                    } => Some((*timeline_start, *duration)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let preserved_durations = clips
                .iter()
                .map(|(_, duration)| duration.value)
                .collect::<Vec<_>>();
            assert_eq!(preserved_durations, expected_source_durations);
            assert!(preserved_durations.iter().sum::<i64>() < target as i64);
            assert_eq!(clips[1].0.value, expected_source_durations[0]);
            assert!(clips
                .iter()
                .all(|(start, duration)| start.rate == frame_rate && duration.rate == frame_rate));
            assert!(draft
                .plan
                .warnings
                .iter()
                .any(|warning| warning.contains("Reviewed scene ranges were preserved")));
        }

        let mut stale_report = report.clone();
        stale_report.source_fingerprint = "f".repeat(64);
        assert!(build_visual_scene_edit_plan(
            &project,
            "Stale",
            &stale_report,
            &["scene-01".into()]
        )
        .unwrap_err()
        .contains("stale"));
        assert!(build_visual_scene_edit_plan(
            &project,
            "Duplicate",
            &report,
            &["scene-01".into(), "scene-01".into()]
        )
        .unwrap_err()
        .contains("unique"));
        assert!(
            build_visual_scene_edit_plan(&project, "Unknown", &report, &["scene-99".into()])
                .unwrap_err()
                .contains("does not exist")
        );

        let mut malformed_range_report = report.clone();
        malformed_range_report.scenes[0].source_end_micros =
            malformed_range_report.scenes[0].source_start_micros;
        assert!(build_visual_scene_edit_plan(
            &project,
            "Malformed source range",
            &malformed_range_report,
            &["scene-01".into()]
        )
        .unwrap_err()
        .contains("invalid or duplicate scene range"));

        std::fs::remove_file(&path).unwrap();
        assert!(build_visual_scene_edit_plan(
            &project,
            "Offline source",
            &report,
            &["scene-01".into()]
        )
        .unwrap_err()
        .contains("offline or missing"));
        std::fs::write(&path, b"disposable video fixture placeholder").unwrap();
        report.source_fingerprint = aggregate_source_fingerprint(&[VisualSceneSource {
            asset_id: "video-1".to_string(),
            asset_name: "visual-fixture.mp4".to_string(),
            path: path.clone(),
        }])
        .unwrap();
        let title_draft = build_visual_scene_edit_plan_with_constraints(
            &project,
            "Create a titled street draft",
            &report,
            &["scene-01".into()],
            None,
            false,
            SceneSelectionConstraints::default(),
            Some(SceneCanvasPreset::Portrait9x16),
            Some("Hội An buổi tối"),
            Some(SceneTitleTemplate::BoldHeadline),
        )
        .unwrap();
        assert!(title_draft.validation.valid, "{:?}", title_draft.validation);
        assert!(matches!(
            title_draft.plan.operations[0].operation,
            CommandOperation::ProjectSettingsSet {
                width: 1080,
                height: 1920
            }
        ));
        assert!(title_draft.plan.operations.iter().any(|operation| matches!(
            &operation.operation,
            CommandOperation::ClipTextSet { text, .. } if text == "Hội An buổi tối"
        )));
        assert!(title_draft.plan.operations.iter().any(|operation| matches!(
            &operation.operation,
            CommandOperation::TextTemplateApply { template_id, template_version: 1, .. }
                if template_id == "bold-headline"
        )));
        assert!(build_visual_scene_edit_plan_with_constraints(
            &project,
            "Missing template",
            &report,
            &["scene-01".into()],
            None,
            false,
            SceneSelectionConstraints::default(),
            None,
            Some("Title only"),
            None,
        )
        .unwrap_err()
        .contains("supplied together"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn phase55_structured_scene_brief_constraint_corpus() {
        let available = vec!["scene-a".to_string(), "scene-b".to_string()];
        let cases = [
            (
                "en",
                "Keep the harbor",
                vec!["scene-b"],
                vec![],
                vec!["scene-b"],
                true,
            ),
            (
                "vi",
                "Giữ cảnh bến cảng",
                vec!["scene-b"],
                vec!["scene-a"],
                vec!["scene-b"],
                true,
            ),
            (
                "en",
                "Exclude the coast",
                vec![],
                vec!["scene-a"],
                vec!["scene-b"],
                true,
            ),
            (
                "vi",
                "Bắt buộc có bến cảng",
                vec!["scene-b"],
                vec![],
                vec!["scene-a"],
                false,
            ),
            (
                "en",
                "Do not use the coast",
                vec![],
                vec!["scene-a"],
                vec!["scene-a"],
                false,
            ),
            (
                "vi",
                "Giữ cảnh không tồn tại",
                vec!["scene-x"],
                vec![],
                vec!["scene-b"],
                false,
            ),
            (
                "en",
                "Exclude an unknown scene",
                vec![],
                vec!["scene-x"],
                vec!["scene-b"],
                false,
            ),
            (
                "vi",
                "Không lặp cảnh bắt buộc",
                vec!["scene-a", "scene-a"],
                vec![],
                vec!["scene-a"],
                false,
            ),
            (
                "en",
                "Unique exclusion only",
                vec![],
                vec!["scene-b", "scene-b"],
                vec!["scene-a"],
                false,
            ),
            (
                "vi",
                "Không vừa giữ vừa loại",
                vec!["scene-a"],
                vec!["scene-a"],
                vec!["scene-a"],
                false,
            ),
            (
                "en",
                "Reject a blank required ID",
                vec![""],
                vec![],
                vec!["scene-a"],
                false,
            ),
            (
                "vi",
                "Reject a blank excluded ID",
                vec![],
                vec![""],
                vec!["scene-b"],
                false,
            ),
        ];

        assert_eq!(cases.len(), 12);
        for (locale, brief, keep, omit, selected, expected_valid) in cases {
            assert!(!brief.trim().is_empty(), "{locale} fixture needs a brief");
            let constraints = SceneSelectionConstraints {
                must_keep_scene_ids: keep.into_iter().map(str::to_string).collect(),
                must_omit_scene_ids: omit.into_iter().map(str::to_string).collect(),
                ..SceneSelectionConstraints::default()
            };
            let selection = selected.into_iter().map(str::to_string).collect::<Vec<_>>();
            let valid = constraints
                .validate(&available)
                .and_then(|()| constraints.validate_selection(&selection))
                .is_ok();
            assert_eq!(
                valid, expected_valid,
                "fixture locale={locale}, brief={brief}"
            );
        }
    }

    #[test]
    fn phase55_bilingual_brief_corpus_runs_through_scene_draft_planner() {
        let dir = std::env::temp_dir().join(format!(
            "tinixcut_phase55_brief_corpus_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path_a = dir.join("source-a.mp4");
        let path_b = dir.join("source-b.mp4");
        for (path, color) in [(&path_a, "blue"), (&path_b, "green")] {
            let mut command =
                crate::modules::media::resolver::MediaBinaryResolver::command("ffmpeg")
                    .expect("bundled FFmpeg must be available for the isolated fixture");
            let output = command
                .args([
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    &format!("color=c={color}:s=128x128:r=30:d=5"),
                    "-an",
                    "-c:v",
                    "libx264",
                    "-pix_fmt",
                    "yuv420p",
                    "-y",
                ])
                .arg(path)
                .output()
                .expect("bundled FFmpeg must render the fixture videos");
            assert!(
                output.status.success(),
                "silent source fixture generation failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let mut project = Project::new("Phase 55 bilingual brief corpus".into());
        for (asset_id, name, path) in [
            ("asset-a", "source-a.mp4", path_a.clone()),
            ("asset-b", "source-b.mp4", path_b.clone()),
        ] {
            project.assets.push(crate::modules::domain::Asset {
                asset_id: asset_id.into(),
                name: name.into(),
                path: path.to_string_lossy().into_owned(),
                media_kind: MediaKind::Video,
                duration: Some(RationalTime::new(150, FrameRate::FPS_30)),
                file_size: std::fs::metadata(path).unwrap().len(),
                dimensions: None,
            });
        }
        let sources = vec![
            VisualSceneSource {
                asset_id: "asset-a".into(),
                asset_name: "source-a.mp4".into(),
                path: path_a.clone(),
            },
            VisualSceneSource {
                asset_id: "asset-b".into(),
                asset_name: "source-b.mp4".into(),
                path: path_b.clone(),
            },
        ];
        let source_fingerprint = aggregate_source_fingerprint(&sources).unwrap();
        let scene = |id: &str, asset_id: &str, start: u64, end: u64, caption: &str| {
            crate::modules::media::visual_scene::VisualSceneDescription {
                scene_id: id.into(),
                asset_id: asset_id.into(),
                asset_name: if asset_id == "asset-a" {
                    "source-a.mp4".into()
                } else {
                    "source-b.mp4".into()
                },
                source_start_micros: start,
                source_end_micros: end,
                sampled_at_micros: start + (end - start) / 2,
                caption: caption.into(),
                tags: vec!["fixture".into()],
                relevance_score: 0.75,
                rationale: "Fixture caption for deterministic planner coverage".into(),
            }
        };
        let report = VisualSceneAnalysisReport {
            schema_version: VISUAL_SCENE_SCHEMA_VERSION,
            algorithm_version: VISUAL_SCENE_ALGORITHM_VERSION.into(),
            provider: "fixture-model".into(),
            model: "fixture-v1".into(),
            query: "Hội An travel montage / Hoi An travel montage".into(),
            source_fingerprint,
            config_fingerprint: "1".repeat(64),
            truncated: false,
            scenes: vec![
                scene("scene-a", "asset-a", 0, 2_000_000, "Bến cảng / harbor"),
                scene(
                    "scene-b",
                    "asset-b",
                    1_000_000,
                    5_000_000,
                    "Phố đèn lồng / lantern street",
                ),
            ],
            warnings: vec!["Captions are fixture data, not model truth".into()],
        };
        let project_before = project.clone();
        let cases = [
            (
                "vi",
                "Mở bằng bến cảng",
                vec!["scene-a"],
                None,
                false,
                SceneSelectionConstraints::default(),
                None,
                None,
                None,
                true,
            ),
            (
                "en",
                "Make an exact two-second opening",
                vec!["scene-a", "scene-b"],
                Some(60),
                true,
                SceneSelectionConstraints::default(),
                None,
                None,
                None,
                true,
            ),
            (
                "vi",
                "Video dọc với tiêu đề Hội An",
                vec!["scene-b"],
                Some(90),
                true,
                SceneSelectionConstraints::default(),
                Some(SceneCanvasPreset::Portrait9x16),
                Some("Hội An"),
                Some(SceneTitleTemplate::BoldHeadline),
                true,
            ),
            (
                "en",
                "Use only source B",
                vec!["scene-b"],
                None,
                false,
                SceneSelectionConstraints {
                    allowed_asset_ids: vec!["asset-b".into()],
                    ..SceneSelectionConstraints::default()
                },
                None,
                None,
                None,
                true,
            ),
            (
                "vi",
                "Bắt buộc có bến cảng",
                vec!["scene-a", "scene-b"],
                None,
                false,
                SceneSelectionConstraints {
                    must_keep_scene_ids: vec!["scene-a".into()],
                    ..SceneSelectionConstraints::default()
                },
                None,
                None,
                None,
                true,
            ),
            (
                "en",
                "Do not use the harbor",
                vec!["scene-b"],
                None,
                false,
                SceneSelectionConstraints {
                    must_omit_scene_ids: vec!["scene-a".into()],
                    ..SceneSelectionConstraints::default()
                },
                None,
                None,
                None,
                true,
            ),
            (
                "vi",
                "Giữ cảnh không có trong media",
                vec!["scene-b"],
                None,
                false,
                SceneSelectionConstraints {
                    must_keep_scene_ids: vec!["scene-missing".into()],
                    ..SceneSelectionConstraints::default()
                },
                None,
                None,
                None,
                false,
            ),
            (
                "en",
                "Allow an unavailable source",
                vec!["scene-b"],
                None,
                false,
                SceneSelectionConstraints {
                    allowed_asset_ids: vec!["asset-missing".into()],
                    ..SceneSelectionConstraints::default()
                },
                None,
                None,
                None,
                false,
            ),
            (
                "vi",
                "Độ dài chính xác quá ngắn",
                vec!["scene-a", "scene-b"],
                Some(1),
                true,
                SceneSelectionConstraints::default(),
                None,
                None,
                None,
                false,
            ),
            (
                "en",
                "Exact duration exceeds source capacity",
                vec!["scene-a", "scene-b"],
                Some(500),
                true,
                SceneSelectionConstraints::default(),
                None,
                None,
                None,
                false,
            ),
            (
                "vi",
                "Từ chối khoảng cảnh sai",
                vec!["scene-a"],
                None,
                false,
                SceneSelectionConstraints::default(),
                None,
                None,
                None,
                false,
            ),
            (
                "en",
                "Reject an offline source",
                vec!["scene-b"],
                None,
                false,
                SceneSelectionConstraints::default(),
                None,
                None,
                None,
                false,
            ),
        ];

        assert_eq!(cases.len(), 12);
        for (
            locale,
            brief,
            selected,
            frames,
            exact,
            constraints,
            canvas,
            title,
            template,
            expected_ok,
        ) in cases
        {
            let mut case_report = report.clone();
            if brief == "Từ chối khoảng cảnh sai" {
                case_report.scenes[0].source_end_micros = case_report.scenes[0].source_start_micros;
            }
            if brief == "Reject an offline source" {
                std::fs::remove_file(&path_b).unwrap();
            }
            let result = build_visual_scene_edit_plan_with_constraints(
                &project,
                brief,
                &case_report,
                &selected.into_iter().map(str::to_string).collect::<Vec<_>>(),
                frames,
                exact,
                constraints,
                canvas,
                title,
                template,
            );
            assert_eq!(
                result.is_ok(),
                expected_ok,
                "locale={locale}, brief={brief}: {result:?}"
            );
            if let Ok(receipt) = result {
                assert!(receipt.validation.valid, "locale={locale}, brief={brief}");
                assert_eq!(receipt.plan.project_id, project.project_id);
                if let Some(target) = frames.filter(|_| exact) {
                    let clip_frames: i64 = receipt
                        .plan
                        .operations
                        .iter()
                        .filter_map(|operation| match &operation.operation {
                            CommandOperation::ClipCreate {
                                kind: ItemKind::Video,
                                duration,
                                ..
                            } => Some(duration.rescaled_to(project.settings.frame_rate).value),
                            _ => None,
                        })
                        .sum();
                    assert_eq!(clip_frames, target as i64, "locale={locale}, brief={brief}");
                }
            }
            assert_eq!(
                project, project_before,
                "draft planning must be read-only: {brief}"
            );
            if brief == "Reject an offline source" {
                let mut command =
                    crate::modules::media::resolver::MediaBinaryResolver::command("ffmpeg")
                        .expect("bundled FFmpeg must be available for fixture restoration");
                let output = command
                    .args([
                        "-hide_banner",
                        "-loglevel",
                        "error",
                        "-f",
                        "lavfi",
                        "-i",
                        "color=c=green:s=128x128:r=30:d=5",
                        "-an",
                        "-c:v",
                        "libx264",
                        "-pix_fmt",
                        "yuv420p",
                        "-y",
                    ])
                    .arg(&path_b)
                    .output()
                    .expect("bundled FFmpeg must restore the fixture video");
                assert!(output.status.success());
            }
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn allowed_source_asset_constraints_reject_unknown_duplicate_and_blank_ids() {
        let available = vec!["asset-a".to_string(), "asset-b".to_string()];
        assert!(SceneSelectionConstraints {
            allowed_asset_ids: vec!["asset-a".into()],
            ..SceneSelectionConstraints::default()
        }
        .validate_allowed_assets(&available)
        .is_ok());
        assert!(SceneSelectionConstraints::default()
            .validate_allowed_assets(&available)
            .is_ok());
        for invalid in [
            vec!["unknown".to_string()],
            vec!["asset-a".to_string(), "asset-a".to_string()],
            vec![String::new()],
        ] {
            assert!(SceneSelectionConstraints {
                allowed_asset_ids: invalid,
                ..SceneSelectionConstraints::default()
            }
            .validate_allowed_assets(&available)
            .is_err());
        }
    }

    #[test]
    fn scene_caption_and_audio_intents_are_bounded_and_plain_text() {
        assert!(SceneSelectionConstraints {
            caption_intent: Some("Vietnamese captions".into()),
            audio_intent: Some("Keep a calm natural sound mood".into()),
            ..SceneSelectionConstraints::default()
        }
        .validate_intents()
        .is_ok());
        for invalid in [
            Some(String::new()),
            Some("x".repeat(501)),
            Some("line\nbreak".into()),
        ] {
            assert!(SceneSelectionConstraints {
                audio_intent: invalid,
                ..SceneSelectionConstraints::default()
            }
            .validate_intents()
            .is_err());
        }
    }

    #[tokio::test]
    async fn local_scene_suggestion_is_bounded_private_review_only_and_validates_ids() {
        use std::io::{BufRead, BufReader, Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let dir =
            std::env::temp_dir().join(format!("tinixcut_scene_suggest_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("private-source-path.mp4");
        std::fs::write(&path, b"disposable scene planner fixture").unwrap();
        let excluded_path = dir.join("excluded-source-path.mp4");
        std::fs::write(&excluded_path, b"excluded disposable fixture").unwrap();
        let mut project = Project::new("Scene suggestion fixture".into());
        project.assets.push(crate::modules::domain::Asset {
            asset_id: "video-asset".into(),
            name: "private-source-name.mp4".into(),
            path: path.to_string_lossy().into_owned(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(30, FrameRate::new(1, 1))),
            file_size: 32,
            dimensions: None,
        });
        project.assets.push(crate::modules::domain::Asset {
            asset_id: "excluded-asset".into(),
            name: "excluded-private-source.mp4".into(),
            path: excluded_path.to_string_lossy().into_owned(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(30, FrameRate::new(1, 1))),
            file_size: 27,
            dimensions: None,
        });
        let source_fingerprint = aggregate_source_fingerprint(&[
            VisualSceneSource {
                asset_id: "video-asset".into(),
                asset_name: "private-source-name.mp4".into(),
                path: path.clone(),
            },
            VisualSceneSource {
                asset_id: "excluded-asset".into(),
                asset_name: "excluded-private-source.mp4".into(),
                path: excluded_path,
            },
        ])
        .unwrap();
        let scene =
            |id: &str, asset_id: &str, asset_name: &str, start: u64, end: u64, caption: &str| {
                crate::modules::media::visual_scene::VisualSceneDescription {
                    scene_id: id.into(),
                    asset_id: asset_id.into(),
                    asset_name: asset_name.into(),
                    source_start_micros: start,
                    source_end_micros: end,
                    sampled_at_micros: start + (end - start) / 2,
                    caption: caption.into(),
                    tags: vec!["coast".into()],
                    relevance_score: 0.8,
                    rationale: "Sampled frame suggestion".into(),
                }
            };
        let report = VisualSceneAnalysisReport {
            schema_version: VISUAL_SCENE_SCHEMA_VERSION,
            algorithm_version: VISUAL_SCENE_ALGORITHM_VERSION.into(),
            provider: "lm-studio-local".into(),
            model: "test-model".into(),
            query: "calm coast".into(),
            source_fingerprint,
            config_fingerprint: "0".repeat(64),
            truncated: false,
            scenes: vec![
                scene(
                    "scene-a",
                    "video-asset",
                    "private-source-name.mp4",
                    0,
                    5_000_000,
                    "Calm coast",
                ),
                scene(
                    "scene-b",
                    "video-asset",
                    "private-source-name.mp4",
                    5_000_000,
                    10_000_000,
                    "Busy harbor",
                ),
                scene(
                    "scene-c",
                    "excluded-asset",
                    "excluded-private-source.mp4",
                    10_000_000,
                    15_000_000,
                    "Restricted content caption",
                ),
            ],
            warnings: vec![],
        };

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let expected_path = path.to_string_lossy().into_owned();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            let mut content_length = 0_usize;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length: ") {
                    content_length = value.trim().parse().unwrap();
                }
            }
            let mut body = vec![0; content_length];
            reader.read_exact(&mut body).unwrap();
            let request: serde_json::Value = serde_json::from_slice(&body).unwrap();
            let user_message = request["messages"]
                .as_array()
                .unwrap()
                .iter()
                .find(|message| message["role"] == "user")
                .unwrap();
            let user_payload: serde_json::Value =
                serde_json::from_str(user_message["content"].as_str().unwrap()).unwrap();
            let request_text = request.to_string();
            assert!(!request_text.contains(&expected_path));
            assert!(!request_text.contains("private-source-name.mp4"));
            assert!(!request_text.contains("excluded-private-source"));
            assert!(!request_text.contains("Restricted content caption"));
            assert!(!request_text.to_ascii_lowercase().contains("transcript"));
            let user_keys = user_payload
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(
                user_keys,
                [
                    "audioIntent",
                    "brief",
                    "captionIntent",
                    "constraints",
                    "scenes",
                    "suggestOpeningTitle",
                    "targetDurationSeconds",
                ]
                .into_iter()
                .collect()
            );
            let scene_keys = user_payload["scenes"][0]
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(
                scene_keys,
                [
                    "analysisRationale",
                    "caption",
                    "endMicros",
                    "sceneId",
                    "startMicros",
                    "tags",
                ]
                .into_iter()
                .collect()
            );
            assert!(request_text.contains("scene-a"));
            assert!(request_text.contains("scene-b"));
            assert!(!request_text.contains("scene-c"));
            assert!(request_text.contains("Calm coast"));
            assert!(request_text.contains("mustKeepSceneIds"));
            assert!(request_text.contains("Vietnamese short captions"));
            assert!(request_text.contains("calm ambient sound"));
            assert!(request_text.contains("suggestOpeningTitle"));
            let response_body = serde_json::json!({"choices":[{"message":{"content":"{\"selectedSceneIds\":[\"scene-b\"],\"rationale\":\"The harbor provides a lively ending.\",\"openingTitleSuggestion\":\"A Calm Harbor\"}"}}]}).to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response_body.len(), response_body).unwrap();
        });
        let before = project.clone();
        let constraints = SceneSelectionConstraints {
            allowed_asset_ids: vec!["video-asset".into()],
            must_keep_scene_ids: vec!["scene-b".into()],
            must_omit_scene_ids: vec!["scene-a".into()],
            caption_intent: Some("Vietnamese short captions".into()),
            audio_intent: Some("calm ambient sound".into()),
            suggest_opening_title: true,
            ..SceneSelectionConstraints::default()
        };
        let suggestion = suggest_visual_scene_selection_at(
            &project,
            "Open with a harbor for a 5-second travel clip",
            &report,
            Some(5),
            "test-model",
            constraints.clone(),
            &format!("http://{address}/v1/chat/completions"),
            std::time::Duration::from_secs(3),
        )
        .await
        .unwrap();
        server.join().unwrap();
        assert_eq!(suggestion.ordered_scene_ids, vec!["scene-b"]);
        assert_eq!(suggestion.selected_duration_micros, 5_000_000);
        assert_eq!(suggestion.target_duration_seconds, Some(5));
        assert_eq!(
            suggestion.opening_title_suggestion.as_deref(),
            Some("A Calm Harbor")
        );
        assert_eq!(suggestion.constraints, constraints);
        assert_eq!(suggestion.provider, LOCAL_LM_STUDIO_PROVIDER);
        assert_eq!(project, before);

        let transcript_fingerprint =
            crate::modules::media::ffmpeg::FFmpegService::source_fingerprint(&path).unwrap();
        let transcript_evidence = vec![TranscriptSceneEvidence {
            asset_id: "video-asset".into(),
            source_fingerprint: transcript_fingerprint.clone(),
            analyzed_duration_micros: 15_000_000,
            segments: vec![
                TranscriptSceneEvidenceSegment {
                    start_micros: 500_000,
                    end_micros: 1_500_000,
                    text: "morning market voices".into(),
                },
                TranscriptSceneEvidenceSegment {
                    start_micros: 5_500_000,
                    end_micros: 6_500_000,
                    text: "boat seller calling".into(),
                },
                TranscriptSceneEvidenceSegment {
                    start_micros: 10_500_000,
                    end_micros: 11_500_000,
                    text: "unmatched excerpt must stay local".into(),
                },
            ],
        }];
        let transcript_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let transcript_address = transcript_listener.local_addr().unwrap();
        let transcript_expected_path = path.to_string_lossy().into_owned();
        let transcript_expected_fingerprint = transcript_fingerprint.clone();
        let transcript_server = thread::spawn(move || {
            let (mut stream, _) = transcript_listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let mut length = 0_usize;
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                if header == "\r\n" || header.is_empty() {
                    break;
                }
                if let Some(value) = header.to_ascii_lowercase().strip_prefix("content-length: ") {
                    length = value.trim().parse().unwrap();
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let request: serde_json::Value = serde_json::from_slice(&body).unwrap();
            let text = request.to_string();
            assert!(!text.contains(&transcript_expected_path));
            assert!(!text.contains("private-source-name.mp4"));
            assert!(!text.contains("video-asset"));
            assert!(!text.contains(&transcript_expected_fingerprint));
            assert!(!text.contains("unmatched excerpt must stay local"));
            assert!(text.contains("morning market voices"));
            assert!(text.contains("boat seller calling"));
            let content = request["messages"][1]["content"].as_str().unwrap();
            let user_payload: serde_json::Value = serde_json::from_str(content).unwrap();
            assert_eq!(
                user_payload["scenes"][0]["transcriptEvidence"][0]["text"],
                "morning market voices"
            );
            assert_eq!(
                user_payload["scenes"][1]["transcriptEvidence"][0]["text"],
                "boat seller calling"
            );
            assert!(user_payload["scenes"][2]
                .get("transcriptEvidence")
                .is_none());
            let response_body = serde_json::json!({"choices":[{"message":{"content":"{\"selectedSceneIds\":[\"scene-b\"],\"rationale\":\"The harbor dialogue matches the brief.\",\"openingTitleSuggestion\":null}"}}]}).to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response_body.len(), response_body).unwrap();
        });
        let transcript_suggestion = suggest_visual_scene_selection_at_with_transcript(
            &project,
            "Find spoken harbor activity",
            &report,
            None,
            "test-model",
            SceneSelectionConstraints {
                allowed_asset_ids: vec!["video-asset".into()],
                ..SceneSelectionConstraints::default()
            },
            transcript_evidence.clone(),
            &format!("http://{transcript_address}/v1/chat/completions"),
            std::time::Duration::from_secs(3),
        )
        .await
        .unwrap();
        transcript_server.join().unwrap();
        assert_eq!(transcript_suggestion.ordered_scene_ids, vec!["scene-b"]);
        assert_eq!(transcript_suggestion.transcript_evidence_sources.len(), 1);
        assert_eq!(
            transcript_suggestion.transcript_evidence_sources[0].source_fingerprint,
            transcript_fingerprint
        );
        assert_eq!(
            transcript_suggestion.transcript_evidence_sources[0].segment_count,
            2
        );
        let stale_error = suggest_visual_scene_selection_at_with_transcript(
            &project,
            "Stale transcript should fail before a model call",
            &report,
            None,
            "test-model",
            SceneSelectionConstraints::default(),
            vec![TranscriptSceneEvidence {
                source_fingerprint: "0".repeat(64),
                ..transcript_evidence[0].clone()
            }],
            "http://127.0.0.1:1",
            std::time::Duration::from_millis(1),
        )
        .await
        .unwrap_err();
        assert!(stale_error.contains("stale"));
        let out_of_range_error = suggest_visual_scene_selection_at_with_transcript(
            &project,
            "Out-of-range transcript should fail before a model call",
            &report,
            None,
            "test-model",
            SceneSelectionConstraints::default(),
            vec![TranscriptSceneEvidence {
                segments: vec![TranscriptSceneEvidenceSegment {
                    start_micros: 14_500_000,
                    end_micros: 15_500_000,
                    text: "beyond analyzed duration".into(),
                }],
                ..transcript_evidence[0].clone()
            }],
            "http://127.0.0.1:1",
            std::time::Duration::from_millis(1),
        )
        .await
        .unwrap_err();
        assert!(out_of_range_error.contains("outside its analyzed source range"));
        let disallowed_error = suggest_visual_scene_selection_at_with_transcript(
            &project,
            "Disallowed transcript should fail before a model call",
            &report,
            None,
            "test-model",
            SceneSelectionConstraints {
                allowed_asset_ids: vec!["excluded-asset".into()],
                ..SceneSelectionConstraints::default()
            },
            transcript_evidence,
            "http://127.0.0.1:1",
            std::time::Duration::from_millis(1),
        )
        .await
        .unwrap_err();
        assert!(disallowed_error.contains("allowed asset"));
        assert_eq!(project, before);

        let invalid_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let invalid_address = invalid_listener.local_addr().unwrap();
        let invalid_server = thread::spawn(move || {
            let (mut stream, _) = invalid_listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let mut length = 0_usize;
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                if header == "\r\n" || header.is_empty() {
                    break;
                }
                if let Some(value) = header.to_ascii_lowercase().strip_prefix("content-length: ") {
                    length = value.trim().parse().unwrap();
                }
            }
            let mut ignored = vec![0; length];
            reader.read_exact(&mut ignored).unwrap();
            let body = r#"{"choices":[{"message":{"content":"{\"selectedSceneIds\":[\"unknown-scene\"],\"rationale\":\"invalid\"}"}}]}"#;
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
        });
        let invalid = suggest_visual_scene_selection_at(
            &project,
            "Select a scene",
            &report,
            None,
            "test-model",
            SceneSelectionConstraints::default(),
            &format!("http://{invalid_address}/v1/chat/completions"),
            std::time::Duration::from_secs(3),
        )
        .await
        .unwrap_err();
        invalid_server.join().unwrap();
        assert!(invalid.contains("unknown or duplicate"));
        let excluded_selection = build_visual_scene_edit_plan_with_constraints(
            &project,
            "Use only the selected source",
            &report,
            &["scene-c".into()],
            None,
            false,
            SceneSelectionConstraints {
                allowed_asset_ids: vec!["video-asset".into()],
                ..SceneSelectionConstraints::default()
            },
            None,
            None,
            None,
        )
        .unwrap_err();
        assert!(excluded_selection.contains("disallowed source asset"));
        assert!(suggest_visual_scene_selection_at(
            &project,
            "Brief",
            &report,
            Some(0),
            "test-model",
            SceneSelectionConstraints::default(),
            "http://127.0.0.1:1",
            std::time::Duration::from_millis(1)
        )
        .await
        .unwrap_err()
        .contains("between 1 and 1800"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn opening_title_suggestion_is_opt_in_bounded_and_control_free() {
        assert_eq!(validate_opening_title_suggestion(None, true).unwrap(), None);
        assert_eq!(
            validate_opening_title_suggestion(Some("  Hoi An  ".into()), true)
                .unwrap()
                .as_deref(),
            Some("Hoi An")
        );
        assert!(validate_opening_title_suggestion(Some("Hoi An".into()), false).is_err());
        assert!(validate_opening_title_suggestion(Some("x".repeat(241)), true).is_err());
        assert!(validate_opening_title_suggestion(Some("Hoi\nAn".into()), true).is_err());
    }

    #[tokio::test]
    #[ignore = "requires an explicitly started LM Studio local API with a loaded vision/text model"]
    async fn live_local_scene_suggestion_returns_reviewable_subset() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../artifacts/phase44/disposable/phase44_hoi_an_visual_fixture.mp4");
        assert!(path.is_file(), "Phase 44 disposable fixture is required");
        let mut project = Project::new("Phase 46 local planner disposable project".into());
        project.assets.push(crate::modules::domain::Asset {
            asset_id: "phase46-video".into(),
            name: "synthetic-video.mp4".into(),
            path: path.to_string_lossy().into_owned(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(15, FrameRate::new(1, 1))),
            file_size: std::fs::metadata(&path).unwrap().len(),
            dimensions: None,
        });
        let source_fingerprint = aggregate_source_fingerprint(&[VisualSceneSource {
            asset_id: "phase46-video".into(),
            asset_name: "synthetic-video.mp4".into(),
            path,
        }])
        .unwrap();
        let scene = |scene_id: &str, start: u64, end: u64, caption: &str, tags: &[&str]| {
            crate::modules::media::visual_scene::VisualSceneDescription {
                scene_id: scene_id.into(),
                asset_id: "phase46-video".into(),
                asset_name: "synthetic-video.mp4".into(),
                source_start_micros: start,
                source_end_micros: end,
                sampled_at_micros: start + (end - start) / 2,
                caption: caption.into(),
                tags: tags.iter().map(|tag| (*tag).into()).collect(),
                relevance_score: 0.8,
                rationale: "Phase 44 representative-frame suggestion; verify by eye".into(),
            }
        };
        let report = VisualSceneAnalysisReport {
            schema_version: VISUAL_SCENE_SCHEMA_VERSION,
            algorithm_version: VISUAL_SCENE_ALGORITHM_VERSION.into(),
            provider: "lm-studio-local".into(),
            model: "google/gemma-4-e4b".into(),
            query: "Hoi An travel scenes".into(),
            source_fingerprint,
            config_fingerprint: "0".repeat(64),
            truncated: false,
            scenes: vec![
                scene(
                    "scene-01",
                    0,
                    5_000_000,
                    "Morning street with buildings and people",
                    &["street", "morning"],
                ),
                scene(
                    "scene-02",
                    5_000_000,
                    10_000_000,
                    "Conical-hat person beside river and boats",
                    &["river", "boat"],
                ),
                scene(
                    "scene-03",
                    10_000_000,
                    15_000_000,
                    "Wet lantern street and parked bicycle",
                    &["lantern", "street"],
                ),
            ],
            warnings: vec!["Captions may be inaccurate; review source media.".into()],
        };
        let before = project.clone();
        let suggestion = suggest_visual_scene_selection_with_constraints(
            &project,
            "Create a calm 8-second Hoi An travel montage that opens with a human-scale scene and ends with a memorable street image.",
            &report,
            Some(8),
            "google/gemma-4-e4b",
            SceneSelectionConstraints {
                suggest_opening_title: true,
                ..SceneSelectionConstraints::default()
            },
        ).await.unwrap();
        assert!(!suggestion.ordered_scene_ids.is_empty());
        assert!(suggestion
            .ordered_scene_ids
            .iter()
            .all(|id| report.scenes.iter().any(|scene| scene.scene_id == *id)));
        assert!(suggestion.selected_duration_micros > 0);
        let title = suggestion
            .opening_title_suggestion
            .as_deref()
            .expect("local model should return the requested title suggestion");
        assert!(!title.is_empty() && title.len() <= 240 && !title.chars().any(char::is_control));
        assert_eq!(project, before);
        eprintln!("Phase 46 live local planner suggestion: {suggestion:?}");
    }

    #[test]
    fn canvas_resize_plan_is_revision_bound_validated_and_does_not_reframe_items() {
        let project = Project::new("Canvas ratio plan".to_string());
        let before = project.clone();
        let plan = build_canvas_resize_plan(&project, "Convert to portrait", 1080, 1920).unwrap();
        assert_eq!(plan.base_revision, project.revision);
        assert!(plan.referenced_media_ids.is_empty());
        assert!(plan
            .warnings
            .iter()
            .any(|warning| warning.contains("does not reposition")));
        assert!(validate_edit_plan(&project, &plan).valid);
        assert_eq!(project, before);
        assert!(build_canvas_resize_plan(&project, "Odd", 1081, 1920).is_err());
        assert!(build_canvas_resize_plan(&project, "Too large", 8194, 8192).is_err());
        assert!(build_canvas_resize_plan(
            &project,
            "Same",
            project.settings.width,
            project.settings.height
        )
        .is_err());
    }

    #[test]
    fn clip_volume_plan_is_revision_bound_and_rejects_invalid_or_locked_edits() {
        let mut project = Project::new("Volume plan".to_string());
        let mut track = crate::modules::domain::Track::new(
            "Music".to_string(),
            crate::modules::domain::TrackKind::Audio,
        );
        let frame_rate = crate::modules::domain::time::FrameRate::FPS_30;
        let item = crate::modules::domain::TimelineItem::new(
            None,
            "Music bed".to_string(),
            ItemKind::Audio,
            TimeRange::new(
                RationalTime::new(0, frame_rate),
                RationalTime::new(90, frame_rate),
            ),
            TimeRange::new(
                RationalTime::new(0, frame_rate),
                RationalTime::new(90, frame_rate),
            ),
        )
        .unwrap();
        let item_id = item.item_id.clone();
        track.items.push(item);
        project.tracks.push(track);
        let before = project.clone();

        let plan = build_clip_volume_plan(&project, "Lower music", &item_id, 0.65).unwrap();
        assert_eq!(plan.base_revision, project.revision);
        assert!(plan.referenced_media_ids.is_empty());
        assert_eq!(plan.operations.len(), 1);
        assert!(matches!(
            &plan.operations[0].operation,
            CommandOperation::ClipVolumeSet {
                item_id: target,
                volume,
            } if target == &item_id && (*volume - 0.65).abs() < f32::EPSILON
        ));
        assert!(validate_edit_plan(&project, &plan).valid);
        assert_eq!(project, before);
        assert!(build_clip_volume_plan(&project, "Invalid", &item_id, -0.1).is_err());
        assert!(build_clip_volume_plan(&project, "Invalid", &item_id, 2.1).is_err());
        assert!(build_clip_volume_plan(&project, "No-op", &item_id, 1.0).is_err());

        project.tracks[0].locked = true;
        assert!(build_clip_volume_plan(&project, "Locked", &item_id, 0.5)
            .unwrap_err()
            .contains("locked track"));
    }

    #[test]
    fn deterministic_video_draft_includes_selected_audio_and_subtitles() {
        let dir = std::env::temp_dir().join(format!(
            "tinixcut_draft_supporting_media_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let mut project = Project::new("Draft with supporting media".to_string());
        for index in 0..2 {
            let path = dir.join(format!("source-{index}.mp4"));
            std::fs::write(&path, b"video fixture").unwrap();
            project.assets.push(crate::modules::domain::Asset {
                asset_id: format!("video-{index}"),
                name: format!("source-{index}.mp4"),
                path: path.to_string_lossy().to_string(),
                media_kind: MediaKind::Video,
                duration: Some(RationalTime::new(300, FrameRate::FPS_30)),
                file_size: 13,
                dimensions: None,
            });
        }
        let audio_path = dir.join("music.wav");
        std::fs::write(&audio_path, b"audio fixture").unwrap();
        project.assets.push(crate::modules::domain::Asset {
            asset_id: "audio-bed".to_string(),
            name: "music.wav".to_string(),
            path: audio_path.to_string_lossy().to_string(),
            media_kind: MediaKind::Audio,
            duration: Some(RationalTime::new(900, FrameRate::FPS_30)),
            file_size: 13,
            dimensions: None,
        });
        let captions = vec![CaptionCue {
            start: RationalTime::new(30, FrameRate::FPS_30),
            duration: RationalTime::new(45, FrameRate::FPS_30),
            text: "A reviewed caption".to_string(),
        }];
        let plan = build_deterministic_video_draft_with_supporting_media(
            &project,
            "Create a vertical draft with subtitles and music",
            &["video-0".to_string(), "video-1".to_string()],
            600,
            Some("audio-bed"),
            &captions,
        )
        .unwrap();
        assert!(plan.referenced_media_ids.contains(&"audio-bed".to_string()));
        assert!(plan.operations.iter().any(|operation| {
            matches!(operation.operation, CommandOperation::ClipTextSet { .. })
        }));
        assert!(plan.operations.iter().any(|operation| {
            matches!(
                operation.operation,
                CommandOperation::ClipCreate {
                    kind: ItemKind::Audio,
                    ..
                }
            )
        }));
        let validation = validate_edit_plan(&project, &plan);
        assert!(validation.valid, "{validation:?}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_clip_create_media_that_is_not_declared_in_plan() {
        let mut project = Project::new("Plan test".to_string());
        let dir =
            std::env::temp_dir().join(format!("tinixcut_plan_media_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("source.mp4");
        std::fs::write(&path, b"test media marker").unwrap();
        project.assets.push(crate::modules::domain::Asset {
            asset_id: "video-asset".to_string(),
            name: "source.mp4".to_string(),
            path: path.to_string_lossy().to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(300, FrameRate::FPS_30)),
            file_size: 17,
            dimensions: None,
        });
        let mut plan = valid_plan(&project);
        plan.operations.push(OperationEnvelope {
            operation_id: "op-clip".to_string(),
            operation: CommandOperation::ClipCreate {
                track_id: "video-1".to_string(),
                item_id: Some("clip-1".to_string()),
                name: "source.mp4".to_string(),
                kind: crate::modules::domain::ItemKind::Video,
                asset_id: Some("video-asset".to_string()),
                timeline_start: RationalTime::new(0, FrameRate::FPS_30),
                duration: RationalTime::new(30, FrameRate::FPS_30),
                source_start: Some(RationalTime::new(0, FrameRate::FPS_30)),
            },
        });
        let result = validate_edit_plan(&project, &plan);
        assert!(!result.valid);
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.contains("must be listed in referencedMediaIds")));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn local_planner_output_must_be_exact_permutation_of_selected_media() {
        let selected = vec!["a".to_string(), "b".to_string()];
        let valid = parse_local_media_order(
            r#"{"orderedMediaIds":["b","a"],"rationale":"Open with the second scene."}"#,
            &selected,
        )
        .unwrap();
        assert_eq!(valid.ordered_media_ids, vec!["b", "a"]);
        assert!(parse_local_media_order(
            r#"{"orderedMediaIds":["b","b"],"rationale":"duplicate"}"#,
            &selected,
        )
        .is_err());
        assert!(parse_local_media_order(
            r#"{"orderedMediaIds":["b","unknown"],"rationale":"unknown"}"#,
            &selected,
        )
        .is_err());
        assert!(parse_local_media_order(r#"{"operations":[]}"#, &selected).is_err());
    }

    #[test]
    fn local_clip_order_output_must_be_exact_permutation_with_rationale() {
        let selected = vec!["clip-a".to_string(), "clip-b".to_string()];
        let valid = parse_local_clip_order(
            r#"{"orderedItemIds":["clip-b","clip-a"],"rationale":"Build toward sunrise."}"#,
            &selected,
        )
        .unwrap();
        assert_eq!(valid.ordered_item_ids, vec!["clip-b", "clip-a"]);
        assert!(parse_local_clip_order(
            r#"{"orderedItemIds":["clip-b","clip-b"],"rationale":"duplicate"}"#,
            &selected,
        )
        .is_err());
        assert!(parse_local_clip_order(
            r#"{"orderedItemIds":["clip-b","unknown"],"rationale":"unknown"}"#,
            &selected,
        )
        .is_err());
        assert!(parse_local_clip_order(
            r#"{"orderedItemIds":["clip-b","clip-a"],"rationale":" "}"#,
            &selected,
        )
        .is_err());
    }

    fn reorderable_project_fixture() -> (Project, std::path::PathBuf, Vec<String>) {
        use crate::modules::domain::time::TimeRange;
        use crate::modules::domain::{Asset, Track};

        let dir = std::env::temp_dir().join(format!(
            "tinixcut_clip_reorder_test_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let mut project = Project::new("Clip reorder test".to_string());
        let mut track = Track::new("Video".to_string(), TrackKind::Video);
        track.track_id = "video-track".to_string();
        let fps = FrameRate::FPS_30;
        let mut item_ids = Vec::new();
        for (index, name, start) in [(0, "Sunrise", 0), (1, "Night market", 60)] {
            let path = dir.join(format!("source-{index}.mp4"));
            std::fs::write(&path, b"private local fixture bytes").unwrap();
            let asset_id = format!("video-asset-{index}");
            project.assets.push(Asset {
                asset_id: asset_id.clone(),
                name: format!("{name}.mp4"),
                path: path.to_string_lossy().to_string(),
                media_kind: MediaKind::Video,
                duration: Some(RationalTime::new(300, fps)),
                file_size: 26,
                dimensions: None,
            });
            let item = crate::modules::domain::TimelineItem::new(
                Some(asset_id),
                format!("{name}.mp4"),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(start, fps), RationalTime::new(60, fps)),
                TimeRange::new(RationalTime::zero(fps), RationalTime::new(60, fps)),
            )
            .unwrap();
            item_ids.push(item.item_id.clone());
            track.items.push(item);
        }
        project.tracks.push(track);
        project.validate().expect("Reorder fixture must be valid");
        (project, dir, item_ids)
    }

    #[tokio::test]
    async fn local_ai_reorder_uses_only_selected_metadata_and_returns_valid_clip_moves() {
        use std::io::{BufRead, BufReader, Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let (project, dir, item_ids) = reorderable_project_fixture();
        let before = project.clone();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let allowed = item_ids.clone();
        let private_path = dir.to_string_lossy().to_string();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            let mut content_length = 0_usize;
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                if header == "\r\n" || header.is_empty() {
                    break;
                }
                if let Some((name, value)) = header.split_once(':') {
                    if name.eq_ignore_ascii_case("content-length") {
                        content_length = value.trim().parse().unwrap();
                    }
                }
            }
            let mut body = vec![0; content_length];
            reader.read_exact(&mut body).unwrap();
            let request: serde_json::Value = serde_json::from_slice(&body).unwrap();
            let request_text = request.to_string();
            assert!(request_text.contains("Sunrise.mp4"));
            assert!(request_text.contains("Night market.mp4"));
            assert!(!request_text.contains(&private_path));
            assert!(allowed.iter().all(|item_id| request_text.contains(item_id)));
            let content = serde_json::json!({
                "orderedItemIds": [allowed[1], allowed[0]],
                "rationale": "Open at the night market, then reveal sunrise."
            })
            .to_string();
            let response = serde_json::json!({
                "choices": [{ "message": { "content": content } }]
            })
            .to_string();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response.len(),
                response
            )
            .unwrap();
        });

        let receipt = build_local_ai_clip_reorder_plan_at(
            &project,
            "Open with the night market, then show sunrise",
            "video-track",
            &item_ids,
            "mock-local-model",
            &format!("http://{address}/v1/chat/completions"),
            std::time::Duration::from_secs(3),
        )
        .await
        .unwrap();
        server.join().unwrap();
        assert!(receipt.validation.valid, "{:?}", receipt.validation);
        assert_eq!(receipt.plan.operations.len(), 4);
        assert!(receipt
            .plan
            .operations
            .iter()
            .all(|operation| matches!(operation.operation, CommandOperation::ClipMove { .. })));
        assert_eq!(project, before);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[tokio::test]
    async fn local_planner_cancellation_drops_pending_request() {
        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
        let request = async { std::future::pending::<Result<(), String>>().await };
        let task = tokio::spawn(wait_for_local_planner_request(request, cancel_rx));
        tokio::task::yield_now().await;
        cancel_tx.send(()).unwrap();
        assert_eq!(
            task.await.unwrap().unwrap_err(),
            "Local planner request was cancelled"
        );
    }

    #[tokio::test]
    async fn local_planner_http_timeout_returns_a_clear_error() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 8192];
            let _ = stream.read(&mut request);
            thread::sleep(std::time::Duration::from_millis(250));
            let body = r#"{"choices":[{"message":{"content":"{}"}}]}"#;
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
        });
        let dir = std::env::temp_dir().join(format!(
            "tinixcut_local_ai_timeout_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let media_path = dir.join("source.mp4");
        std::fs::write(&media_path, b"disposable planner timeout fixture").unwrap();
        let mut project = Project::new("Local AI timeout test".to_string());
        project.assets.push(crate::modules::domain::Asset {
            asset_id: "video-timeout".to_string(),
            name: "source.mp4".to_string(),
            path: media_path.to_string_lossy().to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(900, FrameRate::FPS_30)),
            file_size: 34,
            dimensions: None,
        });
        let result = build_local_ai_video_draft_at(
            &project,
            "Create a short draft",
            &["video-timeout".to_string()],
            60,
            None,
            &[],
            "test-model",
            &format!("http://{address}/v1/chat/completions"),
            std::time::Duration::from_millis(30),
        )
        .await;
        assert!(result.unwrap_err().contains("timed out"));
        server.join().unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[tokio::test]
    #[ignore = "requires an explicitly started local LM Studio server and loaded model"]
    async fn local_lm_studio_draft_is_reviewable_and_does_not_mutate_project() {
        let dir =
            std::env::temp_dir().join(format!("tinixcut_local_ai_draft_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut project = Project::new("Local AI planner test".to_string());
        for (id, name) in [("video-a", "sunrise.mp4"), ("video-b", "night_market.mp4")] {
            let path = dir.join(name);
            std::fs::write(&path, b"disposable local planner metadata fixture").unwrap();
            project.assets.push(crate::modules::domain::Asset {
                asset_id: id.to_string(),
                name: name.to_string(),
                path: path.to_string_lossy().to_string(),
                media_kind: MediaKind::Video,
                duration: Some(RationalTime::new(900, FrameRate::FPS_30)),
                file_size: 44,
                dimensions: None,
            });
        }
        let before = project.clone();
        let ids = vec!["video-a".to_string(), "video-b".to_string()];
        let receipt = build_local_ai_video_draft(
            &project,
            "Open with the sunrise, then build toward the night market",
            &ids,
            900,
            None,
            &[],
            "google/gemma-4-e4b",
        )
        .await
        .unwrap();
        assert!(receipt.provider.starts_with(LOCAL_LM_STUDIO_PROVIDER));
        assert!(receipt.validation.valid, "{:?}", receipt.validation.issues);
        assert_eq!(receipt.plan.base_revision, project.revision);
        assert!(receipt
            .plan
            .assumptions
            .iter()
            .any(|item| item.contains("Local model selected")));
        assert_eq!(project, before);
        let selected = receipt
            .plan
            .referenced_media_ids
            .iter()
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(selected, ids.iter().collect());
        println!(
            "LOCAL_AI_PROVIDER={} RATIONALE={}",
            receipt.provider,
            receipt.plan.assumptions.last().unwrap()
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
