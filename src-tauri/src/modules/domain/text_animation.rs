//! Phase 31 — Canonical Text Animation Engine.
//!
//! Provides pure, deterministic, frame-grid animation evaluation for rich text items:
//! - RationalTime/frame-grid evaluation only (never wall clock or WebView timer)
//! - Unit selection: All, Grapheme, Word, Line with stable mapping from NFC graphemes
//! - Never splits Unicode combining marks, ZWJ clusters, or emoji sequences
//! - StaggerOrder: Forward, Backward, CenterOut, Random (seeded deterministic PRNG)
//! - Easing interpolation via canonical `evaluator::evaluate_eased_progress`
//! - In/Loop/Out phase evaluation with LoopMode (None, Repeat, PingPong)
//! - Duration conflict policies: Reject, ProportionalScale, ClampOut
//! - Documented stagger mathematics with exact zero-drift boundary guarantees

use super::evaluator::evaluate_eased_progress;
use super::text::{
    AnimationTransform, AnimationUnit, DurationConflictPolicy, LoopMode, StaggerOrder,
    TextAnimation, TextAnimationSegment,
};
use super::time::RationalTime;
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

/// Evaluated motion state for each grapheme in a text item at a specific canonical time.
#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatedTextMotion {
    /// Evaluated transform for each grapheme cluster (in string NFC grapheme order).
    pub grapheme_transforms: Vec<AnimationTransform>,
    /// Evaluated transform for each unit (in unit index order).
    pub unit_transforms: Vec<AnimationTransform>,
    /// Map from grapheme index to unit index.
    pub grapheme_to_unit: Vec<usize>,
    /// Number of units.
    pub unit_count: usize,
    /// Animation unit used.
    pub unit: AnimationUnit,
    /// Horizontal pivot in [0.0, 1.0] relative to each unit/cluster bounding box.
    pub pivot_x: f32,
    /// Vertical pivot in [0.0, 1.0] relative to each unit/cluster bounding box.
    pub pivot_y: f32,
    /// True if all transforms are identical to `AnimationTransform::default()`.
    /// When true, the renderer can bypass per-cluster transforms and execute the
    /// byte-identical Phase 30 static path.
    pub is_identity: bool,
}

impl EvaluatedTextMotion {
    /// Identity motion where every grapheme retains default transform.
    pub fn identity(grapheme_count: usize, pivot_x: f32, pivot_y: f32) -> Self {
        Self {
            grapheme_transforms: vec![AnimationTransform::default(); grapheme_count],
            unit_transforms: vec![AnimationTransform::default()],
            grapheme_to_unit: vec![0; grapheme_count],
            unit_count: 1,
            unit: AnimationUnit::All,
            pivot_x,
            pivot_y,
            is_identity: true,
        }
    }
}

/// A lightweight, platform-independent 64-bit linear congruential PRNG
/// used for 100% deterministic seeded random stagger order.
#[derive(Debug, Clone)]
struct SeededPrng {
    state: u64,
}

impl SeededPrng {
    fn new(seed: u64) -> Self {
        // Mix seed with golden ratio constant to avoid zero state stagnation
        let mut prng = Self {
            state: seed.wrapping_add(0x9E3779B97F4A7C15),
        };
        prng.next_u64();
        prng
    }

    fn next_u64(&mut self) -> u64 {
        // Knuth's 64-bit LCG multiplier
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.state
    }

    fn next_usize(&mut self, bound: usize) -> usize {
        if bound <= 1 {
            return 0;
        }
        (self.next_u64() % (bound as u64)) as usize
    }
}

/// Extracts Unicode NFC extended grapheme clusters from text.
///
/// Combining diacritical marks (e.g. Vietnamese accents 'ệ', 'ơ', 'ư'),
/// zero-width joiners (ZWJ emoji sequences, flags), and ligature clusters
/// are preserved as single atomic grapheme strings and never sliced.
pub fn extract_nfc_graphemes(text: &str) -> Vec<String> {
    let nfc: String = text.nfc().collect();
    nfc.graphemes(true).map(str::to_string).collect()
}

