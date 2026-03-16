//! Tailwind CSS support.
//!
//! This module provides utilities for working with Tailwind CSS in PDF documents.
//! The recommended approach is to use precompiled Tailwind CSS (run the Tailwind
//! CLI or build tool before rendering), but this module also provides a subset
//! of utility class resolution for common patterns.

use std::collections::HashMap;

use once_cell::sync::Lazy;

use crate::css::properties::CssProperty;
use crate::css::selector::Selector;
use crate::css::stylesheet::{CssRule, Declaration, Stylesheet};
use crate::css::values::{Color, CssValue, Length};

/// Pre-compiled regex for extracting Tailwind class attributes.
static CLASS_ATTR_RE: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r#"class="([^"]*)"#).expect("static regex"));
// ── Static lookup tables — allocated once at first use ────────────────────
// These replace per-call HashMap allocations that were the main hot-path cost
// in resolve_spacing / resolve_colors / resolve_sizing / resolve_borders.

/// Tailwind spacing scale: suffix → pixel value.
static SPACING_PX: Lazy<HashMap<&'static str, f64>> = Lazy::new(|| {
    [
        ("0", 0.0),
        ("0.5", 2.0),
        ("1", 4.0),
        ("1.5", 6.0),
        ("2", 8.0),
        ("2.5", 10.0),
        ("3", 12.0),
        ("3.5", 14.0),
        ("4", 16.0),
        ("5", 20.0),
        ("6", 24.0),
        ("7", 28.0),
        ("8", 32.0),
        ("9", 36.0),
        ("10", 40.0),
        ("11", 44.0),
        ("12", 48.0),
        ("14", 56.0),
        ("16", 64.0),
        ("20", 80.0),
        ("24", 96.0),
        ("28", 112.0),
        ("32", 128.0),
        ("36", 144.0),
        ("40", 160.0),
        ("44", 176.0),
        ("48", 192.0),
        ("52", 208.0),
        ("56", 224.0),
        ("60", 240.0),
        ("64", 256.0),
        ("72", 288.0),
        ("80", 320.0),
        ("96", 384.0),
        ("px", 1.0),
        ("auto", -1.0),
    ]
    .into_iter()
    .collect()
});

/// Tailwind colour palette: name → Color.
static COLOR_MAP: Lazy<HashMap<&'static str, Color>> = Lazy::new(|| {
    [
        ("white", Color::rgb(255, 255, 255)),
        ("black", Color::rgb(0, 0, 0)),
        ("transparent", Color::transparent()),
        ("gray-50", Color::rgb(249, 250, 251)),
        ("gray-100", Color::rgb(243, 244, 246)),
        ("gray-200", Color::rgb(229, 231, 235)),
        ("gray-300", Color::rgb(209, 213, 219)),
        ("gray-400", Color::rgb(156, 163, 175)),
        ("gray-500", Color::rgb(107, 114, 128)),
        ("gray-600", Color::rgb(75, 85, 99)),
        ("gray-700", Color::rgb(55, 65, 81)),
        ("gray-800", Color::rgb(31, 41, 55)),
        ("gray-900", Color::rgb(17, 24, 39)),
        ("red-50", Color::rgb(254, 242, 242)),
        ("red-500", Color::rgb(239, 68, 68)),
        ("red-600", Color::rgb(220, 38, 38)),
        ("red-700", Color::rgb(185, 28, 28)),
        ("blue-50", Color::rgb(239, 246, 255)),
        ("blue-500", Color::rgb(59, 130, 246)),
        ("blue-600", Color::rgb(37, 99, 235)),
        ("blue-700", Color::rgb(29, 78, 216)),
        ("green-50", Color::rgb(240, 253, 244)),
        ("green-500", Color::rgb(34, 197, 94)),
        ("green-600", Color::rgb(22, 163, 74)),
        ("green-700", Color::rgb(21, 128, 61)),
        ("yellow-50", Color::rgb(254, 252, 232)),
        ("yellow-500", Color::rgb(234, 179, 8)),
        ("indigo-50", Color::rgb(238, 242, 255)),
        ("indigo-500", Color::rgb(99, 102, 241)),
        ("indigo-600", Color::rgb(79, 70, 229)),
        ("purple-500", Color::rgb(168, 85, 247)),
        ("pink-500", Color::rgb(236, 72, 153)),
    ]
    .into_iter()
    .collect()
});

