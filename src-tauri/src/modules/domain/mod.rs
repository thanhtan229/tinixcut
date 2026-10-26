//! Project Domain Model v1
//! The canonical, application-owned source of truth for video editing.
//! Invariant: No renderer-specific state, no arbitrary float seconds, stable IDs only.

pub mod evaluator;
pub mod id;
pub mod template;
pub mod text;
pub mod text_animation;
pub mod time;

pub use template::*;
pub use text::*;
pub use text_animation::*;

use self::id::*;
use self::time::{FrameRate, RationalTime, TimeRange};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const CURRENT_SCHEMA_VERSION: u32 = 11;

#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DomainError {
    #[error("Schema version mismatch: expected {expected}, found {found}")]
    SchemaVersionMismatch { expected: u32, found: u32 },

    #[error("Invalid ID '{0}': ID cannot be empty or invalid")]
    InvalidId(String),

    #[error("Item duration must be positive")]
    NonPositiveDuration,

    #[error(
        "Source range duration ({source_dur}) must match timeline range duration ({timeline_dur})"
    )]
    DurationMismatch {
        source_dur: String,
        timeline_dur: String,
    },

    #[error("Asset with ID '{0}' not found in project")]
    AssetNotFound(String),

    #[error("Track with ID '{0}' not found in project")]
    TrackNotFound(String),

    #[error("Item with ID '{0}' not found in project")]
    ItemNotFound(String),

    #[error("Track '{track_id}' has overlapping items: '{item1_id}' and '{item2_id}'")]
    OverlappingItems {
        track_id: String,
        item1_id: String,
        item2_id: String,
    },

    #[error("Transition with ID '{0}' not found on track")]
    TransitionNotFound(String),

    #[error("Audio crossfade participants must be standalone audio items on an audio track")]
    UnsupportedAudioCrossfadeMedia,

    #[error("Audio crossfade participants '{left_id}' and '{right_id}' are not adjacent on track '{track_id}'")]
    NonAdjacentAudioCrossfade {
        track_id: String,
        left_id: String,
        right_id: String,
    },

    #[error("Cut transition participants must be video items (found non-video item)")]
    UnsupportedTransitionMedia,

    #[error("Transition required handle exceeds available frames: postroll {available_postroll} < {required_postroll} or preroll {available_preroll} < {required_preroll}")]
    InsufficientTransitionHandle {
        required_preroll: RationalTime,
        available_preroll: RationalTime,
        required_postroll: RationalTime,
        available_postroll: RationalTime,
    },

    #[error("Transition handle availability cannot be verified: media duration or stream unprobed/missing")]
    TransitionHandleUnverifiable { reason: String },

    #[error("Cut transition participants '{left_id}' and '{right_id}' are not immediately adjacent on track '{track_id}'")]
    NonAdjacentTransitionParticipants {
        track_id: String,
        left_id: String,
        right_id: String,
    },

    #[error("A cut transition already exists at cut between '{left_id}' and '{right_id}'")]
    DuplicateCutTransition { left_id: String, right_id: String },

    #[error("Domain validation error: {0}")]
    Validation(String),

    #[error("Text exceeds maximum allowed graphemes: max {max}, found {found}")]
    TextTooLong { max: usize, found: usize },

    #[error("Invalid text span [{start}, {end}): {reason}")]
    InvalidTextSpan {
        start: u32,
        end: u32,
        reason: String,
    },

    #[error("Text animation duration conflict: in {in_duration} + out {out_duration} exceeds clip duration {clip_duration}")]
    AnimationDurationConflict {
        in_duration: String,
        out_duration: String,
        clip_duration: String,
    },

    #[error("Unsupported text feature: {0}")]
    UnsupportedTextFeature(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaKind {
    Video,
    Audio,
    Image,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackKind {
    Video,
    Audio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemKind {
    Video,
    Audio,
    Image,
    Text,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum TextAlignment {
    Left,
    #[default]
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TextAnchor {
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    Center,
    MiddleRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TextStyle {
    pub font_family: String,
    pub font_size: f32,
    pub font_weight: u16,
    pub font_color: String,
    pub accent_color: Option<String>,
    pub background_color: Option<String>,
    pub background_opacity: f32,
    pub padding_x: f32,
    pub padding_y: f32,
    pub alignment: TextAlignment,
    pub line_height: f32,
    pub anchor: TextAnchor,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_family: "Inter".to_string(),
            font_size: 48.0,
            font_weight: 700,
            font_color: "#FFFFFF".to_string(),
            accent_color: None,
            background_color: None,
            background_opacity: 0.0,
            padding_x: 0.0,
            padding_y: 0.0,
            alignment: TextAlignment::Center,
            line_height: 1.0,
            anchor: TextAnchor::Center,
        }
    }
}

impl TextStyle {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.font_family.trim().is_empty() {
            return Err(DomainError::Validation(
                "Text font family cannot be empty".to_string(),
            ));
        }
        if !self.font_size.is_finite() || self.font_size <= 0.0 {
            return Err(DomainError::Validation(
                "Text font size must be finite and positive".to_string(),
            ));
        }
        if !(100..=900).contains(&self.font_weight) {
            return Err(DomainError::Validation(
                "Text font weight must be between 100 and 900".to_string(),
            ));
        }
        if !self.background_opacity.is_finite() || !(0.0..=1.0).contains(&self.background_opacity) {
            return Err(DomainError::Validation(
                "Text background opacity must be between 0 and 1".to_string(),
            ));
        }
        if !self.padding_x.is_finite()
            || !self.padding_y.is_finite()
            || self.padding_x < 0.0
            || self.padding_y < 0.0
        {
            return Err(DomainError::Validation(
                "Text padding must be finite and non-negative".to_string(),
            ));
        }
        if !self.line_height.is_finite() || self.line_height <= 0.0 {
            return Err(DomainError::Validation(
                "Text line height must be finite and positive".to_string(),
            ));
        }
        for (name, color) in [
            ("font_color", Some(self.font_color.as_str())),
            ("accent_color", self.accent_color.as_deref()),
            ("background_color", self.background_color.as_deref()),
        ] {
            if let Some(color) = color {
                if !is_hex_color(color) {
                    return Err(DomainError::Validation(format!(
                        "Text {name} must be a #RRGGBB or #RRGGBBAA color"
                    )));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TextTemplateReference {
    pub template_id: String,
    pub template_version: u32,
}

impl TextTemplateReference {
    pub fn validate(&self) -> Result<(), DomainError> {
        if !is_valid_id(&self.template_id) {
            return Err(DomainError::InvalidId(self.template_id.clone()));
        }
        if self.template_version == 0 {
            return Err(DomainError::Validation(
                "Text template version must be positive".to_string(),
            ));
        }
        Ok(())
    }
}

pub(crate) fn is_hex_color(value: &str) -> bool {
    let bytes = value.as_bytes();
    (bytes.len() == 7 || bytes.len() == 9)
        && bytes[0] == b'#'
        && bytes[1..].iter().all(|byte| byte.is_ascii_hexdigit())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Transform {
    pub position_x: f32, // Pixel offset or normalized (-1920 to +1920)
    pub position_y: f32,
    pub scale_x: f32, // 1.0 is default
    pub scale_y: f32,
    pub rotation: f32, // Degrees (-360 to +360)
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position_x: 0.0,
            position_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Crop {
    pub top: f32, // 0.0 to 1.0
    pub bottom: f32,
    pub left: f32,
    pub right: f32,
}

impl Default for Crop {
    fn default() -> Self {
        Self {
            top: 0.0,
            bottom: 0.0,
            left: 0.0,
            right: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectSettings {
    pub width: u32,
    pub height: u32,
    #[serde(default = "default_canvas_background_color")]
    pub canvas_background_color: String,
    pub frame_rate: FrameRate,
    pub sample_rate: u32,
    pub channels: u16,
}

fn default_canvas_background_color() -> String {
    "#000000".to_string()
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            canvas_background_color: default_canvas_background_color(),
            frame_rate: FrameRate::FPS_30,
            sample_rate: 48000,
            channels: 2,
        }
    }
}

impl ProjectSettings {
    /// Validate settings that define the canonical composition canvas.
    ///
    /// The supported H.264 export path produces yuv420p video, which requires
    /// even dimensions. Keeping that invariant in the project model prevents a
    /// project from previewing successfully but failing only when exported.
    pub fn validate(&self) -> Result<(), DomainError> {
        const MAX_CANVAS_DIMENSION: u32 = 8192;

        if self.width == 0 || self.height == 0 {
            return Err(DomainError::Validation(
                "Project canvas dimensions must be positive integers".to_string(),
            ));
        }
        if self.width > MAX_CANVAS_DIMENSION || self.height > MAX_CANVAS_DIMENSION {
            return Err(DomainError::Validation(format!(
                "Project canvas dimensions cannot exceed {MAX_CANVAS_DIMENSION}px"
            )));
        }
        if !self.width.is_multiple_of(2) || !self.height.is_multiple_of(2) {
            return Err(DomainError::Validation(
                "Project canvas dimensions must be even for H.264 export".to_string(),
            ));
        }
        let color = self.canvas_background_color.as_bytes();
        if color.len() != 7 || color[0] != b'#' || !color[1..].iter().all(u8::is_ascii_hexdigit) {
            return Err(DomainError::Validation(
                "Project canvas background must be a #RRGGBB color".to_string(),
            ));
        }
        if !matches!(self.sample_rate, 44_100 | 48_000) {
            return Err(DomainError::Validation(
                "Project audio sample rate must be 44100 or 48000 Hz".to_string(),
            ));
        }
        if self.channels == 0 {
            return Err(DomainError::Validation(
                "Project audio channel count must be positive".to_string(),
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssetDimensions {
    pub width: u32,
    pub height: u32,
}

impl AssetDimensions {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.width == 0 || self.height == 0 {
            return Err(DomainError::Validation(
                "Asset dimensions must be positive".to_string(),
            ));
        }
        if self.width > 32_768 || self.height > 32_768 {
            return Err(DomainError::Validation(
                "Asset dimensions must not exceed 32768 pixels".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum PlacementMode {
    #[default]
    Fit,
    Fill,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Asset {
    pub asset_id: String,
    pub name: String,
    pub path: String,
    pub media_kind: MediaKind,
    pub duration: Option<RationalTime>,
    pub file_size: u64,
    #[serde(default)]
    pub dimensions: Option<AssetDimensions>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyframeProperty {
    Opacity,
    PositionX,
    PositionY,
    ScaleX,
    ScaleY,
    Rotation,
    Volume,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum KeyframeEasing {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    CubicBezier {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    },
}

impl KeyframeEasing {
    pub fn validate(&self) -> Result<(), DomainError> {
        match *self {
            KeyframeEasing::Linear
            | KeyframeEasing::EaseIn
            | KeyframeEasing::EaseOut
            | KeyframeEasing::EaseInOut => Ok(()),
            KeyframeEasing::CubicBezier { x1, y1, x2, y2 } => {
                if !x1.is_finite() || !y1.is_finite() || !x2.is_finite() || !y2.is_finite() {
                    return Err(DomainError::Validation(
                        "CubicBezier control points must be finite numbers".to_string(),
                    ));
                }
                if !(0.0..=1.0).contains(&x1)
                    || !(0.0..=1.0).contains(&y1)
                    || !(0.0..=1.0).contains(&x2)
                    || !(0.0..=1.0).contains(&y2)
                {
                    return Err(DomainError::Validation(
                        "CubicBezier control points must be in range [0.0, 1.0]".to_string(),
                    ));
                }
                Ok(())
            }
        }
    }

    pub fn is_linear(&self) -> bool {
        matches!(self, KeyframeEasing::Linear)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Keyframe {
    pub property: KeyframeProperty,
    pub offset: RationalTime,
    pub value: f32,
    #[serde(default)]
    pub outgoing_easing: KeyframeEasing,
}

impl Keyframe {
    pub const fn new(property: KeyframeProperty, offset: RationalTime, value: f32) -> Self {
        Self {
            property,
            offset,
            value,
            outgoing_easing: KeyframeEasing::Linear,
        }
    }

    pub const fn with_easing(
        property: KeyframeProperty,
        offset: RationalTime,
        value: f32,
        outgoing_easing: KeyframeEasing,
    ) -> Self {
        Self {
            property,
            offset,
            value,
            outgoing_easing,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VisualEffectPreset {
    #[default]
    None,
    Mono,
    Vivid,
    Muted,
}

/// Fixed clip speeds supported by the first non-destructive speed-control
/// release. The source duration is always derived from timeline duration.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClipSpeed {
    Half,
    #[default]
    Normal,
    Double,
}

impl ClipSpeed {
    pub fn as_f64(self) -> f64 {
        match self {
            Self::Half => 0.5,
            Self::Normal => 1.0,
            Self::Double => 2.0,
        }
    }

    fn ratio(self) -> (i64, i64) {
        match self {
            Self::Half => (1, 2),
            Self::Normal => (1, 1),
            Self::Double => (2, 1),
        }
    }

    /// Converts a timeline duration to an exactly representable source duration.
    pub fn source_duration_for(
        self,
        timeline_duration: RationalTime,
        source_rate: FrameRate,
    ) -> Result<RationalTime, DomainError> {
        if self == Self::Normal {
            return Ok(timeline_duration.rescaled_to(source_rate));
        }
        let (speed_num, speed_den) = self.ratio();
        let numerator = (timeline_duration.value as i128)
            .checked_mul(timeline_duration.rate.denominator as i128)
            .and_then(|value| value.checked_mul(speed_num as i128))
            .and_then(|value| value.checked_mul(source_rate.numerator as i128))
            .ok_or_else(|| DomainError::Validation("Speed duration overflow".to_string()))?;
        let denominator = (timeline_duration.rate.numerator as i128)
            .checked_mul(speed_den as i128)
            .and_then(|value| value.checked_mul(source_rate.denominator as i128))
            .ok_or_else(|| DomainError::Validation("Speed duration overflow".to_string()))?;
        if denominator == 0 || numerator % denominator != 0 {
            return Err(DomainError::Validation(format!(
                "Speed {:?} cannot represent timeline duration {} exactly at source rate {}",
                self, timeline_duration, source_rate
            )));
        }
        let frames = numerator / denominator;
        if !(0..=i64::MAX as i128).contains(&frames) {
            return Err(DomainError::Validation(
                "Speed duration is out of range".to_string(),
            ));
        }
        Ok(RationalTime::new(frames as i64, source_rate))
    }

    pub fn source_offset_for(self, timeline_offset: RationalTime) -> RationalTime {
        let (speed_num, speed_den) = self.ratio();
        let numerator = timeline_offset.value as i128
            * timeline_offset.rate.denominator as i128
            * speed_num as i128;
        let denominator = timeline_offset.rate.numerator as i128 * speed_den as i128;
        let divisor = gcd_i128(numerator, denominator);
        let numerator = numerator / divisor;
        let denominator = denominator / divisor;
        assert!(
            numerator >= i64::MIN as i128
                && numerator <= i64::MAX as i128
                && denominator > 0
                && denominator <= u32::MAX as i128,
            "speed source offset is out of range"
        );
        RationalTime::new(numerator as i64, FrameRate::new(denominator as u32, 1))
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransitionKind {
    FadeIn,
    FadeOut,
    CrossDissolve,
    FadeBlack,
    DipToBlack,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transition {
    pub kind: TransitionKind,
    pub duration: RationalTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CutTransition {
    pub transition_id: String,
    pub track_id: String,
    pub left_item_id: String,
    pub right_item_id: String,
    pub kind: TransitionKind,
    pub duration: RationalTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioCrossfade {
    pub transition_id: String,
    pub track_id: String,
    pub left_item_id: String,
    pub right_item_id: String,
    pub duration: RationalTime,
}

impl AudioCrossfade {
    pub fn validate_basic(&self) -> Result<(), DomainError> {
        for id in [
            &self.transition_id,
            &self.track_id,
            &self.left_item_id,
            &self.right_item_id,
        ] {
            if !is_valid_id(id) {
                return Err(DomainError::InvalidId(id.clone()));
            }
        }
        if self.duration.value <= 0 {
            return Err(DomainError::NonPositiveDuration);
        }
        Ok(())
    }
}

impl CutTransition {
    pub fn new(
        track_id: String,
        left_item_id: String,
        right_item_id: String,
        kind: TransitionKind,
        duration: RationalTime,
    ) -> Result<Self, DomainError> {
        let transition_id = generate_transition_id();
        let ct = Self {
            transition_id,
            track_id,
            left_item_id,
            right_item_id,
            kind,
            duration,
        };
        ct.validate_basic()?;
        Ok(ct)
    }

    pub fn validate_basic(&self) -> Result<(), DomainError> {
        if !is_valid_id(&self.transition_id) {
            return Err(DomainError::InvalidId(self.transition_id.clone()));
        }
        if !is_valid_id(&self.track_id) {
            return Err(DomainError::InvalidId(self.track_id.clone()));
        }
        if !is_valid_id(&self.left_item_id) {
            return Err(DomainError::InvalidId(self.left_item_id.clone()));
        }
        if !is_valid_id(&self.right_item_id) {
            return Err(DomainError::InvalidId(self.right_item_id.clone()));
        }
        if self.duration.value <= 0 {
            return Err(DomainError::NonPositiveDuration);
        }
        if !matches!(
            self.kind,
            TransitionKind::CrossDissolve | TransitionKind::DipToBlack
        ) {
            return Err(DomainError::Validation(
                "Only CrossDissolve and DipToBlack are supported for cut transitions".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimelineItem {
    pub item_id: String,
    pub asset_id: Option<String>,
    pub name: String,
    pub kind: ItemKind,
    pub timeline_range: TimeRange,
    pub source_range: TimeRange,
    pub transform: Transform,
    pub crop: Crop,
    #[serde(default)]
    pub placement_mode: PlacementMode,
    #[serde(default)]
    pub visual_effect: VisualEffectPreset,
    #[serde(default)]
    pub speed: ClipSpeed,
    pub opacity: f32,
    pub volume: f32,
    pub text_content: Option<String>,
    #[serde(default)]
    pub text_style: Option<TextStyle>,
    #[serde(default)]
    pub text_template: Option<TextTemplateReference>,
    #[serde(default)]
    pub text_rich: Option<TextRichStyle>,
    #[serde(default)]
    pub text_animation: Option<TextAnimation>,
    #[serde(default)]
    pub template_instance: Option<TemplateInstance>,
    pub fade_in: Option<RationalTime>,
    pub fade_out: Option<RationalTime>,
    pub transition: Option<Transition>,
    pub keyframes: Vec<Keyframe>,
}

impl TimelineItem {
    pub fn new(
        asset_id: Option<String>,
        name: String,
        kind: ItemKind,
        timeline_range: TimeRange,
        source_range: TimeRange,
    ) -> Result<Self, DomainError> {
        let item_id = generate_item_id();
        let item = Self {
            item_id,
            asset_id,
            name,
            kind,
            timeline_range,
            source_range,
            transform: Transform::default(),
            crop: Crop::default(),
            placement_mode: PlacementMode::Fit,
            visual_effect: VisualEffectPreset::None,
            speed: ClipSpeed::Normal,
            opacity: 1.0,
            volume: 1.0,
            text_content: None,
            text_style: (kind == ItemKind::Text).then(TextStyle::default),
            text_template: None,
            text_rich: None,
            text_animation: None,
            template_instance: None,
            fade_in: None,
            fade_out: None,
            transition: None,
            keyframes: Vec::new(),
        };
        item.validate()?;
        Ok(item)
    }

    pub fn validate(&self) -> Result<(), DomainError> {
        if !is_valid_id(&self.item_id) {
            return Err(DomainError::InvalidId(self.item_id.clone()));
        }
        if self.timeline_range.duration.value <= 0 {
            return Err(DomainError::NonPositiveDuration);
        }
        if self.source_range.duration.value <= 0 {
            return Err(DomainError::NonPositiveDuration);
        }
        if let Some(text_style) = &self.text_style {
            text_style.validate()?;
        }
        if let Some(text_template) = &self.text_template {
            text_template.validate()?;
        }
        if self.kind != ItemKind::Text
            && (self.text_rich.is_some() || self.text_animation.is_some())
        {
            return Err(DomainError::Validation(
                "Only Text items can have text_rich or text_animation".to_string(),
            ));
        }
        if self.kind != ItemKind::Text && self.template_instance.is_some() {
            return Err(DomainError::Validation(
                "Only Text items can have template_instance".to_string(),
            ));
        }
        if let Some(instance) = &self.template_instance {
            instance.validate()?;
        }
        if let Some(rich) = &self.text_rich {
            rich.validate(self.text_content.as_deref().unwrap_or(""))?;
        }
        if let Some(anim) = &self.text_animation {
            let effective_duration = anim
                .phase_duration()
                .unwrap_or(self.timeline_range.duration);
            anim.validate_for_clip_duration(effective_duration)?;
        }
        if let Some(transition) = &self.transition {
            if matches!(
                transition.kind,
                TransitionKind::CrossDissolve | TransitionKind::DipToBlack
            ) {
                return Err(DomainError::Validation(
                    "Cut transitions must be stored on the track, not on a timeline item"
                        .to_string(),
                ));
            }
            if transition.duration.value <= 0 {
                return Err(DomainError::NonPositiveDuration);
            }
        }

        if self.speed != ClipSpeed::Normal
            && !matches!(self.kind, ItemKind::Video | ItemKind::Audio)
        {
            return Err(DomainError::Validation(
                "Speed is supported only on video and audio clips".to_string(),
            ));
        }

        let expected_source_duration = self.speed.source_duration_for(
            self.timeline_range.duration,
            self.source_range.duration.rate,
        )?;
        if !self
            .source_range
            .duration
            .semantic_eq(&expected_source_duration)
        {
            return Err(DomainError::DurationMismatch {
                source_dur: format!("{} frames", self.source_range.duration.value),
                timeline_dur: format!(
                    "{} frames at {:?}",
                    self.timeline_range.duration.value, self.speed
                ),
            });
        }

        // Validate keyframes: easing validity and last-keyframe invariant per property channel
        let all_props = [
            KeyframeProperty::Opacity,
            KeyframeProperty::PositionX,
            KeyframeProperty::PositionY,
            KeyframeProperty::ScaleX,
            KeyframeProperty::ScaleY,
            KeyframeProperty::Rotation,
            KeyframeProperty::Volume,
        ];

        for kf in &self.keyframes {
            kf.outgoing_easing.validate()?;
        }

        for prop in all_props {
            let mut prop_kfs: Vec<&Keyframe> = self
                .keyframes
                .iter()
                .filter(|k| k.property == prop)
                .collect();
            if prop_kfs.is_empty() {
                continue;
            }
            prop_kfs.sort_by_key(|a| a.offset);

            // Check for collision at exact same semantic timestamp
            for w in prop_kfs.windows(2) {
                if w[0].offset.is_same_time(&w[1].offset) {
                    return Err(DomainError::Validation(format!(
                        "Collision: duplicate keyframes on property {:?} at semantic offset {:?}",
                        prop, w[0].offset
                    )));
                }
            }

            // Invariant KF-05: The last keyframe in each property channel has no outgoing segment
            // and its canonical outgoing_easing is Linear.
            let last_kf = prop_kfs[prop_kfs.len() - 1];
            if !last_kf.outgoing_easing.is_linear() {
                return Err(DomainError::Validation(format!(
                    "Last keyframe on property {:?} at offset {:?} must have Linear outgoing easing, found {:?}",
                    prop, last_kf.offset, last_kf.outgoing_easing
                )));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Track {
    pub track_id: String,
    pub name: String,
    pub kind: TrackKind,
    pub muted: bool,
    pub locked: bool,
    pub items: Vec<TimelineItem>,
    #[serde(default)]
    pub transitions: Vec<CutTransition>,
    #[serde(default)]
    pub audio_transitions: Vec<AudioCrossfade>,
}

impl Track {
    pub fn new(name: String, kind: TrackKind) -> Self {
        Self {
            track_id: generate_track_id(),
            name,
            kind,
            muted: false,
            locked: false,
            items: Vec::new(),
            transitions: Vec::new(),
            audio_transitions: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<(), DomainError> {
        if !is_valid_id(&self.track_id) {
            return Err(DomainError::InvalidId(self.track_id.clone()));
        }

        for item in &self.items {
            item.validate()?;
        }

        // Check for overlaps between items on the same track
        for i in 0..self.items.len() {
            for j in (i + 1)..self.items.len() {
                let item_a = &self.items[i];
                let item_b = &self.items[j];
                if item_a.timeline_range.overlaps(&item_b.timeline_range) {
                    return Err(DomainError::OverlappingItems {
                        track_id: self.track_id.clone(),
                        item1_id: item_a.item_id.clone(),
                        item2_id: item_b.item_id.clone(),
                    });
                }
            }
        }

        // Validate cut transitions
        for trans in &self.transitions {
            trans.validate_basic()?;
            if trans.track_id != self.track_id {
                return Err(DomainError::Validation(format!(
                    "CutTransition track_id '{}' does not match track '{}'",
                    trans.track_id, self.track_id
                )));
            }

            let left = self
                .find_item(&trans.left_item_id)
                .ok_or_else(|| DomainError::ItemNotFound(trans.left_item_id.clone()))?;
            let right = self
                .find_item(&trans.right_item_id)
                .ok_or_else(|| DomainError::ItemNotFound(trans.right_item_id.clone()))?;

            // CD-04: Video item check
            if left.kind != ItemKind::Video || right.kind != ItemKind::Video {
                return Err(DomainError::UnsupportedTransitionMedia);
            }

            // CD-02: Immediate Adjacency Invariant: left.end == right.start (canonical semantic equality)
            let rate = trans.duration.rate;
            if !left
                .timeline_range
                .is_adjacent_to_with_rate(&right.timeline_range, rate)
            {
                return Err(DomainError::NonAdjacentTransitionParticipants {
                    track_id: self.track_id.clone(),
                    left_id: left.item_id.clone(),
                    right_id: right.item_id.clone(),
                });
            }

            // CD-15: Deterministic L/R tick rule & editorial extent
            let dur_ticks = trans.duration.value;
            let l_ticks = dur_ticks / 2;
            let r_ticks = dur_ticks - l_ticks;
            let left_extent = left.timeline_range.duration.rescaled_to(rate).value;
            let right_extent = right.timeline_range.duration.rescaled_to(rate).value;
            if left_extent < l_ticks || right_extent < r_ticks {
                return Err(DomainError::Validation(format!(
                    "CutTransition duration (L={}, R={}) exceeds participant editorial extent (left={}, right={})",
                    l_ticks, r_ticks, left_extent, right_extent
                )));
            }
        }

        let mut audio_transition_ids = std::collections::HashSet::new();
        let mut audio_transition_cuts = std::collections::HashSet::new();
        for trans in &self.audio_transitions {
            trans.validate_basic()?;
            if trans.track_id != self.track_id {
                return Err(DomainError::Validation(format!(
                    "AudioCrossfade track_id '{}' does not match track '{}'",
                    trans.track_id, self.track_id
                )));
            }
            if !audio_transition_ids.insert(trans.transition_id.as_str()) {
                return Err(DomainError::Validation(format!(
                    "Duplicate audio crossfade ID '{}'",
                    trans.transition_id
                )));
            }
            if !audio_transition_cuts
                .insert((trans.left_item_id.as_str(), trans.right_item_id.as_str()))
            {
                return Err(DomainError::Validation(format!(
                    "Duplicate audio crossfade at cut between '{}' and '{}'",
                    trans.left_item_id, trans.right_item_id
                )));
            }
            if self.kind != TrackKind::Audio {
                return Err(DomainError::UnsupportedAudioCrossfadeMedia);
            }
            let left = self
                .find_item(&trans.left_item_id)
                .ok_or_else(|| DomainError::ItemNotFound(trans.left_item_id.clone()))?;
            let right = self
                .find_item(&trans.right_item_id)
                .ok_or_else(|| DomainError::ItemNotFound(trans.right_item_id.clone()))?;
            if left.kind != ItemKind::Audio || right.kind != ItemKind::Audio {
                return Err(DomainError::UnsupportedAudioCrossfadeMedia);
            }
            if !left
                .timeline_range
                .is_adjacent_to_with_rate(&right.timeline_range, trans.duration.rate)
            {
                return Err(DomainError::NonAdjacentAudioCrossfade {
                    track_id: self.track_id.clone(),
                    left_id: left.item_id.clone(),
                    right_id: right.item_id.clone(),
                });
            }
            let duration = trans.duration;
            let left_ticks = left
                .timeline_range
                .duration
                .rescaled_to(duration.rate)
                .value;
            let right_ticks = right
                .timeline_range
                .duration
                .rescaled_to(duration.rate)
                .value;
            let l_ticks = duration.value / 2;
            let r_ticks = duration.value - l_ticks;
            if left_ticks < l_ticks || right_ticks < r_ticks {
                return Err(DomainError::Validation(format!(
                    "AudioCrossfade duration exceeds participant editorial extent (left={}, right={})",
                    left_ticks, right_ticks
                )));
            }
        }

        // CD-07: Transition singularity per cut
        for i in 0..self.transitions.len() {
            for j in (i + 1)..self.transitions.len() {
                let t_a = &self.transitions[i];
                let t_b = &self.transitions[j];
                if t_a.transition_id == t_b.transition_id {
                    return Err(DomainError::Validation(format!(
                        "Duplicate transition ID '{}'",
                        t_a.transition_id
                    )));
                }
                if t_a.left_item_id == t_b.left_item_id || t_a.right_item_id == t_b.right_item_id {
                    return Err(DomainError::DuplicateCutTransition {
                        left_id: t_b.left_item_id.clone(),
                        right_id: t_b.right_item_id.clone(),
                    });
                }
            }
        }

        Ok(())
    }

    pub fn find_item(&self, item_id: &str) -> Option<&TimelineItem> {
        self.items.iter().find(|i| i.item_id == item_id)
    }

    pub fn find_item_mut(&mut self, item_id: &str) -> Option<&mut TimelineItem> {
        self.items.iter_mut().find(|i| i.item_id == item_id)
    }

    pub fn find_transition(&self, transition_id: &str) -> Option<&CutTransition> {
        self.transitions
            .iter()
            .find(|t| t.transition_id == transition_id)
    }

    pub fn find_transition_mut(&mut self, transition_id: &str) -> Option<&mut CutTransition> {
        self.transitions
            .iter_mut()
            .find(|t| t.transition_id == transition_id)
    }

    pub fn find_transition_at_cut(&self, left_id: &str, right_id: &str) -> Option<&CutTransition> {
        self.transitions
            .iter()
            .find(|t| t.left_item_id == left_id && t.right_item_id == right_id)
    }

    pub fn find_transition_at_cut_mut(
        &mut self,
        left_id: &str,
        right_id: &str,
    ) -> Option<&mut CutTransition> {
        self.transitions
            .iter_mut()
            .find(|t| t.left_item_id == left_id && t.right_item_id == right_id)
    }

    pub fn find_transitions_for_item(&self, item_id: &str) -> Vec<&CutTransition> {
        self.transitions
            .iter()
            .filter(|t| t.left_item_id == item_id || t.right_item_id == item_id)
            .collect()
    }

    pub fn remove_transition(&mut self, transition_id: &str) -> Option<CutTransition> {
        if let Some(pos) = self
            .transitions
            .iter()
            .position(|t| t.transition_id == transition_id)
        {
            Some(self.transitions.remove(pos))
        } else {
            None
        }
    }

    pub fn find_audio_transition(&self, transition_id: &str) -> Option<&AudioCrossfade> {
        self.audio_transitions
            .iter()
            .find(|transition| transition.transition_id == transition_id)
    }

    pub fn remove_audio_transition(&mut self, transition_id: &str) -> Option<AudioCrossfade> {
        let position = self
            .audio_transitions
            .iter()
            .position(|transition| transition.transition_id == transition_id)?;
        Some(self.audio_transitions.remove(position))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClientRecord {
    pub client_epoch: u64,
    pub highest_sequence: u64,
    pub last_active_revision: u64,
    pub registered_at_revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClientRegistry {
    pub min_accepted_epoch: u64,
    pub current_epoch: u64,
    pub max_active_clients: usize,
    pub clients: std::collections::HashMap<String, ClientRecord>,
}

impl Default for ClientRegistry {
    fn default() -> Self {
        Self {
            min_accepted_epoch: 0,
            current_epoch: 1,
            max_active_clients: 64,
            clients: std::collections::HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandLedgerEntry {
    pub operation_id: String,
    pub transaction_id: Option<String>,
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub client_epoch: Option<u64>,
    #[serde(default)]
    pub sequence_number: Option<u64>,
    pub fingerprint: String,
    pub status: String,
    pub applied_at_revision: u64,
    pub affected_item_id: Option<String>,
    pub affected_track_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Project {
    pub schema_version: u32,
    pub project_id: String,
    pub name: String,
    pub revision: u64,
    pub settings: ProjectSettings,
    pub assets: Vec<Asset>,
    pub tracks: Vec<Track>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub client_registry: ClientRegistry,
    #[serde(default)]
    pub command_ledger: Vec<CommandLedgerEntry>,
}

impl Project {
    pub fn new(name: String) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: generate_project_id(),
            name,
            revision: 0,
            settings: ProjectSettings::default(),
            assets: Vec::new(),
            tracks: Vec::new(),
            created_at: now.clone(),
            updated_at: now,
            client_registry: ClientRegistry::default(),
            command_ledger: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<(), DomainError> {
        if self.schema_version != CURRENT_SCHEMA_VERSION {
            return Err(DomainError::SchemaVersionMismatch {
                expected: CURRENT_SCHEMA_VERSION,
                found: self.schema_version,
            });
        }
        if !is_valid_id(&self.project_id) {
            return Err(DomainError::InvalidId(self.project_id.clone()));
        }
        self.settings.validate()?;
        for asset in &self.assets {
            if let Some(dimensions) = &asset.dimensions {
                dimensions.validate()?;
            }
        }

        // Validate all tracks
        for track in &self.tracks {
            track.validate()?;
        }

        // Validate asset references in clips
        for track in &self.tracks {
            for item in &track.items {
                if let Some(ref asset_id) = item.asset_id {
                    if !self.assets.iter().any(|a| &a.asset_id == asset_id) {
                        return Err(DomainError::AssetNotFound(asset_id.clone()));
                    }
                }
            }
            for crossfade in &track.audio_transitions {
                let (left_track, left) = self
                    .find_item_and_track(&crossfade.left_item_id)
                    .ok_or_else(|| DomainError::ItemNotFound(crossfade.left_item_id.clone()))?;
                let (right_track, right) = self
                    .find_item_and_track(&crossfade.right_item_id)
                    .ok_or_else(|| DomainError::ItemNotFound(crossfade.right_item_id.clone()))?;
                if left_track.track_id != track.track_id || right_track.track_id != track.track_id {
                    return Err(DomainError::UnsupportedAudioCrossfadeMedia);
                }
                let rate = crossfade.duration.rate;
                let left_asset_id = left.asset_id.as_deref().ok_or_else(|| {
                    DomainError::Validation(format!(
                        "Audio crossfade outgoing item '{}' has no asset",
                        left.item_id
                    ))
                })?;
                let right_asset_id = right.asset_id.as_deref().ok_or_else(|| {
                    DomainError::Validation(format!(
                        "Audio crossfade incoming item '{}' has no asset",
                        right.item_id
                    ))
                })?;
                let left_asset = self
                    .find_asset(left_asset_id)
                    .ok_or_else(|| DomainError::AssetNotFound(left_asset_id.to_string()))?;
                let right_asset = self
                    .find_asset(right_asset_id)
                    .ok_or_else(|| DomainError::AssetNotFound(right_asset_id.to_string()))?;
                if left_asset.media_kind != MediaKind::Audio
                    || right_asset.media_kind != MediaKind::Audio
                {
                    return Err(DomainError::UnsupportedAudioCrossfadeMedia);
                }
                let left_media_end = left_asset
                    .duration
                    .ok_or_else(|| {
                        DomainError::Validation(format!(
                            "Audio crossfade outgoing asset '{}' has no probed duration",
                            left_asset_id
                        ))
                    })?
                    .rescaled_to(rate)
                    .value;
                let left_source_start = left.source_range.start.rescaled_to(rate).value;
                let left_source_duration = left.source_range.duration.rescaled_to(rate).value;
                let right_preroll = right.source_range.start.rescaled_to(rate).value;
                let l_ticks = crossfade.duration.value / 2;
                let r_ticks = crossfade.duration.value - l_ticks;
                let left_postroll = left_media_end - left_source_start - left_source_duration;
                if left_postroll < r_ticks || right_preroll < l_ticks {
                    return Err(DomainError::Validation(format!(
                        "Audio crossfade source handles are insufficient: postroll {} < {} or preroll {} < {}",
                        left_postroll, r_ticks, right_preroll, l_ticks
                    )));
                }
                if right_asset.duration.is_none() {
                    return Err(DomainError::Validation(format!(
                        "Audio crossfade incoming asset '{}' has no probed duration",
                        right_asset_id
                    )));
                }
                let right_media_end = right_asset.duration.unwrap().rescaled_to(rate).value;
                let right_source_end = right.source_range.end().rescaled_to(rate).value;
                if right_source_end > right_media_end {
                    return Err(DomainError::Validation(format!(
                        "Audio crossfade incoming source range ends at {} beyond probed asset duration {}",
                        right_source_end, right_media_end
                    )));
                }
            }
        }

        // Validate client registry capacity
        if self.client_registry.max_active_clients > 0
            && self.client_registry.clients.len() > self.client_registry.max_active_clients
        {
            return Err(DomainError::Validation(format!(
                "Client registry capacity exceeded: {} > {}",
                self.client_registry.clients.len(),
                self.client_registry.max_active_clients
            )));
        }

        Ok(())
    }

    pub fn find_track(&self, track_id: &str) -> Option<&Track> {
        self.tracks.iter().find(|t| t.track_id == track_id)
    }

    pub fn find_track_mut(&mut self, track_id: &str) -> Option<&mut Track> {
        self.tracks.iter_mut().find(|t| t.track_id == track_id)
    }

    pub fn find_item_and_track(&self, item_id: &str) -> Option<(&Track, &TimelineItem)> {
        for track in &self.tracks {
            if let Some(item) = track.find_item(item_id) {
                return Some((track, item));
            }
        }
        None
    }

    pub fn find_transition(&self, transition_id: &str) -> Option<(&Track, &CutTransition)> {
        for track in &self.tracks {
            if let Some(trans) = track.find_transition(transition_id) {
                return Some((track, trans));
            }
        }
        None
    }

    pub fn find_transition_mut(&mut self, transition_id: &str) -> Option<&mut CutTransition> {
        for track in &mut self.tracks {
            if let Some(trans) = track.find_transition_mut(transition_id) {
                return Some(trans);
            }
        }
        None
    }

    pub fn find_asset(&self, asset_id: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.asset_id == asset_id)
    }

    pub fn total_duration(&self) -> RationalTime {
        let rate = self.settings.frame_rate;
        let mut max_end = RationalTime::zero(rate);
        for track in &self.tracks {
            for item in &track.items {
                let item_end = item.timeline_range.end().rescaled_to(rate);
                if item_end > max_end {
                    max_end = item_end;
                }
            }
        }
        max_end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_creation_and_validation() {
        let project = Project::new("Test Project".to_string());
        assert_eq!(project.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(project.revision, 0);
        assert!(project.validate().is_ok());
    }

    #[test]
    fn test_item_duration_and_range_validation() {
        let rate = FrameRate::FPS_30;
        let t_range = TimeRange::new(
            RationalTime::zero(rate),
            RationalTime::from_frames(30, rate),
        );
        let s_range = TimeRange::new(
            RationalTime::zero(rate),
            RationalTime::from_frames(30, rate),
        );

        let item = TimelineItem::new(
            None,
            "Clip 1".to_string(),
            ItemKind::Video,
            t_range,
            s_range,
        )
        .unwrap();
        assert!(item.validate().is_ok());

        // Mismatched durations
        let s_bad_range = TimeRange::new(
            RationalTime::zero(rate),
            RationalTime::from_frames(60, rate),
        );
        let bad_item = TimelineItem::new(
            None,
            "Bad Clip".to_string(),
            ItemKind::Video,
            t_range,
            s_bad_range,
        );
        assert!(bad_item.is_err());
    }

    #[test]
    fn speed_duration_mapping_requires_exact_source_frames() {
        let rate = FrameRate::FPS_30;
        let timeline_duration = RationalTime::from_frames(60, rate);

        assert_eq!(
            ClipSpeed::Half
                .source_duration_for(timeline_duration, rate)
                .expect("half speed maps 60 timeline frames to 30 source frames"),
            RationalTime::from_frames(30, rate)
        );
        assert_eq!(
            ClipSpeed::Double
                .source_duration_for(timeline_duration, rate)
                .expect("double speed maps 60 timeline frames to 120 source frames"),
            RationalTime::from_frames(120, rate)
        );
        assert!(
            ClipSpeed::Half
                .source_duration_for(RationalTime::from_frames(1, rate), rate)
                .is_err(),
            "a half-speed clip cannot silently use half a source frame"
        );
    }

    #[test]
    fn test_track_overlapping_items_rejected() {
        let rate = FrameRate::FPS_30;
        let mut track = Track::new("Video Track 1".to_string(), TrackKind::Video);

        let t1 = TimeRange::new(
            RationalTime::from_frames(0, rate),
            RationalTime::from_frames(30, rate),
        );
        let s1 = t1;
        let item1 = TimelineItem::new(None, "Clip 1".to_string(), ItemKind::Video, t1, s1).unwrap();

        // Item 2 overlaps item 1 (starts at frame 15, ends at 45)
        let t2 = TimeRange::new(
            RationalTime::from_frames(15, rate),
            RationalTime::from_frames(30, rate),
        );
        let s2 = t2;
        let item2 = TimelineItem::new(None, "Clip 2".to_string(), ItemKind::Video, t2, s2).unwrap();

        track.items.push(item1);
        track.items.push(item2);

        assert!(matches!(
            track.validate(),
            Err(DomainError::OverlappingItems { .. })
        ));
    }

    #[test]
    fn test_phase13_cut_transition_validation_rules() {
        let rate = FrameRate::FPS_30;
        let mut track = Track::new("Video Track 1".to_string(), TrackKind::Video);

        // Clip 1: [0, 30)
        let t1 = TimeRange::new(
            RationalTime::from_frames(0, rate),
            RationalTime::from_frames(30, rate),
        );
        let item1 = TimelineItem::new(None, "Clip 1".to_string(), ItemKind::Video, t1, t1).unwrap();
        let item1_id = item1.item_id.clone();

        // Clip 2: [30, 60) (immediate adjacent at cut C = 30)
        let t2 = TimeRange::new(
            RationalTime::from_frames(30, rate),
            RationalTime::from_frames(30, rate),
        );
        let item2 = TimelineItem::new(None, "Clip 2".to_string(), ItemKind::Video, t2, t2).unwrap();
        let item2_id = item2.item_id.clone();

        track.items.push(item1);
        track.items.push(item2);

        // 1. Valid CutTransition of 10 frames (L=5, R=5, within editorial extents 30 and 30)
        let ct_valid = CutTransition::new(
            track.track_id.clone(),
            item1_id.clone(),
            item2_id.clone(),
            TransitionKind::CrossDissolve,
            RationalTime::from_frames(10, rate),
        )
        .unwrap();
        track.transitions.push(ct_valid);
        assert!(track.validate().is_ok());

        // 2. Duplicate transition at same cut rejected (CD-07)
        let ct_dup = CutTransition::new(
            track.track_id.clone(),
            item1_id.clone(),
            item2_id.clone(),
            TransitionKind::CrossDissolve,
            RationalTime::from_frames(10, rate),
        )
        .unwrap();
        track.transitions.push(ct_dup);
        assert!(matches!(
            track.validate(),
            Err(DomainError::DuplicateCutTransition { .. })
        ));
        track.transitions.pop();

        // 3. Non-video item rejected (CD-04)
        let t3 = TimeRange::new(
            RationalTime::from_frames(60, rate),
            RationalTime::from_frames(30, rate),
        );
        let audio_item =
            TimelineItem::new(None, "Audio Clip".to_string(), ItemKind::Audio, t3, t3).unwrap();
        let audio_id = audio_item.item_id.clone();
        track.items.push(audio_item);

        let ct_audio = CutTransition::new(
            track.track_id.clone(),
            item2_id.clone(),
            audio_id,
            TransitionKind::CrossDissolve,
            RationalTime::from_frames(10, rate),
        )
        .unwrap();
        track.transitions.push(ct_audio);
        assert_eq!(
            track.validate(),
            Err(DomainError::UnsupportedTransitionMedia)
        );
        track.transitions.pop();
        track.items.pop();

        // 4. Non-adjacent items rejected (CD-02)
        let t_gap = TimeRange::new(
            RationalTime::from_frames(100, rate),
            RationalTime::from_frames(30, rate),
        );
        let item_gap =
            TimelineItem::new(None, "Gap Clip".to_string(), ItemKind::Video, t_gap, t_gap).unwrap();
        let gap_id = item_gap.item_id.clone();
        track.items.push(item_gap);

        let ct_gap = CutTransition::new(
            track.track_id.clone(),
            item2_id.clone(),
            gap_id,
            TransitionKind::CrossDissolve,
            RationalTime::from_frames(10, rate),
        )
        .unwrap();
        track.transitions.push(ct_gap);
        assert!(matches!(
            track.validate(),
            Err(DomainError::NonAdjacentTransitionParticipants { .. })
        ));
        track.transitions.pop();
        track.items.pop();

        // 5. Exceeding editorial extent rejected (CD-21)
        // Item 1 has 30 frames. If D = 70, L = 35 > 30 extent -> rejected!
        let ct_too_long = CutTransition::new(
            track.track_id.clone(),
            item1_id.clone(),
            item2_id.clone(),
            TransitionKind::CrossDissolve,
            RationalTime::from_frames(70, rate),
        )
        .unwrap();
        track.transitions.push(ct_too_long);
        assert!(matches!(track.validate(), Err(DomainError::Validation(_))));
        track.transitions.pop();

        // 6. Dangling item reference rejected
        let ct_dangling = CutTransition::new(
            track.track_id.clone(),
            item1_id,
            "clip_nonexistent".to_string(),
            TransitionKind::CrossDissolve,
            RationalTime::from_frames(10, rate),
        )
        .unwrap();
        track.transitions.push(ct_dangling);
        assert!(matches!(
            track.validate(),
            Err(DomainError::ItemNotFound(_))
        ));
    }
}
