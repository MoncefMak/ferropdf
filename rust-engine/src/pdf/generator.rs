//! PDF generator — takes paint commands and produces a PDF document.

use std::collections::HashMap;
use std::io::BufWriter;
use std::sync::Arc;

use printpdf::path::{PaintMode, WindingOrder};
use printpdf::*;

use crate::css::values::Color;
use crate::error::{FastPdfError, Result};
use crate::fonts::metrics;
use crate::fonts::shaping;
use crate::fonts::FontCache;
use crate::layout::pagination::{Page, PageLayout};
use crate::renderer::paint::{PaintCommand, TextAlign};

use super::writer;

/// Key for caching font references within a PDF document.
#[derive(Hash, Eq, PartialEq, Clone, Debug)]
struct FontKey {
    family: String,
    weight: u32,
    italic: bool,
    is_external: bool,
}

/// Configuration for PDF generation.
#[derive(Debug, Clone)]
pub struct PdfConfig {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Vec<String>,
    pub creator: String,
    pub compress: bool,
}

impl Default for PdfConfig {
    fn default() -> Self {
        Self {
            title: None,
            author: None,
            subject: None,
            keywords: Vec::new(),
            creator: "FastPDF Engine".to_string(),
            compress: true,
        }
    }
}

/// PDF generator that converts rendered pages to a PDF file.
pub struct PdfGenerator {
    config: PdfConfig,
    font_cache: Option<Arc<FontCache>>,
}

impl PdfGenerator {
    pub fn new(config: PdfConfig) -> Self {
        Self {
            config,
            font_cache: None,
        }
    }

    pub fn with_font_cache(mut self, cache: Arc<FontCache>) -> Self {
        self.font_cache = Some(cache);
        self
    }

    /// Generate a PDF from rendered pages and return as bytes.
    pub fn generate(&self, pages: &[Page], page_commands: &[Vec<PaintCommand>]) -> Result<Vec<u8>> {
        if pages.is_empty() {
            return Err(FastPdfError::PdfGeneration(
                "No pages to render".to_string(),
            ));
        }

        let first_page = &pages[0];
        let (page_w, page_h) = first_page.layout.size.to_mm();

        let (doc, page1_idx, layer1_idx) = PdfDocument::new(
            self.config.title.as_deref().unwrap_or("Document"),
            Mm(page_w as f32),
            Mm(page_h as f32),
            "Layer 1",
        );

        // Pre-embed all fonts used across all commands
        let mut font_refs: HashMap<FontKey, IndirectFontRef> = HashMap::new();
        self.pre_embed_fonts(&doc, page_commands, &mut font_refs)?;

        // Render first page
        if let Some(commands) = page_commands.first() {
            let layer = doc.get_page(page1_idx).get_layer(layer1_idx);
            self.render_commands_to_layer(&layer, commands, &first_page.layout, &doc, &font_refs)?;
        }

        // Render remaining pages
        for (i, (page, commands)) in pages.iter().zip(page_commands.iter()).enumerate().skip(1) {
            let (page_w, page_h) = page.layout.size.to_mm();
            let (page_idx, layer_idx) = doc.add_page(
                Mm(page_w as f32),
                Mm(page_h as f32),
                format!("Layer {}", i + 1),
            );
            let layer = doc.get_page(page_idx).get_layer(layer_idx);
            self.render_commands_to_layer(&layer, commands, &page.layout, &doc, &font_refs)?;
        }

        // Save to bytes
        let mut buf = BufWriter::new(Vec::new());
        doc.save(&mut buf)
            .map_err(|e| FastPdfError::PdfGeneration(format!("Failed to save PDF: {}", e)))?;

        buf.into_inner()
            .map_err(|e| FastPdfError::PdfGeneration(format!("Buffer error: {}", e)))
    }

