use ferropdf_core::{LayoutBox, PageConfig, PageBreak, PageBreakInside};
use ferropdf_core::layout::Page;

/// Fragment a layout tree into pages based on available height.
pub fn fragment_into_pages(root: &LayoutBox, config: &PageConfig) -> Vec<Page> {
    let page_height = config.content_height_pt();
    let mut pages: Vec<Page> = Vec::new();
    let mut current_content: Vec<LayoutBox> = Vec::new();
    let mut current_y = 0.0_f32;

    fragment_box(root, config, page_height, &mut pages, &mut current_content, &mut current_y);

    // Flush remaining content
    if !current_content.is_empty() {
        let page_num = pages.len() as u32 + 1;
        pages.push(Page {
            page_number: page_num,
            total_pages: 0,
            content: current_content,
            margin_boxes: Vec::new(),
        });
    }

    // Update total_pages
    let total = pages.len() as u32;
    for page in &mut pages {
        page.total_pages = total;
    }

    pages
}

fn fragment_box(
    layout_box: &LayoutBox,
    config: &PageConfig,
    page_height: f32,
    pages: &mut Vec<Page>,
    current_content: &mut Vec<LayoutBox>,
    current_y: &mut f32,
) {
    // Check for page-break-before: always
    if layout_box.style.page_break_before == PageBreak::Always && !current_content.is_empty() {
        flush_page(pages, current_content, current_y);
    }

    let box_height = layout_box.margin_box_height();
    let fits_on_current = *current_y + box_height <= page_height;
    let fits_on_new_page = box_height <= page_height;
    let avoid_break = layout_box.style.page_break_inside == PageBreakInside::Avoid;

    if fits_on_current {
        // Box fits on the current page — just add it
        current_content.push(layout_box.clone());
        *current_y += box_height;
    } else if avoid_break && fits_on_new_page {
        // page-break-inside: avoid — flush current page, place intact on new page
        flush_page(pages, current_content, current_y);
        current_content.push(layout_box.clone());
        *current_y += box_height;
    } else if !layout_box.children.is_empty() {
        // Box has children and doesn't fit — recurse into children to fragment them
        for child in &layout_box.children {
            fragment_box(child, config, page_height, pages, current_content, current_y);
        }
    } else if current_content.is_empty() {
        // Anti-infinite-loop: leaf box too large even alone — force placement
        current_content.push(layout_box.clone());
        *current_y += box_height;
    } else {
        // Leaf box doesn't fit — flush current page, place on new page
        flush_page(pages, current_content, current_y);
        // After flushing, check if leaf fits on a fresh page; if not, force it
        // (anti-infinite-loop: current_content is now empty after flush)
        current_content.push(layout_box.clone());
        *current_y += box_height;
    }

    // Check for page-break-after: always
    if layout_box.style.page_break_after == PageBreak::Always {
        flush_page(pages, current_content, current_y);
    }
}

fn flush_page(pages: &mut Vec<Page>, content: &mut Vec<LayoutBox>, current_y: &mut f32) {
    if content.is_empty() {
        return;
    }
    let page_num = pages.len() as u32 + 1;
    pages.push(Page {
        page_number: page_num,
        total_pages: 0,
        content: std::mem::take(content),
        margin_boxes: Vec::new(),
    });
    *current_y = 0.0;
}

pub fn create_empty_page(_config: &PageConfig) -> Page {
    Page {
        page_number: 1,
        total_pages: 1,
        content: Vec::new(),
        margin_boxes: Vec::new(),
    }
}
