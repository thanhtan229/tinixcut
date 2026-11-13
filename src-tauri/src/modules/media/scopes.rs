//! Video scopes (histogram, waveform, vectorscope) computed from one RGBA frame.
//!
//! Scopes describe what the export will contain: callers render the frame through the export
//! composition and pass the RGBA bytes here. This module is pure analysis; it never touches a
//! project. The scope set follows the usual colorist tools (a comparable set is implemented in
//! Clypra's `scopes.rs`, MIT, see `THIRD_PARTY_NOTICES.md`); the code here is independent and
//! uses BT.709 luma / chroma to match the HD video TinixCut exports.

use serde::{Deserialize, Serialize};

pub const SCOPE_LEVELS: usize = 256;
pub const WAVEFORM_COLUMNS: usize = 256;
pub const VECTORSCOPE_SIZE: usize = 256;
/// Frames larger than this are rejected so a bad request cannot allocate unbounded memory.
pub const MAX_SCOPE_PIXELS: usize = 8192 * 4608;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Histogram {
    pub red: Vec<u32>,
    pub green: Vec<u32>,
    pub blue: Vec<u32>,
    pub luma: Vec<u32>,
}

/// Brightness statistics derived from the luma histogram.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LumaStats {
    pub min: u8,
    pub max: u8,
    pub mean: f32,
    /// Share of pixels at luma 0..=4 (crushed blacks), 0..=1.
    pub black_clip_share: f32,
    /// Share of pixels at luma 251..=255 (clipped highlights), 0..=1.
    pub white_clip_share: f32,
}

/// Column-major intensity maps: `data[column * SCOPE_LEVELS + level]`, normalized to 0..=255.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Waveform {
    pub columns: usize,
    pub luma: Vec<u8>,
    pub red: Vec<u8>,
    pub green: Vec<u8>,
    pub blue: Vec<u8>,
}

/// Row-major `VECTORSCOPE_SIZE`² density map in the Cb/Cr plane (Cb to the right, Cr up).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vectorscope {
    pub size: usize,
    pub density: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrameScopes {
    pub width: u32,
    pub height: u32,
    pub histogram: Histogram,
    pub luma_stats: LumaStats,
    pub waveform: Waveform,
    pub vectorscope: Vectorscope,
}

fn luma709(r: u8, g: u8, b: u8) -> f32 {
    0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32
}

/// Square-root intensity so sparse traces stay visible next to dense ones.
fn normalize(counts: &[u32]) -> Vec<u8> {
    let max = counts.iter().copied().max().unwrap_or(0);
    if max == 0 {
        return vec![0; counts.len()];
    }
    counts
        .iter()
        .map(|&c| {
            if c == 0 {
                0
            } else {
                (((c as f32 / max as f32).sqrt()) * 255.0).round().max(1.0) as u8
            }
        })
        .collect()
}

