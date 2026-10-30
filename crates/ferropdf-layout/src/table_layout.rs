// =============================================================================
// table_layout.rs — CSS Table Layout Algorithm (§17.5)
// =============================================================================
// Based on Blink’s algorithms (BSD license):
//   blink/renderer/core/layout/layout_table.cc
//   blink/renderer/core/layout/layout_table_cell.cc
//
// This module runs BEFORE Taffy.
// It computes column widths and row heights, then builds
// Taffy TrackSizingFunction for CSS Grid.
// =============================================================================

use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Wrap};
use ferropdf_core::Display as FDisplay;
use ferropdf_core::{Document, Length, NodeId};
use ferropdf_style::StyleTree;
use taffy::{
    LengthPercentage, MaxTrackSizingFunction, MinMax, MinTrackSizingFunction, TrackSizingFunction,
};

// =============================================================================
// DATA STRUCTURES
// =============================================================================

/// Result of a table layout computation.
#[derive(Debug)]
pub struct TableLayoutResult {
    pub column_widths: Vec<f32>,
    pub row_heights: Vec<f32>,
    pub total_width: f32,
    pub total_height: f32,
    pub taffy_columns: Vec<TrackSizingFunction>,
    pub taffy_rows: Vec<TrackSizingFunction>,
}

/// Info about a single table cell, including colspan.
#[derive(Debug, Clone)]
pub struct CellInfo {
    pub node_id: NodeId,
    pub colspan: usize,
}

// =============================================================================
// ENTRY POINT — FULL TABLE LAYOUT COMPUTATION
// =============================================================================

/// Compute the full table layout: columns, rows, Taffy tracks.
///
/// 4 phases (CSS 2.1 §17.5):
///   0. Build table grid (collect rows/cells from DOM)
///   1. Compute column widths (fixed + min-content + distribution)
///   2. Compute row heights
///   3. Build Taffy track sizing functions
pub fn compute_table_layout(
    table_id: NodeId,
    table_width: f32,
    doc: &Document,
    styles: &StyleTree,
    font_system: &mut FontSystem,
) -> TableLayoutResult {
    // Phase 0: collect rows with colspan info
    let cell_rows = collect_table_cell_infos(doc, table_id, styles);
    // Effective number of columns (accounting for colspan)
    let num_cols = cell_rows
        .iter()
        .map(|r| r.iter().map(|c| c.colspan).sum::<usize>())
        .max()
        .unwrap_or(1)
        .max(1);

    if cell_rows.is_empty() {
        return TableLayoutResult {
            column_widths: vec![],
            row_heights: vec![],
            total_width: 0.0,
            total_height: 0.0,
            taffy_columns: vec![],
            taffy_rows: vec![],
        };
    }

    // Phase 1: compute column widths (colspan-aware)
    let column_widths = compute_column_widths_with_colspan(
        &cell_rows,
        num_cols,
        table_width,
        doc,
        styles,
        font_system,
    );

    log::debug!(
        "[TABLE] rows={} cols={} table_width={:.1}",
        cell_rows.len(),
        num_cols,
        table_width
    );
    log::debug!("[TABLE] column_widths={:?}", column_widths);

    // Phase 2: compute row heights (colspan-aware)
    let row_heights =
        compute_row_heights_with_colspan(&cell_rows, &column_widths, doc, styles, font_system);

    let total_width: f32 = column_widths.iter().sum();
    let total_height: f32 = row_heights.iter().sum();

    // Phase 3: build Taffy tracks
    let taffy_columns = build_taffy_column_tracks(&column_widths);
    let taffy_rows = build_taffy_row_tracks(&row_heights);

    TableLayoutResult {
        column_widths,
        row_heights,
        total_width,
        total_height,
        taffy_columns,
        taffy_rows,
    }
}

// =============================================================================
// PHASE 1 — COLUMN WIDTH COMPUTATION
// CSS 2.1 §17.5.2 (fixed) + §17.5.3 (simplified auto)
// =============================================================================

