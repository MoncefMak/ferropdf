use std::sync::Mutex;
use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping};
use taffy::prelude::*;
use ferropdf_core::layout::{ShapedLine, ShapedGlyph};

/// Context attached to text leaf nodes in the Taffy tree.
/// Stores the info cosmic-text needs to measure the text on demand.
/// Toutes les valeurs (font_size, line_height) sont en points typographiques (pt).
#[derive(Debug, Clone)]
pub struct TextContext {
    pub text: String,
    pub font_size: f32,    // en pt
    pub line_height: f32,  // en pt
    pub font_family: String,
    pub bold: bool,
    pub italic: bool,
}

// =============================================================================
// FontDatabase — Wrappeur autour de cosmic_text::FontSystem
// =============================================================================
// cosmic_text::FontSystem n'est pas Send → doit être confiné au thread Rust
// ou protégé par Mutex si accès multi-thread.
// Ce wrappeur fournit les méthodes measure() et get_layout_runs() utilisées
// par le callback de Taffy et par la pagination pour l'extraction des lignes.
// =============================================================================

/// Wrappeur thread-safe autour de cosmic_text::FontSystem.
/// Centralise l'accès aux polices et le shaping typographique.
pub struct FontDatabase {
    inner: Mutex<FontSystem>,
}

impl FontDatabase {
    /// Crée une nouvelle FontDatabase avec les polices système chargées.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(FontSystem::new()),
        }
    }

    /// Mesure un bloc de texte avec wrapping à la largeur donnée.
    /// Retourne (width, height) en points typographiques.
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
        if h == 0.0 { h = line_height; }
        (w.ceil(), h.ceil())
    }

    /// Accède au FontSystem interne (nécessaire pour les appels Taffy
    /// qui requièrent &mut FontSystem directement).
    pub fn font_system_mut(&self) -> std::sync::MutexGuard<'_, FontSystem> {
        self.inner.lock().unwrap()
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
                glyph_id: glyph.glyph_id as u16,
                x: glyph.x,
                y: glyph.y,
                advance: glyph.w,
                font_id: 0, // fontdb::ID is opaque; not needed for PDF rendering
            });
        }

        lines.push(ShapedLine {
            glyphs,
            width: run.line_w,
            y: run.line_y,
            text: line_text,
        });
    }

    lines
}
