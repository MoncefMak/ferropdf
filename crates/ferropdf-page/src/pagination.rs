// =============================================================================
// pagination.rs — PDF fragmentation and pagination algorithm
// =============================================================================
// Inspiration:
//   CSS Fragmentation Module Level 3
//   https://www.w3.org/TR/css-break-3/
//
//   Blink fragmentation utils:
//   blink/renderer/core/layout/fragmentation_utils.cc
//   blink/renderer/core/layout/ng/ng_block_break_token.cc
//
// This module runs AFTER Taffy + block_flow.
// It takes the LayoutTree (infinite ribbon of content) and slices it into pages.
//
// MENTAL MODEL:
//   The infinite content ribbon (Y positions from 0 to +∞) is sliced
//   into pages. We iterate over the root's child blocks and place them
//   one by one on the current page. When a block doesn't fit,
//   we fragment it (recursively into its children) or push it
//   to the next page.
//
//   The key variable is `page_y_offset`: the absolute Y coordinate
//   in the ribbon that corresponds to Y=0 on the current page.
//   To reposition a block on the page:
//     y_on_page = y_absolute - page_y_offset
// =============================================================================

use ferropdf_core::layout::Page;
use ferropdf_core::{Display, Insets, LayoutBox, PageBreak, PageBreakInside, PageConfig, Rect};

// =============================================================================
// DATA STRUCTURES
// =============================================================================

/// Current state of the paginator — inspired by FragmentainerContext in Blink.
#[derive(Debug)]
struct PaginationContext {
    /// Absolute Y position (in the ribbon) corresponding to the top of the current page.
    page_y_offset: f32,
    /// Height consumed on the current page (to know how much space remains).
    used_height: f32,
    /// Content of the page being built.
    current_page_boxes: Vec<LayoutBox>,
    /// Already completed pages.
    finished_pages: Vec<Page>,
    /// Height of a page (content area, in CSS pixels).
    page_height: f32,
}

impl PaginationContext {
    fn new(page_height: f32) -> Self {
        Self {
            page_y_offset: 0.0,
            used_height: 0.0,
            current_page_boxes: Vec::new(),
            finished_pages: Vec::new(),
            page_height,
        }
    }

    /// Flush the current page and start a new one.
    /// `next_y` is the absolute Y position of the next element to place
    /// (used to set page_y_offset of the new page).
    fn flush_page(&mut self, next_y: f32) {
        if !self.current_page_boxes.is_empty() {
            let page_number = self.finished_pages.len() as u32 + 1;
            self.finished_pages.push(Page {
                page_number,
                total_pages: 0,
                content: std::mem::take(&mut self.current_page_boxes),
                margin_boxes: Vec::new(),
            });
        }
        self.page_y_offset = next_y;
        self.used_height = 0.0;
    }

    fn is_current_page_empty(&self) -> bool {
        self.current_page_boxes.is_empty()
    }
}

// =============================================================================
// MAIN ENTRY POINT
// =============================================================================

/// Fragments a LayoutTree root into PDF pages.
/// All coordinates are in typographic points (pt).
pub fn paginate(root: &LayoutBox, config: &PageConfig) -> Vec<Page> {
    let page_height = config.content_height_pt();
    let mut ctx = PaginationContext::new(page_height);

    // Process the root's direct children
    for child in &root.children {
        fragment_box(child, &mut ctx);
    }

    // Flush the last page if not empty
    if !ctx.is_current_page_empty() {
        ctx.flush_page(0.0);
    }

    // If no pages were produced, create an empty page
    if ctx.finished_pages.is_empty() {
        ctx.finished_pages.push(Page {
            page_number: 1,
            total_pages: 1,
            content: Vec::new(),
            margin_boxes: Vec::new(),
        });
    }

    // Update total_pages
    let total = ctx.finished_pages.len() as u32;
    for page in &mut ctx.finished_pages {
        page.total_pages = total;
    }

    ctx.finished_pages
}

// =============================================================================
// LAYOUT BOX FRAGMENTATION
// CSS Fragmentation Level 3 §4 — Fragmentation Model
// =============================================================================

