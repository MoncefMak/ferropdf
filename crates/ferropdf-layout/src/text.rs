use cosmic_text::{Align, Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Wrap};
use ferropdf_core::layout::{InlineSpan, ShapedGlyph, ShapedLine, ShapedSegment};
use std::sync::Mutex;
use taffy::prelude::*;

/// Context attached to text leaf nodes in the Taffy tree.
/// Stores the info cosmic-text needs to measure the text on demand.
/// All values (font_size, line_height) are in typographic points (pt).
#[derive(Debug, Clone)]
pub struct TextContext {
    pub text: String,
    /// `false` for `white-space: nowrap | pre` — never break lines.
    pub wrap: bool,
    pub font_size: f32,   // in pt
    pub line_height: f32, // in pt
    pub font_family: String,
    pub bold: bool,
    pub italic: bool,
}

// =============================================================================
// FontDatabase — pool of cosmic_text::FontSystem sharing one font database
// =============================================================================
// System fonts are discovered once (the expensive part). Each render checks
// out its own FontSystem from a pool, so renders on different threads never
// wait for each other. A FontSystem into which a document loaded @font-face
// data is dropped instead of being returned, so custom fonts never leak from
// one document into the next.
// =============================================================================

/// Thread-safe font database shared by all renders of an `Engine`.
pub struct FontDatabase {
    /// System fonts, loaded once. Cloning shares the font data (`Arc`).
    base: fontdb::Database,
    locale: String,
    /// Idle font systems, ready to be checked out.
    pool: Mutex<Vec<FontSystem>>,
}

impl Default for FontDatabase {
    fn default() -> Self {
        Self::new()
    }
}

impl FontDatabase {
    /// Create a new FontDatabase with system fonts loaded.
    pub fn new() -> Self {
        let first = FontSystem::new();
        let base = first.db().clone();
        let locale = first.locale().to_string();
        Self {
            base,
            locale,
            pool: Mutex::new(vec![first]),
        }
    }

    /// Check out a font system for one render. It goes back to the pool when
    /// the session is dropped (unless custom fonts were loaded into it).
    pub fn session(&self) -> FontSession<'_> {
        let pooled = self.pool.lock().unwrap_or_else(|e| e.into_inner()).pop();
        let fs = pooled.unwrap_or_else(|| {
            FontSystem::new_with_locale_and_db(self.locale.clone(), self.base.clone())
        });
        FontSession {
            owner: self,
            fs: Some(fs),
            tainted: false,
        }
    }

    /// Number of idle font systems (for tests and diagnostics).
    pub fn idle_count(&self) -> usize {
        self.pool.lock().unwrap_or_else(|e| e.into_inner()).len()
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
        let mut session = self.session();
        let fs = session.font_system();
        let metrics = Metrics::new(font_size, line_height);
        let mut buffer = Buffer::new(fs, metrics);
        buffer.set_size(fs, max_width, None);

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

        buffer.set_text(fs, text, attrs, Shaping::Advanced);
        buffer.shape_until_scroll(fs, false);

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
}

/// A font system checked out of a [`FontDatabase`] for the duration of one render.
pub struct FontSession<'a> {
    owner: &'a FontDatabase,
    fs: Option<FontSystem>,
    tainted: bool,
}

impl FontSession<'_> {
    /// The font system for shaping and layout.
    pub fn font_system(&mut self) -> &mut FontSystem {
        self.fs.as_mut().expect("font system present until drop")
    }

    /// The font database (system fonts + any @font-face loaded in this session).
    pub fn db(&self) -> &fontdb::Database {
        self.fs
            .as_ref()
            .expect("font system present until drop")
            .db()
    }

    /// Load document-specific font data (`@font-face`). The font is visible
    /// to this session only; the font system is discarded afterwards.
    pub fn load_font_data(&mut self, data: Vec<u8>) {
        self.tainted = true;
        self.font_system().db_mut().load_font_data(data);
    }
}

impl Drop for FontSession<'_> {
    fn drop(&mut self) {
        if self.tainted {
            return;
        }
        if let Some(fs) = self.fs.take() {
            self.owner
                .pool
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(fs);
        }
    }
}

