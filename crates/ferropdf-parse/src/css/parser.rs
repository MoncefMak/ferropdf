use cssparser::{ParseError, Parser, ParserInput, Token};

/// A parsed CSS stylesheet
#[derive(Debug, Clone, Default)]
pub struct Stylesheet {
    pub rules: Vec<StyleRule>,
    pub font_faces: Vec<FontFaceRule>,
    /// `@page { margin: ...; size: ... }` rules. Multiple `@page` rules
    /// override each other in source order; the last one wins. Only the
    /// barebones margin/size pair is captured today — `@page :first` and
    /// margin boxes (`@top-left { content: ... }`) are out of scope.
    pub page_rules: Vec<PageRule>,
}

#[derive(Debug, Clone, Default)]
pub struct PageRule {
    /// Raw value of the `margin` declaration (`"2cm"`, `"10mm 15mm"`, …),
    /// to be parsed by `PageMargins::from_css_str` downstream.
    pub margin: Option<String>,
    /// Raw value of the `size` declaration (`"A4"`, `"210mm 297mm"`, …),
    /// to be parsed by `PageSize::from_str` downstream.
    pub size: Option<String>,
}

/// A parsed @font-face rule
#[derive(Debug, Clone)]
pub struct FontFaceRule {
    pub font_family: String,
    /// Font source: url("path/to/font.woff2") or data URI
    pub src: String,
    pub font_weight: Option<String>,
    pub font_style: Option<String>,
}

/// A single CSS rule: selector(s) + declarations
#[derive(Debug, Clone)]
pub struct StyleRule {
    pub selectors: Vec<String>,
    pub declarations: Vec<Declaration>,
}

/// A CSS declaration: property + value
#[derive(Debug, Clone)]
pub struct Declaration {
    pub property: CssProperty,
    pub value: CssValue,
    pub important: bool,
}

/// CSS property names we support
#[derive(Debug, Clone, PartialEq)]
pub enum CssProperty {
    Display,
    Position,
    Width,
    Height,
    MinWidth,
    MaxWidth,
    MinHeight,
    MaxHeight,
    MarginTop,
    MarginRight,
    MarginBottom,
    MarginLeft,
    Margin,
    PaddingTop,
    PaddingRight,
    PaddingBottom,
    PaddingLeft,
    Padding,
    BorderTop,
    BorderRight,
    BorderBottom,
    BorderLeft,
    Border,
    BorderWidth,
    BorderColor,
    BorderStyle,
    BorderRadius,
    BorderCollapse,
    BorderSpacing,
    Color,
    BackgroundColor,
    Background,
    Opacity,
    FontFamily,
    FontSize,
    FontWeight,
    FontStyle,
    LineHeight,
    TextAlign,
    TextDecoration,
    LetterSpacing,
    FlexDirection,
    FlexWrap,
    JustifyContent,
    AlignItems,
    AlignSelf,
    Flex,
    FlexGrow,
    FlexShrink,
    FlexBasis,
    Gap,
    ColumnGap,
    RowGap,
    PageBreakBefore,
    PageBreakAfter,
    PageBreakInside,
    Orphans,
    Widows,
    Visibility,
    Top,
    Right,
    Bottom,
    Left,
    ZIndex,
    BoxShadow,
    Direction,
    UnicodeBidi,
    BoxSizing,
    ListStyleType,
    WhiteSpace,
    Unknown(String),
}

