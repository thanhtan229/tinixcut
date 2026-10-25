//! Parsing and serialization for manual caption files.
//!
//! Caption files are imported as canonical text timeline items. This module
//! deliberately keeps time conversion in integer arithmetic so a caption
//! import cannot introduce floating-point timeline drift.

use crate::modules::domain::time::{FrameRate, RationalTime};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const MAX_CAPTION_FILE_BYTES: usize = 10 * 1024 * 1024;
pub const MAX_CAPTION_CUES: usize = 10_000;
pub const MAX_CAPTION_TEXT_CHARS: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaptionFormat {
    Srt,
    Vtt,
}

impl CaptionFormat {
    pub fn from_path(path: &Path) -> Result<Self, String> {
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        match extension.as_str() {
            "srt" => Ok(Self::Srt),
            "vtt" => Ok(Self::Vtt),
            _ => Err(format!(
                "Unsupported caption format '.{}'; choose an .srt or .vtt file",
                extension
            )),
        }
    }

    fn timestamp_separator(self) -> char {
        match self {
            Self::Srt => ',',
            Self::Vtt => '.',
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptionCue {
    pub start: RationalTime,
    pub duration: RationalTime,
    pub text: String,
}

pub fn parse_caption_bytes(
    bytes: &[u8],
    format: CaptionFormat,
    frame_rate: FrameRate,
) -> Result<Vec<CaptionCue>, String> {
    if bytes.len() > MAX_CAPTION_FILE_BYTES {
        return Err(format!(
            "Caption file is too large ({} bytes; limit is {} bytes)",
            bytes.len(),
            MAX_CAPTION_FILE_BYTES
        ));
    }

    let source = std::str::from_utf8(bytes)
        .map_err(|_| "Caption file must be valid UTF-8 text".to_string())?
        .trim_start_matches('\u{feff}')
        .replace("\r\n", "\n")
        .replace('\r', "\n");

    let mut cues = match format {
        CaptionFormat::Srt => parse_srt(&source, frame_rate)?,
        CaptionFormat::Vtt => parse_vtt(&source, frame_rate)?,
    };

    if cues.is_empty() {
        return Err("Caption file contains no valid cues".to_string());
    }
    if cues.len() > MAX_CAPTION_CUES {
        return Err(format!(
            "Caption file contains too many cues ({}; limit is {})",
            cues.len(),
            MAX_CAPTION_CUES
        ));
    }

    cues.sort_by(|left, right| left.start.semantic_cmp(&right.start));
    for pair in cues.windows(2) {
        if pair[1]
            .start
            .semantic_lt(&(pair[0].start + pair[0].duration))
        {
            return Err(format!(
                "Caption cues overlap near {}",
                pair[1].start.to_timecode()
            ));
        }
    }
    Ok(cues)
}

fn parse_srt(source: &str, frame_rate: FrameRate) -> Result<Vec<CaptionCue>, String> {
    source
        .split("\n\n")
        .filter(|block| !block.trim().is_empty())
        .enumerate()
        .map(|(index, block)| parse_block(block, CaptionFormat::Srt, frame_rate, index + 1))
        .collect()
}

fn parse_vtt(source: &str, frame_rate: FrameRate) -> Result<Vec<CaptionCue>, String> {
    let mut lines = source.lines();
    let header = lines
        .next()
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if header != "webvtt" && !header.starts_with("webvtt ") {
        return Err("WebVTT file must start with a WEBVTT header".to_string());
    }

    let body = lines.collect::<Vec<_>>().join("\n");
    let mut cues = Vec::new();
    for (index, block) in body
        .split("\n\n")
        .filter(|block| !block.trim().is_empty())
        .enumerate()
    {
        let trimmed = block.trim();
        if trimmed.starts_with("NOTE")
            || trimmed.starts_with("STYLE")
            || trimmed.starts_with("REGION")
        {
            continue;
        }
        if !trimmed.lines().any(|line| line.contains("-->")) {
            continue;
        }
        cues.push(parse_block(
            trimmed,
            CaptionFormat::Vtt,
            frame_rate,
            index + 1,
        )?);
    }
    Ok(cues)
}

fn parse_block(
    block: &str,
    format: CaptionFormat,
    frame_rate: FrameRate,
    block_number: usize,
) -> Result<CaptionCue, String> {
    let lines = block.lines().map(str::trim_end).collect::<Vec<_>>();
    let timing_index = lines
        .iter()
        .position(|line| line.contains("-->"))
        .ok_or_else(|| format!("Caption block {} has no timing line", block_number))?;
    if format == CaptionFormat::Srt && timing_index > 1 {
        return Err(format!(
            "SRT caption block {} has an unexpected line before its timing",
            block_number
        ));
    }

    let timing = lines[timing_index];
    let (start_raw, end_and_settings) = timing
        .split_once("-->")
        .ok_or_else(|| format!("Caption block {} has an invalid timing line", block_number))?;
    let end_raw = end_and_settings
        .split_whitespace()
        .next()
        .ok_or_else(|| format!("Caption block {} has no end timestamp", block_number))?;
    let start = parse_timestamp(start_raw.trim(), format, frame_rate).map_err(|reason| {
        format!(
            "Caption block {} has invalid start timestamp: {}",
            block_number, reason
        )
    })?;
    let end = parse_timestamp(end_raw, format, frame_rate).map_err(|reason| {
        format!(
            "Caption block {} has invalid end timestamp: {}",
            block_number, reason
        )
    })?;
    if end.semantic_le(&start) {
        return Err(format!(
            "Caption block {} must have a positive duration",
            block_number
        ));
    }

    let text = lines
        .iter()
        .skip(timing_index + 1)
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if text.is_empty() {
        return Err(format!("Caption block {} has empty text", block_number));
    }
    if text.chars().count() > MAX_CAPTION_TEXT_CHARS {
        return Err(format!(
            "Caption block {} text exceeds {} characters",
            block_number, MAX_CAPTION_TEXT_CHARS
        ));
    }

    Ok(CaptionCue {
        start,
        duration: end - start,
        text,
    })
}

fn parse_timestamp(
    value: &str,
    format: CaptionFormat,
    frame_rate: FrameRate,
) -> Result<RationalTime, String> {
    let separator = format.timestamp_separator();
    let (whole, millis) = value
        .split_once(separator)
        .ok_or_else(|| "expected milliseconds".to_string())?;
    let millis = if millis.len() == 3 && millis.chars().all(|character| character.is_ascii_digit())
    {
        millis
            .parse::<u128>()
            .map_err(|_| "milliseconds are out of range".to_string())?
    } else {
        return Err("milliseconds must contain exactly three digits".to_string());
    };

    let parts = whole.split(':').collect::<Vec<_>>();
    let (hours, minutes, seconds) = match parts.as_slice() {
        [minutes, seconds] => ("0", *minutes, *seconds),
        [hours, minutes, seconds] => (*hours, *minutes, *seconds),
        _ => return Err("timestamp must be MM:SS or HH:MM:SS".to_string()),
    };
    let hours = parse_timestamp_component(hours, "hours")?;
    let minutes = parse_timestamp_component(minutes, "minutes")?;
    let seconds = parse_timestamp_component(seconds, "seconds")?;
    if minutes >= 60 || seconds >= 60 {
        return Err("minutes and seconds must be below 60".to_string());
    }

    let total_millis = (hours as u128)
        .checked_mul(3_600_000)
        .and_then(|value| value.checked_add((minutes as u128) * 60_000))
        .and_then(|value| value.checked_add((seconds as u128) * 1_000))
        .and_then(|value| value.checked_add(millis))
        .ok_or_else(|| "timestamp is out of range".to_string())?;
    let numerator = total_millis
        .checked_mul(frame_rate.numerator as u128)
        .ok_or_else(|| "timestamp frame conversion overflowed".to_string())?;
    let denominator = (1_000u128)
        .checked_mul(frame_rate.denominator as u128)
        .ok_or_else(|| "timestamp frame conversion overflowed".to_string())?;
    let frames = (numerator + denominator / 2) / denominator;
    if frames > i64::MAX as u128 {
        return Err("timestamp is too large for the project timeline".to_string());
    }
    Ok(RationalTime::from_frames(frames as i64, frame_rate))
}

fn parse_timestamp_component(value: &str, name: &str) -> Result<u64, String> {
    if value.is_empty() || !value.chars().all(|character| character.is_ascii_digit()) {
        return Err(format!("{} must be numeric", name));
    }
    value
        .parse::<u64>()
        .map_err(|_| format!("{} is out of range", name))
}

pub fn serialize_captions(cues: &[CaptionCue], format: CaptionFormat) -> Result<String, String> {
    let mut sorted = cues.to_vec();
    sorted.sort_by(|left, right| left.start.semantic_cmp(&right.start));

    let mut output = String::new();
    if format == CaptionFormat::Vtt {
        output.push_str("WEBVTT\n\n");
    }

    let mut written = 0usize;
    for cue in sorted {
        if cue.text.trim().is_empty() {
            continue;
        }
        if cue.start.value < 0 || cue.duration.value <= 0 {
            return Err("Cannot export a caption with invalid timeline range".to_string());
        }
        let end = cue.start + cue.duration;
        let start_timestamp = format_timestamp(cue.start, format)?;
        let end_timestamp = format_timestamp(end, format)?;
        if format == CaptionFormat::Srt {
            written += 1;
            output.push_str(&format!(
                "{}\n{} --> {}\n{}\n\n",
                written,
                start_timestamp,
                end_timestamp,
                cue.text.trim()
            ));
        } else {
            output.push_str(&format!(
                "{} --> {}\n{}\n\n",
                start_timestamp,
                end_timestamp,
                cue.text.trim()
            ));
            written += 1;
        }
    }
    if written == 0 {
        return Err("There are no non-empty captions to export".to_string());
    }
    Ok(output)
}

fn format_timestamp(time: RationalTime, format: CaptionFormat) -> Result<String, String> {
    let rate = time.rate;
    if time.value < 0 || rate.numerator == 0 {
        return Err("Cannot format an invalid timeline timestamp".to_string());
    }
    let numerator = (time.value as u128)
        .checked_mul(1_000)
        .and_then(|value| value.checked_mul(rate.denominator as u128))
        .ok_or_else(|| "timestamp formatting overflowed".to_string())?;
    let denominator = rate.numerator as u128;
    let total_millis = (numerator + denominator / 2) / denominator;
    let hours = total_millis / 3_600_000;
    let minutes = (total_millis / 60_000) % 60;
    let seconds = (total_millis / 1_000) % 60;
    let millis = total_millis % 1_000;
    let separator = format.timestamp_separator();
    Ok(format!(
        "{:02}:{:02}:{:02}{}{:03}",
        hours, minutes, seconds, separator, millis
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_srt_bom_crlf_and_multiline_text() {
        let input = b"\xEF\xBB\xBF1\r\n00:00:01,000 --> 00:00:02,500\r\nHello\r\nworld\r\n\r\n";
        let cues = parse_caption_bytes(input, CaptionFormat::Srt, FrameRate::FPS_30).unwrap();
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].start.value, 30);
        assert_eq!(cues[0].duration.value, 45);
        assert_eq!(cues[0].text, "Hello\nworld");
    }

    #[test]
    fn parses_vtt_header_identifier_and_settings() {
        let input = b"WEBVTT - TinixCut\n\ncaption-1\n00:01.000 --> 00:02.000 align:start\nHello\n\nNOTE\nignored\n";
        let cues = parse_caption_bytes(input, CaptionFormat::Vtt, FrameRate::FPS_30).unwrap();
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].start.value, 30);
        assert_eq!(cues[0].duration.value, 30);
    }

    #[test]
    fn rejects_malformed_empty_zero_duration_and_overlapping_cues() {
        let malformed = b"1\n00:00:xx,000 --> 00:00:01,000\nText";
        assert!(parse_caption_bytes(malformed, CaptionFormat::Srt, FrameRate::FPS_30).is_err());

        let empty = b"1\n00:00:00,000 --> 00:00:01,000\n\n";
        assert!(parse_caption_bytes(empty, CaptionFormat::Srt, FrameRate::FPS_30).is_err());

        let zero = b"1\n00:00:00,000 --> 00:00:00,000\nText";
        assert!(parse_caption_bytes(zero, CaptionFormat::Srt, FrameRate::FPS_30).is_err());

        let overlap = b"1\n00:00:00,000 --> 00:00:02,000\nA\n\n2\n00:00:01,500 --> 00:00:03,000\nB";
        assert!(parse_caption_bytes(overlap, CaptionFormat::Srt, FrameRate::FPS_30).is_err());
    }

    #[test]
    fn serializes_srt_and_vtt_using_frame_rate_rounding() {
        let cues = vec![CaptionCue {
            start: RationalTime::from_frames(30, FrameRate::FPS_29_97),
            duration: RationalTime::from_frames(30, FrameRate::FPS_29_97),
            text: "Hello".to_string(),
        }];
        let srt = serialize_captions(&cues, CaptionFormat::Srt).unwrap();
        assert!(srt.contains("00:00:01,001 --> 00:00:02,002"));
        assert!(srt.starts_with("1\n"));

        let vtt = serialize_captions(&cues, CaptionFormat::Vtt).unwrap();
        assert!(vtt.starts_with("WEBVTT\n\n"));
        assert!(vtt.contains("00:00:01.001 --> 00:00:02.002"));
    }

    #[test]
    fn accepts_case_insensitive_extensions() {
        assert_eq!(
            CaptionFormat::from_path(Path::new("captions.SRT")).unwrap(),
            CaptionFormat::Srt
        );
        assert_eq!(
            CaptionFormat::from_path(Path::new("captions.Vtt")).unwrap(),
            CaptionFormat::Vtt
        );
    }
}
