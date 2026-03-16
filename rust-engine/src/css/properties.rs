//! CSS properties and computed styles.

use std::collections::HashMap;

use super::values::{Color, CssValue, Length};

/// Enumeration of supported CSS properties.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CssProperty {
    // Display and layout
    Display,
    Position,
    Float,
    Clear,
    Overflow,
    BoxSizing,

    // Flexbox
    FlexDirection,
    FlexWrap,
    JustifyContent,
    AlignItems,
    AlignSelf,
    FlexGrow,
    FlexShrink,
    FlexBasis,
    Gap,

    // Dimensions
    Width,
    Height,
    MinWidth,
    MinHeight,
    MaxWidth,
    MaxHeight,

    // Margin
    MarginTop,
    MarginRight,
    MarginBottom,
    MarginLeft,

    // Padding
    PaddingTop,
    PaddingRight,
    PaddingBottom,
    PaddingLeft,

    // Position
    Top,
    Right,
    Bottom,
    Left,

    // Border
    BorderTopWidth,
    BorderRightWidth,
    BorderBottomWidth,
    BorderLeftWidth,
    BorderTopStyle,
    BorderRightStyle,
    BorderBottomStyle,
    BorderLeftStyle,
    BorderTopColor,
    BorderRightColor,
    BorderBottomColor,
    BorderLeftColor,
    BorderRadius,

    // Typography
    FontFamily,
    FontSize,
    FontWeight,
    FontStyle,
    LineHeight,
    TextAlign,
    TextDecoration,
    TextTransform,
    LetterSpacing,
    WordSpacing,
    WhiteSpace,

    // Colors and backgrounds
    Color,
    BackgroundColor,
    BackgroundImage,
    BackgroundSize,
    BackgroundPosition,
    BackgroundRepeat,
    Opacity,

    // Lists
    ListStyleType,
    ListStylePosition,

    // Tables
    BorderCollapse,
    BorderSpacing,
    TableLayout,

    // Page break (for PDF)
    PageBreakBefore,
    PageBreakAfter,
    PageBreakInside,
    BreakBefore,
    BreakAfter,
    BreakInside,

    // Text direction
    Direction,
    UnicodeBidi,

    // Other
    VerticalAlign,
    ZIndex,
    Visibility,
    TextIndent,
    ObjectFit,

    /// Custom/unknown property
    Custom(String),
}

impl CssProperty {
    /// Parse a property name string into a CssProperty.
    pub fn from_name(name: &str) -> Self {
        match name.trim().to_lowercase().as_str() {
            "display" => Self::Display,
            "position" => Self::Position,
            "float" => Self::Float,
            "clear" => Self::Clear,
            "overflow" => Self::Overflow,
            "box-sizing" => Self::BoxSizing,
            "flex-direction" => Self::FlexDirection,
            "flex-wrap" => Self::FlexWrap,
            "justify-content" => Self::JustifyContent,
            "align-items" => Self::AlignItems,
            "align-self" => Self::AlignSelf,
            "flex-grow" => Self::FlexGrow,
            "flex-shrink" => Self::FlexShrink,
            "flex-basis" => Self::FlexBasis,
            "gap" => Self::Gap,
            "width" => Self::Width,
            "height" => Self::Height,
            "min-width" => Self::MinWidth,
            "min-height" => Self::MinHeight,
            "max-width" => Self::MaxWidth,
            "max-height" => Self::MaxHeight,
            "margin-top" => Self::MarginTop,
            "margin-right" => Self::MarginRight,
            "margin-bottom" => Self::MarginBottom,
            "margin-left" => Self::MarginLeft,
            "padding-top" => Self::PaddingTop,
            "padding-right" => Self::PaddingRight,
            "padding-bottom" => Self::PaddingBottom,
            "padding-left" => Self::PaddingLeft,
            "top" => Self::Top,
            "right" => Self::Right,
            "bottom" => Self::Bottom,
            "left" => Self::Left,
            "border-top-width" => Self::BorderTopWidth,
            "border-right-width" => Self::BorderRightWidth,
            "border-bottom-width" => Self::BorderBottomWidth,
            "border-left-width" => Self::BorderLeftWidth,
            "border-top-style" => Self::BorderTopStyle,
            "border-right-style" => Self::BorderRightStyle,
            "border-bottom-style" => Self::BorderBottomStyle,
            "border-left-style" => Self::BorderLeftStyle,
            "border-top-color" => Self::BorderTopColor,
            "border-right-color" => Self::BorderRightColor,
            "border-bottom-color" => Self::BorderBottomColor,
            "border-left-color" => Self::BorderLeftColor,
            "border-radius" => Self::BorderRadius,
            "font-family" => Self::FontFamily,
            "font-size" => Self::FontSize,
            "font-weight" => Self::FontWeight,
            "font-style" => Self::FontStyle,
            "line-height" => Self::LineHeight,
            "text-align" => Self::TextAlign,
            "text-decoration" => Self::TextDecoration,
            "text-transform" => Self::TextTransform,
            "letter-spacing" => Self::LetterSpacing,
            "word-spacing" => Self::WordSpacing,
            "white-space" => Self::WhiteSpace,
            "color" => Self::Color,
            "background-color" => Self::BackgroundColor,
            "background-image" => Self::BackgroundImage,
            "background-size" => Self::BackgroundSize,
            "background-position" => Self::BackgroundPosition,
            "background-repeat" => Self::BackgroundRepeat,
            "opacity" => Self::Opacity,
            "list-style-type" => Self::ListStyleType,
            "list-style-position" => Self::ListStylePosition,
            "border-collapse" => Self::BorderCollapse,
            "border-spacing" => Self::BorderSpacing,
            "table-layout" => Self::TableLayout,
            "page-break-before" => Self::PageBreakBefore,
            "page-break-after" => Self::PageBreakAfter,
            "page-break-inside" => Self::PageBreakInside,
            "break-before" => Self::BreakBefore,
            "break-after" => Self::BreakAfter,
            "break-inside" => Self::BreakInside,
            "vertical-align" => Self::VerticalAlign,
            "z-index" => Self::ZIndex,
            "visibility" => Self::Visibility,
            "text-indent" => Self::TextIndent,
            "direction" => Self::Direction,
            "unicode-bidi" => Self::UnicodeBidi,
            "object-fit" => Self::ObjectFit,
            other => Self::Custom(other.to_string()),
        }
    }

