//! Evaluation of media query lists (Media Queries 4) for paged output.
//!
//! The document is printed: the media type is `print` (`all` also matches),
//! and the viewport is the page area, in CSS px. Supported features:
//! `width`, `height` (with `min-` / `max-` prefixes and the range syntax),
//! `aspect-ratio`, `orientation`, `color`, `monochrome`, `grid`, `hover`,
//! `pointer`, `prefers-color-scheme`, `prefers-reduced-motion`. An unknown
//! feature or malformed query does not match.

/// What the queries are evaluated against.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MediaContext {
    /// Viewport width and height in CSS px.
    pub width: f32,
    pub height: f32,
}

impl Default for MediaContext {
    /// A4 portrait with 20mm margins: 170mm × 257mm.
    fn default() -> Self {
        Self {
            width: 642.5,
            height: 971.3,
        }
    }
}

/// Whether a comma-separated media query list matches. An empty list
/// matches (it is `all`).
pub fn media_matches(list: &str, ctx: &MediaContext) -> bool {
    let list = list.trim();
    if list.is_empty() {
        return true;
    }
    split_top_level(list, ',')
        .iter()
        .any(|q| query_matches(q, ctx).unwrap_or(false))
}

fn split_top_level(s: &str, sep: char) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0i32, 0);
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            c if c == sep && depth == 0 => {
                out.push(s[start..i].trim());
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    out.push(s[start..].trim());
    out
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),
    Group(String),
}

/// Split a query into identifiers and parenthesised groups.
fn tokens(s: &str) -> Option<Vec<Tok>> {
    let mut out = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '(' {
            let mut depth = 0;
            let start = i;
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
            if depth != 0 {
                return None;
            }
            out.push(Tok::Group(chars[start + 1..i].iter().collect()));
            i += 1;
        } else {
            let start = i;
            while i < chars.len() && !chars[i].is_whitespace() && chars[i] != '(' {
                i += 1;
            }
            out.push(Tok::Ident(
                chars[start..i]
                    .iter()
                    .collect::<String>()
                    .to_ascii_lowercase(),
            ));
        }
    }
    Some(out)
}

/// `[not | only]? <type> [and <condition>]?` or `<condition>`.
fn query_matches(q: &str, ctx: &MediaContext) -> Option<bool> {
    let toks = tokens(q)?;
    let (negate, rest) = match toks.first() {
        Some(Tok::Ident(w)) if w == "not" && matches!(toks.get(1), Some(Tok::Ident(_))) => {
            (true, &toks[1..])
        }
        Some(Tok::Ident(w)) if w == "only" => (false, &toks[1..]),
        _ => (false, &toks[..]),
    };
    let result = match rest.first() {
        Some(Tok::Ident(t)) if t != "not" => {
            let type_ok = matches!(t.as_str(), "all" | "print");
            match rest.get(1) {
                None => type_ok,
                Some(Tok::Ident(a)) if a == "and" => type_ok && condition(&rest[2..], ctx)?,
                _ => return None,
            }
        }
        _ => condition(rest, ctx)?,
    };
    Some(result != negate)
}

/// `not <in-parens>` | `<in-parens> [and <in-parens>]*` | `… [or …]*`.
fn condition(toks: &[Tok], ctx: &MediaContext) -> Option<bool> {
    match toks {
        [Tok::Ident(n), g] if n == "not" => Some(!in_parens(g, ctx)?),
        [first, rest @ ..] => {
            let mut value = in_parens(first, ctx)?;
            let mut op: Option<&str> = None;
            for pair in rest.chunks(2) {
                let [Tok::Ident(o), g] = pair else {
                    return None;
                };
                if o != "and" && o != "or" || op.is_some_and(|p| p != o) {
                    return None; // mixing and / or needs parentheses
                }
                op = Some(o);
                let v = in_parens(g, ctx)?;
                value = if o == "and" { value && v } else { value || v };
            }
            Some(value)
        }
        [] => None,
    }
}

