//! Loudness measurement and two-pass normalization for exported audio.
//!
//! Measurement uses FFmpeg's `ebur128` (ITU-R BS.1770 / EBU R128). Normalization is `loudnorm`
//! in linear mode driven by its own first-pass measurement, with the video stream copied so
//! picture bytes never change. The export engine re-measures the normalized file with
//! `ebur128` and reports that number, so the report is a measurement and not a prediction.

use crate::modules::media::resolver::MediaBinaryResolver;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::{Command, Stdio};

/// Below this integrated level the audio is treated as silence and is never amplified.
pub const SILENCE_FLOOR_LUFS: f64 = -60.0;
/// A render already this close to the target is left untouched (no second encode).
pub const SKIP_TOLERANCE_LU: f64 = 0.3;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LoudnessTarget {
    pub integrated_lufs: f32,
    pub true_peak_db: f32,
}

impl LoudnessTarget {
    pub const MIN_LUFS: f32 = -36.0;
    pub const MAX_LUFS: f32 = -5.0;
    pub const MIN_PEAK_DB: f32 = -9.0;
    pub const MAX_PEAK_DB: f32 = 0.0;

    pub fn validate(&self) -> Result<(), String> {
        if !self.integrated_lufs.is_finite()
            || !(Self::MIN_LUFS..=Self::MAX_LUFS).contains(&self.integrated_lufs)
        {
            return Err(format!(
                "Loudness target must be within {}..={} LUFS",
                Self::MIN_LUFS,
                Self::MAX_LUFS
            ));
        }
        if !self.true_peak_db.is_finite()
            || !(Self::MIN_PEAK_DB..=Self::MAX_PEAK_DB).contains(&self.true_peak_db)
        {
            return Err(format!(
                "True-peak limit must be within {}..={} dBTP",
                Self::MIN_PEAK_DB,
                Self::MAX_PEAK_DB
            ));
        }
        Ok(())
    }
}

/// Integrated loudness, loudness range and true peak of an audio stream.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Measured {
    pub integrated_lufs: f64,
    pub true_peak_db: f64,
    pub loudness_range_lu: f64,
}

/// What the export engine records about the loudness step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoudnessOutcome {
    pub target_lufs: f32,
    pub true_peak_limit_db: f32,
    pub input_lufs: Option<f64>,
    pub output_lufs: Option<f64>,
    pub output_true_peak_db: Option<f64>,
    pub normalized: bool,
    pub note: Option<String>,
}

fn ffmpeg() -> Result<Command, String> {
    MediaBinaryResolver::command("ffmpeg").map_err(|e| format!("Failed to resolve FFmpeg: {e}"))
}

fn parse_number(text: &str) -> Option<f64> {
    let token = text.split_whitespace().next()?;
    if token.eq_ignore_ascii_case("-inf") {
        return Some(f64::NEG_INFINITY);
    }
    token.parse().ok()
}

/// Reads the final `Summary:` block of an `ebur128` run.
pub fn parse_ebur128_summary(log: &str) -> Option<Measured> {
    let summary = &log[log.rfind("Summary:")?..];
    let value = |prefix: &str| {
        summary
            .lines()
            .map(str::trim)
            .find_map(|line| line.strip_prefix(prefix))
            .and_then(parse_number)
    };
    Some(Measured {
        integrated_lufs: value("I:")?,
        true_peak_db: value("Peak:")?,
        loudness_range_lu: value("LRA:")?,
    })
}