    /// Check if this property is inherited by default.
    pub fn is_inherited(&self) -> bool {
        matches!(
            self,
            Self::Color
                | Self::FontFamily
                | Self::FontSize
                | Self::FontWeight
                | Self::FontStyle
                | Self::LineHeight
                | Self::TextAlign
                | Self::TextDecoration
                | Self::TextTransform
                | Self::LetterSpacing
                | Self::WordSpacing
                | Self::WhiteSpace
                | Self::ListStyleType
                | Self::ListStylePosition
                | Self::Visibility
                | Self::TextIndent
                | Self::Direction
                | Self::BorderCollapse
                | Self::BorderSpacing
        )
    }
}

/// Computed style for a single element.
/// Contains all resolved CSS property values.
#[derive(Debug, Clone)]
pub struct ComputedStyle {
    pub properties: HashMap<CssProperty, CssValue>,
}

impl ComputedStyle {
    pub fn new() -> Self {
        Self {
            properties: HashMap::new(),
        }
    }

    /// Get a property value.
    pub fn get(&self, prop: &CssProperty) -> Option<&CssValue> {
        self.properties.get(prop)
    }

    /// Set a property value.
    pub fn set(&mut self, prop: CssProperty, value: CssValue) {
        self.properties.insert(prop, value);
    }

    /// Get display type (defaults to "block").
    pub fn display(&self) -> &str {
        self.get(&CssProperty::Display)
            .and_then(|v| match v {
                CssValue::Keyword(s) => Some(s.as_str()),
                _ => None,
            })
            .unwrap_or("block")
    }

    /// Check if the element is hidden.
    pub fn is_hidden(&self) -> bool {
        self.display() == "none"
            || self
                .get(&CssProperty::Visibility)
                .map(|v| v.to_string() == "hidden")
                .unwrap_or(false)
    }

    /// Get the font size in px (defaults to 16.0).
    pub fn font_size_px(&self, parent_font_size: f64, root_font_size: f64) -> f64 {
        self.get(&CssProperty::FontSize)
            .and_then(|v| v.as_px(parent_font_size, parent_font_size, root_font_size))
            .unwrap_or(16.0)
    }

    /// Get the font weight (defaults to 400).
    pub fn font_weight(&self) -> u32 {
        self.get(&CssProperty::FontWeight)
            .map(|v| match v {
                CssValue::Number(n) => *n as u32,
                CssValue::Keyword(s) => match s.as_str() {
                    "bold" => 700,
                    "bolder" => 700,
                    "lighter" => 300,
                    "normal" => 400,
                    _ => s.parse::<u32>().unwrap_or(400),
                },
                _ => 400,
            })
            .unwrap_or(400)
    }

    /// Get the text color (defaults to black).
    pub fn color(&self) -> Color {
        self.get(&CssProperty::Color)
            .and_then(|v| v.as_color())
            .unwrap_or_default()
    }

    /// Get background color (defaults to transparent).
    pub fn background_color(&self) -> Color {
        self.get(&CssProperty::BackgroundColor)
            .and_then(|v| v.as_color())
            .unwrap_or(Color::transparent())
    }

    /// Get font family (defaults to "serif").
    pub fn font_family(&self) -> &str {
        self.get(&CssProperty::FontFamily)
            .and_then(|v| match v {
                CssValue::Keyword(s) | CssValue::String(s) => Some(s.as_str()),
                _ => None,
            })
            .unwrap_or("serif")
    }

