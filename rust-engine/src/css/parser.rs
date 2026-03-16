//! CSS parser — converts CSS text into a Stylesheet.
//!
//! Uses a custom parser that handles the subset of CSS needed for PDF rendering.

use std::collections::HashMap;

use crate::error::Result;

use super::properties::CssProperty;
use super::selector::Selector;
use super::stylesheet::{CssRule, Declaration, FontFaceRule, PageRule, Stylesheet};
use super::values::CssValue;

/// CSS parser that converts CSS text into a Stylesheet structure.
pub struct CssParser;

impl CssParser {
    /// Parse a CSS string into a Stylesheet.
    pub fn parse(css: &str) -> Result<Stylesheet> {
        let mut stylesheet = Stylesheet::new();
        let css = Self::preprocess(css);
        let mut chars = css.chars().peekable();
        let mut position = 0;

        while chars.peek().is_some() {
            Self::skip_whitespace_and_comments(&mut chars, &mut position);

            if chars.peek().is_none() {
                break;
            }

            // Check for at-rules
            if chars.peek() == Some(&'@') {
                Self::parse_at_rule(&mut chars, &mut position, &mut stylesheet)?;
            } else {
                // Regular rule
                if let Some((rule, vars)) = Self::parse_rule(&mut chars, &mut position)? {
                    stylesheet.rules.push(rule);
                    stylesheet.custom_properties.extend(vars);
                }
            }
        }

        Ok(stylesheet)
    }

    /// Parse a CSS style attribute (inline styles).
    pub fn parse_inline(style: &str) -> Result<Vec<Declaration>> {
        let (decls, _vars) = Self::parse_declarations(style)?;
        Ok(decls)
    }

    /// Preprocess CSS: normalize whitespace.
    fn preprocess(css: &str) -> String {
        css.replace("\r\n", "\n").replace('\r', "\n")
    }

    /// Skip whitespace and CSS comments.
    fn skip_whitespace_and_comments(
        chars: &mut std::iter::Peekable<std::str::Chars>,
        position: &mut usize,
    ) {
        loop {
            // Skip whitespace
            while chars.peek().map(|c| c.is_whitespace()).unwrap_or(false) {
                chars.next();
                *position += 1;
            }

            // Skip comments
            if chars.peek() == Some(&'/') {
                let mut clone = chars.clone();
                clone.next();
                if clone.peek() == Some(&'*') {
                    // Block comment
                    chars.next(); // /
                    chars.next(); // *
                    *position += 2;

                    loop {
                        match chars.next() {
                            Some('*') => {
                                *position += 1;
                                if chars.peek() == Some(&'/') {
                                    chars.next();
                                    *position += 1;
                                    break;
                                }
                            }
                            Some(_) => {
                                *position += 1;
                            }
                            None => break,
                        }
                    }
                    continue;
                }
            }

            break;
        }
    }

