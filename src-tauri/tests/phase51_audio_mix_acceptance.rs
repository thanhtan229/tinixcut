use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    Asset, ItemKind, Keyframe, KeyframeProperty, MediaKind, Project, TimelineItem, Track, TrackKind,
};
use tinixcut_lib::modules::export::{ExportEngine, ExportProfile};
use tinixcut_lib::modules::media::resolver::{BinaryKind, BinaryOrigin, MediaBinaryResolver};

fn media_tool(kind: BinaryKind) -> Command {
    let resolved = MediaBinaryResolver::resolve(kind).expect("app-owned media tool must exist");
    assert!(
        matches!(resolved.origin, BinaryOrigin::AppOwnedPackagedResource(_)),
        "acceptance must use app-owned tool, got {:?}",
        resolved.origin
    );
    Command::new(resolved.path)
}

fn run(command: &mut Command, description: &str) {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("{description}: {error}"));
    assert!(
        output.status.success(),
        "{description}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn rms(samples: &[f32], channels: usize, start: f64, end: f64) -> f64 {
    let from = (start * 48_000.0) as usize * channels;
    let to = ((end * 48_000.0) as usize * channels).min(samples.len());
    assert!(from < to, "RMS range must overlap decoded audio");
    (samples[from..to]
        .iter()
        .map(|sample| f64::from(*sample).powi(2))
        .sum::<f64>()
        / (to - from) as f64)
        .sqrt()
}

fn decode_stereo(path: &Path, output_path: &Path) -> Vec<f32> {
    run(
        media_tool(BinaryKind::FFmpeg)
            .arg("-y")
            .args(["-v", "error", "-i"])
            .arg(path)
            .args(["-map", "0:a:0", "-ac", "2", "-ar", "48000", "-f", "f32le"])
            .arg(output_path),
        "decode exported audio to stereo float PCM",
    );
    fs::read(output_path)
        .expect("read decoded PCM")
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
        .collect()
}

fn temp_dir() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "tinixcut_phase51_mix_{}_{}",
        std::process::id(),
        nonce
    ));
    fs::create_dir_all(&path).expect("create isolated Phase 51 media directory");
    path
}

