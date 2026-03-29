use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping};
use ferropdf_core::layout::{InlineSpan, ShapedGlyph, ShapedLine, ShapedSegment};
use std::sync::Mutex;
use taffy::prelude::*;

/// Context attached to text leaf nodes in the Taffy tree.
/// Stores the info cosmic-text needs to measure the text on demand.
/// All values (font_size, line_height) are in typographic points (pt).
#[derive(Debug, Clone)]
pub struct TextContext {
    pub text: String,
    pub font_size: f32,   // in pt
    pub line_height: f32, // in pt
    pub font_family: String,
    pub bold: bool,
    pub italic: bool,
}

// =============================================================================
// FontDatabase — Wrapper around cosmic_text::FontSystem
// =============================================================================
// cosmic_text::FontSystem is not Send → must be confined to the Rust thread
// or protected by a Mutex for multi-thread access.
// This wrapper provides measure() and get_layout_runs() methods used
// by the Taffy callback and by pagination for line extraction.
// =============================================================================

/// Thread-safe wrapper around cosmic_text::FontSystem.
/// Centralizes font access and typographic shaping.
pub struct FontDatabase {
    inner: Mutex<FontSystem>,
}

impl Default for FontDatabase {
    fn default() -> Self {
        Self::new()
    }
}

impl FontDatabase {
    /// Create a new FontDatabase with system fonts loaded.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(FontSystem::new()),
        }
    }

    /// Load custom font data (TTF/OTF bytes) into the font database.
    pub fn load_font_data(&self, data: Vec<u8>) {
        let mut fs = self.inner.lock().unwrap();
        fs.db_mut().load_font_data(data);
    }

    /// Measure a text block with wrapping at the given width.
    /// Returns (width, height) in typographic points.
    #[allow(clippy::too_many_arguments)]
    pub fn measure(
        &self,
        text: &str,
        font_size: f32,
        line_height: f32,
        font_family: &str,
        bold: bool,
        italic: bool,
        max_width: Option<f32>,
    ) -> (f32, f32) {
        let mut fs = self.inner.lock().unwrap();
        let metrics = Metrics::new(font_size, line_height);
        let mut buffer = Buffer::new(&mut fs, metrics);
        buffer.set_size(&mut fs, max_width, None);

        let family = if font_family.is_empty() {
            Family::SansSerif
        } else {
            Family::Name(font_family)
        };

        let mut attrs = Attrs::new().family(family);
        if bold {
            attrs = attrs.weight(cosmic_text::Weight::BOLD);
        }
        if italic {
            attrs = attrs.style(cosmic_text::Style::Italic);
        }

        buffer.set_text(&mut fs, text, attrs, Shaping::Advanced);
        buffer.shape_until_scroll(&mut fs, false);

        let mut w: f32 = 0.0;
        let mut h: f32 = 0.0;
        for run in buffer.layout_runs() {
            w = w.max(run.line_w);
            h += run.line_height;
        }
        if h == 0.0 {
            h = line_height;
        }
        (w.ceil(), h.ceil())
    }

    /// Access the internal FontSystem (needed for Taffy calls
    /// that require &mut FontSystem directly).
    pub fn font_system_mut(&self) -> std::sync::MutexGuard<'_, FontSystem> {
        self.inner.lock().unwrap()
    }

    /// Access the internal cosmic-text fontdb::Database (read-only).
    /// Allows reusing the same font database for PDF writing.
    pub fn fontdb(&self) -> FontDbGuard<'_> {
        FontDbGuard {
            guard: self.inner.lock().unwrap(),
        }
    }
}

/// RAII guard that provides read access to the inner fontdb::Database.
pub struct FontDbGuard<'a> {
    guard: std::sync::MutexGuard<'a, FontSystem>,
}

impl<'a> FontDbGuard<'a> {
    pub fn db(&self) -> &fontdb::Database {
        self.guard.db()
    }
}

