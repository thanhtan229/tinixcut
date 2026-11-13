//! Phase 59C acceptance: custom rational speed and reverse measured on real renders.
//!
//! The source video's red level is proportional to its own time (`255 * T / 6` over 6 s), so a
//! pixel read from the export says which source instant is on screen at that timeline time.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    Asset, ClipSpeed, ItemKind, MediaKind, Project, TimelineItem, Track, TrackKind,
};
use tinixcut_lib::modules::export::{ExportEngine, ExportProfile};
use tinixcut_lib::modules::media::resolver::{BinaryKind, MediaBinaryResolver};

fn ffmpeg_path() -> PathBuf {
    MediaBinaryResolver::resolve(BinaryKind::FFmpeg)
        .expect("bundled FFmpeg")
        .path
}

fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tinixcut_p59c_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// 6 s source, red = 255 * T / 6, with a 440 Hz tone so the clip also has audio.
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

/// One clip on the timeline: `timeline_frames` long, starting at source 0, with `speed`.
fn project(source: &Path, speed: ClipSpeed, timeline_frames: i64, reverse: bool) -> Project {
    let rate = FrameRate::FPS_30;
    let mut project = Project::new("Speed".to_string());
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
    let source_frames = speed
        .source_duration_for(RationalTime::new(timeline_frames, rate), rate)
        .expect("exact source duration")
        .value;
    let mut track = Track::new("V1".into(), TrackKind::Video);
    let timeline = TimeRange::new(
        RationalTime::new(0, rate),
        RationalTime::new(timeline_frames, rate),
    );
    // `new` assumes 1x, so build it that way and then apply the speed and its source range.
    let mut item = TimelineItem::new(
        Some("ast_src".into()),
        "clip".into(),
        ItemKind::Video,
        timeline,
        timeline,
    )
    .unwrap();
    item.source_range = TimeRange::new(
        RationalTime::new(0, rate),
        RationalTime::new(source_frames, rate),
    );
    item.speed = speed;
    item.reverse = reverse;
    item.validate().expect("valid item");
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

/// Red level (0..255) of the centre pixel at `seconds` into the export.
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
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert_eq!(
        output.stdout.len(),
        3,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout[0] as f64
}

fn expected_red(source_seconds: f64) -> f64 {
    255.0 * source_seconds / 6.0
}

fn assert_close(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= 9.0,
        "{what}: red {actual:.1}, expected {expected:.1}"
    );
}

#[test]
fn p59c_custom_speeds_show_the_expected_source_instant() {
    let dir = test_dir("custom");
    let source = dir.join("gradient.mp4");
    gradient_source(&source);

    // 3/2x: 90 timeline frames consume 135 source frames, so source time = 1.5 * t.
    let fast = ClipSpeed::from_ratio(3, 2).unwrap();
    assert_eq!(
        fast,
        ClipSpeed::Custom {
            numerator: 3,
            denominator: 2
        }
    );
    let out = dir.join("fast.mp4");
    let duration = render(&project(&source, fast, 90, false), &out);
    assert!(
        (duration - 3.0).abs() < 1e-6,
        "export length stays the timeline length"
    );
    for t in [0.5, 1.5, 2.5] {
        assert_close(
            red_at(&out, t),
            expected_red(1.5 * t),
            &format!("3/2x at {t}s"),
        );
    }

    // 1/3x: 90 timeline frames consume 30 source frames, so source time = t / 3.
    let slow = ClipSpeed::from_ratio(1, 3).unwrap();
    let out = dir.join("slow.mp4");
    render(&project(&source, slow, 90, false), &out);
    for t in [0.5, 1.5, 2.5] {
        assert_close(
            red_at(&out, t),
            expected_red(t / 3.0),
            &format!("1/3x at {t}s"),
        );
    }

    // Negative control: a 1/3x clip is nowhere near where 1x would be.
    assert!((red_at(&out, 2.5) - expected_red(2.5)).abs() > 40.0);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn p59c_the_slowest_speed_chains_audio_tempo_and_keeps_the_duration() {
    let dir = test_dir("eighth");
    let source = dir.join("gradient.mp4");
    gradient_source(&source);
    // 1/8x: 24 timeline frames consume 3 source frames.
    let project = project(&source, ClipSpeed::from_ratio(1, 8).unwrap(), 24, false);
    let out = dir.join("eighth.mp4");
    let duration = render(&project, &out);
    assert!((duration - 0.8).abs() < 1e-6);
    assert!(out.exists());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn p59c_reverse_plays_the_source_range_backwards() {
    let dir = test_dir("reverse");
    let source = dir.join("gradient.mp4");
    gradient_source(&source);

    // 3 s of source read backwards: at timeline t the source instant is 3 - t.
    let out = dir.join("reverse.mp4");
    render(&project(&source, ClipSpeed::Normal, 90, true), &out);
    for t in [0.4, 1.5, 2.5] {
        assert_close(
            red_at(&out, t),
            expected_red(3.0 - t),
            &format!("reverse at {t}s"),
        );
    }

    // Negative control: without reverse the same time reads the opposite end.
    let forward = dir.join("forward.mp4");
    render(&project(&source, ClipSpeed::Normal, 90, false), &forward);
    assert!((red_at(&forward, 0.4) - red_at(&out, 0.4)).abs() > 60.0);

    // Reverse composes with speed: 2x reverse of 6 s of source over 3 s.
    let both = dir.join("both.mp4");
    render(&project(&source, ClipSpeed::Double, 90, true), &both);
    assert_close(
        red_at(&both, 0.5),
        expected_red(6.0 - 2.0 * 0.5),
        "2x reverse at 0.5 s",
    );
    fs::remove_dir_all(dir).unwrap();
}
