//! Table captions (CSS 2.1 §17.4).
//!
//! A table is laid out as a grid of its cells, so a `<caption>` child has no
//! place in it. As in CSS, the table and its captions are put in a table
//! wrapper box: an anonymous block that takes the table's vertical margins,
//! with the captions above the table (`caption-side: top`) or below it
//! (`bottom`). Captions take the table's width and horizontal margins, so
//! they line up with it.

use ferropdf_core::{ComputedStyle, Display, Document, Length, NodeId};
use ferropdf_style::StyleTree;

/// Tag of the anonymous wrapper. No selector can match it (styles are
/// already resolved) and tagged PDF treats unknown elements as transparent.
pub(crate) const WRAPPER_TAG: &str = "ferropdf-table-wrapper";

/// Move every table's captions into a table wrapper box. Runs after style
/// resolution and before layout.
pub(crate) fn wrap_table_captions(doc: &mut Document, styles: &mut StyleTree) {
    let tables: Vec<NodeId> = doc
        .iter_preorder(doc.root())
        .filter(|&id| styles.get(&id).is_some_and(|s| s.display == Display::Table))
        .collect();

    for table in tables {
        let captions: Vec<NodeId> = doc
            .get(table)
            .children
            .iter()
            .copied()
            .filter(|&c| {
                doc.get(c).tag() == Some("caption")
                    && styles.get(&c).is_some_and(|s| s.display != Display::None)
            })
            .collect();
        let Some(parent) = doc.get(table).parent else {
            continue;
        };
        if captions.is_empty() {
            continue;
        }

        let table_style = styles.get(&table).cloned().unwrap_or_default();

        // The wrapper takes the table's vertical margins and flow role.
        let mut wrapper_style = ComputedStyle {
            display: Display::Block,
            float: table_style.float,
            ..ComputedStyle::default()
        };
        wrapper_style.margin[0] = table_style.margin[0];
        wrapper_style.margin[2] = table_style.margin[2];
        if let Some(t) = styles.get_mut(&table) {
            t.margin[0] = Length::Zero;
            t.margin[2] = Length::Zero;
        }

        // Captions are as wide as the table and aligned with it.
        for &c in &captions {
            if let Some(cs) = styles.get_mut(&c) {
                if cs.width == Length::Auto {
                    cs.width = table_style.width;
                }
                let unset =
                    |l: &Length| matches!(l, Length::Zero | Length::Pt(0.0) | Length::Px(0.0));
                if unset(&cs.margin[1]) && unset(&cs.margin[3]) {
                    cs.margin[1] = table_style.margin[1];
                    cs.margin[3] = table_style.margin[3];
                }
            }
        }

        let wrapper = doc.create_element(WRAPPER_TAG, Default::default());
        styles.insert(wrapper, wrapper_style);

        // Replace the table by the wrapper in its parent.
        let pos = doc.get(parent).children.iter().position(|&c| c == table);
        let Some(pos) = pos else { continue };
        doc.nodes[parent].children[pos] = wrapper;
        doc.nodes[wrapper].parent = Some(parent);

        doc.nodes[table].children.retain(|c| !captions.contains(c));
        let (bottom, top): (Vec<NodeId>, Vec<NodeId>) = captions
            .iter()
            .partition(|c| styles.get(c).is_some_and(|s| s.caption_bottom));
        for c in top {
            doc.append_child(wrapper, c);
        }
        doc.append_child(wrapper, table);
        for c in bottom {
            doc.append_child(wrapper, c);
        }
    }
}