    /// Parse an at-rule (@page, @font-face, @media, etc.).
    fn parse_at_rule(
        chars: &mut std::iter::Peekable<std::str::Chars>,
        position: &mut usize,
        stylesheet: &mut Stylesheet,
    ) -> Result<()> {
        // Consume '@'
        chars.next();
        *position += 1;

        // Read at-rule name
        let name = Self::read_until(chars, position, |c| {
            c.is_whitespace() || c == '{' || c == ';'
        });

        match name.as_str() {
            "page" => {
                Self::skip_whitespace_and_comments(chars, position);
                // Optional page selector
                let selector_text = Self::read_until(chars, position, |c| c == '{');

                if chars.peek() == Some(&'{') {
                    chars.next();
                    *position += 1;
                }

                let block = Self::read_block(chars, position);
                let (declarations, _) = Self::parse_declarations(&block)?;

                let selector = if selector_text.trim().is_empty() {
                    None
                } else {
                    Some(selector_text.trim().to_string())
                };

                stylesheet.page_rules.push(PageRule {
                    selector,
                    declarations,
                    margin_rules: Vec::new(),
                });
            }
            "font-face" => {
                Self::skip_whitespace_and_comments(chars, position);
                if chars.peek() == Some(&'{') {
                    chars.next();
                    *position += 1;
                }

                let block = Self::read_block(chars, position);
                let (declarations, _) = Self::parse_declarations(&block)?;

                let mut family = String::new();
                let mut src = String::new();
                let mut weight = None;
                let mut style = None;

                for decl in declarations {
                    match decl.property {
                        CssProperty::FontFamily => family = decl.value.to_string(),
                        CssProperty::Custom(ref s) if s == "src" => src = decl.value.to_string(),
                        CssProperty::FontWeight => weight = Some(decl.value.to_string()),
                        CssProperty::FontStyle => style = Some(decl.value.to_string()),
                        _ => {}
                    }
                }

                if !family.is_empty() {
                    stylesheet.font_face_rules.push(FontFaceRule {
                        family,
                        src,
                        weight,
                        style,
                    });
                }
            }
            "media" => {
                // Skip media queries for now — read and discard the block
                Self::skip_whitespace_and_comments(chars, position);
                Self::read_until(chars, position, |c| c == '{');
                if chars.peek() == Some(&'{') {
                    chars.next();
                    *position += 1;
                }
                // Read the nested block
                Self::read_block(chars, position);
            }
            _ => {
                // Unknown at-rule — skip to semicolon or block
                loop {
                    match chars.peek() {
                        Some(&';') => {
                            chars.next();
                            *position += 1;
                            break;
                        }
                        Some(&'{') => {
                            chars.next();
                            *position += 1;
                            Self::read_block(chars, position);
                            break;
                        }
                        Some(_) => {
                            chars.next();
                            *position += 1;
                        }
                        None => break,
                    }
                }
            }
        }

        Ok(())
    }

    /// Parse a regular CSS rule. Returns the rule and any CSS custom properties found.
    fn parse_rule(
        chars: &mut std::iter::Peekable<std::str::Chars>,
        position: &mut usize,
    ) -> Result<Option<(CssRule, HashMap<String, String>)>> {
        Self::skip_whitespace_and_comments(chars, position);

        // Read selector text
        let selector_text = Self::read_until(chars, position, |c| c == '{');

        if chars.peek() == Some(&'{') {
            chars.next();
            *position += 1;
        } else {
            return Ok(None);
        }

        let block = Self::read_block(chars, position);

        let selector_text = selector_text.trim();
        if selector_text.is_empty() {
            return Ok(None);
        }

        // Parse selectors (comma-separated)
        let selectors: Vec<Selector> = selector_text
            .split(',')
            .filter_map(|s| Selector::parse(s.trim()))
            .collect();

        if selectors.is_empty() {
            return Ok(None);
        }

        let (declarations, vars) = Self::parse_declarations(&block)?;

        Ok(Some((
            CssRule {
                selectors,
                declarations,
            },
            vars,
        )))
    }

    /// Parse declarations from a block string.
    /// Returns `(declarations, custom_properties)`.
    /// Custom properties (`--name: value`) are extracted into the second tuple element.
    fn parse_declarations(block: &str) -> Result<(Vec<Declaration>, HashMap<String, String>)> {
        let mut declarations = Vec::new();
        let mut vars: HashMap<String, String> = HashMap::new();

        for part in block.split(';') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }

