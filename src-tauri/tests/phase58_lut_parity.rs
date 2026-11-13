//! Phase 58B acceptance: 3D LUT export semantics.
//!
//! 1. Filter level: FFmpeg `lut3d=interp=trilinear` must agree with the Rust reference sampler
//!    that the preview shader is specified against (layout, domain and interpolation).
//! 2. Export level: a LUT stored on a clip changes the rendered pixels in the expected
//!    direction, and a missing or changed LUT file fails explicitly instead of being skipped.
//! 3. Negative controls: a deliberately different LUT must fail the same comparison.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    Asset, ClipLut, ItemKind, MediaKind, Project, TimelineItem, Track, TrackKind,
};
use tinixcut_lib::modules::export::{ExportEngine, ExportProfile};
use tinixcut_lib::modules::media::lut::Lut3D;
use tinixcut_lib::modules::media::resolver::{BinaryKind, MediaBinaryResolver};

fn ffmpeg() -> Command {
    let resolved = MediaBinaryResolver::resolve(BinaryKind::FFmpeg).expect("bundled FFmpeg");
    Command::new(resolved.path)
}

fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tinixcut_p58_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn cube_text(lut: &Lut3D) -> String {
    let mut text = format!("TITLE \"{}\"\nLUT_3D_SIZE {}\n", lut.title, lut.size);
    for [r, g, b] in &lut.samples {
        text.push_str(&format!("{r:.8} {g:.8} {b:.8}\n"));
    }
    text
}

/// Non-trivial smooth LUT: gamma on each channel plus cross-channel mixing.
fn graded_lut(size: u32) -> Lut3D {
    let mut lut = Lut3D::identity(size);
    lut.title = "Graded".into();
    for s in &mut lut.samples {
        let [r, g, b] = *s;
        *s = [
            (0.75 * r.powf(0.8) + 0.25 * g).clamp(0.0, 1.0),
            (0.9 * g.powf(1.2) + 0.1 * b).clamp(0.0, 1.0),
            (0.8 * b + 0.2 * r * g).clamp(0.0, 1.0),
        ];
    }
    lut
}

fn swap_red_blue_lut(size: u32) -> Lut3D {
    let mut lut = Lut3D::identity(size);
    lut.title = "SwapRB".into();
    for s in &mut lut.samples {
        s.swap(0, 2);
    }
    lut
}

const W: usize = 64;
const H: usize = 64;

/// Deterministic RGB24 test pattern covering the whole input cube.
fn test_pattern() -> Vec<u8> {
    let mut data = Vec::with_capacity(W * H * 3);
    for y in 0..H {
        for x in 0..W {
            data.push((x * 255 / (W - 1)) as u8);
            data.push((y * 255 / (H - 1)) as u8);
            data.push((((x + y) * 255) / (W + H - 2)) as u8);
        }
    }
    data
}

