//! CSS counter support — page / pages counters for headers/footers.

use std::collections::HashMap;

/// A store that holds the value of all CSS counters at a given point in the
/// document. After pagination the `page` and `pages` counters are filled in.
#[derive(Debug, Default, Clone)]
pub struct CounterStore {
    values: HashMap<String, i32>,
}

impl CounterStore {
    pub fn new() -> Self { Self::default() }

    pub fn set(&mut self, name: &str, value: i32) {
        self.values.insert(name.to_string(), value);
    }

    pub fn get(&self, name: &str) -> i32 {
        *self.values.get(name).unwrap_or(&0)
    }

    /// Increment a counter by `by` (default 1 for `counter-increment`).
    pub fn increment(&mut self, name: &str, by: i32) {
        *self.values.entry(name.to_string()).or_insert(0) += by;
    }

    /// Fill in the special `page` and `pages` counters after pagination.
    pub fn finalize(&mut self, current_page: usize, total_pages: usize) {
        self.set("page",  current_page  as i32);
        self.set("pages", total_pages   as i32);
    }
}
