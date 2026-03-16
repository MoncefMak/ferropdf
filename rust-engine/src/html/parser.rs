//! HTML parser using html5ever.
//!
//! Converts HTML strings into our internal DOM tree representation.

use std::collections::HashMap;

use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, NodeData, RcDom};

use super::dom::{DomNode, DomTree, ElementData, NodeType};
use crate::error::{FastPdfError, Result};

/// HTML parser that converts HTML content into a DOM tree.
pub struct HtmlParser;

impl HtmlParser {
    /// Parse an HTML string into a DOM tree.
    pub fn parse(html: &str) -> Result<DomTree> {
        let dom = parse_document(RcDom::default(), Default::default())
            .from_utf8()
            .read_from(&mut html.as_bytes())
            .map_err(|e| FastPdfError::HtmlParse(format!("Failed to parse HTML: {}", e)))?;

        let root = Self::convert_node(&dom.document);
        Ok(DomTree::new(root))
    }

    /// Parse an HTML fragment (not a full document).
    pub fn parse_fragment(html: &str) -> Result<DomTree> {
        // Wrap in a basic document structure if needed
        let wrapped = if html.contains("<html") || html.contains("<!DOCTYPE") {
            html.to_string()
        } else {
            format!(
                "<!DOCTYPE html><html><head></head><body>{}</body></html>",
                html
            )
        };
        Self::parse(&wrapped)
    }

    /// Convert an html5ever node into our DOM node representation.
    fn convert_node(handle: &Handle) -> DomNode {
        let node = handle;

        let (node_type, children_vec) = match &node.data {
            NodeData::Document => {
                let children: Vec<DomNode> = node
                    .children
                    .borrow()
                    .iter()
                    .map(Self::convert_node)
                    .collect();
                (NodeType::Document, children)
            }
            NodeData::Text { contents } => {
                let text = contents.borrow().to_string();
                (NodeType::Text(text), Vec::new())
            }
            NodeData::Element { name, attrs, .. } => {
                let tag_name = name.local.to_string();
                let mut attributes = HashMap::new();
                for attr in attrs.borrow().iter() {
                    let attr_name = attr.name.local.to_string();
                    let attr_value = attr.value.to_string();
                    attributes.insert(attr_name, attr_value);
                }

                let children: Vec<DomNode> = node
                    .children
                    .borrow()
                    .iter()
                    .map(Self::convert_node)
                    .collect();

                (
                    NodeType::Element(ElementData {
                        tag_name,
                        attributes,
                    }),
                    children,
                )
            }
            NodeData::Comment { contents } => (NodeType::Comment(contents.to_string()), Vec::new()),
            NodeData::Doctype { .. } => {
                // Skip doctype nodes, they're handled by the parser
                (NodeType::Document, Vec::new())
            }
            NodeData::ProcessingInstruction { .. } => {
                (NodeType::Comment(String::new()), Vec::new())
            }
        };

        DomNode {
            node_type,
            children: children_vec,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_html() {
        let html = "<h1>Hello World</h1>";
        let tree = HtmlParser::parse_fragment(html).unwrap();
        assert!(tree.body().is_some());
    }

    #[test]
    fn test_parse_full_document() {
        let html = r#"<!DOCTYPE html>
        <html>
        <head><title>Test</title></head>
        <body><p>Content</p></body>
        </html>"#;
        let tree = HtmlParser::parse(html).unwrap();
        assert!(tree.head().is_some());
        assert!(tree.body().is_some());
    }

    #[test]
    fn test_parse_with_attributes() {
        let html = r#"<div class="container" id="main"><p>Text</p></div>"#;
        let tree = HtmlParser::parse_fragment(html).unwrap();
        let body = tree.body().unwrap();
        let divs = body.find_elements_by_tag("div");
        assert!(!divs.is_empty());

        let div = divs[0];
        let data = div.element_data().unwrap();
        assert!(data.has_class("container"));
        assert_eq!(data.id(), Some("main"));
    }

    #[test]
    fn test_extract_styles() {
        let html = r#"<html><head><style>h1 { color: red; }</style></head><body></body></html>"#;
        let tree = HtmlParser::parse(html).unwrap();
        let styles = tree.extract_styles();
        assert_eq!(styles.len(), 1);
        assert!(styles[0].contains("color: red"));
    }

    #[test]
    fn test_text_content() {
        let html = "<div><p>Hello</p> <span>World</span></div>";
        let tree = HtmlParser::parse_fragment(html).unwrap();
        let body = tree.body().unwrap();
        let text = body.text_content();
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }
}