fn run_ffmpeg_lut(dir: &Path, cube: &Path) -> Vec<u8> {
    let input = dir.join("pattern.rgb");
    fs::write(&input, test_pattern()).unwrap();
    // Forward slashes and an escaped drive colon, as the production filter builder does.
    let cube_arg = cube
        .to_string_lossy()
        .replace('\\', "/")
        .replace(':', "\\:");
    let output = ffmpeg()
        .args([
            "-v", "error", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", "64x64", "-i",
        ])
        .arg(&input)
        .args([
            "-vf",
            &format!("lut3d=file='{cube_arg}':interp=trilinear"),
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
    assert!(
        output.status.success(),
        "ffmpeg lut3d failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout.len(), W * H * 3);
    output.stdout
}

/// (mean absolute error, max absolute error) in 8-bit levels against the reference sampler.
fn compare_with_reference(lut: &Lut3D, rendered: &[u8]) -> (f64, f64) {
    let pattern = test_pattern();
    let (mut sum, mut max) = (0.0f64, 0.0f64);
    for (px, out) in pattern.chunks(3).zip(rendered.chunks(3)) {
        let expected = lut.sample_trilinear([
            px[0] as f32 / 255.0,
            px[1] as f32 / 255.0,
            px[2] as f32 / 255.0,
        ]);
        for c in 0..3 {
            let err = (expected[c] as f64 * 255.0 - out[c] as f64).abs();
            sum += err;
            max = max.max(err);
        }
    }
    (sum / (W * H * 3) as f64, max)
}

#[test]
fn p58b_ffmpeg_lut3d_matches_reference_trilinear_sampler() {
    let dir = test_dir("filter");
    for lut in [Lut3D::identity(9), graded_lut(17), swap_red_blue_lut(5)] {
        let cube = dir.join(format!("{}.cube", lut.title));
        fs::write(&cube, cube_text(&lut)).unwrap();
        let (mae, max) = compare_with_reference(&lut, &run_ffmpeg_lut(&dir, &cube));
        assert!(mae <= 1.0, "{}: MAE {mae:.3} exceeds 1.0 level", lut.title);
        assert!(
            max <= 3.0,
            "{}: max error {max:.1} exceeds 3 levels",
            lut.title
        );
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn p58b_negative_control_a_different_lut_fails_the_comparison() {
    let dir = test_dir("negative");
    let rendered_lut = graded_lut(17);
    let cube = dir.join("rendered.cube");
    fs::write(&cube, cube_text(&rendered_lut)).unwrap();
    let rendered = run_ffmpeg_lut(&dir, &cube);
    // The same output compared against another LUT's reference must be far outside tolerance.
    let (mae, _) = compare_with_reference(&swap_red_blue_lut(5), &rendered);
    assert!(
        mae > 8.0,
        "gate is insensitive: MAE {mae:.3} against the wrong LUT"
    );
    let (mae_identity, _) = compare_with_reference(&Lut3D::identity(9), &rendered);
    assert!(
        mae_identity > 4.0,
        "gate is insensitive to a missing grade: {mae_identity:.3}"
    );
    fs::remove_dir_all(dir).unwrap();
}

fn color_video(path: &Path, color: &str) {
    let status = ffmpeg()
        .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
        .arg(format!("color=c={color}:s=64x64:r=30:d=1"))
        .args(["-c:v", "libx264", "-pix_fmt", "yuv420p"])
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success());
}

fn project_with_clip(video: &Path, lut: Option<ClipLut>) -> Project {
    let rate = FrameRate::FPS_30;
    let mut project = Project::new("LUT export".to_string());
    project.settings.width = 64;
    project.settings.height = 64;
    project.settings.frame_rate = rate;
    project.assets.push(Asset {
        asset_id: "ast_v1".into(),
        name: "Color".into(),
        path: video.to_string_lossy().into_owned(),
        media_kind: MediaKind::Video,
        duration: None,
        file_size: 1000,
        dimensions: None,
    });
    let mut track = Track::new("V1".into(), TrackKind::Video);
    let range = TimeRange::new(RationalTime::new(0, rate), RationalTime::new(30, rate));
    let mut item = TimelineItem::new(
        Some("ast_v1".into()),
        "clip_v".into(),
        ItemKind::Video,
        range,
        range,
    )
    .unwrap();
    item.color_lut = lut;
    track.items.push(item);
    project.tracks.push(track);
    project
}

fn render_center_pixel(project: &Project, out: &Path) -> [u8; 3] {
    let profile = ExportProfile {
        width: 64,
        height: 64,
        frame_rate: FrameRate::FPS_30,
        ..Default::default()
    };
    ExportEngine::render_snapshot(project, out, &profile, None).expect("export succeeds");
    let output = ffmpeg()
        .args(["-v", "error", "-i"])
        .arg(out)
        .args([
            "-vf",
            "format=rgb24,crop=1:1:32:32",
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
        3,
        "one RGB pixel expected: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    [output.stdout[0], output.stdout[1], output.stdout[2]]
}

#[test]
fn p58b_export_applies_clip_lut_in_the_expected_direction() {
    let dir = test_dir("export");
    let video = dir.join("red.mp4");
    color_video(&video, "red");

    let plain = render_center_pixel(&project_with_clip(&video, None), &dir.join("plain.mp4"));
    assert!(
        plain[0] > 200 && plain[2] < 60,
        "plain red expected, got {plain:?}"
    );

    let cube = dir.join("swap.cube");
    fs::write(&cube, cube_text(&swap_red_blue_lut(5))).unwrap();
    let clip_lut = ClipLut::from_file(&cube).unwrap();
    let graded = render_center_pixel(
        &project_with_clip(&video, Some(clip_lut)),
        &dir.join("graded.mp4"),
    );
    assert!(
        graded[2] > 200 && graded[0] < 60,
        "swap-R/B LUT must turn red into blue, got {graded:?}"
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn p58b_missing_or_changed_lut_fails_explicitly() {
    let dir = test_dir("integrity");
    let video = dir.join("red.mp4");
    color_video(&video, "red");
    let cube = dir.join("grade.cube");
    fs::write(&cube, cube_text(&graded_lut(5))).unwrap();
    let clip_lut = ClipLut::from_file(&cube).unwrap();
    let project = project_with_clip(&video, Some(clip_lut));
    let profile = ExportProfile {
        width: 64,
        height: 64,
        frame_rate: FrameRate::FPS_30,
        ..Default::default()
    };
    let target = dir.join("out.mp4");

    // Content changed after it was pinned.
    fs::write(&cube, cube_text(&swap_red_blue_lut(5))).unwrap();
    let changed = ExportEngine::render_snapshot(&project, &target, &profile, None)
        .expect_err("a changed LUT must fail");
    assert!(
        changed.to_string().contains("changed since it was added"),
        "{changed}"
    );
    assert!(!target.exists(), "no output may be produced");

    // File gone.
    fs::remove_file(&cube).unwrap();
    let missing = ExportEngine::render_snapshot(&project, &target, &profile, None)
        .expect_err("a missing LUT must fail");
    assert!(missing.to_string().contains("is missing"), "{missing}");
    let preflight = ExportEngine::preflight(&project, &target, &profile)
        .expect_err("preflight reports the same problem");
    assert!(preflight.to_string().contains("is missing"), "{preflight}");
    fs::remove_dir_all(dir).unwrap();
}