/// Tailwind sizing/dimension scale: suffix → CssValue.
static SIZE_MAP: Lazy<HashMap<&'static str, CssValue>> = Lazy::new(|| {
    [
        ("0", CssValue::Length(Length::px(0.0))),
        ("px", CssValue::Length(Length::px(1.0))),
        ("1", CssValue::Length(Length::px(4.0))),
        ("2", CssValue::Length(Length::px(8.0))),
        ("4", CssValue::Length(Length::px(16.0))),
        ("8", CssValue::Length(Length::px(32.0))),
        ("12", CssValue::Length(Length::px(48.0))),
        ("16", CssValue::Length(Length::px(64.0))),
        ("24", CssValue::Length(Length::px(96.0))),
        ("32", CssValue::Length(Length::px(128.0))),
        ("48", CssValue::Length(Length::px(192.0))),
        ("64", CssValue::Length(Length::px(256.0))),
        ("96", CssValue::Length(Length::px(384.0))),
        ("full", CssValue::Percentage(100.0)),
        ("1/2", CssValue::Percentage(50.0)),
        ("1/3", CssValue::Percentage(33.333)),
        ("2/3", CssValue::Percentage(66.667)),
        ("1/4", CssValue::Percentage(25.0)),
        ("3/4", CssValue::Percentage(75.0)),
        ("auto", CssValue::Auto),
    ]
    .into_iter()
    .collect()
});

/// Tailwind border-radius map: class name → pixel radius.
static BORDER_RADIUS_MAP: Lazy<HashMap<&'static str, f64>> = Lazy::new(|| {
    [
        ("rounded-none", 0.0),
        ("rounded-sm", 2.0),
        ("rounded", 4.0),
        ("rounded-md", 6.0),
        ("rounded-lg", 8.0),
        ("rounded-xl", 12.0),
        ("rounded-2xl", 16.0),
        ("rounded-3xl", 24.0),
        ("rounded-full", 9999.0),
    ]
    .into_iter()
    .collect()
});
/// Resolve a subset of Tailwind CSS utility classes to CSS declarations.
///
/// This is NOT a full Tailwind processor — it handles the most common utility
/// classes so that basic Tailwind templates work without a build step.
/// For full Tailwind support, precompile with the Tailwind CLI.
pub struct TailwindResolver;

impl TailwindResolver {
    /// Generate a stylesheet from a list of Tailwind class names found in the HTML.
    pub fn resolve_classes(classes: &[String]) -> Stylesheet {
        let mut stylesheet = Stylesheet::new();

        for class in classes {
            if let Some(declarations) = Self::resolve_class(class) {
                stylesheet.rules.push(CssRule {
                    selectors: vec![Selector::Class(class.clone())],
                    declarations,
                });
            }
        }

        stylesheet
    }

    /// Resolve a single Tailwind utility class to CSS declarations.
    pub fn resolve_class(class: &str) -> Option<Vec<Declaration>> {
        // Spacing: p-*, m-*, px-*, py-*, etc.
        if let Some(decls) = Self::resolve_spacing(class) {
            return Some(decls);
        }

        // Typography
        if let Some(decls) = Self::resolve_typography(class) {
            return Some(decls);
        }

        // Colors
        if let Some(decls) = Self::resolve_colors(class) {
            return Some(decls);
        }

        // Display and layout
        if let Some(decls) = Self::resolve_layout(class) {
            return Some(decls);
        }

        // Sizing
        if let Some(decls) = Self::resolve_sizing(class) {
            return Some(decls);
        }

        // Borders
        if let Some(decls) = Self::resolve_borders(class) {
            return Some(decls);
        }

        None
    }

