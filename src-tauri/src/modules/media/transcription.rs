//! Transcription / Speech-to-Text Integration Boundary
//! Architecture Invariant: Provider output never mutates the Project directly.
//! All transcripts convert to typed domain commands through the Command Service.

use crate::modules::command::{CommandOperation, OperationEnvelope};
use crate::modules::domain::id::generate_item_id;
use crate::modules::domain::time::{FrameRate, RationalTime};
use crate::modules::domain::{ItemKind, TimelineItem};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranscriptionSegment {
    pub start: RationalTime,
    pub duration: RationalTime,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranscriptionResult {
    pub language: String,
    pub segments: Vec<TranscriptionSegment>,
}

pub struct TranscriptionBoundary;

#[derive(Debug, Deserialize)]
struct WhisperJson {
    #[serde(default)]
    transcription: Vec<WhisperJsonSegment>,
}

#[derive(Debug, Deserialize)]
struct WhisperJsonSegment {
    offsets: WhisperOffsets,
    text: String,
}

#[derive(Debug, Deserialize)]
struct WhisperOffsets {
    from: i64,
    to: i64,
}

impl TranscriptionBoundary {
    /// Adapts raw transcription provider output into canonical CommandOperations.
    /// Enforces the invariant: External AI/STT adapters produce commands, not direct model mutations.
    pub fn create_caption_commands(
        track_id: &str,
        result: &TranscriptionResult,
    ) -> Vec<OperationEnvelope> {
        let mut ops = Vec::new();
        for seg in &result.segments {
            let item_id = generate_item_id();
            ops.push(OperationEnvelope {
                operation_id: format!("op_trans_{}", uuid::Uuid::new_v4().simple()),
                operation: CommandOperation::ClipCreate {
                    track_id: track_id.to_string(),
                    item_id: Some(item_id.clone()),
                    name: format!("Caption: {}", seg.text.chars().take(20).collect::<String>()),
                    kind: ItemKind::Text,
                    asset_id: None,
                    timeline_start: seg.start,
                    duration: seg.duration,
                    source_start: Some(RationalTime::zero(seg.start.rate)),
                },
            });
            ops.push(OperationEnvelope {
                operation_id: format!("op_text_{}", uuid::Uuid::new_v4().simple()),
                operation: CommandOperation::ClipTextSet {
                    item_id,
                    text: seg.text.clone(),
                },
            });
        }
        ops
    }

    /// Parses `whisper-cli -oj` JSON. The CLI's offsets are integer milliseconds,
    /// which keeps provider precision explicit until the editor aligns it to frames.
    pub fn parse_whisper_json(
        bytes: &[u8],
        language: String,
    ) -> Result<TranscriptionResult, String> {
        let document: WhisperJson = serde_json::from_slice(bytes)
            .map_err(|error| format!("Invalid whisper JSON: {error}"))?;
        let millisecond_rate = FrameRate::new(1_000, 1);
        let mut segments = Vec::new();
        for raw in document.transcription {
            let text = raw.text.trim().to_string();
            if raw.offsets.to <= raw.offsets.from || text.is_empty() {
                continue;
            }
            segments.push(TranscriptionSegment {
                start: RationalTime::new(raw.offsets.from, millisecond_rate),
                duration: RationalTime::new(raw.offsets.to - raw.offsets.from, millisecond_rate),
                text,
            });
        }
        Ok(TranscriptionResult { language, segments })
    }

    /// Converts source-media offsets to visible timeline text clips for one selected
    /// clip. Provider offsets are clipped to the source range and then aligned to
    /// project ticks, so generated text follows trims and fixed playback speed.
    pub fn map_to_timeline(
        result: &TranscriptionResult,
        target: &TimelineItem,
        project_rate: FrameRate,
    ) -> TranscriptionResult {
        let millisecond_rate = FrameRate::new(1_000, 1);
        let source_start = target
            .source_range
            .start
            .rescaled_to(millisecond_rate)
            .value;
        let source_end = target
            .source_range
            .end()
            .rescaled_to(millisecond_rate)
            .value;
        let timeline_start = target.timeline_range.start.rescaled_to(project_rate).value;
        let timeline_end = target.timeline_range.end().rescaled_to(project_rate).value;
        let (speed_num, speed_den) = match target.speed {
            crate::modules::domain::ClipSpeed::Half => (1_i64, 2_i64),
            crate::modules::domain::ClipSpeed::Normal => (1_i64, 1_i64),
            crate::modules::domain::ClipSpeed::Double => (2_i64, 1_i64),
        };
        let mut mapped = Vec::new();
        let mut last_end = timeline_start;

        for segment in &result.segments {
            let segment_start = segment
                .start
                .rescaled_to(millisecond_rate)
                .value
                .max(source_start);
            let segment_end = (segment.start + segment.duration)
                .rescaled_to(millisecond_rate)
                .value
                .min(source_end);
            if segment_end <= segment_start {
                continue;
            }

            let relative_start = segment_start - source_start;
            let relative_end = segment_end - source_start;
            // Round outward after speed conversion to avoid losing spoken content.
            let start_offset_frames =
                ((relative_start as i128 * speed_den as i128 * project_rate.numerator as i128)
                    / (speed_num as i128 * 1_000_i128 * project_rate.denominator as i128))
                    as i64;
            let end_offset_frames =
                (((relative_end as i128 * speed_den as i128 * project_rate.numerator as i128)
                    + (speed_num as i128 * 1_000_i128 * project_rate.denominator as i128 - 1))
                    / (speed_num as i128 * 1_000_i128 * project_rate.denominator as i128))
                    as i64;
            let start = (timeline_start + start_offset_frames).max(last_end);
            let end = (timeline_start + end_offset_frames).min(timeline_end);
            if end <= start {
                continue;
            }
            last_end = end;
            mapped.push(TranscriptionSegment {
                start: RationalTime::new(start, project_rate),
                duration: RationalTime::new(end - start, project_rate),
                text: segment.text.clone(),
            });
        }
        TranscriptionResult {
            language: result.language.clone(),
            segments: mapped,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::domain::time::TimeRange;
    use crate::modules::domain::{ClipSpeed, ItemKind, TimelineItem};

    #[test]
    fn whisper_json_is_parsed_and_mapped_to_trimmed_clip() {
        let result = TranscriptionBoundary::parse_whisper_json(
            br#"{"transcription":[{"offsets":{"from":500,"to":1500},"text":" hello "},{"offsets":{"from":3100,"to":3200},"text":"outside"}]}"#,
            "en".to_string(),
        )
        .unwrap();
        let mut target = TimelineItem::new(
            Some("ast_1".to_string()),
            "clip".to_string(),
            ItemKind::Video,
            TimeRange::new(
                RationalTime::new(300, FrameRate::FPS_30),
                RationalTime::new(60, FrameRate::FPS_30),
            ),
            TimeRange::new(
                RationalTime::new(1_000, FrameRate::new(1_000, 1)),
                RationalTime::new(2_000, FrameRate::new(1_000, 1)),
            ),
        )
        .unwrap();
        target.speed = ClipSpeed::Normal;
        let mapped = TranscriptionBoundary::map_to_timeline(&result, &target, FrameRate::FPS_30);
        assert_eq!(mapped.segments.len(), 1);
        assert_eq!(mapped.segments[0].start.value, 300);
        assert_eq!(mapped.segments[0].duration.value, 15);
        assert_eq!(mapped.segments[0].text, "hello");
    }
}
