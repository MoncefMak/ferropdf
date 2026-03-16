//! CSS values representation.

use std::fmt;

/// A CSS length value with unit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Length {
    pub value: f64,
    pub unit: LengthUnit,
}

impl Length {
    pub fn new(value: f64, unit: LengthUnit) -> Self {
        Self { value, unit }
    }

    pub fn zero() -> Self {
        Self {
            value: 0.0,
            unit: LengthUnit::Px,
        }
    }

    pub fn px(value: f64) -> Self {
        Self {
            value,
            unit: LengthUnit::Px,
        }
    }

    pub fn pt(value: f64) -> Self {
        Self {
            value,
            unit: LengthUnit::Pt,
        }
    }

    pub fn mm(value: f64) -> Self {
        Self {
            value,
            unit: LengthUnit::Mm,
        }
    }

    pub fn cm(value: f64) -> Self {
        Self {
            value,
            unit: LengthUnit::Cm,
        }
    }

    pub fn em(value: f64) -> Self {
        Self {
            value,
            unit: LengthUnit::Em,
        }
    }

    pub fn rem(value: f64) -> Self {
        Self {
            value,
            unit: LengthUnit::Rem,
        }
    }

    pub fn percent(value: f64) -> Self {
        Self {
            value,
            unit: LengthUnit::Percent,
        }
    }

    /// Convert this length to pixels given a context.
    /// `parent_size` is used for percentage calculations.
    /// `font_size` is used for em calculations.
    /// `root_font_size` is used for rem calculations.
    pub fn to_px(&self, parent_size: f64, font_size: f64, root_font_size: f64) -> f64 {
        match self.unit {
            LengthUnit::Px => self.value,
            LengthUnit::Pt => self.value * 96.0 / 72.0,
            LengthUnit::Mm => self.value * 96.0 / 25.4,
            LengthUnit::Cm => self.value * 96.0 / 2.54,
            LengthUnit::In => self.value * 96.0,
            LengthUnit::Em => self.value * font_size,
            LengthUnit::Rem => self.value * root_font_size,
            LengthUnit::Percent => self.value / 100.0 * parent_size,
            LengthUnit::Vw => self.value, // viewport-based, needs context
            LengthUnit::Vh => self.value,
        }
    }

    /// Convert to points for PDF output.
    pub fn to_pt(&self, parent_size: f64, font_size: f64, root_font_size: f64) -> f64 {
        let px = self.to_px(parent_size, font_size, root_font_size);
        px * 72.0 / 96.0
    }
}

impl Default for Length {
    fn default() -> Self {
        Self::zero()
    }
}

impl fmt::Display for Length {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.value, self.unit)
    }
}

/// CSS length units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthUnit {
    Px,
    Pt,
    Mm,
    Cm,
    In,
    Em,
    Rem,
    Percent,
    Vw,
    Vh,
}

impl fmt::Display for LengthUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LengthUnit::Px => write!(f, "px"),
            LengthUnit::Pt => write!(f, "pt"),
            LengthUnit::Mm => write!(f, "mm"),
            LengthUnit::Cm => write!(f, "cm"),
            LengthUnit::In => write!(f, "in"),
            LengthUnit::Em => write!(f, "em"),
            LengthUnit::Rem => write!(f, "rem"),
            LengthUnit::Percent => write!(f, "%"),
            LengthUnit::Vw => write!(f, "vw"),
            LengthUnit::Vh => write!(f, "vh"),
        }
    }
}

/// A CSS calc() expression tree.
#[derive(Debug, Clone, PartialEq)]
pub enum CalcExpr {
    Leaf(Length),
    Add(Box<CalcExpr>, Box<CalcExpr>),
    Sub(Box<CalcExpr>, Box<CalcExpr>),
    Mul(Box<CalcExpr>, f64),
    Div(Box<CalcExpr>, f64),
}

impl CalcExpr {
    /// Evaluate the expression to pixels given layout context.
    pub fn to_px(&self, parent_size: f64, font_size: f64, root_font_size: f64) -> f64 {
        match self {
            CalcExpr::Leaf(l) => l.to_px(parent_size, font_size, root_font_size),
            CalcExpr::Add(a, b) => {
                a.to_px(parent_size, font_size, root_font_size)
                    + b.to_px(parent_size, font_size, root_font_size)
            }
            CalcExpr::Sub(a, b) => {
                a.to_px(parent_size, font_size, root_font_size)
                    - b.to_px(parent_size, font_size, root_font_size)
            }
            CalcExpr::Mul(e, f) => e.to_px(parent_size, font_size, root_font_size) * f,
            CalcExpr::Div(e, f) => {
                if *f == 0.0 {
                    0.0
                } else {
                    e.to_px(parent_size, font_size, root_font_size) / f
                }
            }
        }
    }

