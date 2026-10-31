use cosmic_text::{
    Align, Attrs, AttrsList, Buffer, Family, FontSystem, LayoutGlyph, LineIter, Metrics,
    ShapeGlyph, ShapeLine, Shaping, Wrap,
};
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
    /// `letter-spacing` / `word-spacing`.
    pub spacing: Spacing,
    /// `text-indent` of the first line (pt), when this text starts a block.
    pub indent: f32,
    /// The text shaped on the first measurement, reused by the next ones.
    pub shaped: Option<ShapedText>,
    /// Sizes already measured, by width limit (Taffy asks for the same
    /// widths several times while sizing a tree).
    pub measured: Vec<(Option<f32>, (f32, f32))>,
}

/// CSS `letter-spacing` / `word-spacing`, in pt, added to the shaped
/// advances before line breaking.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Spacing {
    pub letter: f32,
    pub word: f32,
}

impl Spacing {
    /// The computed `letter-spacing` / `word-spacing` of `style`.
    pub fn of(style: &ferropdf_core::ComputedStyle) -> Self {
        Self {
            letter: style.letter_spacing.resolve_against(0.0),
            word: style.word_spacing.resolve_against(0.0),
        }
    }

    pub fn is_zero(&self) -> bool {
        self.letter == 0.0 && self.word == 0.0
    }
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

/// Renders a cached word shape survives without being used again.
const SHAPE_CACHE_RENDERS: u64 = 2;

impl Drop for FontSession<'_> {
    fn drop(&mut self) {
        if self.tainted {
            return;
        }
        if let Some(mut fs) = self.fs.take() {
            // Word shapes cached by cosmic-text during this render: keep the
            // ones used by the last few renders (repeated templates reuse
            // them) and bound the memory of a long-lived engine.
            fs.shape_run_cache.trim(SHAPE_CACHE_RENDERS);
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

/// cosmic-text attributes for a family / weight / style.
fn text_attrs(font_family: &str, bold: bool, italic: bool) -> Attrs<'_> {
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
    attrs
}

/// One laid-out line: the parts of cosmic-text's `LayoutRun` ferropdf uses.
struct Line {
    /// Text of the paragraph the line belongs to (glyph byte ranges index it).
    text: String,
    glyphs: Vec<LayoutGlyph>,
    line_w: f32,
    line_y: f32,
}

/// Metadata of the synthetic glyph that carries `text-indent`.
const INDENT_GLYPH: usize = usize::MAX - 1;

/// Text shaped paragraph by paragraph. Shaping (font fallback, glyphs,
/// letter / word spacing, indent) does not depend on the available width,
/// so it is done once and the paragraphs are line-broken at every width
/// asked (Taffy measures a text leaf several times).
#[derive(Debug, Clone)]
pub struct ShapedText {
    paragraphs: Vec<(String, ShapeLine)>,
}

/// Shape `spans` (one span when `rich` is false). `spacing` is indexed by
/// span (glyph metadata); `indent` applies to the first line.
#[allow(clippy::too_many_arguments)]
fn shape_text(
    fs: &mut FontSystem,
    spans: &[(&str, Attrs)],
    default_attrs: Attrs,
    rich: bool,
    font_size: f32,
    spacing: &[Spacing],
    indent: f32,
) -> ShapedText {
    let full: String = spans.iter().map(|(t, _)| *t).collect();
    let mut ranges = Vec::with_capacity(spans.len());
    let mut pos = 0;
    for (t, a) in spans {
        ranges.push((pos..pos + t.len(), *a));
        pos += t.len();
    }
    let base = if rich { default_attrs } else { spans[0].1 };
    let spaced = spacing.iter().any(|s| !s.is_zero());

    let mut paragraphs = Vec::new();
    // Paragraph splitting and per-paragraph attributes as in
    // `Buffer::set_text` / `set_rich_text`.
    for (i, (range, _)) in LineIter::new(&full).enumerate() {
        let para = &full[range.clone()];
        let mut attrs_list = AttrsList::new(base);
        if rich {
            for (r, a) in &ranges {
                let (s, e) = (r.start.max(range.start), r.end.min(range.end));
                if s < e {
                    attrs_list.add_span(s - range.start..e - range.start, *a);
                }
            }
        }
        let mut shape = ShapeLine::new(fs, para, &attrs_list, Shaping::Advanced, 8);
        if spaced {
            add_spacing(&mut shape, para, spacing, font_size);
        }
        if i == 0 && indent != 0.0 {
            add_indent(&mut shape, indent, font_size);
        }
        paragraphs.push((para.to_string(), shape));
    }
    ShapedText { paragraphs }
}

/// Break shaped text into lines at `width`, placed as
/// `Buffer::layout_runs` places them.
fn break_lines(
    shaped: &ShapedText,
    metrics: Metrics,
    width: Option<f32>,
    wrap: bool,
    align: Option<Align>,
) -> Vec<Line> {
    let wrap = if wrap { Wrap::WordOrGlyph } else { Wrap::None };
    let mut lines = Vec::new();
    let mut line_top = 0.0;
    for (text, shape) in &shaped.paragraphs {
        for ll in shape.layout(metrics.font_size, width, wrap, align, None) {
            let line_height = ll.line_height_opt.unwrap_or(metrics.line_height);
            let glyph_height = ll.max_ascent + ll.max_descent;
            let line_y = line_top + (line_height - glyph_height) / 2.0 + ll.max_ascent;
            lines.push(Line {
                text: text.clone(),
                glyphs: ll
                    .glyphs
                    .into_iter()
                    .filter(|g| g.metadata != INDENT_GLYPH)
                    .collect(),
                line_w: ll.w,
                line_y,
            });
            line_top += line_height;
        }
    }
    lines
}

/// Widest line and total height of shaped text broken at `width` (what
/// [`break_lines`] would give, without building the lines).
fn measure_lines(
    shaped: &ShapedText,
    metrics: Metrics,
    width: Option<f32>,
    wrap: bool,
) -> (f32, f32) {
    let wrap = if wrap { Wrap::WordOrGlyph } else { Wrap::None };
    let (mut w, mut h) = (0.0f32, 0.0f32);
    for (_, shape) in &shaped.paragraphs {
        for ll in shape.layout(metrics.font_size, width, wrap, None, None) {
            w = w.max(ll.w);
            h += ll.line_height_opt.unwrap_or(metrics.line_height);
        }
    }
    (w, h)
}

/// Shape and break `spans` into lines (see [`shape_text`], [`break_lines`]).
#[allow(clippy::too_many_arguments)]
fn lay_out(
    fs: &mut FontSystem,
    spans: &[(&str, Attrs)],
    default_attrs: Attrs,
    rich: bool,
    metrics: Metrics,
    width: Option<f32>,
    wrap: bool,
    align: Option<Align>,
    spacing: &[Spacing],
    indent: f32,
) -> Vec<Line> {
    let shaped = shape_text(
        fs,
        spans,
        default_attrs,
        rich,
        metrics.font_size,
        spacing,
        indent,
    );
    break_lines(&shaped, metrics, width, wrap, align)
}

/// Add `letter-spacing` after every cluster and `word-spacing` after every
/// word separator (CSS Text 3 §8), as extra advance (in em) on the last
/// glyph of the cluster.
fn add_spacing(shape: &mut ShapeLine, line: &str, spacing: &[Spacing], font_size: f32) {
    for span in &mut shape.spans {
        for word in &mut span.words {
            let n = word.glyphs.len();
            for i in 0..n {
                let key = (word.glyphs[i].start, word.glyphs[i].end);
                if word.glyphs[i + 1..].iter().any(|g| (g.start, g.end) == key) {
                    continue; // not the last glyph of its cluster
                }
                let g = &mut word.glyphs[i];
                let sp = spacing.get(g.metadata).copied().unwrap_or_default();
                let separator = matches!(
                    line.get(g.start..g.end),
                    Some(" " | "\u{a0}" | "\u{1361}" | "\u{10100}" | "\u{10101}" | "\u{1039f}")
                );
                let extra = sp.letter + if separator { sp.word } else { 0.0 };
                if extra != 0.0 {
                    let size = g.metrics_opt.map_or(font_size, |m| m.font_size);
                    g.x_advance += extra / size;
                }
            }
        }
    }
}

/// Insert `text-indent` as an invisible glyph at the logical start of the
/// paragraph (its visual start: the right edge of a right-to-left line).
fn add_indent(shape: &mut ShapeLine, indent: f32, font_size: f32) {
    let Some(span) = shape.spans.first_mut() else {
        return;
    };
    let rtl = span.level.is_rtl();
    let Some(word) = span.words.iter_mut().find(|w| !w.glyphs.is_empty()) else {
        return;
    };
    // Right-to-left words hold their glyphs in visual order.
    let model = if rtl {
        word.glyphs.last()
    } else {
        word.glyphs.first()
    };
    let Some(model) = model else {
        return;
    };
    let glyph = ShapeGlyph {
        end: model.start,
        x_advance: indent / font_size,
        y_advance: 0.0,
        x_offset: 0.0,
        y_offset: 0.0,
        metadata: INDENT_GLYPH,
        ..model.clone()
    };
    if rtl {
        word.glyphs.push(glyph);
    } else {
        word.glyphs.insert(0, glyph);
    }
}

/// Measure a text node using cosmic-text.
///
/// Called by Taffy's layout engine via `compute_layout_with_measure` whenever
/// it needs to know the intrinsic size of a text leaf.
pub fn measure_text(
    ctx: &mut TextContext,
    known_dimensions: Size<Option<f32>>,
    available_space: Size<AvailableSpace>,
    font_system: &mut FontSystem,
) -> Size<f32> {
    if ctx.text.trim().is_empty() {
        return Size::ZERO;
    }

    let metrics = Metrics::new(ctx.font_size, ctx.line_height);

    // Determine width constraint from Taffy
    let width_limit: Option<f32> = known_dimensions.width.or(match available_space.width {
        AvailableSpace::Definite(w) => Some(w),
        AvailableSpace::MaxContent => None,
        AvailableSpace::MinContent => Some(0.0), // Force maximum wrapping
    });

    let size = |(w, h): (f32, f32)| Size {
        width: known_dimensions.width.unwrap_or(w),
        height: known_dimensions.height.unwrap_or(h),
    };
    if let Some((_, wh)) = ctx.measured.iter().find(|(w, _)| *w == width_limit) {
        return size(*wh);
    }
    if ctx.shaped.is_none() {
        let attrs = text_attrs(&ctx.font_family, ctx.bold, ctx.italic);
        let shaped = shape_text(
            font_system,
            &[(&ctx.text, attrs)],
            attrs,
            false,
            ctx.font_size,
            &[ctx.spacing],
            ctx.indent,
        );
        ctx.shaped = Some(shaped);
    }
    let (max_width, mut total_height) = match &ctx.shaped {
        Some(shaped) => measure_lines(shaped, metrics, width_limit, ctx.wrap),
        None => (0.0, 0.0),
    };

    // If no layout runs (e.g. whitespace-only after shaping), return a minimal size
    if total_height == 0.0 {
        total_height = ctx.line_height;
    }

    let wh = (max_width.ceil(), total_height.ceil());
    if ctx.measured.len() < 8 {
        ctx.measured.push((width_limit, wh));
    }
    size(wh)
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
    spacing: Spacing,
    font_system: &mut FontSystem,
) -> Vec<ShapedLine> {
    if text.trim().is_empty() {
        return Vec::new();
    }

    let metrics = Metrics::new(font_size, line_height);
    let attrs = text_attrs(font_family, bold, italic);
    let runs = lay_out(
        font_system,
        &[(text, attrs)],
        attrs,
        false,
        metrics,
        Some(content_width),
        wrap,
        align,
        &[spacing],
        0.0,
    );

    let mut lines = Vec::new();

    for run in &runs {
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
                text: cluster_text(&run.text, glyph, &mut clusters),
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
#[allow(clippy::too_many_arguments)]
pub fn shape_rich_text_lines(
    spans: &[InlineSpan],
    content_width: f32,
    align: Option<Align>,
    wrap: bool,
    indent: f32,
    font_system: &mut FontSystem,
) -> Vec<ShapedLine> {
    if spans.is_empty() {
        return Vec::new();
    }

    let font_size = spans[0].font_size;
    let line_height = spans[0].line_height;
    let metrics = Metrics::new(font_size, line_height);

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

    let spacing: Vec<Spacing> = spans
        .iter()
        .map(|span| Spacing {
            letter: span.letter_spacing,
            word: span.word_spacing,
        })
        .collect();
    let runs = lay_out(
        font_system,
        &rich_spans,
        default_attrs,
        true,
        metrics,
        Some(content_width),
        wrap,
        align,
        &spacing,
        indent,
    );

    let mut lines = Vec::new();

    for run in &runs {
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
                text: cluster_text(&run.text, glyph, &mut clusters),
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

#[cfg(test)]
mod spacing_tests {
    use super::*;

    fn width(text: &str, spacing: Spacing, indent: f32, fs: &mut FontSystem) -> (f32, usize) {
        let attrs = text_attrs("", false, false);
        let lines = lay_out(
            fs,
            &[(text, attrs)],
            attrs,
            false,
            Metrics::new(12.0, 14.0),
            None,
            true,
            None,
            &[spacing],
            indent,
        );
        (lines[0].line_w, lines[0].glyphs.len())
    }

    #[test]
    fn letter_and_word_spacing_add_to_the_line_width() {
        let mut fs = FontSystem::new();
        let (plain, n) = width("ab cd", Spacing::default(), 0.0, &mut fs);
        let (letter, _) = width(
            "ab cd",
            Spacing {
                letter: 2.0,
                word: 0.0,
            },
            0.0,
            &mut fs,
        );
        assert!(
            (letter - plain - 2.0 * 5.0).abs() < 0.01,
            "{plain} {letter}"
        );
        let (word, m) = width(
            "ab cd",
            Spacing {
                letter: 0.0,
                word: 7.0,
            },
            0.0,
            &mut fs,
        );
        assert!((word - plain - 7.0).abs() < 0.01, "one separator");
        assert_eq!(n, m, "no glyph added");
    }

    #[test]
    fn indent_widens_the_first_line_without_a_visible_glyph() {
        let mut fs = FontSystem::new();
        let (plain, n) = width("abc", Spacing::default(), 0.0, &mut fs);
        let (indented, m) = width("abc", Spacing::default(), 20.0, &mut fs);
        assert!((indented - plain - 20.0).abs() < 0.01);
        assert_eq!(n, m, "the indent glyph is filtered out");
    }
}
