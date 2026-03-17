//! Layout engine — builds a LayoutBox tree from a parsed Document.

use ferropdf_core::{EngineConfig, FerroError, Rect};
use ferropdf_parse::{
    css::{
        properties::{ComputedStyle, Display, FlexDirection, Float, GridTrack, JustifyContent, Position, TextAlign},
        resolver::StyleResolver,
        values::Stylesheet,
    },
    html::dom::{Document, NodeId, NodeKind},
};

use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping};
use unicode_bidi::BidiInfo;

use super::box_model::{LayoutBox, LayoutBoxKind, ShapedGlyph, ShapedLine};
use super::table_layout;

const PT_PER_PX: f32 = 0.75; // 1 CSS px = 0.75 pt

// ─── LayoutEngine ─────────────────────────────────────────────────────────────

pub struct LayoutEngine {
    pub config: EngineConfig,
}

impl LayoutEngine {
    pub fn new(config: EngineConfig) -> Self { Self { config } }

    /// Layout a document and return the root LayoutBox.
    pub fn layout(
        &self,
        doc:     &Document,
        sheets:  &[Stylesheet],
    ) -> Result<LayoutBox, FerroError> {
        let resolver = StyleResolver::new(sheets.to_vec());

        // Find body element (or fall back to html root)
        let root_id = doc.body()
            .or_else(|| doc.html_element())
            .unwrap_or(NodeId::root());

        // Initialise cosmic-text FontSystem (loads system fonts)
        let mut font_system = FontSystem::new();

        // Root style (initial containing block)
        let root_style = ComputedStyle::default_root();
        let container_width = self.config.content_width_px();

        let mut root_box = self.build_box(root_id, doc, &root_style, &resolver, &mut font_system)?;

        // Compute layout positions
        self.compute_layout(&mut root_box, container_width, 0.0, &mut font_system);

        Ok(root_box)
    }

    // ─── Build phase ──────────────────────────────────────────────────────────

    fn build_box(
        &self,
        node_id:  NodeId,
        doc:      &Document,
        parent_style: &ComputedStyle,
        resolver: &StyleResolver,
        fs:       &mut FontSystem,
    ) -> Result<LayoutBox, FerroError> {
        let node = doc.get(node_id);
        match &node.kind {
            NodeKind::Text(text) => {
                let collapsed = collapse_whitespace(text, false, false);
                Ok(LayoutBox::text(collapsed, parent_style.clone()))
            }

            NodeKind::Document => {
                let mut b = LayoutBox::new(LayoutBoxKind::Block, parent_style.clone());
                for &child_id in &doc.doc_children() {
                    if let Ok(child) = self.build_box(child_id, doc, parent_style, resolver, fs) {
                        b.children.push(child);
                    }
                }
                Ok(b)
            }

            NodeKind::Element(elem) => {
                let inline_css = elem.get_attr("style").unwrap_or("").to_string();
                let style = resolver.compute(node_id, doc, parent_style, &inline_css);

                if style.is_hidden() {
                    return Ok(LayoutBox::new(LayoutBoxKind::Block, style));
                }

                let tag = &elem.tag_name;

                // Special: <br>
                if tag == "br" {
                    let mut b = LayoutBox::text("\n".to_string(), style);
                    return Ok(b);
                }

                // Special: <hr>
                if tag == "hr" {
                    return Ok(LayoutBox::new(LayoutBoxKind::Block, style));
                }

                // Special: <img>
                if tag == "img" {
                    let src = elem.get_attr("src").unwrap_or("").to_string();
                    return Ok(LayoutBox::new(LayoutBoxKind::Image { src }, style));
                }

                let kind = display_to_kind(&style, tag);

                let mut b = LayoutBox::new(kind, style.clone());

                // Recursively build children
                let mut out_of_flow: Vec<LayoutBox> = Vec::new();
                let children: Vec<NodeId> = elem.children.clone();
                for child_id in children {
                    match self.build_box(child_id, doc, &style, resolver, fs) {
                        Ok(cb) => {
                            if matches!(cb.style.position, Position::Absolute | Position::Fixed) {
                                out_of_flow.push(cb);
                            } else if matches!(&cb.kind, LayoutBoxKind::Inline) && is_pure_inline(&cb)
                               && !matches!(b.kind, LayoutBoxKind::Flex | LayoutBoxKind::Grid
                                                   | LayoutBoxKind::Table | LayoutBoxKind::TableRow) {
                                // Transparently unwrap pure inline elements (<b>, <span>, <em>, <a>, …)
                                // Only in block/table-cell contexts to avoid breaking flex item layout.
                                // Their text children already inherit the inline element's style (bold, color, etc.)
                                flatten_inline(cb, &mut b.children);
                            } else {
                                b.children.push(cb);
                            }
                        }
                        Err(_) => {}
                    }
                }
                b.oof = out_of_flow;

                Ok(b)
            }

            NodeKind::Comment(_) => {
                // Skip comments
                Ok(LayoutBox::new(LayoutBoxKind::Block, parent_style.clone()))
            }
        }
    }

