//! `calc()` parsing (CSS Values 4 §10).
//!
//! An expression of lengths, percentages and numbers combined with `+ - * /`
//! and parentheses reduces to a linear combination of `%`, absolute length,
//! `em` and `rem` ([`CalcLength`]). Multiplication and division require one
//! operand to be a plain number, as in CSS, so the result stays linear.
//! Nested `calc()` is treated as parentheses. `min()`, `max()`, `clamp()` and
//! viewport units are not supported: such values are rejected (`None`) and
//! the declaration is ignored, like an invalid value in a browser.

use ferropdf_core::CalcLength;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Value {
    /// Plain number (only meaningful when `is_number`).
    number: f32,
    is_number: bool,
    len: CalcLength,
}

impl Value {
    fn number(n: f32) -> Self {
        Value {
            number: n,
            is_number: true,
            len: CalcLength::default(),
        }
    }
    fn length(len: CalcLength) -> Self {
        Value {
            number: 0.0,
            is_number: false,
            len,
        }
    }
    fn scale(self, k: f32) -> Self {
        if self.is_number {
            return Value::number(self.number * k);
        }
        let l = self.len;
        Value::length(CalcLength {
            percent: l.percent * k,
            pt: l.pt * k,
            em: l.em * k,
            rem: l.rem * k,
        })
    }
    fn add(self, other: Self, sign: f32) -> Option<Self> {
        match (self.is_number, other.is_number) {
            (true, true) => Some(Value::number(self.number + sign * other.number)),
            (false, false) => {
                let (a, b) = (self.len, other.len);
                Some(Value::length(CalcLength {
                    percent: a.percent + sign * b.percent,
                    pt: a.pt + sign * b.pt,
                    em: a.em + sign * b.em,
                    rem: a.rem + sign * b.rem,
                }))
            }
            // `0` may be written unitless in a sum.
            (true, false) if self.number == 0.0 => Some(other.scale(sign)),
            (false, true) if other.number == 0.0 => Some(self),
            _ => None,
        }
    }
}

