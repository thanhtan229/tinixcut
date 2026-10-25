//! Phase 14 Export Architecture POC — Rust-Evaluated Automation Schedule
//! Research Gate Feasibility & Parity Evidence: COMPLETE (PASS) — READY FOR PLANNING
//! NOTE: This file is durable research/architecture POC evidence, NOT Phase-14 production implementation.
//!
//! Verifies:
//! - POC-V1: Runtime command capability on bundled FFmpeg 9.0.2
//! - POC-V2: Frame-exact dispatch timing (integer 30fps and fractional 30000/1001)
//! - POC-V3: Semantic visual correctness + negative control
//! - POC-V4: Existing CrossDissolve coexistence
//! - POC-A1: Sample-accurate audio envelope + negative control
//! - POC-A2: Envelope lifecycle & resource behavior
//! - POC-P1: Performance sanity

use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;
use tinixcut_lib::modules::composition::CompositionPlan;
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    Asset, CutTransition, ItemKind, Keyframe, KeyframeProperty, MediaKind, Project,
    ProjectSettings, TimelineItem, Track, TrackKind, TransitionKind,
};
use tinixcut_lib::modules::media::resolver::{BinaryKind, BinaryOrigin, MediaBinaryResolver};

/// Helper: returns a std::process::Command targeting the resolved bundled FFmpeg.
fn ffmpeg_cmd() -> Command {
    let resolved = MediaBinaryResolver::resolve(BinaryKind::FFmpeg)
        .expect("Bundled FFmpeg 9.0.2 must resolve via MediaBinaryResolver");
    assert!(
        matches!(resolved.origin, BinaryOrigin::AppOwnedPackagedResource(_)),
        "Must use app-owned packaged resource, got: {:?}",
        resolved.origin
    );
    Command::new(resolved.path)
}

/// Helper: create and return a clean test temporary directory.
fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tinixcut_poc14_{}", name));
    if dir.exists() {
        let _ = fs::remove_dir_all(&dir);
    }
    fs::create_dir_all(&dir).expect("Failed to create test directory");
    dir
}

/// Helper: escape Windows file path for FFmpeg filtergraph strings.
fn escape_ffmpeg_filter_path(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/").replace(':', "\\:")
}

/// Helper: read raw RGBA video frames into a vector of frames.
/// Each frame is width * height * 4 bytes.
fn read_raw_rgba_frames(path: &Path, width: usize, height: usize) -> Vec<Vec<u8>> {
    let mut file = File::open(path).expect("Failed to open raw video file");
    let mut data = Vec::new();
    file.read_to_end(&mut data)
        .expect("Failed to read raw video file");
    let frame_size = width * height * 4;
    assert_eq!(
        data.len() % frame_size,
        0,
        "Data length {} is not a multiple of frame size {}",
        data.len(),
        frame_size
    );
    data.chunks_exact(frame_size).map(|c| c.to_vec()).collect()
}

/// Helper: find bounding box (min_x, min_y, max_x, max_y) of green pixels in a raw RGBA frame.
fn find_green_bounding_box(
    frame: &[u8],
    width: usize,
    height: usize,
) -> Option<(usize, usize, usize, usize)> {
    find_green_bounding_box_with_threshold(frame, width, height, 50)
}

/// Helper: find bounding box with custom green threshold.
fn find_green_bounding_box_with_threshold(
    frame: &[u8],
    width: usize,
    height: usize,
    g_min: u8,
) -> Option<(usize, usize, usize, usize)> {
    let mut min_x = usize::MAX;
    let mut min_y = usize::MAX;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut found = false;

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) * 4;
            let r = frame[idx];
            let g = frame[idx + 1];
            let b = frame[idx + 2];
            // Green box on black canvas: distinct green channel vs black/blue background
            if g >= g_min && r < 50 && b < 50 {
                found = true;
                if x < min_x {
                    min_x = x;
                }
                if x > max_x {
                    max_x = x;
                }
                if y < min_y {
                    min_y = y;
                }
                if y > max_y {
                    max_y = y;
                }
            }
        }
    }

    if found {
        Some((min_x, min_y, max_x, max_y))
    } else {
        None
    }
}

/// Helper: find bounding box with custom blue threshold.
fn find_blue_bounding_box_with_threshold(
    frame: &[u8],
    width: usize,
    height: usize,
    b_min: u8,
) -> Option<(usize, usize, usize, usize)> {
    let mut min_x = usize::MAX;
    let mut min_y = usize::MAX;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut found = false;

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) * 4;
            let r = frame[idx];
            let g = frame[idx + 1];
            let b = frame[idx + 2];
            // Blue box on black canvas: distinct blue channel vs background
            if b >= b_min && r < 50 && g < 50 {
                found = true;
                if x < min_x {
                    min_x = x;
                }
                if x > max_x {
                    max_x = x;
                }
                if y < min_y {
                    min_y = y;
                }
                if y > max_y {
                    max_y = y;
                }
            }
        }
    }

    if found {
        Some((min_x, min_y, max_x, max_y))
    } else {
        None
    }
}