impl CssProperty {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "display" => CssProperty::Display,
            "position" => CssProperty::Position,
            "width" => CssProperty::Width,
            "height" => CssProperty::Height,
            "min-width" => CssProperty::MinWidth,
            "max-width" => CssProperty::MaxWidth,
            "min-height" => CssProperty::MinHeight,
            "max-height" => CssProperty::MaxHeight,
            "margin-top" => CssProperty::MarginTop,
            "margin-right" => CssProperty::MarginRight,
            "margin-bottom" => CssProperty::MarginBottom,
            "margin-left" => CssProperty::MarginLeft,
            "margin" => CssProperty::Margin,
            "padding-top" => CssProperty::PaddingTop,
            "padding-right" => CssProperty::PaddingRight,
            "padding-bottom" => CssProperty::PaddingBottom,
            "padding-left" => CssProperty::PaddingLeft,
            "padding" => CssProperty::Padding,
            "border-top" => CssProperty::BorderTop,
            "border-right" => CssProperty::BorderRight,
            "border-bottom" => CssProperty::BorderBottom,
            "border-left" => CssProperty::BorderLeft,
            "border" => CssProperty::Border,
            "border-width" => CssProperty::BorderWidth,
            "border-color" => CssProperty::BorderColor,
            "border-style" => CssProperty::BorderStyle,
            "border-radius" => CssProperty::BorderRadius,
            "border-collapse" => CssProperty::BorderCollapse,
            "border-spacing" => CssProperty::BorderSpacing,
            "color" => CssProperty::Color,
            "background-color" => CssProperty::BackgroundColor,
            "background" => CssProperty::Background,
            "opacity" => CssProperty::Opacity,
            "font-family" => CssProperty::FontFamily,
            "font-size" => CssProperty::FontSize,
            "font-weight" => CssProperty::FontWeight,
            "font-style" => CssProperty::FontStyle,
            "line-height" => CssProperty::LineHeight,
            "text-align" => CssProperty::TextAlign,
            "text-decoration" => CssProperty::TextDecoration,
            "letter-spacing" => CssProperty::LetterSpacing,
            "flex-direction" => CssProperty::FlexDirection,
            "flex-wrap" => CssProperty::FlexWrap,
            "justify-content" => CssProperty::JustifyContent,
            "align-items" => CssProperty::AlignItems,
            "align-self" => CssProperty::AlignSelf,
            "flex" => CssProperty::Flex,
            "flex-grow" => CssProperty::FlexGrow,
            "flex-shrink" => CssProperty::FlexShrink,
            "flex-basis" => CssProperty::FlexBasis,
            "gap" => CssProperty::Gap,
            "column-gap" => CssProperty::ColumnGap,
            "row-gap" => CssProperty::RowGap,
            // CSS Fragmentation L3 `break-*` and the legacy CSS 2.1 `page-break-*`
            // aliases share one set of computed fields.
            "page-break-before" | "break-before" => CssProperty::PageBreakBefore,
            "page-break-after" | "break-after" => CssProperty::PageBreakAfter,
            "page-break-inside" | "break-inside" => CssProperty::PageBreakInside,
            "orphans" => CssProperty::Orphans,
            "widows" => CssProperty::Widows,
            "visibility" => CssProperty::Visibility,
            "top" => CssProperty::Top,
            "right" => CssProperty::Right,
            "bottom" => CssProperty::Bottom,
            "left" => CssProperty::Left,
            "z-index" => CssProperty::ZIndex,
            "box-shadow" => CssProperty::BoxShadow,
            "direction" => CssProperty::Direction,
            "unicode-bidi" => CssProperty::UnicodeBidi,
            "box-sizing" => CssProperty::BoxSizing,
            "list-style-type" => CssProperty::ListStyleType,
            "list-style" => CssProperty::ListStyleType,
            "white-space" => CssProperty::WhiteSpace,
            other => CssProperty::Unknown(other.to_string()),
        }
    }
}

/// A CSS value (raw string for now — resolved by ferropdf-style)
#[derive(Debug, Clone)]
pub enum CssValue {
    String(String),
    Number(f32),
    Length(f32, String),
    Percentage(f32),
    Color(String),
    Keyword(String),
    Multiple(Vec<CssValue>),
}

impl CssValue {
    pub fn as_str(&self) -> &str {
        match self {
            CssValue::String(s) => s,
            CssValue::Keyword(s) => s,
            CssValue::Color(s) => s,
            _ => "",
        }
    }

    pub fn raw_string(&self) -> String {
        self.to_cow().into_owned()
    }

    /// Return a Cow<str> to avoid allocation for simple string/keyword/color values.
    pub fn to_cow(&self) -> std::borrow::Cow<'_, str> {
        use std::borrow::Cow;
        match self {
            CssValue::String(s) => Cow::Borrowed(s.as_str()),
            CssValue::Keyword(s) => Cow::Borrowed(s.as_str()),
            CssValue::Color(s) => Cow::Borrowed(s.as_str()),
            CssValue::Number(n) => Cow::Owned(n.to_string()),
            CssValue::Length(v, u) => Cow::Owned(format!("{}{}", v, u)),
            CssValue::Percentage(v) => Cow::Owned(format!("{}%", v)),
            CssValue::Multiple(vals) => Cow::Owned(
                vals.iter()
                    .map(|v| v.raw_string())
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
        }
    }
}

