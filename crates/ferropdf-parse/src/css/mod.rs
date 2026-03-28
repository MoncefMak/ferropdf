mod parser;

pub use parser::{
    parse_inline_declarations, parse_stylesheet, CssProperty, CssValue, Declaration, FontFaceRule,
    StyleRule, Stylesheet,
};

/// User-agent stylesheet embarqué
pub const UA_CSS: &str = include_str!("ua.css");
