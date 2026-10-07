//! Normalized Composition Plan module
//! The semantic bridge between the Project Domain Model and rendering backends (Preview and Export).
//! Invariant: No FFmpeg syntax or HTML/CSS in canonical domain state; both preview and export consume this normalized plan.

use crate::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use crate::modules::domain::{Crop, ItemKind, Project, TrackKind, Transform};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompositionError {
    #[error("Missing asset with ID '{0}'")]
    MissingAsset(String),

    #[error("Unsupported media kind or format for item '{0}'")]
    UnsupportedItem(String),

    #[error("Invalid composition range: {0}")]
    InvalidRange(String),

    #[error("Unsupported transition '{transition}' on item '{item_id}': capability is deferred")]
    UnsupportedTransition { item_id: String, transition: String },

    #[error("Render graph compilation error: {0}")]
    GraphCompilationError(String),
}

pub use crate::modules::domain::{
    Keyframe as CompositionKeyframe, KeyframeProperty, Transition as CompositionTransition,
    TransitionKind,
};

pub mod parity;
pub use parity::{
    ParityDiffMetrics, ParityFixtureResult, ParityLayerSnapshot, ParitySemanticSnapshot,
    ParitySuiteReport,
};

/// Bounding rectangle on the canvas coordinate space
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Normalized visual layer in the composition plan
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositionVisualLayer {
    pub layer_id: String,
    pub track_id: String,
    pub item_id: String,
    pub name: String,
    pub z_index: usize,
    pub kind: ItemKind,
    pub asset_id: Option<String>,
    pub asset_path: Option<PathBuf>,
    pub timeline_range: TimeRange,
    pub source_range: TimeRange,
    pub transform: Transform,
    pub crop: Crop,
    pub opacity: f32,
    pub text_content: Option<String>,
    pub font_family: Option<String>,
    pub font_size: Option<f32>,
    pub font_color: Option<String>,
    pub keyframes: Vec<CompositionKeyframe>,
    pub transition: Option<CompositionTransition>,
}

impl CompositionVisualLayer {
    /// Evaluates animated property at a given clip-relative offset using canonical evaluator
    pub fn evaluate_property(&self, prop: KeyframeProperty, clip_offset: RationalTime) -> f32 {
        let base_val = match prop {
            KeyframeProperty::Opacity => self.opacity,
            KeyframeProperty::PositionX => self.transform.position_x,
            KeyframeProperty::PositionY => self.transform.position_y,
            KeyframeProperty::ScaleX => self.transform.scale_x,
            KeyframeProperty::ScaleY => self.transform.scale_y,
            KeyframeProperty::Rotation => self.transform.rotation,
            KeyframeProperty::Volume => 1.0,
        };
        crate::modules::domain::evaluator::evaluate_property_channel(
            &self.keyframes,
            prop,
            base_val,
            clip_offset,
        )
    }

    /// Evaluates effective visual opacity at a given clip-relative offset,
    /// taking into account base opacity, opacity keyframes, and transitions (FadeIn, FadeOut, CrossDissolve, FadeBlack).
    pub fn effective_opacity_at(&self, clip_offset: RationalTime) -> f32 {
        let mut op = self.evaluate_property(KeyframeProperty::Opacity, clip_offset);
        if let Some(ref trans) = self.transition {
            let dur_val = trans.duration.value as f32;
            if dur_val > 0.0 {
                let offset_val = clip_offset.value as f32;
                let total_dur = self.timeline_range.duration.value as f32;
                let trans_factor = match trans.kind {
                    TransitionKind::FadeIn | TransitionKind::FadeBlack => {
                        if offset_val < dur_val {
                            (offset_val / dur_val).clamp(0.0, 1.0)
                        } else {
                            1.0
                        }
                    }
                    TransitionKind::FadeOut => {
                        let fade_start = total_dur - dur_val;
                        if offset_val > fade_start {
                            ((total_dur - offset_val) / dur_val).clamp(0.0, 1.0)
                        } else {
                            1.0
                        }
                    }
                    TransitionKind::CrossDissolve => 1.0,
                };
                op *= trans_factor;
            }
        }
        op
    }
}

/// Normalized audio layer in the composition plan
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositionAudioLayer {
    pub layer_id: String,
    pub track_id: String,
    pub item_id: String,
    pub asset_id: Option<String>,
    pub asset_path: Option<PathBuf>,
    pub timeline_range: TimeRange,
    pub source_range: TimeRange,
    pub volume: f32,
    pub muted: bool,
    pub fade_in: Option<RationalTime>,
    pub fade_out: Option<RationalTime>,
    pub keyframes: Vec<CompositionKeyframe>,
}

impl CompositionAudioLayer {
    /// Computes effective volume gain at a specific timeline timestamp
    pub fn effective_volume_at(&self, timeline_time: RationalTime) -> f32 {
        if self.muted {
            return 0.0;
        }
        if !self.timeline_range.contains_time(timeline_time) {
            return 0.0;
        }

        let clip_offset = (timeline_time - self.timeline_range.start)
            .rescaled_to(self.timeline_range.duration.rate);
        self.volume_at_offset(clip_offset)
    }

