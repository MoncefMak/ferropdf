use std::collections::HashMap;
use pdf_writer::{Pdf, Ref, Content, Finish, Name, Str, TextStr, Rect as PdfWriterRect, Filter};
use pdf_writer::types::FontFlags;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::Write;
use ferropdf_core::{PageConfig, FerroError};
use crate::display_list::{DrawOp, PageDisplayList};
use crate::RenderOptions;

// =============================================================================
// CONVERSION COORDONNÉES HTML/CSS → PDF
// =============================================================================
//
// HTML/CSS : Y=0 en HAUT, Y augmente vers le BAS, unité = pixel (1/96 pouce)
// PDF      : Y=0 en BAS,  Y augmente vers le HAUT, unité = point (1/72 pouce)
//
// Facteur de conversion pixels CSS → points PDF :
//   1 point = 1/72 pouce
//   1 pixel = 1/96 pouce
//   ratio   = 72/96 = 0.75
// =============================================================================

/// Facteur de conversion pixels CSS → points PDF.
const PX_TO_PT: f32 = 72.0 / 96.0;

/// Rectangle en coordonnées PDF (origine bas-gauche, unité = points).
#[derive(Debug, Clone, Copy)]
struct PdfRect {
    x:      f32, // points, depuis le bord gauche
    y:      f32, // points, depuis le bord BAS (convention PDF)
    width:  f32, // points
    height: f32, // points
}

/// Convertit un rectangle HTML/CSS (origine haut-gauche, pixels)
/// en rectangle PDF (origine bas-gauche, points).
///
/// Formule :
///   pdf_x     = html_x * PX_TO_PT
///   pdf_y     = page_height_pt - (html_y * PX_TO_PT) - (html_height * PX_TO_PT)
///   pdf_width = html_width  * PX_TO_PT
///   pdf_h     = html_height * PX_TO_PT
fn html_to_pdf_rect(
    html_x:         f32,
    html_y:         f32,
    html_width:     f32,
    html_height:    f32,
    page_height_pt: f32,
) -> PdfRect {
    PdfRect {
        x:      html_x      * PX_TO_PT,
        y:      page_height_pt
                - (html_y      * PX_TO_PT)
                - (html_height * PX_TO_PT),
        width:  html_width  * PX_TO_PT,
        height: html_height * PX_TO_PT,
    }
}

/// Convertit une coordonnée Y ponctuelle (ex: position de texte)
/// de HTML/CSS (pixels, depuis le haut) vers PDF (points, depuis le bas).
fn html_y_to_pdf(html_y: f32, page_height_pt: f32) -> f32 {
    page_height_pt - (html_y * PX_TO_PT)
}

/// Convertit une taille de police pixels → points PDF.
fn font_size_to_pt(px: f32) -> f32 {
    px * PX_TO_PT
}

/// A resolved font ready to be embedded in the PDF.
struct EmbeddedFont {
    /// Raw TTF/OTF bytes
    data: Vec<u8>,
    /// Parsed face for metric/glyph lookups
    /// We store indices into `data` and re-parse as needed
    /// PDF object refs
    type0_ref: Ref,
    font_stream_ref: Ref,
    descriptor_ref: Ref,
    cid_font_ref: Ref,
    tounicode_ref: Ref,
    /// PDF resource name, e.g. "F4"
    pdf_name: String,
}

/// Key to deduplicate fonts
#[derive(Hash, Eq, PartialEq, Clone)]
struct FontKey {
    family: String,
    bold: bool,
    italic: bool,
}

