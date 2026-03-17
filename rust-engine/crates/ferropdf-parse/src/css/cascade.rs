//! CSS cascade ordering.

use super::specificity::Specificity;
use super::values::Declaration;

/// The origin of a CSS rule (higher numeric value wins in the cascade).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CascadeOrigin {
    UserAgent = 0,  // browser defaults
    Author    = 1,  // the page's own stylesheets
    Inline    = 2,  // style="..."
}

/// One entry in the cascade for a single property.
#[derive(Debug, Clone)]
pub struct CascadeEntry<'a> {
    pub decl:        &'a Declaration,
    pub origin:      CascadeOrigin,
    pub specificity: Specificity,
    /// Source order index (later = higher priority at equal origin/specificity).
    pub order:       u32,
}

impl<'a> CascadeEntry<'a> {
    /// Compare two entries: returns true if `self` wins over `other`.
    pub fn wins_over(&self, other: &CascadeEntry<'_>) -> bool {
        // !important reverses origin priority
        match (self.decl.important, other.decl.important) {
            (true, false) => return true,
            (false, true) => return false,
            _ => {}
        }
        // Same importance → origin wins
        if self.origin != other.origin {
            return self.origin > other.origin;
        }
        // Same origin → specificity wins
        if self.specificity != other.specificity {
            return self.specificity > other.specificity;
        }
        // Same specificity → source order (later wins)
        self.order > other.order
    }
}