    /// Parse a calc expression from the inner content (without "calc(" and ")").
    pub fn parse(inner: &str) -> Option<Self> {
        let tokens = Self::tokenize(inner.trim())?;
        if tokens.is_empty() {
            return None;
        }
        Self::parse_sum(&tokens)
    }

    fn tokenize(s: &str) -> Option<Vec<CalcToken>> {
        let mut tokens: Vec<CalcToken> = Vec::new();
        let mut chars = s.chars().peekable();
        while let Some(&c) = chars.peek() {
            match c {
                ' ' | '\t' | '\n' => {
                    chars.next();
                }
                '+' => {
                    chars.next();
                    tokens.push(CalcToken::Plus);
                }
                '*' => {
                    chars.next();
                    tokens.push(CalcToken::Star);
                }
                '/' => {
                    chars.next();
                    tokens.push(CalcToken::Slash);
                }
                '-' => {
                    chars.next();
                    let after_value = matches!(
                        tokens.last(),
                        Some(CalcToken::Length(_)) | Some(CalcToken::Number(_))
                    );
                    if after_value {
                        tokens.push(CalcToken::Minus);
                    } else {
                        // Unary minus — consume the following value token and negate it
                        while chars.peek() == Some(&' ') {
                            chars.next();
                        }
                        let mut val = String::from('-');
                        while let Some(&c2) = chars.peek() {
                            if c2.is_ascii_alphanumeric() || c2 == '.' || c2 == '%' {
                                val.push(c2);
                                chars.next();
                            } else {
                                break;
                            }
                        }
                        tokens.push(Self::parse_value_str(&val)?);
                    }
                }
                _ if c.is_ascii_alphanumeric() || c == '.' => {
                    let mut val = String::new();
                    while let Some(&c2) = chars.peek() {
                        if c2.is_ascii_alphanumeric() || c2 == '.' || c2 == '%' {
                            val.push(c2);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    tokens.push(Self::parse_value_str(&val)?);
                }
                _ => {
                    chars.next();
                }
            }
        }
        Some(tokens)
    }

    fn parse_value_str(s: &str) -> Option<CalcToken> {
        if let Ok(n) = s.parse::<f64>() {
            return Some(CalcToken::Number(n));
        }
        if let Some(pct) = s.strip_suffix('%') {
            if let Ok(v) = pct.parse::<f64>() {
                return Some(CalcToken::Length(Length::percent(v)));
            }
        }
        for (suffix, unit) in &[
            ("rem", LengthUnit::Rem),
            ("em", LengthUnit::Em),
            ("px", LengthUnit::Px),
            ("pt", LengthUnit::Pt),
            ("mm", LengthUnit::Mm),
            ("cm", LengthUnit::Cm),
            ("in", LengthUnit::In),
            ("vw", LengthUnit::Vw),
            ("vh", LengthUnit::Vh),
        ] {
            if let Some(num_str) = s.strip_suffix(suffix) {
                if let Ok(v) = num_str.parse::<f64>() {
                    return Some(CalcToken::Length(Length::new(v, *unit)));
                }
            }
        }
        None
    }

    /// Split a token slice on operator tokens matching `is_op`.
    /// Returns `(op_before_segment, segment_slice)` pairs; the first has `op = None`.
    fn split_on(
        tokens: &[CalcToken],
        is_op: impl Fn(&CalcToken) -> bool,
    ) -> Vec<(Option<bool>, &[CalcToken])> {
        let mut parts: Vec<(Option<bool>, &[CalcToken])> = Vec::new();
        let mut start = 0;
        let mut pending_op: Option<bool> = None;
        for (i, t) in tokens.iter().enumerate() {
            if is_op(t) && i > start {
                parts.push((pending_op, &tokens[start..i]));
                pending_op = Some(!matches!(t, CalcToken::Minus | CalcToken::Slash));
                start = i + 1;
            }
        }
        parts.push((pending_op, &tokens[start..]));
        parts
    }

    fn parse_sum(tokens: &[CalcToken]) -> Option<Self> {
        let parts = Self::split_on(tokens, |t| matches!(t, CalcToken::Plus | CalcToken::Minus));
        if parts.is_empty() {
            return None;
        }
        let mut result = Self::parse_product(parts[0].1)?;
        for (op, slice) in &parts[1..] {
            let rhs = Self::parse_product(slice)?;
            match op {
                Some(true) => result = CalcExpr::Add(Box::new(result), Box::new(rhs)),
                Some(false) => result = CalcExpr::Sub(Box::new(result), Box::new(rhs)),
                None => return None,
            }
        }
        Some(result)
    }

    fn parse_product(tokens: &[CalcToken]) -> Option<Self> {
        let parts = Self::split_on(tokens, |t| matches!(t, CalcToken::Star | CalcToken::Slash));
        if parts.is_empty() {
            return None;
        }
        let (mut result, mut is_num) = Self::parse_single(parts[0].1)?;
        for (op, slice) in &parts[1..] {
            let (rhs, rhs_is_num) = Self::parse_single(slice)?;
            let is_mul = op.unwrap_or(true);
            match (is_num, rhs_is_num, is_mul) {
                (true, false, true) => {
                    // scalar * length → Mul(length, scalar)
                    let s = Self::extract_num(&result)?;
                    result = CalcExpr::Mul(Box::new(rhs), s);
                    is_num = false;
                }
                (false, true, true) => {
                    // length * scalar → Mul(length, scalar)
                    let s = Self::extract_num(&rhs)?;
                    result = CalcExpr::Mul(Box::new(result), s);
                }
                (false, true, false) => {
                    // length / scalar → Div(length, scalar)
                    let s = Self::extract_num(&rhs)?;
                    result = CalcExpr::Div(Box::new(result), s);
                }
                _ => return None,
            }
        }
        Some(result)
    }

    fn parse_single(tokens: &[CalcToken]) -> Option<(CalcExpr, bool)> {
        match tokens {
            [CalcToken::Length(l)] => Some((CalcExpr::Leaf(*l), false)),
            [CalcToken::Number(n)] => Some((CalcExpr::Leaf(Length::px(*n)), true)),
            _ => None,
        }
    }

    fn extract_num(expr: &CalcExpr) -> Option<f64> {
        // Only valid to call when the expr was created from a Number token (px synthetic leaf)
        match expr {
            CalcExpr::Leaf(l) if l.unit == LengthUnit::Px => Some(l.value),
            _ => None,
        }
    }
}

/// Private token type for calc() parsing.
#[derive(Debug, Clone)]
enum CalcToken {
    Length(Length),
    Number(f64),
    Plus,
    Minus,
    Star,
    Slash,
}

/// An RGBA color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f32,
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub fn black() -> Self {
        Self::rgb(0, 0, 0)
    }

    pub fn white() -> Self {
        Self::rgb(255, 255, 255)
    }

    pub fn transparent() -> Self {
        Self::new(0, 0, 0, 0.0)
    }

    /// Parse a CSS color string.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().to_lowercase();

        // Named colors
        match s.as_str() {
            "black" => return Some(Self::rgb(0, 0, 0)),
            "white" => return Some(Self::rgb(255, 255, 255)),
            "red" => return Some(Self::rgb(255, 0, 0)),
            "green" => return Some(Self::rgb(0, 128, 0)),
            "blue" => return Some(Self::rgb(0, 0, 255)),
            "yellow" => return Some(Self::rgb(255, 255, 0)),
            "orange" => return Some(Self::rgb(255, 165, 0)),
            "purple" => return Some(Self::rgb(128, 0, 128)),
            "gray" | "grey" => return Some(Self::rgb(128, 128, 128)),
            "transparent" => return Some(Self::transparent()),
            "inherit" | "initial" | "unset" => return None,
            _ => {}
        }

        // Hex colors
        if let Some(hex) = s.strip_prefix('#') {
            return Self::parse_hex(hex);
        }

        // rgb() / rgba()
        if s.starts_with("rgb") {
            return Self::parse_rgb_function(&s);
        }

        None
    }

