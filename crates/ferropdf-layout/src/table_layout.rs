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
    // Phase 0: collect rows
    let rows = collect_table_rows(doc, table_id, styles);
    let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(1).max(1);

    if rows.is_empty() {
        return TableLayoutResult {
            column_widths: vec![],
            row_heights: vec![],
            total_width: 0.0,
            total_height: 0.0,
            taffy_columns: vec![],
            taffy_rows: vec![],
        };
    }

    // Phase 1: compute column widths
    let column_widths =
        compute_column_widths(&rows, num_cols, table_width, doc, styles, font_system);

    // Phase 2: compute row heights
    let row_heights = compute_row_heights(&rows, &column_widths, doc, styles, font_system);

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

fn compute_column_widths(
    rows: &[Vec<NodeId>],
    num_cols: usize,
    table_width: f32,
    doc: &Document,
    styles: &StyleTree,
    font_system: &mut FontSystem,
) -> Vec<f32> {
    // Step 1: Fixed widths from CSS width on first cell of each column
    let fixed_widths: Vec<Option<f32>> = (0..num_cols)
        .map(|col_idx| {
            rows.iter()
                .find_map(|row| row.get(col_idx))
                .and_then(|&cell_id| styles.get(&cell_id))
                .and_then(|style| match &style.width {
                    Length::Pt(v) => Some(*v),
                    Length::Px(px) => Some(*px),
                    Length::Percent(p) => Some(table_width * p / 100.0),
                    _ => None,
                })
        })
        .collect();

    // Step 2: Min-content width per column
    let min_content_widths: Vec<f32> = (0..num_cols)
        .map(|col_idx| {
            rows.iter()
                .filter_map(|row| row.get(col_idx))
                .map(|&cell_id| {
                    let text = collect_text_content(doc, cell_id);
                    if text.is_empty() {
                        return 0.0;
                    }
                    let style = styles.get(&cell_id).cloned().unwrap_or_default();
                    let font_size = style.font_size;
                    let font_family = style
                        .font_family
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "sans-serif".to_string());
                    let pad_h = resolve_px(&style.padding[1]) + resolve_px(&style.padding[3]);

                    measure_min_content_width(&text, font_size, &font_family, font_system) + pad_h
                })
                .fold(0.0_f32, f32::max)
        })
        .collect();

    // Step 3: Distribution
    let fixed_total: f32 = fixed_widths.iter().filter_map(|w| *w).sum();
    let available_for_flexible = (table_width - fixed_total).max(0.0);

    // Distribute available space proportionally to min-content widths.
    // CSS §17.5: columns never shrink below their min-content width.
    // If total min-content > available, the table overflows (columns keep min-content).
    // If total min-content <= available, extra space is distributed proportionally.
    let total_min: f32 = min_content_widths.iter().sum();

    (0..num_cols)
        .map(|i| {
            if let Some(fixed) = fixed_widths[i] {
                fixed
            } else if total_min <= 0.0 {
                available_for_flexible / (num_cols as f32)
            } else if total_min <= available_for_flexible {
                // Enough space: min-content + proportional share of remaining
                let remaining = available_for_flexible - total_min;
                let ratio = min_content_widths[i] / total_min;
                min_content_widths[i] + remaining * ratio
            } else {
                // Not enough space: keep min-content width (table overflows)
                min_content_widths[i]
            }
        })
        .collect()
}

// =============================================================================
// PHASE 2 — CALCUL DES HAUTEURS DE LIGNES
// CSS 2.1 §17.5 — Row height = max(height of cells in row)
// =============================================================================

fn compute_row_heights(
    rows: &[Vec<NodeId>],
    column_widths: &[f32],
    doc: &Document,
    styles: &StyleTree,
    font_system: &mut FontSystem,
) -> Vec<f32> {
    rows.iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(col_idx, &cell_id)| {
                    let text = collect_text_content(doc, cell_id);
                    let style = styles.get(&cell_id).cloned().unwrap_or_default();
                    let cell_width = column_widths.get(col_idx).cloned().unwrap_or(50.0);
                    let font_size = style.font_size;
                    let font_family = style
                        .font_family
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "sans-serif".to_string());
                    let padding_v = resolve_px(&style.padding[0]) + resolve_px(&style.padding[2]);
                    let padding_h = resolve_px(&style.padding[1]) + resolve_px(&style.padding[3]);
                    // Measure text at the content width (column width minus cell padding),
                    // matching the width Taffy will give the cell content area.
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
    row_node
        .children
        .iter()
        .filter(|&&child_id| {
            let s = styles.get(&child_id).cloned().unwrap_or_default();
            s.display == FDisplay::TableCell
        })
        .copied()
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
