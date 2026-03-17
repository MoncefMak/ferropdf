//! DOM arena — a flat, indexed tree of nodes.

use std::collections::HashMap;

// ─── NodeId ──────────────────────────────────────────────────────────────────

/// Opaque index into a Document's node arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct NodeId(pub usize);

impl NodeId {
    /// The document root is always node 0.
    pub fn root() -> Self { NodeId(0) }
    pub fn as_usize(self) -> usize { self.0 }
}

// ─── Node kinds ──────────────────────────────────────────────────────────────

/// Data associated with a DOM element.
#[derive(Debug, Clone)]
pub struct ElementData {
    pub tag_name:   String,
    pub attrs:      HashMap<String, String>,
    /// Child NodeIds (in document order).
    pub children:   Vec<NodeId>,
    /// Parent NodeId (None for the root).
    pub parent:     Option<NodeId>,
}

impl ElementData {
    pub fn new(tag_name: String) -> Self {
        Self { tag_name, attrs: HashMap::new(), children: Vec::new(), parent: None }
    }

    pub fn get_attr(&self, name: &str) -> Option<&str> {
        self.attrs.get(name).map(|s| s.as_str())
    }

    pub fn has_class(&self, class: &str) -> bool {
        self.attrs
            .get("class")
            .map(|s| s.split_whitespace().any(|c| c == class))
            .unwrap_or(false)
    }

    pub fn classes(&self) -> Vec<&str> {
        self.attrs
            .get("class")
            .map(|s| s.split_whitespace().collect())
            .unwrap_or_default()
    }

    pub fn id(&self) -> Option<&str> {
        self.attrs.get("id").map(|s| s.as_str())
    }
}

/// The content carried by a node.
#[derive(Debug, Clone)]
pub enum NodeKind {
    Document,
    Element(ElementData),
    Text(String),
    Comment(String),
}

/// A single DOM node inside the Document arena.
#[derive(Debug, Clone)]
pub struct DomNode {
    pub kind:   NodeKind,
    pub parent: Option<NodeId>,
}

impl DomNode {
    pub fn is_element(&self) -> bool { matches!(self.kind, NodeKind::Element(_)) }
    pub fn is_text(&self)    -> bool { matches!(self.kind, NodeKind::Text(_))    }

    pub fn element(&self) -> Option<&ElementData> {
        match &self.kind { NodeKind::Element(e) => Some(e), _ => None }
    }
    pub fn element_mut(&mut self) -> Option<&mut ElementData> {
        match &mut self.kind { NodeKind::Element(e) => Some(e), _ => None }
    }

    pub fn text(&self) -> Option<&str> {
        match &self.kind { NodeKind::Text(t) => Some(t), _ => None }
    }

    pub fn tag_name(&self) -> Option<&str> {
        self.element().map(|e| e.tag_name.as_str())
    }
}

// ─── Document ────────────────────────────────────────────────────────────────

/// The full parsed HTML document — a flat arena of `DomNode`s.
#[derive(Debug, Clone)]
pub struct Document {
    pub nodes: Vec<DomNode>,
}

impl Document {
    /// Create an empty document with just the root node.
    pub fn new() -> Self {
        let root = DomNode { kind: NodeKind::Document, parent: None };
        Self { nodes: vec![root] }
    }

    /// Allocate a new node and return its `NodeId`.
    pub fn alloc(&mut self, kind: NodeKind, parent: Option<NodeId>) -> NodeId {
        let id = NodeId(self.nodes.len());
        self.nodes.push(DomNode { kind, parent });
        id
    }

    pub fn get(&self, id: NodeId) -> &DomNode {
        &self.nodes[id.0]
    }

    pub fn get_mut(&mut self, id: NodeId) -> &mut DomNode {
        &mut self.nodes[id.0]
    }

    /// Children of an element node.
    pub fn children(&self, id: NodeId) -> &[NodeId] {
        match &self.nodes[id.0].kind {
            NodeKind::Element(e) => &e.children,
            NodeKind::Document   => {
                // children of Document are tracked in node 0's element field,
                // but Document is special — use document_children() instead
                &[]
            }
            _ => &[],
        }
    }

    /// Add `child` as the last child of `parent`.
    pub fn append_child(&mut self, parent: NodeId, child: NodeId) {
        // Set child's parent
        self.nodes[child.0].parent = Some(parent);

        // Push into parent's children list
        match &mut self.nodes[parent.0].kind {
            NodeKind::Element(e) => e.children.push(child),
            NodeKind::Document   => {
                // store document-level children in a synthetic element
                // We just skip for the root — callers use document.root_children
            }
            _ => {}
        }
    }

    /// Returns text content of subtree rooted at `id`.
    pub fn text_content(&self, id: NodeId) -> String {
        match &self.nodes[id.0].kind {
            NodeKind::Text(t) => t.clone(),
            NodeKind::Element(e) => {
                e.children.iter().map(|&c| self.text_content(c)).collect()
            }
            _ => String::new(),
        }
    }

    /// Find first element with the given tag name in the subtree rooted at `id`.
    pub fn find_element(&self, id: NodeId, tag: &str) -> Option<NodeId> {
        match &self.nodes[id.0].kind {
            NodeKind::Element(e) => {
                if e.tag_name == tag { return Some(id); }
                for &c in &e.children {
                    if let Some(found) = self.find_element(c, tag) {
                        return Some(found);
                    }
                }
                None
            }
            NodeKind::Document => {
                for c in self.doc_children() {
                    if let Some(found) = self.find_element(c, tag) {
                        return Some(found);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// All direct children of the document root.
    pub fn doc_children(&self) -> Vec<NodeId> {
        // We store doc children in the extra_doc_children field
        self.nodes.iter().enumerate()
            .filter(|(_, n)| n.parent == Some(NodeId::root()))
            .map(|(i, _)| NodeId(i))
            .collect()
    }

    /// The `<html>` element.
    pub fn html_element(&self) -> Option<NodeId> {
        self.find_element(NodeId::root(), "html")
            .or_else(|| self.doc_children().into_iter().find(|&id| {
                self.get(id).tag_name() == Some("html")
            }))
    }

    /// The `<head>` element.
    pub fn head(&self) -> Option<NodeId> {
        let html = self.html_element()?;
        self.find_element(html, "head")
    }

    /// The `<body>` element.
    pub fn body(&self) -> Option<NodeId> {
        let html = self.html_element()?;
        self.find_element(html, "body")
    }

    /// Collect text from `<style>` elements in `<head>`.
    pub fn extract_stylesheets(&self) -> Vec<String> {
        let mut sheets = Vec::new();
        if let Some(head_id) = self.head() {
            self.collect_style_text(head_id, &mut sheets);
        }
        // Also collect style tags scattered through the document
        if let Some(body_id) = self.body() {
            self.collect_style_text(body_id, &mut sheets);
        }
        sheets
    }

    fn collect_style_text(&self, id: NodeId, out: &mut Vec<String>) {
        match &self.nodes[id.0].kind {
            NodeKind::Element(e) => {
                if e.tag_name == "style" {
                    let css = e.children.iter()
                        .filter_map(|&c| self.nodes[c.0].text())
                        .collect::<String>();
                    if !css.is_empty() { out.push(css); }
                }
                for &c in &e.children.clone() {
                    self.collect_style_text(c, out);
                }
            }
            _ => {}
        }
    }
}

impl Default for Document {
    fn default() -> Self { Self::new() }
}