fn fragment_box(layout_box: &LayoutBox, ctx: &mut PaginationContext) {
    let style = &layout_box.style;
    let box_height = layout_box.rect.height;

    // ─── Rule 1: page-break-before ────────────────────────────────────────
    if should_break_before(style) && !ctx.is_current_page_empty() {
        ctx.flush_page(layout_box.rect.y);
    }

    // ─── Position-based fit check ────────────────────────────────────────────
    // Check if the box, at its actual ribbon position, fits within the current page.
    let box_bottom_on_page = (layout_box.rect.y - ctx.page_y_offset) + box_height;
    let fits_on_current_page = box_bottom_on_page <= ctx.page_height;
    let fits_on_new_page = box_height <= ctx.page_height;

    // ─── Rule 2: page-break-inside: avoid ─────────────────────────────────
    let avoid_break_inside = style.page_break_inside == PageBreakInside::Avoid;

    if !fits_on_current_page && avoid_break_inside && fits_on_new_page {
        if !ctx.is_current_page_empty() {
            ctx.flush_page(layout_box.rect.y);
        }
        place_box_on_current_page(layout_box, ctx);
        if should_break_after(style) {
            ctx.flush_page(layout_box.rect.y + layout_box.rect.height);
        }
        return;
    }
    // The block is taller than a page → we cannot avoid the break.

    // ─── Rule 3: The block fits on the current page ───────────────────────
    if fits_on_current_page {
        place_box_on_current_page(layout_box, ctx);
        if should_break_after(style) {
            ctx.flush_page(layout_box.rect.y + layout_box.rect.height);
        }
        return;
    }

    // ─── Rule 3a: Orphans/widows — text blocks near a page boundary ──────
    // If a text block has shaped lines, check whether splitting would violate
    // orphans (min lines on current page) or widows (min lines on next page).
    // If so, push the entire block to the next page.
    if !layout_box.shaped_lines.is_empty() && layout_box.children.is_empty() {
        let total_lines = layout_box.shaped_lines.len();
        let space_left = ctx.page_height - (layout_box.rect.y - ctx.page_y_offset);
        let line_height = if total_lines > 0 {
            box_height / total_lines as f32
        } else {
            box_height
        };
        let lines_that_fit = (space_left / line_height).floor() as usize;
        let lines_on_next = total_lines.saturating_sub(lines_that_fit);

        let orphans = style.orphans as usize;
        let widows = style.widows as usize;

        // Would violate orphans or widows → push entire block to next page
        let violates_orphans = lines_that_fit > 0 && lines_that_fit < orphans;
        let violates_widows = lines_on_next > 0 && lines_on_next < widows;
        if total_lines > 1
            && (violates_orphans || violates_widows)
            && fits_on_new_page
            && !ctx.is_current_page_empty()
        {
            ctx.flush_page(layout_box.rect.y);
            place_box_on_current_page(layout_box, ctx);
            if should_break_after(style) {
                ctx.flush_page(layout_box.rect.y + layout_box.rect.height);
            }
            return;
        }
    }

    // ─── Rule 3b: Table rows are atomic — never split a row across pages ────
    let is_table_row = layout_box.style.display == Display::TableRow;
    if !fits_on_current_page && is_table_row && fits_on_new_page {
        if !ctx.is_current_page_empty() {
            ctx.flush_page(layout_box.rect.y);
        }
        place_box_on_current_page(layout_box, ctx);
        if should_break_after(style) {
            ctx.flush_page(layout_box.rect.y + layout_box.rect.height);
        }
        return;
    }

    // ─── Rule 3c: Text blocks split between lines ─────────────────────────
    // A paragraph (text leaf or block with merged inline text) that does not
    // fit continues on the next page(s), honouring orphans/widows.
    if !layout_box.shaped_lines.is_empty() {
        fragment_text_box(layout_box, ctx);
        if should_break_after(style) && !ctx.is_current_page_empty() {
            ctx.flush_page(layout_box.rect.y + layout_box.rect.height);
        }
        return;
    }

    // ─── Rule 4: The block doesn't fit → fragmentation ─────────────────────
    if !layout_box.children.is_empty() {
        if layout_box.style.display == Display::Table {
            // Table-specific pagination with row atomicity and thead repeating
            fragment_table(layout_box, ctx);
        } else {
            // Generic container fragmentation
            fragment_container(layout_box, ctx);
        }
    } else {
        // Leaf box (text, image, etc.)
        if ctx.is_current_page_empty() {
            // Force: even if too large, place it on the empty page (anti-infinite-loop)
            place_box_on_current_page(layout_box, ctx);
            ctx.flush_page(layout_box.rect.y + layout_box.rect.height);
        } else {
            // Push to the next page
            ctx.flush_page(layout_box.rect.y);
            place_box_on_current_page(layout_box, ctx);
            if box_height > ctx.page_height {
                ctx.flush_page(layout_box.rect.y + layout_box.rect.height);
            }
        }
    }

    // ─── Rule 5: page-break-after ─────────────────────────────────────────
    if should_break_after(style) && !ctx.is_current_page_empty() {
        ctx.flush_page(layout_box.rect.y + layout_box.rect.height);
    }
}

