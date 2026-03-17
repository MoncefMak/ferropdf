//! CSS stylesheet parser.
//!
//! Parses CSS text into a `Stylesheet` (rules + declarations).

use std::collections::HashMap;

use ferropdf_core::FerroError;

use super::values::{
    AttrOp, Combinator, CssLength, CssValue, Declaration, FontFaceRule,
    PageRule, Selector, SelectorComponent, StyleRule, Stylesheet,
};

/// Parse CSS text into a `Stylesheet`.
pub fn parse_stylesheet(css: &str) -> Result<Stylesheet, FerroError> {
    let mut stylesheet = Stylesheet::new();
    let mut pos = 0usize;
    let bytes = css.as_bytes();

    while pos < bytes.len() {
        skip_whitespace_comments(css, &mut pos);
        if pos >= css.len() { break; }

        if bytes[pos] == b'@' {
            parse_at_rule(css, &mut pos, &mut stylesheet);
        } else {
            if let Some(rule) = parse_style_rule(css, &mut pos) {
                stylesheet.rules.push(rule);
            }
        }
    }

    Ok(stylesheet)
}

/// Parse inline `style="..."` declarations.
pub fn parse_inline_style(style: &str) -> Vec<Declaration> {
    parse_declarations_block(style)
}

// ─── At-rules ─────────────────────────────────────────────────────────────────

fn parse_at_rule(css: &str, pos: &mut usize, sheet: &mut Stylesheet) {
    *pos += 1; // skip '@'
    let name = read_ident(css, pos);
    skip_whitespace_comments(css, pos);

    match name.to_ascii_lowercase().as_str() {
        "page" => {
            let selector_text = read_until_char(css, pos, '{').trim().to_string();
            let selector = if selector_text.is_empty() { None } else { Some(selector_text) };
            skip_char(css, pos, '{');
            let block = read_block(css, pos);
            let declarations = parse_declarations_block(&block);
            sheet.page_rules.push(PageRule { selector, declarations });
        }
        "font-face" => {
            skip_whitespace_comments(css, pos);
            skip_char(css, pos, '{');
            let block = read_block(css, pos);
            let declarations = parse_declarations_block(&block);
            let mut family = String::new();
            let mut src = String::new();
            let mut weight = None;
            let mut style = None;
            for decl in &declarations {
                match decl.property.as_str() {
                    "font-family" => {
                        family = match &decl.value {
                            CssValue::Keyword(s) => s.trim_matches('"').trim_matches('\'').to_string(),
                            _ => decl.value.to_string(),
                        };
                    }
                    "src"         => src    = decl.value.to_string(),
                    "font-weight" => weight = Some(decl.value.to_string()),
                    "font-style"  => style  = Some(decl.value.to_string()),
                    _ => {}
                }
            }
            if !family.is_empty() {
                sheet.font_face_rules.push(FontFaceRule { family, src, weight, style });
            }
        }
        "media" => {
            // Read and discard media query + outer block
            read_until_char(css, pos, '{');
            skip_char(css, pos, '{');
            // Read nested content — handle nested braces
            let mut depth = 1;
            while *pos < css.len() && depth > 0 {
                match css.as_bytes()[*pos] {
                    b'{' => { depth += 1; *pos += 1; }
                    b'}' => { depth -= 1; *pos += 1; }
                    _    => { *pos += 1; }
                }
            }
        }
        _ => {
            // Unknown at-rule: skip to ; or block.
            loop {
                if *pos >= css.len() { break; }
                match css.as_bytes()[*pos] {
                    b';' => { *pos += 1; break; }
                    b'{' => {
                        *pos += 1;
                        read_block(css, pos);
                        break;
                    }
                    _ => { *pos += 1; }
                }
            }
        }
    }
}

// ─── Style rules ──────────────────────────────────────────────────────────────

