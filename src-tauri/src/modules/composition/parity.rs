//! Preview / Export Parity POC (POC 2) Module — Phase 7
//!
//! Provides the verification harness, deterministic fixture projects,
//! semantic snapshot comparisons, and raster diffing between
//! the real preview backend and the FFmpeg export backend.
//!
//! Architectural Invariant: Both backends must consume the same normalized
//! composition semantics without semantic drift.

use crate::modules::composition::{CompositionPlan, ItemKind, Rect};
use crate::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use crate::modules::domain::{
    Asset, Crop, MediaKind, Project, ProjectSettings, TimelineItem, Track, TrackKind,
};
use crate::modules::media::resolver::MediaBinaryResolver;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;

/// Numerical differences between preview and export captures
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParityDiffMetrics {
    pub mae: f64,
    pub max_error: u8,
    pub exceeded_ratio: f64,
    pub width: u32,
    pub height: u32,
    pub total_pixels: usize,
    pub exceeded_pixels: usize,
}

/// Normalized layer state captured for parity comparison
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParityLayerSnapshot {
    pub item_id: String,
    pub name: String,
    pub z_index: usize,
    pub kind: ItemKind,
    pub asset_id: Option<String>,
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

/// Normalized audio layer state captured for parity comparison
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParityAudioSnapshot {
    pub item_id: String,
    pub effective_volume: f32,
}

/// Normalized composition semantic snapshot at project time T
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParitySemanticSnapshot {
    pub project_id: String,
    pub project_time: RationalTime,
    pub canvas: (u32, u32),
    pub layers: Vec<ParityLayerSnapshot>,
    pub audio: Vec<ParityAudioSnapshot>,
}

/// Result of evaluating a single parity fixture
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParityFixtureResult {
    pub fixture_id: String,
    pub name: String,
    pub timestamp_frames: i64,
    pub semantic_status: String,
    pub visual_status: String,
    pub overall_status: String,
    pub metrics: Option<ParityDiffMetrics>,
    pub details: String,
    pub preview_artifact: Option<String>,
    pub export_artifact: Option<String>,
    pub diff_artifact: Option<String>,
}

/// Overall report returned by the POC 2 parity suite
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParitySuiteReport {
    pub total_fixtures: usize,
    pub passed_fixtures: usize,
    pub failed_fixtures: usize,
    pub negative_control_passed: bool,
    pub overall_decision: String,
    pub fixture_results: Vec<ParityFixtureResult>,
}

impl ParitySemanticSnapshot {
    /// Builds a semantic snapshot from a preview sample
    pub fn from_preview(plan: &CompositionPlan, timestamp: RationalTime) -> Self {
        let state = plan.sample_preview_at(timestamp);
        let mut layers: Vec<ParityLayerSnapshot> = state
            .active_layers
            .into_iter()
            .map(|l| ParityLayerSnapshot {
                item_id: l.item_id,
                name: l.name,
                z_index: l.z_index,
                kind: l.kind,
                asset_id: l.asset_id,
                source_time: l.source_time,
                position: l.position,
                scale: l.scale,
                rotation: l.rotation,
                crop: l.crop,
                opacity: l.opacity,
                text: l.text,
                font_family: l.font_family,
                font_size: l.font_size,
                computed_bounds: l.computed_bounds,
            })
            .collect();

        for grp in state.transition_groups {
            for l in [grp.left_layer, grp.right_layer] {
                layers.push(ParityLayerSnapshot {
                    item_id: l.item_id,
                    name: l.name,
                    z_index: l.z_index,
                    kind: l.kind,
                    asset_id: l.asset_id,
                    source_time: l.source_time,
                    position: l.position,
                    scale: l.scale,
                    rotation: l.rotation,
                    crop: l.crop,
                    opacity: l.opacity,
                    text: l.text,
                    font_family: l.font_family,
                    font_size: l.font_size,
                    computed_bounds: l.computed_bounds,
                });
            }
        }
        layers.sort_by_key(|l| (l.z_index, l.item_id.clone()));

        let audio: Vec<ParityAudioSnapshot> = state
            .active_audio
            .into_iter()
            .map(|a| ParityAudioSnapshot {
                item_id: a.item_id,
                effective_volume: a.effective_volume,
            })
            .collect();

        Self {
            project_id: plan.project_id.clone(),
            project_time: timestamp,
            canvas: (plan.width, plan.height),
            layers,
            audio,
        }
    }

    /// Builds a semantic snapshot from the composition plan using export-derived active layers
    pub fn from_export(plan: &CompositionPlan, timestamp: RationalTime) -> Self {
        let mut layers = Vec::new();
        let mut suppressed_item_ids = std::collections::HashSet::new();

        let base_w = plan.width as f32;
        let base_h = plan.height as f32;

        for trans in &plan.transitions {
            if trans.contains_time(timestamp) {
                suppressed_item_ids.insert(trans.left_item_id.clone());
                suppressed_item_ids.insert(trans.right_item_id.clone());

                // Left participant
                let left_offset = trans.left_effective_clip_offset(timestamp);
                let left_source_time = trans.left_source_time_at(timestamp);
                let left_opacity = trans.left_opacity_at(timestamp);
                let l_layer = &trans.left_layer;
                let l_pos_x = l_layer.evaluate_property(
                    crate::modules::domain::KeyframeProperty::PositionX,
                    left_offset,
                );
                let l_pos_y = l_layer.evaluate_property(
                    crate::modules::domain::KeyframeProperty::PositionY,
                    left_offset,
                );
                let l_scale_x = l_layer.evaluate_property(
                    crate::modules::domain::KeyframeProperty::ScaleX,
                    left_offset,
                );
                let l_scale_y = l_layer.evaluate_property(
                    crate::modules::domain::KeyframeProperty::ScaleY,
                    left_offset,
                );
                let l_rot = l_layer.evaluate_property(
                    crate::modules::domain::KeyframeProperty::Rotation,
                    left_offset,
                );
                let l_crop_w = base_w * (1.0 - l_layer.crop.left - l_layer.crop.right);
                let l_crop_h = base_h * (1.0 - l_layer.crop.top - l_layer.crop.bottom);
                let l_rw = l_crop_w * l_scale_x;
                let l_rh = l_crop_h * l_scale_y;
                let l_cx = (base_w / 2.0) + l_pos_x;
                let l_cy = (base_h / 2.0) + l_pos_y;
                layers.push(ParityLayerSnapshot {
                    item_id: l_layer.item_id.clone(),
                    name: l_layer.name.clone(),
                    z_index: l_layer.z_index,
                    kind: l_layer.kind,
                    asset_id: l_layer.asset_id.clone(),
                    source_time: left_source_time,
                    position: (l_pos_x, l_pos_y),
                    scale: (l_scale_x, l_scale_y),
                    rotation: l_rot,
                    crop: l_layer.crop.clone(),
                    opacity: left_opacity,
                    text: l_layer.text_content.clone(),
                    font_family: l_layer.font_family.clone(),
                    font_size: l_layer.font_size,
                    computed_bounds: Rect {
                        x: l_cx - (l_rw / 2.0),
                        y: l_cy - (l_rh / 2.0),
                        width: l_rw,
                        height: l_rh,
                    },
                });

                // Right participant
                let right_offset = trans.right_effective_clip_offset(timestamp);
                let right_source_time = trans.right_source_time_at(timestamp);
                let right_opacity = trans.right_opacity_at(timestamp);
                let r_layer = &trans.right_layer;
                let r_pos_x = r_layer.evaluate_property(
                    crate::modules::domain::KeyframeProperty::PositionX,
                    right_offset,
                );
                let r_pos_y = r_layer.evaluate_property(
                    crate::modules::domain::KeyframeProperty::PositionY,
                    right_offset,
                );
                let r_scale_x = r_layer.evaluate_property(
                    crate::modules::domain::KeyframeProperty::ScaleX,
                    right_offset,
                );
                let r_scale_y = r_layer.evaluate_property(
                    crate::modules::domain::KeyframeProperty::ScaleY,
                    right_offset,
                );
                let r_rot = r_layer.evaluate_property(
                    crate::modules::domain::KeyframeProperty::Rotation,
                    right_offset,
                );
                let r_crop_w = base_w * (1.0 - r_layer.crop.left - r_layer.crop.right);
                let r_crop_h = base_h * (1.0 - r_layer.crop.top - r_layer.crop.bottom);
                let r_rw = r_crop_w * r_scale_x;
                let r_rh = r_crop_h * r_scale_y;
                let r_cx = (base_w / 2.0) + r_pos_x;
                let r_cy = (base_h / 2.0) + r_pos_y;
                layers.push(ParityLayerSnapshot {
                    item_id: r_layer.item_id.clone(),
                    name: r_layer.name.clone(),
                    z_index: r_layer.z_index,
                    kind: r_layer.kind,
                    asset_id: r_layer.asset_id.clone(),
                    source_time: right_source_time,
                    position: (r_pos_x, r_pos_y),
                    scale: (r_scale_x, r_scale_y),
                    rotation: r_rot,
                    crop: r_layer.crop.clone(),
                    opacity: right_opacity,
                    text: r_layer.text_content.clone(),
                    font_family: r_layer.font_family.clone(),
                    font_size: r_layer.font_size,
                    computed_bounds: Rect {
                        x: r_cx - (r_rw / 2.0),
                        y: r_cy - (r_rh / 2.0),
                        width: r_rw,
                        height: r_rh,
                    },
                });
            }
        }

        for layer in &plan.visual_layers {
            if suppressed_item_ids.contains(&layer.item_id) {
                continue;
            }
            if layer.timeline_range.contains_time(timestamp) {
                let clip_offset = RationalTime::new(
                    timestamp.value - layer.timeline_range.start.value,
                    timestamp.rate,
                );
                let source_time = RationalTime::new(
                    layer.source_range.start.value + clip_offset.value,
                    timestamp.rate,
                );

                let pos_x = layer.evaluate_property(
                    crate::modules::domain::KeyframeProperty::PositionX,
                    clip_offset,
                );
                let pos_y = layer.evaluate_property(
                    crate::modules::domain::KeyframeProperty::PositionY,
                    clip_offset,
                );
                let scale_x = layer.evaluate_property(
                    crate::modules::domain::KeyframeProperty::ScaleX,
                    clip_offset,
                );
                let scale_y = layer.evaluate_property(
                    crate::modules::domain::KeyframeProperty::ScaleY,
                    clip_offset,
                );
                let rotation = layer.evaluate_property(
                    crate::modules::domain::KeyframeProperty::Rotation,
                    clip_offset,
                );
                let opacity = layer.effective_opacity_at(clip_offset);

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

                layers.push(ParityLayerSnapshot {
                    item_id: layer.item_id.clone(),
                    name: layer.name.clone(),
                    z_index: layer.z_index,
                    kind: layer.kind,
                    asset_id: layer.asset_id.clone(),
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
                });
            }
        }

        layers.sort_by_key(|l| (l.z_index, l.item_id.clone()));

        let mut audio = Vec::new();
        for audio_layer in &plan.audio_layers {
            if plan.audio_layer_active_at(audio_layer, timestamp) {
                audio.push(ParityAudioSnapshot {
                    item_id: audio_layer.item_id.clone(),
                    effective_volume: plan.effective_audio_gain_at(audio_layer, timestamp),
                });
            }
        }

        Self {
            project_id: plan.project_id.clone(),
            project_time: timestamp,
            canvas: (plan.width, plan.height),
            layers,
            audio,
        }
    }
}

/// Compares two semantic snapshots and reports any discrepancies
pub fn compare_semantic_snapshots(
    s1: &ParitySemanticSnapshot,
    s2: &ParitySemanticSnapshot,
    tolerance_px: f32,
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if s1.project_time != s2.project_time {
        errors.push(format!(
            "Timestamp mismatch: {:?} vs {:?}",
            s1.project_time, s2.project_time
        ));
    }
    if s1.canvas != s2.canvas {
        errors.push(format!(
            "Canvas dimensions mismatch: {:?} vs {:?}",
            s1.canvas, s2.canvas
        ));
    }
    if s1.layers.len() != s2.layers.len() {
        errors.push(format!(
            "Active layer count mismatch: preview has {}, export has {}",
            s1.layers.len(),
            s2.layers.len()
        ));
        return Err(errors);
    }

    for (i, (l1, l2)) in s1.layers.iter().zip(s2.layers.iter()).enumerate() {
        if l1.item_id != l2.item_id {
            errors.push(format!(
                "Layer [{i}] item_id mismatch: '{}' vs '{}'",
                l1.item_id, l2.item_id
            ));
        }
        if l1.z_index != l2.z_index {
            errors.push(format!(
                "Layer [{i}] z_index mismatch: {} vs {}",
                l1.z_index, l2.z_index
            ));
        }
        if l1.source_time != l2.source_time {
            errors.push(format!(
                "Layer [{i}] source_time mismatch: {:?} vs {:?}",
                l1.source_time, l2.source_time
            ));
        }
        if (l1.position.0 - l2.position.0).abs() > tolerance_px
            || (l1.position.1 - l2.position.1).abs() > tolerance_px
        {
            errors.push(format!(
                "Layer [{i}] position mismatch: {:?} vs {:?}",
                l1.position, l2.position
            ));
        }
        if (l1.scale.0 - l2.scale.0).abs() > 0.001 || (l1.scale.1 - l2.scale.1).abs() > 0.001 {
            errors.push(format!(
                "Layer [{i}] scale mismatch: {:?} vs {:?}",
                l1.scale, l2.scale
            ));
        }
        if (l1.rotation - l2.rotation).abs() > 0.01 {
            errors.push(format!(
                "Layer [{i}] rotation mismatch: {} vs {}",
                l1.rotation, l2.rotation
            ));
        }
        if (l1.opacity - l2.opacity).abs() > 0.01 {
            errors.push(format!(
                "Layer [{i}] opacity mismatch: {} vs {}",
                l1.opacity, l2.opacity
            ));
        }
        if l1.text != l2.text {
            errors.push(format!(
                "Layer [{i}] text mismatch: {:?} vs {:?}",
                l1.text, l2.text
            ));
        }
        if (l1.computed_bounds.x - l2.computed_bounds.x).abs() > tolerance_px
            || (l1.computed_bounds.y - l2.computed_bounds.y).abs() > tolerance_px
            || (l1.computed_bounds.width - l2.computed_bounds.width).abs() > tolerance_px
            || (l1.computed_bounds.height - l2.computed_bounds.height).abs() > tolerance_px
        {
            errors.push(format!(
                "Layer [{i}] bounds mismatch: {:?} vs {:?}",
                l1.computed_bounds, l2.computed_bounds
            ));
        }
    }

    if s1.audio.len() != s2.audio.len() {
        errors.push(format!(
            "Active audio count mismatch: preview has {}, export has {}",
            s1.audio.len(),
            s2.audio.len()
        ));
    } else {
        for (i, (a1, a2)) in s1.audio.iter().zip(s2.audio.iter()).enumerate() {
            if a1.item_id != a2.item_id {
                errors.push(format!(
                    "Audio [{i}] item_id mismatch: '{}' vs '{}'",
                    a1.item_id, a2.item_id
                ));
            }
            if (a1.effective_volume - a2.effective_volume).abs() > 0.01 {
                errors.push(format!(
                    "Audio [{i}] effective_volume mismatch on '{}': preview={:.3}, export={:.3}",
                    a1.item_id, a1.effective_volume, a2.effective_volume
                ));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Evaluates raster parity across two RGBA buffers
pub fn compare_rgba_buffers(
    preview_rgba: &[u8],
    export_rgba: &[u8],
    width: u32,
    height: u32,
    threshold: u8,
    max_mae: f64,
    max_exceeded_ratio: f64,
) -> (bool, ParityDiffMetrics, Vec<u8>) {
    let total_pixels = (width * height) as usize;
    let expected_bytes = total_pixels * 4;

    if preview_rgba.len() != expected_bytes || export_rgba.len() != expected_bytes {
        let metrics = ParityDiffMetrics {
            mae: 255.0,
            max_error: 255,
            exceeded_ratio: 1.0,
            width,
            height,
            total_pixels,
            exceeded_pixels: total_pixels,
        };
        return (false, metrics, vec![255u8; expected_bytes]);
    }

    let mut sum_diff = 0.0;
    let mut max_err: u8 = 0;
    let mut exceeded_count = 0;
    let mut diff_rgba = vec![0u8; expected_bytes];

    for i in 0..total_pixels {
        let idx = i * 4;
        let pr = preview_rgba[idx];
        let pg = preview_rgba[idx + 1];
        let pb = preview_rgba[idx + 2];

        let er = export_rgba[idx];
        let eg = export_rgba[idx + 1];
        let eb = export_rgba[idx + 2];

        let dr = (pr as i16 - er as i16).unsigned_abs() as u8;
        let dg = (pg as i16 - eg as i16).unsigned_abs() as u8;
        let db = (pb as i16 - eb as i16).unsigned_abs() as u8;

        let pixel_max = dr.max(dg).max(db);
        if pixel_max > max_err {
            max_err = pixel_max;
        }

        let pixel_avg = (dr as f64 + dg as f64 + db as f64) / 3.0;
        sum_diff += pixel_avg;

        if pixel_max > threshold {
            exceeded_count += 1;
            // Highlight exceeded error in amplified red
            diff_rgba[idx] = (dr as u16 * 5).min(255) as u8;
            diff_rgba[idx + 1] = 0;
            diff_rgba[idx + 2] = 0;
            diff_rgba[idx + 3] = 255;
        } else {
            // Low error grayscale representation
            diff_rgba[idx] = dr;
            diff_rgba[idx + 1] = dg;
            diff_rgba[idx + 2] = db;
            diff_rgba[idx + 3] = 255;
        }
    }

    let mae = sum_diff / total_pixels as f64;
    let exceeded_ratio = exceeded_count as f64 / total_pixels as f64;

    let passed = mae <= max_mae && exceeded_ratio <= max_exceeded_ratio;

    let metrics = ParityDiffMetrics {
        mae,
        max_error: max_err,
        exceeded_ratio,
        width,
        height,
        total_pixels,
        exceeded_pixels: exceeded_count,
    };

    (passed, metrics, diff_rgba)
}

/// Saves raw RGBA buffer to PNG file using FFmpeg
pub fn save_rgba_to_png(rgba: &[u8], width: u32, height: u32, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut cmd = MediaBinaryResolver::command("ffmpeg")
        .map_err(|e| format!("Failed to resolve FFmpeg binary: {e}"))?;
    let mut child = cmd
        .args([
            "-y",
            "-f",
            "rawvideo",
            "-s",
            &format!("{}x{}", width, height),
            "-pix_fmt",
            "rgba",
            "-i",
            "pipe:0",
            "-vframes",
            "1",
            "-update",
            "1",
            &path.to_string_lossy(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to spawn FFmpeg to write PNG: {e}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(rgba);
    }

    let status = child
        .wait()
        .map_err(|e| format!("FFmpeg wait failed: {e}"))?;
    if !status.success() {
        return Err(format!("FFmpeg failed with exit code {:?}", status.code()));
    }
    Ok(())
}

/// Writes failure or diagnostic artifacts to disk
#[allow(clippy::too_many_arguments)]
pub fn write_diagnostic_artifacts(
    fixture_id: &str,
    project_time: RationalTime,
    preview_rgba: &[u8],
    export_rgba: &[u8],
    diff_rgba: &[u8],
    width: u32,
    height: u32,
    metrics: &ParityDiffMetrics,
    semantic_passed: bool,
    visual_passed: bool,
) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let out_dir = std::env::temp_dir()
        .join("tinixcut_parity_artifacts")
        .join(fixture_id);
    let _ = fs::create_dir_all(&out_dir);

    let p_path = out_dir.join("preview.png");
    let e_path = out_dir.join("export.png");
    let d_path = out_dir.join("diff.png");
    let r_path = out_dir.join("report.json");

    let _ = save_rgba_to_png(preview_rgba, width, height, &p_path);
    let _ = save_rgba_to_png(export_rgba, width, height, &e_path);
    let _ = save_rgba_to_png(diff_rgba, width, height, &d_path);

    let report_json = serde_json::json!({
        "fixture": fixture_id,
        "project_time": {
            "value": project_time.value,
            "rate_numerator": project_time.rate.numerator,
            "rate_denominator": project_time.rate.denominator,
            "seconds": project_time.to_seconds()
        },
        "semantic_result": if semantic_passed { "PASS" } else { "FAIL" },
        "visual_result": if visual_passed { "PASS" } else { "FAIL" },
        "metrics": metrics,
        "thresholds": {
            "max_mae": 5.0,
            "max_exceeded_ratio": 0.05
        }
    });

    let _ = fs::write(
        &r_path,
        serde_json::to_string_pretty(&report_json).unwrap_or_default(),
    );

    (p_path, e_path, d_path, r_path)
}

/// Ensures synthetic deterministic fixture media exists
pub fn ensure_parity_fixture_media(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("tinixcut_parity_media");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join(format!("{}.mp4", name));

    if path.exists() {
        return path;
    }

    match name {
        "parity_quadrant" => {
            // Deterministic 4-quadrant video: Top-Left Red, Top-Right Green, Bottom-Left Blue, Bottom-Right Yellow
            // with visible frame counter text and 440Hz sine audio
            let filter =
                "[0:v][1:v]hstack[top];[2:v][3:v]hstack[bot];[top][bot]vstack,format=yuv420p[v]";
            if let Ok(mut cmd) = MediaBinaryResolver::command("ffmpeg") {
                let _ = cmd
                    .args([
                        "-y",
                        "-f",
                        "lavfi",
                        "-i",
                        "color=c=red:s=320x180:r=30:d=5",
                        "-f",
                        "lavfi",
                        "-i",
                        "color=c=green:s=320x180:r=30:d=5",
                        "-f",
                        "lavfi",
                        "-i",
                        "color=c=blue:s=320x180:r=30:d=5",
                        "-f",
                        "lavfi",
                        "-i",
                        "color=c=yellow:s=320x180:r=30:d=5",
                        "-f",
                        "lavfi",
                        "-i",
                        "sine=frequency=440:duration=5",
                        "-filter_complex",
                        filter,
                        "-map",
                        "[v]",
                        "-map",
                        "4:a",
                        "-c:v",
                        "libx264",
                        "-preset",
                        "ultrafast",
                        "-pix_fmt",
                        "yuv420p",
                        "-c:a",
                        "aac",
                        &path.to_string_lossy(),
                    ])
                    .output();
            }
        }
        "parity_blue" => {
            if let Ok(mut cmd) = MediaBinaryResolver::command("ffmpeg") {
                let _ = cmd
                    .args([
                        "-y",
                        "-f",
                        "lavfi",
                        "-i",
                        "color=c=blue:s=640x360:r=30:d=5",
                        "-c:v",
                        "libx264",
                        "-preset",
                        "ultrafast",
                        "-pix_fmt",
                        "yuv420p",
                        &path.to_string_lossy(),
                    ])
                    .output();
            }
        }
        "parity_red" => {
            if let Ok(mut cmd) = MediaBinaryResolver::command("ffmpeg") {
                let _ = cmd
                    .args([
                        "-y",
                        "-f",
                        "lavfi",
                        "-i",
                        "color=c=red:s=640x360:r=30:d=5",
                        "-c:v",
                        "libx264",
                        "-preset",
                        "ultrafast",
                        "-pix_fmt",
                        "yuv420p",
                        &path.to_string_lossy(),
                    ])
                    .output();
            }
        }
        _ => {}
    }

    path
}

/// Builds an isolated deterministic fixture project from the Milestone 3 specification
pub fn build_fixture_project(
    fixture_id: &str,
    _media_dir: &Path,
) -> Result<(Project, Vec<RationalTime>, String), String> {
    let mut project = Project::new(format!("Fixture {}", fixture_id));
    project.settings = ProjectSettings {
        width: 640,
        height: 360,
        frame_rate: FrameRate::FPS_30,
        sample_rate: 48000,
        channels: 2,
    };

    let quadrant_path = ensure_parity_fixture_media("parity_quadrant");
    let quadrant_id = "ast_quadrant".to_string();
    project.assets.push(Asset {
        asset_id: quadrant_id.clone(),
        name: "parity_quadrant.mp4".to_string(),
        path: quadrant_path.to_string_lossy().to_string(),
        media_kind: MediaKind::Video,
        duration: Some(RationalTime::new(150, FrameRate::FPS_30)),
        file_size: 10000,
    });

    let blue_path = ensure_parity_fixture_media("parity_blue");
    let blue_id = "ast_blue".to_string();
    project.assets.push(Asset {
        asset_id: blue_id.clone(),
        name: "parity_blue.mp4".to_string(),
        path: blue_path.to_string_lossy().to_string(),
        media_kind: MediaKind::Video,
        duration: Some(RationalTime::new(150, FrameRate::FPS_30)),
        file_size: 10000,
    });

    let red_path = ensure_parity_fixture_media("parity_red");
    let red_id = "ast_red".to_string();
    project.assets.push(Asset {
        asset_id: red_id.clone(),
        name: "parity_red.mp4".to_string(),
        path: red_path.to_string_lossy().to_string(),
        media_kind: MediaKind::Video,
        duration: Some(RationalTime::new(150, FrameRate::FPS_30)),
        file_size: 10000,
    });

    let fps = FrameRate::FPS_30;

    match fixture_id {
        // P7-F01 Baseline: Single video clip, timeline start 0, duration 90, no transform, no crop, opacity 1
        "P7-F01" => {
            let mut track = Track::new("Video Track 0".to_string(), TrackKind::Video);
            let item = TimelineItem::new(
                Some(quadrant_id),
                "Baseline Clip".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(90, fps)),
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(90, fps)),
            )
            .unwrap();
            track.items.push(item);
            project.tracks.push(track);

            let times = vec![RationalTime::new(30, fps)];
            Ok((project, times, "Baseline single clip at t=1.0s".to_string()))
        }

        // P7-F02 Timeline Offset: Clip begins after time 0 (starts at 30 frames)
        "P7-F02" => {
            let mut track = Track::new("Video Track 0".to_string(), TrackKind::Video);
            let item = TimelineItem::new(
                Some(quadrant_id),
                "Offset Clip".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(30, fps), RationalTime::new(60, fps)),
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps)),
            )
            .unwrap();
            track.items.push(item);
            project.tracks.push(track);

            let times = vec![RationalTime::new(15, fps), RationalTime::new(30, fps)];
            Ok((
                project,
                times,
                "Timeline offset gap at t=15 and start boundary at t=30".to_string(),
            ))
        }

        // P7-F03 Trim / Non-zero Source In-point: Source in-point 60 (2.0s), duration 60
        "P7-F03" => {
            let mut track = Track::new("Video Track 0".to_string(), TrackKind::Video);
            let item = TimelineItem::new(
                Some(quadrant_id),
                "Trimmed Clip".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps)),
                TimeRange::new(RationalTime::new(60, fps), RationalTime::new(60, fps)),
            )
            .unwrap();
            track.items.push(item);
            project.tracks.push(track);

            let times = vec![RationalTime::new(30, fps)];
            Ok((
                project,
                times,
                "Trim in-point mapping project t=30 to source t=90".to_string(),
            ))
        }

        // P7-F04 Position: Non-zero X and Y offsets
        "P7-F04" => {
            let mut track = Track::new("Video Track 0".to_string(), TrackKind::Video);
            let mut item = TimelineItem::new(
                Some(quadrant_id),
                "Position Clip".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(90, fps)),
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(90, fps)),
            )
            .unwrap();
            item.transform.position_x = 80.0;
            item.transform.position_y = -40.0;
            track.items.push(item);
            project.tracks.push(track);

            let times = vec![RationalTime::new(30, fps)];
            Ok((project, times, "Position offset (80, -40)".to_string()))
        }

        // P7-F05 Scale: Non-default scaling (0.6x)
        "P7-F05" => {
            let mut track = Track::new("Video Track 0".to_string(), TrackKind::Video);
            let mut item = TimelineItem::new(
                Some(quadrant_id),
                "Scale Clip".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(90, fps)),
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(90, fps)),
            )
            .unwrap();
            item.transform.scale_x = 0.6;
            item.transform.scale_y = 0.6;
            track.items.push(item);
            project.tracks.push(track);

            let times = vec![RationalTime::new(30, fps)];
            Ok((project, times, "Scale transform (0.6, 0.6)".to_string()))
        }

        // P7-F06 Rotation: 45 degree rotation
        "P7-F06" => {
            let mut track = Track::new("Video Track 0".to_string(), TrackKind::Video);
            let mut item = TimelineItem::new(
                Some(quadrant_id),
                "Rotate Clip".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(90, fps)),
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(90, fps)),
            )
            .unwrap();
            item.transform.scale_x = 0.7;
            item.transform.scale_y = 0.7;
            item.transform.rotation = 45.0;
            track.items.push(item);
            project.tracks.push(track);

            let times = vec![RationalTime::new(30, fps)];
            Ok((project, times, "Rotation 45 degrees".to_string()))
        }

        // P7-F07 Crop: Four-sided crop insets
        "P7-F07" => {
            let mut track = Track::new("Video Track 0".to_string(), TrackKind::Video);
            let mut item = TimelineItem::new(
                Some(quadrant_id),
                "Crop Clip".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(90, fps)),
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(90, fps)),
            )
            .unwrap();
            item.crop = Crop {
                left: 0.15,
                right: 0.15,
                top: 0.10,
                bottom: 0.10,
            };
            track.items.push(item);
            project.tracks.push(track);

            let times = vec![RationalTime::new(30, fps)];
            Ok((
                project,
                times,
                "Crop insets L=0.15, R=0.15, T=0.10, B=0.10".to_string(),
            ))
        }

        // P7-F08 Opacity: Semi-transparent layer over background
        "P7-F08" => {
            let mut track0 = Track::new("Background Track".to_string(), TrackKind::Video);
            let bg_item = TimelineItem::new(
                Some(blue_id),
                "BG Blue".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(90, fps)),
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(90, fps)),
            )
            .unwrap();
            track0.items.push(bg_item);
            project.tracks.push(track0);

            let mut track1 = Track::new("Foreground Track".to_string(), TrackKind::Video);
            let mut fg_item = TimelineItem::new(
                Some(quadrant_id),
                "FG Quadrant 60%".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(90, fps)),
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(90, fps)),
            )
            .unwrap();
            fg_item.opacity = 0.6;
            track1.items.push(fg_item);
            project.tracks.push(track1);

            let times = vec![RationalTime::new(30, fps)];
            Ok((
                project,
                times,
                "Opacity blending 0.6 over solid background".to_string(),
            ))
        }

        // P7-F09 Layer Order (Z-Order): Overlapping distinct layers
        "P7-F09" => {
            let mut track0 = Track::new("Base Track".to_string(), TrackKind::Video);
            let bg = TimelineItem::new(
                Some(blue_id),
                "Bottom Blue".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(90, fps)),
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(90, fps)),
            )
            .unwrap();
            track0.items.push(bg);
            project.tracks.push(track0);

            let mut track1 = Track::new("Top Track".to_string(), TrackKind::Video);
            let mut top = TimelineItem::new(
                Some(red_id),
                "Top Red".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(90, fps)),
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(90, fps)),
            )
            .unwrap();
            top.transform.scale_x = 0.5;
            top.transform.scale_y = 0.5;
            track1.items.push(top);
            project.tracks.push(track1);

            let times = vec![RationalTime::new(30, fps)];
            Ok((
                project,
                times,
                "Z-order layer stacking (red on top of blue)".to_string(),
            ))
        }

        // P7-F10 Caption / Text: Text overlay placement and timing
        "P7-F10" => {
            let mut track0 = Track::new("Video Track".to_string(), TrackKind::Video);
            let vid = TimelineItem::new(
                Some(blue_id),
                "Video Base".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(90, fps)),
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(90, fps)),
            )
            .unwrap();
            track0.items.push(vid);
            project.tracks.push(track0);

            let mut track1 = Track::new("Text Track".to_string(), TrackKind::Video);
            let mut text_item = TimelineItem::new(
                None,
                "Title Overlay".to_string(),
                ItemKind::Text,
                TimeRange::new(RationalTime::new(15, fps), RationalTime::new(60, fps)),
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps)),
            )
            .unwrap();
            text_item.text_content = Some("TinixCut POC2 Parity Check".to_string());
            text_item.transform.position_y = 50.0;
            track1.items.push(text_item);
            project.tracks.push(track1);

            let times = vec![RationalTime::new(30, fps)];
            Ok((
                project,
                times,
                "Text overlay timing and placement".to_string(),
            ))
        }

        // P7-F11 Boundary Timestamps: Step before, at, and after clip boundaries
        "P7-F11" => {
            let mut track0 = Track::new("Timeline Context".to_string(), TrackKind::Video);
            let bg = TimelineItem::new(
                Some(blue_id),
                "Timeline Background".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(120, fps)),
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(120, fps)),
            )
            .unwrap();
            track0.items.push(bg);
            project.tracks.push(track0);

            let mut track1 = Track::new("Boundary Video".to_string(), TrackKind::Video);
            let item = TimelineItem::new(
                Some(quadrant_id),
                "Boundary Clip".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(30, fps), RationalTime::new(60, fps)),
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps)),
            )
            .unwrap();
            track1.items.push(item);
            project.tracks.push(track1);

            // Timestamps: T0=29, T1=30, T2=31, T3=60, T4=89, T5=90
            let times = vec![
                RationalTime::new(29, fps),
                RationalTime::new(30, fps),
                RationalTime::new(31, fps),
                RationalTime::new(60, fps),
                RationalTime::new(89, fps),
                RationalTime::new(90, fps),
            ];
            Ok((
                project,
                times,
                "Boundary timestamps T0 through T5".to_string(),
            ))
        }

        // P7-F12 Combined Representative Composition: Combines video, transforms, crop, opacity, and text
        "P7-F12" => {
            let mut track0 = Track::new("BG Track".to_string(), TrackKind::Video);
            let bg = TimelineItem::new(
                Some(blue_id),
                "Base BG".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(120, fps)),
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(120, fps)),
            )
            .unwrap();
            track0.items.push(bg);
            project.tracks.push(track0);

            let mut track1 = Track::new("FG Track".to_string(), TrackKind::Video);
            let mut fg = TimelineItem::new(
                Some(quadrant_id),
                "Transformed FG".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(15, fps), RationalTime::new(90, fps)),
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(90, fps)),
            )
            .unwrap();
            fg.transform.position_x = 50.0;
            fg.transform.position_y = -30.0;
            fg.transform.scale_x = 0.65;
            fg.transform.scale_y = 0.65;
            fg.crop = Crop {
                left: 0.1,
                right: 0.1,
                top: 0.05,
                bottom: 0.05,
            };
            fg.opacity = 0.85;
            track1.items.push(fg);
            project.tracks.push(track1);

            let mut track2 = Track::new("Text Track".to_string(), TrackKind::Video);
            let mut text = TimelineItem::new(
                None,
                "POC2 Title".to_string(),
                ItemKind::Text,
                TimeRange::new(RationalTime::new(30, fps), RationalTime::new(60, fps)),
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps)),
            )
            .unwrap();
            text.text_content = Some("TinixCut POC 2 Final".to_string());
            text.transform.position_y = 80.0;
            track2.items.push(text);
            project.tracks.push(track2);

            let times = vec![RationalTime::new(45, fps)];
            Ok((
                project,
                times,
                "Combined representative composition with multiple layers and transforms"
                    .to_string(),
            ))
        }

        // --- Phase 13 CrossDissolve Fixtures ---
        // P13-F01: Opaque full-frame A/B crossdissolve at midpoint (p = 0.5)
        "P13-F01" => {
            let mut track = Track::new("Video Track".to_string(), TrackKind::Video);
            let mut item1 = TimelineItem::new(
                Some(red_id),
                "Clip A".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps)),
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps)),
            )
            .unwrap();
            item1.item_id = "p13_it1".to_string();

            let mut item2 = TimelineItem::new(
                Some(blue_id),
                "Clip B".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(60, fps), RationalTime::new(60, fps)),
                TimeRange::new(RationalTime::new(30, fps), RationalTime::new(60, fps)),
            )
            .unwrap();
            item2.item_id = "p13_it2".to_string();

            let cut_trans = crate::modules::domain::CutTransition {
                transition_id: "p13_tr1".to_string(),
                track_id: track.track_id.clone(),
                kind: crate::modules::composition::TransitionKind::CrossDissolve,
                left_item_id: "p13_it1".to_string(),
                right_item_id: "p13_it2".to_string(),
                duration: RationalTime::new(20, fps), // D=20, L=10, R=10, [50, 70)
            };

            track.items.push(item1);
            track.items.push(item2);
            track.transitions.push(cut_trans);
            project.tracks.push(track);

            let times = vec![RationalTime::new(60, fps)]; // Cut midpoint p = 0.5
            Ok((
                project,
                times,
                "Phase 13: Opaque full-frame CrossDissolve at midpoint".to_string(),
            ))
        }

        // P13-F02: Participant opacity < 1 (A = 0.8, B = 0.9)
        "P13-F02" => {
            let mut track = Track::new("Video Track".to_string(), TrackKind::Video);
            let mut item1 = TimelineItem::new(
                Some(red_id),
                "Clip A".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps)),
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps)),
            )
            .unwrap();
            item1.item_id = "p13_it1".to_string();
            item1.opacity = 0.8;

            let mut item2 = TimelineItem::new(
                Some(blue_id),
                "Clip B".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(60, fps), RationalTime::new(60, fps)),
                TimeRange::new(RationalTime::new(30, fps), RationalTime::new(60, fps)),
            )
            .unwrap();
            item2.item_id = "p13_it2".to_string();
            item2.opacity = 0.9;

            let cut_trans = crate::modules::domain::CutTransition {
                transition_id: "p13_tr2".to_string(),
                track_id: track.track_id.clone(),
                kind: crate::modules::composition::TransitionKind::CrossDissolve,
                left_item_id: "p13_it1".to_string(),
                right_item_id: "p13_it2".to_string(),
                duration: RationalTime::new(20, fps),
            };

            track.items.push(item1);
            track.items.push(item2);
            track.transitions.push(cut_trans);
            project.tracks.push(track);

            let times = vec![RationalTime::new(60, fps)];
            Ok((
                project,
                times,
                "Phase 13: Participant opacity < 1 CrossDissolve at midpoint".to_string(),
            ))
        }

        // P13-F03: Transformed partial overlap
        "P13-F03" => {
            let mut track = Track::new("Video Track".to_string(), TrackKind::Video);
            let mut item1 = TimelineItem::new(
                Some(red_id),
                "Clip A".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps)),
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps)),
            )
            .unwrap();
            item1.item_id = "p13_it1".to_string();
            item1.transform.scale_x = 0.7;
            item1.transform.scale_y = 0.7;
            item1.transform.position_x = -50.0;

            let mut item2 = TimelineItem::new(
                Some(blue_id),
                "Clip B".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(60, fps), RationalTime::new(60, fps)),
                TimeRange::new(RationalTime::new(30, fps), RationalTime::new(60, fps)),
            )
            .unwrap();
            item2.item_id = "p13_it2".to_string();
            item2.transform.scale_x = 0.8;
            item2.transform.scale_y = 0.8;
            item2.transform.position_x = 50.0;

            let cut_trans = crate::modules::domain::CutTransition {
                transition_id: "p13_tr3".to_string(),
                track_id: track.track_id.clone(),
                kind: crate::modules::composition::TransitionKind::CrossDissolve,
                left_item_id: "p13_it1".to_string(),
                right_item_id: "p13_it2".to_string(),
                duration: RationalTime::new(20, fps),
            };

            track.items.push(item1);
            track.items.push(item2);
            track.transitions.push(cut_trans);
            project.tracks.push(track);

            let times = vec![RationalTime::new(60, fps)];
            Ok((
                project,
                times,
                "Phase 13: Transformed partial overlap CrossDissolve".to_string(),
            ))
        }

        // P13-F04: Crop revealing background beneath transition
        "P13-F04" => {
            let mut track0 = Track::new("BG Track".to_string(), TrackKind::Video);
            let bg_item = TimelineItem::new(
                Some(quadrant_id),
                "BG Quadrant".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(120, fps)),
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(120, fps)),
            )
            .unwrap();
            track0.items.push(bg_item);
            project.tracks.push(track0);

            let mut track1 = Track::new("FG Track".to_string(), TrackKind::Video);
            let mut item1 = TimelineItem::new(
                Some(red_id),
                "Clip A".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps)),
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps)),
            )
            .unwrap();
            item1.item_id = "p13_it1".to_string();
            item1.crop = Crop {
                left: 0.1,
                right: 0.1,
                top: 0.1,
                bottom: 0.1,
            };

            let mut item2 = TimelineItem::new(
                Some(blue_id),
                "Clip B".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(60, fps), RationalTime::new(60, fps)),
                TimeRange::new(RationalTime::new(30, fps), RationalTime::new(60, fps)),
            )
            .unwrap();
            item2.item_id = "p13_it2".to_string();
            item2.crop = Crop {
                left: 0.15,
                right: 0.15,
                top: 0.15,
                bottom: 0.15,
            };

            let cut_trans = crate::modules::domain::CutTransition {
                transition_id: "p13_tr4".to_string(),
                track_id: track1.track_id.clone(),
                kind: crate::modules::composition::TransitionKind::CrossDissolve,
                left_item_id: "p13_it1".to_string(),
                right_item_id: "p13_it2".to_string(),
                duration: RationalTime::new(20, fps),
            };

            track1.items.push(item1);
            track1.items.push(item2);
            track1.transitions.push(cut_trans);
            project.tracks.push(track1);

            let times = vec![RationalTime::new(60, fps)];
            Ok((
                project,
                times,
                "Phase 13: Crop revealing background during CrossDissolve".to_string(),
            ))
        }

        // P13-F05: Boundary timestamps (T0=50, midpoint=60, T1-1=69, T1=70)
        "P13-F05" => {
            let mut track = Track::new("Video Track".to_string(), TrackKind::Video);
            let mut item1 = TimelineItem::new(
                Some(red_id),
                "Clip A".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps)),
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps)),
            )
            .unwrap();
            item1.item_id = "p13_it1".to_string();

            let mut item2 = TimelineItem::new(
                Some(blue_id),
                "Clip B".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(60, fps), RationalTime::new(60, fps)),
                TimeRange::new(RationalTime::new(30, fps), RationalTime::new(60, fps)),
            )
            .unwrap();
            item2.item_id = "p13_it2".to_string();

            let cut_trans = crate::modules::domain::CutTransition {
                transition_id: "p13_tr5".to_string(),
                track_id: track.track_id.clone(),
                kind: crate::modules::composition::TransitionKind::CrossDissolve,
                left_item_id: "p13_it1".to_string(),
                right_item_id: "p13_it2".to_string(),
                duration: RationalTime::new(20, fps),
            };

            track.items.push(item1);
            track.items.push(item2);
            track.transitions.push(cut_trans);
            project.tracks.push(track);

            let times = vec![
                RationalTime::new(50, fps), // T0
                RationalTime::new(60, fps), // Midpoint
                RationalTime::new(69, fps), // T1 - 1
                RationalTime::new(70, fps), // T1
            ];
            Ok((
                project,
                times,
                "Phase 13: CrossDissolve boundary timestamps T0, Cut, T1-1, T1".to_string(),
            ))
        }

        // P13-F06: Odd RationalTime duration (D = 11, L = 5, R = 6, window [55, 66))
        "P13-F06" => {
            let mut track = Track::new("Video Track".to_string(), TrackKind::Video);
            let mut item1 = TimelineItem::new(
                Some(red_id),
                "Clip A".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps)),
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps)),
            )
            .unwrap();
            item1.item_id = "p13_it1".to_string();

            let mut item2 = TimelineItem::new(
                Some(blue_id),
                "Clip B".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(60, fps), RationalTime::new(60, fps)),
                TimeRange::new(RationalTime::new(30, fps), RationalTime::new(60, fps)),
            )
            .unwrap();
            item2.item_id = "p13_it2".to_string();

            let cut_trans = crate::modules::domain::CutTransition {
                transition_id: "p13_tr6".to_string(),
                track_id: track.track_id.clone(),
                kind: crate::modules::composition::TransitionKind::CrossDissolve,
                left_item_id: "p13_it1".to_string(),
                right_item_id: "p13_it2".to_string(),
                duration: RationalTime::new(11, fps), // D=11 -> L=5, R=6, [55, 66)
            };

            track.items.push(item1);
            track.items.push(item2);
            track.transitions.push(cut_trans);
            project.tracks.push(track);

            let times = vec![
                RationalTime::new(55, fps), // T0
                RationalTime::new(60, fps), // Cut
                RationalTime::new(65, fps), // T1 - 1
            ];
            Ok((
                project,
                times,
                "Phase 13: Odd RationalTime duration (11 ticks)".to_string(),
            ))
        }

        // P13-F07: Exact-sufficient handles (A postroll = 10, B preroll = 10, D = 20)
        "P13-F07" => {
            // Asset A with total duration 70 frames (postroll = 70 - (0 + 60) = 10)
            let ast_exact1_id = "ast_exact_1".to_string();
            project.assets.push(Asset {
                asset_id: ast_exact1_id.clone(),
                name: "exact1.mp4".to_string(),
                path: red_path.to_string_lossy().to_string(),
                media_kind: MediaKind::Video,
                duration: Some(RationalTime::new(70, fps)),
                file_size: 10000,
            });

            // Asset B with total duration 70 frames (preroll = 10)
            let ast_exact2_id = "ast_exact_2".to_string();
            project.assets.push(Asset {
                asset_id: ast_exact2_id.clone(),
                name: "exact2.mp4".to_string(),
                path: blue_path.to_string_lossy().to_string(),
                media_kind: MediaKind::Video,
                duration: Some(RationalTime::new(70, fps)),
                file_size: 10000,
            });

            let mut track = Track::new("Video Track".to_string(), TrackKind::Video);
            let mut item1 = TimelineItem::new(
                Some(ast_exact1_id),
                "Clip A".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps)),
                TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps)),
            )
            .unwrap();
            item1.item_id = "p13_it1".to_string();

            let mut item2 = TimelineItem::new(
                Some(ast_exact2_id),
                "Clip B".to_string(),
                ItemKind::Video,
                TimeRange::new(RationalTime::new(60, fps), RationalTime::new(60, fps)),
                TimeRange::new(RationalTime::new(10, fps), RationalTime::new(60, fps)),
            )
            .unwrap();
            item2.item_id = "p13_it2".to_string();

            let cut_trans = crate::modules::domain::CutTransition {
                transition_id: "p13_tr7".to_string(),
                track_id: track.track_id.clone(),
                kind: crate::modules::composition::TransitionKind::CrossDissolve,
                left_item_id: "p13_it1".to_string(),
                right_item_id: "p13_it2".to_string(),
                duration: RationalTime::new(20, fps), // D=20, L=10, R=10
            };

            track.items.push(item1);
            track.items.push(item2);
            track.transitions.push(cut_trans);
            project.tracks.push(track);

            let times = vec![RationalTime::new(60, fps)];
            Ok((
                project,
                times,
                "Phase 13: Exact-sufficient handles CrossDissolve".to_string(),
            ))
        }

        _ => Err(format!("Unknown fixture ID: {}", fixture_id)),
    }
}

