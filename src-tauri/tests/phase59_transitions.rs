//! Phase 59D acceptance: geometric cut transitions measured on real exports.
//!
//! A solid red clip hands over to a solid blue clip through a 30-frame transition (output
//! frames 75..105). At chosen frames the position of the red/blue edge (or the radius of the
//! blue circle) is measured and compared with the formula the preview implements, which was
//! derived from FFmpeg's `vf_xfade.c`. `p59d_write_golden_vectors` (ignored) writes the
//! measurements for the TypeScript preview geometry to check against.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    Asset, CutTransition, ItemKind, MediaKind, Project, TimelineItem, Track, TrackKind,
    TransitionKind,
};
use tinixcut_lib::modules::export::{ExportEngine, ExportProfile};
use tinixcut_lib::modules::media::resolver::{BinaryKind, MediaBinaryResolver};

const SIZE: usize = 128;
const FIRST_TRANSITION_FRAME: i64 = 75;
const TRANSITION_FRAMES: f64 = 30.0;

fn ffmpeg_path() -> PathBuf {
    MediaBinaryResolver::resolve(BinaryKind::FFmpeg)
        .expect("bundled FFmpeg")
        .path
}

fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tinixcut_p59d_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn solid(path: &Path, color: &str) {
    let status = Command::new(ffmpeg_path())
        .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
        .arg(format!("color=c={color}:s={SIZE}x{SIZE}:r=30:d=6"))
        .args(["-c:v", "libx264", "-crf", "10", "-pix_fmt", "yuv444p"])
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success());
}

fn project(red: &Path, blue: &Path, kind: TransitionKind) -> Project {
    let rate = FrameRate::FPS_30;
    let mut project = Project::new("Transition".to_string());
    project.settings.width = SIZE as u32;
    project.settings.height = SIZE as u32;
    project.settings.frame_rate = rate;
    for (id, path) in [("ast_red", red), ("ast_blue", blue)] {
        project.assets.push(Asset {
            asset_id: id.into(),
            name: id.into(),
            path: path.to_string_lossy().into_owned(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(180, rate)),
            file_size: 1000,
            dimensions: None,
        });
    }
    let mut track = Track::new("V1".into(), TrackKind::Video);
    let mut items = Vec::new();
    for (asset, name, start) in [("ast_red", "Red", 0i64), ("ast_blue", "Blue", 90)] {
        let timeline = TimeRange::new(RationalTime::new(start, rate), RationalTime::new(90, rate));
        // Source handles on both sides of the cut so the 30-frame transition is allowed.
        let source = TimeRange::new(RationalTime::new(30, rate), RationalTime::new(90, rate));
        let mut item = TimelineItem::new(
            Some(asset.into()),
            name.into(),
            ItemKind::Video,
            timeline,
            timeline,
        )
        .unwrap();
        item.source_range = source;
        items.push(item);
    }
    let (left, right) = (items[0].item_id.clone(), items[1].item_id.clone());
    track.items = items;
    track.transitions.push(CutTransition {
        transition_id: "tr_cut".into(),
        track_id: track.track_id.clone(),
        left_item_id: left,
        right_item_id: right,
        kind,
        duration: RationalTime::new(30, rate),
    });
    project.tracks.push(track);
    project
}

fn render(project: &Project, out: &Path) {
    let profile = ExportProfile {
        width: SIZE as u32,
        height: SIZE as u32,
        frame_rate: FrameRate::FPS_30,
        ..Default::default()
    };
    ExportEngine::render_snapshot(project, out, &profile, None).expect("export succeeds");
}

