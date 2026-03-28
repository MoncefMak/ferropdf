pub mod css;
mod html;

pub use html::parse_html;
// Assure-toi d'avoir tes définitions CSS dans le module css/
pub use css::{
    parse_inline_declarations, parse_stylesheet, CssProperty, CssValue, Declaration, FontFaceRule,
    StyleRule, Stylesheet,
};

use ferropdf_core::{Document, Result};

pub struct ParseResult {
    pub document: Document,
    pub inline_styles: Vec<String>,
    pub external_stylesheets: Vec<String>,
}

pub fn parse(html: &str) -> Result<ParseResult> {
    html::parse_full(html)
}
