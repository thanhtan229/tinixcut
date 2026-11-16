use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
};
use tinixcut_lib::modules::media::silence::{analyze, SilenceConfig};
fn directory() -> PathBuf {
    let d = std::env::temp_dir().join(format!("tinix-silence-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&d).unwrap();
    d
}
fn wav(path: &std::path::Path, seconds: u32, mode: u8) {
    let rate = 16000u32;
    let mut bytes = Vec::new();
    let size = seconds * rate * 4;
    bytes.extend(b"RIFF");
    bytes.extend((36 + size).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend(rate.to_le_bytes());
    bytes.extend((rate * 4).to_le_bytes());
    bytes.extend(4u16.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(size.to_le_bytes());
    for n in 0..seconds * rate {
        let quiet = mode == 0 || (mode == 1 && n >= rate && n < rate * 2);
        let value = if quiet {
            0
        } else {
            if n % 16 < 8 {
                1000i16
            } else {
                -1000i16
            }
        };
        bytes.extend(value.to_le_bytes());
        bytes.extend((-value).to_le_bytes());
    }
    fs::write(path, bytes).unwrap();
}
fn config() -> SilenceConfig {
    SilenceConfig {
        threshold_dbfs: -40.0,
        minimum_silence_ms: 400,
        padding_ms: 100,
    }
}
#[test]
fn real_ffmpeg_decode_preserves_antiphase_stereo_and_finds_only_silent_pause() {
    let d = directory();
    let source = d.join("voice.wav");
    wav(&source, 3, 1);
    let before = fs::read(&source).unwrap();
    let progress = Arc::new(AtomicU64::new(0));
    let report = analyze(
        &source,
        config(),
        Arc::new(AtomicBool::new(false)),
        progress.clone(),
    )
    .unwrap();
    assert_eq!(report.duration_micros, 3_000_000);
    assert_eq!(report.cuts.len(), 1);
    assert_eq!(report.cuts[0].start_micros, 1_100_000);
    assert_eq!(report.cuts[0].end_micros, 1_900_000);
    assert!(progress.load(Ordering::Relaxed) > 0);
    assert_eq!(fs::read(&source).unwrap(), before);
    if let Ok(output) = std::env::var("TINIX_SILENCE_EVIDENCE") {
        fs::write(output, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
    fs::remove_dir_all(d).unwrap();
}
#[test]
fn all_silent_non_silent_missing_and_cancellation_are_explicit() {
    let d = directory();
    let source = d.join("silence.wav");
    wav(&source, 2, 0);
    let run = |p: &std::path::Path, cancel| {
        analyze(
            p,
            config(),
            Arc::new(AtomicBool::new(cancel)),
            Arc::new(AtomicU64::new(0)),
        )
    };
    let report = run(&source, false).unwrap();
    assert_eq!(report.cuts.len(), 1);
    assert_eq!(report.cuts[0].start_micros, 100000);
    wav(&source, 2, 2);
    assert!(run(&source, false).unwrap().cuts.is_empty());
    assert!(run(&source, true).unwrap_err().contains("cancelled"));
    assert!(run(&d.join("missing.wav"), false).is_err());
    fs::write(&source, b"bad wav").unwrap();
    assert!(run(&source, false).is_err());
    fs::remove_dir_all(d).unwrap();
}
#[test]
fn cancellation_after_decode_progress_stops_owned_decoder() {
    let d = directory();
    let source = d.join("long.wav");
    wav(&source, 120, 2);
    let cancel = Arc::new(AtomicBool::new(false));
    let progress = Arc::new(AtomicU64::new(0));
    let worker_cancel = cancel.clone();
    let worker_progress = progress.clone();
    let worker =
        std::thread::spawn(move || analyze(&source, config(), worker_cancel, worker_progress));
    let started = std::time::Instant::now();
    while progress.load(Ordering::Relaxed) == 0
        && !worker.is_finished()
        && started.elapsed().as_secs() < 10
    {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(progress.load(Ordering::Relaxed) > 0);
    cancel.store(true, Ordering::SeqCst);
    let result = worker.join().unwrap();
    assert!(result.unwrap_err().contains("cancelled"));
    fs::remove_dir_all(d).unwrap();
}

#[test]
fn mono_level_video_audio_and_unsupported_streams() {
    use tinixcut_lib::modules::media::resolver::MediaBinaryResolver;
    let d = directory();
    let stereo = d.join("stereo.wav");
    wav(&stereo, 3, 1);
    let mono = d.join("mono.wav");
    wav(&mono, 2, 2);
    let bytes = fs::read(&mono).unwrap();
    let data: Vec<u8> = bytes[44..]
        .chunks_exact(4)
        .flat_map(|c| c[..2].iter().copied())
        .collect();
    let mut header = bytes[..44].to_vec();
    header[4..8].copy_from_slice(&(36 + data.len() as u32).to_le_bytes());
    header[22..24].copy_from_slice(&1u16.to_le_bytes());
    header[28..32].copy_from_slice(&32000u32.to_le_bytes());
    header[32..34].copy_from_slice(&2u16.to_le_bytes());
    header[40..44].copy_from_slice(&(data.len() as u32).to_le_bytes());
    header.extend(data);
    fs::write(&mono, header).unwrap();
    let cfg = SilenceConfig {
        threshold_dbfs: -32.0,
        ..config()
    };
    assert!(
        analyze(
            &mono,
            cfg,
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicU64::new(0))
        )
        .unwrap()
        .cuts
        .is_empty(),
        "Mono conversion must preserve source amplitude"
    );
    let video = d.join("voice.mp4");
    let mut command = MediaBinaryResolver::command("ffmpeg").unwrap();
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let output = command
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=black:size=32x32:rate=30:duration=3",
            "-i",
        ])
        .arg(&stereo)
        .args([
            "-map",
            "0:v",
            "-map",
            "1:a",
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
            "-shortest",
        ])
        .arg(&video)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = analyze(
        &video,
        config(),
        Arc::new(AtomicBool::new(false)),
        Arc::new(AtomicU64::new(0)),
    )
    .unwrap();
    assert_eq!(report.cuts.len(), 1);
    assert!(report.cuts[0].start_micros.abs_diff(1_100_000) < 40000);
    assert!(report.cuts[0].end_micros.abs_diff(1_900_000) < 40000);
    let no_audio = d.join("no-audio.mp4");
    let mut command = MediaBinaryResolver::command("ffmpeg").unwrap();
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    assert!(command
        .args(["-v", "error", "-i"])
        .arg(&video)
        .args(["-an", "-c:v", "copy"])
        .arg(&no_audio)
        .output()
        .unwrap()
        .status
        .success());
    assert!(analyze(
        &no_audio,
        config(),
        Arc::new(AtomicBool::new(false)),
        Arc::new(AtomicU64::new(0))
    )
    .unwrap_err()
    .contains("audio stream"));
    let offset_video = d.join("offset.mp4");
    let mut command = MediaBinaryResolver::command("ffmpeg").unwrap();
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    assert!(command
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=black:size=32x32:rate=30:duration=4",
            "-itsoffset",
            "0.5",
            "-i"
        ])
        .arg(&stereo)
        .args(["-map", "0:v", "-map", "1:a", "-c:v", "libx264", "-c:a", "aac"])
        .arg(&offset_video)
        .output()
        .unwrap()
        .status
        .success());
    let shifted = analyze(
        &offset_video,
        config(),
        Arc::new(AtomicBool::new(false)),
        Arc::new(AtomicU64::new(0)),
    )
    .unwrap();
    assert!(
        shifted
            .cuts
            .iter()
            .any(|c| c.start_micros.abs_diff(1_600_000) < 40000
                && c.end_micros.abs_diff(2_400_000) < 40000),
        "Audio offsets must stay on the video source clock: {:?}",
        shifted.cuts
    );
    fs::remove_dir_all(d).unwrap();
}