/// Parse `calc(...)` (the whole value). Returns `None` if invalid or unsupported.
pub fn parse_calc(input: &str) -> Option<CalcLength> {
    let s = input.trim();
    let body = s
        .get(..5)
        .filter(|p| p.eq_ignore_ascii_case("calc("))
        .and(s.strip_suffix(')'))
        .map(|b| &b[5..])?;
    let mut p = Parser {
        chars: body.chars().collect(),
        pos: 0,
    };
    let v = p.sum()?;
    p.skip_ws();
    if p.pos != p.chars.len() {
        return None;
    }
    if v.is_number {
        // `calc(0)` is a valid length; other bare numbers are not.
        return (v.number == 0.0).then(CalcLength::default);
    }
    Some(v.len)
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn skip_ws(&mut self) {
        while self.chars.get(self.pos).is_some_and(|c| c.is_whitespace()) {
            self.pos += 1;
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.skip_ws();
        self.chars.get(self.pos).copied()
    }

    fn sum(&mut self) -> Option<Value> {
        let mut acc = self.product()?;
        loop {
            match self.peek() {
                Some(c @ ('+' | '-')) => {
                    self.pos += 1;
                    let rhs = self.product()?;
                    acc = acc.add(rhs, if c == '+' { 1.0 } else { -1.0 })?;
                }
                _ => return Some(acc),
            }
        }
    }

    fn product(&mut self) -> Option<Value> {
        let mut acc = self.factor()?;
        loop {
            match self.peek() {
                Some('*') => {
                    self.pos += 1;
                    let rhs = self.factor()?;
                    acc = match (acc.is_number, rhs.is_number) {
                        (true, _) => rhs.scale(acc.number),
                        (false, true) => acc.scale(rhs.number),
                        (false, false) => return None,
                    };
                }
                Some('/') => {
                    self.pos += 1;
                    let rhs = self.factor()?;
                    if !rhs.is_number || rhs.number == 0.0 {
                        return None;
                    }
                    acc = acc.scale(1.0 / rhs.number);
                }
                _ => return Some(acc),
            }
        }
    }

    fn factor(&mut self) -> Option<Value> {
        match self.peek()? {
            '(' => {
                self.pos += 1;
                let v = self.sum()?;
                (self.peek()? == ')').then_some(())?;
                self.pos += 1;
                Some(v)
            }
            '-' if !self.next_is_number_start(1) => None,
            c if c.is_ascii_digit() || c == '.' || c == '-' || c == '+' => self.dimension(),
            c if c.is_ascii_alphabetic() => {
                let start = self.pos;
                while self
                    .chars
                    .get(self.pos)
                    .is_some_and(|c| c.is_ascii_alphabetic())
                {
                    self.pos += 1;
                }
                let name: String = self.chars[start..self.pos].iter().collect();
                if name.eq_ignore_ascii_case("calc") && self.chars.get(self.pos) == Some(&'(') {
                    self.factor()
                } else {
                    None // min(), max(), clamp(), env(), unknown identifiers
                }
            }
            _ => None,
        }
    }

    fn next_is_number_start(&self, offset: usize) -> bool {
        self.chars
            .get(self.pos + offset)
            .is_some_and(|c| c.is_ascii_digit() || *c == '.')
    }

    fn dimension(&mut self) -> Option<Value> {
        let start = self.pos;
        if matches!(self.chars.get(self.pos), Some('+' | '-')) {
            self.pos += 1;
        }
        while self
            .chars
            .get(self.pos)
            .is_some_and(|c| c.is_ascii_digit() || *c == '.')
        {
            self.pos += 1;
        }
        // Exponent (`1e3`), only when followed by digits.
        if matches!(self.chars.get(self.pos), Some('e' | 'E'))
            && self
                .chars
                .get(self.pos + 1)
                .is_some_and(|c| c.is_ascii_digit() || *c == '-' || *c == '+')
        {
            self.pos += 2;
            while self.chars.get(self.pos).is_some_and(|c| c.is_ascii_digit()) {
                self.pos += 1;
            }
        }
        let num: f32 = self.chars[start..self.pos]
            .iter()
            .collect::<String>()
            .parse()
            .ok()?;
        let unit_start = self.pos;
        while self
            .chars
            .get(self.pos)
            .is_some_and(|c| c.is_ascii_alphabetic() || *c == '%')
        {
            self.pos += 1;
        }
        let unit: String = self.chars[unit_start..self.pos]
            .iter()
            .collect::<String>()
            .to_ascii_lowercase();
        let len = |f: fn(&mut CalcLength, f32)| {
            let mut l = CalcLength::default();
            f(&mut l, num);
            Some(Value::length(l))
        };
        match unit.as_str() {
            "" => Some(Value::number(num)),
            "%" => len(|l, n| l.percent = n),
            "px" => len(|l, n| l.pt = n * 0.75),
            "pt" => len(|l, n| l.pt = n),
            "pc" => len(|l, n| l.pt = n * 12.0),
            "mm" => len(|l, n| l.pt = n * 2.834_646),
            "cm" => len(|l, n| l.pt = n * 28.346_46),
            "in" => len(|l, n| l.pt = n * 72.0),
            "q" => len(|l, n| l.pt = n * 0.708_661_5),
            "em" => len(|l, n| l.em = n),
            "rem" => len(|l, n| l.rem = n),
            _ => None, // vw, vh, ch, ex, … not supported
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(s: &str) -> CalcLength {
        parse_calc(s).unwrap_or_else(|| panic!("failed: {s}"))
    }

    #[test]
    fn absolute_sums() {
        assert_eq!(c("calc(100px + 100px)").pt, 150.0);
        assert_eq!(c("calc(1in - 36pt)").pt, 36.0);
    }

    #[test]
    fn mixed_percent_and_length() {
        let v = c("calc(100% - 100px)");
        assert_eq!((v.percent, v.pt), (100.0, -75.0));
        assert_eq!(v.against(400.0), 325.0);
    }

    #[test]
    fn products_and_parentheses() {
        // Bootstrap gutters after var() substitution.
        assert_eq!(c("calc(1.5rem * .5)").rem, 0.75);
        assert_eq!(c("calc(-.5 * 1.5rem)").rem, -0.75);
        let v = c("calc((100% - 2 * 10px) / 3)");
        assert!((v.percent - 33.333).abs() < 0.01 && (v.pt + 5.0).abs() < 0.01);
        assert_eq!(c("calc(2em + calc(1em / 2))").em, 2.5);
    }

    #[test]
    fn unitless_zero_and_invalid() {
        assert_eq!(c("calc(0)"), CalcLength::default());
        assert_eq!(c("calc(10px + 0)").pt, 7.5);
        assert!(parse_calc("calc(10px * 10px)").is_none());
        assert!(parse_calc("calc(10px + 2)").is_none());
        assert!(parse_calc("calc(10px / 0)").is_none());
        assert!(parse_calc("calc(min(10px, 5%))").is_none());
        assert!(parse_calc("calc(10vw)").is_none());
        assert!(parse_calc("10px").is_none());
    }
}
