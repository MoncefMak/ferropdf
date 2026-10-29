use crate::table_layout;
use crate::text::TextContext;
use crate::{style_to_taffy, text};
use cosmic_text::FontSystem;
use ferropdf_core::{
    ComputedStyle, Display as FDisplay, Document, Insets, LayoutBox, LayoutTree, Length, NodeId,
    Rect,
};
use ferropdf_style::StyleTree;
use std::collections::HashMap;
use taffy::prelude::*;

/// Build a LayoutTree from a styled Document using Taffy for layout computation.
pub fn build_layout(
    doc: &Document,
    styles: &StyleTree,
    font_system: &mut FontSystem,
    available_width: f32,
    _available_height: f32,
) -> ferropdf_core::Result<LayoutTree> {
    let root = doc.root();
    let body = find_body(doc, root).unwrap_or(root);
    build_layout_from(doc, styles, font_system, body, available_width)
}

/// Lay out the subtree rooted at `body` (the `<body>` element, or a running
/// element laid out on its own for a page margin box).
pub fn build_layout_from(
    doc: &Document,
    styles: &StyleTree,
    font_system: &mut FontSystem,
    body: NodeId,
    available_width: f32,
) -> ferropdf_core::Result<LayoutTree> {
    let mut taffy: TaffyTree<TextContext> = TaffyTree::new();
    let mut node_map: HashMap<NodeId, taffy::NodeId> = HashMap::new();
    let mut table_cell_parent = BridgeState {
        layout_root: Some(body),
        ..Default::default()
    };

    build_taffy_tree(
        doc,
        body,
        styles,
        &mut taffy,
        &mut node_map,
        &mut table_cell_parent,
        font_system,
        available_width,
    )?;

    let taffy_root = match node_map.get(&body) {
        Some(n) => *n,
        None => return Ok(LayoutTree::new()),
    };

    // A root with `width: auto` fills the available width, like a block in
    // its containing block (Taffy would otherwise shrink-to-fit it).
    if let Ok(root_style) = taffy.style(taffy_root) {
        if root_style.size.width == Dimension::Auto {
            let horizontal_margin = styles
                .get(&body)
                .map(|s| length_to_px(&s.margin[1]) + length_to_px(&s.margin[3]))
                .unwrap_or(0.0);
            let mut st = root_style.clone();
            st.size.width = Dimension::Length((available_width - horizontal_margin).max(0.0));
            let _ = taffy.set_style(taffy_root, st);
        }
    }

    // Compute layout with cosmic-text measure function for text leaves.
    // Height is unconstrained (infinite ribbon model) so content flows naturally.
    // Pagination slices the ribbon into pages afterward.
    taffy
        .compute_layout_with_measure(
            taffy_root,
            Size {
                width: AvailableSpace::Definite(available_width),
                height: AvailableSpace::MaxContent,
            },
            |known_dimensions, available_space, _node_id, node_context, _style| {
                if let Some(ctx) = node_context {
                    text::measure_text(ctx, known_dimensions, available_space, font_system)
                } else {
                    Size::ZERO
                }
            },
        )
        .map_err(|e| ferropdf_core::FerroError::Layout(format!("Taffy layout error: {:?}", e)))?;

    // Read results from Taffy and build LayoutBox tree
    let layout_root = read_layout(
        doc,
        body,
        styles,
        &taffy,
        &node_map,
        &table_cell_parent,
        0.0,
        0.0,
    )?;

    Ok(LayoutTree {
        root: Some(layout_root),
    })
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
#[allow(clippy::too_many_arguments)]
fn build_taffy_tree(
    doc: &Document,
    node_id: NodeId,
    styles: &StyleTree,
    taffy: &mut TaffyTree<TextContext>,
    node_map: &mut HashMap<NodeId, taffy::NodeId>,
    table_cell_parent: &mut BridgeState,
    font_system: &mut FontSystem,
    available_width: f32,
) -> ferropdf_core::Result<()> {
    build_taffy_tree_inner(
        doc,
        node_id,
        styles,
        taffy,
        node_map,
        table_cell_parent,
        font_system,
        available_width,
        0,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_taffy_tree_inner(
    doc: &Document,
    node_id: NodeId,
    styles: &StyleTree,
    taffy: &mut TaffyTree<TextContext>,
    node_map: &mut HashMap<NodeId, taffy::NodeId>,
    table_cell_parent: &mut BridgeState,
    font_system: &mut FontSystem,
    available_width: f32,
    depth: usize,
) -> ferropdf_core::Result<()> {
    if depth > ferropdf_core::MAX_DOM_DEPTH {
        return Ok(());
    }
    let node = doc.get(node_id);
    let style = styles.get(&node_id).cloned().unwrap_or_default();

    if style.display == FDisplay::None {
        return Ok(());
    }
    // Running elements leave the normal flow; they are laid out separately
    // (with themselves as layout root) for the page margin boxes.
    if style.running.is_some() && table_cell_parent.layout_root != Some(node_id) {
        return Ok(());
    }

    // ── Table: flatten to CSS Grid ──
    if style.display == FDisplay::Table {
        return build_table_as_grid(
            doc,
            node_id,
            styles,
            taffy,
            node_map,
            table_cell_parent,
            font_system,
            available_width,
        );
    }

    // ── Text nodes: create leaf with TextContext for cosmic-text measurement ──
    if node.is_text() {
        if let Some(ref text_content) = node.text {
            let ctx = TextContext {
                text: process_text(text_content, style.white_space),
                wrap: style.white_space.wraps(),
                font_size: style.font_size,
                line_height: style.line_height,
                font_family: style.font_family.first().cloned().unwrap_or_default(),
                bold: style.font_weight.is_bold(),
                italic: style.font_style == ferropdf_core::FontStyle::Italic,
            };

            let taffy_node = taffy
                .new_leaf_with_context(style_to_taffy::convert(&style), ctx)
                .map_err(|e| {
                    ferropdf_core::FerroError::Layout(format!("Taffy leaf error: {:?}", e))
                })?;

            node_map.insert(node_id, taffy_node);
            return Ok(());
        }
    }

    // ── Element nodes: recurse into children ──
    let mut child_taffy_ids = Vec::new();

    for &child_id in &node.children {
        build_taffy_tree_inner(
            doc,
            child_id,
            styles,
            taffy,
            node_map,
            table_cell_parent,
            font_system,
            available_width,
            depth + 1,
        )?;
        if let Some(&tid) = node_map.get(&child_id) {
            child_taffy_ids.push(tid);
        }
    }

    let mut taffy_style = style_to_taffy::convert(&style);

    // Block container mixing block-level and inline-level children: wrap each
    // run of inline content in an anonymous line box (CSS 2.1 §9.2.1.1), so
    // inline-blocks and text flow on lines instead of stacking full-width.
    if matches!(
        style.display,
        FDisplay::Block | FDisplay::ListItem | FDisplay::TableCell
    ) {
        let kinds: Vec<(NodeId, InlineKind)> = node
            .children
            .iter()
            .filter(|cid| node_map.contains_key(cid))
            .map(|&cid| (cid, inline_kind(doc, cid, styles)))
            .collect();
        let has_block = kinds.iter().any(|(_, k)| *k == InlineKind::Block);
        let has_inline = kinds.iter().any(|(_, k)| *k == InlineKind::Inline);
        if has_block && has_inline {
            child_taffy_ids = wrap_inline_runs(
                &kinds,
                &style,
                taffy,
                node_map,
                table_cell_parent,
                font_system,
            )?;
        }
    }

    // If this is a block container whose children are all inline/text,
    // switch to flex-row + wrap so that inline elements flow horizontally.
    // Map text-align to justify-content for inline-block centering (CSS spec).
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
            let kinds: Vec<(NodeId, InlineKind)> = node
                .children
                .iter()
                .filter(|cid| node_map.contains_key(cid))
                .map(|&cid| (cid, inline_kind(doc, cid, styles)))
                .collect();
            size_inner_spaces(&kinds, taffy, node_map, font_system)?;
            taffy_style.display = taffy::Display::Flex;
            taffy_style.flex_direction = taffy::FlexDirection::Row;
            taffy_style.flex_wrap = taffy::FlexWrap::Wrap;
            // text-align on parent maps to justify-content for inline-block children
            taffy_style.justify_content = Some(match style.text_align {
                ferropdf_core::TextAlign::Center => taffy::JustifyContent::Center,
                ferropdf_core::TextAlign::Right => taffy::JustifyContent::FlexEnd,
                _ => taffy::JustifyContent::FlexStart,
            });
        }
    }

    let taffy_node = taffy
        .new_with_children(taffy_style, &child_taffy_ids)
        .map_err(|e| ferropdf_core::FerroError::Layout(format!("Taffy node error: {:?}", e)))?;

    node_map.insert(node_id, taffy_node);
    Ok(())
}

/// Build a `<table>` element as a CSS Grid in Taffy.
/// Uses the 4-phase table layout algorithm (CSS 2.1 §17.5).
#[allow(clippy::too_many_arguments)]
fn build_table_as_grid(
    doc: &Document,
    table_id: NodeId,
    styles: &StyleTree,
    taffy: &mut TaffyTree<TextContext>,
    node_map: &mut HashMap<NodeId, taffy::NodeId>,
    table_cell_parent: &mut BridgeState,
    font_system: &mut FontSystem,
    available_width: f32,
) -> ferropdf_core::Result<()> {
    let table_style = styles.get(&table_id).cloned().unwrap_or_default();

    // Columns are distributed over the table's own width (`width: 45%`,
    // `width: 520px`), not over the whole containing block.
    let table_width = resolve_table_width(&table_style.width, available_width);

    // Use the full 4-phase table layout algorithm
    let table_result =
        table_layout::compute_table_layout(table_id, table_width, doc, styles, font_system);

    // Collect rows/cells with colspan info for building Taffy child nodes
    let cell_rows = table_layout::collect_table_cell_infos(doc, table_id, styles);

    // Build child Taffy nodes for each cell, applying grid-column span for colspan
    let mut cell_taffy_ids = Vec::new();
    for row in &cell_rows {
        let mut col = 0usize;
        for cell in row {
            // Content inside a cell (nested tables in particular) is laid out
            // against the width of the columns the cell spans.
            let span_end = (col + cell.colspan).min(table_result.column_widths.len());
            let cell_width: f32 = table_result.column_widths[col.min(span_end)..span_end]
                .iter()
                .sum();
            let cell_available = if cell_width > 0.0 {
                cell_width
            } else {
                table_width
            };
            col += cell.colspan;
            build_taffy_tree_inner(
                doc,
                cell.node_id,
                styles,
                taffy,
                node_map,
                table_cell_parent,
                font_system,
                cell_available,
                1, // table cells are at depth 1 relative to the table
            )?;
            if let Some(&tid) = node_map.get(&cell.node_id) {
                // Set grid-column span for cells with colspan > 1
                if cell.colspan > 1 {
                    let mut cell_style = taffy
                        .style(tid)
                        .map_err(|e| {
                            ferropdf_core::FerroError::Layout(format!(
                                "Taffy style read error: {:?}",
                                e
                            ))
                        })?
                        .clone();
                    cell_style.grid_column = taffy::Line {
                        start: taffy::GridPlacement::Auto,
                        end: taffy::GridPlacement::Span(cell.colspan as u16),
                    };
                    taffy.set_style(tid, cell_style).map_err(|e| {
                        ferropdf_core::FerroError::Layout(format!("Taffy set style error: {:?}", e))
                    })?;
                }
                cell_taffy_ids.push(tid);
            }
            table_cell_parent
                .table_cell_parent
                .insert(cell.node_id, table_id);
        }
    }

    // Build grid style with computed column and row tracks.
    // Column widths from compute_table_layout already include min-content
    // guarantees and proportional extra-space distribution. Use Fixed/Fixed
    // tracks so Taffy doesn't further shrink columns below their min-content.
    let mut grid_style = style_to_taffy::convert_table_to_grid_with_widths(
        &table_style,
        &table_result.column_widths,
    );
    // Override with Fixed/Fixed column tracks to enforce exact computed widths
    grid_style.grid_template_columns = table_result.taffy_columns;
    // Override rows with computed tracks (min-height with auto growth)
    if !table_result.taffy_rows.is_empty() {
        grid_style.grid_template_rows = table_result.taffy_rows;
    }

    let grid_node = taffy
        .new_with_children(grid_style, &cell_taffy_ids)
        .map_err(|e| {
            ferropdf_core::FerroError::Layout(format!("Taffy grid node error: {:?}", e))
        })?;

    node_map.insert(table_id, grid_node);
    Ok(())
}

/// Read layout results from Taffy and build the LayoutBox tree.
#[allow(clippy::too_many_arguments)]
fn read_layout(
    doc: &Document,
    node_id: NodeId,
    styles: &StyleTree,
    taffy: &TaffyTree<TextContext>,
    node_map: &HashMap<NodeId, taffy::NodeId>,
    table_cell_parent: &BridgeState,
    offset_x: f32,
    offset_y: f32,
) -> ferropdf_core::Result<LayoutBox> {
    let node = doc.get(node_id);
    let style = styles.get(&node_id).cloned().unwrap_or_default();

    let taffy_node = match node_map.get(&node_id) {
        Some(n) => *n,
        None => return Ok(LayoutBox::default()),
    };

    let layout = taffy
        .layout(taffy_node)
        .map_err(|e| ferropdf_core::FerroError::Layout(format!("Taffy read error: {:?}", e)))?;

    let x = offset_x + layout.location.x;
    let y = offset_y + layout.location.y;

    // Border-box rect (full box including padding + border)
    let rect = Rect::new(x, y, layout.size.width, layout.size.height);

    let content = Rect::new(
        x + layout.padding.left + layout.border.left,
        y + layout.padding.top + layout.border.top,
        (layout.size.width
            - layout.padding.left
            - layout.padding.right
            - layout.border.left
            - layout.border.right)
            .max(0.0),
        (layout.size.height
            - layout.padding.top
            - layout.padding.bottom
            - layout.border.top
            - layout.border.bottom)
            .max(0.0),
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
        let cell_rows = table_layout::collect_table_cell_infos(doc, node_id, styles);
        let total_rows = cell_rows.len();
        let total_cols = cell_rows
            .iter()
            .map(|r| r.iter().map(|c| c.colspan).sum::<usize>())
            .max()
            .unwrap_or(0);
        for (row_idx, row) in cell_rows.iter().enumerate() {
            let mut col_idx = 0;
            for cell in row {
                if node_map.contains_key(&cell.node_id) {
                    let mut child_box = read_layout(
                        doc,
                        cell.node_id,
                        styles,
                        taffy,
                        node_map,
                        table_cell_parent,
                        x,
                        y,
                    )?;
                    child_box.table_cell_pos = Some((row_idx, col_idx, total_rows, total_cols));
                    children.push(child_box);
                }
                col_idx += cell.colspan;
            }
        }
    } else {
        for &child_id in &node.children {
            if table_cell_parent.table_cell_parent.contains_key(&child_id) {
                continue;
            }
            if node_map.contains_key(&child_id) {
                // Children grouped into an anonymous line box are positioned
                // relative to that wrapper, not to this element.
                let (cx, cy) = match table_cell_parent.anon_wrapper.get(&child_id) {
                    Some(&wrapper) => {
                        let wl = taffy.layout(wrapper).map_err(|e| {
                            ferropdf_core::FerroError::Layout(format!("Taffy read error: {:?}", e))
                        })?;
                        (x + wl.location.x, y + wl.location.y)
                    }
                    None => (x, y),
                };
                let child_box = read_layout(
                    doc,
                    child_id,
                    styles,
                    taffy,
                    node_map,
                    table_cell_parent,
                    cx,
                    cy,
                )?;
                children.push(child_box);
            }
        }
    }

    let text_content = if node.is_text() {
        node.text
            .as_deref()
            .map(|t| process_text(t, style.white_space))
    } else {
        None
    };

    // Assign list item indices for marker rendering
    let mut list_idx = 0usize;
    for child in &mut children {
        if child.style.display == FDisplay::ListItem {
            list_idx += 1;
            child.list_item_index = Some(list_idx);
        }
    }

    let image_src = if node.tag_name.as_deref() == Some("img") {
        node.attr("src").map(|s| s.to_string())
    } else {
        None
    };

    // Count thead rows for table pagination
    let (thead_row_count, tfoot_row_count) = if style.display == FDisplay::Table {
        (
            table_layout::count_thead_rows(doc, node_id, styles),
            table_layout::count_tfoot_rows(doc, node_id, styles),
        )
    } else {
        (0, 0)
    };

    // Mark out-of-flow for absolute/fixed positioned elements
    let out_of_flow = matches!(
        style.position,
        ferropdf_core::Position::Absolute | ferropdf_core::Position::Fixed
    );

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
        inline_spans: Vec::new(),
        image_src,
        text_content,
        out_of_flow,
        visual_offset_x: 0.0,
        visual_offset_y: 0.0,
        table_cell_pos: None,
        list_item_index: None,
        thead_row_count,
        tfoot_row_count,
    })
}

