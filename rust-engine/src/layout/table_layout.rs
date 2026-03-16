//! Table layout algorithm — supports border-collapse and content-aware column widths.

use super::box_model::{LayoutBox, LayoutBoxType};
use crate::fonts::metrics;
use crate::pdf::writer;

/// Perform table layout on a table LayoutBox.
/// `collapsed` — true when the table uses `border-collapse: collapse`.
pub fn layout_table(
    table_box: &mut LayoutBox,
    container_width: f64,
    font_size: f64,
    _root_font_size: f64,
    collapsed: bool,
) {
    let num_cols = count_columns(table_box);
    if num_cols == 0 {
        return;
    }

    let available = container_width
        - table_box.dimensions.padding.horizontal()
        - table_box.dimensions.border.horizontal();

    // ── Resolve per-column border width (for collapsed mode) ──────────
    // Use the maximum cell border width found in the first row as reference.
    let cell_border = if collapsed {
        first_cell_border_width(table_box)
    } else {
        0.0
    };

    // ── Adjust cell borders for collapsed mode ────────────────────────
    if collapsed {
        adjust_collapsed_borders(table_box, cell_border);
    }

    // ── Compute column widths (content-aware) ─────────────────────────
    let col_widths = compute_column_widths(
        table_box,
        available,
        num_cols,
        font_size,
        collapsed,
        cell_border,
    );

    // ── Layout rows ───────────────────────────────────────────────────
    let mut y = 0.0;
    let mut row_index = 0usize;

    layout_rows(
        &mut table_box.children,
        &col_widths,
        available,
        font_size,
        &mut y,
        &mut row_index,
        collapsed,
        cell_border,
    );

    table_box.dimensions.content.width = available;
    table_box.dimensions.content.height = y;
}

// ── Column width computation ──────────────────────────────────────────────

/// Compute column widths using a hybrid approach:
/// 1. Measure the minimum/preferred width of each column's content
/// 2. Distribute remaining space proportionally
fn compute_column_widths(
    table_box: &LayoutBox,
    available: f64,
    num_cols: usize,
    font_size: f64,
    collapsed: bool,
    cell_border: f64,
) -> Vec<f64> {
    // Collect preferred and minimum widths per column
    let mut col_pref: Vec<f64> = vec![0.0; num_cols];
    let mut col_min: Vec<f64> = vec![40.0; num_cols]; // minimum 40px per column

    let mut rows_sampled = 0usize;
    collect_column_widths(
        &table_box.children,
        &mut col_pref,
        &mut col_min,
        font_size,
        &mut rows_sampled,
    );

    // (use first cell's padding as representative)
    let cell_overhead = first_cell_padding_h(table_box)
        + if collapsed {
            cell_border
        } else {
            first_cell_border_h(table_box)
        };

    for i in 0..num_cols {
        col_pref[i] += cell_overhead;
        col_min[i] += cell_overhead;
    }

    let total_pref: f64 = col_pref.iter().sum();

    if total_pref <= available && total_pref > 0.0 {
        // Everything fits — scale up proportionally to fill the table
        let scale = available / total_pref;
        col_pref.iter().map(|w| w * scale).collect()
    } else if total_pref > available {
        // Shrink proportionally but respect minimums
        let total_min: f64 = col_min.iter().sum();
        if total_min >= available {
            // Just use equal widths — all columns are at minimum
            vec![available / num_cols as f64; num_cols]
        } else {
            // Distribute: give each column its min, then distribute remaining
            // space proportionally to (pref - min).
            let remaining = available - total_min;
            let total_flex: f64 = col_pref
                .iter()
                .zip(col_min.iter())
                .map(|(p, m)| (p - m).max(0.0))
                .sum();
            if total_flex > 0.0 {
                col_pref
                    .iter()
                    .zip(col_min.iter())
                    .map(|(p, m)| {
                        let flex = (p - m).max(0.0);
                        m + remaining * flex / total_flex
                    })
                    .collect()
            } else {
                vec![available / num_cols as f64; num_cols]
            }
        }
    } else {
        // No content measured — equal widths
        vec![available / num_cols as f64; num_cols]
    }
}

/// Maximum rows sampled when estimating column widths. After this many rows the
/// max-width is almost always stable, so we skip the remaining body rows.
/// Header rows (in `<thead>`) always count and are scanned first.
const MAX_SAMPLE_ROWS: usize = 50;

