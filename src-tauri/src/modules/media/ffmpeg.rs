//! FFmpeg and FFprobe Native Service Abstraction
//! Architecture Invariant: Typed arguments only; no raw untrusted string execution.
//! Isolates media inspection and derived asset generation behind safe Rust interfaces.

use crate::modules::domain::time::FrameRate;
use crate::modules::media::resolver::MediaBinaryResolver;
use crate::modules::media::waveform::{WaveformData, WAVEFORM_SCHEMA_VERSION};
use crate::modules::media::MediaError;
use serde::{Deserialize, Serialize};
use std::path::Path;

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

        let output = MediaBinaryResolver::command("ffprobe")?
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

        let output = MediaBinaryResolver::command("ffmpeg")?
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

        let output = MediaBinaryResolver::command("ffmpeg")?
            .arg("-i")
            .arg(source_path)
            .arg("-map")
            .arg("0:v:0")
            .arg("-map")
            .arg("0:a?")
            .arg("-vf")
            .arg(&scale_filter)
            .arg("-c:v")
            .arg("libx264")
            .arg("-pix_fmt")
            .arg("yuv420p")
            .arg("-preset")
            .arg("ultrafast")
            .arg("-crf")
            .arg("28")
            .arg("-c:a")
            .arg("aac")
            .arg("-movflags")
            .arg("+faststart")
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

    /// Extracts compact waveform peak buckets from an audio or video file.
    /// Resamples audio to 4000 Hz mono and aggregates into `bucket_rate` buckets/sec.
    /// Invariant: Does not corrupt project, safe failure on missing/no-audio media.
    pub fn extract_waveform(
        source_path: &Path,
        asset_id: &str,
        bucket_rate: u32,
        out_json_path: &Path,
    ) -> Result<WaveformData, MediaError> {
        if !source_path.exists() {
            return Err(MediaError::TargetNotFound(source_path.to_path_buf()));
        }

        // Verify audio stream exists
        let meta = Self::probe_file(source_path)?;
        if meta.audio_codec.is_none() {
            return Err(MediaError::VerificationFailed(
                "Source media contains no audio stream".to_string(),
            ));
        }

        let duration = if meta.duration_seconds > 0.0 {
            meta.duration_seconds
        } else {
            1.0
        };

        // Extract s16le PCM mono downsampled to 4000 Hz
        let sample_rate = 4000u32;
        let output = MediaBinaryResolver::command("ffmpeg")?
            .arg("-v")
            .arg("quiet")
            .arg("-i")
            .arg(source_path)
            .arg("-vn")
            .arg("-af")
            .arg(format!(
                "aresample={},aformat=channel_layouts=mono",
                sample_rate
            ))
            .arg("-f")
            .arg("s16le")
            .arg("pipe:1")
            .output()
            .map_err(|e| {
                MediaError::VerificationFailed(format!(
                    "Failed to spawn ffmpeg for waveform: {}",
                    e
                ))
            })?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(MediaError::VerificationFailed(format!(
                "FFmpeg waveform extraction failed: {}",
                err
            )));
        }

        let raw_bytes = output.stdout;
        let (chunks, _) = raw_bytes.as_chunks::<2>();
        let mut samples = Vec::with_capacity(chunks.len());
        for chunk in chunks {
            let sample_i16 = i16::from_le_bytes([chunk[0], chunk[1]]);
            let sample_f32 = sample_i16 as f32 / 32768.0;
            samples.push(sample_f32);
        }

        let effective_rate = bucket_rate.max(1);
        let samples_per_bucket = (sample_rate / effective_rate).max(1) as usize;
        let num_buckets = samples.len().div_ceil(samples_per_bucket).max(1);

        let mut peaks = Vec::with_capacity(num_buckets * 2);
        for bucket_idx in 0..num_buckets {
            let start = bucket_idx * samples_per_bucket;
            let end = (start + samples_per_bucket).min(samples.len());
            if start < samples.len() {
                let slice = &samples[start..end];
                let mut min_val = 0.0f32;
                let mut max_val = 0.0f32;
                for &val in slice {
                    if val < min_val {
                        min_val = val;
                    }
                    if val > max_val {
                        max_val = val;
                    }
                }
                peaks.push(min_val);
                peaks.push(max_val);
            } else {
                peaks.push(0.0);
                peaks.push(0.0);
            }
        }

        let data = WaveformData {
            schema_version: WAVEFORM_SCHEMA_VERSION,
            asset_id: asset_id.to_string(),
            source_duration: duration,
            bucket_rate: effective_rate,
            channels: 1,
            peaks,
        };

        // Write to cache atomically
        if let Some(parent) = out_json_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let json_str = serde_json::to_string(&data).map_err(|e| {
            MediaError::VerificationFailed(format!("Failed to serialize waveform data: {}", e))
        })?;
        let tmp_path = out_json_path.with_extension("tmp");
        std::fs::write(&tmp_path, json_str)?;
        let _ = std::fs::rename(&tmp_path, out_json_path);

        Ok(data)
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
        if let Ok(mut cmd) = MediaBinaryResolver::command("ffmpeg") {
            let _ = cmd
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

        // 4. Test waveform extraction
        let wave_path = temp_dir
            .join("waveforms")
            .join("fixture_test_waveform.json");
        let wave_res =
            FFmpegService::extract_waveform(&fixture_path, "ast_fixture_test", 50, &wave_path);
        assert!(wave_res.is_ok());
        let wave_data = wave_res.unwrap();
        assert_eq!(wave_data.asset_id, "ast_fixture_test");
        assert_eq!(wave_data.channels, 1);
        assert_eq!(wave_data.bucket_rate, 50);
        assert!(!wave_data.peaks.is_empty());
        assert!(wave_path.exists());

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
