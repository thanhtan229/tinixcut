//! Canonical Keyframe Easing Evaluator
//! Implements ADR-004 and Phase 14 canonical mathematical specification:
//! - Deterministic 32-iteration bisection solver for Cubic Bezier curves
//! - Standard presets: EaseIn, EaseOut, EaseInOut
//! - Custom CubicBezier with coordinates bounded in [0.0, 1.0]
//! - Exact endpoint bypass
//! - de Casteljau subdivision algorithm for split continuity
//! - Pure, renderer-independent, shared by preview, composition, and export

use super::time::RationalTime;
use super::{Keyframe, KeyframeEasing, KeyframeProperty};

/// Preset: EaseIn = (0.42, 0.0, 1.0, 1.0)
pub const EASE_IN_PARAMS: (f64, f64, f64, f64) = (0.42, 0.0, 1.0, 1.0);

/// Preset: EaseOut = (0.0, 0.0, 0.58, 1.0)
pub const EASE_OUT_PARAMS: (f64, f64, f64, f64) = (0.0, 0.0, 0.58, 1.0);

/// Preset: EaseInOut = (0.42, 0.0, 0.58, 1.0)
pub const EASE_IN_OUT_PARAMS: (f64, f64, f64, f64) = (0.42, 0.0, 0.58, 1.0);

/// Evaluates a 1D cubic Bézier coordinate at parameter q in [0, 1].
/// Formula: 3*(1-q)^2*q*c1 + 3*(1-q)*q^2*c2 + q^3
/// with P0 = 0.0 and P3 = 1.0.
#[inline]
pub fn sample_cubic_coord(q: f64, c1: f64, c2: f64) -> f64 {
    let one_minus_q = 1.0 - q;
    3.0 * one_minus_q * one_minus_q * q * c1 + 3.0 * one_minus_q * q * q * c2 + q * q * q
}

/// Solves for parameter q in [0, 1] such that x(q) = u using exactly 32 bisection iterations.
/// Invariant: u in [0, 1]. Bypass exact endpoints u <= 0.0 and u >= 1.0.
pub fn solve_cubic_q(u: f64, x1: f64, x2: f64) -> f64 {
    if u <= 0.0 {
        return 0.0;
    }
    if u >= 1.0 {
        return 1.0;
    }

    let mut q_min = 0.0f64;
    let mut q_max = 1.0f64;

    for _ in 0..32 {
        let q_mid = (q_min + q_max) * 0.5;
        let x_test = sample_cubic_coord(q_mid, x1, x2);
        if x_test < u {
            q_min = q_mid;
        } else {
            q_max = q_mid;
        }
    }

    (q_min + q_max) * 0.5
}

/// Evaluates eased progress e in [0, 1] given normalized linear time progress u in [0, 1]
/// and an easing descriptor.
pub fn evaluate_eased_progress(u: f64, easing: KeyframeEasing) -> f64 {
    if u <= 0.0 {
        return 0.0;
    }
    if u >= 1.0 {
        return 1.0;
    }

    match easing {
        KeyframeEasing::Linear => u,
        KeyframeEasing::EaseIn => {
            let (x1, y1, x2, y2) = EASE_IN_PARAMS;
            let q = solve_cubic_q(u, x1, x2);
            sample_cubic_coord(q, y1, y2)
        }
        KeyframeEasing::EaseOut => {
            let (x1, y1, x2, y2) = EASE_OUT_PARAMS;
            let q = solve_cubic_q(u, x1, x2);
            sample_cubic_coord(q, y1, y2)
        }
        KeyframeEasing::EaseInOut => {
            let (x1, y1, x2, y2) = EASE_IN_OUT_PARAMS;
            let q = solve_cubic_q(u, x1, x2);
            sample_cubic_coord(q, y1, y2)
        }
        KeyframeEasing::CubicBezier { x1, y1, x2, y2 } => {
            let q = solve_cubic_q(u, x1, x2);
            sample_cubic_coord(q, y1, y2)
        }
    }
}