/// Advance width of one space in the given font, in pt (used for collapsed
/// white space between two inline-level boxes).
pub fn space_width(ctx: &TextContext, font_system: &mut FontSystem) -> f32 {
    let measure = |text: &str, fs: &mut FontSystem| {
        let mut buffer = Buffer::new(fs, Metrics::new(ctx.font_size, ctx.line_height));
        buffer.set_wrap(fs, Wrap::None);
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
        buffer.set_text(fs, text, attrs, Shaping::Advanced);
        buffer.shape_until_scroll(fs, false);
        buffer
            .layout_runs()
            .map(|r| r.line_w)
            .fold(0.0_f32, f32::max)
    };
    // Trailing spaces may be excluded from line widths: measure between glyphs.
    (measure("x x", font_system) - measure("xx", font_system)).max(0.0)
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
    if !ctx.wrap {
        buffer.set_wrap(font_system, Wrap::None);
    }

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

/// Map CSS `text-align` to cosmic-text alignment.
///
/// `left` maps to `None` (cosmic-text's paragraph default) so that RTL
/// paragraphs keep their natural right alignment, matching `text-align: start`.
/// With an explicit alignment, cosmic-text bakes the offset into every
/// `glyph.x`, so painters must not add their own offset.
/// cosmic-text alignment for a *physical* `text-align` (see
/// `TextAlign::physical`). `Left` keeps cosmic-text's default, which follows
/// each paragraph's base direction.
pub fn css_align(align: ferropdf_core::TextAlign) -> Option<Align> {
    match align {
        ferropdf_core::TextAlign::Left | ferropdf_core::TextAlign::Start => None,
        ferropdf_core::TextAlign::End => Some(Align::Right),
        ferropdf_core::TextAlign::Right => Some(Align::Right),
        ferropdf_core::TextAlign::Center => Some(Align::Center),
        ferropdf_core::TextAlign::Justify => Some(Align::Justified),
    }
}

fn apply_align(buffer: &mut Buffer, align: Option<Align>) {
    if align.is_some() {
        for line in buffer.lines.iter_mut() {
            line.set_align(align);
        }
    }
}

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
    align: Option<Align>,
    wrap: bool,
    font_system: &mut FontSystem,
) -> Vec<ShapedLine> {
    if text.trim().is_empty() {
        return Vec::new();
    }

    let metrics = Metrics::new(font_size, line_height);
    let mut buffer = Buffer::new(font_system, metrics);
    buffer.set_size(font_system, Some(content_width), None);
    if !wrap {
        buffer.set_wrap(font_system, Wrap::None);
    }

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
    apply_align(&mut buffer, align);
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

        let mut clusters = std::collections::HashSet::new();
        for glyph in run.glyphs.iter() {
            glyphs.push(ShapedGlyph {
                glyph_id: glyph.glyph_id,
                font_id: glyph.font_id,
                x: glyph.x,
                y: glyph.y,
                advance: glyph.w,
                metadata: 0,
                text: cluster_text(run.text, glyph, &mut clusters),
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
    align: Option<Align>,
    wrap: bool,
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
    if !wrap {
        buffer.set_wrap(font_system, Wrap::None);
    }

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
    apply_align(&mut buffer, align);
    buffer.shape_until_scroll(font_system, false);

    let mut lines = Vec::new();

    for run in buffer.layout_runs() {
        let mut glyphs = Vec::new();
        let mut clusters = std::collections::HashSet::new();
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
                text: cluster_text(run.text, glyph, &mut clusters),
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

/// Text of `glyph`'s cluster, or `""` when an earlier glyph of the same
/// cluster (same source byte range) already carries it.
fn cluster_text(
    line: &str,
    glyph: &cosmic_text::LayoutGlyph,
    seen: &mut std::collections::HashSet<(usize, usize)>,
) -> String {
    if seen.insert((glyph.start, glyph.end)) {
        line.get(glyph.start..glyph.end).unwrap_or("").to_string()
    } else {
        String::new()
    }
}

#[cfg(test)]
mod pool_tests {
    use super::FontDatabase;

    #[test]
    fn sessions_are_pooled_and_reused() {
        let db = FontDatabase::new();
        assert_eq!(db.idle_count(), 1);
        {
            let _a = db.session();
            let _b = db.session(); // pool empty → a second system is created
            assert_eq!(db.idle_count(), 0);
        }
        assert_eq!(db.idle_count(), 2, "both systems go back to the pool");
    }

    #[test]
    fn font_face_sessions_are_discarded() {
        let db = FontDatabase::new();
        {
            let mut s = db.session();
            s.load_font_data(Vec::new()); // even an invalid font taints the session
        }
        assert_eq!(
            db.idle_count(),
            0,
            "a session with @font-face data must not be reused"
        );
        let s = db.session();
        assert_eq!(
            s.db().len(),
            db.base.len(),
            "fresh systems only see system fonts"
        );
    }

    #[test]
    fn font_database_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<FontDatabase>();
    }
}
