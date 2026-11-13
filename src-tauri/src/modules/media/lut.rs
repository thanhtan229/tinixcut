//! Adobe/Resolve `.cube` 3D LUT parsing, content hashing and a reference trilinear sampler.
//!
//! The export path hands the `.cube` file to FFmpeg's `lut3d` filter; this module exists to
//! validate files before they enter a project, pin their content with SHA-256, and provide the
//! reference sampler that the preview shader and the parity tests are checked against.
//!
//! The structure of the parser is informed by Clypra's `lut_parser.rs` (MIT, see
//! `THIRD_PARTY_NOTICES.md`), but it keeps full `f32` precision, bounds the input size and
//! rejects malformed lines instead of skipping them.

use crate::modules::domain::ClipLut;
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::io::Read;
use std::path::Path;

/// Largest accepted `.cube` file. A 65³ LUT is roughly 5 MiB of text.
pub const MAX_LUT_FILE_BYTES: u64 = 16 * 1024 * 1024;
pub const MIN_LUT_SIZE: u32 = 2;
pub const MAX_LUT_SIZE: u32 = 65;

#[derive(Debug, Clone, PartialEq)]
pub enum LutError {
    Io(String),
    TooLarge(u64),
    Unsupported(String),
    Malformed { line: usize, reason: String },
    MissingSize,
    SizeOutOfRange(u32),
    SampleCount { expected: usize, found: usize },
    InvalidDomain,
}

impl fmt::Display for LutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LutError::Io(message) => write!(f, "Cannot read LUT file: {message}"),
            LutError::TooLarge(bytes) => write!(
                f,
                "LUT file is {bytes} bytes; the limit is {MAX_LUT_FILE_BYTES} bytes"
            ),
            LutError::Unsupported(what) => write!(f, "Unsupported LUT feature: {what}"),
            LutError::Malformed { line, reason } => write!(f, "LUT line {line}: {reason}"),
            LutError::MissingSize => write!(f, "LUT file has no LUT_3D_SIZE header"),
            LutError::SizeOutOfRange(size) => write!(
                f,
                "LUT_3D_SIZE {size} is outside {MIN_LUT_SIZE}..={MAX_LUT_SIZE}"
            ),
            LutError::SampleCount { expected, found } => {
                write!(
                    f,
                    "LUT has {found} samples; LUT_3D_SIZE requires {expected}"
                )
            }
            LutError::InvalidDomain => {
                write!(
                    f,
                    "LUT DOMAIN_MIN must be below DOMAIN_MAX on every channel"
                )
            }
        }
    }
}

impl std::error::Error for LutError {}

/// A parsed 3D LUT. `samples` is red-fastest: index = r + size * (g + size * b).
#[derive(Debug, Clone, PartialEq)]
pub struct Lut3D {
    pub title: String,
    pub size: u32,
    pub domain_min: [f32; 3],
    pub domain_max: [f32; 3],
    pub samples: Vec<[f32; 3]>,
}

fn parse_floats<const N: usize>(parts: &[&str], line: usize) -> Result<[f32; N], LutError> {
    if parts.len() != N {
        return Err(LutError::Malformed {
            line,
            reason: format!("expected {N} numbers, found {}", parts.len()),
        });
    }
    let mut out = [0.0f32; N];
    for (slot, text) in out.iter_mut().zip(parts) {
        let value: f32 = text.parse().map_err(|_| LutError::Malformed {
            line,
            reason: format!("'{text}' is not a number"),
        })?;
        if !value.is_finite() {
            return Err(LutError::Malformed {
                line,
                reason: format!("'{text}' is not finite"),
            });
        }
        *slot = value;
    }
    Ok(out)
}

