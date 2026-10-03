//! FFmpeg and FFprobe Native Service Abstraction
//! Architecture Invariant: Typed arguments only; no raw untrusted string execution.
//! Isolates media inspection and derived asset generation behind safe Rust interfaces.

use crate::modules::domain::time::FrameRate;
use crate::modules::media::MediaError;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProbeStream {
    pub codec_type: String, // "video" or "audio"
    pub codec_name: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_seconds: Option<f64>,
    pub frame_rate: Option<FrameRate>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MediaMetadata {
    pub duration_seconds: f64,
    pub width: u32,
    pub height: u32,
    pub frame_rate: FrameRate,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub streams: Vec<ProbeStream>,
}

#[derive(Deserialize)]
struct FFprobeOutput {
    streams: Option<Vec<FFprobeStream>>,
    format: Option<FFprobeFormat>,
}

#[derive(Deserialize)]
struct FFprobeFormat {
    duration: Option<String>,
}

#[derive(Deserialize)]
struct FFprobeStream {
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    duration: Option<String>,
    r_frame_rate: Option<String>,
    sample_rate: Option<String>,
    channels: Option<u16>,
}

pub struct FFmpegService;

impl FFmpegService {
    /// Probe a media file using ffprobe with typed arguments.
    pub fn probe_file(file_path: &Path) -> Result<MediaMetadata, MediaError> {
        if !file_path.exists() {
            return Err(MediaError::TargetNotFound(file_path.to_path_buf()));
        }

        let output = Command::new("ffprobe")
            .arg("-v")
            .arg("quiet")
            .arg("-print_format")
            .arg("json")
            .arg("-show_format")
            .arg("-show_streams")
            .arg(file_path)
            .output()
            .map_err(|e| {
                MediaError::VerificationFailed(format!("Failed to spawn ffprobe: {}", e))
            })?;

        if !output.status.success() {
            let err_msg = String::from_utf8_lossy(&output.stderr);
            return Err(MediaError::VerificationFailed(format!(
                "ffprobe execution failed: {}",
                err_msg
            )));
        }

        let parsed: FFprobeOutput = serde_json::from_slice(&output.stdout).map_err(|e| {
            MediaError::VerificationFailed(format!("Failed to parse ffprobe json output: {}", e))
        })?;

        let mut streams = Vec::new();
        let mut width = 1920;
        let mut height = 1080;
        let mut frame_rate = FrameRate::FPS_30;
        let mut video_codec = None;
        let mut audio_codec = None;
        let mut sample_rate = None;
        let mut channels = None;
        let mut duration_seconds = 0.0;

        if let Some(fmt) = parsed.format {
            if let Some(d) = fmt.duration {
                duration_seconds = d.parse::<f64>().unwrap_or(0.0);
            }
        }

        if let Some(ff_streams) = parsed.streams {
            for st in ff_streams {
                let codec_type = st.codec_type.unwrap_or_default();
                let codec_name = st.codec_name.unwrap_or_default();

                if codec_type == "video" {
                    if let Some(w) = st.width {
                        width = w;
                    }
                    if let Some(h) = st.height {
                        height = h;
                    }
                    video_codec = Some(codec_name.clone());

                    if let Some(ref fps_str) = st.r_frame_rate {
                        if let Some((num, den)) = parse_fraction(fps_str) {
                            if den > 0 {
                                frame_rate = FrameRate::new(num, den);
                            }
                        }
                    }
                } else if codec_type == "audio" {
                    audio_codec = Some(codec_name.clone());
                    if let Some(ref sr) = st.sample_rate {
                        sample_rate = sr.parse::<u32>().ok();
                    }
                    channels = st.channels;
                }

                streams.push(ProbeStream {
                    codec_type,
                    codec_name,
                    width: st.width,
                    height: st.height,
                    duration_seconds: st.duration.and_then(|d| d.parse::<f64>().ok()),
                    frame_rate: Some(frame_rate),
                    sample_rate,
                    channels,
                });
            }
        }

        Ok(MediaMetadata {
            duration_seconds,
            width,
            height,
            frame_rate,
            video_codec,
            audio_codec,
            sample_rate,
            channels,
            streams,
        })
    }

    /// Extract a single frame thumbnail at the given timestamp in seconds.
    pub fn extract_thumbnail(
        source_path: &Path,
        time_seconds: f64,
        out_path: &Path,
        width: u32,
    ) -> Result<(), MediaError> {
        if let Some(parent) = out_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let time_str = format!("{:.3}", time_seconds);
        let scale_filter = format!("scale={}:-1", width);

        let output = Command::new("ffmpeg")
            .arg("-ss")
            .arg(&time_str)
            .arg("-i")
            .arg(source_path)
            .arg("-vframes")
            .arg("1")
            .arg("-vf")
            .arg(&scale_filter)
            .arg("-y")
            .arg(out_path)
            .output()
            .map_err(|e| {
                MediaError::VerificationFailed(format!("Failed to spawn ffmpeg: {}", e))
            })?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(MediaError::VerificationFailed(format!(
                "Thumbnail extraction failed: {}",
                err
            )));
        }

        Ok(())
    }

    /// Generate a downscaled proxy video for responsive playback.
    pub fn generate_proxy(
        source_path: &Path,
        out_path: &Path,
        target_height: u32,
    ) -> Result<(), MediaError> {
        if let Some(parent) = out_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let scale_filter = format!("scale=-2:{}", target_height);

        let output = Command::new("ffmpeg")
            .arg("-i")
            .arg(source_path)
            .arg("-vf")
            .arg(&scale_filter)
            .arg("-c:v")
            .arg("libx264")
            .arg("-preset")
            .arg("ultrafast")
            .arg("-crf")
            .arg("28")
            .arg("-c:a")
            .arg("aac")
            .arg("-y")
            .arg(out_path)
            .output()
            .map_err(|e| {
                MediaError::VerificationFailed(format!("Failed to spawn ffmpeg: {}", e))
            })?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(MediaError::VerificationFailed(format!(
                "Proxy generation failed: {}",
                err
            )));
        }

        Ok(())
    }
}

