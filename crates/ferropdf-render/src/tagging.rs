//! Tagged PDF: the logical structure tree (ISO 32000-1 §14.7–14.8) and
//! PDF/UA-1 (ISO 14289-1).
//!
//! The painter marks every drawing op with its [`Owner`]: the DOM node whose
//! content it draws, a list label, an inline link, or a pagination artifact.
//! The PDF writer wraps each content op in a marked-content sequence
//! (`/P <</MCID n>> BDC … EMC`) and every decoration (backgrounds, borders,
//! shadows, clips, underlines) in `/Artifact BMC … EMC`. [`Tagger`] collects
//! the MCIDs and link annotations into structure elements built from the
//! DOM: one element per semantic HTML element (`h1`–`h6`, `p`, lists,
//! tables, figures, links…), created when it first receives content, so
//! empty elements never appear and the order follows the content.

use crate::display_list::Owner;
use ferropdf_core::{Document, NodeId};
use pdf_writer::{Finish, Name, Pdf, Ref, TextStr};
use std::collections::HashMap;

// =============================================================================
// Roles
// =============================================================================

/// Standard structure types used by ferropdf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    Document,
    Art,
    Sect,
    Div,
    BlockQuote,
    Caption,
    P,
    H(u8),
    L,
    LI,
    Lbl,
    LBody,
    Table,
    THead,
    TBody,
    TFoot,
    TR,
    TH,
    TD,
    Figure,
    Link,
}

impl Role {
    pub fn name(self) -> &'static [u8] {
        match self {
            Role::Document => b"Document",
            Role::Art => b"Art",
            Role::Sect => b"Sect",
            Role::Div => b"Div",
            Role::BlockQuote => b"BlockQuote",
            Role::Caption => b"Caption",
            Role::P => b"P",
            Role::H(1) => b"H1",
            Role::H(2) => b"H2",
            Role::H(3) => b"H3",
            Role::H(4) => b"H4",
            Role::H(5) => b"H5",
            Role::H(_) => b"H6",
            Role::L => b"L",
            Role::LI => b"LI",
            Role::Lbl => b"Lbl",
            Role::LBody => b"LBody",
            Role::Table => b"Table",
            Role::THead => b"THead",
            Role::TBody => b"TBody",
            Role::TFoot => b"TFoot",
            Role::TR => b"TR",
            Role::TH => b"TH",
            Role::TD => b"TD",
            Role::Figure => b"Figure",
            Role::Link => b"Link",
        }
    }
}

/// What an HTML element becomes in the structure tree.
#[derive(Debug, Clone, PartialEq)]
enum Kind {
    /// A structure element.
    Elem(Role),
    /// Decorative (`<img alt="">`, `aria-hidden`): its content is an artifact.
    Artifact,
}

fn kind_of(tag: &str, node: &ferropdf_core::Node) -> Option<Kind> {
    let hidden = node.attr("aria-hidden") == Some("true")
        || matches!(node.attr("role"), Some("presentation" | "none"));
    let role = match tag {
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => Role::H(tag.as_bytes()[1] - b'0'),
        "p" | "pre" | "address" | "dt" | "dd" | "legend" | "summary" => Role::P,
        "ul" | "ol" | "menu" => Role::L,
        "li" => Role::LI,
        "table" => Role::Table,
        "thead" => Role::THead,
        "tbody" => Role::TBody,
        "tfoot" => Role::TFoot,
        "tr" => Role::TR,
        "th" => Role::TH,
        "td" => Role::TD,
        "caption" | "figcaption" => Role::Caption,
        "blockquote" => Role::BlockQuote,
        "article" => Role::Art,
        "section" => Role::Sect,
        "div" | "main" | "header" | "footer" | "nav" | "aside" | "figure" | "form" | "fieldset"
        | "details" | "dl" | "hgroup" | "center" => Role::Div,
        "img" | "svg" | "picture" | "canvas" | "object" | "video" => {
            return Some(if hidden || node.attr("alt") == Some("") {
                Kind::Artifact
            } else {
                Kind::Elem(Role::Figure)
            });
        }
        "a" if node.attr("href").is_some() => Role::Link,
        // Inline formatting (span, b, em, code…) and anything unknown is
        // part of its parent's content.
        _ => return None,
    };
    if hidden {
        return Some(Kind::Artifact);
    }
    Some(Kind::Elem(role))
}

/// `Scope` of a header cell (ISO 32000-1 §14.8.5.7).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Scope {
    Column,
    Row,
}

#[derive(Debug, Clone)]
struct ElemInfo {
    kind: Kind,
    alt: Option<String>,
    lang: Option<String>,
    scope: Option<Scope>,
    /// `colspan` > 1 of a cell.
    colspan: Option<u32>,
}