// =============================================================================
// CONTAINER FRAGMENTATION
// When a container doesn't fit on the current page, we distribute
// its children between the current page and subsequent ones, creating
// "wrapper fragments" on each page to preserve the visual context
// (background, borders) of the parent container.
// =============================================================================

fn fragment_container(layout_box: &LayoutBox, ctx: &mut PaginationContext) {
    // Collect children that go on the current page vs next pages
    let mut current_page_children: Vec<LayoutBox> = Vec::new();
    let mut is_first_page = true;

    for child in &layout_box.children {
        let child_height = child.rect.height;
        // Position-based fit check: does this child's bottom fit on the current page?
        let child_bottom_on_page = (child.rect.y - ctx.page_y_offset) + child_height;
        let child_fits = child_bottom_on_page <= ctx.page_height;

        if child_fits {
            // Child fits on current page — add to current wrapper
            let mut placed_child = child.clone();
            offset_y_recursive(&mut placed_child, -ctx.page_y_offset);
            current_page_children.push(placed_child);
            ctx.used_height = ctx.used_height.max(child_bottom_on_page);
        } else if (!child.shaped_lines.is_empty()
            && child.style.page_break_inside != PageBreakInside::Avoid)
            || (!child.children.is_empty() && child_height > ctx.page_height)
        {
            // Child is a large container that doesn't fit on any single page
            // Flush current wrapper first, then recurse into this child
            if !current_page_children.is_empty() {
                let wrapper = make_container_fragment(
                    layout_box,
                    &current_page_children,
                    ctx,
                    is_first_page,
                    false,
                );
                ctx.current_page_boxes.push(wrapper);
                current_page_children.clear();
            }
            // Recurse into the child's own fragmentation
            fragment_box(child, ctx);
            is_first_page = false;
        } else {
            // Child doesn't fit — flush current page and start new
            if !current_page_children.is_empty() || !ctx.is_current_page_empty() {
                if !current_page_children.is_empty() {
                    let wrapper = make_container_fragment(
                        layout_box,
                        &current_page_children,
                        ctx,
                        is_first_page,
                        false,
                    );
                    ctx.current_page_boxes.push(wrapper);
                    current_page_children.clear();
                }
                ctx.flush_page(child.rect.y);
                is_first_page = false;
            }

            // Place child on new page
            let mut placed_child = child.clone();
            offset_y_recursive(&mut placed_child, -ctx.page_y_offset);
            let child_bottom = placed_child.rect.y + placed_child.rect.height;
            current_page_children.push(placed_child);
            ctx.used_height = ctx.used_height.max(child_bottom);
        }
    }

    // Flush remaining children as a wrapper on the current page
    if !current_page_children.is_empty() {
        let wrapper =
            make_container_fragment(layout_box, &current_page_children, ctx, is_first_page, true);
        ctx.current_page_boxes.push(wrapper);
    }
}

// =============================================================================
// TEXT FRAGMENTATION
// A box carrying shaped lines is sliced between lines (CSS Fragmentation 3
// §4.4 class A break points). Each slice is a clone of the box holding a
// subset of the lines, re-based so its first line sits at the slice top.
// =============================================================================