impl Lut3D {
    pub fn parse_cube_str(content: &str) -> Result<Self, LutError> {
        let mut title = String::new();
        let mut size: Option<u32> = None;
        let mut domain_min = [0.0f32; 3];
        let mut domain_max = [1.0f32; 3];
        let mut samples: Vec<[f32; 3]> = Vec::new();

        for (index, raw) in content.lines().enumerate() {
            let line = index + 1;
            let trimmed = raw.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            match parts[0] {
                "TITLE" => {
                    title = trimmed
                        .trim_start_matches("TITLE")
                        .trim()
                        .trim_matches('"')
                        .to_string();
                }
                "LUT_1D_SIZE" => {
                    return Err(LutError::Unsupported("1D LUT (LUT_1D_SIZE)".into()));
                }
                "LUT_3D_SIZE" => {
                    if size.is_some() {
                        return Err(LutError::Malformed {
                            line,
                            reason: "LUT_3D_SIZE declared twice".into(),
                        });
                    }
                    let [value] = parse_floats::<1>(&parts[1..], line)?;
                    if value.fract() != 0.0 || value < 0.0 {
                        return Err(LutError::Malformed {
                            line,
                            reason: "LUT_3D_SIZE must be a whole number".into(),
                        });
                    }
                    let value = value as u32;
                    if !(MIN_LUT_SIZE..=MAX_LUT_SIZE).contains(&value) {
                        return Err(LutError::SizeOutOfRange(value));
                    }
                    size = Some(value);
                }
                "DOMAIN_MIN" => domain_min = parse_floats::<3>(&parts[1..], line)?,
                "DOMAIN_MAX" => domain_max = parse_floats::<3>(&parts[1..], line)?,
                keyword
                    if keyword
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_alphabetic()) =>
                {
                    return Err(LutError::Unsupported(format!("keyword '{keyword}'")));
                }
                _ => {
                    // Data rows are only valid after the size header so the bound applies.
                    let expected = size.ok_or(LutError::MissingSize)? as usize;
                    if samples.len() >= expected.pow(3) {
                        return Err(LutError::SampleCount {
                            expected: expected.pow(3),
                            found: samples.len() + 1,
                        });
                    }
                    samples.push(parse_floats::<3>(&parts, line)?);
                }
            }
        }

        let size = size.ok_or(LutError::MissingSize)?;
        let expected = (size as usize).pow(3);
        if samples.len() != expected {
            return Err(LutError::SampleCount {
                expected,
                found: samples.len(),
            });
        }
        if (0..3).any(|c| domain_min[c] >= domain_max[c]) {
            return Err(LutError::InvalidDomain);
        }
        Ok(Self {
            title,
            size,
            domain_min,
            domain_max,
            samples,
        })
    }

    pub fn from_file(path: &Path) -> Result<Self, LutError> {
        let bytes = read_bounded(path)?;
        let text = String::from_utf8(bytes)
            .map_err(|_| LutError::Io("LUT file is not valid UTF-8 text".into()))?;
        Self::parse_cube_str(&text)
    }

    fn at(&self, r: usize, g: usize, b: usize) -> [f32; 3] {
        let n = self.size as usize;
        self.samples[r + n * (g + n * b)]
    }

    /// Reference trilinear lookup for one RGB triple in the LUT's input domain. Inputs outside
    /// the domain are clamped, matching FFmpeg's `lut3d` and a hardware-sampled 3D texture.
    pub fn sample_trilinear(&self, rgb: [f32; 3]) -> [f32; 3] {
        let n = self.size as usize;
        let scale = (n - 1) as f32;
        let mut base = [0usize; 3];
        let mut frac = [0.0f32; 3];
        for c in 0..3 {
            let unit = ((rgb[c] - self.domain_min[c]) / (self.domain_max[c] - self.domain_min[c]))
                .clamp(0.0, 1.0)
                * scale;
            let floor = unit.floor().min(scale - 1.0);
            base[c] = floor as usize;
            frac[c] = unit - floor;
        }
        let mut out = [0.0f32; 3];
        for (dr, wr) in [(0usize, 1.0 - frac[0]), (1, frac[0])] {
            for (dg, wg) in [(0usize, 1.0 - frac[1]), (1, frac[1])] {
                for (db, wb) in [(0usize, 1.0 - frac[2]), (1, frac[2])] {
                    let weight = wr * wg * wb;
                    let v = self.at(base[0] + dr, base[1] + dg, base[2] + db);
                    for c in 0..3 {
                        out[c] += weight * v[c];
                    }
                }
            }
        }
        out
    }

    /// Neutral LUT that returns its input unchanged.
    pub fn identity(size: u32) -> Self {
        let n = size as usize;
        let divisor = (n - 1) as f32;
        let mut samples = Vec::with_capacity(n * n * n);
        for b in 0..n {
            for g in 0..n {
                for r in 0..n {
                    samples.push([r as f32 / divisor, g as f32 / divisor, b as f32 / divisor]);
                }
            }
        }
        Self {
            title: "Identity".into(),
            size,
            domain_min: [0.0; 3],
            domain_max: [1.0; 3],
            samples,
        }
    }
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, LutError> {
    let file = fs::File::open(path).map_err(|e| LutError::Io(e.to_string()))?;
    let length = file
        .metadata()
        .map_err(|e| LutError::Io(e.to_string()))?
        .len();
    if length > MAX_LUT_FILE_BYTES {
        return Err(LutError::TooLarge(length));
    }
    let mut bytes = Vec::with_capacity(length as usize);
    file.take(MAX_LUT_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| LutError::Io(e.to_string()))?;
    if bytes.len() as u64 > MAX_LUT_FILE_BYTES {
        return Err(LutError::TooLarge(bytes.len() as u64));
    }
    Ok(bytes)
}

