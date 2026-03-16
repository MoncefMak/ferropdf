//! Complex text layout: Arabic shaping + Unicode BiDi reordering.
//!
//! Workflow:
//!  1. Detect whether the input string contains complex script characters.
//!  2. If complex, apply the Unicode BiDi algorithm to determine visual runs.
//!  3. Shape each run with `rustybuzz` (HarfBuzz port) using the registered
//!     TrueType font, producing a sequence of glyph IDs + advances.
//!  4. Return `ShapedRun`s that the PDF generator uses to emit CID-keyed text.

use unicode_bidi::{BidiInfo, Level};

/// A single shaped glyph ready for PDF output.
#[derive(Debug, Clone)]
pub struct ShapedGlyph {
    /// Glyph ID in the font (for CID-keyed output).
    pub glyph_id: u16,
    /// Horizontal advance in font design units (from shaping / rustybuzz).
    pub advance: i32,
    /// The font's default horizontal advance from the hmtx table.
    /// Used by the PDF generator to compensate for differences between
    /// shaped advance and the auto-advance applied by the PDF viewer.
    pub hmtx_advance: u16,
    /// X offset from the default position (kerning / positioning).
    pub x_offset: i32,
    /// Y offset from the default position.
    pub y_offset: i32,
    /// The original Unicode character(s) this glyph represents (for ToUnicode CMap).
    pub cluster_text: String,
}

/// A visually-ordered run of shaped glyphs, all sharing the same direction.
#[derive(Debug, Clone)]
pub struct ShapedRun {
    /// The shaped glyphs in visual (display) order.
    pub glyphs: Vec<ShapedGlyph>,
    /// True if this run is right-to-left.
    pub is_rtl: bool,
    /// Total advance width in font design units.
    pub total_advance: i32,
}

/// Result of shaping an entire paragraph.
#[derive(Debug, Clone)]
pub struct ShapedText {
    /// Runs in visual order (left-to-right on screen).
    pub runs: Vec<ShapedRun>,
    /// Font units-per-em (to convert advances to px).
    pub units_per_em: u16,
}

impl ShapedText {
    /// Total width in font design units.
    pub fn total_advance(&self) -> i32 {
        self.runs.iter().map(|r| r.total_advance).sum()
    }

    /// Total width in CSS pixels for a given font size.
    pub fn width_px(&self, font_size_px: f64) -> f64 {
        let upem = self.units_per_em as f64;
        if upem == 0.0 {
            return 0.0;
        }
        self.total_advance() as f64 * font_size_px / upem
    }

    /// Collect all glyph IDs (for subsetting / ToUnicode).
    pub fn glyph_ids(&self) -> Vec<u16> {
        self.runs
            .iter()
            .flat_map(|r| r.glyphs.iter().map(|g| g.glyph_id))
            .collect()
    }
}

// ── Detection helpers ──────────────────────────────────────────────────────

/// Returns `true` if the string contains any characters that need complex
/// text layout (Arabic, Hebrew, Devanagari, Thai, etc.).
pub fn needs_complex_layout(text: &str) -> bool {
    text.chars().any(is_complex_char)
}

/// Returns `true` if the string contains any RTL characters.
pub fn contains_rtl(text: &str) -> bool {
    text.chars().any(is_rtl_char)
}

fn is_complex_char(c: char) -> bool {
    let cp = c as u32;
    // Arabic block (0600–06FF) + Arabic Supplement (0750–077F)
    // + Arabic Extended-A (08A0–08FF) + Arabic Presentation Forms-A/B
    (0x0600..=0x06FF).contains(&cp)
        || (0x0750..=0x077F).contains(&cp)
        || (0x08A0..=0x08FF).contains(&cp)
        || (0xFB50..=0xFDFF).contains(&cp) // Presentation Forms-A
        || (0xFE70..=0xFEFF).contains(&cp) // Presentation Forms-B
        // Hebrew (0590–05FF)
        || (0x0590..=0x05FF).contains(&cp)
        // Devanagari, Bengali, Gurmukhi, Gujarati, etc.
        || (0x0900..=0x0DFF).contains(&cp)
        // Thai (0E00–0E7F)
        || (0x0E00..=0x0E7F).contains(&cp)
}

fn is_rtl_char(c: char) -> bool {
    let cp = c as u32;
    (0x0590..=0x05FF).contains(&cp) // Hebrew
        || (0x0600..=0x06FF).contains(&cp) // Arabic
        || (0x0750..=0x077F).contains(&cp)
        || (0x08A0..=0x08FF).contains(&cp)
        || (0xFB50..=0xFDFF).contains(&cp)
        || (0xFE70..=0xFEFF).contains(&cp)
}

// ── Shaping pipeline ───────────────────────────────────────────────────────