    /// Get text direction (defaults to "ltr").
    pub fn direction(&self) -> &str {
        self.get(&CssProperty::Direction)
            .and_then(|v| match v {
                CssValue::Keyword(s) => Some(s.as_str()),
                _ => None,
            })
            .unwrap_or("ltr")
    }

    /// Check if text direction is RTL.
    pub fn is_rtl(&self) -> bool {
        self.direction() == "rtl"
    }

    /// Get text alignment.
    ///
    /// Defaults to "right" when direction is RTL, "left" for LTR.
    /// Also resolves the CSS `start` / `end` keywords based on direction.
    pub fn text_align(&self) -> &str {
        let explicit = self.get(&CssProperty::TextAlign).and_then(|v| match v {
            CssValue::Keyword(s) => Some(s.as_str()),
            _ => None,
        });

        match explicit {
            Some("start") | None => {
                if self.is_rtl() {
                    "right"
                } else {
                    "left"
                }
            }
            Some("end") => {
                if self.is_rtl() {
                    "left"
                } else {
                    "right"
                }
            }
            Some(s) => s,
        }
    }

    /// Get line height factor.
    pub fn line_height(&self, font_size: f64) -> f64 {
        self.get(&CssProperty::LineHeight)
            .map(|v| match v {
                CssValue::Number(n) => *n * font_size,
                CssValue::Length(l) => l.to_px(font_size, font_size, font_size),
                CssValue::Percentage(p) => p / 100.0 * font_size,
                CssValue::Keyword(s) if s == "normal" => font_size * 1.2,
                _ => font_size * 1.2,
            })
            .unwrap_or(font_size * 1.2)
    }

    /// Check if page break before is requested.
    pub fn has_page_break_before(&self) -> bool {
        self.get(&CssProperty::PageBreakBefore)
            .map(|v| matches!(v, CssValue::Keyword(s) if s == "always" || s == "page"))
            .unwrap_or(false)
            || self
                .get(&CssProperty::BreakBefore)
                .map(|v| matches!(v, CssValue::Keyword(s) if s == "page" || s == "always"))
                .unwrap_or(false)
    }

    /// Check if page break after is requested.
    pub fn has_page_break_after(&self) -> bool {
        self.get(&CssProperty::PageBreakAfter)
            .map(|v| matches!(v, CssValue::Keyword(s) if s == "always" || s == "page"))
            .unwrap_or(false)
            || self
                .get(&CssProperty::BreakAfter)
                .map(|v| matches!(v, CssValue::Keyword(s) if s == "page" || s == "always"))
                .unwrap_or(false)
    }

    /// Check if page break inside should be avoided.
    pub fn avoid_page_break_inside(&self) -> bool {
        self.get(&CssProperty::PageBreakInside)
            .map(|v| matches!(v, CssValue::Keyword(s) if s == "avoid"))
            .unwrap_or(false)
            || self
                .get(&CssProperty::BreakInside)
                .map(|v| matches!(v, CssValue::Keyword(s) if s == "avoid"))
                .unwrap_or(false)
    }

    /// Get a length property, returning zero if unset.
    pub fn get_length(
        &self,
        prop: &CssProperty,
        parent_size: f64,
        font_size: f64,
        root_font_size: f64,
    ) -> f64 {
        self.get(prop)
            .and_then(|v| v.as_px(parent_size, font_size, root_font_size))
            .unwrap_or(0.0)
    }

    /// Create default styles for the root HTML element.
    pub fn default_root() -> Self {
        let mut style = Self::new();
        style.set(CssProperty::Display, CssValue::Keyword("block".to_string()));
        style.set(CssProperty::FontSize, CssValue::Length(Length::px(16.0)));
        style.set(CssProperty::FontWeight, CssValue::Number(400.0));
        style.set(
            CssProperty::FontFamily,
            CssValue::Keyword("serif".to_string()),
        );
        style.set(CssProperty::Color, CssValue::Color(Color::black()));
        style.set(
            CssProperty::BackgroundColor,
            CssValue::Color(Color::white()),
        );
        style.set(
            CssProperty::LineHeight,
            CssValue::Keyword("normal".to_string()),
        );
        // Note: TextAlign is NOT set here. The text_align() method will
        // return a direction-aware default ("left" for LTR, "right" for RTL).
        style
    }

    /// Merge inherited properties from parent style.
    pub fn inherit_from(&mut self, parent: &ComputedStyle) {
        for (prop, value) in &parent.properties {
            if prop.is_inherited() && !self.properties.contains_key(prop) {
                self.properties.insert(prop.clone(), value.clone());
            }
        }
    }
}

impl Default for ComputedStyle {
    fn default() -> Self {
        Self::new()
    }
}
