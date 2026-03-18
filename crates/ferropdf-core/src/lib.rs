pub mod color;
pub mod geometry;
pub mod length;
pub mod page;
pub mod dom;
pub mod style;
pub mod layout;
pub mod error;

// Re-exports publics
pub use color::Color;
pub use geometry::{Rect, Point, Size, Insets};
pub use length::Length;
pub use page::{PageSize, PageConfig, PageMargins, Orientation};
pub use dom::{Document, Node, NodeId, NodeType};
pub use style::{
    ComputedStyle, Display, Position, FontWeight, FontStyle,
    TextAlign, FlexDirection, FlexWrap, JustifyContent,
    AlignItems, AlignSelf, PageBreak, PageBreakInside, BorderSide, BorderStyle,
    BorderRadius, BoxDecorationBreak,
};
pub use layout::{LayoutBox, LayoutTree, ShapedLine, ShapedGlyph, BreakUnit};
pub use error::{FerroError, Result};
