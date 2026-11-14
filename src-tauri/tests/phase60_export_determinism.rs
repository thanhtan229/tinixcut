//! G52: an exported frame must not depend on command timing.
//!
//! A keyframed layer used to be driven by `sendcmd` files. The first frame of the clip had no
//! command (it showed the static transform), and commands for frame n+1 could move the overlay
//! while frame n was still in flight when the layer's decode was slow (1080p, not tiny sources).
//! Overlay x/y are now expressions of the frame time, and the chain starts from the keyframe
//! values of its first frame.
//!
//! Set `TC_DETERMINISM_RUNS` to change the number of exports per test (default 12).

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

const CANVAS: (u32, u32) = (1920, 1080);
const FRAMES: i64 = 24;
/// Source position of the vertical bar (x) and horizontal bar (y), both 8 px thick.
const BAR_X: usize = 100;
const BAR_Y: usize = 300;
const BAR: usize = 8;

fn runs() -> usize {
    std::env::var("TC_DETERMINISM_RUNS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(12)
}

fn ffmpeg_path() -> PathBuf {
    MediaBinaryResolver::resolve(BinaryKind::FFmpeg)
        .expect("bundled FFmpeg")
        .path
}

fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tinixcut_p60d_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// 1080p clip of noisy gray (so that decoding it costs something) with two white bars.
fn source_1080p(path: &Path) {
    let status = Command::new(ffmpeg_path())
        .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
        .arg(format!(
            "color=c=gray:s={}x{}:r=30:d=1,\
             drawbox=x={BAR_X}:y=0:w={BAR}:h={}:color=white:t=fill,\
             drawbox=x=0:y={BAR_Y}:w={}:h={BAR}:color=white:t=fill,\
             noise=alls=12:allf=t",
            CANVAS.0, CANVAS.1, CANVAS.1, CANVAS.0
        ))
        .args(["-c:v", "libx264", "-crf", "16", "-pix_fmt", "yuv420p"])
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success());
}

/// One 1080p clip at scale 1.0 whose position is keyframed over the whole clip.
fn project(source: &Path, build: impl Fn(&mut TimelineItem, FrameRate)) -> Project {
    let rate = FrameRate::FPS_30;
    let mut project = Project::new("Determinism".to_string());
    project.settings.width = CANVAS.0;
    project.settings.height = CANVAS.1;
    project.settings.frame_rate = rate;
    project.assets.push(Asset {
        asset_id: "ast_src".into(),
        name: "src.mp4".into(),
        path: source.to_string_lossy().into_owned(),
        media_kind: MediaKind::Video,
        duration: Some(RationalTime::new(30, rate)),
        file_size: 1000,
        dimensions: Some(AssetDimensions {
            width: CANVAS.0,
            height: CANVAS.1,
        }),
    });
    let range = TimeRange::new(RationalTime::new(0, rate), RationalTime::new(FRAMES, rate));
    let source_range = TimeRange::new(RationalTime::new(0, rate), RationalTime::new(FRAMES, rate));
    let mut item = TimelineItem::new(
        Some("ast_src".into()),
        "src".into(),
        ItemKind::Video,
        range,
        source_range,
    )
    .unwrap();
    build(&mut item, rate);
    let mut track = Track::new("V1".into(), TrackKind::Video);
    track.items.push(item);
    project.tracks.push(track);
    project
}

/// Moves 21.4 -> 301.6 px right and 7.4 -> 97.6 px down; the static transform stays at zero so
/// the first frame cannot be right by accident.
fn moving(item: &mut TimelineItem, rate: FrameRate) {
    for (property, from, to) in [
        (KeyframeProperty::PositionX, 21.4, 301.6),
        (KeyframeProperty::PositionY, 7.4, 97.6),
    ] {
        item.keyframes
            .push(Keyframe::new(property, RationalTime::new(0, rate), from));
        item.keyframes.push(Keyframe::new(
            property,
            RationalTime::new(FRAMES - 1, rate),
            to,
        ));
    }
}

fn export(project: &Project, out: &Path) {
    let profile = ExportProfile {
        width: CANVAS.0,
        height: CANVAS.1,
        frame_rate: FrameRate::FPS_30,
        ..Default::default()
    };
    ExportEngine::render_snapshot(project, out, &profile, None).expect("export succeeds");
}

