//! The collapsing border model (CSS 2.1 §17.6.2).
//!
//! Adjacent cells of a `border-collapse: collapse` table share one border per
//! edge. Before painting, every shared edge is resolved once:
//!
//! * row and row-group borders are folded into the cells along them
//!   (a `<tr>` or `<thead>` is not a box of its own in ferropdf's grid);
//! * between two cells, the winning side (§17.6.2.1: wider, then by style,
//!   then the upper / left cell — the right one in a right-to-left table) is
//!   given to the upper or left cell, and the lower or right cell's side is
//!   cleared, so each edge is painted exactly once.

use std::collections::HashMap;

use ferropdf_core::{BorderSide, BorderStyle, Display, Document, LayoutBox, NodeId};
use ferropdf_style::StyleTree;

/// Resolve the collapsed borders of `cells`, the laid-out cells of one table.
pub(crate) fn resolve_collapsed_borders(
    cells: &mut [LayoutBox],
    doc: &Document,
    styles: &StyleTree,
    rtl: bool,
) {
    fold_row_and_group_borders(cells, doc, styles);
    resolve_shared_edges(cells, rtl);
}

/// `b` replaces `a` only when it strictly wins (§17.6.2.1); ties keep `a`,
/// so callers pass the higher-priority side (cell over row, upper/left cell)
/// first.
fn winner(a: &BorderSide, b: &BorderSide) -> BorderSide {
    let visible = |s: &BorderSide| s.style != BorderStyle::None && s.width > 0.0;
    match (visible(a), visible(b)) {
        (_, false) => *a,
        (false, true) => *b,
        (true, true) => {
            let rank = |s: &BorderStyle| match s {
                BorderStyle::Double => 4,
                BorderStyle::Solid => 3,
                BorderStyle::Dashed => 2,
                BorderStyle::Dotted => 1,
                BorderStyle::None => 0,
            };
            if b.width > a.width + 0.01
                || ((b.width - a.width).abs() <= 0.01 && rank(&b.style) > rank(&a.style))
            {
                *b
            } else {
                *a
            }
        }
    }
}

const NONE: BorderSide = BorderSide {
    width: 0.0,
    color: ferropdf_core::Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    },
    style: BorderStyle::None,
};

/// Parent of `id` when it has display `want` (one of `want`).
fn parent_with(doc: &Document, styles: &StyleTree, id: NodeId, want: &[Display]) -> Option<NodeId> {
    let p = doc.get(id).parent?;
    want.contains(&styles.get(&p)?.display).then_some(p)
}

/// Rows (`display: table-row` children) of a row group, in order.
fn rows_of(doc: &Document, styles: &StyleTree, group: NodeId) -> Vec<NodeId> {
    doc.get(group)
        .children
        .iter()
        .copied()
        .filter(|c| {
            styles
                .get(c)
                .is_some_and(|s| s.display == Display::TableRow)
        })
        .collect()
}

fn fold_row_and_group_borders(cells: &mut [LayoutBox], doc: &Document, styles: &StyleTree) {
    const GROUPS: [Display; 3] = [
        Display::TableHeaderGroup,
        Display::TableRowGroup,
        Display::TableFooterGroup,
    ];
    // Visual extent of each row (leftmost / rightmost cell edge), for the
    // row's own left and right borders.
    let mut row_x: HashMap<NodeId, (f32, f32)> = HashMap::new();
    let mut cell_row: Vec<Option<NodeId>> = Vec::with_capacity(cells.len());
    for c in cells.iter() {
        let row = c
            .node_id
            .and_then(|id| parent_with(doc, styles, id, &[Display::TableRow]));
        if let Some(r) = row {
            let e = row_x.entry(r).or_insert((f32::MAX, f32::MIN));
            e.0 = e.0.min(c.rect.x);
            e.1 = e.1.max(c.rect.x + c.rect.width);
        }
        cell_row.push(row);
    }
    let mut group_rows: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    for (c, row) in cells.iter_mut().zip(&cell_row) {
        let Some(row) = *row else { continue };
        let Some(rs) = styles.get(&row) else { continue };
        let (left, right) = row_x[&row];
        let leftmost = (c.rect.x - left).abs() < 0.5;
        let rightmost = (c.rect.x + c.rect.width - right).abs() < 0.5;
        let s = &mut c.style;
        s.border_top = winner(&s.border_top, &rs.border_top);
        s.border_bottom = winner(&s.border_bottom, &rs.border_bottom);
        if leftmost {
            s.border_left = winner(&s.border_left, &rs.border_left);
        }
        if rightmost {
            s.border_right = winner(&s.border_right, &rs.border_right);
        }
        let Some(group) = parent_with(doc, styles, row, &GROUPS) else {
            continue;
        };
        let Some(gs) = styles.get(&group) else {
            continue;
        };
        let rows = group_rows
            .entry(group)
            .or_insert_with(|| rows_of(doc, styles, group));
        if rows.first() == Some(&row) {
            s.border_top = winner(&s.border_top, &gs.border_top);
        }
        if rows.last() == Some(&row) {
            s.border_bottom = winner(&s.border_bottom, &gs.border_bottom);
        }
        if leftmost {
            s.border_left = winner(&s.border_left, &gs.border_left);
        }
        if rightmost {
            s.border_right = winner(&s.border_right, &gs.border_right);
        }
    }
}

