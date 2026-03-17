//! Style resolver — applies the CSS cascade to produce a `ComputedStyle`
//! for every element in the DOM.

use std::collections::HashMap;

use ferropdf_core::Color;

use crate::html::dom::{Document, NodeId, NodeKind};
use super::{
    cascade::{CascadeEntry, CascadeOrigin},
    compute::{parse_color, parse_length_str, resolve_font_size, ComputeCtx},
    matching::matches_selector,
    properties::{
        AlignItems, BorderSide, BorderStyleKind, Clear, ComputedEdge, ComputedStyle,
        Display, FlexDirection, FlexWrap, Float, FontStyle, GridTrack,
        JustifyContent, ListStyleType, Overflow, PageBreak, PageBreakInside,
        Position, TextAlign, TextDecoration,
    },
    specificity::selector_specificity,
    values::{CssValue, Declaration, Stylesheet},
};
use super::parser::parse_stylesheet;

// ─── UA stylesheet (built-in) ─────────────────────────────────────────────────

static UA_CSS: &str = include_str!("ua.css");

fn ua_stylesheet() -> Stylesheet {
    parse_stylesheet(UA_CSS).unwrap_or_default()
}

// ─── StyleResolver ────────────────────────────────────────────────────────────

pub struct StyleResolver {
    ua:     Stylesheet,
    author: Vec<Stylesheet>,
}

impl StyleResolver {
    /// Create a new resolver with the given author stylesheets.
    pub fn new(author_sheets: Vec<Stylesheet>) -> Self {
        Self {
            ua:     ua_stylesheet(),
            author: author_sheets,
        }
    }

    /// Compute the `ComputedStyle` for a single element in the tree.
    ///
    /// - `node_id`:     the element to style
    /// - `parent`:      the already-computed parent style (for inheritance)
    /// - `inline_css`:  the value of the element's `style="..."` attribute
    pub fn compute(&self, node_id: NodeId, doc: &Document, parent: &ComputedStyle, inline_css: &str) -> ComputedStyle {
        // Collect all matching declarations (UA + Author + Inline), ordered by cascade.
        let mut entries: Vec<CascadeEntry<'_>> = Vec::new();

        // 1. UA stylesheet
        let mut order = 0u32;
        for rule in &self.ua.rules {
            for selector in &rule.selectors {
                if matches_selector(selector, node_id, doc) {
                    let spec = selector_specificity(selector);
                    for decl in &rule.declarations {
                        entries.push(CascadeEntry {
                            decl,
                            origin:      CascadeOrigin::UserAgent,
                            specificity: spec,
                            order,
                        });
                        order += 1;
                    }
                }
            }
        }

        // 2. Author stylesheets
        for sheet in &self.author {
            for rule in &sheet.rules {
                for selector in &rule.selectors {
                    if matches_selector(selector, node_id, doc) {
                        let spec = selector_specificity(selector);
                        for decl in &rule.declarations {
                            entries.push(CascadeEntry {
                                decl,
                                origin:      CascadeOrigin::Author,
                                specificity: spec,
                                order,
                            });
                            order += 1;
                        }
                    }
                }
            }
        }

        // 3. Inline styles (highest priority)
        let inline_decls: Vec<Declaration>;
        if !inline_css.is_empty() {
            inline_decls = super::parser::parse_inline_style(inline_css);
            for decl in &inline_decls {
                entries.push(CascadeEntry {
                    decl,
                    origin:      CascadeOrigin::Inline,
                    specificity: super::specificity::Specificity::inline(),
                    order,
                });
                order += 1;
            }
        }

        // Resolve "winning" value for each property
        let mut winning: HashMap<String, &Declaration> = HashMap::new();
        // We iterate in source order; the last entry that wins according to
        // cascade rules ends up in the map.
        let mut best_entries: HashMap<String, CascadeEntry<'_>> = HashMap::new();
        for entry in entries {
            let prop = &entry.decl.property;
            match best_entries.get(prop) {
                None => { best_entries.insert(prop.clone(), entry); }
                Some(existing) => {
                    if entry.wins_over(existing) {
                        best_entries.insert(prop.clone(), entry);
                    }
                }
            }
        }
        for (prop, entry) in &best_entries {
            winning.insert(prop.clone(), entry.decl);
        }

        // Build the computed style by applying winning declarations on top of
        // the inherited parent values.
        self.apply_declarations(&winning, parent, node_id, doc)
    }

