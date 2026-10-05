//! Derived Waveform Data and Cache Types
//! Architecture Invariant: Disposable, derived, non-authoritative.
//! Deleting waveform cache never corrupts the project.

use serde::{Deserialize, Serialize};

pub const WAVEFORM_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_WAVEFORM_BUCKET_RATE: u32 = 50; // 50 buckets per second

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WaveformData {
    pub schema_version: u32,
    pub asset_id: String,
    pub source_duration: f64,
    pub bucket_rate: u32, // Number of buckets per second
    pub channels: u16,    // Mono mixdown = 1
    /// Pairs of [min, max] per bucket, normalized to [-1.0, 1.0].
    /// Length of peaks is `num_buckets * 2`.
    pub peaks: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", content = "payload")]
pub enum WaveformStatus {
    Ready(WaveformData),
    Pending { progress: f32 },
    Unavailable { reason: String },
}