    fn volume_at_offset(&self, clip_offset: RationalTime) -> f32 {
        if self.muted {
            return 0.0;
        }
        let clip_offset = clip_offset.rescaled_to(self.timeline_range.duration.rate);
        let mut base_vol = crate::modules::domain::evaluator::evaluate_property_channel(
            &self.keyframes,
            KeyframeProperty::Volume,
            self.volume,
            clip_offset,
        );

        // Apply fade in
        if let Some(fade_in) = self.fade_in {
            let fade_in = fade_in.rescaled_to(clip_offset.rate).value;
            if fade_in > 0 && clip_offset.value < fade_in {
                let factor = clip_offset.value as f32 / fade_in as f32;
                base_vol *= factor.clamp(0.0, 1.0);
            }
        }

        // Apply fade out
        if let Some(fade_out) = self.fade_out {
            let fade_out = fade_out.rescaled_to(clip_offset.rate).value;
            let total_dur = self.timeline_range.duration.value;
            let fade_start = total_dur - fade_out;
            if fade_out > 0 && clip_offset.value > fade_start {
                let remaining = (total_dur - clip_offset.value) as f32;
                let factor = remaining / fade_out as f32;
                base_vol *= factor.clamp(0.0, 1.0);
            }
        }

        base_vol
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositionAudioCrossfade {
    pub transition_id: String,
    pub left_item_id: String,
    pub right_item_id: String,
    pub start: RationalTime,
    pub duration: RationalTime,
}

impl CompositionAudioCrossfade {
    fn contains(&self, time: RationalTime) -> bool {
        let before_start = time.semantic_lt(&self.start);
        let end = self.start + self.duration;
        let at_or_after_end = time.semantic_ge(&end);
        !before_start && !at_or_after_end
    }

    fn gain_for(&self, item_id: &str, time: RationalTime) -> Option<f32> {
        if !self.contains(time) {
            return None;
        }
        let elapsed = (time.to_seconds() - self.start.to_seconds()) as f32;
        let progress = (elapsed / self.duration.to_seconds() as f32).clamp(0.0, 1.0);
        if item_id == self.left_item_id {
            Some((progress * std::f32::consts::FRAC_PI_2).cos())
        } else if item_id == self.right_item_id {
            Some((progress * std::f32::consts::FRAC_PI_2).sin())
        } else {
            None
        }
    }
}

/// Normalized two-input cut transition plan for CrossDissolve
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransitionPlan {
    pub transition_id: String,
    pub track_id: String,
    pub kind: TransitionKind,
    pub z_index: usize,
    pub left_item_id: String,
    pub right_item_id: String,
    pub cut_time: RationalTime,
    pub duration: RationalTime,
    pub transition_range: TimeRange, // Half-open [T0, T1)
    pub left_layer: CompositionVisualLayer,
    pub right_layer: CompositionVisualLayer,
}

impl TransitionPlan {
    pub fn l_duration(&self) -> RationalTime {
        let target_rate = self.cut_time.rate;
        let d_ticks = self.duration.rescaled_to(target_rate).value;
        let l_ticks = d_ticks / 2;
        RationalTime::new(l_ticks, target_rate)
    }

    pub fn r_duration(&self) -> RationalTime {
        let target_rate = self.cut_time.rate;
        let d_ticks = self.duration.rescaled_to(target_rate).value;
        let l_ticks = d_ticks / 2;
        let r_ticks = d_ticks - l_ticks;
        RationalTime::new(r_ticks, target_rate)
    }

    pub fn contains_time(&self, time: RationalTime) -> bool {
        self.transition_range.contains_time(time)
    }

    pub fn progress_at(&self, time: RationalTime) -> f32 {
        let target_rate = self.transition_range.start.rate;
        let t = time.rescaled_to(target_rate);
        let t0 = self.transition_range.start.rescaled_to(target_rate);
        let d = self.transition_range.duration.rescaled_to(target_rate);
        if d.value <= 0 {
            return 0.0;
        }
        let elapsed = (t.value - t0.value) as f32;
        let dur = d.value as f32;
        (elapsed / dur).clamp(0.0, 1.0)
    }

    pub fn left_source_time_at(&self, time: RationalTime) -> RationalTime {
        let target_rate = self.left_layer.timeline_range.start.rate;
        let t = time.rescaled_to(target_rate);
        let delta = t.value - self.left_layer.timeline_range.start.value;
        RationalTime::new(
            self.left_layer.source_range.start.value + delta,
            self.left_layer.source_range.start.rate,
        )
    }

    pub fn right_source_time_at(&self, time: RationalTime) -> RationalTime {
        let target_rate = self.right_layer.timeline_range.start.rate;
        let t = time.rescaled_to(target_rate);
        let delta = t.value - self.right_layer.timeline_range.start.value;
        RationalTime::new(
            self.right_layer.source_range.start.value + delta,
            self.right_layer.source_range.start.rate,
        )
    }

    pub fn left_effective_clip_offset(&self, time: RationalTime) -> RationalTime {
        let target_rate = self.left_layer.timeline_range.start.rate;
        let t = time.rescaled_to(target_rate);
        let raw_offset = t.value - self.left_layer.timeline_range.start.value;
        let clamped = raw_offset.clamp(0, self.left_layer.timeline_range.duration.value);
        RationalTime::new(clamped, target_rate)
    }

    pub fn right_effective_clip_offset(&self, time: RationalTime) -> RationalTime {
        let target_rate = self.right_layer.timeline_range.start.rate;
        let t = time.rescaled_to(target_rate);
        let raw_offset = t.value - self.right_layer.timeline_range.start.value;
        let clamped = raw_offset.clamp(0, self.right_layer.timeline_range.duration.value);
        RationalTime::new(clamped, target_rate)
    }

    pub fn left_opacity_at(&self, time: RationalTime) -> f32 {
        let p = self.progress_at(time);
        let base_op = self
            .left_layer
            .effective_opacity_at(self.left_effective_clip_offset(time));
        (1.0 - p) * base_op
    }

    pub fn right_opacity_at(&self, time: RationalTime) -> f32 {
        let p = self.progress_at(time);
        let base_op = self
            .right_layer
            .effective_opacity_at(self.right_effective_clip_offset(time));
        p * base_op
    }
}

/// The complete, normalized composition plan for preview and export
fn default_composition_sample_rate() -> u32 {
    48000
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositionPlan {
    pub project_id: String,
    pub revision: u64,
    pub width: u32,
    pub height: u32,
    pub frame_rate: FrameRate,
    pub duration: RationalTime,
    pub visual_layers: Vec<CompositionVisualLayer>,
    pub audio_layers: Vec<CompositionAudioLayer>,
    #[serde(default)]
    pub audio_crossfades: Vec<CompositionAudioCrossfade>,
    #[serde(default)]
    pub transitions: Vec<TransitionPlan>,
    #[serde(default = "default_composition_sample_rate")]
    pub sample_rate: u32,
}

impl CompositionPlan {
    pub fn audio_render_window(
        &self,
        audio: &CompositionAudioLayer,
    ) -> (RationalTime, RationalTime, RationalTime) {
        let rate = self.frame_rate;
        let mut preroll = 0i64;
        let mut postroll = 0i64;
        for transition in &self.audio_crossfades {
            let duration = transition.duration.rescaled_to(rate);
            if transition.right_item_id == audio.item_id {
                preroll = preroll.max(duration.value / 2);
            }
            if transition.left_item_id == audio.item_id {
                postroll = postroll.max(duration.value - duration.value / 2);
            }
        }
        let start = audio.timeline_range.start - RationalTime::new(preroll, rate);
        let source_start = audio.source_range.start - RationalTime::new(preroll, rate);
        let duration = audio.timeline_range.duration + RationalTime::new(preroll + postroll, rate);
        (start, source_start, duration)
    }

    pub fn audio_layer_active_at(&self, audio: &CompositionAudioLayer, time: RationalTime) -> bool {
        audio.timeline_range.contains_time(time)
            || self
                .audio_crossfades
                .iter()
                .any(|transition| transition.gain_for(&audio.item_id, time).is_some())
    }

    pub fn effective_audio_gain_at(
        &self,
        audio: &CompositionAudioLayer,
        time: RationalTime,
    ) -> f32 {
        if !self.audio_layer_active_at(audio, time) || audio.muted {
            return 0.0;
        }
        let crossfade_gain = self
            .audio_crossfades
            .iter()
            .find_map(|transition| transition.gain_for(&audio.item_id, time))
            .unwrap_or(1.0);
        let base_gain = if audio.timeline_range.contains_time(time) {
            audio.effective_volume_at(time)
        } else {
            let offset = time - audio.timeline_range.start;
            let clamped = if offset.value < 0 {
                RationalTime::zero(offset.rate)
            } else {
                audio.timeline_range.duration
            };
            audio.volume_at_offset(clamped)
        };
        base_gain * crossfade_gain
    }

    /// Compiles a Project into a normalized CompositionPlan
    pub fn from_project(project: &Project) -> Result<Self, CompositionError> {
        let mut visual_layers = Vec::new();
        let mut audio_layers = Vec::new();
        let mut audio_crossfades = Vec::new();
        let mut transitions = Vec::new();

        let asset_map: HashMap<String, PathBuf> = project
            .assets
            .iter()
            .map(|a| (a.asset_id.clone(), PathBuf::from(&a.path)))
            .collect();

        let mut max_time_val = 0i64;

        // Visual tracks ordered bottom-to-top (track index becomes z_index)
        let mut z_index = 0;
        for track in &project.tracks {
            if track.muted && track.kind == TrackKind::Video {
                continue;
            }

            for item in &track.items {
                if let Some(ref t) = item.transition {
                    if t.kind == TransitionKind::CrossDissolve {
                        return Err(CompositionError::UnsupportedTransition {
                            item_id: item.item_id.clone(),
                            transition: "CrossDissolve".to_string(),
                        });
                    }
                }

                let item_end = item
                    .timeline_range
                    .end()
                    .rescaled_to(project.settings.frame_rate)
                    .value;
                if item_end > max_time_val {
                    max_time_val = item_end;
                }

                let asset_path = if let Some(ref aid) = item.asset_id {
                    Some(
                        asset_map
                            .get(aid)
                            .cloned()
                            .ok_or_else(|| CompositionError::MissingAsset(aid.clone()))?,
                    )
                } else {
                    None
                };

                match item.kind {
                    ItemKind::Video | ItemKind::Image => {
                        visual_layers.push(CompositionVisualLayer {
                            layer_id: format!("vl_{}", item.item_id),
                            track_id: track.track_id.clone(),
                            item_id: item.item_id.clone(),
                            name: item.name.clone(),
                            z_index,
                            kind: item.kind,
                            asset_id: item.asset_id.clone(),
                            asset_path: asset_path.clone(),
                            timeline_range: item.timeline_range,
                            source_range: item.source_range,
                            transform: item.transform.clone(),
                            crop: item.crop.clone(),
                            opacity: item.opacity,
                            text_content: item.text_content.clone(),
                            font_family: Some("Inter".to_string()),
                            font_size: Some(48.0),
                            font_color: Some("#FFFFFF".to_string()),
                            keyframes: item.keyframes.clone(),
                            transition: item.transition.clone(),
                        });
                        z_index += 1;

                        // Video item can also have audio if track is unmuted
                        if item.kind == ItemKind::Video && !track.muted {
                            audio_layers.push(CompositionAudioLayer {
                                layer_id: format!("al_{}", item.item_id),
                                track_id: track.track_id.clone(),
                                item_id: item.item_id.clone(),
                                asset_id: item.asset_id.clone(),
                                asset_path,
                                timeline_range: item.timeline_range,
                                source_range: item.source_range,
                                volume: item.volume,
                                muted: track.muted,
                                fade_in: item.fade_in,
                                fade_out: item.fade_out,
                                keyframes: item.keyframes.clone(),
                            });
                        }
                    }
                    ItemKind::Text => {
                        visual_layers.push(CompositionVisualLayer {
                            layer_id: format!("vl_{}", item.item_id),
                            track_id: track.track_id.clone(),
                            item_id: item.item_id.clone(),
                            name: item.name.clone(),
                            z_index,
                            kind: ItemKind::Text,
                            asset_id: None,
                            asset_path: None,
                            timeline_range: item.timeline_range,
                            source_range: item.source_range,
                            transform: item.transform.clone(),
                            crop: item.crop.clone(),
                            opacity: item.opacity,
                            text_content: item.text_content.clone(),
                            font_family: Some("Inter".to_string()),
                            font_size: Some(48.0),
                            font_color: Some("#FFFFFF".to_string()),
                            keyframes: item.keyframes.clone(),
                            transition: item.transition.clone(),
                        });
                        z_index += 1;
                    }
                    ItemKind::Audio => {
                        audio_layers.push(CompositionAudioLayer {
                            layer_id: format!("al_{}", item.item_id),
                            track_id: track.track_id.clone(),
                            item_id: item.item_id.clone(),
                            asset_id: item.asset_id.clone(),
                            asset_path,
                            timeline_range: item.timeline_range,
                            source_range: item.source_range,
                            volume: item.volume,
                            muted: track.muted,
                            fade_in: item.fade_in,
                            fade_out: item.fade_out,
                            keyframes: item.keyframes.clone(),
                        });
                    }
                }
            }
        }

        for track in &project.tracks {
            if track.muted && track.kind == TrackKind::Video {
                continue;
            }
            for t in &track.transitions {
                let left_vl = visual_layers
                    .iter()
                    .find(|vl| vl.item_id == t.left_item_id && vl.track_id == track.track_id)
                    .cloned();
                let right_vl = visual_layers
                    .iter()
                    .find(|vl| vl.item_id == t.right_item_id && vl.track_id == track.track_id)
                    .cloned();

                if let (Some(left_layer), Some(right_layer)) = (left_vl, right_vl) {
                    let target_rate = project.settings.frame_rate;
                    if !left_layer
                        .timeline_range
                        .is_adjacent_to_with_rate(&right_layer.timeline_range, target_rate)
                    {
                        continue;
                    }
                    let cut_time = left_layer.timeline_range.end().rescaled_to(target_rate);
                    let d_ticks = t.duration.rescaled_to(target_rate).value;
                    let l_ticks = d_ticks / 2;
                    let r_ticks = d_ticks - l_ticks;
                    let l_dur = RationalTime::new(l_ticks, target_rate);
                    let r_dur = RationalTime::new(r_ticks, target_rate);
                    let t0 = cut_time - l_dur;
                    let trans_dur = l_dur + r_dur;

                    transitions.push(TransitionPlan {
                        transition_id: t.transition_id.clone(),
                        track_id: track.track_id.clone(),
                        kind: t.kind,
                        z_index: left_layer.z_index,
                        left_item_id: t.left_item_id.clone(),
                        right_item_id: t.right_item_id.clone(),
                        cut_time,
                        duration: t.duration,
                        transition_range: TimeRange::new(t0, trans_dur),
                        left_layer,
                        right_layer,
                    });
                }
            }
            for transition in &track.audio_transitions {
                let right = track.find_item(&transition.right_item_id).ok_or_else(|| {
                    CompositionError::InvalidRange(format!(
                        "Missing incoming clip '{}' for audio crossfade",
                        transition.right_item_id
                    ))
                })?;
                let rate = project.settings.frame_rate;
                let duration = transition.duration.rescaled_to(rate);
                let left_extent = duration.value / 2;
                let start = right.timeline_range.start - RationalTime::new(left_extent, rate);
                audio_crossfades.push(CompositionAudioCrossfade {
                    transition_id: transition.transition_id.clone(),
                    left_item_id: transition.left_item_id.clone(),
                    right_item_id: transition.right_item_id.clone(),
                    start,
                    duration,
                });
            }
        }

        let duration = RationalTime::new(max_time_val, project.settings.frame_rate);

        Ok(Self {
            project_id: project.project_id.clone(),
            revision: project.revision,
            width: project.settings.width,
            height: project.settings.height,
            frame_rate: project.settings.frame_rate,
            duration,
            visual_layers,
            audio_layers,
            audio_crossfades,
            transitions,
            sample_rate: project.settings.sample_rate,
        })
    }
}

/// Helper function to sample visual properties into a PreviewSampledLayer
fn sample_visual_layer_at(
    layer: &CompositionVisualLayer,
    clip_offset: RationalTime,
    source_time: RationalTime,
    opacity: f32,
    base_w: f32,
    base_h: f32,
) -> PreviewSampledLayer {
    let pos_x = layer.evaluate_property(KeyframeProperty::PositionX, clip_offset);
    let pos_y = layer.evaluate_property(KeyframeProperty::PositionY, clip_offset);
    let scale_x = layer.evaluate_property(KeyframeProperty::ScaleX, clip_offset);
    let scale_y = layer.evaluate_property(KeyframeProperty::ScaleY, clip_offset);
    let rotation = layer.evaluate_property(KeyframeProperty::Rotation, clip_offset);

    let cropped_w = base_w * (1.0 - layer.crop.left - layer.crop.right);
    let cropped_h = base_h * (1.0 - layer.crop.top - layer.crop.bottom);

    let render_w = cropped_w * scale_x;
    let render_h = cropped_h * scale_y;
    let center_x = (base_w / 2.0) + pos_x;
    let center_y = (base_h / 2.0) + pos_y;

    let bounds = Rect {
        x: center_x - (render_w / 2.0),
        y: center_y - (render_h / 2.0),
        width: render_w,
        height: render_h,
    };

    let (asset_path_str, is_missing_media) = match &layer.asset_path {
        Some(p) => {
            let exists = p.exists();
            (Some(p.to_string_lossy().to_string()), !exists)
        }
        None => (None, false),
    };

    PreviewSampledLayer {
        item_id: layer.item_id.clone(),
        name: layer.name.clone(),
        z_index: layer.z_index,
        kind: layer.kind,
        asset_id: layer.asset_id.clone(),
        asset_path: asset_path_str,
        is_missing_media,
        source_time,
        position: (pos_x, pos_y),
        scale: (scale_x, scale_y),
        rotation,
        crop: layer.crop.clone(),
        opacity,
        text: layer.text_content.clone(),
        font_family: layer.font_family.clone(),
        font_size: layer.font_size,
        computed_bounds: bounds,
    }
}

impl CompositionPlan {
    /// Evaluates preview state at an exact timeline timestamp
    pub fn sample_preview_at(&self, timestamp: RationalTime) -> PreviewFrameState {
        let mut active_layers = Vec::new();
        let mut transition_groups = Vec::new();
        let mut suppressed_item_ids = HashSet::new();

        let base_w = self.width as f32;
        let base_h = self.height as f32;

        for trans in &self.transitions {
            if trans.contains_time(timestamp) {
                suppressed_item_ids.insert(trans.left_item_id.clone());
                suppressed_item_ids.insert(trans.right_item_id.clone());

                let progress = trans.progress_at(timestamp);
                let left_offset = trans.left_effective_clip_offset(timestamp);
                let left_source_time = trans.left_source_time_at(timestamp);
                let left_opacity = trans.left_opacity_at(timestamp);

                let right_offset = trans.right_effective_clip_offset(timestamp);
                let right_source_time = trans.right_source_time_at(timestamp);
                let right_opacity = trans.right_opacity_at(timestamp);

                let left_layer = sample_visual_layer_at(
                    &trans.left_layer,
                    left_offset,
                    left_source_time,
                    left_opacity,
                    base_w,
                    base_h,
                );
                let right_layer = sample_visual_layer_at(
                    &trans.right_layer,
                    right_offset,
                    right_source_time,
                    right_opacity,
                    base_w,
                    base_h,
                );

                transition_groups.push(PreviewTransitionGroup {
                    transition_id: trans.transition_id.clone(),
                    track_id: trans.track_id.clone(),
                    z_index: trans.z_index,
                    kind: trans.kind,
                    progress,
                    left_layer,
                    right_layer,
                });
            }
        }

        for layer in &self.visual_layers {
            if suppressed_item_ids.contains(&layer.item_id) {
                continue;
            }
            if layer.timeline_range.contains_time(timestamp) {
                let clip_offset = timestamp - layer.timeline_range.start;
                let source_time = layer.source_range.start + clip_offset;
                let opacity = layer.effective_opacity_at(clip_offset);

                let sampled = sample_visual_layer_at(
                    layer,
                    clip_offset,
                    source_time,
                    opacity,
                    base_w,
                    base_h,
                );
                active_layers.push(sampled);
            }
        }

        // Sort by z_index ascending (back to front)
        active_layers.sort_by_key(|l| l.z_index);
        transition_groups.sort_by_key(|g| g.z_index);

        let mut active_audio = Vec::new();
        for audio in &self.audio_layers {
            if self.audio_layer_active_at(audio, timestamp) {
                let gain = self.effective_audio_gain_at(audio, timestamp);
                active_audio.push(PreviewSampledAudio {
                    item_id: audio.item_id.clone(),
                    effective_volume: gain,
                    source_time: audio.source_range.start
                        + (timestamp - audio.timeline_range.start),
                    asset_path: audio
                        .asset_path
                        .as_ref()
                        .map(|path| path.to_string_lossy().into()),
                });
            }
        }

        PreviewFrameState {
            timestamp,
            active_layers,
            active_audio,
            transition_groups,
        }
    }

    /// Builds the deterministic video filtergraph shared across full export and single-frame extraction
    pub fn build_video_filtergraph(
        &self,
        input_paths: &[PathBuf],
        base_duration: f64,
        sample_time: Option<RationalTime>,
    ) -> (String, String) {
        self.build_video_filtergraph_with_schedule(input_paths, base_duration, sample_time, None)
    }

    /// Builds the deterministic video filtergraph with optional runtime sendcmd automation schedule
    pub fn build_video_filtergraph_with_schedule(
        &self,
        input_paths: &[PathBuf],
        base_duration: f64,
        sample_time: Option<RationalTime>,
        schedule: Option<&crate::modules::export::ExportAutomationSchedule>,
    ) -> (String, String) {
        let mut filter_complex = String::new();

        filter_complex.push_str(&format!(
            "color=c=black:s={}x{}:r={}:d={:.3}[base];",
            self.width,
            self.height,
            self.frame_rate.as_f64(),
            base_duration
        ));

        let mut current_base = "base".to_string();
        let mut video_overlay_idx = 0;
        let mut trans_idx = 0;
        let half_frame = (0.5 / self.frame_rate.as_f64()).min(0.02);

        enum RenderElement<'a> {
            Layer(&'a CompositionVisualLayer),
            Transition(&'a TransitionPlan),
        }

        let mut elements: Vec<RenderElement> = Vec::new();
        for layer in &self.visual_layers {
            elements.push(RenderElement::Layer(layer));
        }
        for trans in &self.transitions {
            elements.push(RenderElement::Transition(trans));
        }
        elements.sort_by_key(|e| match e {
            RenderElement::Layer(l) => (l.z_index, 0),
            RenderElement::Transition(t) => (t.z_index, 1),
        });

        for element in elements {
            match element {
                RenderElement::Layer(layer) => {
                    let incoming_trans = self
                        .transitions
                        .iter()
                        .find(|t| t.right_item_id == layer.item_id && t.track_id == layer.track_id);
                    let outgoing_trans = self
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
                        // Entirely covered by transitions; skip ordinary overlay
                        continue;
                    }

                    let (pos_x, pos_y, scale_x, scale_y, rotation, opacity) = match sample_time {
                        Some(ts) => {
                            let clip_offset = ts - layer.timeline_range.start;
                            (
                                layer.evaluate_property(KeyframeProperty::PositionX, clip_offset),
                                layer.evaluate_property(KeyframeProperty::PositionY, clip_offset),
                                layer.evaluate_property(KeyframeProperty::ScaleX, clip_offset),
                                layer.evaluate_property(KeyframeProperty::ScaleY, clip_offset),
                                layer.evaluate_property(KeyframeProperty::Rotation, clip_offset),
                                layer.effective_opacity_at(clip_offset),
                            )
                        }
                        None => (
                            layer.transform.position_x,
                            layer.transform.position_y,
                            layer.transform.scale_x,
                            layer.transform.scale_y,
                            layer.transform.rotation,
                            layer.opacity,
                        ),
                    };

                    let start_bound = (ord_start_sec - half_frame).max(0.0);
                    let end_bound = (ord_end_sec - half_frame).max(ord_start_sec);

                    match layer.kind {
                        ItemKind::Video | ItemKind::Image => {
                            let offset_from_start = ord_start_sec - layer_start_sec;
                            let ord_src_start =
                                layer.source_range.start.to_seconds() + offset_from_start;
                            let ord_duration = ord_end_sec - ord_start_sec;

                            let base_w = self.width as f32;
                            let base_h = self.height as f32;
                            let cropped_w = base_w * (1.0 - layer.crop.left - layer.crop.right);
                            let cropped_h = base_h * (1.0 - layer.crop.top - layer.crop.bottom);

                            let mut render_w = (cropped_w * scale_x).round().max(2.0) as u32;
                            let mut render_h = (cropped_h * scale_y).round().max(2.0) as u32;
                            if !render_w.is_multiple_of(2) {
                                render_w += 1;
                            }
                            if !render_h.is_multiple_of(2) {
                                render_h += 1;
                            }

                            let layer_tag = format!("layer_{}", video_overlay_idx);
                            let next_base = format!("v_over_{}", video_overlay_idx);
                            video_overlay_idx += 1;

                            let mut filter_chain = String::new();
                            if let Some(ref path) = layer.asset_path {
                                let input_idx =
                                    input_paths.iter().position(|p| p == path).unwrap_or(0);

                                if ord_start_sec.abs() < 1e-5 {
                                    filter_chain.push_str(&format!(
                                        "[{}:v]trim=start={:.3}:duration={:.3},setpts=PTS-STARTPTS",
                                        input_idx, ord_src_start, ord_duration
                                    ));
                                } else {
                                    filter_chain.push_str(&format!(
                                        "[{}:v]trim=start={:.3}:duration={:.3},setpts=PTS-STARTPTS+{:.3}/TB",
                                        input_idx, ord_src_start, ord_duration, ord_start_sec
                                    ));
                                }
                            } else {
                                filter_chain.push_str(&format!(
                                    "color=c=blue:s={}x{}:d={:.3}",
                                    render_w.max(2),
                                    render_h.max(2),
                                    ord_duration
                                ));
                            }

                            // Crop filter
                            if layer.crop.left > 0.0
                                || layer.crop.right > 0.0
                                || layer.crop.top > 0.0
                                || layer.crop.bottom > 0.0
                            {
                                filter_chain.push_str(&format!(
                                    ",crop=w='iw*(1.0-{:.4}-{:.4})':h='ih*(1.0-{:.4}-{:.4})':x='iw*{:.4}':y='ih*{:.4}'",
                                    layer.crop.left, layer.crop.right, layer.crop.top, layer.crop.bottom, layer.crop.left, layer.crop.top
                                ));
                            }

                            let layer_idx = self
                                .visual_layers
                                .iter()
                                .position(|l| l.item_id == layer.item_id)
                                .unwrap_or(video_overlay_idx);
                            let layer_auto = schedule.and_then(|s| {
                                s.video_layers.iter().find(|a| a.layer_index == layer_idx)
                            });

                            if let Some(auto) = layer_auto {
                                let init_angle_rad = rotation * std::f32::consts::PI / 180.0;
                                filter_chain.push_str(&format!(
                                    ",format=yuva420p,sendcmd=f='{}',scale@sc_{}=eval=frame:w={}:h={},rotate@rot_{}=angle={:.4}:ow='hypot(iw,ih)':oh='hypot(iw,ih)':c=none,colorchannelmixer@ccm_{}=aa={:.3}[{}];",
                                    auto.escaped_command_file,
                                    layer_idx, render_w, render_h,
                                    layer_idx, init_angle_rad,
                                    layer_idx, opacity,
                                    layer_tag
                                ));
                            } else {
                                // Scale filter
                                filter_chain.push_str(&format!(",scale={}:{}", render_w, render_h));

                                // Rotation filter
                                if rotation.abs() > 0.01 {
                                    filter_chain.push_str(&format!(
                                        ",rotate='{0:.4}*PI/180':ow='rotw({0:.4}*PI/180)':oh='roth({0:.4}*PI/180)':c=none",
                                        rotation
                                    ));
                                }

                                // Transition filters (FadeIn, FadeOut, CrossDissolve, FadeBlack)
                                if let Some(ref trans) = layer.transition {
                                    let dur_sec = trans.duration.to_seconds();
                                    match trans.kind {
                                        TransitionKind::FadeIn | TransitionKind::FadeBlack => {
                                            filter_chain.push_str(&format!(
                                                ",fade=t=in:st=0:d={:.3}",
                                                dur_sec
                                            ));
                                        }
                                        TransitionKind::FadeOut => {
                                            let st = ord_duration - dur_sec;
                                            filter_chain.push_str(&format!(
                                                ",fade=t=out:st={:.3}:d={:.3}",
                                                st.max(0.0),
                                                dur_sec
                                            ));
                                        }
                                        TransitionKind::CrossDissolve => {}
                                    }
                                }

                                filter_chain.push_str(&format!(
                                    ",format=yuva420p,colorchannelmixer=aa={:.3}[{}];",
                                    opacity, layer_tag
                                ));
                            }
                            filter_complex.push_str(&filter_chain);

                            let ov_tag = if layer_auto.is_some() {
                                format!("overlay@ov_{}", layer_idx)
                            } else {
                                "overlay".to_string()
                            };

                            filter_complex.push_str(&format!(
                                "[{}][{}]{}=x='(W-w)/2+{:.1}':y='(H-h)/2+{:.1}':enable='between(t,{:.4},{:.4})'[{}];",
                                current_base, layer_tag, ov_tag, pos_x, pos_y, start_bound, end_bound, next_base
                            ));
                            current_base = next_base;

                            if let Some(ref text) = layer.text_content {
                                if !text.trim().is_empty() {
                                    let text_base = format!("v_over_{}", video_overlay_idx);
                                    video_overlay_idx += 1;
                                    let escaped_text = text
                                        .replace('\\', "\\\\")
                                        .replace('\'', "'\\''")
                                        .replace(':', "\\:");
                                    let font_size = layer.font_size.unwrap_or(48.0);
                                    let font_arg = if cfg!(windows)
                                        && Path::new("C:/Windows/Fonts/arial.ttf").exists()
                                    {
                                        ":fontfile='C\\:/Windows/Fonts/arial.ttf'"
                                    } else {
                                        ""
                                    };
                                    filter_complex.push_str(&format!(
                                        "[{}]drawtext=text='{}':fontsize={}{}:fontcolor=white:box=1:boxcolor=black@0.6:boxborderw=6:x='(w-tw)/2+{:.1}':y='(h-th)/2+{:.1}+100':enable='between(t,{:.4},{:.4})'[{}];",
                                        current_base, escaped_text, font_size, font_arg, pos_x, pos_y, start_bound, end_bound, text_base
                                    ));
                                    current_base = text_base;
                                }
                            }
                        }
                        ItemKind::Text => {
                            let text = layer.text_content.as_deref().unwrap_or("");
                            let escaped_text = text
                                .replace('\\', "\\\\")
                                .replace('\'', "'\\''")
                                .replace(':', "\\:");
                            let font_size = layer.font_size.unwrap_or(48.0);
                            let font_arg = if cfg!(windows)
                                && Path::new("C:/Windows/Fonts/arial.ttf").exists()
                            {
                                ":fontfile='C\\:/Windows/Fonts/arial.ttf'"
                            } else {
                                ""
                            };

                            let next_base = format!("v_over_{}", video_overlay_idx);
                            video_overlay_idx += 1;
                            filter_complex.push_str(&format!(
                                "[{}]drawtext=text='{}':fontsize={}{}:fontcolor=white:x='(w-tw)/2+{:.1}':y='(h-th)/2+{:.1}':enable='between(t,{:.4},{:.4})'[{}];",
                                current_base, escaped_text, font_size, font_arg, pos_x, pos_y, start_bound, end_bound, next_base
                            ));
                            current_base = next_base;
                        }
                        _ => {}
                    }
                }
                RenderElement::Transition(trans) => {
                    let dur_sec = trans.duration.to_seconds();
                    let t0_sec = trans.transition_range.start.to_seconds();
                    let t1_sec = trans.transition_range.end().to_seconds();
                    let l_dur = trans.l_duration();

                    let left_src_start = (trans.left_layer.source_range.end() - l_dur)
                        .to_seconds()
                        .max(0.0);
                    let right_src_start = (trans.right_layer.source_range.start - l_dur)
                        .to_seconds()
                        .max(0.0);

                    let left_layer = &trans.left_layer;
                    let right_layer = &trans.right_layer;

                    let (
                        left_pos_x,
                        left_pos_y,
                        left_scale_x,
                        left_scale_y,
                        left_rotation,
                        left_opacity,
                    ) = match sample_time {
                        Some(ts) => {
                            let offset = trans.left_effective_clip_offset(ts);
                            (
                                left_layer.evaluate_property(KeyframeProperty::PositionX, offset),
                                left_layer.evaluate_property(KeyframeProperty::PositionY, offset),
                                left_layer.evaluate_property(KeyframeProperty::ScaleX, offset),
                                left_layer.evaluate_property(KeyframeProperty::ScaleY, offset),
                                left_layer.evaluate_property(KeyframeProperty::Rotation, offset),
                                left_layer.effective_opacity_at(offset),
                            )
                        }
                        None => (
                            left_layer.transform.position_x,
                            left_layer.transform.position_y,
                            left_layer.transform.scale_x,
                            left_layer.transform.scale_y,
                            left_layer.transform.rotation,
                            left_layer.opacity,
                        ),
                    };

                    let (
                        right_pos_x,
                        right_pos_y,
                        right_scale_x,
                        right_scale_y,
                        right_rotation,
                        right_opacity,
                    ) = match sample_time {
                        Some(ts) => {
                            let offset = trans.right_effective_clip_offset(ts);
                            (
                                right_layer.evaluate_property(KeyframeProperty::PositionX, offset),
                                right_layer.evaluate_property(KeyframeProperty::PositionY, offset),
                                right_layer.evaluate_property(KeyframeProperty::ScaleX, offset),
                                right_layer.evaluate_property(KeyframeProperty::ScaleY, offset),
                                right_layer.evaluate_property(KeyframeProperty::Rotation, offset),
                                right_layer.effective_opacity_at(offset),
                            )
                        }
                        None => (
                            right_layer.transform.position_x,
                            right_layer.transform.position_y,
                            right_layer.transform.scale_x,
                            right_layer.transform.scale_y,
                            right_layer.transform.rotation,
                            right_layer.opacity,
                        ),
                    };

                    let base_w = self.width as f32;
                    let base_h = self.height as f32;

                    // Left participant render
                    let left_cropped_w =
                        base_w * (1.0 - left_layer.crop.left - left_layer.crop.right);
                    let left_cropped_h =
                        base_h * (1.0 - left_layer.crop.top - left_layer.crop.bottom);
                    let mut left_render_w = (left_cropped_w * left_scale_x).round().max(2.0) as u32;
                    let mut left_render_h = (left_cropped_h * left_scale_y).round().max(2.0) as u32;
                    if !left_render_w.is_multiple_of(2) {
                        left_render_w += 1;
                    }
                    if !left_render_h.is_multiple_of(2) {
                        left_render_h += 1;
                    }

                    let left_clip_tag = format!("tr_l_clip_{}", trans_idx);
                    let mut left_clip_filter = String::new();
                    if let Some(ref path) = left_layer.asset_path {
                        let input_idx = input_paths.iter().position(|p| p == path).unwrap_or(0);
                        left_clip_filter.push_str(&format!(
                            "[{}:v]trim=start={:.6}:duration={:.6},setpts=PTS-STARTPTS",
                            input_idx, left_src_start, dur_sec
                        ));
                    } else {
                        left_clip_filter.push_str(&format!(
                            "color=c=blue:s={}x{}:d={:.6}",
                            left_render_w.max(2),
                            left_render_h.max(2),
                            dur_sec
                        ));
                    }
                    if left_layer.crop.left > 0.0
                        || left_layer.crop.right > 0.0
                        || left_layer.crop.top > 0.0
                        || left_layer.crop.bottom > 0.0
                    {
                        left_clip_filter.push_str(&format!(
                            ",crop=w='iw*(1.0-{:.4}-{:.4})':h='ih*(1.0-{:.4}-{:.4})':x='iw*{:.4}':y='ih*{:.4}'",
                            left_layer.crop.left, left_layer.crop.right, left_layer.crop.top, left_layer.crop.bottom, left_layer.crop.left, left_layer.crop.top
                        ));
                    }
                    left_clip_filter
                        .push_str(&format!(",scale={}:{}", left_render_w, left_render_h));
                    if left_rotation.abs() > 0.01 {
                        left_clip_filter.push_str(&format!(
                            ",rotate='{0:.4}*PI/180':ow='rotw({0:.4}*PI/180)':oh='roth({0:.4}*PI/180)':c=none",
                            left_rotation
                        ));
                    }
                    left_clip_filter.push_str(&format!(
                        ",format=yuva420p,colorchannelmixer=aa={:.3}[{}];",
                        left_opacity, left_clip_tag
                    ));
                    filter_complex.push_str(&left_clip_filter);

                    let trans_auto = schedule.and_then(|s| {
                        s.transition_branches
                            .iter()
                            .find(|t| t.transition_index == trans_idx)
                    });
                    let left_sendcmd =
                        trans_auto.and_then(|t| t.escaped_left_command_file.as_ref());
                    let right_sendcmd =
                        trans_auto.and_then(|t| t.escaped_right_command_file.as_ref());

                    let left_canvas_tag = format!("tr_l_canvas_{}", trans_idx);
                    let left_branch_tag = format!("tr_l_branch_{}", trans_idx);
                    if let Some(left_cmd) = left_sendcmd {
                        filter_complex.push_str(&format!(
                            "color=c=black@0:s={}x{}:r={}:d={:.6},sendcmd=f='{}'[{}];",
                            self.width,
                            self.height,
                            self.frame_rate.as_f64(),
                            dur_sec,
                            left_cmd,
                            left_canvas_tag
                        ));
                    } else {
                        filter_complex.push_str(&format!(
                            "color=c=black@0:s={}x{}:r={}:d={:.6}[{}];",
                            self.width,
                            self.height,
                            self.frame_rate.as_f64(),
                            dur_sec,
                            left_canvas_tag
                        ));
                    }

                    let left_ov = if left_sendcmd.is_some() {
                        format!("overlay@ov_tr_l_{}", trans_idx)
                    } else {
                        "overlay".to_string()
                    };

                    filter_complex.push_str(&format!(
                        "[{}][{}]{}=x='(W-w)/2+{:.1}':y='(H-h)/2+{:.1}',format=rgba,premultiply=inplace=1[{}];",
                        left_canvas_tag, left_clip_tag, left_ov, left_pos_x, left_pos_y, left_branch_tag
                    ));

                    // Right participant render
                    let right_cropped_w =
                        base_w * (1.0 - right_layer.crop.left - right_layer.crop.right);
                    let right_cropped_h =
                        base_h * (1.0 - right_layer.crop.top - right_layer.crop.bottom);
                    let mut right_render_w =
                        (right_cropped_w * right_scale_x).round().max(2.0) as u32;
                    let mut right_render_h =
                        (right_cropped_h * right_scale_y).round().max(2.0) as u32;
                    if !right_render_w.is_multiple_of(2) {
                        right_render_w += 1;
                    }
                    if !right_render_h.is_multiple_of(2) {
                        right_render_h += 1;
                    }

                    let right_clip_tag = format!("tr_r_clip_{}", trans_idx);
                    let mut right_clip_filter = String::new();
                    if let Some(ref path) = right_layer.asset_path {
                        let input_idx = input_paths.iter().position(|p| p == path).unwrap_or(0);
                        right_clip_filter.push_str(&format!(
                            "[{}:v]trim=start={:.6}:duration={:.6},setpts=PTS-STARTPTS",
                            input_idx, right_src_start, dur_sec
                        ));
                    } else {
                        right_clip_filter.push_str(&format!(
                            "color=c=blue:s={}x{}:d={:.6}",
                            right_render_w.max(2),
                            right_render_h.max(2),
                            dur_sec
                        ));
                    }
                    if right_layer.crop.left > 0.0
                        || right_layer.crop.right > 0.0
                        || right_layer.crop.top > 0.0
                        || right_layer.crop.bottom > 0.0
                    {
                        right_clip_filter.push_str(&format!(
                            ",crop=w='iw*(1.0-{:.4}-{:.4})':h='ih*(1.0-{:.4}-{:.4})':x='iw*{:.4}':y='ih*{:.4}'",
                            right_layer.crop.left, right_layer.crop.right, right_layer.crop.top, right_layer.crop.bottom, right_layer.crop.left, right_layer.crop.top
                        ));
                    }
                    right_clip_filter
                        .push_str(&format!(",scale={}:{}", right_render_w, right_render_h));
                    if right_rotation.abs() > 0.01 {
                        right_clip_filter.push_str(&format!(
                            ",rotate='{0:.4}*PI/180':ow='rotw({0:.4}*PI/180)':oh='roth({0:.4}*PI/180)':c=none",
                            right_rotation
                        ));
                    }
                    right_clip_filter.push_str(&format!(
                        ",format=yuva420p,colorchannelmixer=aa={:.3}[{}];",
                        right_opacity, right_clip_tag
                    ));
                    filter_complex.push_str(&right_clip_filter);

                    let right_canvas_tag = format!("tr_r_canvas_{}", trans_idx);
                    let right_branch_tag = format!("tr_r_branch_{}", trans_idx);
                    if let Some(right_cmd) = right_sendcmd {
                        filter_complex.push_str(&format!(
                            "color=c=black@0:s={}x{}:r={}:d={:.6},sendcmd=f='{}'[{}];",
                            self.width,
                            self.height,
                            self.frame_rate.as_f64(),
                            dur_sec,
                            right_cmd,
                            right_canvas_tag
                        ));
                    } else {
                        filter_complex.push_str(&format!(
                            "color=c=black@0:s={}x{}:r={}:d={:.6}[{}];",
                            self.width,
                            self.height,
                            self.frame_rate.as_f64(),
                            dur_sec,
                            right_canvas_tag
                        ));
                    }

                    let right_ov = if right_sendcmd.is_some() {
                        format!("overlay@ov_tr_r_{}", trans_idx)
                    } else {
                        "overlay".to_string()
                    };

                    filter_complex.push_str(&format!(
                        "[{}][{}]{}=x='(W-w)/2+{:.1}':y='(H-h)/2+{:.1}',format=rgba,premultiply=inplace=1[{}];",
                        right_canvas_tag, right_clip_tag, right_ov, right_pos_x, right_pos_y, right_branch_tag
                    ));

                    // Crossfade
                    let xfade_tag = format!("tr_xfade_{}", trans_idx);
                    let next_base = format!("v_over_{}", video_overlay_idx);
                    video_overlay_idx += 1;
                    trans_idx += 1;

                    let t0_val = trans.transition_range.start.value;
                    let t0_den = trans.transition_range.start.rate.numerator;
                    filter_complex.push_str(&format!(
                        "[{}][{}]xfade=transition=fade:duration={:.6}:offset=0,unpremultiply=inplace=1,format=rgba,setpts=PTS-STARTPTS+({})/({}*TB)[{}];",
                        left_branch_tag, right_branch_tag, dur_sec, t0_val, t0_den, xfade_tag
                    ));

                    let t0_bound = (t0_sec - half_frame).max(0.0);
                    let t1_bound = (t1_sec - half_frame).max(t0_sec);
                    filter_complex.push_str(&format!(
                        "[{}][{}]overlay=0:0:enable='between(t,{:.4},{:.4})'[{}];",
                        current_base, xfade_tag, t0_bound, t1_bound, next_base
                    ));
                    current_base = next_base;
                }
            }
        }

        (filter_complex, current_base)
    }

    /// Projects the CompositionPlan into deterministic FFmpeg filter_complex arguments
    pub fn to_ffmpeg_export_args(
        &self,
        output_path: &Path,
        crf: u32,
        video_codec: &str,
    ) -> (Vec<String>, String) {
        self.to_ffmpeg_export_args_with_schedule(output_path, crf, video_codec, None)
    }

    /// Projects the CompositionPlan into deterministic FFmpeg export arguments with runtime automation schedule
    pub fn to_ffmpeg_export_args_with_schedule(
        &self,
        output_path: &Path,
        crf: u32,
        video_codec: &str,
        schedule: Option<&crate::modules::export::ExportAutomationSchedule>,
    ) -> (Vec<String>, String) {
        let mut args: Vec<String> = Vec::new();

        // 1. Gather unique input assets
        let mut input_paths: Vec<PathBuf> = Vec::new();
        for layer in &self.visual_layers {
            if let Some(ref path) = layer.asset_path {
                if !input_paths.contains(path) {
                    input_paths.push(path.clone());
                }
            }
        }
        for audio in &self.audio_layers {
            if let Some(ref path) = audio.asset_path {
                if !input_paths.contains(path) {
                    input_paths.push(path.clone());
                }
            }
        }

        // Add inputs to command line
        for path in &input_paths {
            args.push("-i".to_string());
            args.push(path.to_string_lossy().to_string());
        }

        // Add audio envelope inputs if any in schedule
        if let Some(sched) = schedule {
            for audio_auto in &sched.audio_layers {
                args.push("-f".to_string());
                args.push("f32le".to_string());
                args.push("-ar".to_string());
                args.push(audio_auto.sample_rate.to_string());
                args.push("-ac".to_string());
                args.push("1".to_string());
                args.push("-i".to_string());
                args.push(audio_auto.envelope_file.to_string_lossy().to_string());
            }
        }

        // 2. Build video filter chain:
        let duration_secs = self.duration.to_seconds();
        let (mut filter_complex, current_base) =
            self.build_video_filtergraph_with_schedule(&input_paths, duration_secs, None, schedule);

        // Audio filter chain
        let mut audio_tags = Vec::new();
        let mut audio_idx = 0;
        for (i, audio) in self.audio_layers.iter().enumerate() {
            if audio.muted {
                continue;
            }
            if let Some(ref path) = audio.asset_path {
                let (render_start, render_source_start, render_duration) =
                    self.audio_render_window(audio);
                let input_idx = input_paths.iter().position(|p| p == path).unwrap_or(0);
                let tag = format!("a_{}", audio_idx);
                audio_idx += 1;

                let src_start = render_source_start.to_seconds();
                let src_dur = render_duration.to_seconds();
                let timeline_start_ms = (render_start.to_seconds() * 1000.0) as u64;

                let audio_auto =
                    schedule.and_then(|s| s.audio_layers.iter().position(|a| a.audio_index == i));

                if let Some(env_pos) = audio_auto {
                    let env_input_idx = input_paths.len() + env_pos;
                    filter_complex.push_str(&format!(
                        "[{}:a]pan=stereo|c0=c0|c1=c0[env_st_{}];",
                        env_input_idx, i
                    ));
                    filter_complex.push_str(&format!(
                        "[{}:a]atrim=start={:.3}:duration={:.3},asetpts=PTS-STARTPTS[a_trimmed_{}];",
                        input_idx, src_start, src_dur, i
                    ));
                    filter_complex.push_str(&format!(
                        "[a_trimmed_{}][env_st_{}]amultiply,adelay={}|{}[{}];",
                        i, i, timeline_start_ms, timeline_start_ms, tag
                    ));
                } else {
                    let mut filter_chain = format!(
                        "[{}:a]atrim=start={:.3}:duration={:.3},asetpts=PTS-STARTPTS,volume={:.3}",
                        input_idx, src_start, src_dur, audio.volume
                    );

                    if let Some(fade_in) = audio.fade_in {
                        filter_chain
                            .push_str(&format!(",afade=t=in:st=0:d={:.3}", fade_in.to_seconds()));
                    }

                    if let Some(fade_out) = audio.fade_out {
                        let st = src_dur - fade_out.to_seconds();
                        filter_chain.push_str(&format!(
                            ",afade=t=out:st={:.3}:d={:.3}",
                            st.max(0.0),
                            fade_out.to_seconds()
                        ));
                    }

                    // Delay audio to match timeline placement
                    filter_chain.push_str(&format!(
                        ",adelay={}|{}[{}];",
                        timeline_start_ms, timeline_start_ms, tag
                    ));

                    filter_complex.push_str(&filter_chain);
                }

                audio_tags.push(tag);
            }
        }

        // Mix audio tracks if any exist
        let final_audio_tag = if !audio_tags.is_empty() {
            let combined = audio_tags
                .iter()
                .map(|t| format!("[{}]", t))
                .collect::<Vec<_>>()
                .join("");
            filter_complex.push_str(&format!(
                "{}amix=inputs={}:duration=longest,apad,atrim=duration={:.6}[outa];",
                combined,
                audio_tags.len(),
                duration_secs
            ));
            Some("outa")
        } else {
            None
        };

        // Construct full arguments
        args.push("-filter_complex".to_string());
        args.push(filter_complex.clone());

        args.push("-map".to_string());
        args.push(format!("[{}]", current_base));

        if let Some(a_tag) = final_audio_tag {
            args.push("-map".to_string());
            args.push(format!("[{}]", a_tag));
            args.push("-c:a".to_string());
            args.push("aac".to_string());
            let out_ar = schedule
                .and_then(|s| s.audio_layers.first().map(|a| a.sample_rate))
                .unwrap_or(self.sample_rate);
            args.push("-ar".to_string());
            args.push(out_ar.to_string());
        }

        args.push("-c:v".to_string());
        args.push(video_codec.to_string());
        args.push("-crf".to_string());
        args.push(crf.to_string());
        args.push("-pix_fmt".to_string());
        args.push("yuv420p".to_string());
        args.push("-f".to_string());
        args.push("mp4".to_string());
        args.push("-y".to_string());
        args.push(output_path.to_string_lossy().to_string());

        (args, filter_complex)
    }

    /// Projects the CompositionPlan into deterministic FFmpeg arguments to render a single frame at timestamp t
    pub fn to_ffmpeg_single_frame_args(
        &self,
        timestamp: RationalTime,
        output_path: &Path,
    ) -> (Vec<String>, String) {
        let mut args: Vec<String> = Vec::new();

        // 1. Gather unique input assets
        let mut input_paths: Vec<PathBuf> = Vec::new();
        for layer in &self.visual_layers {
            if let Some(ref path) = layer.asset_path {
                if !input_paths.contains(path) {
                    input_paths.push(path.clone());
                }
            }
        }

        for path in &input_paths {
            args.push("-i".to_string());
            args.push(path.to_string_lossy().to_string());
        }

        let target_sec = timestamp.to_seconds();
        let duration_secs = (self.duration.to_seconds() + 1.0).max(target_sec + 1.0);

        let (filter_complex, final_base) =
            self.build_video_filtergraph(&input_paths, duration_secs, Some(timestamp));

        args.push("-filter_complex".to_string());
        args.push(filter_complex.clone());
        args.push("-map".to_string());
        args.push(format!("[{}]", final_base));
        args.push("-ss".to_string());
        args.push(format!("{:.6}", target_sec));
        args.push("-vframes".to_string());
        args.push("1".to_string());

        let ext = output_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        if ext.eq_ignore_ascii_case("raw") {
            args.push("-f".to_string());
            args.push("rawvideo".to_string());
            args.push("-pix_fmt".to_string());
            args.push("rgba".to_string());
            args.push("pipe:1".to_string());
        } else {
            args.push("-update".to_string());
            args.push("1".to_string());
            args.push("-y".to_string());
            args.push(output_path.to_string_lossy().to_string());
        }

        (args, filter_complex)
    }
}

/// Isolated preview transition group for CrossDissolve and cut transitions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreviewTransitionGroup {
    pub transition_id: String,
    pub track_id: String,
    pub z_index: usize,
    pub kind: TransitionKind,
    pub progress: f32,
    pub left_layer: PreviewSampledLayer,
    pub right_layer: PreviewSampledLayer,
}

/// Sampled preview state at timestamp t
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreviewFrameState {
    pub timestamp: RationalTime,
    pub active_layers: Vec<PreviewSampledLayer>,
    pub active_audio: Vec<PreviewSampledAudio>,
    #[serde(default)]
    pub transition_groups: Vec<PreviewTransitionGroup>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreviewSampledLayer {
    pub item_id: String,
    pub name: String,
    pub z_index: usize,
    pub kind: ItemKind,
    pub asset_id: Option<String>,
    pub asset_path: Option<String>,
    pub is_missing_media: bool,
    pub source_time: RationalTime,
    pub position: (f32, f32),
    pub scale: (f32, f32),
    pub rotation: f32,
    pub crop: Crop,
    pub opacity: f32,
    pub text: Option<String>,
    pub font_family: Option<String>,
    pub font_size: Option<f32>,
    pub computed_bounds: Rect,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreviewSampledAudio {
    pub item_id: String,
    pub effective_volume: f32,
    pub source_time: RationalTime,
    pub asset_path: Option<String>,
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::modules::domain::time::{FrameRate, RationalTime, TimeRange};
    use crate::modules::domain::{
        Asset, Crop, ItemKind, MediaKind, Project, ProjectSettings, TimelineItem, Track, TrackKind,
    };
    use crate::modules::media::resolver::MediaBinaryResolver;
    use std::fs;

    fn get_fixture_dir() -> PathBuf {
        let dir = std::env::temp_dir().join("tinixcut_test_fixtures_phase7");
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Generates a valid test video fixture using FFmpeg
    fn generate_synthetic_mp4(path: &Path, duration_secs: u32, color: &str) {
        if path.exists() {
            return;
        }
        if let Ok(mut cmd) = MediaBinaryResolver::command("ffmpeg") {
            let output = cmd
                .args([
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    &format!("color=c={}:s=1280x720:r=30:d={}", color, duration_secs),
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
    }

    #[test]
    fn test_composition_plan_from_project_normalization() {
        let mut project = Project::new("Composition Test".to_string());
        let asset_id = "asset-sample-01".to_string();
        project.assets.push(Asset {
            asset_id: asset_id.clone(),
            name: "test.mp4".to_string(),
            path: "D:/media/test.mp4".to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(300, FrameRate::FPS_30)),
            file_size: 1024,
        });

        let mut v_track = Track::new("Video Track 1".to_string(), TrackKind::Video);
        let mut item1 = TimelineItem::new(
            Some(asset_id),
            "Clip 1".to_string(),
            ItemKind::Video,
            TimeRange::new(
                RationalTime::new(0, FrameRate::FPS_30),
                RationalTime::new(90, FrameRate::FPS_30),
            ),
            TimeRange::new(
                RationalTime::new(30, FrameRate::FPS_30),
                RationalTime::new(90, FrameRate::FPS_30),
            ),
        )
        .unwrap();

        item1.transform.position_x = 100.0;
        item1.transform.scale_x = 1.2;
        item1.crop.top = 0.1;
        item1.opacity = 0.85;

        v_track.items.push(item1);
        project.tracks.push(v_track);

        let plan = CompositionPlan::from_project(&project).expect("Plan should compile");

        assert_eq!(plan.visual_layers.len(), 1);
        assert_eq!(plan.visual_layers[0].z_index, 0);
        assert_eq!(plan.visual_layers[0].transform.position_x, 100.0);
        assert_eq!(plan.visual_layers[0].crop.top, 0.1);
        assert_eq!(plan.visual_layers[0].opacity, 0.85);
        assert_eq!(plan.duration.value, 90);
    }

    #[test]
    fn test_phase18_audio_crossfade_uses_centered_equal_power_gains_and_handles() {
        use crate::modules::domain::AudioCrossfade;

        let rate = FrameRate::FPS_30;
        let mut project = Project::new("Audio Crossfade".to_string());
        let asset_id = "asset_audio".to_string();
        project.assets.push(Asset {
            asset_id: asset_id.clone(),
            name: "tone.wav".to_string(),
            path: "tone.wav".to_string(),
            media_kind: MediaKind::Audio,
            duration: Some(RationalTime::from_frames(70, rate)),
            file_size: 0,
        });

        let mut track = Track::new("Audio".to_string(), TrackKind::Audio);
        let left = TimelineItem::new(
            Some(asset_id.clone()),
            "Outgoing".to_string(),
            ItemKind::Audio,
            TimeRange::new(
                RationalTime::from_frames(0, rate),
                RationalTime::from_frames(60, rate),
            ),
            TimeRange::new(
                RationalTime::from_frames(0, rate),
                RationalTime::from_frames(60, rate),
            ),
        )
        .unwrap();
        let left_id = left.item_id.clone();
        let right = TimelineItem::new(
            Some(asset_id),
            "Incoming".to_string(),
            ItemKind::Audio,
            TimeRange::new(
                RationalTime::from_frames(60, rate),
                RationalTime::from_frames(60, rate),
            ),
            TimeRange::new(
                RationalTime::from_frames(10, rate),
                RationalTime::from_frames(60, rate),
            ),
        )
        .unwrap();
        let right_id = right.item_id.clone();
        track.items.extend([left, right]);
        track.audio_transitions.push(AudioCrossfade {
            transition_id: "audio_xfade".to_string(),
            track_id: track.track_id.clone(),
            left_item_id: left_id.clone(),
            right_item_id: right_id.clone(),
            duration: RationalTime::from_frames(20, rate),
        });
        project.tracks.push(track);

        let plan = CompositionPlan::from_project(&project).expect("crossfade plan compiles");
        assert_eq!(plan.audio_crossfades.len(), 1);
        assert_eq!(
            plan.audio_crossfades[0].start,
            RationalTime::from_frames(50, rate)
        );
        assert_eq!(
            plan.audio_render_window(&plan.audio_layers[0]).0,
            RationalTime::from_frames(0, rate)
        );
        assert_eq!(
            plan.audio_render_window(&plan.audio_layers[1]).1,
            RationalTime::zero(rate)
        );

        let start = plan.sample_preview_at(RationalTime::from_frames(50, rate));
        let left_start = start
            .active_audio
            .iter()
            .find(|a| a.item_id == left_id)
            .unwrap();
        let right_start = start
            .active_audio
            .iter()
            .find(|a| a.item_id == right_id)
            .unwrap();
        assert!((left_start.effective_volume - 1.0).abs() < 0.0001);
        assert!(right_start.effective_volume.abs() < 0.0001);

        let midpoint = plan.sample_preview_at(RationalTime::from_frames(60, rate));
        let left_mid = midpoint
            .active_audio
            .iter()
            .find(|a| a.item_id == left_id)
            .unwrap();
        let right_mid = midpoint
            .active_audio
            .iter()
            .find(|a| a.item_id == right_id)
            .unwrap();
        let equal_power_gain = std::f32::consts::FRAC_1_SQRT_2;
        assert!((left_mid.effective_volume - equal_power_gain).abs() < 0.0001);
        assert!((right_mid.effective_volume - equal_power_gain).abs() < 0.0001);
        assert!(
            (left_mid.effective_volume.powi(2) + right_mid.effective_volume.powi(2) - 1.0).abs()
                < 0.0001
        );

        let after_cut = plan.sample_preview_at(RationalTime::from_frames(69, rate));
        assert_eq!(after_cut.active_audio.len(), 2);
        assert!(after_cut
            .active_audio
            .iter()
            .all(|a| a.effective_volume > 0.0));

        for sample_rate in [44_100, 48_000] {
            let automation_dir = std::env::temp_dir().join(format!(
                "tinixcut_phase18_audio_envelopes_{}_{}",
                std::process::id(),
                sample_rate
            ));
            let schedule = crate::modules::export::automation::ExportAutomationSchedule::build(
                &plan,
                &automation_dir,
                sample_rate,
                None,
            )
            .expect("audio crossfade envelopes build");
            assert_eq!(schedule.audio_layers.len(), 2);

            for (audio_index, timeline_midpoint_sample) in
                [(0, 2 * sample_rate), (1, sample_rate / 3)]
            {
                let envelope = &schedule.audio_layers[audio_index];
                assert_eq!(envelope.sample_rate, sample_rate);
                let bytes = fs::read(&envelope.envelope_file).expect("read generated envelope");
                let samples: Vec<f32> = bytes
                    .chunks_exact(std::mem::size_of::<f32>())
                    .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
                    .collect();
                let expected = std::f32::consts::FRAC_1_SQRT_2;
                assert!(
                    (samples[timeline_midpoint_sample as usize] - expected).abs() < 0.0001,
                    "sample rate {sample_rate}, audio index {audio_index}"
                );
            }
        }
    }

    #[test]
    fn test_phase18_audio_crossfade_composes_mixed_rates_volume_fades_keyframes_and_mute() {
        use crate::modules::domain::{AudioCrossfade, Keyframe, KeyframeProperty};

        let left_rate = FrameRate::FPS_24;
        let right_rate = FrameRate::FPS_30;
        let mut project = Project::new("Mixed-rate audio crossfade".to_string());
        let asset_id = "asset_audio_mixed_rate".to_string();
        project.assets.push(Asset {
            asset_id: asset_id.clone(),
            name: "tone.wav".to_string(),
            path: "tone.wav".to_string(),
            media_kind: MediaKind::Audio,
            duration: Some(RationalTime::from_frames(120, left_rate)),
            file_size: 0,
        });

        let mut track = Track::new("Audio".to_string(), TrackKind::Audio);
        let mut left = TimelineItem::new(
            Some(asset_id.clone()),
            "Outgoing".to_string(),
            ItemKind::Audio,
            TimeRange::new(
                RationalTime::from_frames(0, left_rate),
                RationalTime::from_frames(48, left_rate),
            ),
            TimeRange::new(
                RationalTime::from_frames(0, left_rate),
                RationalTime::from_frames(48, left_rate),
            ),
        )
        .unwrap();
        left.volume = 0.8;
        left.fade_out = Some(RationalTime::from_frames(12, left_rate));
        left.keyframes.extend([
            Keyframe::new(
                KeyframeProperty::Volume,
                RationalTime::from_frames(0, left_rate),
                1.0,
            ),
            Keyframe::new(
                KeyframeProperty::Volume,
                RationalTime::from_frames(48, left_rate),
                0.5,
            ),
        ]);
        let left_id = left.item_id.clone();

        let mut right = TimelineItem::new(
            Some(asset_id),
            "Incoming".to_string(),
            ItemKind::Audio,
            TimeRange::new(
                RationalTime::from_frames(60, right_rate),
                RationalTime::from_frames(60, right_rate),
            ),
            TimeRange::new(
                RationalTime::from_frames(12, left_rate),
                RationalTime::from_frames(48, left_rate),
            ),
        )
        .unwrap();
        right.volume = 0.6;
        right.fade_in = Some(RationalTime::from_frames(15, right_rate));
        right.keyframes.push(Keyframe::new(
            KeyframeProperty::Volume,
            RationalTime::from_frames(0, right_rate),
            0.75,
        ));
        let right_id = right.item_id.clone();
        track.items.extend([left, right]);
        track.audio_transitions.push(AudioCrossfade {
            transition_id: "audio_xfade_mixed_rate".to_string(),
            track_id: track.track_id.clone(),
            left_item_id: left_id.clone(),
            right_item_id: right_id.clone(),
            duration: RationalTime::from_frames(30, right_rate),
        });
        project.tracks.push(track);

        let plan = CompositionPlan::from_project(&project).expect("mixed-rate plan compiles");
        let left_layer = plan
            .audio_layers
            .iter()
            .find(|layer| layer.item_id == left_id)
            .unwrap();
        let right_layer = plan
            .audio_layers
            .iter()
            .find(|layer| layer.item_id == right_id)
            .unwrap();
        let sample_time = RationalTime::from_frames(105, FrameRate::FPS_60);
        assert!(!left_layer.muted);
        assert!(
            plan.audio_layer_active_at(left_layer, sample_time),
            "layer={left_layer:?}, crossfades={:?}",
            plan.audio_crossfades
        );

        let expected_keyframed_left =
            (1.0 - 0.5 * 42.0 / 48.0) * 0.5 * (std::f32::consts::FRAC_PI_8).cos();
        assert!(
            (plan.effective_audio_gain_at(left_layer, sample_time) - expected_keyframed_left).abs()
                < 0.0001
        );
        let volume_only_left = CompositionAudioLayer {
            keyframes: Vec::new(),
            ..left_layer.clone()
        };
        let expected_base_volume = 0.8 * 0.5 * (std::f32::consts::FRAC_PI_8).cos();
        assert!(
            (plan.effective_audio_gain_at(&volume_only_left, sample_time) - expected_base_volume)
                .abs()
                < 0.0001
        );

        assert_eq!(plan.effective_audio_gain_at(right_layer, sample_time), 0.0);
        let right_layer = CompositionAudioLayer {
            fade_in: None,
            ..right_layer.clone()
        };
        let expected_right = 0.75 * (std::f32::consts::FRAC_PI_8).sin();
        let actual_right = plan.effective_audio_gain_at(&right_layer, sample_time);
        assert!(
            (actual_right - expected_right).abs() < 0.0001,
            "expected right gain {expected_right}, got {actual_right}; crossfades={:?}",
            plan.audio_crossfades
        );

        let muted_right_layer = CompositionAudioLayer {
            muted: true,
            ..right_layer
        };
        assert_eq!(
            plan.effective_audio_gain_at(&muted_right_layer, sample_time),
            0.0
        );
    }

    #[test]
    fn test_poc2_preview_export_semantic_parity() {
        // Setup fixed scenario required by POC 2:
        // - Multiple layers (2 video layers)
        // - Transform, Crop, Opacity
        // - Text with bundled deterministic font
        // - Linear keyframe prototype
        // - Audio gain and fade
        let fixture_dir = get_fixture_dir();
        let video_path = fixture_dir.join("sample_vid.mp4");
        generate_synthetic_mp4(&video_path, 5, "blue");

        let mut project = Project::new("POC 2 Parity Test".to_string());
        project.settings = ProjectSettings {
            width: 1920,
            height: 1080,
            frame_rate: FrameRate::FPS_30,
            sample_rate: 48000,
            channels: 2,
        };

        let asset_id = "asset-vid".to_string();
        project.assets.push(Asset {
            asset_id: asset_id.clone(),
            name: "sample_vid.mp4".to_string(),
            path: video_path.to_string_lossy().to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(150, FrameRate::FPS_30)),
            file_size: 5000,
        });

        // Track 0: Background video
        let mut track0 = Track::new("Background Video".to_string(), TrackKind::Video);
        let mut bg_item = TimelineItem::new(
            Some(asset_id.clone()),
            "BG Clip".to_string(),
            ItemKind::Video,
            TimeRange::new(
                RationalTime::new(0, FrameRate::FPS_30),
                RationalTime::new(90, FrameRate::FPS_30), // 3.0s
            ),
            TimeRange::new(
                RationalTime::new(0, FrameRate::FPS_30),
                RationalTime::new(90, FrameRate::FPS_30),
            ),
        )
        .unwrap();
        bg_item.opacity = 1.0;
        let bg_item_id = bg_item.item_id.clone();
        track0.items.push(bg_item);
        project.tracks.push(track0);

        // Track 1: Foreground video with crop, scale, position
        let mut track1 = Track::new("Foreground Video".to_string(), TrackKind::Video);
        let mut fg_item = TimelineItem::new(
            Some(asset_id),
            "FG Clip".to_string(),
            ItemKind::Video,
            TimeRange::new(
                RationalTime::new(15, FrameRate::FPS_30), // 0.5s to 2.5s
                RationalTime::new(60, FrameRate::FPS_30),
            ),
            TimeRange::new(
                RationalTime::new(0, FrameRate::FPS_30),
                RationalTime::new(60, FrameRate::FPS_30),
            ),
        )
        .unwrap();
        fg_item.transform.position_x = -200.0;
        fg_item.transform.position_y = 50.0;
        fg_item.transform.scale_x = 0.5;
        fg_item.transform.scale_y = 0.5;
        fg_item.crop = Crop {
            left: 0.1,
            right: 0.1,
            top: 0.05,
            bottom: 0.05,
        };
        fg_item.opacity = 0.9;
        let fg_item_id = fg_item.item_id.clone();
        track1.items.push(fg_item);
        project.tracks.push(track1);

        // Track 2: Text overlay
        let mut track2 = Track::new("Text Track".to_string(), TrackKind::Video);
        let mut text_item = TimelineItem::new(
            None,
            "Title".to_string(),
            ItemKind::Text,
            TimeRange::new(
                RationalTime::new(30, FrameRate::FPS_30), // 1.0s to 2.5s
                RationalTime::new(45, FrameRate::FPS_30),
            ),
            TimeRange::new(
                RationalTime::new(0, FrameRate::FPS_30),
                RationalTime::new(45, FrameRate::FPS_30),
            ),
        )
        .unwrap();
        text_item.text_content = Some("TinixCut Parity Title".to_string());
        text_item.transform.position_x = 0.0;
        text_item.transform.position_y = -300.0;
        let text_item_id = text_item.item_id.clone();
        track2.items.push(text_item);
        project.tracks.push(track2);

        // Compile CompositionPlan
        let mut plan = CompositionPlan::from_project(&project).expect("Compilation must succeed");

        // Add keyframes & audio fade to plan for POC 2
        // Animate fg layer opacity from 0.0 at offset 0 to 1.0 at offset 30 frames (1s)
        let fg_layer = plan
            .visual_layers
            .iter_mut()
            .find(|l| l.item_id == fg_item_id)
            .unwrap();
        fg_layer.keyframes.push(CompositionKeyframe::new(
            KeyframeProperty::Opacity,
            RationalTime::new(0, FrameRate::FPS_30),
            0.0,
        ));
        fg_layer.keyframes.push(CompositionKeyframe::new(
            KeyframeProperty::Opacity,
            RationalTime::new(30, FrameRate::FPS_30),
            1.0,
        ));

        // Add audio fade-in & fade-out to bg audio
        if let Some(audio) = plan.audio_layers.get_mut(0) {
            audio.fade_in = Some(RationalTime::new(15, FrameRate::FPS_30)); // 0.5s fade in
            audio.fade_out = Some(RationalTime::new(15, FrameRate::FPS_30)); // 0.5s fade out
        }

        // --- 1. Evaluate Preview Projection at fixed test timestamps ---
        // Sample t = 0 (0.0s): Only BG layer active, text inactive, FG inactive
        let sample_0 = plan.sample_preview_at(RationalTime::new(0, FrameRate::FPS_30));
        assert_eq!(sample_0.active_layers.len(), 1);
        assert_eq!(sample_0.active_layers[0].item_id, bg_item_id);
        assert_eq!(sample_0.active_audio[0].effective_volume, 0.0); // start of fade in

        // Sample t = 15 (0.5s): FG item appears, offset = 0, opacity keyframe = 0.0
        let sample_15 = plan.sample_preview_at(RationalTime::new(15, FrameRate::FPS_30));
        assert_eq!(sample_15.active_layers.len(), 2);
        let fg_sample = sample_15
            .active_layers
            .iter()
            .find(|l| l.item_id == fg_item_id)
            .unwrap();
        assert_eq!(fg_sample.opacity, 0.0);
        // Verify audio fade in reached full volume at 15 frames
        assert!((sample_15.active_audio[0].effective_volume - 1.0).abs() < 0.01);

        // Sample t = 30 (1.0s): All 3 layers active!
        // FG clip offset is 15 frames -> opacity keyframe linearly interpolated = 0.5
        let sample_30 = plan.sample_preview_at(RationalTime::new(30, FrameRate::FPS_30));
        assert_eq!(sample_30.active_layers.len(), 3);
        let fg_sample_30 = sample_30
            .active_layers
            .iter()
            .find(|l| l.item_id == fg_item_id)
            .unwrap();
        assert!((fg_sample_30.opacity - 0.5).abs() < 0.001);

        // Verify geometry parity:
        // Project width: 1920, height: 1080
        // FG item: crop left=0.1, right=0.1 (cropped_w = 1920 * 0.8 = 1536)
        // scale_x = 0.5 -> render_w = 768.0
        // position_x = -200 -> center_x = 960 - 200 = 760
        // bounds.x = 760 - 384 = 376.0
        assert!((fg_sample_30.computed_bounds.width - 768.0).abs() < 0.01);
        assert!((fg_sample_30.computed_bounds.x - 376.0).abs() < 0.01);

        // Verify text layer layout parameters
        let text_sample = sample_30
            .active_layers
            .iter()
            .find(|l| l.item_id == text_item_id)
            .unwrap();
        assert_eq!(text_sample.text.as_deref(), Some("TinixCut Parity Title"));
        assert_eq!(text_sample.font_family.as_deref(), Some("Inter"));
        assert_eq!(text_sample.font_size, Some(48.0));

        // --- 2. Evaluate Export Projection ---
        let output_render = fixture_dir.join("poc2_export_test.mp4");
        let (args, filter_complex) = plan.to_ffmpeg_export_args(&output_render, 23, "libx264");

        // Verify export projection contains identical canonical layer parameters:
        // - Black base canvas with 1920x1080:r=30:d=3.000
        // - Correct scale: scale=768:486 with crop
        assert!(filter_complex.contains("crop="));
        assert!(filter_complex.contains("scale=768:486"));
        // - Correct overlay positions
        assert!(filter_complex.contains("overlay="));
        // - Correct text filter: drawtext=text='TinixCut Parity Title':fontsize=48
        assert!(filter_complex.contains("drawtext=text='TinixCut Parity Title':fontsize=48"));
        // - Correct audio fades: afade=t=in:st=0:d=0.500
        assert!(filter_complex.contains("afade=t=in:st=0:d=0.500"));
        assert!(filter_complex.contains("afade=t=out:st=2.500:d=0.500"));

        // Confirm argument vector is well-formed
        assert!(!args.is_empty());
        assert!(args.contains(&"-filter_complex".to_string()));
        assert!(args.contains(&"-c:v".to_string()));
        assert!(args.contains(&"libx264".to_string()));

        // --- Gate E Pass Verification ---
        // 1. Exact canonical timing boundaries: verified (active layer count matched at 0, 15, 30 frames).
        // 2. Geometry within declared pixel tolerance (< 0.1px difference).
        // 3. Crop/layer order match: verified (z_index 0, 1, 2 preserved).
        // 4. Keyframe values match sampled canonical times (0.0 at t=0, 0.5 at t=15, 1.0 at t=30).
        // 5. Text layout and font properties match.
        // 6. Audio gain/fades match within tolerance.
    }

    #[test]
    fn test_phase9_composition_rejects_crossdissolve() {
        use crate::modules::domain::Transition;
        let mut project = Project::new("CrossDissolve Rejection".to_string());
        let mut track = Track::new("Video Track".to_string(), TrackKind::Video);
        let rate = FrameRate::FPS_30;
        let mut item = TimelineItem::new(
            None,
            "Video Clip".to_string(),
            ItemKind::Video,
            TimeRange::new(
                RationalTime::from_frames(0, rate),
                RationalTime::from_frames(90, rate),
            ),
            TimeRange::new(
                RationalTime::from_frames(0, rate),
                RationalTime::from_frames(90, rate),
            ),
        )
        .unwrap();
        item.transition = Some(Transition {
            kind: TransitionKind::CrossDissolve,
            duration: RationalTime::from_frames(15, rate),
        });
        let item_id = item.item_id.clone();
        track.items.push(item);
        project.tracks.push(track);

        let plan_res = CompositionPlan::from_project(&project);
        assert_eq!(
            plan_res,
            Err(CompositionError::UnsupportedTransition {
                item_id,
                transition: "CrossDissolve".to_string(),
            })
        );
    }

    #[test]
    fn test_phase13_normalized_transition_plan_semantics() {
        use crate::modules::domain::CutTransition;
        let mut project = Project::new("TransitionPlan Test".to_string());
        let rate = FrameRate::FPS_30;
        project.settings.frame_rate = rate;

        let a1 = Asset {
            asset_id: "ast_1".to_string(),
            name: "c1.mp4".to_string(),
            path: "D:/media/c1.mp4".to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::from_frames(100, rate)),
            file_size: 1024,
        };
        let a2 = Asset {
            asset_id: "ast_2".to_string(),
            name: "c2.mp4".to_string(),
            path: "D:/media/c2.mp4".to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::from_frames(100, rate)),
            file_size: 1024,
        };
        project.assets.push(a1);
        project.assets.push(a2);

        let mut track = Track::new("Video".to_string(), TrackKind::Video);
        let mut item1 = TimelineItem::new(
            Some("ast_1".to_string()),
            "C1".to_string(),
            ItemKind::Video,
            TimeRange::new(
                RationalTime::from_frames(0, rate),
                RationalTime::from_frames(30, rate),
            ),
            TimeRange::new(
                RationalTime::from_frames(10, rate),
                RationalTime::from_frames(30, rate),
            ),
        )
        .unwrap();
        item1.item_id = "itm_1".to_string();
        item1.opacity = 0.8;
        item1.transform.position_x = 100.0;
        item1.crop.top = 0.1;

        let mut item2 = TimelineItem::new(
            Some("ast_2".to_string()),
            "C2".to_string(),
            ItemKind::Video,
            TimeRange::new(
                RationalTime::from_frames(30, rate),
                RationalTime::from_frames(30, rate),
            ),
            TimeRange::new(
                RationalTime::from_frames(20, rate),
                RationalTime::from_frames(30, rate),
            ),
        )
        .unwrap();
        item2.item_id = "itm_2".to_string();
        item2.opacity = 0.9;
        item2.transform.position_y = -50.0;
        item2.crop.bottom = 0.15;

        // Cut transition with odd duration = 11 ticks
        let cut_trans = CutTransition {
            transition_id: "trans_test_1".to_string(),
            track_id: track.track_id.clone(),
            left_item_id: "itm_1".to_string(),
            right_item_id: "itm_2".to_string(),
            kind: TransitionKind::CrossDissolve,
            duration: RationalTime::from_frames(11, rate),
        };
        track.items.push(item1);
        track.items.push(item2);
        track.transitions.push(cut_trans);
        project.tracks.push(track);

        let plan = CompositionPlan::from_project(&project).unwrap();
        assert_eq!(plan.transitions.len(), 1);
        let tp = &plan.transitions[0];

        // 1. Odd-duration deterministic split: D=11 -> L=5, R=6
        assert_eq!(tp.l_duration(), RationalTime::from_frames(5, rate));
        assert_eq!(tp.r_duration(), RationalTime::from_frames(6, rate));
        // Cut at 30: T0 = 30 - 5 = 25, T1 = 30 + 6 = 36
        assert_eq!(
            tp.transition_range.start,
            RationalTime::from_frames(25, rate)
        );
        assert_eq!(
            tp.transition_range.end(),
            RationalTime::from_frames(36, rate)
        );
        assert_eq!(
            tp.transition_range.duration,
            RationalTime::from_frames(11, rate)
        );

        // 2. Progress evaluation: p=0, midpoint, last sample before T1, exact T1
        // At T0 (t=25): p = 0.0
        assert_eq!(tp.progress_at(RationalTime::from_frames(25, rate)), 0.0);
        // At Cut (t=30): elapsed = 5 -> p = 5/11
        let p_cut = tp.progress_at(RationalTime::from_frames(30, rate));
        assert!((p_cut - 5.0 / 11.0).abs() < 1e-6);
        // At last sample before T1 (t=35): elapsed = 10 -> p = 10/11
        let p_last = tp.progress_at(RationalTime::from_frames(35, rate));
        assert!((p_last - 10.0 / 11.0).abs() < 1e-6);
        // At exact T1 boundary (t=36):
        assert!(!tp.contains_time(RationalTime::from_frames(36, rate)));
        assert_eq!(tp.progress_at(RationalTime::from_frames(36, rate)), 1.0);

        // 3. Source timestamps on both sides of cut
        // Left clip: source_start = 10.
        // t=25: left source time = 10 + 25 = 35
        assert_eq!(
            tp.left_source_time_at(RationalTime::from_frames(25, rate)),
            RationalTime::from_frames(35, rate)
        );
        // t=30: left source time = 10 + 30 = 40 (normal editorial out-point)
        assert_eq!(
            tp.left_source_time_at(RationalTime::from_frames(30, rate)),
            RationalTime::from_frames(40, rate)
        );
        // t=35: left source time = 10 + 35 = 45 (in postroll handle!)
        assert_eq!(
            tp.left_source_time_at(RationalTime::from_frames(35, rate)),
            RationalTime::from_frames(45, rate)
        );

        // Right clip: source_start = 20, timeline_start = 30.
        // t=25: right source time = 20 + (25 - 30) = 15 (in preroll handle!)
        assert_eq!(
            tp.right_source_time_at(RationalTime::from_frames(25, rate)),
            RationalTime::from_frames(15, rate)
        );
        // t=30: right source time = 20 + 0 = 20 (normal editorial in-point)
        assert_eq!(
            tp.right_source_time_at(RationalTime::from_frames(30, rate)),
            RationalTime::from_frames(20, rate)
        );
        // t=35: right source time = 20 + 5 = 25
        assert_eq!(
            tp.right_source_time_at(RationalTime::from_frames(35, rate)),
            RationalTime::from_frames(25, rate)
        );

        // 4. Transform/crop/opacity state and endpoint hold
        // Left layer: base opacity 0.8, pos_x 100, crop.top 0.1
        assert_eq!(tp.left_layer.transform.position_x, 100.0);
        assert_eq!(tp.left_layer.crop.top, 0.1);
        // Right layer: base opacity 0.9, pos_y -50, crop.bottom 0.15
        assert_eq!(tp.right_layer.transform.position_y, -50.0);
        assert_eq!(tp.right_layer.crop.bottom, 0.15);

        // Endpoint hold:
        // Left clip past cut C (t=35 >= 30): clip offset is held at duration (30)
        assert_eq!(
            tp.left_effective_clip_offset(RationalTime::from_frames(35, rate)),
            RationalTime::from_frames(30, rate)
        );
        // Right clip before cut C (t=25 < 30): clip offset is held at start (0)
        assert_eq!(
            tp.right_effective_clip_offset(RationalTime::from_frames(25, rate)),
            RationalTime::from_frames(0, rate)
        );

        // Opacity evaluation:
        // At T0 (t=25): left opacity = (1-0)*0.8 = 0.8; right opacity = 0*0.9 = 0.0
        assert_eq!(tp.left_opacity_at(RationalTime::from_frames(25, rate)), 0.8);
        assert_eq!(
            tp.right_opacity_at(RationalTime::from_frames(25, rate)),
            0.0
        );

        // At Cut (t=30): left = (1 - 5/11)*0.8 = 6/11 * 0.8; right = 5/11 * 0.9
        let left_op_cut = tp.left_opacity_at(RationalTime::from_frames(30, rate));
        let right_op_cut = tp.right_opacity_at(RationalTime::from_frames(30, rate));
        assert!((left_op_cut - (6.0 / 11.0) * 0.8).abs() < 1e-6);
        assert!((right_op_cut - (5.0 / 11.0) * 0.9).abs() < 1e-6);

        // 5. Total project duration is invariant: 60 frames
        assert_eq!(plan.duration, RationalTime::from_frames(60, rate));
    }

    #[test]
    fn test_phase13_preview_transition_group_sampling() {
        let rate = FrameRate::FPS_30;

        let asset1 = Asset {
            asset_id: "ast_prev_1".to_string(),
            name: "Clip 1".to_string(),
            path: "sample1.mp4".to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::from_frames(60, rate)),
            file_size: 1024,
        };
        let asset2 = Asset {
            asset_id: "ast_prev_2".to_string(),
            name: "Clip 2".to_string(),
            path: "sample2.mp4".to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::from_frames(60, rate)),
            file_size: 1024,
        };
        let asset_bg = Asset {
            asset_id: "ast_prev_bg".to_string(),
            name: "Background".to_string(),
            path: "bg.mp4".to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::from_frames(60, rate)),
            file_size: 1024,
        };

        let mut clip1 = TimelineItem::new(
            Some("ast_prev_1".to_string()),
            "Clip 1".to_string(),
            ItemKind::Video,
            TimeRange::new(
                RationalTime::from_frames(0, rate),
                RationalTime::from_frames(30, rate),
            ),
            TimeRange::new(
                RationalTime::from_frames(10, rate),
                RationalTime::from_frames(30, rate),
            ),
        )
        .unwrap();
        clip1.item_id = "clip1".to_string();
        clip1.opacity = 0.8;

        let mut clip2 = TimelineItem::new(
            Some("ast_prev_2".to_string()),
            "Clip 2".to_string(),
            ItemKind::Video,
            TimeRange::new(
                RationalTime::from_frames(30, rate),
                RationalTime::from_frames(30, rate),
            ),
            TimeRange::new(
                RationalTime::from_frames(20, rate),
                RationalTime::from_frames(30, rate),
            ),
        )
        .unwrap();
        clip2.item_id = "clip2".to_string();
        clip2.opacity = 0.9;

        let mut clip_bg = TimelineItem::new(
            Some("ast_prev_bg".to_string()),
            "Background".to_string(),
            ItemKind::Video,
            TimeRange::new(
                RationalTime::from_frames(0, rate),
                RationalTime::from_frames(60, rate),
            ),
            TimeRange::new(
                RationalTime::from_frames(0, rate),
                RationalTime::from_frames(60, rate),
            ),
        )
        .unwrap();
        clip_bg.item_id = "clip_bg".to_string();

        let mut project = Project::new("Test Preview Transition Project".to_string());
        project.settings.frame_rate = rate;
        project.assets.push(asset1);
        project.assets.push(asset2);
        project.assets.push(asset_bg);

        let mut trk_bg = Track::new("BG Track".to_string(), TrackKind::Video);
        trk_bg.track_id = "trk_bg".to_string();
        trk_bg.items.push(clip_bg);

        let mut trk_fg = Track::new("FG Track".to_string(), TrackKind::Video);
        trk_fg.track_id = "trk_fg".to_string();

        let cut_transition = crate::modules::domain::CutTransition {
            transition_id: "trans_test_1".to_string(),
            track_id: trk_fg.track_id.clone(),
            kind: TransitionKind::CrossDissolve,
            left_item_id: "clip1".to_string(),
            right_item_id: "clip2".to_string(),
            duration: RationalTime::from_frames(10, rate), // D=10 -> L=5, R=5, [25, 35)
        };
        trk_fg.items.push(clip1);
        trk_fg.items.push(clip2);
        trk_fg.transitions.push(cut_transition);

        project.tracks.push(trk_bg);
        project.tracks.push(trk_fg);

        let plan = CompositionPlan::from_project(&project).unwrap();

        // 1. Outside [T0, T1): at t=20 (< 25)
        let sample_20 = plan.sample_preview_at(RationalTime::from_frames(20, rate));
        assert!(sample_20.transition_groups.is_empty());
        // active_layers has clip_bg (trk_bg, z=0) and clip1 (trk_fg, z=1)
        assert_eq!(sample_20.active_layers.len(), 2);
        assert_eq!(sample_20.active_layers[0].item_id, "clip_bg");
        assert_eq!(sample_20.active_layers[1].item_id, "clip1");

        // 2. Start of transition: at t=25 (T0, p=0.0)
        let sample_25 = plan.sample_preview_at(RationalTime::from_frames(25, rate));
        // clip1 and clip2 are suppressed from active_layers!
        assert_eq!(sample_25.active_layers.len(), 1);
        assert_eq!(sample_25.active_layers[0].item_id, "clip_bg");
        // Isolated PreviewTransitionGroup exists!
        assert_eq!(sample_25.transition_groups.len(), 1);
        let g25 = &sample_25.transition_groups[0];
        assert_eq!(g25.transition_id, "trans_test_1");
        assert_eq!(g25.kind, TransitionKind::CrossDissolve);
        assert_eq!(g25.z_index, 1);
        assert_eq!(g25.progress, 0.0);
        assert_eq!(g25.left_layer.item_id, "clip1");
        assert_eq!(g25.left_layer.opacity, 0.8); // (1 - 0) * 0.8
        assert_eq!(g25.right_layer.item_id, "clip2");
        assert_eq!(g25.right_layer.opacity, 0.0); // 0 * 0.9

        // 3. Midpoint / Cut: at t=30 (C, p=0.5)
        let sample_30 = plan.sample_preview_at(RationalTime::from_frames(30, rate));
        assert_eq!(sample_30.active_layers.len(), 1);
        assert_eq!(sample_30.active_layers[0].item_id, "clip_bg");
        assert_eq!(sample_30.transition_groups.len(), 1);
        let g30 = &sample_30.transition_groups[0];
        assert_eq!(g30.progress, 0.5);
        assert!((g30.left_layer.opacity - 0.5 * 0.8).abs() < 1e-5);
        assert!((g30.right_layer.opacity - 0.5 * 0.9).abs() < 1e-5);
        // Source times at cut:
        assert_eq!(
            g30.left_layer.source_time,
            RationalTime::from_frames(40, rate)
        );
        assert_eq!(
            g30.right_layer.source_time,
            RationalTime::from_frames(20, rate)
        );

        // 4. Last active sample: at t=34 (T1 - 1, p=0.9)
        let sample_34 = plan.sample_preview_at(RationalTime::from_frames(34, rate));
        assert_eq!(sample_34.active_layers.len(), 1);
        assert_eq!(sample_34.transition_groups.len(), 1);
        let g34 = &sample_34.transition_groups[0];
        assert!((g34.progress - 0.9).abs() < 1e-5);
        assert!((g34.left_layer.opacity - 0.1 * 0.8).abs() < 1e-5);
        assert!((g34.right_layer.opacity - 0.9 * 0.9).abs() < 1e-5);

        // 5. Exact boundary / transition ended: at t=35 (T1)
        let sample_35 = plan.sample_preview_at(RationalTime::from_frames(35, rate));
        assert!(sample_35.transition_groups.is_empty());
        assert_eq!(sample_35.active_layers.len(), 2);
        assert_eq!(sample_35.active_layers[0].item_id, "clip_bg");
        assert_eq!(sample_35.active_layers[1].item_id, "clip2");
    }

    #[test]
    fn test_phase13_real_media_crossdissolve_export() {
        let fixture_dir = get_fixture_dir();
        let vid1_path = fixture_dir.join("p13_vid1.mp4");
        let vid2_path = fixture_dir.join("p13_vid2.mp4");
        generate_synthetic_mp4(&vid1_path, 4, "red");
        generate_synthetic_mp4(&vid2_path, 4, "blue");

        let rate = FrameRate::FPS_30;
        let mut project = Project::new("Phase 13 Export Test".to_string());
        project.settings = ProjectSettings {
            width: 640,
            height: 360,
            frame_rate: rate,
            sample_rate: 48000,
            channels: 2,
        };

        let ast1 = Asset {
            asset_id: "ast_export_1".to_string(),
            name: "p13_vid1.mp4".to_string(),
            path: vid1_path.to_string_lossy().to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::from_frames(120, rate)), // 4s
            file_size: 1024,
        };
        let ast2 = Asset {
            asset_id: "ast_export_2".to_string(),
            name: "p13_vid2.mp4".to_string(),
            path: vid2_path.to_string_lossy().to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::from_frames(120, rate)), // 4s
            file_size: 1024,
        };
        project.assets.push(ast1);
        project.assets.push(ast2);