    /// Apply a winning declaration map on top of inherited parent style.
    fn apply_declarations(
        &self,
        decls:  &HashMap<String, &Declaration>,
        parent: &ComputedStyle,
        node_id: NodeId,
        doc:    &Document,
    ) -> ComputedStyle {
        let tag_name = doc.get(node_id).tag_name().unwrap_or("").to_string();

        // Build a resolution context for length values.
        let ctx = ComputeCtx {
            font_size_px:    parent.font_size,
            root_font_px:    16.0, // TODO: thread actual root font size
            parent_font_px:  parent.font_size,
            parent_width_px: 0.0,  // will be filled in during layout
            current_color:   parent.color,
            ..Default::default()
        };

        // Start with a default style.
        let mut s = ComputedStyle {
            // Inherit from parent
            color:           parent.color,
            font_family:     parent.font_family.clone(),
            font_size:       parent.font_size,
            font_weight:     parent.font_weight,
            font_style:      parent.font_style,
            line_height:     parent.line_height,
            letter_spacing:  parent.letter_spacing,
            text_align:      parent.text_align,
            text_decoration: parent.text_decoration,
            text_indent:     parent.text_indent,
            white_space_pre: parent.white_space_pre,
            direction_rtl:   parent.direction_rtl,
            list_style_type: parent.list_style_type,
            ..ComputedStyle::default()
        };

        // Helper macro to get a declaration value
        let get = |prop: &str| -> Option<&CssValue> {
            decls.get(prop).map(|d| &d.value)
        };

        // ── Display ──────────────────────────────────────────────────────────
        if let Some(v) = get("display") {
            if let Some(d) = kw(v) { s.display = parse_display(d); }
        } else {
            s.display = default_display_for_tag(&tag_name);
        }

        // ── Position ─────────────────────────────────────────────────────────
        if let Some(v) = get("position") { s.position = parse_position(kw(v).unwrap_or("")); }

        // ── Float / Clear ─────────────────────────────────────────────────────
        if let Some(v) = get("float") { s.float = parse_float(kw(v).unwrap_or("")); }
        if let Some(v) = get("clear") { s.clear = parse_clear(kw(v).unwrap_or("")); }

        // ── Overflow ──────────────────────────────────────────────────────────
        if let Some(v) = get("overflow") { s.overflow = parse_overflow(kw(v).unwrap_or("")); }

        // ── Box sizing ────────────────────────────────────────────────────────
        if let Some(v) = get("box-sizing") { s.box_sizing_border_box = kw(v) == Some("border-box"); }

        // ── Dimensions ───────────────────────────────────────────────────────
        s.width     = resolve_opt_len(get("width"),      &ctx);
        s.height    = resolve_opt_len(get("height"),     &ctx);
        s.min_width  = resolve_len_or(get("min-width"),  &ctx, 0.0);
        s.min_height = resolve_len_or(get("min-height"), &ctx, 0.0);
        s.max_width  = resolve_opt_len(get("max-width"),  &ctx);
        s.max_height = resolve_opt_len(get("max-height"), &ctx);

        // ── Offsets ───────────────────────────────────────────────────────────
        s.top    = resolve_opt_len(get("top"),    &ctx);
        s.right  = resolve_opt_len(get("right"),  &ctx);
        s.bottom = resolve_opt_len(get("bottom"), &ctx);
        s.left   = resolve_opt_len(get("left"),   &ctx);

        if let Some(v) = get("z-index") {
            if let Some(n) = v.as_number() { s.z_index = n as i32; }
        }

        // ── Margin ────────────────────────────────────────────────────────────
        s.margin.top    = resolve_len_or(get("margin-top"),    &ctx, 0.0);
        s.margin.right  = resolve_len_or(get("margin-right"),  &ctx, 0.0);
        s.margin.bottom = resolve_len_or(get("margin-bottom"), &ctx, 0.0);
        s.margin.left   = resolve_len_or(get("margin-left"),   &ctx, 0.0);

        // ── Padding ───────────────────────────────────────────────────────────
        s.padding.top    = resolve_len_or(get("padding-top"),    &ctx, 0.0);
        s.padding.right  = resolve_len_or(get("padding-right"),  &ctx, 0.0);
        s.padding.bottom = resolve_len_or(get("padding-bottom"), &ctx, 0.0);
        s.padding.left   = resolve_len_or(get("padding-left"),   &ctx, 0.0);

        // ── Borders ───────────────────────────────────────────────────────────
        s.border_top    = resolve_border_side("top",    &decls, &ctx);
        s.border_right  = resolve_border_side("right",  &decls, &ctx);
        s.border_bottom = resolve_border_side("bottom", &decls, &ctx);
        s.border_left   = resolve_border_side("left",   &decls, &ctx);

        // Border radius
        let radii = ["top-left", "top-right", "bottom-right", "bottom-left"];
        for (i, corner) in radii.iter().enumerate() {
            if let Some(v) = get(&format!("border-{}-radius", corner)) {
                s.border_radius[i] = resolve_len_or(Some(v), &ctx, 0.0);
            }
        }

        // ── Background ────────────────────────────────────────────────────────
        if let Some(v) = get("background-color") {
            if let Some(c) = parse_css_color_val(v, parent.color) { s.background_color = c; }
        }
        if let Some(v) = get("background").or_else(|| get("background-image")) {
            if let CssValue::Url(u) = v { s.background_image = Some(u.clone()); }
        }
        if let Some(v) = get("opacity") {
            if let Some(n) = v.as_number() { s.opacity = n.clamp(0.0, 1.0); }
        }

        // ── Typography ────────────────────────────────────────────────────────
        if let Some(v) = get("color") {
            if let Some(c) = parse_css_color_val(v, parent.color) { s.color = c; }
        }

        if let Some(v) = get("font-family") {
            s.font_family = match v {
                CssValue::Keyword(s) => clean_font_family(s),
                _ => v.to_string(),
            };
        }

        if let Some(v) = get("font-size") {
            s.font_size = resolve_font_size_val(v, parent.font_size);
        }
        s.line_height = s.font_size * 1.2; // reset default before line-height prop

        if let Some(v) = get("font-weight") {
            s.font_weight = parse_font_weight(v, parent.font_weight);
        }
        if let Some(v) = get("font-style") {
            s.font_style = parse_font_style_val(v);
        }
        if let Some(v) = get("line-height") {
            s.line_height = resolve_line_height(v, s.font_size, &ctx);
        }
        if let Some(v) = get("letter-spacing") {
            s.letter_spacing = resolve_len_or(Some(v), &ctx, 0.0);
        }
        if let Some(v) = get("text-align") {
            s.text_align = parse_text_align(kw(v).unwrap_or(""));
        }
        if let Some(v) = get("text-decoration") {
            s.text_decoration = parse_text_decoration(kw(v).unwrap_or(""));
        }
        if let Some(v) = get("text-indent") {
            s.text_indent = resolve_len_or(Some(v), &ctx, 0.0);
        }
        if let Some(v) = get("white-space") {
            s.white_space_pre = matches!(kw(v), Some("pre" | "pre-wrap" | "pre-line"));
        }
        if let Some(v) = get("direction") {
            s.direction_rtl = kw(v) == Some("rtl");
        }

        // ── Flex ──────────────────────────────────────────────────────────────
        if let Some(v) = get("flex-direction")  { s.flex_direction  = parse_flex_direction(kw(v).unwrap_or("")); }
        if let Some(v) = get("flex-wrap")        { s.flex_wrap        = parse_flex_wrap(kw(v).unwrap_or("")); }
        if let Some(v) = get("justify-content")  { s.justify_content  = parse_justify_content(kw(v).unwrap_or("")); }
        if let Some(v) = get("align-items")      { s.align_items      = parse_align_items(kw(v).unwrap_or("")); }
        if let Some(v) = get("align-self")       { s.align_self       = parse_align_items(kw(v).unwrap_or("")); }
        if let Some(v) = get("flex-grow")   { if let Some(n) = v.as_number() { s.flex_grow   = n; } }
        if let Some(v) = get("flex-shrink") { if let Some(n) = v.as_number() { s.flex_shrink = n; } }
        if let Some(v) = get("flex-basis")  { s.flex_basis = resolve_opt_len(Some(v), &ctx); }
        {
            let rg = resolve_len_or(get("row-gap").or_else(|| get("gap")),    &ctx, 0.0);
            let cg = resolve_len_or(get("column-gap").or_else(|| get("gap")), &ctx, 0.0);
            s.gap = (rg, cg);
        }

        // ── Grid ──────────────────────────────────────────────────────────────
        if let Some(v) = get("grid-template-columns") {
            s.grid_template_columns = parse_grid_template(v);
        }
        if let Some(v) = get("grid-template-rows") {
            s.grid_template_rows = parse_grid_template(v);
        }

        // ── List ──────────────────────────────────────────────────────────────
        if let Some(v) = get("list-style-type") { s.list_style_type = parse_list_style_type(kw(v).unwrap_or("")); }
        if let Some(v) = get("list-style") {
            // Shorthand — just check for known keywords
            if let Some(t) = kw(v) { s.list_style_type = parse_list_style_type(t); }
        }

        // ── Page breaks ───────────────────────────────────────────────────────
        if let Some(v) = get("page-break-before").or_else(|| get("break-before")) {
            s.page_break_before = parse_page_break(kw(v).unwrap_or(""));
        }
        if let Some(v) = get("page-break-after").or_else(|| get("break-after")) {
            s.page_break_after = parse_page_break(kw(v).unwrap_or(""));
        }
        if let Some(v) = get("page-break-inside").or_else(|| get("break-inside")) {
            s.page_break_inside = match kw(v) {
                Some("avoid") => PageBreakInside::Avoid, _ => PageBreakInside::Auto,
            };
        }

        // ── Visibility ────────────────────────────────────────────────────────
        if let Some(v) = get("visibility") { s.visibility_hidden = kw(v) == Some("hidden"); }

        s
    }
}

