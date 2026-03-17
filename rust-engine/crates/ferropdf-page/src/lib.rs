//! ferropdf-page — pagination, fragmentation, @page rules, CSS counters.

pub mod at_page;
pub mod counters;
pub mod fragment;

pub use fragment::{paginate, Page};
pub use at_page::AtPageResolver;
pub use counters::CounterStore;