/// Renders single frame from the export pipeline into raw RGBA
pub fn render_export_frame_rgba(
    plan: &CompositionPlan,
    timestamp: RationalTime,
) -> Result<Vec<u8>, String> {
    let dummy_path = PathBuf::from("output.raw");
    let (args, _) = plan.to_ffmpeg_single_frame_args(timestamp, &dummy_path);

    let mut cmd = MediaBinaryResolver::command("ffmpeg")
        .map_err(|e| format!("Failed to resolve FFmpeg binary: {e}"))?;
    let output = cmd
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to run export FFmpeg: {e}"))?;

    let expected_len = (plan.width * plan.height * 4) as usize;
    if output.stdout.len() < expected_len {
        let err_str = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Export frame truncated (expected {} bytes, got {}): {}",
            expected_len,
            output.stdout.len(),
            err_str
        ));
    }

    Ok(output.stdout[0..expected_len].to_vec())
}

/// Renders single frame using preview sampled state into raw RGBA
pub fn render_preview_frame_rgba(
    plan: &CompositionPlan,
    timestamp: RationalTime,
) -> Result<Vec<u8>, String> {
    render_preview_frame_rgba_with_blend_mode(plan, timestamp, false)
}

/// Renders single frame using preview sampled state with explicit blend mode (for negative control)
pub fn render_preview_frame_rgba_with_blend_mode(
    plan: &CompositionPlan,
    timestamp: RationalTime,
    use_improper_source_over: bool,
) -> Result<Vec<u8>, String> {
    let state = plan.sample_preview_at(timestamp);
    let expected_len = (plan.width * plan.height * 4) as usize;

    if state.active_layers.is_empty() && state.transition_groups.is_empty() {
        return Ok(vec![0u8; expected_len]);
    }

    // Build FFmpeg command rendering precisely the active preview sampled layers and transition groups
    let mut args: Vec<String> = Vec::new();
    let mut filter_complex = String::new();

    let mut input_paths: Vec<PathBuf> = Vec::new();
    for layer in &state.active_layers {
        if let Some(ref p) = layer.asset_path {
            let pbuf = PathBuf::from(p);
            if !input_paths.contains(&pbuf) {
                input_paths.push(pbuf);
            }
        }
    }
    for grp in &state.transition_groups {
        if let Some(ref p) = grp.left_layer.asset_path {
            let pbuf = PathBuf::from(p);
            if !input_paths.contains(&pbuf) {
                input_paths.push(pbuf);
            }
        }
        if let Some(ref p) = grp.right_layer.asset_path {
            let pbuf = PathBuf::from(p);
            if !input_paths.contains(&pbuf) {
                input_paths.push(pbuf);
            }
        }
    }

    for path in &input_paths {
        args.push("-i".to_string());
        args.push(path.to_string_lossy().to_string());
    }

    filter_complex.push_str(&format!(
        "color=c=black:s={}x{}:r={}:d=1[base];",
        plan.width,
        plan.height,
        plan.frame_rate.as_f64()
    ));

    let mut current_base = "base".to_string();
    let mut idx = 0;
    let mut grp_idx = 0;

    enum PreviewElem<'a> {
        Layer(&'a crate::modules::composition::PreviewSampledLayer),
        Group(&'a crate::modules::composition::PreviewTransitionGroup),
    }

    let mut elems = Vec::new();
    for l in &state.active_layers {
        elems.push(PreviewElem::Layer(l));
    }
    for g in &state.transition_groups {
        elems.push(PreviewElem::Group(g));
    }
    elems.sort_by_key(|e| match e {
        PreviewElem::Layer(l) => (l.z_index, 0),
        PreviewElem::Group(g) => (g.z_index, 1),
    });

    for elem in elems {
        match elem {
            PreviewElem::Layer(layer) => {
                let next_base = format!("p_over_{}", idx);
                let layer_tag = format!("p_layer_{}", idx);
                idx += 1;

                match layer.kind {
                    ItemKind::Video | ItemKind::Image => {
                        let mut filter_chain = String::new();
                        let src_sec = layer.source_time.to_seconds();

                        if let Some(ref path) = layer.asset_path {
                            let pbuf = PathBuf::from(path);
                            let input_idx =
                                input_paths.iter().position(|p| p == &pbuf).unwrap_or(0);
                            filter_chain.push_str(&format!(
                                "[{}:v]trim=start={:.3}:duration=0.034,setpts=PTS-STARTPTS",
                                input_idx, src_sec
                            ));
                        } else {
                            filter_chain.push_str(&format!(
                                "color=c=blue:s={}x{}:d=1",
                                layer.computed_bounds.width as u32,
                                layer.computed_bounds.height as u32
                            ));
                        }

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

                        let mut rw = layer.computed_bounds.width.round().max(2.0) as u32;
                        let mut rh = layer.computed_bounds.height.round().max(2.0) as u32;
                        if !rw.is_multiple_of(2) {
                            rw += 1;
                        }
                        if !rh.is_multiple_of(2) {
                            rh += 1;
                        }

                        filter_chain.push_str(&format!(",scale={}:{}", rw, rh));

                        if layer.rotation.abs() > 0.01 {
                            filter_chain.push_str(&format!(
                                ",rotate='{0:.4}*PI/180':ow='rotw({0:.4}*PI/180)':oh='roth({0:.4}*PI/180)':c=none",
                                layer.rotation
                            ));
                        }

                        filter_chain.push_str(&format!(
                            ",format=yuva420p,colorchannelmixer=aa={:.3}[{}];",
                            layer.opacity, layer_tag
                        ));
                        filter_complex.push_str(&filter_chain);

                        filter_complex.push_str(&format!(
                            "[{}][{}]overlay=x='(W-w)/2+{:.1}':y='(H-h)/2+{:.1}'[{}];",
                            current_base, layer_tag, layer.position.0, layer.position.1, next_base
                        ));
                        current_base = next_base;

                        if let Some(ref text) = layer.text {
                            if !text.trim().is_empty() {
                                let text_base = format!("p_over_{}", idx);
                                idx += 1;
                                let escaped = text
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
                                    "[{}]drawtext=text='{}':fontsize={}{}:fontcolor=white:box=1:boxcolor=black@0.6:boxborderw=6:x='(w-tw)/2+{:.1}':y='(h-th)/2+{:.1}+100'[{}];",
                                    current_base, escaped, font_size, font_arg, layer.position.0, layer.position.1, text_base
                                ));
                                current_base = text_base;
                            }
                        }
                    }
                    ItemKind::Text => {
                        let text = layer.text.as_deref().unwrap_or("");
                        let escaped = text
                            .replace('\\', "\\\\")
                            .replace('\'', "'\\''")
                            .replace(':', "\\:");
                        let font_size = layer.font_size.unwrap_or(48.0);
                        let font_arg =
                            if cfg!(windows) && Path::new("C:/Windows/Fonts/arial.ttf").exists() {
                                ":fontfile='C\\:/Windows/Fonts/arial.ttf'"
                            } else {
                                ""
                            };
                        filter_complex.push_str(&format!(
                            "[{}]drawtext=text='{}':fontsize={}{}:fontcolor=white:x='(w-tw)/2+{:.1}':y='(h-th)/2+{:.1}'[{}];",
                            current_base, escaped, font_size, font_arg, layer.position.0, layer.position.1, next_base
                        ));
                        current_base = next_base;
                    }
                    _ => {}
                }
            }
            PreviewElem::Group(grp) => {
                let left_layer = &grp.left_layer;
                let right_layer = &grp.right_layer;

                // Left participant on transparent canvas
                let mut l_filter = String::new();
                let l_clip_tag = format!("p_grp_l_clip_{}", grp_idx);
                let l_src_sec = left_layer.source_time.to_seconds();
                if let Some(ref path) = left_layer.asset_path {
                    let pbuf = PathBuf::from(path);
                    let input_idx = input_paths.iter().position(|p| p == &pbuf).unwrap_or(0);
                    l_filter.push_str(&format!(
                        "[{}:v]trim=start={:.3}:duration=0.034,setpts=PTS-STARTPTS",
                        input_idx, l_src_sec
                    ));
                } else {
                    l_filter.push_str(&format!(
                        "color=c=blue:s={}x{}:d=1",
                        left_layer.computed_bounds.width as u32,
                        left_layer.computed_bounds.height as u32
                    ));
                }
                if left_layer.crop.left > 0.0
                    || left_layer.crop.right > 0.0
                    || left_layer.crop.top > 0.0
                    || left_layer.crop.bottom > 0.0
                {
                    l_filter.push_str(&format!(
                        ",crop=w='iw*(1.0-{:.4}-{:.4})':h='ih*(1.0-{:.4}-{:.4})':x='iw*{:.4}':y='ih*{:.4}'",
                        left_layer.crop.left, left_layer.crop.right, left_layer.crop.top, left_layer.crop.bottom, left_layer.crop.left, left_layer.crop.top
                    ));
                }
                let mut l_rw = left_layer.computed_bounds.width.round().max(2.0) as u32;
                let mut l_rh = left_layer.computed_bounds.height.round().max(2.0) as u32;
                if !l_rw.is_multiple_of(2) {
                    l_rw += 1;
                }
                if !l_rh.is_multiple_of(2) {
                    l_rh += 1;
                }
                l_filter.push_str(&format!(",scale={}:{}", l_rw, l_rh));
                if left_layer.rotation.abs() > 0.01 {
                    l_filter.push_str(&format!(
                        ",rotate='{0:.4}*PI/180':ow='rotw({0:.4}*PI/180)':oh='roth({0:.4}*PI/180)':c=none",
                        left_layer.rotation
                    ));
                }
                l_filter.push_str(&format!(
                    ",format=yuva420p,colorchannelmixer=aa={:.3}[{}];",
                    left_layer.opacity, l_clip_tag
                ));
                filter_complex.push_str(&l_filter);

                let l_canvas_tag = format!("p_grp_l_canvas_{}", grp_idx);
                let l_branch_tag = format!("p_grp_l_branch_{}", grp_idx);
                filter_complex.push_str(&format!(
                    "color=c=black@0:s={}x{}:r={}:d=1[{}];",
                    plan.width,
                    plan.height,
                    plan.frame_rate.as_f64(),
                    l_canvas_tag
                ));
                filter_complex.push_str(&format!(
                    "[{}][{}]overlay=x='(W-w)/2+{:.1}':y='(H-h)/2+{:.1}',format=rgba,premultiply=inplace=1[{}];",
                    l_canvas_tag, l_clip_tag, left_layer.position.0, left_layer.position.1, l_branch_tag
                ));

                // Right participant on transparent canvas
                let mut r_filter = String::new();
                let r_clip_tag = format!("p_grp_r_clip_{}", grp_idx);
                let r_src_sec = right_layer.source_time.to_seconds();
                if let Some(ref path) = right_layer.asset_path {
                    let pbuf = PathBuf::from(path);
                    let input_idx = input_paths.iter().position(|p| p == &pbuf).unwrap_or(0);
                    r_filter.push_str(&format!(
                        "[{}:v]trim=start={:.3}:duration=0.034,setpts=PTS-STARTPTS",
                        input_idx, r_src_sec
                    ));
                } else {
                    r_filter.push_str(&format!(
                        "color=c=blue:s={}x{}:d=1",
                        right_layer.computed_bounds.width as u32,
                        right_layer.computed_bounds.height as u32
                    ));
                }
                if right_layer.crop.left > 0.0
                    || right_layer.crop.right > 0.0
                    || right_layer.crop.top > 0.0
                    || right_layer.crop.bottom > 0.0
                {
                    r_filter.push_str(&format!(
                        ",crop=w='iw*(1.0-{:.4}-{:.4})':h='ih*(1.0-{:.4}-{:.4})':x='iw*{:.4}':y='ih*{:.4}'",
                        right_layer.crop.left, right_layer.crop.right, right_layer.crop.top, right_layer.crop.bottom, right_layer.crop.left, right_layer.crop.top
                    ));
                }
                let mut r_rw = right_layer.computed_bounds.width.round().max(2.0) as u32;
                let mut r_rh = right_layer.computed_bounds.height.round().max(2.0) as u32;
                if !r_rw.is_multiple_of(2) {
                    r_rw += 1;
                }
                if !r_rh.is_multiple_of(2) {
                    r_rh += 1;
                }
                r_filter.push_str(&format!(",scale={}:{}", r_rw, r_rh));
                if right_layer.rotation.abs() > 0.01 {
                    r_filter.push_str(&format!(
                        ",rotate='{0:.4}*PI/180':ow='rotw({0:.4}*PI/180)':oh='roth({0:.4}*PI/180)':c=none",
                        right_layer.rotation
                    ));
                }
                r_filter.push_str(&format!(
                    ",format=yuva420p,colorchannelmixer=aa={:.3}[{}];",
                    right_layer.opacity, r_clip_tag
                ));
                filter_complex.push_str(&r_filter);

                let r_canvas_tag = format!("p_grp_r_canvas_{}", grp_idx);
                let r_branch_tag = format!("p_grp_r_branch_{}", grp_idx);
                filter_complex.push_str(&format!(
                    "color=c=black@0:s={}x{}:r={}:d=1[{}];",
                    plan.width,
                    plan.height,
                    plan.frame_rate.as_f64(),
                    r_canvas_tag
                ));
                filter_complex.push_str(&format!(
                    "[{}][{}]overlay=x='(W-w)/2+{:.1}':y='(H-h)/2+{:.1}',format=rgba,premultiply=inplace=1[{}];",
                    r_canvas_tag, r_clip_tag, right_layer.position.0, right_layer.position.1, r_branch_tag
                ));

                // Inside the group: plus-lighter blend (or improper source-over for negative control)
                let grp_out = format!("p_grp_out_{}", grp_idx);
                if use_improper_source_over {
                    filter_complex.push_str(&format!(
                        "[{}][{}]overlay=0:0,format=rgba[{}];",
                        l_branch_tag, r_branch_tag, grp_out
                    ));
                } else {
                    filter_complex.push_str(&format!(
                        "[{}][{}]blend=all_mode=addition,unpremultiply=inplace=1,format=rgba[{}];",
                        l_branch_tag, r_branch_tag, grp_out
                    ));
                }

                let next_base = format!("p_over_{}", idx);
                idx += 1;
                grp_idx += 1;

                filter_complex.push_str(&format!(
                    "[{}][{}]overlay=0:0[{}];",
                    current_base, grp_out, next_base
                ));
                current_base = next_base;
            }
        }
    }

    args.push("-filter_complex".to_string());
    args.push(filter_complex);
    args.push("-map".to_string());
    args.push(format!("[{}]", current_base));
    args.push("-vframes".to_string());
    args.push("1".to_string());
    args.push("-f".to_string());
    args.push("rawvideo".to_string());
    args.push("-pix_fmt".to_string());
    args.push("rgba".to_string());
    args.push("pipe:1".to_string());

    let mut cmd = MediaBinaryResolver::command("ffmpeg")
        .map_err(|e| format!("Failed to resolve FFmpeg binary: {e}"))?;
    let output = cmd
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to run preview FFmpeg: {e}"))?;

    if output.stdout.len() < expected_len {
        let err_str = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Preview frame truncated (expected {} bytes, got {}): {}",
            expected_len,
            output.stdout.len(),
            err_str
        ));
    }

    Ok(output.stdout[0..expected_len].to_vec())
}