/// Shape a text string using a TrueType font.
///
/// `font_data` — raw TTF/OTF bytes.
/// `text`      — the Unicode string to shape.
/// `font_size_px` — only used for reference; actual shaping is in design units.
/// `base_direction_rtl` — if `true`, treat the paragraph as RTL.
pub fn shape_text(
    font_data: &[u8],
    text: &str,
    _font_size_px: f64,
    base_direction_rtl: bool,
) -> Option<ShapedText> {
    if text.is_empty() {
        return Some(ShapedText {
            runs: Vec::new(),
            units_per_em: 1000,
        });
    }

    // Parse font for rustybuzz
    let face = rustybuzz::Face::from_slice(font_data, 0)?;
    let units_per_em = face.units_per_em() as u16;

    // Parse font with ttf-parser for hmtx lookups
    let ttf_face = ttf_parser::Face::parse(font_data, 0).ok();

    // --- BiDi reordering ---
    let default_level = if base_direction_rtl {
        Level::rtl()
    } else {
        Level::ltr()
    };
    let bidi_info = BidiInfo::new(text, Some(default_level));

    let mut shaped_runs = Vec::new();

    // Process each paragraph
    for para in &bidi_info.paragraphs {
        let line = para.range.clone();
        let (levels, visual) = bidi_info.visual_runs(para, line.clone());

        // Each element in `visual` is a Range<usize> (byte range) in the original text,
        // reordered for visual display.
        for run_range in &visual {
            let run_text = &text[run_range.clone()];
            if run_text.is_empty() {
                continue;
            }

            // Determine direction from the level at the start of this run
            let run_is_rtl = if run_range.start < levels.len() {
                levels[run_range.start - line.start].is_rtl()
            } else {
                base_direction_rtl
            };

            // Shape this run with rustybuzz
            let mut buffer = rustybuzz::UnicodeBuffer::new();
            buffer.push_str(run_text);
            if run_is_rtl {
                buffer.set_direction(rustybuzz::Direction::RightToLeft);
            } else {
                buffer.set_direction(rustybuzz::Direction::LeftToRight);
            }
            buffer.set_script(detect_script(run_text));

            let output = rustybuzz::shape(&face, &[], buffer);
            let positions = output.glyph_positions();
            let infos = output.glyph_infos();

            let mut glyphs = Vec::with_capacity(infos.len());
            let mut total_advance = 0i32;

            for (info, pos) in infos.iter().zip(positions.iter()) {
                let cluster_start = info.cluster as usize;
                let cluster_end = infos
                    .iter()
                    .filter(|i2| i2.cluster as usize > cluster_start)
                    .map(|i2| i2.cluster as usize)
                    .min()
                    .unwrap_or(run_text.len());
                let cluster_text = if cluster_start < run_text.len() {
                    run_text
                        .get(cluster_start..cluster_end.min(run_text.len()))
                        .unwrap_or("")
                        .to_string()
                } else {
                    String::new()
                };

                // Look up the font's default advance from hmtx table
                let glyph_id = info.glyph_id as u16;
                let hmtx_advance = ttf_face
                    .as_ref()
                    .and_then(|f| f.glyph_hor_advance(ttf_parser::GlyphId(glyph_id)))
                    .unwrap_or(pos.x_advance.unsigned_abs() as u16);

                glyphs.push(ShapedGlyph {
                    glyph_id,
                    advance: pos.x_advance,
                    hmtx_advance,
                    x_offset: pos.x_offset,
                    y_offset: pos.y_offset,
                    cluster_text,
                });
                total_advance += pos.x_advance;
            }

            shaped_runs.push(ShapedRun {
                glyphs,
                is_rtl: run_is_rtl,
                total_advance,
            });
        }
    }

    Some(ShapedText {
        runs: shaped_runs,
        units_per_em,
    })
}

/// Naive script detection for rustybuzz buffer tagging.
fn detect_script(text: &str) -> rustybuzz::Script {
    for c in text.chars() {
        let cp = c as u32;
        if (0x0600..=0x06FF).contains(&cp)
            || (0x0750..=0x077F).contains(&cp)
            || (0x08A0..=0x08FF).contains(&cp)
            || (0xFB50..=0xFDFF).contains(&cp)
            || (0xFE70..=0xFEFF).contains(&cp)
        {
            return rustybuzz::script::ARABIC;
        }
        if (0x0590..=0x05FF).contains(&cp) {
            return rustybuzz::script::HEBREW;
        }
        if (0x0900..=0x097F).contains(&cp) {
            return rustybuzz::script::DEVANAGARI;
        }
        if (0x0E00..=0x0E7F).contains(&cp) {
            return rustybuzz::script::THAI;
        }
    }
    rustybuzz::script::LATIN
}

/// Measure the width of shaped text in CSS pixels.
pub fn measure_shaped_width(
    font_data: &[u8],
    text: &str,
    font_size_px: f64,
    base_rtl: bool,
) -> f64 {
    if let Some(shaped) = shape_text(font_data, text, font_size_px, base_rtl) {
        shaped.width_px(font_size_px)
    } else {
        // Fallback: approximate as 0.5em per character
        text.chars().count() as f64 * font_size_px * 0.5
    }
}