/// Parse a CSS stylesheet string into a Stylesheet
pub fn parse_stylesheet(css: &str) -> ferropdf_core::Result<Stylesheet> {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    let mut rules = Vec::new();
    let mut font_faces = Vec::new();
    let mut page_rules: Vec<PageRule> = Vec::new();

    while !parser.is_exhausted() {
        // Skip whitespace and comments
        let _ = parser.try_parse(|p| -> Result<(), ParseError<'_, ()>> {
            p.expect_whitespace()?;
            Ok(())
        });

        if parser.is_exhausted() {
            break;
        }

        // Try to parse at-rules
        let start = parser.state();
        if let Ok(Token::AtKeyword(ref name)) = parser.next() {
            let name_lower = name.to_lowercase();
            if name_lower == "font-face" {
                if let Some(ff) = parse_font_face_rule(&mut parser) {
                    font_faces.push(ff);
                }
            } else if name_lower == "media" {
                // Inline the rules of `@media print` queries as if they were
                // top-level — PDF rendering treats `print` as always active.
                // Other media types (`screen`, `(min-width: 800px)`, …) are
                // not relevant to a printed document and are skipped.
                let (extra_rules, extra_font_faces) = parse_media_at_rule(&mut parser);
                rules.extend(extra_rules);
                font_faces.extend(extra_font_faces);
            } else if name_lower == "page" {
                if let Some(pr) = parse_page_rule(&mut parser) {
                    page_rules.push(pr);
                }
            } else {
                let _ = skip_at_rule(&mut parser);
            }
            continue;
        }
        parser.reset(&start);

        // Parse a qualified rule (selector { declarations })
        match parse_qualified_rule(&mut parser) {
            Some(rule) => rules.push(rule),
            None => {
                // Skip to next rule
                let _ = parser.next();
            }
        }
    }

    Ok(Stylesheet {
        rules,
        font_faces,
        page_rules,
    })
}

fn skip_at_rule(parser: &mut Parser<'_, '_>) -> Result<(), ()> {
    loop {
        match parser.next() {
            Ok(&Token::CurlyBracketBlock) => {
                let _ = parser.parse_nested_block(|p| -> Result<(), ParseError<'_, ()>> {
                    while p.next().is_ok() {}
                    Ok(())
                });
                return Ok(());
            }
            Ok(&Token::Semicolon) => return Ok(()),
            Err(_) => return Err(()),
            _ => {}
        }
    }
}

/// Parse a barebones `@page { margin: ...; size: ...; }` at-rule. The
/// `@page` keyword has already been consumed. We skip any prelude (e.g.
/// `:first`, `:left`) and read declarations from the curly block. Only
/// `margin` and `size` are recognised — every other property is ignored.
fn parse_page_rule(parser: &mut Parser<'_, '_>) -> Option<PageRule> {
    // Skip any prelude tokens until we hit the curly block.
    loop {
        match parser.next_including_whitespace() {
            Ok(&Token::CurlyBracketBlock) => break,
            Ok(_) => {}
            Err(_) => return None,
        }
    }
    let mut rule = PageRule::default();
    let _ = parser.parse_nested_block(|p| -> Result<(), ParseError<'_, ()>> {
        while !p.is_exhausted() {
            if let Some(decl) = parse_declaration(p) {
                let raw_value = decl.value.to_cow().trim().to_string();
                if let CssProperty::Margin = decl.property {
                    rule.margin = Some(raw_value);
                } else if let CssProperty::Unknown(name) = &decl.property {
                    if name.eq_ignore_ascii_case("size") {
                        rule.size = Some(raw_value);
                    }
                }
            } else {
                let _ = p.next();
            }
        }
        Ok(())
    });
    Some(rule)
}

