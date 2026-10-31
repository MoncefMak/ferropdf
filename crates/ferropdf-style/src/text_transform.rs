//! CSS `text-transform`, applied to the text of the DOM once styles are
//! known (after generated content is filled in, so it covers `::before` /
//! `::after` too).

use crate::StyleTree;
use ferropdf_core::{Document, NodeId, NodeType, TextTransform};

/// Rewrite every text node whose computed `text-transform` is not `none`.
pub fn apply_text_transform(doc: &mut Document, styles: &StyleTree) {
    if styles
        .values()
        .all(|s| s.text_transform == TextTransform::None)
    {
        return;
    }
    // Text nodes in tree order, so `capitalize` can see the character that
    // precedes a node (`<b>hel</b>lo` is one word).
    let order: Vec<NodeId> = doc
        .iter_preorder(doc.root())
        .filter(|&id| doc.get(id).node_type == NodeType::Text)
        .collect();
    let mut prev = ' ';
    for id in order {
        let Some(text) = doc.get(id).text.clone() else {
            continue;
        };
        let transform = styles
            .get(&id)
            .map_or(TextTransform::None, |s| s.text_transform);
        let out = transform_text(&text, transform, prev);
        if let Some(c) = text.chars().last() {
            prev = c;
        }
        if out != text {
            doc.nodes[id].text = Some(out);
        }
    }
}

/// Apply `transform` to `text`; `prev` is the character before it.
pub fn transform_text(text: &str, transform: TextTransform, mut prev: char) -> String {
    match transform {
        TextTransform::None => text.to_string(),
        TextTransform::Uppercase => text.to_uppercase(),
        TextTransform::Lowercase => text.to_lowercase(),
        TextTransform::Capitalize => {
            let mut out = String::with_capacity(text.len());
            for c in text.chars() {
                // A word starts after white space or punctuation, except an
                // apostrophe inside a word (as browsers do: `O'neil`,
                // `Jean-Luc`).
                let word_start = prev.is_whitespace()
                    || (!prev.is_alphanumeric() && !matches!(prev, '\'' | '’'));
                if word_start && c.is_alphabetic() {
                    out.extend(c.to_uppercase());
                } else {
                    out.push(c);
                }
                prev = c;
            }
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transforms() {
        assert_eq!(
            transform_text("straße", TextTransform::Uppercase, ' '),
            "STRASSE"
        );
        assert_eq!(
            transform_text("ÉCOLE", TextTransform::Lowercase, ' '),
            "école"
        );
        assert_eq!(
            transform_text(
                "o'neil and jean-luc (paris)",
                TextTransform::Capitalize,
                ' '
            ),
            "O'neil And Jean-Luc (Paris)"
        );
        // Continuing a word from a previous node: not capitalised.
        assert_eq!(
            transform_text("lo world", TextTransform::Capitalize, 'l'),
            "lo World"
        );
    }
}