    fn parse_hex(hex: &str) -> Option<Self> {
        match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
                Some(Self::rgb(r, g, b))
            }
            4 => {
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
                let a = u8::from_str_radix(&hex[3..4].repeat(2), 16).ok()?;
                Some(Self::new(r, g, b, a as f32 / 255.0))
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Self::rgb(r, g, b))
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                Some(Self::new(r, g, b, a as f32 / 255.0))
            }
            _ => None,
        }
    }

    fn parse_rgb_function(s: &str) -> Option<Self> {
        let inner = s
            .trim_start_matches("rgba(")
            .trim_start_matches("rgb(")
            .trim_end_matches(')');

        let parts: Vec<&str> = inner.split([',', '/']).collect();

        if parts.len() >= 3 {
            let r = parts[0].trim().parse::<u8>().ok()?;
            let g = parts[1].trim().parse::<u8>().ok()?;
            let b = parts[2].trim().parse::<u8>().ok()?;
            let a = if parts.len() >= 4 {
                parts[3].trim().parse::<f32>().ok().unwrap_or(1.0)
            } else {
                1.0
            };
            Some(Self::new(r, g, b, a))
        } else {
            None
        }
    }

    /// Convert to printpdf color values (0.0—1.0 range).
    pub fn to_pdf_rgb(&self) -> (f32, f32, f32) {
        (
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
        )
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::black()
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if (self.a - 1.0).abs() < f32::EPSILON {
            write!(f, "rgb({}, {}, {})", self.r, self.g, self.b)
        } else {
            write!(f, "rgba({}, {}, {}, {})", self.r, self.g, self.b, self.a)
        }
    }
}

