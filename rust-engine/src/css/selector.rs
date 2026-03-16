//! CSS selector matching and specificity.

use std::ops::Add;

use crate::html::dom::ElementData;

/// Represents the pattern for nth-child/nth-of-type selectors.
#[derive(Debug, Clone)]
pub enum NthPattern {
    /// nth-child(even)
    Even,
    /// nth-child(odd)
    Odd,
    /// nth-child(3) — exact 1-based position
    Exact(i32),
    /// nth-child(2n+1), nth-child(-n+3), etc.
    AnPlusB(i32, i32),
}

impl NthPattern {
    /// Parse an nth expression: "odd", "even", "3", "2n", "2n+1", "-n+4"
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s == "even" {
            return Some(NthPattern::Even);
        }
        if s == "odd" {
            return Some(NthPattern::Odd);
        }

        // Try plain integer first
        if let Ok(n) = s.parse::<i32>() {
            return Some(NthPattern::Exact(n));
        }

        // Parse "An+B" / "An" / "+n+B" / "-n+B" / "n+B" / "n"
        // Find the 'n' character
        if let Some(n_pos) = s.to_lowercase().find('n') {
            let a_str = &s[..n_pos];
            let b_str = &s[n_pos + 1..];

            let a: i32 = match a_str.trim() {
                "" | "+" => 1,
                "-" => -1,
                other => other.trim().parse().ok()?,
            };

            let b: i32 = if b_str.trim().is_empty() {
                0
            } else {
                b_str.trim().parse().ok()?
            };

            Some(NthPattern::AnPlusB(a, b))
        } else {
            None
        }
    }

    /// Check whether the given 1-based position matches this pattern.
    pub fn matches_position(&self, pos: i32) -> bool {
        match self {
            NthPattern::Even => pos % 2 == 0,
            NthPattern::Odd => pos % 2 == 1,
            NthPattern::Exact(k) => pos == *k,
            NthPattern::AnPlusB(a, b) => {
                if *a == 0 {
                    pos == *b
                } else {
                    let n = pos - b;
                    n % a == 0 && n / a >= 0
                }
            }
        }
    }
}

/// A CSS selector.
#[derive(Debug, Clone)]
pub enum Selector {
    /// Universal selector `*`
    Universal,
    /// Type/tag selector (e.g., `div`, `p`)
    Type(String),
    /// Class selector (e.g., `.container`)
    Class(String),
    /// ID selector (e.g., `#main`)
    Id(String),
    /// Attribute selector (e.g., `[href]`)
    Attribute(String, Option<String>),
    /// Pseudo-class (e.g., `:first-child`)
    PseudoClass(String),
    /// Pseudo-element (e.g., `::before`)
    PseudoElement(String),
    /// Compound selector (e.g., `div.container#main`)
    Compound(Vec<Selector>),
    /// Descendant combinator (e.g., `div p`)
    Descendant(Box<Selector>, Box<Selector>),
    /// Child combinator (e.g., `div > p`)
    Child(Box<Selector>, Box<Selector>),
    /// Adjacent sibling combinator (e.g., `h1 + p`)
    AdjacentSibling(Box<Selector>, Box<Selector>),
    /// General sibling combinator (e.g., `h1 ~ p`)
    GeneralSibling(Box<Selector>, Box<Selector>),
    /// `:nth-child(An+B)`
    NthChild(NthPattern),
    /// `:nth-of-type(An+B)`
    NthOfType(NthPattern),
    /// `:nth-last-child(An+B)` — counts from end
    NthLastChild(NthPattern),
    /// `:not(selector)`
    Not(Box<Selector>),
}

/// CSS selector specificity (a, b, c).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Specificity(pub u32, pub u32, pub u32);

impl Specificity {
    pub fn new(a: u32, b: u32, c: u32) -> Self {
        Self(a, b, c)
    }

    pub fn zero() -> Self {
        Self(0, 0, 0)
    }
}

impl std::ops::Add for Specificity {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0, self.1 + other.1, self.2 + other.2)
    }
}