/// RGB frame `frame` of the export.
fn frame_rgb(file: &Path, frame: i64) -> Vec<u8> {
    let output = Command::new(ffmpeg_path())
        .args(["-v", "error", "-i"])
        .arg(file)
        .args([
            "-vf",
            &format!("select=eq(n\\,{frame}),format=rgb24"),
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert_eq!(
        output.stdout.len(),
        SIZE * SIZE * 3,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn is_blue(rgb: &[u8], x: usize, y: usize) -> bool {
    let i = (y * SIZE + x) * 3;
    rgb[i + 2] > rgb[i]
}

/// Fraction (0..1) of the row/column that is blue-dominant, plus which side it starts on.
struct Edge {
    /// Position of the red/blue boundary as a fraction of the width/height.
    boundary: f64,
}

fn horizontal_edge(rgb: &[u8], blue_on_right: bool) -> Edge {
    let row = SIZE / 2;
    let blue = (0..SIZE).filter(|&x| is_blue(rgb, x, row)).count();
    let width = blue as f64 / SIZE as f64;
    Edge {
        boundary: if blue_on_right { 1.0 - width } else { width },
    }
}

fn vertical_edge(rgb: &[u8], blue_on_bottom: bool) -> Edge {
    let column = SIZE / 2;
    let blue = (0..SIZE).filter(|&y| is_blue(rgb, column, y)).count();
    let height = blue as f64 / SIZE as f64;
    Edge {
        boundary: if blue_on_bottom { 1.0 - height } else { height },
    }
}

/// Radius (px) at which the circle's blue share along the centre row falls below one half.
fn circle_radius(rgb: &[u8]) -> f64 {
    let row = SIZE / 2;
    let centre = SIZE / 2;
    (centre..SIZE)
        .find(|&x| !is_blue(rgb, x, row))
        .map_or((SIZE - centre) as f64, |x| (x - centre) as f64)
}

fn progress(frame: i64) -> f64 {
    (frame - FIRST_TRANSITION_FRAME) as f64 / TRANSITION_FRAMES
}

const HALF_DIAGONAL: f64 = 90.50967; // hypot(64, 64)

/// Expected boundary fraction for the linear-edge kinds.
fn expected_boundary(kind: TransitionKind, t: f64) -> f64 {
    match kind {
        TransitionKind::WipeLeft | TransitionKind::SlideLeft | TransitionKind::WipeUp => 1.0 - t,
        _ => t,
    }
}

#[test]
fn p59d_linear_transitions_move_their_edge_with_progress() {
    let dir = test_dir("linear");
    let (red, blue) = (dir.join("red.mp4"), dir.join("blue.mp4"));
    solid(&red, "0xFF0000");
    solid(&blue, "0x0000FF");
    for kind in [
        TransitionKind::WipeLeft,
        TransitionKind::WipeRight,
        TransitionKind::WipeUp,
        TransitionKind::WipeDown,
        TransitionKind::SlideLeft,
        TransitionKind::SlideRight,
    ] {
        let out = dir.join(format!("{kind:?}.mp4"));
        render(&project(&red, &blue, kind), &out);
        for frame in [82, 90, 97] {
            let rgb = frame_rgb(&out, frame);
            let measured = match kind {
                TransitionKind::WipeLeft | TransitionKind::SlideLeft => {
                    horizontal_edge(&rgb, true).boundary
                }
                TransitionKind::WipeRight | TransitionKind::SlideRight => {
                    horizontal_edge(&rgb, false).boundary
                }
                TransitionKind::WipeUp => vertical_edge(&rgb, true).boundary,
                _ => vertical_edge(&rgb, false).boundary,
            };
            let expected = expected_boundary(kind, progress(frame));
            assert!(
                (measured - expected).abs() <= 0.04,
                "{kind:?} at frame {frame}: edge at {measured:.3}, formula says {expected:.3}"
            );
        }
        // Before and after the transition the frames are the plain clips.
        assert!(
            !is_blue(&frame_rgb(&out, 70), 64, 64),
            "{kind:?}: red before"
        );
        assert!(
            is_blue(&frame_rgb(&out, 110), 64, 64),
            "{kind:?}: blue after"
        );
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn p59d_circle_open_radius_follows_the_xfade_formula() {
    let dir = test_dir("circle");
    let (red, blue) = (dir.join("red.mp4"), dir.join("blue.mp4"));
    solid(&red, "0xFF0000");
    solid(&blue, "0x0000FF");
    let out = dir.join("circle.mp4");
    render(&project(&red, &blue, TransitionKind::CircleOpen), &out);
    for frame in [87, 88, 90] {
        let t = progress(frame);
        // 50 % mix radius: r / z + (0.5 - t) * 3 = 0.5  =>  r = (3 t - 1) z
        let expected = (3.0 * t - 1.0) * HALF_DIAGONAL;
        let measured = circle_radius(&frame_rgb(&out, frame));
        assert!(
            (measured - expected).abs() <= 4.0,
            "frame {frame}: radius {measured:.1} px, formula says {expected:.1} px"
        );
    }
    // Negative control: a wipe is not a circle. Its centre row is blue to the right edge.
    let wipe = dir.join("wipe.mp4");
    render(&project(&red, &blue, TransitionKind::WipeLeft), &wipe);
    let wipe_radius = circle_radius(&frame_rgb(&wipe, 87));
    let circle_at_87 = circle_radius(&frame_rgb(&out, 87));
    assert!((wipe_radius - circle_at_87).abs() > 10.0);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn p59d_wipe_left_measured_against_the_wipe_right_formula_fails_negative_control() {
    let dir = test_dir("negative");
    let (red, blue) = (dir.join("red.mp4"), dir.join("blue.mp4"));
    solid(&red, "0xFF0000");
    solid(&blue, "0x0000FF");
    let out = dir.join("wipeleft.mp4");
    render(&project(&red, &blue, TransitionKind::WipeLeft), &out);
    let measured = horizontal_edge(&frame_rgb(&out, 82), true).boundary;
    let wrong = expected_boundary(TransitionKind::WipeRight, progress(82));
    assert!(
        (measured - wrong).abs() > 0.3,
        "the gate cannot tell left from right"
    );
    fs::remove_dir_all(dir).unwrap();
}

/// Regenerates `src/components/__fixtures__/transitionGolden.json` from real exports.
#[test]
#[ignore = "writes src/components/__fixtures__/transitionGolden.json"]
fn p59d_write_golden_vectors() {
    let dir = test_dir("golden");
    let (red, blue) = (dir.join("red.mp4"), dir.join("blue.mp4"));
    solid(&red, "0xFF0000");
    solid(&blue, "0x0000FF");
    let mut entries = Vec::new();
    for kind in [
        TransitionKind::WipeLeft,
        TransitionKind::WipeRight,
        TransitionKind::WipeUp,
        TransitionKind::WipeDown,
        TransitionKind::SlideLeft,
        TransitionKind::SlideRight,
        TransitionKind::CircleOpen,
    ] {
        let out = dir.join(format!("{kind:?}.mp4"));
        render(&project(&red, &blue, kind), &out);
        let frames: &[i64] = if kind == TransitionKind::CircleOpen {
            &[87, 88, 90]
        } else {
            &[82, 90, 97]
        };
        for &frame in frames {
            let rgb = frame_rgb(&out, frame);
            let measured = match kind {
                TransitionKind::WipeLeft | TransitionKind::SlideLeft => {
                    horizontal_edge(&rgb, true).boundary
                }
                TransitionKind::WipeRight | TransitionKind::SlideRight => {
                    horizontal_edge(&rgb, false).boundary
                }
                TransitionKind::WipeUp => vertical_edge(&rgb, true).boundary,
                TransitionKind::WipeDown => vertical_edge(&rgb, false).boundary,
                _ => circle_radius(&rgb) / HALF_DIAGONAL,
            };
            entries.push(serde_json::json!({
                "kind": format!("{kind:?}"),
                "progress": progress(frame),
                "measured": measured,
                "unit": if kind == TransitionKind::CircleOpen { "halfDiagonal" } else { "frame" },
            }));
        }
    }
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/components/__fixtures__");
    fs::create_dir_all(&target).unwrap();
    fs::write(
        target.join("transitionGolden.json"),
        serde_json::to_string_pretty(&entries).unwrap(),
    )
    .unwrap();
    fs::remove_dir_all(dir).unwrap();
}
