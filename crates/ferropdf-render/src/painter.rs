use crate::display_list::{DrawOp, PageDisplayList};
use ferropdf_core::layout::Page;
use ferropdf_core::{
    BorderCollapse, BorderStyle, Color, LayoutBox, ListStyleType, PageConfig, Rect, TextDecoration,
};

/// Paint a page into a display list.
/// All coordinates are in points typographiques (pt).
pub fn paint_page(page: &Page, config: &PageConfig) -> PageDisplayList {
    let mut ops = Vec::new();

    // Offsets and container width in points (pt)
    let offset_x = config.margins.left;
    let offset_y = config.margins.top;
    let container_width = config.content_width_pt();

    for layout_box in &page.content {
        paint_box(layout_box, &mut ops, offset_x, offset_y, container_width, 0);
    }

    PageDisplayList {
        ops,
        page_number: page.page_number,
        total_pages: page.total_pages,
    }
}

fn paint_box(
    layout_box: &LayoutBox,
    ops: &mut Vec<DrawOp>,
    offset_x: f32,
    offset_y: f32,
    _parent_content_width: f32,
    depth: usize,
) {
    if depth > ferropdf_core::MAX_DOM_DEPTH {
        return;
    }

    let style = &layout_box.style;

    if !style.visibility {
        return;
    }

    // Opacity: wrap entire box output in Save/SetOpacity/Restore
    let has_opacity = style.opacity < 1.0 - f32::EPSILON;
    if has_opacity {
        ops.push(DrawOp::Save);
        ops.push(DrawOp::SetOpacity(style.opacity));
    }

    // Accumulate position:relative visual offset into effective offsets.
    // This shifts this box, its text, and all descendants.
    let eff_x = offset_x + layout_box.visual_offset_x;
    let eff_y = offset_y + layout_box.visual_offset_y;

    let border_box = layout_box.border_box();
    let x = border_box.x + eff_x;
    let y = border_box.y + eff_y;
    let rect = Rect::new(x, y, border_box.width, border_box.height);

    // Box shadows (rendered before background, behind the element)
    for shadow in &style.box_shadow {
        if !shadow.inset {
            ops.push(DrawOp::DrawBoxShadow {
                rect,
                shadow: shadow.clone(),
                border_radius: style.border_radius.to_array(),
            });
        }
    }

    // Background
    if !style.background_color.is_transparent() {
        ops.push(DrawOp::FillRect {
            rect,
            color: style.background_color,
            border_radius: style.border_radius.to_array(),
        });
    }

    // Borders
    paint_borders(layout_box, ops, rect);

    // Merged inline text — render with per-segment styling
    if !layout_box.inline_spans.is_empty() && !layout_box.shaped_lines.is_empty() {
        let text_x = layout_box.content.x + eff_x;
        // Use this box's own content width for alignment (not parent's),
        // because merge_inline_children shaped text at this box's width.
        let align_container = layout_box.content.width;
        for line in &layout_box.shaped_lines {
            if line.segments.is_empty() {
                continue;
            }
            let line_y = layout_box.content.y + eff_y + line.y;

            // Compute text-align offset for the whole line
            let align_offset = match style.text_align {
                ferropdf_core::TextAlign::Right => align_container - line.width,
                ferropdf_core::TextAlign::Center => (align_container - line.width) / 2.0,
                _ => 0.0,
            };

            for seg in &line.segments {
                if seg.text.trim().is_empty() {
                    continue;
                }
                let span = &layout_box.inline_spans[seg.metadata];
                // Collect shaped glyphs for this segment (with font_id for correct PDF embedding)
                let seg_glyphs: Vec<ferropdf_core::ShapedGlyph> = line
                    .glyphs
                    .iter()
                    .filter(|g| g.metadata == seg.metadata)
                    .cloned()
                    .collect();
                // For shaped glyphs, use text_x only — glyph.x already contains
                // the absolute position within the buffer (no need to add seg.x_offset).
                // For unshaped fallback, use seg_x with offset.
                let draw_x = if !seg_glyphs.is_empty() {
                    text_x
                } else {
                    text_x + seg.x_offset + align_offset
                };
                ops.push(DrawOp::DrawText {
                    text: seg.text.clone(),
                    x: draw_x,
                    y: line_y,
                    font_size: span.font_size,
                    color: span.color,
                    font_family: vec![span.font_family.clone()],
                    bold: span.bold,
                    italic: span.italic,
                    text_align: ferropdf_core::TextAlign::Left,
                    container_width: 0.0,
                    shaped_glyphs: seg_glyphs,
                });
                emit_text_decoration(
                    ops,
                    &span.text_decoration,
                    text_x + seg.x_offset,
                    line_y,
                    seg.width,
                    span.font_size,
                    span.color,
                );
            }
        }
    }
    // Text content — use shaped_lines from cosmic-text when available,
    // otherwise fall back to full text (will be re-wrapped in pdf.rs).
    else if let Some(ref text) = layout_box.text_content {
        let text = text.trim();
        if !text.is_empty() {
            let text_x = layout_box.content.x + eff_x;

            if !layout_box.shaped_lines.is_empty() {
                // Emit one DrawText per shaped line — no re-wrap needed in pdf.rs.
                for line in &layout_box.shaped_lines {
                    let line_text = line.text.trim();
                    if line_text.is_empty() {
                        continue;
                    }
                    // y = content origin + line's baseline Y (from cosmic-text)
                    let line_y = layout_box.content.y + eff_y + line.y;
                    // For shaped glyphs, pass text_x only — cosmic-text already
                    // positions glyphs within the buffer (glyph.x accounts for
                    // RTL layout within the content width).
                    ops.push(DrawOp::DrawText {
                        text: line_text.to_string(),
                        x: text_x,
                        y: line_y,
                        font_size: style.font_size,
                        color: style.color,
                        font_family: style.font_family.clone(),
                        bold: style.font_weight.is_bold(),
                        italic: style.font_style == ferropdf_core::FontStyle::Italic,
                        text_align: ferropdf_core::TextAlign::Left,
                        container_width: 0.0,
                        shaped_glyphs: line.glyphs.clone(),
                    });
                    emit_text_decoration(
                        ops,
                        &style.text_decoration,
                        text_x,
                        line_y,
                        line.width,
                        style.font_size,
                        style.color,
                    );
                }
            } else {
                // Fallback: emit the full text (pdf.rs will word-wrap it)
                let text_y = layout_box.content.y + eff_y + style.font_size * 0.8;
                ops.push(DrawOp::DrawText {
                    text: text.to_string(),
                    x: text_x,
                    y: text_y,
                    font_size: style.font_size,
                    color: style.color,
                    font_family: style.font_family.clone(),
                    bold: style.font_weight.is_bold(),
                    italic: style.font_style == ferropdf_core::FontStyle::Italic,
                    text_align: style.text_align,
                    container_width: layout_box.content.width,
                    shaped_glyphs: Vec::new(),
                });
                emit_text_decoration(
                    ops,
                    &style.text_decoration,
                    text_x,
                    text_y,
                    layout_box.content.width,
                    style.font_size,
                    style.color,
                );
            }
        }
    }

    // Image
    if let Some(ref src) = layout_box.image_src {
        ops.push(DrawOp::DrawImage {
            src: src.clone(),
            rect: Rect::new(
                layout_box.content.x + eff_x,
                layout_box.content.y + eff_y,
                layout_box.content.width,
                layout_box.content.height,
            ),
        });
    }

    // List item marker (bullet or number)
    if let Some(idx) = layout_box.list_item_index {
        let marker_text = format_list_marker(&style.list_style_type, idx);
        if !marker_text.is_empty() {
            let marker_x = layout_box.content.x + eff_x - style.font_size * 1.2;
            let marker_y = layout_box.content.y + eff_y + style.font_size * 0.8;
            ops.push(DrawOp::DrawText {
                text: marker_text,
                x: marker_x,
                y: marker_y,
                font_size: style.font_size,
                color: style.color,
                font_family: style.font_family.clone(),
                bold: false,
                italic: false,
                text_align: ferropdf_core::TextAlign::Left,
                container_width: 0.0,
                shaped_glyphs: Vec::new(),
            });
        }
    }

    // Children — propagate effective offset (includes ancestor relative shifts)
    let my_content_width = layout_box.content.width;
    for child in &layout_box.children {
        paint_box(child, ops, eff_x, eff_y, my_content_width, depth + 1);
    }

    if has_opacity {
        ops.push(DrawOp::Restore);
    }
}

