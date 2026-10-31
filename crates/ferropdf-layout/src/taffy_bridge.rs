use crate::table_layout;
use crate::text::TextContext;
use crate::{style_to_taffy, text};
use cosmic_text::FontSystem;
use ferropdf_core::{
    Clear, ComputedStyle, Display as FDisplay, Document, Float, Insets, LayoutBox, LayoutTree,
    Length, NodeId, Rect,
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

    // Mixed `calc(% ± length)` values were laid out with their percentage
    // only: resolve them against the parent's actual content box and lay
    // out again (a few rounds for calc() nested inside calc()-sized parents).
    for _ in 0..3 {
        if !resolve_calc_nodes(&mut taffy, &table_cell_parent.calc_nodes)? {
            break;
        }
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
            .map_err(|e| {
                ferropdf_core::FerroError::Layout(format!("Taffy layout error: {:?}", e))
            })?;
    }

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
                spacing: text::Spacing::of(&style),
                indent: first_line_indent(doc, node_id, styles, available_width),
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
    // Width available to the children (used to size tables before layout).
    let child_available = content_width_estimate(&style, available_width);
    let is_flex_or_grid = matches!(style.display, FDisplay::Flex | FDisplay::Grid);

    for &child_id in &node.children {
        // Collapsible white space between flex/grid items creates no item
        // (CSS Flexbox §4, Grid §6): it would take a cell / add a gap.
        if is_flex_or_grid && inline_kind(doc, child_id, styles) == InlineKind::Whitespace {
            continue;
        }
        build_taffy_tree_inner(
            doc,
            child_id,
            styles,
            taffy,
            node_map,
            table_cell_parent,
            font_system,
            child_available,
            depth + 1,
        )?;
        if let Some(&tid) = node_map.get(&child_id) {
            child_taffy_ids.push(tid);
        }
    }

    let mut taffy_style = style_to_taffy::convert(&style);
    if style.display == FDisplay::Grid {
        resolve_auto_repeat(&mut taffy_style, child_available, child_taffy_ids.len());
    }

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
        let has_float = kinds
            .iter()
            .any(|(cid, _)| styles.get(cid).is_some_and(|s| s.float != Float::None));
        if has_float {
            let dom_of: HashMap<taffy::NodeId, NodeId> =
                kinds.iter().map(|(cid, _)| (node_map[cid], *cid)).collect();
            child_taffy_ids = wrap_floats(
                &child_taffy_ids,
                &dom_of,
                styles,
                taffy,
                table_cell_parent,
                child_available,
            )?;
        }
    }

    // If this is a block container whose children are all inline/text,
    // switch to flex-row + wrap so that inline elements flow horizontally.
    // Map text-align to justify-content for inline-block centering (CSS spec).
    // Children that produce no box (display: none, running elements) do
    // not count. An inline-block shrink-wraps its inline content the same
    // way, so its width is the sum of its pieces rather than the widest.
    let rendered: Vec<NodeId> = node
        .children
        .iter()
        .copied()
        .filter(|cid| node_map.contains_key(cid))
        .collect();
    if matches!(style.display, FDisplay::Block | FDisplay::InlineBlock) && !rendered.is_empty() {
        let all_inline = rendered.iter().all(|&cid| {
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
            taffy_style.justify_content = Some(match style.text_align.physical(style.direction) {
                ferropdf_core::TextAlign::Center => taffy::JustifyContent::Center,
                ferropdf_core::TextAlign::Right => taffy::JustifyContent::FlexEnd,
                _ => taffy::JustifyContent::FlexStart,
            });
        }
    }

    let taffy_node = taffy
        .new_with_children(taffy_style, &child_taffy_ids)
        .map_err(|e| ferropdf_core::FerroError::Layout(format!("Taffy node error: {:?}", e)))?;

    if let Some(calc) = CalcFields::of(&style) {
        table_cell_parent.calc_nodes.push((taffy_node, calc));
    }
    node_map.insert(node_id, taffy_node);
    Ok(())
}