/// A general CSS value.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum CssValue {
    /// A length value.
    Length(Length),
    /// A color value.
    Color(Color),
    /// A keyword value (e.g., "auto", "none", "bold").
    Keyword(String),
    /// A string value.
    String(String),
    /// A numeric value without units.
    Number(f64),
    /// A percentage.
    Percentage(f64),
    /// A URL reference.
    Url(String),
    /// Multiple values (e.g. for shorthand properties).
    List(Vec<CssValue>),
    /// A calc() expression.
    Calc(CalcExpr),
    /// The "auto" keyword.
    Auto,
    /// The "inherit" keyword.
    Inherit,
    /// The "initial" keyword.
    Initial,
    /// No value set.
    #[default]
    None,
}

impl CssValue {
    /// Try to interpret this value as a length.
    pub fn as_length(&self) -> Option<Length> {
        match self {
            CssValue::Length(l) => Some(*l),
            CssValue::Number(n) if *n == 0.0 => Some(Length::zero()),
            CssValue::Percentage(p) => Some(Length::percent(*p)),
            _ => None,
        }
    }

    /// Evaluate this value to pixels given layout context.
    /// Handles `Length`, `Percentage`, `Number(0)`, and `Calc` variants.
    pub fn as_px(&self, parent_size: f64, font_size: f64, root_font_size: f64) -> Option<f64> {
        match self {
            CssValue::Length(l) => Some(l.to_px(parent_size, font_size, root_font_size)),
            CssValue::Number(n) if *n == 0.0 => Some(0.0),
            CssValue::Percentage(p) => Some(p / 100.0 * parent_size),
            CssValue::Calc(expr) => Some(expr.to_px(parent_size, font_size, root_font_size)),
            _ => None,
        }
    }

    /// Try to interpret this value as a color.
    pub fn as_color(&self) -> Option<Color> {
        match self {
            CssValue::Color(c) => Some(*c),
            CssValue::Keyword(s) => Color::parse(s),
            _ => None,
        }
    }

    /// Parse a CSS value from a string token.
    pub fn parse(s: &str) -> Self {
        let s = s.trim();

        if s.is_empty() {
            return CssValue::None;
        }

        // calc() expressions
        if s.starts_with("calc(") && s.ends_with(')') {
            let inner = &s[5..s.len() - 1];
            if let Some(expr) = CalcExpr::parse(inner) {
                return CssValue::Calc(expr);
            }
        }

        // Check for keywords
        match s {
            "auto" => return CssValue::Auto,
            "inherit" => return CssValue::Inherit,
            "initial" => return CssValue::Initial,
            "none" => return CssValue::None,
            _ => {}
        }

        // Try color
        if s.starts_with('#') || s.starts_with("rgb") {
            if let Some(color) = Color::parse(s) {
                return CssValue::Color(color);
            }
        }

        // Named colors
        if let Some(color) = Color::parse(s) {
            if !["auto", "inherit", "initial", "none"].contains(&s) {
                return CssValue::Color(color);
            }
        }

        // Try length with unit
        if let Some(length) = Self::try_parse_length(s) {
            return CssValue::Length(length);
        }

        // Try percentage
        if let Some(pct) = s.strip_suffix('%') {
            if let Ok(val) = pct.parse::<f64>() {
                return CssValue::Percentage(val);
            }
        }

        // Try plain number
        if let Ok(num) = s.parse::<f64>() {
            return CssValue::Number(num);
        }

        // URL
        if s.starts_with("url(") {
            let inner = s
                .trim_start_matches("url(")
                .trim_end_matches(')')
                .trim_matches(|c| c == '\'' || c == '"');
            return CssValue::Url(inner.to_string());
        }

        // Default to keyword
        CssValue::Keyword(s.to_string())
    }