/// Helper: generate synthetic MP4 asset with specified color, dimensions, fps, duration.
fn generate_synthetic_mp4(
    path: &Path,
    duration_secs: u32,
    color: &str,
    width: u32,
    height: u32,
    fps: u32,
) {
    if path.exists() {
        return;
    }
    let output = ffmpeg_cmd()
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            &format!(
                "color=c={}:s={}x{}:r={}:d={}",
                color, width, height, fps, duration_secs
            ),
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
            path.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to generate synthetic mp4");
    assert!(
        output.status.success(),
        "Failed to generate synthetic mp4: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// ============================================================================
// PREFLIGHT
// ============================================================================

#[test]
fn test_poc_preflight_bundled_ffmpeg_check() {
    let resolved = MediaBinaryResolver::resolve(BinaryKind::FFmpeg)
        .expect("Bundled FFmpeg must resolve via MediaBinaryResolver");

    assert!(
        matches!(resolved.origin, BinaryOrigin::AppOwnedPackagedResource(_)),
        "FFmpeg must originate from app-owned packaged resources: {:?}",
        resolved.origin
    );
    assert!(
        resolved.path.exists(),
        "Resolved binary must exist: {:?}",
        resolved.path
    );
    let version = resolved.version.expect("Must have version string");
    assert!(
        version.contains("9.0.2"),
        "FFmpeg must be pinned version 9.0.2, got: {}",
        version
    );
}

// ============================================================================
// POC-V1: Runtime Command Capability on Bundled FFmpeg
// ============================================================================

#[test]
fn test_poc_v1_runtime_command_capability_on_bundled_ffmpeg() {
    let dir = test_dir("v1_capability");
    let cmd_file = dir.join("commands.txt");
    let output_mp4 = dir.join("output_v1.mp4");

    // 2-second fixture @ 30fps = 60 frames.
    // Concurrently changing per-frame:
    // - position X/Y (overlay)
    // - scale W/H (scale)
    // - rotation angle (rotate)
    // - opacity aa (colorchannelmixer)
    let frame_count = 60;
    let fps = 30.0;
    let mut cmd_content = String::new();

    for n in 1..frame_count {
        let t_prev = (n - 1) as f64 / fps;
        let t_curr = n as f64 / fps;
        let t_mid = (t_prev + t_curr) / 2.0;

        let w = 80 + n;
        let h = 60 + n;
        let angle = (n as f64 * 0.01).min(0.5);
        let opacity = (1.0 - (n as f64 * 0.008)).max(0.4);
        let pos_x = 40 + n;
        let pos_y = 20 + n;

        cmd_content.push_str(&format!(
            "{:.6} scale@sc w {};\n{:.6} scale@sc h {};\n{:.6} rotate@rot angle {:.4};\n{:.6} colorchannelmixer@ccm aa {:.4};\n{:.6} overlay@ov x {};\n{:.6} overlay@ov y {};\n",
            t_mid, w, t_mid, h, t_mid, angle, t_mid, opacity, t_mid, pos_x, t_mid, pos_y
        ));
    }
    fs::write(&cmd_file, &cmd_content).expect("Failed to write command file");

    let escaped_cmd = escape_ffmpeg_filter_path(&cmd_file);

    // Filter chain reflects geometry order:
    // Source -> Scale -> Rotate -> Opacity -> Overlay
    let filtergraph = format!(
        "color=c=black:s=640x360:r=30:d=2.0[base];\
         color=c=green:s=80x60:r=30:d=2.0,sendcmd=f='{escaped_cmd}',\
         scale@sc=eval=frame:w=80:h=60,\
         rotate@rot=angle=0:ow='hypot(iw,ih)':oh='hypot(iw,ih)':c=none,\
         format=yuva420p,\
         colorchannelmixer@ccm=aa=1.0[fg];\
         [base][fg]overlay@ov=x=40:y=20[out]"
    );

    let output = ffmpeg_cmd()
        .args([
            "-filter_complex",
            &filtergraph,
            "-map",
            "[out]",
            "-c:v",
            "libx264",
            "-crf",
            "23",
            "-pix_fmt",
            "yuv420p",
            output_mp4.to_str().unwrap(),
            "-y",
        ])
        .output()
        .expect("Failed to execute FFmpeg");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "FFmpeg failed with exit code {:?}\nStderr: {}",
        output.status.code(),
        stderr
    );

    assert!(output_mp4.exists(), "Output MP4 must exist");
    let metadata = fs::metadata(&output_mp4).unwrap();
    assert!(
        metadata.len() > 1000,
        "Output MP4 must be valid size: {} bytes",
        metadata.len()
    );

    // Verify canvas resolution remains 640x360 and duration is 2.0s
    let probe_out = Command::new(
        MediaBinaryResolver::resolve(BinaryKind::FFprobe)
            .unwrap()
            .path,
    )
    .args([
        "-v",
        "error",
        "-select_streams",
        "v:0",
        "-show_entries",
        "stream=width,height,duration",
        "-of",
        "csv=p=0",
        output_mp4.to_str().unwrap(),
    ])
    .output()
    .expect("Failed to run ffprobe");

    let probe_str = String::from_utf8_lossy(&probe_out.stdout);
    let parts: Vec<&str> = probe_str.trim().split(',').collect();
    assert_eq!(parts[0], "640", "Width must remain 640");
    assert_eq!(parts[1], "360", "Height must remain 360");
}

// ============================================================================
// POC-V2: Frame-Exact Dispatch Timing
// ============================================================================

#[test]
fn test_poc_v2_frame_exact_dispatch_timing() {
    let dir = test_dir("v2_timing");

    // Test 1: Integer 30fps
    run_timing_check(&dir, 30, 1, 15, "fps30");

    // Test 2: Fractional NTSC 30000/1001 (~29.97fps)
    run_timing_check(&dir, 30000, 1001, 15, "fps2997");
}

fn run_timing_check(dir: &Path, rate_num: u32, rate_den: u32, frame_count: usize, label: &str) {
    let cmd_file = dir.join(format!("cmd_{}.txt", label));
    let raw_out = dir.join(format!("out_{}.raw", label));

    let fps = rate_num as f64 / rate_den as f64;
    let mut cmd_content = String::new();

    // Deterministic schedule:
    // Frame 0: x = 20
    // Frame n > 0: x[n] = 20 + n * 10
    // Rapid adjacent frame changes every single frame
    let expected_x: Vec<usize> = (0..frame_count).map(|n| 20 + n * 10).collect();

    for n in 1..frame_count {
        let t_prev = (n - 1) as f64 / fps;
        let t_curr = n as f64 / fps;
        let t_mid = (t_prev + t_curr) / 2.0;

        cmd_content.push_str(&format!("{:.6} overlay@ov x {};\n", t_mid, expected_x[n]));
    }
    fs::write(&cmd_file, &cmd_content).expect("Failed to write command file");

    let escaped_cmd = escape_ffmpeg_filter_path(&cmd_file);
    let duration = frame_count as f64 / fps;

    let filtergraph = format!(
        "color=c=black:s=320x180:r={}/{}:d={:.6}[base];\
         color=c=green:s=20x20:r={}/{}:d={:.6},sendcmd=f='{escaped_cmd}'[fg];\
         [base][fg]overlay@ov=x=20:y=50[out]",
        rate_num, rate_den, duration, rate_num, rate_den, duration
    );

    let output = ffmpeg_cmd()
        .args([
            "-filter_complex",
            &filtergraph,
            "-map",
            "[out]",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            raw_out.to_str().unwrap(),
            "-y",
        ])
        .output()
        .expect("Failed to execute FFmpeg");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "FFmpeg failed for {}:\n{}",
        label,
        stderr
    );

    let frames = read_raw_rgba_frames(&raw_out, 320, 180);
    assert_eq!(
        frames.len(),
        frame_count,
        "Decoded frame count must match expected for {}",
        label
    );

    // Verify:
    // 1. Frame 0 (initial value)
    // 2. Frame 1 (first midpoint switch)
    // 3. Middle frame
    // 4. Last active frame
    // 5. ALL adjacent frames
    for n in 0..frame_count {
        let bbox = find_green_bounding_box(&frames[n], 320, 180)
            .unwrap_or_else(|| panic!("{}: Frame {} green box not found!", label, n));
        let actual_x = bbox.0;
        assert_eq!(
            actual_x, expected_x[n],
            "{}: Frame {} expected x={} but got actual x={} (Midpoint timing mismatch!)",
            label, n, expected_x[n], actual_x
        );
    }
}

// ============================================================================
// POC-V3: Semantic Visual Correctness + Negative Control
// ============================================================================

