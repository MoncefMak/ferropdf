use ferropdf_core::{Color, Rect, BorderStyle};

/// A drawing operation for rendering.
#[derive(Debug, Clone)]
pub enum DrawOp {
    /// Fill a rectangle with a color
    FillRect {
        rect: Rect,
        color: Color,
        border_radius: [f32; 4],
    },
    /// Draw a border around a rectangle
    StrokeRect {
        rect: Rect,
        color: Color,
        width: f32,
        style: BorderStyle,
    },
    /// Draw text at a position
    DrawText {
        text: String,
        x: f32,
        y: f32,
        font_size: f32,
        color: Color,
        font_family: Vec<String>,
        bold: bool,
        italic: bool,
        text_align: ferropdf_core::TextAlign,
        container_width: f32,
    },
    /// Draw an image
    DrawImage {
        src: String,
        rect: Rect,
    },
    /// Save graphics state
    Save,
    /// Restore graphics state
    Restore,
    /// Set clip rect
    ClipRect {
        rect: Rect,
    },
    /// Set opacity
    SetOpacity(f32),
}

/// A display list for one page.
#[derive(Debug, Clone)]
pub struct PageDisplayList {
    pub ops: Vec<DrawOp>,
    pub page_number: u32,
    pub total_pages: u32,
}
