//! CSS value types and Stylesheet representation.

use std::collections::HashMap;
use ferropdf_core::Color;

// ─── Raw CSS values ───────────────────────────────────────────────────────────

/// A raw parsed CSS value (before cascade/compute).
#[derive(Debug, Clone, PartialEq)]
pub enum CssValue {
    Keyword(String),
    Length(CssLength),
    Percentage(f32),
    Number(f32),
    Integer(i32),
    Color(Color),
    Url(String),
    /// space-separated list  (e.g. `12px serif`)
    List(Vec<CssValue>),
    /// comma-separated list  (e.g. font-family fallbacks)
    CommaSep(Vec<CssValue>),
    None,
    Initial,
    Inherit,
    Unset,
}

impl CssValue {
    /// If this is a keyword, return it as a &str.
    pub fn keyword(&self) -> Option<&str> {
        match self { CssValue::Keyword(s) => Some(s), _ => None }
    }

    pub fn as_number(&self) -> Option<f32> {
        match self {
            CssValue::Number(v)  => Some(*v),
            CssValue::Integer(v) => Some(*v as f32),
            _ => None,
        }
    }

    pub fn is_none(&self) -> bool { matches!(self, CssValue::None) }
    pub fn is_inherit(&self) -> bool { matches!(self, CssValue::Inherit) }
    pub fn is_initial(&self) -> bool { matches!(self, CssValue::Initial) }
}

impl std::fmt::Display for CssValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CssValue::Keyword(s)    => write!(f, "{s}"),
            CssValue::Number(v)     => write!(f, "{v}"),
            CssValue::Integer(v)    => write!(f, "{v}"),
            CssValue::Percentage(v) => write!(f, "{v}%"),
            CssValue::Length(l)     => write!(f, "{l:?}"),
            CssValue::Color(c)      => write!(f, "rgba({},{},{},{})", c.r*255.0, c.g*255.0, c.b*255.0, c.a),
            CssValue::Url(u)        => write!(f, "url({u})"),
            CssValue::List(items)   => write!(f, "{}", items.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(" ")),
            CssValue::CommaSep(items) => write!(f, "{}", items.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(",")),
            CssValue::None          => write!(f, "none"),
            CssValue::Initial       => write!(f, "initial"),
            CssValue::Inherit       => write!(f, "inherit"),
            CssValue::Unset         => write!(f, "unset"),
        }
    }
}

/// A raw CSS length token.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CssLength {
    Px(f32),
    Mm(f32),
    Cm(f32),
    Pt(f32),
    Em(f32),
    Rem(f32),
    Percent(f32),
    Vw(f32),
    Vh(f32),
    Zero,
    Auto,
}

// ─── Selector ─────────────────────────────────────────────────────────────────

/// A parsed selector component.
#[derive(Debug, Clone, PartialEq)]
pub enum SelectorComponent {
    /// Matches any element (`*`).
    Universal,
    /// Tag name: `div`.
    Type(String),
    /// Class: `.foo`.
    Class(String),
    /// ID: `#bar`.
    Id(String),
    /// Attribute: `[type="text"]`.
    Attribute { name: String, op: AttrOp, value: String },
    /// Pseudo-class: `:first-child`.
    PseudoClass(String),
    /// Pseudo-element: `::before`.
    PseudoElement(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AttrOp {
    Exists,        // [attr]
    Equals,        // [attr=val]
    Includes,      // [attr~=val]
    DashMatch,     // [attr|=val]
    StartsWith,    // [attr^=val]
    EndsWith,      // [attr$=val]
    Contains,      // [attr*=val]
}

#[derive(Debug, Clone, PartialEq)]
pub enum Combinator {
    Descendant,   // (space)
    Child,        // >
    Adjacent,     // +
    Sibling,      // ~
}

/// A single fully-parsed CSS selector rule.
#[derive(Debug, Clone)]
pub struct Selector {
    /// Ordered list of (combinator, component-list) pairs.
    /// The first entry has Combinator=Descendant (ignored).
    pub parts: Vec<(Combinator, Vec<SelectorComponent>)>,
}

impl Selector {
    /// Simple type selector.
    pub fn type_selector(tag: impl Into<String>) -> Self {
        Self { parts: vec![(Combinator::Descendant, vec![SelectorComponent::Type(tag.into())])] }
    }
    pub fn class_selector(cls: impl Into<String>) -> Self {
        Self { parts: vec![(Combinator::Descendant, vec![SelectorComponent::Class(cls.into())])] }
    }
    pub fn id_selector(id: impl Into<String>) -> Self {
        Self { parts: vec![(Combinator::Descendant, vec![SelectorComponent::Id(id.into())])] }
    }
    pub fn universal() -> Self {
        Self { parts: vec![(Combinator::Descendant, vec![SelectorComponent::Universal])] }
    }
}

// ─── Declarations & Rules ─────────────────────────────────────────────────────

/// A single CSS `property: value [!important]` declaration.
#[derive(Debug, Clone)]
pub struct Declaration {
    pub property:  String,
    pub value:     CssValue,
    pub important: bool,
}

/// A regular style rule: `selector { decl; ... }`.
#[derive(Debug, Clone)]
pub struct StyleRule {
    pub selectors:    Vec<Selector>,
    pub declarations: Vec<Declaration>,
}

/// An `@font-face` rule.
#[derive(Debug, Clone)]
pub struct FontFaceRule {
    pub family: String,
    pub src:    String,
    pub weight: Option<String>,
    pub style:  Option<String>,
}

/// An `@page` rule.
#[derive(Debug, Clone)]
pub struct PageRule {
    /// `None` = anonymous @page, `Some(":first")` etc.
    pub selector:     Option<String>,
    pub declarations: Vec<Declaration>,
}

/// A parsed CSS stylesheet.
#[derive(Debug, Clone, Default)]
pub struct Stylesheet {
    pub rules:             Vec<StyleRule>,
    pub page_rules:        Vec<PageRule>,
    pub font_face_rules:   Vec<FontFaceRule>,
    /// CSS custom properties declared at :root or top level.
    pub custom_properties: HashMap<String, String>,
}

impl Stylesheet {
    pub fn new() -> Self { Self::default() }

    pub fn merge(&mut self, other: Stylesheet) {
        self.rules.extend(other.rules);
        self.page_rules.extend(other.page_rules);
        self.font_face_rules.extend(other.font_face_rules);
        self.custom_properties.extend(other.custom_properties);
    }
}