    /// Resolve spacing utilities (p-*, m-*, px-*, py-*, pt-*, etc.).
    fn resolve_spacing(class: &str) -> Option<Vec<Declaration>> {
        let prefixes = [
            (
                "px-",
                vec![CssProperty::PaddingLeft, CssProperty::PaddingRight],
            ),
            (
                "py-",
                vec![CssProperty::PaddingTop, CssProperty::PaddingBottom],
            ),
            ("pt-", vec![CssProperty::PaddingTop]),
            ("pr-", vec![CssProperty::PaddingRight]),
            ("pb-", vec![CssProperty::PaddingBottom]),
            ("pl-", vec![CssProperty::PaddingLeft]),
            (
                "p-",
                vec![
                    CssProperty::PaddingTop,
                    CssProperty::PaddingRight,
                    CssProperty::PaddingBottom,
                    CssProperty::PaddingLeft,
                ],
            ),
            (
                "mx-",
                vec![CssProperty::MarginLeft, CssProperty::MarginRight],
            ),
            (
                "my-",
                vec![CssProperty::MarginTop, CssProperty::MarginBottom],
            ),
            ("mt-", vec![CssProperty::MarginTop]),
            ("mr-", vec![CssProperty::MarginRight]),
            ("mb-", vec![CssProperty::MarginBottom]),
            ("ml-", vec![CssProperty::MarginLeft]),
            (
                "m-",
                vec![
                    CssProperty::MarginTop,
                    CssProperty::MarginRight,
                    CssProperty::MarginBottom,
                    CssProperty::MarginLeft,
                ],
            ),
            ("gap-", vec![CssProperty::Gap]),
        ];

        for (prefix, props) in &prefixes {
            if let Some(value_str) = class.strip_prefix(prefix) {
                if let Some(&px_value) = SPACING_PX.get(value_str) {
                    let value = if px_value < 0.0 {
                        CssValue::Auto
                    } else {
                        CssValue::Length(Length::px(px_value))
                    };
                    return Some(
                        props
                            .iter()
                            .map(|p| Declaration::new(p.clone(), value.clone()))
                            .collect(),
                    );
                }
            }
        }

        None
    }

