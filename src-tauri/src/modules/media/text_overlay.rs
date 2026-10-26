//! Phase 30 — Production rich-text overlay renderer.
//!
//! Consumes the canonical Phase 29 `TextRichStyle` domain values and produces a
//! deterministic straight-alpha RGBA overlay that both preview and headless
//! export consume (ADR-007). Animation evaluation is Phase 31; this renderer
//! draws the static resolved style at canonical time.

use crate::modules::domain::text::{
    AnimationTransform, AnimationUnit, FontReference, GradientKind, GradientSpace, RichTextSpan,
    StrokeAlign, TextBackgroundBox, TextFill, TextRichStyle, TextWrapMode,
};
use crate::modules::domain::text_animation::{
    evaluate_text_animation_with_layout, EvaluatedTextMotion,
};
use crate::modules::domain::{Asset, TextAlignment, TextAnchor};
use crate::modules::media::text_renderer_poc::{
    blur_alpha_mask, dilate_alpha_mask, ColorRgba, GlyphAlphaMask, RgbaBuffer,
    DEFAULT_CACHE_MAX_BYTES,
};
use ab_glyph::{Font, FontRef, Glyph, GlyphId, Point, PxScale, ScaleFont};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};
use thiserror::Error;
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

/// Bumped whenever layout/rasterization semantics change so derived caches (and
/// evidence) can be invalidated without touching the canonical project.
pub const TEXT_OVERLAY_RENDERER_VERSION: u32 = 1;

/// Default rich-text font size when neither the rich style nor the legacy item
/// carries one.
pub const DEFAULT_RICH_FONT_SIZE: f32 = 48.0;

/// Horizontal margin used when the style does not declare `max_width`.
pub const DEFAULT_MAX_WIDTH_MARGIN: f32 = 80.0;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum TextOverlayError {
    #[error("Font '{family}' is unavailable: {reason}")]
    MissingFont {
        family: String,
        asset_id: Option<String>,
        reason: String,
    },

    #[error("Font asset could not be parsed as an OpenType face: {0}")]
    InvalidFont(String),

    #[error("Per-span font overrides are not supported in Phase 30 (span {start}..{end} requests '{family}')")]
    UnsupportedPerSpanFont {
        start: u32,
        end: u32,
        family: String,
    },

    #[error("Rich text style is invalid: {0}")]
    InvalidStyle(String),

    #[error("Rich text contains graphemes without glyphs in the resolved font: {0:?}")]
    MissingGlyphs(Vec<String>),

    #[error("Composition could not be resolved for text rendering: {0}")]
    Composition(String),

    #[error("Template asset is missing or invalid: {0}")]
    MissingAsset(String),

    #[error("Template rendering failed: {0}")]
    Render(String),
}

/// Where the resolved font bytes came from. Recorded in diagnostics so evidence
/// can prove which face produced an overlay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum FontOrigin {
    Bundled,
    Asset(String),
}

#[derive(Debug, Clone)]
pub struct ResolvedFont {
    pub data: Vec<u8>,
    pub sha256: String,
    pub family: String,
    pub weight: u16,
    pub origin: FontOrigin,
    pub notes: Vec<String>,
}

impl ResolvedFont {
    /// The single font bundled with the application. Font assets/relinking are
    /// a Phase 32 resource-pack concern; until then the bundled face is the only
    /// implicit default.
    pub fn bundled() -> Self {
        let data = crate::modules::media::text_renderer_poc::default_open_source_font_bytes();
        Self {
            sha256: crate::modules::media::text_renderer_poc::compute_sha256_hex(&data),
            data,
            family: "Roboto".to_string(),
            weight: 700,
            origin: FontOrigin::Bundled,
            notes: vec!["bundled Roboto-Bold (Apache-2.0)".to_string()],
        }
    }
}

/// Resolves a canonical `FontReference` without silent substitution.
///
/// * unspecified font -> bundled default (documented in diagnostics)
/// * asset_id present -> that asset must exist and parse, otherwise an explicit
///   missing/invalid font error is returned
/// * named family without an asset -> only the bundled family is accepted
pub fn resolve_font(
    reference: Option<&FontReference>,
    assets: &[Asset],
) -> Result<ResolvedFont, TextOverlayError> {
    let Some(reference) = reference else {
        return Ok(ResolvedFont::bundled());
    };

    if let Some(asset_id) = reference.asset_id.as_deref() {
        let asset = assets
            .iter()
            .find(|asset| asset.asset_id == asset_id)
            .ok_or_else(|| TextOverlayError::MissingFont {
                family: reference.family.clone(),
                asset_id: Some(asset_id.to_string()),
                reason: "font asset is not present in the project".to_string(),
            })?;
        let data = std::fs::read(&asset.path).map_err(|error| TextOverlayError::MissingFont {
            family: reference.family.clone(),
            asset_id: Some(asset_id.to_string()),
            reason: format!("font asset file is missing or unreadable: {error}"),
        })?;
        FontRef::try_from_slice(&data)
            .map_err(|error| TextOverlayError::InvalidFont(format!("{}: {error}", asset.path)))?;
        let mut notes = vec![format!("resolved from asset '{}'", asset.name)];
        if reference.family != asset.name {
            notes.push(format!(
                "canonical family '{}' is recorded on asset '{}'",
                reference.family, asset.name
            ));
        }
        if reference.weight != 400 && reference.weight != 700 {
            notes.push(format!(
                "weight {} is approximated by the resolved face",
                reference.weight
            ));
        }
        return Ok(ResolvedFont {
            sha256: crate::modules::media::text_renderer_poc::compute_sha256_hex(&data),
            data,
            family: reference.family.clone(),
            weight: reference.weight,
            origin: FontOrigin::Asset(asset_id.to_string()),
            notes,
        });
    }

    let requested = reference.family.trim().to_ascii_lowercase();
    let bundled_families = ["roboto", "roboto bold", "roboto-bold"];
    if bundled_families.contains(&requested.as_str()) {
        let mut font = ResolvedFont::bundled();
        font.family = reference.family.clone();
        font.weight = reference.weight;
        if reference.weight != 700 {
            font.notes.push(format!(
                "weight {} is approximated by the bundled Bold face",
                reference.weight
            ));
        }
        return Ok(font);
    }

    Err(TextOverlayError::MissingFont {
        family: reference.family.clone(),
        asset_id: None,
        reason:
            "no bundled face matches this family; add a font asset and set FontReference.asset_id (no silent fallback)"
                .to_string(),
    })
}

#[derive(Debug, Clone, PartialEq)]
pub enum FillSpec {
    Solid(ColorRgba),
    Linear {
        angle_deg: f32,
        space: GradientSpace,
        stops: Vec<(f32, ColorRgba)>,
    },
    Radial {
        radius: Option<f32>,
        space: GradientSpace,
        stops: Vec<(f32, ColorRgba)>,
    },
}

impl FillSpec {
    fn from_domain(fill: &TextFill) -> Result<Self, TextOverlayError> {
        match fill {
            TextFill::Solid { color } => Ok(FillSpec::Solid(parse_hex_color(color)?)),
            TextFill::Gradient {
                gradient_kind,
                angle_deg,
                radius,
                space,
                stops,
            } => {
                if stops.is_empty() {
                    return Err(TextOverlayError::InvalidStyle(
                        "gradient must declare at least one stop".to_string(),
                    ));
                }
                let mut parsed = Vec::with_capacity(stops.len());
                for stop in stops {
                    parsed.push((stop.offset, parse_hex_color(&stop.color)?));
                }
                parsed.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                match gradient_kind {
                    GradientKind::Linear => Ok(FillSpec::Linear {
                        angle_deg: *angle_deg,
                        space: *space,
                        stops: parsed,
                    }),
                    GradientKind::Radial => Ok(FillSpec::Radial {
                        radius: *radius,
                        space: *space,
                        stops: parsed,
                    }),
                }
            }
        }
    }

    /// Samples the fill at a canvas/local coordinate using `bounds` as the
    /// gradient coordinate space (`[x0, y0, x1, y1]`).
    pub fn sample(&self, x: f32, y: f32, bounds: [f32; 4]) -> ColorRgba {
        match self {
            FillSpec::Solid(color) => *color,
            FillSpec::Linear {
                angle_deg, stops, ..
            } => {
                let [bx0, by0, bx1, by1] = bounds;
                let bw = (bx1 - bx0).max(1.0);
                let bh = (by1 - by0).max(1.0);
                let cx = (bx0 + bx1) / 2.0;
                let cy = (by0 + by1) / 2.0;
                let rad = angle_deg.to_radians();
                let dir_x = rad.cos();
                let dir_y = -rad.sin();
                let max_proj = (bw * dir_x.abs() + bh * dir_y.abs()) / 2.0;
                let proj = (x - cx) * dir_x + (y - cy) * dir_y;
                let t = if max_proj > 1e-3 {
                    ((proj / (max_proj * 2.0)) + 0.5).clamp(0.0, 1.0)
                } else {
                    0.5
                };
                sample_stops(stops, t)
            }
            FillSpec::Radial { radius, stops, .. } => {
                let [bx0, by0, bx1, by1] = bounds;
                let cx = (bx0 + bx1) / 2.0;
                let cy = (by0 + by1) / 2.0;
                let w = (bx1 - bx0).max(1.0);
                let h = (by1 - by0).max(1.0);
                let limit = radius
                    .filter(|value| *value > 0.0)
                    .unwrap_or_else(|| ((w / 2.0).powi(2) + (h / 2.0).powi(2)).sqrt());
                let distance = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
                let t = (distance / limit.max(1e-3)).clamp(0.0, 1.0);
                sample_stops(stops, t)
            }
        }
    }
}

