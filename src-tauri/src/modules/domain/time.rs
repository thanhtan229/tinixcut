//! Canonical Time Representation for TinixCut
//! Architecture Decision Gate A: RationalTime { value, rate } vs Integer Ticks
//!
//! Provides mathematically exact time representation for non-integer frame rates
//! (23.976, 29.97, 59.94) as well as integer frame rates (24, 25, 30, 50, 60).
//! Prevents frame drift and precision loss in multi-track editing, trimming, and splitting.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, Sub};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FrameRate {
    pub numerator: u32,
    pub denominator: u32,
}

impl FrameRate {
    pub const fn new(numerator: u32, denominator: u32) -> Self {
        assert!(denominator > 0, "FrameRate denominator cannot be 0");
        Self {
            numerator,
            denominator,
        }
    }

    // Common standard video frame rates
    pub const FPS_23_976: Self = Self::new(24000, 1001);
    pub const FPS_24: Self = Self::new(24, 1);
    pub const FPS_25: Self = Self::new(25, 1);
    pub const FPS_29_97: Self = Self::new(30000, 1001);
    pub const FPS_30: Self = Self::new(30, 1);
    pub const FPS_50: Self = Self::new(50, 1);
    pub const FPS_59_94: Self = Self::new(60000, 1001);
    pub const FPS_60: Self = Self::new(60, 1);

    pub fn as_f64(&self) -> f64 {
        self.numerator as f64 / self.denominator as f64
    }
}

impl fmt::Display for FrameRate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.denominator == 1 {
            write!(f, "{}fps", self.numerator)
        } else {
            write!(
                f,
                "{:.3}fps ({}/{})",
                self.as_f64(),
                self.numerator,
                self.denominator
            )
        }
    }
}

/// Rational canonical time representation.
/// Represents `value / (rate.numerator / rate.denominator)` seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RationalTime {
    pub value: i64,
    pub rate: FrameRate,
}

impl RationalTime {
    pub const fn new(value: i64, rate: FrameRate) -> Self {
        Self { value, rate }
    }

    pub const fn zero(rate: FrameRate) -> Self {
        Self { value: 0, rate }
    }

    pub const fn from_frames(frames: i64, rate: FrameRate) -> Self {
        Self {
            value: frames,
            rate,
        }
    }

    pub fn to_seconds(&self) -> f64 {
        if self.rate.numerator == 0 {
            return 0.0;
        }
        (self.value as f64 * self.rate.denominator as f64) / self.rate.numerator as f64
    }

    pub fn from_seconds(seconds: f64, rate: FrameRate) -> Self {
        let frames = (seconds * rate.numerator as f64 / rate.denominator as f64).round() as i64;
        Self {
            value: frames,
            rate,
        }
    }

    /// Step forward or backward by `frames` count
    pub fn step_by(&self, frames: i64) -> Self {
        Self {
            value: self.value + frames,
            rate: self.rate,
        }
    }

    /// Convert this rational time to another frame rate with exact integer math and proper rounding.
    pub fn rescaled_to(&self, target_rate: FrameRate) -> Self {
        if self.rate == target_rate {
            return *self;
        }

        // value * (target_num / target_den) / (self_num / self_den)
        // = value * target_num * self_den / (self_num * target_den)
        let num_factor = (target_rate.numerator as i128) * (self.rate.denominator as i128);
        let den_factor = (self.rate.numerator as i128) * (target_rate.denominator as i128);

        if den_factor == 0 {
            return Self::zero(target_rate);
        }

        let product = (self.value as i128) * num_factor;
        // Half-up rounding for positive, half-down for negative
        let rounded = if product >= 0 {
            (product + (den_factor / 2)) / den_factor
        } else {
            (product - (den_factor / 2)) / den_factor
        };

        Self {
            value: rounded as i64,
            rate: target_rate,
        }
    }