// ─── Parser helpers ───────────────────────────────────────────────────────────

fn kw(v: &CssValue) -> Option<&str> { v.keyword() }

fn resolve_opt_len(v: Option<&CssValue>, ctx: &ComputeCtx) -> Option<f32> {
    let v = v?;
    match v {
        CssValue::None => None,
        CssValue::Keyword(s) if s == "auto" || s == "none" => None,
        CssValue::Length(l) => {
            use super::compute::resolve_length;
            use super::values::CssLength;
            // Don't resolve percentage lengths when parent width is unknown —
            // the layout engine will use avail_w instead.
            if matches!(l, CssLength::Percent(_)) && ctx.parent_width_px <= 0.0 {
                return None;
            }
            let px = resolve_length(*l, ctx);
            Some(px)
        }
        CssValue::Percentage(p) => {
            // Only resolve if we have a meaningful parent width.
            if ctx.parent_width_px > 0.0 {
                Some(p / 100.0 * ctx.parent_width_px)
            } else {
                None
            }
        }
        CssValue::Number(n) => Some(*n),
        _ => None,
    }
}

fn resolve_len_or(v: Option<&CssValue>, ctx: &ComputeCtx, default: f32) -> f32 {
    resolve_opt_len(v, ctx).unwrap_or(default)
}

