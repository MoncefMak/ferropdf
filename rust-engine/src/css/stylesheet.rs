//! CSS stylesheet representation.

use std::collections::HashMap;

use super::properties::CssProperty;
use super::selector::Selector;
use super::values::CssValue;

/// A single CSS rule (selector + declarations).
#[derive(Debug, Clone)]
pub struct CssRule {
    /// The selector(s) for this rule.
    pub selectors: Vec<Selector>,
    /// The declarations (property-value pairs).
    pub declarations: Vec<Declaration>,
}

/// A CSS declaration (property: value).
#[derive(Debug, Clone)]
pub struct Declaration {
    pub property: CssProperty,
    pub value: CssValue,
    pub important: bool,
}

impl Declaration {
    pub fn new(property: CssProperty, value: CssValue) -> Self {
        Self {
            property,
            value,
            important: false,
        }
    }

    pub fn important(property: CssProperty, value: CssValue) -> Self {
        Self {
            property,
            value,
            important: true,
        }
    }
}

/// A CSS @page rule for page-specific styling.
#[derive(Debug, Clone)]
pub struct PageRule {
    pub selector: Option<String>, // e.g., ":first", ":left", ":right"
    pub declarations: Vec<Declaration>,
    pub margin_rules: Vec<MarginRule>,
}

/// A margin rule within @page (e.g., @top-center { content: "Header" }).
#[derive(Debug, Clone)]
pub struct MarginRule {
    pub position: MarginPosition,
    pub declarations: Vec<Declaration>,
}

/// Positions for @page margin rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarginPosition {
    TopLeft,
    TopCenter,
    TopRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
    LeftTop,
    LeftMiddle,
    LeftBottom,
    RightTop,
    RightMiddle,
    RightBottom,
}

impl MarginPosition {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "top-left" => Some(Self::TopLeft),
            "top-center" => Some(Self::TopCenter),
            "top-right" => Some(Self::TopRight),
            "bottom-left" => Some(Self::BottomLeft),
            "bottom-center" => Some(Self::BottomCenter),
            "bottom-right" => Some(Self::BottomRight),
            "left-top" => Some(Self::LeftTop),
            "left-middle" => Some(Self::LeftMiddle),
            "left-bottom" => Some(Self::LeftBottom),
            "right-top" => Some(Self::RightTop),
            "right-middle" => Some(Self::RightMiddle),
            "right-bottom" => Some(Self::RightBottom),
            _ => None,
        }
    }
}

/// A complete CSS stylesheet.
#[derive(Debug, Clone)]
pub struct Stylesheet {
    /// Regular CSS rules.
    pub rules: Vec<CssRule>,
    /// @page rules.
    pub page_rules: Vec<PageRule>,
    /// @font-face rules.
    pub font_face_rules: Vec<FontFaceRule>,
    /// Custom CSS properties (CSS variables) declared at :root or global scope.
    /// Key includes the `--` prefix, value is the raw string.
    pub custom_properties: HashMap<String, String>,
}

/// A @font-face rule.
#[derive(Debug, Clone)]
pub struct FontFaceRule {
    pub family: String,
    pub src: String,
    pub weight: Option<String>,
    pub style: Option<String>,
}

impl Stylesheet {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            page_rules: Vec::new(),
            font_face_rules: Vec::new(),
            custom_properties: HashMap::new(),
        }
    }

    /// Merge another stylesheet into this one.
    pub fn merge(&mut self, other: Stylesheet) {
        self.rules.extend(other.rules);
        self.page_rules.extend(other.page_rules);
        self.font_face_rules.extend(other.font_face_rules);
        self.custom_properties.extend(other.custom_properties);
    }

    /// Get the total number of rules.
    pub fn rule_count(&self) -> usize {
        self.rules.len() + self.page_rules.len() + self.font_face_rules.len()
    }
}

impl Default for Stylesheet {
    fn default() -> Self {
        Self::new()
    }
}