/// `colspan` as the table layout reads it: leading digits, 0 or invalid
/// is 1, at most 1000.
fn colspan(attr: Option<&str>) -> u32 {
    let digits: String = attr
        .unwrap_or("")
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits
        .parse::<u32>()
        .map_or(if digits.is_empty() { 1 } else { 1000 }, |n| {
            n.clamp(1, 1000)
        })
}

// =============================================================================
// Semantics: what the DOM says
// =============================================================================

/// The structure-relevant facts of the DOM, computed once before writing.
pub(crate) struct Semantics {
    info: HashMap<NodeId, ElemInfo>,
    parent: HashMap<NodeId, NodeId>,
    /// `<html lang>`.
    pub lang: Option<String>,
    /// `<title>` text.
    pub title: Option<String>,
    /// `<img>` / `<svg>` without alt text, by `src` (for warnings).
    pub missing_alt: Vec<String>,
    /// Headings that do not start at `<h1>` or skip a level (PDF/UA-1 §7.4.2).
    pub heading_problem: Option<String>,
}

impl Semantics {
    pub fn from_document(doc: &Document) -> Self {
        let mut info = HashMap::new();
        let mut parent = HashMap::new();
        let mut lang = None;
        let mut title = None;
        let mut missing_alt = Vec::new();
        let mut heading_problem = None;
        let mut last_heading: Option<u8> = None;
        let root = doc.root();
        for id in doc.iter_preorder(root) {
            let node = doc.get(id);
            if let Some(p) = node.parent {
                parent.insert(id, p);
            }
            let Some(tag) = node.tag() else { continue };
            let tag = tag.to_ascii_lowercase();
            if tag == "html" {
                lang = node
                    .attr("lang")
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .map(String::from);
                continue;
            }
            if tag == "title" && title.is_none() {
                let text: String = node
                    .children
                    .iter()
                    .filter_map(|c| doc.get(*c).text.as_deref())
                    .collect::<String>();
                let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
                if !text.is_empty() {
                    title = Some(text);
                }
                continue;
            }
            let Some(kind) = kind_of(&tag, node) else {
                continue;
            };
            if let Kind::Elem(Role::H(level)) = kind {
                if heading_problem.is_none() {
                    heading_problem = match last_heading {
                        None if level != 1 => Some(format!(
                            "the first heading is <h{level}>; headings must start at <h1>"
                        )),
                        Some(prev) if level > prev + 1 => Some(format!(
                            "<h{prev}> is followed by <h{level}>; heading levels must not be skipped"
                        )),
                        _ => None,
                    };
                }
                last_heading = Some(level);
            }
            let alt = node
                .attr("alt")
                .or_else(|| node.attr("aria-label"))
                .map(|a| a.split_whitespace().collect::<Vec<_>>().join(" "));
            if kind == Kind::Elem(Role::Figure) && alt.as_deref().is_none_or(str::is_empty) {
                let src = node.attr("src").unwrap_or("");
                let src = if src.starts_with("data:") {
                    "inline image".to_string()
                } else {
                    src.to_string()
                };
                missing_alt.push(src);
            }
            let scope = (tag == "th").then(|| {
                match node.attr("scope").map(str::to_ascii_lowercase).as_deref() {
                    Some("row" | "rowgroup") => Scope::Row,
                    Some("col" | "colgroup") => Scope::Column,
                    // A header in <thead> heads a column; elsewhere a row.
                    _ if in_thead(doc, id) => Scope::Column,
                    _ => Scope::Row,
                }
            });
            let elem_lang = node
                .attr("lang")
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(String::from);
            let colspan = matches!(tag.as_str(), "td" | "th")
                .then(|| colspan(node.attr("colspan")))
                .filter(|&c| c > 1);
            info.insert(
                id,
                ElemInfo {
                    kind,
                    alt,
                    lang: elem_lang,
                    scope,
                    colspan,
                },
            );
        }
        Self {
            info,
            parent,
            lang,
            title,
            missing_alt,
            heading_problem,
        }
    }

    /// The nearest ancestor-or-self of `node` that is a structure element
    /// or an artifact.
    fn nearest(&self, mut node: NodeId) -> Option<(NodeId, &ElemInfo)> {
        for _ in 0..=ferropdf_core::MAX_DOM_DEPTH {
            if let Some(i) = self.info.get(&node) {
                return Some((node, i));
            }
            node = *self.parent.get(&node)?;
        }
        None
    }