/// Emit a thin FillRect for text-decoration (underline, line-through, overline).
fn emit_text_decoration(
    ops: &mut Vec<DrawOp>,
    decoration: &TextDecoration,
    x: f32,
    y: f32,
    width: f32,
    font_size: f32,
    color: Color,
) {
    let thickness = (font_size * 0.07).max(0.5);
    let line_y = match decoration {
        TextDecoration::Underline => y + font_size * 0.15,
        TextDecoration::LineThrough => y - font_size * 0.3,
        TextDecoration::Overline => y - font_size * 0.8,
        TextDecoration::None => return,
    };
    ops.push(DrawOp::FillRect {
        rect: Rect::new(x, line_y, width, thickness),
        color,
        border_radius: [0.0; 4],
    });
}

fn paint_borders(layout_box: &LayoutBox, ops: &mut Vec<DrawOp>, rect: Rect) {
    let style = &layout_box.style;

    // border-collapse: suppress duplicate inner borders for table cells.
    // For collapsed cells, only draw top border if on first row,
    // and only draw left border if in first column.
    let collapse = style.border_collapse == BorderCollapse::Collapse;
    let (skip_top, skip_left) = if collapse {
        if let Some((row, col, _total_rows, _total_cols)) = layout_box.table_cell_pos {
            (row > 0, col > 0)
        } else {
            (false, false)
        }
    } else {
        (false, false)
    };

    // Top border
    if !skip_top && style.border_top.width > 0.0 && style.border_top.style != BorderStyle::None {
        ops.push(DrawOp::StrokeRect {
            rect: Rect::new(rect.x, rect.y, rect.width, 0.0),
            color: style.border_top.color,
            width: style.border_top.width,
            style: style.border_top.style,
        });
    }

    // Right border
    if style.border_right.width > 0.0 && style.border_right.style != BorderStyle::None {
        ops.push(DrawOp::StrokeRect {
            rect: Rect::new(rect.right(), rect.y, 0.0, rect.height),
            color: style.border_right.color,
            width: style.border_right.width,
            style: style.border_right.style,
        });
    }

    // Bottom border
    if style.border_bottom.width > 0.0 && style.border_bottom.style != BorderStyle::None {
        ops.push(DrawOp::StrokeRect {
            rect: Rect::new(rect.x, rect.bottom(), rect.width, 0.0),
            color: style.border_bottom.color,
            width: style.border_bottom.width,
            style: style.border_bottom.style,
        });
    }

    // Left border
    if !skip_left && style.border_left.width > 0.0 && style.border_left.style != BorderStyle::None {
        ops.push(DrawOp::StrokeRect {
            rect: Rect::new(rect.x, rect.y, 0.0, rect.height),
            color: style.border_left.color,
            width: style.border_left.width,
            style: style.border_left.style,
        });
    }
}