/// Default browser-like stylesheet for HTML elements.
pub fn default_stylesheet() -> Stylesheet {
    let mut stylesheet = Stylesheet::new();

    // Block-level elements
    let block_tags = [
        "html",
        "body",
        "div",
        "article",
        "section",
        "nav",
        "aside",
        "header",
        "footer",
        "main",
        "figure",
        "figcaption",
        "details",
        "summary",
    ];
    for tag in &block_tags {
        stylesheet.rules.push(CssRule {
            selectors: vec![Selector::Type(tag.to_string())],
            declarations: vec![Declaration::new(
                CssProperty::Display,
                CssValue::Keyword("block".to_string()),
            )],
        });
    }

    // Headings
    let heading_sizes = [
        ("h1", 2.0_f64),
        ("h2", 1.5),
        ("h3", 1.17),
        ("h4", 1.0),
        ("h5", 0.83),
        ("h6", 0.67),
    ];
    for (tag, scale) in &heading_sizes {
        stylesheet.rules.push(CssRule {
            selectors: vec![Selector::Type(tag.to_string())],
            declarations: vec![
                Declaration::new(CssProperty::Display, CssValue::Keyword("block".to_string())),
                Declaration::new(
                    CssProperty::FontWeight,
                    CssValue::Keyword("bold".to_string()),
                ),
                Declaration::new(
                    CssProperty::FontSize,
                    CssValue::Length(super::values::Length::em(*scale)),
                ),
                Declaration::new(
                    CssProperty::MarginTop,
                    CssValue::Length(super::values::Length::em(0.67)),
                ),
                Declaration::new(
                    CssProperty::MarginBottom,
                    CssValue::Length(super::values::Length::em(0.67)),
                ),
            ],
        });
    }

    // Paragraph
    stylesheet.rules.push(CssRule {
        selectors: vec![Selector::Type("p".to_string())],
        declarations: vec![
            Declaration::new(CssProperty::Display, CssValue::Keyword("block".to_string())),
            Declaration::new(
                CssProperty::MarginTop,
                CssValue::Length(super::values::Length::em(1.0)),
            ),
            Declaration::new(
                CssProperty::MarginBottom,
                CssValue::Length(super::values::Length::em(1.0)),
            ),
        ],
    });

    // Strong/bold
    stylesheet.rules.push(CssRule {
        selectors: vec![
            Selector::Type("strong".to_string()),
            Selector::Type("b".to_string()),
        ],
        declarations: vec![Declaration::new(
            CssProperty::FontWeight,
            CssValue::Keyword("bold".to_string()),
        )],
    });

    // Italic
    stylesheet.rules.push(CssRule {
        selectors: vec![
            Selector::Type("em".to_string()),
            Selector::Type("i".to_string()),
        ],
        declarations: vec![Declaration::new(
            CssProperty::FontStyle,
            CssValue::Keyword("italic".to_string()),
        )],
    });

    // Inline elements
    let inline_tags = [
        "span", "a", "strong", "em", "b", "i", "code", "small", "sub", "sup",
    ];
    for tag in &inline_tags {
        stylesheet.rules.push(CssRule {
            selectors: vec![Selector::Type(tag.to_string())],
            declarations: vec![Declaration::new(
                CssProperty::Display,
                CssValue::Keyword("inline".to_string()),
            )],
        });
    }

    // Table elements
    stylesheet.rules.push(CssRule {
        selectors: vec![Selector::Type("table".to_string())],
        declarations: vec![
            Declaration::new(CssProperty::Display, CssValue::Keyword("table".to_string())),
            Declaration::new(
                CssProperty::BorderCollapse,
                CssValue::Keyword("separate".to_string()),
            ),
        ],
    });

    stylesheet.rules.push(CssRule {
        selectors: vec![Selector::Type("tr".to_string())],
        declarations: vec![Declaration::new(
            CssProperty::Display,
            CssValue::Keyword("table-row".to_string()),
        )],
    });

    stylesheet.rules.push(CssRule {
        selectors: vec![
            Selector::Type("td".to_string()),
            Selector::Type("th".to_string()),
        ],
        declarations: vec![
            Declaration::new(
                CssProperty::Display,
                CssValue::Keyword("table-cell".to_string()),
            ),
            Declaration::new(
                CssProperty::PaddingTop,
                CssValue::Length(super::values::Length::px(1.0)),
            ),
            Declaration::new(
                CssProperty::PaddingBottom,
                CssValue::Length(super::values::Length::px(1.0)),
            ),
        ],
    });

    // Image
    stylesheet.rules.push(CssRule {
        selectors: vec![Selector::Type("img".to_string())],
        declarations: vec![Declaration::new(
            CssProperty::Display,
            CssValue::Keyword("inline-block".to_string()),
        )],
    });

    // Unordered/ordered lists
    for tag in &["ul", "ol"] {
        stylesheet.rules.push(CssRule {
            selectors: vec![Selector::Type(tag.to_string())],
            declarations: vec![
                Declaration::new(CssProperty::Display, CssValue::Keyword("block".to_string())),
                Declaration::new(
                    CssProperty::PaddingLeft,
                    CssValue::Length(super::values::Length::px(40.0)),
                ),
                Declaration::new(
                    CssProperty::MarginTop,
                    CssValue::Length(super::values::Length::em(1.0)),
                ),
                Declaration::new(
                    CssProperty::MarginBottom,
                    CssValue::Length(super::values::Length::em(1.0)),
                ),
            ],
        });
    }

    stylesheet.rules.push(CssRule {
        selectors: vec![Selector::Type("li".to_string())],
        declarations: vec![Declaration::new(
            CssProperty::Display,
            CssValue::Keyword("list-item".to_string()),
        )],
    });

    stylesheet
}