fn fragment_text_box(layout_box: &LayoutBox, ctx: &mut PaginationContext) {
    let lines = &layout_box.shaped_lines;
    let n = lines.len();
    let first_baseline = lines[0].y;
    let line_height = if n > 1 {
        (lines[1].y - lines[0].y).max(0.1)
    } else {
        layout_box.content.height.max(0.1)
    };
    // Top of line `k` relative to the content box top.
    let top_of = |k: usize| lines[k].y - first_baseline;
    let content_inset_top = layout_box.content.y - layout_box.rect.y;
    let content_inset_bottom = (layout_box.rect.y + layout_box.rect.height)
        - (layout_box.content.y + layout_box.content.height);
    let orphans = (layout_box.style.orphans as usize).max(1);
    let widows = (layout_box.style.widows as usize).max(1);

    let mut start = 0usize;
    let mut guard = 0usize;
    while start < n {
        guard += 1;
        if guard > n * 2 + 4 {
            break; // defensive: never loop forever on degenerate geometry
        }
        let page_bottom_abs = ctx.page_y_offset + ctx.page_height;
        let content_top_abs = layout_box.content.y + top_of(start);
        // How many lines from `start` fit above the page bottom?
        let mut fit = 0usize;
        while start + fit < n
            && layout_box.content.y + top_of(start + fit) + line_height <= page_bottom_abs + 0.01
        {
            fit += 1;
        }
        let remaining_after = n - start - fit;

        // Orphans: too few lines would be left at the bottom of this page.
        if fit > 0
            && fit < orphans.min(n - start)
            && remaining_after > 0
            && !ctx.is_current_page_empty()
        {
            fit = 0;
        }
        // Widows: keep at least `widows` lines for the next page.
        if fit > 0 && remaining_after > 0 && remaining_after < widows {
            let needed = widows - remaining_after;
            fit = if fit > needed { fit - needed } else { fit };
            if fit < orphans.min(fit + needed) && !ctx.is_current_page_empty() {
                fit = 0;
            }
        }
        if fit == 0 {
            if ctx.is_current_page_empty() {
                fit = 1; // a line taller than the page: place it anyway
            } else {
                ctx.flush_page(content_top_abs);
                continue;
            }
        }

        let end = start + fit;
        let is_first = start == 0;
        let is_last = end == n;
        let shift = top_of(start);

        let mut fragment = layout_box.clone();
        fragment.children.clear();
        fragment.shaped_lines = lines[start..end]
            .iter()
            .cloned()
            .map(|mut l| {
                l.y -= shift;
                l
            })
            .collect();
        let content_height = if is_last {
            (layout_box.content.height - shift).max(0.0)
        } else {
            top_of(end) - shift
        };
        fragment.content.y = content_top_abs;
        fragment.content.height = content_height;
        fragment.rect.y = if is_first {
            layout_box.rect.y
        } else {
            content_top_abs
        };
        fragment.rect.height = (if is_first { content_inset_top } else { 0.0 })
            + content_height
            + if is_last { content_inset_bottom } else { 0.0 };
        place_box_on_current_page(&fragment, ctx);

        start = end;
        if start < n {
            ctx.flush_page(layout_box.content.y + top_of(start));
        }
    }
}

// =============================================================================
// TABLE FRAGMENTATION
// Table-aware pagination: rows are atomic (never split), and <thead> is
// repeated on each continuation page.
// =============================================================================