/// Maps each grapheme index `0..N-1` to a unit index `0..K-1`.
///
/// - `AnimationUnit::All`: All graphemes map to unit 0. K = 1.
/// - `AnimationUnit::Grapheme`: Each grapheme maps to its own unit. K = N.
/// - `AnimationUnit::Word`: Non-whitespace sequences form words; whitespace
///   attaches to the preceding word so that every character is mapped.
/// - `AnimationUnit::Line`: Lines delimited by layout-derived line groupings or
///   fallback explicit newlines (`\n` or `\r\n`).
pub fn map_graphemes_to_units(graphemes: &[String], unit: AnimationUnit) -> (Vec<usize>, usize) {
    map_graphemes_to_units_with_layout(graphemes, unit, None)
}

/// Maps each grapheme index to a unit index, optionally accepting layout-derived
/// line indices (from `RichTextRenderer::layout_line_indices`) so that wrapped lines
/// are treated as distinct animation units.
pub fn map_graphemes_to_units_with_layout(
    graphemes: &[String],
    unit: AnimationUnit,
    layout_lines: Option<&[Vec<usize>]>,
) -> (Vec<usize>, usize) {
    if graphemes.is_empty() {
        return (Vec::new(), 0);
    }

    match unit {
        AnimationUnit::All => {
            let mapping = vec![0; graphemes.len()];
            (mapping, 1)
        }
        AnimationUnit::Grapheme => {
            let mapping = (0..graphemes.len()).collect();
            let count = graphemes.len();
            (mapping, count)
        }
        AnimationUnit::Word => {
            let mut mapping = Vec::with_capacity(graphemes.len());
            let mut current_unit = 0;
            let mut in_word = false;
            let mut words_started = 0;

            for grapheme in graphemes {
                let is_whitespace = grapheme.chars().all(char::is_whitespace);
                if is_whitespace {
                    if in_word {
                        // Trailing whitespace attaches to current word
                        in_word = false;
                    }
                    mapping.push(current_unit);
                } else {
                    if !in_word && words_started > 0 {
                        current_unit += 1;
                    }
                    in_word = true;
                    words_started += 1;
                    mapping.push(current_unit);
                }
            }

            let count = if words_started == 0 {
                1 // Only whitespace characters in text
            } else {
                current_unit + 1
            };
            (mapping, count)
        }
        AnimationUnit::Line => {
            if let Some(lines) = layout_lines {
                if !lines.is_empty() {
                    let mut mapping = vec![0; graphemes.len()];
                    for (line_idx, indices) in lines.iter().enumerate() {
                        for &idx in indices {
                            if idx < mapping.len() {
                                mapping[idx] = line_idx;
                            }
                        }
                    }
                    let count = lines.len();
                    return (mapping, count);
                }
            }

            let mut mapping = Vec::with_capacity(graphemes.len());
            let mut current_line = 0;

            for grapheme in graphemes {
                mapping.push(current_line);
                if grapheme.contains('\n') {
                    current_line += 1;
                }
            }

            let count = current_line + 1;
            (mapping, count)
        }
    }
}

/// Computes the stagger rank `r_u \in [0..S_max]` for each unit `u \in [0..K-1]`.
///
/// - `Forward`: Unit `u` has rank `u`.
/// - `Backward`: Unit `u` has rank `(K - 1) - u`.
/// - `CenterOut`: Ranks expand symmetrically from the center index `(K - 1) / 2.0`.
///   Units at identical distance from center share the same rank.
/// - `Random`: Deterministic Fisher-Yates permutation seeded by `seed`.
pub fn compute_stagger_ranks(
    unit_count: usize,
    order: StaggerOrder,
    seed: Option<u64>,
) -> (Vec<usize>, usize) {
    if unit_count <= 1 {
        return (vec![0; unit_count], 0);
    }

    match order {
        StaggerOrder::Forward => {
            let ranks: Vec<usize> = (0..unit_count).collect();
            let max_rank = unit_count - 1;
            (ranks, max_rank)
        }
        StaggerOrder::Backward => {
            let ranks: Vec<usize> = (0..unit_count).map(|u| (unit_count - 1) - u).collect();
            let max_rank = unit_count - 1;
            (ranks, max_rank)
        }
        StaggerOrder::CenterOut => {
            let center = (unit_count - 1) as f64 / 2.0;
            let distances: Vec<f64> = (0..unit_count).map(|u| (u as f64 - center).abs()).collect();

            // Collect unique distances sorted ascending
            let mut unique_dists = distances.clone();
            unique_dists.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            unique_dists.dedup_by(|a, b| (*a - *b).abs() < 1e-6);

            let max_rank = unique_dists.len().saturating_sub(1);
            let ranks: Vec<usize> = distances
                .into_iter()
                .map(|d| {
                    unique_dists
                        .iter()
                        .position(|u| (*u - d).abs() < 1e-6)
                        .unwrap_or(0)
                })
                .collect();
            (ranks, max_rank)
        }
        StaggerOrder::Random => {
            let s = seed.unwrap_or(0);
            let mut prng = SeededPrng::new(s);
            let mut shuffled: Vec<usize> = (0..unit_count).collect();
            for i in (1..unit_count).rev() {
                let j = prng.next_usize(i + 1);
                shuffled.swap(i, j);
            }
            let max_rank = unit_count - 1;
            (shuffled, max_rank)
        }
    }
}

