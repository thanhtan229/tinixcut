//! Phase 14 Export Automation Schedule
//! Evaluates dynamic keyframe properties per-frame in Rust and produces:
//! 1. FFmpeg `sendcmd` scripts with midpoint dispatch for video layers and transition branches.
//! 2. Streamed mono `f32le` audio gain envelopes applied via `amultiply` with exact 1.0 stereo gain.
//! 3. RAII cleanup guard guaranteeing temporary file removal on success, failure, or cancellation.

use crate::modules::composition::{CompositionAudioLayer, CompositionPlan, CompositionVisualLayer};
use crate::modules::domain::time::RationalTime;
use crate::modules::domain::KeyframeProperty;
use crate::modules::export::ExportError;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// RAII Guard that deletes all tracked temporary artifact files on drop.
#[derive(Debug, Default)]
pub struct AutomationArtifactsGuard {
    pub files: Vec<PathBuf>,
}

impl AutomationArtifactsGuard {
    pub fn new() -> Self {
        Self { files: Vec::new() }
    }

    pub fn track(&mut self, path: PathBuf) {
        self.files.push(path);
    }

    pub fn cleanup(&mut self) {
        for path in &self.files {
            let _ = fs::remove_file(path);
        }
        self.files.clear();
    }
}

impl Drop for AutomationArtifactsGuard {
    fn drop(&mut self) {
        self.cleanup();
    }
}

/// Helper: escape Windows path for FFmpeg filtergraph strings.
/// Replaces backslashes with slashes, and escapes colons (e.g. C\:).
pub fn escape_ffmpeg_filter_path(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/").replace(':', "\\:")
}

/// Check if a visual layer has any animated keyframe properties.
pub fn layer_has_animated_visual_properties(layer: &CompositionVisualLayer) -> bool {
    layer.keyframes.iter().any(|k| {
        matches!(
            k.property,
            KeyframeProperty::PositionX
                | KeyframeProperty::PositionY
                | KeyframeProperty::ScaleX
                | KeyframeProperty::ScaleY
                | KeyframeProperty::Rotation
                | KeyframeProperty::Opacity
        )
    })
}

/// Check if an audio layer has animated Volume keyframes.
pub fn layer_has_animated_volume(layer: &CompositionAudioLayer) -> bool {
    layer
        .keyframes
        .iter()
        .any(|k| k.property == KeyframeProperty::Volume)
}

/// Video layer automation schedule metadata
#[derive(Debug, Clone)]
pub struct LayerVideoAutomation {
    pub layer_index: usize,
    pub command_file: PathBuf,
    pub escaped_command_file: String,
}

/// Transition branch automation schedule metadata
#[derive(Debug, Clone)]
pub struct TransitionBranchAutomation {
    pub transition_index: usize,
    pub left_command_file: Option<PathBuf>,
    pub escaped_left_command_file: Option<String>,
    pub right_command_file: Option<PathBuf>,
    pub escaped_right_command_file: Option<String>,
}

/// Audio layer automation envelope metadata
#[derive(Debug, Clone)]
pub struct LayerAudioAutomation {
    pub audio_index: usize,
    pub envelope_file: PathBuf,
    pub sample_rate: u32,
    pub total_samples: usize,
}

/// Derived, non-persisted schedule holding all runtime commands and envelopes for an export job.
pub struct ExportAutomationSchedule {
    pub video_layers: Vec<LayerVideoAutomation>,
    pub transition_branches: Vec<TransitionBranchAutomation>,
    pub audio_layers: Vec<LayerAudioAutomation>,
    pub guard: AutomationArtifactsGuard,
}

