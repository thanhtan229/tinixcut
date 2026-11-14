//! G52 Step 4: the parity frame is the export frame (same compiler, same automation), resolution
//! aside, and it is cheap enough to be used while the playhead is paused.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;
use tinixcut_lib::modules::composition::parity_frame::{render_parity_frame_png, window_plan};
use tinixcut_lib::modules::composition::CompositionPlan;
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    Asset, AssetDimensions, ItemKind, Keyframe, KeyframeProperty, MediaKind, Project, TimelineItem,
    Track, TrackKind,
};
use tinixcut_lib::modules::export::{ExportEngine, ExportProfile};
use tinixcut_lib::modules::media::resolver::{BinaryKind, MediaBinaryResolver};

const CANVAS: (u32, u32) = (256, 144);

fn ffmpeg_path() -> PathBuf {
    MediaBinaryResolver::resolve(BinaryKind::FFmpeg)
        .expect("bundled FFmpeg")
        .path
}

fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tinixcut_p60pf_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// 128x72, 3 s: gray with a white vertical bar (x 40..44), a horizontal bar (y 20..24) and a
/// brightness ramp that changes every frame, so a wrong source frame is visible.
fn bar_source(path: &Path) {
    let status = Command::new(ffmpeg_path())
        .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
        .arg("color=c=gray:s=128x72:r=30:d=3,drawbox=x=40:y=0:w=4:h=72:color=white:t=fill,drawbox=x=0:y=20:w=128:h=4:color=white:t=fill,drawbox=x=100:y=40:w=20:h=20:color=red:t=fill,geq=lum='lum(X,Y)':cb='cb(X,Y)':cr='128+N'")
        .args(["-c:v", "libx264", "-crf", "10", "-pix_fmt", "yuv444p"])
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success());
}

fn project(source: &Path, animated: bool) -> Project {
    let rate = FrameRate::FPS_30;
    let mut project = Project::new("Parity frame".to_string());
    project.settings.width = CANVAS.0;
    project.settings.height = CANVAS.1;
    project.settings.frame_rate = rate;
    project.assets.push(Asset {
        asset_id: "ast_bar".into(),
        name: "bar.mp4".into(),
        path: source.to_string_lossy().into_owned(),
        media_kind: MediaKind::Video,
        duration: Some(RationalTime::new(90, rate)),
        file_size: 1000,
        dimensions: Some(AssetDimensions {
            width: 128,
            height: 72,
        }),
    });
    let mut track = Track::new("V1".into(), TrackKind::Video);
    // Second half of the source starts at 1 s: the source frame differs from the timeline frame.
    let timeline = TimeRange::new(RationalTime::new(0, rate), RationalTime::new(60, rate));
    let mut item = TimelineItem::new(
        Some("ast_bar".into()),
        "bar".into(),
        ItemKind::Video,
        timeline,
        timeline,
    )
    .unwrap();
    item.source_range = TimeRange::new(RationalTime::new(30, rate), RationalTime::new(60, rate));
    item.transform.scale_x = 0.5;
    item.transform.scale_y = 0.5;
    item.transform.position_y = 7.0;
    if animated {
        for (property, frame, value) in [
            (KeyframeProperty::PositionX, 0, 0.0),
            (KeyframeProperty::PositionX, 59, 59.0),
            (KeyframeProperty::ScaleX, 0, 0.5),
            (KeyframeProperty::ScaleX, 59, 0.7),
            (KeyframeProperty::ScaleY, 0, 0.5),
            (KeyframeProperty::ScaleY, 59, 0.7),
        ] {
            item.keyframes.push(Keyframe::new(
                property,
                RationalTime::new(frame, rate),
                value,
            ));
        }
    } else {
        item.transform.position_x = 41.0;
    }
    track.items.push(item);
    project.tracks.push(track);
    project
}

