//! CSS selector specificity.
//!
//! Specificity is the (a, b, c) triplet defined by the CSS spec:
//!   a = count of ID selectors
//!   b = count of class + pseudo-class + attribute selectors
//!   c = count of type + pseudo-element selectors

use crate::css::values::{Combinator, Selector, SelectorComponent};

/// CSS specificity (a, b, c) — higher wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Specificity(pub u32, pub u32, pub u32);

impl Specificity {
    pub fn inline()     -> Self { Specificity(1, 0, 0, ) }
    pub fn id()         -> Self { Specificity(0, 1, 0) }
    pub fn class()      -> Self { Specificity(0, 0, 1) }  // class / pseudo-class / attr
    pub fn element()    -> Self { Specificity(0, 0, 0) }  // type / pseudo-element  (counted in b for us)
    pub fn zero()       -> Self { Specificity(0, 0, 0) }

    pub fn add(self, other: Self) -> Self {
        Specificity(
            self.0.saturating_add(other.0),
            self.1.saturating_add(other.1),
            self.2.saturating_add(other.2),
        )
    }
}

impl PartialOrd for Specificity {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Specificity {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
            .then(self.1.cmp(&other.1))
            .then(self.2.cmp(&other.2))
    }
}

/// Calculate the specificity of a selector.
pub fn selector_specificity(sel: &Selector) -> Specificity {
    let mut spec = Specificity::default();
    for (_combinator, components) in &sel.parts {
        for comp in components {
            spec = spec.add(component_specificity(comp));
        }
    }
    spec
}

fn component_specificity(comp: &SelectorComponent) -> Specificity {
    match comp {
        SelectorComponent::Id(_)                             => Specificity(1, 0, 0),
        SelectorComponent::Class(_)
        | SelectorComponent::PseudoClass(_)
        | SelectorComponent::Attribute { .. }               => Specificity(0, 1, 0),
        SelectorComponent::Type(_)
        | SelectorComponent::PseudoElement(_)               => Specificity(0, 0, 1),
        SelectorComponent::Universal                        => Specificity::zero(),
    }
}
