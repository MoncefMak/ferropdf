//! CSS `::before` / `::after` pseudo-elements and generated content.
//!
//! Two passes around the cascade:
//!
//! 1. [`inject_pseudo_content`] (before the cascade) adds a synthetic
//!    element (tag [`PSEUDO_BEFORE`] / [`PSEUDO_AFTER`]) holding one empty
//!    text node to every element that a `::before` / `::after` rule targets.
//!    The cascade then styles it with those rules (colour, font, display,
//!    counters …), inheriting from the originating element.
//! 2. [`resolve_generated_content`] (after the cascade) walks the document
//!    in tree order, maintains CSS counters (`counter-reset`,
//!    `counter-increment`, `counter-set`, with nested scopes), and fills
//!    each synthetic element's text from its `content` value: strings,
//!    `counter()`, `counters()`, `attr()` and quotes. A pseudo-element whose
//!    `content` is `none` / `normal` / absent is not generated
//!    (`display: none`).

use crate::StyleTree;
use ferropdf_core::dom::{PSEUDO_AFTER, PSEUDO_BEFORE};
use ferropdf_core::{Display, Document, NodeId, NodeType};
use ferropdf_parse::Stylesheet;
use std::collections::HashMap;

/// Pass 1: add the synthetic `::before` / `::after` elements.
pub fn inject_pseudo_content(
    doc: &mut Document,
    sheets: &[&Stylesheet],
    media: &ferropdf_parse::css::MediaContext,
) {
    let rules = crate::matching::parse_rules(sheets, 1, media);
    let has_pseudo = rules
        .iter()
        .any(|r| r.selectors.0.iter().any(|s| s.pseudo_element().is_some()));
    if !has_pseudo {
        return;
    }
    let mut nth_cache = selectors::NthIndexCache::default();
    let elements: Vec<NodeId> = doc
        .nodes
        .iter()
        .filter(|(_, n)| n.node_type == NodeType::Element)
        .map(|(id, _)| id)
        .collect();
    let mut targets = Vec::new();
    for id in elements {
        for after in [false, true] {
            if crate::matching::has_pseudo_rules(doc, id, after, &rules, &mut nth_cache) {
                targets.push((id, after));
            }
        }
    }
    for (id, after) in targets {
        let tag = if after { PSEUDO_AFTER } else { PSEUDO_BEFORE };
        let pseudo = doc.create_element(tag, HashMap::new());
        let text = doc.create_text("");
        doc.append_child(pseudo, text);
        if after {
            doc.append_child(id, pseudo);
        } else {
            doc.prepend_child(id, pseudo);
        }
    }
}

/// Live counter instances: name → stack of (scope owner, value). A counter
/// created on an element is owned by that element's parent, so it is
/// visible to the element, its following siblings and their descendants.
#[derive(Default)]
struct Counters {
    map: HashMap<String, Vec<(NodeId, i32)>>,
    quote_depth: usize,
}

impl Counters {
    /// Create a counter instance, replacing one made by a sibling.
    fn instantiate(&mut self, name: &str, owner: NodeId, value: i32) {
        let stack = self.map.entry(name.to_string()).or_default();
        match stack.last_mut() {
            Some(top) if top.0 == owner => top.1 = value,
            _ => stack.push((owner, value)),
        }
    }

    fn apply(&mut self, style: &ferropdf_core::ComputedStyle, owner: NodeId) {
        for (name, v) in &style.counter_reset {
            self.instantiate(name, owner, *v);
        }
        for (name, v) in &style.counter_increment {
            if self.map.get(name).is_none_or(Vec::is_empty) {
                self.instantiate(name, owner, 0);
            }
            if let Some(top) = self.map.get_mut(name).and_then(|s| s.last_mut()) {
                top.1 = top.1.saturating_add(*v);
            }
        }
        for (name, v) in &style.counter_set {
            match self.map.get_mut(name).and_then(|s| s.last_mut()) {
                Some(top) => top.1 = *v,
                None => self.instantiate(name, owner, *v),
            }
        }
    }

    /// Drop the instances scoped to `owner`'s children.
    fn leave(&mut self, owner: NodeId) {
        for stack in self.map.values_mut() {
            while stack.last().is_some_and(|(o, _)| *o == owner) {
                stack.pop();
            }
        }
    }

    fn value(&self, name: &str) -> i32 {
        self.map
            .get(name)
            .and_then(|s| s.last())
            .map_or(0, |(_, v)| *v)
    }

