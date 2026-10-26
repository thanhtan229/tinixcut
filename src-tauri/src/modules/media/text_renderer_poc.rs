// Text Renderer POC Module — High-Fidelity Shared Text Overlay Engine
// Pure Rust OpenType font parsing, glyph & cluster rasterization, kerning, SHA-256 font verification,
// multi-stop directional linear gradient, concentric multi-stroke dilation, multi-shadow with separable blur,
// background box & decorative shape layers, per-character 2D affine motion (translation + Z-rotation around
// cluster pivot + scale + opacity), Unicode NFC normalization, multi-line text wrapping, bounded 256 MiB
// glyph/primitive cache, dirty ROI computation, Tauri IPC preview seam, and bounded FFmpeg export stream.

use ab_glyph::{Font, FontRef, Glyph, GlyphId, Point, PxScale, ScaleFont};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::io::Write;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

/// Default maximum memory ceiling for the glyph/primitive mask cache (256 MiB).
pub const DEFAULT_CACHE_MAX_BYTES: usize = 256 * 1024 * 1024;

/// RGBA 8-bit uncompressed straight-alpha raster buffer.
#[derive(Debug, Clone)]
pub struct RgbaBuffer {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl RgbaBuffer {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![0u8; (width * height * 4) as usize],
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.data.fill(0);
    }

    #[inline]
    pub fn fill_color(&mut self, r: u8, g: u8, b: u8, a: u8) {
        for chunk in self.data.as_chunks_mut::<4>().0 {
            chunk[0] = r;
            chunk[1] = g;
            chunk[2] = b;
            chunk[3] = a;
        }
    }

    #[inline]
    pub fn get_pixel(&self, x: u32, y: u32) -> [u8; 4] {
        if x < self.width && y < self.height {
            let idx = ((y * self.width + x) * 4) as usize;
            [
                self.data[idx],
                self.data[idx + 1],
                self.data[idx + 2],
                self.data[idx + 3],
            ]
        } else {
            [0, 0, 0, 0]
        }
    }

    #[inline]
    pub fn set_pixel(&mut self, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) {
        if x < self.width && y < self.height {
            let idx = ((y * self.width + x) * 4) as usize;
            self.data[idx] = r;
            self.data[idx + 1] = g;
            self.data[idx + 2] = b;
            self.data[idx + 3] = a;
        }
    }

    /// Porter-Duff Over compositing for straight alpha with zero dark halo.
    #[inline]
    pub fn blend_pixel_over(&mut self, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) {
        if x < self.width && y < self.height && a > 0 {
            let idx = ((y * self.width + x) * 4) as usize;
            let dst_a = self.data[idx + 3];
            if dst_a == 0 {
                self.data[idx] = r;
                self.data[idx + 1] = g;
                self.data[idx + 2] = b;
                self.data[idx + 3] = a;
                return;
            }
            if a == 255 {
                self.data[idx] = r;
                self.data[idx + 1] = g;
                self.data[idx + 2] = b;
                self.data[idx + 3] = 255;
                return;
            }

            let src_a = a as f32 / 255.0;
            let dst_a_f = dst_a as f32 / 255.0;
            let out_a = src_a + dst_a_f * (1.0 - src_a);

            if out_a > 0.0 {
                let inv_out_a = 1.0 / out_a;
                let src_term = src_a * inv_out_a;
                let dst_term = dst_a_f * (1.0 - src_a) * inv_out_a;

                let out_r = (r as f32 * src_term) + (self.data[idx] as f32 * dst_term);
                let out_g = (g as f32 * src_term) + (self.data[idx + 1] as f32 * dst_term);
                let out_b = (b as f32 * src_term) + (self.data[idx + 2] as f32 * dst_term);

                self.data[idx] = (out_r.round() as u32).min(255) as u8;
                self.data[idx + 1] = (out_g.round() as u32).min(255) as u8;
                self.data[idx + 2] = (out_b.round() as u32).min(255) as u8;
                self.data[idx + 3] = (out_a * 255.0).round() as u8;
            }
        }
    }

    /// Blends another buffer on top of this buffer using Porter-Duff Over.
    pub fn blend_buffer_over(&mut self, top: &RgbaBuffer) {
        let w = self.width.min(top.width);
        let h = self.height.min(top.height);
        for y in 0..h {
            for x in 0..w {
                let top_idx = ((y * top.width + x) * 4) as usize;
                let ta = top.data[top_idx + 3];
                if ta > 0 {
                    self.blend_pixel_over(
                        x,
                        y,
                        top.data[top_idx],
                        top.data[top_idx + 1],
                        top.data[top_idx + 2],
                        ta,
                    );
                }
            }
        }
    }

    /// Crops a rectangular region `[x0, y0, x1, y1]` into a new tight `RgbaBuffer`.
    pub fn crop(&self, roi: [u32; 4]) -> RgbaBuffer {
        let x0 = roi[0].min(self.width);
        let y0 = roi[1].min(self.height);
        let x1 = roi[2].min(self.width).max(x0);
        let y1 = roi[3].min(self.height).max(y0);
        let w = (x1 - x0).max(1);
        let h = (y1 - y0).max(1);
        let mut out = RgbaBuffer::new(w, h);
        if x1 > x0 && y1 > y0 {
            for y in 0..(y1 - y0) {
                let src_start = (((y0 + y) * self.width + x0) * 4) as usize;
                let dst_start = (y * w * 4) as usize;
                let row_bytes = ((x1 - x0) * 4) as usize;
                out.data[dst_start..dst_start + row_bytes]
                    .copy_from_slice(&self.data[src_start..src_start + row_bytes]);
            }
        }
        out
    }

    /// Computes minimum axis-aligned bounding box `[x0, y0, x1, y1]` enclosing all non-transparent pixels.
    pub fn compute_dirty_roi(&self) -> Option<[u32; 4]> {
        let mut min_x = self.width;
        let mut min_y = self.height;
        let mut max_x = 0u32;
        let mut max_y = 0u32;
        let mut found = false;

        for y in 0..self.height {
            let row_start = (y * self.width * 4) as usize;
            for x in 0..self.width {
                let a = self.data[row_start + (x * 4) as usize + 3];
                if a > 0 {
                    found = true;
                    if x < min_x {
                        min_x = x;
                    }
                    if x > max_x {
                        max_x = x;
                    }
                    if y < min_y {
                        min_y = y;
                    }
                    if y > max_y {
                        max_y = y;
                    }
                }
            }
        }

        if found {
            Some([min_x, min_y, max_x + 1, max_y + 1])
        } else {
            None
        }
    }