/// Evaluates a single keyframe channel at a target offset using canonical semantic time
/// and the 32-iteration deterministic solver.
///
/// Follows established endpoint-hold behavior:
/// - Target offset before first keyframe -> returns first keyframe value.
/// - Target offset after last keyframe -> returns last keyframe value.
/// - Exact segment endpoints bypass solver and return exact keyframe values.
pub fn evaluate_property_channel(
    keyframes: &[Keyframe],
    prop: KeyframeProperty,
    default_value: f32,
    target_offset: RationalTime,
) -> f32 {
    let mut relevant: Vec<&Keyframe> = keyframes.iter().filter(|k| k.property == prop).collect();

    if relevant.is_empty() {
        return default_value;
    }

    // Sort keyframes using canonical semantic RationalTime ordering
    relevant.sort_by(|a, b| a.offset.semantic_cmp(&b.offset));

    let first = relevant[0];
    let last = relevant[relevant.len() - 1];

    // Hold before or at first keyframe
    if target_offset.semantic_le(&first.offset) {
        return first.value;
    }

    // Hold at or after last keyframe
    if target_offset.semantic_ge(&last.offset) {
        return last.value;
    }

    // Find enclosing segment
    for window in relevant.windows(2) {
        let k0 = window[0];
        let k1 = window[1];

        if target_offset.semantic_ge(&k0.offset) && target_offset.semantic_le(&k1.offset) {
            // Exact endpoint bypass (KF-11)
            if target_offset.is_same_time(&k0.offset) {
                return k0.value;
            }
            if target_offset.is_same_time(&k1.offset) {
                return k1.value;
            }

            // Semantic normalized progress u
            let dur_sec = (k1.offset - k0.offset).to_seconds();
            if dur_sec.abs() < 1e-9 {
                return k0.value;
            }

            let elapsed_sec = (target_offset - k0.offset).to_seconds();
            let u = (elapsed_sec / dur_sec).clamp(0.0, 1.0);
            let progress = evaluate_eased_progress(u, k0.outgoing_easing);

            return k0.value + (k1.value - k0.value) * (progress as f32);
        }
    }

    last.value
}

