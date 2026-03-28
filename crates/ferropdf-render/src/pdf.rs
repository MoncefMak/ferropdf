use crate::display_list::{DrawOp, PageDisplayList};
use crate::font_subsetter::{
    encode_for_cid_font, encode_winansi, load_font_data, subset_font, write_cid_font,
    EmbeddedFont, FontKey,
};
use crate::RenderOptions;
use ferropdf_core::{FerroError, PageConfig};
use flate2::write::ZlibEncoder;
use flate2::Compression;
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect as PdfWriterRect, Ref, Str, TextStr};
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

// =============================================================================
// HTML/CSS → PDF COORDINATE CONVERSION
// =============================================================================
//
// INTERNAL UNIT: typographic points (pt), 1 pt = 1/72 inch
// This is the native PDF format unit → no size conversion needed.
//
// ONLY TRANSFORMATION: Y-axis inversion
//   HTML/CSS: Y=0 at TOP, Y increases DOWNWARD
//   PDF     : Y=0 at BOTTOM, Y increases UPWARD
//   Formula : pdf_y = page_height_pt - html_y_pt - html_height_pt
// =============================================================================

/// Rectangle in PDF coordinates (origin bottom-left, unit = points).
#[derive(Debug, Clone, Copy)]
struct PdfRect {
    x: f32,      // points, from the left edge
    y: f32,      // points, from the BOTTOM edge (PDF convention)
    width: f32,  // points
    height: f32, // points
}

/// Converts a rectangle (origin top-left, pt)
/// to a PDF rectangle (origin bottom-left, pt).
/// Only the Y-axis is inverted, no unit change.
fn to_pdf_rect(x: f32, y: f32, width: f32, height: f32, page_height_pt: f32) -> PdfRect {
    PdfRect {
        x,
        y: page_height_pt - y - height,
        width,
        height,
    }
}

/// Converts a point Y coordinate (pt, from top)
/// to PDF (pt, from bottom).
fn y_to_pdf(y: f32, page_height_pt: f32) -> f32 {
    page_height_pt - y
}