fn compute_column_widths_with_colspan(
    cell_rows: &[Vec<CellInfo>],
    num_cols: usize,
    table_width: f32,
    doc: &Document,
    styles: &StyleTree,
    font_system: &mut FontSystem,
) -> Vec<f32> {
    // Step 1: Fixed widths from CSS width on cells with colspan=1
    let fixed_widths: Vec<Option<f32>> = (0..num_cols)
        .map(|col_idx| {
            cell_rows
                .iter()
                .find_map(|row| {
                    let mut effective_col = 0;
                    for cell in row {
                        if effective_col == col_idx && cell.colspan == 1 {
                            return Some(cell.node_id);
                        }
                        effective_col += cell.colspan;
                        if effective_col > col_idx {
                            break;
                        }
                    }
                    None
                })
                .and_then(|cell_id| styles.get(&cell_id))
                .and_then(|style| match &style.width {
                    Length::Pt(v) => Some(*v),
                    Length::Px(px) => Some(*px),
                    Length::Percent(p) => Some(table_width * p / 100.0),
                    _ => None,
                })
        })
        .collect();

    // Step 2: min-content (longest word / widest fixed-width descendant) and
    // max-content (no wrapping) widths per column, from cells with colspan=1.
    let mut min_content_widths = vec![0.0_f32; num_cols];
    let mut max_content_widths = vec![0.0_f32; num_cols];
    for row in cell_rows {
        let mut effective_col = 0;
        for cell in row {
            if cell.colspan == 1 && effective_col < num_cols {
                let (min_w, max_w) = cell_content_widths(cell.node_id, doc, styles, font_system);
                min_content_widths[effective_col] = min_content_widths[effective_col].max(min_w);
                max_content_widths[effective_col] = max_content_widths[effective_col].max(max_w);
            }
            effective_col += cell.colspan;
        }
    }

    // Step 3: Distribution (CSS 2.1 §17.5.2.2 auto table layout, simplified):
    //   * everything fits at max-content → max-content + proportional extra;
    //   * otherwise interpolate between min-content and max-content so the
    //     columns exactly fill the table;
    //   * if even min-content does not fit, use min-content (table overflows).
    let fixed_total: f32 = fixed_widths.iter().filter_map(|w| *w).sum();
    let available_for_flexible = (table_width - fixed_total).max(0.0);

    log::debug!("[TABLE] fixed_widths={:?}", fixed_widths);
    log::debug!("[TABLE] min_content_widths={:?}", min_content_widths);
    log::debug!("[TABLE] max_content_widths={:?}", max_content_widths);

    let flexible = |i: usize| fixed_widths[i].is_none();
    let total_min: f32 = (0..num_cols)
        .filter(|&i| flexible(i))
        .map(|i| min_content_widths[i])
        .sum();
    let total_max: f32 = (0..num_cols)
        .filter(|&i| flexible(i))
        .map(|i| max_content_widths[i])
        .sum();
    let flexible_count = (0..num_cols).filter(|&i| flexible(i)).count().max(1);

    let mut widths: Vec<f32> = (0..num_cols)
        .map(|i| {
            if let Some(fixed) = fixed_widths[i] {
                fixed
            } else if total_max <= 0.0 {
                available_for_flexible / flexible_count as f32
            } else if total_max <= available_for_flexible {
                let remaining = available_for_flexible - total_max;
                max_content_widths[i] + remaining * (max_content_widths[i] / total_max)
            } else if total_min < available_for_flexible && total_max > total_min {
                let t = (available_for_flexible - total_min) / (total_max - total_min);
                min_content_widths[i] + (max_content_widths[i] - min_content_widths[i]) * t
            } else {
                min_content_widths[i]
            }
        })
        .collect();

    // Step 4: Colspan cells may need wider columns. Distribute their width
    // across the spanned columns if the sum is less than the cell's content.
    for row in cell_rows {
        let mut effective_col = 0;
        for cell in row {
            if cell.colspan > 1 {
                let text = collect_text_content(doc, cell.node_id);
                let style = styles.get(&cell.node_id).cloned().unwrap_or_default();
                let font_size = style.font_size;
                let font_family = style
                    .font_family
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "sans-serif".to_string());
                let pad_h = resolve_px(&style.padding[1]) + resolve_px(&style.padding[3]);
                // Never widen spanned columns past the table itself.
                let needed = if text.is_empty() {
                    0.0
                } else {
                    (measure_min_content_width(&text, font_size, &font_family, font_system) + pad_h)
                        .min(table_width)
                };

                let end_col = (effective_col + cell.colspan).min(num_cols);
                let current_sum: f32 = widths[effective_col..end_col].iter().sum();
                if needed > current_sum && end_col > effective_col {
                    let deficit = needed - current_sum;
                    let per_col = deficit / (end_col - effective_col) as f32;
                    for w in &mut widths[effective_col..end_col] {
                        *w += per_col;
                    }
                }
            }
            effective_col += cell.colspan;
        }
    }

    widths
}