/// Edge index: cells keyed by one coordinate of an edge (quantised), each
/// list sorted by the start of the cell's extent along the edge.
struct EdgeIndex(HashMap<i64, Vec<(f32, f32, usize)>>);

fn key(v: f32) -> i64 {
    (v * 4.0).round() as i64
}

impl EdgeIndex {
    fn new(entries: impl Iterator<Item = (f32, f32, f32, usize)>) -> Self {
        let mut map: HashMap<i64, Vec<(f32, f32, usize)>> = HashMap::new();
        for (at, start, end, i) in entries {
            map.entry(key(at)).or_default().push((start, end, i));
        }
        for v in map.values_mut() {
            v.sort_by(|a, b| a.0.total_cmp(&b.0));
        }
        EdgeIndex(map)
    }

    /// Cells with an edge at `at` overlapping `[start, end)` by more than 0.5pt.
    fn overlapping(&self, at: f32, start: f32, end: f32) -> impl Iterator<Item = usize> + '_ {
        let k = key(at);
        (k - 2..=k + 2)
            .filter_map(|k| self.0.get(&k))
            .flat_map(move |v| {
                let from = v.partition_point(|e| e.1 <= start + 0.5);
                v[from..]
                    .iter()
                    .take_while(move |e| e.0 < end - 0.5)
                    .map(|e| e.2)
            })
    }
}

