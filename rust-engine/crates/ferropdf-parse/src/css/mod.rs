//! CSS parsing and style resolution.

pub mod cascade;
pub mod compute;
pub mod inherit;
pub mod matching;
pub mod parser;
pub mod properties;
pub mod resolver;
pub mod specificity;
pub mod values;

pub use properties::ComputedStyle;
pub use resolver::StyleResolver;
pub use parser::parse_stylesheet;
pub use values::Stylesheet;