    /// Minimal uncompressed raw PNG encoder producing standard-compliant PNG bytes.
    pub fn to_png_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.data.len() + 1024);
        out.extend_from_slice(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);

        // IHDR chunk
        let mut ihdr = Vec::with_capacity(13);
        ihdr.extend_from_slice(&self.width.to_be_bytes());
        ihdr.extend_from_slice(&self.height.to_be_bytes());
        ihdr.push(8); // 8 bits per channel
        ihdr.push(6); // RGBA
        ihdr.push(0); // deflate
        ihdr.push(0); // filter
        ihdr.push(0); // non-interlaced
        write_chunk(&mut out, b"IHDR", &ihdr);

        // IDAT chunk
        let row_stride = (self.width * 4) as usize;
        let mut raw_filtered = Vec::with_capacity((1 + row_stride) * self.height as usize);
        for y in 0..self.height {
            raw_filtered.push(0); // Filter type 0 (None)
            let start = (y * self.width * 4) as usize;
            raw_filtered.extend_from_slice(&self.data[start..start + row_stride]);
        }

        let compressed = zlib_compress_store(&raw_filtered);
        write_chunk(&mut out, b"IDAT", &compressed);

        // IEND chunk
        write_chunk(&mut out, b"IEND", &[]);
        out
    }

    /// Decodes a store-mode PNG produced by `to_png_bytes()` back into an `RgbaBuffer` for round-trip parity verification.
    pub fn from_png_bytes(png: &[u8]) -> Result<Self, String> {
        if png.len() < 33 || png[0..8] != [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
            return Err("Invalid PNG signature".to_string());
        }
        let mut pos = 8usize;
        let mut width = 0u32;
        let mut height = 0u32;
        let mut idat_payload = Vec::new();

        while pos + 12 <= png.len() {
            let len =
                u32::from_be_bytes([png[pos], png[pos + 1], png[pos + 2], png[pos + 3]]) as usize;
            let ctype = &png[pos + 4..pos + 8];
            let data_start = pos + 8;
            let data_end = data_start + len;
            if data_end + 4 > png.len() {
                return Err("Truncated PNG chunk".to_string());
            }
            let chunk_data = &png[data_start..data_end];
            let chunk_crc = u32::from_be_bytes([
                png[data_end],
                png[data_end + 1],
                png[data_end + 2],
                png[data_end + 3],
            ]);
            let mut ctype_arr = [0u8; 4];
            ctype_arr.copy_from_slice(ctype);
            let expected_crc = png_chunk_crc(&ctype_arr, chunk_data);
            if chunk_crc != expected_crc {
                return Err(format!(
                    "PNG chunk {} CRC mismatch: got {:08X}, expected {:08X}",
                    String::from_utf8_lossy(ctype),
                    chunk_crc,
                    expected_crc
                ));
            }
            if ctype == b"IHDR" && len >= 8 {
                width = u32::from_be_bytes([
                    chunk_data[0],
                    chunk_data[1],
                    chunk_data[2],
                    chunk_data[3],
                ]);
                height = u32::from_be_bytes([
                    chunk_data[4],
                    chunk_data[5],
                    chunk_data[6],
                    chunk_data[7],
                ]);
            } else if ctype == b"IDAT" {
                idat_payload.extend_from_slice(chunk_data);
            } else if ctype == b"IEND" {
                break;
            }
            pos = data_end + 4;
        }

        if width == 0 || height == 0 {
            return Err("Missing PNG dimensions".to_string());
        }

        // Decompress store-mode zlib blocks (header 2 bytes, followed by 5-byte store headers + raw blocks, 4-byte adler32)
        if idat_payload.len() < 6 {
            return Err("Invalid zlib stream".to_string());
        }
        let mut zpos = 2usize;
        let mut raw_filtered = Vec::with_capacity(((width * 4 + 1) * height) as usize);
        while zpos + 5 <= idat_payload.len().saturating_sub(4) {
            let is_last = idat_payload[zpos];
            let blen =
                u16::from_le_bytes([idat_payload[zpos + 1], idat_payload[zpos + 2]]) as usize;
            zpos += 5;
            if zpos + blen > idat_payload.len().saturating_sub(4) {
                return Err("Truncated zlib store block".to_string());
            }
            raw_filtered.extend_from_slice(&idat_payload[zpos..zpos + blen]);
            zpos += blen;
            if is_last != 0 {
                break;
            }
        }

        let row_stride = (width * 4) as usize;
        let expected_filtered = (row_stride + 1) * height as usize;
        if raw_filtered.len() < expected_filtered {
            return Err(format!(
                "Decompressed PNG size mismatch: got {}, expected {}",
                raw_filtered.len(),
                expected_filtered
            ));
        }

        let mut buf = RgbaBuffer::new(width, height);
        for y in 0..height as usize {
            let src_start = y * (row_stride + 1) + 1;
            let dst_start = y * row_stride;
            buf.data[dst_start..dst_start + row_stride]
                .copy_from_slice(&raw_filtered[src_start..src_start + row_stride]);
        }

        Ok(buf)
    }
}

/// Computes Structural Similarity Index (SSIM) between two buffers over a rectangular Region of Interest `[x0, y0, x1, y1]`.
pub fn compute_roi_ssim(a: &RgbaBuffer, b: &RgbaBuffer, roi: [u32; 4]) -> f64 {
    let x0 = roi[0].min(a.width).min(b.width);
    let y0 = roi[1].min(a.height).min(b.height);
    let x1 = roi[2].min(a.width).min(b.width).max(x0);
    let y1 = roi[3].min(a.height).min(b.height).max(y0);

    if x1 <= x0 || y1 <= y0 {
        return 1.0;
    }

    let n = ((x1 - x0) as f64) * ((y1 - y0) as f64) * 3.0; // RGB channels
    let mut sum_x = 0.0f64;
    let mut sum_y = 0.0f64;

    for y in y0..y1 {
        for x in x0..x1 {
            let pa = a.get_pixel(x, y);
            let pb = b.get_pixel(x, y);
            for c in 0..3 {
                sum_x += pa[c] as f64;
                sum_y += pb[c] as f64;
            }
        }
    }

    let mu_x = sum_x / n;
    let mu_y = sum_y / n;

    let mut var_x = 0.0f64;
    let mut var_y = 0.0f64;
    let mut cov_xy = 0.0f64;

    for y in y0..y1 {
        for x in x0..x1 {
            let pa = a.get_pixel(x, y);
            let pb = b.get_pixel(x, y);
            for c in 0..3 {
                let dx = pa[c] as f64 - mu_x;
                let dy = pb[c] as f64 - mu_y;
                var_x += dx * dx;
                var_y += dy * dy;
                cov_xy += dx * dy;
            }
        }
    }

    var_x /= n;
    var_y /= n;
    cov_xy /= n;

    let c1 = (0.01 * 255.0f64).powi(2);
    let c2 = (0.03 * 255.0f64).powi(2);

    let num = (2.0 * mu_x * mu_y + c1) * (2.0 * cov_xy + c2);
    let den = (mu_x * mu_x + mu_y * mu_y + c1) * (var_x + var_y + c2);
    num / den
}

/// Computes Mean Absolute Error (MAE) per channel over `[x0, y0, x1, y1]`.
pub fn compute_roi_mae(a: &RgbaBuffer, b: &RgbaBuffer, roi: [u32; 4]) -> f64 {
    let x0 = roi[0].min(a.width).min(b.width);
    let y0 = roi[1].min(a.height).min(b.height);
    let x1 = roi[2].min(a.width).min(b.width).max(x0);
    let y1 = roi[3].min(a.height).min(b.height).max(y0);

    if x1 <= x0 || y1 <= y0 {
        return 0.0;
    }

    let mut diff_sum = 0.0f64;
    let mut count = 0usize;
    for y in y0..y1 {
        for x in x0..x1 {
            let pa = a.get_pixel(x, y);
            let pb = b.get_pixel(x, y);
            for c in 0..4 {
                diff_sum += (pa[c] as f64 - pb[c] as f64).abs();
                count += 1;
            }
        }
    }
    diff_sum / (count.max(1) as f64)
}

fn write_chunk(out: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(chunk_type);
    out.extend_from_slice(data);
    let crc = png_chunk_crc(chunk_type, data);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// Standard PNG chunk CRC-32 computed over chunk_type followed by data.
pub fn png_chunk_crc(chunk_type: &[u8; 4], data: &[u8]) -> u32 {
    let mut crc = 0xFFFFFFFFu32;
    crc = crc32_update(crc, chunk_type);
    crc = crc32_update(crc, data);
    !crc
}

/// Standard IEEE 802.3 CRC-32 of a byte slice.
pub fn crc32_fast(data: &[u8]) -> u32 {
    !crc32_update(0xFFFFFFFF, data)
}

const fn build_crc32_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut index = 0usize;
    while index < 256 {
        let mut value = index as u32;
        let mut bit = 0;
        while bit < 8 {
            value = if (value & 1) != 0 {
                (value >> 1) ^ 0xEDB88320
            } else {
                value >> 1
            };
            bit += 1;
        }
        table[index] = value;
        index += 1;
    }
    table
}

static CRC32_TABLE: [u32; 256] = build_crc32_table();

/// Updates a running CRC32 accumulator across byte slices without inverting bits.
pub fn crc32_update(mut crc: u32, data: &[u8]) -> u32 {
    for &byte in data {
        crc = CRC32_TABLE[((crc ^ byte as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc
}

fn zlib_compress_store(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(0x78);
    out.push(0x01);

    let chunks = data.chunks(65535);
    let total_chunks = chunks.len();
    for (i, chunk) in chunks.enumerate() {
        let is_last = (i == total_chunks - 1) as u8;
        out.push(is_last);
        let len = chunk.len() as u16;
        let nlen = !len;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&nlen.to_le_bytes());
        out.extend_from_slice(chunk);
    }

    let mut s1 = 1u32;
    let mut s2 = 0u32;
    for chunk in data.chunks(5_552) {
        for &byte in chunk {
            s1 += byte as u32;
            s2 += s1;
        }
        s1 %= 65521;
        s2 %= 65521;
    }
    let adler = (s2 << 16) | s1;
    out.extend_from_slice(&adler.to_be_bytes());
    out
}

/// Computes uppercase SHA-256 hex digest of a byte slice.
pub fn compute_sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut hex, "{:02X}", b);
    }
    hex
}

/// Normalized color representation
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ColorRgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl ColorRgba {
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub fn to_u8_array(&self) -> [u8; 4] {
        [
            (self.r * 255.0).clamp(0.0, 255.0).round() as u8,
            (self.g * 255.0).clamp(0.0, 255.0).round() as u8,
            (self.b * 255.0).clamp(0.0, 255.0).round() as u8,
            (self.a * 255.0).clamp(0.0, 255.0).round() as u8,
        ]
    }
}