/// Evaluates a single parity fixture against both semantic and visual criteria
pub fn evaluate_parity_fixture(fixture_id: &str) -> Result<ParityFixtureResult, String> {
    let dummy_dir = std::env::temp_dir();
    let (project, timestamps, name) = build_fixture_project(fixture_id, &dummy_dir)?;
    let plan = CompositionPlan::from_project(&project).map_err(|e| e.to_string())?;

    let mut semantic_all_passed = true;
    let mut visual_all_passed = true;
    let mut details = Vec::new();
    let mut last_metrics = None;
    let mut p_art = None;
    let mut e_art = None;
    let mut d_art = None;

    for ts in timestamps {
        let preview_snap = ParitySemanticSnapshot::from_preview(&plan, ts);
        let export_snap = ParitySemanticSnapshot::from_export(&plan, ts);

        // 1. Level 1 & 2: Semantic and Timing/Geometry Parity
        let sem_res = compare_semantic_snapshots(&preview_snap, &export_snap, 0.1);
        let sem_ok = sem_res.is_ok();
        if let Err(errs) = sem_res {
            semantic_all_passed = false;
            details.push(format!("Semantic error at frame {}: {:?}", ts.value, errs));
        }

        // 2. Level 3: Raster Parity
        let p_rgba = render_preview_frame_rgba(&plan, ts)?;
        let e_rgba = render_export_frame_rgba(&plan, ts)?;

        // Documented tolerance thresholds:
        // Video scaler/chroma differences: MAE <= 4.0, Exceeded ratio <= 0.03
        // Text glyph rendering differences: MAE <= 10.0, Exceeded ratio <= 0.08
        let is_text_fixture = fixture_id == "P7-F10" || fixture_id == "P7-F12";
        let max_mae = if is_text_fixture { 10.0 } else { 4.0 };
        let max_ratio = if is_text_fixture { 0.08 } else { 0.03 };

        let (vis_ok, metrics, diff_rgba) = compare_rgba_buffers(
            &p_rgba,
            &e_rgba,
            plan.width,
            plan.height,
            16,
            max_mae,
            max_ratio,
        );

        if !vis_ok {
            visual_all_passed = false;
            details.push(format!(
                "Visual diff exceeded threshold at frame {}: MAE={:.2}, ExceededRatio={:.3}",
                ts.value, metrics.mae, metrics.exceeded_ratio
            ));
        }

        // Write diagnostic artifacts on visual failure or for the primary sample
        let (p_p, e_p, d_p, _) = write_diagnostic_artifacts(
            fixture_id,
            ts,
            &p_rgba,
            &e_rgba,
            &diff_rgba,
            plan.width,
            plan.height,
            &metrics,
            sem_ok,
            vis_ok,
        );

        p_art = Some(p_p.to_string_lossy().to_string());
        e_art = Some(e_p.to_string_lossy().to_string());
        d_art = Some(d_p.to_string_lossy().to_string());
        last_metrics = Some(metrics);
    }

    let overall = semantic_all_passed && visual_all_passed;

    Ok(ParityFixtureResult {
        fixture_id: fixture_id.to_string(),
        name,
        timestamp_frames: 30,
        semantic_status: if semantic_all_passed { "PASS" } else { "FAIL" }.to_string(),
        visual_status: if visual_all_passed { "PASS" } else { "FAIL" }.to_string(),
        overall_status: if overall { "PASS" } else { "FAIL" }.to_string(),
        metrics: last_metrics,
        details: if details.is_empty() {
            "All semantic & visual parity checks satisfied".to_string()
        } else {
            details.join("; ")
        },
        preview_artifact: p_art,
        export_artifact: e_art,
        diff_artifact: d_art,
    })
}