/// Walk rows and measure preferred / minimum content width for each column.
fn collect_column_widths(
    children: &[LayoutBox],
    col_pref: &mut Vec<f64>,
    col_min: &mut Vec<f64>,
    font_size: f64,
    rows_sampled: &mut usize,
) {
    for child in children {
        if *rows_sampled >= MAX_SAMPLE_ROWS {
            return;
        }
        match child.box_type {
            LayoutBoxType::TableRow => {
                let mut col = 0;
                for cell in &child.children {
                    if !matches!(cell.box_type, LayoutBoxType::TableCell) {
                        continue;
                    }
                    if col >= col_pref.len() {
                        break;
                    }
                    let text = collect_text(cell);
                    if !text.is_empty() {
                        let family = cell.style.font_family();
                        let weight = cell.style.font_weight();
                        let fs = cell.style.font_size_px(font_size, font_size);
                        let font_name = writer::resolve_builtin_font_name(family, weight, false);
                        let w = metrics::measure_text_width_px(&text, font_name, fs);
                        col_pref[col] = col_pref[col].max(w);
                        // Minimum = width of the longest single word in the cell
                        let min_w = text
                            .split_whitespace()
                            .map(|word| metrics::measure_text_width_px(word, font_name, fs))
                            .fold(0.0_f64, f64::max);
                        col_min[col] = col_min[col].max(min_w);
                    }
                    col += 1;
                }
                *rows_sampled += 1;
            }
            LayoutBoxType::Block | LayoutBoxType::AnonymousBlock => {
                let tag = child.tag_name.as_deref().unwrap_or("");
                if matches!(tag, "thead" | "tbody" | "tfoot") {
                    collect_column_widths(
                        &child.children,
                        col_pref,
                        col_min,
                        font_size,
                        rows_sampled,
                    );
                }
            }
            _ => {}
        }
    }
}

// ── Collapsed-border helpers ──────────────────────────────────────────────

/// Find the border width from the first cell encountered (used as reference for collapse).
fn first_cell_border_width(table_box: &LayoutBox) -> f64 {
    for child in &table_box.children {
        match child.box_type {
            LayoutBoxType::TableRow => {
                for cell in &child.children {
                    if matches!(cell.box_type, LayoutBoxType::TableCell) {
                        return cell.dimensions.border.left.max(cell.dimensions.border.top);
                    }
                }
            }
            LayoutBoxType::Block | LayoutBoxType::AnonymousBlock => {
                let tag = child.tag_name.as_deref().unwrap_or("");
                if matches!(tag, "thead" | "tbody" | "tfoot") {
                    let val = first_cell_border_width(child);
                    if val > 0.0 {
                        return val;
                    }
                }
            }
            _ => {}
        }
    }
    0.0
}

fn first_cell_padding_h(table_box: &LayoutBox) -> f64 {
    for child in &table_box.children {
        match child.box_type {
            LayoutBoxType::TableRow => {
                for cell in &child.children {
                    if matches!(cell.box_type, LayoutBoxType::TableCell) {
                        return cell.dimensions.padding.horizontal();
                    }
                }
            }
            LayoutBoxType::Block | LayoutBoxType::AnonymousBlock => {
                let tag = child.tag_name.as_deref().unwrap_or("");
                if matches!(tag, "thead" | "tbody" | "tfoot") {
                    let val = first_cell_padding_h(child);
                    if val > 0.0 {
                        return val;
                    }
                }
            }
            _ => {}
        }
    }
    0.0
}

fn first_cell_border_h(table_box: &LayoutBox) -> f64 {
    for child in &table_box.children {
        match child.box_type {
            LayoutBoxType::TableRow => {
                for cell in &child.children {
                    if matches!(cell.box_type, LayoutBoxType::TableCell) {
                        return cell.dimensions.border.horizontal();
                    }
                }
            }
            LayoutBoxType::Block | LayoutBoxType::AnonymousBlock => {
                let tag = child.tag_name.as_deref().unwrap_or("");
                if matches!(tag, "thead" | "tbody" | "tfoot") {
                    let val = first_cell_border_h(child);
                    if val > 0.0 {
                        return val;
                    }
                }
            }
            _ => {}
        }
    }
    0.0
}

/// In collapsed mode, remove inner borders so adjacent cells share a single border.
/// First column keeps left border; subsequent columns set left border to 0.
/// First row keeps top border; subsequent rows set top border to 0.
fn adjust_collapsed_borders(table_box: &mut LayoutBox, _cell_border: f64) {
    let mut row_index = 0usize;
    adjust_collapsed_borders_inner(&mut table_box.children, &mut row_index);
}

fn adjust_collapsed_borders_inner(children: &mut [LayoutBox], row_index: &mut usize) {
    for child in children.iter_mut() {
        match child.box_type {
            LayoutBoxType::TableRow => {
                let mut col_index = 0usize;
                for cell in child.children.iter_mut() {
                    if !matches!(cell.box_type, LayoutBoxType::TableCell) {
                        continue;
                    }
                    // Remove left border for non-first columns
                    if col_index > 0 {
                        cell.dimensions.border.left = 0.0;
                    }
                    // Remove top border for non-first rows
                    if *row_index > 0 {
                        cell.dimensions.border.top = 0.0;
                    }
                    col_index += 1;
                }
                *row_index += 1;
            }
            LayoutBoxType::Block | LayoutBoxType::AnonymousBlock => {
                let tag = child.tag_name.as_deref().unwrap_or("");
                if matches!(tag, "thead" | "tbody" | "tfoot") {
                    adjust_collapsed_borders_inner(&mut child.children, row_index);
                }
            }
            _ => {}
        }
    }
}

