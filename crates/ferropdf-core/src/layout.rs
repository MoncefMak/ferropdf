use crate::{Color, ComputedStyle, Insets, NodeId, Rect};

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
}

/// A styled segment within a shaped line (populated for rich/inline-merged text).
#[derive(Debug, Clone)]
pub struct ShapedSegment {
    pub text: String,
    pub x_offset: f32,
    pub metadata: usize,
}

#[derive(Debug, Clone)]
pub struct ShapedGlyph {
    pub glyph_id: u16,
    pub x: f32,
    pub y: f32,
    pub advance: f32,
    pub font_id: u64,
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

// =============================================================================
// BreakUnit — Breakable unit for smart pagination
// =============================================================================
// After Taffy layout + cosmic-text shaping, we build a flat list of breakable
// units. Each unit is the smallest entity that can be moved without breaking
// the document's meaning.
// =============================================================================

/// A breakable unit — the smallest entity that can be moved
/// without breaking the document's meaning.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum BreakUnit {
    /// A single line from cosmic-text layout_runs().
    TextLine {
        /// Y coordinate of the line top (absolute continuous space, in pt).
        y_top: f32,
        /// Y coordinate of the line bottom (absolute continuous space, in pt).
        y_bottom: f32,
        /// Line index within its parent paragraph.
        line_index: usize,
        /// NodeId of the parent text node (to group lines from the same paragraph).
        parent_node: Option<NodeId>,
        /// Shaped content of the line.
        content: ShapedLine,
    },
    /// Non-breakable block (image, container with break-inside: avoid).
    Atomic {
        /// Y coordinate of the block top (absolute continuous space, in pt).
        y_top: f32,
        /// Y coordinate of the block bottom (absolute continuous space, in pt).
        y_bottom: f32,
        /// The complete LayoutBox.
        node: LayoutBox,
    },
    /// Forced page break marker (break-before: page).
    ForcedBreak,
}

impl BreakUnit {
    /// Y of the unit's top in continuous space (pt).
    pub fn y_top(&self) -> f32 {
        match self {
            BreakUnit::TextLine { y_top, .. } => *y_top,
            BreakUnit::Atomic { y_top, .. } => *y_top,
            BreakUnit::ForcedBreak => 0.0,
        }
    }

    /// Y of the unit's bottom in continuous space (pt).
    pub fn y_bottom(&self) -> f32 {
        match self {
            BreakUnit::TextLine { y_bottom, .. } => *y_bottom,
            BreakUnit::Atomic { y_bottom, .. } => *y_bottom,
            BreakUnit::ForcedBreak => 0.0,
        }
    }
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
    pub out_of_flow: bool,
    /// Visual offset from position: relative (does not affect flow).
    pub visual_offset_x: f32,
    pub visual_offset_y: f32,
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