/// Multi-stop linear gradient definition with arbitrary angle.
#[derive(Debug, Clone)]
pub struct LinearGradient {
    pub angle_deg: f32,
    pub stops: Vec<(f32, ColorRgba)>,
}

impl LinearGradient {
    pub fn sample_at(&self, t: f32) -> ColorRgba {
        if self.stops.is_empty() {
            return ColorRgba::new(1.0, 1.0, 1.0, 1.0);
        }
        if self.stops.len() == 1 || t <= self.stops[0].0 {
            return self.stops[0].1;
        }
        let last = self.stops.len() - 1;
        if t >= self.stops[last].0 {
            return self.stops[last].1;
        }

        for i in 0..last {
            let (t0, c0) = self.stops[i];
            let (t1, c1) = self.stops[i + 1];
            if t >= t0 && t <= t1 {
                let factor = if (t1 - t0).abs() < 1e-6 {
                    0.0
                } else {
                    (t - t0) / (t1 - t0)
                };
                return ColorRgba::new(
                    c0.r + (c1.r - c0.r) * factor,
                    c0.g + (c1.g - c0.g) * factor,
                    c0.b + (c1.b - c0.b) * factor,
                    c0.a + (c1.a - c0.a) * factor,
                );
            }
        }
        self.stops[last].1
    }

    /// Samples color for 2D position within a bounding box projected onto gradient angle.
    pub fn sample_2d(&self, x: f32, y: f32, bounds: [f32; 4]) -> ColorRgba {
        let [bx0, by0, bx1, by1] = bounds;
        let bw = (bx1 - bx0).max(1.0);
        let bh = (by1 - by0).max(1.0);

        let cx = (bx0 + bx1) / 2.0;
        let cy = (by0 + by1) / 2.0;
        let rad = self.angle_deg.to_radians();
        let dir_x = rad.cos();
        let dir_y = -rad.sin(); // screen Y is inverted

        let max_proj = (bw * dir_x.abs() + bh * dir_y.abs()) / 2.0;
        let proj = (x - cx) * dir_x + (y - cy) * dir_y;
        let t = if max_proj > 1e-3 {
            ((proj / (max_proj * 2.0)) + 0.5).clamp(0.0, 1.0)
        } else {
            0.5
        };

        self.sample_at(t)
    }
}

/// Outer outline stroke definition
#[derive(Debug, Clone)]
pub struct StrokeDef {
    pub color: ColorRgba,
    pub width_px: f32,
}

/// Drop shadow definition
#[derive(Debug, Clone)]
pub struct ShadowDef {
    pub color: ColorRgba,
    pub angle_deg: f32,
    pub distance_px: f32,
    pub blur_px: f32,
}

/// Rounded rectangle background highlight box behind text layout bounds
#[derive(Debug, Clone)]
pub struct BackgroundBoxDef {
    pub color: ColorRgba,
    pub padding_x: f32,
    pub padding_y: f32,
    pub border_radius: f32,
}

/// Decorative shape / banner layer for multi-layer template compositions (e.g. W3 16-layer stress)
#[derive(Debug, Clone)]
pub struct ShapeLayerDef {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub border_radius: f32,
    pub color: ColorRgba,
}

/// Character cluster layout and animation properties
#[derive(Debug, Clone)]
pub struct CharacterLayout {
    pub grapheme: String,
    pub cluster_index: usize,
    pub base_x: f32,
    pub base_y: f32,
    pub width: f32,
    pub height: f32,
    pub transform_y: f32,
    pub rotation_z_deg: f32,
    pub scale: f32,
    pub opacity: f32,
}

/// Cubic Bézier curve evaluator
pub fn evaluate_bezier(p1x: f32, p1y: f32, p2x: f32, p2y: f32, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let mut s = t;
    for _ in 0..8 {
        let x = 3.0 * (1.0 - s) * (1.0 - s) * s * p1x + 3.0 * (1.0 - s) * s * s * p2x + s * s * s;
        let dx = 3.0 * (1.0 - s) * (1.0 - s) * p1x
            + 6.0 * (1.0 - s) * s * (p2x - p1x)
            + 3.0 * s * s * (1.0 - p2x);
        let err = x - t;
        if err.abs() < 1e-4 || dx.abs() < 1e-6 {
            break;
        }
        s -= err / dx;
        s = s.clamp(0.0, 1.0);
    }
    3.0 * (1.0 - s) * (1.0 - s) * s * p1y + 3.0 * (1.0 - s) * s * s * p2y + s * s * s
}

/// Rasterized glyph/cluster alpha mask with placement metrics
#[derive(Debug, Clone)]
pub struct GlyphAlphaMask {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub mask: Vec<u8>,
}

/// Rotates and scales an alpha mask around its local center using bilinear interpolation.
pub fn transform_alpha_mask(mask: &GlyphAlphaMask, angle_deg: f32, scale: f32) -> GlyphAlphaMask {
    if (angle_deg.abs() < 1e-2 || (angle_deg.abs() - 360.0).abs() < 1e-2)
        && (scale - 1.0).abs() < 1e-3
    {
        return mask.clone();
    }

    let rad = angle_deg.to_radians();
    let cos_a = rad.cos();
    let sin_a = rad.sin();

    let orig_w = mask.width as f32;
    let orig_h = mask.height as f32;
    let cx = orig_w / 2.0;
    let cy = orig_h / 2.0;

    // Bounding box of rotated/scaled rectangle
    let new_w = ((orig_w * cos_a.abs() + orig_h * sin_a.abs()) * scale).ceil() as u32 + 2;
    let new_h = ((orig_w * sin_a.abs() + orig_h * cos_a.abs()) * scale).ceil() as u32 + 2;

    let new_cx = new_w as f32 / 2.0;
    let new_cy = new_h as f32 / 2.0;
    let inv_scale = if scale > 1e-3 { 1.0 / scale } else { 1.0 };

    let mut out = vec![0u8; (new_w * new_h) as usize];

    for ny in 0..new_h {
        let dy = (ny as f32 + 0.5) - new_cy;
        for nx in 0..new_w {
            let dx = (nx as f32 + 0.5) - new_cx;

            // Inverse rotation + scale to find original (ox, oy)
            let ox = cx + (dx * cos_a + dy * sin_a) * inv_scale;
            let oy = cy + (-dx * sin_a + dy * cos_a) * inv_scale;

            if ox >= 0.0 && ox < orig_w - 1.0 && oy >= 0.0 && oy < orig_h - 1.0 {
                let x0 = ox.floor() as usize;
                let y0 = oy.floor() as usize;
                let x1 = x0 + 1;
                let y1 = y0 + 1;

                let fx = ox - x0 as f32;
                let fy = oy - y0 as f32;

                let v00 = mask.mask[y0 * mask.width as usize + x0] as f32;
                let v10 = mask.mask[y0 * mask.width as usize + x1] as f32;
                let v01 = mask.mask[y1 * mask.width as usize + x0] as f32;
                let v11 = mask.mask[y1 * mask.width as usize + x1] as f32;

                let val = (v00 * (1.0 - fx) * (1.0 - fy)
                    + v10 * fx * (1.0 - fy)
                    + v01 * (1.0 - fx) * fy
                    + v11 * fx * fy)
                    .round() as u8;

                out[(ny * new_w + nx) as usize] = val;
            }
        }
    }

    GlyphAlphaMask {
        x: (mask.x as f32 + cx - new_cx).round() as i32,
        y: (mask.y as f32 + cy - new_cy).round() as i32,
        width: new_w,
        height: new_h,
        mask: out,
    }
}