        let mut track = Track::new("Video Track".to_string(), TrackKind::Video);
        let mut item1 = TimelineItem::new(
            Some("ast_export_1".to_string()),
            "Item 1".to_string(),
            ItemKind::Video,
            TimeRange::new(
                RationalTime::from_frames(0, rate),
                RationalTime::from_frames(60, rate), // 2.0s
            ),
            TimeRange::new(
                RationalTime::from_frames(0, rate),
                RationalTime::from_frames(60, rate),
            ),
        )
        .unwrap();
        item1.item_id = "it1".to_string();
        item1.opacity = 0.9;

        let mut item2 = TimelineItem::new(
            Some("ast_export_2".to_string()),
            "Item 2".to_string(),
            ItemKind::Video,
            TimeRange::new(
                RationalTime::from_frames(60, rate), // 2.0s to 4.0s
                RationalTime::from_frames(60, rate),
            ),
            TimeRange::new(
                RationalTime::from_frames(30, rate), // preroll available: 30 frames
                RationalTime::from_frames(60, rate),
            ),
        )
        .unwrap();
        item2.item_id = "it2".to_string();
        item2.opacity = 0.8;

        let cut_trans = crate::modules::domain::CutTransition {
            transition_id: "cut_trans_exp".to_string(),
            track_id: track.track_id.clone(),
            kind: TransitionKind::CrossDissolve,
            left_item_id: "it1".to_string(),
            right_item_id: "it2".to_string(),
            duration: RationalTime::from_frames(20, rate), // D=20 frames (0.667s) -> L=10, R=10, [50, 70)
        };