fn sample_stops(stops: &[(f32, ColorRgba)], t: f32) -> ColorRgba {
    if stops.is_empty() {
        return ColorRgba::new(1.0, 1.0, 1.0, 1.0);
    }
    if t <= stops[0].0 {
        return stops[0].1;
    }
    let last = stops.len() - 1;
    if t >= stops[last].0 {
        return stops[last].1;
    }
    for window in stops.windows(2) {
        let (t0, c0) = window[0];
        let (t1, c1) = window[1];
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
    stops[last].1
}

#[derive(Debug, Clone)]
pub struct StrokeSpec {
    pub color: ColorRgba,
    pub width_px: f32,
    pub align: StrokeAlign,
}

impl StrokeSpec {
    fn effective_dilation(&self) -> f32 {
        match self.align {
            // Outside strokes grow the full authored width; centered/inside
            // strokes only expose half of it outside the glyph (the fill covers
            // the inner half). Documented approximation, not a silent rename.
            StrokeAlign::Outside => self.width_px,
            StrokeAlign::Center | StrokeAlign::Inside => self.width_px / 2.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ShadowSpec {
    pub color: ColorRgba,
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur: f32,
}

#[derive(Debug, Clone)]
pub struct BackgroundSpec {
    pub fill: FillSpec,
    pub padding_x: f32,
    pub padding_y: f32,
    pub corner_radius: f32,
    pub opacity: f32,
}

#[derive(Debug, Clone)]
struct SpanSpec {
    start: usize,
    end: usize,
    fill: Option<FillSpec>,
    font_size: Option<f32>,
    letter_spacing: Option<f32>,
    font: Option<FontReference>,
}

#[derive(Debug, Clone)]
pub struct OverlayStyle {
    pub font_size: f32,
    pub line_height: f32,
    pub letter_spacing: f32,
    pub alignment: TextAlignment,
    pub wrap_mode: TextWrapMode,
    pub max_width: Option<f32>,
    pub min_font_size: Option<f32>,
    pub max_font_size: Option<f32>,
    pub fill: FillSpec,
    pub strokes: Vec<StrokeSpec>,
    pub shadows: Vec<ShadowSpec>,
    pub background: Option<BackgroundSpec>,
    spans: Vec<SpanSpec>,
}

impl OverlayStyle {
    /// Builds the renderer-facing style from the canonical Phase 29 values.
    /// `fallback_font_size` keeps legacy item font sizes working when the rich
    /// style leaves `font_size` unset.
    pub fn from_rich(
        rich: &TextRichStyle,
        fallback_font_size: Option<f32>,
    ) -> Result<Self, TextOverlayError> {
        if let Some(size) = rich.font_size {
            if !size.is_finite() || size <= 0.0 {
                return Err(TextOverlayError::InvalidStyle(format!(
                    "font_size must be finite and positive, got {size}"
                )));
            }
        }
        let font_size = rich
            .font_size
            .or(fallback_font_size)
            .filter(|size| size.is_finite() && *size > 0.0)
            .unwrap_or(DEFAULT_RICH_FONT_SIZE);

        let fill = FillSpec::from_domain(&rich.fill)?;

        let mut strokes = Vec::with_capacity(rich.strokes.len());
        for stroke in &rich.strokes {
            if !stroke.width.is_finite() || stroke.width <= 0.0 {
                return Err(TextOverlayError::InvalidStyle(
                    "stroke width must be finite and positive".to_string(),
                ));
            }
            strokes.push(StrokeSpec {
                color: parse_hex_color(&stroke.color)?,
                width_px: stroke.width,
                align: stroke.align,
            });
        }

        let mut shadows = Vec::with_capacity(rich.shadows.len());
        for shadow in &rich.shadows {
            shadows.push(ShadowSpec {
                color: parse_hex_color(&shadow.color)?,
                offset_x: shadow.offset_x,
                offset_y: shadow.offset_y,
                blur: shadow.blur.max(0.0),
            });
        }

        let background = match &rich.background {
            Some(background) => Some(background_from_domain(background)?),
            None => None,
        };

        let mut spans = Vec::with_capacity(rich.spans.len());
        for RichTextSpan { start, end, style } in &rich.spans {
            spans.push(SpanSpec {
                start: *start as usize,
                end: *end as usize,
                fill: style.fill.as_ref().map(FillSpec::from_domain).transpose()?,
                font_size: style.font_size,
                letter_spacing: style.letter_spacing,
                font: style.font.clone(),
            });
        }

        Ok(Self {
            font_size,
            line_height: rich.layout.line_height.max(0.01),
            letter_spacing: rich.layout.letter_spacing,
            alignment: rich.layout.alignment,
            wrap_mode: rich.layout.wrap_mode,
            max_width: rich.layout.max_width,
            min_font_size: rich.layout.min_font_size,
            max_font_size: rich.layout.max_font_size,
            fill,
            strokes,
            shadows,
            background,
            spans,
        })
    }

    fn span_at(&self, index: usize) -> Option<&SpanSpec> {
        self.spans
            .iter()
            .find(|span| index >= span.start && index < span.end)
    }

    /// Phase 30 renders all spans with the resolved base face. A span that asks
    /// for a different face/asset must fail explicitly instead of silently
    /// falling back, because that would change the final export typography.
    fn validate_span_fonts(&self, base: &ResolvedFont) -> Result<(), TextOverlayError> {
        for span in &self.spans {
            let Some(font) = &span.font else {
                continue;
            };
            let same_asset = matches!(
                &base.origin,
                FontOrigin::Asset(asset_id) if font.asset_id.as_deref() == Some(asset_id.as_str())
            );
            let bundled_match = font.asset_id.is_none()
                && base.origin == FontOrigin::Bundled
                && font.family.eq_ignore_ascii_case(&base.family);
            if !same_asset && !bundled_match {
                return Err(TextOverlayError::UnsupportedPerSpanFont {
                    start: span.start as u32,
                    end: span.end as u32,
                    family: font.family.clone(),
                });
            }
        }
        Ok(())
    }

    fn geometry_signature(&self) -> u64 {
        fn mix(hash: u64, value: u64) -> u64 {
            let mut mixed = hash ^ value.wrapping_add(0x9E37_79B9_7F4A_7C15);
            mixed ^= mixed >> 30;
            mixed = mixed.wrapping_mul(0xBF58_476D_1CE4_E5B9);
            mixed ^= mixed >> 27;
            mixed = mixed.wrapping_mul(0x94D0_49BB_1331_11EB);
            mixed ^ (mixed >> 31)
        }

        // The vector lengths are part of the signature so a stroke vector can
        // never alias a shadow vector with matching numeric values.
        let mut sig = mix(0, TEXT_OVERLAY_RENDERER_VERSION as u64);
        sig = mix(sig, self.strokes.len() as u64);
        for stroke in &self.strokes {
            sig = mix(sig, (stroke.effective_dilation() * 100.0).round() as u64);
        }
        sig = mix(sig, self.shadows.len() as u64);
        for shadow in &self.shadows {
            sig = mix(sig, (shadow.blur * 100.0).round() as u64);
        }
        sig
    }
}

fn background_from_domain(
    background: &TextBackgroundBox,
) -> Result<BackgroundSpec, TextOverlayError> {
    Ok(BackgroundSpec {
        fill: FillSpec::from_domain(&background.fill)?,
        padding_x: background.padding_x.max(0.0),
        padding_y: background.padding_y.max(0.0),
        corner_radius: background.corner_radius.max(0.0),
        opacity: background.opacity.clamp(0.0, 1.0),
    })
}

/// Placement of the rendered text block on the canonical canvas. Transforms are
/// baked into the overlay pixels so preview and export consume identical output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlayPlacement {
    pub anchor: TextAnchor,
    pub offset_x: f32,
    pub offset_y: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub rotation_deg: f32,
    pub opacity: f32,
}

impl Default for OverlayPlacement {
    fn default() -> Self {
        Self {
            anchor: TextAnchor::Center,
            offset_x: 0.0,
            offset_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation_deg: 0.0,
            opacity: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayDiagnostics {
    pub font_family: String,
    pub font_sha256: String,
    pub font_origin: String,
    pub missing_glyphs: Vec<String>,
    pub notes: Vec<String>,
    pub renderer_version: u32,
}

/// ROI packet shared by the preview IPC seam and export PNG materialization.
#[derive(Debug, Clone, PartialEq)]
pub struct RichTextOverlay {
    pub width: u32,
    pub height: u32,
    pub roi: [u32; 4],
    pub roi_png_bytes: Vec<u8>,
    pub has_visible_content: bool,
    pub diagnostics: OverlayDiagnostics,
    pub render_micros: u128,
}

#[derive(Debug, Clone)]
struct PreparedGlyph {
    raw: GlyphAlphaMask,
    dilated: Vec<GlyphAlphaMask>,
    blurred: Vec<GlyphAlphaMask>,
    bytes: usize,
}

struct MaskCache {
    entries: HashMap<String, Arc<PreparedGlyph>>,
    order: VecDeque<String>,
    used_bytes: usize,
    max_bytes: usize,
}

impl MaskCache {
    fn new(max_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            used_bytes: 0,
            max_bytes,
        }
    }

    fn get(&mut self, key: &str) -> Option<Arc<PreparedGlyph>> {
        self.entries.get(key).cloned()
    }

    fn insert(&mut self, key: String, value: Arc<PreparedGlyph>) {
        if let Some(previous) = self.entries.insert(key.clone(), value.clone()) {
            self.used_bytes = self.used_bytes.saturating_sub(previous.bytes);
            self.order.retain(|existing| existing != &key);
        }
        self.used_bytes += value.bytes;
        self.order.push_back(key);
        while self.used_bytes > self.max_bytes {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(removed) = self.entries.remove(&oldest) {
                self.used_bytes = self.used_bytes.saturating_sub(removed.bytes);
            }
        }
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.used_bytes = 0;
    }

    fn stats(&self) -> (usize, usize) {
        (self.entries.len(), self.used_bytes)
    }
}

/// Production consumers share one renderer (and therefore one bounded mask
/// cache) per font/canvas identity so preview and export both benefit from
/// warm masks instead of rebuilding them on every IPC/export call.
pub fn shared_renderer(
    canvas_width: u32,
    canvas_height: u32,
    font: ResolvedFont,
) -> Arc<RichTextRenderer> {
    static SHARED: OnceLock<Mutex<HashMap<String, Arc<RichTextRenderer>>>> = OnceLock::new();
    let key = format!(
        "{}:{}:{}x{}:{}:{}",
        TEXT_OVERLAY_RENDERER_VERSION,
        font.sha256,
        canvas_width,
        canvas_height,
        font.family,
        origin_label(&font.origin)
    );
    let registry = SHARED.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = match registry.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(renderer) = guard.get(&key) {
        return renderer.clone();
    }
    if guard.len() >= 16 {
        guard.clear();
    }
    let renderer = Arc::new(RichTextRenderer::new(canvas_width, canvas_height, font));
    guard.insert(key, renderer.clone());
    renderer
}

/// Shared production renderer bound to one resolved font and canvas.
pub struct RichTextRenderer {
    pub canvas_width: u32,
    pub canvas_height: u32,
    font: ResolvedFont,
    cache: Mutex<MaskCache>,
}

impl RichTextRenderer {
    pub fn new(canvas_width: u32, canvas_height: u32, font: ResolvedFont) -> Self {
        Self {
            canvas_width,
            canvas_height,
            font,
            cache: Mutex::new(MaskCache::new(DEFAULT_CACHE_MAX_BYTES)),
        }
    }

    pub fn font(&self) -> &ResolvedFont {
        &self.font
    }

    pub fn font_sha256(&self) -> &str {
        &self.font.sha256
    }

    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
        }
    }

    pub fn cache_stats(&self) -> (usize, usize) {
        self.cache
            .lock()
            .map(|cache| cache.stats())
            .unwrap_or((0, 0))
    }

    /// Renders the overlay and returns a cropped ROI packet. Empty text is a
    /// valid, fully transparent overlay.
    pub fn render_overlay(
        &self,
        text: &str,
        style: &OverlayStyle,
        placement: &OverlayPlacement,
    ) -> Result<RichTextOverlay, TextOverlayError> {
        self.render_overlay_with_motion(text, style, placement, None)
    }

    /// Renders the overlay optionally applying evaluated per-grapheme text motion.
    pub fn render_overlay_with_motion(
        &self,
        text: &str,
        style: &OverlayStyle,
        placement: &OverlayPlacement,
        motion: Option<&EvaluatedTextMotion>,
    ) -> Result<RichTextOverlay, TextOverlayError> {
        let started = std::time::Instant::now();
        let diagnostics = self.diagnostics(text, style);
        if !diagnostics.missing_glyphs.is_empty() {
            return Err(TextOverlayError::MissingGlyphs(
                diagnostics.missing_glyphs.clone(),
            ));
        }

        let buffer = self.render_overlay_buffer_with_motion(text, style, placement, motion)?;
        let roi = buffer.compute_dirty_roi().unwrap_or([0, 0, 0, 0]);
        let has_visible_content = roi[2] > roi[0] && roi[3] > roi[1];
        let roi_png_bytes = if has_visible_content {
            buffer.crop(roi).to_png_bytes()
        } else {
            Vec::new()
        };

        Ok(RichTextOverlay {
            width: buffer.width,
            height: buffer.height,
            roi,
            roi_png_bytes,
            has_visible_content,
            diagnostics,
            render_micros: started.elapsed().as_micros(),
        })
    }

    /// Full-canvas RGBA overlay (diagnostics are available separately so export
    /// can fail explicitly on missing glyphs).
    pub fn render_overlay_buffer(
        &self,
        text: &str,
        style: &OverlayStyle,
        placement: &OverlayPlacement,
    ) -> Result<RgbaBuffer, TextOverlayError> {
        self.render_overlay_buffer_with_motion(text, style, placement, None)
    }

    /// Full-canvas RGBA overlay supporting evaluated per-grapheme text motion.
    pub fn render_overlay_buffer_with_motion(
        &self,
        text: &str,
        style: &OverlayStyle,
        placement: &OverlayPlacement,
        motion: Option<&EvaluatedTextMotion>,
    ) -> Result<RgbaBuffer, TextOverlayError> {
        let mut canvas = RgbaBuffer::new(self.canvas_width, self.canvas_height);
        if let Some((transformed, dest_x, dest_y)) =
            self.render_transformed_layer(text, style, placement, motion)?
        {
            composite_at(&mut canvas, &transformed, dest_x, dest_y);
        }
        Ok(canvas)
    }

    /// Renders text with full transformations (rotation, scaling, opacity, strokes, shadows)
    /// into an unclipped local buffer and calculates the target canvas destination coordinates `(transformed, dest_x, dest_y)`.
    pub fn render_transformed_layer(
        &self,
        text: &str,
        style: &OverlayStyle,
        placement: &OverlayPlacement,
        motion: Option<&EvaluatedTextMotion>,
    ) -> Result<Option<(RgbaBuffer, f32, f32)>, TextOverlayError> {
        style.validate_span_fonts(&self.font)?;
        let diagnostics = self.diagnostics(text, style);
        if !diagnostics.missing_glyphs.is_empty() {
            return Err(TextOverlayError::MissingGlyphs(diagnostics.missing_glyphs));
        }
        if text.trim().is_empty() {
            return Ok(None);
        }
        let local = self.render_local_layer(text, style, placement, motion)?;
        let Some(local) = local else {
            return Ok(None);
        };

        let transformed = transform_rgba(
            &local.buffer,
            placement.rotation_deg,
            placement.scale_x,
            placement.scale_y,
            placement.opacity.clamp(0.0, 1.0),
        );

        let block_center_x = local.block_x + local.block_width / 2.0;
        let block_center_y = local.block_y + local.block_height / 2.0;
        let dest_x = block_center_x - transformed.width as f32 / 2.0;
        let dest_y = block_center_y - transformed.height as f32 / 2.0;

        Ok(Some((transformed, dest_x, dest_y)))
    }

    pub fn diagnostics(&self, text: &str, style: &OverlayStyle) -> OverlayDiagnostics {
        let font = match FontRef::try_from_slice(&self.font.data) {
            Ok(font) => font,
            Err(_) => {
                return OverlayDiagnostics {
                    font_family: self.font.family.clone(),
                    font_sha256: self.font.sha256.clone(),
                    font_origin: origin_label(&self.font.origin),
                    missing_glyphs: Vec::new(),
                    notes: self.font.notes.clone(),
                    renderer_version: TEXT_OVERLAY_RENDERER_VERSION,
                };
            }
        };

        let nfc: String = text.nfc().collect();
        let mut missing = Vec::new();
        for (index, grapheme) in nfc.graphemes(true).enumerate() {
            let size = style
                .span_at(index)
                .and_then(|span| span.font_size)
                .unwrap_or(style.font_size)
                .max(0.1);
            let scaled = font.as_scaled(PxScale::from(size));
            let mut missing_here = false;
            for character in grapheme.chars() {
                if character.is_whitespace() || character.is_control() {
                    continue;
                }
                if scaled.glyph_id(character) == GlyphId(0) {
                    missing_here = true;
                }
            }
            if missing_here {
                missing.push(grapheme.to_string());
            }
        }
        missing.sort();
        missing.dedup();

        let mut notes = self.font.notes.clone();
        for span in &style.spans {
            if let Some(font) = &span.font {
                if font.family != self.font.family {
                    notes.push(format!(
                        "span {}..{} requests font '{}' which is not the resolved face",
                        span.start, span.end, font.family
                    ));
                }
            }
        }

        OverlayDiagnostics {
            font_family: self.font.family.clone(),
            font_sha256: self.font.sha256.clone(),
            font_origin: origin_label(&self.font.origin),
            missing_glyphs: missing,
            notes,
            renderer_version: TEXT_OVERLAY_RENDERER_VERSION,
        }
    }

    /// Computes layout-derived line groupings (including wrapped lines) for line-unit animations.
    pub fn layout_line_indices(
        &self,
        text: &str,
        style: &OverlayStyle,
    ) -> Result<Vec<Vec<usize>>, TextOverlayError> {
        let font = FontRef::try_from_slice(&self.font.data)
            .map_err(|error| TextOverlayError::InvalidFont(error.to_string()))?;
        let nfc: String = text.nfc().collect();
        let graphemes: Vec<String> = nfc.graphemes(true).map(str::to_string).collect();
        if graphemes.is_empty() {
            return Ok(Vec::new());
        }

        let max_width = style
            .max_width
            .unwrap_or_else(|| (self.canvas_width as f32 - DEFAULT_MAX_WIDTH_MARGIN * 2.0).max(1.0))
            .max(1.0);

        let mut factor = 1.0f32;
        if style.wrap_mode == TextWrapMode::AutoFit {
            let metrics = self.build_metrics(&font, &graphemes, style, 1.0);
            let widest = widest_paragraph(&metrics);
            if widest > max_width {
                factor = (max_width / widest).clamp(0.01, 1.0);
            }
            let requested = style.font_size * factor;
            let clamped = requested
                .max(style.min_font_size.unwrap_or(0.1))
                .min(style.max_font_size.unwrap_or(f32::MAX));
            factor = clamped / style.font_size;
        }

        let metrics = self.build_metrics(&font, &graphemes, style, factor);
        let wrap = matches!(style.wrap_mode, TextWrapMode::Wrap | TextWrapMode::AutoFit);
        let lines = build_lines(&metrics, wrap, max_width);
        Ok(lines.into_iter().map(|l| l.indices).collect())
    }

    /// Builds one glyph mask per grapheme, including derived stroke/shadow masks,
    /// with a bounded renderer-versioned cache.
    fn prepared_glyph(
        &self,
        grapheme: &str,
        font_size: f32,
        style: &OverlayStyle,
    ) -> Option<(Arc<PreparedGlyph>, f32)> {
        let font = FontRef::try_from_slice(&self.font.data).ok()?;
        let scaled = font.as_scaled(PxScale::from(font_size));
        let ascent = scaled.ascent();
        let signature = style.geometry_signature();
        let key = format!(
            "{}|{}|{}|{}|{}",
            self.font.sha256,
            (font_size * 10.0).round() as u32,
            signature,
            grapheme,
            TEXT_OVERLAY_RENDERER_VERSION
        );

        if let Ok(mut cache) = self.cache.lock() {
            if let Some(hit) = cache.get(&key) {
                return Some((hit, ascent));
            }
        }

        let raw = self.rasterize_cluster(grapheme, font_size)?;
        let mut bytes = raw.mask.len() + 64;

        let mut ordered_strokes: Vec<&StrokeSpec> = style.strokes.iter().collect();
        ordered_strokes.sort_by(|a, b| {
            b.effective_dilation()
                .partial_cmp(&a.effective_dilation())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let dilated: Vec<GlyphAlphaMask> = ordered_strokes
            .iter()
            .map(|stroke| dilate_alpha_mask(&raw, stroke.effective_dilation()))
            .collect();
        for mask in &dilated {
            bytes += mask.mask.len() + 32;
        }

        let blurred: Vec<GlyphAlphaMask> = style
            .shadows
            .iter()
            .map(|shadow| {
                if shadow.blur > 0.0 {
                    blur_alpha_mask(&raw, shadow.blur.round() as u32)
                } else {
                    raw.clone()
                }
            })
            .collect();
        for mask in &blurred {
            bytes += mask.mask.len() + 32;
        }

        let prepared = Arc::new(PreparedGlyph {
            raw,
            dilated,
            blurred,
            bytes,
        });
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(key, prepared.clone());
        }
        Some((prepared, ascent))
    }

    fn rasterize_cluster(&self, grapheme: &str, font_size: f32) -> Option<GlyphAlphaMask> {
        let font = FontRef::try_from_slice(&self.font.data).ok()?;
        let scale = PxScale::from(font_size);
        let scaled = font.as_scaled(scale);
        let mut pen_x = 0.0f32;
        let baseline_top = scaled.ascent();
        let mut previous: Option<GlyphId> = None;

        struct SubGlyph {
            min_x: i32,
            min_y: i32,
            width: u32,
            height: u32,
            mask: Vec<u8>,
        }

        let mut sub_glyphs = Vec::new();
        let mut union_min_x = i32::MAX;
        let mut union_min_y = i32::MAX;
        let mut union_max_x = i32::MIN;
        let mut union_max_y = i32::MIN;

        for character in grapheme.chars() {
            let glyph_id = scaled.glyph_id(character);
            if let Some(prev) = previous {
                pen_x += scaled.kern(prev, glyph_id);
            }
            let glyph = Glyph {
                id: glyph_id,
                scale,
                position: Point {
                    x: pen_x,
                    y: baseline_top,
                },
            };
            if let Some(outlined) = font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                let width = (bounds.width().ceil() as u32).max(1);
                let height = (bounds.height().ceil() as u32).max(1);
                let min_x = bounds.min.x.floor() as i32;
                let min_y = bounds.min.y.floor() as i32;
                let mut mask = vec![0u8; (width * height) as usize];
                outlined.draw(|gx, gy, coverage| {
                    if gx < width && gy < height {
                        mask[(gy * width + gx) as usize] =
                            (coverage * 255.0).clamp(0.0, 255.0).round() as u8;
                    }
                });
                union_min_x = union_min_x.min(min_x);
                union_min_y = union_min_y.min(min_y);
                union_max_x = union_max_x.max(min_x + width as i32);
                union_max_y = union_max_y.max(min_y + height as i32);
                sub_glyphs.push(SubGlyph {
                    min_x,
                    min_y,
                    width,
                    height,
                    mask,
                });
            }
            pen_x += scaled.h_advance(glyph_id);
            previous = Some(glyph_id);
        }

        if sub_glyphs.is_empty() || union_max_x <= union_min_x || union_max_y <= union_min_y {
            return None;
        }

        let total_width = (union_max_x - union_min_x) as u32;
        let total_height = (union_max_y - union_min_y) as u32;
        let mut combined = vec![0u8; (total_width * total_height) as usize];
        for sub in sub_glyphs {
            let offset_x = (sub.min_x - union_min_x) as u32;
            let offset_y = (sub.min_y - union_min_y) as u32;
            for glyph_y in 0..sub.height {
                for glyph_x in 0..sub.width {
                    let value = sub.mask[(glyph_y * sub.width + glyph_x) as usize];
                    if value > 0 {
                        let destination =
                            ((offset_y + glyph_y) * total_width + offset_x + glyph_x) as usize;
                        combined[destination] = combined[destination].max(value);
                    }
                }
            }
        }

        Some(GlyphAlphaMask {
            x: union_min_x,
            y: union_min_y,
            width: total_width,
            height: total_height,
            mask: combined,
        })
    }

    fn render_local_layer(
        &self,
        text: &str,
        style: &OverlayStyle,
        placement: &OverlayPlacement,
        motion: Option<&EvaluatedTextMotion>,
    ) -> Result<Option<LocalLayer>, TextOverlayError> {
        let font = FontRef::try_from_slice(&self.font.data)
            .map_err(|error| TextOverlayError::InvalidFont(error.to_string()))?;
        let nfc: String = text.nfc().collect();
        let graphemes: Vec<String> = nfc.graphemes(true).map(str::to_string).collect();
        if graphemes.is_empty() {
            return Ok(None);
        }

        let max_width = style
            .max_width
            .unwrap_or_else(|| (self.canvas_width as f32 - DEFAULT_MAX_WIDTH_MARGIN * 2.0).max(1.0))
            .max(1.0);

        let mut factor = 1.0f32;
        if style.wrap_mode == TextWrapMode::AutoFit {
            let metrics = self.build_metrics(&font, &graphemes, style, 1.0);
            let widest = widest_paragraph(&metrics);
            if widest > max_width {
                factor = (max_width / widest).clamp(0.01, 1.0);
            }
            let requested = style.font_size * factor;
            let clamped = requested
                .max(style.min_font_size.unwrap_or(0.1))
                .min(style.max_font_size.unwrap_or(f32::MAX));
            factor = clamped / style.font_size;
        }

        let metrics = self.build_metrics(&font, &graphemes, style, factor);
        let wrap = matches!(style.wrap_mode, TextWrapMode::Wrap | TextWrapMode::AutoFit);
        let lines = build_lines(&metrics, wrap, max_width);

        let use_wrap_box = wrap && lines.iter().any(|line| line.wrapped);
        let content_width = if use_wrap_box {
            max_width
        } else {
            lines
                .iter()
                .map(|line| line.width)
                .fold(0.0f32, f32::max)
                .max(1.0)
        };

        // ScaleFont::ascent() is positive and descent() is negative. The
        // descender magnitude must be added, not subtracted, otherwise glyph
        // bottoms (Vietnamese underdots, p/g/y tails) and shadows are clipped.
        let line_height = style.line_height.max(0.01) * style.font_size * factor;
        let max_ascent = metrics
            .iter()
            .filter(|metric| !metric.newline)
            .map(|metric| font.as_scaled(PxScale::from(metric.font_size)).ascent())
            .fold(0.0f32, f32::max);
        let max_descent = metrics
            .iter()
            .filter(|metric| !metric.newline)
            .map(|metric| -font.as_scaled(PxScale::from(metric.font_size)).descent())
            .fold(0.0f32, f32::max);
        let block_height =
            ((lines.len().saturating_sub(1)) as f32) * line_height + max_ascent + max_descent;

        let (anchor_x, anchor_y) = anchored_origin(
            placement.anchor,
            content_width,
            block_height,
            self.canvas_width as f32,
            self.canvas_height as f32,
        );
        let origin_x = anchor_x + placement.offset_x;
        let origin_y = anchor_y + placement.offset_y;

        let mut clusters = Vec::with_capacity(graphemes.len());
        for (line_index, line) in lines.iter().enumerate() {
            let line_x = origin_x
                + match style.alignment {
                    TextAlignment::Left => 0.0,
                    TextAlignment::Center => (content_width - line.width) / 2.0,
                    TextAlignment::Right => content_width - line.width,
                };
            let baseline_y = origin_y + max_ascent + line_index as f32 * line_height;
            let mut pen_x = line_x;
            for (position, index) in line.indices.iter().enumerate() {
                let metric = &metrics[*index];
                if position > 0 {
                    let previous = &metrics[line.indices[position - 1]];
                    pen_x += inter_cluster_kern(&font, previous, metric);
                }
                let cluster_scaled = font.as_scaled(PxScale::from(metric.font_size));
                clusters.push(LayoutCluster {
                    index: *index,
                    pen_x,
                    baseline_y,
                    advance: metric.width,
                    font_size: metric.font_size,
                    fill: metric.fill.clone(),
                    ascent: cluster_scaled.ascent(),
                    descent: -cluster_scaled.descent(),
                });
                pen_x += metric.width;
            }
        }

        if clusters.is_empty() {
            return Ok(None);
        }

        let mut prepared = Vec::with_capacity(clusters.len());
        for cluster in &clusters {
            if let Some((glyph, _)) =
                self.prepared_glyph(&graphemes[cluster.index], cluster.font_size, style)
            {
                prepared.push((cluster, glyph));
            }
        }
        if prepared.is_empty() {
            return Ok(None);
        }

        // Measure every raw/dilated/blurred mask (including shadow offsets and
        // background padding) before allocating the buffer, so stacked
        // diacritics and shadows can never be cropped by a heuristic margin.
        let (mut need_left, mut need_right, mut need_top, mut need_bottom) =
            (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        for (cluster, glyph) in &prepared {
            let mut consider = |mask: &GlyphAlphaMask, offset_x: f32, offset_y: f32| {
                let left = mask.x as f32 + offset_x;
                let right = left + mask.width as f32;
                let top = mask.y as f32 + offset_y;
                let bottom = top + mask.height as f32;
                need_left = need_left.max(-left);
                need_right = need_right.max(right - cluster.advance);
                need_top = need_top.max(-top);
                need_bottom = need_bottom.max(bottom - (cluster.ascent + cluster.descent));
            };
            consider(&glyph.raw, 0.0, 0.0);
            for mask in &glyph.dilated {
                consider(mask, 0.0, 0.0);
            }
            for (index, mask) in glyph.blurred.iter().enumerate() {
                let shadow = &style.shadows[index];
                consider(mask, shadow.offset_x, shadow.offset_y);
            }
        }
        if let Some(background) = &style.background {
            need_left = need_left.max(background.padding_x);
            need_right = need_right.max(background.padding_x);
            need_top = need_top.max(background.padding_y);
            need_bottom = need_bottom.max(background.padding_y);
        }
        let bleed = need_left
            .max(need_right)
            .max(need_top)
            .max(need_bottom)
            .max(0.0)
            + 4.0;

        let is_animated = motion.is_some_and(|m| !m.is_identity);
        if !is_animated {
            let buffer_width = (content_width + bleed * 2.0).ceil().max(1.0) as u32;
            let buffer_height = (block_height + bleed * 2.0).ceil().max(1.0) as u32;
            let mut buffer = RgbaBuffer::new(buffer_width, buffer_height);

            // Step 0 ? background box (lowest painter's layer).
            if let Some(background) = &style.background {
                let bounds = [
                    bleed - background.padding_x,
                    bleed - background.padding_y,
                    bleed + content_width + background.padding_x,
                    bleed + block_height + background.padding_y,
                ];
                render_rounded_rect(
                    &mut buffer,
                    bounds,
                    background.corner_radius,
                    &background.fill,
                    background.opacity,
                );
            }

            // Step 1 ? drop shadows (authored order).
            for shadow_index in 0..style.shadows.len() {
                let shadow = &style.shadows[shadow_index];
                for (cluster, glyph) in &prepared {
                    let Some(mask) = glyph.blurred.get(shadow_index) else {
                        continue;
                    };
                    let target_x = (bleed + cluster.pen_x - origin_x
                        + mask.x as f32
                        + shadow.offset_x)
                        .round() as i32;
                    let target_y = (bleed + cluster.baseline_y - cluster.ascent - origin_y
                        + mask.y as f32
                        + shadow.offset_y)
                        .round() as i32;
                    draw_mask(&mut buffer, mask, target_x, target_y, shadow.color, 1.0);
                }
            }

            // Step 2 ? strokes, widest/outermost first.
            let stroke_order = ordered_stroke_indices(&style.strokes);
            for (paint_index, stroke_index) in stroke_order.iter().enumerate() {
                let stroke = &style.strokes[*stroke_index];
                for (cluster, glyph) in &prepared {
                    let Some(mask) = glyph.dilated.get(paint_index) else {
                        continue;
                    };
                    let target_x =
                        (bleed + cluster.pen_x - origin_x + mask.x as f32).round() as i32;
                    let target_y = (bleed + cluster.baseline_y - cluster.ascent - origin_y
                        + mask.y as f32)
                        .round() as i32;
                    draw_mask(&mut buffer, mask, target_x, target_y, stroke.color, 1.0);
                }
            }

            // Step 3 ? glyph fills (per-span fill overrides respected).
            let ink_bounds = clusters.iter().fold(
                [f32::MAX, f32::MAX, f32::MIN, f32::MIN],
                |mut bounds, cluster| {
                    bounds[0] = bounds[0].min(cluster.pen_x);
                    bounds[1] = bounds[1].min(cluster.baseline_y - cluster.ascent);
                    bounds[2] = bounds[2].max(cluster.pen_x + cluster.advance);
                    bounds[3] = bounds[3].max(cluster.baseline_y + cluster.descent);
                    bounds
                },
            );
            let local_ink_bounds = [
                bleed + (ink_bounds[0] - origin_x),
                bleed + (ink_bounds[1] - origin_y),
                bleed + (ink_bounds[2] - origin_x),
                bleed + (ink_bounds[3] - origin_y),
            ];
            for (cluster, glyph) in &prepared {
                let mask = &glyph.raw;
                let cluster_bounds = [
                    bleed + (cluster.pen_x - origin_x),
                    bleed + (cluster.baseline_y - cluster.ascent - origin_y),
                    bleed + (cluster.pen_x + cluster.advance - origin_x),
                    bleed + (cluster.baseline_y + cluster.descent - origin_y),
                ];
                let gradient_bounds = match &cluster.fill {
                    FillSpec::Linear { space, .. } | FillSpec::Radial { space, .. } => {
                        match space {
                            GradientSpace::All => local_ink_bounds,
                            GradientSpace::Character => cluster_bounds,
                        }
                    }
                    FillSpec::Solid(_) => cluster_bounds,
                };
                draw_gradient_mask(
                    &mut buffer,
                    mask,
                    (bleed + cluster.pen_x - origin_x + mask.x as f32).round() as i32,
                    (bleed + cluster.baseline_y - cluster.ascent - origin_y + mask.y as f32).round()
                        as i32,
                    &cluster.fill,
                    gradient_bounds,
                    1.0,
                );
            }

            Ok(Some(LocalLayer {
                buffer,
                block_x: origin_x,
                block_y: origin_y,
                block_width: content_width,
                block_height,
            }))
        } else {
            let motion = motion.unwrap();

            // 1. Group prepared clusters by animation unit and compute unit bounding boxes & rigid pivots (Finding 3)
            let mut unit_bounds: Vec<Option<[f32; 4]>> = vec![None; motion.unit_count.max(1)];
            for (cluster, _) in &prepared {
                let u = motion
                    .grapheme_to_unit
                    .get(cluster.index)
                    .copied()
                    .unwrap_or(0);
                if u >= unit_bounds.len() {
                    unit_bounds.resize(u + 1, None);
                }
                let c_min_x = cluster.pen_x;
                let c_min_y = cluster.baseline_y - cluster.ascent;
                let c_max_x = cluster.pen_x + cluster.advance;
                let c_max_y = cluster.baseline_y + cluster.descent;
                match &mut unit_bounds[u] {
                    Some(b) => {
                        b[0] = b[0].min(c_min_x);
                        b[1] = b[1].min(c_min_y);
                        b[2] = b[2].max(c_max_x);
                        b[3] = b[3].max(c_max_y);
                    }
                    None => {
                        unit_bounds[u] = Some([c_min_x, c_min_y, c_max_x, c_max_y]);
                    }
                }
            }

            // Compute unit pivots and transforms
            struct UnitMotionInfo {
                pivot_x: f32,
                pivot_y: f32,
                transform: AnimationTransform,
            }
            let mut unit_infos: Vec<UnitMotionInfo> = Vec::with_capacity(unit_bounds.len());
            for (u, bounds_opt) in unit_bounds.iter().enumerate() {
                let b = bounds_opt.unwrap_or([0.0, 0.0, 1.0, 1.0]);
                let uw = (b[2] - b[0]).max(1.0);
                let uh = (b[3] - b[1]).max(1.0);
                let p_x = b[0] + uw * motion.pivot_x;
                let p_y = b[1] + uh * motion.pivot_y;
                let transform = motion.unit_transforms.get(u).copied().unwrap_or_default();
                unit_infos.push(UnitMotionInfo {
                    pivot_x: p_x,
                    pivot_y: p_y,
                    transform,
                });
            }

            // For each prepared cluster, compute its transformed destination pivot and effective transform
            struct ClusterRenderInfo {
                dest_local_x: f32,
                dest_local_y: f32,
                transform: AnimationTransform,
            }
            let mut cluster_render_info: Vec<ClusterRenderInfo> =
                Vec::with_capacity(prepared.len());
            for (cluster, _) in &prepared {
                let u = motion
                    .grapheme_to_unit
                    .get(cluster.index)
                    .copied()
                    .unwrap_or(0);
                let u_info = if u < unit_infos.len() {
                    &unit_infos[u]
                } else {
                    &unit_infos[0]
                };

                let c_min_x = cluster.pen_x;
                let c_min_y = cluster.baseline_y - cluster.ascent;
                let c_w = cluster.advance.max(1.0);
                let c_h = (cluster.ascent + cluster.descent).max(1.0);
                let c_p_x = c_min_x + c_w * motion.pivot_x;
                let c_p_y = c_min_y + c_h * motion.pivot_y;

                let dx = c_p_x - u_info.pivot_x;
                let dy = c_p_y - u_info.pivot_y;
                let rad = u_info.transform.rotation.to_radians();
                let cos_a = rad.cos();
                let sin_a = rad.sin();
                let sdx = dx * u_info.transform.scale_x;
                let sdy = dy * u_info.transform.scale_y;
                let rx = sdx * cos_a - sdy * sin_a;
                let ry = sdx * sin_a + sdy * cos_a;

                let dest_local_x = (u_info.pivot_x - origin_x) + u_info.transform.translate_x + rx;
                let dest_local_y = (u_info.pivot_y - origin_y) + u_info.transform.translate_y + ry;

                cluster_render_info.push(ClusterRenderInfo {
                    dest_local_x,
                    dest_local_y,
                    transform: u_info.transform,
                });
            }

            // 2. Compute exact union of transformed primitive bounds (Finding 6)
            let mut min_bound_x = 0.0f32;
            let mut max_bound_x = content_width;
            let mut min_bound_y = 0.0f32;
            let mut max_bound_y = block_height;

            for (c_idx, (cluster, _)) in prepared.iter().enumerate() {
                let r_info = &cluster_render_info[c_idx];
                if r_info.transform.opacity <= 1e-4 {
                    continue;
                }
                let u = motion
                    .grapheme_to_unit
                    .get(cluster.index)
                    .copied()
                    .unwrap_or(0);
                let u_info = if u < unit_infos.len() {
                    &unit_infos[u]
                } else {
                    &unit_infos[0]
                };

                // Extent of this cluster (including strokes and shadows)
                let c_box_left = cluster.pen_x - origin_x - need_left;
                let c_box_right = cluster.pen_x + cluster.advance - origin_x + need_right;
                let c_box_top = cluster.baseline_y - cluster.ascent - origin_y - need_top;
                let c_box_bottom = cluster.baseline_y + cluster.descent - origin_y + need_bottom;

                let corners = [
                    (c_box_left, c_box_top),
                    (c_box_right, c_box_top),
                    (c_box_left, c_box_bottom),
                    (c_box_right, c_box_bottom),
                ];

                let rad = r_info.transform.rotation.to_radians();
                let cos_a = rad.cos();
                let sin_a = rad.sin();

                for (cx, cy) in corners {
                    let dx = (cx + origin_x) - u_info.pivot_x;
                    let dy = (cy + origin_y) - u_info.pivot_y;
                    let sdx = dx * r_info.transform.scale_x;
                    let sdy = dy * r_info.transform.scale_y;
                    let rx = sdx * cos_a - sdy * sin_a;
                    let ry = sdx * sin_a + sdy * cos_a;

                    let tx = (u_info.pivot_x - origin_x) + r_info.transform.translate_x + rx;
                    let ty = (u_info.pivot_y - origin_y) + r_info.transform.translate_y + ry;

                    min_bound_x = min_bound_x.min(tx);
                    max_bound_x = max_bound_x.max(tx);
                    min_bound_y = min_bound_y.min(ty);
                    max_bound_y = max_bound_y.max(ty);
                }
            }

            if let Some(background) = &style.background {
                min_bound_x = min_bound_x.min(-background.padding_x);
                max_bound_x = max_bound_x.max(content_width + background.padding_x);
                min_bound_y = min_bound_y.min(-background.padding_y);
                max_bound_y = max_bound_y.max(block_height + background.padding_y);
            }

            let left_bleed = (-min_bound_x).max(0.0) + 4.0;
            let right_bleed = (max_bound_x - content_width).max(0.0) + 4.0;
            let top_bleed = (-min_bound_y).max(0.0) + 4.0;
            let bottom_bleed = (max_bound_y - block_height).max(0.0) + 4.0;

            let effective_bleed_x = left_bleed.max(right_bleed).max(bleed);
            let effective_bleed_y = top_bleed.max(bottom_bleed).max(bleed);

            let buffer_width = (content_width + effective_bleed_x * 2.0).ceil().max(1.0) as u32;
            let buffer_height = (block_height + effective_bleed_y * 2.0).ceil().max(1.0) as u32;
            let mut buffer = RgbaBuffer::new(buffer_width, buffer_height);

            // Step 0 — background box (lowest painter's layer).
            if let Some(background) = &style.background {
                let bounds = [
                    effective_bleed_x - background.padding_x,
                    effective_bleed_y - background.padding_y,
                    effective_bleed_x + content_width + background.padding_x,
                    effective_bleed_y + block_height + background.padding_y,
                ];
                render_rounded_rect(
                    &mut buffer,
                    bounds,
                    background.corner_radius,
                    &background.fill,
                    background.opacity,
                );
            }

            let ink_bounds = clusters.iter().fold(
                [f32::MAX, f32::MAX, f32::MIN, f32::MIN],
                |mut bounds, cluster| {
                    bounds[0] = bounds[0].min(cluster.pen_x);
                    bounds[1] = bounds[1].min(cluster.baseline_y - cluster.ascent);
                    bounds[2] = bounds[2].max(cluster.pen_x + cluster.advance);
                    bounds[3] = bounds[3].max(cluster.baseline_y + cluster.descent);
                    bounds
                },
            );

            let stroke_order = ordered_stroke_indices(&style.strokes);

            // Pre-render buffers for each cluster: shadow buffers, stroke buffers, fill buffer
            struct ClusterBuffers {
                pivot_src_x: f32,
                pivot_src_y: f32,
                shadows: Vec<RgbaBuffer>,
                strokes: Vec<RgbaBuffer>,
                fill: RgbaBuffer,
            }

            let mut cluster_buffers: Vec<ClusterBuffers> = Vec::with_capacity(prepared.len());

            for (cluster, glyph) in &prepared {
                let cluster_w = cluster.advance.max(1.0);
                let cluster_h = (cluster.ascent + cluster.descent).max(1.0);
                let cb_bleed = bleed + 4.0;
                let cb_w = (cluster_w + cb_bleed * 2.0).ceil().max(1.0) as u32;
                let cb_h = (cluster_h + cb_bleed * 2.0).ceil().max(1.0) as u32;

                // Shadows
                let mut shadows = Vec::with_capacity(style.shadows.len());
                for shadow_index in 0..style.shadows.len() {
                    let mut s_buf = RgbaBuffer::new(cb_w, cb_h);
                    let shadow = &style.shadows[shadow_index];
                    if let Some(mask) = glyph.blurred.get(shadow_index) {
                        let target_x = (cb_bleed + mask.x as f32 + shadow.offset_x).round() as i32;
                        let target_y = (cb_bleed + mask.y as f32 + shadow.offset_y).round() as i32;
                        draw_mask(&mut s_buf, mask, target_x, target_y, shadow.color, 1.0);
                    }
                    shadows.push(s_buf);
                }

                // Strokes (in sorted widest-to-narrowest order with consistent colors - Finding 5)
                let mut strokes = Vec::with_capacity(stroke_order.len());
                for (paint_index, stroke_index) in stroke_order.iter().enumerate() {
                    let mut st_buf = RgbaBuffer::new(cb_w, cb_h);
                    let stroke = &style.strokes[*stroke_index];
                    if let Some(mask) = glyph.dilated.get(paint_index) {
                        let target_x = (cb_bleed + mask.x as f32).round() as i32;
                        let target_y = (cb_bleed + mask.y as f32).round() as i32;
                        draw_mask(&mut st_buf, mask, target_x, target_y, stroke.color, 1.0);
                    }
                    strokes.push(st_buf);
                }

                // Fill with coordinate-shifted All-space gradient (Finding 4)
                let mut f_buf = RgbaBuffer::new(cb_w, cb_h);
                let mask = &glyph.raw;
                let cluster_char_bounds = [
                    cb_bleed,
                    cb_bleed,
                    cb_bleed + cluster_w,
                    cb_bleed + cluster_h,
                ];
                let gradient_bounds = match &cluster.fill {
                    FillSpec::Linear { space, .. } | FillSpec::Radial { space, .. } => {
                        match space {
                            GradientSpace::All => {
                                let b0 = cb_bleed + (ink_bounds[0] - cluster.pen_x);
                                let b1 = cb_bleed
                                    + (ink_bounds[1] - (cluster.baseline_y - cluster.ascent));
                                let b2 = b0 + (ink_bounds[2] - ink_bounds[0]);
                                let b3 = b1 + (ink_bounds[3] - ink_bounds[1]);
                                [b0, b1, b2, b3]
                            }
                            GradientSpace::Character => cluster_char_bounds,
                        }
                    }
                    FillSpec::Solid(_) => cluster_char_bounds,
                };
                draw_gradient_mask(
                    &mut f_buf,
                    mask,
                    (cb_bleed + mask.x as f32).round() as i32,
                    (cb_bleed + mask.y as f32).round() as i32,
                    &cluster.fill,
                    gradient_bounds,
                    1.0,
                );

                let pivot_src_x = cb_bleed + cluster_w * motion.pivot_x;
                let pivot_src_y = cb_bleed + cluster_h * motion.pivot_y;

                cluster_buffers.push(ClusterBuffers {
                    pivot_src_x,
                    pivot_src_y,
                    shadows,
                    strokes,
                    fill: f_buf,
                });
            }

            // Step 1: Global shadows pass across all graphemes (Finding 5)
            for shadow_index in 0..style.shadows.len() {
                for (c_idx, _) in prepared.iter().enumerate() {
                    let r_info = &cluster_render_info[c_idx];
                    if r_info.transform.opacity <= 1e-4
                        || (r_info.transform.scale_x.abs() < 1e-4
                            && r_info.transform.scale_y.abs() < 1e-4)
                    {
                        continue;
                    }
                    let c_buf = &cluster_buffers[c_idx];
                    let s_buf = &c_buf.shadows[shadow_index];
                    let dest_pivot_x = effective_bleed_x + r_info.dest_local_x;
                    let dest_pivot_y = effective_bleed_y + r_info.dest_local_y;

                    transform_and_composite_cluster(
                        &mut buffer,
                        s_buf,
                        c_buf.pivot_src_x,
                        c_buf.pivot_src_y,
                        dest_pivot_x,
                        dest_pivot_y,
                        r_info.transform.rotation,
                        r_info.transform.scale_x,
                        r_info.transform.scale_y,
                        r_info.transform.opacity,
                    );
                }
            }

            // Step 2: Global strokes pass (widest to narrowest) across all graphemes (Finding 5)
            for paint_index in 0..stroke_order.len() {
                for (c_idx, _) in prepared.iter().enumerate() {
                    let r_info = &cluster_render_info[c_idx];
                    if r_info.transform.opacity <= 1e-4
                        || (r_info.transform.scale_x.abs() < 1e-4
                            && r_info.transform.scale_y.abs() < 1e-4)
                    {
                        continue;
                    }
                    let c_buf = &cluster_buffers[c_idx];
                    let st_buf = &c_buf.strokes[paint_index];
                    let dest_pivot_x = effective_bleed_x + r_info.dest_local_x;
                    let dest_pivot_y = effective_bleed_y + r_info.dest_local_y;

                    transform_and_composite_cluster(
                        &mut buffer,
                        st_buf,
                        c_buf.pivot_src_x,
                        c_buf.pivot_src_y,
                        dest_pivot_x,
                        dest_pivot_y,
                        r_info.transform.rotation,
                        r_info.transform.scale_x,
                        r_info.transform.scale_y,
                        r_info.transform.opacity,
                    );
                }
            }

            // Step 3: Global fills pass across all graphemes (Finding 5)
            for (c_idx, _) in prepared.iter().enumerate() {
                let r_info = &cluster_render_info[c_idx];
                if r_info.transform.opacity <= 1e-4
                    || (r_info.transform.scale_x.abs() < 1e-4
                        && r_info.transform.scale_y.abs() < 1e-4)
                {
                    continue;
                }
                let c_buf = &cluster_buffers[c_idx];
                let dest_pivot_x = effective_bleed_x + r_info.dest_local_x;
                let dest_pivot_y = effective_bleed_y + r_info.dest_local_y;

                transform_and_composite_cluster(
                    &mut buffer,
                    &c_buf.fill,
                    c_buf.pivot_src_x,
                    c_buf.pivot_src_y,
                    dest_pivot_x,
                    dest_pivot_y,
                    r_info.transform.rotation,
                    r_info.transform.scale_x,
                    r_info.transform.scale_y,
                    r_info.transform.opacity,
                );
            }

            Ok(Some(LocalLayer {
                buffer,
                block_x: origin_x,
                block_y: origin_y,
                block_width: content_width,
                block_height,
            }))
        }
    }

    fn build_metrics(
        &self,
        font: &FontRef<'_>,
        graphemes: &[String],
        style: &OverlayStyle,
        factor: f32,
    ) -> Vec<GraphemeMetric> {
        let mut metrics = Vec::with_capacity(graphemes.len());
        for (index, grapheme) in graphemes.iter().enumerate() {
            let span = style.span_at(index);
            let font_size = span
                .and_then(|span| span.font_size)
                .unwrap_or(style.font_size)
                * factor;
            let font_size = font_size.max(0.1);
            let letter_spacing = span
                .and_then(|span| span.letter_spacing)
                .unwrap_or(style.letter_spacing);
            let fill = span
                .and_then(|span| span.fill.clone())
                .unwrap_or_else(|| style.fill.clone());
            let scaled = font.as_scaled(PxScale::from(font_size));

            let newline = grapheme == "\n" || grapheme == "\r\n";
            let mut width = 0.0f32;
            let mut first: Option<GlyphId> = None;
            let mut last: Option<GlyphId> = None;
            if !newline {
                for character in grapheme.chars() {
                    let glyph_id = scaled.glyph_id(character);
                    if let Some(previous) = last {
                        width += scaled.kern(previous, glyph_id);
                    } else {
                        first = Some(glyph_id);
                    }
                    width += scaled.h_advance(glyph_id);
                    last = Some(glyph_id);
                }
                if width <= 0.0 {
                    width = font_size * 0.35;
                }
                width += letter_spacing;
            }

            metrics.push(GraphemeMetric {
                width: width.max(0.0),
                first,
                last,
                font_size,
                newline,
                fill,
            });
        }
        metrics
    }
}

struct LocalLayer {
    buffer: RgbaBuffer,
    block_x: f32,
    block_y: f32,
    block_width: f32,
    block_height: f32,
}

#[derive(Debug, Clone)]
struct GraphemeMetric {
    width: f32,
    first: Option<GlyphId>,
    last: Option<GlyphId>,
    font_size: f32,
    newline: bool,
    fill: FillSpec,
}

#[derive(Debug, Clone)]
struct LayoutCluster {
    index: usize,
    pen_x: f32,
    baseline_y: f32,
    advance: f32,
    font_size: f32,
    fill: FillSpec,
    ascent: f32,
    descent: f32,
}

#[derive(Debug, Clone)]
struct LineLayout {
    indices: Vec<usize>,
    width: f32,
    wrapped: bool,
}

fn inter_cluster_kern(
    font: &FontRef<'_>,
    previous: &GraphemeMetric,
    current: &GraphemeMetric,
) -> f32 {
    if (previous.font_size - current.font_size).abs() > 1e-3 {
        return 0.0;
    }
    match (previous.last, current.first) {
        (Some(left), Some(right)) => font
            .as_scaled(PxScale::from(current.font_size))
            .kern(left, right),
        _ => 0.0,
    }
}

fn build_lines(metrics: &[GraphemeMetric], wrap: bool, max_width: f32) -> Vec<LineLayout> {
    let mut lines: Vec<LineLayout> = Vec::new();
    let mut indices: Vec<usize> = Vec::new();
    let mut width = 0.0f32;

    for (index, metric) in metrics.iter().enumerate() {
        if metric.newline {
            flush_line(&mut lines, &mut indices, &mut width, false);
            continue;
        }
        if wrap && !indices.is_empty() && width + metric.width > max_width {
            flush_line(&mut lines, &mut indices, &mut width, true);
        }
        indices.push(index);
        width += metric.width;
    }
    flush_line(&mut lines, &mut indices, &mut width, false);
    if lines.is_empty() {
        lines.push(LineLayout {
            indices: Vec::new(),
            width: 0.0,
            wrapped: false,
        });
    }
    lines
}

fn flush_line(
    lines: &mut Vec<LineLayout>,
    indices: &mut Vec<usize>,
    width: &mut f32,
    wrapped: bool,
) {
    if indices.is_empty() {
        return;
    }
    lines.push(LineLayout {
        indices: std::mem::take(indices),
        width: *width,
        wrapped,
    });
    *width = 0.0;
}

fn widest_paragraph(metrics: &[GraphemeMetric]) -> f32 {
    let mut widest = 0.0f32;
    let mut current = 0.0f32;
    for metric in metrics {
        if metric.newline {
            widest = widest.max(current);
            current = 0.0;
        } else {
            current += metric.width;
        }
    }
    widest.max(current)
}

fn ordered_stroke_indices(strokes: &[StrokeSpec]) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..strokes.len()).collect();
    indices.sort_by(|left, right| {
        strokes[*right]
            .effective_dilation()
            .partial_cmp(&strokes[*left].effective_dilation())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    indices
}

fn anchored_origin(
    anchor: TextAnchor,
    block_width: f32,
    block_height: f32,
    canvas_width: f32,
    canvas_height: f32,
) -> (f32, f32) {
    let x = match anchor {
        TextAnchor::TopLeft | TextAnchor::MiddleLeft | TextAnchor::BottomLeft => 0.0,
        TextAnchor::TopCenter | TextAnchor::Center | TextAnchor::BottomCenter => {
            (canvas_width - block_width) / 2.0
        }
        TextAnchor::TopRight | TextAnchor::MiddleRight | TextAnchor::BottomRight => {
            canvas_width - block_width
        }
    };
    let y = match anchor {
        TextAnchor::TopLeft | TextAnchor::TopCenter | TextAnchor::TopRight => 0.0,
        TextAnchor::MiddleLeft | TextAnchor::Center | TextAnchor::MiddleRight => {
            (canvas_height - block_height) / 2.0
        }
        TextAnchor::BottomLeft | TextAnchor::BottomCenter | TextAnchor::BottomRight => {
            canvas_height - block_height
        }
    };
    (x, y)
}

fn draw_mask(
    buffer: &mut RgbaBuffer,
    mask: &GlyphAlphaMask,
    target_x: i32,
    target_y: i32,
    color: ColorRgba,
    alpha_scale: f32,
) {
    let [r, g, b, a] = color.to_u8_array();
    for mask_y in 0..mask.height {
        let y = target_y + mask_y as i32;
        if y < 0 || y >= buffer.height as i32 {
            continue;
        }
        let row = (mask_y * mask.width) as usize;
        for mask_x in 0..mask.width {
            let coverage = mask.mask[row + mask_x as usize];
            if coverage == 0 {
                continue;
            }
            let x = target_x + mask_x as i32;
            if x < 0 || x >= buffer.width as i32 {
                continue;
            }
            let alpha = (coverage as f32 * (a as f32 / 255.0) * alpha_scale)
                .clamp(0.0, 255.0)
                .round() as u8;
            if alpha > 0 {
                buffer.blend_pixel_over(x as u32, y as u32, r, g, b, alpha);
            }
        }
    }
}

fn draw_gradient_mask(
    buffer: &mut RgbaBuffer,
    mask: &GlyphAlphaMask,
    target_x: i32,
    target_y: i32,
    fill: &FillSpec,
    bounds: [f32; 4],
    alpha_scale: f32,
) {
    for mask_y in 0..mask.height {
        let y = target_y + mask_y as i32;
        if y < 0 || y >= buffer.height as i32 {
            continue;
        }
        let row = (mask_y * mask.width) as usize;
        for mask_x in 0..mask.width {
            let coverage = mask.mask[row + mask_x as usize];
            if coverage == 0 {
                continue;
            }
            let x = target_x + mask_x as i32;
            if x < 0 || x >= buffer.width as i32 {
                continue;
            }
            let color = fill.sample(x as f32 + 0.5, y as f32 + 0.5, bounds);
            let [r, g, b, a] = color.to_u8_array();
            let alpha = (coverage as f32 * (a as f32 / 255.0) * alpha_scale)
                .clamp(0.0, 255.0)
                .round() as u8;
            if alpha > 0 {
                buffer.blend_pixel_over(x as u32, y as u32, r, g, b, alpha);
            }
        }
    }
}

fn render_rounded_rect(
    buffer: &mut RgbaBuffer,
    bounds: [f32; 4],
    radius: f32,
    fill: &FillSpec,
    opacity: f32,
) {
    let [x0, y0, x1, y1] = bounds;
    if x1 <= x0 || y1 <= y0 || opacity <= 0.0 {
        return;
    }
    let start_x = x0.floor().max(0.0) as i32;
    let start_y = y0.floor().max(0.0) as i32;
    let end_x = x1.ceil().min(buffer.width as f32) as i32;
    let end_y = y1.ceil().min(buffer.height as f32) as i32;
    let clamp_radius = radius.max(0.0).min((x1 - x0) / 2.0).min((y1 - y0) / 2.0);

    for y in start_y..end_y {
        let fy = y as f32 + 0.5;
        for x in start_x..end_x {
            let fx = x as f32 + 0.5;
            if fx < x0 || fx >= x1 || fy < y0 || fy >= y1 {
                continue;
            }
            if clamp_radius > 0.0 {
                let nearest_x = fx.clamp(x0 + clamp_radius, x1 - clamp_radius);
                let nearest_y = fy.clamp(y0 + clamp_radius, y1 - clamp_radius);
                let distance_sq = (fx - nearest_x).powi(2) + (fy - nearest_y).powi(2);
                if distance_sq > clamp_radius.powi(2) {
                    continue;
                }
            }
            let color = fill.sample(fx, fy, bounds);
            let [r, g, b, a] = color.to_u8_array();
            let alpha = (a as f32 * opacity).clamp(0.0, 255.0).round() as u8;
            if alpha > 0 {
                buffer.blend_pixel_over(x as u32, y as u32, r, g, b, alpha);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn transform_and_composite_cluster(
    target: &mut RgbaBuffer,
    source: &RgbaBuffer,
    pivot_src_x: f32,
    pivot_src_y: f32,
    dest_pivot_x: f32,
    dest_pivot_y: f32,
    angle_deg: f32,
    scale_x: f32,
    scale_y: f32,
    opacity: f32,
) {
    if opacity <= 1e-4 || source.width == 0 || source.height == 0 {
        return;
    }

    let sx = if scale_x.is_finite() && scale_x.abs() > 1e-4 {
        scale_x
    } else {
        return;
    };
    let sy = if scale_y.is_finite() && scale_y.abs() > 1e-4 {
        scale_y
    } else {
        return;
    };

    let rad = angle_deg.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();

    // 4 corners of source relative to pivot_src
    let corners = [
        (-pivot_src_x, -pivot_src_y),
        (source.width as f32 - pivot_src_x, -pivot_src_y),
        (-pivot_src_x, source.height as f32 - pivot_src_y),
        (
            source.width as f32 - pivot_src_x,
            source.height as f32 - pivot_src_y,
        ),
    ];

    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for (cx, cy) in corners {
        let tx = dest_pivot_x + (cx * sx * cos - cy * sy * sin);
        let ty = dest_pivot_y + (cx * sx * sin + cy * sy * cos);
        min_x = min_x.min(tx);
        min_y = min_y.min(ty);
        max_x = max_x.max(tx);
        max_y = max_y.max(ty);
    }

    let start_x = (min_x.floor() as i32).max(0);
    let end_x = (max_x.ceil() as i32).min(target.width as i32 - 1);
    let start_y = (min_y.floor() as i32).max(0);
    let end_y = (max_y.ceil() as i32).min(target.height as i32 - 1);

    if start_x > end_x || start_y > end_y {
        return;
    }

    for y in start_y..=end_y {
        let dy = y as f32 + 0.5 - dest_pivot_y;
        for x in start_x..=end_x {
            let dx = x as f32 + 0.5 - dest_pivot_x;
            let source_dx = (dx * cos + dy * sin) / sx;
            let source_dy = (-dx * sin + dy * cos) / sy;
            let px = pivot_src_x + source_dx - 0.5;
            let py = pivot_src_y + source_dy - 0.5;

            if px < -1.0 || py < -1.0 || px >= source.width as f32 || py >= source.height as f32 {
                continue;
            }

            let x0 = px.floor() as i32;
            let y0 = py.floor() as i32;
            let fx = px - x0 as f32;
            let fy = py - y0 as f32;

            let sample = |sx_i: i32, sy_i: i32| -> [f32; 4] {
                if sx_i >= 0
                    && sx_i < source.width as i32
                    && sy_i >= 0
                    && sy_i < source.height as i32
                {
                    let index = ((sy_i as u32 * source.width + sx_i as u32) * 4) as usize;
                    let a = source.data[index + 3] as f32 / 255.0;
                    [
                        source.data[index] as f32 * a,
                        source.data[index + 1] as f32 * a,
                        source.data[index + 2] as f32 * a,
                        a,
                    ]
                } else {
                    [0.0, 0.0, 0.0, 0.0]
                }
            };

            let p00 = sample(x0, y0);
            let p10 = sample(x0 + 1, y0);
            let p01 = sample(x0, y0 + 1);
            let p11 = sample(x0 + 1, y0 + 1);

            let w00 = (1.0 - fx) * (1.0 - fy);
            let w10 = fx * (1.0 - fy);
            let w01 = (1.0 - fx) * fy;
            let w11 = fx * fy;

            let accum_a = p00[3] * w00 + p10[3] * w10 + p01[3] * w01 + p11[3] * w11;
            if accum_a <= 1e-4 {
                continue;
            }

            let accum_r = p00[0] * w00 + p10[0] * w10 + p01[0] * w01 + p11[0] * w11;
            let accum_g = p00[1] * w00 + p10[1] * w10 + p01[1] * w01 + p11[1] * w11;
            let accum_b = p00[2] * w00 + p10[2] * w10 + p01[2] * w01 + p11[2] * w11;

            let inv_a = 1.0 / accum_a;
            let alpha = (accum_a.clamp(0.0, 1.0) * opacity.clamp(0.0, 1.0)).clamp(0.0, 1.0);
            let red = (accum_r * inv_a).clamp(0.0, 255.0);
            let green = (accum_g * inv_a).clamp(0.0, 255.0);
            let blue = (accum_b * inv_a).clamp(0.0, 255.0);

            let a_u8 = (alpha * 255.0).round() as u8;
            if a_u8 > 0 {
                target.blend_pixel_over(
                    x as u32,
                    y as u32,
                    red.round() as u8,
                    green.round() as u8,
                    blue.round() as u8,
                    a_u8,
                );
            }
        }
    }
}

/// Applies 2D scale/rotation around the buffer center and bakes opacity into
/// straight-alpha pixels. Deterministic bilinear sampling in premultiplied space.
fn transform_rgba(
    source: &RgbaBuffer,
    angle_deg: f32,
    scale_x: f32,
    scale_y: f32,
    opacity: f32,
) -> RgbaBuffer {
    let identity =
        angle_deg.abs() < 1e-4 && (scale_x - 1.0).abs() < 1e-4 && (scale_y - 1.0).abs() < 1e-4;
    if identity {
        let mut out = source.clone();
        if (opacity - 1.0).abs() > 1e-4 {
            for chunk in out.data.as_chunks_mut::<4>().0 {
                chunk[3] = (chunk[3] as f32 * opacity).clamp(0.0, 255.0) as u8;
            }
        }
        return out;
    }

    let sx = if scale_x.is_finite() {
        scale_x.max(0.01)
    } else {
        1.0
    };
    let sy = if scale_y.is_finite() {
        scale_y.max(0.01)
    } else {
        1.0
    };
    let rad = angle_deg.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();
    let width = source.width as f32;
    let height = source.height as f32;
    let center_x = width / 2.0;
    let center_y = height / 2.0;

    let corners = [
        (-center_x, -center_y),
        (width - center_x, -center_y),
        (-center_x, height - center_y),
        (width - center_x, height - center_y),
    ];
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for (x, y) in corners {
        let transformed_x = x * sx * cos - y * sy * sin;
        let transformed_y = x * sx * sin + y * sy * cos;
        min_x = min_x.min(transformed_x);
        min_y = min_y.min(transformed_y);
        max_x = max_x.max(transformed_x);
        max_y = max_y.max(transformed_y);
    }

    let out_width = (max_x - min_x).ceil().max(1.0) as u32;
    let out_height = (max_y - min_y).ceil().max(1.0) as u32;
    let out_center_x = out_width as f32 / 2.0;
    let out_center_y = out_height as f32 / 2.0;
    let mut out = RgbaBuffer::new(out_width, out_height);

    for y in 0..out_height {
        let dy = y as f32 + 0.5 - out_center_y;
        for x in 0..out_width {
            let dx = x as f32 + 0.5 - out_center_x;
            let source_dx = (dx * cos + dy * sin) / sx;
            let source_dy = (-dx * sin + dy * cos) / sy;
            let px = center_x + source_dx;
            let py = center_y + source_dy;
            if px < 0.0 || py < 0.0 || px >= width - 1.0 || py >= height - 1.0 {
                continue;
            }
            let x0 = px.floor() as u32;
            let y0 = py.floor() as u32;
            let fx = px - x0 as f32;
            let fy = py - y0 as f32;
            let mut accum = [0.0f32; 4];
            for (offset_x, offset_y, weight) in [
                (0u32, 0u32, (1.0 - fx) * (1.0 - fy)),
                (1, 0, fx * (1.0 - fy)),
                (0, 1, (1.0 - fx) * fy),
                (1, 1, fx * fy),
            ] {
                if weight <= 0.0 {
                    continue;
                }
                let index = (((y0 + offset_y) * source.width + x0 + offset_x) * 4) as usize;
                let alpha = source.data[index + 3] as f32 / 255.0;
                accum[0] += source.data[index] as f32 * alpha * weight;
                accum[1] += source.data[index + 1] as f32 * alpha * weight;
                accum[2] += source.data[index + 2] as f32 * alpha * weight;
                accum[3] += alpha * weight;
            }
            if accum[3] <= 1e-4 {
                continue;
            }
            let alpha = (accum[3].clamp(0.0, 1.0) * opacity).clamp(0.0, 1.0);
            let red = (accum[0] / accum[3]).clamp(0.0, 255.0);
            let green = (accum[1] / accum[3]).clamp(0.0, 255.0);
            let blue = (accum[2] / accum[3]).clamp(0.0, 255.0);
            let destination = ((y * out_width + x) * 4) as usize;
            out.data[destination] = red.round() as u8;
            out.data[destination + 1] = green.round() as u8;
            out.data[destination + 2] = blue.round() as u8;
            out.data[destination + 3] = (alpha * 255.0).round() as u8;
        }
    }
    out
}

fn composite_at(destination: &mut RgbaBuffer, source: &RgbaBuffer, offset_x: f32, offset_y: f32) {
    let start_x = offset_x.round() as i32;
    let start_y = offset_y.round() as i32;
    for y in 0..source.height {
        let target_y = start_y + y as i32;
        if target_y < 0 || target_y >= destination.height as i32 {
            continue;
        }
        for x in 0..source.width {
            let target_x = start_x + x as i32;
            if target_x < 0 || target_x >= destination.width as i32 {
                continue;
            }
            let index = ((y * source.width + x) * 4) as usize;
            let alpha = source.data[index + 3];
            if alpha == 0 {
                continue;
            }
            destination.blend_pixel_over(
                target_x as u32,
                target_y as u32,
                source.data[index],
                source.data[index + 1],
                source.data[index + 2],
                alpha,
            );
        }
    }
}

fn parse_hex_color(value: &str) -> Result<ColorRgba, TextOverlayError> {
    let trimmed = value.trim();
    let hex = trimmed.strip_prefix('#').ok_or_else(|| {
        TextOverlayError::InvalidStyle(format!("color '{value}' must start with '#'"))
    })?;
    if hex.len() != 6 && hex.len() != 8 {
        return Err(TextOverlayError::InvalidStyle(format!(
            "color '{value}' must be #RRGGBB or #RRGGBBAA"
        )));
    }
    if !hex.as_bytes().iter().all(u8::is_ascii_hexdigit) {
        return Err(TextOverlayError::InvalidStyle(format!(
            "color '{value}' contains non-hex characters"
        )));
    }
    let channel = |start: usize| -> Result<f32, TextOverlayError> {
        u8::from_str_radix(&hex[start..start + 2], 16)
            .map(|value| value as f32 / 255.0)
            .map_err(|error| TextOverlayError::InvalidStyle(error.to_string()))
    };
    let red = channel(0)?;
    let green = channel(2)?;
    let blue = channel(4)?;
    let alpha = if hex.len() == 8 { channel(6)? } else { 1.0 };
    Ok(ColorRgba::new(red, green, blue, alpha))
}

fn origin_label(origin: &FontOrigin) -> String {
    match origin {
        FontOrigin::Bundled => "bundled".to_string(),
        FontOrigin::Asset(asset_id) => format!("asset:{asset_id}"),
    }
}

/// Derives the renderer placement from a normalized composition layer.
pub fn placement_from_layer(
    layer: &crate::modules::composition::CompositionVisualLayer,
    opacity: f32,
) -> OverlayPlacement {
    let anchor = layer
        .text_style
        .as_ref()
        .map(|style| style.anchor)
        .unwrap_or(TextAnchor::Center);
    OverlayPlacement {
        anchor,
        offset_x: layer.transform.position_x,
        offset_y: layer.transform.position_y,
        scale_x: layer.transform.scale_x,
        scale_y: layer.transform.scale_y,
        rotation_deg: layer.transform.rotation,
        opacity: opacity.clamp(0.0, 1.0),
    }
}

fn parse_hex_color_rgba(hex: &str, opacity: f32) -> ColorRgba {
    let hex = hex.trim().trim_start_matches('#');
    let (r, g, b, a) = if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
        (r, g, b, 255)
    } else if hex.len() == 8 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
        let a = u8::from_str_radix(&hex[6..8], 16).unwrap_or(255);
        (r, g, b, a)
    } else {
        (255, 255, 255, 255)
    };
    ColorRgba::new(
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        (a as f32 / 255.0) * opacity.clamp(0.0, 1.0),
    )
}

fn calculate_layer_origin(
    anchor: TextAnchor,
    pos_x: f32,
    pos_y: f32,
    width: f32,
    height: f32,
    canvas_w: f32,
    canvas_h: f32,
) -> (f32, f32) {
    let (ax, ay) = match anchor {
        TextAnchor::TopLeft => (0.0, 0.0),
        TextAnchor::TopCenter => (0.5, 0.0),
        TextAnchor::TopRight => (1.0, 0.0),
        TextAnchor::MiddleLeft => (0.0, 0.5),
        TextAnchor::Center => (0.5, 0.5),
        TextAnchor::MiddleRight => (1.0, 0.5),
        TextAnchor::BottomLeft => (0.0, 1.0),
        TextAnchor::BottomCenter => (0.5, 1.0),
        TextAnchor::BottomRight => (1.0, 1.0),
    };
    let base_x = canvas_w * ax;
    let base_y = canvas_h * ay;
    let x0 = base_x + pos_x - (width * ax);
    let y0 = base_y + pos_y - (height * ay);
    (x0, y0)
}

fn render_ellipse_into(buf: &mut RgbaBuffer, x0: f32, y0: f32, w: f32, h: f32, col: ColorRgba) {
    let rx = (w / 2.0).max(1.0);
    let ry = (h / 2.0).max(1.0);
    let cx = x0 + rx;
    let cy = y0 + ry;
    let [cr, cg, cb, ca] = col.to_u8_array();
    let min_x = (x0.floor().max(0.0) as u32).min(buf.width);
    let max_x = ((x0 + w).ceil().max(0.0) as u32).min(buf.width);
    let min_y = (y0.floor().max(0.0) as u32).min(buf.height);
    let max_y = ((y0 + h).ceil().max(0.0) as u32).min(buf.height);

    for py in min_y..max_y {
        let dy = (py as f32 + 0.5 - cy) / ry;
        let dy_sq = dy * dy;
        if dy_sq > 1.0 {
            continue;
        }
        for px in min_x..max_x {
            let dx = (px as f32 + 0.5 - cx) / rx;
            let dist_sq = dx * dx + dy_sq;
            if dist_sq <= 1.0 {
                buf.blend_pixel_over(px, py, cr, cg, cb, ca);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_stroke_box_into(
    buf: &mut RgbaBuffer,
    x0: f32,
    y0: f32,
    w: f32,
    h: f32,
    stroke_width: f32,
    corner_radius: f32,
    col: ColorRgba,
) {
    let sw = stroke_width.max(1.0);
    let [cr, cg, cb, ca] = col.to_u8_array();
    let min_x = (x0.floor().max(0.0) as u32).min(buf.width);
    let max_x = ((x0 + w).ceil().max(0.0) as u32).min(buf.width);
    let min_y = (y0.floor().max(0.0) as u32).min(buf.height);
    let max_y = ((y0 + h).ceil().max(0.0) as u32).min(buf.height);

    let r_out = corner_radius.min(w / 2.0).min(h / 2.0);
    let r_in = (r_out - sw).max(0.0);

    for py in min_y..max_y {
        let fy = py as f32 + 0.5;
        if fy < y0 || fy > y0 + h {
            continue;
        }
        for px in min_x..max_x {
            let fx = px as f32 + 0.5;
            if fx < x0 || fx > x0 + w {
                continue;
            }

            let in_outer = if r_out > 0.0 {
                let dx = if fx < x0 + r_out {
                    (x0 + r_out) - fx
                } else if fx > x0 + w - r_out {
                    fx - (x0 + w - r_out)
                } else {
                    0.0
                };
                let dy = if fy < y0 + r_out {
                    (y0 + r_out) - fy
                } else if fy > y0 + h - r_out {
                    fy - (y0 + h - r_out)
                } else {
                    0.0
                };
                dx * dx + dy * dy <= r_out * r_out
            } else {
                true
            };

            if !in_outer {
                continue;
            }

            let in_inner =
                if fx >= x0 + sw && fx <= x0 + w - sw && fy >= y0 + sw && fy <= y0 + h - sw {
                    if r_in > 0.0 {
                        let dx = if fx < x0 + sw + r_in {
                            (x0 + sw + r_in) - fx
                        } else if fx > x0 + w - sw - r_in {
                            fx - (x0 + w - sw - r_in)
                        } else {
                            0.0
                        };
                        let dy = if fy < y0 + sw + r_in {
                            (y0 + sw + r_in) - fy
                        } else if fy > y0 + h - sw - r_in {
                            fy - (y0 + h - sw - r_in)
                        } else {
                            0.0
                        };
                        dx * dx + dy * dy <= r_in * r_in
                    } else {
                        true
                    }
                } else {
                    false
                };

            if !in_inner {
                buf.blend_pixel_over(px, py, cr, cg, cb, ca);
            }
        }
    }
}

fn render_scaled_image_into(
    dst: &mut RgbaBuffer,
    src: &RgbaBuffer,
    x0: f32,
    y0: f32,
    target_w: f32,
    target_h: f32,
    opacity: f32,
) {
    if src.width == 0 || src.height == 0 || target_w <= 0.0 || target_h <= 0.0 {
        return;
    }
    let min_x = (x0.floor().max(0.0) as u32).min(dst.width);
    let max_x = ((x0 + target_w).ceil().max(0.0) as u32).min(dst.width);
    let min_y = (y0.floor().max(0.0) as u32).min(dst.height);
    let max_y = ((y0 + target_h).ceil().max(0.0) as u32).min(dst.height);

    for dy in min_y..max_y {
        let sy = (((dy as f32 - y0) / target_h) * src.height as f32) as u32;
        if sy >= src.height {
            continue;
        }
        for dx in min_x..max_x {
            let sx = (((dx as f32 - x0) / target_w) * src.width as f32) as u32;
            if sx >= src.width {
                continue;
            }
            let [r, g, b, a] = src.get_pixel(sx, sy);
            let alpha = ((a as f32) * opacity).round() as u8;
            if alpha > 0 {
                dst.blend_pixel_over(dx, dy, r, g, b, alpha);
            }
        }
    }
}

/// Renders a multi-layer template instance into an RGBA buffer.
fn render_rounded_rect_into(
    buf: &mut RgbaBuffer,
    shape: &crate::modules::media::text_renderer_poc::ShapeLayerDef,
) {
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
            if (dx == 0.0 && dy == 0.0) || dx * dx + dy * dy <= r_sq {
                buf.blend_pixel_over(px as u32, py as u32, cr, cg, cb, ca);
            }
        }
    }
}

pub fn render_template_overlay_buffer(
    project: &crate::modules::domain::Project,
    canvas_width: u32,
    canvas_height: u32,
    layer: &crate::modules::composition::CompositionVisualLayer,
    instance: &crate::modules::domain::template::TemplateInstance,
    opacity: f32,
    motion_timestamp: Option<crate::modules::domain::time::RationalTime>,
) -> Result<Option<RgbaBuffer>, TextOverlayError> {
    if opacity <= 0.001 {
        return Ok(None);
    }

    let mut composite_buf = RgbaBuffer::new(canvas_width, canvas_height);
    let mut has_any_content = false;

    // Sample clip-level keyframe transform at motion_timestamp
    let (clip_pos_x, clip_pos_y, clip_scale_x, clip_scale_y, clip_rot) =
        if let Some(ts) = motion_timestamp {
            let clip_offset = ts.sub_exact(&layer.timeline_range.start);
            (
                layer.evaluate_property(
                    crate::modules::domain::KeyframeProperty::PositionX,
                    clip_offset,
                ),
                layer.evaluate_property(
                    crate::modules::domain::KeyframeProperty::PositionY,
                    clip_offset,
                ),
                layer.evaluate_property(
                    crate::modules::domain::KeyframeProperty::ScaleX,
                    clip_offset,
                ),
                layer.evaluate_property(
                    crate::modules::domain::KeyframeProperty::ScaleY,
                    clip_offset,
                ),
                layer.evaluate_property(
                    crate::modules::domain::KeyframeProperty::Rotation,
                    clip_offset,
                ),
            )
        } else {
            (
                layer.transform.position_x,
                layer.transform.position_y,
                layer.transform.scale_x,
                layer.transform.scale_y,
                layer.transform.rotation,
            )
        };

    // Calculate canvas responsive scale and safe margin clamp
    let (canvas_scale_x, canvas_scale_y, safe_margin) =
        if let Some(ref resp) = instance.responsive_policy {
            let ref_w = (resp.reference_width as f32).max(1.0);
            let ref_h = (resp.reference_height as f32).max(1.0);
            let sx = canvas_width as f32 / ref_w;
            let sy = canvas_height as f32 / ref_h;
            let uniform = sx.min(sy);
            (uniform, uniform, resp.safe_margin * uniform)
        } else {
            (1.0, 1.0, 0.0)
        };

    // Sort layers by z_index
    let mut sorted_layers: Vec<&crate::modules::domain::template::TemplateLayer> =
        instance.layers.iter().filter(|l| l.visible).collect();
    sorted_layers.sort_by_key(|l| l.z_index);

    for t_layer in sorted_layers {
        // Evaluate layer motion if present
        let (motion_dx, motion_dy, motion_sx, motion_sy, motion_rot, motion_alpha) =
            if let (Some(ts), Some(anim)) = (motion_timestamp, &t_layer.motion) {
                let clip_time = layer.source_time_at(ts);
                let clip_dur = if layer.source_range.duration.value > 0 {
                    layer.source_range.duration
                } else {
                    layer.timeline_range.duration
                };
                let evaluated = crate::modules::domain::text_animation::evaluate_text_animation(
                    "A", anim, clip_time, clip_dur,
                );
                if let Some(t) = evaluated.grapheme_transforms.first() {
                    (
                        t.translate_x,
                        t.translate_y,
                        t.scale_x,
                        t.scale_y,
                        t.rotation,
                        t.opacity,
                    )
                } else {
                    (0.0, 0.0, 1.0, 1.0, 0.0, 1.0)
                }
            } else {
                (0.0, 0.0, 1.0, 1.0, 0.0, 1.0)
            };

        let layer_opacity = (t_layer.opacity * motion_alpha).clamp(0.0, 1.0);
        if layer_opacity <= 0.001 {
            continue;
        }

        let eff_pos_x = (clip_pos_x + t_layer.transform.position_x + motion_dx) * canvas_scale_x;
        let eff_pos_y = (clip_pos_y + t_layer.transform.position_y + motion_dy) * canvas_scale_y;
        let eff_scale_x = clip_scale_x * t_layer.transform.scale_x * motion_sx * canvas_scale_x;
        let eff_scale_y = clip_scale_y * t_layer.transform.scale_y * motion_sy * canvas_scale_y;
        let eff_rot = clip_rot + t_layer.transform.rotation + motion_rot;

        match &t_layer.kind {
            crate::modules::domain::template::TemplateLayerKind::Shape(ref s) => {
                let rendered = render_shape_layer_into(
                    &mut composite_buf,
                    canvas_width as f32,
                    canvas_height as f32,
                    t_layer.anchor,
                    eff_pos_x,
                    eff_pos_y,
                    eff_scale_x,
                    eff_scale_y,
                    eff_rot,
                    layer_opacity,
                    safe_margin,
                    s,
                );
                if rendered {
                    has_any_content = true;
                }
            }

            crate::modules::domain::template::TemplateLayerKind::Text(ref t) => {
                let (text_str, rich_opt, anim_opt, text_style_opt) = if t.is_primary {
                    (
                        layer.text_content.as_deref().unwrap_or(""),
                        layer.text_rich.as_ref(),
                        layer.text_animation.as_ref(),
                        layer.text_style.as_ref(),
                    )
                } else {
                    (
                        t.text_content.as_deref().unwrap_or(""),
                        t.text_rich.as_ref(),
                        t.text_animation.as_ref(),
                        t.text_style.as_ref(),
                    )
                };

                if text_str.trim().is_empty() {
                    continue;
                }

                let font = if let Some(r) = rich_opt {
                    resolve_font(r.font.as_ref(), &project.assets)?
                } else if let Some(st) = text_style_opt {
                    let font_ref = FontReference {
                        family: if st.font_family == "Inter" {
                            "Roboto".to_string()
                        } else {
                            st.font_family.clone()
                        },
                        weight: st.font_weight,
                        style: crate::modules::domain::text::FontStyle::Normal,
                        asset_id: None,
                    };
                    resolve_font(Some(&font_ref), &project.assets)?
                } else {
                    resolve_font(None, &project.assets)?
                };

                let font_size = if let Some(r) = rich_opt {
                    r.font_size.unwrap_or(DEFAULT_RICH_FONT_SIZE) * canvas_scale_y
                } else if let Some(st) = text_style_opt {
                    st.font_size * canvas_scale_y
                } else {
                    DEFAULT_RICH_FONT_SIZE * canvas_scale_y
                };

                let mut style = if let Some(r) = rich_opt {
                    OverlayStyle::from_rich(r, Some(font_size))?
                } else if let Some(st) = text_style_opt {
                    let r = TextRichStyle {
                        font: Some(FontReference {
                            family: if st.font_family == "Inter" {
                                "Roboto".to_string()
                            } else {
                                st.font_family.clone()
                            },
                            weight: st.font_weight,
                            style: crate::modules::domain::text::FontStyle::Normal,
                            asset_id: None,
                        }),
                        font_size: Some(font_size),
                        layout: crate::modules::domain::text::TextLayout {
                            alignment: st.alignment,
                            line_height: st.line_height,
                            ..Default::default()
                        },
                        fill: TextFill::Solid {
                            color: st.font_color.clone(),
                        },
                        strokes: Vec::new(),
                        shadows: Vec::new(),
                        background: st.background_color.as_ref().map(|bg| TextBackgroundBox {
                            fill: TextFill::Solid { color: bg.clone() },
                            opacity: st.background_opacity,
                            padding_x: st.padding_x * canvas_scale_x,
                            padding_y: st.padding_y * canvas_scale_y,
                            corner_radius: 0.0,
                        }),
                        spans: Vec::new(),
                    };
                    OverlayStyle::from_rich(&r, Some(font_size))?
                } else {
                    OverlayStyle::from_rich(
                        &TextRichStyle::default(),
                        Some(DEFAULT_RICH_FONT_SIZE * canvas_scale_y),
                    )?
                };

                if let Some(resp) = &instance.responsive_policy {
                    let available_w = (canvas_width as f32 - 2.0 * safe_margin).max(1.0);
                    if resp.auto_wrap {
                        style.wrap_mode = crate::modules::domain::text::TextWrapMode::Wrap;
                        style.max_width = Some(
                            style
                                .max_width
                                .map_or(available_w, |mw| mw.min(available_w)),
                        );
                    }
                    if resp.auto_fit {
                        style.wrap_mode = crate::modules::domain::text::TextWrapMode::AutoFit;
                        style.min_font_size = Some(
                            style
                                .min_font_size
                                .map_or(12.0 * canvas_scale_y, |mf| mf * canvas_scale_y),
                        );
                        style.max_font_size = Some(font_size);
                    }
                }

                let layer_renderer = shared_renderer(canvas_width, canvas_height, font);

                // Text scales via font_size, not placement transform, to prevent double scaling
                let text_scale_x = clip_scale_x * t_layer.transform.scale_x * motion_sx;
                let text_scale_y = clip_scale_y * t_layer.transform.scale_y * motion_sy;

                let placement = OverlayPlacement {
                    anchor: t_layer.anchor,
                    offset_x: eff_pos_x,
                    offset_y: eff_pos_y,
                    scale_x: text_scale_x,
                    scale_y: text_scale_y,
                    rotation_deg: eff_rot,
                    opacity: layer_opacity,
                };

                let motion = if let (Some(timestamp), Some(anim)) = (motion_timestamp, anim_opt) {
                    let clip_time = layer.source_time_at(timestamp);
                    let clip_dur = if layer.source_range.duration.value > 0 {
                        layer.source_range.duration
                    } else {
                        layer.timeline_range.duration
                    };
                    let line_indices = if anim.unit == AnimationUnit::Line {
                        layer_renderer.layout_line_indices(text_str, &style).ok()
                    } else {
                        None
                    };
                    Some(evaluate_text_animation_with_layout(
                        text_str,
                        anim,
                        clip_time,
                        clip_dur,
                        line_indices.as_deref(),
                    ))
                } else {
                    None
                };

                // Propagate MissingGlyphs / UnsupportedPerSpanFont with ?
                let transformed_opt = layer_renderer.render_transformed_layer(
                    text_str,
                    &style,
                    &placement,
                    motion.as_ref(),
                )?;

                if let Some((transformed, mut dest_x, mut dest_y)) = transformed_opt {
                    if safe_margin > 0.0 {
                        if let Some(roi) = transformed.compute_dirty_roi() {
                            let min_safe_x = safe_margin;
                            let max_safe_x = (canvas_width as f32 - safe_margin).max(min_safe_x);
                            let min_safe_y = safe_margin;
                            let max_safe_y = (canvas_height as f32 - safe_margin).max(min_safe_y);

                            let ink_min_x = dest_x + roi[0] as f32;
                            let ink_max_x = dest_x + roi[2] as f32;
                            let ink_w = ink_max_x - ink_min_x;

                            let ink_min_y = dest_y + roi[1] as f32;
                            let ink_max_y = dest_y + roi[3] as f32;
                            let ink_h = ink_max_y - ink_min_y;

                            if ink_w <= (max_safe_x - min_safe_x) {
                                if ink_min_x < min_safe_x {
                                    dest_x += min_safe_x - ink_min_x;
                                } else if ink_max_x > max_safe_x {
                                    dest_x -= ink_max_x - max_safe_x;
                                }
                            }

                            if ink_h <= (max_safe_y - min_safe_y) {
                                if ink_min_y < min_safe_y {
                                    dest_y += min_safe_y - ink_min_y;
                                } else if ink_max_y > max_safe_y {
                                    dest_y -= ink_max_y - max_safe_y;
                                }
                            }
                        }
                    }

                    composite_at(&mut composite_buf, &transformed, dest_x, dest_y);
                    has_any_content = true;
                }
            }

            crate::modules::domain::template::TemplateLayerKind::Image(ref img) => {
                let aid = img.asset_id.as_ref().ok_or_else(|| {
                    TextOverlayError::MissingAsset(
                        "Unassigned image asset in template layer".to_string(),
                    )
                })?;
                let asset = project.find_asset(aid).ok_or_else(|| {
                    TextOverlayError::MissingAsset(format!(
                        "Template image asset not found in project: {}",
                        aid
                    ))
                })?;
                let img_bytes = std::fs::read(&asset.path).map_err(|e| {
                    TextOverlayError::Render(format!(
                        "Failed to read template image asset '{}' at '{}': {}",
                        aid, asset.path, e
                    ))
                })?;
                let img_buf = RgbaBuffer::from_png_bytes(&img_bytes).map_err(|e| {
                    TextOverlayError::Render(format!(
                        "Failed to decode PNG for template image asset '{}': {}",
                        aid, e
                    ))
                })?;

                render_image_layer_into(
                    &mut composite_buf,
                    canvas_width as f32,
                    canvas_height as f32,
                    t_layer.anchor,
                    eff_pos_x,
                    eff_pos_y,
                    eff_scale_x,
                    eff_scale_y,
                    eff_rot,
                    layer_opacity,
                    safe_margin,
                    &img_buf,
                    img.width,
                    img.height,
                );
                has_any_content = true;
            }
        }
    }

    if has_any_content {
        if opacity < 0.9999 {
            let op = opacity.clamp(0.0, 1.0);
            for chunk in composite_buf.data.chunks_mut(4) {
                if chunk.len() == 4 {
                    chunk[3] = (chunk[3] as f32 * op).round() as u8;
                }
            }
        }
        Ok(Some(composite_buf))
    } else {
        Ok(None)
    }
}

pub fn render_template_overlay(
    project: &crate::modules::domain::Project,
    canvas_width: u32,
    canvas_height: u32,
    layer: &crate::modules::composition::CompositionVisualLayer,
    instance: &crate::modules::domain::template::TemplateInstance,
    opacity: f32,
    motion_timestamp: Option<crate::modules::domain::time::RationalTime>,
) -> Result<Option<RichTextOverlay>, TextOverlayError> {
    let buf_opt = render_template_overlay_buffer(
        project,
        canvas_width,
        canvas_height,
        layer,
        instance,
        opacity,
        motion_timestamp,
    )?;

    let Some(buf) = buf_opt else {
        return Ok(None);
    };

    let roi = buf.compute_dirty_roi().unwrap_or([0, 0, 0, 0]);
    let roi_buf = buf.crop(roi);
    let roi_png_bytes = roi_buf.to_png_bytes();

    let font = ResolvedFont::bundled();
    let renderer = shared_renderer(canvas_width, canvas_height, font);
    let style = OverlayStyle::from_rich(&TextRichStyle::default(), Some(DEFAULT_RICH_FONT_SIZE))?;
    let diagnostics = renderer.diagnostics("template", &style);

    Ok(Some(RichTextOverlay {
        width: canvas_width,
        height: canvas_height,
        roi,
        roi_png_bytes,
        has_visible_content: true,
        diagnostics,
        render_micros: 0,
    }))
}

pub fn render_overlay_for_layer(
    project: &crate::modules::domain::Project,
    canvas_width: u32,
    canvas_height: u32,
    layer: &crate::modules::composition::CompositionVisualLayer,
    opacity: f32,
) -> Result<Option<RichTextOverlay>, TextOverlayError> {
    if let Some(instance) = &layer.template_instance {
        return render_template_overlay(
            project,
            canvas_width,
            canvas_height,
            layer,
            instance,
            opacity,
            None,
        );
    }
    let Some(rich) = layer.text_rich.as_ref() else {
        return Ok(None);
    };
    let text = layer.text_content.as_deref().unwrap_or("");
    if text.trim().is_empty() {
        return Ok(None);
    }

    let font = resolve_font(rich.font.as_ref(), &project.assets)?;
    let legacy_size = layer.text_style.as_ref().map(|style| style.font_size);
    let font_size = resolved_font_size(rich, layer.font_size, legacy_size);
    let style = OverlayStyle::from_rich(rich, Some(font_size))?;
    let placement = placement_from_layer(layer, opacity);
    let renderer = shared_renderer(canvas_width, canvas_height, font);
    renderer.render_overlay(text, &style, &placement).map(Some)
}

/// Preview DTO returned by the Tauri IPC seam. Placement is already baked into
/// the ROI pixels, so the WebView only blits the image at canvas coordinates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewRichTextOverlay {
    pub item_id: String,
    pub z_index: usize,
    pub roi: [u32; 4],
    pub roi_png_bytes: Vec<u8>,
    pub has_visible_content: bool,
    pub diagnostics: OverlayDiagnostics,
    pub render_micros: u128,
}

/// Renders an overlay for a visual layer at an exact timestamp, evaluating both
/// canonical text animation (if present) and layer keyframe transforms/opacity.
pub fn render_frame_overlay_at(
    project: &crate::modules::domain::Project,
    plan: &crate::modules::composition::CompositionPlan,
    layer: &crate::modules::composition::CompositionVisualLayer,
    timestamp: crate::modules::domain::time::RationalTime,
) -> Result<Option<RichTextOverlay>, TextOverlayError> {
    let sampled = plan.sample_preview_at(timestamp);
    let active_sample = sampled
        .active_layers
        .iter()
        .find(|l| l.item_id == layer.item_id);

    let effective_opacity = active_sample
        .map(|l| l.opacity)
        .unwrap_or(layer.opacity)
        .clamp(0.0, 1.0);

    if let Some(instance) = &layer.template_instance {
        return render_template_overlay(
            project,
            plan.width,
            plan.height,
            layer,
            instance,
            effective_opacity,
            Some(timestamp),
        );
    }

    let Some(rich) = layer.text_rich.as_ref() else {
        return Ok(None);
    };
    let text = layer.text_content.as_deref().unwrap_or("");
    if text.trim().is_empty() {
        return Ok(None);
    }

    let font = resolve_font(rich.font.as_ref(), &project.assets)?;
    let legacy_size = layer.text_style.as_ref().map(|style| style.font_size);
    let font_size = resolved_font_size(rich, layer.font_size, legacy_size);
    let style = OverlayStyle::from_rich(rich, Some(font_size))?;

    let sampled = plan.sample_preview_at(timestamp);
    let active_sample = sampled
        .active_layers
        .iter()
        .find(|l| l.item_id == layer.item_id);

    let effective_opacity = active_sample
        .map(|l| l.opacity)
        .unwrap_or(layer.opacity)
        .clamp(0.0, 1.0);

    let mut placement = placement_from_layer(layer, effective_opacity);
    placement.opacity = effective_opacity;
    if let Some(active) = active_sample {
        placement.offset_x = active.position.0;
        placement.offset_y = active.position.1;
        placement.scale_x = active.scale.0;
        placement.scale_y = active.scale.1;
        placement.rotation_deg = active.rotation;
    }

    let renderer = shared_renderer(plan.width, plan.height, font);
    let motion = if let Some(ref anim) = layer.text_animation {
        let clip_time = layer.source_time_at(timestamp);
        let clip_dur = if layer.source_range.duration.value > 0 {
            layer.source_range.duration
        } else {
            layer.timeline_range.duration
        };
        let line_indices = if anim.unit == AnimationUnit::Line {
            renderer.layout_line_indices(text, &style).ok()
        } else {
            None
        };
        Some(evaluate_text_animation_with_layout(
            text,
            anim,
            clip_time,
            clip_dur,
            line_indices.as_deref(),
        ))
    } else {
        None
    };

    renderer
        .render_overlay_with_motion(text, &style, &placement, motion.as_ref())
        .map(Some)
}

/// Full-canvas buffer variant of `render_frame_overlay_at` for high-throughput headless export.
pub fn render_frame_overlay_buffer_at(
    project: &crate::modules::domain::Project,
    plan: &crate::modules::composition::CompositionPlan,
    layer: &crate::modules::composition::CompositionVisualLayer,
    timestamp: crate::modules::domain::time::RationalTime,
) -> Result<Option<RgbaBuffer>, TextOverlayError> {
    let sampled = plan.sample_preview_at(timestamp);
    let active_sample = sampled
        .active_layers
        .iter()
        .find(|l| l.item_id == layer.item_id);

    let effective_opacity = active_sample
        .map(|l| l.opacity)
        .unwrap_or(layer.opacity)
        .clamp(0.0, 1.0);

    if let Some(instance) = &layer.template_instance {
        return render_template_overlay_buffer(
            project,
            plan.width,
            plan.height,
            layer,
            instance,
            effective_opacity,
            Some(timestamp),
        );
    }

    let Some(rich) = layer.text_rich.as_ref() else {
        return Ok(None);
    };
    let text = layer.text_content.as_deref().unwrap_or("");
    if text.trim().is_empty() {
        return Ok(None);
    }

    let font = resolve_font(rich.font.as_ref(), &project.assets)?;
    let legacy_size = layer.text_style.as_ref().map(|style| style.font_size);
    let font_size = resolved_font_size(rich, layer.font_size, legacy_size);
    let style = OverlayStyle::from_rich(rich, Some(font_size))?;

    let sampled = plan.sample_preview_at(timestamp);
    let active_sample = sampled
        .active_layers
        .iter()
        .find(|l| l.item_id == layer.item_id);

    let effective_opacity = active_sample
        .map(|l| l.opacity)
        .unwrap_or(layer.opacity)
        .clamp(0.0, 1.0);

    let mut placement = placement_from_layer(layer, effective_opacity);
    placement.opacity = effective_opacity;
    if let Some(active) = active_sample {
        placement.offset_x = active.position.0;
        placement.offset_y = active.position.1;
        placement.scale_x = active.scale.0;
        placement.scale_y = active.scale.1;
        placement.rotation_deg = active.rotation;
    }

    let renderer = shared_renderer(plan.width, plan.height, font);
    let motion = if let Some(ref anim) = layer.text_animation {
        let clip_time = layer.source_time_at(timestamp);
        let clip_dur = if layer.source_range.duration.value > 0 {
            layer.source_range.duration
        } else {
            layer.timeline_range.duration
        };
        let line_indices = if anim.unit == AnimationUnit::Line {
            renderer.layout_line_indices(text, &style).ok()
        } else {
            None
        };
        Some(evaluate_text_animation_with_layout(
            text,
            anim,
            clip_time,
            clip_dur,
            line_indices.as_deref(),
        ))
    } else {
        None
    };

    renderer
        .render_overlay_buffer_with_motion(text, &style, &placement, motion.as_ref())
        .map(Some)
}

/// Resolves the active rich-text layer for `item_id` at the canonical frame and
/// renders its overlay. Missing items/time ranges return `None`.
pub fn preview_overlay_at(
    project: &crate::modules::domain::Project,
    item_id: &str,
    timestamp: crate::modules::domain::time::RationalTime,
) -> Result<Option<PreviewRichTextOverlay>, TextOverlayError> {
    let plan = crate::modules::composition::CompositionPlan::from_project(project)
        .map_err(|error| TextOverlayError::Composition(error.to_string()))?;
    let sampled = plan.sample_preview_at(timestamp);
    let layer = sampled
        .active_layers
        .iter()
        .find(|layer| layer.item_id == item_id);
    let Some(layer) = layer else {
        return Ok(None);
    };

    let composition_layer = plan
        .visual_layers
        .iter()
        .find(|candidate| candidate.item_id == item_id);
    let Some(composition_layer) = composition_layer else {
        return Ok(None);
    };

    let _effective_opacity = layer.opacity.clamp(0.0, 1.0);

    if let Some(instance) = &composition_layer.template_instance {
        let start = std::time::Instant::now();
        let overlay_opt = render_template_overlay(
            project,
            plan.width,
            plan.height,
            composition_layer,
            instance,
            1.0,
            Some(timestamp),
        )?;
        let render_micros = start.elapsed().as_micros();
        return Ok(overlay_opt.map(|overlay| PreviewRichTextOverlay {
            item_id: item_id.to_string(),
            z_index: composition_layer.z_index,
            roi: overlay.roi,
            roi_png_bytes: overlay.roi_png_bytes,
            has_visible_content: overlay.has_visible_content,
            diagnostics: overlay.diagnostics,
            render_micros,
        }));
    }

    let Some(rich) = composition_layer.text_rich.as_ref() else {
        return Ok(None);
    };
    let text = composition_layer.text_content.as_deref().unwrap_or("");
    if text.trim().is_empty() {
        return Ok(None);
    }

    let font = resolve_font(rich.font.as_ref(), &project.assets)?;
    let legacy_size = composition_layer
        .text_style
        .as_ref()
        .map(|style| style.font_size);
    let font_size = resolved_font_size(rich, composition_layer.font_size, legacy_size);
    let style = OverlayStyle::from_rich(rich, Some(font_size))?;

    let renderer = shared_renderer(plan.width, plan.height, font);
    let (placement, motion) = if let Some(ref anim) = composition_layer.text_animation {
        // Preview applies sampled opacity in CSS; bake per-unit animation
        // opacity only through motion, not the whole-layer opacity.
        let mut p = placement_from_layer(composition_layer, 1.0);
        p.opacity = 1.0;
        p.offset_x = layer.position.0;
        p.offset_y = layer.position.1;
        p.scale_x = layer.scale.0;
        p.scale_y = layer.scale.1;
        p.rotation_deg = layer.rotation;

        let clip_time = composition_layer.source_time_at(timestamp);
        let clip_dur = if composition_layer.source_range.duration.value > 0 {
            composition_layer.source_range.duration
        } else {
            composition_layer.timeline_range.duration
        };
        let line_indices = if anim.unit == AnimationUnit::Line {
            renderer.layout_line_indices(text, &style).ok()
        } else {
            None
        };
        (
            p,
            Some(evaluate_text_animation_with_layout(
                text,
                anim,
                clip_time,
                clip_dur,
                line_indices.as_deref(),
            )),
        )
    } else {
        // Generic transform/opacity keyframes must reach the preview even
        // without per-unit animation; opacity stays a CSS property.
        let mut p = placement_from_layer(composition_layer, 1.0);
        p.offset_x = layer.position.0;
        p.offset_y = layer.position.1;
        p.scale_x = layer.scale.0;
        p.scale_y = layer.scale.1;
        p.rotation_deg = layer.rotation;
        (p, None)
    };

    let overlay = renderer.render_overlay_with_motion(text, &style, &placement, motion.as_ref())?;
    Ok(Some(PreviewRichTextOverlay {
        item_id: item_id.to_string(),
        z_index: layer.z_index,
        roi: overlay.roi,
        roi_png_bytes: overlay.roi_png_bytes,
        has_visible_content: overlay.has_visible_content,
        diagnostics: overlay.diagnostics,
        render_micros: overlay.render_micros,
    }))
}

/// Resolved font size fallback chain used by both consumers.
pub fn resolved_font_size(
    rich: &TextRichStyle,
    layer_font_size: Option<f32>,
    legacy_style_size: Option<f32>,
) -> f32 {
    rich.font_size
        .or(layer_font_size)
        .or(legacy_style_size)
        .filter(|size| size.is_finite() && *size > 0.0)
        .unwrap_or(DEFAULT_RICH_FONT_SIZE)
}

#[allow(clippy::too_many_arguments)]
fn render_shape_layer_into(
    composite_buf: &mut RgbaBuffer,
    canvas_w: f32,
    canvas_h: f32,
    anchor: TextAnchor,
    pos_x: f32,
    pos_y: f32,
    scale_x: f32,
    scale_y: f32,
    rotation_deg: f32,
    opacity: f32,
    safe_margin: f32,
    s: &crate::modules::domain::template::TemplateShapeLayerData,
) -> bool {
    let w = (s.width * scale_x).max(1.0);
    let h = (s.height * scale_y).max(1.0);
    let (mut x0, mut y0) = calculate_layer_origin(anchor, pos_x, pos_y, w, h, canvas_w, canvas_h);

    if safe_margin > 0.0 {
        if w <= canvas_w - 2.0 * safe_margin {
            x0 = x0.clamp(safe_margin, canvas_w - safe_margin - w);
        }
        if h <= canvas_h - 2.0 * safe_margin {
            y0 = y0.clamp(safe_margin, canvas_h - safe_margin - h);
        }
    }

    let mut rendered_any = false;

    if rotation_deg.abs() > 0.001 {
        let pad = (s.stroke_width * scale_x.max(scale_y)).ceil() as u32 * 2 + 4;
        let local_w = (w.ceil() as u32) + pad;
        let local_h = (h.ceil() as u32) + pad;
        let mut local_buf = RgbaBuffer::new(local_w, local_h);
        let lx = pad as f32 / 2.0;
        let ly = pad as f32 / 2.0;

        if let Some(ref color_hex) = s.fill_color {
            let col = parse_hex_color_rgba(color_hex, opacity);
            match s.shape_kind {
                crate::modules::domain::template::TemplateShapeKind::Rectangle
                | crate::modules::domain::template::TemplateShapeKind::RoundedRectangle => {
                    let shape_def = crate::modules::media::text_renderer_poc::ShapeLayerDef {
                        x: lx,
                        y: ly,
                        width: w,
                        height: h,
                        color: col,
                        border_radius: s.corner_radius * scale_x,
                    };
                    render_rounded_rect_into(&mut local_buf, &shape_def);
                    rendered_any = true;
                }
                crate::modules::domain::template::TemplateShapeKind::Line => {
                    let line_h = (s.height.max(s.stroke_width).max(1.0) * scale_y).max(1.0);
                    let shape_def = crate::modules::media::text_renderer_poc::ShapeLayerDef {
                        x: lx,
                        y: ly,
                        width: w,
                        height: line_h,
                        color: col,
                        border_radius: s.corner_radius,
                    };
                    render_rounded_rect_into(&mut local_buf, &shape_def);
                    rendered_any = true;
                }
                crate::modules::domain::template::TemplateShapeKind::Circle
                | crate::modules::domain::template::TemplateShapeKind::Ellipse => {
                    render_ellipse_into(&mut local_buf, lx, ly, w, h, col);
                    rendered_any = true;
                }
            }
        }

        if let Some(ref stroke_hex) = s.stroke_color {
            if s.stroke_width > 0.0 {
                let stroke_col = parse_hex_color_rgba(stroke_hex, opacity);
                render_stroke_box_into(
                    &mut local_buf,
                    lx,
                    ly,
                    w,
                    h,
                    s.stroke_width * scale_x,
                    s.corner_radius * scale_x,
                    stroke_col,
                );
                rendered_any = true;
            }
        }

        if rendered_any {
            let rotated_buf = transform_rgba(&local_buf, rotation_deg, 1.0, 1.0, 1.0);
            let orig_center_x = x0 + w / 2.0;
            let orig_center_y = y0 + h / 2.0;
            let mut target_x0 = orig_center_x - (rotated_buf.width as f32 / 2.0);
            let mut target_y0 = orig_center_y - (rotated_buf.height as f32 / 2.0);

            if safe_margin > 0.0 {
                let rot_w = rotated_buf.width as f32;
                let rot_h = rotated_buf.height as f32;
                if rot_w <= canvas_w - 2.0 * safe_margin {
                    target_x0 = target_x0.clamp(safe_margin, canvas_w - safe_margin - rot_w);
                }
                if rot_h <= canvas_h - 2.0 * safe_margin {
                    target_y0 = target_y0.clamp(safe_margin, canvas_h - safe_margin - rot_h);
                }
            }

            let min_x = (target_x0.floor().max(0.0) as u32).min(composite_buf.width);
            let max_x = ((target_x0 + rotated_buf.width as f32).ceil().max(0.0) as u32)
                .min(composite_buf.width);
            let min_y = (target_y0.floor().max(0.0) as u32).min(composite_buf.height);
            let max_y = ((target_y0 + rotated_buf.height as f32).ceil().max(0.0) as u32)
                .min(composite_buf.height);

            for py in min_y..max_y {
                let sy = py as i32 - target_y0.floor() as i32;
                if sy < 0 || sy >= rotated_buf.height as i32 {
                    continue;
                }
                for px in min_x..max_x {
                    let sx = px as i32 - target_x0.floor() as i32;
                    if sx < 0 || sx >= rotated_buf.width as i32 {
                        continue;
                    }
                    let idx = ((sy as u32 * rotated_buf.width + sx as u32) * 4) as usize;
                    let r = rotated_buf.data[idx];
                    let g = rotated_buf.data[idx + 1];
                    let b = rotated_buf.data[idx + 2];
                    let a = rotated_buf.data[idx + 3];
                    if a > 0 {
                        composite_buf.blend_pixel_over(px, py, r, g, b, a);
                    }
                }
            }
        }
    } else {
        if let Some(ref color_hex) = s.fill_color {
            let col = parse_hex_color_rgba(color_hex, opacity);
            match s.shape_kind {
                crate::modules::domain::template::TemplateShapeKind::Rectangle
                | crate::modules::domain::template::TemplateShapeKind::RoundedRectangle => {
                    let shape_def = crate::modules::media::text_renderer_poc::ShapeLayerDef {
                        x: x0,
                        y: y0,
                        width: w,
                        height: h,
                        color: col,
                        border_radius: s.corner_radius * scale_x,
                    };
                    render_rounded_rect_into(composite_buf, &shape_def);
                    rendered_any = true;
                }
                crate::modules::domain::template::TemplateShapeKind::Line => {
                    let line_h = (s.height.max(s.stroke_width).max(1.0) * scale_y).max(1.0);
                    let shape_def = crate::modules::media::text_renderer_poc::ShapeLayerDef {
                        x: x0,
                        y: y0,
                        width: w,
                        height: line_h,
                        color: col,
                        border_radius: s.corner_radius,
                    };
                    render_rounded_rect_into(composite_buf, &shape_def);
                    rendered_any = true;
                }
                crate::modules::domain::template::TemplateShapeKind::Circle
                | crate::modules::domain::template::TemplateShapeKind::Ellipse => {
                    render_ellipse_into(composite_buf, x0, y0, w, h, col);
                    rendered_any = true;
                }
            }
        }

        if let Some(ref stroke_hex) = s.stroke_color {
            if s.stroke_width > 0.0 {
                let stroke_col = parse_hex_color_rgba(stroke_hex, opacity);
                render_stroke_box_into(
                    composite_buf,
                    x0,
                    y0,
                    w,
                    h,
                    s.stroke_width * scale_x,
                    s.corner_radius * scale_x,
                    stroke_col,
                );
                rendered_any = true;
            }
        }
    }

    rendered_any
}

#[allow(clippy::too_many_arguments)]
fn render_image_layer_into(
    composite_buf: &mut RgbaBuffer,
    canvas_w: f32,
    canvas_h: f32,
    anchor: TextAnchor,
    pos_x: f32,
    pos_y: f32,
    scale_x: f32,
    scale_y: f32,
    rotation_deg: f32,
    opacity: f32,
    safe_margin: f32,
    img_buf: &RgbaBuffer,
    img_w: f32,
    img_h: f32,
) {
    let w = (img_w * scale_x).max(1.0);
    let h = (img_h * scale_y).max(1.0);
    let (mut x0, mut y0) = calculate_layer_origin(anchor, pos_x, pos_y, w, h, canvas_w, canvas_h);

    if safe_margin > 0.0 {
        if w <= canvas_w - 2.0 * safe_margin {
            x0 = x0.clamp(safe_margin, canvas_w - safe_margin - w);
        }
        if h <= canvas_h - 2.0 * safe_margin {
            y0 = y0.clamp(safe_margin, canvas_h - safe_margin - h);
        }
    }

    if rotation_deg.abs() > 0.001 {
        let mut scaled_buf = RgbaBuffer::new(w.ceil() as u32, h.ceil() as u32);
        render_scaled_image_into(&mut scaled_buf, img_buf, 0.0, 0.0, w, h, opacity);
        let rotated_buf = transform_rgba(&scaled_buf, rotation_deg, 1.0, 1.0, 1.0);
        let orig_center_x = x0 + w / 2.0;
        let orig_center_y = y0 + h / 2.0;
        let mut target_x0 = orig_center_x - (rotated_buf.width as f32 / 2.0);
        let mut target_y0 = orig_center_y - (rotated_buf.height as f32 / 2.0);

        if safe_margin > 0.0 {
            let rot_w = rotated_buf.width as f32;
            let rot_h = rotated_buf.height as f32;
            if rot_w <= canvas_w - 2.0 * safe_margin {
                target_x0 = target_x0.clamp(safe_margin, canvas_w - safe_margin - rot_w);
            }
            if rot_h <= canvas_h - 2.0 * safe_margin {
                target_y0 = target_y0.clamp(safe_margin, canvas_h - safe_margin - rot_h);
            }
        }

        let min_x = (target_x0.floor().max(0.0) as u32).min(composite_buf.width);
        let max_x = ((target_x0 + rotated_buf.width as f32).ceil().max(0.0) as u32)
            .min(composite_buf.width);
        let min_y = (target_y0.floor().max(0.0) as u32).min(composite_buf.height);
        let max_y = ((target_y0 + rotated_buf.height as f32).ceil().max(0.0) as u32)
            .min(composite_buf.height);

        for py in min_y..max_y {
            let sy = py as i32 - target_y0.floor() as i32;
            if sy < 0 || sy >= rotated_buf.height as i32 {
                continue;
            }
            for px in min_x..max_x {
                let sx = px as i32 - target_x0.floor() as i32;
                if sx < 0 || sx >= rotated_buf.width as i32 {
                    continue;
                }
                let idx = ((sy as u32 * rotated_buf.width + sx as u32) * 4) as usize;
                let r = rotated_buf.data[idx];
                let g = rotated_buf.data[idx + 1];
                let b = rotated_buf.data[idx + 2];
                let a = rotated_buf.data[idx + 3];
                if a > 0 {
                    composite_buf.blend_pixel_over(px, py, r, g, b, a);
                }
            }
        }
    } else {
        render_scaled_image_into(composite_buf, img_buf, x0, y0, w, h, opacity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::domain::text::{
        GradientStop, TextBackgroundBox, TextFill, TextLayout, TextRichStyle, TextSpanStyle,
        TextStroke,
    };

    fn renderer() -> RichTextRenderer {
        RichTextRenderer::new(1920, 1080, ResolvedFont::bundled())
    }

    fn base_style(fill: TextFill) -> TextRichStyle {
        TextRichStyle {
            font: None,
            font_size: Some(96.0),
            layout: TextLayout::default(),
            fill,
            strokes: Vec::new(),
            shadows: Vec::new(),
            background: None,
            spans: Vec::new(),
        }
    }

    #[test]
    fn solid_fill_renders_vietnamese_diacritics_as_graphemes() {
        let renderer = renderer();
        let rich = base_style(TextFill::Solid {
            color: "#FF3355".to_string(),
        });
        let style = OverlayStyle::from_rich(&rich, None).expect("style");
        let overlay = renderer
            .render_overlay("Tiếng Việt", &style, &OverlayPlacement::default())
            .expect("render");
        assert!(overlay.has_visible_content);
        let decoded = RgbaBuffer::from_png_bytes(&overlay.roi_png_bytes).expect("decode roi png");
        assert_eq!(decoded.width, overlay.roi[2] - overlay.roi[0]);
        assert_eq!(decoded.height, overlay.roi[3] - overlay.roi[1]);
        assert!(overlay.diagnostics.missing_glyphs.is_empty());
    }

    #[test]
    fn linear_gradient_and_radial_gradient_differ() {
        let renderer = renderer();
        let linear = base_style(TextFill::Gradient {
            gradient_kind: GradientKind::Linear,
            angle_deg: 118.0,
            radius: None,
            space: GradientSpace::All,
            stops: vec![
                GradientStop {
                    offset: 0.0,
                    color: "#FF0000".to_string(),
                },
                GradientStop {
                    offset: 1.0,
                    color: "#0000FF".to_string(),
                },
            ],
        });
        let radial = base_style(TextFill::Gradient {
            gradient_kind: GradientKind::Radial,
            angle_deg: 0.0,
            radius: Some(200.0),
            space: GradientSpace::Character,
            stops: vec![
                GradientStop {
                    offset: 0.0,
                    color: "#FFFFFF".to_string(),
                },
                GradientStop {
                    offset: 1.0,
                    color: "#000000".to_string(),
                },
            ],
        });
        let linear_style = OverlayStyle::from_rich(&linear, None).expect("style");
        let radial_style = OverlayStyle::from_rich(&radial, None).expect("style");
        let linear_overlay = renderer
            .render_overlay("T", &linear_style, &OverlayPlacement::default())
            .expect("linear");
        let radial_overlay = renderer
            .render_overlay("T", &radial_style, &OverlayPlacement::default())
            .expect("radial");
        assert_ne!(linear_overlay.roi_png_bytes, radial_overlay.roi_png_bytes);
    }

    #[test]
    fn multi_stroke_and_shadow_change_roi_but_keep_alpha() {
        let renderer = renderer();
        let mut rich = base_style(TextFill::Solid {
            color: "#FFFFFF".to_string(),
        });
        rich.strokes = vec![
            TextStroke {
                color: "#AD5100".to_string(),
                width: 4.0,
                align: StrokeAlign::Outside,
            },
            TextStroke {
                color: "#4A2000".to_string(),
                width: 7.0,
                align: StrokeAlign::Outside,
            },
        ];
        rich.shadows = vec![crate::modules::domain::text::TextShadow {
            color: "#000000".to_string(),
            offset_x: 4.0,
            offset_y: 4.0,
            blur: 3.0,
        }];
        let style = OverlayStyle::from_rich(&rich, None).expect("style");
        let overlay = renderer
            .render_overlay("Tinix", &style, &OverlayPlacement::default())
            .expect("render");
        let decoded = RgbaBuffer::from_png_bytes(&overlay.roi_png_bytes).expect("png");
        assert!(overlay.has_visible_content);
        assert!(decoded
            .data
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[3] > 0));
    }

    #[test]
    fn per_span_fill_and_size_alter_rendering() {
        let renderer = renderer();
        let mut rich = base_style(TextFill::Solid {
            color: "#FFFFFF".to_string(),
        });
        rich.spans = vec![RichTextSpan {
            start: 0,
            end: 2,
            style: TextSpanStyle {
                fill: Some(TextFill::Solid {
                    color: "#00FF00".to_string(),
                }),
                font_size: Some(140.0),
                ..TextSpanStyle::default()
            },
        }];
        let style = OverlayStyle::from_rich(&rich, None).expect("style");
        let overlay = renderer
            .render_overlay("Hello", &style, &OverlayPlacement::default())
            .expect("render");
        assert!(overlay.has_visible_content);
    }

    #[test]
    fn wrap_and_auto_fit_respect_max_width() {
        let renderer = renderer();
        let mut rich = base_style(TextFill::Solid {
            color: "#FFFFFF".to_string(),
        });
        rich.layout = TextLayout {
            max_width: Some(220.0),
            wrap_mode: TextWrapMode::Wrap,
            line_height: 1.2,
            ..TextLayout::default()
        };
        let wrap_style = OverlayStyle::from_rich(&rich, None).expect("style");
        let wrap_overlay = renderer
            .render_overlay(
                "Tinix rich text wrapping",
                &wrap_style,
                &OverlayPlacement::default(),
            )
            .expect("wrap");
        assert!(wrap_overlay.roi[2] - wrap_overlay.roi[0] <= 260);

        rich.layout.wrap_mode = TextWrapMode::AutoFit;
        let fitted_style = OverlayStyle::from_rich(&rich, None).expect("style");
        let fitted_overlay = renderer
            .render_overlay(
                "Tinix rich text wrapping",
                &fitted_style,
                &OverlayPlacement::default(),
            )
            .expect("autofit");
        assert!(fitted_overlay.roi[2] - fitted_overlay.roi[0] <= 260);
    }

    #[test]
    fn background_box_and_opacity_are_applied() {
        let renderer = renderer();
        let mut rich = base_style(TextFill::Solid {
            color: "#FFFFFF".to_string(),
        });
        rich.background = Some(TextBackgroundBox {
            fill: TextFill::Solid {
                color: "#000000".to_string(),
            },
            padding_x: 20.0,
            padding_y: 10.0,
            corner_radius: 12.0,
            opacity: 1.0,
        });
        let style = OverlayStyle::from_rich(&rich, None).expect("style");
        let overlay = renderer
            .render_overlay("A", &style, &OverlayPlacement::default())
            .expect("render");
        assert!(overlay.has_visible_content);

        let placement = OverlayPlacement {
            opacity: 0.25,
            ..OverlayPlacement::default()
        };
        let faded = renderer
            .render_overlay("A", &style, &placement)
            .expect("render");
        let full = RgbaBuffer::from_png_bytes(&overlay.roi_png_bytes).expect("png");
        let faded_pixels = RgbaBuffer::from_png_bytes(&faded.roi_png_bytes).expect("png");
        let full_max_alpha = full
            .data
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| pixel[3])
            .max()
            .unwrap_or(0);
        let faded_max_alpha = faded_pixels
            .data
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| pixel[3])
            .max()
            .unwrap_or(0);
        assert!(faded_max_alpha < full_max_alpha);
    }

    #[test]
    fn missing_font_is_reported_without_substitution() {
        let reference = FontReference {
            family: "Comic Sans MS".to_string(),
            weight: 400,
            style: Default::default(),
            asset_id: None,
        };
        let error = resolve_font(Some(&reference), &[]).expect_err("must fail");
        match error {
            TextOverlayError::MissingFont { family, .. } => {
                assert_eq!(family, "Comic Sans MS");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn missing_glyphs_are_diagnosed() {
        let renderer = renderer();
        let rich = base_style(TextFill::Solid {
            color: "#FFFFFF".to_string(),
        });
        let style = OverlayStyle::from_rich(&rich, None).expect("style");
        let diagnostics = renderer.diagnostics("漢字", &style);
        assert!(
            !diagnostics.missing_glyphs.is_empty(),
            "Roboto-Bold does not carry CJK glyphs; diagnostic must be explicit"
        );
        let error = renderer
            .render_overlay("漢字", &style, &OverlayPlacement::default())
            .expect_err("missing glyphs must not silently render .notdef");
        assert!(matches!(error, TextOverlayError::MissingGlyphs(_)));
    }

    #[test]
    fn cache_reuses_prepared_glyphs_and_can_be_cleared() {
        let renderer = renderer();
        let rich = base_style(TextFill::Solid {
            color: "#FFFFFF".to_string(),
        });
        let style = OverlayStyle::from_rich(&rich, None).expect("style");
        renderer
            .render_overlay("Cache", &style, &OverlayPlacement::default())
            .expect("render");
        let (entries, bytes) = renderer.cache_stats();
        assert!(entries > 0);
        assert!(bytes > 0);
        renderer.clear_cache();
        assert_eq!(renderer.cache_stats(), (0, 0));
    }

    #[test]
    fn placement_scale_and_rotation_keep_visible_pixels() {
        let renderer = renderer();
        let rich = base_style(TextFill::Solid {
            color: "#FFFFFF".to_string(),
        });
        let style = OverlayStyle::from_rich(&rich, None).expect("style");
        let placement = OverlayPlacement {
            scale_x: 1.4,
            scale_y: 1.4,
            rotation_deg: 17.0,
            offset_x: -120.0,
            offset_y: 60.0,
            ..OverlayPlacement::default()
        };
        let overlay = renderer
            .render_overlay("Rotate", &style, &placement)
            .expect("render");
        assert!(overlay.has_visible_content);
    }
}