fn resolve_line_height(v: &CssValue, font_size: f32, ctx: &ComputeCtx) -> f32 {
    match v {
        CssValue::Keyword(s) if s == "normal" => font_size * 1.2,
        CssValue::Number(n) => font_size * n,
        CssValue::Percentage(p) => font_size * p / 100.0,
        CssValue::Length(l) => {
            use super::compute::resolve_length;
            resolve_length(*l, ctx)
        }
        _ => font_size * 1.2,
    }
}

fn resolve_font_size_val(v: &CssValue, parent_font: f32) -> f32 {
    match v {
        CssValue::Length(l) => {
            let ctx = ComputeCtx {
                font_size_px:   parent_font,
                parent_font_px: parent_font,
                root_font_px:   16.0,
                parent_width_px: 0.0,
                ..Default::default()
            };
            super::compute::resolve_length(*l, &ctx)
        }
        CssValue::Percentage(p) => parent_font * p / 100.0,
        CssValue::Keyword(s)    => resolve_font_size(&s, parent_font, 16.0),
        CssValue::Number(n)     => *n,
        _ => parent_font,
    }
}

fn parse_css_color_val(v: &CssValue, current: Color) -> Option<Color> {
    match v {
        CssValue::Color(c)   => Some(*c),
        CssValue::Keyword(s) => {
            if s == "currentcolor" || s == "currentColor" { return Some(current); }
            parse_color(s)
        }
        _ => None,
    }
}

