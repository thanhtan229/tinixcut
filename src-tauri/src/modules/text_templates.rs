//! Built-in declarative text templates.
//!
//! Templates are domain data, not renderer-specific instructions.  The command
//! service resolves a template into a persisted `TextStyle`, so preview and
//! export consume the same resolved values.

use crate::modules::domain::{TextAlignment, TextAnchor, TextStyle, TextTemplateReference};

#[derive(Debug, Clone, PartialEq)]
pub struct TextTemplateDefinition {
    pub id: &'static str,
    pub version: u32,
    pub name: &'static str,
    pub style: TextStyle,
}

// The built-in catalog intentionally spells out every user-visible style
// property at each call site so template definitions stay easy to audit.
#[allow(clippy::too_many_arguments)]
fn style(
    font_size: f32,
    font_weight: u16,
    font_color: &str,
    background_color: Option<&str>,
    background_opacity: f32,
    padding_x: f32,
    padding_y: f32,
    alignment: TextAlignment,
    anchor: TextAnchor,
) -> TextStyle {
    TextStyle {
        font_family: "Inter".to_string(),
        font_size,
        font_weight,
        font_color: font_color.to_string(),
        accent_color: None,
        background_color: background_color.map(str::to_string),
        background_opacity,
        padding_x,
        padding_y,
        alignment,
        line_height: 1.0,
        anchor,
    }
}

pub fn list_templates() -> Vec<TextTemplateDefinition> {
    vec![
        TextTemplateDefinition {
            id: "clean-lower-third",
            version: 1,
            name: "Clean lower third",
            style: style(
                42.0,
                700,
                "#FFFFFF",
                Some("#111827"),
                0.82,
                24.0,
                14.0,
                TextAlignment::Left,
                TextAnchor::BottomLeft,
            ),
        },
        TextTemplateDefinition {
            id: "bold-headline",
            version: 1,
            name: "Bold headline",
            style: style(
                72.0,
                800,
                "#FFFFFF",
                Some("#000000"),
                0.48,
                18.0,
                12.0,
                TextAlignment::Center,
                TextAnchor::TopCenter,
            ),
        },
        TextTemplateDefinition {
            id: "center-pop",
            version: 1,
            name: "Center pop",
            style: style(
                64.0,
                800,
                "#FFFFFF",
                Some("#7C3AED"),
                0.88,
                22.0,
                16.0,
                TextAlignment::Center,
                TextAnchor::Center,
            ),
        },
        TextTemplateDefinition {
            id: "subtitle-card",
            version: 1,
            name: "Subtitle card",
            style: style(
                36.0,
                600,
                "#FFFFFF",
                Some("#000000"),
                0.72,
                18.0,
                10.0,
                TextAlignment::Center,
                TextAnchor::BottomCenter,
            ),
        },
        TextTemplateDefinition {
            id: "quote-card",
            version: 1,
            name: "Quote card",
            style: style(
                48.0,
                500,
                "#F8FAFC",
                Some("#0F172A"),
                0.88,
                28.0,
                22.0,
                TextAlignment::Center,
                TextAnchor::Center,
            ),
        },
        TextTemplateDefinition {
            id: "call-to-action",
            version: 1,
            name: "Call to action",
            style: style(
                52.0,
                800,
                "#FFFFFF",
                Some("#DC2626"),
                0.92,
                24.0,
                14.0,
                TextAlignment::Center,
                TextAnchor::BottomCenter,
            ),
        },
    ]
}

pub fn resolve_template(reference: &TextTemplateReference) -> Option<TextStyle> {
    list_templates()
        .into_iter()
        .find(|template| {
            template.id == reference.template_id && template.version == reference.template_version
        })
        .map(|template| template.style)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_templates_are_valid_and_unique() {
        let templates = list_templates();
        assert_eq!(templates.len(), 6);

        for template in &templates {
            assert!(template.style.validate().is_ok(), "{}", template.id);
        }

        let mut ids = templates
            .iter()
            .map(|template| template.id)
            .collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), templates.len());
    }

    #[test]
    fn resolves_only_known_template_versions() {
        let known = TextTemplateReference {
            template_id: "center-pop".to_string(),
            template_version: 1,
        };
        assert_eq!(
            resolve_template(&known).expect("known template").anchor,
            TextAnchor::Center
        );

        let unknown = TextTemplateReference {
            template_id: "center-pop".to_string(),
            template_version: 2,
        };
        assert!(resolve_template(&unknown).is_none());
    }
}
