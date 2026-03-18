mod fragment;
mod at_page;
pub mod pagination;

use ferropdf_core::{LayoutTree, LayoutBox, PageConfig, Rect, Insets};
pub use ferropdf_core::layout::Page;

/// Paginate a layout tree into individual pages.
pub fn paginate(
    layout_tree: &LayoutTree,
    page_config: &PageConfig,
) -> ferropdf_core::Result<Vec<Page>> {
    let root = match &layout_tree.root {
        Some(r) => r,
        None => return Ok(vec![pagination::create_empty_page(page_config)]),
    };

    let pages = pagination::paginate(root, page_config);

    if pages.is_empty() {
        Ok(vec![pagination::create_empty_page(page_config)])
    } else {
        Ok(pages)
    }
}
