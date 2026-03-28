//! CSS Cascade: sort declarations by origin, specificity, and source order.
//!
//! Specificity comes from `selectors::Selector::specificity()` — we never
//! compute it by hand.

use crate::matching::ScoredDeclaration;
use ferropdf_core::*;
use ferropdf_parse::{CssProperty, CssValue, Declaration};

/// Apply declarations sorted by cascade order:
/// 1. Non-important, sorted by (specificity, source_order)
/// 2. Important, sorted by (specificity, source_order)
///
/// Within each group, higher specificity wins; equal specificity → later source order wins.
pub fn apply_scored_declarations(
    style: &mut ComputedStyle,
    scored: &mut [ScoredDeclaration],
    root_font_size: f32,
) {
    // Partition into non-important and important
    let mut non_important: Vec<&ScoredDeclaration> = scored
        .iter()
        .filter(|sd| !sd.declaration.important)
        .collect();
    let mut important: Vec<&ScoredDeclaration> = scored
        .iter()
        .filter(|sd| sd.declaration.important)
        .collect();

    // Sort by (origin, specificity, source_order) — all ascending, so last wins.
    // Origin: 0=UA, 1=author. Per CSS Cascading Level 4 §6.1,
    // author normal always beats UA normal regardless of specificity.
    non_important.sort_by_key(|sd| (sd.origin, sd.specificity, sd.source_order));
    important.sort_by_key(|sd| (sd.origin, sd.specificity, sd.source_order));

    // Apply in order: non-important first, then important overrides
    for sd in &non_important {
        apply_single(
            style,
            &sd.declaration.property,
            &sd.declaration.value,
            root_font_size,
        );
    }
    for sd in &important {
        apply_single(
            style,
            &sd.declaration.property,
            &sd.declaration.value,
            root_font_size,
        );
    }
}

/// Apply inline style declarations (no specificity needed — inline always wins
/// over stylesheet rules, but loses to !important stylesheet rules).
/// In our simplified cascade, we apply inline styles after all stylesheet rules.
pub fn apply_inline_declarations(
    style: &mut ComputedStyle,
    declarations: &[Declaration],
    root_font_size: f32,
) {
    let mut non_important: Vec<&Declaration> = Vec::new();
    let mut important: Vec<&Declaration> = Vec::new();

    for decl in declarations {
        if decl.important {
            important.push(decl);
        } else {
            non_important.push(decl);
        }
    }

    for decl in &non_important {
        apply_single(style, &decl.property, &decl.value, root_font_size);
    }
    for decl in &important {
        apply_single(style, &decl.property, &decl.value, root_font_size);
    }
}

