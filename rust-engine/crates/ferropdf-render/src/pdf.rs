//! PDF generation using `pdf-writer` 0.14.

use std::collections::HashMap;

use pdf_writer::{
    types::{ActionType, AnnotationType, ColorSpaceOperand, LineCapStyle, LineJoinStyle},
    Content, Finish, Name, Pdf, Rect as PdfRect, Ref, Str, TextStr,
};

use crate::display_list::{Color, DrawOp};
use ferropdf_core::{EngineConfig, FerroError};

// ─── Well-known built-in Type1 font names ──────────────────────────────────

const BUILTIN_FONTS: &[(&str, &str)] = &[
    ("Helvetica",             "Helvetica"),
    ("Helvetica-Bold",        "Helvetica-Bold"),
    ("Helvetica-Oblique",     "Helvetica-Oblique"),
    ("Helvetica-BoldOblique", "Helvetica-BoldOblique"),
    ("Times-Roman",           "Times-Roman"),
    ("Times-Bold",            "Times-Bold"),
    ("Times-Italic",          "Times-Italic"),
    ("Times-BoldItalic",      "Times-BoldItalic"),
    ("Courier",               "Courier"),
    ("Courier-Bold",          "Courier-Bold"),
    ("Courier-Oblique",       "Courier-Oblique"),
    ("Courier-BoldOblique",   "Courier-BoldOblique"),
    ("Symbol",                "Symbol"),
    ("ZapfDingbats",          "ZapfDingbats"),
];

fn choose_builtin(family: &str, weight: u16, italic: bool) -> &'static str {
    let f = family.to_lowercase();
    let is_serif    = f.contains("times") || f.contains("serif");
    let is_mono     = f.contains("courier") || f.contains("mono") || f.contains("code");
    let is_bold     = weight >= 700;

    if is_mono {
        match (is_bold, italic) {
            (true,  true)  => "Courier-BoldOblique",
            (true,  false) => "Courier-Bold",
            (false, true)  => "Courier-Oblique",
            (false, false) => "Courier",
        }
    } else if is_serif {
        match (is_bold, italic) {
            (true,  true)  => "Times-BoldItalic",
            (true,  false) => "Times-Bold",
            (false, true)  => "Times-Italic",
            (false, false) => "Times-Roman",
        }
    } else {
        // Sans-serif (default: Helvetica)
        match (is_bold, italic) {
            (true,  true)  => "Helvetica-BoldOblique",
            (true,  false) => "Helvetica-Bold",
            (false, true)  => "Helvetica-Oblique",
            (false, false) => "Helvetica",
        }
    }
}

// ─── PdfRenderer ────────────────────────────────────────────────────────────

pub struct PdfRenderer {
    pub config: EngineConfig,
}

impl PdfRenderer {
    pub fn new(config: EngineConfig) -> Self { Self { config } }