    /// The structure element that receives the content of `node`.
    fn content_key(&self, node: Option<NodeId>) -> Key {
        match node.and_then(|n| self.nearest(n)) {
            None => Key::Document,
            Some((
                _,
                ElemInfo {
                    kind: Kind::Artifact,
                    ..
                },
            )) => Key::Artifact,
            Some((
                id,
                ElemInfo {
                    kind: Kind::Elem(Role::LI),
                    ..
                },
            )) => Key::LBody(id),
            Some((id, _)) => Key::Node(id),
        }
    }

    /// The parent element of the element `id`.
    fn parent_key(&self, id: NodeId) -> Key {
        match self.parent.get(&id) {
            Some(p) => self.content_key(Some(*p)),
            None => Key::Document,
        }
    }
}

fn in_thead(doc: &Document, mut id: NodeId) -> bool {
    for _ in 0..16 {
        let Some(p) = doc.get(id).parent else {
            return false;
        };
        match doc.get(p).tag().map(str::to_ascii_lowercase).as_deref() {
            Some("thead") => return true,
            Some("table") => return false,
            _ => id = p,
        }
    }
    false
}

// =============================================================================
// Tagger: building the tree while writing content
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Key {
    Document,
    /// Content that must not be read: decorative image, aria-hidden.
    Artifact,
    Node(NodeId),
    /// The label (bullet, number) of a list item.
    Lbl(NodeId),
    /// The body of a list item.
    LBody(NodeId),
    /// An inline `<a>` merged into a paragraph: (paragraph key, href).
    InlineLink(Box<Key>, String),
}

enum Kid {
    Elem(usize),
    Mcr { page: usize, mcid: i32 },
    Annot { page: usize, annot: Ref },
}

struct Elem {
    role: Role,
    parent: Option<usize>,
    kids: Vec<Kid>,
    alt: Option<String>,
    lang: Option<String>,
    scope: Option<Scope>,
    colspan: Option<u32>,
}

/// How a content op is marked.
pub(crate) enum Mark {
    /// `/Artifact` — `pagination` for running headers and footers.
    Artifact { pagination: bool },
    /// `/<role> <</MCID n>> BDC`.
    Content { role: Role, mcid: i32 },
}

/// Collects structure elements, MCIDs and annotation references.
pub(crate) struct Tagger<'a> {
    sem: &'a Semantics,
    elems: Vec<Elem>,
    index: HashMap<Key, usize>,
    /// Per page: element index of each MCID.
    page_mcids: Vec<Vec<usize>>,
    /// (StructParent key, element index) of link annotations.
    annots: Vec<(i32, usize)>,
    page_count: usize,
}

impl<'a> Tagger<'a> {
    pub fn new(sem: &'a Semantics, page_count: usize) -> Self {
        let mut t = Self {
            sem,
            elems: Vec::new(),
            index: HashMap::new(),
            page_mcids: vec![Vec::new(); page_count],
            annots: Vec::new(),
            page_count,
        };
        t.elems.push(Elem {
            role: Role::Document,
            parent: None,
            kids: Vec::new(),
            alt: None,
            lang: None,
            scope: None,
            colspan: None,
        });
        t.index.insert(Key::Document, 0);
        t
    }

    fn key_of(&self, owner: &Owner) -> Key {
        match owner {
            Owner::Pagination => Key::Artifact,
            Owner::Node(n) => self.sem.content_key(*n),
            Owner::Label(n) => match self.sem.content_key(Some(*n)) {
                Key::LBody(li) => Key::Lbl(li),
                // A list marker outside a list item (display: list-item).
                other => other,
            },
            Owner::InlineLink { block, target } => match self.sem.content_key(*block) {
                Key::Artifact => Key::Artifact,
                k => Key::InlineLink(Box::new(k), target.clone()),
            },
        }
    }

    /// Index of the element for `key`, created (and attached to its parent)
    /// on first use. `Key::Artifact` has no element.
    fn ensure(&mut self, key: &Key) -> Option<usize> {
        if let Some(&i) = self.index.get(key) {
            return Some(i);
        }
        let (role, parent_key, info) = match key {
            Key::Document => return Some(0),
            Key::Artifact => return None,
            Key::Node(id) => {
                let info = self.sem.info.get(id)?.clone();
                let Kind::Elem(role) = info.kind else {
                    return None;
                };
                (role, self.sem.parent_key(*id), Some(info))
            }
            Key::Lbl(li) => (Role::Lbl, Key::Node(*li), None),
            Key::LBody(li) => (Role::LBody, Key::Node(*li), None),
            Key::InlineLink(block, _) => (Role::Link, (**block).clone(), None),
        };
        // `None`: inside an artifact.
        let parent = self.ensure(&parent_key)?;
        let idx = self.elems.len();
        self.elems.push(Elem {
            role,
            parent: Some(parent),
            kids: Vec::new(),
            alt: info
                .as_ref()
                .and_then(|i| i.alt.clone())
                .filter(|a| !a.is_empty()),
            lang: info.as_ref().and_then(|i| i.lang.clone()),
            scope: info.as_ref().and_then(|i| i.scope),
            colspan: info.as_ref().and_then(|i| i.colspan),
        });
        // A list label goes before the body, whatever the paint order.
        if role == Role::Lbl {
            self.elems[parent].kids.insert(0, Kid::Elem(idx));
        } else {
            self.elems[parent].kids.push(Kid::Elem(idx));
        }
        self.index.insert(key.clone(), idx);
        Some(idx)
    }

