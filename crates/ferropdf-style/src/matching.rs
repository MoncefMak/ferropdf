//! CSS selector matching using the `selectors` crate (Mozilla).
//!
//! This module implements `selectors::SelectorImpl` and `selectors::Element`
//! so that the Mozilla selector engine does ALL the matching work:
//! combinators, pseudo-classes, specificity, etc.
//!
//! **RULE 2: We never write selector matching logic by hand.**

use std::borrow::Borrow;
use std::fmt;

use cssparser::{CowRcStr, ParseError, SourceLocation, ToCss};
use selectors::attr::{AttrSelectorOperation, CaseSensitivity, NamespaceConstraint};
use selectors::context::{
    IgnoreNthChildForInvalidation, MatchingContext, MatchingMode, NeedsSelectorFlags, QuirksMode,
};
use selectors::matching::ElementSelectorFlags;
use selectors::parser::{NonTSPseudoClass, ParseRelative, PseudoElement, SelectorParseErrorKind};
use selectors::{NthIndexCache, OpaqueElement, SelectorList};

use ferropdf_core::{Document, NodeId, NodeType};
use ferropdf_parse::{Declaration, Stylesheet};

// ─── CssString: newtype for String that implements ToCss ─────────────────────

/// Newtype wrapper so we can implement `cssparser::ToCss` (required by selectors).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct CssString(pub String);

impl ToCss for CssString {
    fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
        dest.write_str(&self.0)
    }
}

impl<'a> From<&'a str> for CssString {
    fn from(s: &'a str) -> Self {
        CssString(s.to_string())
    }
}

impl AsRef<str> for CssString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for CssString {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CssString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// ─── SelectorImpl ────────────────────────────────────────────────────────────

/// Our implementation of `SelectorImpl` — the type-level glue that tells
/// the selectors crate what string types we use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FerroSelectorImpl;

impl selectors::parser::SelectorImpl for FerroSelectorImpl {
    type ExtraMatchingData<'a> = ();

    type AttrValue = CssString;
    type Identifier = CssString;
    type LocalName = CssString;
    type NamespaceUrl = CssString;
    type NamespacePrefix = CssString;
    type BorrowedLocalName = str;
    type BorrowedNamespaceUrl = str;
    type NonTSPseudoClass = FerroNonTSPseudoClass;
    type PseudoElement = FerroPseudoElement;
}

// ─── Pseudo-class stub ───────────────────────────────────────────────────────

/// We don't support dynamic pseudo-classes (:hover, :focus, etc.) in PDF.
/// This is a minimal stub required by the trait.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FerroNonTSPseudoClass {}

impl ToCss for FerroNonTSPseudoClass {
    fn to_css<W: fmt::Write>(&self, _dest: &mut W) -> fmt::Result {
        match *self {}
    }
}

impl NonTSPseudoClass for FerroNonTSPseudoClass {
    type Impl = FerroSelectorImpl;

    fn is_active_or_hover(&self) -> bool {
        match *self {}
    }

    fn is_user_action_state(&self) -> bool {
        match *self {}
    }
}

// ─── Pseudo-element ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FerroPseudoElement {
    Before,
    After,
}

impl ToCss for FerroPseudoElement {
    fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
        match self {
            FerroPseudoElement::Before => dest.write_str("::before"),
            FerroPseudoElement::After => dest.write_str("::after"),
        }
    }
}

impl PseudoElement for FerroPseudoElement {
    type Impl = FerroSelectorImpl;
}

// ─── Selector parser ─────────────────────────────────────────────────────────

/// Our parser implementation — tells the selectors crate how to handle
/// custom pseudo-classes/elements (we reject them all since PDF has none).
pub struct FerroSelectorParser;

impl<'i> selectors::Parser<'i> for FerroSelectorParser {
    type Impl = FerroSelectorImpl;
    type Error = SelectorParseErrorKind<'i>;