fn parse_fraction(s: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() == 2 {
        let num = parts[0].trim().parse::<u32>().ok()?;
        let den = parts[1].trim().parse::<u32>().ok()?;
        Some((num, den))
    } else {
        let val = s.trim().parse::<u32>().ok()?;
        Some((val, 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn generate_synthetic_fixture(out_path: &Path) {
        // Generate a 1-second 640x360 30fps test video with audio
        let _ = Command::new("ffmpeg")
            .arg("-f")
            .arg("lavfi")
            .arg("-i")
            .arg("testsrc=duration=1:size=640x360:rate=30")
            .arg("-f")
            .arg("lavfi")
            .arg("-i")
            .arg("sine=frequency=1000:duration=1")
            .arg("-c:v")
            .arg("libx264")
            .arg("-preset")
            .arg("ultrafast")
            .arg("-c:a")
            .arg("aac")
            .arg("-y")
            .arg(out_path)
            .output();
    }

    #[test]
    fn test_ffmpeg_probe_and_derivative_generation() {
        let temp_dir = std::env::temp_dir().join("tinix_ffmpeg_test");
        let _ = fs::create_dir_all(&temp_dir);
        let fixture_path = temp_dir.join("fixture_test.mp4");
        generate_synthetic_fixture(&fixture_path);

        if !fixture_path.exists() {
            eprintln!("FFmpeg not available to generate fixture in test; skipping");
            return;
        }

        // 1. Test probe
        let meta = FFmpegService::probe_file(&fixture_path).expect("probe must succeed");
        assert_eq!(meta.width, 640);
        assert_eq!(meta.height, 360);
        assert!(meta.duration_seconds > 0.8 && meta.duration_seconds < 1.2);
        assert_eq!(meta.video_codec.as_deref(), Some("h264"));
        assert_eq!(meta.audio_codec.as_deref(), Some("aac"));

        // 2. Test thumbnail extraction
        let thumb_path = temp_dir.join("thumb_test.jpg");
        let thumb_res = FFmpegService::extract_thumbnail(&fixture_path, 0.5, &thumb_path, 320);
        assert!(thumb_res.is_ok());
        assert!(thumb_path.exists());
        assert!(thumb_path.metadata().unwrap().len() > 0);

        // 3. Test proxy generation (downscale to 240p)
        let proxy_path = temp_dir.join("proxy_test.mp4");
        let proxy_res = FFmpegService::generate_proxy(&fixture_path, &proxy_path, 240);
        assert!(proxy_res.is_ok());
        assert!(proxy_path.exists());
        assert!(proxy_path.metadata().unwrap().len() > 0);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