    fn values(&self, name: &str) -> Vec<i32> {
        self.map
            .get(name)
            .map(|s| s.iter().map(|(_, v)| *v).collect())
            .filter(|v: &Vec<i32>| !v.is_empty())
            .unwrap_or_else(|| vec![0])
    }
}

/// Pass 2: evaluate counters and `content` for every synthetic element.
pub fn resolve_generated_content(doc: &mut Document, styles: &mut StyleTree) {
    let has_pseudo = doc.nodes.iter().any(|(_, n)| n.is_pseudo());
    let uses_counters = styles.values().any(|s| {
        !s.counter_reset.is_empty() || !s.counter_increment.is_empty() || !s.counter_set.is_empty()
    });
    if !has_pseudo && !uses_counters {
        return;
    }
    let mut counters = Counters::default();
    let mut fills: Vec<(NodeId, NodeId, Option<String>)> = Vec::new();
    walk(doc, doc.root(), styles, &mut counters, &mut fills, 0);
    for (pseudo, text_node, text) in fills {
        match text {
            Some(t) => doc.nodes[text_node].text = Some(t),
            None => {
                if let Some(s) = styles.get_mut(&pseudo) {
                    s.display = Display::None;
                }
            }
        }
    }
}

fn walk(
    doc: &Document,
    id: NodeId,
    styles: &StyleTree,
    counters: &mut Counters,
    fills: &mut Vec<(NodeId, NodeId, Option<String>)>,
    depth: usize,
) {
    if depth > ferropdf_core::MAX_DOM_DEPTH {
        return;
    }
    let node = doc.get(id);
    if node.node_type == NodeType::Element {
        let Some(style) = styles.get(&id) else {
            return;
        };
        // Boxes that are not rendered neither count nor generate content.
        if style.display == Display::None {
            if node.is_pseudo() {
                if let Some(&text) = node.children.first() {
                    fills.push((id, text, None));
                }
            }
            return;
        }
        let owner = node.parent.unwrap_or(id);
        counters.apply(style, owner);
        if node.is_pseudo() {
            let origin = node.parent.map(|p| doc.get(p));
            let text = style
                .content
                .as_deref()
                .and_then(|c| evaluate_content(c, counters, origin));
            if let Some(&text_node) = node.children.first() {
                fills.push((id, text_node, text));
            }
            counters.leave(id);
            return;
        }
    }
    for &child in &node.children {
        walk(doc, child, styles, counters, fills, depth + 1);
    }
    counters.leave(id);
}

/// Evaluate a `content` value. `None` for `none` / `normal` or when nothing
/// in it is understood.
fn evaluate_content(
    raw: &str,
    counters: &mut Counters,
    origin: Option<&ferropdf_core::Node>,
) -> Option<String> {
    let raw = raw.trim();
    if raw.eq_ignore_ascii_case("none") || raw.eq_ignore_ascii_case("normal") {
        return None;
    }
    let mut out = String::new();
    let mut understood = false;
    let chars: Vec<char> = raw.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '"' || c == '\'' {
            let (s, next) = read_string(&chars, i);
            out.push_str(&s);
            understood = true;
            i = next;
        } else {
            // An identifier or a function call.
            let start = i;
            while i < chars.len() && !chars[i].is_whitespace() && chars[i] != '(' {
                i += 1;
            }
            let name: String = chars[start..i].iter().collect::<String>().to_lowercase();
            if i < chars.len() && chars[i] == '(' {
                let open = i + 1;
                let mut depth = 1;
                i += 1;
                while i < chars.len() && depth > 0 {
                    match chars[i] {
                        '(' => depth += 1,
                        ')' => depth -= 1,
                        '"' | '\'' => i = read_string(&chars, i).1 - 1,
                        _ => {}
                    }
                    i += 1;
                }
                let inner: String = chars[open..i.saturating_sub(1)].iter().collect();
                let args = split_args(&inner);
                let piece = match name.as_str() {
                    "counter" => args.first().map(|n| {
                        let style = args.get(1).map_or("decimal", String::as_str);
                        format_counter(counters.value(n), style)
                    }),
                    "counters" => args.first().map(|n| {
                        let sep = args.get(1).cloned().unwrap_or_default();
                        let style = args.get(2).map_or("decimal", String::as_str);
                        counters
                            .values(n)
                            .into_iter()
                            .map(|v| format_counter(v, style))
                            .collect::<Vec<_>>()
                            .join(&sep)
                    }),
                    "attr" => args.first().map(|a| {
                        origin
                            .and_then(|o| o.attr(a))
                            .unwrap_or_default()
                            .to_string()
                    }),
                    _ => None,
                };
                if let Some(p) = piece {
                    out.push_str(&p);
                    understood = true;
                }
            } else {
                match name.as_str() {
                    "open-quote" => {
                        out.push(if counters.quote_depth % 2 == 0 {
                            '\u{201C}'
                        } else {
                            '\u{2018}'
                        });
                        counters.quote_depth += 1;
                        understood = true;
                    }
                    "close-quote" => {
                        counters.quote_depth = counters.quote_depth.saturating_sub(1);
                        out.push(if counters.quote_depth % 2 == 0 {
                            '\u{201D}'
                        } else {
                            '\u{2019}'
                        });
                        understood = true;
                    }
                    "no-open-quote" => {
                        counters.quote_depth += 1;
                        understood = true;
                    }
                    "no-close-quote" => {
                        counters.quote_depth = counters.quote_depth.saturating_sub(1);
                        understood = true;
                    }
                    _ => {}
                }
            }
        }
    }
    understood.then_some(out)
}

