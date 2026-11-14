//! Parity frame: one frame of the composition rendered by the export compiler.
//!
//! The live preview draws video through the browser, whose downscaling and sub-pixel placement
//! differ from the FFmpeg export (G52). The parity frame is the export graph itself, run over a
//! one-frame window of the plan: the same `CompositionPlan` compiler, the same automation
//! schedule (`sendcmd` for keyframed layers) and the same filters, so the picture is the exported
//! picture, resolution aside. Nothing here is persisted; the result is a disposable cache.
//!
//! Window rule: the window starts one frame before the target (the "lead" frame) and the target
//! is the second frame that comes out. A keyframed layer is driven by `sendcmd` commands that
//! reach the overlay for frame n+1 while frame n may still be in flight, and its first frame uses
//! the filter's initial state instead of a command; with the lead frame the target is always a
//! commanded frame whose neighbours carry the same values, so the result cannot depend on that
//! timing. Layers that start inside the window are left untouched (their first frame is the
//! export's first frame too). Older layers are rebased to start at the window with a source start
//! a couple of frames before the wanted source frame, so the input can be seeked; their animated
//! properties are baked to the value at the target and kept as equal keyframes, so they still take
//! the keyframe-automation path of the export.
//!
//! Not supported (the caller keeps the live preview): a frame inside a cut transition, reversed
//! clips, missing media, and text that is drawn below a video layer (text is not part of the
//! parity frame; the preview keeps drawing it on top).

use super::{CompositionPlan, CompositionVisualLayer, KeyframeProperty};
use crate::modules::domain::evaluator::evaluate_property_channel;
use crate::modules::domain::time::{RationalTime, TimeRange};
use crate::modules::domain::{ItemKind, Keyframe};
use crate::modules::export::ExportAutomationSchedule;
use crate::modules::media::resolver::MediaBinaryResolver;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;

/// Frames of margin before the wanted source frame, so an accurate input seek lands before it.
const SEEK_MARGIN_FRAMES: i64 = 2;
/// Frames after the target that the windowed layers stay visible.
const TRAIL_FRAMES: i64 = 1;

#[derive(Debug, Clone, PartialEq)]
pub enum ParityFrameError {
    /// The frame cannot be rendered this way; the live preview stays on screen. Not a failure.
    Unsupported(String),
    /// A newer request replaced this one before it finished; its FFmpeg process was stopped.
    Superseded,
    Render(String),
}

impl std::fmt::Display for ParityFrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported(reason) => write!(f, "Exact frame unavailable: {reason}"),
            Self::Superseded => write!(f, "Exact frame superseded by a newer request"),
            Self::Render(error) => write!(f, "Exact frame failed: {error}"),
        }
    }
}

/// The one-frame plan plus what the renderer needs to start the inputs near the target.
#[derive(Debug, Clone)]
pub struct ParityFrameWindow {
    pub plan: CompositionPlan,
    pub target: RationalTime,
    /// First frame of the window (one before the target, except at frame 0).
    pub window_start: RationalTime,
    /// Index of the target among the frames the window produces.
    pub output_index: i64,
    /// Input seek in seconds per item id (video layers only).
    pub seeks: HashMap<String, f64>,
    /// Text layers active at the target; the preview keeps drawing them above the frame.
    pub text_item_ids: Vec<String>,
}

fn is_text(layer: &CompositionVisualLayer) -> bool {
    layer.kind == ItemKind::Text
}