#[test]
fn phase51_voice_and_stereo_music_ducking_survives_mp4_export() {
    let dir = temp_dir();
    let voice_path = dir.join("voice.mp4");
    let music_path = dir.join("music.wav");
    run(
        media_tool(BinaryKind::FFmpeg)
            .args([
                "-y",
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=64x64:r=30:d=3",
            ])
            .args([
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000:duration=3",
            ])
            .args([
                "-filter:a",
                "volume=0.35",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
                "-b:a",
                "192k",
                "-shortest",
            ])
            .arg(&voice_path),
        "generate video voice fixture",
    );
    run(
        media_tool(BinaryKind::FFmpeg)
            .args([
                "-y",
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=220:sample_rate=48000:duration=3",
            ])
            .args(["-filter:a", "volume=0.35", "-ac", "2", "-c:a", "pcm_s16le"])
            .arg(&music_path),
        "generate stereo music fixture",
    );

    let rate = FrameRate::FPS_30;
    let full = TimeRange::new(RationalTime::new(0, rate), RationalTime::new(90, rate));
    let mut project = Project::new("Phase 51 Voice and Music Acceptance".to_string());
    project.settings.frame_rate = rate;
    project.settings.sample_rate = 48_000;
    project.settings.channels = 2;
    project.settings.width = 64;
    project.settings.height = 64;
    project.assets.extend([
        Asset {
            asset_id: "voice_asset".into(),
            name: "voice.mp4".into(),
            path: voice_path.to_string_lossy().into_owned(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(90, rate)),
            file_size: fs::metadata(&voice_path).unwrap().len(),
            dimensions: None,
        },
        Asset {
            asset_id: "music_asset".into(),
            name: "music.wav".into(),
            path: music_path.to_string_lossy().into_owned(),
            media_kind: MediaKind::Audio,
            duration: Some(RationalTime::new(90, rate)),
            file_size: fs::metadata(&music_path).unwrap().len(),
            dimensions: None,
        },
    ]);
    let mut video_track = Track::new("Voice video".into(), TrackKind::Video);
    let voice = TimelineItem::new(
        Some("voice_asset".into()),
        "Voice".into(),
        ItemKind::Video,
        full,
        full,
    )
    .unwrap();
    video_track.items.push(voice);
    let mut music_track = Track::new("Music".into(), TrackKind::Audio);
    let mut music = TimelineItem::new(
        Some("music_asset".into()),
        "Music".into(),
        ItemKind::Audio,
        full,
        full,
    )
    .unwrap();
    music.keyframes = vec![
        Keyframe::new(KeyframeProperty::Volume, RationalTime::new(0, rate), 1.0),
        Keyframe::new(KeyframeProperty::Volume, RationalTime::new(23, rate), 1.0),
        Keyframe::new(KeyframeProperty::Volume, RationalTime::new(30, rate), 0.25),
        Keyframe::new(KeyframeProperty::Volume, RationalTime::new(60, rate), 0.25),
        Keyframe::new(KeyframeProperty::Volume, RationalTime::new(75, rate), 1.0),
    ];
    music_track.items.push(music);
    project.tracks.extend([video_track, music_track]);
    project.validate().expect("fixture project is valid");

    let profile = ExportProfile {
        width: 64,
        height: 64,
        frame_rate: rate,
        audio_sample_rate: 48_000,
        ..Default::default()
    };
    let baseline_path = dir.join("baseline.mp4");
    let ducked_path = dir.join("ducked.mp4");
    let mut baseline = project.clone();
    baseline.tracks[1].items[0].keyframes.clear();
    ExportEngine::render_snapshot(&baseline, &baseline_path, &profile, None)
        .expect("render baseline export");
    ExportEngine::render_snapshot(&project, &ducked_path, &profile, None)
        .expect("render ducked export");

    let baseline_pcm = decode_stereo(&baseline_path, &dir.join("baseline.f32"));
    let ducked_pcm = decode_stereo(&ducked_path, &dir.join("ducked.f32"));
    assert_eq!(baseline_pcm.len() % 2, 0, "baseline must decode as stereo");
    assert_eq!(
        ducked_pcm.len() % 2,
        0,
        "ducked export must decode as stereo"
    );
    let expected_samples = 3 * 48_000 * 2;
    assert!(
        baseline_pcm.len().abs_diff(expected_samples) < 4_096,
        "unexpected baseline sample count: {}",
        baseline_pcm.len() / 2
    );
    assert!(
        ducked_pcm.len().abs_diff(expected_samples) < 4_096,
        "unexpected ducked sample count: {}",
        ducked_pcm.len() / 2
    );

    let baseline_before = rms(&baseline_pcm, 2, 0.1, 0.6);
    let ducked_before = rms(&ducked_pcm, 2, 0.1, 0.6);
    let baseline_during = rms(&baseline_pcm, 2, 1.2, 1.8);
    let ducked_during = rms(&ducked_pcm, 2, 1.2, 1.8);
    let baseline_after = rms(&baseline_pcm, 2, 2.7, 2.95);
    let ducked_after = rms(&ducked_pcm, 2, 2.7, 2.95);
    println!(
        "RMS ratios before={:.3}, during={:.3}, after={:.3}",
        ducked_before / baseline_before,
        ducked_during / baseline_during,
        ducked_after / baseline_after
    );
    assert!(
        (0.9..=1.1).contains(&(ducked_before / baseline_before)),
        "ducking must not change pre-voice mix"
    );
    assert!(
        ducked_during / baseline_during < 0.85,
        "voice interval should lower the music mix; ratio={:.3}",
        ducked_during / baseline_during
    );
    assert!(
        (0.85..=1.15).contains(&(ducked_after / baseline_after)),
        "music should recover after release; ratio={:.3}",
        ducked_after / baseline_after
    );

    let probe =
        MediaBinaryResolver::resolve(BinaryKind::FFprobe).expect("app-owned ffprobe must exist");
    assert!(matches!(
        probe.origin,
        BinaryOrigin::AppOwnedPackagedResource(_)
    ));
    let output = Command::new(probe.path)
        .args([
            "-v",
            "error",
            "-select_streams",
            "a:0",
            "-show_entries",
            "stream=codec_name,channels,sample_rate",
            "-of",
            "default=noprint_wrappers=1",
        ])
        .arg(&ducked_path)
        .output()
        .expect("probe exported audio stream");
    assert!(
        output.status.success(),
        "ffprobe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stream = String::from_utf8_lossy(&output.stdout);
    assert!(
        stream.contains("codec_name=aac"),
        "expected AAC output: {stream}"
    );
    assert!(
        stream.contains("channels=2"),
        "expected stereo output: {stream}"
    );
    assert!(
        stream.contains("sample_rate=48000"),
        "expected 48 kHz output: {stream}"
    );
}
