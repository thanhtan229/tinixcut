//! Read-only, bounded audio-amplitude analysis. No speech recognition or playback.
use super::{ffmpeg::FFmpegService, resolver::MediaBinaryResolver};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::Path;
use std::process::Stdio;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

pub const SAMPLE_RATE: u64 = 16_000;
const WINDOW: u64 = 160;
pub const MAX_SECONDS: f64 = 1800.0;
const MAX_CUTS: usize = 2000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SilenceConfig {
    pub threshold_dbfs: f64,
    pub minimum_silence_ms: u32,
    pub padding_ms: u32,
}
impl SilenceConfig {
    pub fn validate(&self) -> Result<(), String> {
        if !self.threshold_dbfs.is_finite()
            || !(-80.0..=-10.0).contains(&self.threshold_dbfs)
            || !(100..=10000).contains(&self.minimum_silence_ms)
            || self.padding_ms > 2000
        {
            return Err(
                "Use threshold −80…−10 dBFS, minimum silence 100…10000 ms and padding 0…2000 ms"
                    .into(),
            );
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SilenceCut {
    pub start_micros: u64,
    pub end_micros: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SilenceAnalysis {
    pub source_fingerprint: String,
    pub duration_micros: u64,
    pub cuts: Vec<SilenceCut>,
    pub config: SilenceConfig,
    pub measurement: String,
}

struct Detector {
    config: SilenceConfig,
    threshold: f32,
    samples: u64,
    window_peak: f32,
    quiet_start: Option<u64>,
    cuts: Vec<SilenceCut>,
}
impl Detector {
    fn new(config: SilenceConfig) -> Self {
        Self {
            threshold: 10_f32.powf(config.threshold_dbfs as f32 / 20.0),
            config,
            samples: 0,
            window_peak: 0.0,
            quiet_start: None,
            cuts: Vec::new(),
        }
    }
    fn sample(&mut self, left: f32, right: f32) -> Result<(), String> {
        if !left.is_finite() || !right.is_finite() {
            return Err("Decoder produced non-finite audio samples".into());
        }
        self.window_peak = self.window_peak.max(left.abs()).max(right.abs());
        self.samples += 1;
        if self.samples % WINDOW == 0 {
            self.window(self.samples - WINDOW)?;
        }
        Ok(())
    }
    fn window(&mut self, start: u64) -> Result<(), String> {
        if self.window_peak <= self.threshold {
            self.quiet_start.get_or_insert(start);
        } else if let Some(first) = self.quiet_start.take() {
            self.add_cut(first, start)?;
        }
        self.window_peak = 0.0;
        Ok(())
    }
    fn add_cut(&mut self, first: u64, end: u64) -> Result<(), String> {
        let minimum = self.config.minimum_silence_ms as u64 * SAMPLE_RATE / 1000;
        let padding = self.config.padding_ms as u64 * SAMPLE_RATE / 1000;
        if end - first >= minimum && end - first > padding * 2 {
            if self.cuts.len() >= MAX_CUTS {
                return Err("Too many silence intervals; raise the minimum silence length".into());
            }
            self.cuts.push(SilenceCut {
                start_micros: (first + padding) * 1_000_000 / SAMPLE_RATE,
                end_micros: (end - padding) * 1_000_000 / SAMPLE_RATE,
            });
        }
        Ok(())
    }
    fn finish(mut self) -> Result<(u64, Vec<SilenceCut>), String> {
        if self.samples == 0 {
            return Err("No audio samples decoded".into());
        }
        if self.samples % WINDOW != 0 {
            self.window(self.samples - self.samples % WINDOW)?;
        }
        if let Some(first) = self.quiet_start.take() {
            self.add_cut(first, self.samples)?;
        }
        Ok((self.samples * 1_000_000 / SAMPLE_RATE, self.cuts))
    }
}

/// Stream PCM: constant decode buffer, bounded cut list; first audio stream only.
pub fn analyze(
    source: &Path,
    config: SilenceConfig,
    cancel: Arc<AtomicBool>,
    progress: Arc<AtomicU64>,
) -> Result<SilenceAnalysis, String> {
    config.validate()?;
    if cancel.load(Ordering::SeqCst) {
        return Err("Silence analysis cancelled".into());
    }
    let fingerprint = FFmpegService::source_fingerprint(source).map_err(|e| e.to_string())?;
    let channels = probe_audio(source, &cancel)?;
    let mut command = MediaBinaryResolver::command("ffmpeg").map_err(|e| e.to_string())?;
    command.args(["-v", "error", "-nostdin", "-i"]).arg(source);
    // PCM has no timestamps: explicitly retain initial audio offsets/gaps on
    // the media clock, and duplicate mono without an implicit pan-law gain.
    command.args([
        "-af",
        if channels == 1 {
            "aresample=16000:async=1:first_pts=0,pan=stereo|c0=c0|c1=c0"
        } else {
            "aresample=16000:async=1:first_pts=0"
        },
    ]);
    command
        .args([
            "-map", "0:a:0", "-vn", "-sn", "-dn", "-ar", "16000", "-ac", "2", "-t", "1800.01",
            "-f", "f32le", "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Audio decoder start failed: {e}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or("Audio decoder has no output pipe")?;
    let worker_config = config.clone();
    let worker_progress = progress.clone();
    let reader = std::thread::spawn(move || -> Result<(u64, Vec<SilenceCut>), String> {
        let mut input = std::io::BufReader::new(stdout);
        let mut sample = [0u8; 8];
        let mut detector = Detector::new(worker_config);
        loop {
            let mut n = 0;
            while n < 8 {
                match input.read(&mut sample[n..]) {
                    Ok(0) if n == 0 => return detector.finish(),
                    Ok(0) => return Err("Truncated audio sample".into()),
                    Ok(bytes) => n += bytes,
                    Err(e) => return Err(format!("Audio decode read failed: {e}")),
                }
            }
            detector.sample(
                f32::from_le_bytes(sample[..4].try_into().unwrap()),
                f32::from_le_bytes(sample[4..].try_into().unwrap()),
            )?;
            if detector.samples % 1600 == 0 {
                worker_progress.store(detector.samples, Ordering::Relaxed);
            }
            if detector.samples > (MAX_SECONDS as u64) * SAMPLE_RATE {
                return Err("Source exceeds the 30-minute analysis limit".into());
            }
        }
    });
    let started = Instant::now();
    let status = loop {
        if cancel.load(Ordering::SeqCst) || started.elapsed() > Duration::from_secs(120) {
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) => {
                    let stopped = child.kill().and_then(|_| child.wait());
                    break stopped.map_err(|e| format!("Could not stop owned audio decoder: {e}"));
                }
                Err(e) => break Err(format!("Audio decoder status failed: {e}")),
            }
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => {}
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(e.to_string());
            }
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let decoded = reader
        .join()
        .map_err(|_| "Audio analysis worker panicked")?;
    if cancel.load(Ordering::SeqCst) {
        return Err("Silence analysis cancelled".into());
    }
    if started.elapsed() > Duration::from_secs(120) {
        return Err("Audio analysis exceeded its 120-second time budget".into());
    }
    let (duration_micros, cuts) = decoded?;
    if !status?.success() {
        return Err(
            "Audio decode failed (missing/unsupported audio stream or decoder error)".into(),
        );
    }
    if FFmpegService::source_fingerprint(source).map_err(|e| e.to_string())? != fingerprint {
        return Err("Source changed during analysis".into());
    }
    Ok(SilenceAnalysis { source_fingerprint:fingerprint,duration_micros,cuts,config,
        measurement:"16 kHz stereo PCM, max absolute channel amplitude per 10 ms window; not speech detection".into() })
}

fn probe_audio(source: &Path, cancel: &AtomicBool) -> Result<u64, String> {
    let mut command = MediaBinaryResolver::command("ffprobe").map_err(|e| e.to_string())?;
    command
        .args([
            "-v",
            "error",
            "-select_streams",
            "a:0",
            "-show_entries",
            "stream=channels",
            "-of",
            "json",
        ])
        .arg(source)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Audio probe failed: {e}"))?;
    let mut pipe = child.stdout.take().ok_or("Missing audio probe output")?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        pipe.by_ref()
            .take(65536)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let start = Instant::now();
    let status = loop {
        if cancel.load(Ordering::SeqCst) || start.elapsed() > Duration::from_secs(10) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Err("Audio probe cancelled or exceeded 10 seconds".into());
        }
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err(e.to_string());
            }
        }
    };
    let bytes = reader
        .join()
        .map_err(|_| "Audio probe reader failed")?
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("Source could not be probed for audio".into());
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let channels = value["streams"][0]["channels"]
        .as_u64()
        .ok_or("Source has no supported audio stream")?;
    if !(1..=2).contains(&channels) {
        return Err("Silence analysis currently supports mono/stereo sources only".into());
    }
    Ok(channels)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn silence_job_retention_never_forgets_active_work() {
        use crate::modules::job::{JobKind, JobManager};
        let mut jobs = JobManager::new(1);
        jobs.submit_job(
            "silence".into(),
            JobKind::SilenceAnalysis,
            "source".into(),
            1,
        );
        assert!(!jobs.forget_terminal("silence"));
        jobs.start_job("silence").unwrap();
        assert!(!jobs.forget_terminal("silence"));
        jobs.request_cancel("silence");
        assert!(!jobs.forget_terminal("silence"));
        jobs.cancel_job("silence");
        assert!(jobs.forget_terminal("silence"));
        assert_eq!(jobs.running_count(), 0);
    }
    #[test]
    fn conservative_stereo_quiet_boundaries_and_padding() {
        let mut d = Detector::new(SilenceConfig {
            threshold_dbfs: -40.0,
            minimum_silence_ms: 300,
            padding_ms: 100,
        });
        for frame in 0..48000 {
            let v = if (16000..32000).contains(&frame) {
                0.0
            } else {
                0.02
            };
            d.sample(v, -v).unwrap();
        }
        let (duration, cuts) = d.finish().unwrap();
        assert_eq!(duration, 3_000_000);
        assert_eq!(
            cuts,
            vec![SilenceCut {
                start_micros: 1_100_000,
                end_micros: 1_900_000
            }]
        );
    }
    #[test]
    fn short_pauses_noise_and_low_speech_above_threshold_are_retained() {
        let mut d = Detector::new(SilenceConfig {
            threshold_dbfs: -50.0,
            minimum_silence_ms: 300,
            padding_ms: 0,
        });
        for n in 0..16000 {
            d.sample(if n < 3000 { 0.0 } else { 0.005 }, 0.0).unwrap();
        }
        assert!(d.finish().unwrap().1.is_empty());
    }
    #[test]
    fn no_audio_invalid_settings_and_all_silent_are_explicit() {
        let config = SilenceConfig {
            threshold_dbfs: -40.0,
            minimum_silence_ms: 300,
            padding_ms: 0,
        };
        assert!(Detector::new(config.clone()).finish().is_err());
        let mut d = Detector::new(config);
        for _ in 0..16000 {
            d.sample(0.0, 0.0).unwrap();
        }
        assert_eq!(d.finish().unwrap().1.len(), 1);
        assert!(SilenceConfig {
            threshold_dbfs: f64::NAN,
            minimum_silence_ms: 300,
            padding_ms: 0
        }
        .validate()
        .is_err());
    }
}