impl Selector {
    /// Calculate the specificity of this selector.
    pub fn specificity(&self) -> Specificity {
        match self {
            Selector::Universal => Specificity(0, 0, 0),
            Selector::Type(_) => Specificity(0, 0, 1),
            Selector::Class(_) => Specificity(0, 1, 0),
            Selector::Id(_) => Specificity(1, 0, 0),
            Selector::Attribute(_, _) => Specificity(0, 1, 0),
            Selector::PseudoClass(_) => Specificity(0, 1, 0),
            Selector::PseudoElement(_) => Specificity(0, 0, 1),
            Selector::NthChild(_) | Selector::NthOfType(_) | Selector::NthLastChild(_) => {
                Specificity(0, 1, 0)
            }
            // :not() contributes the specificity of its argument (CSS Selectors 4)
            Selector::Not(inner) => inner.specificity(),
            Selector::Compound(parts) => parts
                .iter()
                .fold(Specificity::zero(), |acc, s| acc.add(s.specificity())),
            Selector::Descendant(a, b)
            | Selector::Child(a, b)
            | Selector::AdjacentSibling(a, b)
            | Selector::GeneralSibling(a, b) => a.specificity().add(b.specificity()),
        }
    }

    /// Check if this selector matches the given element.
    ///
    /// - `ancestors`: ancestor elements from root to direct parent.
    /// - `siblings`: preceding element siblings.
    /// - `following_siblings`: following element siblings (needed for :last-child etc.)
    pub fn matches(
        &self,
        element: &ElementData,
        ancestors: &[&ElementData],
        siblings: &[&ElementData],
        following_siblings: &[&ElementData],
    ) -> bool {
        match self {
            Selector::Universal => true,
            Selector::Type(tag) => element.tag_name == *tag,
            Selector::Class(class) => element.has_class(class),
            Selector::Id(id) => element.id() == Some(id.as_str()),
            Selector::Attribute(name, value) => match value {
                Some(val) => element.get_attr(name) == Some(val.as_str()),
                None => element.get_attr(name).is_some(),
            },
            Selector::PseudoClass(pseudo) => match pseudo.as_str() {
                "first-child" => siblings.is_empty(),
                "last-child" => following_siblings.is_empty(),
                "only-child" => siblings.is_empty() && following_siblings.is_empty(),
                "first-of-type" => !siblings.iter().any(|s| s.tag_name == element.tag_name),
                "last-of-type" => !following_siblings
                    .iter()
                    .any(|s| s.tag_name == element.tag_name),
                "only-of-type" => {
                    !siblings.iter().any(|s| s.tag_name == element.tag_name)
                        && !following_siblings
                            .iter()
                            .any(|s| s.tag_name == element.tag_name)
                }
                _ => false,
            },
            Selector::NthChild(pattern) => {
                // 1-based position among all preceding + self
                let pos = siblings.len() as i32 + 1;
                pattern.matches_position(pos)
            }
            Selector::NthOfType(pattern) => {
                // 1-based position counting only same-tag preceding siblings
                let same_tag_count = siblings
                    .iter()
                    .filter(|s| s.tag_name == element.tag_name)
                    .count() as i32
                    + 1;
                pattern.matches_position(same_tag_count)
            }
            Selector::NthLastChild(pattern) => {
                // 1-based position counting from end
                let pos_from_end = following_siblings.len() as i32 + 1;
                pattern.matches_position(pos_from_end)
            }
            Selector::Not(inner) => {
                !inner.matches(element, ancestors, siblings, following_siblings)
            }
            Selector::PseudoElement(_) => false, // Handled separately
            Selector::Compound(parts) => parts
                .iter()
                .all(|s| s.matches(element, ancestors, siblings, following_siblings)),
            Selector::Descendant(ancestor_sel, self_sel) => {
                if !self_sel.matches(element, ancestors, siblings, following_siblings) {
                    return false;
                }
                ancestors
                    .iter()
                    .any(|anc| ancestor_sel.matches(anc, &[], &[], &[]))
            }
            Selector::Child(parent_sel, self_sel) => {
                if !self_sel.matches(element, ancestors, siblings, following_siblings) {
                    return false;
                }
                ancestors
                    .last()
                    .map(|parent| parent_sel.matches(parent, &[], &[], &[]))
                    .unwrap_or(false)
            }
            Selector::AdjacentSibling(prev_sel, self_sel) => {
                if !self_sel.matches(element, ancestors, siblings, following_siblings) {
                    return false;
                }
                siblings
                    .last()
                    .map(|prev| prev_sel.matches(prev, &[], &[], &[]))
                    .unwrap_or(false)
            }
            Selector::GeneralSibling(sib_sel, self_sel) => {
                if !self_sel.matches(element, ancestors, siblings, following_siblings) {
                    return false;
                }
                siblings
                    .iter()
                    .any(|sib| sib_sel.matches(sib, &[], &[], &[]))
            }
        }
    }