fn compute_row_heights_with_colspan(
    cell_rows: &[Vec<CellInfo>],
    column_widths: &[f32],
    doc: &Document,
    styles: &StyleTree,
    font_system: &mut FontSystem,
) -> Vec<f32> {
    cell_rows
        .iter()
        .map(|row| {
            let mut effective_col = 0;
            row.iter()
                .map(|cell| {
                    let text = collect_text_content(doc, cell.node_id);
                    let style = styles.get(&cell.node_id).cloned().unwrap_or_default();
                    // Cell width is the sum of spanned column widths
                    let end_col = (effective_col + cell.colspan).min(column_widths.len());
                    let cell_width: f32 = column_widths[effective_col..end_col].iter().sum();
                    effective_col = end_col;

                    let font_size = style.font_size;
                    let font_family = style
                        .font_family
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "sans-serif".to_string());
                    let padding_v = resolve_px(&style.padding[0]) + resolve_px(&style.padding[2]);
                    let padding_h = resolve_px(&style.padding[1]) + resolve_px(&style.padding[3]);
                    let content_width = (cell_width - padding_h).max(0.0);
                    measure_text_height(&text, content_width, font_size, &font_family, font_system)
                        + padding_v
                })
                .fold(0.0_f32, f32::max)
        })
        .collect()
}

// =============================================================================
// PHASE 3 — CONSTRUCTION DES TRACK SIZING FUNCTIONS TAFFY
// =============================================================================

pub fn build_taffy_column_tracks(column_widths: &[f32]) -> Vec<TrackSizingFunction> {
    column_widths
        .iter()
        .map(|&w| {
            TrackSizingFunction::Single(MinMax {
                min: MinTrackSizingFunction::Fixed(LengthPercentage::Length(w)),
                max: MaxTrackSizingFunction::Fixed(LengthPercentage::Length(w)),
            })
        })
        .collect()
}

pub fn build_taffy_row_tracks(row_heights: &[f32]) -> Vec<TrackSizingFunction> {
    row_heights
        .iter()
        .map(|&h| {
            TrackSizingFunction::Single(MinMax {
                min: MinTrackSizingFunction::Fixed(LengthPercentage::Length(h)),
                max: MaxTrackSizingFunction::Auto,
            })
        })
        .collect()
}

// =============================================================================
// TEXT MEASUREMENT VIA COSMIC-TEXT
// =============================================================================

/// (min-content, max-content) border-box width of a cell's content, in pt.
///
/// min-content is the longest word, or the widest descendant with an explicit
/// length width (a nested `<table style="width:520px">`, an image...).
fn cell_content_widths(
    cell_id: NodeId,
    doc: &Document,
    styles: &StyleTree,
    font_system: &mut FontSystem,
) -> (f32, f32) {
    let style = styles.get(&cell_id).cloned().unwrap_or_default();
    let pad_h = resolve_px(&style.padding[1]) + resolve_px(&style.padding[3]);
    let fixed = widest_fixed_descendant(doc, cell_id, styles);
    let text = collect_text_content(doc, cell_id);
    if text.is_empty() {
        return (fixed + pad_h, fixed + pad_h);
    }
    let font_family = style
        .font_family
        .first()
        .cloned()
        .unwrap_or_else(|| "sans-serif".to_string());
    let max_w = measure_min_content_width(&text, style.font_size, &font_family, font_system);
    let min_w = text
        .split_whitespace()
        .max_by_key(|w| w.chars().count())
        .map(|w| measure_min_content_width(w, style.font_size, &font_family, font_system))
        .unwrap_or(0.0);
    (min_w.max(fixed) + pad_h, max_w.max(fixed) + pad_h)
}

