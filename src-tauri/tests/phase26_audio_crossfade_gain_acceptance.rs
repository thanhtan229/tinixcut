use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    Asset, AudioCrossfade, ItemKind, MediaKind, Project, TimelineItem, Track, TrackKind,
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
        .expect("decode export with app-owned FFmpeg");
    assert!(
        decoded.status.success(),
        "audio decode failed: {}",
        String::from_utf8_lossy(&decoded.stderr)
    );
    fs::read(output)
        .expect("read decoded audio")
        .chunks_exact(4)
        .map(|sample| f32::from_le_bytes(sample.try_into().unwrap()))
        .collect()
}

fn rms_window(samples: &[f32], start_seconds: usize, end_seconds: usize) -> f64 {
    let start = start_seconds * 48_000;
    let end = (end_seconds * 48_000).min(samples.len());
    assert!(start < end, "requested RMS window is outside decoded audio");
    (samples[start..end]
        .iter()
        .map(|sample| f64::from(*sample) * f64::from(*sample))
        .sum::<f64>()
        / (end - start) as f64)
        .sqrt()
}

fn unique_test_dir() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "tinixcut_phase26_audio_crossfade_{}_{}",
        std::process::id(),
        nonce
    ));
    fs::create_dir_all(&dir).expect("create isolated export test directory");
    dir
}