    /// Parse a simple selector string.
    pub fn parse(input: &str) -> Option<Self> {
        let input = input.trim();
        if input.is_empty() {
            return None;
        }

        // Handle descendant combinators (space-separated)
        // We split carefully to avoid breaking "An+B" expressions inside parens
        let parts = Self::split_respecting_parens(input);
        if parts.len() > 1 {
            let mut i = 0;
            let mut result: Option<Selector> = None;

            while i < parts.len() {
                let part = parts[i].as_str();

                if part == ">" && i + 1 < parts.len() {
                    let right = Self::parse_simple(&parts[i + 1])?;
                    result = Some(Selector::Child(Box::new(result?), Box::new(right)));
                    i += 2;
                } else if part == "+" && i + 1 < parts.len() {
                    let right = Self::parse_simple(&parts[i + 1])?;
                    result = Some(Selector::AdjacentSibling(
                        Box::new(result?),
                        Box::new(right),
                    ));
                    i += 2;
                } else if part == "~" && i + 1 < parts.len() {
                    let right = Self::parse_simple(&parts[i + 1])?;
                    result = Some(Selector::GeneralSibling(Box::new(result?), Box::new(right)));
                    i += 2;
                } else {
                    let sel = Self::parse_simple(part)?;
                    result = Some(match result {
                        Some(left) => Selector::Descendant(Box::new(left), Box::new(sel)),
                        None => sel,
                    });
                    i += 1;
                }
            }

            return result;
        }

        Self::parse_simple(input)
    }

    /// Split a selector by whitespace, but not inside parentheses.
    fn split_respecting_parens(input: &str) -> Vec<String> {
        let mut result = Vec::new();
        let mut current = String::new();
        let mut depth: usize = 0;

        for ch in input.chars() {
            match ch {
                '(' => {
                    depth += 1;
                    current.push(ch);
                }
                ')' => {
                    depth = depth.saturating_sub(1);
                    current.push(ch);
                }
                ' ' | '\t' | '\n' if depth == 0 => {
                    let trimmed = current.trim().to_string();
                    if !trimmed.is_empty() {
                        result.push(trimmed);
                    }
                    current.clear();
                }
                _ => {
                    current.push(ch);
                }
            }
        }
        let trimmed = current.trim().to_string();
        if !trimmed.is_empty() {
            result.push(trimmed);
        }
        result
    }

    /// Parse a simple (non-combinator) selector.
    fn parse_simple(input: &str) -> Option<Self> {
        if input == "*" {
            return Some(Selector::Universal);
        }

        // Check for compound selectors — split on `.`, `#`, `[`, `:` but
        // respect parentheses so :nth-child(2n+1) is not broken up.
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut chars = input.chars().peekable();
        let mut paren_depth: usize = 0;

        while let Some(&ch) = chars.peek() {
            match ch {
                '(' => {
                    paren_depth += 1;
                    current.push(chars.next().unwrap());
                }
                ')' => {
                    paren_depth = paren_depth.saturating_sub(1);
                    current.push(chars.next().unwrap());
                }
                '.' | '#' if !current.is_empty() && paren_depth == 0 => {
                    parts.push(Self::parse_single_simple(&current)?);
                    current.clear();
                    current.push(chars.next().unwrap());
                }
                '[' if paren_depth == 0 => {
                    if !current.is_empty() {
                        parts.push(Self::parse_single_simple(&current)?);
                        current.clear();
                    }
                    // Read until matching ]
                    current.push(chars.next().unwrap());
                    while let Some(&c) = chars.peek() {
                        current.push(chars.next().unwrap());
                        if c == ']' {
                            break;
                        }
                    }
                    parts.push(Self::parse_single_simple(&current)?);
                    current.clear();
                }
                ':' if paren_depth == 0 => {
                    if !current.is_empty() {
                        parts.push(Self::parse_single_simple(&current)?);
                        current.clear();
                    }
                    current.push(chars.next().unwrap());
                }
                _ => {
                    current.push(chars.next().unwrap());
                }
            }
        }

        if !current.is_empty() {
            parts.push(Self::parse_single_simple(&current)?);
        }

        match parts.len() {
            0 => None,
            1 => Some(parts.into_iter().next().unwrap()),
            _ => Some(Selector::Compound(parts)),
        }
    }