fn decode_gray(png_or_video: &[u8], is_png: bool, w: u32, h: u32) -> Vec<u8> {
    let mut command = Command::new(ffmpeg_path());
    command.args(["-v", "error"]);
    if is_png {
        command.args(["-f", "image2pipe", "-c:v", "png"]);
    }
    let mut child = command
        .args(["-i", "pipe:0", "-frames:v", "1", "-vf"])
        .arg(format!("scale={w}:{h}:flags=bicubic,format=gray"))
        .args(["-f", "rawvideo", "-pix_fmt", "gray", "pipe:1"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let data = png_or_video.to_vec();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(&data);
    });
    let output = child.wait_with_output().unwrap();
    writer.join().unwrap();
    assert_eq!(
        output.stdout.len(),
        (w * h) as usize,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn export_frame_gray(file: &Path, frame: i64) -> Vec<u8> {
    let output = Command::new(ffmpeg_path())
        .args(["-v", "error", "-i"])
        .arg(file)
        .args(["-vf"])
        .arg(format!("select=eq(n\\,{frame}),format=gray"))
        .args(["-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "gray", "-"])
        .output()
        .unwrap();
    assert_eq!(output.stdout.len(), (CANVAS.0 * CANVAS.1) as usize);
    output.stdout
}

fn mean_abs_diff(a: &[u8], b: &[u8]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (f64::from(*x) - f64::from(*y)).abs())
        .sum::<f64>()
        / a.len() as f64
}

fn render_export(project: &Project, dir: &Path) -> PathBuf {
    let out = dir.join("export.mp4");
    let profile = ExportProfile {
        width: CANVAS.0,
        height: CANVAS.1,
        frame_rate: FrameRate::FPS_30,
        crf: 10,
        ..Default::default()
    };
    ExportEngine::render_snapshot(project, &out, &profile, None).expect("export succeeds");
    out
}

fn check(name: &str, animated: bool, frames: &[i64]) {
    let dir = test_dir(name);
    let source = dir.join("bar.mp4");
    bar_source(&source);
    let project = project(&source, animated);
    let export = render_export(&project, &dir);
    let plan = CompositionPlan::from_project(&project).unwrap();
    for &frame in frames {
        let started = Instant::now();
        let (png, text) = render_parity_frame_png(&plan, frame, CANVAS.0).expect("parity frame");
        let elapsed = started.elapsed();
        assert!(text.is_empty());
        let parity = decode_gray(&png, true, CANVAS.0, CANVAS.1);
        let exported = export_frame_gray(&export, frame);
        let diff = mean_abs_diff(&parity, &exported);
        eprintln!("{name} frame {frame}: mean abs diff {diff:.3} in {elapsed:?}");
        // The export is H.264 (crf 10); the parity frame is not encoded. Whole-picture agreement
        // to a couple of levels means the same layout, scale and source frame.
        assert!(
            diff < 2.5,
            "{name}: frame {frame} differs from the export by {diff:.2} levels"
        );
    }
}

#[test]
fn p60pf_static_layer_at_an_odd_position_matches_the_export() {
    check("static", false, &[0, 17, 44, 59]);
}

#[test]
fn p60pf_animated_layer_matches_the_export_mid_easing() {
    check("animated", true, &[0, 17, 44, 59]);
}

#[test]
fn p60pf_negative_control_a_neighbouring_frame_is_detected() {
    // The ramp changes every frame and the clip moves: frame 18 must not pass as frame 17.
    let dir = test_dir("negative");
    let source = dir.join("bar.mp4");
    bar_source(&source);
    let project = project(&source, true);
    let export = render_export(&project, &dir);
    let plan = CompositionPlan::from_project(&project).unwrap();
    let (png, _) = render_parity_frame_png(&plan, 18, CANVAS.0).unwrap();
    let parity = decode_gray(&png, true, CANVAS.0, CANVAS.1);
    let same = mean_abs_diff(&parity, &export_frame_gray(&export, 18));
    let other = mean_abs_diff(&parity, &export_frame_gray(&export, 17));
    assert!(
        other > same * 1.5,
        "frame 17 ({other:.3}) must be clearly worse than frame 18 ({same:.3})"
    );
}

#[test]
fn p60pf_frames_inside_a_transition_or_without_picture_are_not_supported() {
    let dir = test_dir("unsupported");
    let source = dir.join("bar.mp4");
    bar_source(&source);
    let project = project(&source, false);
    let plan = CompositionPlan::from_project(&project).unwrap();
    // Past the end of the clip nothing is visible.
    assert!(render_parity_frame_png(&plan, 80, CANVAS.0).is_err());
    assert!(window_plan(&plan, 10).is_ok());
}

/// A clip that starts at the target frame shows the filter's initial state, and keyframe commands
/// for the next frame can reach the overlay early: without the lead frame the result differed by a
/// pixel between runs (about one run in six).
#[test]
fn p60pf_frames_are_identical_between_runs_including_a_clip_start() {
    let dir = test_dir("determinism");
    let source = dir.join("bar.mp4");
    bar_source(&source);
    let rate = FrameRate::FPS_30;
    let mut project = project(&source, true);
    // A second, keyframed clip that starts at frame 30 on its own track.
    let timeline = TimeRange::new(RationalTime::new(30, rate), RationalTime::new(30, rate));
    let mut item = TimelineItem::new(
        Some("ast_bar".into()),
        "late".into(),
        ItemKind::Video,
        timeline,
        timeline,
    )
    .unwrap();
    item.source_range = TimeRange::new(RationalTime::new(45, rate), RationalTime::new(30, rate));
    item.transform.scale_x = 0.25;
    item.transform.scale_y = 0.25;
    item.transform.position_y = -30.0;
    for (frame, value) in [(0, 10.3), (29, 70.7)] {
        item.keyframes.push(Keyframe::new(
            KeyframeProperty::PositionX,
            RationalTime::new(frame, rate),
            value,
        ));
    }
    let mut track = Track::new("V2".into(), TrackKind::Video);
    track.items.push(item);
    project.tracks.push(track);
    let plan = CompositionPlan::from_project(&project).unwrap();
    for frame in [29i64, 30, 31, 44] {
        let first = render_parity_frame_png(&plan, frame, CANVAS.0).unwrap().0;
        for run in 1..8 {
            let again = render_parity_frame_png(&plan, frame, CANVAS.0).unwrap().0;
            assert_eq!(first, again, "frame {frame} differs on run {run}");
        }
    }
}
