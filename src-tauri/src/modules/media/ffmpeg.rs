//! FFmpeg and FFprobe Native Service Abstraction
//! Architecture Invariant: Typed arguments only; no raw untrusted string execution.
//! Isolates media inspection and derived asset generation behind safe Rust interfaces.

use crate::modules::domain::time::FrameRate;
use crate::modules::media::resolver::MediaBinaryResolver;
use crate::modules::media::scene::{
    parse_scene_metadata, SceneAnalysisReport, SCENE_ANALYSIS_ALGORITHM_VERSION,
    SCENE_ANALYSIS_SCHEMA_VERSION,
};
use crate::modules::media::waveform::{WaveformData, WAVEFORM_SCHEMA_VERSION};
use crate::modules::media::MediaError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant, UNIX_EPOCH};

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioLevelReport {
    pub source_fingerprint: String,
    pub mean_dbfs: Option<f32>,
    pub peak_dbfs: Option<f32>,
    pub measurement: String,
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

/// Maximum spacing between keyframes in preview proxies (see `generate_proxy`).
pub const PREVIEW_PROXY_KEYFRAME_INTERVAL_SECS: f64 = 0.5;

pub struct FFmpegService;

impl FFmpegService {
    /// Measures source audio mean and sample peak using FFmpeg volumedetect.
    /// These are dBFS diagnostics, not EBU R128 integrated LUFS measurements.
    pub fn analyze_audio_levels(source_path: &Path) -> Result<AudioLevelReport, MediaError> {
        if !source_path.exists() {
            return Err(MediaError::TargetNotFound(source_path.to_path_buf()));
        }
        let metadata = Self::probe_file(source_path)?;
        if metadata.audio_codec.is_none() {
            return Err(MediaError::VerificationFailed(
                "Source media contains no audio stream".to_string(),
            ));
        }
        #[cfg(windows)]
        let null_sink = "NUL";
        #[cfg(not(windows))]
        let null_sink = "/dev/null";
        let output = MediaBinaryResolver::command("ffmpeg")?
            .arg("-hide_banner")
            .arg("-nostats")
            .arg("-v")
            .arg("info")
            .arg("-i")
            .arg(source_path)
            .arg("-map")
            .arg("0:a:0")
            .arg("-vn")
            .arg("-af")
            .arg("volumedetect")
            .arg("-f")
            .arg("null")
            .arg(null_sink)
            .output()
            .map_err(|error| {
                MediaError::VerificationFailed(format!(
                    "Failed to spawn FFmpeg audio analysis: {error}"
                ))
            })?;
        if !output.status.success() {
            return Err(MediaError::VerificationFailed(format!(
                "FFmpeg audio level analysis failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        let log = String::from_utf8_lossy(&output.stderr);
        let (mean_dbfs, peak_dbfs) = parse_volume_detect(&log).ok_or_else(|| {
            MediaError::VerificationFailed(
                "FFmpeg did not report mean_volume/max_volume".to_string(),
            )
        })?;
        Ok(AudioLevelReport {
            source_fingerprint: Self::source_fingerprint(source_path)?,
            mean_dbfs,
            peak_dbfs,
            measurement: "FFmpeg volumedetect mean/max dBFS (not LUFS)".to_string(),
        })
    }

    /// Builds a cheap cache-invalidation fingerprint from canonical path and file metadata.
    /// This avoids hashing the full media payload on every waveform lookup.
    pub fn source_fingerprint(file_path: &Path) -> Result<String, MediaError> {
        let canonical_path = file_path.canonicalize()?;
        let metadata = std::fs::metadata(&canonical_path)?;
        let modified_ns = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();

        let mut digest = Sha256::new();
        digest.update(canonical_path.to_string_lossy().as_bytes());
        digest.update(metadata.len().to_le_bytes());
        digest.update(modified_ns.to_le_bytes());
        Ok(digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect())
    }

    pub fn scene_analysis_config_fingerprint(
        threshold: f32,
        max_duration_seconds: u32,
        max_candidates: usize,
    ) -> String {
        let config = format!(
            "{}|{}|{}|{}",
            SCENE_ANALYSIS_ALGORITHM_VERSION,
            threshold.to_bits(),
            max_duration_seconds,
            max_candidates
        );
        Sha256::digest(config.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

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

    /// Find a bounded set of scene-boundary candidates without changing a project.
    /// FFmpeg runs in a killable child process and writes only selected-frame metadata
    /// into a temporary directory. The media duration, candidate count and wall time
    /// are capped to keep untrusted input bounded.
    pub fn analyze_scene_candidates(
        source_path: &Path,
        threshold: f32,
        max_duration_seconds: u32,
        max_candidates: usize,
        cancel_flag: &AtomicBool,
    ) -> Result<SceneAnalysisReport, MediaError> {
        if !source_path.is_file() {
            return Err(MediaError::TargetNotFound(source_path.to_path_buf()));
        }
        if !threshold.is_finite() || !(0.05..=0.95).contains(&threshold) {
            return Err(MediaError::VerificationFailed(
                "Scene threshold must be between 0.05 and 0.95".to_string(),
            ));
        }
        if !(1..=1_800).contains(&max_duration_seconds) {
            return Err(MediaError::VerificationFailed(
                "Scene analysis duration must be between 1 and 1800 seconds".to_string(),
            ));
        }
        if !(1..=2_000).contains(&max_candidates) {
            return Err(MediaError::VerificationFailed(
                "Scene candidate limit must be between 1 and 2000".to_string(),
            ));
        }

        let metadata = Self::probe_file(source_path)?;
        if !metadata
            .streams
            .iter()
            .any(|stream| stream.codec_type == "video")
        {
            return Err(MediaError::VerificationFailed(
                "Scene analysis requires a video stream".to_string(),
            ));
        }
        if cancel_flag.load(Ordering::SeqCst) {
            return Err(MediaError::Cancelled);
        }

        let source_fingerprint = Self::source_fingerprint(source_path)?;
        let work_dir = std::env::temp_dir().join(format!(
            "tinixcut-scene-analysis-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&work_dir)?;
        let metadata_path = work_dir.join("scene_metadata.txt");
        let log_path = work_dir.join("ffmpeg.log");
        let filter = format!(
            "select='gt(scene,{threshold:.3})',metadata=mode=print:key=lavfi.scene_score:file=scene_metadata.txt:direct=1"
        );
        let null_sink = if cfg!(windows) { "NUL" } else { "/dev/null" };
        let mut command = MediaBinaryResolver::command("ffmpeg")?;
        command
            .current_dir(&work_dir)
            .arg("-hide_banner")
            .arg("-nostats")
            .arg("-loglevel")
            .arg("error")
            .arg("-i")
            .arg(source_path)
            .arg("-t")
            .arg(max_duration_seconds.to_string())
            .arg("-vf")
            .arg(filter)
            .arg("-an")
            .arg("-frames:v")
            .arg(max_candidates.saturating_add(1).to_string())
            .arg("-f")
            .arg("null")
            .arg(null_sink)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(std::fs::File::create(&log_path)?));

        let process_result = (|| -> Result<Vec<u8>, MediaError> {
            let mut child = command.spawn().map_err(|error| {
                MediaError::VerificationFailed(format!(
                    "FFmpeg scene analysis could not start: {error}"
                ))
            })?;
            let deadline = Instant::now() + Duration::from_secs(120);
            let status = loop {
                if cancel_flag.load(Ordering::SeqCst) {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(MediaError::Cancelled);
                }
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(MediaError::VerificationFailed(
                        "Scene analysis exceeded its 120-second runtime limit".to_string(),
                    ));
                }
                if let Some(status) = child.try_wait().map_err(|error| {
                    MediaError::VerificationFailed(format!(
                        "FFmpeg scene analysis could not be monitored: {error}"
                    ))
                })? {
                    break status;
                }
                thread::sleep(Duration::from_millis(50));
            };
            if !status.success() {
                let details = std::fs::read_to_string(&log_path).unwrap_or_default();
                return Err(MediaError::VerificationFailed(format!(
                    "FFmpeg scene analysis failed with exit code {}: {}",
                    status.code().unwrap_or(-1),
                    details.trim()
                )));
            }
            Ok(std::fs::read(&metadata_path).unwrap_or_default())
        })();
        let _ = std::fs::remove_dir_all(&work_dir);
        let metadata_bytes = process_result?;
        let parsed = parse_scene_metadata(
            &String::from_utf8_lossy(&metadata_bytes),
            max_candidates.saturating_add(1),
        );
        let truncated = metadata.duration_seconds > max_duration_seconds as f64
            || parsed.len() > max_candidates;
        let candidates = parsed.into_iter().take(max_candidates).collect();

        Ok(SceneAnalysisReport {
            schema_version: SCENE_ANALYSIS_SCHEMA_VERSION,
            algorithm_version: SCENE_ANALYSIS_ALGORITHM_VERSION.to_string(),
            source_fingerprint,
            config_fingerprint: Self::scene_analysis_config_fingerprint(
                threshold,
                max_duration_seconds,
                max_candidates,
            ),
            threshold,
            analyzed_duration_micros: (metadata
                .duration_seconds
                .min(max_duration_seconds as f64)
                .max(0.0)
                * 1_000_000.0)
                .round() as u64,
            truncated,
            candidates,
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

    /// Extract one JPEG frame with a strict maximum side and cancellable bounded runtime.
    pub fn extract_bounded_frame(
        source_path: &Path,
        time_micros: u64,
        out_path: &Path,
        max_side: u32,
        cancel_flag: &AtomicBool,
    ) -> Result<(), MediaError> {
        if !source_path.is_file() {
            return Err(MediaError::TargetNotFound(source_path.to_path_buf()));
        }
        if !(64..=1024).contains(&max_side) {
            return Err(MediaError::VerificationFailed(
                "Visual frame size must be between 64 and 1024 pixels".to_string(),
            ));
        }
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let seconds = time_micros as f64 / 1_000_000.0;
        let scale = format!("scale={max_side}:{max_side}:force_original_aspect_ratio=decrease");
        let mut child = MediaBinaryResolver::command("ffmpeg")?
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-ss")
            .arg(format!("{seconds:.6}"))
            .arg("-i")
            .arg(source_path)
            .arg("-map")
            .arg("0:v:0")
            .arg("-frames:v")
            .arg("1")
            .arg("-vf")
            .arg(scale)
            .arg("-q:v")
            .arg("5")
            .arg("-y")
            .arg(out_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| {
                MediaError::VerificationFailed(format!(
                    "Could not start FFmpeg visual frame extraction: {error}"
                ))
            })?;
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if cancel_flag.load(Ordering::SeqCst) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(MediaError::Cancelled);
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(MediaError::VerificationFailed(
                    "Visual frame extraction exceeded its 15-second limit".to_string(),
                ));
            }
            if let Some(status) = child.try_wait()? {
                if !status.success() || !out_path.is_file() {
                    return Err(MediaError::VerificationFailed(format!(
                        "FFmpeg visual frame extraction failed with exit code {}",
                        status.code().unwrap_or(-1)
                    )));
                }
                let metadata = std::fs::metadata(out_path)?;
                if metadata.len() > 512 * 1024 {
                    let _ = std::fs::remove_file(out_path);
                    return Err(MediaError::VerificationFailed(
                        "Extracted visual frame exceeded the 512 KiB limit".to_string(),
                    ));
                }
                return Ok(());
            }
            thread::sleep(Duration::from_millis(25));
        }
    }

    /// Generate a downscaled preview proxy optimized for seeking.
    ///
    /// A keyframe every 0.5 s (frame-rate independent) bounds decode-from-keyframe
    /// work per seek; with x264's default GOP (~8 s at 30 fps) a proxy seeks no
    /// faster than the original. Output is written to a sibling partial file and
    /// renamed, so a reader that checks for the final path never sees a partial proxy.
    pub fn generate_proxy(
        source_path: &Path,
        out_path: &Path,
        target_height: u32,
    ) -> Result<(), MediaError> {
        if let Some(parent) = out_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let scale_filter = format!("scale=-2:{}", target_height);
        let partial_path = out_path.with_extension("partial.mp4");

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
            .arg("-force_key_frames")
            .arg(format!(
                "expr:gte(t,n_forced*{PREVIEW_PROXY_KEYFRAME_INTERVAL_SECS})"
            ))
            .arg("-sc_threshold")
            .arg("0")
            .arg("-c:a")
            .arg("aac")
            .arg("-movflags")
            .arg("+faststart")
            .arg("-y")
            .arg(&partial_path)
            .output()
            .map_err(|e| {
                MediaError::VerificationFailed(format!("Failed to spawn ffmpeg: {}", e))
            })?;

        if !output.status.success() {
            let _ = std::fs::remove_file(&partial_path);
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(MediaError::VerificationFailed(format!(
                "Proxy generation failed: {}",
                err
            )));
        }

        std::fs::rename(&partial_path, out_path).map_err(|error| {
            let _ = std::fs::remove_file(&partial_path);
            MediaError::VerificationFailed(format!(
                "Could not finalize preview proxy at {}: {error}",
                out_path.display()
            ))
        })
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
            source_fingerprint: Self::source_fingerprint(source_path)?,
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

fn parse_volume_detect(log: &str) -> Option<(Option<f32>, Option<f32>)> {
    fn read_value(log: &str, key: &str) -> Option<Option<f32>> {
        let line = log.lines().find(|line| line.contains(key))?;
        let value = line.split_once(key)?.1.trim().split_whitespace().next()?;
        if value.eq_ignore_ascii_case("-inf") || value.eq_ignore_ascii_case("inf") {
            Some(None)
        } else {
            value
                .parse::<f32>()
                .ok()
                .filter(|number| number.is_finite())
                .map(Some)
        }
    }
    Some((
        read_value(log, "mean_volume:")?,
        read_value(log, "max_volume:")?,
    ))
}

#[cfg(test)]
mod phase51_audio_level_tests {
    use super::parse_volume_detect;

    #[test]
    fn parses_mean_and_peak_dbfs() {
        let log = "[Parsed_volumedetect] mean_volume: -21.5 dB\n[Parsed_volumedetect] max_volume: -0.1 dB";
        assert_eq!(parse_volume_detect(log), Some((Some(-21.5), Some(-0.1))));
    }

    #[test]
    fn represents_silence_as_absent_dbfs_values() {
        let log = "mean_volume: -inf dB\nmax_volume: -inf dB";
        assert_eq!(parse_volume_detect(log), Some((None, None)));
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

    #[test]
    fn source_fingerprint_changes_when_source_metadata_changes() {
        let path = std::env::temp_dir().join(format!(
            "tinixcut-source-fingerprint-{}.media",
            uuid::Uuid::new_v4().simple()
        ));
        fs::write(&path, b"source version one").unwrap();
        let first = FFmpegService::source_fingerprint(&path).unwrap();
        fs::write(&path, b"source version two with changed size").unwrap();
        let second = FFmpegService::source_fingerprint(&path).unwrap();

        assert_ne!(first, second);
        fs::remove_file(path).unwrap();
    }

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

        let levels = FFmpegService::analyze_audio_levels(&fixture_path)
            .expect("generated audio stream should have measurable levels");
        assert!(levels.mean_dbfs.is_some());
        assert!(levels.peak_dbfs.is_some());
        assert!(levels.mean_dbfs.unwrap() < 0.0);
        assert!(levels.measurement.contains("not LUFS"));

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
        assert!(
            !proxy_path.with_extension("partial.mp4").exists(),
            "partial proxy must be renamed into place"
        );
        // The 1 s fixture must carry keyframes at 0 s and 0.5 s; the x264 default
        // GOP would leave a single keyframe and make proxy seeks slow.
        let keyframes = MediaBinaryResolver::command("ffprobe")
            .expect("ffprobe resolves when ffmpeg does")
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-skip_frame",
                "nokey",
                "-show_entries",
                "frame=pts_time",
                "-of",
                "csv=p=0",
            ])
            .arg(&proxy_path)
            .output()
            .expect("ffprobe runs");
        let keyframe_count = String::from_utf8_lossy(&keyframes.stdout)
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count();
        assert!(
            keyframe_count >= 2,
            "expected short-GOP proxy, got {keyframe_count} keyframe(s)"
        );

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
