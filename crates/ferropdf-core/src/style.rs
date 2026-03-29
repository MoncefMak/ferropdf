use crate::{Color, Length};

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Display {
    #[default]
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

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Position {
    #[default]
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum FontWeight {
    #[default]
    Normal,
    Bold,
    Bolder,
    Lighter,
    W100,
    W200,
    W300,
    W400,
    W500,
    W600,
    W700,
    W800,
    W900,
}
impl FontWeight {
    pub fn to_number(&self) -> u16 {
        match self {
            FontWeight::Normal | FontWeight::W400 => 400,
            FontWeight::Bold | FontWeight::W700 => 700,
            FontWeight::W100 => 100,
            FontWeight::W200 => 200,
            FontWeight::W300 => 300,
            FontWeight::W500 => 500,
            FontWeight::W600 => 600,
            FontWeight::W800 => 800,
            FontWeight::W900 => 900,
            FontWeight::Bolder => 700,
            FontWeight::Lighter => 300,
        }
    }
    pub fn is_bold(&self) -> bool {
        self.to_number() >= 600
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
    Oblique,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Right,
    Center,
    Justify,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum TextDecoration {
    #[default]
    None,
    Underline,
    LineThrough,
    Overline,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum FlexDirection {
    #[default]
    Row,
    Column,
    RowReverse,
    ColumnReverse,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum FlexWrap {
    #[default]
    NoWrap,
    Wrap,
    WrapReverse,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum JustifyContent {
    #[default]
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum AlignItems {
    FlexStart,
    FlexEnd,
    Center,
    #[default]
    Stretch,
    Baseline,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum AlignSelf {
    #[default]
    Auto,
    FlexStart,
    FlexEnd,
    Center,
    Stretch,
    Baseline,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum PageBreak {
    #[default]
    Auto,
    Always,
    Page,
    Left,
    Right,
    Avoid,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum PageBreakInside {
    #[default]
    Auto,
    Avoid,
}

/// CSS box-decoration-break: behavior of borders/background when a container
/// is fragmented across multiple pages.
///   clone → borders and padding repeated on each fragment
///   slice → clean cut without repeating decorations (CSS default)
#[derive(Debug, Clone, PartialEq, Default)]
pub enum BoxDecorationBreak {
    #[default]
    Slice,
    Clone,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum BorderStyle {
    #[default]
    None,
    Solid,
    Dashed,
    Dotted,
    Double,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum BorderCollapse {
    #[default]
    Separate,
    Collapse,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum ListStyleType {
    #[default]
    Disc,
    Circle,
    Square,
    Decimal,
    DecimalLeadingZero,
    LowerAlpha,
    UpperAlpha,
    LowerRoman,
    UpperRoman,
    None,
}

#[derive(Debug, Clone, Copy)]
pub struct BorderSide {
    pub width: f32,
    pub color: Color,
    pub style: BorderStyle,
}
impl Default for BorderSide {
    fn default() -> Self {
        Self {
            width: 0.0,
            color: Color::black(),
            style: BorderStyle::None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct BorderRadius {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}
impl BorderRadius {
    pub fn uniform(r: f32) -> Self {
        Self {
            top_left: r,
            top_right: r,
            bottom_right: r,
            bottom_left: r,
        }
    }
    pub fn any_nonzero(&self) -> bool {
        self.top_left > 0.0
            || self.top_right > 0.0
            || self.bottom_right > 0.0
            || self.bottom_left > 0.0
    }
    pub fn to_array(&self) -> [f32; 4] {
        [
            self.top_left,
            self.top_right,
            self.bottom_right,
            self.bottom_left,
        ]
    }
}

/// CSS `direction` property for text/bidi.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Direction {
    #[default]
    Ltr,
    Rtl,
}

/// A parsed CSS box-shadow value.
#[derive(Debug, Clone)]
pub struct BoxShadow {
    pub offset_x: f32,    // pt
    pub offset_y: f32,    // pt
    pub blur_radius: f32, // pt
    pub spread: f32,      // pt
    pub color: Color,
    pub inset: bool,
}

/// All properties resolved to absolute values.
/// No em/rem here. Percentages are kept for Taffy (layout).
#[derive(Debug, Clone)]
pub struct ComputedStyle {
    pub display: Display,
    pub position: Position,
    pub direction: Direction,
    pub visibility: bool,

    // Dimensions (Percent passed to Taffy, Auto passed to Taffy)
    pub width: Length,
    pub height: Length,
    pub min_width: Length,
    pub max_width: Length,
    pub min_height: Length,
    pub max_height: Length,

    // Spacing — [top, right, bottom, left]
    pub margin: [Length; 4],
    pub padding: [Length; 4],

    // Borders
    pub border_top: BorderSide,
    pub border_right: BorderSide,
    pub border_bottom: BorderSide,
    pub border_left: BorderSide,
    pub border_radius: BorderRadius,

    // Colors and background
    pub color: Color,
    pub background_color: Color,
    pub opacity: f32,
    pub box_shadow: Vec<BoxShadow>,

    // Text (all values in px)
    pub font_family: Vec<String>,
    pub font_size: f32,
    pub font_weight: FontWeight,
    pub font_style: FontStyle,
    pub line_height: f32,
    pub text_align: TextAlign,
    pub text_decoration: TextDecoration,
    pub letter_spacing: f32,

    // Flexbox
    pub flex_direction: FlexDirection,
    pub flex_wrap: FlexWrap,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub align_self: AlignSelf,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: Length,
    pub column_gap: Length,
    pub row_gap: Length,

    // Positioning offsets (CSS left/right/top/bottom)
    pub left: Length,
    pub right: Length,
    pub top: Length,
    pub bottom: Length,
    pub z_index: i32,

    // Table
    pub border_collapse: BorderCollapse,

    // List
    pub list_style_type: ListStyleType,

    // Pagination
    pub page_break_before: PageBreak,
    pub page_break_after: PageBreak,
    pub page_break_inside: PageBreakInside,
    pub box_decoration_break: BoxDecorationBreak,
    pub orphans: u32,
    pub widows: u32,
}

impl Default for ComputedStyle {
    fn default() -> Self {
        Self {
            display: Display::Block,
            position: Position::Static,
            direction: Direction::Ltr,
            visibility: true,
            width: Length::Auto,
            height: Length::Auto,
            min_width: Length::Zero,
            max_width: Length::None,
            min_height: Length::Zero,
            max_height: Length::None,
            margin: [Length::Zero; 4],
            padding: [Length::Zero; 4],
            border_top: BorderSide::default(),
            border_right: BorderSide::default(),
            border_bottom: BorderSide::default(),
            border_left: BorderSide::default(),
            border_radius: BorderRadius::default(),
            color: Color::black(),
            background_color: Color::transparent(),
            opacity: 1.0,
            box_shadow: Vec::new(),
            font_family: vec!["sans-serif".to_string()],
            font_size: 12.0, // 16px × 0.75 = 12pt
            font_weight: FontWeight::Normal,
            font_style: FontStyle::Normal,
            line_height: 14.4, // 12pt × 1.2 = 14.4pt
            text_align: TextAlign::Left,
            text_decoration: TextDecoration::None,
            letter_spacing: 0.0,
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::NoWrap,
            justify_content: JustifyContent::FlexStart,
            align_items: AlignItems::Stretch,
            align_self: AlignSelf::Auto,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: Length::Auto,
            column_gap: Length::Zero,
            row_gap: Length::Zero,
            left: Length::Auto,
            right: Length::Auto,
            top: Length::Auto,
            bottom: Length::Auto,
            z_index: 0,
            border_collapse: BorderCollapse::Separate,
            list_style_type: ListStyleType::Disc,
            page_break_before: PageBreak::Auto,
            page_break_after: PageBreak::Auto,
            page_break_inside: PageBreakInside::Auto,
            box_decoration_break: BoxDecorationBreak::Slice,
            orphans: 2,
            widows: 2,
        }
    }
}
