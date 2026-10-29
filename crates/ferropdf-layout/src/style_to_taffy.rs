use ferropdf_core::{ComputedStyle, Length, Position as FPosition};
use taffy::prelude::*;
use taffy::MinMax;

/// Convert ComputedStyle → taffy::Style
/// Taffy automatically handles:
/// - width:100% resolved relative to containing block (Fix 1)
/// - padding subtracted only once (Fix 2)
/// - flex, grid, block layout
pub fn convert(style: &ComputedStyle) -> taffy::Style {
    let mut out = taffy::Style {
        position: match style.position {
            FPosition::Absolute | FPosition::Fixed => Position::Absolute,
            _ => Position::Relative,
        },
        inset: taffy::Rect {
            top: lpa(&style.top),
            right: lpa(&style.right),
            bottom: lpa(&style.bottom),
            left: lpa(&style.left),
        },
        display: match style.display {
            ferropdf_core::Display::Block => Display::Block,
            ferropdf_core::Display::Flex => Display::Flex,
            ferropdf_core::Display::Grid => Display::Grid,
            ferropdf_core::Display::None => Display::None,
            // Table → flex column so rows stack vertically
            ferropdf_core::Display::Table => Display::Flex,
            // Table row (and row groups) → flex row so cells sit side by side
            ferropdf_core::Display::TableRow
            | ferropdf_core::Display::TableHeaderGroup
            | ferropdf_core::Display::TableRowGroup
            | ferropdf_core::Display::TableFooterGroup => Display::Flex,
            // Table cells → block (flex_grow makes them share space)
            ferropdf_core::Display::TableCell => Display::Block,
            // Inline, InlineBlock, ListItem → Block for PDF
            _ => Display::Block,
        },

        size: Size {
            width: match style.display {
                // Tables default to 100% width if no explicit width set
                ferropdf_core::Display::Table
                    if style.width == Length::Auto || style.width == Length::None =>
                {
                    Dimension::Percent(1.0)
                }
                _ => length_to_dim(&style.width),
            },
            height: length_to_dim(&style.height),
        },
        min_size: Size {
            width: length_to_dim(&style.min_width),
            height: length_to_dim(&style.min_height),
        },
        max_size: Size {
            width: length_to_dim(&style.max_width),
            height: length_to_dim(&style.max_height),
        },

        // IMPORTANT : Taffy gère le padding correctement — ne pas le recalculer
        padding: taffy::Rect {
            top: lp(&style.padding[0]),
            right: lp(&style.padding[1]),
            bottom: lp(&style.padding[2]),
            left: lp(&style.padding[3]),
        },
        border: taffy::Rect {
            top: LengthPercentage::Length(style.border_top.width),
            right: LengthPercentage::Length(style.border_right.width),
            bottom: LengthPercentage::Length(style.border_bottom.width),
            left: LengthPercentage::Length(style.border_left.width),
        },
        margin: taffy::Rect {
            top: lpa(&style.margin[0]),
            right: lpa(&style.margin[1]),
            bottom: lpa(&style.margin[2]),
            left: lpa(&style.margin[3]),
        },

        flex_direction: match style.display {
            // Table and row groups: children (rows) stack vertically
            ferropdf_core::Display::Table
            | ferropdf_core::Display::TableHeaderGroup
            | ferropdf_core::Display::TableRowGroup
            | ferropdf_core::Display::TableFooterGroup => FlexDirection::Column,
            // Table row: cells go side by side
            ferropdf_core::Display::TableRow => FlexDirection::Row,
            // Everything else: use the CSS flex-direction
            _ => match style.flex_direction {
                ferropdf_core::FlexDirection::Row => FlexDirection::Row,
                ferropdf_core::FlexDirection::Column => FlexDirection::Column,
                ferropdf_core::FlexDirection::RowReverse => FlexDirection::RowReverse,
                ferropdf_core::FlexDirection::ColumnReverse => FlexDirection::ColumnReverse,
            },
        },
        flex_wrap: match style.flex_wrap {
            ferropdf_core::FlexWrap::NoWrap => FlexWrap::NoWrap,
            ferropdf_core::FlexWrap::Wrap => FlexWrap::Wrap,
            ferropdf_core::FlexWrap::WrapReverse => FlexWrap::WrapReverse,
        },
        justify_content: Some(match style.justify_content {
            ferropdf_core::JustifyContent::FlexStart => JustifyContent::FlexStart,
            ferropdf_core::JustifyContent::FlexEnd => JustifyContent::FlexEnd,
            ferropdf_core::JustifyContent::Center => JustifyContent::Center,
            ferropdf_core::JustifyContent::SpaceBetween => JustifyContent::SpaceBetween,
            ferropdf_core::JustifyContent::SpaceAround => JustifyContent::SpaceAround,
            ferropdf_core::JustifyContent::SpaceEvenly => JustifyContent::SpaceEvenly,
        }),
        align_items: Some(match style.align_items {
            ferropdf_core::AlignItems::Stretch => AlignItems::Stretch,
            ferropdf_core::AlignItems::FlexStart => AlignItems::FlexStart,
            ferropdf_core::AlignItems::FlexEnd => AlignItems::FlexEnd,
            ferropdf_core::AlignItems::Center => AlignItems::Center,
            ferropdf_core::AlignItems::Baseline => AlignItems::Baseline,
        }),
        flex_grow: match style.display {
            ferropdf_core::Display::TableCell => 1.0,
            _ => style.flex_grow,
        },
        flex_shrink: match style.display {
            ferropdf_core::Display::TableCell => 1.0,
            _ => style.flex_shrink,
        },
        flex_basis: match style.display {
            // Auto lets cells size to content first, then grow/shrink to fill
            ferropdf_core::Display::TableCell => Dimension::Auto,
            _ => length_to_dim(&style.flex_basis),
        },
        gap: Size {
            width: lp(&style.column_gap),
            height: lp(&style.row_gap),
        },

        aspect_ratio: style.aspect_ratio,
        // Clipping boxes get a zero automatic minimum size in flex/grid
        // (CSS Flexbox §4.5), so long content no longer stretches them.
        overflow: taffy::Point {
            x: if style.overflow_clip_x {
                taffy::Overflow::Hidden
            } else {
                taffy::Overflow::Visible
            },
            y: if style.overflow_clip_y {
                taffy::Overflow::Hidden
            } else {
                taffy::Overflow::Visible
            },
        },
        ..Default::default()
    };
    apply_grid(style, &mut out);
    if style.box_sizing == ferropdf_core::BoxSizing::ContentBox
        && style.display != ferropdf_core::Display::Table
    {
        apply_content_box(style, &mut out);
    }
    out
}