    /// Format to SMPTE-style timecode HH:MM:SS:FF for human display
    pub fn to_timecode(&self) -> String {
        let total_frames = self.value.max(0);
        let fps = (self.rate.as_f64()).round() as i64;
        if fps == 0 {
            return "00:00:00:00".to_string();
        }
        let ff = total_frames % fps;
        let total_seconds = total_frames / fps;
        let ss = total_seconds % 60;
        let total_minutes = total_seconds / 60;
        let mm = total_minutes % 60;
        let hh = total_minutes / 60;
        format!("{:02}:{:02}:{:02}:{:02}", hh, mm, ss, ff)
    }

    /// Checks whether two `RationalTime` instances represent the exact same semantic temporal instant,
    /// regardless of differences in frame rate representation.
    ///
    /// Uses exact 128-bit cross-multiplication without floating-point drift or truncation.
    pub fn is_same_time(&self, other: &Self) -> bool {
        self.semantic_cmp(other) == std::cmp::Ordering::Equal
    }

    /// Alias for [`is_same_time`].
    pub fn semantic_eq(&self, other: &Self) -> bool {
        self.is_same_time(other)
    }

    /// Semantically compares this time with `other` using exact 128-bit cross-multiplication,
    /// avoiding floating-point imprecision and rounding artifacts across mixed frame rates.
    pub fn semantic_cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.rate == other.rate {
            self.value.cmp(&other.value)
        } else {
            let lhs = (self.value as i128)
                * (self.rate.denominator as i128)
                * (other.rate.numerator as i128);
            let rhs = (other.value as i128)
                * (other.rate.denominator as i128)
                * (self.rate.numerator as i128);
            lhs.cmp(&rhs)
        }
    }

    /// Returns `true` if `self` is semantically greater than or equal to `other` (self >= other).
    pub fn semantic_ge(&self, other: &Self) -> bool {
        self.semantic_cmp(other) != std::cmp::Ordering::Less
    }

    /// Returns `true` if `self` is semantically greater than `other` (self > other).
    pub fn semantic_gt(&self, other: &Self) -> bool {
        self.semantic_cmp(other) == std::cmp::Ordering::Greater
    }

    /// Returns `true` if `self` is semantically less than or equal to `other` (self <= other).
    pub fn semantic_le(&self, other: &Self) -> bool {
        self.semantic_cmp(other) != std::cmp::Ordering::Greater
    }

    /// Returns `true` if `self` is semantically less than `other` (self < other).
    pub fn semantic_lt(&self, other: &Self) -> bool {
        self.semantic_cmp(other) == std::cmp::Ordering::Less
    }

    /// Adds two instants without rounding either operand to the other's rate.
    ///
    /// Timeline speed can map one timeline frame to a fractional source-frame
    /// instant. Keeping the result exact here prevents preview and export from
    /// drifting apart at 0.5x speed.
    pub fn add_exact(&self, other: &Self) -> Self {
        let lhs_num = self.value as i128 * self.rate.denominator as i128;
        let lhs_den = self.rate.numerator as i128;
        let rhs_num = other.value as i128 * other.rate.denominator as i128;
        let rhs_den = other.rate.numerator as i128;

        let common_den = lhs_den
            .checked_mul(rhs_den)
            .expect("canonical time denominator overflow");
        let numerator = lhs_num
            .checked_mul(rhs_den)
            .and_then(|value| {
                rhs_num
                    .checked_mul(lhs_den)
                    .and_then(|rhs| value.checked_add(rhs))
            })
            .expect("canonical time numerator overflow");
        let divisor = gcd_i128(numerator, common_den);
        let reduced_num = numerator / divisor;
        let reduced_den = common_den / divisor;

        assert!(
            reduced_num >= i64::MIN as i128
                && reduced_num <= i64::MAX as i128
                && reduced_den > 0
                && reduced_den <= u32::MAX as i128,
            "canonical time is out of range"
        );
        Self::new(reduced_num as i64, FrameRate::new(reduced_den as u32, 1))
    }

    pub fn sub_exact(&self, other: &Self) -> Self {
        self.add_exact(&Self::new(-other.value, other.rate))
    }
}

