//! Freeze-frame extraction measured on a real file: the source's red level is proportional to
//! its own time (`255 * T / 6`), so the extracted PNG says which source instant it shows.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tinixcut_lib::modules::media::ffmpeg::FFmpegService;
use tinixcut_lib::modules::media::resolver::{BinaryKind, MediaBinaryResolver};

fn ffmpeg() -> Command {
    Command::new(
        MediaBinaryResolver::resolve(BinaryKind::FFmpeg)
            .expect("bundled FFmpeg")
            .path,
    )
}

fn red_level(png: &PathBuf) -> f64 {
    let output = ffmpeg()
        .args(["-v", "error", "-i"])
        .arg(png)
        .args(["-vf", "format=rgb24,crop=1:1:32:32", "-f", "rawvideo", "-"])
        .output()
        .unwrap();
    assert_eq!(output.stdout.len(), 3, "one rgb24 pixel");
    f64::from(output.stdout[0])
}

#[test]
fn freeze_frame_shows_the_requested_source_instant() {
    let dir = std::env::temp_dir().join(format!("tinixcut_freeze_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("src.mp4");
    let status = ffmpeg()
        .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
        .arg("color=c=black:s=64x64:r=30:d=6,format=rgb24,geq=r='255*T/6':g=0:b=0")
        .args(["-c:v", "libx264", "-crf", "10", "-pix_fmt", "yuv444p"])
        .arg(&source)
        .status()
        .unwrap();
    assert!(status.success());

    let early = dir.join("freeze").join("early.png");
    let late = dir.join("freeze").join("late.png");
    // Same millisecond offset the command layer adds so a frame boundary picks the right frame.
    FFmpegService::extract_freeze_frame(&source, 1.5 + 0.001, &early).unwrap();
    FFmpegService::extract_freeze_frame(&source, 4.5 + 0.001, &late).unwrap();

    let early_level = red_level(&early);
    let late_level = red_level(&late);
    assert!(
        (early_level - 255.0 * 1.5 / 6.0).abs() < 4.0,
        "1.5 s frame has red {early_level}"
    );
    assert!(
        (late_level - 255.0 * 4.5 / 6.0).abs() < 4.0,
        "4.5 s frame has red {late_level}"
    );
    // Negative control: the two instants must not give the same still.
    assert!((late_level - early_level).abs() > 80.0);

    assert!(FFmpegService::extract_freeze_frame(&dir.join("missing.mp4"), 1.0, &early).is_err());
    assert!(FFmpegService::extract_freeze_frame(&source, -1.0, &early).is_err());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn reverse_proxy_time_maps_to_the_mirrored_source_instant() {
    let dir = std::env::temp_dir().join(format!("tinixcut_revproxy_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("src.mp4");
    let status = ffmpeg()
        .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
        .arg("color=c=black:s=64x64:r=30:d=6,format=rgb24,geq=r='255*T/6':g=0:b=0")
        .args(["-c:v", "libx264", "-crf", "10", "-pix_fmt", "yuv444p"])
        .arg(&source)
        .status()
        .unwrap();
    assert!(status.success());

    // Source range [1 s, 4 s): proxy time t must show source instant 4 - t.
    let proxy = dir.join("rev.mp4");
    FFmpegService::generate_reverse_proxy(&source, 1.0, 3.0, &proxy, 64).unwrap();
    for (proxy_time, source_time) in [(0.5, 3.5), (1.5, 2.5), (2.5, 1.5)] {
        let still = dir.join(format!("p{proxy_time}.png"));
        FFmpegService::extract_freeze_frame(&proxy, proxy_time + 0.001, &still).unwrap();
        let level = red_level(&still);
        let expected = 255.0 * source_time / 6.0;
        assert!(
            (level - expected).abs() < 8.0,
            "proxy {proxy_time}s shows red {level}, expected {expected} (source {source_time}s)"
        );
        // Negative control: a forward (unmirrored) mapping would give a different level.
        let forward = 255.0 * (1.0 + proxy_time) / 6.0;
        assert!((level - forward).abs() > 8.0 || (expected - forward).abs() < 8.0);
    }
    assert!(FFmpegService::generate_reverse_proxy(&source, 1.0, 0.0, &proxy, 64).is_err());
    fs::remove_dir_all(dir).unwrap();
}