/// Rebases `plan` to a one-frame window at `frame`. See the module documentation.
pub fn window_plan(
    plan: &CompositionPlan,
    frame: i64,
) -> Result<ParityFrameWindow, ParityFrameError> {
    let rate = plan.frame_rate;
    let target = RationalTime::new(frame, rate);
    if plan.transitions.iter().any(|t| t.contains_time(target)) {
        return Err(ParityFrameError::Unsupported(
            "the frame is inside a transition".into(),
        ));
    }

    let lead_frames: i64 = if frame >= 1 { 1 } else { 0 };
    let window_start = RationalTime::new(frame - lead_frames, rate);
    let window = RationalTime::from_frames(lead_frames + 1 + TRAIL_FRAMES, rate);
    let window_end = window_start.add_exact(&window);
    let margin = RationalTime::from_frames(SEEK_MARGIN_FRAMES, rate);
    let mut layers: Vec<CompositionVisualLayer> = Vec::new();
    let mut seeks = HashMap::new();
    let mut text_item_ids = Vec::new();
    let mut lowest_text_z: Option<usize> = None;

    for layer in &plan.visual_layers {
        if !layer.timeline_range.contains_time(target) {
            continue;
        }
        if is_text(layer) {
            text_item_ids.push(layer.item_id.clone());
            lowest_text_z = Some(lowest_text_z.map_or(layer.z_index, |z| z.min(layer.z_index)));
            continue;
        }
        if layer.reverse {
            return Err(ParityFrameError::Unsupported(
                "a reversed clip is active".into(),
            ));
        }
        let Some(path) = layer.asset_path.as_ref() else {
            return Err(ParityFrameError::Unsupported("a clip has no media".into()));
        };
        if !path.exists() {
            return Err(ParityFrameError::Unsupported("media is missing".into()));
        }

        let offset = target.sub_exact(&layer.timeline_range.start);
        let mut rebased = layer.clone();
        // A layer that begins exactly at the target would show the filter's initial state, which
        // is not a commanded frame and can differ by a pixel from run to run. When the source has
        // a frame before the start, give it a lead frame too (its content there is discarded).
        let lead_duration = RationalTime::from_frames(lead_frames, rate);
        let begins_at_target = lead_frames > 0
            && layer.timeline_range.start.to_seconds() > window_start.to_seconds() + 1e-9;
        let pre_roll_source = if begins_at_target {
            let before = layer
                .source_range
                .start
                .sub_exact(&layer.speed.source_offset_for(lead_duration));
            (before.to_seconds() >= 0.0 || layer.kind == ItemKind::Image).then_some(before)
        } else {
            None
        };
        // Everything is baked (constant values around the target) unless the target itself would
        // be a layer's first frame: at frame 0, or for a clip that begins at the target with no
        // source before its start. Those keep the export's own first-frame behaviour.
        let starts_inside = lead_frames == 0 || (begins_at_target && pre_roll_source.is_none());
        let layer_start = if starts_inside {
            layer.timeline_range.start
        } else {
            window_start
        };
        let layer_window = window_end.sub_exact(&layer_start);
        rebased.timeline_range = TimeRange::new(layer_start, layer_window);

        if layer.kind == ItemKind::Video {
            let source_start = if starts_inside {
                layer.source_range.start
            } else if let Some(before) = pre_roll_source {
                before
            } else {
                layer.source_time_at(window_start)
            };
            let seek_time = if source_start.to_seconds() > margin.to_seconds() {
                source_start.sub_exact(&margin)
            } else {
                RationalTime::new(0, source_start.rate)
            };
            let relative = source_start.sub_exact(&seek_time);
            let source_window = layer.speed.source_offset_for(layer_window);
            rebased.source_range = TimeRange::new(relative, source_window);
            seeks.insert(layer.item_id.clone(), seek_time.to_seconds());
        }
        // A still image has one frame: it keeps its own source range.

        if starts_inside {
            // Same keyframes, transform and fades as the export: nothing to bake.
            layers.push(rebased);
            continue;
        }

        // Bake animated properties at the target. An animated layer keeps two equal keyframes
        // per property so it takes the same automation path as in the export.
        let mut properties: Vec<KeyframeProperty> = Vec::new();
        for keyframe in &layer.keyframes {
            if !properties.contains(&keyframe.property) {
                properties.push(keyframe.property);
            }
        }
        rebased.keyframes.clear();
        for property in properties {
            let base = match property {
                KeyframeProperty::Opacity => layer.opacity,
                KeyframeProperty::PositionX => layer.transform.position_x,
                KeyframeProperty::PositionY => layer.transform.position_y,
                KeyframeProperty::ScaleX => layer.transform.scale_x,
                KeyframeProperty::ScaleY => layer.transform.scale_y,
                KeyframeProperty::Rotation => layer.transform.rotation,
                KeyframeProperty::Volume => 1.0,
            };
            let value = if property == KeyframeProperty::Opacity {
                layer.effective_opacity_at(offset)
            } else {
                evaluate_property_channel(&layer.keyframes, property, base, offset)
            };
            match property {
                KeyframeProperty::PositionX => rebased.transform.position_x = value,
                KeyframeProperty::PositionY => rebased.transform.position_y = value,
                KeyframeProperty::ScaleX => rebased.transform.scale_x = value,
                KeyframeProperty::ScaleY => rebased.transform.scale_y = value,
                KeyframeProperty::Rotation => rebased.transform.rotation = value,
                KeyframeProperty::Opacity => rebased.opacity = value,
                KeyframeProperty::Volume => {}
            }
            for at in [RationalTime::new(0, rate), layer_window] {
                rebased.keyframes.push(Keyframe::new(property, at, value));
            }
        }
        // Fades are baked into the opacity at the target.
        if layer.transition.is_some() {
            rebased.opacity = layer.effective_opacity_at(offset);
            rebased.transition = None;
        }
        layers.push(rebased);
    }

    if let Some(text_z) = lowest_text_z {
        if layers.iter().any(|layer| layer.z_index > text_z) {
            return Err(ParityFrameError::Unsupported(
                "text sits below a video layer".into(),
            ));
        }
    }

    let mut windowed = plan.clone();
    windowed.visual_layers = layers;
    windowed.audio_layers.clear();
    windowed.audio_crossfades.clear();
    windowed.transitions.clear();
    windowed.duration = window;
    Ok(ParityFrameWindow {
        plan: windowed,
        target,
        window_start,
        output_index: lead_frames,
        seeks,
        text_item_ids,
    })
}