/// `text-indent` for a text node that starts the first line of its block:
/// it must be the first rendered piece of content of every inline ancestor
/// up to a block container. The same indent is applied when the block's
/// inline content is shaped as one paragraph, so measurement and final
/// shaping agree.
fn first_line_indent(
    doc: &Document,
    node_id: NodeId,
    styles: &StyleTree,
    available_width: f32,
) -> f32 {
    let renders = |id: NodeId| -> bool {
        let n = doc.get(id);
        if n.is_text() {
            return n.text.as_deref().is_some_and(|t| !t.trim().is_empty());
        }
        n.is_element() && styles.get(&id).is_some_and(|s| s.display != FDisplay::None)
    };
    let mut cur = node_id;
    while let Some(parent) = doc.get(cur).parent {
        let first = doc
            .get(parent)
            .children
            .iter()
            .copied()
            .find(|&c| renders(c));
        if first != Some(cur) {
            return 0.0;
        }
        let Some(ps) = styles.get(&parent) else {
            return 0.0;
        };
        match ps.display {
            FDisplay::Inline => cur = parent,
            FDisplay::Block | FDisplay::ListItem | FDisplay::TableCell | FDisplay::InlineBlock => {
                return ps.text_indent.resolve_against(available_width);
            }
            _ => return 0.0,
        }
    }
    0.0
}

/// Move a laid-out box and its descendants horizontally by `dx`.
fn translate_box(b: &mut LayoutBox, dx: f32) {
    b.rect.x += dx;
    b.content.x += dx;
    for c in &mut b.children {
        translate_box(c, dx);
    }
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
                    // Right to left, the first column is the rightmost one:
                    // number columns visually so collapsed borders stay right.
                    let visual_col = if style.direction == ferropdf_core::Direction::Rtl {
                        total_cols.saturating_sub(col_idx + cell.colspan)
                    } else {
                        col_idx
                    };
                    child_box.table_cell_pos = Some((row_idx, visual_col, total_rows, total_cols));
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
                let (mut cx, mut cy) = (x, y);
                let mut wrapper = table_cell_parent.anon_wrapper.get(&child_id).copied();
                while let Some(w) = wrapper {
                    let wl = taffy.layout(w).map_err(|e| {
                        ferropdf_core::FerroError::Layout(format!("Taffy read error: {:?}", e))
                    })?;
                    cx += wl.location.x;
                    cy += wl.location.y;
                    wrapper = table_cell_parent.anon_outer.get(&w).copied();
                }
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

    // Taffy has no `direction`: mirror the items of a right-to-left grid
    // (tables are grids) across the content box, so the first column is on
    // the right. Flex containers are handled in `style_to_taffy`.
    if style.direction == ferropdf_core::Direction::Rtl
        && matches!(style.display, FDisplay::Grid | FDisplay::Table)
    {
        let (left, right) = (content.x, content.x + content.width);
        for child in children.iter_mut().filter(|c| !c.out_of_flow) {
            let mirrored_x = left + right - (child.rect.x + child.rect.width);
            translate_box(child, mirrored_x - child.rect.x);
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
        link: node
            .attr("href")
            .filter(|_| node.tag() == Some("a"))
            .map(str::to_string),
        anchor: node
            .attr("id")
            .or_else(|| node.attr("name").filter(|_| node.tag() == Some("a")))
            .filter(|a| !a.is_empty())
            .map(str::to_string),
        bookmark: heading_bookmark(doc, node_id),
    })
}

