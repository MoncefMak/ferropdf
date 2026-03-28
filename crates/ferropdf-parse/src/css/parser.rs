use cssparser::{ParseError, Parser, ParserInput, Token};

/// A parsed CSS stylesheet
#[derive(Debug, Clone)]
pub struct Stylesheet {
    pub rules: Vec<StyleRule>,
    pub font_faces: Vec<FontFaceRule>,
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
            "page-break-before" => CssProperty::PageBreakBefore,
            "page-break-after" => CssProperty::PageBreakAfter,
            "page-break-inside" => CssProperty::PageBreakInside,
            "orphans" => CssProperty::Orphans,
            "widows" => CssProperty::Widows,
            "visibility" => CssProperty::Visibility,
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

    Ok(Stylesheet { rules, font_faces })
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

            // Collect value tokens until semicolon or end
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
                        value_parts.push(url);
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

            let value = value_parts.join(" ").trim().to_string();
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
    // Collect selector tokens until we hit '{'
    let mut selector_text = String::with_capacity(128);
    let start = parser.state();

    loop {
        match parser.next_including_whitespace() {
            Ok(&Token::CurlyBracketBlock) => break,
            Ok(token) => {
                selector_text.push_str(&token.to_css_string());
            }
            Err(_) => {
                parser.reset(&start);
                return None;
            }
        }
    }

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

    let mut value_tokens = Vec::new();
    let mut important = false;

    loop {
        match parser.next_including_whitespace() {
            Ok(&Token::Semicolon) => break,
            Ok(&Token::Delim('!')) => {
                if let Ok(kw) = parser.expect_ident() {
                    if kw.eq_ignore_ascii_case("important") {
                        important = true;
                    }
                }
            }
            Ok(token) => {
                value_tokens.push(token.to_css_string());
            }
            Err(_) => break,
        }
    }

    let raw_value = value_tokens.join("").trim().to_string();
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