    fn parse_non_ts_pseudo_class(
        &self,
        _location: SourceLocation,
        name: CowRcStr<'i>,
    ) -> Result<FerroNonTSPseudoClass, ParseError<'i, Self::Error>> {
        Err(cssparser::ParseError {
            kind: cssparser::ParseErrorKind::Custom(
                SelectorParseErrorKind::UnsupportedPseudoClassOrElement(name),
            ),
            location: _location,
        })
    }

    fn parse_non_ts_functional_pseudo_class<'t>(
        &self,
        name: CowRcStr<'i>,
        _arguments: &mut cssparser::Parser<'i, 't>,
    ) -> Result<FerroNonTSPseudoClass, ParseError<'i, Self::Error>> {
        Err(cssparser::ParseError {
            kind: cssparser::ParseErrorKind::Custom(
                SelectorParseErrorKind::UnsupportedPseudoClassOrElement(name),
            ),
            location: SourceLocation { line: 0, column: 0 },
        })
    }

    fn parse_pseudo_element(
        &self,
        location: SourceLocation,
        name: CowRcStr<'i>,
    ) -> Result<FerroPseudoElement, ParseError<'i, Self::Error>> {
        if name.eq_ignore_ascii_case("before") {
            Ok(FerroPseudoElement::Before)
        } else if name.eq_ignore_ascii_case("after") {
            Ok(FerroPseudoElement::After)
        } else {
            Err(cssparser::ParseError {
                kind: cssparser::ParseErrorKind::Custom(
                    SelectorParseErrorKind::UnsupportedPseudoClassOrElement(name),
                ),
                location,
            })
        }
    }

    fn parse_functional_pseudo_element<'t>(
        &self,
        name: CowRcStr<'i>,
        _arguments: &mut cssparser::Parser<'i, 't>,
    ) -> Result<FerroPseudoElement, ParseError<'i, Self::Error>> {
        Err(cssparser::ParseError {
            kind: cssparser::ParseErrorKind::Custom(
                SelectorParseErrorKind::UnsupportedPseudoClassOrElement(name),
            ),
            location: SourceLocation { line: 0, column: 0 },
        })
    }
}

// ─── Element wrapper ─────────────────────────────────────────────────────────

/// A lightweight handle into our DOM that implements `selectors::Element`.
/// This is what allows the Mozilla selector engine to walk our tree.
#[derive(Clone, Debug)]
pub struct DomNode<'a> {
    pub doc: &'a Document,
    pub id: NodeId,
}

impl<'a> PartialEq for DomNode<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && std::ptr::eq(self.doc, other.doc)
    }
}
impl<'a> Eq for DomNode<'a> {}

impl<'a> DomNode<'a> {
    pub fn new(doc: &'a Document, id: NodeId) -> Self {
        Self { doc, id }
    }

    fn node(&self) -> &'a ferropdf_core::Node {
        self.doc.get(self.id)
    }

    /// Get sibling elements (only Element nodes, skip Text/Comment).
    fn sibling_elements(&self) -> Vec<NodeId> {
        if let Some(pid) = self.node().parent {
            self.doc
                .get(pid)
                .children
                .iter()
                .copied()
                .filter(|&cid| is_real_element(self.doc, cid))
                .collect()
        } else {
            vec![]
        }
    }
}

/// An element node that is not a synthetic `::before` / `::after` box:
/// generated content is invisible to structural selectors.
fn is_real_element(doc: &Document, id: NodeId) -> bool {
    let n = doc.get(id);
    n.node_type == NodeType::Element && !n.is_pseudo()
}

impl<'a> selectors::Element for DomNode<'a> {
    type Impl = FerroSelectorImpl;

    fn opaque(&self) -> OpaqueElement {
        // OpaqueElement needs a stable pointer. We use the NodeId's inner index
        // cast to a pointer. This is safe because we only use it for identity
        // comparison within a single matching pass.
        let node_ref: &ferropdf_core::Node = self.node();
        OpaqueElement::new(node_ref)
    }

    fn parent_element(&self) -> Option<Self> {
        let parent_id = self.node().parent?;
        let parent = self.doc.get(parent_id);
        if parent.node_type == NodeType::Element || parent.node_type == NodeType::Document {
            Some(DomNode::new(self.doc, parent_id))
        } else {
            None
        }
    }

    fn parent_node_is_shadow_root(&self) -> bool {
        false
    }
    fn containing_shadow_host(&self) -> Option<Self> {
        None
    }
    fn is_pseudo_element(&self) -> bool {
        false
    }

    fn prev_sibling_element(&self) -> Option<Self> {
        let siblings = self.sibling_elements();
        let pos = siblings.iter().position(|&id| id == self.id)?;
        if pos > 0 {
            Some(DomNode::new(self.doc, siblings[pos - 1]))
        } else {
            None
        }
    }

    fn next_sibling_element(&self) -> Option<Self> {
        let siblings = self.sibling_elements();
        let pos = siblings.iter().position(|&id| id == self.id)?;
        siblings.get(pos + 1).map(|&id| DomNode::new(self.doc, id))
    }

