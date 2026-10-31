use crate::display_list::{DrawOp, PageDisplayList};
use crate::font_subsetter::{
    encode_for_cid_font, encode_shaped_glyphs, encode_winansi, load_font_data, subset_font,
    write_cid_font, EmbeddedFont, FontKey,
};
use crate::RenderOptions;
use ferropdf_core::{FerroError, PageConfig};
use flate2::write::ZlibEncoder;
use flate2::Compression;
use pdf_writer::types::LineCapStyle;
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect as PdfWriterRect, Ref, Str, TextStr};
use std::collections::HashMap;
use std::io::Write;

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
    warnings: &mut Vec<ferropdf_core::RenderWarning>,
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
    let mut font_raw_data: HashMap<String, (Vec<u8>, u32)> = HashMap::new(); // pdf_name → (raw data, face index)
                                                                             // pdf_name → original glyph ID → shaped cluster text (for ToUnicode)
    let mut font_glyph_text: HashMap<String, HashMap<u16, String>> = HashMap::new();
    let mut font_used_chars: HashMap<String, std::collections::HashSet<u16>> = HashMap::new(); // pdf_name → glyph IDs

    // fontdb::ID → pdf_name mapping for shaped glyphs (keyed by the exact font face)
    let mut fontdb_id_to_pdf_name: HashMap<fontdb::ID, String> = HashMap::new();

    // Single pass: discover fonts AND collect glyph IDs simultaneously
    for display_list in pages {
        for op in &display_list.ops {
            if let DrawOp::DrawText {
                text,
                font_family,
                bold,
                italic,
                shaped_glyphs,
                ..
            } = op
            {
                if !shaped_glyphs.is_empty() {
                    // ── Shaped text path: use font_id from cosmic-text glyphs ──
                    // Group glyphs by font_id (a single line may use multiple fonts
                    // due to cosmic-text's per-glyph font fallback).
                    for glyph in shaped_glyphs {
                        let fid = glyph.font_id;
                        // Register font by fontdb::ID on first encounter
                        if let std::collections::hash_map::Entry::Vacant(entry) =
                            fontdb_id_to_pdf_name.entry(fid)
                        {
                            if let Some(data) =
                                font_db.with_face_data(fid, |data, index| (data.to_vec(), index))
                            {
                                font_name_counter += 1;
                                let pdf_name = format!("F{}", font_name_counter);
                                font_raw_data.insert(pdf_name.clone(), data);
                                font_used_chars
                                    .insert(pdf_name.clone(), std::collections::HashSet::new());
                                entry.insert(pdf_name);
                            }
                        }
                        // Collect glyph IDs
                        if let Some(pdf_name) = fontdb_id_to_pdf_name.get(&fid) {
                            if let Some(used_gids) = font_used_chars.get_mut(pdf_name) {
                                used_gids.insert(0); // .notdef
                                used_gids.insert(glyph.glyph_id);
                            }
                            if !glyph.text.is_empty() {
                                font_glyph_text
                                    .entry(pdf_name.clone())
                                    .or_default()
                                    .entry(glyph.glyph_id)
                                    .or_insert_with(|| glyph.text.clone());
                            }
                        }
                    }
                } else {
                    // ── Unshaped text path: resolve font by family name ──
                    let family_name = font_family.first().cloned().unwrap_or_default();
                    let key = FontKey {
                        family: family_name.clone(),
                        bold: *bold,
                        italic: *italic,
                    };
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
                    if let Some(Some(pdf_name)) = font_cache.get(&key) {
                        if let Some(used_gids) = font_used_chars.get_mut(pdf_name) {
                            used_gids.insert(0); // .notdef
                            if let Some((raw_data, index)) = font_raw_data.get(pdf_name) {
                                if let Ok(face) = ttf_parser::Face::parse(raw_data, *index) {
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
        }
    }

    // Phase 3: Subset fonts and build EmbeddedFont entries
    let pdf_names: Vec<String> = font_raw_data.keys().cloned().collect();
    for pdf_name in pdf_names {
        let (raw_data, face_index) = font_raw_data.remove(&pdf_name).unwrap();
        let used_gids = font_used_chars.remove(&pdf_name).unwrap();
        let glyph_text = font_glyph_text.remove(&pdf_name).unwrap_or_default();

        let type0_ref = next_ref();
        let cid_font_ref = next_ref();
        let descriptor_ref = next_ref();
        let font_stream_ref = next_ref();
        let tounicode_ref = next_ref();

        // Subset the font to only include used glyphs
        let (subset_data, gid_remapping) = subset_font(&raw_data, face_index, &used_gids);

        embedded_fonts.push(EmbeddedFont {
            original_data: raw_data,
            face_index,
            glyph_text,
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

    // ── Collect opacity ExtGState objects ──
    // Scan display lists for SetOpacity and DrawBoxShadow ops and create one ExtGState per unique alpha.
    let mut opacity_states: HashMap<u32, (Ref, String)> = HashMap::new(); // key = (alpha * 1000) as u32
    let mut gs_counter = 0u32;
    let register_alpha = |alpha: f32,
                          opacity_states: &mut HashMap<u32, (Ref, String)>,
                          next_ref: &mut dyn FnMut() -> Ref,
                          gs_counter: &mut u32| {
        let key = (alpha * 1000.0) as u32;
        opacity_states.entry(key).or_insert_with(|| {
            *gs_counter += 1;
            (next_ref(), format!("GS{}", *gs_counter))
        });
    };
    for display_list in pages {
        for op in &display_list.ops {
            match op {
                DrawOp::SetOpacity(alpha) => {
                    register_alpha(*alpha, &mut opacity_states, &mut next_ref, &mut gs_counter);
                }
                DrawOp::DrawBoxShadow { shadow, .. } if shadow.color.a < 1.0 - f32::EPSILON => {
                    // Register alpha for shadow color
                    if shadow.blur_radius < 0.5 {
                        register_alpha(
                            shadow.color.a,
                            &mut opacity_states,
                            &mut next_ref,
                            &mut gs_counter,
                        );
                    } else {
                        let steps = ((shadow.blur_radius / 2.0) as usize).clamp(3, 8);
                        let step_alpha = shadow.color.a / steps as f32;
                        register_alpha(
                            step_alpha,
                            &mut opacity_states,
                            &mut next_ref,
                            &mut gs_counter,
                        );
                    }
                }
                _ => {}
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

    // ── Load images ──
    let mut image_map: HashMap<String, LoadedImage> = HashMap::new();
    let mut img_counter = 0u32;
    for display_list in pages {
        for op in &display_list.ops {
            if let DrawOp::DrawImage { src, .. } = op {
                if !image_map.contains_key(src) {
                    match load_image(src, opts.base_url.as_deref()) {
                        Ok(kind) => {
                            let pdf_name = format!("Im{img_counter}");
                            img_counter += 1;
                            let (pdf_ref, kind) = match kind {
                                ImageKind::Svg { chunk, root } => {
                                    // Renumber svg2pdf's objects into our ref space.
                                    let mut map = HashMap::new();
                                    let chunk = chunk.renumber(|old| {
                                        *map.entry(old).or_insert_with(&mut next_ref)
                                    });
                                    (map[&root], ImageKind::Svg { chunk, root })
                                }
                                raster => (next_ref(), raster),
                            };
                            let smask_ref = match &kind {
                                ImageKind::Raster { alpha: Some(_), .. } => Some(next_ref()),
                                _ => None,
                            };
                            image_map.insert(
                                src.clone(),
                                LoadedImage {
                                    kind,
                                    smask_ref,
                                    pdf_ref,
                                    pdf_name,
                                },
                            );
                        }
                        Err(reason) => {
                            log::warn!("Failed to load image {}: {}", src, reason);
                            warnings.push(ferropdf_core::RenderWarning::ImageLoadFailed {
                                src: src.clone(),
                                reason,
                            });
                        }
                    }
                }
            }
        }
    }

    // ── Gradients: shadings (and alpha soft masks), per page in op order ──
    let mut page_gradients: Vec<Vec<crate::gradient::GradientResources>> = Vec::new();
    let mut gradient_index = 0usize;
    for display_list in pages {
        let mut list = Vec::new();
        for op in &display_list.ops {
            if let DrawOp::FillGradient { rect, gradient, .. } = op {
                list.push(crate::gradient::write_gradient(
                    &mut pdf,
                    &mut next_ref,
                    gradient,
                    *rect,
                    gradient_index,
                    page_h,
                ));
                gradient_index += 1;
            }
        }
        page_gradients.push(list);
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
        let gradients = &page_gradients[i];
        let has_masks = gradients.iter().any(|g| g.mask_state.is_some());
        if !opacity_states.is_empty() || has_masks {
            let mut ext_g_states = resources.ext_g_states();
            for (gs_ref, gs_name) in opacity_states.values() {
                ext_g_states.pair(Name(gs_name.as_bytes()), *gs_ref);
            }
            for (gs_name, gs_ref) in gradients.iter().filter_map(|g| g.mask_state.as_ref()) {
                ext_g_states.pair(Name(gs_name.as_bytes()), *gs_ref);
            }
            ext_g_states.finish();
        }
        if !gradients.is_empty() {
            let mut shadings = resources.shadings();
            for g in gradients {
                shadings.pair(Name(g.shading_name.as_bytes()), g.shading_ref);
            }
            shadings.finish();
        }
        resources.finish();

        page.contents(content_ref);
        page.finish();

        // Content stream
        let mut content = Content::new();
        let mut next_gradient = gradients.iter();

        for op in &display_list.ops {
            match op {
                DrawOp::FillRect {
                    rect,
                    color,
                    border_radius,
                } if !color.is_transparent() => {
                    content.set_fill_rgb(color.r, color.g, color.b);
                    let pr = to_pdf_rect(rect.x, rect.y, rect.width, rect.height, page_h);
                    if !border_radius.is_zero() {
                        rounded_rect_path(
                            &mut content,
                            pr.x,
                            pr.y,
                            pr.width,
                            pr.height,
                            *border_radius,
                        );
                    } else {
                        content.rect(pr.x, pr.y, pr.width, pr.height);
                    }
                    content.fill_nonzero();
                }
                DrawOp::FillRect { .. } => {}
                DrawOp::FillGradient {
                    rect,
                    border_radius,
                    gradient,
                } => {
                    let Some(res) = next_gradient.next() else {
                        continue;
                    };
                    content.save_state();
                    if let Some((gs_name, _)) = &res.mask_state {
                        content.set_parameters(Name(gs_name.as_bytes()));
                    }
                    let pr = to_pdf_rect(rect.x, rect.y, rect.width, rect.height, page_h);
                    rounded_rect_path(
                        &mut content,
                        pr.x,
                        pr.y,
                        pr.width,
                        pr.height,
                        *border_radius,
                    );
                    content.clip_nonzero();
                    content.end_path();
                    crate::gradient::paint_gradient(&mut content, res, gradient, page_h);
                    content.restore_state();
                }
                DrawOp::StrokeRoundedRect {
                    rect,
                    color,
                    width,
                    style,
                    border_radius,
                } => {
                    content.set_stroke_rgb(color.r, color.g, color.b);
                    let pr = to_pdf_rect(rect.x, rect.y, rect.width, rect.height, page_h);
                    let paths: Vec<(f32, f32)> = if *style == ferropdf_core::BorderStyle::Double {
                        // Two strokes of w/3: outer and inner thirds of the border.
                        vec![(width / 3.0, width / 6.0), (width / 3.0, width * 5.0 / 6.0)]
                    } else {
                        vec![(*width, width / 2.0)]
                    };
                    for (line_w, inset) in paths {
                        content.set_line_width(line_w);
                        set_border_dash(&mut content, *style, *width);
                        let radii = border_radius.inset([inset; 4]);
                        rounded_rect_path(
                            &mut content,
                            pr.x + inset,
                            pr.y + inset,
                            (pr.width - 2.0 * inset).max(0.0),
                            (pr.height - 2.0 * inset).max(0.0),
                            radii,
                        );
                        content.stroke();
                    }
                    reset_border_dash(&mut content);
                }
                DrawOp::StrokeRect {
                    rect,
                    color,
                    width,
                    style,
                } if *style == ferropdf_core::BorderStyle::Double && *width >= 3.0 => {
                    // Two parallel lines of w/3, w/3 apart, centred on the edge.
                    content.set_stroke_rgb(color.r, color.g, color.b);
                    content.set_line_width(width / 3.0);
                    for off in [-width / 3.0, width / 3.0] {
                        if rect.height < 0.01 {
                            let py = y_to_pdf(rect.y, page_h) + off;
                            content.move_to(rect.x, py);
                            content.line_to(rect.x + rect.width, py);
                        } else {
                            let px = rect.x + off;
                            content.move_to(px, y_to_pdf(rect.y, page_h));
                            content.line_to(px, y_to_pdf(rect.y + rect.height, page_h));
                        }
                        content.stroke();
                    }
                }
                DrawOp::StrokeRect {
                    rect,
                    color,
                    width,
                    style,
                } => {
                    content.set_stroke_rgb(color.r, color.g, color.b);
                    content.set_line_width(*width);
                    set_border_dash(&mut content, *style, *width);
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
                    reset_border_dash(&mut content);
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
                    shaped_glyphs,
                } => {
                    let font_size_pt = *font_size;
                    let line_text = text.trim();
                    if line_text.is_empty() {
                        continue;
                    }

                    // Convert to PDF coordinates (Y-axis flip only)
                    let pdf_y = y_to_pdf(*y, page_h);
                    content.set_fill_rgb(color.r, color.g, color.b);

                    if !shaped_glyphs.is_empty() {
                        // ── Shaped text path: use font_id from glyphs ──
                        // Sort glyphs by x position (left-to-right visual order).
                        // This handles RTL text: cosmic-text gives glyphs in logical
                        // order with decreasing x, sorting gives visual order with
                        // increasing x, which is what PDF show() expects.
                        let mut sorted_glyphs = shaped_glyphs.clone();
                        sorted_glyphs.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());

                        let base_x = *x;
                        let min_x = sorted_glyphs.first().map(|g| g.x).unwrap_or(0.0);

                        // Natural advances come from the embedded font's hmtx; the
                        // PDF viewer advances by these. Any difference with the
                        // shaped positions (kerning, justification) is emitted as a
                        // TJ adjustment so glyphs land exactly where cosmic-text
                        // placed them.
                        let mut faces: HashMap<fontdb::ID, (ttf_parser::Face, &EmbeddedFont)> =
                            HashMap::new();
                        for g in &sorted_glyphs {
                            if faces.contains_key(&g.font_id) {
                                continue;
                            }
                            let Some(pdf_name) = fontdb_id_to_pdf_name.get(&g.font_id) else {
                                continue;
                            };
                            let Some(ef) = embedded_fonts.iter().find(|f| &f.pdf_name == pdf_name)
                            else {
                                continue;
                            };
                            if let Some(face) = ef.face() {
                                faces.insert(g.font_id, (face, ef));
                            }
                        }

                        content.begin_text();
                        content.next_line(base_x + min_x, pdf_y);

                        let mut i = 0;
                        while i < sorted_glyphs.len() {
                            let fid = sorted_glyphs[i].font_id;
                            let mut j = i;
                            while j < sorted_glyphs.len() && sorted_glyphs[j].font_id == fid {
                                j += 1;
                            }
                            let Some((face, ef)) = faces.get(&fid) else {
                                i = j;
                                continue;
                            };
                            let scale = font_size_pt / face.units_per_em() as f32;
                            content.set_font(Name(ef.pdf_name.as_bytes()), font_size_pt);
                            let mut tj = content.show_positioned();
                            let mut items = tj.items();
                            let mut pending: Vec<u16> = Vec::new();
                            for k in i..j {
                                let g = &sorted_glyphs[k];
                                pending.push(g.glyph_id);
                                let Some(next) = sorted_glyphs.get(k + 1) else {
                                    continue;
                                };
                                let natural = face
                                    .glyph_hor_advance(ttf_parser::GlyphId(g.glyph_id))
                                    .map(|a| a as f32 * scale)
                                    .unwrap_or(g.advance);
                                // TJ amounts are in thousandths of text space,
                                // subtracted from the pen position.
                                let adjust = -(next.x - g.x - natural) * 1000.0 / font_size_pt;
                                if adjust.abs() > 0.5 {
                                    let encoded =
                                        encode_shaped_glyphs(&pending, ef.gid_remapping.as_ref());
                                    items.show(Str(&encoded));
                                    items.adjust(adjust);
                                    pending.clear();
                                }
                            }
                            if !pending.is_empty() {
                                let encoded =
                                    encode_shaped_glyphs(&pending, ef.gid_remapping.as_ref());
                                items.show(Str(&encoded));
                            }
                            drop(items);
                            drop(tj);
                            i = j;
                        }
                        content.end_text();
                    } else {
                        // ── Unshaped text path: resolve font by family name ──
                        let family_name = font_family.first().cloned().unwrap_or_default();
                        let key = FontKey {
                            family: family_name,
                            bold: *bold,
                            italic: *italic,
                        };
                        let ef_option =
                            font_cache
                                .get(&key)
                                .and_then(|v| v.as_ref())
                                .and_then(|pdf_name| {
                                    embedded_fonts.iter().find(|f| &f.pdf_name == pdf_name)
                                });
                        let line_width_pt = measure_text_width(line_text, *font_size, ef_option);
                        let aligned_x = match text_align {
                            ferropdf_core::TextAlign::Right => *x + container_width - line_width_pt,
                            ferropdf_core::TextAlign::Center => {
                                *x + (container_width - line_width_pt) / 2.0
                            }
                            _ => *x,
                        };

                        content.begin_text();
                        match ef_option {
                            Some(ef) => {
                                content.set_font(Name(ef.pdf_name.as_bytes()), font_size_pt);
                                content.next_line(aligned_x, pdf_y);
                                let encoded = encode_for_cid_font(
                                    line_text,
                                    &ef.original_data,
                                    ef.face_index,
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
                                content.next_line(aligned_x, pdf_y);
                                content.show(Str(&encode_winansi(line_text)));
                            }
                        }
                        content.end_text();
                    }
                }
                DrawOp::DrawBoxShadow {
                    rect,
                    shadow,
                    border_radius,
                } => {
                    // Approximate box-shadow using multiple semi-transparent filled rects.
                    // For blur, we draw several layers with decreasing opacity.
                    // An outer shadow is only visible outside the border box
                    // (CSS Backgrounds 3 §7.1.1): clip the box out (even-odd)
                    // so it never shows through a transparent background.
                    content.save_state();
                    content.rect(-10_000.0, -10_000.0, page_w + 20_000.0, page_h + 20_000.0);
                    let br = to_pdf_rect(rect.x, rect.y, rect.width, rect.height, page_h);
                    rounded_rect_path(
                        &mut content,
                        br.x,
                        br.y,
                        br.width,
                        br.height,
                        *border_radius,
                    );
                    content.clip_even_odd();
                    content.end_path();
                    let spread = shadow.spread;
                    let blur = shadow.blur_radius;
                    let sx = rect.x + shadow.offset_x - spread;
                    let sy = rect.y + shadow.offset_y - spread;
                    let sw = rect.width + spread * 2.0;
                    let sh = rect.height + spread * 2.0;

                    if blur < 0.5 {
                        // No blur: single solid rect
                        let pr = to_pdf_rect(sx, sy, sw, sh, page_h);
                        content.save_state();
                        content.set_fill_rgb(shadow.color.r, shadow.color.g, shadow.color.b);
                        // Apply alpha via inline graphics state
                        if shadow.color.a < 1.0 - f32::EPSILON {
                            let key = (shadow.color.a * 1000.0) as u32;
                            if let Some((_ref, gs_name)) = opacity_states.get(&key) {
                                content.set_parameters(Name(gs_name.as_bytes()));
                            }
                        }
                        rounded_rect_path(
                            &mut content,
                            pr.x,
                            pr.y,
                            pr.width,
                            pr.height,
                            border_radius.grow(spread),
                        );
                        content.fill_nonzero();
                        content.restore_state();
                    } else {
                        // Approximate gaussian blur with layered rects
                        let steps = ((blur / 2.0) as usize).clamp(3, 8);
                        let step_alpha = shadow.color.a / steps as f32;
                        content.save_state();
                        for i in 0..steps {
                            let expand = blur * (i as f32 + 1.0) / steps as f32;
                            let lx = sx - expand;
                            let ly = sy - expand;
                            let lw = sw + expand * 2.0;
                            let lh = sh + expand * 2.0;
                            let pr = to_pdf_rect(lx, ly, lw, lh, page_h);

                            let layer_alpha = step_alpha;
                            let key = (layer_alpha * 1000.0) as u32;
                            if let Some((_ref, gs_name)) = opacity_states.get(&key) {
                                content.set_parameters(Name(gs_name.as_bytes()));
                            }
                            content.set_fill_rgb(shadow.color.r, shadow.color.g, shadow.color.b);
                            let radii = border_radius.grow(spread + expand);
                            rounded_rect_path(&mut content, pr.x, pr.y, pr.width, pr.height, radii);
                            content.fill_nonzero();
                        }
                        content.restore_state();
                    }
                    content.restore_state();
                }
                DrawOp::SetOpacity(alpha) => {
                    let key = (*alpha * 1000.0) as u32;
                    if let Some((_ref, gs_name)) = opacity_states.get(&key) {
                        content.set_parameters(Name(gs_name.as_bytes()));
                    }
                }
                DrawOp::ClipRect {
                    rect,
                    border_radius,
                } => {
                    let pr = to_pdf_rect(rect.x, rect.y, rect.width, rect.height, page_h);
                    rounded_rect_path(
                        &mut content,
                        pr.x,
                        pr.y,
                        pr.width,
                        pr.height,
                        *border_radius,
                    );
                    content.clip_nonzero();
                    content.end_path();
                }
                DrawOp::Save => {
                    content.save_state();
                }
                DrawOp::Transform([a, b, c, d, e, f]) => {
                    // Conjugate by the y flip (x, y) → (x, H − y) to move the
                    // page-space matrix into PDF space.
                    let h = page_h;
                    content.transform([*a, -*b, -*c, *d, *c * h + *e, h - *d * h - *f]);
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
    let deflate = |data: &[u8]| -> ferropdf_core::Result<Vec<u8>> {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
        encoder
            .write_all(data)
            .map_err(|e| FerroError::Image(format!("Image compression error: {e}")))?;
        encoder
            .finish()
            .map_err(|e| FerroError::Image(format!("Image compression finish error: {e}")))
    };
    for img in image_map.values() {
        match &img.kind {
            ImageKind::Svg { chunk, .. } => pdf.extend(chunk),
            ImageKind::Raster {
                rgb,
                alpha,
                width,
                height,
            } => {
                let compressed = deflate(rgb)?;
                let mut xobj = pdf.image_xobject(img.pdf_ref, &compressed);
                xobj.filter(Filter::FlateDecode);
                xobj.width(*width as i32);
                xobj.height(*height as i32);
                xobj.color_space().device_rgb();
                xobj.bits_per_component(8);
                if let Some(mask_ref) = img.smask_ref {
                    xobj.s_mask(mask_ref);
                }
                xobj.finish();
                if let (Some(mask_ref), Some(alpha)) = (img.smask_ref, alpha) {
                    let compressed = deflate(alpha)?;
                    let mut mask = pdf.image_xobject(mask_ref, &compressed);
                    mask.filter(Filter::FlateDecode);
                    mask.width(*width as i32);
                    mask.height(*height as i32);
                    mask.color_space().device_gray();
                    mask.bits_per_component(8);
                    mask.finish();
                }
            }
        }
    }

    // Write ExtGState objects for opacity
    for (&key, &(gs_ref, ref _gs_name)) in &opacity_states {
        let alpha = key as f32 / 1000.0;
        let mut gs = pdf.ext_graphics(gs_ref);
        gs.non_stroking_alpha(alpha);
        gs.stroking_alpha(alpha);
        gs.finish();
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
fn measure_text_width(text: &str, font_size: f32, font: Option<&EmbeddedFont>) -> f32 {
    if let Some(font) = font {
        if let Some(face) = font.face() {
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
    kind: ImageKind,
    /// Soft mask (alpha channel) object, for raster images with transparency.
    smask_ref: Option<Ref>,
    pdf_ref: Ref,
    pdf_name: String,
}

#[derive(Debug)]
enum ImageKind {
    Raster {
        rgb: Vec<u8>,
        /// 8-bit alpha, only when the image has non-opaque pixels.
        alpha: Option<Vec<u8>>,
        width: u32,
        height: u32,
    },
    /// Vector SVG as a Form XObject (unit square) produced by svg2pdf.
    Svg { chunk: pdf_writer::Chunk, root: Ref },
}

/// Load an image from a source string (data URI or sandboxed file path).
///
/// Returns `Err(reason)` rather than `FerroError` because image failures are
/// non-fatal — the caller turns the reason into a `RenderWarning::ImageLoadFailed`
/// and the render continues without the image.
fn load_image(src: &str, base_url: Option<&str>) -> Result<ImageKind, String> {
    let data = crate::images::source_bytes(src, base_url)?;

    if crate::images::is_svg(src, &data) {
        let tree = crate::images::parse_svg(&data)?;
        let (chunk, root) = svg2pdf::to_chunk(&tree, svg2pdf::ConversionOptions::default());
        return Ok(ImageKind::Svg { chunk, root });
    }

    let img = image::load_from_memory(&data).map_err(|e| format!("cannot decode image: {}", e))?;
    let (width, height) = (img.width(), img.height());
    let alpha = if img.color().has_alpha() {
        let rgba = img.to_rgba8();
        let a: Vec<u8> = rgba.pixels().map(|p| p.0[3]).collect();
        a.iter().any(|&v| v != 255).then_some(a)
    } else {
        None
    };
    Ok(ImageKind::Raster {
        rgb: img.to_rgb8().into_raw(),
        alpha,
        width,
        height,
    })
}

/// Append a rounded-rectangle path (PDF coordinates, origin bottom-left).
///
/// `radii` = [top-left, top-right, bottom-right, bottom-left] in CSS order.
/// Radii are scaled down together when adjacent ones exceed a side
/// (CSS Backgrounds 3 §5.5), which is what turns `border-radius: 9999px`
/// into a pill rather than a broken shape.
/// Append a rectangle with elliptical corners to the path (PDF space:
/// `(x, y)` is the bottom-left corner). Radii are scaled down to fit.
pub(crate) fn rounded_rect_path(
    content: &mut Content,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radii: ferropdf_core::Radii,
) {
    // A corner with a zero radius on either axis is square.
    let corner = |(rx, ry): (f32, f32)| {
        if rx <= 0.0 || ry <= 0.0 {
            (0.0, 0.0)
        } else {
            (rx, ry)
        }
    };
    let ferropdf_core::Radii([tl, tr, br, bl]) = radii.clamped(w, h);
    let (tl, tr, br, bl) = (corner(tl), corner(tr), corner(br), corner(bl));
    // Bézier control-point distance for a quarter ellipse.
    const K: f32 = 0.552_284_8;
    let top = y + h;
    let right = x + w;
    content.move_to(x + tl.0, top);
    content.line_to(right - tr.0, top);
    if tr.0 > 0.0 {
        content.cubic_to(
            right - tr.0 + tr.0 * K,
            top,
            right,
            top - tr.1 + tr.1 * K,
            right,
            top - tr.1,
        );
    }
    content.line_to(right, y + br.1);
    if br.0 > 0.0 {
        content.cubic_to(
            right,
            y + br.1 - br.1 * K,
            right - br.0 + br.0 * K,
            y,
            right - br.0,
            y,
        );
    }
    content.line_to(x + bl.0, y);
    if bl.0 > 0.0 {
        content.cubic_to(x + bl.0 - bl.0 * K, y, x, y + bl.1 - bl.1 * K, x, y + bl.1);
    }
    content.line_to(x, top - tl.1);
    if tl.0 > 0.0 {
        content.cubic_to(
            x,
            top - tl.1 + tl.1 * K,
            x + tl.0 - tl.0 * K,
            top,
            x + tl.0,
            top,
        );
    }
    content.close_path();
}

/// Dash pattern for `dashed` / `dotted` borders (approximating browsers:
/// dashes 3×width long, dots as round caps spaced 2×width).
fn set_border_dash(content: &mut Content, style: ferropdf_core::BorderStyle, width: f32) {
    match style {
        ferropdf_core::BorderStyle::Dashed => {
            content.set_dash_pattern([3.0 * width, 3.0 * width], 0.0);
        }
        ferropdf_core::BorderStyle::Dotted => {
            content.set_line_cap(LineCapStyle::RoundCap);
            content.set_dash_pattern([0.0, 2.0 * width], 0.0);
        }
        _ => {}
    }
}

fn reset_border_dash(content: &mut Content) {
    content.set_dash_pattern([], 0.0);
    content.set_line_cap(LineCapStyle::ButtCap);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1×1 transparent PNG, base64.
    const ONE_PX_PNG_B64: &str =
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

    #[test]
    fn load_image_decodes_data_uri() {
        let src = format!("data:image/png;base64,{}", ONE_PX_PNG_B64);
        let ImageKind::Raster {
            rgb, width, height, ..
        } = load_image(&src, None).expect("data URI should decode")
        else {
            panic!("expected a raster image")
        };
        assert_eq!((width, height), (1, 1));
        assert_eq!(rgb.len(), 3); // 1 pixel × 3 bytes RGB
    }

    #[test]
    fn load_image_rejects_local_path_without_base_url() {
        // Even if the file is real, no base_url means the sandbox refuses it.
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("logo.png");
        std::fs::write(
            &png,
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, ONE_PX_PNG_B64)
                .unwrap(),
        )
        .unwrap();
        let err = load_image(png.to_str().unwrap(), None).unwrap_err();
        assert!(err.contains("base_url"), "got {}", err);
    }

    #[test]
    fn load_image_loads_relative_path_under_base_url() {
        let dir = tempfile::tempdir().unwrap();
        let png_bytes =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, ONE_PX_PNG_B64)
                .unwrap();
        std::fs::write(dir.path().join("logo.png"), &png_bytes).unwrap();
        let kind = load_image("logo.png", Some(dir.path().to_str().unwrap())).expect("should load");
        assert!(matches!(
            kind,
            ImageKind::Raster {
                width: 1,
                height: 1,
                ..
            }
        ));
    }

    #[test]
    fn load_image_keeps_alpha_channel() {
        // 2×1 RGBA: one opaque red, one fully transparent pixel.
        let mut png = Vec::new();
        image::RgbaImage::from_raw(2, 1, vec![255, 0, 0, 255, 0, 0, 0, 0])
            .unwrap()
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let src = format!(
            "data:image/png;base64,{}",
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &png)
        );
        let ImageKind::Raster { alpha, .. } = load_image(&src, None).unwrap() else {
            panic!("raster expected")
        };
        assert_eq!(alpha, Some(vec![255, 0]));
    }

    #[test]
    fn load_image_converts_svg_to_vector() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="#00f"/></svg>"##;
        let src = format!(
            "data:image/svg+xml;base64,{}",
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, svg)
        );
        assert!(matches!(
            load_image(&src, None).unwrap(),
            ImageKind::Svg { .. }
        ));
    }

    #[test]
    fn load_image_rejects_invalid_data_uri() {
        let err = load_image("data:image/png;base64,$$$invalid$$$", None).unwrap_err();
        assert!(
            err.contains("base64") || err.contains("decode"),
            "got {}",
            err
        );
    }

    #[test]
    fn load_image_rejects_data_uri_without_comma() {
        let err = load_image("data:malformed", None).unwrap_err();
        assert!(err.contains("comma"), "got {}", err);
    }

    #[test]
    fn load_image_rejects_unparseable_bytes() {
        // Valid base64 but the decoded payload is not a recognized image.
        let src = "data:image/png;base64,SGVsbG8gd29ybGQ="; // "Hello world"
        let err = load_image(src, None).unwrap_err();
        assert!(
            err.contains("decode") || err.contains("Format"),
            "got {}",
            err
        );
    }

    #[test]
    fn load_image_rejects_http_urls() {
        let err = load_image("http://example.com/x.png", None).unwrap_err();
        assert!(err.contains("HTTP") || err.contains("http"), "got {}", err);
    }

    #[test]
    fn y_to_pdf_inverts_y_axis() {
        // HTML's top-down y becomes PDF's bottom-up y.
        assert_eq!(y_to_pdf(0.0, 800.0), 800.0);
        assert_eq!(y_to_pdf(800.0, 800.0), 0.0);
        assert_eq!(y_to_pdf(100.0, 800.0), 700.0);
    }
}