fn resolve_margin_insets(style: &ComputedStyle) -> Insets {
    Insets {
        top: length_to_px(&style.margin[0]),
        right: length_to_px(&style.margin[1]),
        bottom: length_to_px(&style.margin[2]),
        left: length_to_px(&style.margin[3]),
    }
}

fn length_to_px(l: &Length) -> f32 {
    match l {
        Length::Pt(v) => *v,
        Length::Px(v) => *v,
        Length::Zero => 0.0,
        _ => 0.0,
    }
}

/// White-space processing for a text node (CSS Text 3 §4.1.1).
///
/// Collapsible white space is reduced to single spaces here; spaces at the
/// start/end of a line box are removed later, when inline content is merged
/// (`merge_inline_children`) or a lone text leaf is shaped.
pub(crate) fn process_text(text: &str, white_space: ferropdf_core::WhiteSpace) -> String {
    white_space.collapse(text)
}

/// Width over which a table distributes its columns, in pt.
fn resolve_table_width(width: &ferropdf_core::Length, available_width: f32) -> f32 {
    use ferropdf_core::Length;
    match *width {
        Length::Pt(v) | Length::Px(v) if v > 0.0 => v,
        Length::Percent(p) if p > 0.0 => available_width * p / 100.0,
        _ => available_width,
    }
}