fn apply_single(
    style: &mut ComputedStyle,
    property: &CssProperty,
    value: &CssValue,
    root_font_size: f32,
) {
    let cow = value.to_cow();
    let raw = cow.trim();

    match property {
        CssProperty::Display => {
            style.display = match raw {
                "block" => Display::Block,
                "inline" => Display::Inline,
                "inline-block" => Display::InlineBlock,
                "flex" => Display::Flex,
                "grid" => Display::Grid,
                "table" => Display::Table,
                "table-row" => Display::TableRow,
                "table-cell" => Display::TableCell,
                "table-header-group" => Display::TableHeaderGroup,
                "table-row-group" => Display::TableRowGroup,
                "table-footer-group" => Display::TableFooterGroup,
                "list-item" => Display::ListItem,
                "none" => Display::None,
                _ => Display::Block,
            };
        }
        CssProperty::Position => {
            style.position = match raw {
                "relative" => Position::Relative,
                "absolute" | "fixed" | "sticky" => {
                    log::warn!(
                        "ferropdf: position:{} is parsed but not rendered — treated as static",
                        raw
                    );
                    // Stored as Static; the warning is emitted via log for now.
                    Position::Static
                }
                _ => Position::Static,
            };
        }
        CssProperty::Width => style.width = parse_length(raw),
        CssProperty::Height => style.height = parse_length(raw),
        CssProperty::MinWidth => style.min_width = parse_length(raw),
        CssProperty::MaxWidth => style.max_width = parse_length(raw),
        CssProperty::MinHeight => style.min_height = parse_length(raw),
        CssProperty::MaxHeight => style.max_height = parse_length(raw),

        CssProperty::Margin => {
            let vals = parse_shorthand_lengths(raw);
            if vals.len() == 1 {
                style.margin = [vals[0]; 4];
            } else if vals.len() == 2 {
                style.margin = [vals[0], vals[1], vals[0], vals[1]];
            } else if vals.len() == 3 {
                style.margin = [vals[0], vals[1], vals[2], vals[1]];
            } else if vals.len() >= 4 {
                style.margin = [vals[0], vals[1], vals[2], vals[3]];
            }
        }
        CssProperty::MarginTop => style.margin[0] = parse_length(raw),
        CssProperty::MarginRight => style.margin[1] = parse_length(raw),
        CssProperty::MarginBottom => style.margin[2] = parse_length(raw),
        CssProperty::MarginLeft => style.margin[3] = parse_length(raw),

        CssProperty::Padding => {
            let vals = parse_shorthand_lengths(raw);
            if vals.len() == 1 {
                style.padding = [vals[0]; 4];
            } else if vals.len() == 2 {
                style.padding = [vals[0], vals[1], vals[0], vals[1]];
            } else if vals.len() == 3 {
                style.padding = [vals[0], vals[1], vals[2], vals[1]];
            } else if vals.len() >= 4 {
                style.padding = [vals[0], vals[1], vals[2], vals[3]];
            }
        }
        CssProperty::PaddingTop => style.padding[0] = parse_length(raw),
        CssProperty::PaddingRight => style.padding[1] = parse_length(raw),
        CssProperty::PaddingBottom => style.padding[2] = parse_length(raw),
        CssProperty::PaddingLeft => style.padding[3] = parse_length(raw),

        CssProperty::Border => {
            let side = parse_border_shorthand(raw);
            style.border_top = side;
            style.border_right = side;
            style.border_bottom = side;
            style.border_left = side;
        }
        CssProperty::BorderTop => style.border_top = parse_border_shorthand(raw),
        CssProperty::BorderRight => style.border_right = parse_border_shorthand(raw),
        CssProperty::BorderBottom => style.border_bottom = parse_border_shorthand(raw),
        CssProperty::BorderLeft => style.border_left = parse_border_shorthand(raw),
        CssProperty::BorderWidth => {
            if let Some(w) = parse_length_to_pt(raw) {
                style.border_top.width = w;
                style.border_right.width = w;
                style.border_bottom.width = w;
                style.border_left.width = w;
            }
        }
        CssProperty::BorderColor => {
            if let Some(c) = parse_color(raw) {
                style.border_top.color = c;
                style.border_right.color = c;
                style.border_bottom.color = c;
                style.border_left.color = c;
            }
        }
        CssProperty::BorderStyle => {
            let bs = parse_border_style(raw);
            style.border_top.style = bs;
            style.border_right.style = bs;
            style.border_bottom.style = bs;
            style.border_left.style = bs;
        }
        CssProperty::BorderRadius => {
            if let Some(r) = parse_length_to_pt(raw) {
                style.border_radius = BorderRadius::uniform(r);
            }
        }

        CssProperty::Color => {
            if let Some(c) = parse_color(raw) {
                style.color = c;
            }
        }
        CssProperty::BackgroundColor | CssProperty::Background => {
            if let Some(c) = parse_color(raw) {
                style.background_color = c;
            }
        }
        CssProperty::Opacity => {
            if let Ok(v) = raw.parse::<f32>() {
                style.opacity = v.clamp(0.0, 1.0);
            }
        }

        CssProperty::FontFamily => {
            style.font_family = raw
                .split(',')
                .map(|s| s.trim().trim_matches(|c| c == '"' || c == '\'').to_string())
                .collect();
        }
        CssProperty::FontSize => {
            style.font_size = resolve_font_size(raw, style.font_size, root_font_size);
        }
        CssProperty::FontWeight => {
            style.font_weight = match raw {
                "bold" | "700" => FontWeight::Bold,
                "normal" | "400" => FontWeight::Normal,
                "bolder" => FontWeight::Bolder,
                "lighter" => FontWeight::Lighter,
                "100" => FontWeight::W100,
                "200" => FontWeight::W200,
                "300" => FontWeight::W300,
                "500" => FontWeight::W500,
                "600" => FontWeight::W600,
                "800" => FontWeight::W800,
                "900" => FontWeight::W900,
                _ => FontWeight::Normal,
            };
        }
        CssProperty::FontStyle => {
            style.font_style = match raw {
                "italic" => FontStyle::Italic,
                "oblique" => FontStyle::Oblique,
                _ => FontStyle::Normal,
            };
        }
        CssProperty::LineHeight => {
            if let Some(pt) = parse_length_to_pt(raw) {
                style.line_height = pt;
            } else if let Ok(factor) = raw.parse::<f32>() {
                style.line_height = factor * style.font_size;
            }
        }
        CssProperty::TextAlign => {
            style.text_align = match raw {
                "center" => TextAlign::Center,
                "right" => TextAlign::Right,
                "justify" => TextAlign::Justify,
                _ => TextAlign::Left,
            };
        }
        CssProperty::TextDecoration => {
            style.text_decoration = match raw {
                "underline" => ferropdf_core::style::TextDecoration::Underline,
                "line-through" => ferropdf_core::style::TextDecoration::LineThrough,
                "overline" => ferropdf_core::style::TextDecoration::Overline,
                _ => ferropdf_core::style::TextDecoration::None,
            };
        }
        CssProperty::LetterSpacing => {
            if let Some(pt) = parse_length_to_pt(raw) {
                style.letter_spacing = pt;
            }
        }

        CssProperty::FlexDirection => {
            style.flex_direction = match raw {
                "column" => FlexDirection::Column,
                "row-reverse" => FlexDirection::RowReverse,
                "column-reverse" => FlexDirection::ColumnReverse,
                _ => FlexDirection::Row,
            };
        }
        CssProperty::FlexWrap => {
            style.flex_wrap = match raw {
                "wrap" => FlexWrap::Wrap,
                "wrap-reverse" => FlexWrap::WrapReverse,
                _ => FlexWrap::NoWrap,
            };
        }
        CssProperty::JustifyContent => {
            style.justify_content = match raw {
                "center" => JustifyContent::Center,
                "flex-end" => JustifyContent::FlexEnd,
                "space-between" => JustifyContent::SpaceBetween,
                "space-around" => JustifyContent::SpaceAround,
                "space-evenly" => JustifyContent::SpaceEvenly,
                _ => JustifyContent::FlexStart,
            };
        }
        CssProperty::AlignItems => {
            style.align_items = match raw {
                "center" => AlignItems::Center,
                "flex-start" => AlignItems::FlexStart,
                "flex-end" => AlignItems::FlexEnd,
                "baseline" => AlignItems::Baseline,
                _ => AlignItems::Stretch,
            };
        }
        CssProperty::AlignSelf => {
            style.align_self = match raw {
                "center" => AlignSelf::Center,
                "flex-start" => AlignSelf::FlexStart,
                "flex-end" => AlignSelf::FlexEnd,
                "stretch" => AlignSelf::Stretch,
                "baseline" => AlignSelf::Baseline,
                _ => AlignSelf::Auto,
            };
        }
        CssProperty::Flex => {
            let parts: Vec<&str> = raw.split_whitespace().collect();
            if let Some(g) = parts.first().and_then(|s| s.parse::<f32>().ok()) {
                style.flex_grow = g;
            }
            if let Some(s) = parts.get(1).and_then(|s| s.parse::<f32>().ok()) {
                style.flex_shrink = s;
            }
            if let Some(b) = parts.get(2) {
                style.flex_basis = parse_length(b);
            }
        }
        CssProperty::FlexGrow => {
            if let Ok(v) = raw.parse::<f32>() {
                style.flex_grow = v;
            }
        }
        CssProperty::FlexShrink => {
            if let Ok(v) = raw.parse::<f32>() {
                style.flex_shrink = v;
            }
        }
        CssProperty::FlexBasis => style.flex_basis = parse_length(raw),
        CssProperty::Gap => {
            let l = parse_length(raw);
            style.column_gap = l;
            style.row_gap = l;
        }
        CssProperty::ColumnGap => style.column_gap = parse_length(raw),
        CssProperty::RowGap => style.row_gap = parse_length(raw),

        CssProperty::PageBreakBefore => {
            style.page_break_before = match raw {
                "always" => PageBreak::Always,
                "page" => PageBreak::Page,
                "left" => PageBreak::Left,
                "right" => PageBreak::Right,
                "avoid" => PageBreak::Avoid,
                _ => PageBreak::Auto,
            };
        }
        CssProperty::PageBreakAfter => {
            style.page_break_after = match raw {
                "always" => PageBreak::Always,
                "page" => PageBreak::Page,
                "left" => PageBreak::Left,
                "right" => PageBreak::Right,
                "avoid" => PageBreak::Avoid,
                _ => PageBreak::Auto,
            };
        }
        CssProperty::PageBreakInside => {
            style.page_break_inside = match raw {
                "avoid" => PageBreakInside::Avoid,
                _ => PageBreakInside::Auto,
            };
        }
        CssProperty::Orphans => {
            if let Ok(v) = raw.parse::<u32>() {
                style.orphans = v;
            }
        }
        CssProperty::Widows => {
            if let Ok(v) = raw.parse::<u32>() {
                style.widows = v;
            }
        }
        CssProperty::Unknown(ref name) if name == "box-decoration-break" => {
            style.box_decoration_break = match raw {
                "clone" => ferropdf_core::BoxDecorationBreak::Clone,
                _ => ferropdf_core::BoxDecorationBreak::Slice,
            };
        }
        CssProperty::Visibility => {
            style.visibility = raw != "hidden";
        }
        CssProperty::BorderCollapse => {
            style.border_collapse = match raw {
                "collapse" => ferropdf_core::BorderCollapse::Collapse,
                _ => ferropdf_core::BorderCollapse::Separate,
            };
        }
        CssProperty::ListStyleType => {
            style.list_style_type = match raw {
                "disc" => ferropdf_core::ListStyleType::Disc,
                "circle" => ferropdf_core::ListStyleType::Circle,
                "square" => ferropdf_core::ListStyleType::Square,
                "decimal" => ferropdf_core::ListStyleType::Decimal,
                "decimal-leading-zero" => ferropdf_core::ListStyleType::DecimalLeadingZero,
                "lower-alpha" | "lower-latin" => ferropdf_core::ListStyleType::LowerAlpha,
                "upper-alpha" | "upper-latin" => ferropdf_core::ListStyleType::UpperAlpha,
                "lower-roman" => ferropdf_core::ListStyleType::LowerRoman,
                "upper-roman" => ferropdf_core::ListStyleType::UpperRoman,
                "none" => ferropdf_core::ListStyleType::None,
                _ => ferropdf_core::ListStyleType::Disc,
            };
        }

        _ => {} // Unknown or not yet handled
    }
}