/// Parse a complete `@media ... { ... }` at-rule. The `@media` keyword
/// has already been consumed by the caller. Walks the prelude looking
/// for `print` (or `all`); if found, descends into the curly block and
/// parses its contents as if they were top-level rules. Otherwise the
/// block is consumed and discarded.
fn parse_media_at_rule(parser: &mut Parser<'_, '_>) -> (Vec<StyleRule>, Vec<FontFaceRule>) {
    let mut should_inline = false;

    // Walk prelude tokens until we encounter the `{...}` block. cssparser
    // yields `Token::CurlyBracketBlock` without entering it; the next call
    // to `parse_nested_block` consumes its contents.
    loop {
        match parser.next_including_whitespace() {
            Ok(&Token::CurlyBracketBlock) => break,
            Ok(Token::Ident(name)) => {
                if name.eq_ignore_ascii_case("print") || name.eq_ignore_ascii_case("all") {
                    should_inline = true;
                }
            }
            // `screen`, comma-separated lists, `(min-width: ...)`, etc. —
            // ignored. We only flatten when we positively saw `print`/`all`.
            Ok(_) => {}
            Err(_) => return (Vec::new(), Vec::new()),
        }
    }

    if !should_inline {
        // We've already consumed the `{`; drain its contents and discard.
        let _ = parser.parse_nested_block(|p| -> Result<(), ParseError<'_, ()>> {
            while p.next().is_ok() {}
            Ok(())
        });
        return (Vec::new(), Vec::new());
    }

    parser
        .parse_nested_block(
            |p| -> Result<(Vec<StyleRule>, Vec<FontFaceRule>), ParseError<'_, ()>> {
                let mut rules = Vec::new();
                let mut font_faces = Vec::new();
                while !p.is_exhausted() {
                    let _ = p.try_parse(|q| -> Result<(), ParseError<'_, ()>> {
                        q.expect_whitespace()?;
                        Ok(())
                    });
                    if p.is_exhausted() {
                        break;
                    }
                    let start = p.state();
                    if let Ok(Token::AtKeyword(ref name)) = p.next() {
                        let name_lower = name.to_lowercase();
                        if name_lower == "font-face" {
                            if let Some(ff) = parse_font_face_rule(p) {
                                font_faces.push(ff);
                            }
                        } else {
                            // Nested @media is not flattened — out of scope.
                            let _ = skip_at_rule(p);
                        }
                        continue;
                    }
                    p.reset(&start);
                    match parse_qualified_rule(p) {
                        Some(rule) => rules.push(rule),
                        None => {
                            let _ = p.next();
                        }
                    }
                }
                Ok((rules, font_faces))
            },
        )
        .unwrap_or_default()
}

/// Parse @font-face { font-family: ...; src: url(...); font-weight: ...; font-style: ...; }
fn parse_font_face_rule(parser: &mut Parser<'_, '_>) -> Option<FontFaceRule> {
    // Expect a { block
    if parser.expect_curly_bracket_block().is_err() {
        return None;
    }

    let mut font_family = None;
    let mut src = None;
    let mut font_weight = None;
    let mut font_style = None;

    let _ = parser.parse_nested_block(|p| -> Result<(), ParseError<'_, ()>> {
        while !p.is_exhausted() {
            // Skip whitespace/semicolons
            let _ = p.try_parse(|pp| -> Result<(), ParseError<'_, ()>> {
                pp.expect_whitespace()?;
                Ok(())
            });
            if p.is_exhausted() {
                break;
            }

            // Read property name
            let prop_name = match p.expect_ident() {
                Ok(name) => name.to_lowercase(),
                Err(_) => {
                    let _ = p.next();
                    continue;
                }
            };

            // Expect colon
            if p.expect_colon().is_err() {
                continue;
            }

            // Collect value tokens until semicolon or end.
            // For `src`, capture only the first url() value (skip format(), later url()s).
            let is_src = prop_name == "src";
            let mut first_url: Option<String> = None;
            let mut value_parts: Vec<String> = Vec::new();
            loop {
                match p.next_including_whitespace() {
                    Ok(Token::Semicolon) => break,
                    Ok(Token::Function(ref name)) if name.eq_ignore_ascii_case("url") => {
                        let url = p
                            .parse_nested_block(|pp| -> Result<String, ParseError<'_, ()>> {
                                match pp.next()? {
                                    Token::QuotedString(ref s) => Ok(s.to_string()),
                                    Token::UnquotedUrl(ref s) => Ok(s.to_string()),
                                    other => Ok(other.to_css_string()),
                                }
                            })
                            .unwrap_or_default();
                        if is_src && first_url.is_none() {
                            first_url = Some(url.clone());
                        }
                        value_parts.push(url);
                    }
                    Ok(Token::Function(ref _name)) => {
                        // Skip format() and other functions
                        let _ = p.parse_nested_block(|pp| -> Result<(), ParseError<'_, ()>> {
                            while pp.next().is_ok() {}
                            Ok(())
                        });
                    }
                    Ok(Token::QuotedString(ref s)) => {
                        value_parts.push(s.to_string());
                    }
                    Ok(Token::Ident(ref s)) => {
                        value_parts.push(s.to_string());
                    }
                    Ok(Token::Number { value, .. }) => {
                        value_parts.push(format!("{}", *value as i32));
                    }
                    Ok(Token::WhiteSpace(_)) => {}
                    Ok(Token::Comma) => {}
                    Err(_) => break,
                    _ => {}
                }
            }

            let value = if is_src {
                first_url.unwrap_or_else(|| value_parts.join(" ").trim().to_string())
            } else {
                value_parts.join(" ").trim().to_string()
            };
            match prop_name.as_str() {
                "font-family" => font_family = Some(value),
                "src" => src = Some(value),
                "font-weight" => font_weight = Some(value),
                "font-style" => font_style = Some(value),
                _ => {}
            }
        }
        Ok(())
    });

    match (font_family, src) {
        (Some(family), Some(src)) => Some(FontFaceRule {
            font_family: family,
            src,
            font_weight,
            font_style,
        }),
        _ => None,
    }
}

