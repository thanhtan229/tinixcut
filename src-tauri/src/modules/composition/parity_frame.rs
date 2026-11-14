//! Parity frame: one frame of the composition rendered by the export compiler.
//!
//! The live preview draws video through the browser, whose downscaling and sub-pixel placement
//! differ from the FFmpeg export (G52). The parity frame is the export graph itself, run over a
//! one-frame window of the plan: the same `CompositionPlan` compiler, the same automation
//! schedule (`sendcmd` for keyframed layers) and the same filters, so the picture is the exported
//! picture, resolution aside. Nothing here is persisted; the result is a disposable cache.
//!
//! Window rule: every layer active at the target frame is rebased to start at the target with a
//! source start that is a couple of frames before the wanted source frame, so the input can be
//! seeked. Animated properties are baked to their value at the target (kept as two equal
//! keyframes, so the layer still takes the same keyframe-automation path as in the export).
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
/// Frames the windowed layer stays visible; the first one is the target.
const WINDOW_FRAMES: i64 = 2;

#[derive(Debug, Clone, PartialEq)]
pub enum ParityFrameError {
    /// The frame cannot be rendered this way; the live preview stays on screen. Not a failure.
    Unsupported(String),
    Render(String),
}

impl std::fmt::Display for ParityFrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported(reason) => write!(f, "Exact frame unavailable: {reason}"),
            Self::Render(error) => write!(f, "Exact frame failed: {error}"),
        }
    }
}

/// The one-frame plan plus what the renderer needs to start the inputs near the target.
#[derive(Debug, Clone)]
pub struct ParityFrameWindow {
    pub plan: CompositionPlan,
    pub target: RationalTime,
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

    let window = RationalTime::from_frames(WINDOW_FRAMES, rate);
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
        rebased.timeline_range = TimeRange::new(target, window);

        if layer.kind == ItemKind::Video {
            let source_time = layer.source_time_at(target);
            let seek_time = if source_time.to_seconds() > margin.to_seconds() {
                source_time.sub_exact(&margin)
            } else {
                RationalTime::new(0, source_time.rate)
            };
            let relative = source_time.sub_exact(&seek_time);
            let source_window = layer.speed.source_offset_for(window);
            rebased.source_range = TimeRange::new(relative, source_window);
            seeks.insert(layer.item_id.clone(), seek_time.to_seconds());
        }
        // A still image has one frame: it keeps its own source range.

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
            for at in [RationalTime::new(0, rate), window] {
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
            Some(self.target.to_seconds()),
        );
        let mut last = base;
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
/// returns the ids of the text layers it leaves out.
pub fn render_parity_frame_png(
    plan: &CompositionPlan,
    frame: i64,
    out_width: u32,
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
    let output = MediaBinaryResolver::command("ffmpeg")
        .map_err(|e| ParityFrameError::Render(e.to_string()))?
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| ParityFrameError::Render(format!("could not run FFmpeg: {e}")))?;
    drop(schedule);
    if output.stdout.len() < 64 {
        return Err(ParityFrameError::Render(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    Ok((output.stdout, window.text_item_ids))
}