#[cfg(test)]
mod table_width_tests {
    use super::resolve_table_width;
    use ferropdf_core::Length;

    #[test]
    fn resolves_against_containing_block() {
        assert_eq!(resolve_table_width(&Length::Percent(50.0), 400.0), 200.0);
        assert_eq!(resolve_table_width(&Length::Pt(390.0), 400.0), 390.0);
        assert_eq!(resolve_table_width(&Length::Auto, 400.0), 400.0);
    }
}

/// Side tables produced while building the Taffy tree.
#[derive(Default)]
pub(crate) struct BridgeState {
    /// Table cell → its `<table>` (cells are laid out by the table grid).
    pub table_cell_parent: HashMap<NodeId, NodeId>,
    /// Child → anonymous line-box wrapper it was placed in.
    pub anon_wrapper: HashMap<NodeId, taffy::NodeId>,
    /// Root of the subtree being laid out.
    pub layout_root: Option<NodeId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InlineKind {
    Block,
    Inline,
    /// Collapsible white space only: belongs to no line box.
    Whitespace,
}

fn inline_kind(doc: &Document, id: NodeId, styles: &StyleTree) -> InlineKind {
    let n = doc.get(id);
    if n.is_text() {
        let ws = styles.get(&id).map(|s| s.white_space).unwrap_or_default();
        let blank = n.text.as_deref().is_none_or(|t| t.trim().is_empty());
        return if blank
            && !matches!(
                ws,
                ferropdf_core::WhiteSpace::Pre | ferropdf_core::WhiteSpace::PreWrap
            ) {
            InlineKind::Whitespace
        } else {
            InlineKind::Inline
        };
    }
    match styles.get(&id).map(|s| &s.display) {
        Some(FDisplay::Inline) | Some(FDisplay::InlineBlock) => InlineKind::Inline,
        _ => InlineKind::Block,
    }
}

/// Replace each run of inline children by one anonymous flex-row/wrap node
/// (the same model ferropdf uses for all-inline containers). Returns the new
/// child list for the parent.
fn wrap_inline_runs(
    kinds: &[(NodeId, InlineKind)],
    parent_style: &ferropdf_core::ComputedStyle,
    taffy: &mut TaffyTree<TextContext>,
    node_map: &HashMap<NodeId, taffy::NodeId>,
    state: &mut BridgeState,
    font_system: &mut FontSystem,
) -> ferropdf_core::Result<Vec<taffy::NodeId>> {
    let mut out = Vec::new();
    let mut run: Vec<NodeId> = Vec::new();

    let flush = |run: &mut Vec<NodeId>,
                 out: &mut Vec<taffy::NodeId>,
                 taffy: &mut TaffyTree<TextContext>,
                 state: &mut BridgeState,
                 font_system: &mut FontSystem|
     -> ferropdf_core::Result<()> {
        if run.is_empty() {
            return Ok(());
        }
        let run_kinds: Vec<(NodeId, InlineKind)> = run
            .iter()
            .map(|&c| {
                let blank = taffy
                    .get_node_context(node_map[&c])
                    .is_some_and(|ctx| ctx.text.trim().is_empty());
                let kind = if blank {
                    InlineKind::Whitespace
                } else {
                    InlineKind::Inline
                };
                (c, kind)
            })
            .collect();
        size_inner_spaces(&run_kinds, taffy, node_map, font_system)?;
        let ids: Vec<taffy::NodeId> = run.iter().map(|c| node_map[c]).collect();
        let wrapper_style = taffy::Style {
            display: taffy::Display::Flex,
            flex_direction: taffy::FlexDirection::Row,
            flex_wrap: taffy::FlexWrap::Wrap,
            align_items: Some(taffy::AlignItems::FlexStart),
            justify_content: Some(match parent_style.text_align {
                ferropdf_core::TextAlign::Center => taffy::JustifyContent::Center,
                ferropdf_core::TextAlign::Right => taffy::JustifyContent::FlexEnd,
                _ => taffy::JustifyContent::FlexStart,
            }),
            ..Default::default()
        };
        let wrapper = taffy.new_with_children(wrapper_style, &ids).map_err(|e| {
            ferropdf_core::FerroError::Layout(format!("Taffy anonymous box error: {:?}", e))
        })?;
        for c in run.drain(..) {
            state.anon_wrapper.insert(c, wrapper);
        }
        out.push(wrapper);
        Ok(())
    };

    for &(id, kind) in kinds {
        match kind {
            InlineKind::Inline => run.push(id),
            InlineKind::Whitespace if !run.is_empty() => run.push(id),
            InlineKind::Whitespace => out.push(node_map[&id]),
            InlineKind::Block => {
                flush(&mut run, &mut out, taffy, state, font_system)?;
                out.push(node_map[&id]);
            }
        }
    }
    flush(&mut run, &mut out, taffy, state, font_system)?;
    Ok(out)
}

/// Collapsed white space between two inline-level boxes of a line renders as
/// one space; at the start or end of the line it disappears. Whitespace-only
/// text leaves measure as zero, so give the inner ones a space's width.
fn size_inner_spaces(
    kinds: &[(NodeId, InlineKind)],
    taffy: &mut TaffyTree<TextContext>,
    node_map: &HashMap<NodeId, taffy::NodeId>,
    font_system: &mut FontSystem,
) -> ferropdf_core::Result<()> {
    let is_inline = |i: usize| kinds.get(i).is_some_and(|(_, k)| *k == InlineKind::Inline);
    for (i, (id, kind)) in kinds.iter().enumerate() {
        if *kind != InlineKind::Whitespace || i == 0 || !is_inline(i - 1) {
            continue;
        }
        if !(i + 1..kinds.len()).any(is_inline) {
            continue;
        }
        let tid = node_map[id];
        let Some(ctx) = taffy.get_node_context(tid).cloned() else {
            continue;
        };
        let w = text::space_width(&ctx, font_system);
        let mut st = taffy
            .style(tid)
            .map_err(|e| ferropdf_core::FerroError::Layout(format!("Taffy style error: {:?}", e)))?
            .clone();
        st.min_size.width = taffy::Dimension::Length(w);
        st.flex_shrink = 0.0;
        taffy.set_style(tid, st).map_err(|e| {
            ferropdf_core::FerroError::Layout(format!("Taffy style error: {:?}", e))
        })?;
    }
    Ok(())
}
