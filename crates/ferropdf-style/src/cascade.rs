//! CSS Cascade: sort declarations by origin, specificity, and source order.
//!
//! Specificity comes from `selectors::Selector::specificity()` — we never
//! compute it by hand.

use crate::matching::ScoredDeclaration;
use ferropdf_core::*;
use ferropdf_parse::{CssProperty, CssValue, Declaration};
use std::borrow::Cow;
use std::collections::HashMap;

/// Substitute `var(--name)` and `var(--name, fallback)` references in `raw`
/// using the given custom-property map. Substitution is bounded to a small
/// number of expansion rounds to defeat cycles like
/// `--a: var(--b); --b: var(--a)`. Unresolved `var()` calls are left in
/// place so downstream parsers can reject them gracefully.
fn substitute_var(raw: &str, props: &HashMap<String, String>) -> String {
    const MAX_ROUNDS: usize = 8;
    let mut current = raw.to_string();
    for _ in 0..MAX_ROUNDS {
        let next = substitute_var_once(&current, props);
        if next == current {
            return next;
        }
        current = next;
    }
    current
}

fn substitute_var_once(raw: &str, props: &HashMap<String, String>) -> String {
    // Find the first "var(" — early exit for the common case of no var().
    let Some(start) = raw.find("var(") else {
        return raw.to_string();
    };
    let mut out = String::with_capacity(raw.len());
    out.push_str(&raw[..start]);

    let body = &raw[start + 4..];
    // Walk byte-by-byte until the matching `)` so we handle nested
    // function calls like `rgb(var(--c-r), 0, 0)` correctly.
    let bytes = body.as_bytes();
    let mut depth: i32 = 1;
    let mut end = None;
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    let Some(end) = end else {
        // Unbalanced — bail and return the original so downstream sees it.
        return raw.to_string();
    };

    let args = &body[..end];
    let rest = &body[end + 1..];

    let (name, fallback) = match args.split_once(',') {
        Some((n, f)) => (n.trim(), Some(f.trim())),
        None => (args.trim(), None),
    };

    let resolved = props
        .get(name)
        .cloned()
        .or_else(|| fallback.map(|s| s.to_string()))
        .unwrap_or_default();

    out.push_str(&resolved);
    out.push_str(&substitute_var_once(rest, props));
    out
}

/// Apply a value through `var()` substitution, returning a borrowed `Cow`
/// when no rewrite was needed (the common case).
fn resolve_value<'a>(raw: &'a str, props: &HashMap<String, String>) -> Cow<'a, str> {
    if !raw.contains("var(") {
        return Cow::Borrowed(raw);
    }
    Cow::Owned(substitute_var(raw, props))
}

/// Pull custom-property declarations out of a list of scored declarations
/// (in cascade order) and merge them into `style.custom_properties`.
/// Already-inherited values are overridden when a higher-priority
/// declaration redefines the same name.
fn collect_custom_properties_scored(style: &mut ComputedStyle, scored: &[&ScoredDeclaration]) {
    for sd in scored {
        if let CssProperty::Unknown(name) = &sd.declaration.property {
            if name.starts_with("--") {
                let value = sd.declaration.value.to_cow().trim().to_string();
                style.custom_properties.insert(name.clone(), value);
            }
        }
    }
}

