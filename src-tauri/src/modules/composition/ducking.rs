//! Automatic ducking: a per-clip gain curve derived from the activity of the rest of the mix.
//!
//! The curve is data, not a live compressor: voice activity is measured once on the timeline
//! grid, turned into a gain curve with attack/release smoothing, and then evaluated by
//! `CompositionPlan::effective_audio_gain_at` like any other volume automation. Export writes
//! it into the same per-sample envelope file as fades and keyframes, and preview samples the
//! same function, so the two cannot disagree about when or how deep the music ducks.
//!
//! The idea (voice bus drives gain reduction on music) follows blyacut's sidechain ducking
//! (GPL-3.0, ideas only, see `THIRD_PARTY_NOTICES.md`); the implementation is independent.

use super::CompositionPlan;
use crate::modules::domain::time::RationalTime;
use crate::modules::domain::Ducking;
use crate::modules::media::align::{decode_mono_pcm, ALIGN_SAMPLE_RATE};
use crate::modules::media::ffmpeg::FFmpegService;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// Analysis step on the timeline: gains and voice levels are sampled every 10 ms.
pub const HOP_SECONDS: f64 = 0.010;
/// RMS is taken over 20 ms, centred on each hop.
const RMS_WINDOW_SAMPLES: usize = (ALIGN_SAMPLE_RATE as usize) / 50;
const SILENCE_DB: f32 = -120.0;

/// Gain (linear) for one clip, sampled every `HOP_SECONDS` from timeline time zero.
#[derive(Debug, Clone, PartialEq)]
pub struct DuckingCurve {
    pub gains: Vec<f32>,
}

impl DuckingCurve {
    /// Linear interpolation between hops; outside the analysed span the nearest value holds.
    pub fn gain_at(&self, seconds: f64) -> f32 {
        if self.gains.is_empty() {
            return 1.0;
        }
        let position = (seconds / HOP_SECONDS).max(0.0);
        let index = position.floor() as usize;
        if index + 1 >= self.gains.len() {
            return *self.gains.last().unwrap();
        }
        let fraction = (position - index as f64) as f32;
        self.gains[index] * (1.0 - fraction) + self.gains[index + 1] * fraction
    }

    pub fn min_gain(&self) -> f32 {
        self.gains.iter().copied().fold(1.0, f32::min)
    }
}

/// Level (dBFS RMS) of the audible rest of the mix on the same grid.
#[derive(Debug, Clone, PartialEq)]
pub struct VoiceTrack {
    pub rms_db: Vec<f32>,
}

fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// Pure gain-curve construction: while the voice is above the threshold the target is
/// `-depth_db`; the current reduction approaches the target with the attack time constant when
/// going down and the release time constant when recovering (one-pole in the dB domain).
pub fn gain_curve(voice: &VoiceTrack, ducking: &Ducking) -> DuckingCurve {
    let attack = (-HOP_SECONDS / (ducking.attack_ms as f64 / 1000.0)).exp() as f32;
    let release = (-HOP_SECONDS / (ducking.release_ms as f64 / 1000.0)).exp() as f32;
    let mut reduction_db = 0.0f32; // 0 = unity, negative = ducked
    let gains = voice
        .rms_db
        .iter()
        .map(|&level| {
            let target = if level > ducking.threshold_db {
                -ducking.depth_db
            } else {
                0.0
            };
            let coefficient = if target < reduction_db {
                attack
            } else {
                release
            };
            reduction_db = target + (reduction_db - target) * coefficient;
            db_to_gain(reduction_db)
        })
        .collect();
    DuckingCurve { gains }
}

fn is_cancelled(cancel: Option<&Arc<AtomicBool>>) -> bool {
    cancel.is_some_and(|flag| flag.load(Ordering::SeqCst))
}