#[test]
fn phase26_crossfade_envelopes_preserve_audio_level_outside_transition() {
    let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../artifacts/animated-text-templates/phase-26/media-qualification");
    let wav_path = fixture_dir.join("phase26_test_audio.wav");
    let video_path = fixture_dir.join("phase26_test_video_audio.mp4");
    assert!(wav_path.is_file(), "missing WAV fixture: {wav_path:?}");
    assert!(
        video_path.is_file(),
        "missing video fixture: {video_path:?}"
    );

    let rate = FrameRate::FPS_30;
    let cut_frame = 75;
    let clip_frames = 75;
    let asset_id = "crossfade_audio".to_string();
    let mut project = Project::new("Phase 26 Audio Crossfade Gain Acceptance".to_string());
    // Export dimensions must match the project canvas; keep the render small.
    project.settings.width = 64;
    project.settings.height = 64;
    project.settings.frame_rate = rate;
    project.settings.sample_rate = 48_000;
    project.assets.extend([
        Asset {
            asset_id: asset_id.clone(),
            name: "phase26_test_audio.wav".to_string(),
            path: wav_path.to_string_lossy().to_string(),
            media_kind: MediaKind::Audio,
            duration: Some(RationalTime::new(150, rate)),
            file_size: fs::metadata(&wav_path).expect("stat WAV fixture").len(),
            dimensions: None,
        },
        Asset {
            asset_id: "video_fixture".to_string(),
            name: "phase26_test_video_audio.mp4".to_string(),
            path: video_path.to_string_lossy().to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(150, rate)),
            file_size: fs::metadata(&video_path).expect("stat video fixture").len(),
            dimensions: None,
        },
    ]);

    let left_range = TimeRange::new(
        RationalTime::new(0, rate),
        RationalTime::new(clip_frames, rate),
    );
    let right_range = TimeRange::new(
        RationalTime::new(cut_frame, rate),
        RationalTime::new(clip_frames, rate),
    );
    let left = TimelineItem::new(
        Some(asset_id.clone()),
        "Left audio segment".to_string(),
        ItemKind::Audio,
        left_range,
        left_range,
    )
    .expect("create left audio segment");
    let right_source_range = TimeRange::new(
        RationalTime::new(cut_frame, rate),
        RationalTime::new(clip_frames, rate),
    );
    let right = TimelineItem::new(
        Some(asset_id),
        "Right audio segment".to_string(),
        ItemKind::Audio,
        right_range,
        right_source_range,
    )
    .expect("create right audio segment");
    let mut track = Track::new("Audio".to_string(), TrackKind::Audio);
    let transition_id = "test_audio_crossfade".to_string();
    track.audio_transitions.push(AudioCrossfade {
        transition_id,
        track_id: track.track_id.clone(),
        left_item_id: left.item_id.clone(),
        right_item_id: right.item_id.clone(),
        duration: RationalTime::new(30, rate),
    });
    track.items.extend([left, right]);
    let mut video_track = Track::new("Video".to_string(), TrackKind::Video);
    let video_range = TimeRange::new(RationalTime::new(0, rate), RationalTime::new(150, rate));
    video_track.items.push(
        TimelineItem::new(
            Some("video_fixture".to_string()),
            "Video background".to_string(),
            ItemKind::Video,
            video_range,
            video_range,
        )
        .expect("create video background"),
    );
    project.tracks.extend([video_track, track]);
    project.validate().expect("acceptance project is valid");

    let mut baseline = project.clone();
    baseline.tracks[1].audio_transitions.clear();
    let dir = unique_test_dir();
    let profile = ExportProfile {
        width: 64,
        height: 64,
        frame_rate: rate,
        audio_sample_rate: 48_000,
        ..Default::default()
    };
    let stereo_wav_path = dir.join("stereo_test_audio.wav");
    let stereo_wav = ffmpeg()
        .args([
            "-y",
            "-v",
            "error",
            "-i",
            wav_path.to_str().expect("UTF-8 fixture path"),
            "-ac",
            "2",
            "-ar",
            "48000",
            "-c:a",
            "pcm_s16le",
            stereo_wav_path.to_str().expect("UTF-8 test path"),
        ])
        .output()
        .expect("create stereo WAV fixture with app-owned FFmpeg");
    assert!(
        stereo_wav.status.success(),
        "stereo WAV fixture generation failed: {}",
        String::from_utf8_lossy(&stereo_wav.stderr)
    );

    let baseline_path = dir.join("baseline.mp4");
    let crossfade_path = dir.join("crossfade.mp4");
    ExportEngine::render_snapshot(&baseline, &baseline_path, &profile, None)
        .expect("render baseline project");
    ExportEngine::render_snapshot(&project, &crossfade_path, &profile, None)
        .expect("render crossfade project");

    let baseline_samples = decode_mono_f32(&baseline_path, &dir.join("baseline.f32"));
    let crossfade_samples = decode_mono_f32(&crossfade_path, &dir.join("crossfade.f32"));
    for (start, end) in [(1, 2), (3, 4)] {
        let baseline_rms = rms_window(&baseline_samples, start, end);
        let crossfade_rms = rms_window(&crossfade_samples, start, end);
        let ratio = crossfade_rms / baseline_rms;
        println!(
            "Outside crossfade {start}-{end}s RMS: baseline={baseline_rms:.6}, crossfade={crossfade_rms:.6}, ratio={ratio:.4}"
        );
        assert!(
            (0.9..=1.1).contains(&ratio),
            "crossfade must preserve steady audio level outside its interval; {start}-{end}s ratio={ratio:.4}"
        );
    }

    let mut stereo_project = project.clone();
    stereo_project.assets[0].path = stereo_wav_path.to_string_lossy().to_string();
    stereo_project.assets[0].name = "stereo_test_audio.wav".to_string();
    stereo_project.assets[0].file_size = fs::metadata(&stereo_wav_path)
        .expect("stat stereo WAV fixture")
        .len();
    let mut stereo_baseline = stereo_project.clone();
    stereo_baseline.tracks[1].audio_transitions.clear();
    let stereo_baseline_path = dir.join("stereo_baseline.mp4");
    let stereo_crossfade_path = dir.join("stereo_crossfade.mp4");
    ExportEngine::render_snapshot(&stereo_baseline, &stereo_baseline_path, &profile, None)
        .expect("render stereo baseline project");
    ExportEngine::render_snapshot(&stereo_project, &stereo_crossfade_path, &profile, None)
        .expect("render stereo crossfade project");
    let stereo_baseline_samples =
        decode_mono_f32(&stereo_baseline_path, &dir.join("stereo_baseline.f32"));
    let stereo_crossfade_samples =
        decode_mono_f32(&stereo_crossfade_path, &dir.join("stereo_crossfade.f32"));
    for (start, end) in [(1, 2), (3, 4)] {
        let baseline_rms = rms_window(&stereo_baseline_samples, start, end);
        let crossfade_rms = rms_window(&stereo_crossfade_samples, start, end);
        let ratio = crossfade_rms / baseline_rms;
        println!(
            "Stereo outside crossfade {start}-{end}s RMS: baseline={baseline_rms:.6}, crossfade={crossfade_rms:.6}, ratio={ratio:.4}"
        );
        assert!(
            (0.9..=1.1).contains(&ratio),
            "crossfade must preserve stereo audio level outside its interval; {start}-{end}s ratio={ratio:.4}"
        );
    }

    fs::remove_dir_all(&dir).expect("remove temporary acceptance artifacts");
}