    /// Pre-embed all fonts referenced by paint commands (embed each font only once).
    fn pre_embed_fonts(
        &self,
        doc: &PdfDocumentReference,
        page_commands: &[Vec<PaintCommand>],
        font_refs: &mut HashMap<FontKey, IndirectFontRef>,
    ) -> Result<()> {
        for commands in page_commands {
            for cmd in commands {
                if let PaintCommand::Text {
                    font_family,
                    font_weight,
                    italic,
                    ..
                } = cmd
                {
                    // Try external font first
                    let ext_key = FontKey {
                        family: font_family.clone(),
                        weight: *font_weight,
                        italic: *italic,
                        is_external: true,
                    };
                    if font_refs.contains_key(&ext_key) {
                        continue;
                    }

                    let builtin_key = FontKey {
                        family: font_family.clone(),
                        weight: *font_weight,
                        italic: *italic,
                        is_external: false,
                    };
                    if font_refs.contains_key(&builtin_key) {
                        continue;
                    }

                    // Try to embed external font
                    if let Some(ref cache) = self.font_cache {
                        if let Some(font_data) = cache.get_font(font_family, *font_weight, *italic)
                        {
                            if !font_data.data.is_empty() {
                                match doc.add_external_font(font_data.data.as_slice()) {
                                    Ok(f) => {
                                        font_refs.insert(ext_key, f);
                                        continue;
                                    }
                                    Err(e) => {
                                        log::warn!("Failed to embed font '{}': {}", font_family, e);
                                    }
                                }
                            }
                        }
                    }

                    // Fall back to built-in
                    let builtin = writer::resolve_builtin_font(font_family, *font_weight, *italic);
                    let f = doc.add_builtin_font(builtin).map_err(|e| {
                        FastPdfError::PdfGeneration(format!("Failed to add font: {}", e))
                    })?;
                    font_refs.insert(builtin_key, f);
                }
            }
        }
        Ok(())
    }

    /// Generate a PDF and save it to a file.
    pub fn generate_to_file(
        &self,
        pages: &[Page],
        page_commands: &[Vec<PaintCommand>],
        path: &str,
    ) -> Result<()> {
        let bytes = self.generate(pages, page_commands)?;
        std::fs::write(path, bytes)
            .map_err(|e| FastPdfError::PdfGeneration(format!("Failed to write file: {}", e)))?;
        Ok(())
    }