/// Separable box blur approximation of Gaussian blur for 2D alpha mask via O(1) prefix sums.
pub fn blur_alpha_mask(mask: &GlyphAlphaMask, blur_radius: u32) -> GlyphAlphaMask {
    if blur_radius == 0 || mask.width == 0 || mask.height == 0 {
        return mask.clone();
    }

    let r = blur_radius as i32;
    let new_w = mask.width + blur_radius * 2;
    let new_h = mask.height + blur_radius * 2;
    let mut temp = vec![0u8; (new_w * new_h) as usize];
    let mut out = vec![0u8; (new_w * new_h) as usize];

    let window_size = (2 * r + 1) as u32;

    // Horizontal pass via prefix sums
    let mut prefix = vec![0u32; (mask.width + 1) as usize];
    for y in 0..mask.height {
        let src_row = (y * mask.width) as usize;
        let dst_row = ((y + blur_radius) * new_w) as usize;

        prefix[0] = 0;
        for i in 0..mask.width as usize {
            prefix[i + 1] = prefix[i] + mask.mask[src_row + i] as u32;
        }

        for x in 0..new_w as i32 {
            let sx_start = (x - blur_radius as i32 - r).max(0) as usize;
            let sx_end = (x - blur_radius as i32 + r).min(mask.width as i32 - 1);
            let sum = if sx_end >= 0 && sx_start <= sx_end as usize {
                prefix[sx_end as usize + 1] - prefix[sx_start]
            } else {
                0
            };
            temp[dst_row + x as usize] = (sum / window_size) as u8;
        }
    }

    // Vertical pass via prefix sums
    let mut col_prefix = vec![0u32; (new_h + 1) as usize];
    for x in 0..new_w {
        col_prefix[0] = 0;
        for y in 0..new_h as usize {
            col_prefix[y + 1] = col_prefix[y] + temp[y * new_w as usize + x as usize] as u32;
        }

        for y in 0..new_h as i32 {
            let sy_start = (y - r).max(0) as usize;
            let sy_end = (y + r).min(new_h as i32 - 1);
            let sum = if sy_end >= 0 && sy_start <= sy_end as usize {
                col_prefix[sy_end as usize + 1] - col_prefix[sy_start]
            } else {
                0
            };
            out[(y as u32 * new_w + x) as usize] = (sum / window_size) as u8;
        }
    }

    GlyphAlphaMask {
        x: mask.x - blur_radius as i32,
        y: mask.y - blur_radius as i32,
        width: new_w,
        height: new_h,
        mask: out,
    }
}

/// Dilation filter for circular / rounded stroke outline around alpha mask.
pub fn dilate_alpha_mask(mask: &GlyphAlphaMask, radius: f32) -> GlyphAlphaMask {
    if radius <= 0.0 || mask.width == 0 || mask.height == 0 {
        return mask.clone();
    }

    let r_ceil = radius.ceil() as i32;
    let r_sq = radius * radius;
    let new_w = mask.width + (r_ceil * 2) as u32;
    let new_h = mask.height + (r_ceil * 2) as u32;
    let mut out = vec![0u8; (new_w * new_h) as usize];

    // Precompute circle footprint offsets
    let mut circle_offsets = Vec::new();
    for dy in -r_ceil..=r_ceil {
        for dx in -r_ceil..=r_ceil {
            if (dx * dx + dy * dy) as f32 <= r_sq {
                circle_offsets.push(((dx + r_ceil) as usize, (dy + r_ceil) as usize));
            }
        }
    }

    for y in 0..mask.height as usize {
        let src_row = y * mask.width as usize;
        for x in 0..mask.width as usize {
            let src_val = mask.mask[src_row + x];
            if src_val > 0 {
                for &(off_x, off_y) in &circle_offsets {
                    let nx = x + off_x;
                    let ny = y + off_y;
                    let dst_idx = ny * new_w as usize + nx;
                    let curr = &mut out[dst_idx];
                    if src_val > *curr {
                        *curr = src_val;
                    }
                }
            }
        }
    }

    GlyphAlphaMask {
        x: mask.x - r_ceil,
        y: mask.y - r_ceil,
        width: new_w,
        height: new_h,
        mask: out,
    }
}

/// Cached relative masks for a single grapheme cluster at a specific transform & style state.
#[derive(Debug, Clone)]
struct PreparedClusterMasks {
    raw: GlyphAlphaMask,
    dilated: Vec<(ColorRgba, GlyphAlphaMask)>, // Ordered outermost (widest) to innermost
    blurred: Vec<(ShadowDef, GlyphAlphaMask)>,
    byte_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct GlyphCacheKey {
    grapheme: String,
    font_size_q: u32,
    scale_q: u32,
    rot_deg_q: i32,
    style_sig: u64,
}

/// Bounded FIFO/LRU-style glyph and primitive mask cache enforcing <= 256 MiB ceiling.
#[derive(Debug)]
struct BoundedGlyphCache {
    entries: HashMap<GlyphCacheKey, Arc<PreparedClusterMasks>>,
    order: VecDeque<GlyphCacheKey>,
    used_bytes: usize,
    max_bytes: usize,
    hits: usize,
    misses: usize,
}

impl BoundedGlyphCache {
    fn new(max_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            used_bytes: 0,
            max_bytes,
            hits: 0,
            misses: 0,
        }
    }

    fn get(&mut self, key: &GlyphCacheKey) -> Option<Arc<PreparedClusterMasks>> {
        if let Some(val) = self.entries.get(key) {
            self.hits += 1;
            Some(val.clone())
        } else {
            self.misses += 1;
            None
        }
    }

    fn insert(&mut self, key: GlyphCacheKey, val: Arc<PreparedClusterMasks>) {
        let entry_bytes = val.byte_size;
        if entry_bytes > self.max_bytes {
            return;
        }
        while self.used_bytes + entry_bytes > self.max_bytes {
            if let Some(oldest_key) = self.order.pop_front() {
                if let Some(removed) = self.entries.remove(&oldest_key) {
                    self.used_bytes = self.used_bytes.saturating_sub(removed.byte_size);
                }
            } else {
                break;
            }
        }
        self.used_bytes += entry_bytes;
        self.order.push_back(key.clone());
        self.entries.insert(key, val);
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.used_bytes = 0;
        self.hits = 0;
        self.misses = 0;
    }
}

/// High-performance Shared Text Renderer Engine using pure Rust typography stack.
pub struct SharedTextRendererPoc {
    pub canvas_width: u32,
    pub canvas_height: u32,
    pub font_data: Vec<u8>,
    pub font_sha256: String,
    pub font_size: f32,
    pub max_width: f32,
    pub gradient: LinearGradient,
    pub strokes: Vec<StrokeDef>,
    pub shadows: Vec<ShadowDef>,
    pub background_box: Option<BackgroundBoxDef>,
    cache: Mutex<BoundedGlyphCache>,
}

impl SharedTextRendererPoc {
    /// Creates a new renderer with verified font SHA-256 identity and rich style (118° gradient, 2 outer strokes, 2 drop shadows).
    pub fn new(canvas_width: u32, canvas_height: u32, font_data: Vec<u8>) -> Self {
        let font_sha256 = compute_sha256_hex(&font_data);
        Self {
            canvas_width,
            canvas_height,
            font_data,
            font_sha256,
            font_size: 56.0,
            max_width: (canvas_width as f32) - 160.0, // margin 80px each side
            gradient: LinearGradient {
                angle_deg: 118.0,
                stops: vec![
                    (0.05088495, ColorRgba::new(1.0, 0.890196, 0.372549, 1.0)), // #FFE35F
                    (0.94911504, ColorRgba::new(1.0, 1.0, 1.0, 1.0)),           // #FFFFFF
                ],
            },
            strokes: vec![
                StrokeDef {
                    color: ColorRgba::new(0.678431, 0.317647, 0.0, 1.0), // Inner stroke #AD5100
                    width_px: 4.0,
                },
                StrokeDef {
                    color: ColorRgba::new(0.290196, 0.125490, 0.0, 0.95), // Outer dark brown stroke #4A2000
                    width_px: 7.0,
                },
            ],
            shadows: vec![
                ShadowDef {
                    color: ColorRgba::new(0.678431, 0.317647, 0.0, 0.9), // #AD5100
                    angle_deg: -45.0,
                    distance_px: 8.0,
                    blur_px: 0.0,
                },
                ShadowDef {
                    color: ColorRgba::new(1.0, 0.654902, 0.345098, 0.9), // #FFA758
                    angle_deg: -45.0,
                    distance_px: 12.0,
                    blur_px: 3.0,
                },
            ],
            background_box: None,
            cache: Mutex::new(BoundedGlyphCache::new(DEFAULT_CACHE_MAX_BYTES)),
        }
    }

    /// Returns the verified uppercase SHA-256 hex digest of the loaded font asset.
    pub fn font_sha256(&self) -> &str {
        &self.font_sha256
    }

    /// Clears the bounded glyph/primitive cache (used to benchmark Cold Cache vs. Warm Cache).
    pub fn clear_cache(&self) {
        if let Ok(mut c) = self.cache.lock() {
            c.clear();
        }
    }

    /// Returns current memory bytes used by the bounded glyph cache.
    pub fn cache_bytes_used(&self) -> usize {
        self.cache.lock().map(|c| c.used_bytes).unwrap_or(0)
    }

    /// Returns the configured maximum cache ceiling in bytes (default 256 MiB).
    pub fn cache_max_bytes(&self) -> usize {
        self.cache
            .lock()
            .map(|c| c.max_bytes)
            .unwrap_or(DEFAULT_CACHE_MAX_BYTES)
    }

    /// Returns `(entry_count, cache_hits, cache_misses)`.
    pub fn cache_stats(&self) -> (usize, usize, usize) {
        self.cache
            .lock()
            .map(|c| (c.entries.len(), c.hits, c.misses))
            .unwrap_or((0, 0, 0))
    }

