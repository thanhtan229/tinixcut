//! Audio-based clip alignment (proposals only).
//!
//! Given a reference clip and other clips that recorded the same event (several cameras, a
//! separate microphone), estimates how far each clip must move on the timeline so that their
//! audio lines up. The estimate is a normalized cross-correlation of 8 kHz mono audio, computed
//! with an FFT so a 60 s search window stays fast. Nothing here edits a project: callers apply
//! a proposal with the existing clip-move commands, and low-confidence results are reported
//! as such instead of being applied.
//!
//! The approach (decode to low-rate mono, cross-correlate, search a bounded window) follows the
//! OpenCut-AI multicam sync (MIT, see `THIRD_PARTY_NOTICES.md`); this is an independent Rust
//! implementation with an FFT instead of its O(n·lag) loop.

use crate::modules::domain::{ClipSpeed, ItemKind, Project};
use crate::modules::media::resolver::MediaBinaryResolver;
use schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};

pub const ALIGN_SAMPLE_RATE: u32 = 8000;
pub const DEFAULT_WINDOW_SECONDS: f64 = 60.0;
pub const MAX_WINDOW_SECONDS: f64 = 120.0;
/// Peaks below this normalized score are not trusted.
pub const MIN_CONFIDENT_SCORE: f64 = 0.10;
/// The best peak must beat the best unrelated peak by this factor to be trusted.
pub const MIN_PEAK_RATIO: f64 = 1.5;

#[derive(Clone, Copy, Debug, Default)]
struct Complex {
    re: f64,
    im: f64,
}

impl Complex {
    fn mul(self, other: Complex) -> Complex {
        Complex {
            re: self.re * other.re - self.im * other.im,
            im: self.re * other.im + self.im * other.re,
        }
    }
    fn conj(self) -> Complex {
        Complex {
            re: self.re,
            im: -self.im,
        }
    }
}