/// Taffy 0.5 sizes are border-box. For `box-sizing: content-box` (the CSS
/// default) add padding + border to definite lengths. Percentage sizes are
/// left as-is (would need `calc()`), i.e. they behave as border-box.
fn apply_content_box(style: &ComputedStyle, out: &mut taffy::Style) {
    let abs = |l: &Length| match l {
        Length::Pt(v) => Some(*v),
        Length::Zero | Length::Auto | Length::None => Some(0.0),
        _ => None,
    };
    let horizontal = abs(&style.padding[1])
        .zip(abs(&style.padding[3]))
        .map(|(r, l)| r + l + style.border_left.width + style.border_right.width);
    let vertical = abs(&style.padding[0])
        .zip(abs(&style.padding[2]))
        .map(|(t, b)| t + b + style.border_top.width + style.border_bottom.width);
    let grow = |d: &mut Dimension, extra: Option<f32>| {
        if let (Dimension::Length(v), Some(e)) = (*d, extra) {
            *d = Dimension::Length(v + e);
        }
    };
    grow(&mut out.size.width, horizontal);
    grow(&mut out.min_size.width, horizontal);
    grow(&mut out.max_size.width, horizontal);
    grow(&mut out.size.height, vertical);
    grow(&mut out.min_size.height, vertical);
    grow(&mut out.max_size.height, vertical);
}

