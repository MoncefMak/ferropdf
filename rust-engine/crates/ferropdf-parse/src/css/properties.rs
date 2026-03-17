//! Computed style properties — the resolved style for a single DOM node.

use ferropdf_core::Color;

/// Computed display value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    Block,
    Inline,
    InlineBlock,
    Flex,
    Grid,
    Table,
    TableRow,
    TableCell,
    TableHeaderGroup,
    TableRowGroup,
    TableFooterGroup,
    ListItem,
    None,
}

impl Default for Display {
    fn default() -> Self { Display::Inline }
}

/// Computed `position` property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Position {
    #[default]
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
}

/// Float.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Float { #[default] None, Left, Right }

/// Clear.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Clear { #[default] None, Left, Right, Both }

/// Overflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Overflow { #[default] Visible, Hidden, Scroll, Auto }

/// Text alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign { #[default] Left, Right, Center, Justify }

/// Text decoration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextDecoration { #[default] None, Underline, Overline, LineThrough }

/// Font style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontStyle { #[default] Normal, Italic, Oblique }

/// Border style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BorderStyleKind { #[default] None, Solid, Dashed, Dotted, Double, Groove, Ridge, Inset, Outset }

/// Flex direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexDirection {
    #[default]
    Row,
    RowReverse,
    Column,
    ColumnReverse,
}

/// Flex wrap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexWrap { #[default] NoWrap, Wrap, WrapReverse }

/// Alignment in flex/grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignItems { #[default] Stretch, FlexStart, FlexEnd, Center, Baseline }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JustifyContent {
    #[default]
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// Computed edge (margin/padding/border) sizes in CSS pixels.
#[derive(Debug, Clone, Copy, Default)]
pub struct ComputedEdge {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl ComputedEdge {
    pub fn horizontal(&self) -> f32 { self.left + self.right }
    pub fn vertical(&self)   -> f32 { self.top  + self.bottom }
}

/// A resolved border side.
#[derive(Debug, Clone, Copy, Default)]
pub struct BorderSide {
    pub width: f32,
    pub style: BorderStyleKind,
    pub color: Color,
}

/// Fully resolved, computed style for one element.
///
/// All lengths are in CSS pixels; all colours are RGBA (0–1 range).
#[derive(Debug, Clone)]
pub struct ComputedStyle {
    // ── Box model ────────────────────────────────────────────────────────────
    pub display:   Display,
    pub position:  Position,
    pub float:     Float,
    pub clear:     Clear,
    pub overflow:  Overflow,
    pub box_sizing_border_box: bool,  // true = border-box

    // ── Dimensions ───────────────────────────────────────────────────────────
    pub width:      Option<f32>,   // None = auto
    pub height:     Option<f32>,
    pub min_width:  f32,
    pub min_height: f32,
    pub max_width:  Option<f32>,
    pub max_height: Option<f32>,

    // ── Stacking ─────────────────────────────────────────────────────────────
    pub top:    Option<f32>,
    pub right:  Option<f32>,
    pub bottom: Option<f32>,
    pub left:   Option<f32>,
    pub z_index: i32,

    // ── Spacing ──────────────────────────────────────────────────────────────
    pub margin:  ComputedEdge,
    pub padding: ComputedEdge,

    // ── Borders ──────────────────────────────────────────────────────────────
    pub border_top:    BorderSide,
    pub border_right:  BorderSide,
    pub border_bottom: BorderSide,
    pub border_left:   BorderSide,
    pub border_radius:  [f32; 4],  // tl / tr / br / bl

    // ── Background ───────────────────────────────────────────────────────────
    pub background_color: Color,
    pub background_image: Option<String>,
    pub opacity:           f32,

    // ── Text ─────────────────────────────────────────────────────────────────
    pub color:           Color,
    pub font_family:     String,
    pub font_size:       f32,     // px
    pub font_weight:     u32,     // 100–900
    pub font_style:      FontStyle,
    pub line_height:     f32,     // px
    pub letter_spacing:  f32,     // px
    pub text_align:      TextAlign,
    pub text_decoration: TextDecoration,
    pub text_indent:     f32,
    pub white_space_pre: bool,
    pub direction_rtl:   bool,

    // ── Flex ─────────────────────────────────────────────────────────────────
    pub flex_direction:  FlexDirection,
    pub flex_wrap:       FlexWrap,
    pub justify_content: JustifyContent,
    pub align_items:     AlignItems,
    pub align_self:      AlignItems,
    pub flex_grow:       f32,
    pub flex_shrink:     f32,
    pub flex_basis:      Option<f32>,
    pub gap:             (f32, f32),   // (row, column) gap in px