/// Evaluates negative control test to prove harness sensitivity to deliberate drift
pub fn verify_negative_control() -> bool {
    let dummy_dir = std::env::temp_dir();
    let (project, timestamps, _) = match build_fixture_project("P7-F04", &dummy_dir) {
        Ok(res) => res,
        Err(_) => return false,
    };
    let plan = match CompositionPlan::from_project(&project) {
        Ok(p) => p,
        Err(_) => return false,
    };
    let ts = timestamps[0];

    let preview_snap = ParitySemanticSnapshot::from_preview(&plan, ts);
    let mut corrupted_export_snap = ParitySemanticSnapshot::from_export(&plan, ts);

    // Introduce deliberate position drift (+120px on position_x)
    if let Some(l) = corrupted_export_snap.layers.get_mut(0) {
        l.position.0 += 120.0;
        l.computed_bounds.x += 120.0;
    }

    let sem_cmp = compare_semantic_snapshots(&preview_snap, &corrupted_export_snap, 0.1);
    // Harness MUST detect discrepancy and return Err!
    sem_cmp.is_err()
}

/// Evaluates negative control test for Phase 13 CrossDissolve to prove that improper
/// source-over blending (instead of plus-lighter premultiplied addition) fails the parity gate.
pub fn verify_phase13_negative_control() -> bool {
    let dummy_dir = std::env::temp_dir();
    let (project, timestamps, _) = match build_fixture_project("P13-F01", &dummy_dir) {
        Ok(res) => res,
        Err(_) => return false,
    };
    let plan = match CompositionPlan::from_project(&project) {
        Ok(p) => p,
        Err(_) => return false,
    };
    let ts = timestamps[0];

    // Render preview frame with deliberate improper source-over
    let preview_improper_rgba = match render_preview_frame_rgba_with_blend_mode(&plan, ts, true) {
        Ok(buf) => buf,
        Err(_) => return false,
    };
    let export_rgba = match render_export_frame_rgba(&plan, ts) {
        Ok(buf) => buf,
        Err(_) => return false,
    };

    let (passed, metrics, _) = compare_rgba_buffers(
        &preview_improper_rgba,
        &export_rgba,
        plan.width,
        plan.height,
        16,
        4.0,
        0.03,
    );

    // Negative control MUST fail visual parity and exceed MAE threshold (> 4.0)
    !passed && metrics.mae > 4.0
}