/// Convert a `<table>` ComputedStyle into a CSS Grid taffy::Style.
///
/// The table becomes a grid with `num_cols` columns, each sized `auto` (min-content/max-content).
/// This lets Taffy handle column alignment natively instead of our manual pre-pass.
pub fn convert_table_to_grid(style: &ComputedStyle, num_cols: usize) -> taffy::Style {
    let col_widths: Vec<f32> = vec![0.0; num_cols];
    convert_table_to_grid_with_widths(style, &col_widths)
}

/// Convert a `<table>` into a CSS Grid with pre-computed column widths.
///
/// If all widths are 0 (no content measured), falls back to minmax(min-content, 1fr).
/// Otherwise, each column gets a fixed width proportional to its content.
pub fn convert_table_to_grid_with_widths(
    style: &ComputedStyle,
    col_widths: &[f32],
) -> taffy::Style {
    use taffy::geometry::MinMax;
    use taffy::style::{MaxTrackSizingFunction, MinTrackSizingFunction, TrackSizingFunction};

    let total_min: f32 = col_widths.iter().sum();
    let has_measured = total_min > 0.0;

    let col_template: Vec<TrackSizingFunction> = col_widths
        .iter()
        .map(|&w| {
            if has_measured && w > 0.0 {
                // Use measured min-content as the minimum, and 1fr for proportional growth
                TrackSizingFunction::Single(MinMax {
                    min: MinTrackSizingFunction::Fixed(LengthPercentage::Length(w)),
                    max: MaxTrackSizingFunction::Fraction(w / total_min),
                })
            } else {
                // Fallback: auto sizing
                TrackSizingFunction::Single(MinMax {
                    min: MinTrackSizingFunction::MinContent,
                    max: MaxTrackSizingFunction::Fraction(1.0),
                })
            }
        })
        .collect();

    taffy::Style {
        display: Display::Grid,

        size: Size {
            width: if style.width == Length::Auto || style.width == Length::None {
                Dimension::Percent(1.0)
            } else {
                length_to_dim(&style.width)
            },
            height: length_to_dim(&style.height),
        },

        padding: taffy::Rect {
            top: lp(&style.padding[0]),
            right: lp(&style.padding[1]),
            bottom: lp(&style.padding[2]),
            left: lp(&style.padding[3]),
        },
        border: taffy::Rect {
            top: LengthPercentage::Length(style.border_top.width),
            right: LengthPercentage::Length(style.border_right.width),
            bottom: LengthPercentage::Length(style.border_bottom.width),
            left: LengthPercentage::Length(style.border_left.width),
        },
        margin: taffy::Rect {
            top: lpa(&style.margin[0]),
            right: lpa(&style.margin[1]),
            bottom: lpa(&style.margin[2]),
            left: lpa(&style.margin[3]),
        },

        grid_template_columns: col_template,

        ..Default::default()
    }
}

fn length_to_dim(l: &Length) -> Dimension {
    match l {
        Length::Pt(v) => Dimension::Length(*v), // pt is the internal unit
        Length::Px(v) => Dimension::Length(*v * 0.75), // fallback: shouldn't appear after resolution
        Length::Percent(v) => Dimension::Percent(v / 100.0),
        // Mixed calc(): provisional percentage, fixed by the second layout
        // pass in taffy_bridge (Taffy 0.5 has no calc()).
        Length::Calc(c) => Dimension::Percent(c.percent / 100.0),
        Length::Auto => Dimension::Auto,
        Length::Zero => Dimension::Length(0.0),
        Length::None => Dimension::Auto,
        other => {
            log::warn!("Unresolved length passed to Taffy: {:?}", other);
            Dimension::Auto
        }
    }
}

