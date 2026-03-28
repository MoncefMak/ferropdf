pub mod color;
pub mod dom;
pub mod error;
pub mod geometry;
pub mod layout;
pub mod length;
pub mod page;
pub mod style;

/// Maximum DOM nesting depth before recursive functions bail out.
/// Prevents stack overflow from maliciously deep HTML documents.
pub const MAX_DOM_DEPTH: usize = 256;

// Re-exports publics
pub use color::Color;
pub use dom::{Document, Node, NodeId, NodeType};
pub use error::{FerroError, RenderWarning, Result};
pub use geometry::{Insets, Point, Rect, Size};
pub use layout::{InlineSpan, LayoutBox, LayoutTree, ShapedGlyph, ShapedLine, ShapedSegment};
pub use length::Length;
pub use page::{Orientation, PageConfig, PageMargins, PageSize};
pub use style::{
    AlignItems, AlignSelf, BorderCollapse, BorderRadius, BorderSide, BorderStyle,
    BoxDecorationBreak, ComputedStyle, Display, FlexDirection, FlexWrap, FontStyle, FontWeight,
    JustifyContent, ListStyleType, PageBreak, PageBreakInside, Position, TextAlign, TextDecoration,
};