fn parse_length(s: &str) -> Length {
    let s = s.trim();
    if s == "auto" {
        return Length::Auto;
    }
    if s == "0" || s == "0px" {
        return Length::Zero;
    }
    if s == "none" {
        return Length::None;
    }

    if let Some(v) = s.strip_suffix('%') {
        if let Ok(v) = v.trim().parse::<f32>() {
            return Length::Percent(v);
        }
    }

    for (suffix, ctor) in &[
        ("px", Length::Px as fn(f32) -> Length),
        ("pt", Length::Pt),
        ("mm", Length::Mm),
        ("em", Length::Em),
        ("rem", Length::Rem),
    ] {
        if let Some(v) = s.strip_suffix(suffix) {
            if let Ok(v) = v.trim().parse::<f32>() {
                return ctor(v);
            }
        }
    }

    // Bare number → px
    if let Ok(v) = s.parse::<f32>() {
        return Length::Px(v);
    }

    Length::Auto
}

fn parse_shorthand_lengths(s: &str) -> Vec<Length> {
    s.split_whitespace().map(parse_length).collect()
}

/// Parse a CSS length string directly to pt (points typographiques).
/// Used for properties stored as f32 (border widths, border-radius, etc.)
fn parse_length_to_pt(s: &str) -> Option<f32> {
    let s = s.trim();
    if s == "0" || s == "0px" {
        return Some(0.0);
    }

    if let Some(v) = s.strip_suffix("px") {
        return v.trim().parse::<f32>().ok().map(|v| v * 0.75);
    }
    if let Some(v) = s.strip_suffix("pt") {
        return v.trim().parse::<f32>().ok();
    }
    if let Some(v) = s.strip_suffix("mm") {
        return v.trim().parse::<f32>().ok().map(|v| v * 2.834_646);
    }

    // Bare number → treated as px
    s.parse::<f32>().ok().map(|v| v * 0.75)
}

fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim();
    match s {
        "transparent" => Some(Color::transparent()),
        "black" => Some(Color::black()),
        "white" => Some(Color::white()),
        "red" => Some(Color::from_rgb8(255, 0, 0)),
        "green" => Some(Color::from_rgb8(0, 128, 0)),
        "blue" => Some(Color::from_rgb8(0, 0, 255)),
        "gray" | "grey" => Some(Color::from_rgb8(128, 128, 128)),
        "orange" => Some(Color::from_rgb8(255, 165, 0)),
        "yellow" => Some(Color::from_rgb8(255, 255, 0)),
        "purple" => Some(Color::from_rgb8(128, 0, 128)),
        _ if s.starts_with('#') => Color::from_hex(s),
        _ if s.starts_with("rgb") => parse_rgb_function(s),
        _ => None,
    }
}

fn parse_rgb_function(s: &str) -> Option<Color> {
    let inner = s
        .trim_start_matches("rgba(")
        .trim_start_matches("rgb(")
        .trim_end_matches(')');
    let parts: Vec<&str> = inner.split([',', ' ']).filter(|s| !s.is_empty()).collect();
    if parts.len() >= 3 {
        let r = parts[0].trim().parse::<u8>().ok()?;
        let g = parts[1].trim().parse::<u8>().ok()?;
        let b = parts[2].trim().parse::<u8>().ok()?;
        if parts.len() >= 4 {
            let a = parts[3].trim().parse::<f32>().ok().unwrap_or(1.0);
            Some(Color::new(
                r as f32 / 255.0,
                g as f32 / 255.0,
                b as f32 / 255.0,
                a,
            ))
        } else {
            Some(Color::from_rgb8(r, g, b))
        }
    } else {
        None
    }
}