/// Measure a text node using cosmic-text.
///
/// Called by Taffy's layout engine via `compute_layout_with_measure` whenever
/// it needs to know the intrinsic size of a text leaf.
pub fn measure_text(
    ctx: &TextContext,
    known_dimensions: Size<Option<f32>>,
    available_space: Size<AvailableSpace>,
    font_system: &mut FontSystem,
) -> Size<f32> {
    if ctx.text.trim().is_empty() {
        return Size::ZERO;
    }

    let metrics = Metrics::new(ctx.font_size, ctx.line_height);
    let mut buffer = Buffer::new(font_system, metrics);

    // Determine width constraint from Taffy
    let width_limit: Option<f32> = known_dimensions.width.or(match available_space.width {
        AvailableSpace::Definite(w) => Some(w),
        AvailableSpace::MaxContent => None,
        AvailableSpace::MinContent => Some(0.0), // Force maximum wrapping
    });

    buffer.set_size(font_system, width_limit, None);

    let family = if ctx.font_family.is_empty() {
        Family::SansSerif
    } else {
        Family::Name(&ctx.font_family)
    };

    let mut attrs = Attrs::new().family(family);
    if ctx.bold {
        attrs = attrs.weight(cosmic_text::Weight::BOLD);
    }
    if ctx.italic {
        attrs = attrs.style(cosmic_text::Style::Italic);
    }

    buffer.set_text(font_system, &ctx.text, attrs, Shaping::Advanced);
    buffer.shape_until_scroll(font_system, false);

    let mut max_width: f32 = 0.0;
    let mut total_height: f32 = 0.0;

    for run in buffer.layout_runs() {
        max_width = max_width.max(run.line_w);
        total_height += run.line_height;
    }

    // If no layout runs (e.g. whitespace-only after shaping), return a minimal size
    if total_height == 0.0 {
        total_height = ctx.line_height;
    }

    Size {
        width: known_dimensions.width.unwrap_or(max_width.ceil()),
        height: known_dimensions.height.unwrap_or(total_height.ceil()),
    }
}

// =============================================================================
// shape_text_lines — Shape text and return per-line data for rendering
// =============================================================================
// After Taffy layout determines the final content width, we re-shape the text
// at that exact width to get the definitive line breaks + glyph positions.
// This is the SINGLE SOURCE OF TRUTH for text rendering — pdf.rs must not
// re-wrap the text.
// =============================================================================

/// Shape a text node at its final content width and return shaped lines.
/// Each ShapedLine contains the line's text, width, y-offset, and glyph data.
#[allow(clippy::too_many_arguments)]
pub fn shape_text_lines(
    text: &str,
    font_size: f32,
    line_height: f32,
    font_family: &str,
    bold: bool,
    italic: bool,
    content_width: f32,
    font_system: &mut FontSystem,
) -> Vec<ShapedLine> {
    if text.trim().is_empty() {
        return Vec::new();
    }

    let metrics = Metrics::new(font_size, line_height);
    let mut buffer = Buffer::new(font_system, metrics);
    buffer.set_size(font_system, Some(content_width), None);

    let family = if font_family.is_empty() {
        Family::SansSerif
    } else {
        Family::Name(font_family)
    };

    let mut attrs = Attrs::new().family(family);
    if bold {
        attrs = attrs.weight(cosmic_text::Weight::BOLD);
    }
    if italic {
        attrs = attrs.style(cosmic_text::Style::Italic);
    }

    buffer.set_text(font_system, text, attrs, Shaping::Advanced);
    buffer.shape_until_scroll(font_system, false);

    let mut lines = Vec::new();

    for run in buffer.layout_runs() {
        let mut glyphs = Vec::new();

        // Extract visual line text from the source BufferLine using glyph byte ranges.
        // glyph.start/end are byte offsets into run.text (the BufferLine), NOT the full text.
        let line_text = if !run.glyphs.is_empty() {
            let min_start = run.glyphs.iter().map(|g| g.start).min().unwrap();
            let max_end = run.glyphs.iter().map(|g| g.end).max().unwrap();
            run.text.get(min_start..max_end).unwrap_or("").to_string()
        } else {
            String::new()
        };

        for glyph in run.glyphs.iter() {
            glyphs.push(ShapedGlyph {
                glyph_id: glyph.glyph_id,
                font_id: glyph.font_id,
                x: glyph.x,
                y: glyph.y,
                advance: glyph.w,
                metadata: 0,
            });
        }

        lines.push(ShapedLine {
            glyphs,
            width: run.line_w,
            y: run.line_y,
            text: line_text,
            segments: Vec::new(),
        });
    }

    lines
}