            if let Some(colon_pos) = part.find(':') {
                let prop_name = part[..colon_pos].trim();
                let mut value_str = part[colon_pos + 1..].trim().to_string();

                // CSS custom property (variable declaration)
                if prop_name.starts_with("--") {
                    vars.insert(prop_name.to_string(), value_str);
                    continue;
                }

                let important = if value_str.contains("!important") {
                    value_str = value_str.replace("!important", "").trim().to_string();
                    true
                } else {
                    false
                };

                let property = CssProperty::from_name(prop_name);

                // Handle shorthand properties
                let expanded = Self::expand_shorthand(&property, &value_str);

                if let Some(expanded_decls) = expanded {
                    for (prop, val) in expanded_decls {
                        declarations.push(if important {
                            Declaration::important(prop, val)
                        } else {
                            Declaration::new(prop, val)
                        });
                    }
                } else {
                    let value = CssValue::parse(&value_str);
                    declarations.push(if important {
                        Declaration::important(property, value)
                    } else {
                        Declaration::new(property, value)
                    });
                }
            }
        }

        Ok((declarations, vars))
    }

    /// Expand CSS shorthand properties into individual properties.
    fn expand_shorthand(
        property: &CssProperty,
        value_str: &str,
    ) -> Option<Vec<(CssProperty, CssValue)>> {
        let parts: Vec<&str> = value_str.split_whitespace().collect();

        match property {
            CssProperty::Custom(name) => match name.as_str() {
                "margin" => {
                    let values = Self::expand_box_shorthand(&parts);
                    Some(vec![
                        (CssProperty::MarginTop, values.0),
                        (CssProperty::MarginRight, values.1),
                        (CssProperty::MarginBottom, values.2),
                        (CssProperty::MarginLeft, values.3),
                    ])
                }
                "padding" => {
                    let values = Self::expand_box_shorthand(&parts);
                    Some(vec![
                        (CssProperty::PaddingTop, values.0),
                        (CssProperty::PaddingRight, values.1),
                        (CssProperty::PaddingBottom, values.2),
                        (CssProperty::PaddingLeft, values.3),
                    ])
                }
                "border" => {
                    let mut width = CssValue::Length(super::values::Length::px(1.0));
                    let mut style = CssValue::Keyword("solid".to_string());
                    let mut color = CssValue::Color(super::values::Color::black());

                    for part in &parts {
                        let val = CssValue::parse(part);
                        match &val {
                            CssValue::Length(_) => width = val,
                            CssValue::Color(_) => color = val,
                            CssValue::Keyword(k)
                                if ["solid", "dashed", "dotted", "double", "none"]
                                    .contains(&k.as_str()) =>
                            {
                                style = val
                            }
                            _ => {}
                        }
                    }

                    Some(vec![
                        (CssProperty::BorderTopWidth, width.clone()),
                        (CssProperty::BorderRightWidth, width.clone()),
                        (CssProperty::BorderBottomWidth, width.clone()),
                        (CssProperty::BorderLeftWidth, width),
                        (CssProperty::BorderTopStyle, style.clone()),
                        (CssProperty::BorderRightStyle, style.clone()),
                        (CssProperty::BorderBottomStyle, style.clone()),
                        (CssProperty::BorderLeftStyle, style),
                        (CssProperty::BorderTopColor, color.clone()),
                        (CssProperty::BorderRightColor, color.clone()),
                        (CssProperty::BorderBottomColor, color.clone()),
                        (CssProperty::BorderLeftColor, color),
                    ])
                }
                "border-width" => {
                    let values = Self::expand_box_shorthand(&parts);
                    Some(vec![
                        (CssProperty::BorderTopWidth, values.0),
                        (CssProperty::BorderRightWidth, values.1),
                        (CssProperty::BorderBottomWidth, values.2),
                        (CssProperty::BorderLeftWidth, values.3),
                    ])
                }
                "flex" => {
                    // flex: <grow> <shrink> <basis>
                    match parts.len() {
                        1 => Some(vec![
                            (CssProperty::FlexGrow, CssValue::parse(parts[0])),
                            (CssProperty::FlexShrink, CssValue::Number(1.0)),
                            (CssProperty::FlexBasis, CssValue::Auto),
                        ]),
                        2 => Some(vec![
                            (CssProperty::FlexGrow, CssValue::parse(parts[0])),
                            (CssProperty::FlexShrink, CssValue::parse(parts[1])),
                            (CssProperty::FlexBasis, CssValue::Auto),
                        ]),
                        3 => Some(vec![
                            (CssProperty::FlexGrow, CssValue::parse(parts[0])),
                            (CssProperty::FlexShrink, CssValue::parse(parts[1])),
                            (CssProperty::FlexBasis, CssValue::parse(parts[2])),
                        ]),
                        _ => None,
                    }
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// Expand box-model shorthand (margin, padding, border-width).
    fn expand_box_shorthand(parts: &[&str]) -> (CssValue, CssValue, CssValue, CssValue) {
        match parts.len() {
            1 => {
                let v = CssValue::parse(parts[0]);
                (v.clone(), v.clone(), v.clone(), v)
            }
            2 => {
                let vertical = CssValue::parse(parts[0]);
                let horizontal = CssValue::parse(parts[1]);
                (vertical.clone(), horizontal.clone(), vertical, horizontal)
            }
            3 => {
                let top = CssValue::parse(parts[0]);
                let horizontal = CssValue::parse(parts[1]);
                let bottom = CssValue::parse(parts[2]);
                (top, horizontal.clone(), bottom, horizontal)
            }
            4 => (
                CssValue::parse(parts[0]),
                CssValue::parse(parts[1]),
                CssValue::parse(parts[2]),
                CssValue::parse(parts[3]),
            ),
            _ => {
                let v = CssValue::parse("0");
                (v.clone(), v.clone(), v.clone(), v)
            }
        }
    }

    /// Read characters until a predicate matches or EOF.
    fn read_until(
        chars: &mut std::iter::Peekable<std::str::Chars>,
        position: &mut usize,
        pred: impl Fn(char) -> bool,
    ) -> String {
        let mut result = String::new();
        while let Some(&c) = chars.peek() {
            if pred(c) {
                break;
            }
            result.push(c);
            chars.next();
            *position += 1;
        }
        result
    }

    /// Read a block delimited by braces, handling nesting.
    fn read_block(
        chars: &mut std::iter::Peekable<std::str::Chars>,
        position: &mut usize,
    ) -> String {
        let mut result = String::new();
        let mut depth = 1;

        for c in chars.by_ref() {
            *position += 1;
            match c {
                '{' => {
                    depth += 1;
                    result.push(c);
                }
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    result.push(c);
                }
                _ => result.push(c),
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_rule() {
        let css = "h1 { color: red; font-size: 24px; }";
        let stylesheet = CssParser::parse(css).unwrap();
        assert_eq!(stylesheet.rules.len(), 1);
        assert_eq!(stylesheet.rules[0].declarations.len(), 2);
    }

    #[test]
    fn test_parse_multiple_rules() {
        let css = "h1 { color: red; } p { margin: 10px; }";
        let stylesheet = CssParser::parse(css).unwrap();
        assert_eq!(stylesheet.rules.len(), 2);
    }

    #[test]
    fn test_parse_comments() {
        let css = "/* comment */ h1 { color: red; /* inline */ }";
        let stylesheet = CssParser::parse(css).unwrap();
        assert_eq!(stylesheet.rules.len(), 1);
    }

    #[test]
    fn test_parse_page_rule() {
        let css = "@page { margin: 1cm; }";
        let stylesheet = CssParser::parse(css).unwrap();
        assert_eq!(stylesheet.page_rules.len(), 1);
    }

    #[test]
    fn test_parse_inline() {
        let decls = CssParser::parse_inline("color: red; font-size: 16px;").unwrap();
        assert_eq!(decls.len(), 2);
    }

    #[test]
    fn test_parse_important() {
        let decls = CssParser::parse_inline("color: red !important;").unwrap();
        assert_eq!(decls.len(), 1);
        assert!(decls[0].important);
    }
}