/// Write a complete PDF document from display lists.
pub fn write_pdf(
    pages: &[PageDisplayList],
    config: &PageConfig,
    opts: &RenderOptions,
) -> ferropdf_core::Result<Vec<u8>> {
    let mut pdf = Pdf::new();
    let mut ref_id = 1u32;
    let mut next_ref = || { let r = Ref::new(ref_id as i32); ref_id += 1; r };

    let catalog_ref = next_ref();
    let page_tree_ref = next_ref();

    // Built-in Type1 fallback fonts
    let font_ref = next_ref();
    let font_bold_ref = next_ref();
    let font_italic_ref = next_ref();

    let (page_w, page_h) = config.size.dimensions_pt();

    // ── Resolve embedded fonts ──
    let mut font_db = fontdb::Database::new();
    font_db.load_system_fonts();

    let mut font_cache: HashMap<FontKey, Option<String>> = HashMap::new();
    let mut embedded_fonts: Vec<EmbeddedFont> = Vec::new();
    let mut font_name_counter = 3u32; // F1, F2, F3 are reserved for Type1

    // Scan all DrawText ops to discover which fonts we need
    for display_list in pages {
        for op in &display_list.ops {
            if let DrawOp::DrawText { font_family, bold, italic, .. } = op {
                let family_name = font_family.first().cloned().unwrap_or_default();
                let key = FontKey {
                    family: family_name.clone(),
                    bold: *bold,
                    italic: *italic,
                };
                if font_cache.contains_key(&key) {
                    continue;
                }
                // Try to resolve from system fonts
                match load_font_data(&font_db, &family_name, *bold, *italic) {
                    Some(data) => {
                        font_name_counter += 1;
                        let pdf_name = format!("F{}", font_name_counter);

                        let type0_ref = next_ref();
                        let cid_font_ref = next_ref();
                        let descriptor_ref = next_ref();
                        let font_stream_ref = next_ref();
                        let tounicode_ref = next_ref();

                        font_cache.insert(key, Some(pdf_name.clone()));
                        embedded_fonts.push(EmbeddedFont {
                            data,
                            type0_ref,
                            font_stream_ref,
                            descriptor_ref,
                            cid_font_ref,
                            tounicode_ref,
                            pdf_name,
                        });
                    }
                    None => {
                        // No system font found — will fall back to Helvetica
                        font_cache.insert(key, None);
                    }
                }
            }
        }
    }

    // Collect page refs
    let mut page_refs = Vec::new();
    let mut content_refs = Vec::new();
    for _ in pages {
        page_refs.push(next_ref());
        content_refs.push(next_ref());
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

        // Resources with fonts
        let mut resources = page.resources();
        let mut fonts = resources.fonts();
        fonts.pair(Name(b"F1"), font_ref);
        fonts.pair(Name(b"F2"), font_bold_ref);
        fonts.pair(Name(b"F3"), font_italic_ref);
        for ef in &embedded_fonts {
            fonts.pair(Name(ef.pdf_name.as_bytes()), ef.type0_ref);
        }
        fonts.finish();
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
                        let pr = html_to_pdf_rect(rect.x, rect.y, rect.width, rect.height, page_h);
                        content.rect(pr.x, pr.y, pr.width, pr.height);
                        content.fill_nonzero();
                    }
                }
                DrawOp::StrokeRect { rect, color, width, .. } => {
                    content.set_stroke_rgb(color.r, color.g, color.b);
                    content.set_line_width(*width * PX_TO_PT);
                    // Borders are lines — use move_to/line_to for precise placement
                    // StrokeRect encodes a single border side as a degenerate rect:
                    //   height==0 → horizontal line, width==0 → vertical line
                    if rect.height < 0.01 {
                        // Horizontal line at rect.y
                        let py = html_y_to_pdf(rect.y, page_h);
                        let x1 = rect.x * PX_TO_PT;
                        let x2 = (rect.x + rect.width) * PX_TO_PT;
                        content.move_to(x1, py);
                        content.line_to(x2, py);
                        content.stroke();
                    } else if rect.width < 0.01 {
                        // Vertical line at rect.x
                        let px = rect.x * PX_TO_PT;
                        let y1 = html_y_to_pdf(rect.y, page_h);
                        let y2 = html_y_to_pdf(rect.y + rect.height, page_h);
                        content.move_to(px, y1);
                        content.line_to(px, y2);
                        content.stroke();
                    } else {
                        let pr = html_to_pdf_rect(rect.x, rect.y, rect.width, rect.height, page_h);
                        content.rect(pr.x, pr.y, pr.width.max(0.1), pr.height.max(0.1));
                        content.stroke();
                    }
                }
                DrawOp::DrawText { text, x, y, font_size, color, font_family, bold, italic, text_align, container_width, .. } => {
                    let family_name = font_family.first().cloned().unwrap_or_default();
                    let key = FontKey {
                        family: family_name,
                        bold: *bold,
                        italic: *italic,
                    };

                    // Look up embedded font data for this key
                    let ef_option = font_cache.get(&key)
                        .and_then(|v| v.as_ref())
                        .and_then(|pdf_name| embedded_fonts.iter().find(|f| &f.pdf_name == pdf_name));
                    let font_data = ef_option.map(|ef| ef.data.as_slice());

                    // Convert px values to pt for PDF
                    let font_size_pt = font_size_to_pt(*font_size);

                    // Word-wrap uses pixel units (layout is in px)
                    let lines = wrap_text(text, *font_size, *container_width, font_data);
                    let line_height_px = font_size * 1.2;

                    for (line_idx, line) in lines.iter().enumerate() {
                        let line_text = line.trim();
                        if line_text.is_empty() {
                            continue;
                        }

                        // Measure in pixels, then scale for alignment
                        let line_width_px = measure_text_width(line_text, *font_size, font_data);

                        // Compute aligned X in pixels, then convert to pt
                        let aligned_x_px = match text_align {
                            ferropdf_core::TextAlign::Left => *x,
                            ferropdf_core::TextAlign::Right => *x + container_width - line_width_px,
                            ferropdf_core::TextAlign::Center => *x + (container_width - line_width_px) / 2.0,
                            ferropdf_core::TextAlign::Justify => *x,
                        };

                        let line_y_px = *y + (line_idx as f32) * line_height_px;

                        // Convert to PDF coordinates
                        let pdf_x = aligned_x_px * PX_TO_PT;
                        let pdf_y = html_y_to_pdf(line_y_px, page_h);

                        content.set_fill_rgb(color.r, color.g, color.b);
                        content.begin_text();

                        match ef_option {
                            Some(ef) => {
                                content.set_font(Name(ef.pdf_name.as_bytes()), font_size_pt);
                                content.next_line(pdf_x, pdf_y);
                                let encoded = encode_for_cid_font(line_text, &ef.data);
                                content.show(Str(&encoded));
                            }
                            None => {
                                let font_name = if *bold { "F2" } else if *italic { "F3" } else { "F1" };
                                content.set_font(Name(font_name.as_bytes()), font_size_pt);
                                content.next_line(pdf_x, pdf_y);
                                content.show(Str(&encode_winansi(line_text)));
                            }
                        }

                        content.end_text();
                    }
                }
                DrawOp::Save => { content.save_state(); }
                DrawOp::Restore => { content.restore_state(); }
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

