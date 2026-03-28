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

impl Node {
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
