//! Phase 59B acceptance: ducking measured on real renders.
//!
//! Music (220 Hz) sits under a voice-like tone (1 kHz) that plays from 2 s to 5 s. A band-pass
//! around 220 Hz isolates the music in the exported mix, so its level can be compared between a
//! render with ducking and the same render without it.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    Asset, Ducking, ItemKind, MediaKind, Project, TimelineItem, Track, TrackKind,
};
use tinixcut_lib::modules::export::{ExportEngine, ExportProfile};
use tinixcut_lib::modules::media::resolver::{BinaryKind, MediaBinaryResolver};

fn ffmpeg_path() -> PathBuf {
    MediaBinaryResolver::resolve(BinaryKind::FFmpeg)
        .expect("bundled FFmpeg")
        .path
}

fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tinixcut_p59b_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn tone(path: &Path, frequency: u32, seconds: u32, gain_db: f64) {
    let status = Command::new(ffmpeg_path())
        .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
        .arg(format!(
            "sine=frequency={frequency}:duration={seconds}:sample_rate=48000"
        ))
        .args(["-af"])
        .arg(format!("volume={gain_db}dB"))
        .args(["-c:a", "pcm_s16le"])
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success());
}

fn project(music: &Path, voice: &Path, ducking: Option<Ducking>) -> Project {
    let rate = FrameRate::FPS_30;
    let mut project = Project::new("Ducking".to_string());
    project.settings.width = 64;
    project.settings.height = 64;
    project.settings.frame_rate = rate;
    for (id, path, seconds) in [("ast_music", music, 8), ("ast_voice", voice, 3)] {
        project.assets.push(Asset {
            asset_id: id.into(),
            name: id.into(),
            path: path.to_string_lossy().into_owned(),
            media_kind: MediaKind::Audio,
            duration: Some(RationalTime::new(seconds * 30, rate)),
            file_size: 1000,
            dimensions: None,
        });
    }
    let clips = [
        ("Music", "ast_music", 0i64, 8i64, ducking),
        ("Voice", "ast_voice", 60, 3, None),
    ];
    for (name, asset, start, seconds, duck) in clips {
        let mut track = Track::new(name.to_string(), TrackKind::Audio);
        let timeline = TimeRange::new(
            RationalTime::new(start, rate),
            RationalTime::new(seconds * 30, rate),
        );
        let source = TimeRange::new(
            RationalTime::new(0, rate),
            RationalTime::new(seconds * 30, rate),
        );
        let mut item = TimelineItem::new(
            Some(asset.into()),
            name.into(),
            ItemKind::Audio,
            timeline,
            source,
        )
        .unwrap();
        item.ducking = duck;
        track.items.push(item);
        project.tracks.push(track);
    }
    project
}

fn render(project: &Project, out: &Path) {
    let profile = ExportProfile {
        width: 64,
        height: 64,
        frame_rate: FrameRate::FPS_30,
        ..Default::default()
    };
    ExportEngine::render_snapshot(project, out, &profile, None).expect("export succeeds");
}

/// Mean level (dB) of the 220 Hz band between `start` and `start + length` seconds.
fn music_band_db(file: &Path, start: f64, length: f64) -> f64 {
    let output = Command::new(ffmpeg_path())
        .args(["-hide_banner", "-nostats", "-ss"])
        .arg(format!("{start}"))
        .args(["-t"])
        .arg(format!("{length}"))
        .arg("-i")
        .arg(file)
        .args([
            "-af",
            "bandpass=f=220:width_type=h:w=60,volumedetect",
            "-f",
            "null",
            "-",
        ])
        .output()
        .unwrap();
    let log = String::from_utf8_lossy(&output.stderr);
    log.lines()
        .find_map(|l| l.split("mean_volume:").nth(1))
        .and_then(|v| v.trim().trim_end_matches("dB").trim().parse().ok())
        .unwrap_or_else(|| panic!("no mean_volume in: {log}"))
}