/// Format a list item marker string based on list-style-type.
fn format_list_marker(list_style: &ListStyleType, index: usize) -> String {
    match list_style {
        ListStyleType::Disc => "\u{2022}".to_string(),   // •
        ListStyleType::Circle => "\u{25E6}".to_string(), // ◦
        ListStyleType::Square => "\u{25AA}".to_string(), // ▪
        ListStyleType::Decimal => format!("{}.", index),
        ListStyleType::DecimalLeadingZero => format!("{:02}.", index),
        ListStyleType::LowerAlpha => {
            let c = (b'a' + ((index - 1) % 26) as u8) as char;
            format!("{}.", c)
        }
        ListStyleType::UpperAlpha => {
            let c = (b'A' + ((index - 1) % 26) as u8) as char;
            format!("{}.", c)
        }
        ListStyleType::LowerRoman => format!("{}.", to_roman(index).to_lowercase()),
        ListStyleType::UpperRoman => format!("{}.", to_roman(index)),
        ListStyleType::None => String::new(),
    }
}

fn to_roman(mut n: usize) -> String {
    const VALS: &[(usize, &str)] = &[
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut result = String::new();
    for &(val, sym) in VALS {
        while n >= val {
            result.push_str(sym);
            n -= val;
        }
    }
    result
}
