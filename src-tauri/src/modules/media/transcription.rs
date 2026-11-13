//! Transcription / Speech-to-Text Integration Boundary
//! Architecture Invariant: Provider output never mutates the Project directly.
//! All transcripts convert to typed domain commands through the Command Service.

use crate::modules::command::{CommandOperation, OperationEnvelope};
use crate::modules::domain::id::generate_item_id;
use crate::modules::domain::time::{FrameRate, RationalTime};
use crate::modules::domain::{
    FontReference, ItemKind, TextLayout, TextRichStyle, TextWrapMode, TimelineItem,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionSegment {
    pub start: RationalTime,
    pub duration: RationalTime,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionResult {
    pub language: String,
    pub segments: Vec<TranscriptionSegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSourceEntry {
    pub source_start: RationalTime,
    pub source_duration: RationalTime,
    pub timeline_start: Option<RationalTime>,
    pub text: String,
}

pub struct TranscriptionBoundary;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptAnalysisReport {
    pub schema_version: u32,
    pub algorithm_version: String,
    pub source_fingerprint: String,
    pub config_fingerprint: String,
    pub analyzed_duration_micros: u64,
    pub result: TranscriptionResult,
}

pub const TRANSCRIPT_ANALYSIS_SCHEMA_VERSION: u32 = 1;
pub const TRANSCRIPT_ANALYSIS_ALGORITHM_VERSION: &str = "whisper-cli-json-v1";

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
    /// Keeps provider source timing alongside the mapped timeline position.
    /// A segment outside the selected clip's source range remains visible, but
    /// has no timeline target and therefore cannot seek into an unrelated range.
    pub fn source_entries(
        result: &TranscriptionResult,
        target: &TimelineItem,
        project_rate: FrameRate,
    ) -> Vec<TranscriptSourceEntry> {
        let mut last_end = target.timeline_range.start.rescaled_to(project_rate).value;
        result
            .segments
            .iter()
            .map(|segment| {
                let timeline_start =
                    map_segment_to_timeline(segment, target, project_rate, &mut last_end)
                        .map(|mapped| mapped.start);
                TranscriptSourceEntry {
                    source_start: segment.start,
                    source_duration: segment.duration,
                    timeline_start,
                    text: segment.text.clone(),
                }
            })
            .collect()
    }

    pub fn analyze_media_read_only(
        media_path: &Path,
        whisper_cli_path: &Path,
        model_path: &Path,
        language: Option<&str>,
        cancel_flag: &AtomicBool,
    ) -> Result<(TranscriptionResult, u64), String> {
        if !media_path.is_file() {
            return Err(format!(
                "Source media is unavailable: {}",
                media_path.display()
            ));
        }
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
        let metadata = crate::modules::media::ffmpeg::FFmpegService::probe_file(media_path)
            .map_err(|error| format!("Could not inspect transcript source: {error}"))?;
        if metadata.duration_seconds <= 0.0 || metadata.duration_seconds > 1_800.0 {
            return Err(
                "Transcript analysis requires media longer than 0 and no longer than 1800 seconds"
                    .to_string(),
            );
        }
        if cancel_flag.load(Ordering::SeqCst) {
            return Err("Transcript analysis was cancelled".to_string());
        }

        let work = std::env::temp_dir().join(format!(
            "tinixcut-transcript-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&work).map_err(|error| error.to_string())?;
        let wav = work.join("audio.wav");
        let json = work.join("transcript.json");
        let output_base = work.join("transcript");
        let result = (|| {
            let mut ffmpeg =
                crate::modules::media::resolver::MediaBinaryResolver::command("ffmpeg")
                    .map_err(|error| error.to_string())?;
            ffmpeg
                .args(["-v", "error", "-i"])
                .arg(media_path)
                .args(["-vn", "-ar", "16000", "-ac", "1", "-c:a", "pcm_s16le", "-y"])
                .arg(&wav);
            run_cancellable(ffmpeg, cancel_flag, "Audio extraction")?;

            let mut whisper = Command::new(whisper_cli_path);
            whisper
                .arg("-m")
                .arg(model_path)
                .arg("-f")
                .arg(&wav)
                .arg("-oj")
                .arg("-of")
                .arg(&output_base);
            if let Some(language) =
                language.filter(|value| !value.trim().is_empty() && *value != "auto")
            {
                whisper.arg("-l").arg(language);
            }
            run_cancellable(whisper, cancel_flag, "Whisper transcription")?;
            let bytes = std::fs::read(&json)
                .map_err(|error| format!("Whisper did not produce JSON output: {error}"))?;
            let parsed = Self::parse_whisper_json(&bytes, language.unwrap_or("auto").to_string())?;
            Ok((
                parsed,
                (metadata.duration_seconds * 1_000_000.0).round() as u64,
            ))
        })();
        let _ = std::fs::remove_dir_all(work);
        result
    }

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
                    item_id: item_id.clone(),
                    text: seg.text.clone(),
                },
            });
            ops.push(OperationEnvelope {
                operation_id: format!("op_text_rich_{}", uuid::Uuid::new_v4().simple()),
                operation: CommandOperation::ClipTextRichSet {
                    item_id,
                    rich: auto_caption_style(),
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
        let mut mapped = Vec::new();
        let mut last_end = target.timeline_range.start.rescaled_to(project_rate).value;
        for segment in &result.segments {
            if let Some(mapped_segment) =
                map_segment_to_timeline(segment, target, project_rate, &mut last_end)
            {
                mapped.push(mapped_segment);
            }
        }
        TranscriptionResult {
            language: result.language.clone(),
            segments: mapped,
        }
    }
}

fn map_segment_to_timeline(
    segment: &TranscriptionSegment,
    target: &TimelineItem,
    project_rate: FrameRate,
    last_end: &mut i64,
) -> Option<TranscriptionSegment> {
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
    let (speed_num, speed_den) = target.speed.ratio();
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
        return None;
    }
    let relative_start = segment_start - source_start;
    let relative_end = segment_end - source_start;
    let start_offset_frames =
        ((relative_start as i128 * speed_den as i128 * project_rate.numerator as i128)
            / (speed_num as i128 * 1_000_i128 * project_rate.denominator as i128)) as i64;
    let end_offset_frames =
        (((relative_end as i128 * speed_den as i128 * project_rate.numerator as i128)
            + (speed_num as i128 * 1_000_i128 * project_rate.denominator as i128 - 1))
            / (speed_num as i128 * 1_000_i128 * project_rate.denominator as i128)) as i64;
    let start = (timeline_start + start_offset_frames).max(*last_end);
    let end = (timeline_start + end_offset_frames).min(timeline_end);
    if end <= start {
        return None;
    }
    *last_end = end;
    Some(TranscriptionSegment {
        start: RationalTime::new(start, project_rate),
        duration: RationalTime::new(end - start, project_rate),
        text: segment.text.clone(),
    })
}

fn run_cancellable(mut command: Command, cancel: &AtomicBool, label: &str) -> Result<(), String> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("{label} could not start: {error}"))?;
    let deadline = Instant::now() + Duration::from_secs(900);
    loop {
        if cancel.load(Ordering::SeqCst) || Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(if cancel.load(Ordering::SeqCst) {
                "Transcript analysis was cancelled".into()
            } else {
                format!("{label} exceeded the 900-second limit")
            });
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("{label} could not be monitored: {error}"))?
        {
            return if status.success() {
                Ok(())
            } else {
                Err(format!(
                    "{label} failed with exit code {}",
                    status.code().unwrap_or(-1)
                ))
            };
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn auto_caption_style() -> TextRichStyle {
    TextRichStyle {
        font: Some(FontReference {
            family: "Roboto".to_string(),
            weight: 700,
            ..FontReference::default()
        }),
        font_size: Some(48.0),
        layout: TextLayout {
            max_width: Some(1_600.0),
            wrap_mode: TextWrapMode::Wrap,
            ..TextLayout::default()
        },
        ..TextRichStyle::default()
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

    #[test]
    fn transcript_source_entries_preserve_source_times_and_only_map_visible_ranges() {
        let result = TranscriptionBoundary::parse_whisper_json(
            br#"{"transcription":[{"offsets":{"from":500,"to":1500},"text":"visible"},{"offsets":{"from":3100,"to":3200},"text":"outside"}]}"#,
            "en".to_string(),
        ).unwrap();
        let target = TimelineItem::new(
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

        let entries = TranscriptionBoundary::source_entries(&result, &target, FrameRate::FPS_30);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].source_start.value, 500);
        assert_eq!(entries[0].timeline_start.unwrap().value, 300);
        assert_eq!(entries[0].text, "visible");
        assert_eq!(entries[1].source_start.value, 3_100);
        assert!(entries[1].timeline_start.is_none());
        assert_eq!(entries[1].text, "outside");
    }

    #[test]
    fn generated_caption_commands_enable_shared_wrapping_renderer() {
        let result = TranscriptionResult {
            language: "en".to_string(),
            segments: vec![TranscriptionSegment {
                start: RationalTime::from_frames(0, FrameRate::FPS_30),
                duration: RationalTime::from_frames(90, FrameRate::FPS_30),
                text: "And so my fellow Americans ask not what your country can do for you, ask what you can do for your country.".to_string(),
            }],
        };

        let operations = TranscriptionBoundary::create_caption_commands("trk_captions", &result);
        assert_eq!(operations.len(), 3);
        let CommandOperation::ClipTextRichSet { rich, .. } = &operations[2].operation else {
            panic!("generated captions should receive a shared rich-text style");
        };
        assert_eq!(rich.layout.wrap_mode, TextWrapMode::Wrap);
        assert_eq!(rich.font_size, Some(48.0));
        rich.validate(&result.segments[0].text)
            .expect("generated caption style should be valid");

        let renderer = crate::modules::media::text_overlay::RichTextRenderer::new(
            1920,
            1080,
            crate::modules::media::text_overlay::ResolvedFont::bundled(),
        );
        let style = crate::modules::media::text_overlay::OverlayStyle::from_rich(&rich, Some(48.0))
            .expect("overlay style");
        let overlay = renderer
            .render_overlay(
                &result.segments[0].text,
                &style,
                &crate::modules::media::text_overlay::OverlayPlacement::default(),
            )
            .expect("caption should render");
        let width = overlay.roi[2] - overlay.roi[0];
        let height = overlay.roi[3] - overlay.roi[1];
        assert!(
            width <= 1760,
            "caption width {width} exceeded its safe width"
        );
        assert!(
            height >= 75,
            "caption height {height} did not contain wrapped lines"
        );
    }
}
