pub mod style_to_taffy;
pub mod table_layout;
mod taffy_bridge;
pub mod text;

use ferropdf_core::{Display, Document, InlineSpan, LayoutBox, LayoutTree};
use ferropdf_style::StyleTree;
pub use text::FontDatabase;

/// Build a layout tree from a styled document using Taffy for layout computation.
/// All coordinates are in typographic points (pt).
pub fn layout(
    document: &Document,
    styles: &StyleTree,
    available_width: f32,
    available_height: f32,
) -> ferropdf_core::Result<LayoutTree> {
    let owned_font_db = FontDatabase::new();
    layout_with_fonts(
        document,
        styles,
        available_width,
        available_height,
        &owned_font_db,
    )
}

/// Same as `layout` but reuses an existing FontDatabase (avoids reloading system fonts).
pub fn layout_with_fonts(
    document: &Document,
    styles: &StyleTree,
    available_width: f32,
    available_height: f32,
    font_db: &FontDatabase,
) -> ferropdf_core::Result<LayoutTree> {
    let mut font_system = font_db.font_system_mut();
    let mut tree = taffy_bridge::build_layout(
        document,
        styles,
        &mut font_system,
        available_width,
        available_height,
    )?;

    // NOTE: block_flow post-pass disabled.
    // Taffy 0.5 with block_layout feature handles margin collapsing natively.
    // The post-pass was propagating pending margins through containers without
    // BFC boundaries, causing cascading Y shifts and text overlap.
    // block_flow::apply_block_flow(&mut tree, available_width);

    // Post-pass: shape text at final content widths to populate shaped_lines.
    // This is the SINGLE SOURCE OF TRUTH for line breaks — pdf.rs must not re-wrap.
    if let Some(ref mut root) = tree.root {
        populate_shaped_lines(root, &mut font_system);
    }

    drop(font_system);

    Ok(tree)
}

/// Walk the LayoutBox tree and populate `shaped_lines` for each text node
/// by re-shaping text at the box's final content width.
///
/// When a block container has all-inline children (text + `<strong>`, `<em>`, etc.),
/// their text is merged into a single rich-text buffer and shaped as one paragraph.
/// This ensures correct word-wrapping across span boundaries.
fn populate_shaped_lines(lb: &mut LayoutBox, fs: &mut cosmic_text::FontSystem) {
    // Check if this block should merge its inline children
    if should_merge_inline_children(lb) {
        merge_inline_children(lb, fs);
        return;
    }

    if let Some(ref txt) = lb.text_content {
        if !txt.trim().is_empty() && lb.shaped_lines.is_empty() {
            let style = &lb.style;
            let font_family = style.font_family.first().map(|s| s.as_str()).unwrap_or("");
            lb.shaped_lines = text::shape_text_lines(
                txt,
                style.font_size,
                style.line_height,
                font_family,
                style.font_weight.is_bold(),
                style.font_style == ferropdf_core::FontStyle::Italic,
                lb.content.width,
                fs,
            );
        }
    }
    for child in &mut lb.children {
        populate_shaped_lines(child, fs);
    }
}

/// Check if a LayoutBox is a block container whose children are all inline content.
/// Returns true if inline merging should be applied.
fn should_merge_inline_children(lb: &LayoutBox) -> bool {
    // Must not be a text leaf itself
    if lb.text_content.is_some() {
        return false;
    }
    if lb.children.is_empty() {
        return false;
    }
    // All children must be inline-mergeable
    let parent_fs = lb.style.font_size;
    lb.children
        .iter()
        .all(|child| is_mergeable_inline(child, parent_fs))
}

/// Check if a LayoutBox can be merged as inline content.
fn is_mergeable_inline(lb: &LayoutBox, parent_fs: f32) -> bool {
    if lb.is_text_leaf() {
        // Text leaf with same font size as parent — mergeable
        return (lb.style.font_size - parent_fs).abs() < 0.1;
    }
    // Inline element whose children are all mergeable
    if lb.style.display != Display::Inline {
        return false;
    }
    lb.children
        .iter()
        .all(|child| is_mergeable_inline(child, parent_fs))
}

/// Merge inline children into a single rich-text buffer on the parent LayoutBox.
fn merge_inline_children(lb: &mut LayoutBox, fs: &mut cosmic_text::FontSystem) {
    let mut spans = Vec::new();
    // Single pass: collect spans and clear text content simultaneously
    for child in &mut lb.children {
        collect_and_clear(child, &mut spans);
    }

    if spans.is_empty() || spans.iter().all(|s| s.text.trim().is_empty()) {
        // Nothing to merge — recurse normally
        for child in &mut lb.children {
            populate_shaped_lines(child, fs);
        }
        return;
    }

    // Shape merged rich text at parent's content width
    lb.shaped_lines = text::shape_rich_text_lines(&spans, lb.content.width, fs);
    lb.inline_spans = spans;
}

/// Recursively collect InlineSpans and clear text content in a single pass.
fn collect_and_clear(lb: &mut LayoutBox, spans: &mut Vec<InlineSpan>) {
    if lb.is_text_leaf() {
        if let Some(text) = lb.text_content.take() {
            let style = &lb.style;
            spans.push(InlineSpan {
                text,
                font_size: style.font_size,
                line_height: style.line_height,
                font_family: style.font_family.first().cloned().unwrap_or_default(),
                bold: style.font_weight.is_bold(),
                italic: style.font_style == ferropdf_core::FontStyle::Italic,
                color: style.color,
                text_decoration: style.text_decoration.clone(),
            });
        }
    } else {
        for child in &mut lb.children {
            collect_and_clear(child, spans);
        }
    }
    lb.shaped_lines.clear();
}