fn parse_style_rule(css: &str, pos: &mut usize) -> Option<StyleRule> {
    let selector_text = read_until_char(css, pos, '{');
    if *pos >= css.len() { return None; }
    *pos += 1; // skip '{'
    let block = read_block(css, pos);

    let selectors = parse_selectors(&selector_text);
    let declarations = parse_declarations_block(&block);

    if selectors.is_empty() && declarations.is_empty() { return None; }

    Some(StyleRule { selectors, declarations })
}

// ─── Selectors ────────────────────────────────────────────────────────────────

fn parse_selectors(text: &str) -> Vec<Selector> {
    text.split(',')
        .filter_map(|s| parse_one_selector(s.trim()))
        .collect()
}

fn parse_one_selector(text: &str) -> Option<Selector> {
    if text.is_empty() { return None; }

    let mut parts: Vec<(Combinator, Vec<SelectorComponent>)> = Vec::new();
    let mut current_components: Vec<SelectorComponent> = Vec::new();
    let mut combinator = Combinator::Descendant; // placeholder for first part
    let mut chars = text.char_indices().peekable();

    while let Some((_, ch)) = chars.next() {
        match ch {
            ' ' | '\t' | '\n' => {
                if !current_components.is_empty() {
                    parts.push((combinator, current_components.clone()));
                    current_components.clear();
                }
                // Check for explicit combinator after whitespace
                let mut next_comb = Combinator::Descendant;
                while let Some(&(_, nc)) = chars.peek() {
                    match nc {
                        ' ' | '\t' | '\n' => { chars.next(); }
                        '>' => { next_comb = Combinator::Child;   chars.next(); }
                        '+' => { next_comb = Combinator::Adjacent; chars.next(); }
                        '~' => { next_comb = Combinator::Sibling; chars.next(); }
                        _   => break,
                    }
                }
                combinator = next_comb;
            }
            '>' => {
                if !current_components.is_empty() {
                    parts.push((combinator, current_components.clone()));
                    current_components.clear();
                }
                combinator = Combinator::Child;
            }
            '+' => {
                if !current_components.is_empty() {
                    parts.push((combinator, current_components.clone()));
                    current_components.clear();
                }
                combinator = Combinator::Adjacent;
            }
            '~' => {
                if !current_components.is_empty() {
                    parts.push((combinator, current_components.clone()));
                    current_components.clear();
                }
                combinator = Combinator::Sibling;
            }
            '*' => {
                current_components.push(SelectorComponent::Universal);
            }
            '.' => {
                let name = read_ident_from_chars(&mut chars);
                current_components.push(SelectorComponent::Class(name));
            }
            '#' => {
                let id = read_ident_from_chars(&mut chars);
                current_components.push(SelectorComponent::Id(id));
            }
            ':' => {
                // pseudo-class or pseudo-element
                let is_element = chars.peek().map(|(_, c)| *c == ':').unwrap_or(false);
                if is_element { chars.next(); } // skip second ':'
                let name = read_ident_from_chars(&mut chars);
                // skip optional parens
                if chars.peek().map(|(_, c)| *c == '(').unwrap_or(false) {
                    let mut depth = 0;
                    loop {
                        match chars.next().map(|(_, c)| c) {
                            Some('(') => depth += 1,
                            Some(')') => { depth -= 1; if depth == 0 { break; } }
                            None      => break,
                            _         => {}
                        }
                    }
                }
                if is_element {
                    current_components.push(SelectorComponent::PseudoElement(name));
                } else {
                    current_components.push(SelectorComponent::PseudoClass(name));
                }
            }
            '[' => {
                // Attribute selector: [name op "value"]
                let mut content = String::new();
                loop {
                    match chars.next().map(|(_, c)| c) {
                        Some(']') | None => break,
                        Some(c) => content.push(c),
                    }
                }
                if let Some(comp) = parse_attr_selector(&content) {
                    current_components.push(comp);
                }
            }
            _ => {
                // Regular identifier (type selector)
                let mut name = String::from(ch);
                while let Some(&(_, nc)) = chars.peek() {
                    if nc.is_alphanumeric() || nc == '-' || nc == '_' {
                        name.push(nc);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if !name.is_empty() {
                    current_components.push(SelectorComponent::Type(name.to_ascii_lowercase()));
                }
            }
        }
    }

    if !current_components.is_empty() {
        parts.push((combinator, current_components));
    }

    if parts.is_empty() { None } else { Some(Selector { parts }) }
}

fn parse_attr_selector(content: &str) -> Option<SelectorComponent> {
    let content = content.trim();
    let ops = ["~=", "|=", "^=", "$=", "*=", "="];
    for op_str in &ops {
        if let Some(idx) = content.find(op_str) {
            let name  = content[..idx].trim().to_string();
            let value = content[idx + op_str.len()..].trim()
                .trim_matches('"').trim_matches('\'').to_string();
            let op = match *op_str {
                "="  => AttrOp::Equals,
                "~=" => AttrOp::Includes,
                "|=" => AttrOp::DashMatch,
                "^=" => AttrOp::StartsWith,
                "$=" => AttrOp::EndsWith,
                "*=" => AttrOp::Contains,
                _    => return None,
            };
            return Some(SelectorComponent::Attribute { name, op, value });
        }
    }
    // Just [attr]
    let name = content.trim().trim_matches('"').trim_matches('\'').to_string();
    if !name.is_empty() {
        Some(SelectorComponent::Attribute { name, op: AttrOp::Exists, value: String::new() })
    } else {
        None
    }
}

// ─── Declarations ─────────────────────────────────────────────────────────────

fn parse_declarations_block(block: &str) -> Vec<Declaration> {
    let mut result = Vec::new();

    for stmt in block.split(';') {
        let stmt = stmt.trim();
        if stmt.is_empty() { continue; }
        if let Some(colon) = stmt.find(':') {
            let prop  = stmt[..colon].trim().to_lowercase();
            let value_str = stmt[colon + 1..].trim();
            let (important, value_str) = if value_str.ends_with("!important") {
                (true, value_str[..value_str.len() - 10].trim())
            } else {
                (false, value_str)
            };

            // Expand shorthands
            expand_shorthand(&prop, value_str, important, &mut result);
        }
    }

    result
}

/// Expand a CSS shorthand property into individual declarations.
fn expand_shorthand(prop: &str, value: &str, important: bool, out: &mut Vec<Declaration>) {
    match prop {
        "margin" => expand_edge("margin", value, important, out),
        "padding" => expand_edge("padding", value, important, out),
        "border" => {
            expand_border_shorthand(value, "top",    important, out);
            expand_border_shorthand(value, "right",  important, out);
            expand_border_shorthand(value, "bottom", important, out);
            expand_border_shorthand(value, "left",   important, out);
        }
        "border-width" => expand_edge("border-width", value, important, out),
        "border-color" => expand_edge("border-color", value, important, out),
        "border-style" => expand_edge("border-style", value, important, out),
        "border-top" | "border-right" | "border-bottom" | "border-left" => {
            let side = prop.split('-').nth(1).unwrap_or("top");
            expand_border_shorthand(value, side, important, out);
        }
        "border-radius" => expand_border_radius(value, important, out),
        "font" => expand_font_shorthand(value, important, out),
        "background" => {
            // Minimal: treat as background-color
            let v = parse_css_value(value);
            out.push(Declaration { property: "background-color".to_string(), value: v, important });
        }
        "flex" => expand_flex_shorthand(value, important, out),
        "gap" | "grid-gap" => {
            let parts: Vec<&str> = value.split_whitespace().collect();
            let row_gap = parts.first().copied().unwrap_or("0");
            let col_gap = parts.get(1).copied().unwrap_or(row_gap);
            out.push(Declaration { property: "row-gap".to_string(),    value: parse_css_value(row_gap), important });
            out.push(Declaration { property: "column-gap".to_string(), value: parse_css_value(col_gap), important });
        }
        "overflow" => {
            let v = parse_css_value(value);
            out.push(Declaration { property: "overflow-x".to_string(), value: v.clone(), important });
            out.push(Declaration { property: "overflow-y".to_string(), value: v,         important });
        }
        _ => {
            out.push(Declaration { property: prop.to_string(), value: parse_css_value(value), important });
        }
    }
}

fn expand_edge(prefix: &str, value: &str, important: bool, out: &mut Vec<Declaration>) {
    let parts: Vec<&str> = value.split_whitespace().collect();
    let (top, right, bottom, left) = match parts.as_slice() {
        [all]               => (*all, *all, *all, *all),
        [tb, lr]            => (*tb,  *lr,  *tb,  *lr),
        [t, lr, b]          => (*t,   *lr,  *b,   *lr),
        [t, r, b, l, ..]    => (*t,   *r,   *b,   *l),
        _                   => ("0", "0", "0", "0"),
    };
    for (side, val) in [("top", top), ("right", right), ("bottom", bottom), ("left", left)] {
        let p = if prefix == "margin" || prefix == "padding" {
            format!("{}-{}", prefix, side)
        } else if prefix == "border-width" {
            format!("border-{}-width", side)
        } else if prefix == "border-color" {
            format!("border-{}-color", side)
        } else if prefix == "border-style" {
            format!("border-{}-style", side)
        } else {
            format!("{}-{}", prefix, side)
        };
        out.push(Declaration { property: p, value: parse_css_value(val), important });
    }
}

fn expand_border_shorthand(value: &str, side: &str, important: bool, out: &mut Vec<Declaration>) {
    for token in value.split_whitespace() {
        if let Some(w) = parse_border_width(token) {
            out.push(Declaration {
                property: format!("border-{side}-width"),
                value: CssValue::Length(w),
                important,
            });
        } else if is_border_style(token) {
            out.push(Declaration {
                property: format!("border-{side}-style"),
                value: CssValue::Keyword(token.to_string()),
                important,
            });
        } else {
            // Assume color
            out.push(Declaration {
                property: format!("border-{side}-color"),
                value: parse_css_value(token),
                important,
            });
        }
    }
}

fn parse_border_width(s: &str) -> Option<CssLength> {
    match s {
        "thin"   => Some(CssLength::Px(1.0)),
        "medium" => Some(CssLength::Px(3.0)),
        "thick"  => Some(CssLength::Px(5.0)),
        _ if s.ends_with("px") || s.ends_with("em") || s.ends_with("rem") || s.ends_with("mm") => {
            parse_css_length(s)
        }
        _ => None,
    }
}

fn is_border_style(s: &str) -> bool {
    matches!(s, "solid" | "dashed" | "dotted" | "double" | "none" | "hidden"
        | "groove" | "ridge" | "inset" | "outset")
}

fn expand_border_radius(value: &str, important: bool, out: &mut Vec<Declaration>) {
    let parts: Vec<&str> = value.split_whitespace().collect();
    let (tl, tr, br, bl) = match parts.as_slice() {
        [all]            => (*all, *all, *all, *all),
        [tl_br, tr_bl]   => (*tl_br, *tr_bl, *tl_br, *tr_bl),
        [tl, tr_bl, br]  => (*tl,    *tr_bl, *br,    *tr_bl),
        [tl, tr, br, bl] => (*tl,    *tr,    *br,    *bl),
        _                => ("0", "0", "0", "0"),
    };
    for (corner, val) in [
        ("border-top-left-radius",     tl),
        ("border-top-right-radius",    tr),
        ("border-bottom-right-radius", br),
        ("border-bottom-left-radius",  bl),
    ] {
        out.push(Declaration { property: corner.to_string(), value: parse_css_value(val), important });
    }
}

fn expand_font_shorthand(value: &str, important: bool, out: &mut Vec<Declaration>) {
    // Minimal: "italic bold 16px/1.5 Arial, sans-serif"
    let mut tokens: Vec<&str> = value.split_whitespace().collect();

    // Font style
    if tokens.first().map(|&t| matches!(t, "italic" | "oblique" | "normal")).unwrap_or(false) {
        let style = tokens.remove(0);
        out.push(Declaration { property: "font-style".to_string(), value: CssValue::Keyword(style.to_string()), important });
    }

    // Font weight
    if let Some(&first) = tokens.first() {
        if matches!(first, "bold" | "bolder" | "lighter")
            || first.parse::<u32>().is_ok()
        {
            let w = tokens.remove(0);
            out.push(Declaration { property: "font-weight".to_string(), value: parse_css_value(w), important });
        }
    }

    // Font size / line-height
    if let Some(&size_token) = tokens.first() {
        if size_token.ends_with("px") || size_token.ends_with("em")
            || size_token.ends_with("rem") || size_token.ends_with("%")
            || size_token.ends_with("pt")
        {
            tokens.remove(0);
            let (size_str, lh_str): (&str, Option<&str>) = if let Some(slash) = size_token.find('/') {
                (&size_token[..slash], Some(&size_token[slash + 1..]))
            } else {
                (size_token, None)
            };
            out.push(Declaration { property: "font-size".to_string(), value: parse_css_value(size_str), important });
            if let Some(lh) = lh_str {
                out.push(Declaration { property: "line-height".to_string(), value: parse_css_value(lh), important });
            }
        }
    }

    // Remaining = font-family
    if !tokens.is_empty() {
        let family = tokens.join(" ").trim_matches('"').trim_matches('\'').to_string();
        out.push(Declaration { property: "font-family".to_string(), value: CssValue::Keyword(family), important });
    }
}

fn expand_flex_shorthand(value: &str, important: bool, out: &mut Vec<Declaration>) {
    match value {
        "none" => {
            out.push(Declaration { property: "flex-grow".to_string(),   value: CssValue::Number(0.0), important });
            out.push(Declaration { property: "flex-shrink".to_string(), value: CssValue::Number(0.0), important });
            return;
        }
        "auto" => {
            out.push(Declaration { property: "flex-grow".to_string(),   value: CssValue::Number(1.0), important });
            out.push(Declaration { property: "flex-shrink".to_string(), value: CssValue::Number(1.0), important });
            return;
        }
        _ => {}
    }

    let parts: Vec<&str> = value.split_whitespace().collect();
    match parts.as_slice() {
        [grow] => {
            if let Ok(g) = grow.parse::<f32>() {
                out.push(Declaration { property: "flex-grow".to_string(), value: CssValue::Number(g), important });
            }
        }
        [grow, shrink] => {
            if let (Ok(g), Ok(s)) = (grow.parse::<f32>(), shrink.parse::<f32>()) {
                out.push(Declaration { property: "flex-grow".to_string(),   value: CssValue::Number(g), important });
                out.push(Declaration { property: "flex-shrink".to_string(), value: CssValue::Number(s), important });
            }
        }
        [grow, shrink, basis] => {
            if let (Ok(g), Ok(s)) = (grow.parse::<f32>(), shrink.parse::<f32>()) {
                out.push(Declaration { property: "flex-grow".to_string(),   value: CssValue::Number(g), important });
                out.push(Declaration { property: "flex-shrink".to_string(), value: CssValue::Number(s), important });
                out.push(Declaration { property: "flex-basis".to_string(),  value: parse_css_value(basis), important });
            }
        }
        _ => {}
    }
}

// ─── Value parsing ────────────────────────────────────────────────────────────

pub fn parse_css_value(s: &str) -> CssValue {
    let s = s.trim();
    if s.is_empty() { return CssValue::Keyword(String::new()); }

    match s {
        "none"    => return CssValue::None,
        "initial" => return CssValue::Initial,
        "inherit" => return CssValue::Inherit,
        "unset"   => return CssValue::Unset,
        _ => {}
    }

    if let Some(len) = parse_css_length(s) {
        return CssValue::Length(len);
    }

    if let Some(pct_str) = s.strip_suffix('%') {
        if let Ok(v) = pct_str.trim().parse::<f32>() {
            return CssValue::Percentage(v);
        }
    }

    if let Ok(v) = s.parse::<i32>() {
        return CssValue::Integer(v);
    }

    if let Ok(v) = s.parse::<f32>() {
        return CssValue::Number(v);
    }

    if let Some(color) = crate::css::compute::parse_color(s) {
        return CssValue::Color(color);
    }

    if let Some(url) = parse_url(s) {
        return CssValue::Url(url);
    }

    // Treat as keyword / string
    CssValue::Keyword(s.trim_matches('"').trim_matches('\'').to_string())
}

fn parse_css_length(s: &str) -> Option<CssLength> {
    let s = s.trim();

    if s == "0" { return Some(CssLength::Zero); }
    if s == "auto" { return Some(CssLength::Auto); }

    // Find where digits end and unit begins
    let unit_start = s.find(|c: char| c.is_alphabetic() || c == '%')?;
    let num_str = &s[..unit_start];
    let unit    = &s[unit_start..];
    let v: f32  = num_str.parse().ok()?;

    Some(match unit {
        "px"  => CssLength::Px(v),
        "mm"  => CssLength::Mm(v),
        "cm"  => CssLength::Cm(v),
        "pt"  => CssLength::Pt(v),
        "em"  => CssLength::Em(v),
        "rem" => CssLength::Rem(v),
        "%"   => CssLength::Percent(v),
        "vw"  => CssLength::Vw(v),
        "vh"  => CssLength::Vh(v),
        _     => return None,
    })
}

fn parse_url(s: &str) -> Option<String> {
    let inner = s.strip_prefix("url(")?.strip_suffix(')')?;
    let inner = inner.trim().trim_matches('"').trim_matches('\'');
    Some(inner.to_string())
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn skip_whitespace_comments(css: &str, pos: &mut usize) {
    let bytes = css.as_bytes();
    loop {
        while *pos < bytes.len() && (bytes[*pos] == b' ' || bytes[*pos] == b'\t'
            || bytes[*pos] == b'\n' || bytes[*pos] == b'\r')
        {
            *pos += 1;
        }
        if *pos + 1 < bytes.len() && bytes[*pos] == b'/' && bytes[*pos + 1] == b'*' {
            *pos += 2;
            while *pos + 1 < bytes.len() {
                if bytes[*pos] == b'*' && bytes[*pos + 1] == b'/' {
                    *pos += 2;
                    break;
                }
                *pos += 1;
            }
        } else {
            break;
        }
    }
}

fn read_ident(css: &str, pos: &mut usize) -> String {
    let start = *pos;
    let bytes = css.as_bytes();
    while *pos < bytes.len() && (bytes[*pos].is_ascii_alphanumeric() || bytes[*pos] == b'-' || bytes[*pos] == b'_') {
        *pos += 1;
    }
    css[start..*pos].to_string()
}

fn read_ident_from_chars(chars: &mut std::iter::Peekable<std::str::CharIndices>) -> String {
    let mut s = String::new();
    while let Some(&(_, c)) = chars.peek() {
        if c.is_alphanumeric() || c == '-' || c == '_' {
            s.push(c);
            chars.next();
        } else {
            break;
        }
    }
    s
}

fn read_until_char(css: &str, pos: &mut usize, stop: char) -> String {
    let start = *pos;
    let bytes = css.as_bytes();
    while *pos < bytes.len() && bytes[*pos] != stop as u8 {
        *pos += 1;
    }
    css[start..*pos].to_string()
}

fn skip_char(css: &str, pos: &mut usize, ch: char) {
    if *pos < css.len() && css.as_bytes()[*pos] == ch as u8 {
        *pos += 1;
    }
}

/// Read everything inside `{...}` (balanced), consuming the closing `}`.
fn read_block(css: &str, pos: &mut usize) -> String {
    let mut s = String::new();
    let mut depth = 0i32;
    let bytes = css.as_bytes();
    while *pos < bytes.len() {
        match bytes[*pos] {
            b'{' => { depth += 1; s.push('{'); *pos += 1; }
            b'}' => {
                if depth == 0 { *pos += 1; break; }
                depth -= 1;
                s.push('}');
                *pos += 1;
            }
            c => { s.push(c as char); *pos += 1; }
        }
    }
    s
}
