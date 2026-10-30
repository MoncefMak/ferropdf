pub mod color;
pub mod dom;
pub mod error;
pub mod geometry;
pub mod gradient;
pub mod layout;
pub mod length;
pub mod page;
pub mod style;
pub mod transform;

/// Maximum DOM nesting depth before recursive functions bail out.
/// Prevents stack overflow from maliciously deep HTML documents.
pub const MAX_DOM_DEPTH: usize = 256;

// Re-exports publics
pub use color::Color;
pub use dom::{Document, Node, NodeId, NodeType};
pub use error::{FerroError, RenderWarning, Result};
pub use geometry::{Insets, Point, Rect, Size};
pub use gradient::{
    ColorStop, Gradient, GradientDirection, GradientLength, RadialShape, RadialSize,
};
pub use layout::{InlineSpan, LayoutBox, LayoutTree, ShapedGlyph, ShapedLine, ShapedSegment};
pub use length::{CalcLength, Length};
pub use page::{Orientation, PageConfig, PageMarginOverrides, PageMargins, PageSize};
pub use style::{
    AlignItems, AlignSelf, BorderCollapse, BorderRadius, BorderSide, BorderStyle,
    BoxDecorationBreak, BoxShadow, BoxSizing, Clear, ComputedStyle, Direction, Display,
    FlexDirection, FlexWrap, Float, FontStyle, FontWeight, GridProps, JustifyContent,
    ListStyleType, PageBreak, PageBreakInside, Position, TextAlign, TextDecoration, WhiteSpace,
};
pub use transform::{Matrix, TransformFunction};