impl ExportAutomationSchedule {
    /// Builds the complete export automation schedule:
    /// - Checks cancellation and disk space preflight
    /// - Evaluates visual property keyframes per-frame in Rust
    /// - Writes sendcmd command scripts with midpoint dispatch
    /// - Streams audio volume envelopes with bounded memory
    pub fn build(
        plan: &CompositionPlan,
        temp_dir: &Path,
        audio_sample_rate: u32,
        cancel_flag: Option<&Arc<AtomicBool>>,
    ) -> Result<Self, ExportError> {
        let mut guard = AutomationArtifactsGuard::new();
        let fps = plan.frame_rate.as_f64();
        let pid = std::process::id();
        let random_nonce = uuid::Uuid::new_v4().simple().to_string();

        fs::create_dir_all(temp_dir).map_err(|e| {
            ExportError::IoError(format!("Failed to create automation temp dir: {e}"))
        })?;

        // 1. Preflight check: estimate required audio envelope bytes
        let mut total_estimated_audio_bytes: u64 = 0;
        for audio in &plan.audio_layers {
            if !audio.muted && layer_has_animated_volume(audio) {
                let dur = audio.source_range.duration.to_seconds();
                let samples = (dur * audio_sample_rate as f64).round() as u64;
                total_estimated_audio_bytes += samples * 4; // 4 bytes per mono f32le sample
            }
        }

        // Bounded preflight: maximum 4GB per export session sanity limit
        if total_estimated_audio_bytes > 4 * 1024 * 1024 * 1024 {
            return Err(ExportError::RenderFailed(format!(
                "Estimated audio envelope size ({:.2} GB) exceeds safe 4GB limit",
                total_estimated_audio_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
            )));
        }

        // 2. Generate Video Layer Schedules
        let mut video_layers = Vec::new();
        for (idx, layer) in plan.visual_layers.iter().enumerate() {
            if let Some(flag) = cancel_flag {
                if flag.load(Ordering::SeqCst) {
                    return Err(ExportError::Cancelled);
                }
            }

            if !layer_has_animated_visual_properties(layer) {
                continue;
            }

            let incoming_trans = plan
                .transitions
                .iter()
                .find(|t| t.right_item_id == layer.item_id && t.track_id == layer.track_id);
            let outgoing_trans = plan
                .transitions
                .iter()
                .find(|t| t.left_item_id == layer.item_id && t.track_id == layer.track_id);

            let layer_start_sec = layer.timeline_range.start.to_seconds();
            let layer_end_sec = layer.timeline_range.end().to_seconds();

            let ord_start_sec = if let Some(t_in) = incoming_trans {
                t_in.transition_range.end().to_seconds()
            } else {
                layer_start_sec
            };

            let ord_end_sec = if let Some(t_out) = outgoing_trans {
                t_out.transition_range.start.to_seconds()
            } else {
                layer_end_sec
            };

            if ord_start_sec >= ord_end_sec {
                continue;
            }

            let cmd_file = temp_dir.join(format!(
                "cmd_layer_{}_{}_{}.txt",
                idx,
                pid,
                &random_nonce[..6]
            ));
            let mut cmd_content = String::new();

            let start_frame = (ord_start_sec * fps).floor() as i64;
            let end_frame = (ord_end_sec * fps).ceil() as i64;

            let base_w = plan.width as f32;
            let base_h = plan.height as f32;
            let cropped_w = base_w * (1.0 - layer.crop.left - layer.crop.right);
            let cropped_h = base_h * (1.0 - layer.crop.top - layer.crop.bottom);

            // Emit runtime command for frames n > start_frame using midpoint dispatch
            for n in (start_frame + 1)..=end_frame {
                let t_prev = (n - 1) as f64 / fps;
                let t_curr = n as f64 / fps;
                let t_mid = (t_prev + t_curr) / 2.0;

                let timeline_time = RationalTime::from_frames(n, plan.frame_rate);
                let clip_offset = timeline_time - layer.timeline_range.start;

                let pos_x = layer.evaluate_property(KeyframeProperty::PositionX, clip_offset);
                let pos_y = layer.evaluate_property(KeyframeProperty::PositionY, clip_offset);
                let scale_x = layer.evaluate_property(KeyframeProperty::ScaleX, clip_offset);
                let scale_y = layer.evaluate_property(KeyframeProperty::ScaleY, clip_offset);
                let rotation = layer.evaluate_property(KeyframeProperty::Rotation, clip_offset);
                let opacity = layer.effective_opacity_at(clip_offset);

                let mut render_w = (cropped_w * scale_x).round().max(2.0) as u32;
                let mut render_h = (cropped_h * scale_y).round().max(2.0) as u32;
                if !render_w.is_multiple_of(2) {
                    render_w += 1;
                }
                if !render_h.is_multiple_of(2) {
                    render_h += 1;
                }

                let top_left_x = ((base_w - render_w as f32) / 2.0 + pos_x).round() as i32;
                let top_left_y = ((base_h - render_h as f32) / 2.0 + pos_y).round() as i32;
                let angle_rad = rotation * std::f32::consts::PI / 180.0;

                cmd_content.push_str(&format!(
                    "{:.6} scale@sc_{} w {};\n\
                     {:.6} scale@sc_{} h {};\n\
                     {:.6} rotate@rot_{} angle {:.4};\n\
                     {:.6} colorchannelmixer@ccm_{} aa {:.3};\n\
                     {:.6} overlay@ov_{} x {};\n\
                     {:.6} overlay@ov_{} y {};\n",
                    t_mid,
                    idx,
                    render_w,
                    t_mid,
                    idx,
                    render_h,
                    t_mid,
                    idx,
                    angle_rad,
                    t_mid,
                    idx,
                    opacity,
                    t_mid,
                    idx,
                    top_left_x,
                    t_mid,
                    idx,
                    top_left_y,
                ));
            }

            fs::write(&cmd_file, &cmd_content).map_err(|e| {
                ExportError::IoError(format!("Failed to write sendcmd script: {e}"))
            })?;

            let escaped = escape_ffmpeg_filter_path(&cmd_file);
            guard.track(cmd_file.clone());

            video_layers.push(LayerVideoAutomation {
                layer_index: idx,
                command_file: cmd_file,
                escaped_command_file: escaped,
            });
        }

        // 3. Generate Transition Branch Schedules
        let mut transition_branches = Vec::new();
        for (idx, trans) in plan.transitions.iter().enumerate() {
            if let Some(flag) = cancel_flag {
                if flag.load(Ordering::SeqCst) {
                    return Err(ExportError::Cancelled);
                }
            }

            let dur_sec = trans.duration.to_seconds();
            let branch_frames = (dur_sec * fps).round() as i64;
            let _t0_sec = trans.transition_range.start.to_seconds();
            let base_w = plan.width as f32;
            let base_h = plan.height as f32;

            // Left transition participant branch
            let left_cmd = if layer_has_animated_visual_properties(&trans.left_layer) {
                let cmd_file = temp_dir.join(format!(
                    "cmd_tr_l_{}_{}_{}.txt",
                    idx,
                    pid,
                    &random_nonce[..6]
                ));
                let mut cmd_content = String::new();

                let cropped_w =
                    base_w * (1.0 - trans.left_layer.crop.left - trans.left_layer.crop.right);
                let cropped_h =
                    base_h * (1.0 - trans.left_layer.crop.top - trans.left_layer.crop.bottom);

                for k in 1..branch_frames {
                    let t_prev = (k - 1) as f64 / fps;
                    let t_curr = k as f64 / fps;
                    let t_mid = (t_prev + t_curr) / 2.0;

                    let branch_offset = RationalTime::from_frames(k, plan.frame_rate);
                    let timeline_time = trans.transition_range.start + branch_offset;
                    let clip_offset = timeline_time - trans.left_layer.timeline_range.start;

                    let pos_x = trans
                        .left_layer
                        .evaluate_property(KeyframeProperty::PositionX, clip_offset);
                    let pos_y = trans
                        .left_layer
                        .evaluate_property(KeyframeProperty::PositionY, clip_offset);
                    let scale_x = trans
                        .left_layer
                        .evaluate_property(KeyframeProperty::ScaleX, clip_offset);
                    let scale_y = trans
                        .left_layer
                        .evaluate_property(KeyframeProperty::ScaleY, clip_offset);

                    let mut render_w = (cropped_w * scale_x).round().max(2.0) as u32;
                    let mut render_h = (cropped_h * scale_y).round().max(2.0) as u32;
                    if !render_w.is_multiple_of(2) {
                        render_w += 1;
                    }
                    if !render_h.is_multiple_of(2) {
                        render_h += 1;
                    }

                    let top_left_x = ((base_w - render_w as f32) / 2.0 + pos_x).round() as i32;
                    let top_left_y = ((base_h - render_h as f32) / 2.0 + pos_y).round() as i32;

                    cmd_content.push_str(&format!(
                        "{:.6} overlay@ov_tr_l_{} x {};\n\
                         {:.6} overlay@ov_tr_l_{} y {};\n",
                        t_mid, idx, top_left_x, t_mid, idx, top_left_y,
                    ));
                }

                fs::write(&cmd_file, &cmd_content).map_err(|e| {
                    ExportError::IoError(format!("Failed to write left transition sendcmd: {e}"))
                })?;
                let escaped = escape_ffmpeg_filter_path(&cmd_file);
                guard.track(cmd_file.clone());
                Some((cmd_file, escaped))
            } else {
                None
            };

            // Right transition participant branch
            let right_cmd = if layer_has_animated_visual_properties(&trans.right_layer) {
                let cmd_file = temp_dir.join(format!(
                    "cmd_tr_r_{}_{}_{}.txt",
                    idx,
                    pid,
                    &random_nonce[..6]
                ));
                let mut cmd_content = String::new();

                let cropped_w =
                    base_w * (1.0 - trans.right_layer.crop.left - trans.right_layer.crop.right);
                let cropped_h =
                    base_h * (1.0 - trans.right_layer.crop.top - trans.right_layer.crop.bottom);

                for k in 1..branch_frames {
                    let t_prev = (k - 1) as f64 / fps;
                    let t_curr = k as f64 / fps;
                    let t_mid = (t_prev + t_curr) / 2.0;

                    let branch_offset = RationalTime::from_frames(k, plan.frame_rate);
                    let timeline_time = trans.transition_range.start + branch_offset;
                    let clip_offset = timeline_time - trans.right_layer.timeline_range.start;

                    let pos_x = trans
                        .right_layer
                        .evaluate_property(KeyframeProperty::PositionX, clip_offset);
                    let pos_y = trans
                        .right_layer
                        .evaluate_property(KeyframeProperty::PositionY, clip_offset);
                    let scale_x = trans
                        .right_layer
                        .evaluate_property(KeyframeProperty::ScaleX, clip_offset);
                    let scale_y = trans
                        .right_layer
                        .evaluate_property(KeyframeProperty::ScaleY, clip_offset);

                    let mut render_w = (cropped_w * scale_x).round().max(2.0) as u32;
                    let mut render_h = (cropped_h * scale_y).round().max(2.0) as u32;
                    if !render_w.is_multiple_of(2) {
                        render_w += 1;
                    }
                    if !render_h.is_multiple_of(2) {
                        render_h += 1;
                    }

                    let top_left_x = ((base_w - render_w as f32) / 2.0 + pos_x).round() as i32;
                    let top_left_y = ((base_h - render_h as f32) / 2.0 + pos_y).round() as i32;

                    cmd_content.push_str(&format!(
                        "{:.6} overlay@ov_tr_r_{} x {};\n\
                         {:.6} overlay@ov_tr_r_{} y {};\n",
                        t_mid, idx, top_left_x, t_mid, idx, top_left_y,
                    ));
                }

                fs::write(&cmd_file, &cmd_content).map_err(|e| {
                    ExportError::IoError(format!("Failed to write right transition sendcmd: {e}"))
                })?;
                let escaped = escape_ffmpeg_filter_path(&cmd_file);
                guard.track(cmd_file.clone());
                Some((cmd_file, escaped))
            } else {
                None
            };

            transition_branches.push(TransitionBranchAutomation {
                transition_index: idx,
                left_command_file: left_cmd.as_ref().map(|(p, _)| p.clone()),
                escaped_left_command_file: left_cmd.map(|(_, e)| e),
                right_command_file: right_cmd.as_ref().map(|(p, _)| p.clone()),
                escaped_right_command_file: right_cmd.map(|(_, e)| e),
            });
        }

        // 4. Generate Audio Volume Envelopes
        let mut audio_layers = Vec::new();
        for (idx, audio) in plan.audio_layers.iter().enumerate() {
            if audio.muted || !layer_has_animated_volume(audio) {
                continue;
            }

            let env_file = temp_dir.join(format!(
                "env_audio_{}_{}_{}.raw",
                idx,
                pid,
                &random_nonce[..6]
            ));
            let dur_sec = audio.source_range.duration.to_seconds();
            let total_samples = (dur_sec * audio_sample_rate as f64).round() as usize;

            let file = File::create(&env_file).map_err(|e| {
                ExportError::IoError(format!("Failed to create audio envelope file: {e}"))
            })?;
            let mut writer = BufWriter::with_capacity(65536, file);

            for i in 0..total_samples {
                if i % 65536 == 0 {
                    if let Some(flag) = cancel_flag {
                        if flag.load(Ordering::SeqCst) {
                            return Err(ExportError::Cancelled);
                        }
                    }
                }

                let sample_sec = i as f64 / audio_sample_rate as f64;
                let clip_offset = RationalTime::from_seconds(sample_sec, plan.frame_rate);
                let timeline_time = audio.timeline_range.start + clip_offset;
                let gain = audio.effective_volume_at(timeline_time);

                writer.write_all(&gain.to_le_bytes()).map_err(|e| {
                    ExportError::IoError(format!("Failed to write audio envelope sample: {e}"))
                })?;
            }

            writer.flush().map_err(|e| {
                ExportError::IoError(format!("Failed to flush audio envelope: {e}"))
            })?;

            guard.track(env_file.clone());

            audio_layers.push(LayerAudioAutomation {
                audio_index: idx,
                envelope_file: env_file,
                sample_rate: audio_sample_rate,
                total_samples,
            });
        }

        Ok(Self {
            video_layers,
            transition_branches,
            audio_layers,
            guard,
        })
    }
}
