//! G52: a keyframed base clip plus a keyframed overlay on a second track (two decoders racing),
//! exported repeatedly at 1080p. Every frame must put both clips where the evaluator says, and
//! every run must produce the same pictures. One-clip tests do not show timing effects; two
//! inputs plus an overlay did (47-67 of 450 frames differed between runs before the fix).
//!
//! Set `TC_DETERMINISM_RUNS` to change the number of exports (default 6).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tinixcut_lib::modules::composition::CompositionPlan;
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    Asset, AssetDimensions, ItemKind, Keyframe, KeyframeProperty, MediaKind, Project, TimelineItem,
    Track, TrackKind,
};
use tinixcut_lib::modules::export::{ExportEngine, ExportProfile};
use tinixcut_lib::modules::media::resolver::{BinaryKind, MediaBinaryResolver};

const W: usize = 1920;
const H: usize = 1080;
const FRAMES: i64 = 48;
const OVERLAY_START: i64 = 12;
/// Bars in the source pictures (8 px thick).
const BAR_X: f64 = 100.0;
const BAR_Y: f64 = 300.0;
const BAR: f64 = 8.0;

fn runs() -> usize {
    std::env::var("TC_DETERMINISM_RUNS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(6)
}

fn ffmpeg_path() -> PathBuf {
    MediaBinaryResolver::resolve(BinaryKind::FFmpeg)
        .expect("bundled FFmpeg")
        .path
}

fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tinixcut_p60tt_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Gray picture with white bars (base) or white picture with black bars (overlay), plus noise so
/// the decoder has real work to do.
fn source(path: &Path, dark_bars: bool) {
    let (background, bars) = if dark_bars {
        ("white", "black")
    } else {
        ("gray", "white")
    };
    let status = Command::new(ffmpeg_path())
        .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
        .arg(format!(
            "color=c={background}:s={W}x{H}:r=30:d=2,\
             drawbox=x={BAR_X}:y=0:w={BAR}:h={H}:color={bars}:t=fill,\
             drawbox=x=0:y={BAR_Y}:w={W}:h={BAR}:color={bars}:t=fill,\
             noise=alls=10:allf=t",
        ))
        .args(["-c:v", "libx264", "-crf", "16", "-pix_fmt", "yuv420p"])
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success());
}

fn asset(id: &str, path: &Path) -> Asset {
    Asset {
        asset_id: id.into(),
        name: format!("{id}.mp4"),
        path: path.to_string_lossy().into_owned(),
        media_kind: MediaKind::Video,
        duration: Some(RationalTime::new(60, FrameRate::FPS_30)),
        file_size: 1000,
        dimensions: Some(AssetDimensions {
            width: W as u32,
            height: H as u32,
        }),
    }
}

fn keyframes(item: &mut TimelineItem, rate: FrameRate, last: i64, x: (f32, f32), y: (f32, f32)) {
    for (property, from, to) in [
        (KeyframeProperty::PositionX, x.0, x.1),
        (KeyframeProperty::PositionY, y.0, y.1),
    ] {
        item.keyframes
            .push(Keyframe::new(property, RationalTime::new(0, rate), from));
        item.keyframes
            .push(Keyframe::new(property, RationalTime::new(last, rate), to));
    }
}

fn project(base: &Path, overlay: &Path) -> Project {
    let rate = FrameRate::FPS_30;
    let mut project = Project::new("Two track".to_string());
    project.settings.width = W as u32;
    project.settings.height = H as u32;
    project.settings.frame_rate = rate;
    project.assets.push(asset("ast_base", base));
    project.assets.push(asset("ast_over", overlay));

    let range = |start: i64, frames: i64| {
        TimeRange::new(
            RationalTime::new(start, rate),
            RationalTime::new(frames, rate),
        )
    };
    let mut a = TimelineItem::new(
        Some("ast_base".into()),
        "base".into(),
        ItemKind::Video,
        range(0, FRAMES),
        range(0, FRAMES),
    )
    .unwrap();
    // Whole-canvas clip moving 21.4 -> 181.6 px right and 7.4 -> 57.6 px down.
    keyframes(&mut a, rate, FRAMES - 1, (21.4, 181.6), (7.4, 57.6));
    let mut v1 = Track::new("V1".into(), TrackKind::Video);
    v1.items.push(a);

    let overlay_frames = FRAMES - OVERLAY_START;
    let mut b = TimelineItem::new(
        Some("ast_over".into()),
        "overlay".into(),
        ItemKind::Video,
        range(OVERLAY_START, overlay_frames),
        range(5, overlay_frames),
    )
    .unwrap();
    b.transform.scale_x = 0.5;
    b.transform.scale_y = 0.5;
    // Half-size clip drifting 101.3 -> 301.9 px right and 33.1 -> 83.4 px down.
    keyframes(
        &mut b,
        rate,
        overlay_frames - 1,
        (101.3, 301.9),
        (33.1, 83.4),
    );
    let mut v2 = Track::new("V2".into(), TrackKind::Video);
    v2.items.push(b);

    project.tracks.push(v1);
    project.tracks.push(v2);
    project
}