fn in_parens(t: &Tok, ctx: &MediaContext) -> Option<bool> {
    let Tok::Group(inner) = t else {
        return None;
    };
    let inner = inner.trim();
    // A nested condition: `((a) or (b))`, `(not (a))`.
    if inner.starts_with('(') || inner.to_ascii_lowercase().starts_with("not ") {
        return condition(&tokens(inner)?, ctx);
    }
    feature(inner, ctx)
}

/// A value: a length in px, a ratio, a number or an identifier.
#[derive(Debug, Clone, PartialEq)]
enum Value {
    Num(f32),
    Ident(String),
}

fn value(s: &str) -> Option<Value> {
    let s = s.trim().to_ascii_lowercase();
    if let Some((a, b)) = s.split_once('/') {
        let (a, b) = (a.trim().parse::<f32>().ok()?, b.trim().parse::<f32>().ok()?);
        return (b != 0.0).then_some(Value::Num(a / b));
    }
    let units = [
        ("px", 1.0),
        ("rem", 16.0),
        ("em", 16.0),
        ("pt", 4.0 / 3.0),
        ("pc", 16.0),
        ("mm", 96.0 / 25.4),
        ("cm", 96.0 / 2.54),
        ("in", 96.0),
        ("q", 96.0 / 101.6),
    ];
    for (unit, factor) in units {
        if let Some(n) = s.strip_suffix(unit) {
            if let Ok(v) = n.trim().parse::<f32>() {
                return Some(Value::Num(v * factor));
            }
        }
    }
    if let Ok(v) = s.parse::<f32>() {
        return Some(Value::Num(v));
    }
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-')
        .then_some(Value::Ident(s))
}

fn numeric_feature(name: &str, ctx: &MediaContext) -> Option<f32> {
    Some(match name {
        "width" => ctx.width,
        "height" => ctx.height,
        "aspect-ratio" => ctx.width / ctx.height.max(f32::MIN_POSITIVE),
        "color" => 8.0,
        "monochrome" | "grid" | "color-index" => 0.0,
        "resolution" => 96.0,
        _ => return None,
    })
}

fn compare(a: f32, op: &str, b: f32) -> Option<bool> {
    const EPS: f32 = 0.01;
    Some(match op {
        "<" => a < b - EPS,
        "<=" => a <= b + EPS,
        ">" => a > b + EPS,
        ">=" => a >= b - EPS,
        "=" => (a - b).abs() <= EPS,
        _ => return None,
    })
}

