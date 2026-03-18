pub mod style_to_taffy;
pub mod text;
pub mod block_flow;
pub mod table_layout;
mod taffy_bridge;

use ferropdf_core::{Document, LayoutTree};
use ferropdf_style::StyleTree;

/// Build a layout tree from a styled document using Taffy for layout computation.
pub fn layout(
    document: &Document,
    styles: &StyleTree,
    available_width: f32,
    available_height: f32,
) -> ferropdf_core::Result<LayoutTree> {
    let mut font_system = cosmic_text::FontSystem::new();
    let mut tree = taffy_bridge::build_layout(
        document,
        styles,
        &mut font_system,
        available_width,
        available_height,
    )?;

    // Post-pass: apply CSS 2.1 block flow (margin collapsing + relative positioning)
    block_flow::apply_block_flow(&mut tree, available_width);

    Ok(tree)
}