    fn parse_single_simple(input: &str) -> Option<Self> {
        if let Some(stripped) = input.strip_prefix('#') {
            Some(Selector::Id(stripped.to_string()))
        } else if let Some(stripped) = input.strip_prefix('.') {
            Some(Selector::Class(stripped.to_string()))
        } else if input.starts_with('[') && input.ends_with(']') {
            let inner = &input[1..input.len() - 1];
            if let Some(eq_pos) = inner.find('=') {
                let name = inner[..eq_pos].trim().to_string();
                let value = inner[eq_pos + 1..]
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string();
                Some(Selector::Attribute(name, Some(value)))
            } else {
                Some(Selector::Attribute(inner.to_string(), None))
            }
        } else if let Some(stripped) = input.strip_prefix("::") {
            Some(Selector::PseudoElement(stripped.to_string()))
        } else if let Some(pseudo) = input.strip_prefix(':') {
            // Detect functional pseudo-classes: :nth-child(...), :not(...), etc.
            if let Some(paren_pos) = pseudo.find('(') {
                if pseudo.ends_with(')') {
                    let name = &pseudo[..paren_pos];
                    let arg = &pseudo[paren_pos + 1..pseudo.len() - 1];
                    match name {
                        "nth-child" => NthPattern::parse(arg).map(Selector::NthChild),
                        "nth-of-type" => NthPattern::parse(arg).map(Selector::NthOfType),
                        "nth-last-child" => NthPattern::parse(arg).map(Selector::NthLastChild),
                        "not" => Selector::parse(arg).map(|inner| Selector::Not(Box::new(inner))),
                        _ => Some(Selector::PseudoClass(pseudo.to_string())),
                    }
                } else {
                    Some(Selector::PseudoClass(pseudo.to_string()))
                }
            } else {
                Some(Selector::PseudoClass(pseudo.to_string()))
            }
        } else {
            Some(Selector::Type(input.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_element(tag: &str, class: &str, id: &str) -> ElementData {
        let mut attrs = HashMap::new();
        if !class.is_empty() {
            attrs.insert("class".to_string(), class.to_string());
        }
        if !id.is_empty() {
            attrs.insert("id".to_string(), id.to_string());
        }
        ElementData {
            tag_name: tag.to_string(),
            attributes: attrs,
        }
    }

    #[test]
    fn test_type_selector() {
        let sel = Selector::parse("div").unwrap();
        let elem = make_element("div", "", "");
        assert!(sel.matches(&elem, &[], &[], &[]));
    }

    #[test]
    fn test_class_selector() {
        let sel = Selector::parse(".container").unwrap();
        let elem = make_element("div", "container", "");
        assert!(sel.matches(&elem, &[], &[], &[]));
    }

    #[test]
    fn test_id_selector() {
        let sel = Selector::parse("#main").unwrap();
        let elem = make_element("div", "", "main");
        assert!(sel.matches(&elem, &[], &[], &[]));
    }

    #[test]
    fn test_first_child() {
        let sel = Selector::parse(":first-child").unwrap();
        let elem = make_element("p", "", "");
        assert!(sel.matches(&elem, &[], &[], &[]));
        let prev = make_element("p", "", "");
        assert!(!sel.matches(&elem, &[], &[&prev], &[]));
    }

    #[test]
    fn test_last_child() {
        let sel = Selector::parse(":last-child").unwrap();
        let elem = make_element("p", "", "");
        // No following siblings → last child
        assert!(sel.matches(&elem, &[], &[], &[]));
        // Has a following sibling → not last child
        let next = make_element("p", "", "");
        assert!(!sel.matches(&elem, &[], &[], &[&next]));
    }

    #[test]
    fn test_nth_child_odd() {
        let sel = Selector::parse(":nth-child(odd)").unwrap();
        let elem = make_element("li", "", "");
        // Position 1 (no preceding siblings) — odd → matches
        assert!(sel.matches(&elem, &[], &[], &[]));
        let prev = make_element("li", "", "");
        // Position 2 — even → no match
        assert!(!sel.matches(&elem, &[], &[&prev], &[]));
    }

    #[test]
    fn test_nth_child_2n_plus_1() {
        let pattern = NthPattern::parse("2n+1").unwrap();
        assert!(pattern.matches_position(1));
        assert!(!pattern.matches_position(2));
        assert!(pattern.matches_position(3));
    }

    #[test]
    fn test_not_selector() {
        let sel = Selector::parse(":not(.active)").unwrap();
        let plain = make_element("div", "", "");
        let active = make_element("div", "active", "");
        assert!(sel.matches(&plain, &[], &[], &[]));
        assert!(!sel.matches(&active, &[], &[], &[]));
    }

    #[test]
    fn test_specificity_ordering() {
        let a = Specificity::new(0, 0, 1); // type
        let b = Specificity::new(0, 1, 0); // class
        let c = Specificity::new(1, 0, 0); // id
        assert!(a < b);
        assert!(b < c);
    }
}
