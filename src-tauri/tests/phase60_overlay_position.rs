//! G52 Step 2: a video layer at an odd pixel position must land exactly there in the export.
//!
//! `overlay` on 4:2:0 frames floors x/y to even pixels, which put every layer at an odd position
//! one pixel up/left of where the preview draws it. The graph composites in 4:4:4 instead.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    Asset, AssetDimensions, ItemKind, Keyframe, KeyframeProperty, MediaKind, Project, TimelineItem,
    Track, TrackKind,
};
use tinixcut_lib::modules::export::{ExportEngine, ExportProfile};
use tinixcut_lib::modules::media::resolver::{BinaryKind, MediaBinaryResolver};

const CANVAS: (u32, u32) = (256, 144);
const FRAMES: i64 = 30;

fn ffmpeg_path() -> PathBuf {
    MediaBinaryResolver::resolve(BinaryKind::FFmpeg)
        .expect("bundled FFmpeg")
        .path
}

fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tinixcut_p60o_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// 128x72 gray source with a white vertical bar at x 40..44 and a horizontal bar at y 20..24.
fn bar_source(path: &Path) {
    bar_source_frames(path, 1);
}

fn bar_source_frames(path: &Path, seconds: u32) {
    let status = Command::new(ffmpeg_path())
        .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
        .arg(format!("color=c=gray:s=128x72:r=30:d={seconds},drawbox=x=40:y=0:w=4:h=72:color=white:t=fill,drawbox=x=0:y=20:w=128:h=4:color=white:t=fill"))
        .args(["-c:v", "libx264", "-crf", "10", "-pix_fmt", "yuv444p"])
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success());
}

fn project(source: &Path, pos: (f32, f32), animated: bool) -> Project {
    let rate = FrameRate::FPS_30;
    let mut project = Project::new("Overlay position".to_string());
    project.settings.width = CANVAS.0;
    project.settings.height = CANVAS.1;
    project.settings.frame_rate = rate;
    project.assets.push(Asset {
        asset_id: "ast_bar".into(),
        name: "bar.mp4".into(),
        path: source.to_string_lossy().into_owned(),
        media_kind: MediaKind::Video,
        duration: Some(RationalTime::new(FRAMES, rate)),
        file_size: 1000,
        dimensions: Some(AssetDimensions {
            width: 128,
            height: 72,
        }),
    });
    let range = TimeRange::new(RationalTime::new(0, rate), RationalTime::new(FRAMES, rate));
    let mut item = TimelineItem::new(
        Some("ast_bar".into()),
        "bar".into(),
        ItemKind::Video,
        range,
        range,
    )
    .unwrap();
    // Fit fills the 256x144 canvas; 0.5 gives exactly the 128x72 source size (no resampling).
    item.transform.scale_x = 0.5;
    item.transform.scale_y = 0.5;
    item.transform.position_x = pos.0;
    item.transform.position_y = pos.1;
    if animated {
        // Equal values at both ends: the layer does not move, but the export takes the
        // keyframe-automation path.
        for (property, value) in [
            (KeyframeProperty::PositionX, pos.0),
            (KeyframeProperty::PositionY, pos.1),
        ] {
            item.keyframes
                .push(Keyframe::new(property, RationalTime::new(0, rate), value));
            item.keyframes.push(Keyframe::new(
                property,
                RationalTime::new(FRAMES - 1, rate),
                value,
            ));
        }
    }
    let mut track = Track::new("V1".into(), TrackKind::Video);
    track.items.push(item);
    project.tracks.push(track);
    project
}

fn gray_frame(file: &Path) -> Vec<u8> {
    gray_frame_at(file, "0.5")
}

