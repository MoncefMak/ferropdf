use std::borrow::Cow;
use std::collections::HashMap;
use html5ever::{
    parse_document, tendril::TendrilSink,
};
use markup5ever::interface::tree_builder::{NodeOrText, TreeSink, ElementFlags, QuirksMode};
use markup5ever::interface::{QualName, Attribute, ExpandedName};
use markup5ever::{ns, namespace_url, local_name};
use html5ever::tendril::StrTendril;
use ferropdf_core::{Document, NodeId};
use crate::ParseResult;

pub fn parse_full(html: &str) -> ferropdf_core::Result<ParseResult> {
    let sink = DomSink::new();
    let sink = parse_document(sink, Default::default()).one(html);
    let inline_styles = extract_style_tags(&sink.doc);
    Ok(ParseResult {
        external_stylesheets: sink.external_sheets,
        inline_styles,
        document: sink.doc,
    })
}

pub fn parse_html(html: &str) -> ferropdf_core::Result<Document> {
    Ok(parse_full(html)?.document)
}

struct DomSink {
    doc:             Document,
    external_sheets: Vec<String>,
    qual_names:      HashMap<NodeId, QualName>,
    default_qn:      QualName,
}

impl DomSink {
    fn new() -> Self {
        let mut doc = Document::new();
        let root = doc.create_document_root();
        let default_qn = QualName::new(None, ns!(html), local_name!("div"));
        let mut qual_names = HashMap::new();
        qual_names.insert(root, default_qn.clone());
        Self { doc, external_sheets: Vec::new(), qual_names, default_qn }
    }
}

impl TreeSink for DomSink {
    type Handle = NodeId;
    type Output = Self;

    fn finish(self) -> Self { self }

    fn parse_error(&mut self, msg: Cow<'static, str>) {
        log::debug!("HTML parse warning: {}", msg);
    }

    fn get_document(&mut self) -> Self::Handle { self.doc.root() }
    fn get_template_contents(&mut self, t: &Self::Handle) -> Self::Handle { *t }
    fn same_node(&self, x: &Self::Handle, y: &Self::Handle) -> bool { x == y }

    fn elem_name<'a>(&'a self, target: &'a Self::Handle) -> ExpandedName<'a> {
        self.qual_names.get(target)
            .unwrap_or(&self.default_qn)
            .expanded()
    }

    fn create_element(
        &mut self, name: QualName,
        attrs: Vec<Attribute>,
        _: ElementFlags,
    ) -> Self::Handle {
        let tag = name.local.as_ref().to_lowercase();
        let attr_map: HashMap<String, String> = attrs.iter()
            .map(|a| (a.name.local.to_string(), a.value.to_string()))
            .collect();

        // Collecter les <link rel="stylesheet">
        if tag == "link"
            && attr_map.get("rel").map(|s| s.to_lowercase()) == Some("stylesheet".to_string())
        {
            if let Some(href) = attr_map.get("href") {
                self.external_sheets.push(href.clone());
            }
        }

        let id = self.doc.create_element(&tag, attr_map);
        self.qual_names.insert(id, name);
        id
    }

    fn create_comment(&mut self, _: StrTendril) -> Self::Handle {
        self.doc.create_text("")
    }

    fn create_pi(&mut self, _: StrTendril, _: StrTendril) -> Self::Handle {
        self.doc.create_text("")
    }

    fn append(&mut self, parent: &Self::Handle, child: NodeOrText<Self::Handle>) {
        match child {
            NodeOrText::AppendNode(id)   => self.doc.append_child(*parent, id),
            NodeOrText::AppendText(text) => {
                let id = self.doc.create_text(text.as_ref());
                self.doc.append_child(*parent, id);
            }
        }
    }

    fn append_based_on_parent_node(
        &mut self, element: &Self::Handle,
        _prev: &Self::Handle,
        child: NodeOrText<Self::Handle>,
    ) { self.append(element, child); }

    fn append_before_sibling(
        &mut self,
        sibling: &Self::Handle,
        new_node: NodeOrText<Self::Handle>,
    ) {
        if let Some(parent_id) = self.doc.nodes[*sibling].parent {
            match new_node {
                NodeOrText::AppendNode(id) => {
                    self.doc.nodes[id].parent = Some(parent_id);
                    let children = &mut self.doc.nodes[parent_id].children;
                    if let Some(pos) = children.iter().position(|&c| c == *sibling) {
                        children.insert(pos, id);
                    } else {
                        children.push(id);
                    }
                }
                NodeOrText::AppendText(text) => {
                    let id = self.doc.create_text(text.as_ref());
                    self.doc.nodes[id].parent = Some(parent_id);
                    let children = &mut self.doc.nodes[parent_id].children;
                    if let Some(pos) = children.iter().position(|&c| c == *sibling) {
                        children.insert(pos, id);
                    } else {
                        children.push(id);
                    }
                }
            }
        }
    }

    fn append_doctype_to_document(&mut self, _: StrTendril, _: StrTendril, _: StrTendril) {}

    fn add_attrs_if_missing(&mut self, target: &Self::Handle, attrs: Vec<Attribute>) {
        for attr in attrs {
            self.doc.nodes[*target].attributes
                .entry(attr.name.local.to_string())
                .or_insert_with(|| attr.value.to_string());
        }
    }

    fn remove_from_parent(&mut self, target: &Self::Handle) {
        if let Some(pid) = self.doc.nodes[*target].parent.take() {
            self.doc.nodes[pid].children.retain(|&c| c != *target);
        }
    }

    fn reparent_children(&mut self, node: &Self::Handle, new_parent: &Self::Handle) {
        let children: Vec<_> = self.doc.nodes[*node].children.drain(..).collect();
        for child in children {
            self.doc.nodes[child].parent = Some(*new_parent);
            self.doc.nodes[*new_parent].children.push(child);
        }
    }

    fn mark_script_already_started(&mut self, _: &Self::Handle) {}
    fn pop(&mut self, _: &Self::Handle) {}
    fn set_quirks_mode(&mut self, _: QuirksMode) {}
    fn is_mathml_annotation_xml_integration_point(&self, _: &Self::Handle) -> bool { false }
    fn set_current_line(&mut self, _: u64) {}
}

fn extract_style_tags(doc: &Document) -> Vec<String> {
    doc.nodes.iter()
        .filter(|(_, n)| n.tag_name.as_deref() == Some("style"))
        .map(|(_, n)| {
            n.children.iter()
                .filter_map(|&c| doc.nodes[c].text.clone())
                .collect::<String>()
        })
        .filter(|s| !s.is_empty())
        .collect()
}