fn ducking(depth_db: f32) -> Ducking {
    Ducking {
        depth_db,
        attack_ms: 30,
        release_ms: 300,
        threshold_db: -40.0,
    }
}

#[test]
fn p59b_music_ducks_by_the_configured_depth_only_while_the_voice_plays() {
    let dir = test_dir("depth");
    let music = dir.join("music.wav");
    let voice = dir.join("voice.wav");
    tone(&music, 220, 8, -12.0);
    tone(&voice, 1000, 3, -10.0);

    let plain = dir.join("plain.mp4");
    render(&project(&music, &voice, None), &plain);
    let ducked = dir.join("ducked.mp4");
    render(&project(&music, &voice, Some(ducking(12.0))), &ducked);

    // Before the voice, after it has gone and recovered: unchanged.
    for (start, label) in [(0.3, "before"), (6.4, "after")] {
        let difference = music_band_db(&ducked, start, 1.0) - music_band_db(&plain, start, 1.0);
        assert!(
            difference.abs() < 0.6,
            "{label} the voice: {difference:.2} dB"
        );
    }
    // While the voice plays: down by the depth.
    let during = music_band_db(&ducked, 2.6, 2.0) - music_band_db(&plain, 2.6, 2.0);
    assert!(
        (during - -12.0).abs() < 1.5,
        "during the voice the music should be 12 dB down, was {during:.2} dB"
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn p59b_depth_scales_and_no_ducking_setting_means_no_change() {
    let dir = test_dir("scale");
    let music = dir.join("music.wav");
    let voice = dir.join("voice.wav");
    tone(&music, 220, 8, -12.0);
    tone(&voice, 1000, 3, -10.0);

    let plain = dir.join("plain.mp4");
    render(&project(&music, &voice, None), &plain);
    let shallow = dir.join("shallow.mp4");
    render(&project(&music, &voice, Some(ducking(6.0))), &shallow);
    let deep = dir.join("deep.mp4");
    render(&project(&music, &voice, Some(ducking(15.0))), &deep);

    let reference = music_band_db(&plain, 2.6, 2.0);
    let shallow_db = music_band_db(&shallow, 2.6, 2.0) - reference;
    let deep_db = music_band_db(&deep, 2.6, 2.0) - reference;
    assert!(
        (shallow_db - -6.0).abs() < 1.5,
        "6 dB setting measured {shallow_db:.2}"
    );
    assert!(
        (deep_db - -15.0).abs() < 2.0,
        "15 dB setting measured {deep_db:.2}"
    );
    assert!(
        deep_db < shallow_db - 6.0,
        "negative control: deeper must be quieter"
    );

    // The same project twice without ducking is identical in the music band.
    let again = dir.join("again.mp4");
    render(&project(&music, &voice, None), &again);
    let drift = (music_band_db(&again, 2.6, 2.0) - reference).abs();
    assert!(drift < 0.1, "unducked renders differ by {drift:.3} dB");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn p59b_a_clip_alone_is_never_ducked() {
    // With no other audible clip there is no voice to duck under.
    let dir = test_dir("alone");
    let music = dir.join("music.wav");
    let voice = dir.join("voice.wav");
    tone(&music, 220, 8, -12.0);
    tone(&voice, 1000, 3, -10.0);
    let mut solo = project(&music, &voice, Some(ducking(24.0)));
    solo.tracks[1].muted = true; // mute the voice track
    let plain = dir.join("plain.mp4");
    let mut reference = project(&music, &voice, None);
    reference.tracks[1].muted = true;
    render(&reference, &plain);
    let out = dir.join("solo.mp4");
    render(&solo, &out);
    let difference = music_band_db(&out, 2.6, 2.0) - music_band_db(&plain, 2.6, 2.0);
    assert!(
        difference.abs() < 0.3,
        "ducked with no voice: {difference:.2} dB"
    );
    fs::remove_dir_all(dir).unwrap();
}
