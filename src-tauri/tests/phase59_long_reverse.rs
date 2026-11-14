//! A reversed clip longer than one reverse piece is exported through scratch files; the result
//! must still show the mirrored source instant everywhere, including at the piece boundary.
//!
//! The source is 20 s long (two pieces: 15 s and 5 s) and its red level is `255 * T / 20`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    Asset, ItemKind, MediaKind, Project, TimelineItem, Track, TrackKind,
};
use tinixcut_lib::modules::export::{ExportEngine, ExportProfile};
use tinixcut_lib::modules::media::ffmpeg::{FFmpegService, ReverseOutput};
use tinixcut_lib::modules::media::resolver::{BinaryKind, MediaBinaryResolver};

const SOURCE_SECONDS: i64 = 20;

fn ffmpeg_path() -> PathBuf {
    MediaBinaryResolver::resolve(BinaryKind::FFmpeg)
        .expect("bundled FFmpeg")
        .path
}

fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tinixcut_p59r_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn gradient_source(path: &Path) {
    let status = Command::new(ffmpeg_path())
        .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
        .arg(format!(
            "color=c=black:s=64x64:r=30:d={SOURCE_SECONDS},format=rgb24,geq=r='255*T/{SOURCE_SECONDS}':g=0:b=0"
        ))
        .args(["-f", "lavfi", "-i"])
        .arg(format!(
            "sine=frequency=440:duration={SOURCE_SECONDS}:sample_rate=48000"
        ))
        .args(["-c:v", "libx264", "-crf", "10", "-pix_fmt", "yuv444p", "-c:a", "aac", "-shortest"])
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success());
}

fn project(source: &Path, reverse: bool) -> Project {
    let rate = FrameRate::FPS_30;
    let frames = SOURCE_SECONDS * 30;
    let mut project = Project::new("Long reverse".to_string());
    project.settings.width = 64;
    project.settings.height = 64;
    project.settings.frame_rate = rate;
    project.assets.push(Asset {
        asset_id: "ast_src".into(),
        name: "src.mp4".into(),
        path: source.to_string_lossy().into_owned(),
        media_kind: MediaKind::Video,
        duration: Some(RationalTime::new(frames, rate)),
        file_size: 1000,
        dimensions: None,
    });
    let range = TimeRange::new(RationalTime::new(0, rate), RationalTime::new(frames, rate));
    let mut item = TimelineItem::new(
        Some("ast_src".into()),
        "clip".into(),
        ItemKind::Video,
        range,
        range,
    )
    .unwrap();
    item.reverse = reverse;
    let mut track = Track::new("V1".into(), TrackKind::Video);
    track.items.push(item);
    project.tracks.push(track);
    project
}

fn render(project: &Project, out: &Path) -> f64 {
    let profile = ExportProfile {
        width: 64,
        height: 64,
        frame_rate: FrameRate::FPS_30,
        ..Default::default()
    };
    ExportEngine::render_snapshot(project, out, &profile, None)
        .expect("export succeeds")
        .duration_secs
}

