pub mod media;
mod parser;

pub use media::{media_matches, MediaContext};
pub use parser::{
    parse_inline_declarations, parse_stylesheet, CssProperty, CssValue, Declaration, FontFaceRule,
    MarginBoxRule, PageRule, PageSelector, StyleRule, Stylesheet,
};

/// User-agent stylesheet embarqué
pub const UA_CSS: &str = include_str!("ua.css");