// ── Font resolution via fontdb ──

fn load_font_data(
    db: &fontdb::Database,
    family: &str,
    bold: bool,
    italic: bool,
) -> Option<Vec<u8>> {
    if family.is_empty() {
        return None;
    }

    let query = fontdb::Query {
        families: &[
            fontdb::Family::Name(family),
            fontdb::Family::SansSerif,
        ],
        weight: if bold { fontdb::Weight(700) } else { fontdb::Weight(400) },
        style: if italic { fontdb::Style::Italic } else { fontdb::Style::Normal },
        ..Default::default()
    };

    let id = db.query(&query)?;
    db.with_face_data(id, |data, _| data.to_vec())
}

// ── CIDFont embedding ──

fn write_cid_font(pdf: &mut Pdf, ef: &EmbeddedFont) -> ferropdf_core::Result<()> {
    let face = ttf_parser::Face::parse(&ef.data, 0)
        .map_err(|e| FerroError::Font(format!("Failed to parse TTF: {:?}", e)))?;

    let units_per_em = face.units_per_em() as f32;
    let scale = 1000.0 / units_per_em;
    let ascender = face.ascender() as f32 * scale;
    let descender = face.descender() as f32 * scale;
    let cap_height = face.capital_height().unwrap_or(face.ascender()) as f32 * scale;

    let bbox = face.global_bounding_box();
    let font_bbox = PdfWriterRect::new(
        bbox.x_min as f32 * scale,
        bbox.y_min as f32 * scale,
        bbox.x_max as f32 * scale,
        bbox.y_max as f32 * scale,
    );

    // Default width (space glyph or 500)
    let default_width = face
        .glyph_index(' ')
        .and_then(|gid| face.glyph_hor_advance(gid))
        .map(|adv| adv as f32 * scale)
        .unwrap_or(500.0);

    // 1. Write compressed font stream
    let compressed = compress_data(&ef.data)?;
    let mut stream = pdf.stream(ef.font_stream_ref, &compressed);
    stream.filter(Filter::FlateDecode);
    stream.pair(Name(b"Length1"), ef.data.len() as i32);
    stream.finish();

    // 2. Write FontDescriptor
    // Use filter + find_map so we skip platform records where to_string() returns None
    // (e.g. Macintosh platform), and reach the Windows/Unicode record that decodes properly.
    let base_font_name = face
        .names()
        .into_iter()
        .filter(|n| n.name_id == ttf_parser::name_id::POST_SCRIPT_NAME)
        .find_map(|n| n.to_string())
        .or_else(|| {
            // Fallback: try FULL_NAME (4), then FAMILY (1)
            face.names().into_iter()
                .filter(|n| n.name_id == ttf_parser::name_id::FULL_NAME)
                .find_map(|n| n.to_string())
        })
        .or_else(|| {
            face.names().into_iter()
                .filter(|n| n.name_id == ttf_parser::name_id::FAMILY)
                .find_map(|n| n.to_string())
        })
        .unwrap_or_else(|| "CustomFont".to_string());

    let ps_name = sanitize_ps_name(&base_font_name);

    let mut descriptor = pdf.font_descriptor(ef.descriptor_ref);
    descriptor.name(Name(ps_name.as_bytes()));
    descriptor.flags(fontdb_flags(&face));
    descriptor.bbox(font_bbox);
    descriptor.italic_angle(if face.is_italic() { -12.0 } else { 0.0 });
    descriptor.ascent(ascender);
    descriptor.descent(descender);
    descriptor.cap_height(cap_height);
    descriptor.stem_v((80.0 + ascender * 0.08) as f32);
    descriptor.font_file2(ef.font_stream_ref);
    descriptor.finish();

    // 3. Write CIDFont (CIDFontType2)
    let mut cid_font = pdf.cid_font(ef.cid_font_ref);
    cid_font.subtype(pdf_writer::types::CidFontType::Type2);
    cid_font.base_font(Name(ps_name.as_bytes()));
    cid_font.system_info(pdf_writer::types::SystemInfo {
        registry: Str(b"Adobe"),
        ordering: Str(b"Identity"),
        supplement: 0,
    });
    cid_font.font_descriptor(ef.descriptor_ref);
    cid_font.default_width(default_width);

    // Build per-glyph W (widths) array so PDF viewers space characters correctly.
    // Collect all glyph IDs that have a horizontal advance, then emit consecutive runs.
    {
        let num_glyphs = face.number_of_glyphs();
        let mut glyph_widths: Vec<(u16, f32)> = Vec::new();
        for gid in 0..num_glyphs {
            let glyph_id = ttf_parser::GlyphId(gid);
            if let Some(adv) = face.glyph_hor_advance(glyph_id) {
                let w = adv as f32 * scale;
                glyph_widths.push((gid, w));
            }
        }

        if !glyph_widths.is_empty() {
            let mut widths = cid_font.widths();
            // Group into consecutive runs for compact W array
            let mut run_start = glyph_widths[0].0;
            let mut run_widths: Vec<f32> = vec![glyph_widths[0].1];

            for i in 1..glyph_widths.len() {
                let (gid, w) = glyph_widths[i];
                if gid == run_start + run_widths.len() as u16 {
                    // Consecutive
                    run_widths.push(w);
                } else {
                    // Flush previous run
                    widths.consecutive(run_start, run_widths.iter().copied());
                    run_start = gid;
                    run_widths = vec![w];
                }
            }
            // Flush last run
            widths.consecutive(run_start, run_widths.iter().copied());
            widths.finish();
        }
    }

    // CIDToGIDMap Identity — CID values map directly to glyph IDs
    cid_font.cid_to_gid_map_predefined(Name(b"Identity"));
    cid_font.finish();

    // 4. Write ToUnicode CMap
    let tounicode_cmap = build_tounicode_cmap(&face);
    pdf.stream(ef.tounicode_ref, tounicode_cmap.as_bytes());

    // 5. Write Type0 font
    let mut type0 = pdf.type0_font(ef.type0_ref);
    type0.base_font(Name(ps_name.as_bytes()));
    type0.encoding_predefined(Name(b"Identity-H"));
    type0.descendant_font(ef.cid_font_ref);
    type0.to_unicode(ef.tounicode_ref);
    type0.finish();

    Ok(())
}

