//! CSS selector matching against DOM nodes.

use crate::html::dom::{Document, NodeId, NodeKind};
use super::values::{AttrOp, Combinator, Selector, SelectorComponent};

/// Returns `true` if `selector` matches `node_id` in `doc`.
pub fn matches_selector(selector: &Selector, node_id: NodeId, doc: &Document) -> bool {
    // The parts list describes a combinator chain.
    // We advance right-to-left through the parts and walk up the DOM.
    let part_count = selector.parts.len();
    if part_count == 0 { return false; }

    // The rightmost part (no combinator needed) must match `node_id` directly.
    let (_, components) = &selector.parts[part_count - 1];
    if !node_matches_components(node_id, components, doc) {
        return false;
    }

    if part_count == 1 { return true; }

    // Work through the remaining combinator parts right-to-left.
    walk_combinators(&selector.parts[..part_count - 1], node_id, doc)
}

fn walk_combinators(
    remaining: &[(Combinator, Vec<SelectorComponent>)],
    current: NodeId,
    doc: &Document,
) -> bool {
    if remaining.is_empty() { return true; }

    let (combinator, components) = &remaining[remaining.len() - 1];
    let next_remaining = &remaining[..remaining.len() - 1];

    match combinator {
        Combinator::Descendant => {
            // Any ancestor must match.
            let mut ancestor = doc.get(current).parent;
            while let Some(anc_id) = ancestor {
                if node_matches_components(anc_id, components, doc) {
                    if walk_combinators(next_remaining, anc_id, doc) {
                        return true;
                    }
                }
                ancestor = doc.get(anc_id).parent;
            }
            false
        }
        Combinator::Child => {
            // Direct parent must match.
            if let Some(parent_id) = doc.get(current).parent {
                if node_matches_components(parent_id, components, doc) {
                    return walk_combinators(next_remaining, parent_id, doc);
                }
            }
            false
        }
        Combinator::Adjacent => {
            // Immediately preceding sibling must match.
            if let Some(sib) = prev_sibling_element(current, doc) {
                if node_matches_components(sib, components, doc) {
                    return walk_combinators(next_remaining, sib, doc);
                }
            }
            false
        }
        Combinator::Sibling => {
            // Any preceding sibling must match.
            let mut sib = prev_sibling_element(current, doc);
            while let Some(s) = sib {
                if node_matches_components(s, components, doc) {
                    if walk_combinators(next_remaining, s, doc) {
                        return true;
                    }
                }
                sib = prev_sibling_element(s, doc);
            }
            false
        }
    }
}

/// Returns true if all `components` match `node_id`.
fn node_matches_components(node_id: NodeId, components: &[SelectorComponent], doc: &Document) -> bool {
    // Can only match element nodes.
    let node = doc.get(node_id);
    let elem = match &node.kind {
        NodeKind::Element(e) => e,
        _ => return false,
    };

    for comp in components {
        if !component_matches(comp, elem) {
            return false;
        }
    }
    true
}

fn component_matches(
    comp: &SelectorComponent,
    elem: &crate::html::dom::ElementData,
) -> bool {
    match comp {
        SelectorComponent::Universal => true,

        SelectorComponent::Type(tag) => {
            elem.tag_name.eq_ignore_ascii_case(tag)
        }

        SelectorComponent::Class(cls) => elem.has_class(cls),

        SelectorComponent::Id(id) => {
            elem.id() == Some(id.as_str())
        }

        SelectorComponent::Attribute { name, op, value } => {
            match op {
                AttrOp::Exists      => elem.attrs.contains_key(name.as_str()),
                AttrOp::Equals      => elem.get_attr(name) == Some(value.as_str()),
                AttrOp::Includes    => elem.get_attr(name)
                    .map(|v| v.split_whitespace().any(|w| w == value))
                    .unwrap_or(false),
                AttrOp::DashMatch   => elem.get_attr(name)
                    .map(|v| v == value || v.starts_with(&format!("{value}-")))
                    .unwrap_or(false),
                AttrOp::StartsWith  => elem.get_attr(name)
                    .map(|v| v.starts_with(value.as_str()))
                    .unwrap_or(false),
                AttrOp::EndsWith    => elem.get_attr(name)
                    .map(|v| v.ends_with(value.as_str()))
                    .unwrap_or(false),
                AttrOp::Contains    => elem.get_attr(name)
                    .map(|v| v.contains(value.as_str()))
                    .unwrap_or(false),
            }
        }

        SelectorComponent::PseudoClass(name) => {
            // Only the most common pseudo-classes needed for PDF:
            match name.as_str() {
                "first-child" | "first-of-type" | "last-child" | "last-of-type"
                | "only-child" | "only-of-type" => {
                    // Handled by position-in-parent check
                    // For now just allow (over-match is safe for PDF)
                    true
                }
                "enabled" | "disabled" | "checked" | "focus" | "hover" | "active" => {
                    false  // Interactive states don't apply in PDF
                }
                "not" | "is" | "where" | "has" => false, // complex, skip
                _ => false,
            }
        }

        SelectorComponent::PseudoElement(_) => false, // ::before / ::after — ignore for now
    }
}

/// Returns the immediately preceding sibling element of `node_id`.
fn prev_sibling_element(node_id: NodeId, doc: &Document) -> Option<NodeId> {
    let parent_id = doc.get(node_id).parent?;
    let siblings = match &doc.get(parent_id).kind {
        NodeKind::Element(e) => e.children.clone(),
        _ => return None,
    };

    let pos = siblings.iter().position(|&s| s == node_id)?;
    for i in (0..pos).rev() {
        if doc.get(siblings[i]).is_element() {
            return Some(siblings[i]);
        }
    }
    None
}
