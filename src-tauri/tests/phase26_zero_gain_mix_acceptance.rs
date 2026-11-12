use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    Asset, AssetDimensions, ItemKind, MediaKind, Project, TimelineItem, Track, TrackKind,
};
use tinixcut_lib::modules::export::{ExportEngine, ExportProfile};
use tinixcut_lib::modules::media::resolver::{BinaryKind, BinaryOrigin, MediaBinaryResolver};

fn ffmpeg() -> Command {
    let resolved = MediaBinaryResolver::resolve(BinaryKind::FFmpeg)
        .expect("app-owned FFmpeg must be available for export acceptance");
    assert!(
        matches!(resolved.origin, BinaryOrigin::AppOwnedPackagedResource(_)),
        "acceptance test must use app-owned FFmpeg, got {:?}",
        resolved.origin
    );
    Command::new(resolved.path)
}

fn decode_mono_f32(input: &Path, output: &Path) -> Vec<f32> {
    let decoded = ffmpeg()
        .args([
            "-y",
            "-v",
            "error",
            "-i",
            input.to_str().expect("UTF-8 test path"),
            "-map",
            "0:a:0",
            "-ac",
            "1",
            "-ar",
            "48000",
            "-f",
            "f32le",
            output.to_str().expect("UTF-8 test path"),
        ])
        .output()
        .expect("decode audio with app-owned FFmpeg");
    assert!(
        decoded.status.success(),
        "audio decode failed: {}",
        String::from_utf8_lossy(&decoded.stderr)
    );
    let bytes = fs::read(output).expect("read decoded audio samples");
    assert_eq!(bytes.len() % 4, 0);
    bytes
        .chunks_exact(4)
        .map(|sample| f32::from_le_bytes(sample.try_into().unwrap()))
        .collect()
}

fn rms(samples: &[f32]) -> f64 {
    assert!(!samples.is_empty(), "RMS requires decoded samples");
    (samples
        .iter()
        .map(|sample| f64::from(*sample) * f64::from(*sample))
        .sum::<f64>()
        / samples.len() as f64)
        .sqrt()
}

fn unique_test_dir() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "tinixcut_phase26_zero_gain_{}_{}",
        std::process::id(),
        nonce
    ));
    fs::create_dir_all(&dir).expect("create isolated export test directory");
    dir
}

#[test]
fn phase26_zero_gain_video_audio_does_not_reduce_wav_export_rms() {
    let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../artifacts/animated-text-templates/phase-26/media-qualification");
    let video_path = fixture_dir.join("phase26_test_video_audio.mp4");
    let wav_path = fixture_dir.join("phase26_test_audio.wav");
    assert!(
        video_path.is_file(),
        "missing video/audio fixture: {video_path:?}"
    );
    assert!(wav_path.is_file(), "missing WAV fixture: {wav_path:?}");

    let rate = FrameRate::FPS_30;
    let clip_range = TimeRange::new(RationalTime::new(0, rate), RationalTime::new(150, rate));
    let mut project = Project::new("Phase 26 Zero-Gain Audio Mix Acceptance".to_string());
    // Export dimensions must match the project canvas; keep the render small.
    project.settings.width = 64;
    project.settings.height = 64;
    project.settings.frame_rate = rate;
    project.settings.sample_rate = 48_000;
    project.assets.extend([
        Asset {
            asset_id: "video_with_silent_audio".to_string(),
            name: "Video with audio at zero gain".to_string(),
            path: video_path.to_string_lossy().to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(150, rate)),
            file_size: fs::metadata(&video_path).expect("stat video fixture").len(),
            dimensions: Some(AssetDimensions {
                width: 1280,
                height: 720,
            }),
        },
        Asset {
            asset_id: "standalone_wav".to_string(),
            name: "Standalone WAV".to_string(),
            path: wav_path.to_string_lossy().to_string(),
            media_kind: MediaKind::Audio,
            duration: Some(RationalTime::new(150, rate)),
            file_size: fs::metadata(&wav_path).expect("stat WAV fixture").len(),
            dimensions: None,
        },
    ]);

    let mut video_track = Track::new("Video".to_string(), TrackKind::Video);
    let mut video_item = TimelineItem::new(
        Some("video_with_silent_audio".to_string()),
        "Video at zero gain".to_string(),
        ItemKind::Video,
        clip_range,
        clip_range,
    )
    .expect("create video clip");
    video_item.volume = 0.0;
    video_track.items.push(video_item);

    let mut audio_track = Track::new("Audio".to_string(), TrackKind::Audio);
    let audio_item = TimelineItem::new(
        Some("standalone_wav".to_string()),
        "WAV at full gain".to_string(),
        ItemKind::Audio,
        clip_range,
        clip_range,
    )
    .expect("create WAV clip");
    audio_track.items.push(audio_item);
    project.tracks.extend([video_track, audio_track]);
    project.validate().expect("acceptance project is valid");

    let dir = unique_test_dir();
    let output_path = dir.join("zero_gain_mix.mp4");
    let report = ExportEngine::render_snapshot(
        &project,
        &output_path,
        &ExportProfile {
            width: 64,
            height: 64,
            frame_rate: rate,
            audio_sample_rate: 48_000,
            ..Default::default()
        },
        None,
    )
    .expect("render the project through ExportEngine");
    assert!((report.duration_secs - 5.0).abs() < 0.03);

    let source_samples = decode_mono_f32(&wav_path, &dir.join("source.f32"));
    let output_samples = decode_mono_f32(&output_path, &dir.join("export.f32"));
    let source_rms = rms(&source_samples);
    let output_rms = rms(&output_samples);
    let ratio = output_rms / source_rms;
    println!("WAV RMS: source={source_rms:.6}, exported={output_rms:.6}, ratio={ratio:.4}");
    assert!(
        (0.85..=1.15).contains(&ratio),
        "exported WAV RMS must remain within 15% of source; source={source_rms:.6}, output={output_rms:.6}, ratio={ratio:.4}"
    );

    fs::remove_dir_all(&dir).expect("remove temporary acceptance artifacts");
}