// ── Text encoding for CIDFont (glyph IDs as big-endian u16) ──

fn encode_for_cid_font(text: &str, font_data: &[u8]) -> Vec<u8> {
    let face = match ttf_parser::Face::parse(font_data, 0) {
        Ok(f) => f,
        Err(_) => return encode_winansi(text), // fallback
    };

    let mut bytes = Vec::with_capacity(text.len() * 2);
    for ch in text.chars() {
        let gid = face.glyph_index(ch).map(|g| g.0).unwrap_or(0);
        bytes.push((gid >> 8) as u8);
        bytes.push((gid & 0xFF) as u8);
    }
    bytes
}

// ── ToUnicode CMap builder ──

fn build_tounicode_cmap(face: &ttf_parser::Face) -> String {
    // Build a mapping of glyph ID → Unicode codepoint
    // by iterating over the cmap subtables
    let mut gid_to_unicode: HashMap<u16, char> = HashMap::new();

    if let Some(subtable) = face.tables().cmap {
        for sub in subtable.subtables {
            if !sub.is_unicode() {
                continue;
            }
            sub.codepoints(|cp| {
                if let Some(ch) = char::from_u32(cp) {
                    if let Some(gid) = face.glyph_index(ch) {
                        gid_to_unicode.entry(gid.0).or_insert(ch);
                    }
                }
            });
        }
    }

    let mut entries: Vec<(u16, char)> = gid_to_unicode.into_iter().collect();
    entries.sort_by_key(|&(gid, _)| gid);

    let mut cmap = String::new();
    cmap.push_str("/CIDInit /ProcSet findresource begin\n");
    cmap.push_str("12 dict begin\n");
    cmap.push_str("begincmap\n");
    cmap.push_str("/CIDSystemInfo\n");
    cmap.push_str("<< /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n");
    cmap.push_str("/CMapName /Adobe-Identity-UCS def\n");
    cmap.push_str("/CMapType 2 def\n");
    cmap.push_str("1 begincodespacerange\n");
    cmap.push_str("<0000> <FFFF>\n");
    cmap.push_str("endcodespacerange\n");

    // Write in chunks of 100 (PDF limit per beginbfchar block)
    for chunk in entries.chunks(100) {
        cmap.push_str(&format!("{} beginbfchar\n", chunk.len()));
        for &(gid, ch) in chunk {
            cmap.push_str(&format!("<{:04X}> <{:04X}>\n", gid, ch as u32));
        }
        cmap.push_str("endbfchar\n");
    }

    cmap.push_str("endcmap\n");
    cmap.push_str("CMapName currentdict /CMap defineresource pop\n");
    cmap.push_str("end\n");
    cmap.push_str("end\n");

    cmap
}