fn widest_fixed_descendant(doc: &Document, node_id: NodeId, styles: &StyleTree) -> f32 {
    let mut widest = 0.0_f32;
    for &child in &doc.get(node_id).children {
        if let Some(style) = styles.get(&child) {
            if let Length::Pt(v) | Length::Px(v) = style.width {
                widest = widest.max(v);
            }
        }
        widest = widest.max(widest_fixed_descendant(doc, child, styles));
    }
    widest
}

/// Width of `text` laid out on a single line (no wrapping) — i.e. its
/// max-content width.
fn measure_min_content_width(
    text: &str,
    font_size: f32,
    font_family: &str,
    font_system: &mut FontSystem,
) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    let line_height = font_size * 1.2;
    let mut buffer = Buffer::new(font_system, Metrics::new(font_size, line_height));
    buffer.set_wrap(font_system, Wrap::None);
    let attrs = Attrs::new().family(Family::Name(font_family));
    buffer.set_text(font_system, text, attrs, Shaping::Advanced);
    buffer.shape_until_scroll(font_system, false);
    buffer
        .layout_runs()
        .map(|run| run.line_w)
        .fold(0.0_f32, f32::max)
}

fn measure_text_height(
    text: &str,
    available_width: f32,
    font_size: f32,
    font_family: &str,
    font_system: &mut FontSystem,
) -> f32 {
    if text.is_empty() {
        return font_size * 1.2;
    }
    let line_height = font_size * 1.2;
    let mut buffer = Buffer::new(font_system, Metrics::new(font_size, line_height));
    buffer.set_wrap(font_system, Wrap::Word);
    buffer.set_size(font_system, Some(available_width), None);
    let attrs = Attrs::new().family(Family::Name(font_family));
    buffer.set_text(font_system, text, attrs, Shaping::Advanced);
    buffer.shape_until_scroll(font_system, false);
    let num_lines = buffer.layout_runs().count().max(1);
    num_lines as f32 * line_height
}

// =============================================================================
// DOM HELPERS
// =============================================================================

/// Collect all rows in a table. Each row is a Vec of cell NodeIds.
pub fn collect_table_rows(
    doc: &Document,
    table_id: NodeId,
    styles: &StyleTree,
) -> Vec<Vec<NodeId>> {
    collect_table_cell_infos(doc, table_id, styles)
        .into_iter()
        .map(|row| row.into_iter().map(|c| c.node_id).collect())
        .collect()
}

/// Collect all rows with colspan info.
pub fn collect_table_cell_infos(
    doc: &Document,
    table_id: NodeId,
    styles: &StyleTree,
) -> Vec<Vec<CellInfo>> {
    // Header groups render first and footer groups last, wherever they
    // appear in the source (CSS 2.1 §17.2: `<tfoot>` may precede `<tbody>`).
    let mut head = Vec::new();
    let mut body = Vec::new();
    let mut foot = Vec::new();
    let table_node = doc.get(table_id);

    for &child_id in &table_node.children {
        let child_style = styles.get(&child_id).cloned().unwrap_or_default();
        let target = match child_style.display {
            FDisplay::TableRow => {
                body.push(collect_cell_infos(doc, child_id, styles));
                continue;
            }
            FDisplay::TableHeaderGroup => &mut head,
            FDisplay::TableRowGroup => &mut body,
            FDisplay::TableFooterGroup => &mut foot,
            _ => continue,
        };
        let group_node = doc.get(child_id);
        for &row_id in &group_node.children {
            let row_style = styles.get(&row_id).cloned().unwrap_or_default();
            if row_style.display == FDisplay::TableRow {
                target.push(collect_cell_infos(doc, row_id, styles));
            }
        }
    }
    head.extend(body);
    head.extend(foot);
    head
}