fn feature(inner: &str, ctx: &MediaContext) -> Option<bool> {
    // Range syntax: `width >= 600px`, `400px <= width < 800px`.
    if inner.contains(['<', '>', '=']) {
        // Tokenise on the operators, longest first at each position.
        let mut parts = Vec::new();
        let mut operators = Vec::new();
        let mut rest = inner;
        loop {
            let found = ["<=", ">=", "<", ">", "="]
                .iter()
                .filter_map(|op| rest.find(op).map(|i| (i, *op)))
                .min_by_key(|(i, op)| (*i, usize::MAX - op.len()));
            match found {
                Some((i, op)) => {
                    parts.push(rest[..i].trim());
                    operators.push(op);
                    rest = &rest[i + op.len()..];
                }
                None => {
                    parts.push(rest.trim());
                    break;
                }
            }
        }
        let num = |s: &str| -> Option<f32> {
            match numeric_feature(&s.to_ascii_lowercase(), ctx) {
                Some(v) => Some(v),
                None => match value(s)? {
                    Value::Num(v) => Some(v),
                    Value::Ident(_) => None,
                },
            }
        };
        return match (parts.as_slice(), operators.as_slice()) {
            ([a, b], [op]) => compare(num(a)?, op, num(b)?),
            ([a, f, b], [op1, op2]) => {
                let v = num(f)?;
                Some(compare(num(a)?, op1, v)? && compare(v, op2, num(b)?)?)
            }
            _ => None,
        };
    }

    let (name, val) = match inner.split_once(':') {
        Some((n, v)) => (n.trim().to_ascii_lowercase(), Some(value(v)?)),
        None => (inner.trim().to_ascii_lowercase(), None),
    };
    let (prefix, base) = match name.split_once('-') {
        Some((p @ ("min" | "max"), rest)) => (Some(p), rest),
        _ => (None, name.as_str()),
    };
    match (base, &val) {
        ("orientation", Some(Value::Ident(o))) if prefix.is_none() => {
            let portrait = ctx.height >= ctx.width;
            Some(match o.as_str() {
                "portrait" => portrait,
                "landscape" => !portrait,
                _ => return None,
            })
        }
        ("hover" | "any-hover", None) => Some(false),
        ("hover" | "any-hover", Some(Value::Ident(v))) => Some(v == "none"),
        ("pointer" | "any-pointer", None) => Some(false),
        ("pointer" | "any-pointer", Some(Value::Ident(v))) => Some(v == "none"),
        ("prefers-color-scheme", Some(Value::Ident(v))) => Some(v == "light"),
        ("prefers-reduced-motion", Some(Value::Ident(v))) => Some(v == "no-preference"),
        ("prefers-reduced-motion", None) => Some(false),
        // Paper is not updated after printing, and has no scan mode.
        ("update", Some(Value::Ident(v))) => Some(v == "none"),
        ("scan", _) => Some(false),
        (_, None) => numeric_feature(base, ctx).map(|v| v != 0.0),
        (_, Some(Value::Num(target))) => {
            let actual = numeric_feature(base, ctx)?;
            match prefix {
                Some("min") => compare(actual, ">=", *target),
                Some("max") => compare(actual, "<=", *target),
                _ => compare(actual, "=", *target),
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A4: MediaContext = MediaContext {
        width: 642.5,
        height: 971.3,
    };

    fn m(q: &str) -> bool {
        media_matches(q, &A4)
    }

    #[test]
    fn media_types() {
        assert!(m("print") && m("all") && m("") && m("only print"));
        assert!(!m("screen") && !m("speech") && !m("not print"));
        assert!(m("not screen"));
        assert!(m("screen, print"));
    }

    #[test]
    fn width_features() {
        assert!(m("(min-width: 576px)") && m("(min-width: 640px)"));
        assert!(!m("(min-width: 768px)") && m("(max-width: 767.98px)"));
        assert!(m("print and (min-width: 600px)"));
        assert!(!m("screen and (min-width: 600px)"));
        assert!(m("(min-width: 30em)") && !m("(min-width: 48em)"));
        assert!(m("(width >= 600px)") && !m("(width > 700px)"));
        assert!(m("(600px <= width < 700px)") && !m("(400px <= width <= 600px)"));
        assert!(m("(min-height: 900px)"));
    }

    #[test]
    fn logic_and_other_features() {
        assert!(m("(orientation: portrait)") && !m("(orientation: landscape)"));
        assert!(m("(min-width: 1px) and (max-width: 700px)"));
        assert!(m("(min-width: 1000px) or (max-width: 700px)"));
        assert!(m("not (min-width: 1000px)"));
        assert!(m("((min-width: 1000px) or (orientation: portrait))"));
        assert!(m("(color)") && !m("(monochrome)"));
        assert!(m("(hover: none)") && !m("(hover: hover)"));
        assert!(m("(prefers-color-scheme: light)") && !m("(prefers-color-scheme: dark)"));
        assert!(m("(max-aspect-ratio: 1/1)") && !m("(min-aspect-ratio: 16/9)"));
        // Unknown features and malformed queries do not match.
        assert!(!m("(min-wobble: 3px)") && !m("(min-width: 1px) and") && !m("print and"));
        assert!(!m("(min-width: 1px) and (max-width: 2px) or (color)"));
    }
}