    /// Render paint commands to a PDF layer.
    fn render_commands_to_layer(
        &self,
        layer: &PdfLayerReference,
        commands: &[PaintCommand],
        page_layout: &PageLayout,
        doc: &PdfDocumentReference,
        font_refs: &HashMap<FontKey, IndirectFontRef>,
    ) -> Result<()> {
        let page_height_mm = page_layout.size.height * 25.4 / 96.0;

        for cmd in commands {
            match cmd {
                PaintCommand::FillRect {
                    x,
                    y,
                    width,
                    height,
                    color,
                } => {
                    if color.a < 0.01 || (*width < 0.1 && *height < 0.1) {
                        continue;
                    }
                    self.draw_filled_rect(layer, *x, *y, *width, *height, color, page_height_mm);
                }
                PaintCommand::StrokeRect {
                    x,
                    y,
                    width,
                    height,
                    color,
                    width_px,
                } => {
                    self.draw_stroked_rect(
                        layer,
                        *x,
                        *y,
                        *width,
                        *height,
                        color,
                        *width_px,
                        page_height_mm,
                    );
                }
                PaintCommand::Text {
                    x,
                    y,
                    text,
                    font_family,
                    font_size,
                    font_weight,
                    italic,
                    color,
                    align,
                    available_width,
                    direction_rtl,
                } => {
                    self.draw_text(
                        layer,
                        *x,
                        *y,
                        text,
                        font_family,
                        *font_size,
                        *font_weight,
                        *italic,
                        color,
                        align,
                        *available_width,
                        *direction_rtl,
                        page_height_mm,
                        doc,
                        font_refs,
                    )?;
                }
                PaintCommand::Image {
                    x,
                    y,
                    width,
                    height,
                    src,
                } => {
                    if let Err(e) =
                        self.draw_image(layer, *x, *y, *width, *height, src, page_height_mm)
                    {
                        log::warn!("Failed to embed image '{}': {}", src, e);
                    }
                }
                PaintCommand::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    color,
                    width_px,
                } => {
                    self.draw_line(layer, *x1, *y1, *x2, *y2, color, *width_px, page_height_mm);
                }
                PaintCommand::Link { .. } => {
                    // PDF links require annotation objects — handled in a future version
                }
            }
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_filled_rect(
        &self,
        layer: &PdfLayerReference,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        color: &Color,
        page_height_mm: f64,
    ) {
        let rgb = writer::color_to_rgb(color);
        layer.set_fill_color(printpdf::Color::Rgb(rgb));

        let x_mm = x * 25.4 / 96.0;
        let y_mm = page_height_mm - (y * 25.4 / 96.0) - (height * 25.4 / 96.0);
        let w_mm = width * 25.4 / 96.0;
        let h_mm = height * 25.4 / 96.0;

        let points = vec![
            (Point::new(Mm(x_mm as f32), Mm(y_mm as f32)), false),
            (Point::new(Mm((x_mm + w_mm) as f32), Mm(y_mm as f32)), false),
            (
                Point::new(Mm((x_mm + w_mm) as f32), Mm((y_mm + h_mm) as f32)),
                false,
            ),
            (Point::new(Mm(x_mm as f32), Mm((y_mm + h_mm) as f32)), false),
        ];

        let polygon = Polygon {
            rings: vec![points],
            mode: PaintMode::Fill,
            winding_order: WindingOrder::NonZero,
        };

        layer.add_polygon(polygon);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_stroked_rect(
        &self,
        layer: &PdfLayerReference,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        color: &Color,
        stroke_width: f64,
        page_height_mm: f64,
    ) {
        let rgb = writer::color_to_rgb(color);
        layer.set_outline_color(printpdf::Color::Rgb(rgb));
        layer.set_outline_thickness((stroke_width * 72.0 / 96.0) as f32);

        let x_mm = x * 25.4 / 96.0;
        let y_mm = page_height_mm - (y * 25.4 / 96.0) - (height * 25.4 / 96.0);
        let w_mm = width * 25.4 / 96.0;
        let h_mm = height * 25.4 / 96.0;

        let points = vec![
            (Point::new(Mm(x_mm as f32), Mm(y_mm as f32)), false),
            (Point::new(Mm((x_mm + w_mm) as f32), Mm(y_mm as f32)), false),
            (
                Point::new(Mm((x_mm + w_mm) as f32), Mm((y_mm + h_mm) as f32)),
                false,
            ),
            (Point::new(Mm(x_mm as f32), Mm((y_mm + h_mm) as f32)), false),
        ];

        let line = Line {
            points,
            is_closed: true,
        };

        layer.add_line(line);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_text(
        &self,
        layer: &PdfLayerReference,
        x: f64,
        y: f64,
        text: &str,
        font_family: &str,
        font_size: f64,
        font_weight: u32,
        italic: bool,
        color: &Color,
        align: &TextAlign,
        available_width: f64,
        direction_rtl: bool,
        page_height_mm: f64,
        doc: &PdfDocumentReference,
        font_refs: &HashMap<FontKey, IndirectFontRef>,
    ) -> Result<()> {
        if text.trim().is_empty() {
            return Ok(());
        }

        // Check if we have custom font data for complex text shaping
        let font_data_opt = self.font_cache.as_ref().and_then(|cache| {
            cache
                .get_font(font_family, font_weight, italic)
                .filter(|fd| !fd.data.is_empty())
        });

        let needs_shaping =
            shaping::needs_complex_layout(text) || shaping::contains_rtl(text) || direction_rtl;

        // If we have font data and text needs complex layout, use shaped glyph output
        if needs_shaping {
            if let Some(font_data) = &font_data_opt {
                return self.draw_shaped_text(
                    layer,
                    x,
                    y,
                    text,
                    font_family,
                    font_size,
                    font_weight,
                    italic,
                    color,
                    align,
                    available_width,
                    direction_rtl,
                    page_height_mm,
                    font_refs,
                    &font_data.data,
                );
            }
        }

        // --- Standard text path (built-in fonts or simple text) ---
        // Look up pre-cached font reference
        let font_ref =
            self.get_cached_font_ref(font_family, font_weight, italic, font_refs, doc)?;

        let rgb = writer::color_to_rgb(color);
        layer.set_fill_color(printpdf::Color::Rgb(rgb));

        let size_pt = font_size * 72.0 / 96.0;
        let builtin_name = writer::resolve_builtin_font_name(font_family, font_weight, italic);

        let x_mm = x * 25.4 / 96.0;
        let y_mm = page_height_mm - (y * 25.4 / 96.0) - (font_size * 25.4 / 96.0);

        // Use TTF metrics if available, otherwise built-in
        let (lines, use_ttf_measure) = if let Some(fd) = &font_data_opt {
            let lines = shaping::wrap_ttf_text(&fd.data, text, font_size, available_width);
            (lines, true)
        } else {
            (
                metrics::wrap_text_measured(text, builtin_name, font_size, available_width),
                false,
            )
        };

        let line_height_mm = font_size * 1.2 * 25.4 / 96.0;

        for (i, line) in lines.iter().enumerate() {
            if line.trim().is_empty() {
                continue;
            }

            let mut line_x_mm = x_mm;
            match align {
                TextAlign::Center => {
                    let text_width_px = if use_ttf_measure {
                        if let Some(fd) = &font_data_opt {
                            shaping::measure_ttf_text_width_px(&fd.data, line, font_size)
                        } else {
                            metrics::measure_text_width_px(line, builtin_name, font_size)
                        }
                    } else {
                        metrics::measure_text_width_px(line, builtin_name, font_size)
                    };
                    let text_width_mm = text_width_px * 25.4 / 96.0;
                    let avail_mm = available_width * 25.4 / 96.0;
                    line_x_mm += (avail_mm - text_width_mm) / 2.0;
                }
                TextAlign::Right => {
                    let text_width_px = if use_ttf_measure {
                        if let Some(fd) = &font_data_opt {
                            shaping::measure_ttf_text_width_px(&fd.data, line, font_size)
                        } else {
                            metrics::measure_text_width_px(line, builtin_name, font_size)
                        }
                    } else {
                        metrics::measure_text_width_px(line, builtin_name, font_size)
                    };
                    let text_width_mm = text_width_px * 25.4 / 96.0;
                    let avail_mm = available_width * 25.4 / 96.0;
                    line_x_mm += avail_mm - text_width_mm;
                }
                _ => {}
            }

            let line_y = y_mm - (i as f64 * line_height_mm);

            layer.use_text(
                line.as_str(),
                size_pt as f32,
                Mm(line_x_mm as f32),
                Mm(line_y as f32),
                &font_ref,
            );
        }

        Ok(())
    }

    /// Get a cached font reference, or create a fallback builtin if needed.
    fn get_cached_font_ref(
        &self,
        font_family: &str,
        font_weight: u32,
        italic: bool,
        font_refs: &HashMap<FontKey, IndirectFontRef>,
        doc: &PdfDocumentReference,
    ) -> Result<IndirectFontRef> {
        // Check external font first
        let ext_key = FontKey {
            family: font_family.to_string(),
            weight: font_weight,
            italic,
            is_external: true,
        };
        if let Some(f) = font_refs.get(&ext_key) {
            return Ok(f.clone());
        }

        // Check builtin
        let builtin_key = FontKey {
            family: font_family.to_string(),
            weight: font_weight,
            italic,
            is_external: false,
        };
        if let Some(f) = font_refs.get(&builtin_key) {
            return Ok(f.clone());
        }

        // Fallback: create builtin on the fly
        let builtin = writer::resolve_builtin_font(font_family, font_weight, italic);
        doc.add_builtin_font(builtin)
            .map_err(|e| FastPdfError::PdfGeneration(format!("Failed to add font: {}", e)))
    }

    /// Draw text using shaped glyphs (for Arabic/RTL/complex scripts).
    #[allow(clippy::too_many_arguments)]
    fn draw_shaped_text(
        &self,
        layer: &PdfLayerReference,
        x: f64,
        y: f64,
        text: &str,
        font_family: &str,
        font_size: f64,
        font_weight: u32,
        italic: bool,
        color: &Color,
        align: &TextAlign,
        available_width: f64,
        direction_rtl: bool,
        page_height_mm: f64,
        font_refs: &HashMap<FontKey, IndirectFontRef>,
        font_data: &[u8],
    ) -> Result<()> {
        // Get the pre-cached font reference
        let ext_key = FontKey {
            family: font_family.to_string(),
            weight: font_weight,
            italic,
            is_external: true,
        };
        let font_ref = font_refs.get(&ext_key).cloned().unwrap_or_else(|| {
            // Shouldn't happen if pre_embed_fonts ran correctly, but fall back
            let builtin_key = FontKey {
                family: font_family.to_string(),
                weight: font_weight,
                italic,
                is_external: false,
            };
            font_refs.get(&builtin_key).cloned().unwrap_or_else(|| {
                // Last resort
                panic!("Font not found in cache: {}", font_family)
            })
        });

        let rgb = writer::color_to_rgb(color);
        layer.set_fill_color(printpdf::Color::Rgb(rgb));

        let size_pt = font_size * 72.0 / 96.0;
        let x_mm = x * 25.4 / 96.0;
        let y_mm = page_height_mm - (y * 25.4 / 96.0) - (font_size * 25.4 / 96.0);

        // Word-wrap using shaped measurements
        let lines = shaping::wrap_ttf_text(font_data, text, font_size, available_width);
        let line_height_mm = font_size * 1.2 * 25.4 / 96.0;

        for (i, line) in lines.iter().enumerate() {
            if line.trim().is_empty() {
                continue;
            }

            // Shape this line
            let shaped = match shaping::shape_text(font_data, line, font_size, direction_rtl) {
                Some(s) => s,
                None => continue,
            };

            let text_width_px = shaped.width_px(font_size);

            // Calculate alignment offset
            let mut line_x_mm = x_mm;
            match align {
                TextAlign::Center => {
                    let text_width_mm = text_width_px * 25.4 / 96.0;
                    let avail_mm = available_width * 25.4 / 96.0;
                    line_x_mm += (avail_mm - text_width_mm) / 2.0;
                }
                TextAlign::Right => {
                    let text_width_mm = text_width_px * 25.4 / 96.0;
                    let avail_mm = available_width * 25.4 / 96.0;
                    line_x_mm += avail_mm - text_width_mm;
                }
                _ => {
                    // TextAlign::Left or Justify — no horizontal offset
                }
            }

            let line_y = y_mm - (i as f64 * line_height_mm);

            // Output shaped glyphs using positioned codepoints
            layer.begin_text_section();
            layer.set_font(&font_ref, size_pt as f32);
            layer.set_text_cursor(Mm(line_x_mm as f32), Mm(line_y as f32));

            let upem = shaped.units_per_em as f64;

            // Use cursor tracking to compute correct TJ displacements.
            //
            // After showing a glyph, the PDF viewer auto-advances the cursor
            // by the glyph's hmtx width (from the font's metrics table).
            // If the shaped advance differs (due to kerning, ligatures, etc.)
            // we must inject a TJ displacement to compensate.
            //
            // The TJ displacement is in thousandths of text space and is
            // SUBTRACTED from the current position:
            //   positive = move backward (left in LTR)
            //   negative = move forward (right in LTR)
            let mut positioned: Vec<(i64, u16)> = Vec::new();
            let mut desired_x: f64 = 0.0; // where we want the next glyph (font design units)
            let mut expected_x: f64 = 0.0; // where PDF thinks the cursor is (font design units)

            for run in &shaped.runs {
                for glyph in &run.glyphs {
                    // The glyph should be drawn at desired_x + x_offset
                    let draw_at = desired_x + glyph.x_offset as f64;

                    // Compute displacement needed to move PDF cursor from expected_x to draw_at
                    // displacement = (expected_x - draw_at) * 1000 / upem
                    let displacement = ((expected_x - draw_at) * 1000.0 / upem).round() as i64;

                    positioned.push((displacement, glyph.glyph_id));

                    // After showing this glyph, PDF auto-advances by hmtx advance
                    expected_x = draw_at + glyph.hmtx_advance as f64;

                    // Our desired cursor advances by the shaped advance
                    desired_x += glyph.advance as f64;
                }
            }

            layer.write_positioned_codepoints(positioned);
            layer.end_text_section();
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_line(
        &self,
        layer: &PdfLayerReference,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        color: &Color,
        stroke_width: f64,
        page_height_mm: f64,
    ) {
        let rgb = writer::color_to_rgb(color);
        layer.set_outline_color(printpdf::Color::Rgb(rgb));
        layer.set_outline_thickness((stroke_width * 72.0 / 96.0) as f32);

        let x1_mm = x1 * 25.4 / 96.0;
        let y1_mm = page_height_mm - (y1 * 25.4 / 96.0);
        let x2_mm = x2 * 25.4 / 96.0;
        let y2_mm = page_height_mm - (y2 * 25.4 / 96.0);

        let points = vec![
            (Point::new(Mm(x1_mm as f32), Mm(y1_mm as f32)), false),
            (Point::new(Mm(x2_mm as f32), Mm(y2_mm as f32)), false),
        ];

        let line = Line {
            points,
            is_closed: false,
        };

        layer.add_line(line);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_image(
        &self,
        layer: &PdfLayerReference,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        src: &str,
        page_height_mm: f64,
    ) -> Result<()> {
        // Load image data
        let data = if src.starts_with("data:") {
            // Parse data URI
            let uri = src.strip_prefix("data:").unwrap_or(src);
            let parts: Vec<&str> = uri.splitn(2, ',').collect();
            if parts.len() != 2 {
                return Err(FastPdfError::PdfGeneration(
                    "Invalid data URI for image".to_string(),
                ));
            }
            let meta = parts[0];
            let data_str = parts[1];
            if meta.contains("base64") {
                use base64::Engine;
                base64::engine::general_purpose::STANDARD
                    .decode(data_str)
                    .map_err(|e| {
                        FastPdfError::PdfGeneration(format!("Base64 decode error: {}", e))
                    })?
            } else {
                data_str.as_bytes().to_vec()
            }
        } else {
            // Read from file
            std::fs::read(src).map_err(|e| {
                FastPdfError::PdfGeneration(format!("Failed to read image '{}': {}", src, e))
            })?
        };

        // Decode with the image crate
        let img = ::image::load_from_memory(&data).map_err(|e| {
            FastPdfError::PdfGeneration(format!("Failed to decode image '{}': {}", src, e))
        })?;

        let pdf_image = printpdf::Image::from_dynamic_image(&img);

        // Position in PDF coordinates
        let x_mm = x * 25.4 / 96.0;
        let y_mm = page_height_mm - (y * 25.4 / 96.0) - (height * 25.4 / 96.0);
        let w_mm = (width * 25.4 / 96.0) as f32;
        let h_mm = (height * 25.4 / 96.0) as f32;

        // Calculate scale: printpdf renders images at their native DPI.
        // We need to scale to fit the desired width/height in mm.
        let img_w = img.width() as f32;
        let img_h = img.height() as f32;
        let dpi = 96.0f32;
        let native_w_mm = img_w * 25.4 / dpi;
        let native_h_mm = img_h * 25.4 / dpi;

        let scale_x = if native_w_mm > 0.0 {
            w_mm / native_w_mm
        } else {
            1.0
        };
        let scale_y = if native_h_mm > 0.0 {
            h_mm / native_h_mm
        } else {
            1.0
        };

        let transform = ImageTransform {
            translate_x: Some(Mm(x_mm as f32)),
            translate_y: Some(Mm(y_mm as f32)),
            scale_x: Some(scale_x),
            scale_y: Some(scale_y),
            dpi: Some(dpi),
            ..Default::default()
        };

        pdf_image.add_to_layer(layer.clone(), transform);

        Ok(())
    }
}

impl Default for PdfGenerator {
    fn default() -> Self {
        Self::new(PdfConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use crate::fonts::metrics;

    #[test]
    fn test_wrap_text() {
        let lines =
            metrics::wrap_text_measured("Hello World this is a test", "Helvetica", 12.0, 80.0);
        assert!(lines.len() >= 2);
    }

    #[test]
    fn test_wrap_text_short() {
        let lines = metrics::wrap_text_measured("Hi", "Helvetica", 12.0, 200.0);
        assert_eq!(lines.len(), 1);
    }
}
