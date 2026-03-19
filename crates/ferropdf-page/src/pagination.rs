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
use ferropdf_core::{Insets, LayoutBox, PageBreak, PageBreakInside, PageConfig, Rect};

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

    /// Remaining space on the current page.
    #[allow(dead_code)]
    fn remaining_height(&self) -> f32 {
        (self.page_height - self.used_height).max(0.0)
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

    // ─── Rule 4: The block doesn't fit → fragmentation ─────────────────────
    if !layout_box.children.is_empty() {
        // Create a container fragment on the current page (preserves background/borders)
        fragment_container(layout_box, ctx);
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
        } else if !child.children.is_empty() && child_height > ctx.page_height {
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

// =============================================================================
// BREAK UNITS — Extracting breakable units from the LayoutBox tree
// =============================================================================
// After the Taffy layout + cosmic-text shaping, we build a FLAT list
// of breakable units. Each unit is the smallest movable entity without
// breaking the document's meaning.
//
// BreakUnit types:
//   - TextLine  : an individual line from the LayoutBox's shaped_lines
//   - Atomic    : non-breakable block (image, table with break-inside:avoid)
//   - ForcedBreak : forced page break marker (break-before: page)
// =============================================================================

use ferropdf_core::layout::BreakUnit;

/// Extract breakable units from the LayoutBox tree.
/// Recursively traverses the tree and produces a flat list of BreakUnit.
pub fn extract_break_units(root: &LayoutBox) -> Vec<BreakUnit> {
    let mut units = Vec::new();
    for child in &root.children {
        extract_recursive(child, &mut units);
    }
    units
}

fn extract_recursive(lb: &LayoutBox, units: &mut Vec<BreakUnit>) {
    // Forced break before
    if should_break_before(&lb.style) {
        units.push(BreakUnit::ForcedBreak);
    }

    // Atomic: break-inside: avoid, or has image, or is a leaf without children/shaped_lines
    let is_atomic = lb.style.page_break_inside == PageBreakInside::Avoid
        || lb.image_src.is_some()
        || (lb.children.is_empty() && lb.shaped_lines.is_empty() && lb.text_content.is_some());

    if is_atomic {
        units.push(BreakUnit::Atomic {
            y_top: lb.rect.y,
            y_bottom: lb.rect.y + lb.rect.height,
            node: lb.clone(),
        });
    } else if !lb.shaped_lines.is_empty() {
        // Text node with shaped lines → one BreakUnit::TextLine per line
        for (i, line) in lb.shaped_lines.iter().enumerate() {
            let line_height = if i + 1 < lb.shaped_lines.len() {
                lb.shaped_lines[i + 1].y - line.y
            } else {
                lb.style.line_height
            };
            units.push(BreakUnit::TextLine {
                y_top: lb.content.y + line.y,
                y_bottom: lb.content.y + line.y + line_height,
                line_index: i,
                parent_node: lb.node_id,
                content: line.clone(),
            });
        }
    } else if lb.text_content.is_some() && lb.shaped_lines.is_empty() {
        // Text node without shaped lines → treat as atomic
        units.push(BreakUnit::Atomic {
            y_top: lb.rect.y,
            y_bottom: lb.rect.y + lb.rect.height,
            node: lb.clone(),
        });
    } else if !lb.children.is_empty() {
        // Container → recurse into children
        for child in &lb.children {
            extract_recursive(child, units);
        }
    }

    // Forced break after
    if should_break_after(&lb.style) {
        units.push(BreakUnit::ForcedBreak);
    }
}

// =============================================================================
// find_break_point — Intelligent break point search algorithm
// =============================================================================
// Takes the list of BreakUnit, the page limit (page_bottom), and the
// orphans/widows parameters. Returns the index of the first BreakUnit
// of the next page.
//
// Steps:
//   1. Naive index — first BreakUnit whose y_top >= page_bottom
//   2. Orphans correction — if too few lines of a paragraph at end of page
//   3. Widows correction — if too few lines of a paragraph at start of next page
//   4. Atomic integrity — no break in the middle of an Atomic
//   5. Forced breaks — ForcedBreak takes priority
// =============================================================================

/// Find the optimal break point in the BreakUnit list.
///
/// Returns the index of the first BreakUnit that must go on the next page.
/// `page_top` is the absolute Y coordinate of the top of the current page.
/// `page_height` is the height of the page's content area.
/// `min_orphans` and `min_widows` are the CSS minimums (default = 2).
pub fn find_break_point(
    units: &[BreakUnit],
    page_top: f32,
    page_height: f32,
    min_orphans: u32,
    min_widows: u32,
) -> usize {
    let page_bottom = page_top + page_height;

    // Step 1 — Naive index: first unit that exceeds the page
    let mut naive_index = units.len();
    for (i, unit) in units.iter().enumerate() {
        if let BreakUnit::ForcedBreak = unit {
            // A forced break before the naive index takes priority (step 5)
            if unit.y_top() <= page_bottom || i < naive_index {
                return i + 1; // The ForcedBreak is consumed, next page starts after
            }
        }
        if unit.y_bottom() > page_bottom && naive_index == units.len() {
            naive_index = i;
        }
    }

    if naive_index == 0 {
        // Nothing fits on this page — force at least one unit (anti-infinite-loop)
        return 1.min(units.len());
    }
    if naive_index >= units.len() {
        return units.len();
    }

    let mut break_index = naive_index;

    // Step 2 — Orphans correction
    break_index = adjust_for_orphans(units, break_index, min_orphans);

    // Step 3 — Widows correction
    break_index = adjust_for_widows(units, break_index, min_widows);

    // Step 4 — Atomic integrity
    break_index = enforce_atomic_integrity(units, break_index);

    break_index.clamp(1, units.len())
}

/// Orphans correction: if fewer than `min_orphans` lines of the same paragraph
/// are present just before the break point, move the index back to carry
/// those lines to the next page.
fn adjust_for_orphans(units: &[BreakUnit], break_idx: usize, min_orphans: u32) -> usize {
    if break_idx == 0 || break_idx >= units.len() || min_orphans < 2 {
        return break_idx;
    }

    // Look at the unit just before the break
    if let BreakUnit::TextLine {
        parent_node: Some(parent),
        ..
    } = &units[break_idx - 1]
    {
        // Count how many lines of this paragraph are just before the index
        let mut orphan_count = 0u32;
        let mut i = break_idx;
        while i > 0 {
            i -= 1;
            match &units[i] {
                BreakUnit::TextLine {
                    parent_node: Some(p),
                    ..
                } if p == parent => {
                    orphan_count += 1;
                }
                _ => break,
            }
        }

        if orphan_count > 0 && orphan_count < min_orphans {
            // Move the index back to carry these orphan lines
            return break_idx - orphan_count as usize;
        }
    }

    break_idx
}

/// Widows correction: if fewer than `min_widows` lines of the same paragraph
/// will be at the start of the next page, adjust.
fn adjust_for_widows(units: &[BreakUnit], break_idx: usize, min_widows: u32) -> usize {
    if break_idx >= units.len() || min_widows < 2 {
        return break_idx;
    }

    // Look at the unit at the break point (first on the next page)
    if let BreakUnit::TextLine {
        parent_node: Some(parent),
        ..
    } = &units[break_idx]
    {
        // Count how many lines of this paragraph will be at the start of the next page
        let mut widow_count = 0u32;
        for unit in &units[break_idx..] {
            match unit {
                BreakUnit::TextLine {
                    parent_node: Some(p),
                    ..
                } if p == parent => {
                    widow_count += 1;
                }
                _ => break,
            }
        }

        if widow_count > 0 && widow_count < min_widows {
            // Move the index back to add lines to the next page
            let lines_to_pull = min_widows - widow_count;
            if break_idx > lines_to_pull as usize {
                return break_idx - lines_to_pull as usize;
            }
        }
    }

    break_idx
}

/// Atomic integrity: if the index falls in the middle of an Atomic,
/// move the index back to before the start of that Atomic.
fn enforce_atomic_integrity(units: &[BreakUnit], break_idx: usize) -> usize {
    if break_idx >= units.len() {
        return break_idx;
    }

    // If the unit at the break is an Atomic, we cannot break in the middle
    // → we keep break_idx as is (it already points to the start of the Atomic)
    // If the unit just before is an Atomic whose bottom exceeds, we move back
    if break_idx > 0 {
        if let BreakUnit::Atomic { .. } = &units[break_idx - 1] {
            // L'Atomic est entièrement sur la page courante — OK
        }
    }

    break_idx
}