/// In-place iterative radix-2 FFT. `data.len()` must be a power of two.
fn fft(data: &mut [Complex], inverse: bool) {
    let n = data.len();
    debug_assert!(n.is_power_of_two());
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            data.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let angle = 2.0 * std::f64::consts::PI / len as f64 * if inverse { 1.0 } else { -1.0 };
        let step = Complex {
            re: angle.cos(),
            im: angle.sin(),
        };
        for start in (0..n).step_by(len) {
            let mut w = Complex { re: 1.0, im: 0.0 };
            for k in 0..len / 2 {
                let even = data[start + k];
                let odd = data[start + k + len / 2].mul(w);
                data[start + k] = Complex {
                    re: even.re + odd.re,
                    im: even.im + odd.im,
                };
                data[start + k + len / 2] = Complex {
                    re: even.re - odd.re,
                    im: even.im - odd.im,
                };
                w = w.mul(step);
            }
        }
        len <<= 1;
    }
    if inverse {
        for value in data.iter_mut() {
            value.re /= n as f64;
            value.im /= n as f64;
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Correlation {
    /// Samples by which `other` lags `reference`: other[n + lag] matches reference[n].
    pub lag_samples: i64,
    /// Peak value divided by sqrt(E_ref * E_other), roughly 0..1.
    pub score: f64,
    /// Peak divided by the best peak at least `exclusion` samples away (>= 1).
    pub peak_ratio: f64,
}

/// Normalized cross-correlation of two signals, searched within `max_lag` samples either way.
pub fn cross_correlate(reference: &[f32], other: &[f32], max_lag: usize) -> Option<Correlation> {
    if reference.is_empty() || other.is_empty() {
        return None;
    }
    let size = (reference.len() + other.len()).next_power_of_two();
    let mut a = vec![Complex::default(); size];
    let mut b = vec![Complex::default(); size];
    for (slot, v) in a.iter_mut().zip(reference) {
        slot.re = *v as f64;
    }
    for (slot, v) in b.iter_mut().zip(other) {
        slot.re = *v as f64;
    }
    fft(&mut a, false);
    fft(&mut b, false);
    for (x, y) in a.iter_mut().zip(&b) {
        *x = x.conj().mul(*y);
    }
    fft(&mut a, true);

    let energy = |signal: &[f32]| {
        signal
            .iter()
            .map(|v| (*v as f64) * (*v as f64))
            .sum::<f64>()
    };
    let norm = (energy(reference) * energy(other)).sqrt();
    if norm <= f64::EPSILON {
        return None;
    }

    // c[k] = sum_n reference[n] * other[n + k]; negative k wraps to size + k.
    let value_at = |lag: i64| -> f64 {
        let index = if lag >= 0 {
            lag as usize
        } else {
            (size as i64 + lag) as usize
        };
        a[index].re / norm
    };
    let max_lag = max_lag as i64;
    let lo = -(reference.len() as i64 - 1).min(max_lag);
    let hi = (other.len() as i64 - 1).min(max_lag);
    let mut best = (lo, f64::MIN);
    for lag in lo..=hi {
        let v = value_at(lag);
        if v > best.1 {
            best = (lag, v);
        }
    }
    // Strongest peak outside the main lobe (25 ms) measures how distinctive the match is.
    let exclusion = (ALIGN_SAMPLE_RATE / 40) as i64;
    let mut runner_up = f64::MIN;
    for lag in lo..=hi {
        if (lag - best.0).abs() > exclusion {
            runner_up = runner_up.max(value_at(lag));
        }
    }
    let peak_ratio = if runner_up > 0.0 {
        best.1 / runner_up
    } else {
        f64::INFINITY
    };
    Some(Correlation {
        lag_samples: best.0,
        score: best.1,
        peak_ratio,
    })
}

/// Mono f32 PCM at `ALIGN_SAMPLE_RATE` from `seconds` of a file starting at `start_seconds`.
pub fn decode_mono_pcm(
    path: &std::path::Path,
    start_seconds: f64,
    seconds: f64,
) -> Result<Vec<f32>, String> {
    let mut command: Command = MediaBinaryResolver::command("ffmpeg")
        .map_err(|e| format!("Failed to resolve FFmpeg: {e}"))?;
    let output = command
        .args(["-v", "error", "-ss", &format!("{start_seconds:.6}"), "-t"])
        .arg(format!("{seconds:.6}"))
        .arg("-i")
        .arg(path)
        .args(["-vn", "-ac", "1", "-ar"])
        .arg(ALIGN_SAMPLE_RATE.to_string())
        .args(["-f", "f32le", "-"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to run FFmpeg: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "FFmpeg could not decode audio from '{}': {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    if output.stdout.len() < 4 {
        return Err(format!("'{}' has no decodable audio", path.display()));
    }
    Ok(output
        .stdout
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AlignmentProposal {
    pub item_id: String,
    /// Timeline start (in project frames) that lines this clip's audio up with the reference.
    pub proposed_start_frame: i64,
    pub current_start_frame: i64,
    /// `proposed - current`; apply with a clip move of this many frames.
    pub delta_frames: i64,
    pub offset_seconds: f64,
    pub score: f64,
    pub peak_ratio: f64,
    /// False when the match is weak or ambiguous; such proposals must not be applied blindly.
    pub confident: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AlignmentReport {
    pub reference_item_id: String,
    pub window_seconds: f64,
    pub proposals: Vec<AlignmentProposal>,
}

struct ClipAudio {
    start_frame: i64,
    samples: Vec<f32>,
}

fn load_clip_audio(
    project: &Project,
    item_id: &str,
    window_seconds: f64,
) -> Result<ClipAudio, String> {
    let item = project
        .tracks
        .iter()
        .flat_map(|t| t.items.iter())
        .find(|i| i.item_id == item_id)
        .ok_or_else(|| format!("Clip '{item_id}' not found"))?;
    if !matches!(item.kind, ItemKind::Video | ItemKind::Audio) {
        return Err(format!("Clip '{item_id}' has no audio to align"));
    }
    if item.speed != ClipSpeed::Normal {
        return Err(format!(
            "Clip '{item_id}' is not at 1x speed; alignment would be wrong"
        ));
    }
    let asset = item
        .asset_id
        .as_ref()
        .and_then(|id| project.assets.iter().find(|a| &a.asset_id == id))
        .ok_or_else(|| format!("Clip '{item_id}' has no media asset"))?;
    let available = item.source_range.duration.to_seconds();
    let seconds = window_seconds.min(available).max(0.1);
    let samples = decode_mono_pcm(
        std::path::Path::new(&asset.path),
        item.source_range.start.to_seconds(),
        seconds,
    )?;
    Ok(ClipAudio {
        start_frame: item
            .timeline_range
            .start
            .rescaled_to(project.settings.frame_rate)
            .value,
        samples,
    })
}

/// Proposes timeline moves that align `item_ids` to `reference_item_id` by audio.
pub fn propose_alignment(
    project: &Project,
    reference_item_id: &str,
    item_ids: &[String],
    window_seconds: Option<f64>,
) -> Result<AlignmentReport, String> {
    let window = window_seconds
        .unwrap_or(DEFAULT_WINDOW_SECONDS)
        .clamp(1.0, MAX_WINDOW_SECONDS);
    if item_ids.is_empty() {
        return Err("Provide at least one clip to align".into());
    }
    let fps = project.settings.frame_rate.as_f64();
    let reference = load_clip_audio(project, reference_item_id, window)?;
    let max_lag = (window * ALIGN_SAMPLE_RATE as f64) as usize;

    let mut proposals = Vec::new();
    for item_id in item_ids {
        if item_id == reference_item_id {
            return Err("The reference clip cannot also be aligned".into());
        }
        let other = load_clip_audio(project, item_id, window)?;
        let correlation = cross_correlate(&reference.samples, &other.samples, max_lag)
            .ok_or_else(|| format!("Clip '{item_id}' or the reference is silent"))?;
        // other[n + lag] matches reference[n], so the other clip must start `lag` earlier
        // relative to the reference start (see module docs for the derivation).
        let lag_seconds = correlation.lag_samples as f64 / ALIGN_SAMPLE_RATE as f64;
        let proposed_start_frame = reference.start_frame - (lag_seconds * fps).round() as i64;
        let confident =
            correlation.score >= MIN_CONFIDENT_SCORE && correlation.peak_ratio >= MIN_PEAK_RATIO;
        let note = (!confident).then(|| {
            format!(
                "Weak or ambiguous match (score {:.3}, peak ratio {:.2}); do not apply without checking",
                correlation.score, correlation.peak_ratio
            )
        });
        proposals.push(AlignmentProposal {
            item_id: item_id.clone(),
            proposed_start_frame,
            current_start_frame: other.start_frame,
            delta_frames: proposed_start_frame - other.start_frame,
            offset_seconds: lag_seconds,
            score: correlation.score,
            peak_ratio: correlation.peak_ratio,
            confident,
            note,
        });
    }
    Ok(AlignmentReport {
        reference_item_id: reference_item_id.to_string(),
        window_seconds: window,
        proposals,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noise(len: usize, seed: u64) -> Vec<f32> {
        let mut state = seed;
        (0..len)
            .map(|_| {
                state = state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                ((state >> 33) as f32 / (1u64 << 31) as f32) * 2.0 - 1.0
            })
            .collect()
    }

    fn direct(reference: &[f32], other: &[f32], lag: i64) -> f64 {
        let mut sum = 0.0;
        for (n, r) in reference.iter().enumerate() {
            let m = n as i64 + lag;
            if m >= 0 && (m as usize) < other.len() {
                sum += *r as f64 * other[m as usize] as f64;
            }
        }
        sum
    }

    #[test]
    fn fft_correlation_matches_the_direct_definition() {
        let a = noise(300, 1);
        let b = noise(250, 2);
        let size = (a.len() + b.len()).next_power_of_two();
        let result = cross_correlate(&a, &b, 200).unwrap();
        // The reported peak must be the true maximum of the directly computed correlation.
        let norm = (a.iter().map(|v| (*v as f64).powi(2)).sum::<f64>()
            * b.iter().map(|v| (*v as f64).powi(2)).sum::<f64>())
        .sqrt();
        let best = (-(a.len() as i64 - 1).min(200)..=(b.len() as i64 - 1).min(200))
            .map(|lag| (lag, direct(&a, &b, lag) / norm))
            .fold((0, f64::MIN), |acc, x| if x.1 > acc.1 { x } else { acc });
        assert_eq!(result.lag_samples, best.0);
        assert!(
            (result.score - best.1).abs() < 1e-9,
            "{} vs {}",
            result.score,
            best.1
        );
        assert!(size >= 550);
    }

    #[test]
    fn recovers_a_known_delay_in_both_directions() {
        let base = noise(16000, 7);
        let delay = 1200usize; // 0.15 s at 8 kHz
        let mut later = vec![0.0f32; delay];
        later.extend_from_slice(&base);
        let forward = cross_correlate(&base, &later, 8000).unwrap();
        assert_eq!(forward.lag_samples, delay as i64);
        assert!(forward.score > 0.9, "{forward:?}");
        assert!(forward.peak_ratio > 5.0);
        let backward = cross_correlate(&later, &base, 8000).unwrap();
        assert_eq!(backward.lag_samples, -(delay as i64));
    }

    #[test]
    fn unrelated_signals_are_not_confident_negative_control() {
        let a = noise(16000, 11);
        let b = noise(16000, 12);
        let result = cross_correlate(&a, &b, 8000).unwrap();
        assert!(
            result.score < MIN_CONFIDENT_SCORE || result.peak_ratio < MIN_PEAK_RATIO,
            "unrelated noise must not look like a match: {result:?}"
        );
    }

    #[test]
    fn silence_has_no_alignment() {
        assert!(cross_correlate(&[0.0; 100], &noise(100, 3), 50).is_none());
        assert!(cross_correlate(&[], &noise(100, 3), 50).is_none());
    }
}
