pub mod color;
pub mod dom;
pub mod error;
pub mod geometry;
pub mod layout;
pub mod length;
pub mod page;
pub mod style;

// Re-exports publics
pub use color::Color;
pub use dom::{Document, Node, NodeId, NodeType};
pub use error::{FerroError, Result};
pub use geometry::{Insets, Point, Rect, Size};
pub use layout::{
    BreakUnit, InlineSpan, LayoutBox, LayoutTree, ShapedGlyph, ShapedLine, ShapedSegment,
};
pub use length::Length;
pub use page::{Orientation, PageConfig, PageMargins, PageSize};
pub use style::{
    AlignItems, AlignSelf, BorderRadius, BorderSide, BorderStyle, BoxDecorationBreak,
    ComputedStyle, Display, FlexDirection, FlexWrap, FontStyle, FontWeight, JustifyContent,
    PageBreak, PageBreakInside, Position, TextAlign,
};