fn resolve_border_side(
    side: &str,
    decls: &HashMap<String, &Declaration>,
    ctx:   &ComputeCtx,
) -> BorderSide {
    let width_key = format!("border-{side}-width");
    let style_key = format!("border-{side}-style");
    let color_key = format!("border-{side}-color");

    let width = decls.get(width_key.as_str())
        .and_then(|d| resolve_opt_len(Some(&d.value), ctx))
        .unwrap_or(0.0);

    let bstyle = decls.get(style_key.as_str())
        .and_then(|d| d.value.keyword())
        .map(parse_border_style)
        .unwrap_or(BorderStyleKind::None);

    let color = decls.get(color_key.as_str())
        .and_then(|d| parse_css_color_val(&d.value, Color::BLACK))
        .unwrap_or(Color::BLACK);

    BorderSide { width, style: bstyle, color }
}

fn clean_font_family(s: &str) -> String {
    s.split(',').next().unwrap_or(s)
        .trim().trim_matches('"').trim_matches('\'').to_string()
}

// ─── Parsing helpers ──────────────────────────────────────────────────────────

fn parse_display(s: &str) -> Display {
    match s {
        "block"              => Display::Block,
        "inline"             => Display::Inline,
        "inline-block"       => Display::InlineBlock,
        "flex"               => Display::Flex,
        "inline-flex"        => Display::Flex,
        "grid"               => Display::Grid,
        "inline-grid"        => Display::Grid,
        "table"              => Display::Table,
        "table-row"          => Display::TableRow,
        "table-cell"         => Display::TableCell,
        "table-header-group" => Display::TableHeaderGroup,
        "table-row-group"    => Display::TableRowGroup,
        "table-footer-group" => Display::TableFooterGroup,
        "list-item"          => Display::ListItem,
        "none"               => Display::None,
        _                    => Display::Block,
    }
}

fn default_display_for_tag(tag: &str) -> Display {
    match tag {
        "div" | "p" | "section" | "article" | "aside" | "header" | "footer"
        | "main" | "nav" | "figure" | "blockquote" | "pre" | "ul" | "ol"
        | "dl" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "hr"
        | "body" | "html" | "summary" | "details" | "address" => Display::Block,

        "li"      => Display::ListItem,
        "table"   => Display::Table,
        "tr"      => Display::TableRow,
        "td" | "th" => Display::TableCell,
        "thead"   => Display::TableHeaderGroup,
        "tbody"   => Display::TableRowGroup,
        "tfoot"   => Display::TableFooterGroup,

        "span" | "a" | "strong" | "b" | "em" | "i" | "u" | "s"
        | "code" | "mark" | "small" | "big" | "sub" | "sup"
        | "label" | "abbr" | "time" | "img" | "br" => Display::Inline,

        _ => Display::Inline,  // safe default for unknown tags
    }
}

fn parse_position(s: &str) -> Position {
    match s {
        "absolute" => Position::Absolute,
        "fixed"    => Position::Fixed,
        "relative" => Position::Relative,
        "sticky"   => Position::Sticky,
        _          => Position::Static,
    }
}

fn parse_float(s: &str) -> Float {
    match s { "left" => Float::Left, "right" => Float::Right, _ => Float::None }
}

fn parse_clear(s: &str) -> Clear {
    match s {
        "left"  => Clear::Left,
        "right" => Clear::Right,
        "both"  => Clear::Both,
        _       => Clear::None,
    }
}

fn parse_overflow(s: &str) -> Overflow {
    match s {
        "hidden" => Overflow::Hidden,
        "scroll" => Overflow::Scroll,
        "auto"   => Overflow::Auto,
        _        => Overflow::Visible,
    }
}

fn parse_font_weight(v: &CssValue, parent: u32) -> u32 {
    match v {
        CssValue::Keyword(s) => match s.as_str() {
            "bold"    => 700,
            "bolder"  => (parent + 300).min(900),
            "lighter" => parent.saturating_sub(300).max(100),
            "normal"  => 400,
            _         => s.parse().unwrap_or(parent),
        },
        CssValue::Integer(n) => (*n as u32).clamp(100, 900),
        CssValue::Number(n)  => (*n as u32).clamp(100, 900),
        _ => parent,
    }
}

