//! Phase 14 Export Automation Schedule
//! Evaluates dynamic keyframe properties per-frame in Rust and produces:
//! 1. FFmpeg `sendcmd` scripts with midpoint dispatch for video layers and transition branches.
//! 2. Streamed `f32le` audio gain envelopes applied via `amultiply` with the source channel count.
//! 3. RAII cleanup guard guaranteeing temporary file removal on success, failure, or cancellation.

use crate::modules::composition::{CompositionAudioLayer, CompositionPlan, CompositionVisualLayer};
use crate::modules::domain::time::FrameRate;
use crate::modules::domain::time::RationalTime;
use crate::modules::domain::KeyframeProperty;
use crate::modules::export::ExportError;
use crate::modules::media::ffmpeg::FFmpegService;
use std::collections::HashMap;
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

fn audio_sample_time(
    start: RationalTime,
    sample_index: usize,
    sample_rate: u32,
    timeline_rate: FrameRate,
) -> Result<RationalTime, ExportError> {
    fn gcd(mut a: u32, mut b: u32) -> u32 {
        while b != 0 {
            (a, b) = (b, a % b);
        }
        a
    }

    if sample_rate == 0 || timeline_rate.numerator == 0 {
        return Err(ExportError::IoError(
            "Cannot construct audio sample time with a zero rate".to_string(),
        ));
    }
    let common_rate = (timeline_rate.numerator / gcd(timeline_rate.numerator, sample_rate))
        .checked_mul(sample_rate)
        .ok_or_else(|| ExportError::IoError("Audio sample time rate overflow".to_string()))?;
    let start_ticks = (start.value as i128)
        .checked_mul(start.rate.denominator as i128)
        .and_then(|value| value.checked_mul((common_rate / start.rate.numerator) as i128))
        .ok_or_else(|| ExportError::IoError("Audio sample start overflow".to_string()))?;
    let sample_ticks = (sample_index as i128)
        .checked_mul((common_rate / sample_rate) as i128)
        .ok_or_else(|| ExportError::IoError("Audio sample offset overflow".to_string()))?;
    let ticks = start_ticks
        .checked_add(sample_ticks)
        .and_then(|value| i64::try_from(value).ok())
        .ok_or_else(|| ExportError::IoError("Audio sample time overflow".to_string()))?;
    Ok(RationalTime::new(ticks, FrameRate::new(common_rate, 1)))
}

/// Video layer automation schedule metadata
#[derive(Debug, Clone)]
pub struct LayerVideoAutomation {
    pub layer_index: usize,
    /// Scale, rotate and opacity commands: they run on the layer's own filter chain.
    pub command_file: PathBuf,
    pub escaped_command_file: String,
    /// `overlay` x/y commands. They must fire on the overlay's *main* input, which advances in
    /// lockstep with the overlay: fired from the layer chain they could run ahead of it by
    /// several frames whenever the overlay is the slower filter.
    pub overlay_command_file: PathBuf,
    pub escaped_overlay_command_file: String,
    /// Fixed square output of the layer's `rotate` filter, large enough for the
    /// largest scaled frame in the clip. FFmpeg evaluates rotate's `ow`/`oh` once,
    /// so a per-frame size would crop scaled-up frames and misplace the overlay.
    pub rotate_box: u32,
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
    pub channels: u16,
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

        let mut probed_audio_channels = HashMap::new();
        let mut audio_channels = Vec::with_capacity(plan.audio_layers.len());
        for audio in &plan.audio_layers {
            let needs_envelope = layer_has_animated_volume(audio)
                || plan.ducking_curves.contains_key(&audio.item_id)
                || plan.audio_crossfades.iter().any(|transition| {
                    transition.left_item_id == audio.item_id
                        || transition.right_item_id == audio.item_id
                });
            if audio.muted || !needs_envelope {
                audio_channels.push(2);
                continue;
            }

            let channels = match audio.asset_path.as_ref().filter(|path| path.is_file()) {
                Some(path) => {
                    if let Some(channels) = probed_audio_channels.get(path) {
                        *channels
                    } else {
                        let metadata = FFmpegService::probe_file(path).map_err(|error| {
                            ExportError::RenderFailed(format!(
                                "Failed to inspect audio channels for '{}': {error}",
                                path.display()
                            ))
                        })?;
                        let channels = metadata
                            .streams
                            .iter()
                            .find(|stream| stream.codec_type == "audio")
                            .and_then(|stream| stream.channels)
                            .unwrap_or(2)
                            .max(1);
                        probed_audio_channels.insert(path.clone(), channels);
                        channels
                    }
                }
                _ => 2,
            };
            audio_channels.push(channels);
        }