fn fragment_table(table_box: &LayoutBox, ctx: &mut PaginationContext) {
    use std::collections::BTreeMap;

    let thead_row_count = table_box.thead_row_count;

    // Group cells by row index using table_cell_pos
    let mut rows: BTreeMap<usize, Vec<&LayoutBox>> = BTreeMap::new();
    for child in &table_box.children {
        if let Some((row, _col, _total_r, _total_c)) = child.table_cell_pos {
            rows.entry(row).or_default().push(child);
        }
    }

    // Compute row bounds (min_y, max_bottom) for each row
    struct RowInfo {
        row_idx: usize,
        top: f32,
        bottom: f32,
        cells: Vec<LayoutBox>,
    }

    let row_infos: Vec<RowInfo> = rows
        .iter()
        .map(|(&row_idx, cells)| {
            let top = cells.iter().map(|c| c.rect.y).fold(f32::MAX, f32::min);
            let bottom = cells
                .iter()
                .map(|c| c.rect.y + c.rect.height)
                .fold(0.0f32, f32::max);
            RowInfo {
                row_idx,
                top,
                bottom,
                cells: cells.iter().map(|c| (*c).clone()).collect(),
            }
        })
        .collect();

    // Separate thead rows and body rows
    let thead_rows: Vec<&RowInfo> = row_infos
        .iter()
        .filter(|r| r.row_idx < thead_row_count)
        .collect();
    let body_rows: Vec<&RowInfo> = row_infos
        .iter()
        .filter(|r| r.row_idx >= thead_row_count)
        .collect();

    let thead_height: f32 = if !thead_rows.is_empty() {
        let thead_top = thead_rows.iter().map(|r| r.top).fold(f32::MAX, f32::min);
        let thead_bottom = thead_rows.iter().map(|r| r.bottom).fold(0.0f32, f32::max);
        thead_bottom - thead_top
    } else {
        0.0
    };

    let mut current_page_children: Vec<LayoutBox> = Vec::new();
    let mut is_first_page = true;

    // On the first page, thead cells are placed naturally
    for row in &thead_rows {
        for cell in &row.cells {
            let mut placed = cell.clone();
            offset_y_recursive(&mut placed, -ctx.page_y_offset);
            current_page_children.push(placed);
        }
        let row_bottom_on_page = (row.bottom - ctx.page_y_offset).max(0.0);
        ctx.used_height = ctx.used_height.max(row_bottom_on_page);
    }

    for body_row in &body_rows {
        let row_bottom_on_page =
            (body_row.bottom - ctx.page_y_offset) + if !is_first_page { thead_height } else { 0.0 };
        let row_fits = row_bottom_on_page <= ctx.page_height;

        if row_fits {
            for cell in &body_row.cells {
                let mut placed = cell.clone();
                offset_y_recursive(&mut placed, -ctx.page_y_offset);
                if !is_first_page && thead_height > 0.0 {
                    offset_y_recursive(&mut placed, thead_height);
                }
                current_page_children.push(placed);
            }
            ctx.used_height = ctx.used_height.max(row_bottom_on_page);
        } else {
            // Flush current page with table wrapper
            if !current_page_children.is_empty() || !ctx.is_current_page_empty() {
                if !current_page_children.is_empty() {
                    let wrapper = make_container_fragment(
                        table_box,
                        &current_page_children,
                        ctx,
                        is_first_page,
                        false,
                    );
                    ctx.current_page_boxes.push(wrapper);
                    current_page_children.clear();
                }
                ctx.flush_page(body_row.top);
                is_first_page = false;
            }

            // On continuation pages, repeat the thead cells
            if !thead_rows.is_empty() {
                for thead_row in &thead_rows {
                    for cell in &thead_row.cells {
                        let mut repeated = cell.clone();
                        // Position at Y=0 on the new page
                        let cell_dy = -repeated.rect.y;
                        offset_y_recursive(&mut repeated, cell_dy);
                        current_page_children.push(repeated);
                    }
                }
                ctx.used_height = thead_height;
            }

            // Place body row cells, shifted down by thead_height
            for cell in &body_row.cells {
                let mut placed = cell.clone();
                offset_y_recursive(&mut placed, -ctx.page_y_offset);
                if thead_height > 0.0 {
                    offset_y_recursive(&mut placed, thead_height);
                }
                current_page_children.push(placed);
            }
            let row_bottom = (body_row.bottom - ctx.page_y_offset) + thead_height;
            ctx.used_height = ctx.used_height.max(row_bottom);
        }
    }

    // Flush remaining cells
    if !current_page_children.is_empty() {
        let wrapper =
            make_container_fragment(table_box, &current_page_children, ctx, is_first_page, true);
        ctx.current_page_boxes.push(wrapper);
    }
}

