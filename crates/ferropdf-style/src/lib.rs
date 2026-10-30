pub mod calc;
mod cascade;
mod compute;
mod gradient;
mod inherit;
pub mod matching;
pub mod pseudo_elements;

use ferropdf_core::{ComputedStyle, Document, NodeId, NodeType};
use ferropdf_parse::Stylesheet;
use std::collections::HashMap;

pub type StyleTree = HashMap<NodeId, ComputedStyle>;

/// Resolve all styles for a document given stylesheets.
///
/// Pipeline:
/// 1. Parse all CSS selectors using the `selectors` crate (Mozilla engine)
/// 2. Walk the DOM tree depth-first
/// 3. For each element, match rules using `selectors::matching::matches_selector`
/// 4. Sort matched declarations by (specificity, source_order) — specificity
///    is computed by the `selectors` crate, not by us
/// 5. Apply cascade: non-important → important → inline styles
/// 6. Inherit from parent, resolve relative units
pub fn resolve(
    document: &Document,
    stylesheets: &[Stylesheet],
    ua_css: &str,
    _page_width: f32,
) -> ferropdf_core::Result<StyleTree> {
    // Parse UA stylesheet
    let ua_sheet = ferropdf_parse::parse_stylesheet(ua_css)?;

    // All sheets: UA first, then author sheets (by reference — no cloning)
    let mut all_sheets: Vec<&Stylesheet> = Vec::with_capacity(1 + stylesheets.len());
    all_sheets.push(&ua_sheet);
    all_sheets.extend(stylesheets.iter());

    // Parse all selector lists using the `selectors` crate.
    // UA sheet is the first one — author sheets follow.
    let rules = matching::parse_rules(&all_sheets, 1);

    let root = document.root();
    let mut style_tree = StyleTree::new();
    let root_font_size = 12.0_f32; // 16px × 0.75 = 12pt
    let mut nth_cache = selectors::NthIndexCache::default();

    resolve_recursive(
        document,
        root,
        &rules,
        &mut style_tree,
        None,
        root_font_size,
        &mut nth_cache,
        0,
    );

    Ok(style_tree)
}

/// Compute the style of a box that is not in the DOM (an `@page` margin box)
/// from its declarations, inheriting from `parent` (the root element style).
pub fn style_from_declarations(
    parent: &ComputedStyle,
    declarations: &[ferropdf_parse::Declaration],
) -> ComputedStyle {
    let root_font_size = 12.0_f32;
    let mut style = inherit::inherit_from(parent);
    cascade::apply_inline_declarations(&mut style, declarations, root_font_size);
    compute::resolve_units(&mut style, Some(parent), root_font_size);
    style
}

#[allow(clippy::too_many_arguments)]
fn resolve_recursive(
    doc: &Document,
    node_id: NodeId,
    rules: &[matching::MatchedRule],
    tree: &mut StyleTree,
    parent_style: Option<&ComputedStyle>,
    root_font_size: f32,
    nth_cache: &mut selectors::NthIndexCache,
    depth: usize,
) {
    if depth > ferropdf_core::MAX_DOM_DEPTH {
        log::warn!(
            "DOM depth limit ({}) exceeded, skipping subtree",
            ferropdf_core::MAX_DOM_DEPTH
        );
        return;
    }

    let node = doc.get(node_id);

    let mut style = match parent_style {
        Some(ps) => inherit::inherit_from(ps),
        None => ComputedStyle::default(),
    };

    if node.node_type == NodeType::Element {
        // Tag-specific defaults (<b> bold, <a> underlined, <span> inline…) act
        // like user-agent rules: applied first, so any author rule or inline
        // style overrides them (`span { display: inline-block }`,
        // `a { text-decoration: none }`).
        compute::apply_tag_defaults(&mut style, node.tag());

        // Match stylesheet rules using the selectors crate
        let mut scored = matching::match_node(doc, node_id, rules, nth_cache);

        // Apply matched declarations sorted by cascade (specificity + source order)
        cascade::apply_scored_declarations(&mut style, &mut scored, root_font_size);

        // Apply inline style attribute (highest specificity for non-!important)
        if let Some(inline) = node.attr("style") {
            let declarations = ferropdf_parse::parse_inline_declarations(inline);
            if !declarations.is_empty() {
                cascade::apply_inline_declarations(&mut style, &declarations, root_font_size);
            }
        }

        // Apply HTML dir attribute (lower priority than CSS direction property)
        if let Some(dir) = node.attr("dir") {
            match dir.to_lowercase().as_str() {
                "rtl" => style.direction = ferropdf_core::Direction::Rtl,
                "ltr" => style.direction = ferropdf_core::Direction::Ltr,
                _ => {}
            }
        }

        // Resolve relative units (em/rem/px/mm → pt)
        compute::resolve_units(&mut style, parent_style, root_font_size);

        // Floats are blockified (CSS 2.1 §9.7); absolutely positioned boxes
        // do not float.
        if matches!(
            style.position,
            ferropdf_core::Position::Absolute | ferropdf_core::Position::Fixed
        ) {
            style.float = ferropdf_core::Float::None;
        }
        if style.float != ferropdf_core::Float::None
            && matches!(
                style.display,
                ferropdf_core::Display::Inline | ferropdf_core::Display::InlineBlock
            )
        {
            style.display = ferropdf_core::Display::Block;
        }
    }

    tree.insert(node_id, style.clone());

    for &child in &node.children {
        resolve_recursive(
            doc,
            child,
            rules,
            tree,
            Some(&style),
            root_font_size,
            nth_cache,
            depth + 1,
        );
    }
}

#[cfg(test)]
mod tag_default_order_tests {
    use ferropdf_core::{Display, FontWeight, TextDecoration};

    fn style_of(html: &str, css: &str, tag: &str) -> ferropdf_core::ComputedStyle {
        let pr = ferropdf_parse::parse(html).unwrap();
        let sheets = vec![ferropdf_parse::parse_stylesheet(css).unwrap()];
        let styles =
            crate::resolve(&pr.document, &sheets, ferropdf_parse::css::UA_CSS, 480.0).unwrap();
        let (id, _) = pr
            .document
            .nodes
            .iter()
            .find(|(_, n)| n.tag_name.as_deref() == Some(tag))
            .unwrap();
        styles[&id].clone()
    }

    #[test]
    fn author_rules_override_tag_defaults() {
        let html = "<html><body><p><a href='#'>l</a><b>b</b><span>s</span></p></body></html>";
        let css =
            "a { text-decoration: none } b { font-weight: normal } span { display: inline-block }";
        assert_eq!(
            style_of(html, css, "a").text_decoration,
            TextDecoration::None
        );
        assert_eq!(style_of(html, css, "b").font_weight, FontWeight::Normal);
        assert_eq!(style_of(html, css, "span").display, Display::InlineBlock);
    }

    #[test]
    fn tag_defaults_still_apply_without_author_rules() {
        let html = "<html><body><p><a href='#'>l</a><b>b</b><span>s</span></p></body></html>";
        assert_eq!(
            style_of(html, "", "a").text_decoration,
            TextDecoration::Underline
        );
        assert!(style_of(html, "", "b").font_weight.is_bold());
        assert_eq!(style_of(html, "", "span").display, Display::Inline);
    }
}