    /// Resolve typography utilities.
    fn resolve_typography(class: &str) -> Option<Vec<Declaration>> {
        match class {
            // Font size
            "text-xs" => Some(vec![
                Declaration::new(CssProperty::FontSize, CssValue::Length(Length::px(12.0))),
                Declaration::new(CssProperty::LineHeight, CssValue::Length(Length::px(16.0))),
            ]),
            "text-sm" => Some(vec![
                Declaration::new(CssProperty::FontSize, CssValue::Length(Length::px(14.0))),
                Declaration::new(CssProperty::LineHeight, CssValue::Length(Length::px(20.0))),
            ]),
            "text-base" => Some(vec![
                Declaration::new(CssProperty::FontSize, CssValue::Length(Length::px(16.0))),
                Declaration::new(CssProperty::LineHeight, CssValue::Length(Length::px(24.0))),
            ]),
            "text-lg" => Some(vec![
                Declaration::new(CssProperty::FontSize, CssValue::Length(Length::px(18.0))),
                Declaration::new(CssProperty::LineHeight, CssValue::Length(Length::px(28.0))),
            ]),
            "text-xl" => Some(vec![
                Declaration::new(CssProperty::FontSize, CssValue::Length(Length::px(20.0))),
                Declaration::new(CssProperty::LineHeight, CssValue::Length(Length::px(28.0))),
            ]),
            "text-2xl" => Some(vec![
                Declaration::new(CssProperty::FontSize, CssValue::Length(Length::px(24.0))),
                Declaration::new(CssProperty::LineHeight, CssValue::Length(Length::px(32.0))),
            ]),
            "text-3xl" => Some(vec![
                Declaration::new(CssProperty::FontSize, CssValue::Length(Length::px(30.0))),
                Declaration::new(CssProperty::LineHeight, CssValue::Length(Length::px(36.0))),
            ]),
            "text-4xl" => Some(vec![
                Declaration::new(CssProperty::FontSize, CssValue::Length(Length::px(36.0))),
                Declaration::new(CssProperty::LineHeight, CssValue::Length(Length::px(40.0))),
            ]),

            // Font weight
            "font-thin" => Some(vec![Declaration::new(
                CssProperty::FontWeight,
                CssValue::Number(100.0),
            )]),
            "font-extralight" => Some(vec![Declaration::new(
                CssProperty::FontWeight,
                CssValue::Number(200.0),
            )]),
            "font-light" => Some(vec![Declaration::new(
                CssProperty::FontWeight,
                CssValue::Number(300.0),
            )]),
            "font-normal" => Some(vec![Declaration::new(
                CssProperty::FontWeight,
                CssValue::Number(400.0),
            )]),
            "font-medium" => Some(vec![Declaration::new(
                CssProperty::FontWeight,
                CssValue::Number(500.0),
            )]),
            "font-semibold" => Some(vec![Declaration::new(
                CssProperty::FontWeight,
                CssValue::Number(600.0),
            )]),
            "font-bold" => Some(vec![Declaration::new(
                CssProperty::FontWeight,
                CssValue::Number(700.0),
            )]),
            "font-extrabold" => Some(vec![Declaration::new(
                CssProperty::FontWeight,
                CssValue::Number(800.0),
            )]),
            "font-black" => Some(vec![Declaration::new(
                CssProperty::FontWeight,
                CssValue::Number(900.0),
            )]),

            // Font style
            "italic" => Some(vec![Declaration::new(
                CssProperty::FontStyle,
                CssValue::Keyword("italic".to_string()),
            )]),
            "not-italic" => Some(vec![Declaration::new(
                CssProperty::FontStyle,
                CssValue::Keyword("normal".to_string()),
            )]),

            // Text alignment
            "text-left" => Some(vec![Declaration::new(
                CssProperty::TextAlign,
                CssValue::Keyword("left".to_string()),
            )]),
            "text-center" => Some(vec![Declaration::new(
                CssProperty::TextAlign,
                CssValue::Keyword("center".to_string()),
            )]),
            "text-right" => Some(vec![Declaration::new(
                CssProperty::TextAlign,
                CssValue::Keyword("right".to_string()),
            )]),
            "text-justify" => Some(vec![Declaration::new(
                CssProperty::TextAlign,
                CssValue::Keyword("justify".to_string()),
            )]),

            // Text decoration
            "underline" => Some(vec![Declaration::new(
                CssProperty::TextDecoration,
                CssValue::Keyword("underline".to_string()),
            )]),
            "line-through" => Some(vec![Declaration::new(
                CssProperty::TextDecoration,
                CssValue::Keyword("line-through".to_string()),
            )]),
            "no-underline" => Some(vec![Declaration::new(
                CssProperty::TextDecoration,
                CssValue::Keyword("none".to_string()),
            )]),

            // Text transform
            "uppercase" => Some(vec![Declaration::new(
                CssProperty::TextTransform,
                CssValue::Keyword("uppercase".to_string()),
            )]),
            "lowercase" => Some(vec![Declaration::new(
                CssProperty::TextTransform,
                CssValue::Keyword("lowercase".to_string()),
            )]),
            "capitalize" => Some(vec![Declaration::new(
                CssProperty::TextTransform,
                CssValue::Keyword("capitalize".to_string()),
            )]),
            "normal-case" => Some(vec![Declaration::new(
                CssProperty::TextTransform,
                CssValue::Keyword("none".to_string()),
            )]),

            // Whitespace
            "whitespace-normal" => Some(vec![Declaration::new(
                CssProperty::WhiteSpace,
                CssValue::Keyword("normal".to_string()),
            )]),
            "whitespace-nowrap" => Some(vec![Declaration::new(
                CssProperty::WhiteSpace,
                CssValue::Keyword("nowrap".to_string()),
            )]),
            "whitespace-pre" => Some(vec![Declaration::new(
                CssProperty::WhiteSpace,
                CssValue::Keyword("pre".to_string()),
            )]),

            // Font family
            "font-sans" => Some(vec![Declaration::new(
                CssProperty::FontFamily,
                CssValue::Keyword("sans-serif".to_string()),
            )]),
            "font-serif" => Some(vec![Declaration::new(
                CssProperty::FontFamily,
                CssValue::Keyword("serif".to_string()),
            )]),
            "font-mono" => Some(vec![Declaration::new(
                CssProperty::FontFamily,
                CssValue::Keyword("monospace".to_string()),
            )]),

            _ => None,
        }
    }