fn lp(l: &Length) -> LengthPercentage {
    match l {
        Length::Pt(v) => LengthPercentage::Length(*v),
        Length::Px(v) => LengthPercentage::Length(*v * 0.75),
        Length::Percent(v) => LengthPercentage::Percent(v / 100.0),
        Length::Calc(c) => LengthPercentage::Percent(c.percent / 100.0),
        Length::Zero => LengthPercentage::Length(0.0),
        _ => LengthPercentage::Length(0.0),
    }
}

fn lpa(l: &Length) -> LengthPercentageAuto {
    match l {
        Length::Pt(v) => LengthPercentageAuto::Length(*v),
        Length::Px(v) => LengthPercentageAuto::Length(*v * 0.75),
        Length::Percent(v) => LengthPercentageAuto::Percent(v / 100.0),
        Length::Calc(c) => LengthPercentageAuto::Percent(c.percent / 100.0),
        Length::Auto => LengthPercentageAuto::Auto,
        Length::Zero => LengthPercentageAuto::Length(0.0),
        _ => LengthPercentageAuto::Auto,
    }
}

// =============================================================================
// CSS Grid → Taffy
// =============================================================================

fn apply_grid(style: &ComputedStyle, out: &mut taffy::Style) {
    let g = &style.grid;
    let fs = style.font_size;
    if let Some(v) = &g.template_columns {
        out.grid_template_columns = parse_track_list(v, fs);
    }
    if let Some(v) = &g.template_rows {
        out.grid_template_rows = parse_track_list(v, fs);
    }
    if let Some(v) = &g.auto_columns {
        out.grid_auto_columns = split_top_level(v)
            .iter()
            .filter_map(|t| parse_track(t, fs))
            .collect();
    }
    if let Some(v) = &g.auto_rows {
        out.grid_auto_rows = split_top_level(v)
            .iter()
            .filter_map(|t| parse_track(t, fs))
            .collect();
    }
    if let Some(v) = &g.auto_flow {
        let v = v.to_ascii_lowercase();
        out.grid_auto_flow = match (v.contains("column"), v.contains("dense")) {
            (true, true) => GridAutoFlow::ColumnDense,
            (true, false) => GridAutoFlow::Column,
            (false, true) => GridAutoFlow::RowDense,
            (false, false) => GridAutoFlow::Row,
        };
    }
    out.grid_column = taffy::Line {
        start: parse_placement(g.column_start.as_deref()),
        end: parse_placement(g.column_end.as_deref()),
    };
    out.grid_row = taffy::Line {
        start: parse_placement(g.row_start.as_deref()),
        end: parse_placement(g.row_end.as_deref()),
    };
    if let Some(v) = &g.justify_items {
        out.justify_items = parse_align_items(v);
    }
    if let Some(v) = &g.justify_self {
        out.justify_self = parse_align_items(v);
    }
}

fn parse_align_items(v: &str) -> Option<AlignItems> {
    match v.trim() {
        "start" | "flex-start" | "self-start" | "left" => Some(AlignItems::Start),
        "end" | "flex-end" | "self-end" | "right" => Some(AlignItems::End),
        "center" => Some(AlignItems::Center),
        "baseline" => Some(AlignItems::Baseline),
        "stretch" => Some(AlignItems::Stretch),
        _ => None, // normal / auto
    }
}

/// `auto`, `3`, `-1`, `span 2`, `2 / span 3` halves. Named lines are not supported.
fn parse_placement(v: Option<&str>) -> GridPlacement {
    let Some(v) = v.map(str::trim) else {
        return GridPlacement::Auto;
    };
    let mut words = v.split_whitespace();
    match (words.next(), words.next()) {
        (Some(w), Some(n)) if w.eq_ignore_ascii_case("span") => n
            .parse::<u16>()
            .ok()
            .filter(|n| *n > 0)
            .map(GridPlacement::Span)
            .unwrap_or(GridPlacement::Auto),
        (Some(n), None) => n
            .parse::<i16>()
            .ok()
            .filter(|n| *n != 0)
            .map(|n| GridPlacement::Line(n.into()))
            .unwrap_or(GridPlacement::Auto),
        _ => GridPlacement::Auto,
    }
}