fn gcd_i128(mut lhs: i128, mut rhs: i128) -> i128 {
    lhs = lhs.abs();
    rhs = rhs.abs();
    while rhs != 0 {
        let remainder = lhs % rhs;
        lhs = rhs;
        rhs = remainder;
    }
    lhs.max(1)
}

impl std::fmt::Display for RationalTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}/{} ({:.3}s)",
            self.value,
            self.rate.as_f64(),
            self.to_seconds()
        )
    }
}

/// Canonical representation ordering for RationalTime.
/// Invariant: `a.cmp(&b) == Ordering::Equal <=> a == b`.
///
/// For timeline/domain semantic comparisons across differing frame rates, use explicit semantic helpers:
/// [`RationalTime::semantic_cmp`], [`RationalTime::semantic_ge`], [`RationalTime::semantic_lt`], etc.
impl PartialOrd for RationalTime {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RationalTime {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.value
            .cmp(&other.value)
            .then_with(|| self.rate.cmp(&other.rate))
    }
}

impl Add for RationalTime {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        if self.rate == rhs.rate {
            Self {
                value: self.value + rhs.value,
                rate: self.rate,
            }
        } else {
            let rhs_rescaled = rhs.rescaled_to(self.rate);
            Self {
                value: self.value + rhs_rescaled.value,
                rate: self.rate,
            }
        }
    }
}

impl Sub for RationalTime {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        if self.rate == rhs.rate {
            Self {
                value: self.value - rhs.value,
                rate: self.rate,
            }
        } else {
            let rhs_rescaled = rhs.rescaled_to(self.rate);
            Self {
                value: self.value - rhs_rescaled.value,
                rate: self.rate,
            }
        }
    }
}

/// Canonical time range with start and duration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: RationalTime,
    pub duration: RationalTime,
}

impl TimeRange {
    pub fn new(start: RationalTime, duration: RationalTime) -> Self {
        assert!(duration.value >= 0, "Duration must be non-negative");
        let duration_aligned = if start.rate == duration.rate {
            duration
        } else {
            duration.rescaled_to(start.rate)
        };
        Self {
            start,
            duration: duration_aligned,
        }
    }

    pub fn end(&self) -> RationalTime {
        self.start + self.duration
    }

    pub fn contains_time(&self, time: RationalTime) -> bool {
        time.semantic_ge(&self.start) && time.semantic_lt(&self.end())
    }

    pub fn overlaps(&self, other: &TimeRange) -> bool {
        self.start.semantic_lt(&other.end()) && other.start.semantic_lt(&self.end())
    }

    pub fn rescaled_to(&self, rate: FrameRate) -> Self {
        Self {
            start: self.start.rescaled_to(rate),
            duration: self.duration.rescaled_to(rate),
        }
    }

    /// Checks whether this time range is immediately adjacent to `other` (self.end == other.start).
    /// Uses exact semantic time equality, safe across mixed frame rates.
    pub fn is_adjacent_to(&self, other: &Self) -> bool {
        self.end().is_same_time(&other.start)
    }