impl ParityFrameWindow {
    /// FFmpeg arguments that render the target frame as PNG on stdout, `out_width` pixels wide.
    pub fn ffmpeg_args(
        &self,
        schedule: Option<&ExportAutomationSchedule>,
        out_width: u32,
    ) -> Vec<String> {
        let plan = &self.plan;
        let mut args: Vec<String> = vec!["-hide_banner".into(), "-loglevel".into(), "error".into()];
        let mut input_paths: Vec<PathBuf> = Vec::new();
        let mut layer_input_indices: HashMap<String, usize> = HashMap::new();
        for layer in &plan.visual_layers {
            if let Some(path) = &layer.asset_path {
                layer_input_indices.insert(layer.item_id.clone(), input_paths.len());
                input_paths.push(path.clone());
                if let Some(seek) = self.seeks.get(&layer.item_id).filter(|s| **s > 0.0) {
                    args.push("-ss".into());
                    args.push(format!("{seek:.6}"));
                }
                args.push("-i".into());
                args.push(path.to_string_lossy().into_owned());
            }
        }
        let (mut filter_complex, base) = plan.build_video_filtergraph_with_layer_inputs(
            &input_paths,
            plan.duration.to_seconds(),
            None,
            schedule,
            &[],
            Some(&layer_input_indices),
            Some(self.window_start.to_seconds()),
        );
        let mut last = base;
        // Keep only the target frame (the frames before it are the lead-in).
        filter_complex.push_str(&format!(
            "[{last}]trim=start_frame={0}:end_frame={1},setpts=PTS-STARTPTS[parity_frame];",
            self.output_index,
            self.output_index + 1
        ));
        last = "parity_frame".into();
        if out_width != plan.width {
            let out_height = ((f64::from(plan.height) * f64::from(out_width)
                / f64::from(plan.width))
            .round() as u32)
                .max(1);
            filter_complex.push_str(&format!(
                "[{last}]scale={out_width}:{out_height}:flags=bicubic[parity_out];"
            ));
            last = "parity_out".into();
        }
        args.extend([
            "-filter_complex".into(),
            filter_complex,
            "-map".into(),
            format!("[{last}]"),
            "-frames:v".into(),
            "1".into(),
            "-f".into(),
            "image2pipe".into(),
            "-c:v".into(),
            "png".into(),
            // Fast, lightly compressed PNG: this is a display copy, not an archive.
            "-compression_level".into(),
            "1".into(),
            "pipe:1".into(),
        ]);
        args
    }
}

/// Removes the scratch directory when dropped.
struct ScratchDir(PathBuf);

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Renders the parity frame of `plan` at `frame` as PNG bytes, `out_width` pixels wide, and
/// returns the ids of the text layers it leaves out. `superseded` is polled while FFmpeg runs;
/// when it returns true the process is killed and `Superseded` is returned.
pub fn render_parity_frame_png(
    plan: &CompositionPlan,
    frame: i64,
    out_width: u32,
    superseded: &dyn Fn() -> bool,
) -> Result<(Vec<u8>, Vec<String>), ParityFrameError> {
    let window = window_plan(plan, frame)?;
    if window.plan.visual_layers.is_empty() {
        return Err(ParityFrameError::Unsupported("nothing is visible".into()));
    }
    let dir = std::env::temp_dir().join(format!(
        "tc_parity_{}_{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&dir).map_err(|e| ParityFrameError::Render(e.to_string()))?;
    let _scratch = ScratchDir(dir.clone());
    let schedule =
        ExportAutomationSchedule::build(&window.plan, &dir, window.plan.sample_rate, None)
            .map_err(|e| ParityFrameError::Render(e.to_string()))?;
    let args = window.ffmpeg_args(Some(&schedule), out_width.clamp(16, window.plan.width));
    let mut child = MediaBinaryResolver::command("ffmpeg")
        .map_err(|e| ParityFrameError::Render(e.to_string()))?
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| ParityFrameError::Render(format!("could not run FFmpeg: {e}")))?;
    // Drain both pipes on threads: the PNG is larger than a pipe buffer.
    let mut stdout = child.stdout.take().expect("stdout is piped");
    let mut stderr = child.stderr.take().expect("stderr is piped");
    let png_reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stdout, &mut bytes);
        bytes
    });
    let error_reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stderr, &mut bytes);
        bytes
    });
    loop {
        if superseded() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = png_reader.join();
            let _ = error_reader.join();
            return Err(ParityFrameError::Superseded);
        }
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(10)),
            Err(error) => return Err(ParityFrameError::Render(error.to_string())),
        }
    }
    drop(schedule);
    let png = png_reader.join().unwrap_or_default();
    let log = error_reader.join().unwrap_or_default();
    if png.len() < 64 {
        return Err(ParityFrameError::Render(
            String::from_utf8_lossy(&log).trim().to_string(),
        ));
    }
    Ok((png, window.text_item_ids))
}