/// Outline entry of an `<h1>`–`<h6>`: its level and text (white space
/// collapsed). Empty headings get no entry.
fn heading_bookmark(doc: &Document, node_id: NodeId) -> Option<(u8, String)> {
    let level = match doc.get(node_id).tag()? {
        "h1" => 1,
        "h2" => 2,
        "h3" => 3,
        "h4" => 4,
        "h5" => 5,
        "h6" => 6,
        _ => return None,
    };
    let text: String = doc
        .iter_preorder(node_id)
        .filter_map(|id| doc.get(id).text.as_deref())
        .collect::<Vec<_>>()
        .join("");
    let title = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (!title.is_empty()).then_some((level, title))
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
    /// Anonymous wrapper → the anonymous wrapper enclosing it (float rows
    /// contain line boxes and content blocks).
    pub anon_outer: HashMap<taffy::NodeId, taffy::NodeId>,
    /// Root of the subtree being laid out.
    pub layout_root: Option<NodeId>,
    /// Nodes with mixed `calc()` values, resolved after a first layout.
    pub calc_nodes: Vec<(taffy::NodeId, CalcFields)>,
}

/// The `calc(% ± length)` properties of one node (pt already resolved).
#[derive(Debug, Clone, Default)]
pub(crate) struct CalcFields {
    width: Option<ferropdf_core::CalcLength>,
    min_width: Option<ferropdf_core::CalcLength>,
    max_width: Option<ferropdf_core::CalcLength>,
    height: Option<ferropdf_core::CalcLength>,
    min_height: Option<ferropdf_core::CalcLength>,
    max_height: Option<ferropdf_core::CalcLength>,
    margin: [Option<ferropdf_core::CalcLength>; 4],
    padding: [Option<ferropdf_core::CalcLength>; 4],
    /// Padding + border added to widths/heights for `box-sizing: content-box`.
    content_box_extra: (f32, f32),
}

impl CalcFields {
    fn of(style: &ferropdf_core::ComputedStyle) -> Option<Self> {
        let c = |l: &Length| match l {
            Length::Calc(c) => Some(*c),
            _ => None,
        };
        let f = CalcFields {
            width: c(&style.width),
            min_width: c(&style.min_width),
            max_width: c(&style.max_width),
            height: c(&style.height),
            min_height: c(&style.min_height),
            max_height: c(&style.max_height),
            margin: style.margin.each_ref().map(c),
            padding: style.padding.each_ref().map(c),
            content_box_extra: if style.box_sizing == ferropdf_core::BoxSizing::ContentBox {
                let pt = |l: &Length| if let Length::Pt(v) = l { *v } else { 0.0 };
                (
                    pt(&style.padding[1])
                        + pt(&style.padding[3])
                        + style.border_left.width
                        + style.border_right.width,
                    pt(&style.padding[0])
                        + pt(&style.padding[2])
                        + style.border_top.width
                        + style.border_bottom.width,
                )
            } else {
                (0.0, 0.0)
            },
        };
        let any = [
            f.width,
            f.min_width,
            f.max_width,
            f.height,
            f.min_height,
            f.max_height,
        ]
        .iter()
        .chain(f.margin.iter())
        .chain(f.padding.iter())
        .any(Option::is_some);
        any.then_some(f)
    }
}