pub fn compute_scopes(rgba: &[u8], width: u32, height: u32) -> Result<FrameScopes, String> {
    let pixels = (width as usize)
        .checked_mul(height as usize)
        .filter(|&p| p > 0 && p <= MAX_SCOPE_PIXELS)
        .ok_or_else(|| format!("Unsupported frame size {width}x{height} for scopes"))?;
    if rgba.len() != pixels * 4 {
        return Err(format!(
            "Frame has {} bytes; {width}x{height} RGBA needs {}",
            rgba.len(),
            pixels * 4
        ));
    }

    let mut hist = [[0u32; SCOPE_LEVELS]; 4]; // r, g, b, luma
    let columns = WAVEFORM_COLUMNS.min(width as usize);
    let mut wave = vec![vec![0u32; columns * SCOPE_LEVELS]; 4];
    let mut vector = vec![0u32; VECTORSCOPE_SIZE * VECTORSCOPE_SIZE];

    for (index, px) in rgba.chunks_exact(4).enumerate() {
        let (r, g, b) = (px[0], px[1], px[2]);
        let y = luma709(r, g, b);
        let luma = y.round().clamp(0.0, 255.0) as usize;
        hist[0][r as usize] += 1;
        hist[1][g as usize] += 1;
        hist[2][b as usize] += 1;
        hist[3][luma] += 1;

        let x = index % width as usize;
        let column = x * columns / width as usize;
        wave[0][column * SCOPE_LEVELS + luma] += 1;
        wave[1][column * SCOPE_LEVELS + r as usize] += 1;
        wave[2][column * SCOPE_LEVELS + g as usize] += 1;
        wave[3][column * SCOPE_LEVELS + b as usize] += 1;

        // BT.709 full-range chroma, centred so neutral grey lands in the middle.
        let cb = (b as f32 - y) / 1.8556;
        let cr = (r as f32 - y) / 1.5748;
        let vx = (cb + 128.0).round().clamp(0.0, 255.0) as usize;
        let vy = (128.0 - cr).round().clamp(0.0, 255.0) as usize;
        vector[vy * VECTORSCOPE_SIZE + vx] += 1;
    }

    let total = pixels as f32;
    let luma_hist = &hist[3];
    let min = luma_hist.iter().position(|&c| c > 0).unwrap_or(0) as u8;
    let max = luma_hist.iter().rposition(|&c| c > 0).unwrap_or(0) as u8;
    let mean = luma_hist
        .iter()
        .enumerate()
        .map(|(level, &count)| level as f32 * count as f32)
        .sum::<f32>()
        / total;
    let share = |range: std::ops::RangeInclusive<usize>| {
        luma_hist[range].iter().map(|&c| c as f32).sum::<f32>() / total
    };

    Ok(FrameScopes {
        width,
        height,
        histogram: Histogram {
            red: hist[0].to_vec(),
            green: hist[1].to_vec(),
            blue: hist[2].to_vec(),
            luma: hist[3].to_vec(),
        },
        luma_stats: LumaStats {
            min,
            max,
            mean,
            black_clip_share: share(0..=4),
            white_clip_share: share(251..=255),
        },
        waveform: Waveform {
            columns,
            luma: normalize(&wave[0]),
            red: normalize(&wave[1]),
            green: normalize(&wave[2]),
            blue: normalize(&wave[3]),
        },
        vectorscope: Vectorscope {
            size: VECTORSCOPE_SIZE,
            density: normalize(&vector),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: u32, height: u32, rgb: [u8; 3]) -> Vec<u8> {
        std::iter::repeat([rgb[0], rgb[1], rgb[2], 255])
            .take((width * height) as usize)
            .flatten()
            .collect()
    }

    #[test]
    fn solid_color_fills_one_bin_per_channel() {
        let scopes = compute_scopes(&solid(32, 16, [200, 100, 50]), 32, 16).unwrap();
        let pixels = 32 * 16;
        assert_eq!(scopes.histogram.red[200], pixels);
        assert_eq!(scopes.histogram.green[100], pixels);
        assert_eq!(scopes.histogram.blue[50], pixels);
        assert_eq!(scopes.histogram.red.iter().sum::<u32>(), pixels);
        let luma = (0.2126f32 * 200.0 + 0.7152 * 100.0 + 0.0722 * 50.0).round() as usize;
        assert_eq!(scopes.histogram.luma[luma], pixels);
        assert_eq!(scopes.luma_stats.min as usize, luma);
        assert_eq!(scopes.luma_stats.max as usize, luma);
        assert!((scopes.luma_stats.mean - luma as f32).abs() < 1e-3);
    }

    #[test]
    fn waveform_traces_the_luma_row_in_every_column() {
        let scopes = compute_scopes(&solid(64, 8, [255, 255, 255]), 64, 8).unwrap();
        assert_eq!(scopes.waveform.columns, 64);
        for column in 0..64 {
            let col = &scopes.waveform.luma[column * SCOPE_LEVELS..(column + 1) * SCOPE_LEVELS];
            assert_eq!(col[255], 255);
            assert_eq!(col.iter().filter(|&&v| v > 0).count(), 1);
        }
        assert!((scopes.luma_stats.white_clip_share - 1.0).abs() < 1e-6);
        assert_eq!(scopes.luma_stats.black_clip_share, 0.0);
    }

    #[test]
    fn horizontal_ramp_draws_a_diagonal_waveform() {
        let (w, h) = (256u32, 4u32);
        let mut frame = Vec::new();
        for _ in 0..h {
            for x in 0..w {
                frame.extend_from_slice(&[x as u8, x as u8, x as u8, 255]);
            }
        }
        let scopes = compute_scopes(&frame, w, h).unwrap();
        for column in [0usize, 64, 128, 255] {
            let col = &scopes.waveform.luma[column * SCOPE_LEVELS..(column + 1) * SCOPE_LEVELS];
            let level = col.iter().position(|&v| v > 0).unwrap();
            assert_eq!(level, column, "ramp level at column {column}");
        }
    }

    #[test]
    fn vectorscope_places_neutral_in_the_centre_and_red_above_it() {
        let grey = compute_scopes(&solid(8, 8, [128, 128, 128]), 8, 8).unwrap();
        let centre = 128 * VECTORSCOPE_SIZE + 128;
        assert!(grey.vectorscope.density[centre] > 0);

        let red = compute_scopes(&solid(8, 8, [255, 0, 0]), 8, 8).unwrap();
        let hit = red.vectorscope.density.iter().position(|&v| v > 0).unwrap();
        let (x, y) = (hit % VECTORSCOPE_SIZE, hit / VECTORSCOPE_SIZE);
        assert!(
            y < 16,
            "red has the maximum Cr, so it sits at the top (y={y})"
        );
        assert!(
            x < 128,
            "red has negative Cb, so it sits left of centre (x={x})"
        );
    }

    #[test]
    fn rejects_bad_sizes_and_lengths() {
        assert!(compute_scopes(&[], 0, 0).is_err());
        assert!(compute_scopes(&[0; 8], 2, 2).is_err());
        assert!(compute_scopes(&[], 100_000, 100_000).is_err());
    }

    #[test]
    fn different_frames_give_different_scopes_negative_control() {
        let a = compute_scopes(&solid(16, 16, [10, 10, 10]), 16, 16).unwrap();
        let b = compute_scopes(&solid(16, 16, [240, 240, 240]), 16, 16).unwrap();
        assert_ne!(a.histogram.luma, b.histogram.luma);
        assert_ne!(a.waveform.luma, b.waveform.luma);
        assert!(a.luma_stats.mean < 20.0 && b.luma_stats.mean > 230.0);
    }
}
