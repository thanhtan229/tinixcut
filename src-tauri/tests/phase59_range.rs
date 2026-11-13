//! Phase 59E acceptance: exporting an In/Out range of the timeline.
//!
//! The source's red level is proportional to its own time (255 * T / 6), so pixels identify
//! which instant of the timeline each exported frame shows.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    Asset, ItemKind, MediaKind, Project, TimelineItem, Track, TrackKind,
};
use tinixcut_lib::modules::export::{ExportEngine, ExportProfile, ExportRange};
use tinixcut_lib::modules::media::resolver::{BinaryKind, MediaBinaryResolver};

fn ffmpeg_path() -> PathBuf {
    MediaBinaryResolver::resolve(BinaryKind::FFmpeg)
        .expect("bundled FFmpeg")
        .path
}

fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tinixcut_p59e_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn gradient_source(path: &Path) {
    let status = Command::new(ffmpeg_path())
        .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
        .arg("color=c=black:s=64x64:r=30:d=6,format=rgb24,geq=r='255*T/6':g=0:b=0")
        .args([
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=6:sample_rate=48000",
        ])
        .args([
            "-c:v",
            "libx264",
            "-crf",
            "10",
            "-pix_fmt",
            "yuv444p",
            "-c:a",
            "aac",
            "-shortest",
        ])
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success());
}

fn project(source: &Path) -> Project {
    let rate = FrameRate::FPS_30;
    let mut project = Project::new("Range".to_string());
    project.settings.width = 64;
    project.settings.height = 64;
    project.settings.frame_rate = rate;
    project.assets.push(Asset {
        asset_id: "ast_src".into(),
        name: "src.mp4".into(),
        path: source.to_string_lossy().into_owned(),
        media_kind: MediaKind::Video,
        duration: Some(RationalTime::new(180, rate)),
        file_size: 1000,
        dimensions: None,
    });
    let range = TimeRange::new(RationalTime::new(0, rate), RationalTime::new(180, rate));
    let mut track = Track::new("V1".into(), TrackKind::Video);
    track.items.push(
        TimelineItem::new(
            Some("ast_src".into()),
            "clip".into(),
            ItemKind::Video,
            range,
            range,
        )
        .unwrap(),
    );
    project.tracks.push(track);
    project
}

fn profile(range: Option<ExportRange>) -> ExportProfile {
    ExportProfile {
        width: 64,
        height: 64,
        frame_rate: FrameRate::FPS_30,
        range,
        ..Default::default()
    }
}

fn probe(file: &Path, entry: &str) -> f64 {
    let ffprobe = MediaBinaryResolver::resolve(BinaryKind::FFprobe)
        .expect("ffprobe")
        .path;
    let output = Command::new(ffprobe)
        .args([
            "-v",
            "error",
            "-show_entries",
            entry,
            "-of",
            "default=nw=1:nk=1",
        ])
        .arg(file)
        .stdout(Stdio::piped())
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .and_then(|l| l.trim().parse().ok())
        .unwrap_or_else(|| panic!("no {entry}"))
}

fn red_at(file: &Path, seconds: f64) -> f64 {
    let output = Command::new(ffmpeg_path())
        .args(["-v", "error", "-ss"])
        .arg(format!("{seconds}"))
        .arg("-i")
        .arg(file)
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
        .output()
        .unwrap();
    assert_eq!(output.stdout.len(), 3);
    output.stdout[0] as f64
}

#[test]
fn p59e_range_export_has_the_exact_length_and_shows_the_selected_instants() {
    let dir = test_dir("range");
    let source = dir.join("gradient.mp4");
    gradient_source(&source);
    let project = project(&source);

    // Frames 60..120 = seconds 2..4 of the timeline.
    let out = dir.join("range.mp4");
    let report = ExportEngine::render_snapshot(
        &project,
        &out,
        &profile(Some(ExportRange {
            start_frame: 60,
            end_frame: 120,
        })),
        None,
    )
    .expect("range export succeeds");
    assert!((report.duration_secs - 2.0).abs() < 1e-6);
    let video_seconds = probe(&out, "stream=duration");
    assert!((video_seconds - 2.0).abs() < 0.1, "video {video_seconds}");

    // First frame shows source second 2, a later one second 3.5.
    assert!((red_at(&out, 0.05) - 255.0 * 2.05 / 6.0).abs() < 10.0);
    assert!((red_at(&out, 1.5) - 255.0 * 3.5 / 6.0).abs() < 10.0);

    // Negative control: without a range the same instant is second 1.5 of the timeline.
    let full = dir.join("full.mp4");
    ExportEngine::render_snapshot(&project, &full, &profile(None), None).unwrap();
    assert!((probe(&full, "format=duration") - 6.0).abs() < 0.1);
    assert!((red_at(&full, 1.5) - red_at(&out, 1.5)).abs() > 30.0);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn p59e_invalid_ranges_are_rejected_before_rendering() {
    let dir = test_dir("invalid");
    let source = dir.join("gradient.mp4");
    gradient_source(&source);
    let project = project(&source);
    let out = dir.join("never.mp4");
    for range in [
        ExportRange {
            start_frame: 90,
            end_frame: 90,
        },
        ExportRange {
            start_frame: 100,
            end_frame: 50,
        },
        ExportRange {
            start_frame: -5,
            end_frame: 30,
        },
        ExportRange {
            start_frame: 0,
            end_frame: 400,
        },
    ] {
        let result = ExportEngine::render_snapshot(&project, &out, &profile(Some(range)), None);
        assert!(result.is_err(), "{range:?} must be rejected");
        assert!(!out.exists());
    }
    fs::remove_dir_all(dir).unwrap();
}