    fn first_element_child(&self) -> Option<Self> {
        self.node()
            .children
            .iter()
            .copied()
            .find(|&cid| is_real_element(self.doc, cid))
            .map(|id| DomNode::new(self.doc, id))
    }

    fn is_html_element_in_html_document(&self) -> bool {
        true
    }

    fn has_local_name(&self, local_name: &str) -> bool {
        self.node().tag_name.as_deref() == Some(local_name)
    }

    fn has_namespace(&self, _ns: &str) -> bool {
        // We treat everything as HTML namespace
        true
    }

    fn is_same_type(&self, other: &Self) -> bool {
        self.node().tag_name == other.node().tag_name
    }

    fn attr_matches(
        &self,
        ns: &NamespaceConstraint<&CssString>,
        local_name: &CssString,
        operation: &AttrSelectorOperation<&CssString>,
    ) -> bool {
        match ns {
            NamespaceConstraint::Any => {}
            NamespaceConstraint::Specific(s) if s.0.is_empty() => {}
            NamespaceConstraint::Specific(_) => return false,
        }
        if let Some(attr_val) = self.node().attr(&local_name.0) {
            // Convert to &str-based operation for eval
            match operation {
                AttrSelectorOperation::Exists => true,
                AttrSelectorOperation::WithValue {
                    operator,
                    case_sensitivity,
                    value,
                } => {
                    let str_op = AttrSelectorOperation::WithValue {
                        operator: *operator,
                        case_sensitivity: *case_sensitivity,
                        value: value.0.as_str(),
                    };
                    str_op.eval_str(attr_val)
                }
            }
        } else {
            false
        }
    }

    fn has_attr_in_no_namespace(&self, local_name: &CssString) -> bool {
        self.node().attributes.contains_key(&local_name.0)
    }

    fn match_non_ts_pseudo_class(
        &self,
        _pc: &FerroNonTSPseudoClass,
        _context: &mut MatchingContext<FerroSelectorImpl>,
    ) -> bool {
        // No pseudo-classes supported in PDF
        false
    }

    fn match_pseudo_element(
        &self,
        _pe: &FerroPseudoElement,
        _context: &mut MatchingContext<FerroSelectorImpl>,
    ) -> bool {
        false
    }

    fn apply_selector_flags(&self, _flags: ElementSelectorFlags) {
        // We don't need incremental restyling — ignore flags
    }

    fn is_link(&self) -> bool {
        self.node().tag_name.as_deref() == Some("a") && self.node().attributes.contains_key("href")
    }

    fn is_html_slot_element(&self) -> bool {
        false
    }

    fn has_id(&self, id: &CssString, case_sensitivity: CaseSensitivity) -> bool {
        if let Some(elem_id) = self.node().attr("id") {
            match case_sensitivity {
                CaseSensitivity::CaseSensitive => elem_id == id.0.as_str(),
                CaseSensitivity::AsciiCaseInsensitive => elem_id.eq_ignore_ascii_case(&id.0),
            }
        } else {
            false
        }
    }

    fn has_class(&self, name: &CssString, case_sensitivity: CaseSensitivity) -> bool {
        if let Some(class_attr) = self.node().attr("class") {
            class_attr
                .split_whitespace()
                .any(|c| match case_sensitivity {
                    CaseSensitivity::CaseSensitive => c == name.0.as_str(),
                    CaseSensitivity::AsciiCaseInsensitive => c.eq_ignore_ascii_case(&name.0),
                })
        } else {
            false
        }
    }

    fn imported_part(&self, _name: &CssString) -> Option<CssString> {
        None
    }
    fn is_part(&self, _name: &CssString) -> bool {
        false
    }

    fn is_empty(&self) -> bool {
        self.node().children.iter().all(|&cid| {
            let child = self.doc.get(cid);
            match child.node_type {
                NodeType::Element => child.is_pseudo(),
                NodeType::Text => child.text.as_deref().is_none_or(|t| t.trim().is_empty()),
                _ => true,
            }
        })
    }

    fn is_root(&self) -> bool {
        if let Some(pid) = self.node().parent {
            self.doc.get(pid).node_type == NodeType::Document
        } else {
            false
        }
    }
}

// ─── Parsed selector with specificity ────────────────────────────────────────

/// A parsed selector list + the declarations it maps to.
pub struct MatchedRule {
    pub selectors: SelectorList<FerroSelectorImpl>,
    pub declarations: Vec<Declaration>,
    /// Source order index for cascade tiebreaking.
    pub source_order: usize,
    /// Cascade origin: 0 = user-agent, 1 = author.
    /// Per CSS Cascading Level 4 §6.1, author normal > UA normal.
    pub origin: u8,
}