/// Count the number of rows that come from <tfoot> elements.
pub fn count_tfoot_rows(doc: &Document, table_id: NodeId, styles: &StyleTree) -> usize {
    count_group_rows(doc, table_id, styles, FDisplay::TableFooterGroup)
}

fn count_group_rows(
    doc: &Document,
    table_id: NodeId,
    styles: &StyleTree,
    group: FDisplay,
) -> usize {
    let mut count = 0;
    for &child_id in &doc.get(table_id).children {
        let child_style = styles.get(&child_id).cloned().unwrap_or_default();
        if child_style.display == group {
            for &row_id in &doc.get(child_id).children {
                let row_style = styles.get(&row_id).cloned().unwrap_or_default();
                if row_style.display == FDisplay::TableRow {
                    count += 1;
                }
            }
        }
    }
    count
}

/// Count the number of rows that come from <thead> elements.
pub fn count_thead_rows(doc: &Document, table_id: NodeId, styles: &StyleTree) -> usize {
    let mut count = 0;
    let table_node = doc.get(table_id);
    for &child_id in &table_node.children {
        let child_style = styles.get(&child_id).cloned().unwrap_or_default();
        if child_style.display == FDisplay::TableHeaderGroup {
            let group_node = doc.get(child_id);
            for &row_id in &group_node.children {
                let row_style = styles.get(&row_id).cloned().unwrap_or_default();
                if row_style.display == FDisplay::TableRow {
                    count += 1;
                }
            }
        }
    }
    count
}

fn collect_cell_infos(doc: &Document, row_id: NodeId, styles: &StyleTree) -> Vec<CellInfo> {
    let row_node = doc.get(row_id);
    row_node
        .children
        .iter()
        .filter(|&&child_id| {
            let s = styles.get(&child_id).cloned().unwrap_or_default();
            s.display == FDisplay::TableCell
        })
        .map(|&child_id| {
            let colspan = doc
                .get(child_id)
                .attr("colspan")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(1)
                .max(1);
            CellInfo {
                node_id: child_id,
                colspan,
            }
        })
        .collect()
}