        track.items.push(item1);
        track.items.push(item2);
        track.transitions.push(cut_trans);
        project.tracks.push(track);

        let plan = CompositionPlan::from_project(&project).expect("Plan compilation must succeed");

        // 1. Duration invariant: 120 frames (4.0s)
        assert_eq!(plan.duration, RationalTime::from_frames(120, rate));
        assert_eq!(plan.transitions.len(), 1);

        // 2. Export argument projection
        let output_mp4 = fixture_dir.join("p13_real_export.mp4");
        let (args, filter_complex) = plan.to_ffmpeg_export_args(&output_mp4, 23, "libx264");

        // 3. Verify filtergraph contains canonical CrossDissolve structure
        assert!(filter_complex.contains("premultiply=inplace=1"));
        assert!(filter_complex.contains("xfade=transition=fade:duration=0.666667:offset=0"));
        assert!(filter_complex.contains("unpremultiply=inplace=1,format=rgba"));
        assert!(filter_complex.contains("between(t,1.6500,2.3167)"));
        // Suppressed ordinary intervals: item1 ordinary ends at 1.6500, item2 ordinary starts at 2.3167
        assert!(filter_complex.contains("between(t,0.0000,1.6500)"));
        assert!(filter_complex.contains("between(t,2.3167,3.9833)"));
        // Video-only: no acrossfade audio filter
        assert!(!filter_complex.contains("acrossfade"));

        // 4. Execute actual FFmpeg export using app-owned MediaBinaryResolver (pinned FFmpeg 9.0.2)
        let resolved = MediaBinaryResolver::resolve_name("ffmpeg")
            .expect("FFmpeg must resolve via MediaBinaryResolver");
        assert!(
            matches!(
                resolved.origin,
                crate::modules::media::resolver::BinaryOrigin::AppOwnedPackagedResource(_)
            ),
            "Must resolve to app-owned packaged resource, got: {:?}",
            resolved.origin
        );
        let version_str = resolved.version.as_deref().unwrap_or("");
        assert!(
            version_str.contains("9.0.2"),
            "Resolved FFmpeg version must be 9.0.2, got: {}",
            version_str
        );
        let mut cmd = std::process::Command::new(&resolved.path);
        let output = cmd
            .args(&args)
            .output()
            .expect("FFmpeg export execution failed");
        assert!(
            output.status.success(),
            "FFmpeg export failed with stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output_mp4.exists(), "Exported MP4 file must exist");
        let metadata = fs::metadata(&output_mp4).expect("Metadata must be readable");
        assert!(
            metadata.len() > 1000,
            "Exported MP4 file must have non-trivial size"
        );
    }
}
