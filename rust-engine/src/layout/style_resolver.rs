//! Style resolver — maps DOM + CSS into computed styles for each node.

use std::collections::HashMap;

use crate::css::properties::{ComputedStyle, CssProperty};
use crate::css::selector::Specificity;
use crate::css::stylesheet::{Declaration, Stylesheet};
use crate::css::values::CssValue;
use crate::css::CssParser;
use crate::html::dom::ElementData;

/// Resolves CSS styles for DOM nodes.
pub struct StyleResolver<'a> {
    stylesheets: Vec<&'a Stylesheet>,
    /// Merged CSS custom properties (--name → value) from all stylesheets.
    custom_properties: HashMap<String, String>,
}

impl<'a> StyleResolver<'a> {
    pub fn new(stylesheets: Vec<&'a Stylesheet>) -> Self {
        let mut custom_properties = HashMap::new();
        for sheet in &stylesheets {
            custom_properties.extend(sheet.custom_properties.clone());
        }
        Self {
            stylesheets,
            custom_properties,
        }
    }

    /// Resolve `var(--name)` references in a raw value string using stored custom properties.
    fn resolve_vars(&self, value: &str) -> String {
        let mut result = value.to_string();
        // Iteratively resolve var() references (handles nested or chained vars)
        for _ in 0..10 {
            if !result.contains("var(") {
                break;
            }
            let snapshot = result.clone();
            // Find and replace each var(--name) or var(--name, fallback)
            let mut new_result = String::new();
            let mut remaining = snapshot.as_str();
            while let Some(start) = remaining.find("var(") {
                new_result.push_str(&remaining[..start]);
                remaining = &remaining[start + 4..]; // skip "var("
                                                     // Find the closing paren (naive: first ')')
                if let Some(end) = remaining.find(')') {
                    let arg = &remaining[..end];
                    remaining = &remaining[end + 1..];
                    let parts: Vec<&str> = arg.splitn(2, ',').collect();
                    let var_name = parts[0].trim();
                    if let Some(val) = self.custom_properties.get(var_name) {
                        new_result.push_str(val.trim());
                    } else if parts.len() > 1 {
                        // Use fallback
                        new_result.push_str(parts[1].trim());
                    }
                    // else: var() resolves to nothing (invalid)
                } else {
                    // Unclosed var() — leave as-is
                    new_result.push_str("var(");
                }
            }
            new_result.push_str(remaining);
            result = new_result;
        }
        result
    }

    /// Compute the style for a given element, given its ancestors.
    pub fn compute_style(
        &self,
        element: &ElementData,
        parent_style: &ComputedStyle,
        ancestors: &[&ElementData],
        siblings: &[&ElementData],
        following_siblings: &[&ElementData],
    ) -> ComputedStyle {
        let mut style = ComputedStyle::new();

        // 1. Apply inherited properties from parent
        style.inherit_from(parent_style);

        // 2. Collect all matching rules with specificity
        let mut matched: Vec<(Specificity, Declaration)> = Vec::new();

        for stylesheet in &self.stylesheets {
            for rule in &stylesheet.rules {
                for selector in &rule.selectors {
                    if selector.matches(element, ancestors, siblings, following_siblings) {
                        let specificity = selector.specificity();
                        for decl in &rule.declarations {
                            // Resolve var() references in the declaration value
                            let resolved_value = if decl.value.to_string().contains("var(") {
                                let raw = decl.value.to_string();
                                let resolved = self.resolve_vars(&raw);
                                CssValue::parse(&resolved)
                            } else {
                                decl.value.clone()
                            };
                            matched.push((
                                specificity,
                                Declaration {
                                    property: decl.property.clone(),
                                    value: resolved_value,
                                    important: decl.important,
                                },
                            ));
                        }
                    }
                }
            }
        }

        // 3. Sort by specificity (stable sort preserves source order for equal specificity)
        matched.sort_by_key(|(spec, _)| *spec);

        // 4. Apply declarations in specificity order
        for (_, decl) in &matched {
            if !decl.important {
                style.set(decl.property.clone(), decl.value.clone());
            }
        }

        // 5. Apply !important declarations (override everything)
        for (_, decl) in &matched {
            if decl.important {
                style.set(decl.property.clone(), decl.value.clone());
            }
        }

        // 6. Apply inline styles (highest specificity for non-important)
        if let Some(inline) = element.get_attr("style") {
            // Resolve vars in inline styles too
            let resolved_inline = self.resolve_vars(inline);
            if let Ok(inline_decls) = CssParser::parse_inline(&resolved_inline) {
                for decl in inline_decls {
                    style.set(decl.property, decl.value);
                }
            }
        }

        style
    }

    /// Apply default display values based on tag name.
    pub fn apply_tag_defaults(tag_name: &str, style: &mut ComputedStyle) {
        match tag_name {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                if style.get(&CssProperty::FontWeight).is_none() {
                    style.set(
                        CssProperty::FontWeight,
                        CssValue::Keyword("bold".to_string()),
                    );
                }
            }
            "b" | "strong" => {
                if style.get(&CssProperty::FontWeight).is_none() {
                    style.set(
                        CssProperty::FontWeight,
                        CssValue::Keyword("bold".to_string()),
                    );
                }
            }
            "i" | "em" => {
                if style.get(&CssProperty::FontStyle).is_none() {
                    style.set(
                        CssProperty::FontStyle,
                        CssValue::Keyword("italic".to_string()),
                    );
                }
            }
            "a" => {
                if style.get(&CssProperty::Color).is_none() {
                    style.set(
                        CssProperty::Color,
                        CssValue::Color(crate::css::values::Color::rgb(0, 0, 238)),
                    );
                }
                if style.get(&CssProperty::TextDecoration).is_none() {
                    style.set(
                        CssProperty::TextDecoration,
                        CssValue::Keyword("underline".to_string()),
                    );
                }
            }
            "code" | "pre" => {
                if style.get(&CssProperty::FontFamily).is_none() {
                    style.set(
                        CssProperty::FontFamily,
                        CssValue::Keyword("monospace".to_string()),
                    );
                }
            }
            _ => {}
        }
    }
}