/// Subdivides a cubic Bézier curve at split parameter `u_split` in (0, 1) using de Casteljau's
/// algorithm and normalizes the two resulting subcurves into standard unit square [0, 1] x [0, 1].
///
/// Returns (left_easing, right_easing).
pub fn subdivide_cubic_bezier(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    u_split: f64,
) -> (KeyframeEasing, KeyframeEasing) {
    if u_split <= 0.0 {
        return (
            KeyframeEasing::Linear,
            KeyframeEasing::CubicBezier { x1, y1, x2, y2 },
        );
    }
    if u_split >= 1.0 {
        return (
            KeyframeEasing::CubicBezier { x1, y1, x2, y2 },
            KeyframeEasing::Linear,
        );
    }

    // 1. Solve parameter q_s at u_split
    let q = solve_cubic_q(u_split, x1, x2);

    // 2. de Casteljau subdivision at q:
    // P0 = (0, 0), P1 = (x1, y1), P2 = (x2, y2), P3 = (1, 1)
    let p01_x = q * x1;
    let p01_y = q * y1;

    let p12_x = (1.0 - q) * x1 + q * x2;
    let p12_y = (1.0 - q) * y1 + q * y2;

    let p23_x = (1.0 - q) * x2 + q * 1.0;
    let p23_y = (1.0 - q) * y2 + q * 1.0;

    let p012_x = (1.0 - q) * p01_x + q * p12_x;
    let p012_y = (1.0 - q) * p01_y + q * p12_y;

    let p123_x = (1.0 - q) * p12_x + q * p23_x;
    let p123_y = (1.0 - q) * p12_y + q * p23_y;

    let p0123_x = (1.0 - q) * p012_x + q * p123_x;
    let p0123_y = (1.0 - q) * p012_y + q * p123_y;

    let u_s = p0123_x;
    let e_s = p0123_y;

    // 3. Normalize left subcurve into unit square [0, 1] x [0, 1]
    let left_x1 = if u_s > 1e-9 {
        (p01_x / u_s).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let left_y1 = if e_s > 1e-9 {
        (p01_y / e_s).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let left_x2 = if u_s > 1e-9 {
        (p012_x / u_s).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let left_y2 = if e_s > 1e-9 {
        (p012_y / e_s).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // 4. Normalize right subcurve into unit square [0, 1] x [0, 1]
    let rem_u = 1.0 - u_s;
    let rem_e = 1.0 - e_s;
    let right_x1 = if rem_u > 1e-9 {
        ((p123_x - u_s) / rem_u).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let right_y1 = if rem_e > 1e-9 {
        ((p123_y - e_s) / rem_e).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let right_x2 = if rem_u > 1e-9 {
        ((p23_x - u_s) / rem_u).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let right_y2 = if rem_e > 1e-9 {
        ((p23_y - e_s) / rem_e).clamp(0.0, 1.0)
    } else {
        1.0
    };

    (
        KeyframeEasing::CubicBezier {
            x1: left_x1,
            y1: left_y1,
            x2: left_x2,
            y2: left_y2,
        },
        KeyframeEasing::CubicBezier {
            x1: right_x1,
            y1: right_y1,
            x2: right_x2,
            y2: right_y2,
        },
    )
}

/// Subdivides any supported easing variant at split parameter `u_split` in (0, 1).
pub fn subdivide_easing(easing: KeyframeEasing, u_split: f64) -> (KeyframeEasing, KeyframeEasing) {
    match easing {
        KeyframeEasing::Linear => (KeyframeEasing::Linear, KeyframeEasing::Linear),
        KeyframeEasing::EaseIn => {
            let (x1, y1, x2, y2) = EASE_IN_PARAMS;
            subdivide_cubic_bezier(x1, y1, x2, y2, u_split)
        }
        KeyframeEasing::EaseOut => {
            let (x1, y1, x2, y2) = EASE_OUT_PARAMS;
            subdivide_cubic_bezier(x1, y1, x2, y2, u_split)
        }
        KeyframeEasing::EaseInOut => {
            let (x1, y1, x2, y2) = EASE_IN_OUT_PARAMS;
            subdivide_cubic_bezier(x1, y1, x2, y2, u_split)
        }
        KeyframeEasing::CubicBezier { x1, y1, x2, y2 } => {
            subdivide_cubic_bezier(x1, y1, x2, y2, u_split)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::domain::time::FrameRate;

    #[test]
    fn test_exact_endpoints() {
        let fps = FrameRate::FPS_30;
        let kfs = vec![
            Keyframe::with_easing(
                KeyframeProperty::Opacity,
                RationalTime::new(0, fps),
                0.2,
                KeyframeEasing::EaseIn,
            ),
            Keyframe::new(KeyframeProperty::Opacity, RationalTime::new(30, fps), 0.8),
        ];

        // Before first keyframe
        assert_eq!(
            evaluate_property_channel(
                &kfs,
                KeyframeProperty::Opacity,
                1.0,
                RationalTime::new(-10, fps)
            ),
            0.2
        );
        // Exact start
        assert_eq!(
            evaluate_property_channel(
                &kfs,
                KeyframeProperty::Opacity,
                1.0,
                RationalTime::new(0, fps)
            ),
            0.2
        );
        // Exact end
        assert_eq!(
            evaluate_property_channel(
                &kfs,
                KeyframeProperty::Opacity,
                1.0,
                RationalTime::new(30, fps)
            ),
            0.8
        );
        // After last keyframe
        assert_eq!(
            evaluate_property_channel(
                &kfs,
                KeyframeProperty::Opacity,
                1.0,
                RationalTime::new(50, fps)
            ),
            0.8
        );
    }

    #[test]
    fn test_presets_evaluation() {
        // EaseIn at u = 0.5: should be less than 0.5 (slow start)
        let ease_in_mid = evaluate_eased_progress(0.5, KeyframeEasing::EaseIn);
        assert!(
            ease_in_mid < 0.5,
            "EaseIn at 0.5 must be < 0.5, got {}",
            ease_in_mid
        );

        // EaseOut at u = 0.5: should be greater than 0.5 (fast start, decelerating)
        let ease_out_mid = evaluate_eased_progress(0.5, KeyframeEasing::EaseOut);
        assert!(
            ease_out_mid > 0.5,
            "EaseOut at 0.5 must be > 0.5, got {}",
            ease_out_mid
        );

        // EaseInOut at u = 0.5: should be approximately 0.5 by symmetry
        let ease_in_out_mid = evaluate_eased_progress(0.5, KeyframeEasing::EaseInOut);
        assert!(
            (ease_in_out_mid - 0.5).abs() < 1e-4,
            "EaseInOut at 0.5 must be ~0.5, got {}",
            ease_in_out_mid
        );

        // Linear at u = 0.5: exactly 0.5
        let linear_mid = evaluate_eased_progress(0.5, KeyframeEasing::Linear);
        assert_eq!(linear_mid, 0.5);
    }

    #[test]
    fn test_subdivision_continuity() {
        // Verify that subdividing an EaseInOut curve at u_split = 0.4 preserves function values
        let (left_curve, right_curve) = subdivide_easing(KeyframeEasing::EaseInOut, 0.4);

        // Check 20 sample points in [0, 0.4] on left subcurve
        for i in 0..=20 {
            let u = 0.4 * (i as f64 / 20.0);
            let expected = evaluate_eased_progress(u, KeyframeEasing::EaseInOut);

            let u_left = if u > 0.0 { u / 0.4 } else { 0.0 };
            let e_left = evaluate_eased_progress(u_left, left_curve);
            let e_split = evaluate_eased_progress(0.4, KeyframeEasing::EaseInOut);
            let actual = e_left * e_split;

            assert!(
                (actual - expected).abs() < 1e-4,
                "Left subcurve mismatch at u={}: actual {} vs expected {}",
                u,
                actual,
                expected
            );
        }

        // Check 20 sample points in [0.4, 1.0] on right subcurve
        for i in 0..=20 {
            let u = 0.4 + 0.6 * (i as f64 / 20.0);
            let expected = evaluate_eased_progress(u, KeyframeEasing::EaseInOut);

            let u_right = if u > 0.4 { (u - 0.4) / 0.6 } else { 0.0 };
            let e_right = evaluate_eased_progress(u_right, right_curve);
            let e_split = evaluate_eased_progress(0.4, KeyframeEasing::EaseInOut);
            let actual = e_split + e_right * (1.0 - e_split);

            assert!(
                (actual - expected).abs() < 1e-4,
                "Right subcurve mismatch at u={}: actual {} vs expected {}",
                u,
                actual,
                expected
            );
        }
    }
}