fn parse_border_shorthand(s: &str) -> BorderSide {
    let parts: Vec<&str> = s.split_whitespace().collect();
    let mut side = BorderSide::default();

    for part in &parts {
        if let Some(w) = parse_length_to_pt(part) {
            side.width = w;
            if side.style == BorderStyle::None {
                side.style = BorderStyle::Solid;
            }
        } else if let Some(c) = parse_color(part) {
            side.color = c;
        } else {
            side.style = parse_border_style(part);
        }
    }

    side
}

fn parse_border_style(s: &str) -> BorderStyle {
    match s.trim() {
        "solid" => BorderStyle::Solid,
        "dashed" => BorderStyle::Dashed,
        "dotted" => BorderStyle::Dotted,
        "double" => BorderStyle::Double,
        _ => BorderStyle::None,
    }
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_length_px() {
        assert_eq!(parse_length("10px"), Length::Px(10.0));
    }

    #[test]
    fn parse_length_pt() {
        assert_eq!(parse_length("12pt"), Length::Pt(12.0));
    }

    #[test]
    fn parse_length_em() {
        assert_eq!(parse_length("2em"), Length::Em(2.0));
    }

    #[test]
    fn parse_length_rem() {
        assert_eq!(parse_length("1.5rem"), Length::Rem(1.5));
    }

    #[test]
    fn parse_length_percent() {
        assert_eq!(parse_length("50%"), Length::Percent(50.0));
    }

    #[test]
    fn parse_length_auto() {
        assert_eq!(parse_length("auto"), Length::Auto);
    }

    #[test]
    fn parse_length_zero() {
        assert_eq!(parse_length("0"), Length::Zero);
        assert_eq!(parse_length("0px"), Length::Zero);
    }

    #[test]
    fn parse_length_bare_number() {
        assert_eq!(parse_length("16"), Length::Px(16.0));
    }

    #[test]
    fn parse_length_to_pt_px() {
        let result = parse_length_to_pt("16px").unwrap();
        assert!(
            (result - 12.0).abs() < 0.01,
            "16px should be 12pt, got {}",
            result
        );
    }

    #[test]
    fn parse_length_to_pt_mm() {
        let result = parse_length_to_pt("10mm").unwrap();
        assert!(
            (result - 28.346).abs() < 0.01,
            "10mm should be ~28.35pt, got {}",
            result
        );
    }

    #[test]
    fn parse_color_named() {
        assert_eq!(parse_color("red"), Some(Color::from_rgb8(255, 0, 0)));
        assert_eq!(parse_color("transparent"), Some(Color::transparent()));
    }

    #[test]
    fn parse_color_hex() {
        let c = parse_color("#ff0000").unwrap();
        assert!((c.r - 1.0).abs() < 0.01);
        assert!(c.g.abs() < 0.01);
    }

    #[test]
    fn parse_color_rgb() {
        let c = parse_color("rgb(128, 64, 32)").unwrap();
        assert!((c.r - 128.0 / 255.0).abs() < 0.01);
        assert!((c.g - 64.0 / 255.0).abs() < 0.01);
    }

    #[test]
    fn parse_color_rgba() {
        let c = parse_color("rgba(255, 0, 0, 0.5)").unwrap();
        assert!((c.r - 1.0).abs() < 0.01);
        assert!((c.a - 0.5).abs() < 0.01);
    }

    #[test]
    fn apply_display_block() {
        let mut style = ComputedStyle::default();
        apply_single(
            &mut style,
            &CssProperty::Display,
            &CssValue::String("flex".into()),
            12.0,
        );
        assert_eq!(style.display, Display::Flex);
    }

    #[test]
    fn apply_font_weight() {
        let mut style = ComputedStyle::default();
        apply_single(
            &mut style,
            &CssProperty::FontWeight,
            &CssValue::String("bold".into()),
            12.0,
        );
        assert_eq!(style.font_weight, FontWeight::Bold);
        assert!(style.font_weight.is_bold());
    }

    #[test]
    fn apply_margin_shorthand_two_values() {
        let mut style = ComputedStyle::default();
        apply_single(
            &mut style,
            &CssProperty::Margin,
            &CssValue::String("10px 20px".into()),
            12.0,
        );
        assert_eq!(style.margin[0], Length::Px(10.0)); // top
        assert_eq!(style.margin[1], Length::Px(20.0)); // right
        assert_eq!(style.margin[2], Length::Px(10.0)); // bottom
        assert_eq!(style.margin[3], Length::Px(20.0)); // left
    }

    #[test]
    fn apply_border_shorthand() {
        let mut style = ComputedStyle::default();
        apply_single(
            &mut style,
            &CssProperty::Border,
            &CssValue::String("2px solid red".into()),
            12.0,
        );
        assert!((style.border_top.width - 1.5).abs() < 0.01); // 2px = 1.5pt
        assert_eq!(style.border_top.style, BorderStyle::Solid);
    }

    #[test]
    fn apply_opacity() {
        let mut style = ComputedStyle::default();
        apply_single(
            &mut style,
            &CssProperty::Opacity,
            &CssValue::String("0.5".into()),
            12.0,
        );
        assert!((style.opacity - 0.5).abs() < 0.01);
    }

    #[test]
    fn resolve_font_size_em() {
        let result = resolve_font_size("2em", 12.0, 12.0);
        assert!(
            (result - 24.0).abs() < 0.01,
            "2em of 12pt = 24pt, got {}",
            result
        );
    }

    #[test]
    fn resolve_font_size_rem() {
        let result = resolve_font_size("1.5rem", 24.0, 12.0);
        assert!(
            (result - 18.0).abs() < 0.01,
            "1.5rem of 12pt root = 18pt, got {}",
            result
        );
    }

    #[test]
    fn resolve_font_size_keyword() {
        assert!((resolve_font_size("medium", 12.0, 12.0) - 12.0).abs() < 0.01);
        assert!((resolve_font_size("large", 12.0, 12.0) - 13.5).abs() < 0.01);
    }

    #[test]
    fn resolve_font_size_percent() {
        let result = resolve_font_size("150%", 12.0, 12.0);
        assert!(
            (result - 18.0).abs() < 0.01,
            "150% of 12pt = 18pt, got {}",
            result
        );
    }
}

