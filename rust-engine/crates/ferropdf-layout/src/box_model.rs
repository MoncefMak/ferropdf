//! Layout box types used by the layout engine and renderer.

use ferropdf_core::{Color, Rect};
use ferropdf_parse::css::properties::ComputedStyle;

// ─── Shaped text ──────────────────────────────────────────────────────────────

/// A single positioned glyph produced by text shaping.
#[derive(Debug, Clone)]
pub struct ShapedGlyph {
    pub glyph_id:  u16,
    pub x:         f32,   // x advance offset within the line
    pub y:         f32,   // baseline-relative y (usually 0)
    pub advance:   f32,   // horizontal advance in px
}

/// One line of shaped / wrapped text.
#[derive(Debug, Clone)]
pub struct ShapedLine {
    pub text:       String,
    pub glyphs:     Vec<ShapedGlyph>,
    /// Pixel height of this line (= line-height).
    pub height:     f32,
    /// Pixel width of this line.
    pub width:      f32,
    /// Baseline y-offset from top of line.
    pub baseline:   f32,
    pub rtl:        bool,
}

// ─── LayoutBoxKind ────────────────────────────────────────────────────────────

/// The kind / role of a layout box.
#[derive(Debug, Clone)]
pub enum LayoutBoxKind {
    /// A block-level container.
    Block,
    /// An inline text run.
    Inline,
    /// An inline-block container.
    InlineBlock,
    /// A flex container.
    Flex,
    /// A CSS grid container.
    Grid,
    /// A table.
    Table,
    /// A table row.
    TableRow,
    /// A table cell.
    TableCell,
    /// A list item (block with marker).
    ListItem { marker: String },
    /// A text box — carries pre-shaped lines.
    Text { lines: Vec<ShapedLine>, raw_text: String },
    /// An image placeholder.
    Image { src: String },
    /// An absolutely or fixed-positioned box.
    Positioned,
    /// Anonymous block box (wraps inline children inside a block).
    AnonymousBlock,
}

// ─── LayoutBox ────────────────────────────────────────────────────────────────

/// A fully laid-out box with position, size, children.
#[derive(Debug, Clone)]
pub struct LayoutBox {
    pub kind:      LayoutBoxKind,
    pub style:     ComputedStyle,
    /// Position + size of the **content** area in the page coordinate space.
    pub content:   Rect,
    pub children:  Vec<LayoutBox>,
    /// Out-of-flow children (absolute / fixed positioned).
    pub oof:       Vec<LayoutBox>,
}

impl LayoutBox {
    /// Create a new empty box.
    pub fn new(kind: LayoutBoxKind, style: ComputedStyle) -> Self {
        Self { kind, style, content: Rect::zero(), children: Vec::new(), oof: Vec::new() }
    }

    /// Create a text box.
    pub fn text(raw: String, style: ComputedStyle) -> Self {
        Self::new(LayoutBoxKind::Text { lines: Vec::new(), raw_text: raw }, style)
    }

    /// Total width including padding + border.
    pub fn border_width(&self) -> f32 {
        self.content.width
            + self.style.padding.horizontal()
            + self.style.border_width_h()
    }

    /// Total height including padding + border.
    pub fn border_height(&self) -> f32 {
        self.content.height
            + self.style.padding.vertical()
            + self.style.border_width_v()
    }

    /// Origin of the border box (content origin - padding - border).
    pub fn border_x(&self) -> f32 {
        self.content.x - self.style.padding.left - self.style.border_left.width
    }
    pub fn border_y(&self) -> f32 {
        self.content.y - self.style.padding.top - self.style.border_top.width
    }

    /// Returns true if this is a text box.
    pub fn is_text(&self) -> bool { matches!(self.kind, LayoutBoxKind::Text { .. }) }

    /// Raw text (for text boxes).
    pub fn raw_text(&self) -> Option<&str> {
        match &self.kind {
            LayoutBoxKind::Text { raw_text, .. } => Some(raw_text),
            _ => None,
        }
    }

    /// Shaped lines (for text boxes).
    pub fn lines(&self) -> Option<&[ShapedLine]> {
        match &self.kind {
            LayoutBoxKind::Text { lines, .. } => Some(lines),
            _ => None,
        }
    }

    pub fn lines_mut(&mut self) -> Option<&mut Vec<ShapedLine>> {
        match &mut self.kind {
            LayoutBoxKind::Text { lines, .. } => Some(lines),
            _ => None,
        }
    }
}