/// Create a container fragment (partial copy of the parent) that wraps
/// a subset of children for one page. Preserves background, borders, etc.
fn make_container_fragment(
    parent: &LayoutBox,
    children: &[LayoutBox],
    ctx: &PaginationContext,
    is_first_fragment: bool,
    is_last_fragment: bool,
) -> LayoutBox {
    // Compute bounding box of children on this page
    let min_y = children.iter().map(|c| c.rect.y).fold(f32::MAX, f32::min);
    let max_bottom = children
        .iter()
        .map(|c| c.rect.y + c.rect.height)
        .fold(0.0f32, f32::max);
    let fragment_height = max_bottom - min_y
        + if is_first_fragment {
            parent.padding.top + parent.border.top
        } else {
            0.0
        }
        + if is_last_fragment {
            parent.padding.bottom + parent.border.bottom
        } else {
            0.0
        };

    let page_rel_y = (parent.rect.y - ctx.page_y_offset).max(0.0);
    let y = if is_first_fragment { page_rel_y } else { 0.0 };

    let rect = Rect::new(parent.rect.x, y, parent.rect.width, fragment_height);
    let content = Rect::new(
        parent.content.x,
        y + if is_first_fragment {
            parent.padding.top + parent.border.top
        } else {
            0.0
        },
        parent.content.width,
        (fragment_height - parent.padding.vertical() - parent.border.vertical()).max(0.0),
    );

    LayoutBox {
        node_id: parent.node_id,
        style: parent.style.clone(),
        rect,
        content,
        padding: if is_first_fragment {
            parent.padding
        } else {
            Insets {
                top: 0.0,
                ..parent.padding
            }
        },
        border: if is_first_fragment {
            parent.border
        } else {
            Insets {
                top: 0.0,
                ..parent.border
            }
        },
        margin: Insets::zero(),
        children: children.to_vec(),
        shaped_lines: Vec::new(),
        inline_spans: Vec::new(),
        image_src: None,
        text_content: None,
        out_of_flow: false,
        visual_offset_x: 0.0,
        visual_offset_y: 0.0,
        table_cell_pos: None,
        list_item_index: None,
        thead_row_count: 0,
    }
}

// =============================================================================
// PLACING A BOX ON THE CURRENT PAGE
// Y repositioning: y_page = y_absolute - page_y_offset
// =============================================================================

fn place_box_on_current_page(layout_box: &LayoutBox, ctx: &mut PaginationContext) {
    let mut placed_box = layout_box.clone();
    offset_y_recursive(&mut placed_box, -ctx.page_y_offset);

    // Track the actual bottom extent on the page (not sum of heights)
    let box_bottom = placed_box.rect.y + placed_box.rect.height;
    ctx.used_height = ctx.used_height.max(box_bottom);
    ctx.current_page_boxes.push(placed_box);
}

/// Recursively offset all Y coordinates in a LayoutBox tree.
fn offset_y_recursive(layout_box: &mut LayoutBox, dy: f32) {
    layout_box.rect.y += dy;
    layout_box.content.y += dy;
    for child in &mut layout_box.children {
        offset_y_recursive(child, dy);
    }
}

// =============================================================================
// HELPERS — CSS fragmentation rule detection
// =============================================================================

fn should_break_before(style: &ferropdf_core::ComputedStyle) -> bool {
    matches!(
        style.page_break_before,
        PageBreak::Always | PageBreak::Page | PageBreak::Left | PageBreak::Right
    )
}

fn should_break_after(style: &ferropdf_core::ComputedStyle) -> bool {
    matches!(
        style.page_break_after,
        PageBreak::Always | PageBreak::Page | PageBreak::Left | PageBreak::Right
    )
}

