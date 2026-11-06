//! Read-only scene boundary analysis results.

use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};

pub const SCENE_ANALYSIS_SCHEMA_VERSION: u32 = 1;
pub const SCENE_ANALYSIS_ALGORITHM_VERSION: &str = "ffmpeg-select-scene-v1";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SceneCandidate {
    /// Source-media offset in microseconds.
    pub source_time_micros: u64,
    /// FFmpeg's normalized scene-change score in the range [0, 1].
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SceneAnalysisReport {
    pub schema_version: u32,
    pub algorithm_version: String,
    pub source_fingerprint: String,
    pub config_fingerprint: String,
    pub threshold: f32,
    pub analyzed_duration_micros: u64,
    pub truncated: bool,
    pub candidates: Vec<SceneCandidate>,
}

pub fn parse_scene_metadata(text: &str, max_candidates: usize) -> Vec<SceneCandidate> {
    let mut frame_time_micros = None;
    let mut score = None;
    let mut candidates = Vec::new();

    for line in text.lines() {
        if line.trim_start().starts_with("frame:") {
            frame_time_micros = None;
            score = None;
        }
        if let Some((_, value)) = line.split_once("pts_time:") {
            frame_time_micros = value
                .split_whitespace()
                .next()
                .and_then(|value| value.parse::<f64>().ok())
                .filter(|value| value.is_finite() && *value >= 0.0)
                .map(|seconds| (seconds * 1_000_000.0).round() as u64);
        }
        if let Some((_, value)) = line.split_once("lavfi.scene_score=") {
            score = value
                .trim()
                .parse::<f32>()
                .ok()
                .filter(|value| value.is_finite())
                .map(|value| value.clamp(0.0, 1.0));
        }

        if let (Some(source_time_micros), Some(score_value)) = (frame_time_micros, score) {
            candidates.push(SceneCandidate {
                source_time_micros,
                score: score_value,
            });
            frame_time_micros = None;
            score = None;
            if candidates.len() >= max_candidates {
                break;
            }
        }
    }

    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scene_offsets_and_scores_with_candidate_limit() {
        let metadata = "frame:0 pts:15360 pts_time:1\nlavfi.scene_score=0.400000\nframe:1 pts:30720 pts_time:2.025\nlavfi.scene_score=1.2\n";
        let candidates = parse_scene_metadata(metadata, 1);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].source_time_micros, 1_000_000);
        assert_eq!(candidates[0].score, 0.4);
    }

    #[test]
    fn ignores_incomplete_and_invalid_scene_metadata() {
        let metadata = "frame:0 pts:0 pts_time:NaN\nlavfi.scene_score=0.8\nframe:1 pts:1 pts_time:1.5\nlavfi.scene_score=bad\n";
        assert!(parse_scene_metadata(metadata, 10).is_empty());
    }
}
