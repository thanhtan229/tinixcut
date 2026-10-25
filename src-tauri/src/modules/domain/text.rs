use crate::modules::domain::id::is_valid_id;
use crate::modules::domain::time::RationalTime;
use crate::modules::domain::{is_hex_color, DomainError, KeyframeEasing, TextAlignment};
use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

pub const MAX_TEXT_GRAPHEMES: usize = 512;
pub const MAX_TEXT_STROKES: usize = 4;
pub const MAX_TEXT_SHADOWS: usize = 4;
pub const MAX_GRADIENT_STOPS: usize = 8;

pub fn grapheme_count(text: &str) -> usize {
    text.graphemes(true).count()
}

fn default_line_height() -> f32 {
    1.0
}

fn default_unit_opacity() -> f32 {
    1.0
}

fn default_pivot() -> f32 {
    0.5
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
    Oblique,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FontReference {
    pub family: String,
    pub weight: u16,
    #[serde(default)]
    pub style: FontStyle,
    #[serde(default)]
    pub asset_id: Option<String>,
}

impl Default for FontReference {
    fn default() -> Self {
        Self {
            family: "Inter".to_string(),
            weight: 400,
            style: FontStyle::Normal,
            asset_id: None,
        }
    }
}

impl FontReference {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.family.trim().is_empty() {
            return Err(DomainError::Validation(
                "Font family cannot be empty".to_string(),
            ));
        }
        if !(100..=900).contains(&self.weight) {
            return Err(DomainError::Validation(
                "Font weight must be between 100 and 900".to_string(),
            ));
        }
        if let Some(ref asset_id) = self.asset_id {
            if !is_valid_id(asset_id) {
                return Err(DomainError::InvalidId(asset_id.clone()));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TextWrapMode {
    #[default]
    None,
    Wrap,
    AutoFit,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextLayout {
    #[serde(default)]
    pub max_width: Option<f32>,
    #[serde(default)]
    pub alignment: TextAlignment,
    #[serde(default = "default_line_height")]
    pub line_height: f32,
    #[serde(default)]
    pub letter_spacing: f32,
    #[serde(default)]
    pub wrap_mode: TextWrapMode,
    #[serde(default)]
    pub min_font_size: Option<f32>,
    #[serde(default)]
    pub max_font_size: Option<f32>,
}

impl Default for TextLayout {
    fn default() -> Self {
        Self {
            max_width: None,
            alignment: TextAlignment::Center,
            line_height: 1.0,
            letter_spacing: 0.0,
            wrap_mode: TextWrapMode::None,
            min_font_size: None,
            max_font_size: None,
        }
    }
}

impl TextLayout {
    pub fn validate(&self) -> Result<(), DomainError> {
        if let Some(mw) = self.max_width {
            if !mw.is_finite() || mw <= 0.0 {
                return Err(DomainError::Validation(
                    "Layout max_width must be finite and positive".to_string(),
                ));
            }
        }
        if !self.line_height.is_finite() || self.line_height <= 0.0 {
            return Err(DomainError::Validation(
                "Layout line_height must be finite and positive".to_string(),
            ));
        }
        if !self.letter_spacing.is_finite() {
            return Err(DomainError::Validation(
                "Layout letter_spacing must be finite".to_string(),
            ));
        }
        if let Some(min_fs) = self.min_font_size {
            if !min_fs.is_finite() || min_fs <= 0.0 {
                return Err(DomainError::Validation(
                    "Layout min_font_size must be finite and positive".to_string(),
                ));
            }
        }
        if let Some(max_fs) = self.max_font_size {
            if !max_fs.is_finite() || max_fs <= 0.0 {
                return Err(DomainError::Validation(
                    "Layout max_font_size must be finite and positive".to_string(),
                ));
            }
        }
        if let (Some(min_fs), Some(max_fs)) = (self.min_font_size, self.max_font_size) {
            if min_fs > max_fs {
                return Err(DomainError::Validation(
                    "Layout min_font_size cannot exceed max_font_size".to_string(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GradientKind {
    #[default]
    Linear,
    Radial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GradientSpace {
    #[default]
    All,
    Character,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GradientStop {
    pub offset: f32,
    pub color: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum TextFill {
    Solid {
        color: String,
    },
    Gradient {
        gradient_kind: GradientKind,
        angle_deg: f32,
        #[serde(default)]
        radius: Option<f32>,
        #[serde(default)]
        space: GradientSpace,
        stops: Vec<GradientStop>,
    },
}

impl Default for TextFill {
    fn default() -> Self {
        Self::Solid {
            color: "#FFFFFF".to_string(),
        }
    }
}

impl TextFill {
    pub fn validate(&self) -> Result<(), DomainError> {
        match self {
            TextFill::Solid { color } => {
                if !is_hex_color(color) {
                    return Err(DomainError::Validation(format!(
                        "Invalid solid fill color '{}': must be #RRGGBB or #RRGGBBAA",
                        color
                    )));
                }
            }
            TextFill::Gradient {
                angle_deg,
                radius,
                stops,
                ..
            } => {
                if !angle_deg.is_finite() {
                    return Err(DomainError::Validation(
                        "Gradient angle_deg must be finite".to_string(),
                    ));
                }
                if let Some(r) = radius {
                    if !r.is_finite() || *r <= 0.0 {
                        return Err(DomainError::Validation(
                            "Gradient radius must be finite and positive".to_string(),
                        ));
                    }
                }
                if stops.len() > MAX_GRADIENT_STOPS {
                    return Err(DomainError::Validation(format!(
                        "Gradient stops count ({}) exceeds maximum ({})",
                        stops.len(),
                        MAX_GRADIENT_STOPS
                    )));
                }
                let mut prev_offset = -1.0f32;
                for stop in stops {
                    if !stop.offset.is_finite() || !(0.0..=1.0).contains(&stop.offset) {
                        return Err(DomainError::Validation(
                            "Gradient stop offset must be between 0.0 and 1.0".to_string(),
                        ));
                    }
                    if stop.offset <= prev_offset {
                        return Err(DomainError::Validation(
                            "Gradient stop offsets must be strictly increasing".to_string(),
                        ));
                    }
                    prev_offset = stop.offset;
                    if !is_hex_color(&stop.color) {
                        return Err(DomainError::Validation(format!(
                            "Invalid gradient stop color '{}': must be #RRGGBB or #RRGGBBAA",
                            stop.color
                        )));
                    }
                }
                if let Some(last) = stops.last() {
                    if last.offset != 1.0 {
                        return Err(DomainError::Validation(
                            "Last gradient stop offset must be exactly 1.0".to_string(),
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum StrokeAlign {
    #[default]
    Outside,
    Center,
    Inside,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextStroke {
    pub color: String,
    pub width: f32,
    #[serde(default)]
    pub align: StrokeAlign,
}

impl TextStroke {
    pub fn validate(&self) -> Result<(), DomainError> {
        if !self.width.is_finite() || self.width <= 0.0 {
            return Err(DomainError::Validation(
                "Text stroke width must be finite and positive".to_string(),
            ));
        }
        if !is_hex_color(&self.color) {
            return Err(DomainError::Validation(format!(
                "Invalid text stroke color '{}': must be #RRGGBB or #RRGGBBAA",
                self.color
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextShadow {
    pub color: String,
    #[serde(default)]
    pub offset_x: f32,
    #[serde(default)]
    pub offset_y: f32,
    #[serde(default)]
    pub blur: f32,
}

impl TextShadow {
    pub fn validate(&self) -> Result<(), DomainError> {
        if !self.offset_x.is_finite() || !self.offset_y.is_finite() {
            return Err(DomainError::Validation(
                "Text shadow offset must be finite".to_string(),
            ));
        }
        if !self.blur.is_finite() || self.blur < 0.0 {
            return Err(DomainError::Validation(
                "Text shadow blur must be finite and non-negative".to_string(),
            ));
        }
        if !is_hex_color(&self.color) {
            return Err(DomainError::Validation(format!(
                "Invalid text shadow color '{}': must be #RRGGBB or #RRGGBBAA",
                self.color
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextBackgroundBox {
    #[serde(default)]
    pub fill: TextFill,
    #[serde(default)]
    pub padding_x: f32,
    #[serde(default)]
    pub padding_y: f32,
    #[serde(default)]
    pub corner_radius: f32,
    #[serde(default = "default_unit_opacity")]
    pub opacity: f32,
}

impl Default for TextBackgroundBox {
    fn default() -> Self {
        Self {
            fill: TextFill::default(),
            padding_x: 0.0,
            padding_y: 0.0,
            corner_radius: 0.0,
            opacity: 1.0,
        }
    }
}

impl TextBackgroundBox {
    pub fn validate(&self) -> Result<(), DomainError> {
        self.fill.validate()?;
        if !self.padding_x.is_finite() || self.padding_x < 0.0 {
            return Err(DomainError::Validation(
                "Background padding_x must be finite and non-negative".to_string(),
            ));
        }
        if !self.padding_y.is_finite() || self.padding_y < 0.0 {
            return Err(DomainError::Validation(
                "Background padding_y must be finite and non-negative".to_string(),
            ));
        }
        if !self.corner_radius.is_finite() || self.corner_radius < 0.0 {
            return Err(DomainError::Validation(
                "Background corner_radius must be finite and non-negative".to_string(),
            ));
        }
        if !self.opacity.is_finite() || !(0.0..=1.0).contains(&self.opacity) {
            return Err(DomainError::Validation(
                "Background opacity must be between 0.0 and 1.0".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TextSpanStyle {
    #[serde(default)]
    pub fill: Option<TextFill>,
    #[serde(default)]
    pub font: Option<FontReference>,
    #[serde(default)]
    pub font_size: Option<f32>,
    #[serde(default)]
    pub letter_spacing: Option<f32>,
}

impl TextSpanStyle {
    pub fn validate(&self) -> Result<(), DomainError> {
        if let Some(ref fill) = self.fill {
            fill.validate()?;
        }
        if let Some(ref font) = self.font {
            font.validate()?;
        }
        if let Some(fs) = self.font_size {
            if !fs.is_finite() || fs <= 0.0 {
                return Err(DomainError::Validation(
                    "Span font_size must be finite and positive".to_string(),
                ));
            }
        }
        if let Some(ls) = self.letter_spacing {
            if !ls.is_finite() {
                return Err(DomainError::Validation(
                    "Span letter_spacing must be finite".to_string(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RichTextSpan {
    pub start: u32,
    pub end: u32,
    #[serde(default)]
    pub style: TextSpanStyle,
}

impl RichTextSpan {
    pub fn validate(&self, text_grapheme_len: usize) -> Result<(), DomainError> {
        if self.start >= self.end {
            return Err(DomainError::InvalidTextSpan {
                start: self.start,
                end: self.end,
                reason: "start must be strictly less than end".to_string(),
            });
        }
        if self.end as usize > text_grapheme_len {
            return Err(DomainError::InvalidTextSpan {
                start: self.start,
                end: self.end,
                reason: format!(
                    "end ({}) exceeds text grapheme length ({})",
                    self.end, text_grapheme_len
                ),
            });
        }
        self.style.validate()?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TextRichStyle {
    #[serde(default)]
    pub font: Option<FontReference>,
    #[serde(default)]
    pub font_size: Option<f32>,
    #[serde(default)]
    pub layout: TextLayout,
    #[serde(default)]
    pub fill: TextFill,
    #[serde(default)]
    pub strokes: Vec<TextStroke>,
    #[serde(default)]
    pub shadows: Vec<TextShadow>,
    #[serde(default)]
    pub background: Option<TextBackgroundBox>,
    #[serde(default)]
    pub spans: Vec<RichTextSpan>,
}

impl TextRichStyle {
    pub fn validate(&self, text: &str) -> Result<(), DomainError> {
        let text_graphemes = grapheme_count(text);
        if text_graphemes > MAX_TEXT_GRAPHEMES {
            return Err(DomainError::TextTooLong {
                max: MAX_TEXT_GRAPHEMES,
                found: text_graphemes,
            });
        }
        if let Some(ref font) = self.font {
            font.validate()?;
        }
        if let Some(fs) = self.font_size {
            if !fs.is_finite() || fs <= 0.0 {
                return Err(DomainError::Validation(
                    "Rich text font_size must be finite and positive".to_string(),
                ));
            }
        }
        self.layout.validate()?;
        self.fill.validate()?;
        if self.strokes.len() > MAX_TEXT_STROKES {
            return Err(DomainError::Validation(format!(
                "Text strokes count ({}) exceeds maximum ({})",
                self.strokes.len(),
                MAX_TEXT_STROKES
            )));
        }
        for stroke in &self.strokes {
            stroke.validate()?;
        }
        if self.shadows.len() > MAX_TEXT_SHADOWS {
            return Err(DomainError::Validation(format!(
                "Text shadows count ({}) exceeds maximum ({})",
                self.shadows.len(),
                MAX_TEXT_SHADOWS
            )));
        }
        for shadow in &self.shadows {
            shadow.validate()?;
        }
        if let Some(ref bg) = self.background {
            bg.validate()?;
        }

        let mut prev_end: u32 = 0;
        for (i, span) in self.spans.iter().enumerate() {
            span.validate(text_graphemes)?;
            if i > 0 {
                let prev_span = &self.spans[i - 1];
                if span.start < prev_span.start {
                    return Err(DomainError::InvalidTextSpan {
                        start: span.start,
                        end: span.end,
                        reason: "spans must be sorted by start index".to_string(),
                    });
                }
                if span.start < prev_end {
                    return Err(DomainError::InvalidTextSpan {
                        start: span.start,
                        end: span.end,
                        reason: format!(
                            "span [{start}, {end}) overlaps with previous span ending at {prev_end}",
                            start = span.start,
                            end = span.end,
                            prev_end = prev_end
                        ),
                    });
                }
            }
            prev_end = span.end;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AnimationUnit {
    #[default]
    All,
    Grapheme,
    Word,
    Line,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum StaggerOrder {
    #[default]
    Forward,
    Backward,
    CenterOut,
    Random,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LoopMode {
    #[default]
    None,
    Repeat,
    PingPong,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DurationConflictPolicy {
    #[default]
    Reject,
    ProportionalScale,
    ClampOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AnimationTransform {
    pub translate_x: f32,
    pub translate_y: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub rotation: f32,
    pub opacity: f32,
}

impl Default for AnimationTransform {
    fn default() -> Self {
        Self {
            translate_x: 0.0,
            translate_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
            opacity: 1.0,
        }
    }
}

impl AnimationTransform {
    pub fn validate(&self) -> Result<(), DomainError> {
        if !self.translate_x.is_finite()
            || !self.translate_y.is_finite()
            || !self.scale_x.is_finite()
            || !self.scale_y.is_finite()
            || !self.rotation.is_finite()
        {
            return Err(DomainError::Validation(
                "Animation transform values must be finite".to_string(),
            ));
        }
        if !self.opacity.is_finite() || !(0.0..=1.0).contains(&self.opacity) {
            return Err(DomainError::Validation(
                "Animation transform opacity must be between 0.0 and 1.0".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextAnimationSegment {
    pub duration: RationalTime,
    #[serde(default)]
    pub easing: KeyframeEasing,
    #[serde(default)]
    pub stagger_ratio: f32,
    #[serde(default)]
    pub from: AnimationTransform,
    #[serde(default)]
    pub to: AnimationTransform,
}

impl TextAnimationSegment {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.duration.value <= 0 {
            return Err(DomainError::NonPositiveDuration);
        }
        self.easing.validate()?;
        if !self.stagger_ratio.is_finite() || !(0.0..=1.0).contains(&self.stagger_ratio) {
            return Err(DomainError::Validation(
                "Animation stagger_ratio must be between 0.0 and 1.0".to_string(),
            ));
        }
        self.from.validate()?;
        self.to.validate()?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextAnimation {
    #[serde(default)]
    pub unit: AnimationUnit,
    #[serde(default)]
    pub order: StaggerOrder,
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default = "default_pivot")]
    pub pivot_x: f32,
    #[serde(default = "default_pivot")]
    pub pivot_y: f32,
    #[serde(default)]
    pub in_anim: Option<TextAnimationSegment>,
    #[serde(default)]
    pub loop_anim: Option<TextAnimationSegment>,
    #[serde(default)]
    pub out_anim: Option<TextAnimationSegment>,
    #[serde(default)]
    pub loop_mode: LoopMode,
    #[serde(default)]
    pub conflict_policy: DurationConflictPolicy,
}

impl Default for TextAnimation {
    fn default() -> Self {
        Self {
            unit: AnimationUnit::All,
            order: StaggerOrder::Forward,
            seed: None,
            pivot_x: 0.5,
            pivot_y: 0.5,
            in_anim: None,
            loop_anim: None,
            out_anim: None,
            loop_mode: LoopMode::None,
            conflict_policy: DurationConflictPolicy::Reject,
        }
    }
}

impl TextAnimation {
    pub fn validate_for_clip_duration(
        &self,
        clip_duration: RationalTime,
    ) -> Result<(), DomainError> {
        if clip_duration.value <= 0 {
            return Err(DomainError::NonPositiveDuration);
        }
        if !self.pivot_x.is_finite() || !(0.0..=1.0).contains(&self.pivot_x) {
            return Err(DomainError::Validation(
                "Animation pivot_x must be between 0.0 and 1.0".to_string(),
            ));
        }
        if !self.pivot_y.is_finite() || !(0.0..=1.0).contains(&self.pivot_y) {
            return Err(DomainError::Validation(
                "Animation pivot_y must be between 0.0 and 1.0".to_string(),
            ));
        }
        if self.order == StaggerOrder::Random && self.seed.is_none() {
            return Err(DomainError::Validation(
                "StaggerOrder::Random requires a seed".to_string(),
            ));
        }

        let mut in_scaled = None;
        if let Some(ref in_anim) = self.in_anim {
            in_anim.validate()?;
            let scaled = in_anim.duration.rescaled_to(clip_duration.rate);
            if scaled.value <= 0 {
                return Err(DomainError::NonPositiveDuration);
            }
            in_scaled = Some(scaled);
        }

        let mut out_scaled = None;
        if let Some(ref out_anim) = self.out_anim {
            out_anim.validate()?;
            let scaled = out_anim.duration.rescaled_to(clip_duration.rate);
            if scaled.value <= 0 {
                return Err(DomainError::NonPositiveDuration);
            }
            out_scaled = Some(scaled);
        }

        if let Some(ref loop_anim) = self.loop_anim {
            loop_anim.validate()?;
            let scaled = loop_anim.duration.rescaled_to(clip_duration.rate);
            if scaled.value <= 0 {
                return Err(DomainError::NonPositiveDuration);
            }
        }

        let in_frames = in_scaled.as_ref().map(|t| t.value).unwrap_or(0);
        let out_frames = out_scaled.as_ref().map(|t| t.value).unwrap_or(0);

        if in_frames + out_frames > clip_duration.value {
            match self.conflict_policy {
                DurationConflictPolicy::Reject => {
                    let in_str = in_scaled
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| "0 frames".to_string());
                    let out_str = out_scaled
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| "0 frames".to_string());
                    return Err(DomainError::AnimationDurationConflict {
                        in_duration: in_str,
                        out_duration: out_str,
                        clip_duration: clip_duration.to_string(),
                    });
                }
                DurationConflictPolicy::ProportionalScale | DurationConflictPolicy::ClampOut => {}
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::domain::time::FrameRate;

    #[test]
    fn test_grapheme_count_vietnamese_nfc_and_nfd() {
        use unicode_normalization::UnicodeNormalization;
        let original = "Tiếng Việt — Xin chào thế giới!";
        let nfc: String = original.nfc().collect();
        let nfd: String = original.nfd().collect();

        assert_ne!(nfc.len(), nfd.len(), "Byte lengths differ for NFC and NFD");
        assert_ne!(
            nfc.chars().count(),
            nfd.chars().count(),
            "Char counts differ for NFC and NFD"
        );
        assert_eq!(
            grapheme_count(&nfc),
            grapheme_count(&nfd),
            "Grapheme counts must match for NFC and NFD"
        );
        assert_eq!(grapheme_count(&nfc), 31);
    }

    #[test]
    fn test_grapheme_count_zwj_emoji() {
        let family_emoji = "👨‍👩‍👧‍👦";
        assert_eq!(
            family_emoji.chars().count(),
            7,
            "ZWJ family is 7 code points"
        );
        assert_eq!(
            grapheme_count(family_emoji),
            1,
            "ZWJ family emoji is exactly 1 grapheme cluster"
        );

        let waving_hand_toned = "👋🏽";
        assert_eq!(
            grapheme_count(waving_hand_toned),
            1,
            "Emoji with skin tone modifier is 1 grapheme"
        );
    }

    #[test]
    fn test_text_length_limit() {
        let text_512 = "a".repeat(512);
        let rich = TextRichStyle::default();
        assert!(rich.validate(&text_512).is_ok());

        let text_513 = "a".repeat(513);
        let err = rich.validate(&text_513).unwrap_err();
        match err {
            DomainError::TextTooLong { max, found } => {
                assert_eq!(max, 512);
                assert_eq!(found, 513);
            }
            other => panic!("Expected TextTooLong, got {:?}", other),
        }
    }

    #[test]
    fn test_rich_text_span_validation() {
        let text = "Hello, world!";
        let count = grapheme_count(text) as u32;

        let valid_style = TextRichStyle {
            spans: vec![
                RichTextSpan {
                    start: 0,
                    end: 5,
                    style: TextSpanStyle::default(),
                },
                RichTextSpan {
                    start: 5,
                    end: count,
                    style: TextSpanStyle::default(),
                },
            ],
            ..Default::default()
        };
        assert!(valid_style.validate(text).is_ok());

        // start >= end
        let inverted_span = TextRichStyle {
            spans: vec![RichTextSpan {
                start: 5,
                end: 5,
                style: TextSpanStyle::default(),
            }],
            ..Default::default()
        };
        assert!(matches!(
            inverted_span.validate(text).unwrap_err(),
            DomainError::InvalidTextSpan { .. }
        ));

        // end > graphemes
        let oob_span = TextRichStyle {
            spans: vec![RichTextSpan {
                start: 0,
                end: count + 1,
                style: TextSpanStyle::default(),
            }],
            ..Default::default()
        };
        assert!(matches!(
            oob_span.validate(text).unwrap_err(),
            DomainError::InvalidTextSpan { .. }
        ));

        // overlapping
        let overlap = TextRichStyle {
            spans: vec![
                RichTextSpan {
                    start: 0,
                    end: 6,
                    style: TextSpanStyle::default(),
                },
                RichTextSpan {
                    start: 4,
                    end: 10,
                    style: TextSpanStyle::default(),
                },
            ],
            ..Default::default()
        };
        assert!(matches!(
            overlap.validate(text).unwrap_err(),
            DomainError::InvalidTextSpan { .. }
        ));

        // unsorted
        let unsorted = TextRichStyle {
            spans: vec![
                RichTextSpan {
                    start: 6,
                    end: 10,
                    style: TextSpanStyle::default(),
                },
                RichTextSpan {
                    start: 0,
                    end: 5,
                    style: TextSpanStyle::default(),
                },
            ],
            ..Default::default()
        };
        assert!(matches!(
            unsorted.validate(text).unwrap_err(),
            DomainError::InvalidTextSpan { .. }
        ));
    }

    #[test]
    fn test_strokes_and_shadows_limits() {
        let text = "test";
        let mut rich = TextRichStyle::default();

        for _ in 0..4 {
            rich.strokes.push(TextStroke {
                color: "#FF0000".to_string(),
                width: 2.0,
                align: StrokeAlign::Outside,
            });
            rich.shadows.push(TextShadow {
                color: "#00000088".to_string(),
                offset_x: 1.0,
                offset_y: 1.0,
                blur: 2.0,
            });
        }
        assert!(rich.validate(text).is_ok());

        // 5th stroke exceeds limit
        rich.strokes.push(TextStroke {
            color: "#FF0000".to_string(),
            width: 1.0,
            align: StrokeAlign::Center,
        });
        assert!(matches!(
            rich.validate(text).unwrap_err(),
            DomainError::Validation(_)
        ));

        rich.strokes.pop();
        // 5th shadow exceeds limit
        rich.shadows.push(TextShadow {
            color: "#000000".to_string(),
            offset_x: 0.0,
            offset_y: 0.0,
            blur: 0.0,
        });
        assert!(matches!(
            rich.validate(text).unwrap_err(),
            DomainError::Validation(_)
        ));
    }

    #[test]
    fn test_gradient_validation() {
        let text = "gradient test";
        let valid_gradient = TextFill::Gradient {
            gradient_kind: GradientKind::Linear,
            angle_deg: 90.0,
            radius: None,
            space: GradientSpace::All,
            stops: vec![
                GradientStop {
                    offset: 0.0,
                    color: "#FF0000".to_string(),
                },
                GradientStop {
                    offset: 0.5,
                    color: "#00FF00".to_string(),
                },
                GradientStop {
                    offset: 1.0,
                    color: "#0000FF".to_string(),
                },
            ],
        };
        let rich = TextRichStyle {
            fill: valid_gradient,
            ..Default::default()
        };
        assert!(rich.validate(text).is_ok());

        // Last stop not 1.0
        let invalid_last = TextFill::Gradient {
            gradient_kind: GradientKind::Linear,
            angle_deg: 0.0,
            radius: None,
            space: GradientSpace::All,
            stops: vec![
                GradientStop {
                    offset: 0.0,
                    color: "#FF0000".to_string(),
                },
                GradientStop {
                    offset: 0.8,
                    color: "#0000FF".to_string(),
                },
            ],
        };
        let rich_invalid = TextRichStyle {
            fill: invalid_last,
            ..Default::default()
        };
        assert!(matches!(
            rich_invalid.validate(text).unwrap_err(),
            DomainError::Validation(_)
        ));

        // Non-increasing offsets
        let non_increasing = TextFill::Gradient {
            gradient_kind: GradientKind::Linear,
            angle_deg: 0.0,
            radius: None,
            space: GradientSpace::All,
            stops: vec![
                GradientStop {
                    offset: 0.5,
                    color: "#FF0000".to_string(),
                },
                GradientStop {
                    offset: 0.5,
                    color: "#00FF00".to_string(),
                },
                GradientStop {
                    offset: 1.0,
                    color: "#0000FF".to_string(),
                },
            ],
        };
        let rich_non_inc = TextRichStyle {
            fill: non_increasing,
            ..Default::default()
        };
        assert!(matches!(
            rich_non_inc.validate(text).unwrap_err(),
            DomainError::Validation(_)
        ));
    }

    #[test]
    fn test_color_validation() {
        let text = "color test";
        let invalid_color = TextFill::Solid {
            color: "red".to_string(),
        };
        let rich = TextRichStyle {
            fill: invalid_color,
            ..Default::default()
        };
        assert!(matches!(
            rich.validate(text).unwrap_err(),
            DomainError::Validation(_)
        ));
    }

    #[test]
    fn test_random_stagger_requires_seed() {
        let rate = FrameRate::new(30, 1);
        let clip_dur = RationalTime::from_frames(60, rate);

        let anim_without_seed = TextAnimation {
            order: StaggerOrder::Random,
            seed: None,
            ..Default::default()
        };
        assert!(matches!(
            anim_without_seed
                .validate_for_clip_duration(clip_dur)
                .unwrap_err(),
            DomainError::Validation(_)
        ));

        let anim_with_seed = TextAnimation {
            order: StaggerOrder::Random,
            seed: Some(42),
            ..Default::default()
        };
        assert!(anim_with_seed.validate_for_clip_duration(clip_dur).is_ok());
    }

    #[test]
    fn test_duration_conflict_policies() {
        let rate = FrameRate::new(30, 1);
        let clip_dur = RationalTime::from_frames(30, rate); // 1 second

        let in_seg = TextAnimationSegment {
            duration: RationalTime::from_frames(20, rate),
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.0,
            from: AnimationTransform::default(),
            to: AnimationTransform::default(),
        };
        let out_seg = TextAnimationSegment {
            duration: RationalTime::from_frames(20, rate), // total 40 > 30
            easing: KeyframeEasing::Linear,
            stagger_ratio: 0.0,
            from: AnimationTransform::default(),
            to: AnimationTransform::default(),
        };

        // Reject -> Error
        let reject_anim = TextAnimation {
            in_anim: Some(in_seg.clone()),
            out_anim: Some(out_seg.clone()),
            conflict_policy: DurationConflictPolicy::Reject,
            ..Default::default()
        };
        assert!(matches!(
            reject_anim
                .validate_for_clip_duration(clip_dur)
                .unwrap_err(),
            DomainError::AnimationDurationConflict { .. }
        ));

        // ProportionalScale -> Accepted
        let scale_anim = TextAnimation {
            in_anim: Some(in_seg.clone()),
            out_anim: Some(out_seg.clone()),
            conflict_policy: DurationConflictPolicy::ProportionalScale,
            ..Default::default()
        };
        assert!(scale_anim.validate_for_clip_duration(clip_dur).is_ok());

        // ClampOut -> Accepted
        let clamp_anim = TextAnimation {
            in_anim: Some(in_seg),
            out_anim: Some(out_seg),
            conflict_policy: DurationConflictPolicy::ClampOut,
            ..Default::default()
        };
        assert!(clamp_anim.validate_for_clip_duration(clip_dur).is_ok());
    }
}