    /// Counts how many non-whitespace characters in `text` resolve to `.notdef` (`GlyphId(0)`).
    pub fn count_missing_glyphs(&self, text: &str) -> usize {
        let font = FontRef::try_from_slice(&self.font_data).expect("Valid TTF font data");
        let scale = PxScale::from(self.font_size);
        let scaled_font = font.as_scaled(scale);
        let nfc_text: String = text.nfc().collect();
        nfc_text
            .chars()
            .filter(|c| !c.is_whitespace() && !c.is_control())
            .filter(|&c| scaled_font.glyph_id(c) == GlyphId(0))
            .count()
    }

    fn style_signature(&self) -> u64 {
        let mut sig = (self.strokes.len() as u64) << 32 | (self.shadows.len() as u64);
        for s in &self.strokes {
            sig = sig
                .wrapping_mul(31)
                .wrapping_add((s.width_px * 100.0).round() as u64);
        }
        for sh in &self.shadows {
            sig = sig
                .wrapping_mul(31)
                .wrapping_add((sh.blur_px * 100.0).round() as u64);
        }
        sig
    }

    /// Evaluates CapCut Sample 01 Lua character motion at progress `p` in [0.0, 1.0] with kerning, multi-line wrap, and NFC normalization.
    pub fn evaluate_sample_motion(&self, text: &str, progress: f32) -> Vec<CharacterLayout> {
        self.evaluate_sample_motion_at_y(text, progress, None)
    }

