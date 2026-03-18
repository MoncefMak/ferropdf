use std::collections::HashMap;
use taffy::prelude::*;
use cosmic_text::FontSystem;
use ferropdf_core::{
    Document, NodeId, ComputedStyle, Display as FDisplay,
    LayoutBox, LayoutTree, Rect, Insets, Length,
};
use ferropdf_style::StyleTree;
use crate::{style_to_taffy, text};
use crate::text::TextContext;
use crate::table_layout;

/// Build a LayoutTree from a styled Document using Taffy for layout computation.
pub fn build_layout(
    doc: &Document,
    styles: &StyleTree,
    font_system: &mut FontSystem,
    available_width: f32,
    available_height: f32,
) -> ferropdf_core::Result<LayoutTree> {
    let mut taffy: TaffyTree<TextContext> = TaffyTree::new();
    let mut node_map: HashMap<NodeId, taffy::NodeId> = HashMap::new();
    let mut table_cell_parent: HashMap<NodeId, NodeId> = HashMap::new();

    let root = doc.root();
    let body = find_body(doc, root).unwrap_or(root);

    build_taffy_tree(doc, body, styles, &mut taffy, &mut node_map, &mut table_cell_parent, font_system, available_width)?;

    let taffy_root = match node_map.get(&body) {
        Some(n) => *n,
        None => return Ok(LayoutTree::new()),
    };

    // Compute layout with cosmic-text measure function for text leaves
    taffy.compute_layout_with_measure(
        taffy_root,
        Size {
            width: AvailableSpace::Definite(available_width),
            height: AvailableSpace::Definite(available_height),
        },
        |known_dimensions, available_space, _node_id, node_context, _style| {
            if let Some(ctx) = node_context {
                text::measure_text(ctx, known_dimensions, available_space, font_system)
            } else {
                Size::ZERO
            }
        },
    ).map_err(|e| ferropdf_core::FerroError::Layout(format!("Taffy layout error: {:?}", e)))?;

    // Read results from Taffy and build LayoutBox tree
    let layout_root = read_layout(doc, body, styles, &taffy, &node_map, &table_cell_parent, 0.0, 0.0)?;

    Ok(LayoutTree { root: Some(layout_root) })
}

fn find_body(doc: &Document, start: NodeId) -> Option<NodeId> {
    let node = doc.get(start);
    if node.tag_name.as_deref() == Some("body") {
        return Some(start);
    }
    for &child in &node.children {
        if let Some(found) = find_body(doc, child) {
            return Some(found);
        }
    }
    None
}

/// Recursively build the Taffy layout tree.
///
/// For `<table>` elements, the structure is flattened:
///   - The `<table>` becomes a `Display::Grid` node.
///   - `<thead>`, `<tbody>`, `<tfoot>`, `<tr>` are skipped (not added to Taffy).
///   - `<td>` and `<th>` cells are added as direct children of the grid node.
///   - `grid-template-columns` is set to `repeat(num_cols, auto)`.
fn build_taffy_tree(
    doc: &Document,
    node_id: NodeId,
    styles: &StyleTree,
    taffy: &mut TaffyTree<TextContext>,
    node_map: &mut HashMap<NodeId, taffy::NodeId>,
    table_cell_parent: &mut HashMap<NodeId, NodeId>,
    font_system: &mut FontSystem,
    available_width: f32,
) -> ferropdf_core::Result<()> {
    let node = doc.get(node_id);
    let style = styles.get(&node_id).cloned().unwrap_or_default();

    if style.display == FDisplay::None {
        return Ok(());
    }

    // ── Table: flatten to CSS Grid ──
    if style.display == FDisplay::Table {
        return build_table_as_grid(doc, node_id, styles, taffy, node_map, table_cell_parent, font_system, available_width);
    }

    // ── Text nodes: create leaf with TextContext for cosmic-text measurement ──
    if node.is_text() {
        if let Some(ref text_content) = node.text {
            let ctx = TextContext {
                text: text_content.clone(),
                font_size: style.font_size,
                line_height: style.line_height,
                font_family: style.font_family.first().cloned().unwrap_or_default(),
                bold: style.font_weight.is_bold(),
                italic: style.font_style == ferropdf_core::FontStyle::Italic,
            };

            let taffy_node = taffy.new_leaf_with_context(
                style_to_taffy::convert(&style),
                ctx,
            ).map_err(|e| ferropdf_core::FerroError::Layout(format!("Taffy leaf error: {:?}", e)))?;

            node_map.insert(node_id, taffy_node);
            return Ok(());
        }
    }

    // ── Element nodes: recurse into children ──
    let mut child_taffy_ids = Vec::new();

    for &child_id in &node.children {
        build_taffy_tree(doc, child_id, styles, taffy, node_map, table_cell_parent, font_system, available_width)?;
        if let Some(&tid) = node_map.get(&child_id) {
            child_taffy_ids.push(tid);
        }
    }

    let mut taffy_style = style_to_taffy::convert(&style);

    // If this is a block container whose children are all inline/text,
    // switch to flex-row + wrap so that inline elements flow horizontally.
    if matches!(style.display, FDisplay::Block) && !node.children.is_empty() {
        let all_inline = node.children.iter().all(|&cid| {
            let child_node = doc.get(cid);
            if child_node.is_text() {
                return true;
            }
            let cs = styles.get(&cid).cloned().unwrap_or_default();
            matches!(cs.display, FDisplay::Inline | FDisplay::InlineBlock)
        });
        if all_inline {
            taffy_style.display = taffy::Display::Flex;
            taffy_style.flex_direction = taffy::FlexDirection::Row;
            taffy_style.flex_wrap = taffy::FlexWrap::Wrap;
        }
    }

    let taffy_node = taffy.new_with_children(taffy_style, &child_taffy_ids)
        .map_err(|e| ferropdf_core::FerroError::Layout(format!("Taffy node error: {:?}", e)))?;

    node_map.insert(node_id, taffy_node);
    Ok(())
}

