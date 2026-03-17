//! Layout engine module.
//!
//! Takes a styled DOM tree and computes the geometric layout (box model)
//! for each element, including pagination for multi-page PDF output.

pub mod box_model;
pub mod engine;
pub mod pagination;
pub mod style_resolver;
pub mod table_layout;

pub use box_model::{BoxDimensions, EdgeSizes, FloatSide, LayoutBox, LayoutBoxType, PositionType, Rect};
pub use engine::LayoutEngine;
pub use pagination::{Page, PageLayout, PageSize};