// ============================================================================
// STAGGER MATHEMATICS
// ============================================================================
// Let:
//   T       = total segment duration (seconds)
//   S       = segment stagger_ratio \in [0.0, 1.0]
//   K       = number of distinct animation units
//   r       = stagger rank of a unit \in [0..S_max]
//   M       = S_max + 1  (total number of stagger tiers)
//
// Invariants & Requirements:
// 1. Total segment animation begins at t = 0 and completes exactly at t = T.
// 2. When S == 0.0 (no stagger):
//      Every unit animates simultaneously across [0, T].
//      unit_duration = T, delta_t = 0.
// 3. When S == 1.0 (sequential stagger):
//      Units run strictly back-to-back across [0, T] without gaps or overlap.
//      unit_duration = T / M, delta_t = T / M.
//      First unit: [0, T/M]. Last unit: [(M-1)*T/M, T].
// 4. For any S \in [0.0, 1.0]:
//      delta_t        = S * (T / M)
//      unit_duration  = T * (1.0 - S * (S_max / M))
//      start_offset(r)= r * delta_t
//      end_offset(r)  = start_offset(r) + unit_duration
//
// Proof of Boundary Completion:
// For the final rank r = S_max:
//   end_offset(S_max) = S_max * S * (T / M) + T * (1.0 - S * (S_max / M))
//                     = T * [ S * (S_max / M) + 1.0 - S * (S_max / M) ]
//                     = T.
// Thus, the final unit completes at exactly t = T for all S \in [0, 1], with
// zero time drift and no divide-by-zero when K <= 1.
// ============================================================================

/// Computes the start offset and active duration for a unit with rank `rank`.
#[inline]
pub fn unit_stagger_window(
    total_duration_sec: f64,
    stagger_ratio: f32,
    rank: usize,
    max_rank: usize,
) -> (f64, f64) {
    if total_duration_sec <= 0.0 {
        return (0.0, 0.0);
    }
    if max_rank == 0 {
        return (0.0, total_duration_sec);
    }

    let s = (stagger_ratio as f64).clamp(0.0, 1.0);
    let m = (max_rank + 1) as f64;
    let s_max = max_rank as f64;

    let delta_t = s * (total_duration_sec / m);
    let unit_duration = total_duration_sec * (1.0 - s * (s_max / m));
    let start_offset = (rank as f64) * delta_t;

    (start_offset, unit_duration)
}

/// Evaluates a single animation segment at local time `local_time_sec`.
pub fn evaluate_segment_unit(
    segment: &TextAnimationSegment,
    effective_duration_sec: f64,
    rank: usize,
    max_rank: usize,
    local_time_sec: f64,
) -> AnimationTransform {
    if effective_duration_sec <= 1e-6 {
        return segment.to;
    }

    let (start_offset, unit_duration) = unit_stagger_window(
        effective_duration_sec,
        segment.stagger_ratio,
        rank,
        max_rank,
    );

    if local_time_sec <= start_offset {
        return segment.from;
    }
    if unit_duration <= 1e-6 || local_time_sec >= start_offset + unit_duration {
        return segment.to;
    }

    let linear_u = ((local_time_sec - start_offset) / unit_duration).clamp(0.0, 1.0);
    let eased_p = evaluate_eased_progress(linear_u, segment.easing);

    interpolate_transform(&segment.from, &segment.to, eased_p as f32)
}

/// Numerically interpolates between two `AnimationTransform` states.
#[inline]
pub fn gcd_common_rate(mut a: i128, mut b: i128) -> i128 {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a.abs().max(1)
}