    // ─── Compute phase ────────────────────────────────────────────────────────

    fn compute_layout(
        &self,
        b:        &mut LayoutBox,
        avail_w:  f32,
        offset_y: f32,
        fs:       &mut FontSystem,
    ) {
        // Set up initial content rect
        b.content.x = self.config.margin.left;
        b.content.y = offset_y;

        match &b.style.display {
            Display::None => { b.content.width = 0.0; b.content.height = 0.0; return; }
            Display::Flex => {
                self.layout_flex(b, avail_w, offset_y, fs);
                return;
            }
            Display::Grid => {
                self.layout_grid(b, avail_w, offset_y, fs);
                return;
            }
            Display::Table => {
                table_layout::layout_table(b, avail_w, offset_y, fs, self);
                return;
            }
            _ => {}
        }

        self.layout_block(b, avail_w, offset_y, fs);
    }

    /// Dispatch to the correct layout function based on the box's display type.
    pub(crate) fn layout_node(&self, b: &mut LayoutBox, avail_w: f32, offset_y: f32, fs: &mut FontSystem) {
        match b.style.display {
            Display::None  => { b.content.width = 0.0; b.content.height = 0.0; }
            Display::Flex  => self.layout_flex(b, avail_w, offset_y, fs),
            Display::Grid  => self.layout_grid(b, avail_w, offset_y, fs),
            Display::Table => table_layout::layout_table(b, avail_w, offset_y, fs, self),
            _              => self.layout_block(b, avail_w, offset_y, fs),
        }
    }