fn resolve_font_size(raw: &str, parent_size: f32, root_font_size: f32) -> f32 {
    let raw = raw.trim();

    // Font-size keywords (values in pt: 16px default = 12pt)
    match raw {
        "xx-small" => return 6.75, // 9px × 0.75
        "x-small" => return 7.5,   // 10px × 0.75
        "small" => return 9.75,    // 13px × 0.75
        "medium" => return 12.0,   // 16px × 0.75
        "large" => return 13.5,    // 18px × 0.75
        "x-large" => return 18.0,  // 24px × 0.75
        "xx-large" => return 24.0, // 32px × 0.75
        "smaller" => return parent_size * 0.833,
        "larger" => return parent_size * 1.2,
        _ => {}
    }

    if let Some(v) = raw.strip_suffix("em") {
        if let Ok(v) = v.trim().parse::<f32>() {
            return v * parent_size;
        }
    }
    if let Some(v) = raw.strip_suffix("rem") {
        if let Ok(v) = v.trim().parse::<f32>() {
            return v * root_font_size;
        }
    }
    if let Some(v) = raw.strip_suffix('%') {
        if let Ok(v) = v.trim().parse::<f32>() {
            return v / 100.0 * parent_size;
        }
    }
    if let Some(v) = raw.strip_suffix("px") {
        if let Ok(v) = v.trim().parse::<f32>() {
            return v * 0.75; // 1px = 72/96 pt
        }
    }
    if let Some(v) = raw.strip_suffix("pt") {
        if let Ok(v) = v.trim().parse::<f32>() {
            return v; // pt is the internal unit
        }
    }
    if let Some(v) = raw.strip_suffix("mm") {
        if let Ok(v) = v.trim().parse::<f32>() {
            return v * 2.834_646;
        }
    }

    if let Ok(v) = raw.parse::<f32>() {
        return v * 0.75; // bare numbers treated as px
    }

    parent_size
}