fn red_at(file: &Path, seconds: f64) -> f64 {
    let output = Command::new(ffmpeg_path())
        .args(["-v", "error", "-ss"])
        .arg(format!("{seconds}"))
        .arg("-i")
        .arg(file)
        .args(["-vf", "format=rgb24,crop=1:1:32:32", "-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert_eq!(output.stdout.len(), 3, "{}", String::from_utf8_lossy(&output.stderr));
    f64::from(output.stdout[0])
}

fn expected_red(source_seconds: f64) -> f64 {
    255.0 * source_seconds / SOURCE_SECONDS as f64
}

#[test]
fn p59r_a_clip_reversed_in_pieces_shows_the_mirrored_source_instant() {
    let dir = test_dir("export");
    let source = dir.join("gradient.mp4");
    gradient_source(&source);
    let out = dir.join("reversed.mp4");
    let duration = render(&project(&source, true), &out);
    assert!(
        (duration - SOURCE_SECONDS as f64).abs() < 0.05,
        "export length {duration}"
    );
    // Output time t shows source time 20 - t. 4.9 s and 5.1 s straddle the piece boundary
    // (source second 15 is output second 5).
    for t in [0.5, 4.9, 5.1, 10.0, 15.5, 19.4] {
        let actual = red_at(&out, t);
        let expected = expected_red(SOURCE_SECONDS as f64 - t);
        assert!(
            (actual - expected).abs() <= 9.0,
            "t={t}: red {actual:.1}, expected {expected:.1}"
        );
    }
    // Negative control: forward playback would read the opposite end.
    assert!((red_at(&out, 0.5) - expected_red(0.5)).abs() > 150.0);
    // Scratch files are removed.
    let leftovers: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("tc_rev_"))
        .collect();
    assert!(leftovers.is_empty(), "scratch directory left behind");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn p59r_piecewise_reverse_matches_a_single_pass_frame_for_frame() {
    let dir = test_dir("pieces");
    let source = dir.join("gradient.mp4");
    gradient_source(&source);
    // A 12 s range is a single piece; 20 s is two. Both must put frame N where a reverse puts it.
    let single = dir.join("single.mkv");
    FFmpegService::render_reversed_range(&source, 2.0, 12.0, &single, ReverseOutput::Intermediate, None)
        .unwrap();
    let pieces = dir.join("pieces.mkv");
    FFmpegService::render_reversed_range(&source, 0.0, 20.0, &pieces, ReverseOutput::Intermediate, None)
        .unwrap();
    // Frame counts: 12 s and 20 s at 30 fps, no frame lost or repeated at the join.
    let count = |file: &Path| -> usize {
        let output = Command::new(ffmpeg_path())
            .args(["-v", "error", "-i"])
            .arg(file)
            .args(["-map", "0:v:0", "-f", "framemd5", "-"])
            .output()
            .unwrap();
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty())
            .count()
    };
    assert_eq!(count(&single), 360);
    assert_eq!(count(&pieces), 600);
    // Order across the join: the red level of the reversed gradient never increases, and it
    // still sweeps (almost) the whole 0..255 range, so no stretch was repeated or skipped.
    let output = Command::new(ffmpeg_path())
        .args(["-v", "error", "-i"])
        .arg(&pieces)
        .args(["-map", "0:v:0", "-vf", "format=rgb24,crop=1:1:32:32", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"])
        .output()
        .unwrap();
    let reds: Vec<u8> = output.stdout.chunks_exact(3).map(|pixel| pixel[0]).collect();
    assert_eq!(reds.len(), 600);
    assert!(
        reds.windows(2).all(|pair| u16::from(pair[1]) <= u16::from(pair[0]) + 2),
        "the reversed gradient must not climb back anywhere (join at frame 150)"
    );
    assert!(reds[0] > 245 && reds[599] < 10, "{} .. {}", reds[0], reds[599]);
    assert!(FFmpegService::render_reversed_range(&source, 0.0, 0.0, &single, ReverseOutput::Intermediate, None).is_err());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn p59r_long_preview_proxy_is_mirrored_across_the_piece_boundary() {
    let dir = test_dir("proxy");
    let source = dir.join("gradient.mp4");
    gradient_source(&source);
    let proxy = dir.join("proxy.mp4");
    FFmpegService::generate_reverse_proxy(&source, 0.0, 20.0, &proxy, 64).unwrap();
    // Proxy time t shows source instant 20 - t; 5 s is the join of the 15 s and 5 s pieces.
    for t in [0.5, 4.9, 5.1, 12.0, 19.0] {
        let actual = red_at(&proxy, t);
        let expected = expected_red(SOURCE_SECONDS as f64 - t);
        assert!(
            (actual - expected).abs() <= 10.0,
            "proxy {t}s: red {actual:.1}, expected {expected:.1}"
        );
    }
    // No scratch pieces remain next to the proxy.
    let names: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        names.iter().all(|name| !name.contains(".seg") && !name.contains("concat")),
        "{names:?}"
    );
    fs::remove_dir_all(dir).unwrap();
}
