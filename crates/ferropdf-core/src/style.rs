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
    /// `start` (the initial value): left in LTR, right in RTL.
    #[default]
    Start,
    /// `end`: right in LTR, left in RTL.
    End,
    Left,
    Right,
    Center,
    Justify,
}

impl TextAlign {
    /// The physical alignment for a box of direction `dir`: `start` and
    /// `end` are inherited as keywords and resolved per box (CSS Text §7.1).
    pub fn physical(self, dir: Direction) -> TextAlign {
        let rtl = dir == Direction::Rtl;
        match self {
            TextAlign::Start if rtl => TextAlign::Right,
            TextAlign::Start => TextAlign::Left,
            TextAlign::End if rtl => TextAlign::Left,
            TextAlign::End => TextAlign::Right,
            other => other,
        }
    }
}

/// CSS `text-transform`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextTransform {
    #[default]
    None,
    Uppercase,
    Lowercase,
    Capitalize,
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

/// Computed `border-radius`: for each corner (top-left, top-right,
/// bottom-right, bottom-left) a horizontal and a vertical radius. Lengths are
/// in pt after unit resolution; percentages stay until paint time, where
/// [`BorderRadius::resolve`] measures them against the border box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BorderRadius {
    pub corners: [(Length, Length); 4],
}

impl Default for BorderRadius {
    fn default() -> Self {
        Self {
            corners: [(Length::Zero, Length::Zero); 4],
        }
    }
}

impl BorderRadius {
    /// The same circular radius on every corner.
    pub fn uniform(r: Length) -> Self {
        Self {
            corners: [(r, r); 4],
        }
    }

    /// Resolve against a border box of `w` × `h` pt, then scale down
    /// overlapping radii (CSS Backgrounds §5.5).
    pub fn resolve(&self, w: f32, h: f32) -> Radii {
        let len = |l: Length, basis: f32| -> f32 {
            let v = match l {
                Length::Pt(v) => v,
                Length::Px(v) => v * 0.75,
                Length::Mm(v) => v * 2.834_646,
                Length::Percent(p) => p / 100.0 * basis,
                Length::Calc(c) => c.against(basis),
                _ => 0.0,
            };
            v.max(0.0)
        };
        Radii(self.corners.map(|(x, y)| (len(x, w), len(y, h)))).clamped(w, h)
    }
}

/// Resolved elliptical corner radii in pt: `(horizontal, vertical)` for
/// top-left, top-right, bottom-right, bottom-left.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Radii(pub [(f32, f32); 4]);

impl Radii {
    pub const ZERO: Radii = Radii([(0.0, 0.0); 4]);

    /// Circular radius `r` on every corner.
    pub fn uniform(r: f32) -> Self {
        Radii([(r, r); 4])
    }

    pub fn is_zero(&self) -> bool {
        self.0.iter().all(|(x, y)| *x <= 0.0 || *y <= 0.0)
    }

    /// Scale all radii by one factor so that adjacent radii fit the box.
    pub fn clamped(self, w: f32, h: f32) -> Self {
        let [tl, tr, br, bl] = self.0;
        let fit = |side: f32, a: f32, b: f32| {
            if a + b > side && a + b > 0.0 {
                side.max(0.0) / (a + b)
            } else {
                1.0
            }
        };
        let f = fit(w, tl.0, tr.0)
            .min(fit(w, bl.0, br.0))
            .min(fit(h, tl.1, bl.1))
            .min(fit(h, tr.1, br.1));
        if f >= 1.0 {
            return self;
        }
        Radii(self.0.map(|(x, y)| (x * f, y * f)))
    }

    /// Radii of a box inset by `[top, right, bottom, left]` (e.g. the padding
    /// edge inside the borders): each radius shrinks by the adjacent inset.
    pub fn inset(self, [t, r, b, l]: [f32; 4]) -> Self {
        let [tl, tr, br, bl] = self.0;
        let sub = |v: f32, d: f32| (v - d).max(0.0);
        Radii([
            (sub(tl.0, l), sub(tl.1, t)),
            (sub(tr.0, r), sub(tr.1, t)),
            (sub(br.0, r), sub(br.1, b)),
            (sub(bl.0, l), sub(bl.1, b)),
        ])
    }