/// All frames of the export as 8-bit gray.
fn gray_frames(file: &Path) -> Vec<Vec<u8>> {
    let output = Command::new(ffmpeg_path())
        .args(["-v", "error", "-i"])
        .arg(file)
        .args(["-f", "rawvideo", "-pix_fmt", "gray", "-"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    let size = (CANVAS.0 * CANVAS.1) as usize;
    assert_eq!(output.stdout.len() % size, 0);
    output.stdout.chunks(size).map(<[u8]>::to_vec).collect()
}

/// Centre (edge coordinates) of the bright bar in a band, from the profile along one axis.
fn bright_centre(frame: &[u8], axis_x: bool, band: (usize, usize)) -> f64 {
    let (w, h) = (CANVAS.0 as usize, CANVAS.1 as usize);
    let n = if axis_x { w } else { h };
    let mut weight = 0.0;
    let mut moment = 0.0;
    for i in 0..n {
        let mut sum = 0.0;
        for c in band.0..band.1 {
            let (x, y) = if axis_x { (i, c) } else { (c, i) };
            sum += f64::from(frame[y * w + x]);
        }
        let v = sum / (band.1 - band.0) as f64;
        if v > 200.0 {
            weight += v - 128.0;
            moment += (v - 128.0) * (i as f64 + 0.5);
        }
    }
    moment / weight
}

/// Where the bars must be in output frame `n`: the layer sits at the rounded keyframe position.
fn expected_bars(plan: &CompositionPlan, n: i64) -> (f64, f64) {
    let rate = FrameRate::FPS_30;
    let layer = &plan.visual_layers[0];
    let offset = RationalTime::new(n, rate);
    let px = layer.evaluate_property(KeyframeProperty::PositionX, offset);
    let py = layer.evaluate_property(KeyframeProperty::PositionY, offset);
    (
        f64::from(px.round()) + BAR_X as f64 + BAR as f64 / 2.0,
        f64::from(py.round()) + BAR_Y as f64 + BAR as f64 / 2.0,
    )
}

/// Every frame, the first included, must have the bars where the evaluator puts them, and every
/// run must produce the same pictures.
#[test]
fn p60d_keyframed_1080p_clip_matches_the_evaluator_in_every_run() {
    check_moving("moving", moving);
}

/// Same, but the static transform already equals the first keyframe (whole pixels), so only
/// command timing can make the first frame differ between runs.
#[test]
fn p60d_first_frame_is_stable_when_the_static_transform_matches() {
    check_moving("static_matches", |item, rate| {
        item.transform.position_x = 21.0;
        item.transform.position_y = 7.0;
        for (property, from, to) in [
            (KeyframeProperty::PositionX, 21.0, 301.6),
            (KeyframeProperty::PositionY, 7.0, 97.6),
        ] {
            item.keyframes
                .push(Keyframe::new(property, RationalTime::new(0, rate), from));
            item.keyframes.push(Keyframe::new(
                property,
                RationalTime::new(FRAMES - 1, rate),
                to,
            ));
        }
    });
}

fn check_moving(name: &str, build: impl Fn(&mut TimelineItem, FrameRate)) {
    let dir = test_dir(name);
    let source = dir.join("src.mp4");
    source_1080p(&source);
    let project = project(&source, build);
    let plan = CompositionPlan::from_project(&project).expect("plan");
    let mut reference: Option<Vec<Vec<u8>>> = None;
    let mut distinct_first_frames = Vec::<Vec<u8>>::new();
    let mut failures = Vec::new();
    for run in 0..runs() {
        let out = dir.join(format!("out{run}.mp4"));
        export(&project, &out);
        let frames = gray_frames(&out);
        assert_eq!(frames.len(), FRAMES as usize, "run {run}");
        for (n, frame) in frames.iter().enumerate() {
            let (ex, ey) = expected_bars(&plan, n as i64);
            // Rows 600..640 cross only the vertical bar, columns 1500..1540 only the horizontal
            // bar (the clip is at most 302 px right of the canvas origin).
            let x = bright_centre(frame, true, (600, 640));
            let y = bright_centre(frame, false, (1500, 1540));
            if (x - ex).abs() > 0.35 || (y - ey).abs() > 0.35 {
                failures.push(format!(
                    "run {run} frame {n}: bars ({x:.2}, {y:.2}) expected ({ex:.2}, {ey:.2})"
                ));
            }
        }
        if !distinct_first_frames.contains(&frames[0]) {
            distinct_first_frames.push(frames[0].clone());
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
    println!(
        "{} runs, {} distinct first frame(s), {} failure(s)",
        runs(),
        distinct_first_frames.len(),
        failures.len()
    );
    assert!(
        failures.is_empty(),
        "{} failure(s):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// The chain's first frame must use its keyframe values for scale, rotation and opacity too:
/// a constant 50% opacity keyframed over a clip whose static opacity is 100%.
#[test]
fn p60d_first_frame_uses_keyframed_opacity() {
    let dir = test_dir("opacity");
    let source = dir.join("src.mp4");
    source_1080p(&source);
    let project = project(&source, |item, rate| {
        for at in [0, FRAMES - 1] {
            item.keyframes.push(Keyframe::new(
                KeyframeProperty::Opacity,
                RationalTime::new(at, rate),
                0.5,
            ));
        }
    });
    let out = dir.join("out.mp4");
    export(&project, &out);
    let frames = gray_frames(&out);
    // Mean over a bar-free block: gray 128 at full opacity, about 64 at half.
    let mean = |frame: &[u8]| {
        let w = CANVAS.0 as usize;
        let mut sum = 0u64;
        for y in 600..700 {
            for x in 800..900 {
                sum += u64::from(frame[y * w + x]);
            }
        }
        sum as f64 / 10_000.0
    };
    let means: Vec<f64> = frames.iter().map(|f| mean(f)).collect();
    println!("opacity means: first {:.1}, second {:.1}", means[0], means[1]);
    for (n, m) in means.iter().enumerate() {
        assert!(
            (m - means[1]).abs() < 2.0,
            "frame {n} mean {m:.1} differs from frame 1 ({:.1})",
            means[1]
        );
    }
}