fn parse_qualified_rule(parser: &mut Parser<'_, '_>) -> Option<StyleRule> {
    // Capture the original source slice between the start of the rule and the
    // opening `{`. Reconstructing the selector token-by-token via
    // `Token::to_css_string()` is unsafe for functional pseudo-classes:
    // `Token::Function("nth-child")` only round-trips to `nth-child(` and the
    // arguments inside the function block are skipped by
    // `next_including_whitespace`, so we'd silently truncate
    // `:nth-child(odd)` to `:nth-child(`. Slicing the source preserves
    // every character (including arguments and the closing paren).
    let saved = parser.state();
    let selector_start = saved.position();
    let selector_end;

    loop {
        // Position *before* consuming the next token: after the loop breaks
        // on `{`, this points one past the last selector character.
        let pos_before = parser.position();
        match parser.next_including_whitespace() {
            Ok(&Token::CurlyBracketBlock) => {
                selector_end = pos_before;
                break;
            }
            // Block-opener tokens (`:nth-child(`, `:not(`, `[lang]`,
            // `(args)`) are yielded without their contents —
            // `parser.position()` then points just after the open delimiter,
            // and a subsequent `next` silently skips to the matching close
            // PLUS the next top-level token in one go. That breaks
            // position-based slicing because `pos_before` ends up inside the
            // block. Explicitly enter the nested block so position advances
            // past the close delimiter and `pos_before` of the next
            // iteration is meaningful.
            Ok(&Token::Function(_) | &Token::ParenthesisBlock | &Token::SquareBracketBlock) => {
                let _ =
                    parser.parse_nested_block(|p| -> Result<(), cssparser::ParseError<'_, ()>> {
                        while p.next_including_whitespace_and_comments().is_ok() {}
                        Ok(())
                    });
            }
            Ok(_) => {}
            Err(_) => {
                parser.reset(&saved);
                return None;
            }
        }
    }

    let selector_text = parser.slice(selector_start..selector_end);
    let selectors: Vec<String> = selector_text
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if selectors.is_empty() {
        return None;
    }

    // Parse declarations inside the block
    let declarations = parser
        .parse_nested_block(|p| {
            let mut decls = Vec::new();
            while !p.is_exhausted() {
                if let Some(decl) = parse_declaration(p) {
                    decls.push(decl);
                } else {
                    // Skip to next semicolon or end
                    let _ = p.next();
                }
            }
            Ok::<_, ParseError<'_, ()>>(decls)
        })
        .unwrap_or_default();

    Some(StyleRule {
        selectors,
        declarations,
    })
}