    /// Evaluates character motion with an optional vertical center override `center_y_override` for multi-track layout.
    pub fn evaluate_sample_motion_at_y(
        &self,
        text: &str,
        progress: f32,
        center_y_override: Option<f32>,
    ) -> Vec<CharacterLayout> {
        let font = FontRef::try_from_slice(&self.font_data).expect("Valid TTF font data");
        let scale = PxScale::from(self.font_size);
        let scaled_font = font.as_scaled(scale);

        // Normalize text to Unicode NFC so combining marks stay properly composed into precomposed glyphs
        let nfc_text: String = text.nfc().collect();
        let graphemes: Vec<&str> =
            UnicodeSegmentation::graphemes(nfc_text.as_str(), true).collect();
        let total_count = graphemes.len();
        let non_space_count = graphemes
            .iter()
            .filter(|g| **g != " " && **g != "\n" && **g != "\r")
            .count();
        let spring_num = if non_space_count <= 3 { 1 } else { 3 };
        let total_virtual_frames = (non_space_count.saturating_sub(1) * 5 + 41) as f32;
        let cur_frame = (total_virtual_frames * progress.clamp(0.0, 1.0)).ceil();

        // Calculate cluster advance widths with OpenType horizontal kerning inside & across clusters
        let mut char_widths = Vec::with_capacity(total_count);
        let mut first_last_glyphs: Vec<Option<(GlyphId, GlyphId)>> =
            Vec::with_capacity(total_count);

        for g in &graphemes {
            let mut w = 0.0f32;
            let mut prev_in_cluster: Option<GlyphId> = None;
            let mut first_g: Option<GlyphId> = None;
            for ch in g.chars() {
                let glyph_id = scaled_font.glyph_id(ch);
                if let Some(prev) = prev_in_cluster {
                    w += scaled_font.kern(prev, glyph_id);
                } else {
                    first_g = Some(glyph_id);
                }
                w += scaled_font.h_advance(glyph_id);
                prev_in_cluster = Some(glyph_id);
            }
            if w <= 0.0 {
                w = self.font_size * 0.35; // space fallback
            }
            char_widths.push(w);
            first_last_glyphs.push(first_g.zip(prev_in_cluster));
        }

        // Line wrapping logic with inter-cluster kerning
        let line_height = self.font_size * 1.25;
        let mut lines: Vec<Vec<usize>> = Vec::new();
        let mut current_line: Vec<usize> = Vec::new();
        let mut current_line_width = 0.0f32;

        for (i, &cw) in char_widths.iter().enumerate() {
            if graphemes[i] == "\n" {
                lines.push(current_line);
                current_line = Vec::new();
                current_line_width = 0.0;
                continue;
            }

            let kern_with_prev = if let Some(&prev_idx) = current_line.last() {
                if let (Some((_, prev_last)), Some((curr_first, _))) =
                    (first_last_glyphs[prev_idx], first_last_glyphs[i])
                {
                    scaled_font.kern(prev_last, curr_first)
                } else {
                    0.0
                }
            } else {
                0.0
            };

            if current_line_width + kern_with_prev + cw > self.max_width && !current_line.is_empty()
            {
                lines.push(current_line);
                current_line = vec![i];
                current_line_width = cw;
            } else {
                current_line.push(i);
                current_line_width += kern_with_prev + cw;
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
        if lines.is_empty() {
            lines.push(Vec::new());
        }

        let total_text_height = lines.len() as f32 * line_height;
        let center_y = center_y_override.unwrap_or(self.canvas_height as f32 / 2.0);
        let initial_start_y = center_y - (total_text_height / 2.0);

        let mut layouts = Vec::with_capacity(total_count);
        let mut valid_idx = 0usize;

        for (line_idx, line) in lines.iter().enumerate() {
            let mut line_w = 0.0f32;
            for (pos, &idx) in line.iter().enumerate() {
                if pos > 0 {
                    let prev_idx = line[pos - 1];
                    if let (Some((_, prev_last)), Some((curr_first, _))) =
                        (first_last_glyphs[prev_idx], first_last_glyphs[idx])
                    {
                        line_w += scaled_font.kern(prev_last, curr_first);
                    }
                }
                line_w += char_widths[idx];
            }

            let mut cur_x = (self.canvas_width as f32 - line_w) / 2.0;
            let cur_y = initial_start_y + (line_idx as f32 * line_height);

            for (pos, &i) in line.iter().enumerate() {
                if pos > 0 {
                    let prev_idx = line[pos - 1];
                    if let (Some((_, prev_last)), Some((curr_first, _))) =
                        (first_last_glyphs[prev_idx], first_last_glyphs[i])
                    {
                        cur_x += scaled_font.kern(prev_last, curr_first);
                    }
                }

                let g = graphemes[i];
                let is_valid = g != " " && g != "\n" && g != "\r";
                let (transform_y, rot_z, opacity) = if is_valid {
                    valid_idx += 1;
                    let step_off1 = -self.font_size;
                    let step_off2 = self.font_size;

                    if valid_idx <= non_space_count.saturating_sub(spring_num) {
                        // Group A: Slide
                        let start_f = ((valid_idx - 1) * 5 + 1) as f32;
                        let end_f = ((valid_idx - 1) * 5 + 20) as f32;
                        let pt = ((cur_frame - start_f) / (end_f - start_f)).clamp(0.0, 1.0);
                        let cur_pt = evaluate_bezier(0.166667, 0.166667, 0.14, 1.0, pt);
                        let y = step_off1 + (0.0 - step_off1) * cur_pt;

                        let opt_pt = (cur_frame - start_f + 1.0).clamp(0.0, 1.0);
                        let opt_cur =
                            evaluate_bezier(0.166667, 0.166667, 0.833333, 0.833333, opt_pt);
                        (y, 0.0f32, opt_cur)
                    } else {
                        // Group B: Bounce & Z-rotation
                        let start_f = ((valid_idx - 1) * 5 + 1) as f32;
                        let end_f1 = ((valid_idx - 1) * 5 + 21) as f32;
                        let end_f2 = ((valid_idx - 1) * 5 + 41) as f32;

                        let y = if cur_frame <= end_f1 {
                            let pt = ((cur_frame - start_f) / (end_f1 - start_f)).clamp(0.0, 1.0);
                            let cur_pt = evaluate_bezier(0.166667, 0.166667, 0.0, 1.0, pt);
                            step_off1 + (step_off2 - step_off1) * cur_pt
                        } else {
                            let pt = ((cur_frame - end_f1) / (end_f2 - end_f1)).clamp(0.0, 1.0);
                            let cur_pt = evaluate_bezier(1.0, 0.0, 0.833333, 0.833333, pt);
                            step_off2 + (0.0 - step_off2) * cur_pt
                        };

                        let rot_pt =
                            ((cur_frame - start_f + 1.0) / (end_f1 - start_f)).clamp(0.0, 1.0);
                        let rot_cur = evaluate_bezier(0.015, 0.213_203_43, 0.09, 1.0, rot_pt);
                        let rot_z = -180.0 + (-360.0 - (-180.0)) * rot_cur;

                        let opt_pt = (cur_frame - start_f + 1.0).clamp(0.0, 1.0);
                        let opt_cur =
                            evaluate_bezier(0.166667, 0.166667, 0.833333, 0.833333, opt_pt);
                        (y, rot_z, opt_cur)
                    }
                } else {
                    (0.0, 0.0, 0.0) // Spaces have 0 opacity
                };

                let cw = char_widths[i];
                layouts.push(CharacterLayout {
                    grapheme: g.to_string(),
                    cluster_index: i,
                    base_x: cur_x,
                    base_y: cur_y,
                    width: cw,
                    height: self.font_size,
                    transform_y,
                    rotation_z_deg: rot_z,
                    scale: 1.0,
                    opacity,
                });
                cur_x += cw;
            }
        }

        layouts
    }

    /// Rasterizes all codepoints of a grapheme cluster into a SINGLE unified relative alpha mask
    /// and applies 2D affine rotation and scale around the unified cluster center.
    fn rasterize_relative_cluster_mask(&self, ch: &CharacterLayout) -> Option<GlyphAlphaMask> {
        let font = FontRef::try_from_slice(&self.font_data).expect("Valid TTF font");
        let scale = PxScale::from(self.font_size);
        let scaled_font = font.as_scaled(scale);

        let mut cur_x = 0.0f32;
        let baseline_y = scaled_font.ascent();
        let mut prev_glyph: Option<GlyphId> = None;

        struct SubGlyph {
            min_x: i32,
            min_y: i32,
            w: u32,
            h: u32,
            mask: Vec<u8>,
        }

        let mut sub_glyphs = Vec::new();
        let mut union_min_x = i32::MAX;
        let mut union_min_y = i32::MAX;
        let mut union_max_x = i32::MIN;
        let mut union_max_y = i32::MIN;

        for c in ch.grapheme.chars() {
            let glyph_id = scaled_font.glyph_id(c);
            if let Some(prev) = prev_glyph {
                cur_x += scaled_font.kern(prev, glyph_id);
            }
            let glyph = Glyph {
                id: glyph_id,
                scale,
                position: Point {
                    x: cur_x,
                    y: baseline_y,
                },
            };

            if let Some(outlined) = font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                let w = (bounds.width().ceil() as u32).max(1);
                let h = (bounds.height().ceil() as u32).max(1);
                let min_x = bounds.min.x.floor() as i32;
                let min_y = bounds.min.y.floor() as i32;
                let mut mask_bytes = vec![0u8; (w * h) as usize];

                outlined.draw(|gx, gy, cov| {
                    if gx < w && gy < h {
                        let val = (cov * 255.0).clamp(0.0, 255.0).round() as u8;
                        mask_bytes[(gy * w + gx) as usize] = val;
                    }
                });

                union_min_x = union_min_x.min(min_x);
                union_min_y = union_min_y.min(min_y);
                union_max_x = union_max_x.max(min_x + w as i32);
                union_max_y = union_max_y.max(min_y + h as i32);

                sub_glyphs.push(SubGlyph {
                    min_x,
                    min_y,
                    w,
                    h,
                    mask: mask_bytes,
                });
            }

            cur_x += scaled_font.h_advance(glyph_id);
            prev_glyph = Some(glyph_id);
        }

        if sub_glyphs.is_empty() || union_max_x <= union_min_x || union_max_y <= union_min_y {
            return None;
        }

        let total_w = (union_max_x - union_min_x) as u32;
        let total_h = (union_max_y - union_min_y) as u32;
        let mut combined_mask = vec![0u8; (total_w * total_h) as usize];

        for sg in sub_glyphs {
            let off_x = (sg.min_x - union_min_x) as u32;
            let off_y = (sg.min_y - union_min_y) as u32;
            for gy in 0..sg.h {
                for gx in 0..sg.w {
                    let v = sg.mask[(gy * sg.w + gx) as usize];
                    if v > 0 {
                        let dst_idx = ((off_y + gy) * total_w + (off_x + gx)) as usize;
                        combined_mask[dst_idx] = combined_mask[dst_idx].max(v);
                    }
                }
            }
        }

        let base_mask = GlyphAlphaMask {
            x: union_min_x,
            y: union_min_y,
            width: total_w,
            height: total_h,
            mask: combined_mask,
        };

        // Apply 2D affine rotation & scale to the unified cluster mask
        Some(transform_alpha_mask(
            &base_mask,
            ch.rotation_z_deg,
            ch.scale,
        ))
    }

    /// Rasterizes a character cluster into canvas-positioned alpha masks (public API for tests).
    pub fn rasterize_cluster_glyphs(&self, ch: &CharacterLayout) -> Vec<GlyphAlphaMask> {
        if let Some(mut rel) = self.rasterize_relative_cluster_mask(ch) {
            rel.x += ch.base_x.round() as i32;
            rel.y += (ch.base_y + ch.transform_y).round() as i32;
            vec![rel]
        } else {
            Vec::new()
        }
    }

    /// Retrieves or builds cached `PreparedClusterMasks` (raw + sorted dilated strokes + blurred shadows).
    fn get_or_prepare_cluster_masks(
        &self,
        ch: &CharacterLayout,
        style_sig: u64,
    ) -> Option<Arc<PreparedClusterMasks>> {
        let key = GlyphCacheKey {
            grapheme: ch.grapheme.clone(),
            font_size_q: (self.font_size * 10.0).round() as u32,
            scale_q: (ch.scale * 100.0).round() as u32,
            rot_deg_q: (ch.rotation_z_deg * 10.0).round() as i32,
            style_sig,
        };

        if let Ok(mut cache) = self.cache.lock() {
            if let Some(cached) = cache.get(&key) {
                return Some(cached);
            }
        }

        let raw = self.rasterize_relative_cluster_mask(ch)?;
        let mut byte_size = raw.mask.len() + 64;

        // Sort strokes from widest (outermost) to narrowest (innermost) for correct Painter's order
        let mut sorted_strokes = self.strokes.clone();
        sorted_strokes.sort_by(|a, b| {
            b.width_px
                .partial_cmp(&a.width_px)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut dilated = Vec::with_capacity(sorted_strokes.len());
        for s in &sorted_strokes {
            let dm = dilate_alpha_mask(&raw, s.width_px);
            byte_size += dm.mask.len() + 32;
            dilated.push((s.color, dm));
        }

        let mut blurred = Vec::with_capacity(self.shadows.len());
        for sh in &self.shadows {
            let bm = if sh.blur_px > 0.0 {
                blur_alpha_mask(&raw, sh.blur_px.round() as u32)
            } else {
                raw.clone()
            };
            byte_size += bm.mask.len() + 32;
            blurred.push((sh.clone(), bm));
        }

        let prepared = Arc::new(PreparedClusterMasks {
            raw,
            dilated,
            blurred,
            byte_size,
        });

        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(key, prepared.clone());
        }

        Some(prepared)
    }

    /// Renders a decorative rounded-rectangle shape layer onto `buf` using Porter-Duff Over.
    pub fn render_shape_layer(&self, buf: &mut RgbaBuffer, shape: &ShapeLayerDef) {
        let x0 = shape.x.floor().max(0.0) as i32;
        let y0 = shape.y.floor().max(0.0) as i32;
        let x1 = (shape.x + shape.width).ceil().min(buf.width as f32) as i32;
        let y1 = (shape.y + shape.height).ceil().min(buf.height as f32) as i32;
        if x1 <= x0 || y1 <= y0 || shape.color.a <= 0.001 {
            return;
        }

        let [cr, cg, cb, ca] = shape.color.to_u8_array();
        let r = shape
            .border_radius
            .min(shape.width / 2.0)
            .min(shape.height / 2.0)
            .max(0.0);
        let r_sq = r * r;

        let inner_x0 = shape.x + r;
        let inner_x1 = shape.x + shape.width - r;
        let inner_y0 = shape.y + r;
        let inner_y1 = shape.y + shape.height - r;

        for py in y0..y1 {
            let fy = py as f32 + 0.5;
            let dy = if fy < inner_y0 {
                inner_y0 - fy
            } else if fy > inner_y1 {
                fy - inner_y1
            } else {
                0.0
            };

            for px in x0..x1 {
                let fx = px as f32 + 0.5;
                let dx = if fx < inner_x0 {
                    inner_x0 - fx
                } else if fx > inner_x1 {
                    fx - inner_x1
                } else {
                    0.0
                };

                if dx == 0.0 && dy == 0.0 {
                    buf.blend_pixel_over(px as u32, py as u32, cr, cg, cb, ca);
                } else {
                    let d_sq = dx * dx + dy * dy;
                    if d_sq <= r_sq {
                        buf.blend_pixel_over(px as u32, py as u32, cr, cg, cb, ca);
                    }
                }
            }
        }
    }

    /// Renders text overlay frame into uncompressed RGBA buffer with strict Painter's order.
    pub fn render_frame(&self, text: &str, progress: f32) -> RgbaBuffer {
        let mut buf = RgbaBuffer::new(self.canvas_width, self.canvas_height);
        self.render_frame_into(&mut buf, text, progress);
        buf
    }

    /// Renders directly into pre-allocated buffer (clears buffer first).
    pub fn render_frame_into(&self, buf: &mut RgbaBuffer, text: &str, progress: f32) {
        buf.clear();
        self.composite_layer_into(buf, text, progress);
    }

    /// Composites text layer into an existing buffer WITHOUT clearing (enables multi-title / multi-layer concurrency).
    pub fn composite_layer_into(&self, buf: &mut RgbaBuffer, text: &str, progress: f32) {
        self.composite_layer_at_into(buf, text, progress, None, self.background_box.as_ref());
    }

    /// Composites text layer at an optional vertical center `center_y_override` and optional `BackgroundBoxDef`.
    pub fn composite_layer_at_into(
        &self,
        buf: &mut RgbaBuffer,
        text: &str,
        progress: f32,
        center_y_override: Option<f32>,
        bg_box_override: Option<&BackgroundBoxDef>,
    ) {
        let characters = self.evaluate_sample_motion_at_y(text, progress, center_y_override);
        if characters.is_empty() {
            return;
        }

        // Compute static layout bounds (independent of animation transform_y) so mode="all" gradient does not wobble
        let mut static_bounds = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        for ch in &characters {
            if ch.grapheme != " " && ch.grapheme != "\n" && ch.grapheme != "\r" {
                static_bounds[0] = static_bounds[0].min(ch.base_x);
                static_bounds[1] = static_bounds[1].min(ch.base_y);
                static_bounds[2] = static_bounds[2].max(ch.base_x + ch.width);
                static_bounds[3] = static_bounds[3].max(ch.base_y + ch.height);
            }
        }
        if static_bounds[0] >= static_bounds[2] {
            return; // Empty or whitespace only
        }

        let style_sig = self.style_signature();
        let mut prepared_clusters: Vec<(CharacterLayout, i32, i32, Arc<PreparedClusterMasks>)> =
            Vec::with_capacity(characters.len());

        for ch in &characters {
            if ch.opacity <= 0.001 {
                continue;
            }
            if let Some(prep) = self.get_or_prepare_cluster_masks(ch, style_sig) {
                let pos_x = ch.base_x.round() as i32;
                let pos_y = (ch.base_y + ch.transform_y).round() as i32;
                prepared_clusters.push((ch.clone(), pos_x, pos_y, prep));
            }
        }

        if prepared_clusters.is_empty() {
            return;
        }

        // --- Step 0: Optional Background Box (Lowest in Painter's Order) ---
        if let Some(bg) = bg_box_override {
            let shape = ShapeLayerDef {
                x: static_bounds[0] - bg.padding_x,
                y: static_bounds[1] - bg.padding_y,
                width: (static_bounds[2] - static_bounds[0]) + bg.padding_x * 2.0,
                height: (static_bounds[3] - static_bounds[1]) + bg.padding_y * 2.0,
                border_radius: bg.border_radius,
                color: bg.color,
            };
            self.render_shape_layer(buf, &shape);
        }

        // --- Step 1: Drop Shadows (Multi-layer back to front) ---
        for shadow_idx in 0..self.shadows.len() {
            for (ch, pos_x, pos_y, prep) in &prepared_clusters {
                let (shadow, blurred) = &prep.blurred[shadow_idx];
                let rad = shadow.angle_deg.to_radians();
                let off_x = (shadow.distance_px * rad.cos()).round() as i32;
                let off_y = (-shadow.distance_px * rad.sin()).round() as i32;

                let target_x = pos_x + blurred.x + off_x;
                let target_y = pos_y + blurred.y + off_y;

                let sr = (shadow.color.r * 255.0).round() as u8;
                let sg = (shadow.color.g * 255.0).round() as u8;
                let sb = (shadow.color.b * 255.0).round() as u8;
                let opacity_scale = shadow.color.a * ch.opacity;

                for my in 0..blurred.height {
                    let py = target_y + my as i32;
                    if py < 0 || py >= buf.height as i32 {
                        continue;
                    }
                    let row_offset = (my * blurred.width) as usize;
                    for mx in 0..blurred.width {
                        let px = target_x + mx as i32;
                        if px < 0 || px >= buf.width as i32 {
                            continue;
                        }
                        let m_val = blurred.mask[row_offset + mx as usize];
                        if m_val > 0 {
                            let alpha = (m_val as f32 * opacity_scale).round() as u8;
                            buf.blend_pixel_over(px as u32, py as u32, sr, sg, sb, alpha);
                        }
                    }
                }
            }
        }

        // --- Step 2: Outer Strokes (Sorted widest/outermost to narrowest/innermost) ---
        let stroke_count = self.strokes.len();
        for stroke_idx in 0..stroke_count {
            for (ch, pos_x, pos_y, prep) in &prepared_clusters {
                let (stroke_col, dilated) = &prep.dilated[stroke_idx];
                let target_x = pos_x + dilated.x;
                let target_y = pos_y + dilated.y;

                let sr = (stroke_col.r * 255.0).round() as u8;
                let sg = (stroke_col.g * 255.0).round() as u8;
                let sb = (stroke_col.b * 255.0).round() as u8;
                let opacity_scale = stroke_col.a * ch.opacity;

                for my in 0..dilated.height {
                    let py = target_y + my as i32;
                    if py < 0 || py >= buf.height as i32 {
                        continue;
                    }
                    let row_offset = (my * dilated.width) as usize;
                    for mx in 0..dilated.width {
                        let px = target_x + mx as i32;
                        if px < 0 || px >= buf.width as i32 {
                            continue;
                        }
                        let m_val = dilated.mask[row_offset + mx as usize];
                        if m_val > 0 {
                            let alpha = (m_val as f32 * opacity_scale).round() as u8;
                            buf.blend_pixel_over(px as u32, py as u32, sr, sg, sb, alpha);
                        }
                    }
                }
            }
        }

        // --- Step 3: Glyph Fills with 118° Linear Gradient (Anchored to Static Layout Bounds) ---
        let [bx0, by0, bx1, by1] = static_bounds;
        let bw = (bx1 - bx0).max(1.0);
        let bh = (by1 - by0).max(1.0);
        let grad_cx = (bx0 + bx1) / 2.0;
        let grad_cy = (by0 + by1) / 2.0;
        let grad_rad = self.gradient.angle_deg.to_radians();
        let dir_x = grad_rad.cos();
        let dir_y = -grad_rad.sin();
        let max_proj = (bw * dir_x.abs() + bh * dir_y.abs()) / 2.0;
        let inv_span = if max_proj > 1e-3 {
            1.0 / (max_proj * 2.0)
        } else {
            0.0
        };

        for (ch, pos_x, pos_y, prep) in &prepared_clusters {
            let mask = &prep.raw;
            let target_x = pos_x + mask.x;
            let target_y = pos_y + mask.y;

            // Anchor gradient projection to character's static layout position so vertical animation does not slide or clamp the gradient
            let static_char_x = ch.base_x.round() as i32 + mask.x;
            let static_char_y = ch.base_y.round() as i32 + mask.y;

            for my in 0..mask.height {
                let py = target_y + my as i32;
                if py < 0 || py >= buf.height as i32 {
                    continue;
                }
                let row_offset = (my * mask.width) as usize;
                let static_py = (static_char_y + my as i32) as f32;
                let fy_term = (static_py - grad_cy) * dir_y;

                for mx in 0..mask.width {
                    let px = target_x + mx as i32;
                    if px < 0 || px >= buf.width as i32 {
                        continue;
                    }
                    let m_val = mask.mask[row_offset + mx as usize];
                    if m_val > 0 {
                        let static_px = (static_char_x + mx as i32) as f32;
                        let proj = (static_px - grad_cx) * dir_x + fy_term;
                        let t = ((proj * inv_span) + 0.5).clamp(0.0, 1.0);
                        let grad_col = self.gradient.sample_at(t);
                        let alpha = (m_val as f32 * grad_col.a * ch.opacity).round() as u8;
                        buf.blend_pixel_over(
                            px as u32,
                            py as u32,
                            (grad_col.r * 255.0).round() as u8,
                            (grad_col.g * 255.0).round() as u8,
                            (grad_col.b * 255.0).round() as u8,
                            alpha,
                        );
                    }
                }
            }
        }
    }

    /// Consumer 1 (Preview Seam): Delivers overlay packet with dirty ROI and cropped ROI PNG bytes.
    /// Avoids encoding full-frame PNG bytes when only the cropped ROI is needed for preview canvas blitting.
    pub fn render_preview_roi_packet(&self, text: &str, progress: f32) -> PreviewOverlayPacket {
        let buf = self.render_frame(text, progress);
        let roi = buf.compute_dirty_roi().unwrap_or([0, 0, 0, 0]);
        let has_visible_content = roi[2] > roi[0] && roi[3] > roi[1];
        let roi_png_bytes = if has_visible_content {
            buf.crop(roi).to_png_bytes()
        } else {
            Vec::new()
        };

        PreviewOverlayPacket {
            width: buf.width,
            height: buf.height,
            roi,
            png_bytes: Vec::new(),
            roi_png_bytes,
            has_visible_content,
            font_sha256: self.font_sha256.clone(),
        }
    }

    /// Consumer 1 (Preview Seam with Full PNG): Delivers overlay packet with dirty ROI, full PNG bytes, and tight ROI PNG bytes for Tauri IPC.
    pub fn render_preview_packet(&self, text: &str, progress: f32) -> PreviewOverlayPacket {
        let buf = self.render_frame(text, progress);
        let roi = buf.compute_dirty_roi().unwrap_or([0, 0, 0, 0]);
        let has_visible_content = roi[2] > roi[0] && roi[3] > roi[1];
        let png_bytes = buf.to_png_bytes();
        let roi_png_bytes = if has_visible_content {
            buf.crop(roi).to_png_bytes()
        } else {
            Vec::new()
        };

        PreviewOverlayPacket {
            width: buf.width,
            height: buf.height,
            roi,
            png_bytes,
            roi_png_bytes,
            has_visible_content,
            font_sha256: self.font_sha256.clone(),
        }
    }

    /// Consumer 2 (Headless Export Seam): Streams bounded RGBA frames into an FFmpeg stdin pipe.
    #[allow(clippy::too_many_arguments)]
    pub fn stream_to_ffmpeg_overlay(
        &self,
        ffmpeg_bin: &Path,
        bg_video_or_color: &str,
        text: &str,
        duration_secs: f32,
        fps: u32,
        out_mp4: &Path,
        cancel_token: Arc<AtomicBool>,
    ) -> Result<ExportStreamSummary, String> {
        let total_frames = (duration_secs * fps as f32).round() as usize;
        let mut child: Child = if bg_video_or_color.starts_with("color=") {
            let col = bg_video_or_color.strip_prefix("color=").unwrap();
            Command::new(ffmpeg_bin)
                .args([
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    &format!(
                        "color=c={}:s={}x{}:r={}:d={}",
                        col, self.canvas_width, self.canvas_height, fps, duration_secs
                    ),
                    "-f",
                    "rawvideo",
                    "-pixel_format",
                    "rgba",
                    "-video_size",
                    &format!("{}x{}", self.canvas_width, self.canvas_height),
                    "-framerate",
                    &fps.to_string(),
                    "-i",
                    "pipe:0",
                    "-filter_complex",
                    "[0:v][1:v]overlay=format=auto[outv]",
                    "-map",
                    "[outv]",
                    "-c:v",
                    "libx264",
                    "-preset",
                    "ultrafast",
                    "-pix_fmt",
                    "yuv420p",
                    out_mp4.to_str().unwrap(),
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| format!("Failed to spawn FFmpeg: {}", e))?
        } else {
            Command::new(ffmpeg_bin)
                .args([
                    "-y",
                    "-i",
                    bg_video_or_color,
                    "-f",
                    "rawvideo",
                    "-pixel_format",
                    "rgba",
                    "-video_size",
                    &format!("{}x{}", self.canvas_width, self.canvas_height),
                    "-framerate",
                    &fps.to_string(),
                    "-i",
                    "pipe:0",
                    "-filter_complex",
                    "[0:v][1:v]overlay=format=auto[outv]",
                    "-map",
                    "[outv]",
                    "-c:v",
                    "libx264",
                    "-preset",
                    "ultrafast",
                    "-pix_fmt",
                    "yuv420p",
                    out_mp4.to_str().unwrap(),
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| format!("Failed to spawn FFmpeg: {}", e))?
        };

        let mut stdin = child.stdin.take().ok_or("Cannot open FFmpeg stdin pipe")?;
        let mut stderr = child.stderr.take();

        // Concurrently drain FFmpeg stderr in a background thread to prevent OS pipe deadlock on long exports
        let stderr_handle = std::thread::spawn(move || {
            let mut last_error_lines = Vec::new();
            if let Some(ref mut err_pipe) = stderr {
                use std::io::BufRead;
                let reader = std::io::BufReader::new(err_pipe);
                for line in reader.lines().map_while(Result::ok) {
                    if last_error_lines.len() >= 20 {
                        last_error_lines.remove(0);
                    }
                    last_error_lines.push(line);
                }
            }
            last_error_lines.join("\n")
        });

        let mut buf = RgbaBuffer::new(self.canvas_width, self.canvas_height);
        let mut frames_written = 0usize;

        for f in 0..total_frames {
            if cancel_token.load(Ordering::SeqCst) {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stderr_handle.join();
                if out_mp4.exists() {
                    let _ = std::fs::remove_file(out_mp4);
                }
                return Err("Export cancelled by user".to_string());
            }

            let progress = if total_frames <= 1 {
                1.0
            } else {
                (f as f32 / (total_frames - 1) as f32).min(1.0)
            };
            self.render_frame_into(&mut buf, text, progress);

            if stdin.write_all(&buf.data).is_err() {
                let _ = child.kill();
                let _ = child.wait();
                let err_msg = stderr_handle.join().unwrap_or_default();
                return Err(format!("FFmpeg stdin broken pipe: {}", err_msg));
            }
            frames_written += 1;
        }

        drop(stdin); // Send EOF
        let status = child
            .wait()
            .map_err(|e| format!("FFmpeg wait error: {}", e))?;
        let stderr_output = stderr_handle.join().unwrap_or_default();

        if !status.success() {
            return Err(format!(
                "FFmpeg returned non-zero exit code ({:?}): {}",
                status.code(),
                stderr_output
            ));
        }

        Ok(ExportStreamSummary {
            total_frames: frames_written,
            duration_secs,
            output_file: out_mp4.to_string_lossy().to_string(),
            file_size_bytes: std::fs::metadata(out_mp4).map(|m| m.len()).unwrap_or(0),
        })
    }
}

/// DTO for Tauri Preview IPC
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PreviewOverlayPacket {
    pub width: u32,
    pub height: u32,
    pub roi: [u32; 4],
    pub png_bytes: Vec<u8>,
    pub roi_png_bytes: Vec<u8>,
    pub has_visible_content: bool,
    pub font_sha256: String,
}

/// Lightweight IPC transport summary for Tauri Preview command (`render_text_template_poc_preview`)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewOverlayIpcResponse {
    pub width: u32,
    pub height: u32,
    pub roi: [u32; 4],
    pub roi_png_bytes: Vec<u8>,
    pub has_visible_content: bool,
    pub font_sha256: String,
    pub render_elapsed_ms: f64,
}

/// Returns the bundled Apache-2.0 open-source `Roboto-Bold.ttf` font bytes (`SHA-256: 61F89F8DB49261C2F6106E8DCCC35DF7B2F7ED909020DB40A3FC905E95F99334`).
pub fn default_open_source_font_bytes() -> Vec<u8> {
    include_bytes!("../../../fixtures/fonts/Roboto-Bold.ttf").to_vec()
}

static SHARED_POC_PREVIEW_RENDERER: std::sync::OnceLock<SharedTextRendererPoc> =
    std::sync::OnceLock::new();

/// Executes the Tauri Preview IPC seam (`render_text_template_poc_preview`) and returns the serialized response with timing.
/// Reuses the static `SharedTextRendererPoc` instance across preview frames to preserve warm cache hits.
pub fn invoke_poc_preview_ipc(text: &str, progress: f32) -> PreviewOverlayIpcResponse {
    let start = std::time::Instant::now();
    let renderer = SHARED_POC_PREVIEW_RENDERER
        .get_or_init(|| SharedTextRendererPoc::new(1920, 1080, default_open_source_font_bytes()));
    let packet = renderer.render_preview_roi_packet(text, progress);
    let render_elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
    PreviewOverlayIpcResponse {
        width: packet.width,
        height: packet.height,
        roi: packet.roi,
        roi_png_bytes: packet.roi_png_bytes,
        has_visible_content: packet.has_visible_content,
        font_sha256: packet.font_sha256,
        render_elapsed_ms,
    }
}

/// DTO for FFmpeg Export stream summary
#[derive(Debug, Clone)]
pub struct ExportStreamSummary {
    pub total_frames: usize,
    pub duration_secs: f32,
    pub output_file: String,
    pub file_size_bytes: u64,
}