/// Word-wrap shaped text. Returns lines of ShapedText.
pub fn wrap_shaped_text(
    font_data: &[u8],
    text: &str,
    font_size_px: f64,
    max_width_px: f64,
    base_rtl: bool,
) -> Vec<(String, ShapedText)> {
    if text.is_empty() || max_width_px <= 0.0 {
        let shaped = shape_text(font_data, text, font_size_px, base_rtl).unwrap_or(ShapedText {
            runs: Vec::new(),
            units_per_em: 1000,
        });
        return vec![(text.to_string(), shaped)];
    }

    let effective_max = max_width_px + 0.5;
    let space_width = measure_shaped_width(font_data, " ", font_size_px, false);

    let mut lines: Vec<(String, ShapedText)> = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0.0;

    for word in text.split_whitespace() {
        let word_width = measure_shaped_width(font_data, word, font_size_px, base_rtl);

        if current_line.is_empty() {
            current_line = word.to_string();
            current_width = word_width;
        } else if current_width + space_width + word_width <= effective_max {
            current_line.push(' ');
            current_line.push_str(word);
            current_width += space_width + word_width;
        } else {
            // Wrap current line
            let shaped = shape_text(font_data, &current_line, font_size_px, base_rtl).unwrap_or(
                ShapedText {
                    runs: Vec::new(),
                    units_per_em: 1000,
                },
            );
            lines.push((current_line, shaped));
            current_line = word.to_string();
            current_width = word_width;
        }
    }

    if !current_line.is_empty() {
        let shaped =
            shape_text(font_data, &current_line, font_size_px, base_rtl).unwrap_or(ShapedText {
                runs: Vec::new(),
                units_per_em: 1000,
            });
        lines.push((current_line, shaped));
    }

    if lines.is_empty() {
        lines.push((
            String::new(),
            ShapedText {
                runs: Vec::new(),
                units_per_em: 1000,
            },
        ));
    }

    lines
}

// ── TTF metrics extraction ─────────────────────────────────────────────────

/// Simple word-wrap using TTF font metrics, returns just the text lines.
/// This is the TTF-aware equivalent of `metrics::wrap_text_measured`.
pub fn wrap_ttf_text(
    font_data: &[u8],
    text: &str,
    font_size_px: f64,
    max_width_px: f64,
) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    if max_width_px <= 0.0 {
        return vec![text.to_string()];
    }

    let effective_max = max_width_px + 0.5;
    let space_width = measure_ttf_text_width_px(font_data, " ", font_size_px);

    let mut lines: Vec<String> = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0.0f64;

    for word in text.split_whitespace() {
        let word_width = measure_ttf_text_width_px(font_data, word, font_size_px);

        if current_line.is_empty() {
            current_line = word.to_string();
            current_width = word_width;
        } else if current_width + space_width + word_width <= effective_max {
            current_line.push(' ');
            current_line.push_str(word);
            current_width += space_width + word_width;
        } else {
            lines.push(current_line);
            current_line = word.to_string();
            current_width = word_width;
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

/// Parse a TTF/OTF font and return its units-per-em value.
pub fn font_units_per_em(font_data: &[u8]) -> Option<u16> {
    let face = ttf_parser::Face::parse(font_data, 0).ok()?;
    Some(face.units_per_em())
}

/// Get the horizontal advance of a single character from a TTF font,
/// in font design units. Returns `None` if the glyph is missing.
pub fn ttf_char_advance(font_data: &[u8], ch: char) -> Option<u16> {
    let face = ttf_parser::Face::parse(font_data, 0).ok()?;
    let glyph_id = face.glyph_index(ch)?;
    face.glyph_hor_advance(glyph_id)
}

/// Measure text width in CSS pixels using the raw TTF font.
/// Falls back to a default width for missing glyphs.
pub fn measure_ttf_text_width_px(font_data: &[u8], text: &str, font_size_px: f64) -> f64 {
    let face = match ttf_parser::Face::parse(font_data, 0) {
        Ok(f) => f,
        Err(_) => return text.len() as f64 * font_size_px * 0.5,
    };
    let upem = face.units_per_em() as f64;
    if upem == 0.0 {
        return 0.0;
    }
    let total: f64 = text
        .chars()
        .map(|ch| {
            face.glyph_index(ch)
                .and_then(|gid| face.glyph_hor_advance(gid))
                .unwrap_or((upem * 0.5) as u16) as f64
        })
        .sum();
    total * font_size_px / upem
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_needs_complex_layout() {
        assert!(needs_complex_layout("مرحبا"));
        assert!(needs_complex_layout("Hello مرحبا World"));
        assert!(!needs_complex_layout("Hello World"));
    }

    #[test]
    fn test_contains_rtl() {
        assert!(contains_rtl("مرحبا"));
        assert!(contains_rtl("שלום"));
        assert!(!contains_rtl("Hello"));
    }
}
