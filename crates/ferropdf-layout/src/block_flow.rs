// =============================================================================
// block_flow.rs — Block Flow Layout Algorithm (CSS 2.1 §8.3.1)
// =============================================================================
// Translated from Blink's algorithms (BSD licence):
//   blink/renderer/core/layout/layout_block_flow.cc
//   blink/renderer/core/layout/layout_block.cc
//
// This module runs AFTER Taffy as a post-pass.
// Taffy computes dimensions (width, height, padding, border).
// This module corrects Y positions for margin collapsing
// and applies position: relative/absolute.
// =============================================================================

use ferropdf_core::{ComputedStyle, Display as FDisplay, LayoutBox, LayoutTree, Length, Position};

// =============================================================================
// DATA STRUCTURES
// =============================================================================

/// Block formatting context — stores the current layout state.
/// Inspired by BlockFormattingContext in Blink LayoutNG.
#[derive(Debug)]
struct BlockFormattingContext {
    /// Current Y position in the flow (vertical cursor)
    current_y: f32,
    /// Bottom margin of the previous block (for margin collapsing)
    pending_margin_bottom: f32,
    /// Width of the containing block
    containing_width: f32,
}

impl BlockFormattingContext {
    fn new(containing_width: f32) -> Self {
        Self {
            current_y: 0.0,
            pending_margin_bottom: 0.0,
            containing_width,
        }
    }
}

// =============================================================================
// MAIN ENTRY POINT
// =============================================================================

/// Applies block flow layout on an existing LayoutTree.
/// Corrects Y positions after the Taffy pass.
pub fn apply_block_flow(layout_tree: &mut LayoutTree, page_width: f32) {
    if let Some(ref mut root) = layout_tree.root {
        let mut ctx = BlockFormattingContext::new(page_width);
        layout_block_children(root, &mut ctx);
    }
}

// =============================================================================
// BLOCK CHILDREN LAYOUT — RECURSIVE VERSION
// Inspired by LayoutBlockFlow::LayoutBlockChild() in Blink
// =============================================================================

/// Applies margin collapsing + positioning on the children of a layout_box.
fn layout_block_children(parent: &mut LayoutBox, ctx: &mut BlockFormattingContext) {
    // Block flow only applies to block-level containers.
    // Skip tables (grid), flex, inline — their children are positioned by Taffy.
    match parent.style.display {
        FDisplay::Table
        | FDisplay::TableRow
        | FDisplay::TableCell
        | FDisplay::TableHeaderGroup
        | FDisplay::TableRowGroup
        | FDisplay::TableFooterGroup
        | FDisplay::Flex
        | FDisplay::Grid
        | FDisplay::Inline
        | FDisplay::InlineBlock => {
            return;
        }
        _ => {}
    }

    let parent_y = parent.rect.y;
    let parent_height = parent.rect.height;
    let parent_width = ctx.containing_width;

    // If the parent has border-top or padding-top, it creates a new BFC
    // → children's margins do not collapse with the outside
    let creates_new_bfc = parent.border.top > 0.0
        || resolve_length_to_px(&parent.style.padding[0], parent_width) > 0.0;

    let mut child_ctx = if creates_new_bfc {
        // New BFC: cursor starts from the inner edge of the parent (after padding-top)
        let mut bfc = BlockFormattingContext::new(parent_width);
        bfc.current_y = parent_y + parent.border.top + parent.padding.top;
        bfc
    } else {
        // No own BFC: use the parent's context
        let mut bfc = BlockFormattingContext::new(parent_width);
        bfc.current_y = parent_y;
        bfc.pending_margin_bottom = ctx.pending_margin_bottom;
        bfc
    };

    for child in &mut parent.children {
        if child.out_of_flow {
            continue;
        }

        // Skip zero-height whitespace text nodes — they don't participate in block flow
        // and would incorrectly reset pending_margin_bottom (CSS 2.1 §9.2.1.1)
        if child.rect.height < 0.5 && child.text_content.is_some() {
            let is_ws = child
                .text_content
                .as_ref()
                .map(|t| t.trim().is_empty())
                .unwrap_or(false);
            if is_ws {
                continue;
            }
        }

        // Extract style values before mutating child (borrow checker)
        let margin_top = resolve_length_to_px(&child.style.margin[0], child_ctx.containing_width);
        let margin_bottom =
            resolve_length_to_px(&child.style.margin[2], child_ctx.containing_width);
        let is_relative = child.style.position == Position::Relative;
        let block_height = child.rect.height;
        let is_empty = is_empty_block(&child.style, block_height);

        // ─── MARGIN COLLAPSING ──────────────────────────────────────────
        // CSS 2.1 §8.3.1: the effective margin between two siblings is
        // max(previous_margin_bottom, current_margin_top)
        // and NOT their sum.
        let effective_top_margin = collapse_margins(child_ctx.pending_margin_bottom, margin_top);

        // Position the block at current_y + effective margin
        let block_y = child_ctx.current_y + effective_top_margin;

        // Update the LayoutBox Y position
        let dy = block_y - child.rect.y;
        if dy.abs() > 0.001 && std::env::var("FERROPDF_DEBUG").is_ok() {
            let _tag = child.node_id.map(|_| "").unwrap_or("");
            let text = child
                .text_content
                .as_deref()
                .unwrap_or("")
                .chars()
                .take(20)
                .collect::<String>();
            eprintln!(
                "[block_flow] MOVE dy={:.1} old_y={:.1} new_y={:.1} text=\"{}\"",
                dy, child.rect.y, block_y, text
            );
        }
        child.rect.y = block_y;
        child.content.y += dy;

        // Propagate Y shift to all descendants.
        // Children have absolute coordinates from read_layout, based on the old
        // parent position. We must shift them by the same dy so they stay
        // correctly positioned relative to the parent.
        if dy.abs() > 0.001 {
            shift_subtree_y(&mut child.children, dy);
        }

        // Recurse into children — for block containers, this recalculates
        // children positions (overriding the shift above when appropriate).
        layout_block_children(child, &mut child_ctx);

        // ─── POSITION: RELATIVE ──────────────────────────────────────────
        if is_relative {
            apply_relative_position(child, parent_width, parent_height);
        }

        // Advance the Y cursor
        child_ctx.current_y = block_y + block_height;

        // Store the bottom margin for the next sibling
        child_ctx.pending_margin_bottom = margin_bottom;

        // ─── EMPTY BLOCK MARGIN COLLAPSING ──────────────────────────────
        // CSS 2.1 §8.3.1 case 4
        if is_empty {
            child_ctx.pending_margin_bottom = collapse_margins(margin_top, margin_bottom);
            child_ctx.current_y = block_y;
        }
    }

    // Propagate pending_margin_bottom back to parent context if no BFC boundary
    if !creates_new_bfc {
        ctx.pending_margin_bottom = child_ctx.pending_margin_bottom;
    }
}