/// Recursively collect all text content from a subtree.
/// Collapses whitespace like HTML: runs of whitespace → single space, then trim.
pub fn collect_text_content(doc: &Document, node_id: NodeId) -> String {
    let mut raw = String::new();
    collect_text_raw(doc, node_id, &mut raw);
    // HTML whitespace collapsing: replace runs of whitespace with a single space
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn collect_text_raw(doc: &Document, node_id: NodeId, out: &mut String) {
    let node = doc.get(node_id);
    if node.is_text() {
        if let Some(ref t) = node.text {
            out.push_str(t);
        }
        return;
    }
    for &child_id in &node.children {
        collect_text_raw(doc, child_id, out);
    }
}

fn resolve_px(length: &Length) -> f32 {
    match length {
        Length::Pt(v) => *v,
        Length::Px(px) => *px,
        Length::Zero => 0.0,
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferropdf_core::{ComputedStyle, Document};
    use std::collections::HashMap;

    fn elem(doc: &mut Document, parent: NodeId, tag: &str) -> NodeId {
        let id = doc.create_element(tag, HashMap::new());
        doc.append_child(parent, id);
        id
    }

    fn elem_attr(doc: &mut Document, parent: NodeId, tag: &str, key: &str, val: &str) -> NodeId {
        let mut attrs = HashMap::new();
        attrs.insert(key.to_string(), val.to_string());
        let id = doc.create_element(tag, attrs);
        doc.append_child(parent, id);
        id
    }

    /// Apply the same default `display` values that compute.rs::apply_tag_defaults
    /// would set in the real pipeline so the table-helper functions can find
    /// rows/cells.
    fn apply_table_display_defaults(doc: &Document, styles: &mut StyleTree) {
        for (id, node) in doc.nodes.iter() {
            let tag = node.tag_name.as_deref().unwrap_or("");
            let display = match tag {
                "table" => FDisplay::Table,
                "thead" => FDisplay::TableHeaderGroup,
                "tbody" => FDisplay::TableRowGroup,
                "tfoot" => FDisplay::TableFooterGroup,
                "tr" => FDisplay::TableRow,
                "td" | "th" => FDisplay::TableCell,
                _ => FDisplay::Block,
            };
            let style = ComputedStyle {
                display,
                ..Default::default()
            };
            styles.insert(id, style);
        }
    }

    /// `<table><tbody><tr><td>a</td><td>b</td></tr><tr><td>c</td><td>d</td></tr></tbody></table>`
    fn build_2x2_table() -> (Document, NodeId, StyleTree) {
        let mut doc = Document::new();
        let root = doc.create_document_root();
        let body = elem(&mut doc, root, "body");
        let table = elem(&mut doc, body, "table");
        let tbody = elem(&mut doc, table, "tbody");
        let r1 = elem(&mut doc, tbody, "tr");
        let r2 = elem(&mut doc, tbody, "tr");
        let _ = elem(&mut doc, r1, "td");
        let _ = elem(&mut doc, r1, "td");
        let _ = elem(&mut doc, r2, "td");
        let _ = elem(&mut doc, r2, "td");
        let mut styles = StyleTree::new();
        apply_table_display_defaults(&doc, &mut styles);
        (doc, table, styles)
    }

    #[test]
    fn collect_rows_descends_through_tbody() {
        let (doc, table, styles) = build_2x2_table();
        let rows = collect_table_rows(&doc, table, &styles);
        assert_eq!(rows.len(), 2, "two <tr> rows must be discovered");
        for row in &rows {
            assert_eq!(row.len(), 2, "each row has two cells");
        }
    }

    #[test]
    fn cell_infos_default_colspan_is_one() {
        let (doc, table, styles) = build_2x2_table();
        let infos = collect_table_cell_infos(&doc, table, &styles);
        for row in infos {
            for cell in row {
                assert_eq!(cell.colspan, 1, "no colspan attribute → colspan=1");
            }
        }
    }

    #[test]
    fn cell_infos_respect_colspan_attribute() {
        let mut doc = Document::new();
        let root = doc.create_document_root();
        let body = elem(&mut doc, root, "body");
        let table = elem(&mut doc, body, "table");
        let tbody = elem(&mut doc, table, "tbody");
        let r1 = elem(&mut doc, tbody, "tr");
        let _ = elem_attr(&mut doc, r1, "td", "colspan", "2");
        let _ = elem(&mut doc, r1, "td");
        let mut styles = StyleTree::new();
        apply_table_display_defaults(&doc, &mut styles);
        let infos = collect_table_cell_infos(&doc, table, &styles);
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].len(), 2);
        assert_eq!(infos[0][0].colspan, 2);
        assert_eq!(infos[0][1].colspan, 1);
    }

    #[test]
    fn taffy_column_tracks_have_fixed_min_max() {
        let widths = [50.0, 100.0, 200.0];
        let tracks = build_taffy_column_tracks(&widths);
        assert_eq!(tracks.len(), 3);
    }

    #[test]
    fn taffy_row_tracks_emit_one_per_row() {
        let heights = [20.0, 30.0, 40.0, 50.0];
        let tracks = build_taffy_row_tracks(&heights);
        assert_eq!(tracks.len(), 4);
    }

    #[test]
    fn resolve_px_handles_known_units() {
        assert_eq!(resolve_px(&Length::Pt(12.0)), 12.0);
        assert_eq!(resolve_px(&Length::Px(24.0)), 24.0);
        assert_eq!(resolve_px(&Length::Zero), 0.0);
        // Em / Rem / Percent / Auto fall back to 0 (resolved upstream).
        assert_eq!(resolve_px(&Length::Auto), 0.0);
    }
}