    /// Resolve color utilities.
    fn resolve_colors(class: &str) -> Option<Vec<Declaration>> {
        // text-{color}
        if let Some(color_name) = class.strip_prefix("text-") {
            if let Some(color) = COLOR_MAP.get(color_name) {
                return Some(vec![Declaration::new(
                    CssProperty::Color,
                    CssValue::Color(*color),
                )]);
            }
        }

        // bg-{color}
        if let Some(color_name) = class.strip_prefix("bg-") {
            if let Some(color) = COLOR_MAP.get(color_name) {
                return Some(vec![Declaration::new(
                    CssProperty::BackgroundColor,
                    CssValue::Color(*color),
                )]);
            }
        }

        // border-{color}
        if let Some(color_name) = class.strip_prefix("border-") {
            if let Some(color) = COLOR_MAP.get(color_name) {
                return Some(vec![
                    Declaration::new(CssProperty::BorderTopColor, CssValue::Color(*color)),
                    Declaration::new(CssProperty::BorderRightColor, CssValue::Color(*color)),
                    Declaration::new(CssProperty::BorderBottomColor, CssValue::Color(*color)),
                    Declaration::new(CssProperty::BorderLeftColor, CssValue::Color(*color)),
                ]);
            }
        }

        None
    }

    /// Resolve layout utilities.
    fn resolve_layout(class: &str) -> Option<Vec<Declaration>> {
        match class {
            "block" => Some(vec![Declaration::new(
                CssProperty::Display,
                CssValue::Keyword("block".to_string()),
            )]),
            "inline-block" => Some(vec![Declaration::new(
                CssProperty::Display,
                CssValue::Keyword("inline-block".to_string()),
            )]),
            "inline" => Some(vec![Declaration::new(
                CssProperty::Display,
                CssValue::Keyword("inline".to_string()),
            )]),
            "flex" => Some(vec![Declaration::new(
                CssProperty::Display,
                CssValue::Keyword("flex".to_string()),
            )]),
            "inline-flex" => Some(vec![Declaration::new(
                CssProperty::Display,
                CssValue::Keyword("inline-flex".to_string()),
            )]),
            "table" => Some(vec![Declaration::new(
                CssProperty::Display,
                CssValue::Keyword("table".to_string()),
            )]),
            "hidden" => Some(vec![Declaration::new(
                CssProperty::Display,
                CssValue::Keyword("none".to_string()),
            )]),
            "grid" => Some(vec![Declaration::new(
                CssProperty::Display,
                CssValue::Keyword("grid".to_string()),
            )]),

            // Flex direction
            "flex-row" => Some(vec![Declaration::new(
                CssProperty::FlexDirection,
                CssValue::Keyword("row".to_string()),
            )]),
            "flex-row-reverse" => Some(vec![Declaration::new(
                CssProperty::FlexDirection,
                CssValue::Keyword("row-reverse".to_string()),
            )]),
            "flex-col" => Some(vec![Declaration::new(
                CssProperty::FlexDirection,
                CssValue::Keyword("column".to_string()),
            )]),
            "flex-col-reverse" => Some(vec![Declaration::new(
                CssProperty::FlexDirection,
                CssValue::Keyword("column-reverse".to_string()),
            )]),

            // Flex wrap
            "flex-wrap" => Some(vec![Declaration::new(
                CssProperty::FlexWrap,
                CssValue::Keyword("wrap".to_string()),
            )]),
            "flex-nowrap" => Some(vec![Declaration::new(
                CssProperty::FlexWrap,
                CssValue::Keyword("nowrap".to_string()),
            )]),

            // Justify content
            "justify-start" => Some(vec![Declaration::new(
                CssProperty::JustifyContent,
                CssValue::Keyword("flex-start".to_string()),
            )]),
            "justify-end" => Some(vec![Declaration::new(
                CssProperty::JustifyContent,
                CssValue::Keyword("flex-end".to_string()),
            )]),
            "justify-center" => Some(vec![Declaration::new(
                CssProperty::JustifyContent,
                CssValue::Keyword("center".to_string()),
            )]),
            "justify-between" => Some(vec![Declaration::new(
                CssProperty::JustifyContent,
                CssValue::Keyword("space-between".to_string()),
            )]),
            "justify-around" => Some(vec![Declaration::new(
                CssProperty::JustifyContent,
                CssValue::Keyword("space-around".to_string()),
            )]),
            "justify-evenly" => Some(vec![Declaration::new(
                CssProperty::JustifyContent,
                CssValue::Keyword("space-evenly".to_string()),
            )]),

            // Align items
            "items-start" => Some(vec![Declaration::new(
                CssProperty::AlignItems,
                CssValue::Keyword("flex-start".to_string()),
            )]),
            "items-end" => Some(vec![Declaration::new(
                CssProperty::AlignItems,
                CssValue::Keyword("flex-end".to_string()),
            )]),
            "items-center" => Some(vec![Declaration::new(
                CssProperty::AlignItems,
                CssValue::Keyword("center".to_string()),
            )]),
            "items-stretch" => Some(vec![Declaration::new(
                CssProperty::AlignItems,
                CssValue::Keyword("stretch".to_string()),
            )]),
            "items-baseline" => Some(vec![Declaration::new(
                CssProperty::AlignItems,
                CssValue::Keyword("baseline".to_string()),
            )]),

            // Flex grow/shrink
            "flex-1" => Some(vec![
                Declaration::new(CssProperty::FlexGrow, CssValue::Number(1.0)),
                Declaration::new(CssProperty::FlexShrink, CssValue::Number(1.0)),
                Declaration::new(CssProperty::FlexBasis, CssValue::Percentage(0.0)),
            ]),
            "flex-auto" => Some(vec![
                Declaration::new(CssProperty::FlexGrow, CssValue::Number(1.0)),
                Declaration::new(CssProperty::FlexShrink, CssValue::Number(1.0)),
                Declaration::new(CssProperty::FlexBasis, CssValue::Auto),
            ]),
            "flex-none" => Some(vec![
                Declaration::new(CssProperty::FlexGrow, CssValue::Number(0.0)),
                Declaration::new(CssProperty::FlexShrink, CssValue::Number(0.0)),
                Declaration::new(CssProperty::FlexBasis, CssValue::Auto),
            ]),
            "grow" => Some(vec![Declaration::new(
                CssProperty::FlexGrow,
                CssValue::Number(1.0),
            )]),
            "grow-0" => Some(vec![Declaration::new(
                CssProperty::FlexGrow,
                CssValue::Number(0.0),
            )]),
            "shrink" => Some(vec![Declaration::new(
                CssProperty::FlexShrink,
                CssValue::Number(1.0),
            )]),
            "shrink-0" => Some(vec![Declaration::new(
                CssProperty::FlexShrink,
                CssValue::Number(0.0),
            )]),

            // Position
            "static" => Some(vec![Declaration::new(
                CssProperty::Position,
                CssValue::Keyword("static".to_string()),
            )]),
            "relative" => Some(vec![Declaration::new(
                CssProperty::Position,
                CssValue::Keyword("relative".to_string()),
            )]),
            "absolute" => Some(vec![Declaration::new(
                CssProperty::Position,
                CssValue::Keyword("absolute".to_string()),
            )]),

            // Page breaks (PDF-specific)
            "break-before-page" => Some(vec![Declaration::new(
                CssProperty::PageBreakBefore,
                CssValue::Keyword("always".to_string()),
            )]),
            "break-after-page" => Some(vec![Declaration::new(
                CssProperty::PageBreakAfter,
                CssValue::Keyword("always".to_string()),
            )]),
            "break-inside-avoid" => Some(vec![Declaration::new(
                CssProperty::PageBreakInside,
                CssValue::Keyword("avoid".to_string()),
            )]),

            _ => None,
        }
    }