/// Build a `<table>` element as a CSS Grid in Taffy.
/// Uses the 4-phase table layout algorithm (CSS 2.1 §17.5).
fn build_table_as_grid(
    doc: &Document,
    table_id: NodeId,
    styles: &StyleTree,
    taffy: &mut TaffyTree<TextContext>,
    node_map: &mut HashMap<NodeId, taffy::NodeId>,
    table_cell_parent: &mut HashMap<NodeId, NodeId>,
    font_system: &mut FontSystem,
    available_width: f32,
) -> ferropdf_core::Result<()> {
    let table_style = styles.get(&table_id).cloned().unwrap_or_default();

    // Use the full 4-phase table layout algorithm
    let table_result = table_layout::compute_table_layout(
        table_id,
        available_width,
        doc,
        styles,
        font_system,
    );

    // Collect rows/cells for building Taffy child nodes
    let rows = table_layout::collect_table_rows(doc, table_id, styles);

    // Build child Taffy nodes for each cell
    let mut cell_taffy_ids = Vec::new();
    for row in &rows {
        for &cell_id in row {
            build_taffy_tree(doc, cell_id, styles, taffy, node_map, table_cell_parent, font_system, available_width)?;
            if let Some(&tid) = node_map.get(&cell_id) {
                cell_taffy_ids.push(tid);
            }
            table_cell_parent.insert(cell_id, table_id);
        }
    }

    // Build grid style with computed column and row tracks
    let mut grid_style = style_to_taffy::convert_table_to_grid_with_widths(
        &table_style,
        &table_result.column_widths,
    );
    // Override rows with computed tracks
    if !table_result.taffy_rows.is_empty() {
        grid_style.grid_template_rows = table_result.taffy_rows;
    }
    // Override columns with fixed tracks (from phase 3)
    if !table_result.taffy_columns.is_empty() {
        grid_style.grid_template_columns = table_result.taffy_columns;
    }

    let grid_node = taffy.new_with_children(grid_style, &cell_taffy_ids)
        .map_err(|e| ferropdf_core::FerroError::Layout(format!("Taffy grid node error: {:?}", e)))?;

    node_map.insert(table_id, grid_node);
    Ok(())
}

/// Collect all rows in a table. Each row is a Vec of cell NodeIds.
fn collect_table_rows(doc: &Document, table_id: NodeId, styles: &StyleTree) -> Vec<Vec<NodeId>> {
    let mut rows = Vec::new();
    let table_node = doc.get(table_id);

    for &child_id in &table_node.children {
        let child_style = styles.get(&child_id).cloned().unwrap_or_default();
        match child_style.display {
            FDisplay::TableRow => {
                rows.push(collect_cells(doc, child_id, styles));
            }
            FDisplay::TableHeaderGroup | FDisplay::TableRowGroup | FDisplay::TableFooterGroup => {
                let group_node = doc.get(child_id);
                for &row_id in &group_node.children {
                    let row_style = styles.get(&row_id).cloned().unwrap_or_default();
                    if row_style.display == FDisplay::TableRow {
                        rows.push(collect_cells(doc, row_id, styles));
                    }
                }
            }
            _ => {}
        }
    }
    rows
}

fn collect_cells(doc: &Document, row_id: NodeId, styles: &StyleTree) -> Vec<NodeId> {
    let row_node = doc.get(row_id);
    row_node.children.iter()
        .filter(|&&child_id| {
            let s = styles.get(&child_id).cloned().unwrap_or_default();
            s.display == FDisplay::TableCell
        })
        .copied()
        .collect()
}