    // ── Grid ─────────────────────────────────────────────────────────────────
    pub grid_template_columns: Vec<GridTrack>,
    pub grid_template_rows:    Vec<GridTrack>,
    /// Span count for table cells (html: colspan).
    pub grid_column_span: u32,

    // ── Table ────────────────────────────────────────────────────────────────
    /// Spacing between table cells in px.
    pub border_spacing: f32,

    // ── Lists ────────────────────────────────────────────────────────────────
    pub list_style_type: ListStyleType,

    // ── Pagination ───────────────────────────────────────────────────────────
    pub page_break_before: PageBreak,
    pub page_break_after:  PageBreak,
    pub page_break_inside: PageBreakInside,

    // ── Visibility ───────────────────────────────────────────────────────────
    pub visibility_hidden: bool,

    // ── Links ────────────────────────────────────────────────────────────────
    /// Target URL when the element is an <a href="...">.
    pub href: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ListStyleType { #[default] None, Disc, Circle, Square, Decimal, LowerAlpha, UpperAlpha, LowerRoman, UpperRoman }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PageBreak { #[default] Auto, Always, Avoid }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PageBreakInside { #[default] Auto, Avoid }

#[derive(Debug, Clone)]
pub enum GridTrack {
    Px(f32),
    Fr(f32),
    Auto,
    Percent(f32),
    MinMax(Box<GridTrack>, Box<GridTrack>),
}

impl Default for ComputedStyle {
    fn default() -> Self {
        Self {
            display:   Display::Block,
            position:  Position::Static,
            float:     Float::None,
            clear:     Clear::None,
            overflow:  Overflow::Visible,
            box_sizing_border_box: true,

            width:      None,
            height:     None,
            min_width:  0.0,
            min_height: 0.0,
            max_width:  None,
            max_height: None,

            top:    None,
            right:  None,
            bottom: None,
            left:   None,
            z_index: 0,

            margin:  ComputedEdge::default(),
            padding: ComputedEdge::default(),

            border_top:    BorderSide::default(),
            border_right:  BorderSide::default(),
            border_bottom: BorderSide::default(),
            border_left:   BorderSide::default(),
            border_radius: [0.0; 4],

            background_color:  Color::TRANSPARENT,
            background_image:  None,
            opacity:           1.0,

            color:           Color::BLACK,
            font_family:     "serif".to_string(),
            font_size:       16.0,
            font_weight:     400,
            font_style:      FontStyle::Normal,
            line_height:     19.2,  // 1.2 × 16px
            letter_spacing:  0.0,
            text_align:      TextAlign::Left,
            text_decoration: TextDecoration::None,
            text_indent:     0.0,
            white_space_pre: false,
            direction_rtl:   false,

            flex_direction:  FlexDirection::Row,
            flex_wrap:       FlexWrap::NoWrap,
            justify_content: JustifyContent::FlexStart,
            align_items:     AlignItems::Stretch,
            align_self:      AlignItems::Stretch,
            flex_grow:       0.0,
            flex_shrink:     1.0,
            flex_basis:      None,
            gap:             (0.0, 0.0),

            grid_template_columns: Vec::new(),
            grid_template_rows:    Vec::new(),
            grid_column_span: 1,

            border_spacing: 0.0,

            list_style_type: ListStyleType::None,

            page_break_before: PageBreak::Auto,
            page_break_after:  PageBreak::Auto,
            page_break_inside: PageBreakInside::Auto,

            visibility_hidden: false,
            href: None,
        }
    }
}

impl ComputedStyle {
    /// Default styles for the document root (`<body>` / `<html>`).
    pub fn default_root() -> Self {
        let mut s = Self::default();
        s.font_size = 16.0;
        s.line_height = 16.0 * 1.2;
        s
    }

    pub fn is_hidden(&self) -> bool {
        self.display == Display::None || self.visibility_hidden
    }

    pub fn is_block_level(&self) -> bool {
        matches!(self.display, Display::Block | Display::Flex | Display::Grid | Display::Table | Display::ListItem)
    }

    pub fn is_inline(&self) -> bool {
        matches!(self.display, Display::Inline | Display::InlineBlock)
    }

    pub fn border_width_h(&self)  -> f32 { self.border_left.width  + self.border_right.width }
    pub fn border_width_v(&self)  -> f32 { self.border_top.width   + self.border_bottom.width }
}