/// Same as the scored variant but for inline style declarations.
fn collect_custom_properties_inline(style: &mut ComputedStyle, decls: &[&Declaration]) {
    for d in decls {
        if let CssProperty::Unknown(name) = &d.property {
            if name.starts_with("--") {
                let value = d.value.to_cow().trim().to_string();
                style.custom_properties.insert(name.clone(), value);
            }
        }
    }
}

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

    // First, fold custom properties (`--x: val`) into the style. Custom
    // properties cascade like normal declarations, so we walk the same
    // sorted lists. They must be merged before `apply_single` runs so
    // `var(--x)` references resolve correctly even when the `--x`
    // declaration appears in the same rule block as a `var()` user.
    collect_custom_properties_scored(style, &non_important);
    collect_custom_properties_scored(style, &important);

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

    collect_custom_properties_inline(style, &non_important);
    collect_custom_properties_inline(style, &important);

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
    // Resolve any `var(--name)` references against the style's custom
    // property map (which already includes inherited values plus any
    // declarations collected from the same rule before apply_single ran).
    let resolved = resolve_value(cow.as_ref(), &style.custom_properties);
    let raw = resolved.trim();

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
        CssProperty::Position if raw.starts_with("running(") => {
            let name = raw["running(".len()..].trim_end_matches(')').trim();
            if !name.is_empty() {
                style.running = Some(name.to_string());
            }
        }
        CssProperty::Position => {
            style.position = match raw {
                "relative" => Position::Relative,
                "absolute" => Position::Absolute,
                "fixed" => Position::Fixed,
                "sticky" => Position::Sticky,
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
            if let Some(r) = parse_border_radius(raw) {
                style.border_radius = r;
            }
        }
        CssProperty::Unknown(ref name)
            if matches!(
                name.as_str(),
                "border-top-left-radius"
                    | "border-top-right-radius"
                    | "border-bottom-right-radius"
                    | "border-bottom-left-radius"
            ) =>
        {
            let corner = match name.as_str() {
                "border-top-left-radius" => 0,
                "border-top-right-radius" => 1,
                "border-bottom-right-radius" => 2,
                _ => 3,
            };
            let values: Option<Vec<Length>> =
                crate::gradient::split_top_level(raw, char::is_whitespace)
                    .into_iter()
                    .map(parse_radius_length)
                    .collect();
            match values.as_deref() {
                Some([r]) => style.border_radius.corners[corner] = (*r, *r),
                Some([x, y]) => style.border_radius.corners[corner] = (*x, *y),
                _ => {}
            }
        }

        CssProperty::Color => {
            if let Some(c) = parse_color(raw) {
                style.color = c;
            }
        }
        CssProperty::BackgroundColor => {
            if let Some(c) = parse_color(raw) {
                style.background_color = c;
            }
        }
        CssProperty::Background => {
            // The shorthand resets the colour and the image layers.
            if let Some(bg) = crate::gradient::parse_background(raw) {
                style.background_color = bg.color;
                style.background_gradients = bg.gradients;
            }
        }
        CssProperty::Unknown(ref name)
            if matches!(
                name.as_str(),
                "transform" | "translate" | "rotate" | "scale" | "transform-origin"
            ) =>
        {
            use crate::transform as t;
            match name.as_str() {
                "transform" => {
                    if let Some(v) = t::parse_transform(raw) {
                        style.transform = v;
                    }
                }
                "translate" => {
                    if let Some(v) = t::parse_translate(raw) {
                        style.transform_individual[0] = v;
                    }
                }
                "rotate" => {
                    if let Some(v) = t::parse_rotate(raw) {
                        style.transform_individual[1] = v;
                    }
                }
                "scale" => {
                    if let Some(v) = t::parse_scale(raw) {
                        style.transform_individual[2] = v;
                    }
                }
                _ => {
                    if let Some(v) = t::parse_transform_origin(raw) {
                        style.transform_origin = v;
                    }
                }
            }
        }
        CssProperty::Unknown(ref name) if name == "content" => {
            style.content = Some(raw.to_string());
        }
        CssProperty::Unknown(ref name)
            if matches!(
                name.as_str(),
                "counter-reset" | "counter-increment" | "counter-set"
            ) =>
        {
            let default = if name == "counter-increment" { 1 } else { 0 };
            if let Some(list) = parse_counter_list(raw, default) {
                match name.as_str() {
                    "counter-reset" => style.counter_reset = list,
                    "counter-increment" => style.counter_increment = list,
                    _ => style.counter_set = list,
                }
            }
        }
        CssProperty::Unknown(ref name) if name == "background-image" => {
            if let Some(g) = crate::gradient::parse_background_image(raw) {
                style.background_gradients = g;
            }
        }
        CssProperty::Opacity => {
            if let Ok(v) = raw.parse::<f32>() {
                style.opacity = v.clamp(0.0, 1.0);
            }
        }

        CssProperty::Direction => {
            style.direction = match raw {
                "rtl" => Direction::Rtl,
                _ => Direction::Ltr,
            };
        }
        CssProperty::UnicodeBidi => {} // handled implicitly by cosmic-text's bidi algorithm

        CssProperty::Top => style.top = parse_length(raw),
        CssProperty::Right => style.right = parse_length(raw),
        CssProperty::Bottom => style.bottom = parse_length(raw),
        CssProperty::Left => style.left = parse_length(raw),
        CssProperty::ZIndex => {
            if let Ok(v) = raw.parse::<i32>() {
                style.z_index = v;
            }
        }

        CssProperty::BoxShadow => {
            if raw == "none" {
                style.box_shadow.clear();
            } else {
                style.box_shadow = parse_box_shadows(raw);
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
            if let Some(a) = match raw {
                "start" => Some(TextAlign::Start),
                "end" => Some(TextAlign::End),
                "left" => Some(TextAlign::Left),
                "center" => Some(TextAlign::Center),
                "right" => Some(TextAlign::Right),
                "justify" => Some(TextAlign::Justify),
                _ => None,
            } {
                style.text_align = a;
            }
        }
        CssProperty::TextDecoration => {
            style.text_decoration = match raw {
                "underline" => ferropdf_core::style::TextDecoration::Underline,
                "line-through" => ferropdf_core::style::TextDecoration::LineThrough,
                "overline" => ferropdf_core::style::TextDecoration::Overline,
                _ => ferropdf_core::style::TextDecoration::None,
            };
        }
        CssProperty::BoxSizing => {
            style.box_sizing = match raw {
                "border-box" => BoxSizing::BorderBox,
                _ => BoxSizing::ContentBox,
            };
        }
        CssProperty::WhiteSpace => {
            style.white_space = match raw {
                "nowrap" => WhiteSpace::Nowrap,
                "pre" => WhiteSpace::Pre,
                "pre-wrap" | "break-spaces" => WhiteSpace::PreWrap,
                "pre-line" => WhiteSpace::PreLine,
                _ => WhiteSpace::Normal,
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
                "flex-end" | "end" | "right" => JustifyContent::FlexEnd,
                "space-between" => JustifyContent::SpaceBetween,
                "space-around" => JustifyContent::SpaceAround,
                "space-evenly" => JustifyContent::SpaceEvenly,
                _ => JustifyContent::FlexStart,
            };
        }
        CssProperty::AlignItems => {
            style.align_items = match raw {
                "center" => AlignItems::Center,
                "flex-start" | "start" | "self-start" => AlignItems::FlexStart,
                "flex-end" | "end" | "self-end" => AlignItems::FlexEnd,
                "baseline" => AlignItems::Baseline,
                _ => AlignItems::Stretch,
            };
        }
        CssProperty::AlignSelf => {
            style.align_self = match raw {
                "center" => AlignSelf::Center,
                "flex-start" | "start" | "self-start" => AlignSelf::FlexStart,
                "flex-end" | "end" | "self-end" => AlignSelf::FlexEnd,
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
            // `gap: <row> [<column>]`
            let parts = parse_shorthand_lengths(raw);
            let row = parts.first().copied().unwrap_or(Length::Zero);
            style.row_gap = row;
            style.column_gap = parts.get(1).copied().unwrap_or(row);
        }
        CssProperty::ColumnGap => style.column_gap = parse_length(raw),
        CssProperty::RowGap => style.row_gap = parse_length(raw),

        CssProperty::PageBreakBefore => {
            style.page_break_before = match raw {
                "always" => PageBreak::Always,
                "page" | "recto" | "verso" => PageBreak::Page,
                "left" => PageBreak::Left,
                "right" => PageBreak::Right,
                "avoid" | "avoid-page" => PageBreak::Avoid,
                _ => PageBreak::Auto,
            };
        }
        CssProperty::PageBreakAfter => {
            style.page_break_after = match raw {
                "always" => PageBreak::Always,
                "page" | "recto" | "verso" => PageBreak::Page,
                "left" => PageBreak::Left,
                "right" => PageBreak::Right,
                "avoid" | "avoid-page" => PageBreak::Avoid,
                _ => PageBreak::Auto,
            };
        }
        CssProperty::PageBreakInside => {
            style.page_break_inside = match raw {
                "avoid" | "avoid-page" => PageBreakInside::Avoid,
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
        CssProperty::Unknown(ref name)
            if matches!(name.as_str(), "overflow" | "overflow-x" | "overflow-y") =>
        {
            let clips = |v: &str| matches!(v, "hidden" | "clip" | "auto" | "scroll");
            let mut parts = raw.split_whitespace();
            let first = parts.next().unwrap_or("visible");
            let second = parts.next().unwrap_or(first);
            match name.as_str() {
                "overflow-x" => style.overflow_clip_x = clips(first),
                "overflow-y" => style.overflow_clip_y = clips(first),
                _ => {
                    style.overflow_clip_x = clips(first);
                    style.overflow_clip_y = clips(second);
                }
            }
        }
        CssProperty::Unknown(ref name) if name == "float" => {
            style.float = match raw {
                "left" | "inline-start" => Float::Left,
                "right" | "inline-end" => Float::Right,
                _ => Float::None,
            };
        }
        CssProperty::Unknown(ref name) if name == "clear" => {
            style.clear = match raw {
                "left" | "inline-start" => Clear::Left,
                "right" | "inline-end" => Clear::Right,
                "both" => Clear::Both,
                _ => Clear::None,
            };
        }
        CssProperty::Unknown(ref name) if apply_grid_property(style, name, raw) => {}
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
    if s.len() > 5 && s[..5].eq_ignore_ascii_case("calc(") {
        return crate::calc::parse_calc(s)
            .map(|c| c.simplify())
            .unwrap_or(Length::Auto);
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
        ("cm", |v| Length::Mm(v * 10.0)),
        ("in", |v| Length::Pt(v * 72.0)),
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

/// Grid properties (stored raw, converted by the layout crate). Returns
/// whether `name` was a grid property.
fn apply_grid_property(style: &mut ComputedStyle, name: &str, raw: &str) -> bool {
    let g = &mut style.grid;
    let v = || Some(raw.trim().to_string());
    let pair = |s: &str| -> (Option<String>, Option<String>) {
        let mut it = s.splitn(2, '/').map(|p| p.trim().to_string());
        (it.next(), it.next())
    };
    match name {
        "grid-template-columns" => g.template_columns = v(),
        "grid-template-rows" => g.template_rows = v(),
        "grid-auto-columns" => g.auto_columns = v(),
        "grid-auto-rows" => g.auto_rows = v(),
        "grid-auto-flow" => g.auto_flow = v(),
        "grid-column-start" => g.column_start = v(),
        "grid-column-end" => g.column_end = v(),
        "grid-row-start" => g.row_start = v(),
        "grid-row-end" => g.row_end = v(),
        "grid-column" => (g.column_start, g.column_end) = pair(raw),
        "grid-row" => (g.row_start, g.row_end) = pair(raw),
        "grid-area" => {
            let parts: Vec<String> = raw.split('/').map(|p| p.trim().to_string()).collect();
            g.row_start = parts.first().cloned();
            g.column_start = parts.get(1).cloned();
            g.row_end = parts.get(2).cloned();
            g.column_end = parts.get(3).cloned();
        }
        "justify-items" => g.justify_items = v(),
        "justify-self" => g.justify_self = v(),
        "place-items" | "place-self" => {
            let mut it = raw.split_whitespace();
            let align = it.next().unwrap_or("").to_string();
            let justify = it
                .next()
                .map(str::to_string)
                .unwrap_or_else(|| align.clone());
            if name == "place-items" {
                apply_single(
                    style,
                    &CssProperty::AlignItems,
                    &CssValue::Keyword(align),
                    12.0,
                );
                style.grid.justify_items = Some(justify);
            } else {
                apply_single(
                    style,
                    &CssProperty::AlignSelf,
                    &CssValue::Keyword(align),
                    12.0,
                );
                style.grid.justify_self = Some(justify);
            }
        }
        // Legacy aliases of the gap properties.
        "grid-gap" => {
            let parts = parse_shorthand_lengths(raw);
            style.row_gap = parts.first().copied().unwrap_or(Length::Zero);
            style.column_gap = parts.get(1).copied().unwrap_or(style.row_gap);
        }
        "grid-row-gap" => style.row_gap = parse_length(raw),
        "grid-column-gap" => style.column_gap = parse_length(raw),
        _ => return false,
    }
    true
}

fn parse_shorthand_lengths(s: &str) -> Vec<Length> {
    split_top_level(s).into_iter().map(parse_length).collect()
}

/// Split on whitespace outside parentheses (`0 calc(100% - 10px)` → 2 parts).
fn split_top_level(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut start: Option<usize> = None;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
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

/// One radius: a non-negative length or percentage.
fn parse_radius_length(s: &str) -> Option<Length> {
    match parse_length(s) {
        Length::Auto | Length::None => None,
        Length::Px(v)
        | Length::Pt(v)
        | Length::Mm(v)
        | Length::Em(v)
        | Length::Rem(v)
        | Length::Percent(v)
            if v < 0.0 =>
        {
            None
        }
        l => Some(l),
    }
}

/// `border-radius`: one to four horizontal radii, optionally followed by
/// `/` and one to four vertical radii (CSS Backgrounds §5.1).
fn parse_border_radius(raw: &str) -> Option<BorderRadius> {
    // Split on a top-level `/` (one inside `calc()` is a division).
    let mut depth = 0;
    let slash = raw.char_indices().find_map(|(i, c)| {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            '/' if depth == 0 => return Some(i),
            _ => {}
        }
        None
    });
    let (h, v) = match slash {
        Some(i) => (&raw[..i], Some(&raw[i + 1..])),
        None => (raw, None),
    };
    let expand = |part: &str| -> Option<[Length; 4]> {
        let vals: Vec<Length> = crate::gradient::split_top_level(part, char::is_whitespace)
            .into_iter()
            .map(parse_radius_length)
            .collect::<Option<_>>()?;
        // Corner order: top-left, top-right, bottom-right, bottom-left.
        match vals.as_slice() {
            [a] => Some([*a; 4]),
            [a, b] => Some([*a, *b, *a, *b]),
            [a, b, c] => Some([*a, *b, *c, *b]),
            [a, b, c, d] => Some([*a, *b, *c, *d]),
            _ => None,
        }
    };
    let hs = expand(h)?;
    let vs = match v {
        Some(v) => expand(v)?,
        None => hs,
    };
    Some(BorderRadius {
        corners: [0, 1, 2, 3].map(|i| (hs[i], vs[i])),
    })
}

/// `none` or a list of `<name> <integer>?` pairs.
fn parse_counter_list(raw: &str, default: i32) -> Option<Vec<(String, i32)>> {
    let raw = raw.trim();
    if raw.eq_ignore_ascii_case("none") {
        return Some(Vec::new());
    }
    let mut out: Vec<(String, i32)> = Vec::new();
    for word in raw.split_whitespace() {
        match word.parse::<i32>() {
            Ok(v) => out.last_mut()?.1 = v,
            Err(_) => {
                let valid = word
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
                    && !word.starts_with(|c: char| c.is_ascii_digit());
                if !valid || matches!(word, "initial" | "inherit" | "unset") {
                    return None;
                }
                out.push((word.to_string(), default));
            }
        }
    }
    Some(out)
}

/// Parse a CSS length string directly to pt (points typographiques).
/// Used for properties stored as f32 (border widths, etc.)
pub(crate) fn parse_length_to_pt(s: &str) -> Option<f32> {
    let s = s.trim();
    if s == "0" || s == "0px" {
        return Some(0.0);
    }
    if s.len() > 5 && s[..5].eq_ignore_ascii_case("calc(") {
        // No font context here: em/rem relative to the 12pt default.
        let c = crate::calc::parse_calc(s)?;
        return (c.percent == 0.0).then(|| c.resolve_font_relative(12.0, 12.0).pt);
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

pub(crate) fn parse_color(s: &str) -> Option<Color> {
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

/// Parse CSS box-shadow value(s).
/// Supports: `<offset-x> <offset-y> [blur] [spread] [color] [inset]`
/// Multiple shadows separated by commas.
fn parse_box_shadows(raw: &str) -> Vec<BoxShadow> {
    raw.split(',')
        .filter_map(|shadow_str| parse_single_box_shadow(shadow_str.trim()))
        .collect()
}

fn parse_single_box_shadow(raw: &str) -> Option<BoxShadow> {
    if raw.is_empty() || raw == "none" {
        return None;
    }

    let mut lengths: Vec<f32> = Vec::new();
    let mut color = Color::new(0.0, 0.0, 0.0, 0.3); // default semi-transparent black
    let mut inset = false;

    // Handle rgba(...) or rgb(...) by collecting them first
    let mut s = raw.to_string();
    // Extract color functions (rgb/rgba/hsl) before splitting by whitespace
    if let Some(start) = s.find("rgb") {
        if let Some(end) = s[start..].find(')') {
            let color_str = &s[start..start + end + 1];
            if let Some(c) = parse_color(color_str) {
                color = c;
            }
            s = format!("{} {}", &s[..start], &s[start + end + 1..]);
        }
    }

    let parts: Vec<&str> = s.split_whitespace().collect();
    for part in &parts {
        if part.eq_ignore_ascii_case("inset") {
            inset = true;
        } else if let Some(pt) = parse_length_to_pt(part) {
            lengths.push(pt);
        } else if let Some(c) = parse_color(part) {
            color = c;
        }
    }

    if lengths.len() >= 2 {
        Some(BoxShadow {
            offset_x: lengths[0],
            offset_y: lengths[1],
            blur_radius: lengths.get(2).copied().unwrap_or(0.0),
            spread: lengths.get(3).copied().unwrap_or(0.0),
            color,
            inset,
        })
    } else {
        None
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

#[cfg(test)]
mod var_tests {
    use super::*;

    fn props(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn substitutes_simple_variable() {
        let p = props(&[("--primary", "red")]);
        assert_eq!(substitute_var("var(--primary)", &p), "red");
    }

    #[test]
    fn substitutes_inside_other_text() {
        let p = props(&[("--pad", "12pt")]);
        assert_eq!(
            substitute_var("0 var(--pad) 0 var(--pad)", &p),
            "0 12pt 0 12pt"
        );
    }

    #[test]
    fn missing_var_with_fallback_uses_fallback() {
        let p = props(&[]);
        assert_eq!(substitute_var("var(--missing, blue)", &p), "blue");
    }

    #[test]
    fn missing_var_without_fallback_resolves_to_empty() {
        let p = props(&[]);
        assert_eq!(substitute_var("var(--missing)", &p), "");
    }

    #[test]
    fn defined_var_takes_precedence_over_fallback() {
        let p = props(&[("--c", "green")]);
        assert_eq!(substitute_var("var(--c, red)", &p), "green");
    }

    #[test]
    fn nested_var_resolves_iteratively() {
        // var(--a) → "var(--b)" → "blue"
        let p = props(&[("--a", "var(--b)"), ("--b", "blue")]);
        assert_eq!(substitute_var("var(--a)", &p), "blue");
    }

    #[test]
    fn cycle_does_not_infinite_loop() {
        let p = props(&[("--x", "var(--y)"), ("--y", "var(--x)")]);
        // After MAX_ROUNDS rounds the result still contains a `var(`
        // call — the test only asserts we returned without hanging.
        let result = substitute_var("var(--x)", &p);
        assert!(
            result.contains("var(") || result.is_empty(),
            "got {:?}",
            result
        );
    }

    #[test]
    fn unbalanced_var_call_left_alone() {
        let p = props(&[("--a", "x")]);
        // No closing paren — substitution must bail and return the input.
        assert_eq!(substitute_var("var(--a", &p), "var(--a");
    }

    #[test]
    fn no_var_in_input_is_zero_cost() {
        let p = props(&[("--a", "x")]);
        let raw = "12pt solid red";
        let result = resolve_value(raw, &p);
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result.as_ref(), raw);
    }

    #[test]
    fn handles_nested_function_calls() {
        let p = props(&[("--r", "255"), ("--g", "128")]);
        assert_eq!(
            substitute_var("rgb(var(--r), var(--g), 0)", &p),
            "rgb(255, 128, 0)"
        );
    }
}

#[cfg(test)]
mod border_radius_tests {
    use super::*;

    #[test]
    fn shorthand_expansion_and_slash() {
        let r = parse_border_radius("10px 20%").unwrap();
        assert_eq!(r.corners[0], (Length::Px(10.0), Length::Px(10.0)));
        assert_eq!(r.corners[1], (Length::Percent(20.0), Length::Percent(20.0)));
        assert_eq!(r.corners[2].0, Length::Px(10.0));
        let r = parse_border_radius("50% / 10px 20px").unwrap();
        assert_eq!(r.corners[0], (Length::Percent(50.0), Length::Px(10.0)));
        assert_eq!(r.corners[1], (Length::Percent(50.0), Length::Px(20.0)));
        let r = parse_border_radius(".5rem").unwrap();
        assert_eq!(r.corners[3].0, Length::Rem(0.5));
        assert!(parse_border_radius("-3px").is_none());
        assert!(parse_border_radius("1px 2px 3px 4px 5px").is_none());
    }

    #[test]
    fn percent_resolves_against_box_and_clamps() {
        let r = parse_border_radius("50%").unwrap().resolve(200.0, 100.0);
        assert_eq!(r.0[0], (100.0, 50.0));
        // 100pt radii on a 60pt-tall box are scaled down to fit.
        let r = BorderRadius::uniform(Length::Pt(100.0)).resolve(200.0, 60.0);
        assert!((r.0[0].1 - 30.0).abs() < 1e-4 && (r.0[0].0 - 30.0).abs() < 1e-4);
    }
}