    /// Resolve sizing utilities (w-*, h-*, min-w-*, max-w-*, etc.).
    fn resolve_sizing(class: &str) -> Option<Vec<Declaration>> {
        let prefixes: &[(&str, CssProperty)] = &[
            ("w-", CssProperty::Width),
            ("h-", CssProperty::Height),
            ("min-w-", CssProperty::MinWidth),
            ("min-h-", CssProperty::MinHeight),
            ("max-w-", CssProperty::MaxWidth),
            ("max-h-", CssProperty::MaxHeight),
        ];

        for (prefix, prop) in prefixes {
            if let Some(value_str) = class.strip_prefix(prefix) {
                if let Some(value) = SIZE_MAP.get(value_str) {
                    return Some(vec![Declaration::new(prop.clone(), value.clone())]);
                }
            }
        }

        None
    }

    /// Resolve border utilities.
    fn resolve_borders(class: &str) -> Option<Vec<Declaration>> {
        let width = match class {
            "border" => Some(Length::px(1.0)),
            "border-0" => Some(Length::px(0.0)),
            "border-2" => Some(Length::px(2.0)),
            "border-4" => Some(Length::px(4.0)),
            "border-8" => Some(Length::px(8.0)),
            _ => None,
        };

        if let Some(w) = width {
            return Some(vec![
                Declaration::new(CssProperty::BorderTopWidth, CssValue::Length(w)),
                Declaration::new(CssProperty::BorderRightWidth, CssValue::Length(w)),
                Declaration::new(CssProperty::BorderBottomWidth, CssValue::Length(w)),
                Declaration::new(CssProperty::BorderLeftWidth, CssValue::Length(w)),
                Declaration::new(
                    CssProperty::BorderTopStyle,
                    CssValue::Keyword("solid".to_string()),
                ),
                Declaration::new(
                    CssProperty::BorderRightStyle,
                    CssValue::Keyword("solid".to_string()),
                ),
                Declaration::new(
                    CssProperty::BorderBottomStyle,
                    CssValue::Keyword("solid".to_string()),
                ),
                Declaration::new(
                    CssProperty::BorderLeftStyle,
                    CssValue::Keyword("solid".to_string()),
                ),
            ]);
        }

        // Border radius
        if let Some(&radius) = BORDER_RADIUS_MAP.get(class) {
            return Some(vec![Declaration::new(
                CssProperty::BorderRadius,
                CssValue::Length(Length::px(radius)),
            )]);
        }

        None
    }