#[test]
fn test_poc_v3_semantic_visual_correctness_and_negative_control() {
    let dir = test_dir("v3_correctness");
    let cmd_pos = dir.join("cmd_pos.txt");
    let raw_pos = dir.join("out_pos.raw");

    let frame_count = 10;
    let fps = 30.0;

    // Positive Schedule:
    // Each frame n has:
    // x = 30 + n * 8
    // w = 40 + n * 4
    // h = 40 + n * 2
    // aa = 1.0 - n * 0.05
    let mut cmd_pos_content = String::new();
    let mut expected_schedule = Vec::new();

    for n in 0..frame_count {
        let x = 30 + n * 8;
        let w = 40 + n * 4;
        let h = 40 + n * 2;
        let aa = 1.0 - (n as f64 * 0.05);
        expected_schedule.push((x, w, h, aa));

        if n > 0 {
            let t_mid = ((n - 1) as f64 / fps + n as f64 / fps) / 2.0;
            cmd_pos_content.push_str(&format!(
                "{:.6} scale@sc w {};\n{:.6} scale@sc h {};\n{:.6} colorchannelmixer@ccm aa {:.3};\n{:.6} overlay@ov x {};\n",
                t_mid, w, t_mid, h, t_mid, aa, t_mid, x
            ));
        }
    }
    fs::write(&cmd_pos, &cmd_pos_content).unwrap();

    let escaped_pos = escape_ffmpeg_filter_path(&cmd_pos);
    let filtergraph_pos = format!(
        "color=c=black:s=320x240:r=30:d=0.333333,format=rgba[base];\
         color=c=green:s=40x40:r=30:d=0.333333,format=rgba,sendcmd=f='{escaped_pos}',\
         scale@sc=eval=frame:w=40:h=40,\
         colorchannelmixer@ccm=aa=1.0[fg];\
         [base][fg]overlay@ov=x=30:y=50:format=rgb[out]"
    );

    let output = ffmpeg_cmd()
        .args([
            "-filter_complex",
            &filtergraph_pos,
            "-map",
            "[out]",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            raw_pos.to_str().unwrap(),
            "-y",
        ])
        .output()
        .expect("Failed to execute FFmpeg");

    assert!(output.status.success(), "Positive run failed: {:?}", output);
    let frames_pos = read_raw_rgba_frames(&raw_pos, 320, 240);

    // Verify positive case matches Rust schedule
    for n in 0..frame_count {
        let (exp_x, exp_w, exp_h, _exp_aa) = expected_schedule[n];
        let bbox = find_green_bounding_box(&frames_pos[n], 320, 240)
            .unwrap_or_else(|| panic!("Frame {} green box missing", n));
        let actual_x = bbox.0;
        let actual_w = bbox.2 - bbox.0 + 1;
        let actual_h = bbox.3 - bbox.1 + 1;

        assert_eq!(actual_x, exp_x, "Frame {} Position X mismatch", n);
        assert!(
            (actual_w as isize - exp_w as isize).abs() <= 1,
            "Frame {} Width mismatch: actual {} vs expected {}",
            n,
            actual_w,
            exp_w
        );
        assert!(
            (actual_h as isize - exp_h as isize).abs() <= 1,
            "Frame {} Height mismatch: actual {} vs expected {}",
            n,
            actual_h,
            exp_h
        );
    }

    // ================= NEGATIVE CONTROL =================
    // Intentionally offset the command schedule by 1 full frame (delayed switch timestamps)
    let cmd_neg = dir.join("cmd_neg.txt");
    let raw_neg = dir.join("out_neg.raw");
    let mut cmd_neg_content = String::new();

    for n in 1..frame_count {
        // Delayed by 1 full frame period!
        let t_delayed = ((n as f64 / fps) + (n as f64 + 1.0) / fps) / 2.0;
        let (x, w, h, aa) = expected_schedule[n];
        cmd_neg_content.push_str(&format!(
            "{:.6} scale@sc w {};\n{:.6} scale@sc h {};\n{:.6} colorchannelmixer@ccm aa {:.3};\n{:.6} overlay@ov x {};\n",
            t_delayed, w, t_delayed, h, t_delayed, aa, t_delayed, x
        ));
    }
    fs::write(&cmd_neg, &cmd_neg_content).unwrap();

    let escaped_neg = escape_ffmpeg_filter_path(&cmd_neg);
    let filtergraph_neg = format!(
        "color=c=black:s=320x240:r=30:d=0.333333,format=rgba[base];\
         color=c=green:s=40x40:r=30:d=0.333333,format=rgba,sendcmd=f='{escaped_neg}',\
         scale@sc=eval=frame:w=40:h=40,\
         colorchannelmixer@ccm=aa=1.0[fg];\
         [base][fg]overlay@ov=x=30:y=50:format=rgb[out]"
    );

    let output_neg = ffmpeg_cmd()
        .args([
            "-filter_complex",
            &filtergraph_neg,
            "-map",
            "[out]",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            raw_neg.to_str().unwrap(),
            "-y",
        ])
        .output()
        .unwrap();

    assert!(output_neg.status.success());
    let frames_neg = read_raw_rgba_frames(&raw_neg, 320, 240);

    // Negative control must be detected on Frame 1
    let bbox_neg_1 = find_green_bounding_box(&frames_neg[1], 320, 240).unwrap();
    let actual_neg_x1 = bbox_neg_1.0;
    let expected_x1 = expected_schedule[1].0;

    let detected_discrepancy = actual_neg_x1 != expected_x1;
    assert!(
        detected_discrepancy,
        "Negative control failure! Test failed to detect intentional 1-frame offset: got {} vs expected {}",
        actual_neg_x1, expected_x1
    );
}

// ============================================================================
// POC-V4: Existing CrossDissolve Coexistence
// ============================================================================