/// Write a complete PDF document from display lists.
pub fn write_pdf(
    pages: &[PageDisplayList],
    config: &PageConfig,
    opts: &RenderOptions,
    ext_font_db: Option<&fontdb::Database>,
) -> ferropdf_core::Result<Vec<u8>> {
    let mut pdf = Pdf::new();
    let mut ref_id = 1u32;
    let mut next_ref = || {
        let r = Ref::new(ref_id as i32);
        ref_id += 1;
        r
    };

    let catalog_ref = next_ref();
    let page_tree_ref = next_ref();

    // Built-in Type1 fallback fonts
    let font_ref = next_ref();
    let font_bold_ref = next_ref();
    let font_italic_ref = next_ref();

    let (page_w, page_h) = config.size.dimensions_pt();

    // ── Resolve embedded fonts ──
    let owned_db;
    let font_db: &fontdb::Database = match ext_font_db {
        Some(db) => db,
        None => {
            owned_db = {
                let mut db = fontdb::Database::new();
                db.load_system_fonts();
                db
            };
            &owned_db
        }
    };

    let mut font_cache: HashMap<FontKey, Option<String>> = HashMap::new();
    let mut embedded_fonts: Vec<EmbeddedFont> = Vec::new();
    let mut font_name_counter = 3u32; // F1, F2, F3 are reserved for Type1

    // Temporary: collect raw font data and used glyphs per font
    let mut font_raw_data: HashMap<String, Vec<u8>> = HashMap::new(); // pdf_name → raw data
    let mut font_used_chars: HashMap<String, std::collections::HashSet<u16>> = HashMap::new(); // pdf_name → glyph IDs

    // Single pass: discover fonts AND collect glyph IDs simultaneously
    for display_list in pages {
        for op in &display_list.ops {
            if let DrawOp::DrawText {
                text,
                font_family,
                bold,
                italic,
                ..
            } = op
            {
                let family_name = font_family.first().cloned().unwrap_or_default();
                let key = FontKey {
                    family: family_name.clone(),
                    bold: *bold,
                    italic: *italic,
                };
                // Discover font on first encounter
                if !font_cache.contains_key(&key) {
                    match load_font_data(font_db, &family_name, *bold, *italic) {
                        Some(data) => {
                            font_name_counter += 1;
                            let pdf_name = format!("F{}", font_name_counter);
                            font_raw_data.insert(pdf_name.clone(), data);
                            font_used_chars
                                .insert(pdf_name.clone(), std::collections::HashSet::new());
                            font_cache.insert(key.clone(), Some(pdf_name));
                        }
                        None => {
                            font_cache.insert(key.clone(), None);
                            continue;
                        }
                    }
                }
                // Collect glyph IDs for this text
                if let Some(Some(pdf_name)) = font_cache.get(&key) {
                    if let (Some(raw_data), Some(used_gids)) = (
                        font_raw_data.get(pdf_name),
                        font_used_chars.get_mut(pdf_name),
                    ) {
                        if let Ok(face) = ttf_parser::Face::parse(raw_data, 0) {
                            used_gids.insert(0); // .notdef
                            for ch in text.chars() {
                                if let Some(gid) = face.glyph_index(ch) {
                                    used_gids.insert(gid.0);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Phase 3: Subset fonts and build EmbeddedFont entries
    let pdf_names: Vec<String> = font_raw_data.keys().cloned().collect();
    for pdf_name in pdf_names {
        let raw_data = font_raw_data.remove(&pdf_name).unwrap();
        let used_gids = font_used_chars.remove(&pdf_name).unwrap();

        let type0_ref = next_ref();
        let cid_font_ref = next_ref();
        let descriptor_ref = next_ref();
        let font_stream_ref = next_ref();
        let tounicode_ref = next_ref();

        // Subset the font to only include used glyphs
        let (subset_data, gid_remapping) = subset_font(&raw_data, &used_gids);

        embedded_fonts.push(EmbeddedFont {
            original_data: raw_data,
            subset_data,
            gid_remapping,
            type0_ref,
            font_stream_ref,
            descriptor_ref,
            cid_font_ref,
            tounicode_ref,
            pdf_name,
        });
    }

    // Collect page refs
    let mut page_refs = Vec::new();
    let mut content_refs = Vec::new();
    for _ in pages {
        page_refs.push(next_ref());
        content_refs.push(next_ref());
    }

    // ── Load images ──
    let mut image_map: HashMap<String, LoadedImage> = HashMap::new();
    let mut img_counter = 0u32;
    for display_list in pages {
        for op in &display_list.ops {
            if let DrawOp::DrawImage { src, .. } = op {
                if !image_map.contains_key(src) {
                    match load_image(src) {
                        Ok((rgb_data, width, height)) => {
                            let pdf_ref = next_ref();
                            let pdf_name = format!("Im{img_counter}");
                            img_counter += 1;
                            image_map.insert(
                                src.clone(),
                                LoadedImage {
                                    rgb_data,
                                    width,
                                    height,
                                    pdf_ref,
                                    pdf_name,
                                },
                            );
                        }
                        Err(e) => {
                            log::warn!("Failed to load image {}: {}", src, e);
                        }
                    }
                }
            }
        }
    }

    // Write catalog
    pdf.catalog(catalog_ref).pages(page_tree_ref);

    // Write page tree
    let mut page_tree = pdf.pages(page_tree_ref);
    page_tree.count(pages.len() as i32);
    page_tree.kids(page_refs.iter().copied());
    page_tree.finish();

    // Write each page
    for (i, display_list) in pages.iter().enumerate() {
        let page_ref = page_refs[i];
        let content_ref = content_refs[i];

        // Page dictionary
        let mut page = pdf.page(page_ref);
        page.parent(page_tree_ref);
        page.media_box(PdfWriterRect::new(0.0, 0.0, page_w, page_h));

        // Resources with fonts and images
        let mut resources = page.resources();
        let mut fonts = resources.fonts();
        fonts.pair(Name(b"F1"), font_ref);
        fonts.pair(Name(b"F2"), font_bold_ref);
        fonts.pair(Name(b"F3"), font_italic_ref);
        for ef in &embedded_fonts {
            fonts.pair(Name(ef.pdf_name.as_bytes()), ef.type0_ref);
        }
        fonts.finish();
        if !image_map.is_empty() {
            let mut xobjects = resources.x_objects();
            for img in image_map.values() {
                xobjects.pair(Name(img.pdf_name.as_bytes()), img.pdf_ref);
            }
            xobjects.finish();
        }
        resources.finish();

        page.contents(content_ref);
        page.finish();

        // Content stream
        let mut content = Content::new();

        for op in &display_list.ops {
            match op {
                DrawOp::FillRect { rect, color, .. } => {
                    if !color.is_transparent() {
                        content.set_fill_rgb(color.r, color.g, color.b);
                        let pr = to_pdf_rect(rect.x, rect.y, rect.width, rect.height, page_h);
                        content.rect(pr.x, pr.y, pr.width, pr.height);
                        content.fill_nonzero();
                    }
                }
                DrawOp::StrokeRect {
                    rect, color, width, ..
                } => {
                    content.set_stroke_rgb(color.r, color.g, color.b);
                    content.set_line_width(*width);
                    // Borders are lines — use move_to/line_to for precise placement
                    // StrokeRect encodes a single border side as a degenerate rect:
                    //   height==0 → horizontal line, width==0 → vertical line
                    if rect.height < 0.01 {
                        // Horizontal line at rect.y
                        let py = y_to_pdf(rect.y, page_h);
                        let x1 = rect.x;
                        let x2 = rect.x + rect.width;
                        content.move_to(x1, py);
                        content.line_to(x2, py);
                        content.stroke();
                    } else if rect.width < 0.01 {
                        // Vertical line at rect.x
                        let px = rect.x;
                        let y1 = y_to_pdf(rect.y, page_h);
                        let y2 = y_to_pdf(rect.y + rect.height, page_h);
                        content.move_to(px, y1);
                        content.line_to(px, y2);
                        content.stroke();
                    } else {
                        let pr = to_pdf_rect(rect.x, rect.y, rect.width, rect.height, page_h);
                        content.rect(pr.x, pr.y, pr.width.max(0.1), pr.height.max(0.1));
                        content.stroke();
                    }
                }
                DrawOp::DrawText {
                    text,
                    x,
                    y,
                    font_size,
                    color,
                    font_family,
                    bold,
                    italic,
                    text_align,
                    container_width,
                    ..
                } => {
                    let family_name = font_family.first().cloned().unwrap_or_default();
                    let key = FontKey {
                        family: family_name,
                        bold: *bold,
                        italic: *italic,
                    };

                    // Look up embedded font data for this key
                    let ef_option =
                        font_cache
                            .get(&key)
                            .and_then(|v| v.as_ref())
                            .and_then(|pdf_name| {
                                embedded_fonts.iter().find(|f| &f.pdf_name == pdf_name)
                            });
                    let font_data = ef_option.map(|ef| ef.original_data.as_slice());

                    // Font size is already in pt — no conversion needed
                    let font_size_pt = *font_size;

                    let line_text = text.trim();
                    if line_text.is_empty() {
                        continue;
                    }

                    // Measure line width for text-align
                    let line_width_pt = measure_text_width(line_text, *font_size, font_data);

                    // Compute aligned X in pt
                    let aligned_x = match text_align {
                        ferropdf_core::TextAlign::Left => *x,
                        ferropdf_core::TextAlign::Right => *x + container_width - line_width_pt,
                        ferropdf_core::TextAlign::Center => {
                            *x + (container_width - line_width_pt) / 2.0
                        }
                        ferropdf_core::TextAlign::Justify => *x,
                    };

                    // Convert to PDF coordinates (Y-axis flip only, no unit conversion)
                    let pdf_x = aligned_x;
                    let pdf_y = y_to_pdf(*y, page_h);

                    content.set_fill_rgb(color.r, color.g, color.b);
                    content.begin_text();

                    match ef_option {
                        Some(ef) => {
                            content.set_font(Name(ef.pdf_name.as_bytes()), font_size_pt);
                            content.next_line(pdf_x, pdf_y);
                            let encoded = encode_for_cid_font(
                                line_text,
                                &ef.original_data,
                                ef.gid_remapping.as_ref(),
                            );
                            content.show(Str(&encoded));
                        }
                        None => {
                            let font_name = if *bold {
                                "F2"
                            } else if *italic {
                                "F3"
                            } else {
                                "F1"
                            };
                            content.set_font(Name(font_name.as_bytes()), font_size_pt);
                            content.next_line(pdf_x, pdf_y);
                            content.show(Str(&encode_winansi(line_text)));
                        }
                    }

                    content.end_text();
                }
                DrawOp::Save => {
                    content.save_state();
                }
                DrawOp::Restore => {
                    content.restore_state();
                }
                DrawOp::DrawImage { src, rect } => {
                    if let Some(img) = image_map.get(src) {
                        let pr = to_pdf_rect(rect.x, rect.y, rect.width, rect.height, page_h);
                        // Position image: scale to target size and translate
                        content.save_state();
                        content.transform([pr.width, 0.0, 0.0, pr.height, pr.x, pr.y]);
                        content.x_object(Name(img.pdf_name.as_bytes()));
                        content.restore_state();
                    }
                }
                _ => {}
            }
        }

        let content_bytes = content.finish();
        pdf.stream(content_ref, &content_bytes);
    }

    // Write built-in Type1 fallback fonts
    write_type1_font(&mut pdf, font_ref, "Helvetica");
    write_type1_font(&mut pdf, font_bold_ref, "Helvetica-Bold");
    write_type1_font(&mut pdf, font_italic_ref, "Helvetica-Oblique");

    // Write embedded CIDFont Type0 fonts
    for ef in &embedded_fonts {
        write_cid_font(&mut pdf, ef)?;
    }

    // Write image XObjects
    for img in image_map.values() {
        // Deflate-compress the raw RGB data
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
        encoder
            .write_all(&img.rgb_data)
            .map_err(|e| FerroError::Image(format!("Image compression error: {e}")))?;
        let compressed = encoder
            .finish()
            .map_err(|e| FerroError::Image(format!("Image compression finish error: {e}")))?;

        let mut xobj = pdf.image_xobject(img.pdf_ref, &compressed);
        xobj.filter(Filter::FlateDecode);
        xobj.width(img.width as i32);
        xobj.height(img.height as i32);
        xobj.color_space().device_rgb();
        xobj.bits_per_component(8);
        xobj.finish();
    }

    // Write metadata
    let mut info = pdf.document_info(next_ref());
    info.producer(TextStr("ferropdf"));
    if let Some(ref title) = opts.title {
        info.title(TextStr(title));
    }
    if let Some(ref author) = opts.author {
        info.author(TextStr(author));
    }
    info.finish();

    Ok(pdf.finish())
}

// ── Helpers ──

fn write_type1_font(pdf: &mut Pdf, font_ref: Ref, base_font: &str) {
    let mut font = pdf.type1_font(font_ref);
    font.base_font(Name(base_font.as_bytes()));
    font.encoding_predefined(Name(b"WinAnsiEncoding"));
    font.finish();
}

/// Measure text width using actual glyph advances from the embedded font.
/// Falls back to a rough estimate if font data is unavailable.
fn measure_text_width(text: &str, font_size: f32, font_data: Option<&[u8]>) -> f32 {
    if let Some(data) = font_data {
        if let Ok(face) = ttf_parser::Face::parse(data, 0) {
            let units_per_em = face.units_per_em() as f32;
            let scale = font_size / units_per_em;
            let mut width = 0.0f32;
            for ch in text.chars() {
                if let Some(gid) = face.glyph_index(ch) {
                    if let Some(adv) = face.glyph_hor_advance(gid) {
                        width += adv as f32 * scale;
                    }
                }
            }
            return width;
        }
    }
    // Fallback heuristic for Type1 Helvetica
    text.chars().count() as f32 * font_size * 0.52
}

// =============================================================================
// IMAGE LOADING
// =============================================================================

struct LoadedImage {
    rgb_data: Vec<u8>,
    width: u32,
    height: u32,
    pdf_ref: Ref,
    pdf_name: String,
}

/// Load image from a source string (file path or data URI).
fn load_image(src: &str) -> Result<(Vec<u8>, u32, u32), FerroError> {
    let data = if src.starts_with("data:") {
        // data URI — extract base64 payload
        let comma = src
            .find(',')
            .ok_or_else(|| FerroError::Image("Invalid data URI: no comma".into()))?;
        let encoded = &src[comma + 1..];
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded)
            .map_err(|e| FerroError::Image(format!("Base64 decode error: {e}")))?
    } else {
        // File path
        std::fs::read(Path::new(src))
            .map_err(|e| FerroError::Image(format!("Cannot read image {src}: {e}")))?
    };

    let img = image::load_from_memory(&data)
        .map_err(|e| FerroError::Image(format!("Cannot decode image {src}: {e}")))?;
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    Ok((rgb.into_raw(), w, h))
}
