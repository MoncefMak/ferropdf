//! DOM tree representation for parsed HTML.

use std::collections::HashMap;
use std::fmt;

/// Represents a node in the DOM tree.
#[derive(Debug, Clone)]
pub struct DomNode {
    /// The type of this node.
    pub node_type: NodeType,
    /// Child nodes.
    pub children: Vec<DomNode>,
}

/// The type of a DOM node.
#[derive(Debug, Clone)]
pub enum NodeType {
    /// A text node containing character data.
    Text(String),
    /// An element node with tag name and attributes.
    Element(ElementData),
    /// A comment node.
    Comment(String),
    /// The document root.
    Document,
}

/// Data associated with an HTML element.
#[derive(Debug, Clone)]
pub struct ElementData {
    /// The tag name (e.g., "div", "h1", "p").
    pub tag_name: String,
    /// HTML attributes as key-value pairs.
    pub attributes: HashMap<String, String>,
}

impl ElementData {
    /// Create a new ElementData with the given tag name.
    pub fn new(tag_name: String) -> Self {
        Self {
            tag_name,
            attributes: HashMap::new(),
        }
    }

    /// Get the value of an attribute.
    pub fn get_attr(&self, name: &str) -> Option<&str> {
        self.attributes.get(name).map(|s| s.as_str())
    }

    /// Check if the element has a specific class.
    pub fn has_class(&self, class: &str) -> bool {
        self.attributes
            .get("class")
            .map(|classes| classes.split_whitespace().any(|c| c == class))
            .unwrap_or(false)
    }

    /// Get all classes as a vector.
    pub fn classes(&self) -> Vec<&str> {
        self.attributes
            .get("class")
            .map(|classes| classes.split_whitespace().collect())
            .unwrap_or_default()
    }

    /// Get the ID attribute if present.
    pub fn id(&self) -> Option<&str> {
        self.get_attr("id")
    }
}

impl DomNode {
    /// Create a new text node.
    pub fn text(content: String) -> Self {
        Self {
            node_type: NodeType::Text(content),
            children: Vec::new(),
        }
    }

    /// Create a new element node.
    pub fn element(tag_name: String, attributes: HashMap<String, String>) -> Self {
        Self {
            node_type: NodeType::Element(ElementData {
                tag_name,
                attributes,
            }),
            children: Vec::new(),
        }
    }

    /// Create a document root node.
    pub fn document() -> Self {
        Self {
            node_type: NodeType::Document,
            children: Vec::new(),
        }
    }

    /// Returns true if this is a text node.
    pub fn is_text(&self) -> bool {
        matches!(self.node_type, NodeType::Text(_))
    }

    /// Returns true if this is an element node.
    pub fn is_element(&self) -> bool {
        matches!(self.node_type, NodeType::Element(_))
    }

    /// Get the tag name if this is an element.
    pub fn tag_name(&self) -> Option<&str> {
        match &self.node_type {
            NodeType::Element(data) => Some(&data.tag_name),
            _ => None,
        }
    }

    /// Get the element data if this is an element.
    pub fn element_data(&self) -> Option<&ElementData> {
        match &self.node_type {
            NodeType::Element(data) => Some(data),
            _ => None,
        }
    }

    /// Get text content recursively.
    pub fn text_content(&self) -> String {
        match &self.node_type {
            NodeType::Text(text) => text.clone(),
            _ => self.children.iter().map(|c| c.text_content()).collect(),
        }
    }

    /// Find all elements matching a tag name.
    pub fn find_elements_by_tag(&self, tag: &str) -> Vec<&DomNode> {
        let mut result = Vec::new();
        self.find_elements_by_tag_recursive(tag, &mut result);
        result
    }

    fn find_elements_by_tag_recursive<'a>(&'a self, tag: &str, result: &mut Vec<&'a DomNode>) {
        if let NodeType::Element(data) = &self.node_type {
            if data.tag_name == tag {
                result.push(self);
            }
        }
        for child in &self.children {
            child.find_elements_by_tag_recursive(tag, result);
        }
    }

    /// Find the first element matching a tag name.
    pub fn find_element_by_tag(&self, tag: &str) -> Option<&DomNode> {
        if let NodeType::Element(data) = &self.node_type {
            if data.tag_name == tag {
                return Some(self);
            }
        }
        for child in &self.children {
            if let Some(node) = child.find_element_by_tag(tag) {
                return Some(node);
            }
        }
        None
    }

    /// Find elements by class name.
    pub fn find_elements_by_class(&self, class: &str) -> Vec<&DomNode> {
        let mut result = Vec::new();
        self.find_elements_by_class_recursive(class, &mut result);
        result
    }

    fn find_elements_by_class_recursive<'a>(&'a self, class: &str, result: &mut Vec<&'a DomNode>) {
        if let NodeType::Element(data) = &self.node_type {
            if data.has_class(class) {
                result.push(self);
            }
        }
        for child in &self.children {
            child.find_elements_by_class_recursive(class, result);
        }
    }

    /// Count all descendant nodes.
    pub fn node_count(&self) -> usize {
        1 + self.children.iter().map(|c| c.node_count()).sum::<usize>()
    }
}

/// A complete DOM tree.
#[derive(Debug, Clone)]
pub struct DomTree {
    /// The root node of the tree.
    pub root: DomNode,
}

impl DomTree {
    /// Create a new DOM tree with the given root.
    pub fn new(root: DomNode) -> Self {
        Self { root }
    }

    /// Get the <head> element if present.
    pub fn head(&self) -> Option<&DomNode> {
        self.root.find_element_by_tag("head")
    }

    /// Get the <body> element if present.
    pub fn body(&self) -> Option<&DomNode> {
        self.root.find_element_by_tag("body")
    }

    /// Get the total node count.
    pub fn node_count(&self) -> usize {
        self.root.node_count()
    }

    /// Extract all <style> elements' content.
    pub fn extract_styles(&self) -> Vec<String> {
        self.root
            .find_elements_by_tag("style")
            .iter()
            .map(|node| node.text_content())
            .collect()
    }

    /// Extract all <link rel="stylesheet"> hrefs.
    pub fn extract_stylesheet_links(&self) -> Vec<String> {
        self.root
            .find_elements_by_tag("link")
            .iter()
            .filter_map(|node| {
                if let Some(data) = node.element_data() {
                    if data.get_attr("rel") == Some("stylesheet") {
                        return data.get_attr("href").map(|s| s.to_string());
                    }
                }
                None
            })
            .collect()
    }
}

impl fmt::Display for DomNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.node_type {
            NodeType::Text(text) => write!(f, "{}", text),
            NodeType::Element(data) => {
                write!(f, "<{}", data.tag_name)?;
                for (key, value) in &data.attributes {
                    write!(f, " {}=\"{}\"", key, value)?;
                }
                if self.children.is_empty() {
                    write!(f, " />")
                } else {
                    write!(f, ">")?;
                    for child in &self.children {
                        write!(f, "{}", child)?;
                    }
                    write!(f, "</{}>", data.tag_name)
                }
            }
            NodeType::Comment(text) => write!(f, "<!--{}-->", text),
            NodeType::Document => {
                for child in &self.children {
                    write!(f, "{}", child)?;
                }
                Ok(())
            }
        }
    }
}
