//! CSS parser and style resolution module.
//!
//! Parses CSS stylesheets, resolves selectors, and computes final styles.

pub mod parser;
pub mod properties;
pub mod selector;
pub mod stylesheet;
pub mod values;

pub use parser::CssParser;
pub use properties::{ComputedStyle, CssProperty};
pub use stylesheet::{CssRule, Stylesheet};
pub use values::{Color, CssValue, Length, LengthUnit};