// ── Helpers ──

fn write_type1_font(pdf: &mut Pdf, font_ref: Ref, base_font: &str) {
    let mut font = pdf.type1_font(font_ref);
    font.base_font(Name(base_font.as_bytes()));
    font.encoding_predefined(Name(b"WinAnsiEncoding"));
    font.finish();
}

fn compress_data(data: &[u8]) -> ferropdf_core::Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(data)
        .map_err(|e| FerroError::PdfWrite(format!("Compression error: {}", e)))?;
    encoder
        .finish()
        .map_err(|e| FerroError::PdfWrite(format!("Compression finish error: {}", e)))
}

fn sanitize_ps_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '+')
        .collect()
}

fn fontdb_flags(face: &ttf_parser::Face) -> FontFlags {
    let mut flags = FontFlags::empty();
    if face.is_monospaced() {
        flags |= FontFlags::FIXED_PITCH;
    }
    // Identity-H encoding → always symbolic
    flags |= FontFlags::SYMBOLIC;
    if face.is_italic() {
        flags |= FontFlags::ITALIC;
    }
    flags
}

/// Measure text width using actual glyph advances from the embedded font.
/// Falls back to a rough estimate if font data is unavailable.
fn measure_text_width(
    text: &str,
    font_size: f32,
    font_data: Option<&[u8]>,
) -> f32 {
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

/// Encode text to WinAnsiEncoding for Type1 fallback fonts.
fn encode_winansi(text: &str) -> Vec<u8> {
    text.chars().map(unicode_to_winansi).collect()
}

fn unicode_to_winansi(c: char) -> u8 {
    let cp = c as u32;
    if cp < 0x80 {
        return cp as u8;
    }
    if (0xA0..=0xFF).contains(&cp) {
        return cp as u8;
    }
    match cp {
        0x20AC => 0x80, // €
        0x201A => 0x82, // ‚
        0x0192 => 0x83, // ƒ
        0x201E => 0x84, // „
        0x2026 => 0x85, // …
        0x2020 => 0x86, // †
        0x2021 => 0x87, // ‡
        0x02C6 => 0x88, // ˆ
        0x2030 => 0x89, // ‰
        0x0160 => 0x8A, // Š
        0x2039 => 0x8B, // ‹
        0x0152 => 0x8C, // Œ
        0x017D => 0x8E, // Ž
        0x2018 => 0x91, // '
        0x2019 => 0x92, // '
        0x201C => 0x93, // "
        0x201D => 0x94, // "
        0x2022 => 0x95, // •
        0x2013 => 0x96, // –
        0x2014 => 0x97, // —
        0x02DC => 0x98, // ˜
        0x2122 => 0x99, // ™
        0x0161 => 0x9A, // š
        0x203A => 0x9B, // ›
        0x0153 => 0x9C, // œ
        0x017E => 0x9E, // ž
        0x0178 => 0x9F, // Ÿ
        _ => b'?',
    }
}

/// Word-wrap text into lines that fit within `max_width`.
/// Uses glyph-level width measurements when font data is available.
fn wrap_text(
    text: &str,
    font_size: f32,
    max_width: f32,
    font_data: Option<&[u8]>,
) -> Vec<String> {
    // If container is very small or zero, don't wrap — just return as-is
    if max_width <= 0.0 {
        return vec![text.to_string()];
    }

    let mut lines: Vec<String> = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0.0f32;

    for word in text.split_inclusive(|c: char| c.is_whitespace()) {
        let word_width = measure_text_width(word, font_size, font_data);

        if current_line.is_empty() {
            // First word on the line — always add it
            current_line.push_str(word);
            current_width = word_width;
        } else if current_width + word_width <= max_width {
            // Word fits on current line
            current_line.push_str(word);
            current_width += word_width;
        } else {
            // Word doesn't fit — flush current line and start new one
            lines.push(current_line);
            current_line = word.to_string();
            current_width = word_width;
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    if lines.is_empty() {
        lines.push(text.to_string());
    }

    lines
}