/// Read a quoted CSS string starting at `chars[start]` (the quote), decoding
/// escapes. Returns the string and the index after the closing quote.
fn read_string(chars: &[char], start: usize) -> (String, usize) {
    let quote = chars[start];
    let mut out = String::new();
    let mut i = start + 1;
    while i < chars.len() {
        let c = chars[i];
        if c == quote {
            return (out, i + 1);
        }
        if c == '\\' && i + 1 < chars.len() {
            i += 1;
            let mut hex = String::new();
            while hex.len() < 6 && i < chars.len() && chars[i].is_ascii_hexdigit() {
                hex.push(chars[i]);
                i += 1;
            }
            if hex.is_empty() {
                // `\"`, `\\`, or an escaped newline (line continuation).
                if chars[i] != '\n' {
                    out.push(chars[i]);
                }
                i += 1;
            } else {
                if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    out.push(if ch == '\0' { '\u{FFFD}' } else { ch });
                }
                // One whitespace after a hex escape is part of it.
                if i < chars.len() && chars[i].is_whitespace() {
                    i += 1;
                }
            }
            continue;
        }
        out.push(c);
        i += 1;
    }
    (out, i)
}

/// Split function arguments on top-level commas; strings are unquoted.
fn split_args(inner: &str) -> Vec<String> {
    let chars: Vec<char> = inner.chars().collect();
    let mut args = Vec::new();
    let mut cur = String::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '"' | '\'' => {
                let (s, next) = read_string(&chars, i);
                cur.push_str(&s);
                i = next;
                continue;
            }
            ',' => {
                args.push(std::mem::take(&mut cur));
            }
            c if c.is_whitespace() => {}
            c => cur.push(c),
        }
        i += 1;
    }
    args.push(cur);
    args.into_iter().map(|a| a.trim().to_string()).collect()
}

