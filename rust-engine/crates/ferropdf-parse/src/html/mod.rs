//! HTML document object model and parser.

pub mod dom;
pub mod parser;

pub use dom::{Document, DomNode, NodeId, NodeKind, ElementData};
pub use parser::parse_html;