/// Set every recorded calc() property to its value against the parent's
/// current content box. Returns whether any style changed.
fn resolve_calc_nodes(
    taffy: &mut TaffyTree<TextContext>,
    nodes: &[(taffy::NodeId, CalcFields)],
) -> ferropdf_core::Result<bool> {
    let err = |e: taffy::TaffyError| {
        ferropdf_core::FerroError::Layout(format!("Taffy calc error: {:?}", e))
    };
    let mut changed = false;
    for (node, f) in nodes {
        let Some(parent) = taffy.parent(*node) else {
            continue;
        };
        let pl = *taffy.layout(parent).map_err(err)?;
        let cw =
            (pl.size.width - pl.padding.left - pl.padding.right - pl.border.left - pl.border.right)
                .max(0.0);
        let ch = (pl.size.height
            - pl.padding.top
            - pl.padding.bottom
            - pl.border.top
            - pl.border.bottom)
            .max(0.0);
        let mut st = taffy.style(*node).map_err(err)?.clone();
        let before = st.clone();
        let dim =
            |c: &Option<ferropdf_core::CalcLength>, basis: f32, extra: f32, d: &mut Dimension| {
                if let Some(c) = c {
                    *d = Dimension::Length((c.against(basis) + extra).max(0.0));
                }
            };
        let (ew, eh) = f.content_box_extra;
        dim(&f.width, cw, ew, &mut st.size.width);
        dim(&f.min_width, cw, ew, &mut st.min_size.width);
        dim(&f.max_width, cw, ew, &mut st.max_size.width);
        dim(&f.height, ch, eh, &mut st.size.height);
        dim(&f.min_height, ch, eh, &mut st.min_size.height);
        dim(&f.max_height, ch, eh, &mut st.max_size.height);
        // Margins and paddings resolve against the containing block *width*.
        let margins = [
            &mut st.margin.top,
            &mut st.margin.right,
            &mut st.margin.bottom,
            &mut st.margin.left,
        ];
        for (c, m) in f.margin.iter().zip(margins) {
            if let Some(c) = c {
                *m = LengthPercentageAuto::Length(c.against(cw));
            }
        }
        let paddings = [
            &mut st.padding.top,
            &mut st.padding.right,
            &mut st.padding.bottom,
            &mut st.padding.left,
        ];
        for (c, p) in f.padding.iter().zip(paddings) {
            if let Some(c) = c {
                *p = LengthPercentage::Length(c.against(cw).max(0.0));
            }
        }
        if st != before {
            taffy.set_style(*node, st).map_err(err)?;
            changed = true;
        }
    }
    Ok(changed)
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
    match styles.get(&id).map(|s| (&s.display, s.float)) {
        Some((_, f)) if f != Float::None => InlineKind::Block, // floats leave the line
        Some((FDisplay::Inline, _)) | Some((FDisplay::InlineBlock, _)) => InlineKind::Inline,
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
            justify_content: Some(
                match parent_style.text_align.physical(parent_style.direction) {
                    ferropdf_core::TextAlign::Center => taffy::JustifyContent::Center,
                    ferropdf_core::TextAlign::Right => taffy::JustifyContent::FlexEnd,
                    _ => taffy::JustifyContent::FlexStart,
                },
            ),
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

/// Best pre-layout estimate of a block's content width, from the width
/// available to it: its own definite width, or the available width minus
/// margins (negative ones widen it, like Bootstrap's `.row`), padding and
/// borders. Flex/grid items and inline boxes keep the parent's estimate.
fn content_width_estimate(style: &ComputedStyle, available: f32) -> f32 {
    if !matches!(
        style.display,
        FDisplay::Block | FDisplay::ListItem | FDisplay::TableCell
    ) {
        return available;
    }
    let pt = |l: &Length, basis: f32| match l {
        Length::Pt(v) | Length::Px(v) => *v,
        Length::Percent(p) => basis * p / 100.0,
        Length::Calc(c) => c.against(basis),
        _ => 0.0,
    };
    let pad = pt(&style.padding[1], available) + pt(&style.padding[3], available);
    let border = style.border_left.width + style.border_right.width;
    let width = match style.width {
        Length::Pt(_) | Length::Px(_) | Length::Percent(_) | Length::Calc(_) => {
            let w = pt(&style.width, available);
            if style.box_sizing == ferropdf_core::BoxSizing::BorderBox {
                w - pad - border
            } else {
                w
            }
        }
        _ => {
            let margin = pt(&style.margin[1], available) + pt(&style.margin[3], available);
            available - margin - pad - border
        }
    };
    width.max(0.0)
}

/// Turn `repeat(auto-fill | auto-fit, …)` into a fixed count computed from
/// the estimated container width. Taffy 0.5 panics on auto repetitions whose
/// tracks have a flexible max (`minmax(120px, 1fr)` — its definite-size
/// helper reads `max` twice), and that is the most common way to use them.
fn resolve_auto_repeat(st: &mut taffy::Style, inner_width: f32, item_count: usize) {
    use taffy::{
        GridTrackRepetition, MaxTrackSizingFunction, MinTrackSizingFunction, TrackSizingFunction,
    };
    let fixed = |lp: &LengthPercentage| match lp {
        LengthPercentage::Length(v) => Some(*v),
        LengthPercentage::Percent(p) => Some(p * inner_width),
    };
    let track_size = |t: &taffy::NonRepeatedTrackSizingFunction| -> Option<f32> {
        let max = match &t.max {
            MaxTrackSizingFunction::Fixed(lp) => fixed(lp),
            _ => None,
        };
        let min = match &t.min {
            MinTrackSizingFunction::Fixed(lp) => fixed(lp),
            _ => None,
        };
        match (min, max) {
            (Some(a), Some(b)) => Some(a.max(b).max(1.0)),
            (a, b) => a.or(b).map(|v| v.max(1.0)),
        }
    };
    let gap = match st.gap.width {
        LengthPercentage::Length(v) => v,
        LengthPercentage::Percent(p) => p * inner_width,
    };
    let fixed_others: f32 = st
        .grid_template_columns
        .iter()
        .filter_map(|t| match t {
            TrackSizingFunction::Single(t) => track_size(t).map(|w| w + gap),
            _ => None,
        })
        .sum();
    for track in st
        .grid_template_columns
        .iter_mut()
        .chain(st.grid_template_rows.iter_mut())
    {
        let TrackSizingFunction::Repeat(rep, tracks) = track else {
            continue;
        };
        if !matches!(
            rep,
            GridTrackRepetition::AutoFill | GridTrackRepetition::AutoFit
        ) {
            continue;
        }
        let auto_fit = matches!(rep, GridTrackRepetition::AutoFit);
        let unit: Option<f32> = tracks.iter().map(track_size).sum();
        let count = match unit {
            Some(unit) => {
                let per = unit + gap * tracks.len() as f32;
                (((inner_width - fixed_others + gap) / per).floor() as usize).max(1)
            }
            None => 1, // invalid per CSS: auto repetition needs definite sizes
        };
        // auto-fit collapses empty repeated tracks: never more than the items.
        let count = if auto_fit {
            count.min(item_count.max(1))
        } else {
            count
        };
        *rep = GridTrackRepetition::Count(count.min(u16::MAX as usize) as u16);
    }
}

/// Emulate floats (Taffy 0.5 has none). From each floated child up to the
/// next element with `clear` (or the end), floats are placed on lines in
/// source order, like CSS: a float goes on the current line if its declared
/// width still fits (auto widths are assumed to fit), otherwise it starts a
/// new line. Each line is an anonymous flex row: left floats in order, then
/// (first line only) the run's non-floated content in a flexible column,
/// then right floats with the first one pushed to the right edge
/// (`margin-left: auto`), so successive right floats stack from the right.
///
/// Approximation: content beside a float stays in a column next to it; it
/// does not wrap underneath the float once it is taller.
fn wrap_floats(
    items: &[taffy::NodeId],
    dom_of: &HashMap<taffy::NodeId, NodeId>,
    styles: &StyleTree,
    taffy: &mut TaffyTree<TextContext>,
    state: &mut BridgeState,
    container_width: f32,
) -> ferropdf_core::Result<Vec<taffy::NodeId>> {
    let err = |e: taffy::TaffyError| {
        ferropdf_core::FerroError::Layout(format!("Taffy float error: {:?}", e))
    };
    let style_of = |t: &taffy::NodeId| dom_of.get(t).and_then(|id| styles.get(id));
    let float_of = |t: &taffy::NodeId| style_of(t).map_or(Float::None, |s| s.float);
    let clears = |t: &taffy::NodeId| style_of(t).is_some_and(|s| s.clear != Clear::None);
    // Declared outer width of a float, when known before layout.
    let outer_width = |t: &taffy::NodeId| -> f32 {
        let Some(st) = style_of(t) else { return 0.0 };
        let len = |l: &Length| match l {
            Length::Pt(v) | Length::Px(v) => *v,
            Length::Percent(p) => container_width * p / 100.0,
            Length::Calc(c) => c.against(container_width),
            _ => 0.0,
        };
        let w = len(&st.width);
        if w == 0.0 {
            return 0.0; // auto: shrink-to-fit, assume it fits
        }
        let extra = if st.box_sizing == ferropdf_core::BoxSizing::ContentBox {
            len(&st.padding[1]) + len(&st.padding[3]) + st.border_left.width + st.border_right.width
        } else {
            0.0
        };
        w + extra + len(&st.margin[1]) + len(&st.margin[3])
    };

    struct Line {
        lefts: Vec<taffy::NodeId>,
        rights: Vec<taffy::NodeId>,
        used: f32,
    }

    let mut out = Vec::new();
    let mut i = 0;
    while i < items.len() {
        if float_of(&items[i]) == Float::None {
            out.push(items[i]);
            i += 1;
            continue;
        }
        let mut end = i + 1;
        while end < items.len() && !clears(&items[end]) {
            end += 1;
        }
        let run = &items[i..end];
        let others: Vec<_> = run
            .iter()
            .copied()
            .filter(|t| float_of(t) == Float::None)
            .collect();

        let mut lines = vec![Line {
            lefts: Vec::new(),
            rights: Vec::new(),
            used: 0.0,
        }];
        for t in run.iter().filter(|t| float_of(t) != Float::None) {
            let w = outer_width(t);
            let line = lines.last_mut().expect("at least one line");
            let empty = line.lefts.is_empty() && line.rights.is_empty();
            if !empty && line.used + w > container_width + 0.5 {
                lines.push(Line {
                    lefts: Vec::new(),
                    rights: Vec::new(),
                    used: 0.0,
                });
            }
            let line = lines.last_mut().expect("at least one line");
            line.used += w;
            match float_of(t) {
                Float::Left => line.lefts.push(*t),
                _ => line.rights.push(*t),
            }
        }

        for (n, line) in lines.iter().enumerate() {
            let mut row_children = line.lefts.clone();
            let content = if n == 0 && !others.is_empty() {
                let content_style = taffy::Style {
                    display: taffy::Display::Block,
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    flex_basis: Dimension::Length(0.0),
                    min_size: Size {
                        width: Dimension::Length(0.0),
                        height: Dimension::Auto,
                    },
                    ..Default::default()
                };
                let c = taffy
                    .new_with_children(content_style, &others)
                    .map_err(err)?;
                row_children.push(c);
                Some(c)
            } else {
                None
            };
            for (k, r) in line.rights.iter().rev().enumerate() {
                if k == 0 {
                    let mut st = taffy.style(*r).map_err(err)?.clone();
                    st.margin.left = LengthPercentageAuto::Auto;
                    taffy.set_style(*r, st).map_err(err)?;
                }
                row_children.push(*r);
            }
            let row_style = taffy::Style {
                display: taffy::Display::Flex,
                flex_direction: taffy::FlexDirection::Row,
                align_items: Some(taffy::AlignItems::FlexStart),
                ..Default::default()
            };
            let row = taffy
                .new_with_children(row_style, &row_children)
                .map_err(err)?;
            for t in line.lefts.iter().chain(line.rights.iter()) {
                place_in(*t, row, dom_of, state);
            }
            if let Some(c) = content {
                state.anon_outer.insert(c, row);
                for t in &others {
                    place_in(*t, c, dom_of, state);
                }
            }
            out.push(row);
        }
        i = end;
    }
    Ok(out)
}

/// Record that `item` (a DOM child or an anonymous wrapper) now sits in `wrapper`.
fn place_in(
    item: taffy::NodeId,
    wrapper: taffy::NodeId,
    dom_of: &HashMap<taffy::NodeId, NodeId>,
    state: &mut BridgeState,
) {
    match dom_of.get(&item) {
        Some(id) => {
            state.anon_wrapper.insert(*id, wrapper);
        }
        None => {
            state.anon_outer.insert(item, wrapper);
        }
    }
}
