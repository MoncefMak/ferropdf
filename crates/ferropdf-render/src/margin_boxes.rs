//! `@page` margin boxes — running headers and footers (CSS Paged Media 3 §5).
//!
//! Runs after pagination, when the total page count is known:
//!
//! 1. for each page, the `@page` rules that match it (`@page`, `:left` /
//!    `:right`, `:first`) are cascaded by specificity then source order;
//! 2. each margin box's `content` is resolved (strings, `counter(page)`,
//!    `counter(pages)`, with an optional counter style);
//! 3. the text is shaped and placed in the page margin, as an extra
//!    `LayoutBox` in the page's content-area coordinates (negative `y` for
//!    the top margin), so the normal painter draws it.
//!
//! `content: element(name)` places a running element (`position:
//! running(name)`, laid out separately at the content width) in the box.
//!
//! Supported boxes: `top-left`, `top-center`, `top-right`, `bottom-left`,
//! `bottom-center`, `bottom-right`. Each spans the content width, aligned
//! left / centre / right (overridable with `text-align`), and is vertically
//! centred in its margin.

use cosmic_text::FontSystem;
use ferropdf_core::layout::Page;
use ferropdf_core::{ComputedStyle, LayoutBox, PageConfig, Rect, TextAlign};
use ferropdf_parse::{CssProperty, Declaration, PageRule, PageSelector};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CounterStyle {
    Decimal,
    DecimalLeadingZero,
    LowerRoman,
    UpperRoman,
    LowerAlpha,
    UpperAlpha,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Part {
    Text(String),
    Page(CounterStyle),
    Pages(CounterStyle),
    /// `element(name)`: a running element (`position: running(name)`).
    Element(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Top,
    Bottom,
}

const SLOTS: [(&str, Side, TextAlign); 6] = [
    ("top-left", Side::Top, TextAlign::Left),
    ("top-center", Side::Top, TextAlign::Center),
    ("top-right", Side::Top, TextAlign::Right),
    ("bottom-left", Side::Bottom, TextAlign::Left),
    ("bottom-center", Side::Bottom, TextAlign::Center),
    ("bottom-right", Side::Bottom, TextAlign::Right),
];

/// Add the margin boxes of every page. `rules` are all `@page` rules in
/// source order; `root` is the root element's computed style (margin boxes
/// inherit from the page context, which inherits from the root element).
pub(crate) fn add_margin_boxes(
    pages: &mut [Page],
    rules: &[&PageRule],
    config: &PageConfig,
    root: &ComputedStyle,
    running: &HashMap<String, LayoutBox>,
    fs: &mut FontSystem,
) {
    if rules.iter().all(|r| r.margin_boxes.is_empty()) {
        return;
    }
    let total = pages.len() as u32;

    for page in pages.iter_mut() {
        let n = page.page_number;
        // Margin areas follow this page's margins (`@page :first` …).
        let margins = config.margins_for(n);
        let content_w = (config.page_width_pt() - margins.left - margins.right).max(0.0);
        let content_h = config.content_height_for(n);
        // Matching rules, lowest priority first (stable sort keeps source order).
        let mut matching: Vec<&PageRule> = rules
            .iter()
            .copied()
            .filter(|r| selector_matches(r.selector, n))
            .collect();
        matching.sort_by_key(|r| r.selector.specificity());

        for (name, side, default_align) in SLOTS {
            let decls: Vec<Declaration> = matching
                .iter()
                .flat_map(|r| r.margin_boxes.iter().filter(|m| m.name == name))
                .flat_map(|m| m.declarations.iter().cloned())
                .collect();
            let Some(raw_content) = decls
                .iter()
                .rev()
                .find(|d| is_property(d, "content"))
                .map(|d| d.value.to_cow().into_owned())
            else {
                continue;
            };
            let Some(parts) = parse_content(&raw_content) else {
                continue; // `content: none` / `normal`
            };
            let (area_y, area_h) = match side {
                Side::Top => (-margins.top, margins.top),
                Side::Bottom => (content_h, margins.bottom),
            };

            // `element(name)`: place the pre-laid-out running element.
            if let Some(element) = parts.iter().find_map(|p| match p {
                Part::Element(name) => running.get(name),
                _ => None,
            }) {
                let mut placed = element.clone();
                let w = placed.rect.width.min(content_w);
                let dx = match default_align {
                    TextAlign::Center => (content_w - w) / 2.0,
                    TextAlign::Right => content_w - w,
                    _ => 0.0,
                } - placed.rect.x;
                let dy = area_y + ((area_h - placed.rect.height) / 2.0).max(0.0) - placed.rect.y;
                translate(&mut placed, dx, dy);
                page.content.push(placed);
                continue;
            }

            let text = render_parts(&parts, n, total);
            if text.trim().is_empty() {
                continue;
            }

            let mut style = ferropdf_style::style_from_declarations(root, &decls);
            if !decls
                .iter()
                .any(|d| matches!(d.property, CssProperty::TextAlign))
            {
                style.text_align = default_align;
            }
            let family = style.font_family.first().map(|s| s.as_str()).unwrap_or("");
            let lines = ferropdf_layout::text::shape_text_lines(
                &text,
                style.font_size,
                style.line_height,
                family,
                style.font_weight.is_bold(),
                style.font_style == ferropdf_core::FontStyle::Italic,
                content_w,
                ferropdf_layout::text::css_align(style.text_align.physical(style.direction)),
                true,
                fs,
            );
            if lines.is_empty() {
                continue;
            }
            let text_h = lines.len() as f32 * style.line_height;
            let content_y = area_y + ((area_h - text_h) / 2.0).max(0.0);

            page.content.push(LayoutBox {
                rect: Rect::new(0.0, area_y, content_w, area_h),
                content: Rect::new(0.0, content_y, content_w, text_h),
                shaped_lines: lines,
                text_content: Some(text),
                style,
                ..Default::default()
            });
        }
    }
}

fn translate(b: &mut LayoutBox, dx: f32, dy: f32) {
    b.rect.x += dx;
    b.rect.y += dy;
    b.content.x += dx;
    b.content.y += dy;
    for c in &mut b.children {
        translate(c, dx, dy);
    }
}

fn selector_matches(selector: PageSelector, page_number: u32) -> bool {
    match selector {
        PageSelector::All => true,
        PageSelector::First => page_number == 1,
        // Left-to-right documents start on a right-hand (recto) page.
        PageSelector::Right => page_number % 2 == 1,
        PageSelector::Left => page_number % 2 == 0,
    }
}

fn is_property(d: &Declaration, name: &str) -> bool {
    matches!(&d.property, CssProperty::Unknown(n) if n.eq_ignore_ascii_case(name))
}

fn render_parts(parts: &[Part], page: u32, pages: u32) -> String {
    parts
        .iter()
        .map(|p| match p {
            Part::Text(t) => t.clone(),
            Part::Page(style) => format_counter(page, *style),
            Part::Pages(style) => format_counter(pages, *style),
            Part::Element(_) => String::new(),
        })
        .collect()
}

/// Parse a `content` value. Returns `None` for `none` / `normal`.
/// Unknown functions (`string()`, `element()`, `attr()`) are skipped.
pub(crate) fn parse_content(raw: &str) -> Option<Vec<Part>> {
    let raw = raw.trim();
    if raw.eq_ignore_ascii_case("none") || raw.eq_ignore_ascii_case("normal") {
        return None;
    }
    let chars: Vec<char> = raw.chars().collect();
    let mut parts = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '"' || c == '\'' {
            let quote = c;
            let mut s = String::new();
            i += 1;
            while i < chars.len() && chars[i] != quote {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    i += 1;
                    // `\A` is a line break in CSS strings; other escapes are literal.
                    if chars[i] == 'A' || chars[i] == 'a' {
                        s.push('\n');
                        i += 1;
                        while i < chars.len() && chars[i] == ' ' {
                            i += 1;
                        }
                        continue;
                    }
                }
                s.push(chars[i]);
                i += 1;
            }
            i += 1; // closing quote
            parts.push(Part::Text(s));
        } else {
            // Identifier, possibly a function call.
            let start = i;
            while i < chars.len()
                && (chars[i].is_alphanumeric() || chars[i] == '-' || chars[i] == '_')
            {
                i += 1;
            }
            let ident: String = chars[start..i]
                .iter()
                .collect::<String>()
                .to_ascii_lowercase();
            if i < chars.len() && chars[i] == '(' {
                let args_start = i + 1;
                let mut depth = 0;
                while i < chars.len() {
                    match chars[i] {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
                let args: String = chars[args_start..i.min(chars.len())].iter().collect();
                i += 1;
                if ident == "element" {
                    let name = args.split(',').next().unwrap_or("").trim();
                    if !name.is_empty() {
                        parts.push(Part::Element(name.to_string()));
                    }
                } else if ident == "counter" {
                    let mut it = args.split(',').map(|a| a.trim().to_ascii_lowercase());
                    let name = it.next().unwrap_or_default();
                    let style = counter_style(it.next().as_deref().unwrap_or("decimal"));
                    match name.as_str() {
                        "page" => parts.push(Part::Page(style)),
                        "pages" => parts.push(Part::Pages(style)),
                        _ => {}
                    }
                }
            } else if i == start {
                i += 1; // stray character
            }
        }
    }
    Some(parts)
}

fn counter_style(name: &str) -> CounterStyle {
    match name {
        "decimal-leading-zero" => CounterStyle::DecimalLeadingZero,
        "lower-roman" => CounterStyle::LowerRoman,
        "upper-roman" => CounterStyle::UpperRoman,
        "lower-alpha" | "lower-latin" => CounterStyle::LowerAlpha,
        "upper-alpha" | "upper-latin" => CounterStyle::UpperAlpha,
        _ => CounterStyle::Decimal,
    }
}

pub(crate) fn format_counter(n: u32, style: CounterStyle) -> String {
    match style {
        CounterStyle::Decimal => n.to_string(),
        CounterStyle::DecimalLeadingZero => format!("{n:02}"),
        CounterStyle::LowerRoman => roman(n).to_lowercase(),
        CounterStyle::UpperRoman => roman(n),
        CounterStyle::LowerAlpha => alpha(n).to_lowercase(),
        CounterStyle::UpperAlpha => alpha(n),
    }
}

fn roman(mut n: u32) -> String {
    if n == 0 || n >= 4000 {
        return n.to_string();
    }
    const TABLE: [(u32, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut out = String::new();
    for (value, sym) in TABLE {
        while n >= value {
            out.push_str(sym);
            n -= value;
        }
    }
    out
}

fn alpha(mut n: u32) -> String {
    if n == 0 {
        return "0".into();
    }
    let mut out = Vec::new();
    while n > 0 {
        n -= 1;
        out.push((b'A' + (n % 26) as u8) as char);
        n /= 26;
    }
    out.iter().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_strings_and_counters() {
        let parts =
            parse_content(r#""Page " counter(page) " of " counter(pages, upper-roman)"#).unwrap();
        assert_eq!(
            parts,
            vec![
                Part::Text("Page ".into()),
                Part::Page(CounterStyle::Decimal),
                Part::Text(" of ".into()),
                Part::Pages(CounterStyle::UpperRoman),
            ]
        );
        assert_eq!(render_parts(&parts, 3, 12), "Page 3 of XII");
    }

    #[test]
    fn none_and_unknown_functions() {
        assert_eq!(parse_content("none"), None);
        assert_eq!(
            parse_content(r#"string(title) "x" element(h)"#).unwrap(),
            vec![Part::Text("x".into()), Part::Element("h".into())]
        );
        assert_eq!(
            parse_content(r#"'it\'s'"#).unwrap(),
            vec![Part::Text("it's".into())]
        );
    }

    #[test]
    fn counter_styles() {
        assert_eq!(format_counter(4, CounterStyle::LowerRoman), "iv");
        assert_eq!(format_counter(1994, CounterStyle::UpperRoman), "MCMXCIV");
        assert_eq!(format_counter(28, CounterStyle::UpperAlpha), "AB");
        assert_eq!(format_counter(7, CounterStyle::DecimalLeadingZero), "07");
    }

    #[test]
    fn selectors() {
        assert!(selector_matches(PageSelector::First, 1));
        assert!(!selector_matches(PageSelector::First, 2));
        assert!(selector_matches(PageSelector::Right, 1));
        assert!(selector_matches(PageSelector::Left, 2));
    }
}