/// `grid-template-*`: `1fr 2fr`, `repeat(3, 1fr)`, `repeat(auto-fill, minmax(200px, 1fr))`,
/// `200px auto minmax(100px, 30%)`, `[name]` line names ignored.
fn parse_track_list(v: &str, font_size: f32) -> Vec<TrackSizingFunction> {
    let v = v.trim();
    if v.eq_ignore_ascii_case("none") {
        return Vec::new();
    }
    let mut out = Vec::new();
    for tok in split_top_level(v) {
        if tok.starts_with('[') {
            continue;
        }
        if let Some(args) = function_args(tok, "repeat") {
            let mut parts = args.splitn(2, ',');
            let count = parts.next().unwrap_or("").trim().to_ascii_lowercase();
            let tracks: Vec<NonRepeatedTrackSizingFunction> =
                split_top_level(parts.next().unwrap_or(""))
                    .iter()
                    .filter(|t| !t.starts_with('['))
                    .filter_map(|t| parse_track(t, font_size))
                    .collect();
            if tracks.is_empty() {
                continue;
            }
            let repetition = match count.as_str() {
                "auto-fill" => GridTrackRepetition::AutoFill,
                "auto-fit" => GridTrackRepetition::AutoFit,
                n => match n.parse::<u16>() {
                    Ok(n) if n > 0 => GridTrackRepetition::Count(n),
                    _ => continue,
                },
            };
            out.push(TrackSizingFunction::Repeat(repetition, tracks));
        } else if let Some(track) = parse_track(tok, font_size) {
            out.push(TrackSizingFunction::Single(track));
        }
    }
    out
}

fn parse_track(t: &str, font_size: f32) -> Option<NonRepeatedTrackSizingFunction> {
    let t = t.trim();
    if let Some(args) = function_args(t, "minmax") {
        let mut it = split_comma(args).into_iter();
        let min = it.next()?;
        let max = it.next()?;
        let min = match min.to_ascii_lowercase().as_str() {
            "auto" => MinTrackSizingFunction::Auto,
            "min-content" => MinTrackSizingFunction::MinContent,
            "max-content" => MinTrackSizingFunction::MaxContent,
            m => MinTrackSizingFunction::Fixed(track_length(m, font_size)?),
        };
        let max = parse_max_track(max, font_size)?;
        return Some(MinMax { min, max });
    }
    if let Some(args) = function_args(t, "fit-content") {
        let lp = track_length(args.trim(), font_size)?;
        return Some(MinMax {
            min: MinTrackSizingFunction::Auto,
            max: MaxTrackSizingFunction::FitContent(lp),
        });
    }
    let max = parse_max_track(t, font_size)?;
    let min = match max {
        MaxTrackSizingFunction::Fixed(lp) => MinTrackSizingFunction::Fixed(lp),
        MaxTrackSizingFunction::MinContent => MinTrackSizingFunction::MinContent,
        MaxTrackSizingFunction::MaxContent => MinTrackSizingFunction::MaxContent,
        _ => MinTrackSizingFunction::Auto, // `1fr` = minmax(auto, 1fr)
    };
    Some(MinMax { min, max })
}

fn parse_max_track(t: &str, font_size: f32) -> Option<MaxTrackSizingFunction> {
    let t = t.trim();
    let lower = t.to_ascii_lowercase();
    Some(match lower.as_str() {
        "auto" => MaxTrackSizingFunction::Auto,
        "min-content" => MaxTrackSizingFunction::MinContent,
        "max-content" => MaxTrackSizingFunction::MaxContent,
        _ => {
            if let Some(n) = lower.strip_suffix("fr") {
                MaxTrackSizingFunction::Fraction(
                    n.trim().parse::<f32>().ok().filter(|f| *f >= 0.0)?,
                )
            } else {
                MaxTrackSizingFunction::Fixed(track_length(t, font_size)?)
            }
        }
    })
}

