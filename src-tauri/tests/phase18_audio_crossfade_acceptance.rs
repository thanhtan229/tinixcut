use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    Asset, AudioCrossfade, ItemKind, MediaKind, Project, TimelineItem, Track, TrackKind,
};
use tinixcut_lib::modules::export::{ExportEngine, ExportProfile};
use tinixcut_lib::modules::media::resolver::{BinaryKind, BinaryOrigin, MediaBinaryResolver};

fn resolved_tool(kind: BinaryKind) -> Command {
    let resolved =
        MediaBinaryResolver::resolve(kind).expect("app-owned FFmpeg tools must be available");
    assert!(
        matches!(resolved.origin, BinaryOrigin::AppOwnedPackagedResource(_)),
        "acceptance test must use app-owned FFmpeg resources, got {:?}",
        resolved.origin
    );
    if kind == BinaryKind::FFmpeg {
        assert!(
            resolved
                .version
                .as_deref()
                .is_some_and(|version| version.contains("9.0.2")),
            "acceptance test requires bundled FFmpeg 9.0.2"
        );
    }
    Command::new(resolved.path)
}

fn test_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("tinixcut_phase18_audio_crossfade");
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clear stale Phase 18 fixture directory");
    }
    fs::create_dir_all(&dir).expect("create Phase 18 fixture directory");
    dir
}

