//! Project Domain Model: Layered Templates, Parameters & Portable Packs (Phase 32)
//!
//! Adds declarative, bounded multi-layer template definitions and instances
//! with typed parameter bindings, immutable versioning, and resource manifests.

use crate::modules::domain::id::is_valid_id;
use crate::modules::domain::text::{
    grapheme_count, FontReference, FontStyle, TextAnimation, TextFill, TextRichStyle,
};
use crate::modules::domain::time::{FrameRate, RationalTime};
use crate::modules::domain::{
    is_hex_color, DomainError, PlacementMode, TextAnchor, TextStyle, Transform,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Maximum layers allowed in a single template instance (Phase 27/32 bounded budget).
pub const MAX_TEMPLATE_LAYERS: usize = 16;

/// Maximum text slots permitted within a template instance.
pub const MAX_TEMPLATE_TEXT_SLOTS: usize = 8;

/// Maximum graphemes allowed per text slot.
pub const MAX_TEMPLATE_GRAPHEMES_PER_SLOT: usize = 512;

fn default_true() -> bool {
    true
}

fn default_one() -> f32 {
    1.0
}

fn default_center_anchor() -> TextAnchor {
    TextAnchor::Center
}

fn default_ref_width() -> u32 {
    1920
}

fn default_ref_height() -> u32 {
    1080
}

fn default_safe_margin() -> f32 {
    80.0
}

fn default_template_duration() -> RationalTime {
    RationalTime::new(90, FrameRate::FPS_30)
}

/// Whitelisted, typed property paths that parameters can bind to.
/// Strictly bounded: NO arbitrary code execution or unvetted property path traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TemplatePropertyPath {
    TextContent,
    FontColor,
    FontSize,
    FontFamily,
    BackgroundColor,
    PositionX,
    PositionY,
    ScaleX,
    ScaleY,
    Rotation,
    Opacity,
    FillColor,
    StrokeColor,
    StrokeWidth,
    CornerRadius,
    Width,
    Height,
    Visible,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TemplateParameterType {
    Text,
    Color,
    Number { min: f64, max: f64, step: f64 },
    Boolean,
    Enum { options: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum TemplateParameterValue {
    Text(String),
    Color(String),
    Number(f64),
    Boolean(bool),
    Enum(String),
}

impl TemplateParameterValue {
    pub fn matches_type(&self, param_type: &TemplateParameterType) -> bool {
        match (self, param_type) {
            (TemplateParameterValue::Text(_), TemplateParameterType::Text) => true,
            (TemplateParameterValue::Color(c), TemplateParameterType::Color) => is_hex_color(c),
            (TemplateParameterValue::Number(n), TemplateParameterType::Number { min, max, .. }) => {
                n.is_finite() && *n >= *min && *n <= *max
            }
            (TemplateParameterValue::Boolean(_), TemplateParameterType::Boolean) => true,
            (TemplateParameterValue::Enum(val), TemplateParameterType::Enum { options }) => {
                options.contains(val)
            }
            _ => false,
        }
    }
}

/// A parameter definition declared in a template definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateParameterDefinition {
    pub key: String,
    pub name: String,
    pub param_type: TemplateParameterType,
    pub default_value: TemplateParameterValue,
    #[serde(default)]
    pub description: Option<String>,
}

impl TemplateParameterDefinition {
    pub fn validate(&self) -> Result<(), DomainError> {
        if !is_valid_id(&self.key) {
            return Err(DomainError::Validation(format!(
                "Invalid parameter key '{}'",
                self.key
            )));
        }
        if self.name.trim().is_empty() {
            return Err(DomainError::Validation(format!(
                "Parameter '{}' name cannot be empty",
                self.key
            )));
        }
        if !self.default_value.matches_type(&self.param_type) {
            return Err(DomainError::Validation(format!(
                "Default value for parameter '{}' does not match its declared type",
                self.key
            )));
        }
        Ok(())
    }
}

/// Binding between a parameter and a target layer's property.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateParameterBinding {
    pub parameter_key: String,
    pub target_layer_id: String,
    pub target_property: TemplatePropertyPath,
}

impl TemplateParameterBinding {
    pub fn validate(
        &self,
        known_params: &HashMap<&str, &TemplateParameterType>,
        known_layers: &HashSet<&str>,
    ) -> Result<(), DomainError> {
        let param_type = known_params
            .get(self.parameter_key.as_str())
            .ok_or_else(|| {
                DomainError::Validation(format!(
                    "Binding references unknown parameter '{}'",
                    self.parameter_key
                ))
            })?;
        if !known_layers.contains(self.target_layer_id.as_str()) {
            return Err(DomainError::Validation(format!(
                "Binding references unknown target layer '{}'",
                self.target_layer_id
            )));
        }
        // Verify property-type whitelist compatibility
        #[allow(clippy::match_like_matches_macro)]
        let compatible = match (self.target_property, param_type) {
            (TemplatePropertyPath::TextContent, TemplateParameterType::Text) => true,
            (
                TemplatePropertyPath::FontColor
                | TemplatePropertyPath::BackgroundColor
                | TemplatePropertyPath::FillColor
                | TemplatePropertyPath::StrokeColor,
                TemplateParameterType::Color,
            ) => true,
            (
                TemplatePropertyPath::FontSize
                | TemplatePropertyPath::PositionX
                | TemplatePropertyPath::PositionY
                | TemplatePropertyPath::ScaleX
                | TemplatePropertyPath::ScaleY
                | TemplatePropertyPath::Rotation
                | TemplatePropertyPath::Opacity
                | TemplatePropertyPath::StrokeWidth
                | TemplatePropertyPath::CornerRadius
                | TemplatePropertyPath::Width
                | TemplatePropertyPath::Height,
                TemplateParameterType::Number { .. },
            ) => true,
            (TemplatePropertyPath::FontFamily, TemplateParameterType::Text) => true,
            (TemplatePropertyPath::Visible, TemplateParameterType::Boolean) => true,
            _ => false,
        };
        if !compatible {
            return Err(DomainError::Validation(format!(
                "Property '{:?}' is not compatible with parameter '{}' type",
                self.target_property, self.parameter_key
            )));
        }
        Ok(())
    }
}

/// User override of a specific layer property.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateOverride {
    pub target_layer_id: String,
    pub property: TemplatePropertyPath,
    pub value: TemplateParameterValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TemplateShapeKind {
    #[default]
    Rectangle,
    RoundedRectangle,
    Ellipse,
    Circle,
    Line,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateShapeLayerData {
    pub shape_kind: TemplateShapeKind,
    pub width: f32,
    pub height: f32,
    #[serde(default)]
    pub fill_color: Option<String>,
    #[serde(default)]
    pub stroke_color: Option<String>,
    #[serde(default)]
    pub stroke_width: f32,
    #[serde(default)]
    pub corner_radius: f32,
}

impl TemplateShapeLayerData {
    pub fn validate(&self) -> Result<(), DomainError> {
        if !self.width.is_finite() || self.width < 0.0 {
            return Err(DomainError::Validation(
                "Shape width must be finite and non-negative".to_string(),
            ));
        }
        if !self.height.is_finite() || self.height < 0.0 {
            return Err(DomainError::Validation(
                "Shape height must be finite and non-negative".to_string(),
            ));
        }
        if !self.stroke_width.is_finite() || self.stroke_width < 0.0 {
            return Err(DomainError::Validation(
                "Shape stroke width must be finite and non-negative".to_string(),
            ));
        }
        if !self.corner_radius.is_finite() || self.corner_radius < 0.0 {
            return Err(DomainError::Validation(
                "Shape corner radius must be finite and non-negative".to_string(),
            ));
        }
        if let Some(color) = &self.fill_color {
            if !is_hex_color(color) {
                return Err(DomainError::Validation(
                    "Shape fill color must be #RRGGBB or #RRGGBBAA".to_string(),
                ));
            }
        }
        if let Some(color) = &self.stroke_color {
            if !is_hex_color(color) {
                return Err(DomainError::Validation(
                    "Shape stroke color must be #RRGGBB or #RRGGBBAA".to_string(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct TemplateTextLayerData {
    /// If true, this is the primary slot and reuses the owning TimelineItem's
    /// text_content, text_rich, and text_animation (no duplicate authority!).
    #[serde(default)]
    pub is_primary: bool,
    #[serde(default)]
    pub slot_key: Option<String>,
    #[serde(default)]
    pub text_content: Option<String>,
    #[serde(default)]
    pub text_style: Option<TextStyle>,
    #[serde(default)]
    pub text_rich: Option<TextRichStyle>,
    #[serde(default)]
    pub text_animation: Option<TextAnimation>,
}

impl TemplateTextLayerData {
    pub fn validate(&self) -> Result<(), DomainError> {
        if let Some(text) = &self.text_content {
            let count = grapheme_count(text);
            if count > MAX_TEMPLATE_GRAPHEMES_PER_SLOT {
                return Err(DomainError::TextTooLong {
                    max: MAX_TEMPLATE_GRAPHEMES_PER_SLOT,
                    found: count,
                });
            }
        }
        if let Some(style) = &self.text_style {
            style.validate()?;
        }
        if let Some(rich) = &self.text_rich {
            rich.validate(self.text_content.as_deref().unwrap_or(""))?;
        }
        if let Some(anim) = &self.text_animation {
            if let Some(in_seg) = &anim.in_anim {
                in_seg.validate()?;
            }
            if let Some(loop_seg) = &anim.loop_anim {
                loop_seg.validate()?;
            }
            if let Some(out_seg) = &anim.out_anim {
                out_seg.validate()?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateImageLayerData {
    pub asset_id: Option<String>,
    #[serde(default)]
    pub asset_name: Option<String>,
    #[serde(default)]
    pub asset_hash: Option<String>,
    #[serde(default)]
    pub width: f32,
    #[serde(default)]
    pub height: f32,
    #[serde(default)]
    pub placement_mode: PlacementMode,
}

impl TemplateImageLayerData {
    pub fn validate(&self) -> Result<(), DomainError> {
        if !self.width.is_finite() || self.width < 0.0 {
            return Err(DomainError::Validation(
                "Image layer width must be finite and non-negative".to_string(),
            ));
        }
        if !self.height.is_finite() || self.height < 0.0 {
            return Err(DomainError::Validation(
                "Image layer height must be finite and non-negative".to_string(),
            ));
        }
        if let Some(aid) = &self.asset_id {
            if !is_valid_id(aid) {
                return Err(DomainError::InvalidId(aid.clone()));
            }
        }
        Ok(())
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum TemplateLayerKind {
    Text(TemplateTextLayerData),
    Shape(TemplateShapeLayerData),
    Image(TemplateImageLayerData),
}

/// A layer within a template definition or instance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateLayer {
    pub layer_id: String,
    pub name: String,
    #[serde(default)]
    pub z_index: i32,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default = "default_one")]
    pub opacity: f32,
    #[serde(default)]
    pub transform: Transform,
    #[serde(default = "default_center_anchor")]
    pub anchor: TextAnchor,
    #[serde(default)]
    pub pivot: (f32, f32),
    #[serde(default)]
    pub motion: Option<TextAnimation>,
    pub kind: TemplateLayerKind,
}

impl TemplateLayer {
    pub fn validate(&self) -> Result<(), DomainError> {
        if !is_valid_id(&self.layer_id) {
            return Err(DomainError::InvalidId(self.layer_id.clone()));
        }
        if self.name.trim().is_empty() {
            return Err(DomainError::Validation(format!(
                "Layer '{}' name cannot be empty",
                self.layer_id
            )));
        }
        if !self.opacity.is_finite() || !(0.0..=1.0).contains(&self.opacity) {
            return Err(DomainError::Validation(format!(
                "Layer '{}' opacity must be between 0.0 and 1.0",
                self.layer_id
            )));
        }
        if !self.transform.position_x.is_finite()
            || !self.transform.position_y.is_finite()
            || !self.transform.scale_x.is_finite()
            || !self.transform.scale_y.is_finite()
            || !self.transform.rotation.is_finite()
        {
            return Err(DomainError::Validation(format!(
                "Layer '{}' transform values must be finite",
                self.layer_id
            )));
        }
        if !self.pivot.0.is_finite() || !self.pivot.1.is_finite() {
            return Err(DomainError::Validation(format!(
                "Layer '{}' pivot values must be finite",
                self.layer_id
            )));
        }
        if let Some(anim) = &self.motion {
            if let Some(in_seg) = &anim.in_anim {
                in_seg.validate()?;
            }
            if let Some(loop_seg) = &anim.loop_anim {
                loop_seg.validate()?;
            }
            if let Some(out_seg) = &anim.out_anim {
                out_seg.validate()?;
            }
        }
        match &self.kind {
            TemplateLayerKind::Text(t) => t.validate()?,
            TemplateLayerKind::Shape(s) => s.validate()?,
            TemplateLayerKind::Image(i) => i.validate()?,
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemplateResourceKind {
    Font,
    Image,
    Thumbnail,
}

/// Validates whether a resource file_name is a safe single-component path.
/// Must be ASCII alphanumeric + '-' + '_' with one optional extension.
/// Rejects "..", "/", "\", ":", empty names, or path traversal attempts.
pub fn is_safe_resource_filename(name: &str) -> bool {
    if name.is_empty() || name.len() > 128 {
        return false;
    }
    if name.contains('/')
        || name.contains('\\')
        || name.contains(':')
        || name.contains("..")
        || name.contains('\0')
    {
        return false;
    }
    let parts: Vec<&str> = name.split('.').collect();
    if parts.is_empty() || parts.len() > 2 {
        return false;
    }
    let base = parts[0];
    if base.is_empty()
        || !base
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return false;
    }
    if parts.len() == 2 {
        let ext = parts[1];
        if ext.is_empty()
            || !ext
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return false;
        }
    }
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateResourceReference {
    pub resource_id: String,
    pub kind: TemplateResourceKind,
    pub file_name: String,
    pub sha256: String,
    pub byte_size: u64,
    #[serde(default = "default_true")]
    pub required: bool,
}

impl TemplateResourceReference {
    pub fn validate(&self) -> Result<(), DomainError> {
        if !is_valid_id(&self.resource_id) {
            return Err(DomainError::InvalidId(self.resource_id.clone()));
        }
        if self.file_name.trim().is_empty() {
            return Err(DomainError::Validation(
                "Resource file_name cannot be empty".to_string(),
            ));
        }
        if !is_safe_resource_filename(&self.file_name) {
            return Err(DomainError::Validation(format!(
                "Resource file_name '{}' is not a safe single-component filename",
                self.file_name
            )));
        }
        if self.sha256.len() != 64 || !self.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(DomainError::Validation(
                "Resource sha256 must be a 64-character hex string".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct TemplateResourceManifest {
    #[serde(default)]
    pub resources: Vec<TemplateResourceReference>,
}

impl TemplateResourceManifest {
    pub fn validate(&self) -> Result<(), DomainError> {
        let mut ids = HashSet::new();
        for res in &self.resources {
            res.validate()?;
            if !ids.insert(&res.resource_id) {
                return Err(DomainError::Validation(format!(
                    "Duplicate resource_id '{}' in manifest",
                    res.resource_id
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponsivePolicy {
    #[serde(default = "default_ref_width")]
    pub reference_width: u32,
    #[serde(default = "default_ref_height")]
    pub reference_height: u32,
    #[serde(default)]
    pub supported_aspect_ratios: Vec<String>,
    #[serde(default = "default_true")]
    pub auto_wrap: bool,
    #[serde(default)]
    pub auto_fit: bool,
    #[serde(default = "default_safe_margin")]
    pub safe_margin: f32,
}

impl Default for ResponsivePolicy {
    fn default() -> Self {
        Self {
            reference_width: default_ref_width(),
            reference_height: default_ref_height(),
            supported_aspect_ratios: vec![
                "16:9".to_string(),
                "9:16".to_string(),
                "1:1".to_string(),
                "4:5".to_string(),
            ],
            auto_wrap: true,
            auto_fit: false,
            safe_margin: default_safe_margin(),
        }
    }
}

impl ResponsivePolicy {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.reference_width == 0 || self.reference_height == 0 {
            return Err(DomainError::Validation(
                "Responsive reference dimensions must be positive".to_string(),
            ));
        }
        if !self.safe_margin.is_finite() || self.safe_margin < 0.0 {
            return Err(DomainError::Validation(
                "Responsive safe margin must be finite and non-negative".to_string(),
            ));
        }
        Ok(())
    }
}

/// Canonical, immutable template definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateDefinition {
    pub id: String,
    pub version: u32,
    pub name: String,
    pub category: String,
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub parameters: Vec<TemplateParameterDefinition>,
    #[serde(default)]
    pub bindings: Vec<TemplateParameterBinding>,
    pub layers: Vec<TemplateLayer>,
    #[serde(default)]
    pub resources: TemplateResourceManifest,
    #[serde(default)]
    pub responsive_policy: ResponsivePolicy,
    #[serde(default = "default_template_duration")]
    pub default_duration: RationalTime,
}

impl TemplateDefinition {
    pub fn validate(&self) -> Result<(), DomainError> {
        if !is_valid_id(&self.id) {
            return Err(DomainError::InvalidId(self.id.clone()));
        }
        if self.version == 0 {
            return Err(DomainError::Validation(
                "Template version must be positive".to_string(),
            ));
        }
        if self.name.trim().is_empty() {
            return Err(DomainError::Validation(
                "Template name cannot be empty".to_string(),
            ));
        }
        if self.layers.is_empty() {
            return Err(DomainError::Validation(
                "Template must contain at least one layer".to_string(),
            ));
        }
        if self.layers.len() > MAX_TEMPLATE_LAYERS {
            return Err(DomainError::Validation(format!(
                "Template exceeds maximum allowed layers ({MAX_TEMPLATE_LAYERS}): found {}",
                self.layers.len()
            )));
        }

        // Validate layer uniqueness and text slot limit
        let mut layer_ids = HashSet::new();
        let mut text_slot_count = 0;
        let mut primary_count = 0;
        for layer in &self.layers {
            layer.validate()?;
            if !layer_ids.insert(layer.layer_id.as_str()) {
                return Err(DomainError::Validation(format!(
                    "Duplicate layer ID '{}' in template '{}'",
                    layer.layer_id, self.id
                )));
            }
            if let TemplateLayerKind::Text(t) = &layer.kind {
                text_slot_count += 1;
                if t.is_primary {
                    primary_count += 1;
                }
            }
        }

        if text_slot_count > MAX_TEMPLATE_TEXT_SLOTS {
            return Err(DomainError::Validation(format!(
                "Template exceeds maximum text slots ({MAX_TEMPLATE_TEXT_SLOTS}): found {text_slot_count}"
            )));
        }
        if primary_count > 1 {
            return Err(DomainError::Validation(format!(
                "Template '{}' has {primary_count} primary text layers; at most one primary slot is allowed",
                self.id
            )));
        }

        // Validate parameters
        let mut param_map = HashMap::new();
        for param in &self.parameters {
            param.validate()?;
            if param_map
                .insert(param.key.as_str(), &param.param_type)
                .is_some()
            {
                return Err(DomainError::Validation(format!(
                    "Duplicate parameter key '{}' in template '{}'",
                    param.key, self.id
                )));
            }
        }

        // Validate bindings against parameters and layers
        for binding in &self.bindings {
            binding.validate(&param_map, &layer_ids)?;
        }

        self.resources.validate()?;
        self.responsive_policy.validate()?;

        if self.default_duration.value <= 0 {
            return Err(DomainError::NonPositiveDuration);
        }

        Ok(())
    }

    /// Evaluates current parameter values and returns updated layers and primary text state.
    pub fn resolve_instance_layers(
        &self,
        param_values: &HashMap<String, TemplateParameterValue>,
    ) -> Vec<TemplateLayer> {
        let mut layers = self.layers.clone();
        for binding in &self.bindings {
            if let Some(val) = param_values.get(&binding.parameter_key) {
                if let Some(layer) = layers
                    .iter_mut()
                    .find(|l| l.layer_id == binding.target_layer_id)
                {
                    apply_property_to_layer(layer, binding.target_property, val);
                }
            }
        }
        layers
    }

    /// Creates an initial TemplateInstance from this definition with default parameters.
    pub fn to_instance(&self) -> TemplateInstance {
        let mut param_map = HashMap::new();
        for p in &self.parameters {
            param_map.insert(p.key.clone(), p.default_value.clone());
        }
        let resolved_layers = self.resolve_instance_layers(&param_map);
        TemplateInstance {
            template_id: self.id.clone(),
            template_version: self.version,
            layers: resolved_layers,
            parameters: param_map.clone(),
            initial_parameters: param_map,
            overrides: Vec::new(),
            responsive_policy: Some(self.responsive_policy.clone()),
        }
    }
}

/// Applies a whitelisted property to a layer.
pub fn apply_property_to_layer(
    layer: &mut TemplateLayer,
    property: TemplatePropertyPath,
    value: &TemplateParameterValue,
) {
    match (property, value) {
        (TemplatePropertyPath::PositionX, TemplateParameterValue::Number(n)) => {
            layer.transform.position_x = *n as f32;
        }
        (TemplatePropertyPath::PositionY, TemplateParameterValue::Number(n)) => {
            layer.transform.position_y = *n as f32;
        }
        (TemplatePropertyPath::ScaleX, TemplateParameterValue::Number(n)) => {
            layer.transform.scale_x = *n as f32;
        }
        (TemplatePropertyPath::ScaleY, TemplateParameterValue::Number(n)) => {
            layer.transform.scale_y = *n as f32;
        }
        (TemplatePropertyPath::Rotation, TemplateParameterValue::Number(n)) => {
            layer.transform.rotation = *n as f32;
        }
        (TemplatePropertyPath::Opacity, TemplateParameterValue::Number(n)) => {
            layer.opacity = (*n as f32).clamp(0.0, 1.0);
        }
        (TemplatePropertyPath::Visible, TemplateParameterValue::Boolean(b)) => {
            layer.visible = *b;
        }
        (TemplatePropertyPath::TextContent, TemplateParameterValue::Text(s)) => {
            if let TemplateLayerKind::Text(t) = &mut layer.kind {
                t.text_content = Some(s.clone());
            }
        }
        (TemplatePropertyPath::FontColor, TemplateParameterValue::Color(c)) => {
            if let TemplateLayerKind::Text(t) = &mut layer.kind {
                if t.text_style.is_none() {
                    t.text_style = Some(crate::modules::domain::TextStyle::default());
                }
                if let Some(style) = &mut t.text_style {
                    style.font_color = c.clone();
                }
                if let Some(rich) = &mut t.text_rich {
                    rich.fill = TextFill::Solid { color: c.clone() };
                }
            }
        }
        (TemplatePropertyPath::FontSize, TemplateParameterValue::Number(n)) => {
            if let TemplateLayerKind::Text(t) = &mut layer.kind {
                let size = (*n as f32).max(1.0);
                if t.text_style.is_none() {
                    t.text_style = Some(crate::modules::domain::TextStyle::default());
                }
                if let Some(style) = &mut t.text_style {
                    style.font_size = size;
                }
                if let Some(rich) = &mut t.text_rich {
                    rich.font_size = Some(size);
                }
            }
        }
        (TemplatePropertyPath::FontFamily, TemplateParameterValue::Text(f)) => {
            if let TemplateLayerKind::Text(t) = &mut layer.kind {
                if t.text_style.is_none() {
                    t.text_style = Some(crate::modules::domain::TextStyle::default());
                }
                if let Some(style) = &mut t.text_style {
                    style.font_family = f.clone();
                }
                if let Some(rich) = &mut t.text_rich {
                    rich.font = Some(FontReference {
                        family: f.clone(),
                        weight: 700,
                        style: FontStyle::Normal,
                        asset_id: None,
                    });
                }
            }
        }
        (TemplatePropertyPath::BackgroundColor, TemplateParameterValue::Color(c)) => {
            if let TemplateLayerKind::Text(t) = &mut layer.kind {
                if t.text_style.is_none() {
                    t.text_style = Some(crate::modules::domain::TextStyle::default());
                }
                if let Some(style) = &mut t.text_style {
                    style.background_color = Some(c.clone());
                    style.background_opacity = 1.0;
                }
            }
        }
        (TemplatePropertyPath::FillColor, TemplateParameterValue::Color(c)) => {
            if let TemplateLayerKind::Shape(s) = &mut layer.kind {
                s.fill_color = Some(c.clone());
            }
        }
        (TemplatePropertyPath::StrokeColor, TemplateParameterValue::Color(c)) => {
            if let TemplateLayerKind::Shape(s) = &mut layer.kind {
                s.stroke_color = Some(c.clone());
            }
        }
        (TemplatePropertyPath::StrokeWidth, TemplateParameterValue::Number(n)) => {
            if let TemplateLayerKind::Shape(s) = &mut layer.kind {
                s.stroke_width = (*n as f32).max(0.0);
            }
        }
        (TemplatePropertyPath::CornerRadius, TemplateParameterValue::Number(n)) => {
            if let TemplateLayerKind::Shape(s) = &mut layer.kind {
                s.corner_radius = (*n as f32).max(0.0);
            }
        }
        (TemplatePropertyPath::Width, TemplateParameterValue::Number(n)) => match &mut layer.kind {
            TemplateLayerKind::Shape(s) => s.width = (*n as f32).max(0.0),
            TemplateLayerKind::Image(i) => i.width = (*n as f32).max(0.0),
            _ => {}
        },
        (TemplatePropertyPath::Height, TemplateParameterValue::Number(n)) => {
            match &mut layer.kind {
                TemplateLayerKind::Shape(s) => s.height = (*n as f32).max(0.0),
                TemplateLayerKind::Image(i) => i.height = (*n as f32).max(0.0),
                _ => {}
            }
        }
        _ => {}
    }
}

/// A resolved template instance attached additively to an owning TimelineItem.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateInstance {
    pub template_id: String,
    pub template_version: u32,
    #[serde(default)]
    pub layers: Vec<TemplateLayer>,
    #[serde(default)]
    pub parameters: HashMap<String, TemplateParameterValue>,
    #[serde(default)]
    pub initial_parameters: HashMap<String, TemplateParameterValue>,
    #[serde(default)]
    pub overrides: Vec<TemplateOverride>,
    #[serde(default)]
    pub responsive_policy: Option<ResponsivePolicy>,
}

impl TemplateInstance {
    pub fn validate(&self) -> Result<(), DomainError> {
        if !is_valid_id(&self.template_id) {
            return Err(DomainError::InvalidId(self.template_id.clone()));
        }
        if self.template_version == 0 {
            return Err(DomainError::Validation(
                "Template instance version must be positive".to_string(),
            ));
        }
        if self.layers.len() > MAX_TEMPLATE_LAYERS {
            return Err(DomainError::Validation(format!(
                "Template instance exceeds maximum allowed layers ({MAX_TEMPLATE_LAYERS}): found {}",
                self.layers.len()
            )));
        }

        let mut layer_ids = HashSet::new();
        let mut text_slots = 0;
        let mut primary_count = 0;
        for layer in &self.layers {
            layer.validate()?;
            if !layer_ids.insert(layer.layer_id.as_str()) {
                return Err(DomainError::Validation(format!(
                    "Duplicate layer ID '{}' in template instance",
                    layer.layer_id
                )));
            }
            if let TemplateLayerKind::Text(t) = &layer.kind {
                text_slots += 1;
                if t.is_primary {
                    primary_count += 1;
                }
            }
        }

        if text_slots > MAX_TEMPLATE_TEXT_SLOTS {
            return Err(DomainError::Validation(format!(
                "Template instance exceeds maximum text slots ({MAX_TEMPLATE_TEXT_SLOTS}): found {text_slots}"
            )));
        }
        if primary_count > 1 {
            return Err(DomainError::Validation(format!(
                "Template instance has {primary_count} primary text layers; at most one primary slot is allowed"
            )));
        }

        for ovr in &self.overrides {
            if !layer_ids.contains(ovr.target_layer_id.as_str()) {
                return Err(DomainError::Validation(format!(
                    "Override references unknown target layer '{}'",
                    ovr.target_layer_id
                )));
            }
        }

        Ok(())
    }
}
