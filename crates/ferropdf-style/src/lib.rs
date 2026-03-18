mod cascade;
mod inherit;
mod compute;
pub mod matching;

use std::collections::HashMap;
use ferropdf_core::{ComputedStyle, Document, NodeId, NodeType};
use ferropdf_parse::Stylesheet;

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
    document:    &Document,
    stylesheets: &[Stylesheet],
    ua_css:      &str,
    _page_width: f32,
) -> ferropdf_core::Result<StyleTree> {
    // Parse UA stylesheet
    let ua_sheet = ferropdf_parse::parse_stylesheet(ua_css)?;

    // All sheets: UA first, then author sheets
    let mut all_sheets = vec![ua_sheet];
    all_sheets.extend(stylesheets.iter().cloned());

    // Parse all selector lists using the `selectors` crate.
    // UA sheet is the first one — author sheets follow.
    let rules = matching::parse_rules(&all_sheets, 1);

    let root = document.root();
    let mut style_tree = StyleTree::new();
    let root_font_size = 12.0_f32; // 16px × 0.75 = 12pt

    resolve_recursive(document, root, &rules, &mut style_tree, None, root_font_size);

    Ok(style_tree)
}

fn resolve_recursive(
    doc: &Document,
    node_id: NodeId,
    rules: &[matching::MatchedRule],
    tree: &mut StyleTree,
    parent_style: Option<&ComputedStyle>,
    root_font_size: f32,
) {
    let node = doc.get(node_id);

    let mut style = match parent_style {
        Some(ps) => inherit::inherit_from(ps),
        None => ComputedStyle::default(),
    };

    if node.node_type == NodeType::Element {
        // Match stylesheet rules using the selectors crate
        let mut scored = matching::match_node(doc, node_id, rules);

        // Apply matched declarations sorted by cascade (specificity + source order)
        cascade::apply_scored_declarations(&mut style, &mut scored, root_font_size);

        // Apply inline style attribute (highest specificity for non-!important)
        if let Some(inline) = node.attr("style") {
            if let Ok(sheet) = ferropdf_parse::parse_stylesheet(&format!("__inline__ {{ {} }}", inline)) {
                for rule in &sheet.rules {
                    cascade::apply_inline_declarations(&mut style, &rule.declarations, root_font_size);
                }
            }
        }

        // Apply tag-specific defaults (only if stylesheet didn't override)
        compute::apply_tag_defaults(&mut style, node.tag());

        // Resolve relative units (em/rem/px/mm → pt)
        compute::resolve_units(&mut style, parent_style, root_font_size);
    }

    tree.insert(node_id, style.clone());

    for &child in &node.children {
        resolve_recursive(doc, child, rules, tree, Some(&style), root_font_size);
    }
}
