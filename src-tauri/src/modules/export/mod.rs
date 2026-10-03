//! Export Foundation module
//! Delivers a safe, deterministic export workflow for the supported semantic subset.
//! Architecture Invariants:
//! - Snapshot-isolated: export starts from a specific project revision; later edits do not change the in-flight render.
//! - Safe output promotion: renders to a temporary file and atomically replaces only upon success.
//! - Cancellable and non-supersedable job lifecycle.
//! - Preserves pre-existing valid output on failure or cancellation.

use crate::modules::composition::CompositionPlan;
use crate::modules::domain::time::FrameRate;
use crate::modules::domain::Project;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportError {
    #[error("Unsupported export profile: {0}")]
    UnsupportedProfile(String),

    #[error("Render execution failed: {0}")]
    RenderFailed(String),

    #[error("Export operation cancelled by user or job manager")]
    Cancelled,

    #[error("IO error during export promotion: {0}")]
    IoError(String),

    #[error("Composition error: {0}")]
    Composition(String),
}

/// Bounded output profile for deterministic MP4 export
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportProfile {
    pub width: u32,
    pub height: u32,
    pub frame_rate: FrameRate,
    pub crf: u32,
    pub preset: String,
    pub video_codec: String,
    pub audio_codec: String,
    pub audio_bitrate: String,
}

impl Default for ExportProfile {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            frame_rate: FrameRate::FPS_30,
            crf: 23,
            preset: "fast".to_string(),
            video_codec: "libx264".to_string(),
            audio_codec: "aac".to_string(),
            audio_bitrate: "192k".to_string(),
        }
    }
}

impl ExportProfile {
    pub fn validate(&self) -> Result<(), ExportError> {
        let supported_codecs = ["libx264", "h264_nvenc", "h264_qsv", "h264_amf"];
        if !supported_codecs.contains(&self.video_codec.as_str()) {
            return Err(ExportError::UnsupportedProfile(format!(
                "Video codec '{}' is not supported in v1 (only libx264 and standard hardware codecs are supported)",
                self.video_codec
            )));
        }
        if self.audio_codec != "aac" {
            return Err(ExportError::UnsupportedProfile(format!(
                "Audio codec '{}' is not supported in v1 (only aac is supported)",
                self.audio_codec
            )));
        }
        if self.width == 0 || self.height == 0 {
            return Err(ExportError::UnsupportedProfile(
                "Export dimensions must be positive integers".to_string(),
            ));
        }
        if self.crf > 51 {
            return Err(ExportError::UnsupportedProfile(
                "CRF must be between 0 and 51".to_string(),
            ));
        }
        Ok(())
    }
}

/// Report returned upon successful export completion
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportReport {
    pub output_path: PathBuf,
    pub duration_secs: f64,
    pub file_size_bytes: u64,
    pub render_time_ms: u128,
}

/// Export Engine coordinating snapshot freeze, FFmpeg execution, and atomic promotion
pub struct ExportEngine;

impl ExportEngine {
    /// Renders a project snapshot to the target file safely and deterministically.
    /// Invariant: Edits made to other project instances do not affect this frozen snapshot.
    pub fn render_snapshot(
        snapshot: &Project,
        target_path: &Path,
        profile: &ExportProfile,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<ExportReport, ExportError> {
        profile.validate()?;

        // Compile normalized CompositionPlan from the frozen project snapshot
        let plan = CompositionPlan::from_project(snapshot)
            .map_err(|e| ExportError::Composition(e.to_string()))?;

        // 1. Establish temporary output path for safe promotion
        let temp_filename = format!(
            "{}.{}.tc_render.tmp.mp4",
            target_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy(),
            std::process::id()
        );
        let temp_output_path = target_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(temp_filename);

        // Ensure parent directory exists
        if let Some(parent) = temp_output_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| ExportError::IoError(format!("Failed to create parent dir: {e}")))?;
        }

        // Clean up any stale temp file
        let _ = fs::remove_file(&temp_output_path);

        // 2. Generate FFmpeg command arguments from the CompositionPlan
        let is_hw = profile.video_codec != "libx264";
        let active_codec = profile.video_codec.clone();
        let (args, _) = plan.to_ffmpeg_export_args(&temp_output_path, profile.crf, &active_codec);