fn resolve_shared_edges(cells: &mut [LayoutBox], rtl: bool) {
    let tops = EdgeIndex::new(
        cells
            .iter()
            .enumerate()
            .map(|(i, c)| (c.rect.y, c.rect.x, c.rect.x + c.rect.width, i)),
    );
    let lefts = EdgeIndex::new(
        cells
            .iter()
            .enumerate()
            .map(|(i, c)| (c.rect.x, c.rect.y, c.rect.y + c.rect.height, i)),
    );
    let mut bottom: Vec<BorderSide> = cells.iter().map(|c| c.style.border_bottom).collect();
    let mut right: Vec<BorderSide> = cells.iter().map(|c| c.style.border_right).collect();
    let mut clear_top = vec![false; cells.len()];
    let mut clear_left = vec![false; cells.len()];

    for (i, c) in cells.iter().enumerate() {
        let r = c.rect;
        // Cells below `c`: their top edge lies on c's bottom edge.
        for j in tops.overlapping(r.y + r.height, r.x, r.x + r.width) {
            if j == i || (cells[j].rect.y - (r.y + r.height)).abs() > 0.5 {
                continue;
            }
            bottom[i] = winner(&bottom[i], &cells[j].style.border_top);
            clear_top[j] = true;
        }
        // Cells to the right of `c`.
        for j in lefts.overlapping(r.x + r.width, r.y, r.y + r.height) {
            if j == i || (cells[j].rect.x - (r.x + r.width)).abs() > 0.5 {
                continue;
            }
            let theirs = &cells[j].style.border_left;
            // Ties go to the cell on the start side: left, or right in RTL.
            right[i] = if rtl {
                winner(theirs, &right[i])
            } else {
                winner(&right[i], theirs)
            };
            clear_left[j] = true;
        }
    }
    for (i, c) in cells.iter_mut().enumerate() {
        c.style.border_bottom = bottom[i];
        c.style.border_right = right[i];
        if clear_top[i] {
            c.style.border_top = NONE;
        }
        if clear_left[i] {
            c.style.border_left = NONE;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferropdf_core::{Color, ComputedStyle, Rect};

    fn side(width: f32, style: BorderStyle, rgb: (u8, u8, u8)) -> BorderSide {
        BorderSide {
            width,
            color: Color::from_rgb8(rgb.0, rgb.1, rgb.2),
            style,
        }
    }

    fn same(a: BorderSide, b: BorderSide) -> bool {
        a.width == b.width && a.style == b.style && a.color == b.color
    }

    fn cell(x: f32, y: f32, w: f32, h: f32, all: BorderSide) -> LayoutBox {
        LayoutBox {
            style: ComputedStyle {
                display: Display::TableCell,
                border_top: all,
                border_right: all,
                border_bottom: all,
                border_left: all,
                ..Default::default()
            },
            rect: Rect::new(x, y, w, h),
            ..Default::default()
        }
    }

    #[test]
    fn winner_follows_css21_precedence() {
        let thin = side(1.0, BorderStyle::Solid, (0, 0, 0));
        let thick = side(3.0, BorderStyle::Solid, (0, 128, 0));
        let dotted = side(1.0, BorderStyle::Dotted, (255, 0, 0));
        let double = side(1.0, BorderStyle::Double, (0, 0, 255));
        assert!(same(winner(&thin, &thick), thick), "wider wins");
        assert!(same(winner(&thick, &thin), thick));
        assert!(same(winner(&thin, &dotted), thin), "solid beats dotted");
        assert!(same(winner(&thin, &double), double), "double beats solid");
        assert!(
            same(winner(&NONE, &dotted), dotted),
            "any visible border beats none"
        );
        let other = side(1.0, BorderStyle::Solid, (9, 9, 9));
        assert!(
            same(winner(&thin, &other), thin),
            "full tie keeps the first"
        );
    }

    #[test]
    fn thicker_lower_border_wins_and_is_painted_once() {
        let grey = side(0.75, BorderStyle::Solid, (204, 204, 204));
        let green = side(2.25, BorderStyle::Solid, (0, 128, 0));
        let mut cells = vec![
            cell(0.0, 0.0, 100.0, 20.0, grey),
            cell(0.0, 20.0, 100.0, 20.0, grey),
        ];
        cells[1].style.border_top = green;
        resolve_shared_edges(&mut cells, false);
        assert!(same(cells[0].style.border_bottom, green));
        assert_eq!(cells[1].style.border_top.style, BorderStyle::None);
    }

    #[test]
    fn colspan_cell_resolves_each_segment() {
        // One cell spanning two columns above two cells; the right one has a
        // thick top: only that segment changes.
        let thin = side(1.0, BorderStyle::Solid, (0, 0, 0));
        let thick = side(3.0, BorderStyle::Solid, (255, 0, 0));
        let mut cells = vec![
            cell(0.0, 0.0, 100.0, 20.0, thin),
            cell(100.0, 0.0, 100.0, 20.0, thin),
            cell(0.0, 20.0, 200.0, 20.0, thin),
        ];
        cells[2].style.border_top = thick;
        resolve_shared_edges(&mut cells, false);
        assert!(same(cells[0].style.border_bottom, thick));
        assert!(same(cells[1].style.border_bottom, thick));
        assert_eq!(cells[2].style.border_top.style, BorderStyle::None);
        // Horizontal edge between the two upper cells: tie → left keeps it.
        assert!(same(cells[0].style.border_right, thin));
        assert_eq!(cells[1].style.border_left.style, BorderStyle::None);
    }

    #[test]
    fn rtl_tie_goes_to_the_right_cell() {
        let a = side(1.0, BorderStyle::Solid, (255, 0, 0));
        let b = side(1.0, BorderStyle::Solid, (0, 0, 255));
        let mut cells = vec![
            cell(0.0, 0.0, 50.0, 20.0, a),
            cell(50.0, 0.0, 50.0, 20.0, b),
        ];
        resolve_shared_edges(&mut cells, true);
        assert!(same(cells[0].style.border_right, b));
    }
}
