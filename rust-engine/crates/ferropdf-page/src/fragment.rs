//! Page fragmentation — splits a stream of LayoutBoxes across physical pages.

use ferropdf_core::{EngineConfig, PageSize, Rect};
use ferropdf_layout::box_model::LayoutBox;

/// A single rendered page containing its physical size and all boxes on that page.
#[derive(Debug, Clone)]
pub struct Page {
    /// Physical page dimensions in CSS pixels.
    pub width:    f32,
    pub height:   f32,
    /// Boxes placed on this page (y is already page-local).
    pub boxes:    Vec<LayoutBox>,
    /// 1-based page number.
    pub number:   usize,
}

impl Page {
    pub fn new(number: usize, width: f32, height: f32) -> Self {
        Self { width, height, boxes: Vec::new(), number }
    }
}

/// Paginate a flattened list of LayoutBoxes, splitting them across pages of
/// `page_height_px` each. Returns one `Page` per physical page.
pub fn paginate(
    root:   &LayoutBox,
    config: &EngineConfig,
) -> Vec<Page> {
    let page_w  = config.page_size.width_px();
    let page_h  = config.page_size.height_px();
    let margin  = &config.margin;

    let content_h = page_h - margin.top - margin.bottom;

    let mut pages: Vec<Page> = Vec::new();
    let mut current_page = Page::new(1, page_w, page_h);
    let mut page_offset  = 0.0f32;    // absolute y where current page starts

    // Collect direct children in document order (depth-first pre-order)
    let mut flat: Vec<LayoutBox> = Vec::new();
    flatten(root, &mut flat);

    for mut b in flat {
        let b_top    = b.content.y;
        let b_bottom = b_top + b.content.height;

        // Check if this box crosses a page boundary
        let local_top    = b_top    - page_offset;
        let local_bottom = b_bottom - page_offset;

        let can_break = can_break_before(&b);

        if local_top >= content_h && can_break {
            // Box is entirely on the next page
            pages.push(std::mem::replace(
                &mut current_page,
                Page::new(pages.len() + 2, page_w, page_h),
            ));
            page_offset += content_h;
        } else if local_bottom > content_h && can_break {
            // Box straddles — force it to the next page unless it's very tall
            // (for very tall boxes we clip/continue; simple approach: just push start)
            if local_top > 0.0 {
                pages.push(std::mem::replace(
                    &mut current_page,
                    Page::new(pages.len() + 2, page_w, page_h),
                ));
                page_offset += content_h;
            }
        }

        // Adjust y to be page-local (x already includes margins from layout)
        b.content.y -= page_offset;
        b.content.y += margin.top;
        current_page.boxes.push(b);
    }

    if !current_page.boxes.is_empty() || pages.is_empty() {
        pages.push(current_page);
    }

    if pages.is_empty() {
        pages.push(Page::new(1, page_w, page_h));
    }

    pages
}

fn flatten(b: &LayoutBox, out: &mut Vec<LayoutBox>) {
    let mut b_clone = b.clone();
    b_clone.children.clear();
    b_clone.oof.clear();
    out.push(b_clone);
    for child in &b.children {
        flatten(child, out);
    }
    for child in &b.oof {
        flatten(child, out);
    }
}

fn can_break_before(b: &LayoutBox) -> bool {
    use ferropdf_parse::css::properties::PageBreak;
    !matches!(b.style.page_break_before, PageBreak::Avoid)
}