        // 1. Preflight check: estimate required audio envelope bytes
        let mut total_estimated_audio_bytes: u64 = 0;
        for (idx, audio) in plan.audio_layers.iter().enumerate() {
            let needs_envelope = layer_has_animated_volume(audio)
                || plan.ducking_curves.contains_key(&audio.item_id)
                || plan.audio_crossfades.iter().any(|transition| {
                    transition.left_item_id == audio.item_id
                        || transition.right_item_id == audio.item_id
                });
            if !audio.muted && needs_envelope {
                let (_, _, render_duration) = plan.audio_render_window(audio);
                let dur = render_duration.to_seconds();
                let samples = (dur * audio_sample_rate as f64).round() as u64;
                total_estimated_audio_bytes += samples * 4 * audio_channels[idx] as u64;
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
            let overlay_cmd_file = temp_dir.join(format!(
                "cmd_layer_ov_{}_{}_{}.txt",
                idx,
                pid,
                &random_nonce[..6]
            ));
            let mut cmd_content = String::new();
            let mut overlay_cmd_content = String::new();

            let start_frame = (ord_start_sec * fps).floor() as i64;
            let end_frame = (ord_end_sec * fps).ceil() as i64;

            let base_w = plan.width as f32;
            let base_h = plan.height as f32;

            // Evaluate every frame first: the rotate box must cover the largest
            // scaled frame before any overlay position can be computed against it.
            struct FrameSample {
                t_mid: f64,
                render_w: u32,
                render_h: u32,
                pos_x: f32,
                pos_y: f32,
                angle_rad: f32,
                opacity: f32,
            }
            let mut frames = Vec::new();
            // Include the start frame: it sets the filter's initial size.
            for n in start_frame..=end_frame {
                let t_prev = (n - 1) as f64 / fps;
                let t_curr = n as f64 / fps;
                let timeline_time = RationalTime::from_frames(n, plan.frame_rate);
                let clip_offset = timeline_time - layer.timeline_range.start;

                let scale_x = layer.evaluate_property(KeyframeProperty::ScaleX, clip_offset);
                let scale_y = layer.evaluate_property(KeyframeProperty::ScaleY, clip_offset);
                // Match the same source-aware Fit/Fill geometry used by the
                // static composition and native preview. Using canvas size
                // here stretches portrait/landscape sources when any visual
                // keyframe activates the runtime automation path.
                let (render_width, render_height) =
                    layer.rendered_size(base_w, base_h, scale_x, scale_y);
                let mut render_w = render_width.round().max(2.0) as u32;
                let mut render_h = render_height.round().max(2.0) as u32;
                if !render_w.is_multiple_of(2) {
                    render_w += 1;
                }
                if !render_h.is_multiple_of(2) {
                    render_h += 1;
                }
                frames.push(FrameSample {
                    t_mid: (t_prev + t_curr) / 2.0,
                    render_w,
                    render_h,
                    pos_x: layer.evaluate_property(KeyframeProperty::PositionX, clip_offset),
                    pos_y: layer.evaluate_property(KeyframeProperty::PositionY, clip_offset),
                    angle_rad: layer.evaluate_property(KeyframeProperty::Rotation, clip_offset)
                        * std::f32::consts::PI
                        / 180.0,
                    opacity: layer.effective_opacity_at(clip_offset),
                });
            }

            // The animated FFmpeg branch rotates into a fixed square box so runtime
            // angle/scale changes never renegotiate the overlay input size. Place
            // that box around the intended layer center.
            let mut rotate_box = frames
                .iter()
                .map(|f| (f.render_w as f32).hypot(f.render_h as f32).ceil() as u32)
                .max()
                .unwrap_or(2)
                .max(2);
            if !rotate_box.is_multiple_of(2) {
                rotate_box += 1;
            }

            // Emit runtime commands for frames n > start_frame using midpoint dispatch.
            for f in frames.iter().skip(1) {
                let top_left_x = ((base_w - rotate_box as f32) / 2.0 + f.pos_x).round() as i32;
                let top_left_y = ((base_h - rotate_box as f32) / 2.0 + f.pos_y).round() as i32;

                cmd_content.push_str(&format!(
                    "{:.6} scale@sc_{} w {};\n\
                     {:.6} scale@sc_{} h {};\n\
                     {:.6} rotate@rot_{} angle {:.4};\n\
                     {:.6} colorchannelmixer@ccm_{} aa {:.3};\n",
                    f.t_mid,
                    idx,
                    f.render_w,
                    f.t_mid,
                    idx,
                    f.render_h,
                    f.t_mid,
                    idx,
                    f.angle_rad,
                    f.t_mid,
                    idx,
                    f.opacity,
                ));
                overlay_cmd_content.push_str(&format!(
                    "{:.6} overlay@ov_{} x {};\n\
                     {:.6} overlay@ov_{} y {};\n",
                    f.t_mid, idx, top_left_x, f.t_mid, idx, top_left_y,
                ));
            }

            fs::write(&cmd_file, &cmd_content).map_err(|e| {
                ExportError::IoError(format!("Failed to write sendcmd script: {e}"))
            })?;

            fs::write(&overlay_cmd_file, &overlay_cmd_content).map_err(|e| {
                ExportError::IoError(format!("Failed to write overlay sendcmd script: {e}"))
            })?;

            let escaped = escape_ffmpeg_filter_path(&cmd_file);
            guard.track(cmd_file.clone());
            let escaped_overlay = escape_ffmpeg_filter_path(&overlay_cmd_file);
            guard.track(overlay_cmd_file.clone());

            video_layers.push(LayerVideoAutomation {
                layer_index: idx,
                command_file: cmd_file,
                escaped_command_file: escaped,
                overlay_command_file: overlay_cmd_file,
                escaped_overlay_command_file: escaped_overlay,
                rotate_box,
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

                    let (render_width, render_height) = trans
                        .left_layer
                        .rendered_size(base_w, base_h, scale_x, scale_y);
                    let mut render_w = render_width.round().max(2.0) as u32;
                    let mut render_h = render_height.round().max(2.0) as u32;
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

                    let (render_width, render_height) = trans
                        .right_layer
                        .rendered_size(base_w, base_h, scale_x, scale_y);
                    let mut render_w = render_width.round().max(2.0) as u32;
                    let mut render_h = render_height.round().max(2.0) as u32;
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
            let needs_envelope = layer_has_animated_volume(audio)
                || plan.ducking_curves.contains_key(&audio.item_id)
                || plan.audio_crossfades.iter().any(|transition| {
                    transition.left_item_id == audio.item_id
                        || transition.right_item_id == audio.item_id
                });
            if audio.muted || !needs_envelope {
                continue;
            }

            let env_file = temp_dir.join(format!(
                "env_audio_{}_{}_{}.raw",
                idx,
                pid,
                &random_nonce[..6]
            ));
            let (render_start, _, render_duration) = plan.audio_render_window(audio);
            let dur_sec = render_duration.to_seconds();
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

                let timeline_time =
                    audio_sample_time(render_start, i, audio_sample_rate, plan.frame_rate)?;
                let gain = plan.effective_audio_gain_at(audio, timeline_time);

                let gain_bytes = gain.to_le_bytes();
                for _ in 0..audio_channels[idx] {
                    writer.write_all(&gain_bytes).map_err(|e| {
                        ExportError::IoError(format!("Failed to write audio envelope sample: {e}"))
                    })?;
                }
            }

            writer.flush().map_err(|e| {
                ExportError::IoError(format!("Failed to flush audio envelope: {e}"))
            })?;

            guard.track(env_file.clone());

            audio_layers.push(LayerAudioAutomation {
                audio_index: idx,
                envelope_file: env_file,
                sample_rate: audio_sample_rate,
                channels: audio_channels[idx],
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

#[cfg(test)]
mod tests {
    use super::ExportAutomationSchedule;
    use crate::modules::composition::CompositionPlan;
    use crate::modules::domain::time::{FrameRate, RationalTime, TimeRange};
    use crate::modules::domain::{
        Asset, AssetDimensions, CutTransition, ItemKind, Keyframe, KeyframeProperty, MediaKind,
        Project, TimelineItem, Track, TrackKind, TransitionKind,
    };

    #[test]
    fn animated_portrait_fit_uses_source_aware_cropped_dimensions() {
        let rate = FrameRate::FPS_30;
        let mut project = Project::new("Animated portrait fit regression".to_string());
        project.assets.push(Asset {
            asset_id: "asset_portrait".to_string(),
            name: "portrait.mp4".to_string(),
            path: "portrait.mp4".to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::from_frames(60, rate)),
            file_size: 0,
            dimensions: Some(AssetDimensions {
                width: 1080,
                height: 1920,
            }),
        });

        let range = TimeRange::new(
            RationalTime::from_frames(0, rate),
            RationalTime::from_frames(60, rate),
        );
        let mut item = TimelineItem::new(
            Some("asset_portrait".to_string()),
            "Portrait clip".to_string(),
            ItemKind::Video,
            range,
            range,
        )
        .expect("valid portrait item");
        item.crop.top = 0.05;
        item.keyframes.push(Keyframe::new(
            KeyframeProperty::PositionX,
            RationalTime::from_frames(30, rate),
            40.0,
        ));
        let mut track = Track::new("Video".to_string(), TrackKind::Video);
        track.items.push(item);
        project.tracks.push(track);

        let plan = CompositionPlan::from_project(&project).expect("valid composition plan");
        let temp_dir = std::env::temp_dir().join(format!(
            "tinixcut_phase52_automation_geometry_{}",
            uuid::Uuid::new_v4()
        ));
        let schedule = ExportAutomationSchedule::build(&plan, &temp_dir, 48_000, None)
            .expect("automation schedule should build");
        assert_eq!(schedule.video_layers.len(), 1);

        let commands = std::fs::read_to_string(&schedule.video_layers[0].command_file)
            .expect("generated automation command file")
            + &std::fs::read_to_string(&schedule.video_layers[0].overlay_command_file)
                .expect("generated overlay command file");
        assert!(commands.contains("scale@sc_0 w 640;"), "{commands}");
        assert!(commands.contains("scale@sc_0 h 1080;"), "{commands}");
        assert!(commands.contains("overlay@ov_0 x 372;"), "{commands}");
        assert!(commands.contains("overlay@ov_0 y -88;"), "{commands}");

        drop(schedule);
        let _ = std::fs::remove_dir(temp_dir);
    }

    #[test]
    fn animated_scale_uses_one_rotate_box_for_the_largest_frame() {
        // G52 (2026-11-12): scale 1.0 -> 1.5 with PositionX 0 -> 300 exported the layer
        // cropped and near the center, because rotate's output size is fixed at the
        // first frame while overlay positions assumed a per-frame box.
        let rate = FrameRate::FPS_30;
        let mut project = Project::new("Animated scale rotate box regression".to_string());
        project.assets.push(Asset {
            asset_id: "asset_portrait".to_string(),
            name: "portrait.mp4".to_string(),
            path: "portrait.mp4".to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::from_frames(61, rate)),
            file_size: 0,
            dimensions: Some(AssetDimensions {
                width: 1080,
                height: 1920,
            }),
        });
        let range = TimeRange::new(
            RationalTime::from_frames(0, rate),
            RationalTime::from_frames(61, rate),
        );
        let mut item = TimelineItem::new(
            Some("asset_portrait".to_string()),
            "Portrait clip".to_string(),
            ItemKind::Video,
            range,
            range,
        )
        .expect("valid portrait item");
        for (property, start, end) in [
            (KeyframeProperty::ScaleX, 1.0, 1.5),
            (KeyframeProperty::ScaleY, 1.0, 1.5),
            (KeyframeProperty::PositionX, 0.0, 300.0),
        ] {
            item.keyframes.push(Keyframe::new(
                property,
                RationalTime::from_frames(0, rate),
                start,
            ));
            item.keyframes.push(Keyframe::new(
                property,
                RationalTime::from_frames(60, rate),
                end,
            ));
        }
        let mut track = Track::new("Video".to_string(), TrackKind::Video);
        track.items.push(item);
        project.tracks.push(track);

        let plan = CompositionPlan::from_project(&project).expect("valid composition plan");
        let layer = &plan.visual_layers[0];
        let even = |v: f32| {
            let r = v.round().max(2.0) as u32;
            r + r % 2
        };
        let (w, h) = layer.rendered_size(1920.0, 1080.0, 1.5, 1.5);
        let mut expected_box = (even(w) as f32).hypot(even(h) as f32).ceil() as u32;
        expected_box += expected_box % 2;

        let temp_dir =
            std::env::temp_dir().join(format!("tinixcut_g52_rotate_box_{}", uuid::Uuid::new_v4()));
        let schedule = ExportAutomationSchedule::build(&plan, &temp_dir, 48_000, None)
            .expect("automation schedule should build");
        let auto = &schedule.video_layers[0];
        assert_eq!(auto.rotate_box, expected_box, "box must fit the 1.5x frame");

        let commands = std::fs::read_to_string(&auto.command_file).expect("command file")
            + &std::fs::read_to_string(&auto.overlay_command_file).expect("overlay command file");
        let last_x = (1920 - expected_box as i32) / 2 + 300;
        let last_y = (1080 - expected_box as i32) / 2;
        assert!(
            commands.contains(&format!("overlay@ov_0 x {last_x};")),
            "{commands}"
        );
        assert!(
            commands.contains(&format!("overlay@ov_0 y {last_y};")),
            "{commands}"
        );
        // Every frame is positioned against the same box: at scale 1.0 (frame 1) the
        // x offset is still relative to the large box, not the small frame.
        let first_x = ((1920.0 - expected_box as f32) / 2.0 + 5.0).round() as i32;
        assert!(
            commands.contains(&format!("overlay@ov_0 x {first_x};")),
            "{commands}"
        );

        drop(schedule);
        let _ = std::fs::remove_dir(temp_dir);
    }

    #[test]
    fn animated_portrait_transition_branches_keep_centered_fit_placement() {
        let rate = FrameRate::FPS_30;
        let mut project = Project::new("Animated portrait transition placement regression".into());
        project.assets.push(Asset {
            asset_id: "asset_portrait_transition".into(),
            name: "portrait.mp4".into(),
            path: "portrait.mp4".into(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::from_frames(450, rate)),
            file_size: 0,
            dimensions: Some(AssetDimensions {
                width: 1080,
                height: 1920,
            }),
        });

        let mut left = TimelineItem::new(
            Some("asset_portrait_transition".into()),
            "Left portrait".into(),
            ItemKind::Video,
            TimeRange::new(
                RationalTime::from_frames(0, rate),
                RationalTime::from_frames(225, rate),
            ),
            TimeRange::new(
                RationalTime::from_frames(0, rate),
                RationalTime::from_frames(225, rate),
            ),
        )
        .expect("valid left clip");
        left.item_id = "transition_left_portrait".into();
        left.transform.position_x = 40.0;
        left.keyframes.push(Keyframe::new(
            KeyframeProperty::PositionX,
            RationalTime::from_frames(30, rate),
            40.0,
        ));

        let mut right = TimelineItem::new(
            Some("asset_portrait_transition".into()),
            "Right portrait".into(),
            ItemKind::Video,
            TimeRange::new(
                RationalTime::from_frames(225, rate),
                RationalTime::from_frames(225, rate),
            ),
            TimeRange::new(
                RationalTime::from_frames(225, rate),
                RationalTime::from_frames(225, rate),
            ),
        )
        .expect("valid right clip");
        right.item_id = "transition_right_portrait".into();
        right.transform.position_x = 40.0;
        right.keyframes.push(Keyframe::new(
            KeyframeProperty::PositionX,
            RationalTime::from_frames(30, rate),
            40.0,
        ));

        let mut track = Track::new("Video".into(), TrackKind::Video);
        track.track_id = "transition_portrait_track".into();
        track.items.extend([left, right]);
        track.transitions.push(CutTransition {
            transition_id: "portrait_cross_dissolve".into(),
            track_id: track.track_id.clone(),
            left_item_id: "transition_left_portrait".into(),
            right_item_id: "transition_right_portrait".into(),
            kind: TransitionKind::CrossDissolve,
            duration: RationalTime::from_frames(30, rate),
        });
        project.tracks.push(track);

        let plan = CompositionPlan::from_project(&project).expect("valid composition plan");
        let temp_dir = std::env::temp_dir().join(format!(
            "tinixcut_phase50_transition_geometry_{}",
            uuid::Uuid::new_v4()
        ));
        let schedule = ExportAutomationSchedule::build(&plan, &temp_dir, 48_000, None)
            .expect("automation schedule should build");
        let transition = &schedule.transition_branches[0];
        let left_commands = std::fs::read_to_string(
            transition
                .left_command_file
                .as_ref()
                .expect("animated left branch command file"),
        )
        .expect("left branch automation commands");
        let right_commands = std::fs::read_to_string(
            transition
                .right_command_file
                .as_ref()
                .expect("animated right branch command file"),
        )
        .expect("right branch automation commands");

        // 1080x1920 Fit on 1920x1080 becomes 608x1080 after even-width rounding.
        // The 40px position offset is relative to canvas center, matching static preview/export.
        assert!(
            left_commands.contains("overlay@ov_tr_l_0 x 696;"),
            "{left_commands}"
        );
        assert!(
            right_commands.contains("overlay@ov_tr_r_0 x 696;"),
            "{right_commands}"
        );
        assert!(
            !left_commands.contains("overlay@ov_tr_l_0 x 40;"),
            "{left_commands}"
        );
        assert!(
            !right_commands.contains("overlay@ov_tr_r_0 x 40;"),
            "{right_commands}"
        );

        drop(schedule);
        let _ = std::fs::remove_dir(temp_dir);
    }
}
