//! html5ever-based HTML parser that builds our `Document` arena.

use std::borrow::Cow;
use std::collections::HashMap;

use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, NodeData, RcDom};

use ferropdf_core::FerroError;

use super::dom::{Document, ElementData, NodeId, NodeKind};

/// Parse an HTML string into a `Document`.
pub fn parse_html(html: &str) -> Result<Document, FerroError> {
    let input = if html.contains("<html") || html.contains("<!DOCTYPE") || html.contains("<!doctype") {
        Cow::Borrowed(html)
    } else {
        Cow::Owned(format!(
            "<!DOCTYPE html><html><head></head><body>{}</body></html>",
            html
        ))
    };

    let rcdom = parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut input.as_bytes())
        .map_err(|e| FerroError::HtmlParse(format!("{e}")))?;

    let mut doc = Document::new();
    // The root of rcdom is a Document; recurse into its children.
    let root_children: Vec<Handle> = rcdom.document.children.borrow().clone();
    for child in root_children {
        convert_node(&child, NodeId::root(), &mut doc);
    }

    Ok(doc)
}

/// Recursively convert an RcDom `Handle` into our arena.
fn convert_node(handle: &Handle, parent: NodeId, doc: &mut Document) {
    match &handle.data {
        NodeData::Document => {
            for child in handle.children.borrow().iter() {
                convert_node(child, parent, doc);
            }
        }

        NodeData::Doctype { .. } => { /* ignore */ }

        NodeData::Text { contents } => {
            let text = contents.borrow().to_string();
            if !text.is_empty() {
                let id = doc.alloc(NodeKind::Text(text), Some(parent));
                if let Some(elem) = doc.get_mut(parent).element_mut() {
                    elem.children.push(id);
                }
            }
        }

        NodeData::Comment { contents } => {
            let id = doc.alloc(NodeKind::Comment(contents.to_string()), Some(parent));
            if let Some(elem) = doc.get_mut(parent).element_mut() {
                elem.children.push(id);
            }
        }

        NodeData::Element { name, attrs, .. } => {
            let tag_name = name.local.to_string().to_lowercase();
            let mut attributes = HashMap::new();
            for attr in attrs.borrow().iter() {
                attributes.insert(attr.name.local.to_string(), attr.value.to_string());
            }

            let mut elem = ElementData::new(tag_name);
            elem.attrs = attributes;
            elem.parent = Some(parent);

            let id = doc.alloc(NodeKind::Element(elem), Some(parent));

            // Register this id in the parent's children list
            match doc.get_mut(parent).kind {
                NodeKind::Element(ref mut e) => { e.children.push(id); }
                NodeKind::Document => { /* document root: no Element wrapper */ }
                _ => {}
            }

            // Recurse
            for child in handle.children.borrow().iter() {
                convert_node(child, id, doc);
            }
        }

        NodeData::ProcessingInstruction { .. } => { /* ignore */ }
    }
}