    /// Radii of a box grown by `d` on every side (box-shadow spread);
    /// square corners stay square.
    pub fn grow(self, d: f32) -> Self {
        Radii(self.0.map(|(x, y)| {
            if x <= 0.0 || y <= 0.0 {
                (x, y)
            } else {
                ((x + d).max(0.0), (y + d).max(0.0))
            }
        }))
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
    /// Raw `content` value (used by `::before` / `::after`).
    pub content: Option<String>,
    /// `counter-reset`, `counter-increment`, `counter-set`: (name, value).
    pub counter_reset: Vec<(String, i32)>,
    pub counter_increment: Vec<(String, i32)>,
    pub counter_set: Vec<(String, i32)>,
    /// `transform` function list (applied right to left).
    pub transform: Vec<crate::TransformFunction>,
    /// Individual `translate`, `rotate`, `scale` properties, in that order.
    pub transform_individual: [Option<crate::TransformFunction>; 3],
    /// `transform-origin`, relative to the border box.
    pub transform_origin: (crate::GradientLength, crate::GradientLength),
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
    /// `letter-spacing` / `word-spacing`: extra space after each character /
    /// word separator (`normal` = 0). Resolved to pt per element.
    pub letter_spacing: Length,
    pub word_spacing: Length,
    /// `text-indent` of the first line of a block (percentages of its width).
    pub text_indent: Length,
    pub text_transform: TextTransform,
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
    /// Shared with the parent until an element declares its own (`--x`):
    /// inheriting is a pointer copy (copy-on-write via `Arc::make_mut`).
    pub custom_properties: std::sync::Arc<std::collections::HashMap<String, String>>,
}

impl ComputedStyle {
    /// The initial style, shared (for nodes without a computed style).
    pub fn initial() -> &'static ComputedStyle {
        static INITIAL: std::sync::OnceLock<ComputedStyle> = std::sync::OnceLock::new();
        INITIAL.get_or_init(ComputedStyle::default)
    }
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
            content: None,
            counter_reset: Vec::new(),
            counter_increment: Vec::new(),
            counter_set: Vec::new(),
            transform: Vec::new(),
            transform_individual: [None; 3],
            transform_origin: (
                crate::GradientLength::Percent(50.0),
                crate::GradientLength::Percent(50.0),
            ),
            opacity: 1.0,
            box_shadow: Vec::new(),
            font_family: vec!["sans-serif".to_string()],
            font_size: 12.0, // 16px × 0.75 = 12pt
            font_weight: FontWeight::Normal,
            font_style: FontStyle::Normal,
            line_height: 14.4, // 12pt × 1.2 = 14.4pt
            text_align: TextAlign::Start,
            text_decoration: TextDecoration::None,
            letter_spacing: Length::Zero,
            word_spacing: Length::Zero,
            text_indent: Length::Zero,
            text_transform: TextTransform::None,
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
            custom_properties: std::sync::Arc::default(),
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

#[cfg(test)]
mod text_align_tests {
    use super::{Direction, TextAlign};

    #[test]
    fn logical_values_resolve_per_direction() {
        assert_eq!(TextAlign::Start.physical(Direction::Ltr), TextAlign::Left);
        assert_eq!(TextAlign::Start.physical(Direction::Rtl), TextAlign::Right);
        assert_eq!(TextAlign::End.physical(Direction::Rtl), TextAlign::Left);
        assert_eq!(
            TextAlign::Center.physical(Direction::Rtl),
            TextAlign::Center
        );
        assert_eq!(TextAlign::default(), TextAlign::Start);
    }
}

#[cfg(test)]
mod radii_tests {
    use super::Radii;

    #[test]
    fn pill_radii_are_clamped_to_half_the_height() {
        let r = Radii::uniform(9999.0).clamped(200.0, 40.0);
        for (x, y) in r.0 {
            assert!((x - 20.0).abs() < 0.01 && (y - 20.0).abs() < 0.01, "{r:?}");
        }
    }

    #[test]
    fn small_radii_are_untouched() {
        let r = Radii([(5.0, 5.0), (6.0, 6.0), (7.0, 7.0), (8.0, 8.0)]);
        assert_eq!(r.clamped(100.0, 50.0), r);
    }

    #[test]
    fn inset_and_grow() {
        let r = Radii::uniform(10.0).inset([2.0, 3.0, 4.0, 5.0]);
        assert_eq!(r.0[0], (5.0, 8.0)); // top-left: minus left, minus top
        assert_eq!(r.0[2], (7.0, 6.0)); // bottom-right: minus right, minus bottom
        assert_eq!(
            Radii::ZERO.grow(5.0),
            Radii::ZERO,
            "square corners stay square"
        );
        assert_eq!(Radii::uniform(4.0).grow(2.0).0[1], (6.0, 6.0));
    }
}
