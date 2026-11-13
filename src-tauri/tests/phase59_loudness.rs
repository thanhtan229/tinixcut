//! Phase 59A acceptance: export loudness targets measured on real renders.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    Asset, ItemKind, MediaKind, Project, TimelineItem, Track, TrackKind,
};
use tinixcut_lib::modules::export::{ExportEngine, ExportProfile};
use tinixcut_lib::modules::media::loudness::{measure, LoudnessTarget};
use tinixcut_lib::modules::media::resolver::{BinaryKind, MediaBinaryResolver};

fn ffmpeg() -> Command {
    Command::new(
        MediaBinaryResolver::resolve(BinaryKind::FFmpeg)
            .expect("bundled FFmpeg")
            .path,
    )
}

fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tinixcut_p59a_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Speech-like test signal: a 220 Hz tone with a 3 Hz amplitude modulation, at `gain_db`.
fn make_audio(path: &Path, gain_db: f64, seconds: u32) {
    let status = ffmpeg()
        .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
        .arg(format!(
            "sine=frequency=220:duration={seconds}:sample_rate=48000"
        ))
        .args(["-af"])
        .arg(format!("tremolo=f=3:d=0.5,volume={gain_db}dB"))
        .args(["-c:a", "pcm_s16le"])
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success());
}

fn project_with_audio(audio: &Path, seconds: u32) -> Project {
    let rate = FrameRate::FPS_30;
    let mut project = Project::new("Loudness".to_string());
    project.settings.width = 64;
    project.settings.height = 64;
    project.settings.frame_rate = rate;
    project.assets.push(Asset {
        asset_id: "ast_audio".into(),
        name: "audio.wav".into(),
        path: audio.to_string_lossy().into_owned(),
        media_kind: MediaKind::Audio,
        duration: Some(RationalTime::new(seconds as i64 * 30, rate)),
        file_size: 1000,
        dimensions: None,
    });
    let mut track = Track::new("A1".into(), TrackKind::Audio);
    let range = TimeRange::new(
        RationalTime::new(0, rate),
        RationalTime::new(seconds as i64 * 30, rate),
    );
    track.items.push(
        TimelineItem::new(
            Some("ast_audio".into()),
            "clip".into(),
            ItemKind::Audio,
            range,
            range,
        )
        .unwrap(),
    );
    project.tracks.push(track);
    project
}

fn profile(target: Option<LoudnessTarget>) -> ExportProfile {
    ExportProfile {
        width: 64,
        height: 64,
        frame_rate: FrameRate::FPS_30,
        loudness: target,
        ..Default::default()
    }
}

fn render(project: &Project, out: &Path, target: Option<LoudnessTarget>) -> ExportReportLite {
    let report = ExportEngine::render_snapshot(project, out, &profile(target), None)
        .expect("export succeeds");
    ExportReportLite {
        loudness: report.loudness,
    }
}

struct ExportReportLite {
    loudness: Option<tinixcut_lib::modules::media::loudness::LoudnessOutcome>,
}

const YOUTUBE: LoudnessTarget = LoudnessTarget {
    integrated_lufs: -14.0,
    true_peak_db: -1.0,
};

#[test]
fn p59a_quiet_and_loud_mixes_both_land_on_the_target() {
    let dir = test_dir("targets");
    for (name, gain_db) in [("quiet", -30.0), ("loud", -3.0)] {
        let audio = dir.join(format!("{name}.wav"));
        make_audio(&audio, gain_db, 6);
        let project = project_with_audio(&audio, 6);

        let plain = dir.join(format!("{name}_plain.mp4"));
        render(&project, &plain, None);
        let before = measure(&plain).unwrap().expect("audio present");

        let normalized = dir.join(format!("{name}_norm.mp4"));
        let outcome = render(&project, &normalized, Some(YOUTUBE))
            .loudness
            .unwrap();
        let after = measure(&normalized).unwrap().expect("audio present");

        assert!(
            (after.integrated_lufs - -14.0).abs() <= 0.5,
            "{name}: {:.2} LUFS is not within 0.5 LU of -14 (before {:.2})",
            after.integrated_lufs,
            before.integrated_lufs
        );
        assert!(
            after.true_peak_db <= -1.0 + 0.2,
            "{name}: true peak {:.2} dB exceeds the limit",
            after.true_peak_db
        );
        // The report states what the file measures, not what was aimed for.
        assert!((outcome.output_lufs.unwrap() - after.integrated_lufs).abs() < 0.05);
        assert!((outcome.input_lufs.unwrap() - before.integrated_lufs).abs() < 0.5);
        // Negative control: the untouched render is nowhere near the target.
        assert!(
            (before.integrated_lufs - -14.0).abs() > 5.0,
            "{name}: control render is already near the target ({:.2})",
            before.integrated_lufs
        );
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn p59a_off_leaves_the_render_untouched_and_other_targets_work() {
    let dir = test_dir("off");
    let audio = dir.join("a.wav");
    make_audio(&audio, -20.0, 5);
    let project = project_with_audio(&audio, 5);

    let off = dir.join("off.mp4");
    let report = render(&project, &off, None);
    assert!(report.loudness.is_none(), "no target, no loudness report");

    let ebu = LoudnessTarget {
        integrated_lufs: -23.0,
        true_peak_db: -1.0,
    };
    let out = dir.join("ebu.mp4");
    render(&project, &out, Some(ebu));
    let measured = measure(&out).unwrap().unwrap();
    assert!(
        (measured.integrated_lufs - -23.0).abs() <= 0.5,
        "{measured:?}"
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn p59a_a_mix_already_on_target_is_not_reencoded_and_silence_is_not_amplified() {
    let dir = test_dir("skip");
    let audio = dir.join("a.wav");
    make_audio(&audio, -8.0, 5);
    let project = project_with_audio(&audio, 5);
    let plain = dir.join("plain.mp4");
    render(&project, &plain, None);
    let measured = measure(&plain).unwrap().unwrap();

    // Ask for exactly what the render already measures: nothing should change.
    let same = LoudnessTarget {
        integrated_lufs: measured.integrated_lufs as f32,
        true_peak_db: -1.0,
    };
    let out = dir.join("same.mp4");
    let outcome = render(&project, &out, Some(same)).loudness.unwrap();
    assert!(!outcome.normalized, "{outcome:?}");

    // Silence: never amplified.
    let silent = dir.join("silent.wav");
    make_audio(&silent, -120.0, 4);
    let silent_project = project_with_audio(&silent, 4);
    let silent_out = dir.join("silent.mp4");
    let silent_outcome = render(&silent_project, &silent_out, Some(YOUTUBE))
        .loudness
        .unwrap();
    assert!(!silent_outcome.normalized, "{silent_outcome:?}");
    assert!(silent_outcome.note.is_some());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn p59a_invalid_targets_are_rejected_before_rendering() {
    let dir = test_dir("invalid");
    let audio = dir.join("a.wav");
    make_audio(&audio, -20.0, 3);
    let project = project_with_audio(&audio, 3);
    let out = dir.join("never.mp4");
    let bad = LoudnessTarget {
        integrated_lufs: -2.0,
        true_peak_db: -1.0,
    };
    let result = ExportEngine::render_snapshot(&project, &out, &profile(Some(bad)), None);
    assert!(result.is_err());
    assert!(!out.exists());
    fs::remove_dir_all(dir).unwrap();
}
