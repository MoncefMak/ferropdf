pub mod style_to_taffy;
pub mod text;
pub mod block_flow;
pub mod table_layout;
mod taffy_bridge;

use ferropdf_core::{Document, LayoutTree, LayoutBox};
use ferropdf_style::StyleTree;
pub use text::FontDatabase;

/// Build a layout tree from a styled document using Taffy for layout computation.
/// Toutes les coordonnées sont en points typographiques (pt).
pub fn layout(
    document: &Document,
    styles: &StyleTree,
    available_width: f32,
    available_height: f32,
) -> ferropdf_core::Result<LayoutTree> {
    let owned_font_db = FontDatabase::new();
    layout_with_fonts(document, styles, available_width, available_height, &owned_font_db)
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
fn populate_shaped_lines(lb: &mut LayoutBox, fs: &mut cosmic_text::FontSystem) {
    if let Some(ref txt) = lb.text_content {
        if !txt.trim().is_empty() && lb.shaped_lines.is_empty() {
            let style = &lb.style;
            let font_family = style.font_family.first().cloned().unwrap_or_default();
            lb.shaped_lines = text::shape_text_lines(
                txt,
                style.font_size,
                style.line_height,
                &font_family,
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