/// A length or percentage inside a track definition, in pt.
fn track_length(t: &str, font_size: f32) -> Option<LengthPercentage> {
    let t = t.trim().to_ascii_lowercase();
    if t == "0" {
        return Some(LengthPercentage::Length(0.0));
    }
    if t.starts_with("calc(") {
        let c = ferropdf_style::calc::parse_calc(&t)?.resolve_font_relative(font_size, 12.0);
        return match (c.percent != 0.0, c.pt != 0.0) {
            (true, false) => Some(LengthPercentage::Percent(c.percent / 100.0)),
            (false, _) => Some(LengthPercentage::Length(c.pt)),
            (true, true) => None, // mixed calc() in tracks: unsupported
        };
    }
    let num = |suffix: &str| {
        t.strip_suffix(suffix)
            .and_then(|n| n.trim().parse::<f32>().ok())
    };
    if let Some(v) = num("%") {
        return Some(LengthPercentage::Percent(v / 100.0));
    }
    let pt = if let Some(v) = num("rem") {
        v * 12.0
    } else if let Some(v) = num("em") {
        v * font_size
    } else if let Some(v) = num("px") {
        v * 0.75
    } else if let Some(v) = num("pt") {
        v
    } else if let Some(v) = num("mm") {
        v * 2.834_646
    } else if let Some(v) = num("cm") {
        v * 28.346_46
    } else {
        num("in")? * 72.0
    };
    Some(LengthPercentage::Length(pt))
}

/// Inner text of `name( … )` when `t` is exactly that function call.
fn function_args<'a>(t: &'a str, name: &str) -> Option<&'a str> {
    let open = t.find('(')?;
    (t[..open].trim().eq_ignore_ascii_case(name) && t.ends_with(')'))
        .then(|| &t[open + 1..t.len() - 1])
}

/// Split on whitespace outside parentheses.
fn split_top_level(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut start: Option<usize> = None;
    for (i, c) in s.char_indices() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            _ => {}
        }
        if c.is_whitespace() && depth == 0 {
            if let Some(st) = start.take() {
                parts.push(&s[st..i]);
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(st) = start {
        parts.push(&s[st..]);
    }
    parts
}

/// Split on commas outside parentheses.
fn split_comma(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut last = 0;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(s[last..i].trim());
                last = i + 1;
            }
            _ => {}
        }
    }
    parts.push(s[last..].trim());
    parts
}

#[cfg(test)]
mod grid_tests {
    use super::*;

    #[test]
    fn track_lists() {
        let t = parse_track_list("repeat(4, 1fr)", 12.0);
        assert!(
            matches!(&t[0], TrackSizingFunction::Repeat(GridTrackRepetition::Count(4), v) if v.len() == 1)
        );
        let t = parse_track_list("[a] 200px minmax(100px, 1fr) [b] auto 30%", 12.0);
        assert_eq!(t.len(), 4);
        let t = parse_track_list("repeat(auto-fill, minmax(10em, 1fr))", 10.0);
        let TrackSizingFunction::Repeat(GridTrackRepetition::AutoFill, v) = &t[0] else {
            panic!("{t:?}")
        };
        assert_eq!(
            v[0].min,
            MinTrackSizingFunction::Fixed(LengthPercentage::Length(100.0))
        );
        assert!(parse_track_list("none", 12.0).is_empty());
    }

    #[test]
    fn placements() {
        assert_eq!(parse_placement(Some("span 2")), GridPlacement::Span(2));
        assert_eq!(
            parse_placement(Some("-1")),
            GridPlacement::Line((-1).into())
        );
        assert_eq!(parse_placement(Some("auto")), GridPlacement::Auto);
        assert_eq!(parse_placement(None), GridPlacement::Auto);
    }
}
