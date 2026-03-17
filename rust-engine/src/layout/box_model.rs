//! Box model types for the layout engine.

use crate::css::properties::ComputedStyle;

/// A rectangle in the coordinate space (origin at top-left, y increases downward).
/// All values are in CSS pixels.
#[derive(Debug, Clone, Copy, Default)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn zero() -> Self {
        Self::default()
    }

    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }

    /// Expand the rect to include another rect.
    pub fn union(&self, other: &Rect) -> Rect {
        if self.width == 0.0 && self.height == 0.0 {
            return *other;
        }
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        Rect::new(x, y, right - x, bottom - y)
    }

    /// Check if this rect intersects another.
    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }
}

/// Sizes for box-model edges (margin, padding, border).
#[derive(Debug, Clone, Copy, Default)]
pub struct EdgeSizes {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl EdgeSizes {
    pub fn new(top: f64, right: f64, bottom: f64, left: f64) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    pub fn uniform(value: f64) -> Self {
        Self::new(value, value, value, value)
    }

    pub fn zero() -> Self {
        Self::default()
    }

    pub fn horizontal(&self) -> f64 {
        self.left + self.right
    }

    pub fn vertical(&self) -> f64 {
        self.top + self.bottom
    }
}

/// Full box dimensions (content + padding + border + margin).
#[derive(Debug, Clone, Default)]
pub struct BoxDimensions {
    /// The content area rectangle.
    pub content: Rect,
    /// Padding sizes.
    pub padding: EdgeSizes,
    /// Border sizes.
    pub border: EdgeSizes,
    /// Margin sizes.
    pub margin: EdgeSizes,
}

impl BoxDimensions {
    /// Get the padding box (content + padding).
    pub fn padding_box(&self) -> Rect {
        Rect {
            x: self.content.x - self.padding.left,
            y: self.content.y - self.padding.top,
            width: self.content.width + self.padding.horizontal(),
            height: self.content.height + self.padding.vertical(),
        }
    }

    /// Get the border box (content + padding + border).
    pub fn border_box(&self) -> Rect {
        let padding = self.padding_box();
        Rect {
            x: padding.x - self.border.left,
            y: padding.y - self.border.top,
            width: padding.width + self.border.horizontal(),
            height: padding.height + self.border.vertical(),
        }
    }

    /// Get the margin box (content + padding + border + margin).
    pub fn margin_box(&self) -> Rect {
        let border = self.border_box();
        Rect {
            x: border.x - self.margin.left,
            y: border.y - self.margin.top,
            width: border.width + self.margin.horizontal(),
            height: border.height + self.margin.vertical(),
        }
    }

    /// Total width including all box-model layers.
    pub fn total_width(&self) -> f64 {
        self.margin_box().width
    }

    /// Total height including all box-model layers.
    pub fn total_height(&self) -> f64 {
        self.margin_box().height
    }
}

/// The type of layout box.
#[derive(Debug, Clone)]
pub enum LayoutBoxType {
    /// A block-level box.
    Block,
    /// An inline-level box.
    Inline,
    /// An inline-block box.
    InlineBlock,
    /// A flex container.
    Flex,
    /// A grid container.
    Grid,
    /// A table box.
    Table,
    /// A table row.
    TableRow,
    /// A table cell.
    TableCell,
    /// A list item.
    ListItem,
    /// An anonymous block box (for wrapping inline content in block flow).
    AnonymousBlock,
    /// An anonymous inline box.
    AnonymousInline,
    /// An image/replaced element.
    Replaced,
}

/// CSS position value for a layout box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PositionType {
    Static,
    Relative,
    Absolute,
    Fixed,
}

/// CSS float value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FloatSide {
    None,
    Left,
    Right,
}

/// A layout box with computed dimensions and styles.
#[derive(Debug, Clone)]
pub struct LayoutBox {
    /// The type of this layout box.
    pub box_type: LayoutBoxType,
    /// The computed dimensions.
    pub dimensions: BoxDimensions,
    /// The computed style for this box.
    pub style: ComputedStyle,
    /// Child layout boxes.
    pub children: Vec<LayoutBox>,
    /// Optional text content for text nodes.
    pub text: Option<String>,
    /// Tag name (if element).
    pub tag_name: Option<String>,
    /// Source URL for images.
    pub image_src: Option<String>,
    /// Whether this box forces a page break before it.
    pub page_break_before: bool,
    /// Whether this box forces a page break after it.
    pub page_break_after: bool,
    /// Whether page breaks inside this box should be avoided.
    pub avoid_break_inside: bool,
    /// Inline styles from the style attribute.
    pub inline_style: Option<String>,
    /// Element attributes (for rendering anchors, etc.).
    pub attributes: std::collections::HashMap<String, String>,
    /// CSS position type (static, relative, absolute, fixed).
    pub position_type: PositionType,
    /// Children that are positioned out of flow (absolute/fixed).
    pub out_of_flow_children: Vec<LayoutBox>,
    /// CSS float value (none, left, right).
    pub float_side: FloatSide,
}

impl LayoutBox {
    pub fn new(box_type: LayoutBoxType, style: ComputedStyle) -> Self {
        Self {
            box_type,
            dimensions: BoxDimensions::default(),
            style,
            children: Vec::new(),
            text: None,
            tag_name: None,
            image_src: None,
            page_break_before: false,
            page_break_after: false,
            avoid_break_inside: false,
            inline_style: None,
            attributes: std::collections::HashMap::new(),
            position_type: PositionType::Static,
            out_of_flow_children: Vec::new(),
            float_side: FloatSide::None,
        }
    }

    pub fn anonymous_block() -> Self {
        Self::new(LayoutBoxType::AnonymousBlock, ComputedStyle::new())
    }

    pub fn text_box(text: String, style: ComputedStyle) -> Self {
        let mut b = Self::new(LayoutBoxType::Inline, style);
        b.text = Some(text);
        b
    }

    /// Check if this box has only inline children.
    pub fn has_inline_children(&self) -> bool {
        self.children.iter().all(|c| {
            matches!(
                c.box_type,
                LayoutBoxType::Inline | LayoutBoxType::AnonymousInline
            )
        })
    }

    /// Check if this box has any block-level children.
    pub fn has_block_children(&self) -> bool {
        self.children.iter().any(|c| {
            matches!(
                c.box_type,
                LayoutBoxType::Block
                    | LayoutBoxType::AnonymousBlock
                    | LayoutBoxType::Flex
                    | LayoutBoxType::Table
                    | LayoutBoxType::ListItem
            )
        })
    }

    /// Get the total content height of all children.
    pub fn children_height(&self) -> f64 {
        self.children
            .iter()
            .map(|c| c.dimensions.margin_box().height)
            .sum()
    }
}