/// Measures the power of every audible clip that is not itself ducked.
pub fn voice_track(
    plan: &CompositionPlan,
    cancel: Option<&Arc<AtomicBool>>,
) -> Result<VoiceTrack, String> {
    let duration = plan.duration.to_seconds();
    let hops = (duration / HOP_SECONDS).ceil() as usize + 1;
    let mut power = vec![0.0f64; hops];
    let half = RMS_WINDOW_SAMPLES / 2;
    let mut has_audio_stream: std::collections::HashMap<std::path::PathBuf, bool> =
        std::collections::HashMap::new();

    for layer in &plan.audio_layers {
        if is_cancelled(cancel) {
            return Err("Ducking analysis cancelled".into());
        }
        if layer.muted || layer.ducking.is_some() {
            continue;
        }
        let Some(path) = &layer.asset_path else {
            continue;
        };
        let speed = layer.speed.as_f64();
        let start = layer.timeline_range.start.to_seconds();
        let length = layer.timeline_range.duration.to_seconds();
        if length <= 0.0 {
            continue;
        }
        // A silent video is an audio layer too (embedded audio defaults to on) but has no
        // stream to measure; it contributes no voice power rather than failing the analysis.
        let has_audio = *has_audio_stream
            .entry(path.clone())
            .or_insert_with(|| {
                FFmpegService::probe_file(path)
                    .map(|metadata| metadata.audio_codec.is_some())
                    .unwrap_or(true)
            });
        if !has_audio {
            continue;
        }
        let source_start = layer.source_range.start.to_seconds();
        let samples = decode_mono_pcm(path, source_start, length * speed)?;
        for (i, slot) in power.iter_mut().enumerate() {
            let t = i as f64 * HOP_SECONDS;
            if t < start || t >= start + length {
                continue;
            }
            let centre = ((t - start) * speed * ALIGN_SAMPLE_RATE as f64) as usize;
            let lo = centre.saturating_sub(half);
            let hi = (centre + half).min(samples.len());
            if hi <= lo {
                continue;
            }
            let mean_square = samples[lo..hi]
                .iter()
                .map(|v| (*v as f64) * (*v as f64))
                .sum::<f64>()
                / (hi - lo) as f64;
            let gain =
                plan.effective_audio_gain_at(layer, RationalTime::from_seconds(t, plan.frame_rate));
            *slot += mean_square * (gain as f64) * (gain as f64);
        }
    }

    Ok(VoiceTrack {
        rms_db: power
            .iter()
            .map(|p| {
                if *p <= 1e-12 {
                    SILENCE_DB
                } else {
                    (10.0 * p.log10()) as f32
                }
            })
            .collect(),
    })
}

/// Curves for every clip that has ducking, keyed by item id. Empty when no clip ducks.
pub fn compute_curves(
    plan: &CompositionPlan,
    cancel: Option<&Arc<AtomicBool>>,
) -> Result<HashMap<String, Arc<DuckingCurve>>, String> {
    let ducked: Vec<_> = plan
        .audio_layers
        .iter()
        .filter(|layer| layer.ducking.is_some() && !layer.muted)
        .collect();
    if ducked.is_empty() {
        return Ok(HashMap::new());
    }
    let voice = voice_track(plan, cancel)?;
    Ok(ducked
        .into_iter()
        .map(|layer| {
            let setting = layer.ducking.expect("filtered above");
            (
                layer.item_id.clone(),
                Arc::new(gain_curve(&voice, &setting)),
            )
        })
        .collect())
}

pub type CurveMap = HashMap<String, Arc<DuckingCurve>>;

/// Hash of everything the curves depend on: every audio layer's source file (size and
/// modification time), ranges, speed, volume automation and ducking settings.
pub fn signature(plan: &CompositionPlan) -> u64 {
    let mut hasher = DefaultHasher::new();
    for layer in &plan.audio_layers {
        let file = layer
            .asset_path
            .as_ref()
            .and_then(|path| std::fs::metadata(path).ok());
        format!(
            "{:?}|{:?}|{:?}|{:?}",
            (
                &layer.item_id,
                &layer.asset_path,
                &layer.timeline_range,
                &layer.source_range,
                layer.speed,
                layer.volume,
                layer.track_volume,
                layer.muted,
            ),
            (
                &layer.fade_in,
                &layer.fade_out,
                &layer.keyframes,
                &layer.ducking
            ),
            file.as_ref().map(|m| (m.len(), m.modified().ok())),
            &plan.audio_crossfades,
        )
        .hash(&mut hasher);
    }
    plan.duration.to_seconds().to_bits().hash(&mut hasher);
    hasher.finish()
}

fn cache() -> &'static Mutex<Option<(u64, Arc<CurveMap>)>> {
    static CACHE: OnceLock<Mutex<Option<(u64, Arc<CurveMap>)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

