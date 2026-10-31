pub mod media;
mod parser;

pub use media::{media_matches, MediaContext};
pub use parser::{
    parse_inline_declarations, parse_stylesheet, CssProperty, CssValue, Declaration, FontFaceRule,
    MarginBoxRule, PageRule, PageSelector, StyleRule, Stylesheet,
};

/// User-agent stylesheet embarqué
pub const UA_CSS: &str = include_str!("ua.css");

/// The user-agent stylesheet, parsed once per process.
pub fn ua_stylesheet() -> &'static Stylesheet {
    static UA: std::sync::OnceLock<Stylesheet> = std::sync::OnceLock::new();
    UA.get_or_init(|| parse_stylesheet(UA_CSS).expect("the UA stylesheet parses"))
}

/// `css` parsed, reusing the cached UA stylesheet when `css` is [`UA_CSS`].
pub fn parse_ua(css: &str) -> crate::Result<std::borrow::Cow<'static, Stylesheet>> {
    if css == UA_CSS {
        Ok(std::borrow::Cow::Borrowed(ua_stylesheet()))
    } else {
        parse_stylesheet(css).map(std::borrow::Cow::Owned)
    }
}