    pub(crate) fn layout_block(
        &self,
        b:        &mut LayoutBox,
        avail_w:  f32,
        offset_y: f32,
        fs:       &mut FontSystem,
    ) {
        let padding = b.style.padding;
        let border  = ferropdf_core::Edge {
            top:    b.style.border_top.width,
            right:  b.style.border_right.width,
            bottom: b.style.border_bottom.width,
            left:   b.style.border_left.width,
        };

        // Resolve width.
        // avail_w = border-box space offered by parent.
        // style.width (if set) is a CSS content-box px value.
        // Auto-width: content_w = avail_w - padding - border (= content-box).
        // inner_w = children's available space = content_w.
        let content_w = if let Some(w) = b.style.width {
            w.max(b.style.min_width)
        } else {
            (avail_w - padding.horizontal() - border.horizontal()).max(0.0)
        };
        b.content.width = content_w;
        let inner_w = content_w;

        let mut cursor_y = b.content.y + padding.top + border.top;

        // Float tracking (left/right floats)
        let mut float_left_x  = b.content.x + padding.left + border.left;
        let mut float_right_x = float_left_x + inner_w;
        let mut float_clear_y = 0.0f32;

        // Collect inline children into runs, then lay them out as IFC
        let mut inline_run: Vec<usize> = Vec::new();
        let n = b.children.len();

        // We need to process children in order, then lay them out
        let mut child_positions: Vec<(f32, f32, f32, f32)> = vec![(0.0, 0.0, 0.0, 0.0); n]; // (x, y, w, h)

        for i in 0..n {
            let child = &b.children[i];
            match child.style.float {
                Float::Left => {
                    // Placeholder — we do float layout inline
                    inline_run.push(i);
                }
                Float::Right => {
                    inline_run.push(i);
                }
                Float::None => {
                    if child.style.is_inline() || child.is_text() {
                        inline_run.push(i);
                    } else {
                        // Flush any pending inline run
                        if !inline_run.is_empty() {
                            let run_h = self.layout_inline_run(
                                b, &inline_run, inner_w, cursor_y, fs, &mut child_positions,
                            );
                            cursor_y += run_h;
                            inline_run.clear();
                        }

                        // Layout block child
                        let child = &mut b.children[i];
                        child.content.x = float_left_x;
                        child.content.y = cursor_y;
                        self.layout_node(child, inner_w, cursor_y, fs);
                        // Flex, Grid and Table children report border-box height in content.height
                        // (padding + border already included). Block children report content-only height.
                        let child_h = match child.style.display {
                            Display::Flex | Display::Grid | Display::Table => {
                                child.content.height + child.style.margin.vertical()
                            }
                            _ => {
                                child.content.height + child.style.padding.vertical()
                                    + child.style.border_top.width + child.style.border_bottom.width
                                    + child.style.margin.vertical()
                            }
                        };
                        cursor_y += child_h;
                    }
                }
            }
        }

        // Flush remaining inline run
        if !inline_run.is_empty() {
            let run_h = self.layout_inline_run(
                b, &inline_run, inner_w, cursor_y, fs, &mut child_positions,
            );
            cursor_y += run_h;
        }

        // Resolve explicit height
        let content_h = cursor_y - b.content.y - padding.top - border.top;
        let content_h = content_h.max(b.style.min_height);
        b.content.height = b.style.height.unwrap_or(content_h);
    }