    /// Checks whether this time range is adjacent to `other` either via exact semantic time
    /// or via frame-tick equality when rescaled to `target_rate`.
    pub fn is_adjacent_to_with_rate(&self, other: &Self, target_rate: FrameRate) -> bool {
        self.end().is_same_time(&other.start)
            || self.end().rescaled_to(target_rate).value
                == other.start.rescaled_to(target_rate).value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gate_a_frame_rate_constants() {
        assert_eq!(FrameRate::FPS_24.as_f64(), 24.0);
        assert_eq!(FrameRate::FPS_25.as_f64(), 25.0);
        assert_eq!(FrameRate::FPS_30.as_f64(), 30.0);
        assert_eq!(FrameRate::FPS_50.as_f64(), 50.0);
        assert_eq!(FrameRate::FPS_60.as_f64(), 60.0);

        // NTSC drop/non-integer rates
        let diff_23_976 = (FrameRate::FPS_23_976.as_f64() - 23.976023976).abs();
        assert!(diff_23_976 < 1e-6);

        let diff_29_97 = (FrameRate::FPS_29_97.as_f64() - 29.97002997).abs();
        assert!(diff_29_97 < 1e-6);

        let diff_59_94 = (FrameRate::FPS_59_94.as_f64() - 59.94005994).abs();
        assert!(diff_59_94 < 1e-6);
    }

    #[test]
    fn test_gate_a_frame_stepping() {
        let t = RationalTime::from_frames(100, FrameRate::FPS_30);
        let t_next = t.step_by(1);
        assert_eq!(t_next.value, 101);
        let t_prev = t.step_by(-1);
        assert_eq!(t_prev.value, 99);
    }

    #[test]
    fn test_gate_a_range_arithmetic() {
        let start = RationalTime::from_frames(30, FrameRate::FPS_30);
        let duration = RationalTime::from_frames(60, FrameRate::FPS_30);
        let range = TimeRange::new(start, duration);

        assert_eq!(range.end().value, 90);
        assert_eq!(range.end() - start, duration);
        assert_eq!(start + duration, range.end());

        assert!(range.contains_time(RationalTime::from_frames(30, FrameRate::FPS_30)));
        assert!(range.contains_time(RationalTime::from_frames(89, FrameRate::FPS_30)));
        assert!(!range.contains_time(RationalTime::from_frames(90, FrameRate::FPS_30)));
    }

    #[test]
    fn test_gate_a_trim_split_boundary_preservation() {
        // Clip duration: 100 frames at 30fps
        let origin_start = RationalTime::from_frames(0, FrameRate::FPS_30);
        let origin_dur = RationalTime::from_frames(100, FrameRate::FPS_30);
        let origin_range = TimeRange::new(origin_start, origin_dur);

        // Split at frame 42
        let split_point = RationalTime::from_frames(42, FrameRate::FPS_30);
        let part1 = TimeRange::new(origin_start, split_point - origin_start);
        let part2 = TimeRange::new(split_point, origin_range.end() - split_point);

        assert_eq!(part1.duration.value, 42);
        assert_eq!(part2.duration.value, 58);
        assert_eq!(part1.duration + part2.duration, origin_dur);
        assert_eq!(part1.end(), part2.start);
        assert_eq!(part2.end(), origin_range.end());
    }

    #[test]
    fn test_gate_a_conversions_across_rates() {
        // Exactly 1 second at 24 fps is 24 frames
        let one_sec_24 = RationalTime::from_frames(24, FrameRate::FPS_24);
        // Rescaled to 30 fps must be exactly 30 frames
        let one_sec_30 = one_sec_24.rescaled_to(FrameRate::FPS_30);
        assert_eq!(one_sec_30.value, 30);
        // Rescaled to 60 fps must be exactly 60 frames
        let one_sec_60 = one_sec_24.rescaled_to(FrameRate::FPS_60);
        assert_eq!(one_sec_60.value, 60);
        // Rescaled to 25 fps must be exactly 25 frames
        let one_sec_25 = one_sec_24.rescaled_to(FrameRate::FPS_25);
        assert_eq!(one_sec_25.value, 25);
        // Rescaled to 50 fps must be exactly 50 frames
        let one_sec_50 = one_sec_24.rescaled_to(FrameRate::FPS_50);
        assert_eq!(one_sec_50.value, 50);

        // Exactly 1001 frames of 23.976 fps (24000/1001) = exactly 1001 * 1001 / 24000 s = (1001^2)/24000 s
        // In 24000 ticks it is exact.
        // Test round trip 30fps -> 60fps -> 30fps
        let t30 = RationalTime::from_frames(45, FrameRate::FPS_30);
        let t60 = t30.rescaled_to(FrameRate::FPS_60);
        assert_eq!(t60.value, 90);
        let back_to_30 = t60.rescaled_to(FrameRate::FPS_30);
        assert_eq!(back_to_30.value, 45);
    }

    #[test]
    fn test_gate_a_serialization_round_trip() {
        let t = RationalTime::from_frames(12345, FrameRate::FPS_29_97);
        let json = serde_json::to_string(&t).expect("serialize RationalTime");
        let deserialized: RationalTime =
            serde_json::from_str(&json).expect("deserialize RationalTime");
        assert_eq!(t, deserialized);

        let range = TimeRange::new(t, RationalTime::from_frames(300, FrameRate::FPS_29_97));
        let range_json = serde_json::to_string(&range).expect("serialize TimeRange");
        let deserialized_range: TimeRange =
            serde_json::from_str(&range_json).expect("deserialize TimeRange");
        assert_eq!(range, deserialized_range);
    }

    #[test]
    fn test_gate_a_timecode_formatting() {
        let t = RationalTime::from_frames(3665, FrameRate::FPS_30); // 3665 frames at 30 fps = 122 sec + 5 frames = 2m 2s 5f
        assert_eq!(t.to_timecode(), "00:02:02:05");
    }

    #[test]
    fn test_semantic_equality_across_rates() {
        let t_30fps = RationalTime::from_frames(60, FrameRate::FPS_30); // 2.0s
        let t_60fps = RationalTime::from_frames(120, FrameRate::FPS_60); // 2.0s
        let t_24fps = RationalTime::from_frames(48, FrameRate::FPS_24); // 2.0s

        // Raw derived PartialEq fails because (value, rate) tuples differ
        assert_ne!(t_30fps, t_60fps);
        assert_ne!(t_30fps, t_24fps);

        // Semantic equality succeeds
        assert!(t_30fps.is_same_time(&t_60fps));
        assert!(t_30fps.is_same_time(&t_24fps));
        assert!(t_60fps.is_same_time(&t_24fps));
        assert!(t_30fps.semantic_eq(&t_60fps));

        // Range adjacency across mixed rates
        let r1 = TimeRange::new(
            RationalTime::zero(FrameRate::FPS_30),
            RationalTime::from_frames(60, FrameRate::FPS_30), // ends at 2.0s (60@30fps)
        );
        let r2 = TimeRange::new(
            RationalTime::from_frames(120, FrameRate::FPS_60), // starts at 2.0s (120@60fps)
            RationalTime::from_frames(60, FrameRate::FPS_60),
        );
        assert!(r1.is_adjacent_to(&r2));
        assert!(r1.is_adjacent_to_with_rate(&r2, FrameRate::FPS_30));
    }

    #[test]
    fn test_representation_vs_semantic_equality_and_btreeset_contract() {
        let a = RationalTime::new(100, FrameRate::FPS_25); // 4.0s
        let b = RationalTime::new(120, FrameRate::FPS_30); // 4.0s

        // 1. Representation equality: must be false (different value and rate representations)
        assert_ne!(a, b);
        assert!(!(a == b));

        // 2. Semantic equality and comparison helpers: must be exact
        assert_eq!(a.semantic_cmp(&b), std::cmp::Ordering::Equal);
        assert!(a.semantic_eq(&b));
        assert!(a.is_same_time(&b));
        assert!(a.semantic_ge(&b));
        assert!(a.semantic_le(&b));
        assert!(!a.semantic_lt(&b));
        assert!(!a.semantic_gt(&b));

        // 3. Rust Ord / Eq contract consistency invariant:
        // `a.cmp(&b) == Ordering::Equal <=> a == b`
        // Since a != b, a.cmp(&b) MUST NOT be Ordering::Equal!
        assert_ne!(a.cmp(&b), std::cmp::Ordering::Equal);

        // 4. Ordered collection (BTreeSet) regression test:
        // Two representation-distinct RationalTimes that represent the same semantic instant
        // must NOT be accidentally collapsed in a BTreeSet.
        let mut set = std::collections::BTreeSet::new();
        set.insert(a);
        set.insert(b);
        assert_eq!(
            set.len(),
            2,
            "BTreeSet must not collapse distinct representations of the same semantic time"
        );
        assert!(set.contains(&a));
        assert!(set.contains(&b));
    }
}