/// Exact remainder of elapsed modulo cycle on a common rational grid,
/// returned in seconds. Sampling rate never quantizes the loop boundary.
fn rational_remainder_seconds(elapsed: RationalTime, cycle: RationalTime) -> f64 {
    let elapsed_num = elapsed.value as i128 * elapsed.rate.denominator as i128;
    let elapsed_den = elapsed.rate.numerator as i128;
    let cycle_num = cycle.value as i128 * cycle.rate.denominator as i128;
    let cycle_den = cycle.rate.numerator as i128;
    if cycle_num <= 0 || elapsed_den <= 0 || cycle_den <= 0 {
        return 0.0;
    }
    let common = elapsed_den / gcd_common_rate(elapsed_den, cycle_den) * cycle_den;
    let elapsed_scaled = elapsed_num * (common / elapsed_den);
    let cycle_scaled = cycle_num * (common / cycle_den);
    if cycle_scaled == 0 {
        return 0.0;
    }
    elapsed_scaled.rem_euclid(cycle_scaled) as f64 / common as f64
}

fn interpolate_transform(
    from: &AnimationTransform,
    to: &AnimationTransform,
    progress: f32,
) -> AnimationTransform {
    let p = progress.clamp(0.0, 1.0);
    AnimationTransform {
        translate_x: from.translate_x + (to.translate_x - from.translate_x) * p,
        translate_y: from.translate_y + (to.translate_y - from.translate_y) * p,
        scale_x: from.scale_x + (to.scale_x - from.scale_x) * p,
        scale_y: from.scale_y + (to.scale_y - from.scale_y) * p,
        rotation: from.rotation + (to.rotation - from.rotation) * p,
        opacity: (from.opacity + (to.opacity - from.opacity) * p).clamp(0.0, 1.0),
    }
}

/// Resolves effective in-phase and out-phase durations under the specified conflict policy.
pub fn resolve_phase_durations(
    in_duration_sec: f64,
    out_duration_sec: f64,
    clip_duration_sec: f64,
    policy: DurationConflictPolicy,
) -> (f64, f64) {
    if clip_duration_sec <= 0.0 {
        return (0.0, 0.0);
    }

    let sum = in_duration_sec + out_duration_sec;
    if sum <= clip_duration_sec {
        return (in_duration_sec, out_duration_sec);
    }

    match policy {
        DurationConflictPolicy::Reject => {
            // When already validated upstream, fallback cleanly by clamping out duration
            let eff_in = in_duration_sec.min(clip_duration_sec);
            let eff_out = (clip_duration_sec - eff_in).max(0.0);
            (eff_in, eff_out)
        }
        DurationConflictPolicy::ProportionalScale => {
            if sum <= 1e-6 {
                (0.0, 0.0)
            } else {
                let ratio = clip_duration_sec / sum;
                let eff_in = in_duration_sec * ratio;
                let eff_out = clip_duration_sec - eff_in;
                (eff_in, eff_out)
            }
        }
        DurationConflictPolicy::ClampOut => {
            let eff_in = in_duration_sec.min(clip_duration_sec);
            let eff_out = (clip_duration_sec - eff_in).max(0.0);
            (eff_in, eff_out)
        }
    }
}

/// Evaluates a `TextAnimation` on `text` at canonical `clip_time` within `clip_duration`.
///
/// Both `clip_time` and `clip_duration` are canonical `RationalTime` values.
/// Evaluation is strictly frame-grid and deterministic across all platforms.
pub fn evaluate_text_animation(
    text: &str,
    animation: &TextAnimation,
    clip_time: RationalTime,
    clip_duration: RationalTime,
) -> EvaluatedTextMotion {
    evaluate_text_animation_with_layout(text, animation, clip_time, clip_duration, None)
}