fn gray_frame_at(file: &Path, seconds: &str) -> Vec<u8> {
    let output = Command::new(ffmpeg_path())
        .args(["-v", "error", "-ss", seconds, "-i"])
        .arg(file)
        .args(["-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "gray", "-"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert_eq!(output.stdout.len(), (CANVAS.0 * CANVAS.1) as usize);
    output.stdout
}

/// Centre of a bar along one axis, in edge coordinates, from the mean profile over the other axis
/// restricted to the part of the canvas the bar occupies.
fn bar_centre(frame: &[u8], axis_x: bool, lo: usize, hi: usize, cross: (usize, usize)) -> f64 {
    let (w, h) = (CANVAS.0 as usize, CANVAS.1 as usize);
    let n = if axis_x { w } else { h };
    let mut profile = vec![0.0f64; n];
    for (i, value) in profile.iter_mut().enumerate() {
        let mut sum = 0.0;
        for c in cross.0..cross.1 {
            let (x, y) = if axis_x { (i, c) } else { (c, i) };
            sum += f64::from(frame[y * w + x]);
        }
        *value = sum / (cross.1 - cross.0) as f64;
    }
    let base = profile[lo..hi].iter().cloned().fold(f64::MAX, f64::min);
    let mut weight = 0.0;
    let mut moment = 0.0;
    for (i, value) in profile.iter().enumerate().take(hi).skip(lo) {
        let v = (value - base).max(0.0);
        weight += v;
        moment += v * (i as f64 + 0.5);
    }
    moment / weight
}

fn check(name: &str, pos: (f32, f32), animated: bool) {
    let dir = test_dir(name);
    let source = dir.join("bar.mp4");
    bar_source(&source);
    let out = dir.join("out.mp4");
    let profile = ExportProfile {
        width: CANVAS.0,
        height: CANVAS.1,
        frame_rate: FrameRate::FPS_30,
        ..Default::default()
    };
    ExportEngine::render_snapshot(&project(&source, pos, animated), &out, &profile, None)
        .expect("export succeeds");
    let frame = gray_frame(&out);
    // Layer origin: canvas centre - 64 + pos. Bars are 4 px wide, centred at src 42 and 22.
    let left = f64::from(CANVAS.0) / 2.0 - 64.0 + f64::from(pos.0);
    let top = f64::from(CANVAS.1) / 2.0 - 36.0 + f64::from(pos.1);
    let (l, t) = (left as usize, top as usize);
    // Measure inside the layer, away from its borders: the vertical bar uses rows t+30..t+60,
    // the horizontal bar uses columns l+60..l+120.
    let x = bar_centre(&frame, true, l + 30, l + 55, (t + 30, t + 60));
    let y = bar_centre(&frame, false, t + 10, t + 34, (l + 60, l + 120));
    let (ex, ey) = (left + 42.0, top + 22.0);
    assert!(
        (x - ex).abs() < 0.3 && (y - ey).abs() < 0.3,
        "{name}: bar centre ({x:.2}, {y:.2}) expected ({ex:.2}, {ey:.2})"
    );
}

#[test]
fn p60o_static_layer_at_an_odd_position_is_not_floored_to_even() {
    check("static", (41.0, 7.0), false);
}

#[test]
fn p60o_animated_layer_at_an_odd_position_is_not_floored_to_even() {
    check("animated", (41.0, 7.0), true);
}

#[test]
fn p60o_even_position_control_is_exact_too() {
    check("even", (40.0, 8.0), false);
}

/// Two clips with the same picture and position joined by a cross dissolve: whatever the progress,
/// the blend shows the bar where the layers are placed. Exercises the transition branches.
#[test]
fn p60o_transition_branches_keep_an_odd_position_too() {
    use tinixcut_lib::modules::domain::{CutTransition, TransitionKind};
    let dir = test_dir("transition");
    let source = dir.join("bar.mp4");
    bar_source_frames(&source, 3);
    let rate = FrameRate::FPS_30;
    let mut project = project(&source, (41.0, 7.0), false);
    project.assets[0].duration = Some(RationalTime::new(90, rate));
    let mut track = Track::new("V1".into(), TrackKind::Video);
    let mut make = |start: i64, src: i64| {
        let timeline = TimeRange::new(RationalTime::new(start, rate), RationalTime::new(30, rate));
        let mut item = TimelineItem::new(
            Some("ast_bar".into()),
            "bar".into(),
            ItemKind::Video,
            timeline,
            timeline,
        )
        .unwrap();
        item.source_range =
            TimeRange::new(RationalTime::new(src, rate), RationalTime::new(30, rate));
        item.transform.scale_x = 0.5;
        item.transform.scale_y = 0.5;
        item.transform.position_x = 41.0;
        item.transform.position_y = 7.0;
        item
    };
    let left = make(0, 30);
    let right = make(30, 30);
    track.transitions.push(CutTransition {
        transition_id: "tr_x".into(),
        track_id: track.track_id.clone(),
        left_item_id: left.item_id.clone(),
        right_item_id: right.item_id.clone(),
        kind: TransitionKind::CrossDissolve,
        duration: RationalTime::new(10, rate),
    });
    track.items = vec![left, right];
    project.tracks = vec![track];
    let out = dir.join("out.mp4");
    let profile = ExportProfile {
        width: CANVAS.0,
        height: CANVAS.1,
        frame_rate: FrameRate::FPS_30,
        ..Default::default()
    };
    ExportEngine::render_snapshot(&project, &out, &profile, None).expect("export succeeds");
    // t = 1.0 s is the cut, in the middle of the transition.
    let frame = gray_frame_at(&out, "1.0");
    let (left_x, top_y) = (128.0 - 64.0 + 41.0, 72.0 - 36.0 + 7.0);
    let (l, t) = (left_x as usize, top_y as usize);
    let x = bar_centre(&frame, true, l + 30, l + 55, (t + 30, t + 60));
    let y = bar_centre(&frame, false, t + 10, t + 34, (l + 60, l + 120));
    assert!(
        (x - (left_x + 42.0)).abs() < 0.3 && (y - (top_y + 22.0)).abs() < 0.3,
        "transition bar centre ({x:.2}, {y:.2}) expected ({:.2}, {:.2})",
        left_x + 42.0,
        top_y + 22.0
    );
}

/// The keyframe-automation path must place a moving layer at the same pixel in every run.
#[test]
fn p60o_moving_layer_lands_on_the_same_pixel_every_run() {
    let dir = test_dir("moving");
    let source = dir.join("bar.mp4");
    bar_source(&source);
    let rate = FrameRate::FPS_30;
    let mut positions = Vec::new();
    for run in 0..8 {
        let mut p = project(&source, (0.0, 7.0), false);
        let item = &mut p.tracks[0].items[0];
        for (frame, value) in [(0, 0.0), (FRAMES - 1, 58.0)] {
            item.keyframes.push(Keyframe::new(
                KeyframeProperty::PositionX,
                RationalTime::new(frame, rate),
                value,
            ));
        }
        let out = dir.join(format!("out{run}.mp4"));
        let profile = ExportProfile {
            width: CANVAS.0,
            height: CANVAS.1,
            frame_rate: FrameRate::FPS_30,
            ..Default::default()
        };
        ExportEngine::render_snapshot(&p, &out, &profile, None).expect("export succeeds");
        let frame = gray_frame_at(&out, "0.5");
        // Frame 15 of 29 steps: x = 15 * 2 = 30 -> layer left = 64 + 30.
        let left = 64.0 + 30.0;
        let x = bar_centre(
            &frame,
            true,
            left as usize + 30,
            left as usize + 55,
            (50, 100),
        );
        positions.push(x - (left + 42.0));
    }
    let worst = positions.iter().fold(0.0f64, |a, d| a.max(d.abs()));
    assert!(worst < 1.1, "bar offsets per run: {positions:?}");
    let spread = positions.iter().cloned().fold(f64::MIN, f64::max)
        - positions.iter().cloned().fold(f64::MAX, f64::min);
    assert!(spread < 0.3, "positions differ between runs: {positions:?}");
}

/// Several moving clips on two tracks, exported repeatedly: the keyframe commands must reach the
/// overlay for the same frame in every run (commands fired from a layer chain that runs ahead of
/// the overlay used to move a clip a few frames early now and then).
#[test]
fn p60o_several_moving_clips_land_on_the_same_pixels_every_run() {
    let dir = test_dir("several");
    let source = dir.join("bar.mp4");
    bar_source(&source);
    let rate = FrameRate::FPS_30;
    let frames = [10i64, 25, 40, 55];
    let mut worst = 0.0f64;
    let mut seen: Vec<Vec<f64>> = Vec::new();
    for run in 0..10 {
        let mut p = project(&source, (0.0, 7.0), false);
        p.assets[0].duration = Some(RationalTime::new(30, rate));
        let mut track = Track::new("V1".into(), TrackKind::Video);
        let mut small = Track::new("V2".into(), TrackKind::Video);
        for start in [0i64, 30] {
            let timeline =
                TimeRange::new(RationalTime::new(start, rate), RationalTime::new(30, rate));
            let make = |scale: f32, y: f32| {
                let mut item = TimelineItem::new(
                    Some("ast_bar".into()),
                    "bar".into(),
                    ItemKind::Video,
                    timeline,
                    timeline,
                )
                .unwrap();
                item.source_range =
                    TimeRange::new(RationalTime::new(0, rate), RationalTime::new(30, rate));
                item.transform.scale_x = scale;
                item.transform.scale_y = scale;
                item.transform.position_y = y;
                for (frame, value) in [(0, 0.0), (29, 58.0)] {
                    item.keyframes.push(Keyframe::new(
                        KeyframeProperty::PositionX,
                        RationalTime::new(frame, rate),
                        value,
                    ));
                }
                item
            };
            track.items.push(make(0.5, 7.0));
            small.items.push(make(0.25, 50.0));
        }
        p.tracks = vec![track, small];
        let out = dir.join(format!("out{run}.mp4"));
        let profile = ExportProfile {
            width: CANVAS.0,
            height: CANVAS.1,
            frame_rate: FrameRate::FPS_30,
            ..Default::default()
        };
        ExportEngine::render_snapshot(&p, &out, &profile, None).expect("export succeeds");
        let mut offsets = Vec::new();
        for &f in &frames {
            let frame = gray_frame_at(&out, &format!("{:.4}", (f as f64 - 0.1) / 30.0));
            let left = 64.0 + 2.0 * (f % 30) as f64;
            let x = bar_centre(
                &frame,
                true,
                left as usize + 30,
                left as usize + 55,
                (50, 80),
            );
            offsets.push(x - (left + 42.0));
            worst = worst.max((x - (left + 42.0)).abs());
        }
        seen.push(offsets);
    }
    assert!(worst < 1.1, "bar offsets per run and frame: {seen:?}");
}
