use crate::{Color, ComputedStyle, Insets, NodeId, Rect, TextDecoration};

/// A styled text span from inline element merging.
/// When a block container has all-inline children (text, <strong>, <em>, etc.),
/// their text is merged into a single paragraph with per-span styling.
#[derive(Debug, Clone)]
pub struct InlineSpan {
    pub text: String,
    pub font_size: f32,
    pub line_height: f32,
    pub font_family: String,
    pub bold: bool,
    pub italic: bool,
    pub color: Color,
    pub text_decoration: TextDecoration,
}

/// A styled segment within a shaped line (populated for rich/inline-merged text).
#[derive(Debug, Clone)]
pub struct ShapedSegment {
    pub text: String,
    pub x_offset: f32,
    pub width: f32,
    pub metadata: usize,
}

#[derive(Debug, Clone)]
pub struct ShapedGlyph {
    pub glyph_id: u16,
    /// The fontdb face ID that was used during shaping.
    /// This is the ONLY reliable way to find the exact font binary
    /// whose glyph table these glyph_ids refer to.
    pub font_id: fontdb::ID,
    pub x: f32,
    pub y: f32,
    pub advance: f32,
    pub metadata: usize,
}

#[derive(Debug, Clone)]
pub struct ShapedLine {
    pub glyphs: Vec<ShapedGlyph>,
    pub width: f32,
    pub y: f32,
    /// The text content of this line (for encoding in the PDF).
    pub text: String,
    /// Per-segment styling info (non-empty only for inline-merged rich text).
    pub segments: Vec<ShapedSegment>,
}

#[derive(Debug, Clone)]
pub struct LayoutBox {
    pub node_id: Option<NodeId>,
    pub style: ComputedStyle,
    /// Border-box rectangle (x, y, width, height) in absolute coordinates.
    pub rect: Rect,
    pub content: Rect,
    pub padding: Insets,
    pub border: Insets,
    pub margin: Insets,
    pub children: Vec<LayoutBox>,
    pub shaped_lines: Vec<ShapedLine>,
    /// Inline span styles for merged rich text.
    /// When non-empty, shaped_lines contain per-segment metadata referencing these spans.
    pub inline_spans: Vec<InlineSpan>,
    pub image_src: Option<String>,
    pub text_content: Option<String>,
    /// True if this box is absolutely positioned (out of normal flow).
    // Reserved for future position:absolute support. Always false currently.
    pub out_of_flow: bool,
    /// Visual offset from position: relative (does not affect flow).
    pub visual_offset_x: f32,
    pub visual_offset_y: f32,
    /// Table cell grid position (row, col, total_rows, total_cols) for border-collapse.
    pub table_cell_pos: Option<(usize, usize, usize, usize)>,
    /// Index of this list item within its parent list (1-based), for marker rendering.
    pub list_item_index: Option<usize>,
    /// For Display::Table boxes: number of rows that came from <thead> (for repeat on pagination).
    pub thead_row_count: usize,
}

impl Default for LayoutBox {
    fn default() -> Self {
        Self {
            node_id: None,
            style: ComputedStyle::default(),
            rect: Rect::zero(),
            content: Rect::zero(),
            padding: Insets::zero(),
            border: Insets::zero(),
            margin: Insets::zero(),
            children: Vec::new(),
            shaped_lines: Vec::new(),
            inline_spans: Vec::new(),
            image_src: None,
            text_content: None,
            out_of_flow: false,
            visual_offset_x: 0.0,
            visual_offset_y: 0.0,
            table_cell_pos: None,
            list_item_index: None,
            thead_row_count: 0,
        }
    }
}

impl LayoutBox {
    pub fn border_box(&self) -> Rect {
        Rect::new(
            self.content.x - self.padding.left - self.border.left,
            self.content.y - self.padding.top - self.border.top,
            self.content.width + self.padding.horizontal() + self.border.horizontal(),
            self.content.height + self.padding.vertical() + self.border.vertical(),
        )
    }

    pub fn margin_box_height(&self) -> f32 {
        self.margin.top
            + self.border.top
            + self.padding.top
            + self.content.height
            + self.padding.bottom
            + self.border.bottom
            + self.margin.bottom
    }

    pub fn is_text_leaf(&self) -> bool {
        self.text_content.is_some() && self.children.is_empty()
    }
}

#[derive(Debug, Default)]
pub struct LayoutTree {
    pub root: Option<LayoutBox>,
}

impl LayoutTree {
    pub fn new() -> Self {
        Self::default()
    }

    /// Return references to the root's direct children.
    pub fn root_children_boxes(&self) -> Vec<&LayoutBox> {
        match &self.root {
            Some(root) => root.children.iter().collect(),
            None => Vec::new(),
        }
    }
}

/// A paginated page = a subset of the LayoutTree
#[derive(Debug, Clone)]
pub struct Page {
    pub page_number: u32,
    pub total_pages: u32,
    pub content: Vec<LayoutBox>,
    pub margin_boxes: Vec<MarginBox>,
}

#[derive(Debug, Clone)]
pub struct MarginBox {
    pub position: MarginBoxPosition,
    pub rect: Rect,
    pub text: String,
    pub style: ComputedStyle,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MarginBoxPosition {
    TopLeft,
    TopCenter,
    TopRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}