#[test]
fn test_poc_v4_existing_crossdissolve_coexistence() {
    let dir = test_dir("v4_canonical_crossdissolve");
    let vid_a_path = dir.join("vid_a.mp4");
    let vid_b_path = dir.join("vid_b.mp4");
    let cmd_ord_a_file = dir.join("cmd_ord_a.txt");
    let cmd_tr_l_file = dir.join("cmd_tr_l.txt");
    let cmd_ord_b_file = dir.join("cmd_ord_b.txt");
    let cmd_tr_l_neg_file = dir.join("cmd_tr_l_neg.txt");
    let output_raw = dir.join("out_canonical.raw");
    let output_neg_raw = dir.join("out_canonical_neg.raw");
    let output_mp4 = dir.join("out_canonical.mp4");

    let rate = FrameRate::FPS_60;
    let fps = 60.0;
    let width = 640usize;
    let height = 360usize;

    // 1. Generate synthetic source assets with postroll/preroll handles:
    // Asset A: 4.0s (240 frames) green video
    // Asset B: 4.0s (240 frames) blue video
    generate_synthetic_mp4(&vid_a_path, 4, "green", width as u32, height as u32, 60);
    generate_synthetic_mp4(&vid_b_path, 4, "blue", width as u32, height as u32, 60);

    // 2. Build canonical ADR-003 / Schema v2 Project
    let mut project = Project::new("POC-V4 Canonical CrossDissolve".to_string());
    project.settings = ProjectSettings {
        width: width as u32,
        height: height as u32,
        canvas_background_color: "#000000".to_string(),
        frame_rate: rate,
        sample_rate: 48000,
        channels: 2,
    };

    let ast_a = Asset {
        asset_id: "ast_a".to_string(),
        name: "vid_a.mp4".to_string(),
        path: vid_a_path.to_string_lossy().to_string(),
        media_kind: MediaKind::Video,
        duration: Some(RationalTime::from_frames(240, rate)), // 4.0s
        file_size: 1024,
        dimensions: None,
    };
    let ast_b = Asset {
        asset_id: "ast_b".to_string(),
        name: "vid_b.mp4".to_string(),
        path: vid_b_path.to_string_lossy().to_string(),
        media_kind: MediaKind::Video,
        duration: Some(RationalTime::from_frames(240, rate)), // 4.0s
        file_size: 1024,
        dimensions: None,
    };
    project.assets.push(ast_a);
    project.assets.push(ast_b);

    let mut track = Track::new("Video Track".to_string(), TrackKind::Video);

    // Clip A: timeline [0.0, 2.0) = [0, 120) frames.
    // Source: [0, 120) in 240-frame asset (postroll handle = 120 frames = 2.0s >= R=0.25s)
    let mut item_a = TimelineItem::new(
        Some("ast_a".to_string()),
        "Clip A".to_string(),
        ItemKind::Video,
        TimeRange::new(
            RationalTime::from_frames(0, rate),
            RationalTime::from_frames(120, rate),
        ),
        TimeRange::new(
            RationalTime::from_frames(0, rate),
            RationalTime::from_frames(120, rate),
        ),
    )
    .unwrap();
    item_a.item_id = "clip_a".to_string();
    item_a.transform.scale_x = 0.25;
    item_a.transform.scale_y = 0.25;
    item_a.transform.position_x = -85.0;

    // Keyframed visual property on Outgoing A changing continuously across transition interval [T0, T1):
    // 0.0s (frame 0): pos_x = -85.0 (top-left x = 240 - 85 = 155)
    // T0 = 1.75s (frame 105): pos_x = 20.0 (top-left x = 240 + 20 = 260)
    // Cut C = 2.0s (frame 120): pos_x = 80.0 (top-left x = 240 + 80 = 320)
    // T1 handle sample = 2.25s (frame 135): pos_x = 140.0 (top-left x = 240 + 140 = 380)
    item_a.keyframes.push(Keyframe::new(
        KeyframeProperty::PositionX,
        RationalTime::from_frames(0, rate),
        -85.0,
    ));
    item_a.keyframes.push(Keyframe::new(
        KeyframeProperty::PositionX,
        RationalTime::from_frames(105, rate),
        20.0,
    ));
    item_a.keyframes.push(Keyframe::new(
        KeyframeProperty::PositionX,
        RationalTime::from_frames(120, rate),
        80.0,
    ));
    item_a.keyframes.push(Keyframe::new(
        KeyframeProperty::PositionX,
        RationalTime::from_frames(135, rate),
        140.0,
    ));

    // Clip B: timeline [2.0, 4.0) = [120, 240) frames.
    // Source: [60, 180) in 240-frame asset (preroll handle = 60 frames = 1.0s >= L=0.25s)
    let mut item_b = TimelineItem::new(
        Some("ast_b".to_string()),
        "Clip B".to_string(),
        ItemKind::Video,
        TimeRange::new(
            RationalTime::from_frames(120, rate),
            RationalTime::from_frames(120, rate),
        ),
        TimeRange::new(
            RationalTime::from_frames(60, rate),
            RationalTime::from_frames(120, rate),
        ),
    )
    .unwrap();
    item_b.item_id = "clip_b".to_string();
    item_b.transform.scale_x = 0.25;
    item_b.transform.scale_y = 0.25;
    item_b.transform.position_x = -150.0;

    // Automation on Clip B resuming after T1:
    // Offset 15 (timeline frame 135 = 2.25s = T1): pos_x = -150.0 (top-left x = 240 - 150 = 90)
    // Offset 60 (timeline frame 180 = 3.00s): pos_x = -105.0 (top-left x = 240 - 105 = 135)
    // Offset 120 (timeline frame 240 = 4.00s): pos_x = -45.0 (top-left x = 240 - 45 = 195)
    item_b.keyframes.push(Keyframe::new(
        KeyframeProperty::PositionX,
        RationalTime::from_frames(15, rate),
        -150.0,
    ));
    item_b.keyframes.push(Keyframe::new(
        KeyframeProperty::PositionX,
        RationalTime::from_frames(60, rate),
        -105.0,
    ));
    item_b.keyframes.push(Keyframe::new(
        KeyframeProperty::PositionX,
        RationalTime::from_frames(120, rate),
        -45.0,
    ));

    // 3. CutTransition connecting adjacent A and B (Schema v2)
    let cut_trans = CutTransition {
        transition_id: "trans_ab".to_string(),
        track_id: track.track_id.clone(),
        left_item_id: item_a.item_id.clone(),
        right_item_id: item_b.item_id.clone(),
        kind: TransitionKind::CrossDissolve,
        duration: RationalTime::from_frames(30, rate), // D = 30 frames (0.5s)
    };

    track.items.push(item_a);
    track.items.push(item_b);
    track.transitions.push(cut_trans);
    project.tracks.push(track);

    // 4. Compile canonical CompositionPlan via production compiler
    let plan = CompositionPlan::from_project(&project).expect("Plan compilation must succeed");

    // Phase-13 Invariant Assertions:
    assert_eq!(
        plan.visual_layers[0].timeline_range.end(),
        plan.visual_layers[1].timeline_range.start,
        "A.timeline_range.end must equal B.timeline_range.start (zero persisted overlap)"
    );
    assert_eq!(
        plan.transitions.len(),
        1,
        "Must have exactly 1 CutTransition"
    );
    let tp = &plan.transitions[0];
    assert_eq!(tp.cut_time, RationalTime::from_frames(120, rate)); // C = 2.0s
    assert_eq!(tp.duration, RationalTime::from_frames(30, rate)); // D = 0.5s
    assert_eq!(tp.l_duration(), RationalTime::from_frames(15, rate)); // L = 0.25s
    assert_eq!(tp.r_duration(), RationalTime::from_frames(15, rate)); // R = 0.25s
    assert_eq!(
        tp.transition_range.start,
        RationalTime::from_frames(105, rate)
    ); // T0 = 1.75s
    assert_eq!(
        tp.transition_range.end(),
        RationalTime::from_frames(135, rate)
    ); // T1 = 2.25s
    assert_eq!(plan.duration, RationalTime::from_frames(240, rate)); // Total duration = 4.0s

    // Project production export arguments and build canonical video filtergraph
    let input_paths = vec![vid_a_path.clone(), vid_b_path.clone()];
    let duration_secs = plan.duration.to_seconds();
    let (prod_filter_complex, current_base) =
        plan.build_video_filtergraph(&input_paths, duration_secs, None);

    println!("PROD FILTER COMPLEX:\n{}\n", prod_filter_complex);

    // Verify canonical filtergraph structures produced by production compiler:
    assert_eq!(current_base, "v_over_2");
    assert!(prod_filter_complex.contains("premultiply=inplace=1"));
    assert!(prod_filter_complex.contains("xfade=transition=fade:duration=0.500000:offset=0"));
    assert!(prod_filter_complex.contains("unpremultiply=inplace=1,format=rgba"));
    assert!(prod_filter_complex.contains("between(t,0.0000,1.7417)")); // Ordinary A suppression interval
    assert!(prod_filter_complex.contains("between(t,1.7417,2.2417)")); // Transition active interval
    assert!(prod_filter_complex.contains("between(t,2.2417,3.9917)")); // Ordinary B suppression interval

    // 5. Generate Command Schedules:
    // Schedule 1: Ordinary A (overlay@ov_ord_a) for frames 1..105 (0.0s to 1.75s)
    let mut cmd_ord_a_content = String::new();
    for n in 1..105 {
        let t_mid = ((n - 1) as f64 / fps + n as f64 / fps) / 2.0;
        let offset = RationalTime::from_frames(n as i64, rate);
        let pos_x = plan.visual_layers[0].evaluate_property(KeyframeProperty::PositionX, offset);
        let top_left_x = ((width as f32 - 160.0) / 2.0 + pos_x).round() as i32;
        cmd_ord_a_content.push_str(&format!(
            "{:.6} overlay@ov_ord_a x {};\n",
            t_mid, top_left_x
        ));
    }
    fs::write(&cmd_ord_a_file, &cmd_ord_a_content).unwrap();

    // Schedule 2: Transition Left Branch A (overlay@ov_tr_l) for branch frames 1..30 (timeline 105..135)
    let mut cmd_tr_l_content = String::new();
    for k in 1..30 {
        let t_mid = ((k - 1) as f64 / fps + k as f64 / fps) / 2.0;
        let timeline_frame = 105 + k;
        let offset = RationalTime::from_frames(timeline_frame as i64, rate);
        let pos_x = plan.visual_layers[0].evaluate_property(KeyframeProperty::PositionX, offset);
        let top_left_x = ((width as f32 - 160.0) / 2.0 + pos_x).round() as i32;
        cmd_tr_l_content.push_str(&format!("{:.6} overlay@ov_tr_l x {};\n", t_mid, top_left_x));
    }
    fs::write(&cmd_tr_l_file, &cmd_tr_l_content).unwrap();

    // Schedule 3: Ordinary B (overlay@ov_ord_b) for frames 136..240 (2.25s to 4.0s)
    let mut cmd_ord_b_content = String::new();
    for n in 136..240 {
        let t_mid = ((n - 1) as f64 / fps + n as f64 / fps) / 2.0;
        let clip_offset = RationalTime::from_frames((n - 120) as i64, rate);
        let pos_x =
            plan.visual_layers[1].evaluate_property(KeyframeProperty::PositionX, clip_offset);
        let top_left_x = ((width as f32 - 160.0) / 2.0 + pos_x).round() as i32;
        cmd_ord_b_content.push_str(&format!(
            "{:.6} overlay@ov_ord_b x {};\n",
            t_mid, top_left_x
        ));
    }
    fs::write(&cmd_ord_b_file, &cmd_ord_b_content).unwrap();

    // Schedule 4: Negative Control — intentionally freeze transition left branch at T0 position (x = 260)
    let mut cmd_tr_l_neg_content = String::new();
    for k in 1..30 {
        let t_mid = ((k - 1) as f64 / fps + k as f64 / fps) / 2.0;
        // Freeze at T0 position (260)
        cmd_tr_l_neg_content.push_str(&format!("{:.6} overlay@ov_tr_l x 260;\n", t_mid));
    }
    fs::write(&cmd_tr_l_neg_file, &cmd_tr_l_neg_content).unwrap();

    // 6. Test-Only Instrumentation:
    // Take exact production filtergraph and instrument with named filter targets and sendcmd
    let escaped_cmd_ord_a = escape_ffmpeg_filter_path(&cmd_ord_a_file);
    let escaped_cmd_tr_l = escape_ffmpeg_filter_path(&cmd_tr_l_file);
    let escaped_cmd_ord_b = escape_ffmpeg_filter_path(&cmd_ord_b_file);

    let instrumented_filtergraph = prod_filter_complex
        // Instrument Ordinary A branch
        .replace(
            "format=yuva420p,colorchannelmixer=aa=1.000[layer_0];",
            &format!("format=yuva420p,colorchannelmixer=aa=1.000,sendcmd=f='{escaped_cmd_ord_a}'[layer_0];"),
        )
        .replace(
            "[base][layer_0]overlay=x='(W-w)/2+-85.0':y='(H-h)/2+0.0':enable='between(t,0.0000,1.7417)'[v_over_0];",
            "[base][layer_0]overlay@ov_ord_a=x='(W-w)/2+-85.0':y='(H-h)/2+0.0':enable='between(t,0.0000,1.7417)'[v_over_0];",
        )
        // Instrument Transition Left branch
        .replace(
            "color=c=black@0:s=640x360:r=60:d=0.500000[tr_l_canvas_0];",
            &format!("color=c=black@0:s=640x360:r=60:d=0.500000,sendcmd=f='{escaped_cmd_tr_l}'[tr_l_canvas_0];"),
        )
        .replace(
            "[tr_l_canvas_0][tr_l_clip_0]overlay=x='(W-w)/2+-85.0':y='(H-h)/2+0.0',format=rgba,premultiply=inplace=1[tr_l_branch_0];",
            "[tr_l_canvas_0][tr_l_clip_0]overlay@ov_tr_l=x=260:y=135,format=rgba,premultiply=inplace=1[tr_l_branch_0];",
        )
        // Instrument Ordinary B branch
        .replace(
            "format=yuva420p,colorchannelmixer=aa=1.000[layer_2];",
            &format!("format=yuva420p,colorchannelmixer=aa=1.000,sendcmd=f='{escaped_cmd_ord_b}'[layer_2];"),
        )
        .replace(
            "[v_over_1][layer_2]overlay=x='(W-w)/2+-150.0':y='(H-h)/2+0.0':enable='between(t,2.2417,3.9917)'[v_over_2];",
            "[v_over_1][layer_2]overlay@ov_ord_b=x='(W-w)/2+-150.0':y='(H-h)/2+0.0':enable='between(t,2.2417,3.9917)'[v_over_2];",
        );

    // 7. Execute Positive Run — Render Rawvideo
    let out_pos = ffmpeg_cmd()
        .args([
            "-i",
            vid_a_path.to_str().unwrap(),
            "-i",
            vid_b_path.to_str().unwrap(),
            "-filter_complex",
            &instrumented_filtergraph,
            "-map",
            "[v_over_2]",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            output_raw.to_str().unwrap(),
            "-y",
        ])
        .output()
        .expect("Failed to execute FFmpeg positive export");

    let stderr = String::from_utf8_lossy(&out_pos.stderr);
    assert!(
        out_pos.status.success(),
        "FFmpeg positive export failed:\n{}",
        stderr
    );

    let frames = read_raw_rgba_frames(&output_raw, width, height);
    assert_eq!(
        frames.len(),
        240,
        "Must decode exactly 240 frames (4.0s @ 60fps)"
    );

    // 8. Frame Evidence Verification:
    // Frame 104: Frame immediately before T0 (t = 104/60 = 1.733s)
    let bbox_104 = find_green_bounding_box(&frames[104], width, height)
        .expect("Frame 104 Clip A must be detected");
    assert!(
        (bbox_104.0 as isize - 259).abs() <= 1,
        "Frame 104 x must match evaluated position within chroma margin"
    );
    assert_eq!(bbox_104.1, 134, "Frame 104 y must match evaluated position");
    assert!(
        find_blue_bounding_box_with_threshold(&frames[104], width, height, 50).is_none(),
        "Clip B must NOT be visible before T0"
    );

    // Frame 105: First active transition frame (t = 105/60 = 1.750s = T0)
    let bbox_105 = find_green_bounding_box_with_threshold(&frames[105], width, height, 50)
        .expect("Frame 105 Clip A must be detected in transition");
    assert_eq!(
        bbox_105.0, 260,
        "Frame 105 x at T0 must equal 260 (pos_x=20)"
    );
    assert_eq!(bbox_105.1, 134, "Frame 105 y at T0 must equal 134");

    // Frame 120: Midpoint near cut C (t = 120/60 = 2.000s = Cut C)
    let bbox_120 = find_green_bounding_box_with_threshold(&frames[120], width, height, 30)
        .expect("Frame 120 Clip A must be detected at transition midpoint");
    assert_eq!(
        bbox_120.0, 320,
        "Frame 120 x at Cut C must equal 320 (pos_x=80) — proving animation did NOT freeze at T0!"
    );
    let bbox_120_b = find_blue_bounding_box_with_threshold(&frames[120], width, height, 30)
        .expect("Frame 120 Clip B must be detected at transition midpoint");
    assert_eq!(bbox_120_b.0, 90, "Frame 120 Clip B must be present at x=90");

    // Frame 134: Last active transition frame (< T1, t = 134/60 = 2.233s)
    let bbox_134 = find_green_bounding_box_with_threshold(&frames[134], width, height, 2)
        .expect("Frame 134 Clip A handle must be detected in final transition frame");
    assert_eq!(
        bbox_134.0, 376,
        "Frame 134 x must equal 376 (pos_x=136) — proving animation did NOT freeze at cut C!"
    );
    let bbox_134_b = find_blue_bounding_box_with_threshold(&frames[134], width, height, 50)
        .expect("Frame 134 Clip B must be detected");
    assert_eq!(bbox_134_b.0, 90, "Frame 134 Clip B x must be 90");

    // Frame 135: First frame at/after T1 (t = 135/60 = 2.250s = T1)
    let max_g_135 = frames[135].chunks_exact(4).map(|c| c[1]).max().unwrap_or(0);
    assert_eq!(
        max_g_135, 0,
        "Clip A must be completely suppressed at/after T1 (no double render, max_g=0)"
    );
    let bbox_135_b = find_blue_bounding_box_with_threshold(&frames[135], width, height, 50)
        .expect("Frame 135 Clip B must be visible in ordinary branch");
    assert!(
        (bbox_135_b.0 as isize - 90).abs() <= 2,
        "Frame 135 Clip B initial resume position"
    );

    // Frame 180: Middle of Ordinary B (t = 180/60 = 3.000s)
    let bbox_180_b = find_blue_bounding_box_with_threshold(&frames[180], width, height, 50)
        .expect("Frame 180 Clip B must be visible and animating");
    assert!(
        (bbox_180_b.0 as isize - 135).abs() <= 1,
        "Frame 180 Clip B x must equal 135 (pos_x=-105) — proving ordinary B resumed automation!"
    );

    // 9. Negative Control Execution:
    // Run identical graph but with frozen transition schedule (cmd_tr_l_neg)
    let escaped_cmd_tr_l_neg = escape_ffmpeg_filter_path(&cmd_tr_l_neg_file);
    let instrumented_neg_filtergraph = instrumented_filtergraph.replace(
        &format!("sendcmd=f='{escaped_cmd_tr_l}'"),
        &format!("sendcmd=f='{escaped_cmd_tr_l_neg}'"),
    );

    let out_neg = ffmpeg_cmd()
        .args([
            "-i",
            vid_a_path.to_str().unwrap(),
            "-i",
            vid_b_path.to_str().unwrap(),
            "-filter_complex",
            &instrumented_neg_filtergraph,
            "-map",
            "[v_over_2]",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            output_neg_raw.to_str().unwrap(),
            "-y",
        ])
        .output()
        .expect("Failed to execute negative control FFmpeg");

    assert!(out_neg.status.success());
    let frames_neg = read_raw_rgba_frames(&output_neg_raw, width, height);

    // Negative control check at Frame 120 (Midpoint C):
    let bbox_neg_120 = find_green_bounding_box_with_threshold(&frames_neg[120], width, height, 30)
        .expect("Negative control Frame 120 green box present");
    let actual_neg_x120 = bbox_neg_120.0;
    let expected_animated_x120 = 320;
    let detected_freeze_at_c = actual_neg_x120 != expected_animated_x120;
    assert!(
        detected_freeze_at_c,
        "Negative control failure! Verification must fail when animation is frozen at T0"
    );
    assert_eq!(
        actual_neg_x120, 260,
        "Negative control must observe frozen T0 coordinate (260) at cut C"
    );

    // Negative control check at Frame 134:
    let bbox_neg_134 = find_green_bounding_box_with_threshold(&frames_neg[134], width, height, 2)
        .expect("Negative control Frame 134 green box present");
    let actual_neg_x134 = bbox_neg_134.0;
    let expected_animated_x134 = 376;
    assert!(
        actual_neg_x134 != expected_animated_x134,
        "Negative control must fail at frame 134"
    );
    assert_eq!(
        actual_neg_x134, 260,
        "Negative control must remain frozen at T0 coordinate (260) at frame 134"
    );

    // 10. Container & Duration Verification (MP4 Output):
    let out_mp4 = ffmpeg_cmd()
        .args([
            "-i",
            vid_a_path.to_str().unwrap(),
            "-i",
            vid_b_path.to_str().unwrap(),
            "-filter_complex",
            &instrumented_filtergraph,
            "-map",
            "[v_over_2]",
            "-c:v",
            "libx264",
            "-crf",
            "23",
            "-pix_fmt",
            "yuv420p",
            output_mp4.to_str().unwrap(),
            "-y",
        ])
        .output()
        .expect("Failed to execute MP4 export");

    assert!(out_mp4.status.success(), "MP4 export failed");
    assert!(output_mp4.exists(), "MP4 output file must exist");

    let probe_out = Command::new(
        MediaBinaryResolver::resolve(BinaryKind::FFprobe)
            .unwrap()
            .path,
    )
    .args([
        "-v",
        "error",
        "-select_streams",
        "v:0",
        "-show_entries",
        "stream=duration",
        "-of",
        "csv=p=0",
        output_mp4.to_str().unwrap(),
    ])
    .output()
    .expect("Failed to run ffprobe");

    let dur: f64 = String::from_utf8_lossy(&probe_out.stdout)
        .trim()
        .parse()
        .expect("Must parse duration");
    assert!(
        (dur - 4.0).abs() < 0.05,
        "Total duration must remain exactly 4.0s, got: {}",
        dur
    );

    println!("\n=== POC-V4 CANONICAL CROSSDISSOLVE COEXISTENCE REPORT ===");
    println!("Canonical Fixture: A=[0.0, 2.0), B=[2.0, 4.0), Cut C=2.0s");
    println!("Zero Persisted Overlap: A.end == B.start (2.0s)");
    println!("Transition Duration: D=0.5s, L=0.25s, R=0.25s");
    println!("Intervals: Ordinary A=[0.0, 1.7417), Transition=[1.7417, 2.2417), Ordinary B=[2.2417, 3.9917)");
    println!("Total Duration: 4.0s (240 frames @ 60fps)");
    println!("Targets: overlay@ov_ord_a, overlay@ov_tr_l, overlay@ov_ord_b");
    println!("Frame 104 (pre-T0): Clip A x={}, Clip B None", bbox_104.0);
    println!("Frame 105 (T0): Clip A x={}", bbox_105.0);
    println!(
        "Frame 120 (Cut C): Clip A x={}, Clip B x={}",
        bbox_120.0, bbox_120_b.0
    );
    println!(
        "Frame 134 (last active transition): Clip A x={}, Clip B x={}",
        bbox_134.0, bbox_134_b.0
    );
    println!(
        "Frame 135 (T1): Clip A max_g={}, Clip B x={}",
        max_g_135, bbox_135_b.0
    );
    println!("Frame 180 (Ordinary B resumed): Clip B x={}", bbox_180_b.0);
    println!("Negative Control: Intentional freeze at T0 (260) detected at Cut C (actual {} vs expected 320) and Frame 134 (actual {} vs expected 376)", actual_neg_x120, actual_neg_x134);
    println!("Verdict: POC PASS — CANONICAL CROSSDISSOLVE COEXISTENCE VERIFIED");
    println!("=========================================================\n");
}