fn generate_tone(path: &Path, frequency: u32) {
    let output = resolved_tool(BinaryKind::FFmpeg)
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            &format!("sine=frequency={frequency}:duration=2.5:sample_rate=48000"),
            "-c:a",
            "pcm_s16le",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("generate deterministic tone fixture");
    assert!(
        output.status.success(),
        "tone generation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn goertzel_amplitude(samples: &[f32], start_seconds: f64, frequency: f64) -> f64 {
    let sample_rate = 48_000.0;
    let start = (start_seconds * sample_rate) as usize;
    let count = 4_800;
    let segment = &samples[start..start + count];
    let omega = 2.0 * std::f64::consts::PI * frequency / sample_rate;
    let coefficient = 2.0 * omega.cos();
    let (mut previous, mut previous_previous) = (0.0f64, 0.0f64);
    for &sample in segment {
        let current = f64::from(sample) + coefficient * previous - previous_previous;
        previous_previous = previous;
        previous = current;
    }
    ((previous * previous + previous_previous * previous_previous
        - coefficient * previous * previous_previous)
        .max(0.0))
    .sqrt()
        * 2.0
        / count as f64
}

#[test]
fn phase18_crossfade_export_preserves_duration_and_both_tones() {
    let dir = test_dir();
    let outgoing_path = dir.join("outgoing_440hz.wav");
    let incoming_path = dir.join("incoming_880hz.wav");
    generate_tone(&outgoing_path, 440);
    generate_tone(&incoming_path, 880);

    let rate = FrameRate::FPS_30;
    let mut project = Project::new("Phase 18 Audio Crossfade Export".to_string());
    project.settings.frame_rate = rate;
    project.settings.sample_rate = 48_000;
    project.assets.extend([
        Asset {
            asset_id: "asset_outgoing".to_string(),
            name: "Outgoing 440 Hz".to_string(),
            path: outgoing_path.to_string_lossy().to_string(),
            media_kind: MediaKind::Audio,
            duration: Some(RationalTime::new(75, rate)),
            file_size: 0,
        },
        Asset {
            asset_id: "asset_incoming".to_string(),
            name: "Incoming 880 Hz".to_string(),
            path: incoming_path.to_string_lossy().to_string(),
            media_kind: MediaKind::Audio,
            duration: Some(RationalTime::new(75, rate)),
            file_size: 0,
        },
    ]);

    let mut track = Track::new("Audio".to_string(), TrackKind::Audio);
    let outgoing = TimelineItem::new(
        Some("asset_outgoing".to_string()),
        "Outgoing".to_string(),
        ItemKind::Audio,
        TimeRange::new(RationalTime::new(0, rate), RationalTime::new(60, rate)),
        TimeRange::new(RationalTime::new(0, rate), RationalTime::new(60, rate)),
    )
    .unwrap();
    let incoming = TimelineItem::new(
        Some("asset_incoming".to_string()),
        "Incoming".to_string(),
        ItemKind::Audio,
        TimeRange::new(RationalTime::new(60, rate), RationalTime::new(60, rate)),
        TimeRange::new(RationalTime::new(15, rate), RationalTime::new(60, rate)),
    )
    .unwrap();
    track.audio_transitions.push(AudioCrossfade {
        transition_id: "audio_xfade_export".to_string(),
        track_id: track.track_id.clone(),
        left_item_id: outgoing.item_id.clone(),
        right_item_id: incoming.item_id.clone(),
        duration: RationalTime::new(30, rate),
    });
    track.items.extend([outgoing, incoming]);
    project.tracks.push(track);
    project
        .validate()
        .expect("fixture crossfade has verified handles");

    let output_path = dir.join("crossfade.mp4");
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
    .expect("actual ExportEngine crossfade render");
    assert!((report.duration_secs - 4.0).abs() < 0.001);

    let decoded_path = dir.join("decoded_audio.f32");
    let decoded = resolved_tool(BinaryKind::FFmpeg)
        .args([
            "-y",
            "-i",
            output_path.to_str().unwrap(),
            "-map",
            "0:a:0",
            "-ac",
            "1",
            "-ar",
            "48000",
            "-f",
            "f32le",
            decoded_path.to_str().unwrap(),
        ])
        .output()
        .expect("decode exported audio");
    assert!(
        decoded.status.success(),
        "audio decode failed: {}",
        String::from_utf8_lossy(&decoded.stderr)
    );
    let bytes = fs::read(decoded_path).expect("read decoded samples");
    assert_eq!(bytes.len() % 4, 0);
    let samples: Vec<f32> = bytes
        .chunks_exact(4)
        .map(|sample| f32::from_le_bytes(sample.try_into().unwrap()))
        .collect();

    let decoded_duration = samples.len() as f64 / 48_000.0;
    assert!(
        (decoded_duration - 4.0).abs() <= 0.03,
        "crossfade export must retain the exact 4-second project extent within AAC padding; decoded {decoded_duration:.3}s"
    );

    let outgoing_only = goertzel_amplitude(&samples, 0.5, 440.0);
    let incoming_only_leak = goertzel_amplitude(&samples, 0.5, 880.0);
    assert!(
        outgoing_only > incoming_only_leak * 8.0,
        "before the transition the outgoing tone must dominate ({outgoing_only:.4} vs {incoming_only_leak:.4})"
    );

    let start_out = goertzel_amplitude(&samples, 1.51, 440.0);
    let start_in = goertzel_amplitude(&samples, 1.51, 880.0);
    assert!(
        start_out > start_in * 2.0,
        "outgoing tone must dominate near the transition start ({start_out:.4} vs {start_in:.4})"
    );

    let middle_out = goertzel_amplitude(&samples, 1.98, 440.0);
    let middle_in = goertzel_amplitude(&samples, 1.98, 880.0);
    assert!(
        middle_out > 0.015 && middle_in > 0.015,
        "both sources must contribute at the transition midpoint ({middle_out:.4}, {middle_in:.4})"
    );

    let end_out = goertzel_amplitude(&samples, 2.45, 440.0);
    let end_in = goertzel_amplitude(&samples, 2.45, 880.0);
    assert!(
        end_in > end_out * 2.0,
        "incoming tone must dominate near the transition end ({end_in:.4} vs {end_out:.4})"
    );

    let incoming_only = goertzel_amplitude(&samples, 3.5, 880.0);
    let outgoing_leak = goertzel_amplitude(&samples, 3.5, 440.0);
    assert!(
        incoming_only > outgoing_leak * 8.0,
        "after the transition the incoming tone must dominate ({incoming_only:.4} vs {outgoing_leak:.4})"
    );
}