/// Shape merged inline text using cosmic-text's rich text API.
/// Each InlineSpan maps to a cosmic-text span with per-span bold/italic/font_family.
/// Returns ShapedLines with per-segment metadata linking back to the span index.
pub fn shape_rich_text_lines(
    spans: &[InlineSpan],
    content_width: f32,
    font_system: &mut FontSystem,
) -> Vec<ShapedLine> {
    if spans.is_empty() {
        return Vec::new();
    }

    let font_size = spans[0].font_size;
    let line_height = spans[0].line_height;
    let metrics = Metrics::new(font_size, line_height);
    let mut buffer = Buffer::new(font_system, metrics);
    buffer.set_size(font_system, Some(content_width), None);

    // Build per-span Attrs with metadata = span index
    let rich_spans: Vec<(&str, Attrs)> = spans
        .iter()
        .enumerate()
        .map(|(i, span)| {
            let family = if span.font_family.is_empty() {
                Family::SansSerif
            } else {
                Family::Name(&span.font_family)
            };
            let mut attrs = Attrs::new().family(family).metadata(i);
            if span.bold {
                attrs = attrs.weight(cosmic_text::Weight::BOLD);
            }
            if span.italic {
                attrs = attrs.style(cosmic_text::Style::Italic);
            }
            (span.text.as_str(), attrs)
        })
        .collect();

    let default_family = if spans[0].font_family.is_empty() {
        Family::SansSerif
    } else {
        Family::Name(&spans[0].font_family)
    };
    let default_attrs = Attrs::new().family(default_family);

    buffer.set_rich_text(font_system, rich_spans, default_attrs, Shaping::Advanced);
    buffer.shape_until_scroll(font_system, false);

    let mut lines = Vec::new();

    for run in buffer.layout_runs() {
        let mut glyphs = Vec::new();
        let mut segments: Vec<ShapedSegment> = Vec::new();
        let mut current_meta = usize::MAX;
        let mut seg_start_byte = usize::MAX;
        let mut seg_end_byte: usize = 0;
        let mut seg_x: f32 = 0.0;

        for glyph in run.glyphs.iter() {
            if glyph.metadata != current_meta {
                // Flush previous segment
                if current_meta != usize::MAX && seg_start_byte < seg_end_byte {
                    let seg_text = run
                        .text
                        .get(seg_start_byte..seg_end_byte)
                        .unwrap_or("")
                        .to_string();
                    let seg_width = glyph.x - seg_x;
                    segments.push(ShapedSegment {
                        text: seg_text,
                        x_offset: seg_x,
                        width: seg_width.max(0.0),
                        metadata: current_meta,
                    });
                }
                current_meta = glyph.metadata;
                seg_start_byte = glyph.start;
                seg_end_byte = glyph.end;
                seg_x = glyph.x;
            } else {
                seg_end_byte = seg_end_byte.max(glyph.end);
            }

            glyphs.push(ShapedGlyph {
                glyph_id: glyph.glyph_id,
                font_id: glyph.font_id,
                x: glyph.x,
                y: glyph.y,
                advance: glyph.w,
                metadata: glyph.metadata,
            });
        }

        // Flush last segment
        if current_meta != usize::MAX && seg_start_byte < seg_end_byte {
            let seg_text = run
                .text
                .get(seg_start_byte..seg_end_byte)
                .unwrap_or("")
                .to_string();
            let seg_width = run.line_w - seg_x;
            segments.push(ShapedSegment {
                text: seg_text,
                x_offset: seg_x,
                width: seg_width.max(0.0),
                metadata: current_meta,
            });
        }

        // Build full line text
        let line_text = if !run.glyphs.is_empty() {
            let min_start = run.glyphs.iter().map(|g| g.start).min().unwrap();
            let max_end = run.glyphs.iter().map(|g| g.end).max().unwrap();
            run.text.get(min_start..max_end).unwrap_or("").to_string()
        } else {
            String::new()
        };

        lines.push(ShapedLine {
            glyphs,
            width: run.line_w,
            y: run.line_y,
            text: line_text,
            segments,
        });
    }

    lines
}
