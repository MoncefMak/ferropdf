//! Painter — converts a paginated LayoutBox tree into a display list.

use ferropdf_core::EngineConfig;
use ferropdf_layout::box_model::{LayoutBox, LayoutBoxKind};
use ferropdf_page::fragment::Page;
use ferropdf_parse::css::properties::{BorderStyleKind, ComputedStyle, TextDecoration};

use crate::display_list::{Color, DrawOp};

const PX_TO_PT: f32 = 0.75;

pub struct Painter {
    config: EngineConfig,
}

impl Painter {
    pub fn new(config: EngineConfig) -> Self { Self { config } }

    /// Convert a list of pages into per-page display lists.
    pub fn paint_pages(&self, pages: &[Page]) -> Vec<Vec<DrawOp>> {
        pages.iter().map(|p| self.paint_page(p)).collect()
    }

    fn paint_page(&self, page: &Page) -> Vec<DrawOp> {
        let w_pt = page.width  * PX_TO_PT;
        let h_pt = page.height * PX_TO_PT;

        let mut ops: Vec<DrawOp> = Vec::new();
        ops.push(DrawOp::BeginPage { width_pt: w_pt, height_pt: h_pt });
        ops.push(DrawOp::FillRect {
            x: 0.0, y: 0.0, w: w_pt, h: h_pt,
            color: Color::WHITE,
        });

        for b in &page.boxes {
            self.paint_box(b, h_pt, &mut ops);
        }

        ops.push(DrawOp::EndPage);
        ops
    }

    fn paint_box(&self, b: &LayoutBox, page_h_pt: f32, ops: &mut Vec<DrawOp>) {
        if b.style.opacity < 0.001 { return; }
        if matches!(b.style.display, ferropdf_parse::css::properties::Display::None) { return; }

        // Convert CSS px coordinates to PDF pt (origin bottom-left in PDF)
        let x  = b.content.x  * PX_TO_PT;
        let y  = b.content.y  * PX_TO_PT;
        let w  = b.content.width  * PX_TO_PT;
        let h  = b.content.height * PX_TO_PT;

        // In PDF: y=0 is at the bottom; flip
        let pdf_y = page_h_pt - y - h;

        // Background
        let bg = color_from_core(b.style.background_color);
        if bg.a > 0.001 {
            ops.push(DrawOp::FillRect { x, y: pdf_y, w, h, color: bg });
        }

        // Borders
        self.paint_borders(b, x, pdf_y, w, h, ops);

        // Content
        match &b.kind {
            LayoutBoxKind::Text { lines, raw_text: _ } => {
                for line in lines {
                    let lx = x;
                    let ly = page_h_pt - (b.content.y + line.baseline) * PX_TO_PT;
                    ops.push(DrawOp::GlyphRun {
                        x: lx, y: ly,
                        size:   b.style.font_size * PX_TO_PT,
                        color:  color_from_core(b.style.color),
                        font:   b.style.font_family.clone(),
                        weight: b.style.font_weight as u16,
                        italic: matches!(b.style.font_style,
                            ferropdf_parse::css::properties::FontStyle::Italic
                          | ferropdf_parse::css::properties::FontStyle::Oblique),
                        text:      line.text.clone(),
                        glyph_ids: line.glyphs.iter().map(|g| g.glyph_id).collect(),
                        advances:  line.glyphs.iter().map(|g| g.advance * PX_TO_PT).collect(),
                    });

                    // Underline
                    if matches!(b.style.text_decoration, TextDecoration::Underline) {
                        let ul_y = ly - b.style.font_size * 0.1 * PX_TO_PT;
                        ops.push(DrawOp::BorderLine {
                            x1: lx, y1: ul_y, x2: lx + line.width * PX_TO_PT, y2: ul_y,
                            color: color_from_core(b.style.color),
                            width: 0.5,
                        });
                    }
                }
            }

            LayoutBoxKind::Image { src } => {
                // Images loaded elsewhere; emit placeholder
                ops.push(DrawOp::Image {
                    x, y: pdf_y, w, h,
                    data: Vec::new(),
                    src: src.clone(),
                });
            }

            LayoutBoxKind::ListItem { marker } => {
                if !marker.is_empty() {
                    let mx = x - b.style.font_size * PX_TO_PT * 1.2;
                    let my = page_h_pt - (b.content.y + b.style.font_size * 0.8) * PX_TO_PT;
                    ops.push(DrawOp::GlyphRun {
                        x: mx, y: my,
                        size:      b.style.font_size * PX_TO_PT,
                        color:     color_from_core(b.style.color),
                        font:      b.style.font_family.clone(),
                        weight:    b.style.font_weight as u16,
                        italic:    false,
                        text:      marker.clone(),
                        glyph_ids: Vec::new(),
                        advances:  Vec::new(),
                    });
                }
            }

            _ => {}
        }

        // Paint href link overlay
        if let Some(href) = b.style.href.as_deref() {
            if !href.is_empty() {
                ops.push(DrawOp::Link { x, y: pdf_y, w, h, uri: href.to_string() });
            }
        }
    }

    fn paint_borders(
        &self,
        b:      &LayoutBox,
        x:      f32,
        pdf_y:  f32,
        w:      f32,
        h:      f32,
        ops:    &mut Vec<DrawOp>,
    ) {
        let sides = [
            (&b.style.border_top,    x,       pdf_y + h,  x + w, pdf_y + h),
            (&b.style.border_bottom, x,       pdf_y,      x + w, pdf_y),
            (&b.style.border_left,   x,       pdf_y,      x,     pdf_y + h),
            (&b.style.border_right,  x + w,   pdf_y,      x + w, pdf_y + h),
        ];
        for (side, x1, y1, x2, y2) in sides {
            if side.width > 0.0 && !matches!(side.style, BorderStyleKind::None) {
                ops.push(DrawOp::BorderLine {
                    x1, y1, x2, y2,
                    color: color_from_core(side.color),
                    width: side.width * PX_TO_PT,
                });
            }
        }
    }
}

fn color_from_core(c: ferropdf_core::Color) -> Color {
    Color { r: c.r, g: c.g, b: c.b, a: c.a }
}