    /// Layout a group of inline children (text / inline boxes) into a line-box.
    fn layout_inline_run(
        &self,
        parent:    &mut LayoutBox,
        indices:   &[usize],
        avail_w:   f32,
        start_y:   f32,
        fs:        &mut FontSystem,
        positions: &mut Vec<(f32, f32, f32, f32)>,
    ) -> f32 {
        let origin_x = parent.content.x
            + parent.style.padding.left
            + parent.style.border_left.width;
        let text_align = parent.style.text_align;

        let mut x = 0.0f32;
        let mut y = start_y;
        let mut line_h = 0.0f32;
        let mut total_h = 0.0f32;

        // Collect per-child results to avoid borrow conflicts
        struct ChildResult {
            idx:    usize,
            cx:     f32,
            cy:     f32,
            cw:     f32,
            ch:     f32,
            shaped: Vec<ShapedLine>,
        }
        let mut results: Vec<ChildResult> = Vec::with_capacity(indices.len());

        for &i in indices {
            // ── Read child data via scoped immutable borrow ──────────────────
            let (is_text, text, font_size, line_height, font_family, font_weight, italic,
                 ibox_w, ibox_h, margin_h) = {
                let child = &parent.children[i];
                let is_text = matches!(&child.kind, LayoutBoxKind::Text { .. });
                let (text, ibox_w, ibox_h) = if let LayoutBoxKind::Text { raw_text, .. } = &child.kind {
                    (raw_text.clone(), 0.0f32, 0.0f32)
                } else {
                    let w = child.style.width.unwrap_or(80.0);
                    let h = child.style.height.unwrap_or(child.style.line_height);
                    (String::new(), w, h)
                };
                (
                    is_text, text,
                    child.style.font_size,
                    child.style.line_height,
                    child.style.font_family.clone(),
                    child.style.font_weight,
                    matches!(child.style.font_style,
                        ferropdf_parse::css::properties::FontStyle::Italic
                        | ferropdf_parse::css::properties::FontStyle::Oblique),
                    ibox_w, ibox_h,
                    child.style.margin.horizontal(),
                )
            }; // immutable borrow released here

            if is_text {
                let shaped = shape_text(
                    &text, &font_family, font_size, line_height,
                    font_weight, italic, (avail_w - x).max(1.0), fs,
                );
                let mut child_w = 0.0f32;
                let mut child_h = 0.0f32;
                for sl in &shaped { child_w = child_w.max(sl.width); child_h += sl.height; }

                if x + child_w > avail_w && x > 0.0 && !shaped.is_empty() {
                    y += line_h; total_h += line_h; x = 0.0; line_h = 0.0;
                }

                results.push(ChildResult { idx: i, cx: origin_x + x, cy: y,
                    cw: child_w, ch: child_h, shaped });
                x += child_w;
                line_h = line_h.max(child_h);
            } else {
                if x + ibox_w > avail_w && x > 0.0 {
                    y += line_h; total_h += line_h; x = 0.0; line_h = 0.0;
                }
                results.push(ChildResult { idx: i, cx: origin_x + x, cy: y,
                    cw: ibox_w, ch: ibox_h, shaped: Vec::new() });
                x += ibox_w + margin_h;
                line_h = line_h.max(ibox_h);
            }
        }

        // Apply text-align shift to the last (or only) line.
        // For single-line runs this is exact; for wrapped runs only the last line shifts.
        let shift = match text_align {
            TextAlign::Right  => (avail_w - x).max(0.0),
            TextAlign::Center => ((avail_w - x) / 2.0).max(0.0),
            _                 => 0.0,
        };

        // Write results back to parent.children (all borrows on parent are released)
        for r in results {
            parent.children[r.idx].content.x = r.cx + shift;
            parent.children[r.idx].content.y = r.cy;
            parent.children[r.idx].content.width  = r.cw;
            parent.children[r.idx].content.height = r.ch;
            if !r.shaped.is_empty() {
                if let LayoutBoxKind::Text { lines, .. } = &mut parent.children[r.idx].kind {
                    *lines = r.shaped;
                }
            }
        }

        total_h + line_h
    }

    // ─── Flex layout ──────────────────────────────────────────────────────────