/// A single declaration with its specificity and source order, ready for cascade sorting.
pub struct ScoredDeclaration {
    pub declaration: Declaration,
    /// Specificity as computed by the `selectors` crate (packed u32).
    pub specificity: u32,
    /// Source order for tiebreaking.
    pub source_order: usize,
    /// Cascade origin: 0 = user-agent, 1 = author.
    pub origin: u8,
}

// ─── Public API ──────────────────────────────────────────────────────────────

/// Parse all stylesheets into `MatchedRule`s (selector lists + declarations).
/// Selectors are parsed using the `selectors` crate, not by hand.
///
/// `ua_sheet_count` indicates how many of the leading sheets are UA stylesheets.
/// UA rules get origin=0, author rules get origin=1. Per CSS Cascading Level 4,
/// author rules always beat UA rules regardless of specificity.
pub fn parse_rules(sheets: &[&Stylesheet], ua_sheet_count: usize) -> Vec<MatchedRule> {
    let mut rules = Vec::new();
    let mut order = 0usize;

    for (sheet_idx, sheet) in sheets.iter().enumerate() {
        let origin: u8 = if sheet_idx < ua_sheet_count { 0 } else { 1 };
        for rule in &sheet.rules {
            // Join selectors back to a single string and let the selectors crate parse them
            let selector_text = rule.selectors.join(", ");
            let mut input = cssparser::ParserInput::new(&selector_text);
            let mut parser = cssparser::Parser::new(&mut input);

            let parsed = SelectorList::parse(&FerroSelectorParser, &mut parser, ParseRelative::No);

            if let Ok(selector_list) = parsed {
                rules.push(MatchedRule {
                    selectors: selector_list,
                    declarations: rule.declarations.clone(),
                    source_order: order,
                    origin,
                });
                order += 1;
            } else {
                log::debug!("Failed to parse selector: {}", selector_text);
            }
        }
    }

    rules
}

fn pseudo_kind(node: &ferropdf_core::Node) -> Option<FerroPseudoElement> {
    match node.tag()? {
        ferropdf_core::dom::PSEUDO_BEFORE => Some(FerroPseudoElement::Before),
        ferropdf_core::dom::PSEUDO_AFTER => Some(FerroPseudoElement::After),
        _ => None,
    }
}

/// Whether a rule declaring `content` targets the `::before`
/// (`after == false`) or `::after` pseudo-element of `element`. Rules
/// without `content` (`*, ::before, ::after { box-sizing: … }`) alone never
/// generate a box.
pub fn has_pseudo_rules(
    doc: &Document,
    element: NodeId,
    after: bool,
    rules: &[MatchedRule],
    nth_cache: &mut NthIndexCache,
) -> bool {
    let kind = if after {
        FerroPseudoElement::After
    } else {
        FerroPseudoElement::Before
    };
    let el = DomNode::new(doc, element);
    let declares_content = |rule: &MatchedRule| {
        rule.declarations.iter().any(|d| {
            matches!(&d.property, ferropdf_parse::CssProperty::Unknown(n) if n.eq_ignore_ascii_case("content"))
        })
    };
    rules.iter().filter(|r| declares_content(r)).any(|rule| {
        rule.selectors.0.iter().any(|selector| {
            if selector.pseudo_element() != Some(&kind) {
                return false;
            }
            let mut context = MatchingContext::new(
                MatchingMode::ForStatelessPseudoElement,
                None,
                nth_cache,
                QuirksMode::NoQuirks,
                NeedsSelectorFlags::No,
                IgnoreNthChildForInvalidation::No,
            );
            selectors::matching::matches_selector(selector, 0, None, &el, &mut context)
        })
    })
}

