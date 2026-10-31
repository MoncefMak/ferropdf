use id_arena::{Arena, Id};
use std::collections::HashMap;

pub type NodeId = Id<Node>;

#[derive(Debug, Clone, PartialEq)]
pub enum NodeType {
    Document,
    Element,
    Text,
    Comment,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub node_type: NodeType,
    pub tag_name: Option<String>,
    pub attributes: HashMap<String, String>,
    pub text: Option<String>,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
}

/// Tag names of the synthetic elements standing for `::before` / `::after`.
/// No author selector can name them; the cascade matches them through
/// pseudo-element selectors on their parent (the originating element).
pub const PSEUDO_BEFORE: &str = "::before";
pub const PSEUDO_AFTER: &str = "::after";

impl Node {
    /// Whether this is a synthetic `::before` / `::after` element.
    pub fn is_pseudo(&self) -> bool {
        matches!(self.tag(), Some(PSEUDO_BEFORE | PSEUDO_AFTER))
    }
    pub fn is_element(&self) -> bool {
        self.node_type == NodeType::Element
    }
    pub fn is_text(&self) -> bool {
        self.node_type == NodeType::Text
    }
    pub fn tag(&self) -> Option<&str> {
        self.tag_name.as_deref()
    }
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attributes.get(name).map(|s| s.as_str())
    }
}

#[derive(Debug, Default)]
pub struct Document {
    pub nodes: Arena<Node>,
    pub root: Option<NodeId>,
}

impl Document {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_document_root(&mut self) -> NodeId {
        let id = self.nodes.alloc(Node {
            node_type: NodeType::Document,
            tag_name: None,
            attributes: HashMap::new(),
            text: None,
            parent: None,
            children: Vec::new(),
        });
        self.root = Some(id);
        id
    }

    pub fn create_element(&mut self, tag: &str, attrs: HashMap<String, String>) -> NodeId {
        self.nodes.alloc(Node {
            node_type: NodeType::Element,
            tag_name: Some(tag.to_lowercase()),
            attributes: attrs,
            text: None,
            parent: None,
            children: Vec::new(),
        })
    }

    pub fn create_text(&mut self, content: &str) -> NodeId {
        self.nodes.alloc(Node {
            node_type: NodeType::Text,
            tag_name: None,
            attributes: HashMap::new(),
            text: Some(content.to_string()),
            parent: None,
            children: Vec::new(),
        })
    }

    pub fn append_child(&mut self, parent: NodeId, child: NodeId) {
        self.nodes[child].parent = Some(parent);
        self.nodes[parent].children.push(child);
    }

    /// Insert `child` as the first child of `parent`. Used to materialise
    /// the `::before` pseudo-element as a real text node ahead of the
    /// element's existing children.
    pub fn prepend_child(&mut self, parent: NodeId, child: NodeId) {
        self.nodes[child].parent = Some(parent);
        self.nodes[parent].children.insert(0, child);
    }

    pub fn get(&self, id: NodeId) -> &Node {
        &self.nodes[id]
    }

    pub fn root(&self) -> NodeId {
        self.root.expect("Document has no root node")
    }

    /// Iterate over all nodes (pre-order depth-first)
    pub fn iter_preorder(&self, start: NodeId) -> PreorderIter<'_> {
        let mut stack = Vec::with_capacity(32);
        stack.push(start);
        PreorderIter { doc: self, stack }
    }
}

pub struct PreorderIter<'a> {
    doc: &'a Document,
    stack: Vec<NodeId>,
}

impl<'a> Iterator for PreorderIter<'a> {
    type Item = NodeId;
    fn next(&mut self) -> Option<NodeId> {
        let id = self.stack.pop()?;
        let node = self.doc.get(id);
        for &child in node.children.iter().rev() {
            self.stack.push(child);
        }
        Some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_create_and_append() {
        let mut doc = Document::new();
        let root = doc.create_document_root();
        let div = doc.create_element("div", HashMap::new());
        doc.append_child(root, div);

        assert_eq!(doc.get(root).children.len(), 1);
        assert_eq!(doc.get(div).parent, Some(root));
        assert_eq!(doc.get(div).tag(), Some("div"));
    }

    #[test]
    fn text_node() {
        let mut doc = Document::new();
        let text = doc.create_text("hello world");
        assert!(doc.get(text).is_text());
        assert_eq!(doc.get(text).text.as_deref(), Some("hello world"));
    }

    #[test]
    fn element_attributes() {
        let mut doc = Document::new();
        let mut attrs = HashMap::new();
        attrs.insert("class".to_string(), "header".to_string());
        attrs.insert("id".to_string(), "main".to_string());
        let elem = doc.create_element("div", attrs);

        assert_eq!(doc.get(elem).attr("class"), Some("header"));
        assert_eq!(doc.get(elem).attr("id"), Some("main"));
        assert_eq!(doc.get(elem).attr("nonexistent"), None);
    }

    #[test]
    fn preorder_traversal() {
        let mut doc = Document::new();
        let root = doc.create_document_root();
        let a = doc.create_element("a", HashMap::new());
        let b = doc.create_element("b", HashMap::new());
        let c = doc.create_element("c", HashMap::new());
        doc.append_child(root, a);
        doc.append_child(root, b);
        doc.append_child(a, c);

        let order: Vec<NodeId> = doc.iter_preorder(root).collect();
        assert_eq!(order.len(), 4); // root, a, c, b
        assert_eq!(order[0], root);
        assert_eq!(order[1], a);
        assert_eq!(order[2], c);
        assert_eq!(order[3], b);
    }

    #[test]
    fn tag_name_lowercased() {
        let mut doc = Document::new();
        let elem = doc.create_element("DIV", HashMap::new());
        // create_element lowercases the tag
        assert_eq!(doc.get(elem).tag(), Some("div"));
    }
}
