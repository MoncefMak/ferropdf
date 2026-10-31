use ferropdf_core::FerroError;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use pdf_writer::types::FontFlags;
use pdf_writer::{Filter, Finish, Name, Pdf, Rect as PdfWriterRect, Ref, Str};
use std::collections::HashMap;
use std::io::Write;

/// A resolved font ready to be embedded in the PDF.
pub(crate) struct EmbeddedFont {
    /// Original raw TTF/OTF/TTC bytes (for char→glyph lookups)
    pub original_data: Vec<u8>,
    /// Index of the face inside `original_data` (non-zero for a face of a
    /// `.ttc` / `.otc` collection).
    pub face_index: u32,
    /// Text of each original glyph ID as shaped (clusters, ligatures), for
    /// an exact ToUnicode map.
    pub glyph_text: HashMap<u16, String>,
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

impl EmbeddedFont {
    /// The embedded face, parsed from the original data.
    pub fn face(&self) -> Option<ttf_parser::Face<'_>> {
        ttf_parser::Face::parse(&self.original_data, self.face_index).ok()
    }
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
) -> Option<(Vec<u8>, u32)> {
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
    db.with_face_data(id, |data, index| (data.to_vec(), index))
}

// ── CIDFont embedding ──

pub(crate) fn write_cid_font(pdf: &mut Pdf, ef: &EmbeddedFont) -> ferropdf_core::Result<()> {
    // Parse the ORIGINAL font for metrics (ascender, bbox, etc.)
    let face = ef
        .face()
        .ok_or_else(|| FerroError::Font("Failed to parse the embedded font".into()))?;
    // CFF-flavoured OpenType (`.otf`, most `.ttc` CJK / macOS faces) must be
    // embedded as FontFile3 /OpenType with a CIDFontType0; TrueType outlines
    // as FontFile2 with a CIDFontType2 (PDF 32000 §9.9).
    let is_cff = face.tables().cff.is_some();

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
    if is_cff {
        stream.pair(Name(b"Subtype"), Name(b"OpenType"));
    } else {
        stream.pair(Name(b"Length1"), ef.subset_data.len() as i32);
    }
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

    // A subset font's name starts with a six-letter tag (PDF 32000 §9.6.4),
    // which tells readers the font is partial and keeps subsets of the same
    // font apart.
    let ps_name = match &ef.gid_remapping {
        Some(map) => format!(
            "{}+{}",
            subset_tag(&ef.pdf_name, map),
            sanitize_ps_name(&base_font_name)
        ),
        None => sanitize_ps_name(&base_font_name),
    };

    let mut descriptor = pdf.font_descriptor(ef.descriptor_ref);
    descriptor.name(Name(ps_name.as_bytes()));
    descriptor.flags(fontdb_flags(&face));
    descriptor.bbox(font_bbox);
    descriptor.italic_angle(if face.is_italic() { -12.0 } else { 0.0 });
    descriptor.ascent(ascender);
    descriptor.descent(descender);
    descriptor.cap_height(cap_height);
    descriptor.stem_v(80.0 + ascender * 0.08);
    if is_cff {
        descriptor.font_file3(ef.font_stream_ref);
    } else {
        descriptor.font_file2(ef.font_stream_ref);
    }
    descriptor.finish();

    // 3. Write CIDFont (CIDFontType2)
    let mut cid_font = pdf.cid_font(ef.cid_font_ref);
    cid_font.subtype(if is_cff {
        pdf_writer::types::CidFontType::Type0
    } else {
        pdf_writer::types::CidFontType::Type2
    });
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
            // Parse the subsetted font (a single face) for width info
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

    // CIDToGIDMap Identity — CID values map directly to glyph IDs. Only
    // defined for CIDFontType2; a CFF font's CIDs are its glyph indices.
    if !is_cff {
        cid_font.cid_to_gid_map_predefined(Name(b"Identity"));
    }
    cid_font.finish();

    // 4. Write ToUnicode CMap
    let tounicode_cmap = build_tounicode_cmap(&face, ef.gid_remapping.as_ref(), &ef.glyph_text);
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
    face_index: u32,
    gid_remapping: Option<&HashMap<u16, u16>>,
) -> Vec<u8> {
    let face = match ttf_parser::Face::parse(font_data, face_index) {
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
    glyph_text: &HashMap<u16, String>,
) -> String {
    let remap = |gid: u16| -> Option<u16> {
        match gid_remapping {
            Some(map) => map.get(&gid).copied(),
            None => Some(gid),
        }
    };
    // (Remapped) glyph ID → text. The shaped text wins: it is exact for
    // ligatures and for glyphs shared by several code points (CJK
    // compatibility ideographs, Kangxi radicals).
    let mut gid_to_text: HashMap<u16, String> = HashMap::new();
    for (&gid, text) in glyph_text {
        if let Some(mapped) = remap(gid) {
            gid_to_text.insert(mapped, text.clone());
        }
    }
    // Other glyphs (unshaped text): reverse lookup in the cmap, preferring
    // the lowest code point.
    if let Some(subtable) = face.tables().cmap {
        let mut reverse: HashMap<u16, char> = HashMap::new();
        for sub in subtable.subtables {
            if !sub.is_unicode() {
                continue;
            }
            sub.codepoints(|cp| {
                if let Some(ch) = char::from_u32(cp) {
                    if let Some(gid) = face.glyph_index(ch) {
                        if let Some(mapped) = remap(gid.0) {
                            let e = reverse.entry(mapped).or_insert(ch);
                            if ch < *e {
                                *e = ch;
                            }
                        }
                    }
                }
            });
        }
        for (gid, ch) in reverse {
            gid_to_text.entry(gid).or_insert_with(|| ch.to_string());
        }
    }

    let mut entries: Vec<(u16, String)> = gid_to_text
        .into_iter()
        .filter(|(_, t)| !t.is_empty())
        .collect();
    entries.sort_by_key(|(gid, _)| *gid);

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
        for (gid, text) in chunk {
            // Destination strings are UTF-16BE (surrogate pairs beyond the
            // BMP, several code units for ligatures).
            let utf16: String = text.encode_utf16().map(|u| format!("{u:04X}")).collect();
            cmap.push_str(&format!("<{gid:04X}> <{utf16}>\n"));
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
    face_index: u32,
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

    match subsetter::subset(font_data, face_index, &remapper) {
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
        Err(e) => {
            // Subsetting failed — fall back to the full font. A collection
            // cannot be embedded whole: the face would be unusable.
            if font_data.starts_with(b"ttcf") {
                log::warn!("could not extract face {face_index} of a font collection: {e:?}");
            }
            (font_data.to_vec(), None)
        }
    }
}

/// Deterministic six-uppercase-letter subset tag (FNV-1a of the resource
/// name and the retained glyphs).
fn subset_tag(pdf_name: &str, gids: &HashMap<u16, u16>) -> String {
    let mut sorted: Vec<u16> = gids.keys().copied().collect();
    sorted.sort_unstable();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let bytes = pdf_name
        .bytes()
        .chain(sorted.iter().flat_map(|g| g.to_be_bytes()));
    for b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    (0..6)
        .map(|i| (b'A' + ((h >> (i * 5)) % 26) as u8) as char)
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn winansi_passes_ascii_through() {
        let bytes = encode_winansi("Hello, World!");
        assert_eq!(bytes, b"Hello, World!");
    }

    #[test]
    fn winansi_handles_latin1_supplement() {
        // U+00E9 (é) → 0xE9 in WinANSI
        let bytes = encode_winansi("café");
        assert_eq!(bytes, vec![b'c', b'a', b'f', 0xE9]);
    }

    #[test]
    fn winansi_maps_special_punctuation() {
        // U+2014 (em dash) → 0x97
        let bytes = encode_winansi("a—b");
        assert_eq!(bytes, vec![b'a', 0x97, b'b']);
    }

    #[test]
    fn winansi_maps_euro_sign() {
        let bytes = encode_winansi("10€");
        assert_eq!(bytes, vec![b'1', b'0', 0x80]);
    }

    #[test]
    fn winansi_falls_back_to_question_mark_for_unmapped() {
        // CJK characters have no WinANSI mapping.
        let bytes = encode_winansi("漢字");
        assert_eq!(bytes, vec![b'?', b'?']);
    }

    #[test]
    fn encode_shaped_glyphs_emits_big_endian_u16() {
        let gids = [0x0001u16, 0x0102, 0xFFFF];
        let bytes = encode_shaped_glyphs(&gids, None);
        assert_eq!(bytes, vec![0x00, 0x01, 0x01, 0x02, 0xFF, 0xFF]);
    }

    #[test]
    fn encode_shaped_glyphs_applies_remapping() {
        let gids = [42u16, 7, 1000];
        let mut map = HashMap::new();
        map.insert(42u16, 1u16);
        map.insert(7u16, 2u16);
        // 1000 is not in the map → falls back to 0 (notdef glyph).
        let bytes = encode_shaped_glyphs(&gids, Some(&map));
        assert_eq!(bytes, vec![0x00, 0x01, 0x00, 0x02, 0x00, 0x00]);
    }

    #[test]
    fn sanitize_ps_name_strips_disallowed_chars() {
        // Spaces and other special characters must be removed; alphanumerics,
        // `-`, and `+` survive (PostScript name rules).
        assert_eq!(sanitize_ps_name("Helvetica Neue"), "HelveticaNeue");
        assert_eq!(sanitize_ps_name("Noto-Sans+CJK"), "Noto-Sans+CJK");
        assert_eq!(sanitize_ps_name("DejaVu Sans Mono"), "DejaVuSansMono");
        assert_eq!(sanitize_ps_name("Bad/Name?"), "BadName");
    }

    #[test]
    fn subset_font_falls_back_on_garbage_input() {
        let used: std::collections::HashSet<u16> = [1u16, 2, 3].into_iter().collect();
        let (data, mapping) = subset_font(b"not a real font", 0, &used);
        // On parse failure subsetter returns the original bytes unchanged
        // and signals "no remapping" via None.
        assert_eq!(data, b"not a real font");
        assert!(mapping.is_none());
    }
}

#[cfg(test)]
mod subset_tag_tests {
    use super::*;

    #[test]
    fn tag_is_six_uppercase_letters_and_deterministic() {
        let map: HashMap<u16, u16> = [(3, 1), (7, 2)].into_iter().collect();
        let a = subset_tag("F4", &map);
        assert_eq!(a.len(), 6);
        assert!(a.chars().all(|c| c.is_ascii_uppercase()));
        assert_eq!(a, subset_tag("F4", &map));
        assert_ne!(a, subset_tag("F5", &map));
    }
}
