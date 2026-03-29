use ferropdf_core::FerroError;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use pdf_writer::types::FontFlags;
use pdf_writer::{Filter, Finish, Name, Pdf, Rect as PdfWriterRect, Ref, Str};
use std::collections::HashMap;
use std::io::Write;

/// A resolved font ready to be embedded in the PDF.
pub(crate) struct EmbeddedFont {
    /// Original raw TTF/OTF bytes (for char→glyph lookups)
    pub original_data: Vec<u8>,
    /// Subsetted TTF/OTF bytes (for embedding — much smaller)
    pub subset_data: Vec<u8>,
    /// Mapping from original glyph IDs to subsetted glyph IDs (if subsetted)
    pub gid_remapping: Option<HashMap<u16, u16>>,
    /// PDF object refs
    pub type0_ref: Ref,
    pub font_stream_ref: Ref,
    pub descriptor_ref: Ref,
    pub cid_font_ref: Ref,
    pub tounicode_ref: Ref,
    /// PDF resource name, e.g. "F4"
    pub pdf_name: String,
}

/// Key to deduplicate fonts
#[derive(Hash, Eq, PartialEq, Clone)]
pub(crate) struct FontKey {
    pub family: String,
    pub bold: bool,
    pub italic: bool,
}

// ── Font resolution via fontdb ──

pub(crate) fn load_font_data(
    db: &fontdb::Database,
    family: &str,
    bold: bool,
    italic: bool,
) -> Option<Vec<u8>> {
    if family.is_empty() {
        return None;
    }

    let query = fontdb::Query {
        families: &[fontdb::Family::Name(family), fontdb::Family::SansSerif],
        weight: if bold {
            fontdb::Weight(700)
        } else {
            fontdb::Weight(400)
        },
        style: if italic {
            fontdb::Style::Italic
        } else {
            fontdb::Style::Normal
        },
        ..Default::default()
    };

    let id = db.query(&query)?;
    db.with_face_data(id, |data, _| data.to_vec())
}

// ── CIDFont embedding ──