fn hex(digest: &[u8]) -> String {
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// SHA-256 of a bounded `.cube` file, lowercase hex.
pub fn lut_file_sha256(path: &Path) -> Result<String, LutError> {
    Ok(hex(&Sha256::digest(read_bounded(path)?)))
}

impl ClipLut {
    /// Parse and hash a `.cube` file, producing the reference stored on a clip. The file is
    /// fully validated, so an invalid LUT can never enter a project through this path.
    pub fn from_file(path: &Path) -> Result<Self, LutError> {
        let bytes = read_bounded(path)?;
        let sha256 = hex(&Sha256::digest(&bytes));
        let text = String::from_utf8(bytes)
            .map_err(|_| LutError::Io("LUT file is not valid UTF-8 text".into()))?;
        let lut = Lut3D::parse_cube_str(&text)?;
        let fallback = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "LUT".into());
        let name = if lut.title.trim().is_empty() {
            fallback
        } else {
            lut.title.clone()
        };
        let name: String = name.chars().take(ClipLut::MAX_NAME_LEN).collect();
        Ok(ClipLut {
            name,
            path: path.to_string_lossy().into_owned(),
            sha256,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity_cube(size: u32) -> String {
        let lut = Lut3D::identity(size);
        let mut text = format!("TITLE \"Test\"\nLUT_3D_SIZE {size}\n");
        for [r, g, b] in &lut.samples {
            text.push_str(&format!("{r} {g} {b}\n"));
        }
        text
    }

    #[test]
    fn parses_identity_and_samples_it_exactly() {
        let lut = Lut3D::parse_cube_str(&identity_cube(5)).unwrap();
        assert_eq!(lut.title, "Test");
        assert_eq!(lut.size, 5);
        for rgb in [
            [0.0, 0.0, 0.0],
            [0.3, 0.6, 0.9],
            [1.0, 1.0, 1.0],
            [0.11, 0.97, 0.5],
        ] {
            let out = lut.sample_trilinear(rgb);
            for c in 0..3 {
                assert!((out[c] - rgb[c]).abs() < 1e-5, "{rgb:?} -> {out:?}");
            }
        }
    }

    #[test]
    fn red_is_the_fastest_axis() {
        let mut text = String::from("LUT_3D_SIZE 2\n");
        // Output equals input except red and blue are swapped, so the layout is observable.
        for b in 0..2 {
            for g in 0..2 {
                for r in 0..2 {
                    text.push_str(&format!("{b} {g} {r}\n"));
                }
            }
        }
        let lut = Lut3D::parse_cube_str(&text).unwrap();
        assert_eq!(lut.sample_trilinear([1.0, 0.0, 0.0]), [0.0, 0.0, 1.0]);
        assert_eq!(lut.sample_trilinear([0.0, 0.0, 1.0]), [1.0, 0.0, 0.0]);
    }

    #[test]
    fn domain_scales_the_input() {
        let text = "LUT_3D_SIZE 2\nDOMAIN_MIN 0 0 0\nDOMAIN_MAX 2 2 2\n\
                    0 0 0\n1 0 0\n0 1 0\n1 1 0\n0 0 1\n1 0 1\n0 1 1\n1 1 1\n";
        let lut = Lut3D::parse_cube_str(text).unwrap();
        let out = lut.sample_trilinear([1.0, 2.0, 0.0]);
        assert!((out[0] - 0.5).abs() < 1e-6 && (out[1] - 1.0).abs() < 1e-6 && out[2] == 0.0);
    }

    #[test]
    fn rejects_malformed_inputs() {
        let bad_size = Lut3D::parse_cube_str("LUT_3D_SIZE 1\n0 0 0\n");
        assert_eq!(bad_size, Err(LutError::SizeOutOfRange(1)));
        assert_eq!(
            Lut3D::parse_cube_str("LUT_3D_SIZE 66\n"),
            Err(LutError::SizeOutOfRange(66))
        );
        assert_eq!(Lut3D::parse_cube_str("0 0 0\n"), Err(LutError::MissingSize));
        assert!(matches!(
            Lut3D::parse_cube_str("LUT_1D_SIZE 4\n"),
            Err(LutError::Unsupported(_))
        ));
        assert!(matches!(
            Lut3D::parse_cube_str("LUT_3D_SIZE 2\n0 0 x\n"),
            Err(LutError::Malformed { line: 2, .. })
        ));
        assert!(matches!(
            Lut3D::parse_cube_str("LUT_3D_SIZE 2\n0 0 NaN\n"),
            Err(LutError::Malformed { .. })
        ));
        assert!(matches!(
            Lut3D::parse_cube_str("LUT_3D_SIZE 2\n0 0 0\n"),
            Err(LutError::SampleCount {
                expected: 8,
                found: 1
            })
        ));
        let mut too_many = identity_cube(2);
        too_many.push_str("0 0 0\n");
        assert!(matches!(
            Lut3D::parse_cube_str(&too_many),
            Err(LutError::SampleCount { .. })
        ));
        let bad_domain =
            identity_cube(2).replace("LUT_3D_SIZE 2", "LUT_3D_SIZE 2\nDOMAIN_MIN 1 0 0");
        assert_eq!(
            Lut3D::parse_cube_str(&bad_domain),
            Err(LutError::InvalidDomain)
        );
    }

    #[test]
    fn clip_lut_from_file_pins_content() {
        let dir = std::env::temp_dir().join(format!("tinixcut_lut_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("warm.cube");
        fs::write(&path, identity_cube(3)).unwrap();
        let clip_lut = ClipLut::from_file(&path).unwrap();
        clip_lut.validate().unwrap();
        assert_eq!(clip_lut.name, "Test");
        assert_eq!(clip_lut.sha256, lut_file_sha256(&path).unwrap());
        fs::write(&path, identity_cube(4)).unwrap();
        assert_ne!(clip_lut.sha256, lut_file_sha256(&path).unwrap());
        fs::remove_dir_all(dir).unwrap();
    }
}