    /// Convert a per-page display list into serialised PDF bytes.
    pub fn render(&self, pages: &[Vec<DrawOp>]) -> Result<Vec<u8>, FerroError> {
        if pages.is_empty() {
            return Err(FerroError::Layout("no pages to render".into()));
        }

        let mut pdf  = Pdf::new();
        let mut alloc = Allocator::new();

        let catalog_id = alloc.next();
        let pages_id   = alloc.next();

        // Collect font aliases needed across all pages
        let mut needed_fonts: HashMap<String, Ref> = HashMap::new();
        for page_ops in pages {
            for op in page_ops {
                if let DrawOp::GlyphRun { font, weight, italic, .. } = op {
                    let base = choose_builtin(font, *weight, *italic);
                    needed_fonts.entry(base.to_string()).or_insert_with(|| alloc.next());
                }
            }
        }

        // Write font objects
        for (base_name, &font_ref) in &needed_fonts {
            let mut f = pdf.type1_font(font_ref);
            f.base_font(Name(base_name.as_bytes()));
        }

        // Build font-alias map: "F0", "F1", … keyed by base-name
        let font_alias: HashMap<String, String> = needed_fonts.keys()
            .enumerate()
            .map(|(i, name)| (name.clone(), format!("F{i}")))
            .collect();
        let font_ref_map: HashMap<String, Ref> = needed_fonts.iter()
            .map(|(n, &r)| (n.clone(), r))
            .collect();

        // Write each page
        let mut page_ids: Vec<Ref> = Vec::with_capacity(pages.len());

        for page_ops in pages {
            // Determine page size from the first BeginPage
            let (w_pt, h_pt) = page_ops.iter().find_map(|op| {
                if let DrawOp::BeginPage { width_pt, height_pt } = op {
                    Some((*width_pt, *height_pt))
                } else { None }
            }).unwrap_or((595.276, 841.890)); // A4

            let page_id    = alloc.next();
            let content_id = alloc.next();
            page_ids.push(page_id);

            // Build content stream
            let mut content = Content::new();

            for op in page_ops {
                match op {
                    DrawOp::BeginPage { .. } | DrawOp::EndPage => {}

                    DrawOp::SaveState    => { content.save_state(); }
                    DrawOp::RestoreState => { content.restore_state(); }
                    DrawOp::SetOpacity(a) => {
                        // Opacity via graphics state — simplified: set fill/stroke alpha
                        // pdf-writer doesn't expose ExtGState alpha directly in Content,
                        // so we skip for now (opacity handled by colour alpha blending).
                        let _ = a;
                    }
                    DrawOp::ClipRect { x, y, w, h } => {
                        content.save_state();
                        content
                            .move_to(*x, *y)
                            .line_to(x + w, *y)
                            .line_to(x + w, y + h)
                            .line_to(*x, y + h)
                            .close_path()
                            .clip_nonzero()
                            .end_path();
                    }

                    DrawOp::FillRect { x, y, w, h, color } => {
                        content.save_state();
                        set_fill_color(&mut content, color);
                        content
                            .rect(*x, *y, *w, *h)
                            .fill_nonzero();
                        content.restore_state();
                    }

                    DrawOp::StrokeRect { x, y, w, h, color, width } => {
                        content.save_state();
                        set_stroke_color(&mut content, color);
                        content.set_line_width(*width);
                        content
                            .rect(*x, *y, *w, *h)
                            .stroke();
                        content.restore_state();
                    }

                    DrawOp::BorderLine { x1, y1, x2, y2, color, width } => {
                        content.save_state();
                        set_stroke_color(&mut content, color);
                        content.set_line_width(*width);
                        content
                            .move_to(*x1, *y1)
                            .line_to(*x2, *y2)
                            .stroke();
                        content.restore_state();
                    }

                    DrawOp::GlyphRun { x, y, size, color, font, weight, italic, text, glyph_ids, advances } => {
                        if text.is_empty() { continue; }
                        let base  = choose_builtin(font, *weight, *italic);
                        let alias = &font_alias[base];

                        content.save_state();
                        set_fill_color(&mut content, color);
                        content.begin_text();
                        content.set_font(Name(alias.as_bytes()), *size);
                        // Absolute text position: Tm matrix [1 0 0 1 x y]
                        content.set_text_matrix([1.0, 0.0, 0.0, 1.0, *x, *y]);
                        // Show the whole run as a single string for correct built-in font spacing
                        let encoded = encode_latin1(text);
                        content.show(Str(&encoded));
                        content.end_text();
                        content.restore_state();
                    }

                    DrawOp::Image { x, y, w, h, data, src } => {
                        if !data.is_empty() {
                            // Image writing requires an XObject — skip for now
                            // and render a placeholder grey rect
                            content.save_state();
                            set_fill_color(&mut content, &Color::from_rgba(0.85, 0.85, 0.85, 1.0));
                            content.rect(*x, *y, *w, *h).fill_nonzero();
                            content.restore_state();
                        }
                    }

                    DrawOp::Link { x, y, w, h, uri } => {
                        // Links are written as page annotations — collected separately
                    }
                }
            }

            let content_bytes = content.finish();
            pdf.stream(content_id, &content_bytes);

            // Collect link annotations for this page
            let annot_ids: Vec<Ref> = page_ops.iter().filter_map(|op| {
                if let DrawOp::Link { .. } = op { Some(alloc.next()) } else { None }
            }).collect();

            // Write page dict
            {
                let mut page_dict = pdf.page(page_id);
                page_dict.parent(pages_id);
                page_dict.media_box(PdfRect::new(0.0, 0.0, w_pt, h_pt));

                let mut res = page_dict.resources();
                let mut font_dict = res.fonts();
                for (base_name, alias) in &font_alias {
                    font_dict.pair(Name(alias.as_bytes()), font_ref_map[base_name]);
                }
                font_dict.finish();
                res.finish();
                page_dict.contents(content_id);

                if !annot_ids.is_empty() {
                    page_dict.annotations(annot_ids.iter().copied());
                }
            }

            // Write link annotation objects
            let mut link_ops = page_ops.iter().filter_map(|op| {
                if let DrawOp::Link { x, y, w, h, uri } = op { Some((x, y, w, h, uri)) }
                else { None }
            });
            for (aid, (x, y, w, h, uri)) in annot_ids.iter().zip(&mut link_ops) {
                let mut annot = pdf.annotation(*aid);
                annot.subtype(AnnotationType::Link);
                annot.rect(PdfRect::new(*x, *y, x + w, y + h));
                let mut action = annot.action();
                action.action_type(ActionType::Uri);
                action.uri(Str(uri.as_bytes()));
            }
        }

        // Write pages dict
        {
            let mut pages_dict = pdf.pages(pages_id);
            pages_dict.kids(page_ids.iter().copied());
            pages_dict.count(page_ids.len() as i32);
        }

        // Write catalog
        {
            let mut cat = pdf.catalog(catalog_id);
            cat.pages(pages_id);
            if let Some(title) = &self.config.title {
                // Info dict — simplified
            }
        }

        Ok(pdf.finish())
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

struct Allocator(i32);
impl Allocator {
    fn new() -> Self { Self(1) }
    fn next(&mut self) -> Ref { let r = Ref::new(self.0); self.0 += 1; r }
}

fn set_fill_color(content: &mut Content, c: &Color) {
    if c.r == c.g && c.g == c.b {
        content.set_fill_gray(c.r);
    } else {
        content.set_fill_color_space(ColorSpaceOperand::DeviceRgb);
        content.set_fill_color([c.r, c.g, c.b]);
    }
}

fn set_stroke_color(content: &mut Content, c: &Color) {
    if c.r == c.g && c.g == c.b {
        content.set_stroke_gray(c.r);
    } else {
        content.set_stroke_color_space(ColorSpaceOperand::DeviceRgb);
        content.set_stroke_color([c.r, c.g, c.b]);
    }
}

/// Encode a UTF-8 string into Latin-1 / WinAnsi bytes for built-in Type1 fonts.
/// Characters outside Latin-1 range are replaced with '?'.
fn encode_latin1(s: &str) -> Vec<u8> {
    s.chars().map(|c| {
        let cp = c as u32;
        if cp <= 0xFF { cp as u8 } else { b'?' }
    }).collect()
}
