//! ferropdf-render — display list abstraction, painter, PDF writer.

pub mod display_list;
pub mod painter;
pub mod pdf;

pub use display_list::{DrawOp, Color as DrawColor};
pub use painter::Painter;
pub use pdf::PdfRenderer;