    fn layout_flex(
        &self,
        b:        &mut LayoutBox,
        avail_w:  f32,
        offset_y: f32,
        fs:       &mut FontSystem,
    ) {
        let padding = b.style.padding;
        let border  = ferropdf_core::Edge {
            top:    b.style.border_top.width,
            right:  b.style.border_right.width,
            bottom: b.style.border_bottom.width,
            left:   b.style.border_left.width,
        };

        // NOTE: b.content.x / b.content.y are set by the CALLER before invoking layout_flex.
        // We must not reset them here (would break nested flex/grid children).
        let width = b.style.width.unwrap_or(avail_w);
        b.content.width = width;

        let inner_w = (width - padding.horizontal() - border.horizontal()).max(0.0);
        let inner_start_x = b.content.x + padding.left + border.left;
        let inner_start_y = b.content.y + padding.top  + border.top;

        let is_column = matches!(b.style.flex_direction, FlexDirection::Column | FlexDirection::ColumnReverse);
        let (row_gap, col_gap) = b.style.gap;
        let gap = if is_column { row_gap } else { col_gap };

        let n = b.children.len();
        let n_gaps = if n > 0 { (n - 1) as f32 * gap } else { 0.0 };

        // Compute the allocated main-axis size for each child
        let mut children_main_sizes: Vec<f32> = Vec::with_capacity(n);
        let mut total_grow = 0.0f32;
        let mut total_explicit_main = 0.0f32;
        let mut has_explicit: Vec<bool> = Vec::with_capacity(n);

        for child in b.children.iter() {
            let (main, explicit) = if is_column {
                let h = child.style.height;
                (h.unwrap_or(0.0), h.is_some())
            } else {
                let w = child.style.width;
                (w.unwrap_or(0.0), w.is_some())
            };
            children_main_sizes.push(main);
            has_explicit.push(explicit);
            if explicit { total_explicit_main += main; }
            total_grow += child.style.flex_grow;
        }

        // Free space after explicit-sized items and gaps
        let explicit_n = has_explicit.iter().filter(|&&x| x).count() as f32;
        let free_for_grow_or_auto = (inner_w - total_explicit_main - n_gaps).max(0.0);
        let auto_n = n as f32 - explicit_n;
        let auto_alloc = if auto_n > 0.0 && total_grow == 0.0 {
            // No flex-grow: distribute evenly among auto-sized items
            free_for_grow_or_auto / auto_n
        } else { 0.0 };

        // Fill in auto sizes and apply flex-grow
        for i in 0..n {
            if !has_explicit[i] {
                if total_grow > 0.0 {
                    children_main_sizes[i] = free_for_grow_or_auto * b.children[i].style.flex_grow / total_grow;
                } else {
                    children_main_sizes[i] = auto_alloc;
                }
            }
        }

        if is_column {
            // ── Column direction ──────────────────────────────────────────────
            let mut cursor = inner_start_y;
            let mut max_cross = 0.0f32;

            for i in 0..n {
                let alloc_h = children_main_sizes[i];
                b.children[i].content.x = inner_start_x;
                b.children[i].content.y = cursor;
                self.layout_node(&mut b.children[i], inner_w, cursor, fs);
                let child_h = b.children[i].content.height;
                max_cross = max_cross.max(b.children[i].content.width);
                cursor += child_h.max(alloc_h) + gap;
            }

            b.content.height = b.style.height.unwrap_or(
                (cursor - inner_start_y) + padding.vertical() + border.vertical()
            );
        } else {
            // ── Row direction: layout pass, then reposition with justify-content ──
            let mut initial_xs: Vec<f32> = Vec::with_capacity(n);
            let mut temp_cursor = inner_start_x;

            for i in 0..n {
                let alloc_w = children_main_sizes[i].max(1.0);
                b.children[i].content.x = temp_cursor;
                b.children[i].content.y = inner_start_y;
                initial_xs.push(temp_cursor);
                self.layout_node(&mut b.children[i], alloc_w, inner_start_y, fs);
                temp_cursor += alloc_w + gap;
            }

            // Actual widths after layout
            let actual_widths: Vec<f32> = b.children.iter().map(|c| c.content.width).collect();
            let actual_total: f32 = actual_widths.iter().sum::<f32>();
            let remaining = (inner_w - actual_total - n_gaps).max(0.0);

            // justify-content repositioning
            let (start_off, between) = justify_offsets(b.style.justify_content, n, remaining);
            let mut final_cursor = inner_start_x + start_off;
            for i in 0..n {
                let dx = final_cursor - initial_xs[i];
                if dx.abs() > 0.1 {
                    shift_subtree_x(&mut b.children[i], dx);
                }
                final_cursor += actual_widths[i] + gap + between;
            }

            let max_cross = b.children.iter().map(|c| c.content.height).fold(0.0f32, f32::max);
            b.content.height = b.style.height.unwrap_or(
                max_cross + padding.vertical() + border.vertical()
            );
        }

        b.content.height = b.content.height.max(b.style.min_height);
    }

    // ─── Grid layout ──────────────────────────────────────────────────────────