/// Read layout results from Taffy and build the LayoutBox tree.
fn read_layout(
    doc: &Document,
    node_id: NodeId,
    styles: &StyleTree,
    taffy: &TaffyTree<TextContext>,
    node_map: &HashMap<NodeId, taffy::NodeId>,
    table_cell_parent: &HashMap<NodeId, NodeId>,
    offset_x: f32,
    offset_y: f32,
) -> ferropdf_core::Result<LayoutBox> {
    let node = doc.get(node_id);
    let style = styles.get(&node_id).cloned().unwrap_or_default();

    let taffy_node = match node_map.get(&node_id) {
        Some(n) => *n,
        None => return Ok(LayoutBox::default()),
    };

    let layout = taffy.layout(taffy_node)
        .map_err(|e| ferropdf_core::FerroError::Layout(format!("Taffy read error: {:?}", e)))?;

    let x = offset_x + layout.location.x;
    let y = offset_y + layout.location.y;

    // Border-box rect (full box including padding + border)
    let rect = Rect::new(x, y, layout.size.width, layout.size.height);

    let content = Rect::new(
        x + layout.padding.left + layout.border.left,
        y + layout.padding.top + layout.border.top,
        (layout.size.width - layout.padding.left - layout.padding.right - layout.border.left - layout.border.right).max(0.0),
        (layout.size.height - layout.padding.top - layout.padding.bottom - layout.border.top - layout.border.bottom).max(0.0),
    );

    let padding = Insets {
        top: layout.padding.top,
        right: layout.padding.right,
        bottom: layout.padding.bottom,
        left: layout.padding.left,
    };

    let border = Insets {
        top: layout.border.top,
        right: layout.border.right,
        bottom: layout.border.bottom,
        left: layout.border.left,
    };

    let margin = resolve_margin_insets(&style);

    // Build children
    let mut children = Vec::new();

    if style.display == FDisplay::Table {
        let rows = table_layout::collect_table_rows(doc, node_id, styles);
        for row in &rows {
            for &cell_id in row {
                if node_map.contains_key(&cell_id) {
                    let child_box = read_layout(doc, cell_id, styles, taffy, node_map, table_cell_parent, x, y)?;
                    children.push(child_box);
                }
            }
        }
    } else {
        for &child_id in &node.children {
            if table_cell_parent.contains_key(&child_id) {
                continue;
            }
            if node_map.contains_key(&child_id) {
                let child_box = read_layout(doc, child_id, styles, taffy, node_map, table_cell_parent, x, y)?;
                children.push(child_box);
            }
        }
    }

    let text_content = if node.is_text() {
        node.text.clone()
    } else {
        None
    };

    let image_src = if node.tag_name.as_deref() == Some("img") {
        node.attr("src").map(|s| s.to_string())
    } else {
        None
    };

    Ok(LayoutBox {
        node_id: Some(node_id),
        style,
        rect,
        content,
        padding,
        border,
        margin,
        children,
        shaped_lines: Vec::new(),
        image_src,
        text_content,
        out_of_flow: false,
        visual_offset_x: 0.0,
        visual_offset_y: 0.0,
    })
}

fn resolve_margin_insets(style: &ComputedStyle) -> Insets {
    Insets {
        top:    length_to_px(&style.margin[0]),
        right:  length_to_px(&style.margin[1]),
        bottom: length_to_px(&style.margin[2]),
        left:   length_to_px(&style.margin[3]),
    }
}

fn length_to_px(l: &Length) -> f32 {
    match l {
        Length::Px(v) => *v,
        Length::Zero => 0.0,
        _ => 0.0,
    }
}

/// Recursively collect all text content from a subtree.
fn collect_text_content(doc: &Document, node_id: NodeId) -> String {
    let node = doc.get(node_id);
    if node.is_text() {
        return node.text.clone().unwrap_or_default();
    }
    let mut result = String::new();
    for &child_id in &node.children {
        result.push_str(&collect_text_content(doc, child_id));
    }
    result
}

/// Measure min-content width of text (single line, no wrap) using cosmic-text.
fn measure_min_content_width(
    text: &str,
    font_size: f32,
    font_family: &str,
    font_system: &mut FontSystem,
) -> f32 {
    if text.trim().is_empty() {
        return 0.0;
    }
    let metrics = cosmic_text::Metrics::new(font_size, font_size * 1.2);
    let mut buffer = cosmic_text::Buffer::new(font_system, metrics);
    // No wrap → measure on a single line
    buffer.set_size(font_system, None, None);

    let family = if font_family.is_empty() {
        cosmic_text::Family::SansSerif
    } else {
        cosmic_text::Family::Name(font_family)
    };

    let attrs = cosmic_text::Attrs::new().family(family);
    buffer.set_text(font_system, text, attrs, cosmic_text::Shaping::Advanced);
    buffer.shape_until_scroll(font_system, false);

    buffer.layout_runs()
        .map(|r| r.line_w)
        .fold(0.0_f32, f32::max)
}

fn resolve_length(l: &Length) -> f32 {
    match l {
        Length::Px(v) => *v,
        Length::Zero => 0.0,
        _ => 0.0,
    }
}