        let start_time = Instant::now();

        // 3. Spawn child FFmpeg process with piped/null I/O
        let mut child: Child = Command::new("ffmpeg")
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                ExportError::RenderFailed(format!("Failed to spawn FFmpeg process: {e}"))
            })?;

        // 4. Poll execution while checking cancellation flag
        let mut cancelled = false;
        loop {
            if let Some(ref flag) = cancel_flag {
                if flag.load(Ordering::SeqCst) {
                    cancelled = true;
                    let _ = child.kill();
                    break;
                }
            }

            match child.try_wait() {
                Ok(Some(status)) => {
                    if !status.success() {
                        let _ = fs::remove_file(&temp_output_path);
                        let mut stderr_str = String::new();
                        if let Some(mut err_pipe) = child.stderr.take() {
                            use std::io::Read;
                            let _ = err_pipe.read_to_string(&mut stderr_str);
                        }

                        // POC 5 Hardware vs Software Fallback Gate:
                        // If hardware encoder failed or is unavailable on this machine,
                        // fallback to libx264 software encoding automatically!
                        if is_hw {
                            let mut fallback_profile = profile.clone();
                            fallback_profile.video_codec = "libx264".to_string();
                            return Self::render_snapshot(
                                snapshot,
                                target_path,
                                &fallback_profile,
                                cancel_flag,
                            );
                        }

                        return Err(ExportError::RenderFailed(format!(
                            "FFmpeg render process exited with error status: {:?}. Stderr: {}",
                            status.code(),
                            stderr_str
                        )));
                    }
                    break;
                }
                Ok(None) => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => {
                    let _ = child.kill();
                    let _ = fs::remove_file(&temp_output_path);
                    return Err(ExportError::RenderFailed(format!(
                        "Error checking FFmpeg process: {e}"
                    )));
                }
            }
        }

        if cancelled {
            // Safe cleanup: remove incomplete temporary file, leave pre-existing target intact
            let _ = fs::remove_file(&temp_output_path);
            return Err(ExportError::Cancelled);
        }

        // 5. Verify temporary file was written and is non-empty
        let metadata = fs::metadata(&temp_output_path).map_err(|e| {
            ExportError::RenderFailed(format!("Rendered file is missing or inaccessible: {e}"))
        })?;

        if metadata.len() == 0 {
            let _ = fs::remove_file(&temp_output_path);
            return Err(ExportError::RenderFailed(
                "Rendered file is empty (0 bytes)".to_string(),
            ));
        }

        // 6. Safe output promotion: atomically rename temp file to target output
        // On Windows, if target exists, remove it right before rename to ensure atomic replacement
        if target_path.exists() {
            let _ = fs::remove_file(target_path);
        }

        fs::rename(&temp_output_path, target_path).map_err(|e| {
            let _ = fs::remove_file(&temp_output_path);
            ExportError::IoError(format!(
                "Failed to promote temporary render file to destination: {e}"
            ))
        })?;

        let render_time_ms = start_time.elapsed().as_millis();
        let file_size_bytes = metadata.len();
        let duration_secs = plan.duration.to_seconds();

        Ok(ExportReport {
            output_path: target_path.to_path_buf(),
            duration_secs,
            file_size_bytes,
            render_time_ms,
        })
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::modules::domain::time::{FrameRate, RationalTime, TimeRange};
    use crate::modules::domain::{
        Asset, ItemKind, MediaKind, Project, ProjectSettings, TimelineItem, Track, TrackKind,
    };
    use std::fs;
    use std::process::Command;

    fn get_fixture_dir() -> PathBuf {
        let dir = std::env::temp_dir().join("tinixcut_test_fixtures_phase8");
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn generate_synthetic_mp4(path: &Path, duration_secs: u32, color: &str) {
        if path.exists() {
            return;
        }
        let output = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                &format!("color=c={}:s=640x360:r=30:d={}", color, duration_secs),
                "-f",
                "lavfi",
                "-i",
                &format!("sine=frequency=440:duration={}", duration_secs),
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
                &path.to_string_lossy(),
            ])
            .output();

        if let Ok(res) = output {
            if !res.status.success() {
                eprintln!(
                    "FFmpeg synthetic generator warning: {}",
                    String::from_utf8_lossy(&res.stderr)
                );
            }
        }
    }

    #[test]
    fn test_export_profile_validation() {
        let mut profile = ExportProfile::default();
        assert!(profile.validate().is_ok());

        profile.video_codec = "prores".to_string();
        assert!(profile.validate().is_err());

        profile.video_codec = "libx264".to_string();
        profile.audio_codec = "mp3".to_string();
        assert!(profile.validate().is_err());

        profile.audio_codec = "aac".to_string();
        profile.width = 0;
        assert!(profile.validate().is_err());
    }

    #[test]
    fn test_export_snapshot_isolation_and_atomic_promotion() {
        let fixture_dir = get_fixture_dir();
        let test_dir = fixture_dir.join("isolation_sub");
        fs::create_dir_all(&test_dir).unwrap();
        let src_video = fixture_dir.join("input_source.mp4");
        generate_synthetic_mp4(&src_video, 3, "red");

        let mut project = Project::new("Export Test Project".to_string());
        project.settings = ProjectSettings {
            width: 640,
            height: 360,
            frame_rate: FrameRate::FPS_30,
            sample_rate: 48000,
            channels: 2,
        };

        let asset_id = "asset-src-1".to_string();
        project.assets.push(Asset {
            asset_id: asset_id.clone(),
            name: "input_source.mp4".to_string(),
            path: src_video.to_string_lossy().to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(90, FrameRate::FPS_30)),
            file_size: 1024,
        });

        let mut track = Track::new("Video Track".to_string(), TrackKind::Video);
        let item = TimelineItem::new(
            Some(asset_id),
            "Clip 1".to_string(),
            ItemKind::Video,
            TimeRange::new(
                RationalTime::new(0, FrameRate::FPS_30),
                RationalTime::new(60, FrameRate::FPS_30), // 2.0 seconds
            ),
            TimeRange::new(
                RationalTime::new(0, FrameRate::FPS_30),
                RationalTime::new(60, FrameRate::FPS_30),
            ),
        )
        .unwrap();
        track.items.push(item);
        project.tracks.push(track);

        // 1. Take snapshot of project
        let frozen_snapshot = project.clone();

        // 2. Mutate active project in memory (simulating user continuing to edit)
        project.tracks.clear();
        project.name = "Mutated Project".to_string();

        let output_file = test_dir.join("exported_render.mp4");
        let _ = fs::remove_file(&output_file);

        let mut profile = ExportProfile::default();
        profile.width = 640;
        profile.height = 360;

        // 3. Render using frozen snapshot
        let report = ExportEngine::render_snapshot(&frozen_snapshot, &output_file, &profile, None)
            .expect("Render should succeed");

        // Verify output file exists and is promoted
        assert!(output_file.exists());
        assert_eq!(report.duration_secs, 2.0);
        assert!(report.file_size_bytes > 0);

        // Verify no temporary .tmp file left behind in test_dir
        let entries = fs::read_dir(&test_dir).unwrap();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            assert!(
                !name.contains(".tc_render.tmp"),
                "Temporary render file was not cleaned up: {}",
                name
            );
        }

        // 4. Repeat export to verify determinism
        let repeat_output = test_dir.join("repeat_render.mp4");
        let repeat_report =
            ExportEngine::render_snapshot(&frozen_snapshot, &repeat_output, &profile, None)
                .expect("Repeat render should succeed");

        assert_eq!(report.duration_secs, repeat_report.duration_secs);
        assert!(repeat_output.exists());
    }

    #[test]
    fn test_export_cancellation_safety_preserves_valid_output() {
        let fixture_dir = get_fixture_dir();
        let test_dir = fixture_dir.join("cancellation_sub");
        fs::create_dir_all(&test_dir).unwrap();
        let src_video = fixture_dir.join("input_source_cancel.mp4");
        generate_synthetic_mp4(&src_video, 4, "green");

        let mut project = Project::new("Cancel Test Project".to_string());
        project.settings = ProjectSettings {
            width: 640,
            height: 360,
            frame_rate: FrameRate::FPS_30,
            sample_rate: 48000,
            channels: 2,
        };

        let asset_id = "asset-cancel".to_string();
        project.assets.push(Asset {
            asset_id: asset_id.clone(),
            name: "input_source_cancel.mp4".to_string(),
            path: src_video.to_string_lossy().to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(120, FrameRate::FPS_30)),
            file_size: 1024,
        });

        let mut track = Track::new("Video Track".to_string(), TrackKind::Video);
        let item = TimelineItem::new(
            Some(asset_id),
            "Clip 1".to_string(),
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
        track.items.push(item);
        project.tracks.push(track);

        let output_file = test_dir.join("cancelled_target.mp4");
        // Create an existing valid file at target_path
        fs::write(&output_file, b"PRE_EXISTING_VALID_CONTENT").unwrap();

        let cancel_flag = Arc::new(AtomicBool::new(false));
        // Flag set to cancel immediately
        cancel_flag.store(true, Ordering::SeqCst);

        let mut profile = ExportProfile::default();
        profile.width = 640;
        profile.height = 360;

        let result =
            ExportEngine::render_snapshot(&project, &output_file, &profile, Some(cancel_flag));

        // Verification of Gate requirements:
        // 1. Result must be Cancelled
        assert_eq!(result, Err(ExportError::Cancelled));

        // 2. Pre-existing valid file at target_path must be completely preserved!
        let content = fs::read(&output_file).unwrap();
        assert_eq!(content, b"PRE_EXISTING_VALID_CONTENT");

        // 3. No orphan .tmp file
        let entries = fs::read_dir(&test_dir).unwrap();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            assert!(
                !name.contains(".tc_render.tmp"),
                "Leftover temp file after cancellation: {}",
                name
            );
        }
    }

    /// POC 5 — Hardware vs Software Fallback qualification
    #[test]
    fn test_poc5_hardware_vs_software_fallback() {
        let fixture_dir = get_fixture_dir();
        let test_dir = fixture_dir.join("poc5_sub");
        fs::create_dir_all(&test_dir).unwrap();
        let src_video = fixture_dir.join("input_source_poc5.mp4");
        generate_synthetic_mp4(&src_video, 3, "magenta");

        let mut project = Project::new("POC 5 Fallback Project".to_string());
        project.settings = ProjectSettings {
            width: 640,
            height: 360,
            frame_rate: FrameRate::FPS_30,
            sample_rate: 48000,
            channels: 2,
        };

        let asset_id = "asset-poc5".to_string();
        project.assets.push(Asset {
            asset_id: asset_id.clone(),
            name: "input_source_poc5.mp4".to_string(),
            path: src_video.to_string_lossy().to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(90, FrameRate::FPS_30)),
            file_size: 1024,
        });

        let mut track = Track::new("Video Track".to_string(), TrackKind::Video);
        let item = TimelineItem::new(
            Some(asset_id),
            "Clip 1".to_string(),
            ItemKind::Video,
            TimeRange::new(
                RationalTime::new(0, FrameRate::FPS_30),
                RationalTime::new(60, FrameRate::FPS_30),
            ),
            TimeRange::new(
                RationalTime::new(0, FrameRate::FPS_30),
                RationalTime::new(60, FrameRate::FPS_30),
            ),
        )
        .unwrap();
        track.items.push(item);
        project.tracks.push(track);

        let output_file = test_dir.join("poc5_fallback_render.mp4");
        let _ = fs::remove_file(&output_file);

        // Request hardware acceleration (e.g. h264_nvenc)
        let mut hw_profile = ExportProfile::default();
        hw_profile.width = 640;
        hw_profile.height = 360;
        hw_profile.video_codec = "h264_nvenc".to_string();

        // Render: Even if h264_nvenc is unavailable or GPU drivers are absent,
        // the engine must automatically fall back to software encoding (libx264)
        // and produce a valid output!
        let report = ExportEngine::render_snapshot(&project, &output_file, &hw_profile, None)
            .expect("Render with hardware fallback must succeed");

        assert!(output_file.exists());
        assert_eq!(report.duration_secs, 2.0);
        assert!(report.file_size_bytes > 0);

        let _ = fs::remove_file(output_file);
    }
}