    /// Mark the next content op of `page`, owned by `owner`.
    pub fn content(&mut self, owner: &Owner, page: usize) -> Mark {
        let key = self.key_of(owner);
        let pagination = matches!(owner, Owner::Pagination);
        let Some(idx) = self.ensure(&key) else {
            return Mark::Artifact { pagination };
        };
        let mcids = &mut self.page_mcids[page];
        let mcid = mcids.len() as i32;
        mcids.push(idx);
        self.elems[idx].kids.push(Kid::Mcr { page, mcid });
        Mark::Content {
            role: self.elems[idx].role,
            mcid,
        }
    }

    /// Register a link annotation drawn for `owner`; returns its
    /// `/StructParent` key, or `None` when the link is inside an artifact.
    pub fn annotation(&mut self, owner: &Owner, page: usize, annot: Ref) -> Option<i32> {
        let key = self.key_of(owner);
        let idx = self.ensure(&key)?;
        let k = (self.page_count + self.annots.len()) as i32;
        self.annots.push((k, idx));
        self.elems[idx].kids.push(Kid::Annot { page, annot });
        Some(k)
    }

    /// Write the structure tree, its root at `root_ref`.
    pub fn write(
        self,
        pdf: &mut Pdf,
        next_ref: &mut dyn FnMut() -> Ref,
        page_refs: &[Ref],
        root_ref: Ref,
    ) {
        let refs: Vec<Ref> = self.elems.iter().map(|_| next_ref()).collect();

        for (i, e) in self.elems.iter().enumerate() {
            let mut se = pdf.struct_element(refs[i]);
            se.custom_kind(Name(e.role.name()));
            se.parent(e.parent.map_or(root_ref, |p| refs[p]));
            if let Some(alt) = &e.alt {
                se.alt(TextStr(alt));
            }
            if let Some(lang) = &e.lang {
                se.lang(TextStr(lang));
            }
            if e.scope.is_some() || e.colspan.is_some() {
                let mut a = se.insert(Name(b"A")).dict();
                a.pair(Name(b"O"), Name(b"Table"));
                if let Some(scope) = e.scope {
                    a.pair(
                        Name(b"Scope"),
                        Name(match scope {
                            Scope::Column => b"Column",
                            Scope::Row => b"Row",
                        }),
                    );
                }
                if let Some(span) = e.colspan {
                    a.pair(Name(b"ColSpan"), span as i32);
                }
                a.finish();
            }
            let mut kids = se.children();
            for kid in &e.kids {
                match kid {
                    Kid::Elem(c) => {
                        kids.struct_element(refs[*c]);
                    }
                    Kid::Mcr { page, mcid } => {
                        kids.marked_content_ref()
                            .marked_content_id(*mcid)
                            .page(page_refs[*page]);
                    }
                    Kid::Annot { page, annot } => {
                        kids.object_ref().object(*annot).page(page_refs[*page]);
                    }
                }
            }
            kids.finish();
            se.finish();
        }

        // Parent tree: page i → array of the element of each MCID (written as
        // indirect arrays); annotation key → its Link element.
        let page_arrays: Vec<Option<Ref>> = self
            .page_mcids
            .iter()
            .map(|m| (!m.is_empty()).then(&mut *next_ref))
            .collect();
        for (mcids, r) in self.page_mcids.iter().zip(&page_arrays) {
            if let Some(r) = r {
                pdf.indirect(*r)
                    .array()
                    .items(mcids.iter().map(|&e| refs[e]));
            }
        }

        let mut root = pdf
            .indirect(root_ref)
            .start::<pdf_writer::writers::StructTreeRoot>();
        root.child(refs[0]);
        {
            let mut tree = root.parent_tree();
            let mut nums = tree.nums();
            for (page, r) in page_arrays.iter().enumerate() {
                if let Some(r) = r {
                    nums.insert(page as i32, *r);
                }
            }
            for (key, elem) in &self.annots {
                nums.insert(*key, refs[*elem]);
            }
        }
        root.parent_tree_next_key((self.page_count + self.annots.len()) as i32);
        root.finish();
    }
}