fn export(project: &Project, out: &Path) {
    let profile = ExportProfile {
        width: W as u32,
        height: H as u32,
        frame_rate: FrameRate::FPS_30,
        ..Default::default()
    };
    ExportEngine::render_snapshot(project, out, &profile, None).expect("export succeeds");
}

fn gray_frames(file: &Path) -> Vec<Vec<u8>> {
    let output = Command::new(ffmpeg_path())
        .args(["-v", "error", "-i"])
        .arg(file)
        .args(["-f", "rawvideo", "-pix_fmt", "gray", "-"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert_eq!(output.stdout.len() % (W * H), 0);
    output.stdout.chunks(W * H).map(<[u8]>::to_vec).collect()
}

/// Centre of the bar (edge coordinates) along one axis: `bright` looks for pixels above 200,
/// otherwise below 60. `band` is the other-axis range averaged, `span` the range searched.
fn bar_centre(
    frame: &[u8],
    axis_x: bool,
    band: (usize, usize),
    span: (usize, usize),
    bright: bool,
) -> f64 {
    let mut weight = 0.0;
    let mut moment = 0.0;
    for i in span.0..span.1 {
        let mut sum = 0.0;
        for c in band.0..band.1 {
            let (x, y) = if axis_x { (i, c) } else { (c, i) };
            sum += f64::from(frame[y * W + x]);
        }
        let v = sum / (band.1 - band.0) as f64;
        let hit = if bright { v > 200.0 } else { v < 60.0 };
        if hit {
            let w = if bright { v - 128.0 } else { 128.0 - v };
            weight += w;
            moment += w * (i as f64 + 0.5);
        }
    }
    moment / weight
}

#[test]
fn p60tt_two_keyframed_tracks_match_the_evaluator_and_repeat_exactly() {
    let dir = test_dir("two_track");
    let base = dir.join("base.mp4");
    let overlay = dir.join("overlay.mp4");
    source(&base, false);
    source(&overlay, true);
    let project = project(&base, &overlay);
    let plan = CompositionPlan::from_project(&project).expect("plan");
    let rate = FrameRate::FPS_30;

    let mut reference: Option<Vec<Vec<u8>>> = None;
    let mut failures = Vec::new();
    for run in 0..runs() {
        let out = dir.join(format!("out{run}.mp4"));
        export(&project, &out);
        let frames = gray_frames(&out);
        assert_eq!(frames.len(), FRAMES as usize, "run {run}");
        for (n, frame) in frames.iter().enumerate() {
            let n = n as i64;
            // Base clip: its bars sit at the rounded keyframe position.
            let base_layer = &plan.visual_layers[0];
            let offset = RationalTime::new(n, rate);
            let bx = f64::from(
                base_layer
                    .evaluate_property(KeyframeProperty::PositionX, offset)
                    .round(),
            );
            let by = f64::from(
                base_layer
                    .evaluate_property(KeyframeProperty::PositionY, offset)
                    .round(),
            );
            // Rows 900..940 cross only the vertical bar; columns 1800..1840 only the horizontal bar.
            let x = bar_centre(frame, true, (900, 940), (0, 900), true);
            let y = bar_centre(frame, false, (1800, 1840), (0, 700), true);
            let (ex, ey) = (bx + BAR_X + BAR / 2.0, by + BAR_Y + BAR / 2.0);
            if (x - ex).abs() > 0.4 || (y - ey).abs() > 0.4 {
                failures.push(format!(
                    "run {run} frame {n}: base bars ({x:.2}, {y:.2}) expected ({ex:.2}, {ey:.2})"
                ));
            }
            // Overlay clip (half size, dark bars on white): from its first frame on.
            if n >= OVERLAY_START {
                let layer = &plan.visual_layers[1];
                let local = RationalTime::new(n - OVERLAY_START, rate);
                let px = f64::from(
                    layer
                        .evaluate_property(KeyframeProperty::PositionX, local)
                        .round(),
                );
                let py = f64::from(
                    layer
                        .evaluate_property(KeyframeProperty::PositionY, local)
                        .round(),
                );
                let (left, top) = (480.0 + px, 270.0 + py);
                let (l, t) = (left as usize, top as usize);
                let ox = bar_centre(frame, true, (t + 40, t + 100), (l + 10, l + 120), false);
                let oy = bar_centre(frame, false, (l + 400, l + 700), (t + 100, t + 260), false);
                let (eox, eoy) = (
                    left + (BAR_X + BAR / 2.0) / 2.0,
                    top + (BAR_Y + BAR / 2.0) / 2.0,
                );
                if (ox - eox).abs() > 0.6 || (oy - eoy).abs() > 0.6 {
                    failures.push(format!(
                        "run {run} frame {n}: overlay bars ({ox:.2}, {oy:.2}) expected ({eox:.2}, {eoy:.2})"
                    ));
                }
            }
        }
        match &reference {
            None => reference = Some(frames),
            Some(first) => {
                let differing: Vec<usize> = (0..first.len())
                    .filter(|&n| first[n] != frames[n])
                    .collect();
                if !differing.is_empty() {
                    failures.push(format!(
                        "run {run} differs from run 0 in frames {differing:?}"
                    ));
                }
            }
        }
    }
    println!("{} runs, {} failure(s)", runs(), failures.len());
    assert!(
        failures.is_empty(),
        "{} failure(s):\n{}",
        failures.len(),
        failures
            .iter()
            .take(25)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// 40 eased keyframe segments over 20 s: the position expression alone is longer than the
/// command-line budget, so the graph is passed to FFmpeg through a file. The export must still
/// put the clip where the evaluator says.
#[test]
fn p60tt_long_motion_is_exported_through_a_filter_graph_file() {
    use tinixcut_lib::modules::export::automation::frame_position_expression;
    let frames: i64 = 600;
    let rate = FrameRate::FPS_30;
    let dir = test_dir("long_motion");
    let base = dir.join("base.mp4");
    source(&base, false);

    let mut project = Project::new("Long motion".to_string());
    project.settings.width = W as u32;
    project.settings.height = H as u32;
    project.settings.frame_rate = rate;
    project.assets.push(asset("ast_base", &base));
    let range = TimeRange::new(RationalTime::new(0, rate), RationalTime::new(frames, rate));
    let mut item = TimelineItem::new(
        Some("ast_base".into()),
        "base".into(),
        ItemKind::Video,
        range,
        TimeRange::new(RationalTime::new(0, rate), RationalTime::new(frames, rate)),
    )
    .unwrap();
    item.transform.scale_x = 0.5;
    item.transform.scale_y = 0.5;
    for k in 0..=40i64 {
        let value = if k % 2 == 0 { -400.3 } else { 400.7 };
        let frame = (k * (frames - 1)) / 40;
        item.keyframes.push(Keyframe::new(
            KeyframeProperty::PositionX,
            RationalTime::new(frame, rate),
            value,
        ));
    }
    let mut track = Track::new("V1".into(), TrackKind::Video);
    track.items.push(item);
    project.tracks.push(track);

    let plan = CompositionPlan::from_project(&project).expect("plan");
    let layer = &plan.visual_layers[0];
    let values: Vec<i32> = (0..frames)
        .map(|n| {
            layer
                .evaluate_property(KeyframeProperty::PositionX, RationalTime::new(n, rate))
                .round() as i32
        })
        .collect();
    let expression = frame_position_expression(0, &values, rate).expect("expression");
    assert!(
        expression.len() > 5000,
        "the motion must make the whole graph exceed the inline budget ({} characters)",
        expression.len()
    );

    let out = dir.join("out.mp4");
    export(&project, &out);
    let frames_out = gray_frames(&out);
    assert_eq!(frames_out.len(), frames as usize);
    let mut failures = Vec::new();
    for n in (0..frames).step_by(7) {
        let left = 480.0 + f64::from(values[n as usize]);
        let (l, t) = (left as usize, 270usize);
        let x = bar_centre(
            &frames_out[n as usize],
            true,
            (t + 40, t + 100),
            (l + 10, l + 120),
            true,
        );
        let expected = left + (BAR_X + BAR / 2.0) / 2.0;
        // An exact .5 position (the eased keyframes pass through them) is rounded together with
        // the rotate-box offset, away from zero, so it can land one pixel from round(position).
        if (x - expected).abs() > 1.05 {
            failures.push(format!("frame {n}: bar at {x:.2}, expected {expected:.2}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