/// Measures the first audio stream. `Ok(None)` means the file has no audio stream.
pub fn measure(path: &Path) -> Result<Option<Measured>, String> {
    let output = ffmpeg()?
        .args(["-nostdin", "-nostats", "-hide_banner", "-i"])
        .arg(path)
        .args([
            "-map",
            "0:a:0",
            "-af",
            "ebur128=peak=true",
            "-f",
            "null",
            "-",
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .output()
        .map_err(|e| format!("Failed to run FFmpeg: {e}"))?;
    let log = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        if log.contains("Stream map '0:a:0' matches no streams")
            || log.contains("matches no streams")
        {
            return Ok(None);
        }
        return Err(format!("Loudness measurement failed: {}", log.trim()));
    }
    parse_ebur128_summary(&log)
        .map(Some)
        .ok_or_else(|| "Loudness measurement produced no summary".to_string())
}

#[derive(Debug, Deserialize)]
struct LoudnormFirstPass {
    input_i: String,
    input_tp: String,
    input_lra: String,
    input_thresh: String,
    target_offset: String,
}

fn parse_loudnorm_json(log: &str) -> Result<LoudnormFirstPass, String> {
    let start = log
        .rfind('{')
        .ok_or_else(|| "loudnorm printed no measurement".to_string())?;
    let end = log[start..]
        .find('}')
        .ok_or_else(|| "loudnorm measurement is truncated".to_string())?;
    serde_json::from_str(&log[start..=start + end])
        .map_err(|e| format!("loudnorm measurement is not valid JSON: {e}"))
}

/// Re-encodes only the audio of `input` into `output` so its integrated loudness hits the target.
pub fn normalize(
    input: &Path,
    output: &Path,
    target: &LoudnessTarget,
    measured_input: &Measured,
    audio_bitrate: &str,
    sample_rate: u32,
) -> Result<(), String> {
    target.validate()?;
    let filter = format!(
        "loudnorm=I={:.2}:TP={:.2}:LRA=11:print_format=json",
        target.integrated_lufs, target.true_peak_db
    );
    let first = ffmpeg()?
        .args(["-nostdin", "-nostats", "-hide_banner", "-i"])
        .arg(input)
        .args(["-map", "0:a:0", "-af", &filter, "-f", "null", "-"])
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .output()
        .map_err(|e| format!("Failed to run FFmpeg: {e}"))?;
    let log = String::from_utf8_lossy(&first.stderr).into_owned();
    if !first.status.success() {
        return Err(format!("loudnorm analysis failed: {}", log.trim()));
    }
    let pass = parse_loudnorm_json(&log)?;
    // The independent ebur128 reading and loudnorm's own must agree before a gain is applied.
    let loudnorm_input: f64 = pass
        .input_i
        .trim()
        .parse()
        .map_err(|_| "bad loudnorm input_i")?;
    if (loudnorm_input - measured_input.integrated_lufs).abs() > 1.0 {
        return Err(format!(
            "Loudness measurements disagree ({loudnorm_input:.1} vs {:.1} LUFS); refusing to normalize",
            measured_input.integrated_lufs
        ));
    }
    let second = format!(
        "loudnorm=I={:.2}:TP={:.2}:LRA=11:measured_I={}:measured_TP={}:measured_LRA={}:measured_thresh={}:offset={}:linear=true",
        target.integrated_lufs,
        target.true_peak_db,
        pass.input_i.trim(),
        pass.input_tp.trim(),
        pass.input_lra.trim(),
        pass.input_thresh.trim(),
        pass.target_offset.trim()
    );
    let status = ffmpeg()?
        .args(["-nostdin", "-hide_banner", "-v", "error", "-y", "-i"])
        .arg(input)
        .args([
            "-map", "0:v?", "-map", "0:a:0", "-c:v", "copy", "-af", &second,
        ])
        .args(["-c:a", "aac", "-b:a", audio_bitrate, "-ar"])
        .arg(sample_rate.to_string())
        .args(["-f", "mp4"])
        .arg(output)
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .output()
        .map_err(|e| format!("Failed to run FFmpeg: {e}"))?;
    if !status.status.success() {
        return Err(format!(
            "loudnorm encode failed: {}",
            String::from_utf8_lossy(&status.stderr).trim()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_an_ebur128_summary_block() {
        let log = "frame noise\n[Parsed_ebur128_0 @ 0x1] Summary:\n\n  Integrated loudness:\n    I:         -23.4 LUFS\n    Threshold: -33.4 LUFS\n\n  Loudness range:\n    LRA:         5.2 LU\n    Threshold: -43.4 LUFS\n\n  True peak:\n    Peak:       -1.5 dBFS\n";
        let measured = parse_ebur128_summary(log).unwrap();
        assert_eq!(measured.integrated_lufs, -23.4);
        assert_eq!(measured.loudness_range_lu, 5.2);
        assert_eq!(measured.true_peak_db, -1.5);
    }

    #[test]
    fn silence_reads_as_negative_infinity() {
        let log = "Summary:\n I:         -inf LUFS\n LRA:         0.0 LU\n Peak:       -inf dBFS\n";
        let measured = parse_ebur128_summary(log).unwrap();
        assert!(measured.integrated_lufs.is_infinite() && measured.integrated_lufs < 0.0);
    }

    #[test]
    fn rejects_logs_without_a_summary() {
        assert!(parse_ebur128_summary("nothing useful").is_none());
    }

    #[test]
    fn validates_target_ranges() {
        assert!(LoudnessTarget {
            integrated_lufs: -14.0,
            true_peak_db: -1.0
        }
        .validate()
        .is_ok());
        assert!(LoudnessTarget {
            integrated_lufs: -2.0,
            true_peak_db: -1.0
        }
        .validate()
        .is_err());
        assert!(LoudnessTarget {
            integrated_lufs: -14.0,
            true_peak_db: 3.0
        }
        .validate()
        .is_err());
        assert!(LoudnessTarget {
            integrated_lufs: f32::NAN,
            true_peak_db: -1.0
        }
        .validate()
        .is_err());
    }

    #[test]
    fn parses_loudnorm_first_pass_json() {
        let log = "stuff\n{\n\t\"input_i\" : \"-27.1\",\n\t\"input_tp\" : \"-9.0\",\n\t\"input_lra\" : \"3.2\",\n\t\"input_thresh\" : \"-37.2\",\n\t\"output_i\" : \"-14.1\",\n\t\"target_offset\" : \"0.1\"\n}\n";
        let pass = parse_loudnorm_json(log).unwrap();
        assert_eq!(pass.input_i, "-27.1");
        assert_eq!(pass.target_offset, "0.1");
    }
}