pub(crate) fn write_cid_font(pdf: &mut Pdf, ef: &EmbeddedFont) -> ferropdf_core::Result<()> {
    // Parse the ORIGINAL font for metrics (ascender, bbox, etc.)
    let face = ttf_parser::Face::parse(&ef.original_data, 0)
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

    // 1. Write compressed font stream (subsetted — much smaller)
    let compressed = compress_data(&ef.subset_data)?;
    let mut stream = pdf.stream(ef.font_stream_ref, &compressed);
    stream.filter(Filter::FlateDecode);
    stream.pair(Name(b"Length1"), ef.subset_data.len() as i32);
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
            face.names()
                .into_iter()
                .filter(|n| n.name_id == ttf_parser::name_id::FULL_NAME)
                .find_map(|n| n.to_string())
        })
        .or_else(|| {
            face.names()
                .into_iter()
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
    descriptor.stem_v(80.0 + ascender * 0.08);
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
    // When subsetted, iterate only the subsetted font's glyphs (much fewer).
    {
        let width_face = if ef.gid_remapping.is_some() {
            // Parse the subsetted font for width info
            ttf_parser::Face::parse(&ef.subset_data, 0).ok()
        } else {
            None
        };
        let wf = width_face.as_ref().unwrap_or(&face);
        let num_glyphs = wf.number_of_glyphs();
        let wf_units_per_em = wf.units_per_em() as f32;
        let wf_scale = 1000.0 / wf_units_per_em;

        let mut glyph_widths: Vec<(u16, f32)> = Vec::new();
        for gid in 0..num_glyphs {
            let glyph_id = ttf_parser::GlyphId(gid);
            if let Some(adv) = wf.glyph_hor_advance(glyph_id) {
                let w = adv as f32 * wf_scale;
                glyph_widths.push((gid, w));
            }
        }

        if !glyph_widths.is_empty() {
            let mut widths = cid_font.widths();
            // Group into consecutive runs for compact W array
            let mut run_start = glyph_widths[0].0;
            let mut run_widths: Vec<f32> = vec![glyph_widths[0].1];

            for &(gid, w) in &glyph_widths[1..] {
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
    let tounicode_cmap = build_tounicode_cmap(&face, ef.gid_remapping.as_ref());
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

pub(crate) fn encode_for_cid_font(
    text: &str,
    font_data: &[u8],
    gid_remapping: Option<&HashMap<u16, u16>>,
) -> Vec<u8> {
    let face = match ttf_parser::Face::parse(font_data, 0) {
        Ok(f) => f,
        Err(_) => return encode_winansi(text), // fallback
    };

    let mut bytes = Vec::with_capacity(text.len() * 2);
    for ch in text.chars() {
        let gid = face.glyph_index(ch).map(|g| g.0).unwrap_or(0);
        // If font was subsetted, map original GID → subsetted GID
        let mapped_gid = match gid_remapping {
            Some(map) => *map.get(&gid).unwrap_or(&0),
            None => gid,
        };
        bytes.push((mapped_gid >> 8) as u8);
        bytes.push((mapped_gid & 0xFF) as u8);
    }
    bytes
}

/// Encode pre-shaped glyph IDs as big-endian u16 bytes for CIDFont embedding.
/// Used when cosmic-text has already shaped the text (essential for Arabic ligatures).
pub(crate) fn encode_shaped_glyphs(
    glyph_ids: &[u16],
    gid_remapping: Option<&HashMap<u16, u16>>,
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(glyph_ids.len() * 2);
    for &gid in glyph_ids {
        let mapped_gid = match gid_remapping {
            Some(map) => *map.get(&gid).unwrap_or(&0),
            None => gid,
        };
        bytes.push((mapped_gid >> 8) as u8);
        bytes.push((mapped_gid & 0xFF) as u8);
    }
    bytes
}

// ── ToUnicode CMap builder ──

fn build_tounicode_cmap(
    face: &ttf_parser::Face,
    gid_remapping: Option<&HashMap<u16, u16>>,
) -> String {
    // Build a mapping of (remapped) glyph ID → Unicode codepoint
    let mut gid_to_unicode: HashMap<u16, char> = HashMap::new();

    if let Some(subtable) = face.tables().cmap {
        for sub in subtable.subtables {
            if !sub.is_unicode() {
                continue;
            }
            sub.codepoints(|cp| {
                if let Some(ch) = char::from_u32(cp) {
                    if let Some(gid) = face.glyph_index(ch) {
                        // Use the remapped GID if font was subsetted
                        let mapped = match gid_remapping {
                            Some(map) => match map.get(&gid.0) {
                                Some(&new_gid) => new_gid,
                                None => return, // glyph not in subset, skip
                            },
                            None => gid.0,
                        };
                        gid_to_unicode.entry(mapped).or_insert(ch);
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

fn compress_data(data: &[u8]) -> ferropdf_core::Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
    encoder
        .write_all(data)
        .map_err(|e| FerroError::PdfWrite(format!("Compression error: {}", e)))?;
    encoder
        .finish()
        .map_err(|e| FerroError::PdfWrite(format!("Compression finish error: {}", e)))
}

/// Subset a font to only include the specified glyph IDs.
/// Returns (subsetted_data, gid_remapping) on success, or (original_data, None) on failure.
pub(crate) fn subset_font(
    font_data: &[u8],
    used_gids: &std::collections::HashSet<u16>,
) -> (Vec<u8>, Option<HashMap<u16, u16>>) {
    // Build a sorted list of GIDs the subsetter should retain
    let mut gids: Vec<u16> = used_gids.iter().copied().collect();
    gids.sort_unstable();

    // Create a remapper and register all used glyphs
    let mut remapper = subsetter::GlyphRemapper::new();
    for &gid in &gids {
        remapper.remap(gid);
    }

    match subsetter::subset(font_data, 0, &remapper) {
        Ok(subsetted) => {
            // Build the remapping: old GID → new GID
            let mut mapping = HashMap::new();
            for &old_gid in &gids {
                if let Some(new_gid) = remapper.get(old_gid) {
                    mapping.insert(old_gid, new_gid);
                }
            }
            (subsetted, Some(mapping))
        }
        Err(_) => {
            // Subsetting failed — fall back to full font
            (font_data.to_vec(), None)
        }
    }
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

/// Encode text to WinAnsiEncoding for Type1 fallback fonts.
pub(crate) fn encode_winansi(text: &str) -> Vec<u8> {
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