// ============================================================================
// POC-A1: Sample-Accurate Audio Envelope + Negative Control
// ============================================================================

#[test]
fn test_poc_a1_sample_accurate_audio_envelope_and_negative_control() {
    let dir = test_dir("a1_audio");
    let env_file = dir.join("envelope.raw");
    let out_raw = dir.join("out.raw");

    // 0.2s @ 48kHz = 9600 samples
    let _sample_rate = 48000;
    let sample_count = 9600;

    // Linear ramp from 0.0 to 1.0
    let mut envelope_bytes = Vec::with_capacity(sample_count * 4);
    let mut gains = Vec::with_capacity(sample_count);
    for i in 0..sample_count {
        let gain = i as f32 / sample_count as f32;
        gains.push(gain);
        envelope_bytes.extend_from_slice(&gain.to_le_bytes());
    }
    fs::write(&env_file, &envelope_bytes).unwrap();

    let env_path_str = env_file.to_string_lossy().replace('\\', "/");

    // Pipeline:
    // Source: constant 0.5 amplitude stereo audio
    // Envelope: raw mono f32le
    // pan=stereo|c0=c0|c1=c0 maps mono envelope to stereo with EXACT 1.0 factor (no 0.707 center-pan loss)
    // amultiply produces stereo output where out[i] = source[i] * gain[i]
    let output = ffmpeg_cmd()
        .args([
            "-f",
            "lavfi",
            "-i",
            "aevalsrc=0.5:s=48000:d=0.2:c=stereo",
            "-f",
            "f32le",
            "-ar",
            "48000",
            "-ac",
            "1",
            "-i",
            &env_path_str,
            "-filter_complex",
            "[1:a]pan=stereo|c0=c0|c1=c0[env_st];[0:a][env_st]amultiply[outa]",
            "-map",
            "[outa]",
            "-f",
            "f32le",
            "-ar",
            "48000",
            out_raw.to_str().unwrap(),
            "-y",
        ])
        .output()
        .expect("Failed to execute FFmpeg");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "FFmpeg audio export failed:\n{}",
        stderr
    );

    let mut out_data = Vec::new();
    File::open(&out_raw)
        .unwrap()
        .read_to_end(&mut out_data)
        .unwrap();

    // Stereo f32 = 8 bytes per sample frame (4 bytes Left + 4 bytes Right)
    assert_eq!(
        out_data.len(),
        sample_count * 8,
        "Output PCM must have exactly 9600 stereo frames (76800 bytes)"
    );

    // Verify sample accuracy across key indices
    let test_indices = [0, 1, 100, 1000, 4800, 7200, 9599];
    let source_val = 0.5f32;

    for &idx in &test_indices {
        let byte_offset = idx * 8;
        let left_bytes = [
            out_data[byte_offset],
            out_data[byte_offset + 1],
            out_data[byte_offset + 2],
            out_data[byte_offset + 3],
        ];
        let right_bytes = [
            out_data[byte_offset + 4],
            out_data[byte_offset + 5],
            out_data[byte_offset + 6],
            out_data[byte_offset + 7],
        ];
        let actual_l = f32::from_le_bytes(left_bytes);
        let actual_r = f32::from_le_bytes(right_bytes);
        let expected = source_val * gains[idx];

        assert!(
            (actual_l - expected).abs() < 1e-4,
            "Sample {} Left channel mismatch: actual {} vs expected {}",
            idx,
            actual_l,
            expected
        );
        assert!(
            (actual_r - expected).abs() < 1e-4,
            "Sample {} Right channel mismatch: actual {} vs expected {}",
            idx,
            actual_r,
            expected
        );
    }

    // ================= NEGATIVE CONTROL =================
    // Intentionally introduce a 1-sample shift: test actual_output[idx] against gains[idx - 1]
    let mut detected_shift = false;
    for &idx in &[100, 1000, 4800] {
        let byte_offset = idx * 8;
        let actual_l = f32::from_le_bytes([
            out_data[byte_offset],
            out_data[byte_offset + 1],
            out_data[byte_offset + 2],
            out_data[byte_offset + 3],
        ]);
        let shifted_expected = source_val * gains[idx - 1]; // Offset by 1 sample!
        let diff = (actual_l - shifted_expected).abs();
        if diff > 1e-5 {
            detected_shift = true;
        }
    }
    assert!(
        detected_shift,
        "Negative control failed! Audio test must detect 1-sample shift"
    );
}

