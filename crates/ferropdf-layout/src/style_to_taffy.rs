use ferropdf_core::{ComputedStyle, Length};
use taffy::prelude::*;

/// Convert ComputedStyle → taffy::Style
/// Taffy automatically handles:
/// - width:100% resolved relative to containing block (Fix 1)
/// - padding subtracted only once (Fix 2)
/// - flex, grid, block layout
pub fn convert(style: &ComputedStyle) -> taffy::Style {
    taffy::Style {
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

        ..Default::default()
    }
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
        Length::Zero => LengthPercentage::Length(0.0),
        _ => LengthPercentage::Length(0.0),
    }
}

fn lpa(l: &Length) -> LengthPercentageAuto {
    match l {
        Length::Pt(v) => LengthPercentageAuto::Length(*v),
        Length::Px(v) => LengthPercentageAuto::Length(*v * 0.75),
        Length::Percent(v) => LengthPercentageAuto::Percent(v / 100.0),
        Length::Auto => LengthPercentageAuto::Auto,
        Length::Zero => LengthPercentageAuto::Length(0.0),
        _ => LengthPercentageAuto::Auto,
    }
}