/// Recursively shift all descendants' Y positions by `dy`.
/// Used when block_flow repositions a parent — children with absolute positions
/// (computed from read_layout using the old parent position) must be updated.
fn shift_subtree_y(children: &mut [LayoutBox], dy: f32) {
    for child in children.iter_mut() {
        child.rect.y += dy;
        child.content.y += dy;
        shift_subtree_y(&mut child.children, dy);
    }
}

// =============================================================================
// MARGIN COLLAPSING ALGORITHM
// CSS 2.1 §8.3.1 — translated from Blink CollapseMargins()
// =============================================================================

/// Collapses two adjacent vertical margins.
/// - Both positive → max(a, b)
/// - Both negative → min(a, b)
/// - One positive, one negative → algebraic sum
fn collapse_margins(margin_a: f32, margin_b: f32) -> f32 {
    match (margin_a >= 0.0, margin_b >= 0.0) {
        (true, true) => margin_a.max(margin_b),
        (false, false) => margin_a.min(margin_b),
        _ => margin_a + margin_b,
    }
}

// =============================================================================
// POSITIONING (position: relative)
// Inspired by LayoutBox::ApplyRelativePositionIfNeeded() in Blink
// CSS 2.1 §9.4.3
// =============================================================================

fn apply_relative_position(
    layout_box: &mut LayoutBox,
    containing_width: f32,
    containing_height: f32,
) {
    let style = &layout_box.style;

    let offset_left = resolve_length_to_px(&style.left, containing_width);
    let offset_right = resolve_length_to_px(&style.right, containing_width);
    let offset_top = resolve_length_to_px(&style.top, containing_height);
    let offset_bottom = resolve_length_to_px(&style.bottom, containing_height);

    // If both left and right are specified, left wins (LTR)
    let dx = if style.left != Length::Auto {
        offset_left
    } else if style.right != Length::Auto {
        -offset_right
    } else {
        0.0
    };

    let dy = if style.top != Length::Auto {
        offset_top
    } else if style.bottom != Length::Auto {
        -offset_bottom
    } else {
        0.0
    };

    layout_box.visual_offset_x = dx;
    layout_box.visual_offset_y = dy;
}

// =============================================================================
// HELPERS
// =============================================================================

/// Checks if a block is "empty" in the CSS margin collapsing sense.
/// CSS 2.1 §8.3.1 case 4
fn is_empty_block(style: &ComputedStyle, height: f32) -> bool {
    if height != 0.0 {
        return false;
    }
    let padding_top = resolve_length_to_px(&style.padding[0], 0.0);
    let padding_bottom = resolve_length_to_px(&style.padding[2], 0.0);

    padding_top == 0.0
        && padding_bottom == 0.0
        && style.border_top.width == 0.0
        && style.border_bottom.width == 0.0
}

/// Converts a CSS Length value to absolute pixels.
fn resolve_length_to_px(length: &Length, containing_width: f32) -> f32 {
    match length {
        Length::Pt(v) => *v,
        Length::Px(px) => *px,
        Length::Percent(p) => containing_width * p / 100.0,
        Length::Em(em) => em * 16.0,
        Length::Rem(rem) => rem * 16.0,
        Length::Auto => 0.0,
        Length::Zero => 0.0,
        _ => 0.0,
    }
}

// =============================================================================
// UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collapse_two_positives() {
        assert_eq!(collapse_margins(30.0, 20.0), 30.0);
    }

    #[test]
    fn test_collapse_two_negatives() {
        assert_eq!(collapse_margins(-10.0, -20.0), -20.0);
    }

    #[test]
    fn test_collapse_positive_negative() {
        assert_eq!(collapse_margins(30.0, -10.0), 20.0);
    }

    #[test]
    fn test_collapse_symmetric() {
        assert_eq!(collapse_margins(48.0, 48.0), 48.0);
    }

    #[test]
    fn test_collapse_zero() {
        assert_eq!(collapse_margins(0.0, 20.0), 20.0);
    }
}