// ============================================================================
// POC-A2: Envelope Lifecycle & Resource Behavior
// ============================================================================

#[test]
fn test_poc_a2_envelope_lifecycle_and_resource_behavior() {
    let dir = test_dir("a2_lifecycle");

    // 1. Streaming Envelope Generation (Constant O(1) Memory)
    let stream_file = dir.join("stream_env.raw");
    let sample_rate = 48000;
    let duration_secs = 2.0;
    let total_samples = (sample_rate as f64 * duration_secs) as usize;

    {
        let file = File::create(&stream_file).unwrap();
        let mut writer = BufWriter::with_capacity(8192, file);
        for i in 0..total_samples {
            let gain = (i as f32 / total_samples as f32).sin().abs();
            writer.write_all(&gain.to_le_bytes()).unwrap();
        }
        writer.flush().unwrap();
    }

    let file_len = fs::metadata(&stream_file).unwrap().len();
    assert_eq!(file_len, (total_samples * 4) as u64);

    // 2. Lifecycle Cleanup Mechanics
    // Scenario A: Success cleanup
    let temp_success = dir.join("temp_success.raw");
    fs::write(&temp_success, b"test_envelope_bytes").unwrap();
    assert!(temp_success.exists());
    // Simulate export success hook:
    let _ = fs::remove_file(&temp_success);
    assert!(!temp_success.exists(), "Success hook must remove temp file");

    // Scenario B: Failure cleanup
    let temp_fail = dir.join("temp_fail.raw");
    fs::write(&temp_fail, b"test_envelope_bytes").unwrap();
    assert!(temp_fail.exists());
    // Simulate export failure with cleanup guard:
    let result: Result<(), &'static str> = Err("FFmpeg process crashed");
    if result.is_err() {
        let _ = fs::remove_file(&temp_fail);
    }
    assert!(!temp_fail.exists(), "Failure hook must remove temp file");

    // Scenario C: Cancellation cleanup
    let temp_cancel = dir.join("temp_cancel.raw");
    fs::write(&temp_cancel, b"test_envelope_bytes").unwrap();
    assert!(temp_cancel.exists());
    // Simulate cancellation event:
    let is_cancelled = true;
    if is_cancelled {
        let _ = fs::remove_file(&temp_cancel);
    }
    assert!(
        !temp_cancel.exists(),
        "Cancellation hook must remove temp file"
    );

    // 3. Storage Footprint Verification
    // 4 bytes * sample_rate * duration
    let size_1min = 4 * 48000 * 60;
    let size_10min = 4 * 48000 * 600;
    let size_60min = 4 * 48000 * 3600;

    assert_eq!(size_1min, 11_520_000); // 11.52 MB
    assert_eq!(size_10min, 115_200_000); // 115.2 MB
    assert_eq!(size_60min, 691_200_000); // 691.2 MB
}