    /// Extract all Tailwind utility classes from an HTML string.
    pub fn extract_classes_from_html(html: &str) -> Vec<String> {
        // Use a HashSet for O(1) dedup instead of O(n) Vec::contains.
        let mut seen = std::collections::HashSet::new();
        let mut classes = Vec::new();

        for cap in CLASS_ATTR_RE.captures_iter(html) {
            if let Some(class_str) = cap.get(1) {
                for class in class_str.as_str().split_whitespace() {
                    if !seen.contains(class) {
                        seen.insert(class.to_string());
                        classes.push(class.to_string());
                    }
                }
            }
        }

        classes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spacing_resolution() {
        let decls = TailwindResolver::resolve_class("p-4").unwrap();
        assert_eq!(decls.len(), 4); // top, right, bottom, left
    }

    #[test]
    fn test_typography_resolution() {
        let decls = TailwindResolver::resolve_class("text-xl").unwrap();
        assert!(!decls.is_empty());
    }

    #[test]
    fn test_flex_resolution() {
        let decls = TailwindResolver::resolve_class("flex").unwrap();
        assert_eq!(decls.len(), 1);
    }

    #[test]
    fn test_color_resolution() {
        let decls = TailwindResolver::resolve_class("text-blue-500").unwrap();
        assert_eq!(decls.len(), 1);
    }

    #[test]
    fn test_extract_classes() {
        let html = r#"<div class="flex p-4 text-lg"><span class="font-bold">Hi</span></div>"#;
        let classes = TailwindResolver::extract_classes_from_html(html);
        assert!(classes.contains(&"flex".to_string()));
        assert!(classes.contains(&"p-4".to_string()));
        assert!(classes.contains(&"font-bold".to_string()));
    }
}