/// Runs the complete POC 2 parity suite across all 12 fixtures
pub fn run_all_parity_fixtures() -> ParitySuiteReport {
    let fixture_ids = [
        "P7-F01", "P7-F02", "P7-F03", "P7-F04", "P7-F05", "P7-F06", "P7-F07", "P7-F08", "P7-F09",
        "P7-F10", "P7-F11", "P7-F12",
    ];

    let mut fixture_results = Vec::new();
    let mut passed_fixtures = 0;
    let mut failed_fixtures = 0;

    for fid in fixture_ids {
        match evaluate_parity_fixture(fid) {
            Ok(res) => {
                if res.overall_status == "PASS" {
                    passed_fixtures += 1;
                } else {
                    failed_fixtures += 1;
                }
                fixture_results.push(res);
            }
            Err(e) => {
                failed_fixtures += 1;
                fixture_results.push(ParityFixtureResult {
                    fixture_id: fid.to_string(),
                    name: fid.to_string(),
                    timestamp_frames: 0,
                    semantic_status: "FAIL".to_string(),
                    visual_status: "FAIL".to_string(),
                    overall_status: "FAIL".to_string(),
                    metrics: None,
                    details: format!("Execution error: {}", e),
                    preview_artifact: None,
                    export_artifact: None,
                    diff_artifact: None,
                });
            }
        }
    }

    let negative_control_passed = verify_negative_control();
    let total_fixtures = fixture_ids.len();

    let overall_decision = if passed_fixtures == total_fixtures && negative_control_passed {
        "PHASE 7: PASS".to_string()
    } else {
        "PHASE 7: BLOCKED".to_string()
    };

    ParitySuiteReport {
        total_fixtures,
        passed_fixtures,
        failed_fixtures,
        negative_control_passed,
        overall_decision,
        fixture_results,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::domain::{Keyframe, KeyframeProperty};

    #[test]
    fn test_phase18_audio_crossfade_preview_export_parity_detects_drift() {
        use crate::modules::domain::{AudioCrossfade, MediaKind};

        let rate = FrameRate::FPS_30;
        let mut project = Project::new("Phase 18 Audio Parity".to_string());
        project.assets.push(Asset {
            asset_id: "tone".to_string(),
            name: "tone.wav".to_string(),
            path: "tone.wav".to_string(),
            media_kind: MediaKind::Audio,
            duration: Some(RationalTime::new(70, rate)),
            file_size: 0,
        });
        let mut track = Track::new("Audio".to_string(), TrackKind::Audio);
        let left = TimelineItem::new(
            Some("tone".to_string()),
            "Outgoing".to_string(),
            ItemKind::Audio,
            TimeRange::new(RationalTime::new(0, rate), RationalTime::new(60, rate)),
            TimeRange::new(RationalTime::new(0, rate), RationalTime::new(60, rate)),
        )
        .unwrap();
        let right = TimelineItem::new(
            Some("tone".to_string()),
            "Incoming".to_string(),
            ItemKind::Audio,
            TimeRange::new(RationalTime::new(60, rate), RationalTime::new(60, rate)),
            TimeRange::new(RationalTime::new(10, rate), RationalTime::new(60, rate)),
        )
        .unwrap();
        track.audio_transitions.push(AudioCrossfade {
            transition_id: "fade".to_string(),
            track_id: track.track_id.clone(),
            left_item_id: left.item_id.clone(),
            right_item_id: right.item_id.clone(),
            duration: RationalTime::new(20, rate),
        });
        track.items.extend([left, right]);
        project.tracks.push(track);
        let plan = CompositionPlan::from_project(&project).expect("audio plan compiles");

        for frame in [50, 55, 60, 69, 70, 80] {
            let time = RationalTime::new(frame, rate);
            let preview = ParitySemanticSnapshot::from_preview(&plan, time);
            let export = ParitySemanticSnapshot::from_export(&plan, time);
            assert!(
                compare_semantic_snapshots(&preview, &export, 0.01).is_ok(),
                "preview and export should match at frame {frame}"
            );
        }

        let time = RationalTime::new(55, rate);
        let preview = ParitySemanticSnapshot::from_preview(&plan, time);
        let mut altered_export = ParitySemanticSnapshot::from_export(&plan, time);
        altered_export.audio[0].effective_volume *= 0.75;
        assert!(
            compare_semantic_snapshots(&preview, &altered_export, 0.01).is_err(),
            "parity comparison must detect a deliberately altered crossfade gain"
        );
    }

    /// Gate 7A: Semantic pipeline understood and mapped
    #[test]
    fn test_gate_7a_semantic_pipeline() {
        let dummy_dir = std::env::temp_dir();
        let (project, timestamps, _) = build_fixture_project("P7-F01", &dummy_dir).unwrap();
        let plan = CompositionPlan::from_project(&project).unwrap();

        let preview_snap = ParitySemanticSnapshot::from_preview(&plan, timestamps[0]);
        let export_snap = ParitySemanticSnapshot::from_export(&plan, timestamps[0]);

        assert_eq!(preview_snap.canvas, (640, 360));
        assert_eq!(preview_snap.layers.len(), 1);
        assert!(compare_semantic_snapshots(&preview_snap, &export_snap, 0.1).is_ok());
    }

    /// Gate 7B: All 12 deterministic fixtures constructible and well-formed
    #[test]
    fn test_gate_7b_all_12_deterministic_fixtures() {
        let dummy_dir = std::env::temp_dir();
        let fixture_ids = [
            "P7-F01", "P7-F02", "P7-F03", "P7-F04", "P7-F05", "P7-F06", "P7-F07", "P7-F08",
            "P7-F09", "P7-F10", "P7-F11", "P7-F12",
        ];

        for fid in fixture_ids {
            let (proj, timestamps, desc) = build_fixture_project(fid, &dummy_dir).unwrap();
            assert!(!proj.tracks.is_empty(), "Fixture {} has no tracks", fid);
            assert!(!timestamps.is_empty(), "Fixture {} has no timestamps", fid);
            assert!(!desc.is_empty(), "Fixture {} has no description", fid);

            let plan = CompositionPlan::from_project(&proj).unwrap();
            for ts in timestamps {
                let p_snap = ParitySemanticSnapshot::from_preview(&plan, ts);
                let e_snap = ParitySemanticSnapshot::from_export(&plan, ts);
                assert!(
                    compare_semantic_snapshots(&p_snap, &e_snap, 0.1).is_ok(),
                    "Semantic snapshot mismatch in fixture {}",
                    fid
                );
            }
        }
    }

    /// Gate 7C: Both render paths capturable without error
    #[test]
    fn test_gate_7c_render_paths_capturable() {
        let dummy_dir = std::env::temp_dir();
        let (project, timestamps, _) = build_fixture_project("P7-F01", &dummy_dir).unwrap();
        let plan = CompositionPlan::from_project(&project).unwrap();

        let p_rgba = render_preview_frame_rgba(&plan, timestamps[0]).unwrap();
        let e_rgba = render_export_frame_rgba(&plan, timestamps[0]).unwrap();

        assert_eq!(p_rgba.len(), (640 * 360 * 4) as usize);
        assert_eq!(e_rgba.len(), (640 * 360 * 4) as usize);
    }

    /// Gate 7D: Harness detects drift on deliberate perturbation (Negative Control)
    #[test]
    fn test_gate_7d_negative_control_detects_drift() {
        assert!(
            verify_negative_control(),
            "Negative control test failed: harness did not detect deliberate position drift!"
        );
    }

    /// Gate 7E: Shared composition semantics pass within documented tolerance
    #[test]
    fn test_gate_7e_shared_semantics_pass() {
        let report = run_all_parity_fixtures();
        println!("Parity Report Decision: {}", report.overall_decision);
        for res in &report.fixture_results {
            println!(
                "Fixture {} ({}): Semantic={}, Visual={}, Overall={}, Details={}",
                res.fixture_id,
                res.name,
                res.semantic_status,
                res.visual_status,
                res.overall_status,
                res.details
            );
        }
        assert_eq!(report.overall_decision, "PHASE 7: PASS");
        assert_eq!(report.passed_fixtures, report.total_fixtures);
        assert!(report.negative_control_passed);
    }

    /// Milestone 11: Animated preview and export parity across all keyframe properties
    #[test]
    fn test_phase9_animation_parity() {
        let fps = FrameRate::FPS_30;
        let mut project = Project::new("Phase 9 Animation Parity".to_string());
        project.settings.width = 640;
        project.settings.height = 360;
        project.settings.frame_rate = fps;

        let mut track = Track::new("Anim Track".to_string(), TrackKind::Video);
        let mut clip = TimelineItem::new(
            None,
            "Animated Clip".to_string(),
            ItemKind::Video,
            TimeRange::new(RationalTime::new(0, fps), RationalTime::new(120, fps)),
            TimeRange::new(RationalTime::new(0, fps), RationalTime::new(120, fps)),
        )
        .unwrap();

        // Add keyframes for PositionX, Scale, Rotation, Opacity
        // First keyframe at offset 20 frames, second keyframe at offset 80 frames
        clip.keyframes.push(Keyframe::new(
            KeyframeProperty::PositionX,
            RationalTime::new(20, fps),
            0.0,
        ));
        clip.keyframes.push(Keyframe::new(
            KeyframeProperty::PositionX,
            RationalTime::new(80, fps),
            120.0,
        ));

        clip.keyframes.push(Keyframe::new(
            KeyframeProperty::Opacity,
            RationalTime::new(20, fps),
            0.2,
        ));
        clip.keyframes.push(Keyframe::new(
            KeyframeProperty::Opacity,
            RationalTime::new(80, fps),
            0.8,
        ));

        clip.keyframes.push(Keyframe::new(
            KeyframeProperty::ScaleX,
            RationalTime::new(20, fps),
            0.5,
        ));
        clip.keyframes.push(Keyframe::new(
            KeyframeProperty::ScaleX,
            RationalTime::new(80, fps),
            1.0,
        ));

        clip.keyframes.push(Keyframe::new(
            KeyframeProperty::Rotation,
            RationalTime::new(20, fps),
            0.0,
        ));
        clip.keyframes.push(Keyframe::new(
            KeyframeProperty::Rotation,
            RationalTime::new(80, fps),
            45.0,
        ));

        track.items.push(clip);
        project.tracks.push(track);

        // Add Audio track with Volume keyframes (Milestone 9.2 & Gap B)
        let mut audio_track = Track::new("Audio Track".to_string(), TrackKind::Audio);
        let mut audio_clip = TimelineItem::new(
            None,
            "Animated Audio".to_string(),
            ItemKind::Audio,
            TimeRange::new(RationalTime::new(0, fps), RationalTime::new(120, fps)),
            TimeRange::new(RationalTime::new(0, fps), RationalTime::new(120, fps)),
        )
        .unwrap();
        audio_clip.keyframes.push(Keyframe::new(
            KeyframeProperty::Volume,
            RationalTime::new(20, fps),
            0.2,
        ));
        audio_clip.keyframes.push(Keyframe::new(
            KeyframeProperty::Volume,
            RationalTime::new(80, fps),
            0.8,
        ));
        let audio_clip_id = audio_clip.item_id.clone();
        audio_track.items.push(audio_clip);
        project.tracks.push(audio_track);

        let plan = CompositionPlan::from_project(&project).unwrap();

        // Sample points:
        // T0 = 10 frames (before first: hold first value)
        // T1 = 20 frames (first keyframe)
        // Tmid = 50 frames (midpoint: linear interpolation)
        // T2 = 80 frames (second keyframe)
        // T3 = 100 frames (after last: hold second value)
        let sample_times = [
            (RationalTime::new(10, fps), 0.0, 0.2, 0.5, 0.0, 0.2),
            (RationalTime::new(20, fps), 0.0, 0.2, 0.5, 0.0, 0.2),
            (RationalTime::new(50, fps), 60.0, 0.5, 0.75, 22.5, 0.5),
            (RationalTime::new(80, fps), 120.0, 0.8, 1.0, 45.0, 0.8),
            (RationalTime::new(100, fps), 120.0, 0.8, 1.0, 45.0, 0.8),
        ];

        for (ts, exp_pos_x, exp_op, exp_scale, exp_rot, exp_vol) in sample_times {
            let p_snap = ParitySemanticSnapshot::from_preview(&plan, ts);
            let e_snap = ParitySemanticSnapshot::from_export(&plan, ts);

            let layer_p = &p_snap.layers[0];
            let layer_e = &e_snap.layers[0];

            assert!(
                (layer_p.position.0 - exp_pos_x).abs() < 0.1,
                "Preview pos_x mismatch at frame {}",
                ts.value
            );
            assert!(
                (layer_e.position.0 - exp_pos_x).abs() < 0.1,
                "Export pos_x mismatch at frame {}",
                ts.value
            );

            assert!(
                (layer_p.opacity - exp_op).abs() < 0.01,
                "Preview opacity mismatch at frame {}",
                ts.value
            );
            assert!(
                (layer_e.opacity - exp_op).abs() < 0.01,
                "Export opacity mismatch at frame {}",
                ts.value
            );

            assert!(
                (layer_p.scale.0 - exp_scale).abs() < 0.01,
                "Preview scale_x mismatch at frame {}",
                ts.value
            );
            assert!(
                (layer_e.scale.0 - exp_scale).abs() < 0.01,
                "Export scale_x mismatch at frame {}",
                ts.value
            );

            assert!(
                (layer_p.rotation - exp_rot).abs() < 0.1,
                "Preview rotation mismatch at frame {}",
                ts.value
            );
            assert!(
                (layer_e.rotation - exp_rot).abs() < 0.1,
                "Export rotation mismatch at frame {}",
                ts.value
            );

            let audio_p = p_snap
                .audio
                .iter()
                .find(|a| a.item_id == audio_clip_id)
                .unwrap();
            let audio_e = e_snap
                .audio
                .iter()
                .find(|a| a.item_id == audio_clip_id)
                .unwrap();
            assert!(
                (audio_p.effective_volume - exp_vol).abs() < 0.01,
                "Preview volume mismatch at frame {}",
                ts.value
            );
            assert!(
                (audio_e.effective_volume - exp_vol).abs() < 0.01,
                "Export volume mismatch at frame {}",
                ts.value
            );

            assert!(
                compare_semantic_snapshots(&p_snap, &e_snap, 0.1).is_ok(),
                "Semantic snapshot mismatch during animation at frame {}",
                ts.value
            );
        }
    }

    /// Milestone 9.3: Transition preview and export parity for FadeIn, FadeOut, and CrossDissolve
    #[test]
    fn test_phase9_transition_parity() {
        use crate::modules::domain::{Transition, TransitionKind};

        let fps = FrameRate::FPS_30;
        let mut project = Project::new("Phase 9 Transition Parity".to_string());
        project.settings.width = 640;
        project.settings.height = 360;
        project.settings.frame_rate = fps;

        let mut track = Track::new("Trans Track".to_string(), TrackKind::Video);

        // Clip 1: FadeIn transition of 30 frames on a 90-frame clip (frames 0 to 90)
        let mut clip_fadein = TimelineItem::new(
            None,
            "FadeIn Clip".to_string(),
            ItemKind::Video,
            TimeRange::new(RationalTime::new(0, fps), RationalTime::new(90, fps)),
            TimeRange::new(RationalTime::new(0, fps), RationalTime::new(90, fps)),
        )
        .unwrap();
        clip_fadein.opacity = 1.0;
        clip_fadein.transition = Some(Transition {
            kind: TransitionKind::FadeIn,
            duration: RationalTime::new(30, fps),
        });

        // Clip 2: FadeOut transition of 30 frames on a 90-frame clip (frames 100 to 190)
        let mut clip_fadeout = TimelineItem::new(
            None,
            "FadeOut Clip".to_string(),
            ItemKind::Video,
            TimeRange::new(RationalTime::new(100, fps), RationalTime::new(90, fps)),
            TimeRange::new(RationalTime::new(0, fps), RationalTime::new(90, fps)),
        )
        .unwrap();
        clip_fadeout.opacity = 1.0;
        clip_fadeout.transition = Some(Transition {
            kind: TransitionKind::FadeOut,
            duration: RationalTime::new(30, fps),
        });

        track.items.push(clip_fadein);
        track.items.push(clip_fadeout);
        project.tracks.push(track);

        let plan = CompositionPlan::from_project(&project).unwrap();

        // Test FadeIn sample points on Clip 1 (frames 0 to 90):
        // T1 = frame 0 (start: opacity 0.0)
        // Tmid = frame 15 (midpoint: opacity 0.5)
        // T2 = frame 29 (end-1: opacity 29/30 = 0.967)
        // T3 = frame 30 (end of transition: opacity 1.0)
        // T4 = frame 60 (after transition: opacity 1.0)
        let fadein_samples = [
            (RationalTime::new(0, fps), 0.0),
            (RationalTime::new(15, fps), 0.5),
            (RationalTime::new(29, fps), 29.0 / 30.0),
            (RationalTime::new(30, fps), 1.0),
            (RationalTime::new(60, fps), 1.0),
        ];

        for (ts, exp_op) in fadein_samples {
            let p_snap = ParitySemanticSnapshot::from_preview(&plan, ts);
            let e_snap = ParitySemanticSnapshot::from_export(&plan, ts);

            let layer_p = &p_snap.layers[0];
            let layer_e = &e_snap.layers[0];

            assert!(
                (layer_p.opacity - exp_op).abs() < 0.01,
                "FadeIn preview opacity mismatch at frame {}",
                ts.value
            );
            assert!(
                (layer_e.opacity - exp_op).abs() < 0.01,
                "FadeIn export opacity mismatch at frame {}",
                ts.value
            );
            assert!(compare_semantic_snapshots(&p_snap, &e_snap, 0.1).is_ok());
        }

        // Test FadeOut sample points on Clip 2 (range: 100 to 190 frames, duration 90 frames):
        // Fade starts at clip offset 60 (timeline frame 160).
        // T0 = frame 130 (before fade: opacity 1.0)
        // T1 = frame 160 (fade start: opacity 1.0)
        // Tmid = frame 175 (midpoint: offset 75 -> remaining 15/30 = 0.5)
        // T2 = frame 189 (end-1: offset 89 -> remaining 1/30 = 0.033)
        let fadeout_samples = [
            (RationalTime::new(130, fps), 1.0),
            (RationalTime::new(160, fps), 1.0),
            (RationalTime::new(175, fps), 0.5),
            (RationalTime::new(189, fps), 1.0 / 30.0),
        ];

        for (ts, exp_op) in fadeout_samples {
            let p_snap = ParitySemanticSnapshot::from_preview(&plan, ts);
            let e_snap = ParitySemanticSnapshot::from_export(&plan, ts);

            let layer_p = &p_snap.layers[0];
            let layer_e = &e_snap.layers[0];

            assert!(
                (layer_p.opacity - exp_op).abs() < 0.01,
                "FadeOut preview opacity mismatch at frame {}",
                ts.value
            );
            assert!(
                (layer_e.opacity - exp_op).abs() < 0.01,
                "FadeOut export opacity mismatch at frame {}",
                ts.value
            );
            assert!(compare_semantic_snapshots(&p_snap, &e_snap, 0.1).is_ok());
        }
    }

    /// Phase 13: Deterministic parity fixtures P13-F01 through P13-F07
    #[test]
    fn test_phase13_fixtures_parity() {
        let fixtures = [
            "P13-F01", "P13-F02", "P13-F03", "P13-F04", "P13-F05", "P13-F06", "P13-F07",
        ];
        for fid in fixtures {
            let res = evaluate_parity_fixture(fid)
                .unwrap_or_else(|e| panic!("Failed to evaluate {}: {}", fid, e));
            assert_eq!(
                res.overall_status, "PASS",
                "Fixture {} failed parity: {}",
                fid, res.details
            );
        }
    }

    /// Phase 13: Negative control test verifies deliberate improper source-over blend fails parity
    #[test]
    fn test_phase13_negative_control() {
        assert!(
            verify_phase13_negative_control(),
            "Phase 13 negative control must fail parity when improper source-over blend is used"
        );
    }

    /// Phase 13: Handle limits validation (exact-sufficient vs 1-tick short)
    #[test]
    fn test_phase13_handle_limits() {
        use crate::modules::command::{
            CommandEnvelope, CommandOperation, CommandService, OperationEnvelope,
        };
        use crate::modules::domain::time::{FrameRate, RationalTime, TimeRange};
        use crate::modules::domain::{
            Asset, ItemKind, MediaKind, Project, TimelineItem, Track, TrackKind,
            CURRENT_SCHEMA_VERSION,
        };

        let fps = FrameRate::FPS_30;
        let mut project = Project::new("Handle Limits Test".to_string());
        project.settings.frame_rate = fps;

        let ast1 = Asset {
            asset_id: "ast1".to_string(),
            name: "v1.mp4".to_string(),
            path: "v1.mp4".to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(70, fps)),
            file_size: 1000,
        };
        let ast2 = Asset {
            asset_id: "ast2".to_string(),
            name: "v2.mp4".to_string(),
            path: "v2.mp4".to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(70, fps)),
            file_size: 1000,
        };
        project.assets.push(ast1);
        project.assets.push(ast2);

        let mut track = Track::new("Video Track".to_string(), TrackKind::Video);
        // Clip A: timeline 0..60, source 0..60. Asset duration 70. Postroll = 70 - 60 = 10 ticks.
        let mut it1 = TimelineItem::new(
            Some("ast1".to_string()),
            "A".to_string(),
            ItemKind::Video,
            TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps)),
            TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps)),
        )
        .unwrap();
        it1.item_id = "it1".to_string();

        // Clip B: timeline 60..120, source 10..70. Preroll = 10 ticks.
        let mut it2 = TimelineItem::new(
            Some("ast2".to_string()),
            "B".to_string(),
            ItemKind::Video,
            TimeRange::new(RationalTime::new(60, fps), RationalTime::new(60, fps)),
            TimeRange::new(RationalTime::new(10, fps), RationalTime::new(60, fps)),
        )
        .unwrap();
        it2.item_id = "it2".to_string();

        let track_id = track.track_id.clone();
        track.items.push(it1);
        track.items.push(it2);
        project.tracks.push(track);

        let mut service = CommandService::new(project);

        // 1. Duration = 20: L = 10, R = 10. Exactly sufficient (postroll=10 >= R=10, preroll=10 >= L=10).
        let env_ok = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: service.project().project_id.clone(),
            base_revision: service.revision(),
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: None,
            operations: vec![OperationEnvelope {
                operation_id: "op_trans_ok".to_string(),
                operation: CommandOperation::CutTransitionCreate {
                    track_id: track_id.clone(),
                    transition_id: Some("tr_ok".to_string()),
                    kind: crate::modules::composition::TransitionKind::CrossDissolve,
                    left_item_id: "it1".to_string(),
                    right_item_id: "it2".to_string(),
                    duration: RationalTime::new(20, fps),
                },
            }],
        };
        let res_ok = service.execute(env_ok);
        assert!(res_ok.is_ok(), "Exact sufficient handle should succeed");

        // Remove it to test 1-tick short
        let env_rm = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: service.project().project_id.clone(),
            base_revision: service.revision(),
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: None,
            operations: vec![OperationEnvelope {
                operation_id: "op_trans_rm".to_string(),
                operation: CommandOperation::CutTransitionRemove {
                    transition_id: "tr_ok".to_string(),
                },
            }],
        };
        service.execute(env_rm).unwrap();

        // 2. Duration = 22: L = 11, R = 11. 1-tick short! (postroll=10 < 11).
        let env_err = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: service.project().project_id.clone(),
            base_revision: service.revision(),
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: None,
            operations: vec![OperationEnvelope {
                operation_id: "op_trans_err".to_string(),
                operation: CommandOperation::CutTransitionCreate {
                    track_id: track_id.clone(),
                    transition_id: Some("tr_err".to_string()),
                    kind: crate::modules::composition::TransitionKind::CrossDissolve,
                    left_item_id: "it1".to_string(),
                    right_item_id: "it2".to_string(),
                    duration: RationalTime::new(22, fps),
                },
            }],
        };
        let res_err = service.execute(env_err);
        assert!(res_err.is_err(), "1-tick short handle should fail");
        let err = res_err.err().unwrap();
        assert!(matches!(
            err,
            crate::modules::command::CommandError::InsufficientTransitionHandle { .. }
        ));
    }

    /// Phase 13: Offline media with persisted duration allows handle validation
    #[test]
    fn test_phase13_offline_media_persisted_duration() {
        use crate::modules::command::{
            CommandEnvelope, CommandOperation, CommandService, OperationEnvelope,
        };
        use crate::modules::domain::time::{FrameRate, RationalTime, TimeRange};
        use crate::modules::domain::{
            Asset, ItemKind, MediaKind, Project, TimelineItem, Track, TrackKind,
            CURRENT_SCHEMA_VERSION,
        };

        let fps = FrameRate::FPS_30;
        let mut project = Project::new("Offline Media Duration Test".to_string());
        project.settings.frame_rate = fps;

        let ast1 = Asset {
            asset_id: "ast_off1".to_string(),
            name: "offline1.mp4".to_string(),
            path: "C:/nonexistent/offline1.mp4".to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(100, fps)),
            file_size: 5000,
        };
        let ast2 = Asset {
            asset_id: "ast_off2".to_string(),
            name: "offline2.mp4".to_string(),
            path: "C:/nonexistent/offline2.mp4".to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(100, fps)),
            file_size: 5000,
        };
        project.assets.push(ast1);
        project.assets.push(ast2);

        let mut track = Track::new("Video Track".to_string(), TrackKind::Video);
        let mut it1 = TimelineItem::new(
            Some("ast_off1".to_string()),
            "A".to_string(),
            ItemKind::Video,
            TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps)),
            TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps)),
        )
        .unwrap();
        it1.item_id = "it1".to_string();

        let mut it2 = TimelineItem::new(
            Some("ast_off2".to_string()),
            "B".to_string(),
            ItemKind::Video,
            TimeRange::new(RationalTime::new(60, fps), RationalTime::new(60, fps)),
            TimeRange::new(RationalTime::new(10, fps), RationalTime::new(60, fps)),
        )
        .unwrap();
        it2.item_id = "it2".to_string();

        let track_id = track.track_id.clone();
        track.items.push(it1);
        track.items.push(it2);
        project.tracks.push(track);

        let mut service = CommandService::new(project);

        let env = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: service.project().project_id.clone(),
            base_revision: service.revision(),
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: None,
            operations: vec![OperationEnvelope {
                operation_id: "op_trans_offline".to_string(),
                operation: CommandOperation::CutTransitionCreate {
                    track_id: track_id.clone(),
                    transition_id: Some("tr_offline".to_string()),
                    kind: crate::modules::composition::TransitionKind::CrossDissolve,
                    left_item_id: "it1".to_string(),
                    right_item_id: "it2".to_string(),
                    duration: RationalTime::new(20, fps),
                },
            }],
        };
        let res = service.execute(env);
        assert!(
            res.is_ok(),
            "Offline media with persisted duration should succeed"
        );

        // If asset has NO duration (None), handle is unverifiable
        let mut proj = service.project().clone();
        proj.assets[0].duration = None;
        service.set_project(proj);

        let env_unverifiable = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: service.project().project_id.clone(),
            base_revision: service.revision(),
            client_id: None,
            client_epoch: None,
            sequence_number: None,
            dry_run: false,
            transaction_id: None,
            operations: vec![OperationEnvelope {
                operation_id: "op_trans_unverifiable".to_string(),
                operation: CommandOperation::CutTransitionUpdateDuration {
                    transition_id: "tr_offline".to_string(),
                    new_duration: RationalTime::new(30, fps),
                },
            }],
        };
        let res_unv = service.execute(env_unverifiable);
        assert!(res_unv.is_err());
        assert!(matches!(
            res_unv.err().unwrap(),
            crate::modules::command::CommandError::TransitionHandleUnverifiable { .. }
        ));
    }

    /// Phase 13: Schema v2 save/reopen exact roundtrip
    #[test]
    fn test_phase13_schema_v2_save_reopen() {
        use crate::modules::domain::time::{FrameRate, RationalTime, TimeRange};
        use crate::modules::domain::{
            CutTransition, ItemKind, Project, TimelineItem, Track, TrackKind,
        };
        use crate::modules::persistence::{load_project, save_project_atomic};

        let fps = FrameRate::FPS_30;
        let mut project = Project::new("Schema v2 Save Reopen".to_string());
        project.settings.frame_rate = fps;

        let mut track = Track::new("Video Track".to_string(), TrackKind::Video);
        let mut item1 = TimelineItem::new(
            None,
            "Clip A".to_string(),
            ItemKind::Video,
            TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps)),
            TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps)),
        )
        .unwrap();
        item1.item_id = "it1".to_string();

        let mut item2 = TimelineItem::new(
            None,
            "Clip B".to_string(),
            ItemKind::Video,
            TimeRange::new(RationalTime::new(60, fps), RationalTime::new(60, fps)),
            TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps)),
        )
        .unwrap();
        item2.item_id = "it2".to_string();

        let cut_trans = CutTransition {
            transition_id: "trans_test".to_string(),
            track_id: track.track_id.clone(),
            kind: crate::modules::composition::TransitionKind::CrossDissolve,
            left_item_id: "it1".to_string(),
            right_item_id: "it2".to_string(),
            duration: RationalTime::new(20, fps),
        };

        track.items.push(item1);
        track.items.push(item2);
        track.transitions.push(cut_trans);
        project.tracks.push(track);

        let temp_file =
            std::env::temp_dir().join(format!("tinix_test_v2_save_{}.json", uuid::Uuid::new_v4()));
        save_project_atomic(&project, &temp_file).expect("Failed to save project");

        let loaded = load_project(&temp_file).expect("Failed to load project");
        let _ = std::fs::remove_file(temp_file);

        assert_eq!(
            loaded.schema_version,
            crate::modules::domain::CURRENT_SCHEMA_VERSION
        );
        assert_eq!(loaded.tracks.len(), 1);
        assert_eq!(loaded.tracks[0].transitions.len(), 1);
        let tr = &loaded.tracks[0].transitions[0];
        assert_eq!(tr.transition_id, "trans_test");
        assert_eq!(tr.left_item_id, "it1");
        assert_eq!(tr.right_item_id, "it2");
        assert_eq!(tr.duration, RationalTime::new(20, fps));
    }

    /// Phase 13: Schema v1 migration preserves fades and allows cut transitions
    #[test]
    fn test_phase13_v1_migration_preserves_fades_and_allows_transition() {
        use crate::modules::domain::time::{FrameRate, RationalTime, TimeRange};
        use crate::modules::domain::{ItemKind, Project, TimelineItem, Track, TrackKind};
        use crate::modules::persistence::load_project;

        let fps = FrameRate::FPS_30;
        let mut proj = Project::new("V1 Project".to_string());
        proj.schema_version = 1;

        let mut track = Track::new("Video Track".to_string(), TrackKind::Video);
        let mut it1 = TimelineItem::new(
            None,
            "Clip With Fade".to_string(),
            ItemKind::Video,
            TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps)),
            TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps)),
        )
        .unwrap();
        it1.transition = Some(crate::modules::domain::Transition {
            kind: crate::modules::composition::TransitionKind::FadeIn,
            duration: RationalTime::new(15, fps),
        });

        let it2 = TimelineItem::new(
            None,
            "Clip Adjacent".to_string(),
            ItemKind::Video,
            TimeRange::new(RationalTime::new(60, fps), RationalTime::new(60, fps)),
            TimeRange::new(RationalTime::new(0, fps), RationalTime::new(60, fps)),
        )
        .unwrap();

        track.items.push(it1);
        track.items.push(it2);
        proj.tracks.push(track);

        let mut val = serde_json::to_value(&proj).unwrap();
        val["schema_version"] = serde_json::json!(1);
        if let Some(tracks) = val["tracks"].as_array_mut() {
            for trk in tracks {
                if let Some(obj) = trk.as_object_mut() {
                    obj.remove("transitions");
                }
            }
        }

        let temp_file = std::env::temp_dir().join(format!(
            "tinix_test_v1_migration_{}.json",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&temp_file, serde_json::to_string_pretty(&val).unwrap()).unwrap();

        let loaded = load_project(&temp_file).expect("Failed to migrate v1 to v2");
        let _ = std::fs::remove_file(temp_file);

        assert_eq!(
            loaded.schema_version,
            crate::modules::domain::CURRENT_SCHEMA_VERSION
        );
        assert_eq!(loaded.tracks[0].items.len(), 2);
        // Verify FadeIn is preserved
        assert!(loaded.tracks[0].items[0].transition.is_some());
        assert_eq!(
            loaded.tracks[0].items[0].transition.as_ref().unwrap().kind,
            crate::modules::composition::TransitionKind::FadeIn
        );
        // Transitions array is initialized empty (migration does not invent CrossDissolve)
        assert_eq!(loaded.tracks[0].transitions.len(), 0);
    }
}