    fn layout_grid(
        &self,
        b:        &mut LayoutBox,
        avail_w:  f32,
        offset_y: f32,
        fs:       &mut FontSystem,
    ) {
        let padding = b.style.padding;
        let border  = ferropdf_core::Edge {
            top:    b.style.border_top.width,
            right:  b.style.border_right.width,
            bottom: b.style.border_bottom.width,
            left:   b.style.border_left.width,
        };

        // b.content.x / b.content.y set by caller — do not reset.
        let width = b.style.width.unwrap_or(avail_w);
        b.content.width = width;

        let inner_w = (width - padding.horizontal() - border.horizontal()).max(0.0);
        let inner_start_x = b.content.x + padding.left + border.left;
        let inner_start_y = b.content.y + padding.top  + border.top;

        // Resolve column tracks
        let tracks = &b.style.grid_template_columns;
        let col_widths = resolve_grid_tracks(tracks, inner_w);
        let n_cols = col_widths.len().max(1);

        let (row_gap, col_gap) = b.style.gap;

        let mut col = 0;
        let mut row_y = inner_start_y;
        let mut row_h = 0.0f32;
        let mut col_x = inner_start_x;
        let n = b.children.len();

        for i in 0..n {
            let cell_w = if col < col_widths.len() { col_widths[col] } else { inner_w / n_cols as f32 };

            b.children[i].content.x = col_x;
            b.children[i].content.y = row_y;
            self.layout_node(&mut b.children[i], cell_w, row_y, fs);
            let cell_h = b.children[i].content.height;
            row_h = row_h.max(cell_h);

            col_x += cell_w + col_gap;
            col += 1;
            if col >= n_cols {
                col = 0;
                row_y += row_h + row_gap;
                col_x = inner_start_x;
                row_h = 0.0;
            }
        }

        if row_h > 0.0 { row_y += row_h; }

        b.content.height = row_y - offset_y;
        b.content.height = b.content.height.max(b.style.min_height);
    }
}

// ─── Text shaping with cosmic-text ───────────────────────────────────────────

pub fn shape_text(
    text:       &str,
    font_family: &str,
    font_size:  f32,
    line_height: f32,
    weight:     u32,
    italic:     bool,
    avail_w:    f32,
    fs:         &mut FontSystem,
) -> Vec<ShapedLine> {
    if text.trim().is_empty() { return Vec::new(); }

    let metrics = Metrics::new(font_size, line_height);
    let mut buffer = Buffer::new(fs, metrics);

    buffer.set_size(fs, Some(avail_w.max(1.0)), None);

    let attrs = Attrs::new()
        .family(Family::Name(font_family))
        .weight(cosmic_text::Weight(weight as u16))
        .style(if italic { cosmic_text::Style::Italic } else { cosmic_text::Style::Normal });

    buffer.set_text(fs, text, attrs, Shaping::Advanced);
    buffer.shape_until_scroll(fs, false);

    let mut lines: Vec<ShapedLine> = Vec::new();

    for run in buffer.layout_runs() {
        let mut glyphs = Vec::new();
        let mut line_w = 0.0f32;

        for glyph in run.glyphs.iter() {
            glyphs.push(ShapedGlyph {
                glyph_id: glyph.glyph_id,
                x:        glyph.x,
                y:        glyph.y,
                advance:  glyph.w,
            });
            line_w = line_w.max(glyph.x + glyph.w);
        }

        let rtl = is_rtl_text(run.text);

        lines.push(ShapedLine {
            text:     run.text.to_string(),
            glyphs,
            height:   line_height,
            width:    line_w,
            baseline: font_size * 0.8,
            rtl,
        });
    }

    lines
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn collapse_whitespace(text: &str, has_leading: bool, has_trailing: bool) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        if text.chars().any(|c| c.is_whitespace()) {
            return " ".to_string();
        }
        return String::new();
    }
    let mut s = String::new();
    if text.starts_with(|c: char| c.is_whitespace()) { s.push(' '); }
    s.push_str(&words.join(" "));
    if text.ends_with(|c: char| c.is_whitespace()) { s.push(' '); }
    s
}

fn display_to_kind(style: &ComputedStyle, tag: &str) -> LayoutBoxKind {
    use ferropdf_parse::css::properties::Display::*;
    match style.display {
        Flex       => LayoutBoxKind::Flex,
        Grid       => LayoutBoxKind::Grid,
        Table      => LayoutBoxKind::Table,
        TableRow       => LayoutBoxKind::TableRow,
        TableCell
        | TableHeaderGroup
        | TableRowGroup
        | TableFooterGroup  => LayoutBoxKind::TableCell,
        ListItem       => LayoutBoxKind::ListItem {
            marker: list_marker(style),
        },
        Inline | InlineBlock => LayoutBoxKind::Inline,
        None       => LayoutBoxKind::Block,
        _          => LayoutBoxKind::Block,
    }
}