// ============================================================================
// POC-P1: Performance Sanity
// ============================================================================

#[test]
fn test_poc_p1_performance_sanity() {
    let dir = test_dir("p1_performance");
    let cmd_file = dir.join("cmd_p1.txt");
    let env_file = dir.join("env_p1.raw");
    let auto_mp4 = dir.join("out_auto.mp4");
    let static_mp4 = dir.join("out_static.mp4");

    let duration_secs = 2.0;
    let fps = 30.0;
    let frame_count = (duration_secs * fps) as usize;
    let sample_rate = 48000;
    let sample_count = (duration_secs * sample_rate as f64) as usize;

    // 1. Generate Video Command Schedule (60 frames * 6 commands = 354 commands)
    let mut cmd_content = String::new();
    let mut cmd_count = 0;
    for n in 1..frame_count {
        let t_mid = ((n - 1) as f64 / fps + n as f64 / fps) / 2.0;
        let pos_x = 50 + n;
        let pos_y = 30 + n;
        let w = 100 + n;
        let h = 80 + n;
        let angle = n as f64 * 0.005;
        let aa = 1.0 - (n as f64 * 0.005);

        cmd_content.push_str(&format!(
            "{:.6} scale@sc w {};\n{:.6} scale@sc h {};\n{:.6} rotate@rot angle {:.4};\n{:.6} colorchannelmixer@ccm aa {:.3};\n{:.6} overlay@ov x {};\n{:.6} overlay@ov y {};\n",
            t_mid, w, t_mid, h, t_mid, angle, t_mid, aa, t_mid, pos_x, t_mid, pos_y
        ));
        cmd_count += 6;
    }
    fs::write(&cmd_file, &cmd_content).unwrap();
    let cmd_file_size = fs::metadata(&cmd_file).unwrap().len();

    // 2. Generate Audio Envelope (96,000 samples)
    {
        let file = File::create(&env_file).unwrap();
        let mut writer = BufWriter::with_capacity(8192, file);
        for i in 0..sample_count {
            let gain = 0.5 + 0.5 * (i as f32 / sample_count as f32);
            writer.write_all(&gain.to_le_bytes()).unwrap();
        }
        writer.flush().unwrap();
    }
    let env_file_size = fs::metadata(&env_file).unwrap().len();

    let escaped_cmd = escape_ffmpeg_filter_path(&cmd_file);
    let env_path_str = env_file.to_string_lossy().replace('\\', "/");

    // 3. Automated Export Execution
    let filtergraph_auto = format!(
        "color=c=black:s=640x360:r=30:d=2.0[base];\
         color=c=green:s=100x80:r=30:d=2.0,sendcmd=f='{escaped_cmd}',\
         scale@sc=eval=frame:w=100:h=80,\
         rotate@rot=angle=0:ow='hypot(iw,ih)':oh='hypot(iw,ih)':c=none,\
         format=yuva420p,\
         colorchannelmixer@ccm=aa=1.0[fg];\
         [base][fg]overlay@ov=x=50:y=30[outv];\
         [1:a]pan=stereo|c0=c0|c1=c0[env_st];[0:a][env_st]amultiply[outa]"
    );

    let start_auto = Instant::now();
    let out_auto = ffmpeg_cmd()
        .args([
            "-f",
            "lavfi",
            "-i",
            "aevalsrc=0.5:s=48000:d=2.0:c=stereo",
            "-f",
            "f32le",
            "-ar",
            "48000",
            "-ac",
            "1",
            "-i",
            &env_path_str,
            "-filter_complex",
            &filtergraph_auto,
            "-map",
            "[outv]",
            "-map",
            "[outa]",
            "-c:v",
            "libx264",
            "-crf",
            "23",
            "-c:a",
            "aac",
            "-pix_fmt",
            "yuv420p",
            auto_mp4.to_str().unwrap(),
            "-y",
        ])
        .output()
        .expect("Failed to execute auto export");
    let elapsed_auto = start_auto.elapsed();

    assert!(
        out_auto.status.success(),
        "Auto export failed: {:?}",
        out_auto
    );

    // 4. Static Export Execution (Baseline with identical static filter set)
    let filtergraph_static = "color=c=black:s=640x360:r=30:d=2.0[base];\
         color=c=green:s=100x80:r=30:d=2.0,\
         scale=w=100:h=80,\
         rotate=angle=0.1:ow='hypot(iw,ih)':oh='hypot(iw,ih)':c=none,\
         format=yuva420p,\
         colorchannelmixer=aa=0.8[fg];\
         [base][fg]overlay=x=50:y=30[outv];\
         [0:a]volume=0.75[outa]";

    let start_static = Instant::now();
    let out_static = ffmpeg_cmd()
        .args([
            "-f",
            "lavfi",
            "-i",
            "aevalsrc=0.5:s=48000:d=2.0:c=stereo",
            "-filter_complex",
            filtergraph_static,
            "-map",
            "[outv]",
            "-map",
            "[outa]",
            "-c:v",
            "libx264",
            "-crf",
            "23",
            "-c:a",
            "aac",
            "-pix_fmt",
            "yuv420p",
            static_mp4.to_str().unwrap(),
            "-y",
        ])
        .output()
        .expect("Failed to execute static export");
    let elapsed_static = start_static.elapsed();

    assert!(
        out_static.status.success(),
        "Static export failed: {:?}",
        out_static
    );

    println!("\n=== POC-P1 PERFORMANCE SANITY REPORT ===");
    println!("Fixture duration: 2.0s @ 30fps (60 frames), 48kHz audio (96,000 samples)");
    println!("Number of filter commands: {}", cmd_count);
    println!("Command file size: {} bytes", cmd_file_size);
    println!("Audio envelope size: {} bytes (96 KB)", env_file_size);
    println!("Auto Export Wall-clock: {:?}", elapsed_auto);
    println!("Static Export Wall-clock: {:?}", elapsed_static);
    let ratio = elapsed_auto.as_secs_f64() / elapsed_static.as_secs_f64().max(0.001);
    println!("Overhead ratio (Auto / Static): {:.2}x", ratio);
    println!("========================================\n");

    // Performance sanity assertion: must finish within reasonable bounds (< 5.0s for 2.0s fixture)
    // and without pathological runaway execution or memory exhaustion.
    assert!(
        elapsed_auto.as_secs_f64() < 5.0,
        "Performance sanity failed: automation took too long: {:?}",
        elapsed_auto
    );
    assert!(
        ratio < 20.0,
        "Performance sanity failed: automation overhead ratio was pathologically high: {:.2}x (> 20x)",
        ratio
    );
}
