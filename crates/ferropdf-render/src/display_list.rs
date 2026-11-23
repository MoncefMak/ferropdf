use ferropdf_core::{BorderStyle, BoxShadow, Color, Rect, ShapedGlyph};
use ferropdf_core::{NodeId, Radii};

/// A drawing operation for rendering.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum DrawOp {
    /// Fill a rectangle with a color
    FillRect {
        rect: Rect,
        color: Color,
        border_radius: Radii,
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
        border_radius: Radii,
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
        border_radius: Radii,
        gradient: crate::gradient::ResolvedGradient,
    },
    /// Draw a box shadow (approximated as offset semi-transparent filled rects)
    DrawBoxShadow {
        rect: Rect,
        shadow: BoxShadow,
        border_radius: Radii,
    },
    /// Save graphics state
    Save,
    /// Restore graphics state
    Restore,
    /// Set clip rect
    ClipRect {
        rect: Rect,
        /// Corner radii [top-left, top-right, bottom-right, bottom-left].
        border_radius: Radii,
    },
    /// Set opacity
    SetOpacity(f32),
    /// A clickable area: an external URI, or `#name` for an anchor.
    /// `text` is the link's text, its accessible description.
    Link {
        rect: Rect,
        target: String,
        text: String,
    },
    /// The position (top of a box) a `#name` link jumps to.
    Anchor { name: String, y: f32 },
    /// A PDF outline (bookmark) entry; `key` identifies the heading so that
    /// fragments split across pages produce one entry.
    Bookmark {
        key: usize,
        level: u8,
        title: String,
        y: f32,
    },
    /// Concatenate an affine matrix `[a, b, c, d, e, f]` expressed in page
    /// coordinates (top-left origin, y down) to the current transform.
    Transform([f32; 6]),
    /// What the following ops draw, for tagged PDF (see `tagging`). Content
    /// ops (text, images) are tagged with their owner; decorations are
    /// artifacts.
    Owner(Owner),
}

/// The logical owner of drawing ops.
#[derive(Debug, Clone, PartialEq)]
pub enum Owner {
    /// Running headers, footers and page counters.
    Pagination,
    /// Content of a DOM node (a text node or anonymous box resolves to its
    /// nearest semantic ancestor).
    Node(Option<NodeId>),
    /// The marker of the list item `NodeId`.
    Label(NodeId),
    /// The part of a paragraph's text that is an inline `<a href>`.
    InlineLink {
        block: Option<NodeId>,
        target: String,
    },
}

/// A display list for one page.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PageDisplayList {
    pub ops: Vec<DrawOp>,
    pub page_number: u32,
    pub total_pages: u32,
}
