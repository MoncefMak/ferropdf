use ferropdf_core::{BorderStyle, BoxShadow, Color, Rect, ShapedGlyph};

/// A drawing operation for rendering.
#[derive(Debug, Clone)]
#[allow(dead_code)]
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
    /// Stroke all four sides of a box with rounded corners (uniform border).
    /// `rect` is the border box; the stroke is drawn inside it.
    StrokeRoundedRect {
        rect: Rect,
        color: Color,
        width: f32,
        style: BorderStyle,
        border_radius: [f32; 4],
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
        /// Pre-shaped glyphs from cosmic-text. When non-empty, these carry both
        /// glyph_id and font_id, so the PDF writer can embed the exact font binary
        /// that was used for shaping. Essential for Arabic/Hebrew ligatures and
        /// font fallback scenarios.
        shaped_glyphs: Vec<ShapedGlyph>,
    },
    /// Draw an image
    DrawImage { src: String, rect: Rect },
    /// Fill `rect` (rounded by `border_radius`) with a resolved gradient.
    FillGradient {
        rect: Rect,
        border_radius: [f32; 4],
        gradient: crate::gradient::ResolvedGradient,
    },
    /// Draw a box shadow (approximated as offset semi-transparent filled rects)
    DrawBoxShadow {
        rect: Rect,
        shadow: BoxShadow,
        border_radius: [f32; 4],
    },
    /// Save graphics state
    Save,
    /// Restore graphics state
    Restore,
    /// Set clip rect
    ClipRect {
        rect: Rect,
        /// Corner radii [top-left, top-right, bottom-right, bottom-left].
        border_radius: [f32; 4],
    },
    /// Set opacity
    SetOpacity(f32),
}

/// A display list for one page.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PageDisplayList {
    pub ops: Vec<DrawOp>,
    pub page_number: u32,
    pub total_pages: u32,
}