fn list_marker(style: &ComputedStyle) -> String {
    use ferropdf_parse::css::properties::ListStyleType::*;
    match style.list_style_type {
        Disc         => "•".to_string(),
        Circle       => "◦".to_string(),
        Square       => "▪".to_string(),
        Decimal      => "1.".to_string(),
        LowerAlpha   => "a.".to_string(),
        UpperAlpha   => "A.".to_string(),
        LowerRoman   => "i.".to_string(),
        UpperRoman   => "I.".to_string(),
        _            => String::new(),
    }
}

fn resolve_grid_tracks(tracks: &[GridTrack], inner_w: f32) -> Vec<f32> {
    if tracks.is_empty() { return vec![inner_w]; }
    let mut widths = Vec::with_capacity(tracks.len());
    let mut remaining = inner_w;
    let mut fr_total  = 0.0f32;

    // First pass: resolve non-fr tracks
    for track in tracks {
        match track {
            GridTrack::Px(v)      => { widths.push(*v); remaining -= v; }
            GridTrack::Percent(v) => { let w = v / 100.0 * inner_w; widths.push(w); remaining -= w; }
            GridTrack::Fr(f)      => { widths.push(0.0); fr_total += f; }
            GridTrack::Auto       => { widths.push(0.0); } // handled in 2nd pass
            _                     => { widths.push(0.0); }
        }
    }

    // Second pass: distribute remaining to fr tracks
    if fr_total > 0.0 {
        for (i, track) in tracks.iter().enumerate() {
            if let GridTrack::Fr(f) = track {
                widths[i] = (f / fr_total * remaining).max(0.0);
            }
        }
    }

    widths
}

/// Returns true if a box is a pure inline wrapper (no block children).
fn is_pure_inline(b: &LayoutBox) -> bool {
    b.children.iter().all(|c| {
        c.is_text()
            || (matches!(&c.kind, LayoutBoxKind::Inline) && is_pure_inline(c))
    })
}

/// Recursively flatten a pure-inline box into `out`, preserving each text node's
/// already-computed style (which inherited the inline element's font/color).
fn flatten_inline(b: LayoutBox, out: &mut Vec<LayoutBox>) {
    for child in b.children {
        if child.is_text() {
            out.push(child);
        } else if matches!(&child.kind, LayoutBoxKind::Inline) && is_pure_inline(&child) {
            flatten_inline(child, out);
        } else {
            out.push(child);
        }
    }
}

fn is_rtl_text(text: &str) -> bool {
    let bidi = BidiInfo::new(text, None);
    bidi.paragraphs.first().map(|p| p.level.is_rtl()).unwrap_or(false)
}

/// Compute (start_offset, between_gap) for justify-content along the main axis.
fn justify_offsets(jc: JustifyContent, n: usize, remaining: f32) -> (f32, f32) {
    if n == 0 { return (0.0, 0.0); }
    match jc {
        JustifyContent::FlexStart  => (0.0, 0.0),
        JustifyContent::FlexEnd    => (remaining, 0.0),
        JustifyContent::Center     => (remaining / 2.0, 0.0),
        JustifyContent::SpaceBetween => (0.0, if n > 1 { remaining / (n - 1) as f32 } else { 0.0 }),
        JustifyContent::SpaceAround  => {
            let s = remaining / n as f32;
            (s / 2.0, s)
        }
        JustifyContent::SpaceEvenly => {
            let s = remaining / (n + 1) as f32;
            (s, s)
        }
    }
}

/// Recursively shift all x-coordinates in a subtree by `dx`.
fn shift_subtree_x(b: &mut LayoutBox, dx: f32) {
    b.content.x += dx;
    for child in &mut b.children { shift_subtree_x(child, dx); }
    for child in &mut b.oof     { shift_subtree_x(child, dx); }
}