/// Evaluates a `TextAnimation` on `text` at canonical `clip_time` within `clip_duration`,
/// optionally accepting layout-derived line groupings for wrapped line support.
pub fn evaluate_text_animation_with_layout(
    text: &str,
    animation: &TextAnimation,
    clip_time: RationalTime,
    clip_duration: RationalTime,
    layout_lines: Option<&[Vec<usize>]>,
) -> EvaluatedTextMotion {
    let graphemes = extract_nfc_graphemes(text);
    if graphemes.is_empty() {
        return EvaluatedTextMotion::identity(0, animation.pivot_x, animation.pivot_y);
    }

    let has_any_segment = animation.in_anim.is_some()
        || animation.loop_anim.is_some()
        || animation.out_anim.is_some();

    if !has_any_segment {
        return EvaluatedTextMotion::identity(
            graphemes.len(),
            animation.pivot_x,
            animation.pivot_y,
        );
    }

    // Anchor evaluation to persistent phase_origin and phase_end if present.
    // When split or trimmed, phase_origin/phase_end maintain absolute source time anchor
    // so in-animation stays at origin, out-animation stays at end, and loop continues seamlessly.
    let rate = clip_duration.rate;
    let phase_origin = animation.phase_origin.unwrap_or(RationalTime::zero(rate));
    let phase_end = animation.phase_end.unwrap_or(clip_duration);
    let mut total_phase_dur = phase_end.sub_exact(&phase_origin);
    if total_phase_dur.value <= 0 {
        total_phase_dur = clip_duration;
    }
    let total_phase_sec = total_phase_dur.to_seconds().max(0.0);

    // Elapsed time from phase_origin
    let elapsed_in_phase = clip_time.sub_exact(&phase_origin);
    let current_time_sec = elapsed_in_phase.to_seconds().clamp(0.0, total_phase_sec);

    let (grapheme_to_unit, unit_count) =
        map_graphemes_to_units_with_layout(&graphemes, animation.unit, layout_lines);
    let (unit_ranks, max_rank) = compute_stagger_ranks(unit_count, animation.order, animation.seed);

    let in_dur_sec = animation
        .in_anim
        .as_ref()
        .map_or(0.0, |s| s.duration.to_seconds());
    let out_dur_sec = animation
        .out_anim
        .as_ref()
        .map_or(0.0, |s| s.duration.to_seconds());

    let (eff_in_sec, eff_out_sec) = resolve_phase_durations(
        in_dur_sec,
        out_dur_sec,
        total_phase_sec,
        animation.conflict_policy,
    );

    let out_phase_start = total_phase_sec - eff_out_sec;

    // Evaluate transform for each unit
    let mut unit_transforms = Vec::with_capacity(unit_count);
    for rank in unit_ranks.iter().take(unit_count).copied() {
        let transform = if eff_in_sec > 0.0 && current_time_sec < eff_in_sec {
            // In-phase
            if let Some(ref in_seg) = animation.in_anim {
                evaluate_segment_unit(in_seg, eff_in_sec, rank, max_rank, current_time_sec)
            } else {
                AnimationTransform::default()
            }
        } else if eff_out_sec > 0.0 && current_time_sec >= out_phase_start {
            // Out-phase
            if let Some(ref out_seg) = animation.out_anim {
                let local_out_time = current_time_sec - out_phase_start;
                evaluate_segment_unit(out_seg, eff_out_sec, rank, max_rank, local_out_time)
            } else {
                AnimationTransform::default()
            }
        } else {
            // Middle / Loop phase
            if let (Some(ref loop_seg), loop_mode) = (&animation.loop_anim, animation.loop_mode) {
                let cycle_rate = loop_seg.duration.rate;
                let n_ticks = loop_seg.duration.value;
                if n_ticks > 0 && loop_mode != LoopMode::None {
                    // Reconstruct the loop origin from the exact in-animation
                    // duration when it was not rescaled by a conflict policy.
                    let in_rt = animation
                        .in_anim
                        .as_ref()
                        .map(|segment| {
                            if (segment.duration.to_seconds() - eff_in_sec).abs() < 1e-9 {
                                segment.duration
                            } else {
                                RationalTime::from_seconds(eff_in_sec, segment.duration.rate)
                            }
                        })
                        .unwrap_or_else(|| RationalTime::zero(cycle_rate));
                    let loop_start_time = phase_origin.add_exact(&in_rt);
                    let elapsed_loop = if clip_time.semantic_ge(&loop_start_time) {
                        clip_time.sub_exact(&loop_start_time)
                    } else {
                        RationalTime::zero(clip_time.rate)
                    };
                    let cycle_seconds = loop_seg.duration.to_seconds();
                    let local_loop_time_sec = match loop_mode {
                        LoopMode::None => 0.0,
                        LoopMode::Repeat => {
                            rational_remainder_seconds(elapsed_loop, loop_seg.duration)
                        }
                        LoopMode::PingPong => {
                            let period = RationalTime::new(n_ticks.saturating_mul(2), cycle_rate);
                            let remainder = rational_remainder_seconds(elapsed_loop, period);
                            if remainder <= cycle_seconds {
                                remainder
                            } else {
                                (2.0 * cycle_seconds - remainder).max(0.0)
                            }
                        }
                    };
                    evaluate_segment_unit(
                        loop_seg,
                        loop_seg.duration.to_seconds(),
                        rank,
                        max_rank,
                        local_loop_time_sec,
                    )
                } else {
                    AnimationTransform::default()
                }
            } else {
                AnimationTransform::default()
            }
        };

        unit_transforms.push(transform);
    }

    // Map unit transforms back to individual graphemes
    let mut grapheme_transforms = Vec::with_capacity(graphemes.len());
    let mut is_identity = true;
    let default_transform = AnimationTransform::default();

    for &unit_idx in &grapheme_to_unit {
        let t = if unit_idx < unit_transforms.len() {
            unit_transforms[unit_idx]
        } else {
            default_transform
        };
        if (t.translate_x - default_transform.translate_x).abs() > 1e-4
            || (t.translate_y - default_transform.translate_y).abs() > 1e-4
            || (t.scale_x - default_transform.scale_x).abs() > 1e-4
            || (t.scale_y - default_transform.scale_y).abs() > 1e-4
            || (t.rotation - default_transform.rotation).abs() > 1e-4
            || (t.opacity - default_transform.opacity).abs() > 1e-4
        {
            is_identity = false;
        }
        grapheme_transforms.push(t);
    }

    EvaluatedTextMotion {
        grapheme_transforms,
        unit_transforms,
        grapheme_to_unit,
        unit_count,
        unit: animation.unit,
        pivot_x: animation.pivot_x,
        pivot_y: animation.pivot_y,
        is_identity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::domain::time::FrameRate;
    use crate::modules::domain::KeyframeEasing;

    #[test]
    fn test_nfc_nfd_vietnamese_mapping_equality() {
        let text_nfc = "Tiếng Việt — Xin chào thế giới!";
        let text_nfd: String = text_nfc.nfd().collect();

        let g_nfc = extract_nfc_graphemes(text_nfc);
        let g_nfd = extract_nfc_graphemes(&text_nfd);

        assert_eq!(g_nfc, g_nfd);
        assert_eq!(g_nfc.len(), 31);
    }

    #[test]
    fn test_emoji_zwj_and_flags_not_split() {
        let text = "Family: 👨‍👩‍👧‍👦 Flag: 🇻🇳 End";
        let graphemes = extract_nfc_graphemes(text);

        // "👨‍👩‍👧‍👦" is 1 grapheme cluster
        assert!(graphemes.contains(&"👨‍👩‍👧‍👦".to_string()));
        // "🇻🇳" is 1 grapheme cluster
        assert!(graphemes.contains(&"🇻🇳".to_string()));
    }

    #[test]
    fn test_unit_mapping_all_grapheme_word_line() {
        let text = "Hello world\nSecond line";
        let graphemes = extract_nfc_graphemes(text);

        // All
        let (all_map, all_count) = map_graphemes_to_units(&graphemes, AnimationUnit::All);
        assert_eq!(all_count, 1);
        assert!(all_map.iter().all(|&u| u == 0));

        // Grapheme
        let (g_map, g_count) = map_graphemes_to_units(&graphemes, AnimationUnit::Grapheme);
        assert_eq!(g_count, graphemes.len());
        for (i, &u) in g_map.iter().enumerate() {
            assert_eq!(i, u);
        }

        // Word
        let (w_map, w_count) = map_graphemes_to_units(&graphemes, AnimationUnit::Word);
        // "Hello ", "world\n", "Second ", "line" -> 4 words
        assert_eq!(w_count, 4);
        assert_eq!(w_map.len(), graphemes.len());

        // Line
        let (l_map, l_count) = map_graphemes_to_units(&graphemes, AnimationUnit::Line);
        assert_eq!(l_count, 2);
        assert_eq!(l_map.len(), graphemes.len());
    }

    #[test]
    fn test_stagger_math_boundary_completion() {
        // Test varying stagger ratios from 0.0 to 1.0 with varying units
        for k in 1..=10 {
            let max_rank = k - 1;
            let total_dur = 2.0f64;
            for &stagger in &[0.0f32, 0.25, 0.5, 0.75, 1.0] {
                let (start, dur) = unit_stagger_window(total_dur, stagger, max_rank, max_rank);
                let end = start + dur;
                assert!(
                    (end - total_dur).abs() < 1e-6,
                    "Failed boundary completion for k={}, stagger={}: end={}",
                    k,
                    stagger,
                    end
                );
            }
        }
    }

    #[test]
    fn test_seeded_random_stagger_is_deterministic() {
        let (r1, _) = compute_stagger_ranks(10, StaggerOrder::Random, Some(12345));
        let (r2, _) = compute_stagger_ranks(10, StaggerOrder::Random, Some(12345));
        let (r3, _) = compute_stagger_ranks(10, StaggerOrder::Random, Some(99999));

        assert_eq!(r1, r2);
        assert_ne!(r1, r3);
    }

    #[test]
    fn test_center_out_stagger_symmetry() {
        // 5 units: 0, 1, 2, 3, 4. Center = 2
        let (ranks, max_rank) = compute_stagger_ranks(5, StaggerOrder::CenterOut, None);
        assert_eq!(max_rank, 2);
        assert_eq!(ranks[2], 0); // center
        assert_eq!(ranks[1], 1); // dist 1
        assert_eq!(ranks[3], 1); // dist 1
        assert_eq!(ranks[0], 2); // dist 2
        assert_eq!(ranks[4], 2); // dist 2
    }

    #[test]
    fn test_loop_mode_repeat_and_pingpong() {
        let rate = FrameRate::new(30, 1);
        let clip_dur = RationalTime::from_frames(120, rate); // 4 seconds

        let loop_seg = TextAnimationSegment {
            duration: RationalTime::from_frames(30, rate), // 1 second
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.0,
            from: AnimationTransform {
                opacity: 0.0,
                ..Default::default()
            },
            to: AnimationTransform {
                opacity: 1.0,
                ..Default::default()
            },
        };

        // Repeat mode
        let anim_repeat = TextAnimation {
            loop_anim: Some(loop_seg.clone()),
            loop_mode: LoopMode::Repeat,
            ..Default::default()
        };

        // At t = 0.5s (frame 15 of 30) -> opacity should be ~0.5
        let m1 = evaluate_text_animation(
            "Test",
            &anim_repeat,
            RationalTime::from_frames(15, rate),
            clip_dur,
        );
        assert!((m1.grapheme_transforms[0].opacity - 0.5).abs() < 0.05);

        // At t = 1.0s (frame 30 -> start of next cycle) -> opacity should be ~0.0
        let m2 = evaluate_text_animation(
            "Test",
            &anim_repeat,
            RationalTime::from_frames(30, rate),
            clip_dur,
        );
        assert!((m2.grapheme_transforms[0].opacity - 0.0).abs() < 0.05);

        // PingPong mode
        let anim_pingpong = TextAnimation {
            loop_anim: Some(loop_seg),
            loop_mode: LoopMode::PingPong,
            ..Default::default()
        };

        // At t = 1.0s (frame 30 -> peak of forward pass) -> opacity should be ~1.0
        let m3 = evaluate_text_animation(
            "Test",
            &anim_pingpong,
            RationalTime::from_frames(30, rate),
            clip_dur,
        );
        assert!((m3.grapheme_transforms[0].opacity - 1.0).abs() < 0.05);

        // At t = 1.5s (frame 45 -> halfway down reverse pass) -> opacity should be ~0.5
        let m4 = evaluate_text_animation(
            "Test",
            &anim_pingpong,
            RationalTime::from_frames(45, rate),
            clip_dur,
        );
        assert!((m4.grapheme_transforms[0].opacity - 0.5).abs() < 0.05);
    }

    #[test]
    fn test_loop_exact_rational_fractional_fps_and_one_frame() {
        // Fractional frame rate: 29.97 fps (30000/1001)
        let rate = FrameRate::new(30000, 1001);
        let clip_dur = RationalTime::from_frames(300, rate);

        let loop_seg = TextAnimationSegment {
            duration: RationalTime::from_frames(30, rate),
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.0,
            from: AnimationTransform {
                translate_x: 0.0,
                ..Default::default()
            },
            to: AnimationTransform {
                translate_x: 100.0,
                ..Default::default()
            },
        };

        let anim = TextAnimation {
            loop_anim: Some(loop_seg),
            loop_mode: LoopMode::Repeat,
            ..Default::default()
        };

        // Frame 0: exactly 0.0
        let m0 = evaluate_text_animation("A", &anim, RationalTime::from_frames(0, rate), clip_dur);
        assert!((m0.grapheme_transforms[0].translate_x - 0.0).abs() < 1e-4);

        // Frame 30: exactly starts second cycle (0.0)
        let m30 =
            evaluate_text_animation("A", &anim, RationalTime::from_frames(30, rate), clip_dur);
        assert!((m30.grapheme_transforms[0].translate_x - 0.0).abs() < 1e-4);

        // Frame 60: exactly starts third cycle (0.0)
        let m60 =
            evaluate_text_animation("A", &anim, RationalTime::from_frames(60, rate), clip_dur);
        assert!((m60.grapheme_transforms[0].translate_x - 0.0).abs() < 1e-4);

        // One-frame loop test (must not panic or divide by zero)
        let one_frame_seg = TextAnimationSegment {
            duration: RationalTime::from_frames(1, rate),
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.0,
            from: AnimationTransform::default(),
            to: AnimationTransform {
                translate_y: 50.0,
                ..Default::default()
            },
        };
        let anim_one = TextAnimation {
            loop_anim: Some(one_frame_seg),
            loop_mode: LoopMode::PingPong,
            ..Default::default()
        };
        for f in 0..10 {
            let mf = evaluate_text_animation(
                "B",
                &anim_one,
                RationalTime::from_frames(f, rate),
                clip_dur,
            );
            assert_eq!(mf.grapheme_transforms.len(), 1);
        }
    }

    #[test]
    fn test_split_trim_phase_origin_continuity() {
        let rate = FrameRate::FPS_30;
        let original_dur = RationalTime::from_frames(90, rate); // 3 seconds

        // In: 30 frames (0..1s), Out: 30 frames (2..3s)
        let in_seg = TextAnimationSegment {
            duration: RationalTime::from_frames(30, rate),
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.0,
            from: AnimationTransform {
                opacity: 0.0,
                ..Default::default()
            },
            to: AnimationTransform {
                opacity: 1.0,
                ..Default::default()
            },
        };
        let out_seg = TextAnimationSegment {
            duration: RationalTime::from_frames(30, rate),
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.0,
            from: AnimationTransform {
                opacity: 1.0,
                ..Default::default()
            },
            to: AnimationTransform {
                opacity: 0.0,
                ..Default::default()
            },
        };

        let anim = TextAnimation {
            in_anim: Some(in_seg),
            out_anim: Some(out_seg),
            phase_origin: Some(RationalTime::zero(rate)),
            phase_end: Some(original_dur),
            ..Default::default()
        };

        // Frame 45 (1.5s): middle, opacity should be 1.0
        let m_mid_orig = evaluate_text_animation(
            "Test",
            &anim,
            RationalTime::from_frames(45, rate),
            original_dur,
        );
        assert!((m_mid_orig.grapheme_transforms[0].opacity - 1.0).abs() < 1e-4);

        // Frame 75 (2.5s): halfway through out-phase, opacity should be 0.5
        let m_out_orig = evaluate_text_animation(
            "Test",
            &anim,
            RationalTime::from_frames(75, rate),
            original_dur,
        );
        assert!((m_out_orig.grapheme_transforms[0].opacity - 0.5).abs() < 0.05);

        // Now simulate split at frame 40 into Fragment 2 (frames 40..90, duration 50 frames):
        // Fragment 2 has source duration 50 frames, but its source time at frame 75 is still 75!
        // phase_origin (0) and phase_end (90) remain anchored.
        let fragment2_dur = RationalTime::from_frames(50, rate);
        let m_out_frag = evaluate_text_animation(
            "Test",
            &anim,
            RationalTime::from_frames(75, rate),
            fragment2_dur,
        );
        assert!(
            (m_out_frag.grapheme_transforms[0].opacity - m_out_orig.grapheme_transforms[0].opacity)
                .abs()
                < 1e-4,
            "Split fragment must preserve exact out-animation timing"
        );
    }

    #[test]
    fn test_wrapped_lines_layout_stagger() {
        let text = "Line one wraps here";
        let graphemes = extract_nfc_graphemes(text);

        // Simulate layout wrap into two lines: [0..9] and [9..19]
        let layout_lines = vec![
            (0..9).collect::<Vec<usize>>(),
            (9..graphemes.len()).collect::<Vec<usize>>(),
        ];

        let (mapping, count) = map_graphemes_to_units_with_layout(
            &graphemes,
            AnimationUnit::Line,
            Some(&layout_lines),
        );
        assert_eq!(count, 2);
        assert_eq!(mapping[0], 0);
        assert_eq!(mapping[8], 0);
        assert_eq!(mapping[9], 1);
        assert_eq!(mapping[graphemes.len() - 1], 1);
    }
}