fn parse_font_style_val(v: &CssValue) -> FontStyle {
    match kw(v) {
        Some("italic")  => FontStyle::Italic,
        Some("oblique") => FontStyle::Oblique,
        _               => FontStyle::Normal,
    }
}

fn parse_text_align(s: &str) -> TextAlign {
    match s {
        "right"   => TextAlign::Right,
        "center"  => TextAlign::Center,
        "justify" => TextAlign::Justify,
        _         => TextAlign::Left,
    }
}

fn parse_text_decoration(s: &str) -> TextDecoration {
    match s {
        "underline"    => TextDecoration::Underline,
        "overline"     => TextDecoration::Overline,
        "line-through" => TextDecoration::LineThrough,
        _              => TextDecoration::None,
    }
}

fn parse_border_style(s: &str) -> BorderStyleKind {
    match s {
        "solid"  => BorderStyleKind::Solid,
        "dashed" => BorderStyleKind::Dashed,
        "dotted" => BorderStyleKind::Dotted,
        "double" => BorderStyleKind::Double,
        "groove" => BorderStyleKind::Groove,
        "ridge"  => BorderStyleKind::Ridge,
        "inset"  => BorderStyleKind::Inset,
        "outset" => BorderStyleKind::Outset,
        _        => BorderStyleKind::None,
    }
}

fn parse_flex_direction(s: &str) -> FlexDirection {
    match s {
        "row-reverse"    => FlexDirection::RowReverse,
        "column"         => FlexDirection::Column,
        "column-reverse" => FlexDirection::ColumnReverse,
        _                => FlexDirection::Row,
    }
}

fn parse_flex_wrap(s: &str) -> FlexWrap {
    match s {
        "wrap"         => FlexWrap::Wrap,
        "wrap-reverse" => FlexWrap::WrapReverse,
        _              => FlexWrap::NoWrap,
    }
}

fn parse_justify_content(s: &str) -> JustifyContent {
    match s {
        "flex-end" | "end" => JustifyContent::FlexEnd,
        "center"           => JustifyContent::Center,
        "space-between"    => JustifyContent::SpaceBetween,
        "space-around"     => JustifyContent::SpaceAround,
        "space-evenly"     => JustifyContent::SpaceEvenly,
        _                  => JustifyContent::FlexStart,
    }
}

fn parse_align_items(s: &str) -> AlignItems {
    match s {
        "flex-start" | "start" => AlignItems::FlexStart,
        "flex-end"   | "end"   => AlignItems::FlexEnd,
        "center"               => AlignItems::Center,
        "baseline"             => AlignItems::Baseline,
        _                      => AlignItems::Stretch,
    }
}

fn parse_list_style_type(s: &str) -> ListStyleType {
    match s {
        "disc"         => ListStyleType::Disc,
        "circle"       => ListStyleType::Circle,
        "square"       => ListStyleType::Square,
        "decimal"      => ListStyleType::Decimal,
        "lower-alpha"  => ListStyleType::LowerAlpha,
        "upper-alpha"  => ListStyleType::UpperAlpha,
        "lower-roman"  => ListStyleType::LowerRoman,
        "upper-roman"  => ListStyleType::UpperRoman,
        _              => ListStyleType::None,
    }
}

fn parse_page_break(s: &str) -> PageBreak {
    match s {
        "always" | "page" => PageBreak::Always,
        "avoid"            => PageBreak::Avoid,
        _                  => PageBreak::Auto,
    }
}

fn parse_grid_template(v: &CssValue) -> Vec<GridTrack> {
    let s = v.to_string();
    s.split_whitespace()
        .map(|tok| {
            if tok == "auto"          { GridTrack::Auto }
            else if tok.ends_with("fr") {
                let f: f32 = tok[..tok.len()-2].parse().unwrap_or(1.0);
                GridTrack::Fr(f)
            }
            else if tok.ends_with('%') {
                let p: f32 = tok[..tok.len()-1].parse().unwrap_or(0.0);
                GridTrack::Percent(p)
            }
            else if tok.ends_with("px") {
                let p: f32 = tok[..tok.len()-2].parse().unwrap_or(0.0);
                GridTrack::Px(p)
            }
            else { GridTrack::Auto }
        })
        .collect()
}
