//! CSS table layout — resolves rows, cells and column widths.

use ferropdf_core::Edge;
use ferropdf_parse::css::properties::Display;

use super::box_model::{LayoutBox, LayoutBoxKind};
use super::engine::LayoutEngine;

// ─── Row accessor ─────────────────────────────────────────────────────────────

/// Flat reference into the table's row hierarchy.
/// Tables can have direct `<tr>` children or section groups (`<thead>/<tbody>/<tfoot>`).
#[derive(Clone, Copy)]
struct RowRef { section: Option<usize>, row: usize }

fn collect_rows(table: &LayoutBox) -> Vec<RowRef> {
    let mut refs = Vec::new();
    for (si, section) in table.children.iter().enumerate() {
        match section.style.display {
            Display::TableRow => { refs.push(RowRef { section: None,        row: si }); }
            Display::TableHeaderGroup
            | Display::TableRowGroup
            | Display::TableFooterGroup => {
                for (ri, row) in section.children.iter().enumerate() {
                    if matches!(row.style.display, Display::TableRow) {
                        refs.push(RowRef { section: Some(si), row: ri });
                    }
                }
            }
            _ => {}
        }
    }
    refs
}

fn get_row<'a>(table: &'a LayoutBox, rr: RowRef) -> &'a LayoutBox {
    if let Some(si) = rr.section { &table.children[si].children[rr.row] }
    else { &table.children[rr.row] }
}

fn get_row_mut<'a>(table: &'a mut LayoutBox, rr: RowRef) -> &'a mut LayoutBox {
    if let Some(si) = rr.section { &mut table.children[si].children[rr.row] }
    else { &mut table.children[rr.row] }
}

// ─── layout_table ─────────────────────────────────────────────────────────────

/// Lay out a table box and all its rows/cells.
pub fn layout_table(
    table:    &mut LayoutBox,
    avail_w:  f32,
    offset_y: f32,
    fs:       &mut cosmic_text::FontSystem,
    engine:   &LayoutEngine,
) {
    let padding = table.style.padding;
    let border  = Edge {
        top:    table.style.border_top.width,
        right:  table.style.border_right.width,
        bottom: table.style.border_bottom.width,
        left:   table.style.border_left.width,
    };

    // NOTE: table.content.x / .y set by the caller — do not reset.
    let width = table.style.width.unwrap_or(avail_w);
    table.content.width = width;

    let inner_w = (width - padding.horizontal() - border.horizontal()).max(0.0);
    let inner_start_x = table.content.x + padding.left + border.left;
    let inner_start_y = table.content.y + padding.top  + border.top;

    // Flat list of all rows (through thead/tbody/tfoot sections)
    let row_refs = collect_rows(table);
    if row_refs.is_empty() {
        table.content.height = padding.vertical() + border.vertical();
        return;
    }

    // Number of columns (max cells in any row)
    let n_cols = row_refs.iter()
        .map(|&rr| get_row(table, rr).children.len())
        .max()
        .unwrap_or(0)
        .max(1);

    // Column widths
    let col_widths = compute_column_widths(table, &row_refs, n_cols, inner_w);

    // Position section groups (so their backgrounds render correctly)
    for section in &mut table.children {
        if matches!(section.style.display,
            Display::TableHeaderGroup | Display::TableRowGroup | Display::TableFooterGroup)
        {
            section.content.x = inner_start_x;
            section.content.width = inner_w;
        }
    }

    let border_sp = table.style.border_spacing;
    let mut cursor_y = inner_start_y;

    for rr in row_refs {
        let row_border = {
            let row = get_row(table, rr);
            Edge {
                top:    row.style.border_top.width,
                right:  row.style.border_right.width,
                bottom: row.style.border_bottom.width,
                left:   row.style.border_left.width,
            }
        };

        // Set row position
        get_row_mut(table, rr).content.x = inner_start_x;
        get_row_mut(table, rr).content.y = cursor_y;
        get_row_mut(table, rr).content.width = inner_w;

        let inner_row_y = cursor_y + get_row(table, rr).style.padding.top + row_border.top;
        let mut cell_x  = inner_start_x + get_row(table, rr).style.padding.left + row_border.left;
        let mut row_h   = 0.0f32;
        let n_cells     = get_row(table, rr).children.len();

        // Layout each cell
        for col_idx in 0..n_cells {
            let col_span = get_row(table, rr).children[col_idx].style.grid_column_span.max(1) as usize;
            let cell_w: f32 = col_widths[col_idx..col_idx.saturating_add(col_span).min(n_cols)]
                .iter()
                .sum::<f32>()
                + border_sp * (col_span.saturating_sub(1)) as f32;

            let row = get_row_mut(table, rr);
            row.children[col_idx].content.x = cell_x;
            row.children[col_idx].content.y = inner_row_y;
            engine.layout_node(&mut row.children[col_idx], cell_w, inner_row_y, fs);

            let full_h = row.children[col_idx].content.height
                + row.children[col_idx].style.padding.vertical()
                + row.children[col_idx].style.border_top.width
                + row.children[col_idx].style.border_bottom.width;
            row_h = row_h.max(full_h);
            cell_x += cell_w + border_sp;
        }

        // Equalise row height across cells
        for col_idx in 0..n_cells {
            let row = get_row_mut(table, rr);
            if row.children[col_idx].content.height < row_h {
                let cell = &mut row.children[col_idx];
                cell.content.height = (row_h
                    - cell.style.padding.vertical()
                    - cell.style.border_top.width
                    - cell.style.border_bottom.width)
                    .max(0.0);
            }
        }

        let row = get_row_mut(table, rr);
        row.content.height = row_h + row.style.padding.vertical() + row_border.vertical();
        cursor_y += row.content.height + border_sp;

        // Update section content.y / height if this is an inner row
        if let Some(si) = rr.section {
            if table.children[si].content.y == 0.0 || table.children[si].content.y > cursor_y {
                table.children[si].content.y = table.children[si].children[rr.row].content.y;
            }
            let section_end = cursor_y;
            table.children[si].content.height =
                section_end - table.children[si].content.y;
        }
    }

    table.content.height = cursor_y - offset_y + padding.bottom + border.bottom;
    table.content.height = table.content.height.max(table.style.min_height);
}

// ─── compute_column_widths ────────────────────────────────────────────────────

fn compute_column_widths(
    table:    &LayoutBox,
    row_refs: &[RowRef],
    n_cols:   usize,
    inner_w:  f32,
) -> Vec<f32> {
    let mut explicit: Vec<Option<f32>> = vec![None; n_cols];

    for &rr in row_refs {
        let row = get_row(table, rr);
        for (ci, cell) in row.children.iter().enumerate() {
            if ci >= n_cols { break; }
            if explicit[ci].is_none() {
                if let Some(w) = cell.style.width {
                    explicit[ci] = Some(w);
                }
            }
        }
    }

    let n_fixed     = explicit.iter().filter(|e| e.is_some()).count();
    let fixed_total: f32 = explicit.iter().filter_map(|e| *e).sum();
    let remaining   = (inner_w - fixed_total).max(0.0);
    let flexible    = n_cols - n_fixed;
    let even_w      = if flexible > 0 { remaining / flexible as f32 } else { 0.0 };
    let fallback    = inner_w / n_cols as f32;

    explicit.into_iter()
        .map(|e| e.unwrap_or(if flexible > 0 { even_w } else { fallback }))
        .collect()
}
