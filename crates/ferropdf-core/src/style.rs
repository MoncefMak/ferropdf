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

/// CSS Grid properties, kept as raw CSS and converted to Taffy's track /
/// placement types by the layout crate (which owns the Taffy dependency).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GridProps {
    pub template_columns: Option<String>,
    pub template_rows: Option<String>,
    pub auto_columns: Option<String>,
    pub auto_rows: Option<String>,
    pub auto_flow: Option<String>,
    pub column_start: Option<String>,
    pub column_end: Option<String>,
    pub row_start: Option<String>,
    pub row_end: Option<String>,
    pub justify_items: Option<String>,
    pub justify_self: Option<String>,
}

/// CSS `float`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Float {
    #[default]
    None,
    Left,
    Right,
}

/// CSS `clear`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Clear {
    #[default]
    None,
    Left,
    Right,
    Both,
}

/// CSS `box-sizing`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoxSizing {
    #[default]
    ContentBox,
    BorderBox,
}

/// CSS `white-space` (CSS Text 3 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WhiteSpace {
    #[default]
    Normal,
    Nowrap,
    Pre,
    PreWrap,
    PreLine,
}

impl WhiteSpace {
    /// Whether lines may wrap at soft wrap opportunities.
    pub fn wraps(self) -> bool {
        !matches!(self, WhiteSpace::Nowrap | WhiteSpace::Pre)
    }

    /// Apply white-space processing (CSS Text 3 §4.1.1) to a text node.
    ///
    /// * `normal` / `nowrap`: every run of spaces, tabs and segment breaks
    ///   becomes a single space.
    /// * `pre-line`: runs of spaces/tabs collapse, newlines are kept, and
    ///   spaces around a newline are removed.
    /// * `pre` / `pre-wrap`: text is returned unchanged.
    ///
    /// Leading/trailing spaces at line or block boundaries are handled by the
    /// layout code, which knows about neighbouring inline content.
    pub fn collapse(self, text: &str) -> String {
        match self {
            WhiteSpace::Pre | WhiteSpace::PreWrap => text.to_string(),
            WhiteSpace::Normal | WhiteSpace::Nowrap => {
                let mut out = String::with_capacity(text.len());
                let mut in_space = false;
                for ch in text.chars() {
                    if matches!(ch, ' ' | '\t' | '\n' | '\r' | '\x0C') {
                        if !in_space {
                            out.push(' ');
                            in_space = true;
                        }
                    } else {
                        out.push(ch);
                        in_space = false;
                    }
                }
                out
            }
            WhiteSpace::PreLine => text
                .replace("\r\n", "\n")
                .split('\n')
                .map(|line| WhiteSpace::Normal.collapse(line).trim().to_string())
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }
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
    /// `position: running(name)` — element moved out of the flow and shown in
    /// `@page` margin boxes via `content: element(name)` (GCPM 3 §2.2).
    pub running: Option<String>,
    /// Width / height ratio of a replaced element (`<img>`), from its
    /// intrinsic size. Lets Taffy derive the missing dimension.
    pub aspect_ratio: Option<f32>,
    /// `grid-*`, `justify-items`, `justify-self` (not inherited).
    pub grid: GridProps,
    pub float: Float,
    pub clear: Clear,
    /// `overflow-x` / `overflow-y` other than `visible`: content is clipped to
    /// the padding box (in print, `auto` and `scroll` clip like `hidden`).
    pub overflow_clip_x: bool,
    pub overflow_clip_y: bool,
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
    /// `background-image` gradient layers, first = topmost.
    pub background_gradients: Vec<crate::Gradient>,
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
    pub white_space: WhiteSpace,
    pub box_sizing: BoxSizing,

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

    /// CSS custom properties (`--name: value`) declared on or inherited
    /// by this element. Used to substitute `var(--name)` references in
    /// other declarations during cascade application.
    pub custom_properties: std::collections::HashMap<String, String>,
}

impl Default for ComputedStyle {
    fn default() -> Self {
        Self {
            display: Display::Block,
            position: Position::Static,
            running: None,
            aspect_ratio: None,
            grid: GridProps::default(),
            float: Float::None,
            clear: Clear::None,
            overflow_clip_x: false,
            overflow_clip_y: false,
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
            background_gradients: Vec::new(),
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
            white_space: WhiteSpace::Normal,
            box_sizing: BoxSizing::ContentBox,
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
            custom_properties: std::collections::HashMap::new(),
        }
    }
}

#[cfg(test)]
mod white_space_tests {
    use super::WhiteSpace;

    #[test]
    fn normal_collapses_runs_and_newlines() {
        assert_eq!(
            WhiteSpace::Normal.collapse("alpha\n        beta   gamma\n\tdelta"),
            "alpha beta gamma delta"
        );
        assert_eq!(WhiteSpace::Normal.collapse("  a  "), " a ");
    }

    #[test]
    fn pre_is_untouched() {
        assert_eq!(WhiteSpace::Pre.collapse("a\n  b"), "a\n  b");
        assert_eq!(WhiteSpace::PreWrap.collapse("a  b"), "a  b");
    }

    #[test]
    fn pre_line_keeps_newlines_only() {
        assert_eq!(WhiteSpace::PreLine.collapse("a   b \n   c"), "a b\nc");
    }

    #[test]
    fn wrapping() {
        assert!(WhiteSpace::Normal.wraps());
        assert!(WhiteSpace::PreWrap.wraps());
        assert!(!WhiteSpace::Nowrap.wraps());
        assert!(!WhiteSpace::Pre.wraps());
    }
}