fn parse_declaration(parser: &mut Parser<'_, '_>) -> Option<Declaration> {
    let _ = parser.try_parse(|p| -> Result<(), ParseError<'_, ()>> {
        p.expect_whitespace()?;
        Ok(())
    });

    let name = parser.expect_ident().ok()?.to_string();
    parser.expect_colon().ok()?;

    // Slice the raw value text from the source so that block-opening
    // tokens (`var(--x)`, `rgb(...)`, `calc(...)`) round-trip correctly.
    // Token::to_css_string for a Function only returns `name(` and a
    // follow-up `next` call silently skips the function block, so a
    // token-by-token reconstruction would truncate `var(--primary)`
    // to `var(`.
    let value_start = parser.position();
    let mut value_end = value_start;
    let mut important = false;

    loop {
        let pos_before = parser.position();
        match parser.next_including_whitespace() {
            Ok(&Token::Semicolon) => {
                value_end = pos_before;
                break;
            }
            Ok(&Token::Delim('!')) => {
                if let Ok(kw) = parser.expect_ident() {
                    if kw.eq_ignore_ascii_case("important") {
                        important = true;
                        value_end = pos_before;
                    }
                }
            }
            Ok(&Token::Function(_) | &Token::ParenthesisBlock | &Token::SquareBracketBlock) => {
                // Drain the nested block so position() advances past the
                // closing delimiter.
                let _ =
                    parser.parse_nested_block(|p| -> Result<(), cssparser::ParseError<'_, ()>> {
                        while p.next_including_whitespace_and_comments().is_ok() {}
                        Ok(())
                    });
                value_end = parser.position();
            }
            Ok(_) => {
                value_end = parser.position();
            }
            Err(_) => break,
        }
    }

    let raw_value = parser.slice(value_start..value_end).trim().to_string();
    if raw_value.is_empty() {
        return None;
    }

    let property = CssProperty::from_str(&name);
    let value = parse_css_value(&raw_value);

    Some(Declaration {
        property,
        value,
        important,
    })
}

fn parse_css_value(raw: &str) -> CssValue {
    let raw = raw.trim();

    // Try as a number
    if let Ok(n) = raw.parse::<f32>() {
        return CssValue::Number(n);
    }

    // Try as percentage
    if let Some(v) = raw.strip_suffix('%') {
        if let Ok(n) = v.trim().parse::<f32>() {
            return CssValue::Percentage(n);
        }
    }

    // Try as length
    for unit in &["px", "pt", "em", "rem", "mm", "cm", "in", "vh", "vw"] {
        if let Some(v) = raw.strip_suffix(unit) {
            if let Ok(n) = v.trim().parse::<f32>() {
                return CssValue::Length(n, unit.to_string());
            }
        }
    }

    // Try as color
    if raw.starts_with('#') || raw.starts_with("rgb") || raw.starts_with("hsl") {
        return CssValue::Color(raw.to_string());
    }

    // Check for multiple values (space-separated)
    let parts: Vec<&str> = raw.split_whitespace().collect();
    if parts.len() > 1 {
        let values: Vec<CssValue> = parts.iter().map(|p| parse_css_value(p)).collect();
        return CssValue::Multiple(values);
    }

    // Default: keyword or string
    CssValue::Keyword(raw.to_string())
}

/// Parse inline style declarations directly (e.g. from a `style=""` attribute).
/// Avoids the overhead of wrapping in a fake CSS rule and parsing selectors.
pub fn parse_inline_declarations(style_attr: &str) -> Vec<Declaration> {
    let mut input = ParserInput::new(style_attr);
    let mut parser = Parser::new(&mut input);
    let mut decls = Vec::new();
    while !parser.is_exhausted() {
        if let Some(decl) = parse_declaration(&mut parser) {
            decls.push(decl);
        } else {
            let _ = parser.next();
        }
    }
    decls
}

trait ToCssString {
    fn to_css_string(&self) -> String;
}

impl<'a> ToCssString for Token<'a> {
    fn to_css_string(&self) -> String {
        match self {
            Token::Ident(s) => s.to_string(),
            Token::Number { value, .. } => value.to_string(),
            Token::Percentage { unit_value, .. } => format!("{}%", unit_value * 100.0),
            Token::Dimension { value, unit, .. } => format!("{}{}", value, unit),
            Token::Hash(s) | Token::IDHash(s) => format!("#{}", s),
            Token::QuotedString(s) => format!("\"{}\"", s),
            Token::WhiteSpace(_) => " ".to_string(),
            Token::Colon => ":".to_string(),
            Token::Semicolon => ";".to_string(),
            Token::Comma => ",".to_string(),
            Token::Delim(c) => c.to_string(),
            Token::Function(name) => format!("{}(", name),
            Token::ParenthesisBlock => "(".to_string(),
            Token::SquareBracketBlock => "[".to_string(),
            Token::CurlyBracketBlock => "{".to_string(),
            Token::CloseParenthesis => ")".to_string(),
            Token::CloseSquareBracket => "]".to_string(),
            Token::CloseCurlyBracket => "}".to_string(),
            _ => String::new(),
        }
    }
}
