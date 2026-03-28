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

// ─── Pseudo-element stub ─────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FerroPseudoElement {}

impl ToCss for FerroPseudoElement {
    fn to_css<W: fmt::Write>(&self, _dest: &mut W) -> fmt::Result {
        match *self {}
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
        _location: SourceLocation,
        name: CowRcStr<'i>,
    ) -> Result<FerroPseudoElement, ParseError<'i, Self::Error>> {
        Err(cssparser::ParseError {
            kind: cssparser::ParseErrorKind::Custom(
                SelectorParseErrorKind::UnsupportedPseudoClassOrElement(name),
            ),
            location: _location,
        })
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
                .filter(|&cid| self.doc.get(cid).node_type == NodeType::Element)
                .collect()
        } else {
            vec![]
        }
    }
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
            .find(|&cid| self.doc.get(cid).node_type == NodeType::Element)
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
                NodeType::Element => false,
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

    let element = DomNode::new(doc, node_id);
    let mut result = Vec::new();

    for rule in rules {
        for selector in rule.selectors.0.iter() {
            let mut context = MatchingContext::new(
                MatchingMode::Normal,
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