/// Curves for the plan if they are already known: an empty map when nothing ducks, the cached
/// map when the analysis has run for this exact mix, `None` while it still has to run. Only
/// the most recent mix is kept, so the cache is bounded.
pub fn cached(plan: &CompositionPlan) -> Option<Arc<CurveMap>> {
    if !plan
        .audio_layers
        .iter()
        .any(|l| l.ducking.is_some() && !l.muted)
    {
        return Some(Arc::new(HashMap::new()));
    }
    let key = signature(plan);
    cache().lock().ok().and_then(|guard| {
        guard
            .as_ref()
            .filter(|(k, _)| *k == key)
            .map(|(_, v)| Arc::clone(v))
    })
}

/// Runs the analysis (blocking) and remembers the result for `cached`.
pub fn prepare(
    plan: &CompositionPlan,
    cancel: Option<&Arc<AtomicBool>>,
) -> Result<Arc<CurveMap>, String> {
    if let Some(known) = cached(plan) {
        return Ok(known);
    }
    let key = signature(plan);
    let curves = Arc::new(compute_curves(plan, cancel)?);
    if let Ok(mut guard) = cache().lock() {
        *guard = Some((key, Arc::clone(&curves)));
    }
    Ok(curves)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn voice_with_burst(total_hops: usize, start: usize, end: usize, loud: f32) -> VoiceTrack {
        VoiceTrack {
            rms_db: (0..total_hops)
                .map(|i| {
                    if (start..end).contains(&i) {
                        loud
                    } else {
                        SILENCE_DB
                    }
                })
                .collect(),
        }
    }

    fn settings(depth: f32, attack: u32, release: u32) -> Ducking {
        Ducking {
            depth_db: depth,
            attack_ms: attack,
            release_ms: release,
            threshold_db: -40.0,
        }
    }

    fn to_db(gain: f32) -> f32 {
        20.0 * gain.log10()
    }

    #[test]
    fn reaches_the_configured_depth_while_the_voice_is_active() {
        // 4 s of analysis, voice from 1 s to 3 s.
        let curve = gain_curve(
            &voice_with_burst(400, 100, 300, -20.0),
            &settings(12.0, 20, 300),
        );
        let mid = to_db(curve.gain_at(2.0));
        assert!((mid - -12.0).abs() < 0.2, "{mid} dB at 2 s");
        assert!(
            to_db(curve.gain_at(0.5)).abs() < 0.01,
            "unity before the voice"
        );
    }

    #[test]
    fn attack_and_release_follow_their_time_constants() {
        let curve = gain_curve(
            &voice_with_burst(600, 100, 300, -20.0),
            &settings(12.0, 50, 400),
        );
        // One attack time constant after onset (the onset hop itself is the first step) the reduction has covered 1 - 1/e of the depth.
        let after_attack = to_db(curve.gain_at(1.0 + 0.050 - HOP_SECONDS));
        assert!(
            (after_attack - -12.0 * (1.0 - (-1.0f32).exp())).abs() < 0.6,
            "{after_attack}"
        );
        // One release time constant after the voice stops, 1/e of the depth remains.
        let after_release = to_db(curve.gain_at(3.0 + 0.400 - HOP_SECONDS));
        assert!(
            (after_release - -12.0 * (-1.0f32).exp()).abs() < 0.6,
            "{after_release}"
        );
        // Fully recovered well after the voice.
        assert!(to_db(curve.gain_at(5.5)).abs() < 0.05);
    }

    #[test]
    fn no_voice_means_no_ducking_negative_control() {
        let silent = VoiceTrack {
            rms_db: vec![SILENCE_DB; 300],
        };
        let curve = gain_curve(&silent, &settings(30.0, 5, 20));
        assert!(curve.gains.iter().all(|g| (*g - 1.0).abs() < 1e-6));
        assert_eq!(curve.min_gain(), 1.0);
    }

    #[test]
    fn a_voice_below_the_threshold_does_not_duck() {
        let quiet = voice_with_burst(300, 50, 250, -55.0);
        let curve = gain_curve(&quiet, &settings(12.0, 20, 300));
        assert_eq!(curve.min_gain(), 1.0);
    }

    #[test]
    fn deeper_settings_duck_deeper() {
        let voice = voice_with_burst(400, 100, 300, -20.0);
        let shallow = gain_curve(&voice, &settings(6.0, 20, 300)).min_gain();
        let deep = gain_curve(&voice, &settings(24.0, 20, 300)).min_gain();
        assert!(deep < shallow);
    }

    use crate::modules::domain::time::{FrameRate, TimeRange};
    use crate::modules::domain::{
        Asset, ItemKind, MediaKind, Project, TimelineItem, Track, TrackKind,
    };

    fn ducked_music_project(depth: f32) -> Project {
        let rate = FrameRate::FPS_30;
        let mut project = Project::new("Duck plan".into());
        project.settings.frame_rate = rate;
        for id in ["ast_m", "ast_v"] {
            project.assets.push(Asset {
                asset_id: id.into(),
                name: id.into(),
                path: format!("missing_{id}.wav"),
                media_kind: MediaKind::Audio,
                duration: Some(RationalTime::new(300, rate)),
                file_size: 0,
                dimensions: None,
            });
        }
        let range = TimeRange::new(RationalTime::new(0, rate), RationalTime::new(300, rate));
        for (name, asset, ducking) in [
            ("Music", "ast_m", Some(settings(depth, 20, 300))),
            ("Voice", "ast_v", None),
        ] {
            let mut track = Track::new(name.into(), TrackKind::Audio);
            let mut item = TimelineItem::new(
                Some(asset.into()),
                name.into(),
                ItemKind::Audio,
                range,
                range,
            )
            .unwrap();
            item.ducking = ducking;
            track.items.push(item);
            project.tracks.push(track);
        }
        project
    }

    #[test]
    fn preview_and_export_read_the_same_gain_and_pending_state_is_explicit() {
        let project = ducked_music_project(12.0);
        let mut plan = CompositionPlan::from_project(&project).unwrap();
        let music = plan
            .audio_layers
            .iter()
            .find(|l| l.ducking.is_some())
            .unwrap()
            .clone();

        // Before the analysis: nothing is cached, the preview says so, the gain is untouched.
        assert!(cached(&plan).is_none());
        assert!(plan.ducking_pending());
        let at = RationalTime::from_seconds(2.0, plan.frame_rate);
        assert!(plan.sample_preview_at(at).ducking_pending);
        let undropped = plan.effective_audio_gain_at(&music, at);

        let curve = Arc::new(DuckingCurve {
            gains: vec![0.25; 1000],
        });
        let mut curves = CurveMap::new();
        curves.insert(music.item_id.clone(), curve);
        plan.attach_ducking_curves(curves);
        assert!(!plan.ducking_pending());

        // The export envelope calls `effective_audio_gain_at`; preview reports the same number.
        let export_gain = plan.effective_audio_gain_at(&music, at);
        assert!((export_gain - undropped * 0.25).abs() < 1e-6);
        let sampled = plan.sample_preview_at(at);
        let preview_gain = sampled
            .active_audio
            .iter()
            .find(|a| a.item_id == music.item_id)
            .unwrap()
            .effective_volume;
        assert_eq!(preview_gain, export_gain);
        assert!(!sampled.ducking_pending);
    }

    #[test]
    fn signature_changes_with_the_settings_and_is_stable_otherwise() {
        let project = ducked_music_project(12.0);
        let a = CompositionPlan::from_project(&project).unwrap();
        let b = CompositionPlan::from_project(&project).unwrap();
        let mut deeper_project = project.clone();
        deeper_project.tracks[0].items[0].ducking = Some(settings(20.0, 20, 300));
        let deeper = CompositionPlan::from_project(&deeper_project).unwrap();
        assert_eq!(signature(&a), signature(&b));
        assert_ne!(signature(&a), signature(&deeper));
    }

    #[test]
    fn a_project_without_ducking_needs_no_analysis() {
        let mut project = ducked_music_project(12.0);
        project.tracks[0].items[0].ducking = None;
        let plan = CompositionPlan::from_project(&project).unwrap();
        assert!(!plan.ducking_pending());
        assert!(cached(&plan).unwrap().is_empty());
    }

    #[test]
    fn interpolation_and_edges_are_safe() {
        let curve = DuckingCurve {
            gains: vec![1.0, 0.5, 0.25],
        };
        assert!((curve.gain_at(0.005) - 0.75).abs() < 1e-6);
        assert_eq!(curve.gain_at(10.0), 0.25);
        assert_eq!(curve.gain_at(-1.0), 1.0);
        assert_eq!(DuckingCurve { gains: vec![] }.gain_at(1.0), 1.0);
    }
}