/// Format a counter value in a `list-style-type`-like counter style.
pub fn format_counter(value: i32, style: &str) -> String {
    let alpha = |v: i32, base: &[char]| -> String {
        if v <= 0 {
            return v.to_string();
        }
        let mut n = v as u32;
        let mut s = Vec::new();
        while n > 0 {
            n -= 1;
            s.push(base[(n % base.len() as u32) as usize]);
            n /= base.len() as u32;
        }
        s.iter().rev().collect()
    };
    let roman = |v: i32| -> String {
        if !(1..4000).contains(&v) {
            return v.to_string();
        }
        let table = [
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
        let mut n = v;
        let mut s = String::new();
        for (k, r) in table {
            while n >= k {
                s.push_str(r);
                n -= k;
            }
        }
        s
    };
    let latin: Vec<char> = ('a'..='z').collect();
    let greek: Vec<char> = "αβγδεζηθικλμνξοπρστυφχψω".chars().collect();
    match style.trim().to_ascii_lowercase().as_str() {
        "none" => String::new(),
        "disc" => "•".into(),
        "circle" => "◦".into(),
        "square" => "▪".into(),
        "decimal-leading-zero" if (0..10).contains(&value) => format!("0{value}"),
        "lower-roman" => roman(value).to_lowercase(),
        "upper-roman" => roman(value),
        "lower-alpha" | "lower-latin" => alpha(value, &latin),
        "upper-alpha" | "upper-latin" => alpha(value, &latin).to_uppercase(),
        "lower-greek" => alpha(value, &greek),
        _ => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(html: &str, css: &str) -> String {
        let mut pr = ferropdf_parse::parse(html).unwrap();
        let sheets = vec![ferropdf_parse::parse_stylesheet(css).unwrap()];
        let ua = ferropdf_parse::parse_stylesheet(ferropdf_parse::css::UA_CSS).unwrap();
        inject_pseudo_content(
            &mut pr.document,
            &[&ua, &sheets[0]],
            &ferropdf_parse::css::MediaContext::default(),
        );
        let mut styles =
            crate::resolve(&pr.document, &sheets, ferropdf_parse::css::UA_CSS, 480.0).unwrap();
        resolve_generated_content(&mut pr.document, &mut styles);
        // Concatenate visible text in tree order.
        let doc = &pr.document;
        let mut out = String::new();
        fn go(doc: &Document, id: NodeId, styles: &StyleTree, out: &mut String) {
            let n = doc.get(id);
            if n.is_element() && styles.get(&id).is_some_and(|s| s.display == Display::None) {
                return;
            }
            if let Some(t) = &n.text {
                out.push_str(t);
            }
            for &c in &n.children {
                go(doc, c, styles, out);
            }
        }
        go(doc, doc.root(), &styles, &mut out);
        // Blocks are not separated by spaces in the DOM: compare without
        // whitespace.
        out.split_whitespace().collect::<String>()
    }

    #[test]
    fn literal_content_and_styles() {
        let out = render(
            "<html><body><p class=n>x</p></body></html>",
            r#".n::before { content: "[" } .n::after { content: "]" }"#,
        );
        assert_eq!(out, "[x]");
    }

    #[test]
    fn section_numbering() {
        let out = render(
            "<html><body><h2>A</h2><h3>a</h3><h3>b</h3><h2>B</h2><h3>c</h3></body></html>",
            r#"body { counter-reset: s } h2 { counter-reset: ss; counter-increment: s }
               h3 { counter-increment: ss }
               h2::before { content: counter(s) ". " }
               h3::before { content: counter(s) "." counter(ss) " " }"#,
        );
        assert_eq!(out, "1.A1.1a1.2b2.B2.1c");
    }

    #[test]
    fn nested_counters_function() {
        let out = render(
            "<html><body><ol><li>a<ol><li>b</li><li>c</li></ol></li><li>d</li></ol></body></html>",
            r#"ol { counter-reset: item; list-style: none } li { counter-increment: item }
               li::before { content: counters(item, ".") " " }"#,
        );
        assert_eq!(out, "1a1.1b1.2c2d");
    }

    #[test]
    fn increment_on_pseudo_and_styles() {
        let out = render(
            "<html><body><p>x</p><p>y</p><p class=skip>z</p></body></html>",
            r#"body { counter-reset: n }
               p::before { counter-increment: n; content: counter(n, upper-roman) ") " }
               .skip::before { content: none }"#,
        );
        assert_eq!(out, "I)xII)yz");
    }

    #[test]
    fn implicit_counter_on_pseudo_is_local() {
        // As in browsers: without a counter-reset, an increment on the
        // pseudo-element creates a counter scoped to that element.
        let out = render(
            "<html><body><p>x</p><p>y</p></body></html>",
            r#"p::before { counter-increment: n; content: counter(n) }"#,
        );
        assert_eq!(out, "1x1y");
    }

    #[test]
    fn attr_quotes_and_escapes() {
        let out = render(
            r#"<html><body><a href="https://x.dev">l</a><q>q</q></body></html>"#,
            r#"a::after { content: " (" attr(href) ")" }
               q::before { content: open-quote } q::after { content: close-quote "\2014" }"#,
        );
        assert_eq!(out, "l(https://x.dev)\u{201C}q\u{201D}\u{2014}");
    }

    #[test]
    fn structural_selectors_ignore_generated_boxes() {
        let out = render(
            "<html><body><div><p>a</p><p>b</p></div></body></html>",
            r#"div::before { content: "+" } div > :first-child::before { content: "1" }
               p:first-child { color: red }"#,
        );
        assert_eq!(out, "+1ab");
    }

    #[test]
    fn counter_formats() {
        assert_eq!(format_counter(4, "lower-roman"), "iv");
        assert_eq!(format_counter(28, "upper-alpha"), "AB");
        assert_eq!(format_counter(7, "decimal-leading-zero"), "07");
        assert_eq!(format_counter(2, "lower-greek"), "β");
    }
}