    fn try_parse_length(s: &str) -> Option<Length> {
        let units = [
            ("rem", LengthUnit::Rem),
            ("em", LengthUnit::Em),
            ("px", LengthUnit::Px),
            ("pt", LengthUnit::Pt),
            ("mm", LengthUnit::Mm),
            ("cm", LengthUnit::Cm),
            ("in", LengthUnit::In),
            ("vw", LengthUnit::Vw),
            ("vh", LengthUnit::Vh),
        ];

        for (suffix, unit) in &units {
            if let Some(num_str) = s.strip_suffix(suffix) {
                if let Ok(val) = num_str.parse::<f64>() {
                    return Some(Length::new(val, *unit));
                }
            }
        }
        None
    }
}

impl fmt::Display for CssValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CssValue::Length(l) => write!(f, "{}", l),
            CssValue::Color(c) => write!(f, "{}", c),
            CssValue::Keyword(s) => write!(f, "{}", s),
            CssValue::String(s) => write!(f, "\"{}\"", s),
            CssValue::Number(n) => write!(f, "{}", n),
            CssValue::Percentage(p) => write!(f, "{}%", p),
            CssValue::Url(u) => write!(f, "url(\"{}\")", u),
            CssValue::List(vals) => {
                let strs: Vec<String> = vals.iter().map(|v| v.to_string()).collect();
                write!(f, "{}", strs.join(" "))
            }
            CssValue::Calc(_) => write!(f, "calc(...)"),
            CssValue::Auto => write!(f, "auto"),
            CssValue::Inherit => write!(f, "inherit"),
            CssValue::Initial => write!(f, "initial"),
            CssValue::None => write!(f, "none"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_parse_hex() {
        let c = Color::parse("#ff0000").unwrap();
        assert_eq!(c.r, 255);
        assert_eq!(c.g, 0);
        assert_eq!(c.b, 0);
    }

    #[test]
    fn test_color_parse_named() {
        let c = Color::parse("red").unwrap();
        assert_eq!(c.r, 255);
    }

    #[test]
    fn test_length_to_px() {
        let l = Length::pt(12.0);
        let px = l.to_px(0.0, 16.0, 16.0);
        assert!((px - 16.0).abs() < 0.1);
    }

    #[test]
    fn test_css_value_parse() {
        assert!(matches!(CssValue::parse("auto"), CssValue::Auto));
        assert!(matches!(CssValue::parse("16px"), CssValue::Length(_)));
        assert!(matches!(CssValue::parse("#ff0000"), CssValue::Color(_)));
    }

    #[test]
    fn test_calc_subtract() {
        let val = CssValue::parse("calc(100% - 20px)");
        assert!(matches!(val, CssValue::Calc(_)));
        if let CssValue::Calc(expr) = val {
            // 100% of 500px = 500, minus 20px = 480
            let px = expr.to_px(500.0, 16.0, 16.0);
            assert!((px - 480.0).abs() < 0.001);
        }
    }

    #[test]
    fn test_calc_add() {
        let val = CssValue::parse("calc(50% + 10px)");
        if let CssValue::Calc(expr) = val {
            let px = expr.to_px(200.0, 16.0, 16.0);
            assert!((px - 110.0).abs() < 0.001);
        } else {
            panic!("expected Calc");
        }
    }

    #[test]
    fn test_calc_multiply() {
        let val = CssValue::parse("calc(2 * 16px)");
        if let CssValue::Calc(expr) = val {
            let px = expr.to_px(0.0, 16.0, 16.0);
            assert!((px - 32.0).abs() < 0.001);
        } else {
            panic!("expected Calc");
        }
    }

    #[test]
    fn test_calc_divide() {
        let val = CssValue::parse("calc(100px / 4)");
        if let CssValue::Calc(expr) = val {
            let px = expr.to_px(0.0, 16.0, 16.0);
            assert!((px - 25.0).abs() < 0.001);
        } else {
            panic!("expected Calc");
        }
    }

    #[test]
    fn test_calc_em_based() {
        let val = CssValue::parse("calc(1.5em + 2px)");
        if let CssValue::Calc(expr) = val {
            // 1.5 * 16 + 2 = 24 + 2 = 26
            let px = expr.to_px(0.0, 16.0, 16.0);
            assert!((px - 26.0).abs() < 0.001);
        } else {
            panic!("expected Calc");
        }
    }

    #[test]
    fn test_as_px_handles_calc() {
        let val = CssValue::parse("calc(100% - 20px)");
        let result = val.as_px(500.0, 16.0, 16.0);
        assert!(result.is_some());
        assert!((result.unwrap() - 480.0).abs() < 0.001);
    }
}