/// Creates an empty page.
pub fn create_empty_page(_config: &PageConfig) -> Page {
    Page {
        page_number: 1,
        total_pages: 1,
        content: Vec::new(),
        margin_boxes: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferropdf_core::{ComputedStyle, Orientation, PageMargins, PageSize};

    fn make_config(page_height: f32) -> PageConfig {
        PageConfig {
            size: PageSize::Custom(595.0, page_height + 100.0), // +margins
            margins: PageMargins {
                top: 50.0,
                right: 50.0,
                bottom: 50.0,
                left: 50.0,
            },
            orientation: Orientation::Portrait,
        }
    }

    fn make_box(y: f32, height: f32) -> LayoutBox {
        LayoutBox {
            rect: Rect::new(0.0, y, 400.0, height),
            content: Rect::new(0.0, y, 400.0, height),
            style: ComputedStyle::default(),
            ..Default::default()
        }
    }

    fn make_root(children: Vec<LayoutBox>) -> LayoutBox {
        let total_height = children
            .iter()
            .map(|c| c.rect.y + c.rect.height)
            .fold(0.0f32, f32::max);
        LayoutBox {
            rect: Rect::new(0.0, 0.0, 400.0, total_height),
            content: Rect::new(0.0, 0.0, 400.0, total_height),
            children,
            ..Default::default()
        }
    }

    fn make_text_box(y: f32, lines: usize, line_height: f32) -> LayoutBox {
        let mut b = make_box(y, lines as f32 * line_height);
        b.shaped_lines = (0..lines)
            .map(|i| ferropdf_core::ShapedLine {
                glyphs: Vec::new(),
                width: 100.0,
                y: i as f32 * line_height + line_height * 0.8,
                text: format!("line {i}"),
                segments: Vec::new(),
            })
            .collect();
        b
    }

    #[test]
    fn tall_paragraph_is_split_between_lines() {
        // 100 lines of 20pt in a 300pt page → 15 lines per page, 7 pages.
        let config = make_config(300.0);
        let root = make_root(vec![make_text_box(0.0, 100, 20.0)]);
        let pages = paginate(&root, &config);
        assert_eq!(pages.len(), 7);
        let total: usize = pages
            .iter()
            .flat_map(|p| p.content.iter())
            .map(|b| b.shaped_lines.len())
            .sum();
        assert_eq!(total, 100, "no line may be lost or duplicated");
        // Every fragment's lines are re-based to its own content box.
        for page in &pages {
            for b in &page.content {
                if let Some(first) = b.shaped_lines.first() {
                    assert!((first.y - 16.0).abs() < 0.01);
                    assert!(b.content.y >= -0.01 && b.content.y + b.content.height <= 300.01);
                }
            }
        }
    }

    #[test]
    fn paragraph_split_respects_widows() {
        // 16 lines, 15 fit: a single widow would be left → move 1 more line.
        let config = make_config(300.0);
        let root = make_root(vec![make_text_box(0.0, 16, 20.0)]);
        let pages = paginate(&root, &config);
        let per_page: Vec<usize> = pages
            .iter()
            .map(|p| p.content.iter().map(|b| b.shaped_lines.len()).sum())
            .collect();
        assert_eq!(per_page, vec![14, 2]);
    }

    #[test]
    fn single_box_fits_one_page() {
        let config = make_config(800.0);
        let root = make_root(vec![make_box(0.0, 100.0)]);
        let pages = paginate(&root, &config);
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].content.len(), 1);
    }

    #[test]
    fn two_boxes_overflow_to_two_pages() {
        let config = make_config(200.0);
        let root = make_root(vec![make_box(0.0, 150.0), make_box(150.0, 150.0)]);
        let pages = paginate(&root, &config);
        assert_eq!(pages.len(), 2, "expected 2 pages, got {}", pages.len());
    }

    #[test]
    fn page_break_before_forces_new_page() {
        let config = make_config(800.0);
        let mut box2 = make_box(100.0, 50.0);
        box2.style.page_break_before = PageBreak::Always;
        let root = make_root(vec![make_box(0.0, 100.0), box2]);
        let pages = paginate(&root, &config);
        assert_eq!(pages.len(), 2);
    }

    #[test]
    fn page_break_after_forces_new_page() {
        let config = make_config(800.0);
        let mut box1 = make_box(0.0, 100.0);
        box1.style.page_break_after = PageBreak::Always;
        let root = make_root(vec![box1, make_box(100.0, 50.0)]);
        let pages = paginate(&root, &config);
        assert_eq!(pages.len(), 2);
    }

    #[test]
    fn page_break_inside_avoid_pushes_to_next() {
        let config = make_config(200.0);
        // First box uses 150px, second box is 100px tall with avoid → doesn't fit,
        // should be pushed to next page
        let mut box2 = make_box(150.0, 100.0);
        box2.style.page_break_inside = PageBreakInside::Avoid;
        let root = make_root(vec![make_box(0.0, 150.0), box2]);
        let pages = paginate(&root, &config);
        assert_eq!(pages.len(), 2);
    }

    #[test]
    fn empty_root_produces_one_page() {
        let config = make_config(800.0);
        let root = make_root(vec![]);
        let pages = paginate(&root, &config);
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].page_number, 1);
        assert_eq!(pages[0].total_pages, 1);
    }

    #[test]
    fn total_pages_updated_correctly() {
        let config = make_config(100.0);
        let root = make_root(vec![
            make_box(0.0, 80.0),
            make_box(80.0, 80.0),
            make_box(160.0, 80.0),
        ]);
        let pages = paginate(&root, &config);
        assert!(pages.len() >= 2);
        for page in &pages {
            assert_eq!(page.total_pages, pages.len() as u32);
        }
    }
}
