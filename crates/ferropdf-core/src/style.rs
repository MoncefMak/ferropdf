use crate::{Color, Length};

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Display {
    #[default]
    Block,
    Inline, InlineBlock,
    Flex, Grid,
    Table, TableRow, TableCell,
    TableHeaderGroup, TableRowGroup, TableFooterGroup,
    ListItem, None,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Position { #[default] Static, Relative, Absolute, Fixed, Sticky }

#[derive(Debug, Clone, PartialEq)]
pub enum FontWeight {
    Normal, Bold, Bolder, Lighter,
    W100, W200, W300, W400, W500, W600, W700, W800, W900,
}
impl Default for FontWeight { fn default() -> Self { FontWeight::Normal } }
impl FontWeight {
    pub fn to_number(&self) -> u16 {
        match self {
            FontWeight::Normal | FontWeight::W400 => 400,
            FontWeight::Bold   | FontWeight::W700 => 700,
            FontWeight::W100 => 100, FontWeight::W200 => 200,
            FontWeight::W300 => 300, FontWeight::W500 => 500,
            FontWeight::W600 => 600, FontWeight::W800 => 800,
            FontWeight::W900 => 900,
            FontWeight::Bolder  => 700,
            FontWeight::Lighter => 300,
        }
    }
    pub fn is_bold(&self) -> bool { self.to_number() >= 600 }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum FontStyle { #[default] Normal, Italic, Oblique }

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TextAlign { #[default] Left, Right, Center, Justify }

#[derive(Debug, Clone, PartialEq, Default)]
pub enum TextDecoration { #[default] None, Underline, LineThrough, Overline }

#[derive(Debug, Clone, PartialEq, Default)]
pub enum FlexDirection { #[default] Row, Column, RowReverse, ColumnReverse }

#[derive(Debug, Clone, PartialEq, Default)]
pub enum FlexWrap { #[default] NoWrap, Wrap, WrapReverse }

#[derive(Debug, Clone, PartialEq, Default)]
pub enum JustifyContent {
    #[default] FlexStart, FlexEnd, Center,
    SpaceBetween, SpaceAround, SpaceEvenly,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum AlignItems { FlexStart, FlexEnd, Center, #[default] Stretch, Baseline }

#[derive(Debug, Clone, PartialEq, Default)]
pub enum AlignSelf { #[default] Auto, FlexStart, FlexEnd, Center, Stretch, Baseline }

#[derive(Debug, Clone, PartialEq, Default)]
pub enum PageBreak { #[default] Auto, Always, Page, Left, Right, Avoid }

#[derive(Debug, Clone, PartialEq, Default)]
pub enum PageBreakInside { #[default] Auto, Avoid }

#[derive(Debug, Clone, PartialEq, Default)]
pub enum BorderStyle { #[default] None, Solid, Dashed, Dotted, Double }

#[derive(Debug, Clone)]
pub struct BorderSide {
    pub width: f32,
    pub color: Color,
    pub style: BorderStyle,
}
impl Default for BorderSide {
    fn default() -> Self {
        Self { width: 0.0, color: Color::black(), style: BorderStyle::None }
    }
}

#[derive(Debug, Clone, Default)]
pub struct BorderRadius {
    pub top_left: f32, pub top_right: f32,
    pub bottom_right: f32, pub bottom_left: f32,
}
impl BorderRadius {
    pub fn uniform(r: f32) -> Self {
        Self { top_left: r, top_right: r, bottom_right: r, bottom_left: r }
    }
    pub fn any_nonzero(&self) -> bool {
        self.top_left > 0.0 || self.top_right > 0.0
        || self.bottom_right > 0.0 || self.bottom_left > 0.0
    }
    pub fn to_array(&self) -> [f32; 4] {
        [self.top_left, self.top_right, self.bottom_right, self.bottom_left]
    }
}

/// Toutes les propriétés résolues en valeurs absolues.
/// Pas de em/rem ici. Les Percent sont gardés pour Taffy (layout).
#[derive(Debug, Clone)]
pub struct ComputedStyle {
    pub display:    Display,
    pub position:   Position,
    pub visibility: bool,

    // Dimensions (Percent passé à Taffy, Auto passé à Taffy)
    pub width:      Length,
    pub height:     Length,
    pub min_width:  Length,
    pub max_width:  Length,
    pub min_height: Length,
    pub max_height: Length,

    // Spacing — [top, right, bottom, left]
    pub margin:  [Length; 4],
    pub padding: [Length; 4],

    // Borders
    pub border_top:    BorderSide,
    pub border_right:  BorderSide,
    pub border_bottom: BorderSide,
    pub border_left:   BorderSide,
    pub border_radius: BorderRadius,

    // Couleurs et fond
    pub color:            Color,
    pub background_color: Color,
    pub opacity:          f32,

    // Texte (toutes les valeurs en px)
    pub font_family:      Vec<String>,
    pub font_size:        f32,
    pub font_weight:      FontWeight,
    pub font_style:       FontStyle,
    pub line_height:      f32,
    pub text_align:       TextAlign,
    pub text_decoration:  TextDecoration,
    pub letter_spacing:   f32,

    // Flexbox
    pub flex_direction:  FlexDirection,
    pub flex_wrap:       FlexWrap,
    pub justify_content: JustifyContent,
    pub align_items:     AlignItems,
    pub align_self:      AlignSelf,
    pub flex_grow:       f32,
    pub flex_shrink:     f32,
    pub flex_basis:      Length,
    pub column_gap:      Length,
    pub row_gap:         Length,

    // Positioning offsets (CSS left/right/top/bottom)
    pub left:   Length,
    pub right:  Length,
    pub top:    Length,
    pub bottom: Length,

    // Pagination
    pub page_break_before: PageBreak,
    pub page_break_after:  PageBreak,
    pub page_break_inside: PageBreakInside,
    pub orphans:           u32,
    pub widows:            u32,
}

impl Default for ComputedStyle {
    fn default() -> Self {
        Self {
            display:          Display::Block,
            position:         Position::Static,
            visibility:       true,
            width:            Length::Auto,
            height:           Length::Auto,
            min_width:        Length::Zero,
            max_width:        Length::None,
            min_height:       Length::Zero,
            max_height:       Length::None,
            margin:           [Length::Zero; 4],
            padding:          [Length::Zero; 4],
            border_top:       BorderSide::default(),
            border_right:     BorderSide::default(),
            border_bottom:    BorderSide::default(),
            border_left:      BorderSide::default(),
            border_radius:    BorderRadius::default(),
            color:            Color::black(),
            background_color: Color::transparent(),
            opacity:          1.0,
            font_family:      vec!["sans-serif".to_string()],
            font_size:        16.0,
            font_weight:      FontWeight::Normal,
            font_style:       FontStyle::Normal,
            line_height:      19.2,
            text_align:       TextAlign::Left,
            text_decoration:  TextDecoration::None,
            letter_spacing:   0.0,
            flex_direction:   FlexDirection::Row,
            flex_wrap:        FlexWrap::NoWrap,
            justify_content:  JustifyContent::FlexStart,
            align_items:      AlignItems::Stretch,
            align_self:       AlignSelf::Auto,
            flex_grow:        0.0,
            flex_shrink:      1.0,
            flex_basis:       Length::Auto,
            column_gap:       Length::Zero,
            row_gap:          Length::Zero,
            left:              Length::Auto,
            right:             Length::Auto,
            top:               Length::Auto,
            bottom:            Length::Auto,
            page_break_before: PageBreak::Auto,
            page_break_after:  PageBreak::Auto,
            page_break_inside: PageBreakInside::Auto,
            orphans:           2,
            widows:            2,
        }
    }
}