/// Match all rules against a node and return declarations scored with specificity.
/// The `selectors` crate handles ALL matching logic: combinators, pseudo-classes,
/// attribute selectors, :nth-child, etc.
///
/// `nth_cache` should be created once and reused across all nodes in the document
/// to avoid O(n²) re-computation of :nth-child indices.
pub fn match_node(
    doc: &Document,
    node_id: NodeId,
    rules: &[MatchedRule],
    nth_cache: &mut NthIndexCache,
) -> Vec<ScoredDeclaration> {
    let node = doc.get(node_id);
    if node.node_type != NodeType::Element {
        return Vec::new();
    }

    // A synthetic `::before` / `::after` element takes the rules whose
    // selector ends in that pseudo-element, matched against its parent
    // (the originating element). Other elements skip those rules.
    let pseudo = pseudo_kind(node);
    let (target, mode) = match (pseudo.is_some(), node.parent) {
        (true, Some(parent)) => (parent, MatchingMode::ForStatelessPseudoElement),
        (true, None) => return Vec::new(),
        (false, _) => (node_id, MatchingMode::Normal),
    };
    let element = DomNode::new(doc, target);
    let mut result = Vec::new();

    for rule in rules {
        for selector in rule.selectors.0.iter() {
            if selector.pseudo_element() != pseudo.as_ref() {
                continue;
            }
            let mut context = MatchingContext::new(
                mode,
                None, // no bloom filter
                nth_cache,
                QuirksMode::NoQuirks,
                NeedsSelectorFlags::No,
                IgnoreNthChildForInvalidation::No,
            );

            if selectors::matching::matches_selector(selector, 0, None, &element, &mut context) {
                // Use the specificity computed by the selectors crate
                let specificity = selector.specificity();
                for decl in &rule.declarations {
                    result.push(ScoredDeclaration {
                        declaration: decl.clone(),
                        specificity,
                        source_order: rule.source_order,
                        origin: rule.origin,
                    });
                }
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferropdf_core::{Document, NodeId};
    use std::collections::HashMap;

    fn elem(doc: &mut Document, parent: NodeId, tag: &str) -> NodeId {
        let id = doc.create_element(tag, HashMap::new());
        doc.append_child(parent, id);
        id
    }

    fn elem_attr(doc: &mut Document, parent: NodeId, tag: &str, attr: &str, value: &str) -> NodeId {
        let mut attrs = HashMap::new();
        attrs.insert(attr.to_string(), value.to_string());
        let id = doc.create_element(tag, attrs);
        doc.append_child(parent, id);
        id
    }

    /// Parse a single selector through FerroSelectorParser; useful to assert
    /// that a syntax actually round-trips through our parser glue.
    fn parses(selector: &str) -> bool {
        let mut input = cssparser::ParserInput::new(selector);
        let mut parser = cssparser::Parser::new(&mut input);
        SelectorList::parse(&FerroSelectorParser, &mut parser, ParseRelative::No).is_ok()
    }

    fn matches_selector(doc: &Document, node: NodeId, selector: &str) -> bool {
        let mut input = cssparser::ParserInput::new(selector);
        let mut parser = cssparser::Parser::new(&mut input);
        let list =
            SelectorList::parse(&FerroSelectorParser, &mut parser, ParseRelative::No).unwrap();
        let element = DomNode::new(doc, node);
        let mut nth_cache = NthIndexCache::default();
        list.0.iter().any(|s| {
            let mut ctx = MatchingContext::new(
                MatchingMode::Normal,
                None,
                &mut nth_cache,
                QuirksMode::NoQuirks,
                NeedsSelectorFlags::No,
                IgnoreNthChildForInvalidation::No,
            );
            selectors::matching::matches_selector(s, 0, None, &element, &mut ctx)
        })
    }

    fn three_li_doc() -> (Document, NodeId, NodeId, NodeId) {
        let mut doc = Document::new();
        let root = doc.create_document_root();
        let body = elem(&mut doc, root, "body");
        let ul = elem(&mut doc, body, "ul");
        let l1 = elem(&mut doc, ul, "li");
        let l2 = elem(&mut doc, ul, "li");
        let l3 = elem(&mut doc, ul, "li");
        (doc, l1, l2, l3)
    }

    #[test]
    fn type_selector_matches() {
        let (doc, l1, _, _) = three_li_doc();
        assert!(matches_selector(&doc, l1, "li"));
        assert!(!matches_selector(&doc, l1, "div"));
    }

    #[test]
    fn first_child_pseudo_matches_only_first() {
        let (doc, l1, l2, l3) = three_li_doc();
        assert!(matches_selector(&doc, l1, "li:first-child"));
        assert!(!matches_selector(&doc, l2, "li:first-child"));
        assert!(!matches_selector(&doc, l3, "li:first-child"));
    }

    #[test]
    fn last_child_pseudo_matches_only_last() {
        let (doc, l1, l2, l3) = three_li_doc();
        assert!(!matches_selector(&doc, l1, "li:last-child"));
        assert!(!matches_selector(&doc, l2, "li:last-child"));
        assert!(matches_selector(&doc, l3, "li:last-child"));
    }

    #[test]
    fn nth_child_index_parses() {
        // Sanity: ensure :nth-child(...) variants survive the parser glue.
        assert!(parses("li:nth-child(1)"));
        assert!(parses("li:nth-child(odd)"));
        assert!(parses("li:nth-child(2n)"));
        assert!(parses("li:nth-child(2n+1)"));
        assert!(parses("li:nth-of-type(odd)"));
        assert!(parses("li:nth-last-child(1)"));
    }

    #[test]
    fn nth_child_one_matches_first() {
        let (doc, l1, l2, _) = three_li_doc();
        assert!(matches_selector(&doc, l1, "li:nth-child(1)"));
        assert!(!matches_selector(&doc, l2, "li:nth-child(1)"));
    }

    #[test]
    fn nth_child_two_matches_second() {
        let (doc, l1, l2, l3) = three_li_doc();
        assert!(!matches_selector(&doc, l1, "li:nth-child(2)"));
        assert!(matches_selector(&doc, l2, "li:nth-child(2)"));
        assert!(!matches_selector(&doc, l3, "li:nth-child(2)"));
    }

    #[test]
    fn nth_child_odd_matches_first_and_third() {
        let (doc, l1, l2, l3) = three_li_doc();
        assert!(matches_selector(&doc, l1, "li:nth-child(odd)"));
        assert!(!matches_selector(&doc, l2, "li:nth-child(odd)"));
        assert!(matches_selector(&doc, l3, "li:nth-child(odd)"));
    }

    #[test]
    fn nth_child_even_matches_second() {
        let (doc, l1, l2, l3) = three_li_doc();
        assert!(!matches_selector(&doc, l1, "li:nth-child(even)"));
        assert!(matches_selector(&doc, l2, "li:nth-child(even)"));
        assert!(!matches_selector(&doc, l3, "li:nth-child(even)"));
    }

    #[test]
    fn nth_child_2n_plus_1_matches_odd_positions() {
        let (doc, l1, l2, l3) = three_li_doc();
        assert!(matches_selector(&doc, l1, "li:nth-child(2n+1)"));
        assert!(!matches_selector(&doc, l2, "li:nth-child(2n+1)"));
        assert!(matches_selector(&doc, l3, "li:nth-child(2n+1)"));
    }

    #[test]
    fn class_selector_matches() {
        let mut doc = Document::new();
        let root = doc.create_document_root();
        let body = elem(&mut doc, root, "body");
        let p = elem_attr(&mut doc, body, "p", "class", "warning highlight");
        assert!(matches_selector(&doc, p, "p.warning"));
        assert!(matches_selector(&doc, p, ".highlight"));
        assert!(!matches_selector(&doc, p, ".missing"));
    }

    #[test]
    fn id_selector_matches() {
        let mut doc = Document::new();
        let root = doc.create_document_root();
        let body = elem(&mut doc, root, "body");
        let p = elem_attr(&mut doc, body, "p", "id", "main");
        assert!(matches_selector(&doc, p, "#main"));
        assert!(!matches_selector(&doc, p, "#other"));
    }

    #[test]
    fn descendant_combinator_matches() {
        let mut doc = Document::new();
        let root = doc.create_document_root();
        let body = elem(&mut doc, root, "body");
        let div = elem(&mut doc, body, "div");
        let span = elem(&mut doc, div, "span");
        assert!(matches_selector(&doc, span, "div span"));
        assert!(matches_selector(&doc, span, "body span"));
        assert!(!matches_selector(&doc, div, "div span"));
    }

    #[test]
    fn child_combinator_matches() {
        let mut doc = Document::new();
        let root = doc.create_document_root();
        let body = elem(&mut doc, root, "body");
        let div = elem(&mut doc, body, "div");
        let span = elem(&mut doc, div, "span");
        assert!(matches_selector(&doc, span, "div > span"));
        // body > span is false: span is a grandchild, not a direct child.
        assert!(!matches_selector(&doc, span, "body > span"));
    }

    #[test]
    fn attribute_selector_matches() {
        let mut doc = Document::new();
        let root = doc.create_document_root();
        let body = elem(&mut doc, root, "body");
        let p = elem_attr(&mut doc, body, "p", "lang", "fr");
        assert!(matches_selector(&doc, p, "p[lang]"));
        assert!(matches_selector(&doc, p, "p[lang=\"fr\"]"));
        assert!(!matches_selector(&doc, p, "p[lang=\"en\"]"));
    }
}