// ── Row layout ────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn layout_rows(
    children: &mut [LayoutBox],
    col_widths: &[f64],
    available: f64,
    font_size: f64,
    y: &mut f64,
    row_index: &mut usize,
    collapsed: bool,
    cell_border: f64,
) {
    for child in children.iter_mut() {
        match child.box_type {
            LayoutBoxType::TableRow => {
                layout_row(
                    child,
                    col_widths,
                    available,
                    font_size,
                    y,
                    *row_index,
                    collapsed,
                    cell_border,
                );
                *row_index += 1;
            }
            LayoutBoxType::Block | LayoutBoxType::AnonymousBlock => {
                let tag = child.tag_name.as_deref().unwrap_or("");
                if matches!(tag, "thead" | "tbody" | "tfoot") {
                    child.dimensions.content.y = *y;
                    child.dimensions.content.width = available;
                    let section_y_start = *y;
                    layout_rows(
                        &mut child.children,
                        col_widths,
                        available,
                        font_size,
                        y,
                        row_index,
                        collapsed,
                        cell_border,
                    );
                    child.dimensions.content.height = *y - section_y_start;
                }
            }
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn layout_row(
    row: &mut LayoutBox,
    col_widths: &[f64],
    available: f64,
    font_size: f64,
    y: &mut f64,
    _row_index: usize,
    _collapsed: bool,
    _cell_border: f64,
) {
    let mut x = 0.0;
    let mut row_height: f64 = 0.0;
    let mut col = 0;

    for cell in row.children.iter_mut() {
        if !matches!(cell.box_type, LayoutBoxType::TableCell) {
            continue;
        }

        let cw = if col < col_widths.len() {
            col_widths[col]
        } else {
            col_widths.last().copied().unwrap_or(100.0)
        };

        cell.dimensions.content.x = x + cell.dimensions.padding.left + cell.dimensions.border.left;
        cell.dimensions.content.y = cell.dimensions.padding.top + cell.dimensions.border.top;
        cell.dimensions.content.width =
            cw - cell.dimensions.padding.horizontal() - cell.dimensions.border.horizontal();

        // Estimate cell height using real font metrics + cell's actual font
        let cell_content_height =
            estimate_text_height_measured(cell, cell.dimensions.content.width, font_size);
        cell.dimensions.content.height = cell_content_height;

        let total_cell_height = cell.dimensions.padding.vertical()
            + cell.dimensions.border.vertical()
            + cell_content_height;
        row_height = row_height.max(total_cell_height);

        x += cw;
        col += 1;
    }

    // Set row dimensions
    row.dimensions.content.x = 0.0;
    row.dimensions.content.y = *y;
    row.dimensions.content.width = available;
    row.dimensions.content.height = row_height;

    // Equalize cell heights within the row
    for cell in row.children.iter_mut() {
        if matches!(cell.box_type, LayoutBoxType::TableCell) {
            cell.dimensions.content.height =
                row_height - cell.dimensions.padding.vertical() - cell.dimensions.border.vertical();
        }
    }

    *y += row_height;
}

// ── Column counting ───────────────────────────────────────────────────────

fn count_columns(table_box: &LayoutBox) -> usize {
    count_columns_in_children(&table_box.children)
}

fn count_columns_in_children(children: &[LayoutBox]) -> usize {
    let mut max_cols = 0;
    for child in children {
        match child.box_type {
            LayoutBoxType::TableRow => {
                let cols = child
                    .children
                    .iter()
                    .filter(|c| matches!(c.box_type, LayoutBoxType::TableCell))
                    .count();
                max_cols = max_cols.max(cols);
            }
            LayoutBoxType::Block | LayoutBoxType::AnonymousBlock => {
                let tag = child.tag_name.as_deref().unwrap_or("");
                if matches!(tag, "thead" | "tbody" | "tfoot") {
                    max_cols = max_cols.max(count_columns_in_children(&child.children));
                }
            }
            _ => {}
        }
    }
    max_cols
}

// ── Height estimation ─────────────────────────────────────────────────────

/// Estimate text content height using the cell's actual font.
fn estimate_text_height_measured(
    box_node: &LayoutBox,
    available_width: f64,
    parent_font_size: f64,
) -> f64 {
    let fs = box_node
        .style
        .font_size_px(parent_font_size, parent_font_size);
    let line_height = box_node.style.line_height(fs);
    let text = collect_text(box_node);

    if text.is_empty() {
        return line_height;
    }

    let family = box_node.style.font_family();
    let weight = box_node.style.font_weight();
    let font_name = writer::resolve_builtin_font_name(family, weight, false);

    let lines = metrics::wrap_text_measured(&text, font_name, fs, available_width);
    (lines.len() as f64 * line_height).max(line_height)
}

/// Collect all text content from a layout box recursively.
fn collect_text(box_node: &LayoutBox) -> String {
    let mut text = String::new();
    if let Some(t) = &box_node.text {
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(t);
    }
    for child in &box_node.children {
        let child_text = collect_text(child);
        if !child_text.is_empty() {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(&child_text);
        }
    }
    text
}
