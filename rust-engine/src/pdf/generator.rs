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
use crate::renderer::paint::{BorderStyle, PaintCommand, TextAlign, TextDecorationKind};

use super::writer;

/// Collected link annotations from rendering a page.
struct CollectedLink {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    url: String,
}

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

        // Collect all unique alpha keys across all pages for ExtGState post-processing.
        let mut all_alpha_keys: std::collections::HashSet<u32> = std::collections::HashSet::new();

        // Render first page
        if let Some(commands) = page_commands.first() {
            let layer = doc.get_page(page1_idx).get_layer(layer1_idx);
            let (links, alphas) = self.render_commands_to_layer(&layer, commands, &first_page.layout, &doc, &font_refs)?;
            all_alpha_keys.extend(&alphas);
            self.add_link_annotations(&doc, page1_idx, &links, &first_page.layout);
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
            let (links, alphas) = self.render_commands_to_layer(&layer, commands, &page.layout, &doc, &font_refs)?;
            all_alpha_keys.extend(&alphas);
            self.add_link_annotations(&doc, page_idx, &links, &page.layout);
        }

        // Save to bytes
        let mut buf = BufWriter::new(Vec::new());
        doc.save(&mut buf)
            .map_err(|e| FastPdfError::PdfGeneration(format!("Failed to save PDF: {}", e)))?;

        let raw_bytes = buf
            .into_inner()
            .map_err(|e| FastPdfError::PdfGeneration(format!("Buffer error: {}", e)))?;

        // Post-process: inject ExtGState resources for opacity if any were used.
        if all_alpha_keys.is_empty() {
            Ok(raw_bytes)
        } else {
            self.inject_alpha_extgstates(&raw_bytes, &all_alpha_keys)
        }
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

    /// Render paint commands to a PDF layer. Returns collected link annotations
    /// and the set of alpha keys used (for ExtGState post-processing).
    fn render_commands_to_layer(
        &self,
        layer: &PdfLayerReference,
        commands: &[PaintCommand],
        page_layout: &PageLayout,
        doc: &PdfDocumentReference,
        font_refs: &HashMap<FontKey, IndirectFontRef>,
    ) -> Result<(Vec<CollectedLink>, std::collections::HashSet<u32>)> {
        let page_height_mm = page_layout.size.height * 25.4 / 96.0;
        let mut links: Vec<CollectedLink> = Vec::new();
        let mut alpha_keys: std::collections::HashSet<u32> = std::collections::HashSet::new();

        for cmd in commands {
            match cmd {
                PaintCommand::FillRect {
                    x,
                    y,
                    width,
                    height,
                    color,
                    border_radius,
                    opacity,
                } => {
                    if color.a < 0.01 || (*width < 0.1 && *height < 0.1) {
                        continue;
                    }
                    if *opacity < 1.0 && *opacity > 0.01 {
                        // Apply true PDF opacity via ExtendedGraphicsState (ca/CA)
                        layer.save_graphics_state();
                        let key = (*opacity as f32 * 10000.0).round() as u32;
                        alpha_keys.insert(key);
                        self.apply_opacity(layer, *opacity as f32);
                        self.draw_filled_rect_rounded(
                            layer,
                            *x,
                            *y,
                            *width,
                            *height,
                            color,
                            *border_radius,
                            page_height_mm,
                        );
                        layer.restore_graphics_state();
                    } else if *opacity > 0.01 {
                        self.draw_filled_rect_rounded(
                            layer,
                            *x,
                            *y,
                            *width,
                            *height,
                            color,
                            *border_radius,
                            page_height_mm,
                        );
                    }
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
                    line_height,
                    letter_spacing,
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
                        *line_height,
                        *letter_spacing,
                        page_height_mm,
                        doc,
                        font_refs,
                    )?;
                }
                PaintCommand::TextDecoration {
                    x,
                    y,
                    width,
                    color,
                    thickness,
                    kind,
                    font_size,
                } => {
                    self.draw_text_decoration(
                        layer,
                        *x,
                        *y,
                        *width,
                        color,
                        *thickness,
                        kind,
                        *font_size,
                        page_height_mm,
                    );
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
                    style,
                } => {
                    self.draw_line_styled(
                        layer,
                        *x1,
                        *y1,
                        *x2,
                        *y2,
                        color,
                        *width_px,
                        style,
                        page_height_mm,
                    );
                }
                PaintCommand::Link {
                    x,
                    y,
                    width,
                    height,
                    url,
                } => {
                    links.push(CollectedLink {
                        x: *x,
                        y: *y,
                        width: *width,
                        height: *height,
                        url: url.clone(),
                    });
                }
            }
        }

        Ok((links, alpha_keys))
    }

    /// Add link annotations to a PDF page using the low-level lopdf API
    /// via printpdf's `extend_with` mechanism.
    fn add_link_annotations(
        &self,
        doc: &PdfDocumentReference,
        page_idx: PdfPageIndex,
        links: &[CollectedLink],
        page_layout: &PageLayout,
    ) {
        if links.is_empty() {
            return;
        }
        let page_height_mm = page_layout.size.height * 25.4 / 96.0;
        let page_ref = doc.get_page(page_idx);

        // Build annotation objects as raw lopdf dictionaries
        let mut annot_objects: Vec<lopdf::Object> = Vec::new();

        for link in links {
            // Convert CSS pixel coordinates to PDF points (bottom-left origin)
            let llx_pt = (link.x * 72.0 / 96.0) as f32;
            let lly_pt = ((page_height_mm * 72.0 / 25.4) - ((link.y + link.height) * 72.0 / 96.0)) as f32;
            let urx_pt = ((link.x + link.width) * 72.0 / 96.0) as f32;
            let ury_pt = ((page_height_mm * 72.0 / 25.4) - (link.y * 72.0 / 96.0)) as f32;

            let mut annot_dict = lopdf::Dictionary::new();
            annot_dict.set("Type", lopdf::Object::Name(b"Annot".to_vec()));
            annot_dict.set("Subtype", lopdf::Object::Name(b"Link".to_vec()));
            annot_dict.set(
                "Rect",
                lopdf::Object::Array(vec![
                    lopdf::Object::Real(llx_pt),
                    lopdf::Object::Real(lly_pt),
                    lopdf::Object::Real(urx_pt),
                    lopdf::Object::Real(ury_pt),
                ]),
            );
            // No visible border
            annot_dict.set(
                "Border",
                lopdf::Object::Array(vec![
                    lopdf::Object::Integer(0),
                    lopdf::Object::Integer(0),
                    lopdf::Object::Integer(0),
                ]),
            );
            // URI action
            let mut action_dict = lopdf::Dictionary::new();
            action_dict.set("S", lopdf::Object::Name(b"URI".to_vec()));
            action_dict.set(
                "URI",
                lopdf::Object::String(
                    link.url.as_bytes().to_vec(),
                    lopdf::StringFormat::Literal,
                ),
            );
            annot_dict.set("A", lopdf::Object::Dictionary(action_dict));

            annot_objects.push(lopdf::Object::Dictionary(annot_dict));
        }

        let mut ext_dict = lopdf::Dictionary::new();
        ext_dict.set("Annots", lopdf::Object::Array(annot_objects));
        page_ref.extend_with(ext_dict);
    }

    /// Post-process PDF bytes to inject ExtGState resources for opacity.
    /// Parses the PDF with lopdf, finds every page's Resources dictionary,
    /// and adds `/ExtGState` entries for each alpha key used.
    fn inject_alpha_extgstates(
        &self,
        raw_bytes: &[u8],
        alpha_keys: &std::collections::HashSet<u32>,
    ) -> Result<Vec<u8>> {
        let mut doc = lopdf::Document::load_mem(raw_bytes)
            .map_err(|e| FastPdfError::PdfGeneration(format!("lopdf load: {}", e)))?;

        // Build the ExtGState entries for all alpha levels used.
        let mut extgstate_entries: Vec<(Vec<u8>, lopdf::Object)> = Vec::new();
        for &key in alpha_keys {
            let alpha = key as f32 / 10000.0;
            let name = format!("FP_a{:04}", key);
            let mut gs_dict = lopdf::Dictionary::new();
            gs_dict.set("Type", lopdf::Object::Name(b"ExtGState".to_vec()));
            gs_dict.set("ca", lopdf::Object::Real(alpha));
            gs_dict.set("CA", lopdf::Object::Real(alpha));
            extgstate_entries.push((name.into_bytes(), lopdf::Object::Dictionary(gs_dict)));
        }

        // Walk every page and inject the ExtGState entries into its Resources.
        let page_ids: Vec<lopdf::ObjectId> = doc.page_iter().collect();
        for page_id in page_ids {
            if let Ok(page_dict) = doc.get_dictionary(page_id) {
                // Resolve the Resources object id.
                let resources_id = if let Ok(lopdf::Object::Reference(res_id)) =
                    page_dict.get(b"Resources")
                {
                    Some(*res_id)
                } else {
                    None
                };

                if let Some(res_id) = resources_id {
                    if let Ok(res_dict) = doc.get_dictionary_mut(res_id) {
                        // Get or create ExtGState sub-dictionary.
                        let extgs_obj = res_dict
                            .get(b"ExtGState")
                            .ok()
                            .cloned();
                        let mut extgs_dict = match extgs_obj {
                            Some(lopdf::Object::Dictionary(d)) => d,
                            Some(lopdf::Object::Reference(ref_id)) => {
                                doc.get_dictionary(ref_id)
                                    .map(|d| d.clone())
                                    .unwrap_or_default()
                            }
                            _ => lopdf::Dictionary::new(),
                        };
                        for (name, obj) in &extgstate_entries {
                            extgs_dict.set(name.clone(), obj.clone());
                        }
                        // Re-fetch res_dict since we may have borrowed doc above.
                        if let Ok(res_dict) = doc.get_dictionary_mut(res_id) {
                            res_dict.set(
                                "ExtGState",
                                lopdf::Object::Dictionary(extgs_dict),
                            );
                        }
                    }
                }
            }
        }

        let mut output = BufWriter::new(Vec::new());
        doc.save_to(&mut output)
            .map_err(|e| FastPdfError::PdfGeneration(format!("lopdf save: {}", e)))?;
        output
            .into_inner()
            .map_err(|e| FastPdfError::PdfGeneration(format!("Buffer error: {}", e)))
    }

    /// Apply PDF transparency by emitting a `gs` operator referencing a named
    /// ExtGState resource. The resource name encodes the alpha as `FP_aXXXX`
    /// (alpha × 10000, zero-padded). The actual ExtGState dictionary is injected
    /// into page resources during post-processing.
    fn apply_opacity(&self, layer: &PdfLayerReference, alpha: f32) {
        let key = (alpha * 10000.0).round() as u32;
        let gs_name = format!("FP_a{:04}", key);
        layer.add_operation(lopdf::content::Operation::new(
            "gs",
            vec![lopdf::Object::Name(gs_name.into_bytes())],
        ));
    }

    /// Draw a filled rectangle with optional rounded corners.
    #[allow(clippy::too_many_arguments)]
    fn draw_filled_rect_rounded(
        &self,
        layer: &PdfLayerReference,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        color: &Color,
        border_radius: f64,
        page_height_mm: f64,
    ) {
        let rgb = writer::color_to_rgb(color);
        layer.set_fill_color(printpdf::Color::Rgb(rgb));

        let x_mm = x * 25.4 / 96.0;
        let y_mm = page_height_mm - (y * 25.4 / 96.0) - (height * 25.4 / 96.0);
        let w_mm = width * 25.4 / 96.0;
        let h_mm = height * 25.4 / 96.0;

        if border_radius > 0.5 {
            // Draw rounded rectangle using bezier curves
            let r = (border_radius * 25.4 / 96.0).min(w_mm / 2.0).min(h_mm / 2.0);
            // Kappa constant for approximating circular arcs with cubics
            let k = r * 0.5522847498;

            let points = vec![
                // Start at top-left + radius (moving right)
                (
                    Point::new(Mm((x_mm + r) as f32), Mm((y_mm + h_mm) as f32)),
                    false,
                ),
                // Top edge to top-right corner
                (
                    Point::new(Mm((x_mm + w_mm - r) as f32), Mm((y_mm + h_mm) as f32)),
                    false,
                ),
                // Top-right corner (bezier)
                (
                    Point::new(
                        Mm((x_mm + w_mm - r + k) as f32),
                        Mm((y_mm + h_mm) as f32),
                    ),
                    true,
                ),
                (
                    Point::new(
                        Mm((x_mm + w_mm) as f32),
                        Mm((y_mm + h_mm - r + k) as f32),
                    ),
                    true,
                ),
                (
                    Point::new(
                        Mm((x_mm + w_mm) as f32),
                        Mm((y_mm + h_mm - r) as f32),
                    ),
                    false,
                ),
                // Right edge to bottom-right corner
                (
                    Point::new(Mm((x_mm + w_mm) as f32), Mm((y_mm + r) as f32)),
                    false,
                ),
                // Bottom-right corner (bezier)
                (
                    Point::new(
                        Mm((x_mm + w_mm) as f32),
                        Mm((y_mm + r - k) as f32),
                    ),
                    true,
                ),
                (
                    Point::new(
                        Mm((x_mm + w_mm - r + k) as f32),
                        Mm(y_mm as f32),
                    ),
                    true,
                ),
                (
                    Point::new(Mm((x_mm + w_mm - r) as f32), Mm(y_mm as f32)),
                    false,
                ),
                // Bottom edge to bottom-left corner
                (
                    Point::new(Mm((x_mm + r) as f32), Mm(y_mm as f32)),
                    false,
                ),
                // Bottom-left corner (bezier)
                (
                    Point::new(Mm((x_mm + r - k) as f32), Mm(y_mm as f32)),
                    true,
                ),
                (
                    Point::new(Mm(x_mm as f32), Mm((y_mm + r - k) as f32)),
                    true,
                ),
                (
                    Point::new(Mm(x_mm as f32), Mm((y_mm + r) as f32)),
                    false,
                ),
                // Left edge to top-left corner
                (
                    Point::new(Mm(x_mm as f32), Mm((y_mm + h_mm - r) as f32)),
                    false,
                ),
                // Top-left corner (bezier)
                (
                    Point::new(
                        Mm(x_mm as f32),
                        Mm((y_mm + h_mm - r + k) as f32),
                    ),
                    true,
                ),
                (
                    Point::new(
                        Mm((x_mm + r - k) as f32),
                        Mm((y_mm + h_mm) as f32),
                    ),
                    true,
                ),
                (
                    Point::new(Mm((x_mm + r) as f32), Mm((y_mm + h_mm) as f32)),
                    false,
                ),
            ];

            let polygon = Polygon {
                rings: vec![points],
                mode: PaintMode::Fill,
                winding_order: WindingOrder::NonZero,
            };
            layer.add_polygon(polygon);
        } else {
            // Simple rectangle (no rounding)
            let points = vec![
                (Point::new(Mm(x_mm as f32), Mm(y_mm as f32)), false),
                (
                    Point::new(Mm((x_mm + w_mm) as f32), Mm(y_mm as f32)),
                    false,
                ),
                (
                    Point::new(Mm((x_mm + w_mm) as f32), Mm((y_mm + h_mm) as f32)),
                    false,
                ),
                (
                    Point::new(Mm(x_mm as f32), Mm((y_mm + h_mm) as f32)),
                    false,
                ),
            ];

            let polygon = Polygon {
                rings: vec![points],
                mode: PaintMode::Fill,
                winding_order: WindingOrder::NonZero,
            };
            layer.add_polygon(polygon);
        }
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
        line_height: f64,
        letter_spacing: f64,
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
                    line_height,
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

        // Use CSS line-height instead of hardcoded 1.2x
        let line_height_mm = line_height * 25.4 / 96.0;

        // Apply letter-spacing if set
        if letter_spacing.abs() > 0.01 {
            let spacing_pt = letter_spacing * 72.0 / 96.0;
            layer.set_character_spacing(spacing_pt as f32);
        }

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
        line_height: f64,
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
        let line_height_mm = line_height * 25.4 / 96.0;

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

    /// Draw a text decoration (underline or line-through).
    #[allow(clippy::too_many_arguments)]
    fn draw_text_decoration(
        &self,
        layer: &PdfLayerReference,
        x: f64,
        y: f64,
        width: f64,
        color: &Color,
        thickness: f64,
        kind: &TextDecorationKind,
        font_size: f64,
        page_height_mm: f64,
    ) {
        let rgb = writer::color_to_rgb(color);
        layer.set_outline_color(printpdf::Color::Rgb(rgb));
        layer.set_outline_thickness((thickness * 72.0 / 96.0) as f32);

        let x_mm = x * 25.4 / 96.0;
        let w_mm = width * 25.4 / 96.0;

        // Position depends on decoration type
        let y_offset = match kind {
            TextDecorationKind::Underline => font_size * 1.15, // slightly below baseline
            TextDecorationKind::LineThrough => font_size * 0.55, // middle of text
        };

        let y_mm = page_height_mm - ((y + y_offset) * 25.4 / 96.0);

        let points = vec![
            (Point::new(Mm(x_mm as f32), Mm(y_mm as f32)), false),
            (Point::new(Mm((x_mm + w_mm) as f32), Mm(y_mm as f32)), false),
        ];

        let line = Line {
            points,
            is_closed: false,
        };

        layer.add_line(line);
    }

    /// Draw a line with optional style (solid, dashed, dotted).
    #[allow(clippy::too_many_arguments)]
    fn draw_line_styled(
        &self,
        layer: &PdfLayerReference,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        color: &Color,
        stroke_width: f64,
        style: &BorderStyle,
        page_height_mm: f64,
    ) {
        let rgb = writer::color_to_rgb(color);
        layer.set_outline_color(printpdf::Color::Rgb(rgb));
        layer.set_outline_thickness((stroke_width * 72.0 / 96.0) as f32);

        // Apply dash pattern based on style
        let stroke_pt = (stroke_width * 72.0 / 96.0).max(0.5) as f32;
        match style {
            BorderStyle::Dashed => {
                let dash_len = (stroke_pt * 3.0).max(2.0);
                let gap_len = (stroke_pt * 2.0).max(1.5);
                layer.set_line_dash_pattern(LineDashPattern {
                    dash_1: Some(dash_len as i64),
                    gap_1: Some(gap_len as i64),
                    ..Default::default()
                });
            }
            BorderStyle::Dotted => {
                let dot = stroke_pt.max(1.0);
                layer.set_line_dash_pattern(LineDashPattern {
                    dash_1: Some(dot as i64),
                    gap_1: Some((dot * 2.0) as i64),
                    ..Default::default()
                });
                // Use round line cap for dots
                layer.set_line_cap_style(LineCapStyle::Round);
            }
            _ => {
                // Solid or default — reset dash
                layer.set_line_dash_pattern(LineDashPattern::default());
            }
        }

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

        // Reset dash pattern and cap style after drawing
        match style {
            BorderStyle::Dashed | BorderStyle::Dotted => {
                layer.set_line_dash_pattern(LineDashPattern::default());
                layer.set_line_cap_style(LineCapStyle::Butt);
            }
            _ => {}
        }
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
