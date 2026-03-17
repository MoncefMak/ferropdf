//! The display list — backend-agnostic drawing operations.

/// RGBA colour (components in 0.0..=1.0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const BLACK:       Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const WHITE:       Color = Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    pub const TRANSPARENT: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };

    pub fn from_rgba(r: f32, g: f32, b: f32, a: f32) -> Self { Self { r, g, b, a } }
}

impl From<ferropdf_core::Color> for Color {
    fn from(c: ferropdf_core::Color) -> Self {
        Self { r: c.r, g: c.g, b: c.b, a: c.a }
    }
}

/// A resolved, backend-agnostic drawing command.
#[derive(Debug, Clone)]
pub enum DrawOp {
    /// Begin a new page (dimensions in PDF points).
    BeginPage { width_pt: f32, height_pt: f32 },
    /// End the current page.
    EndPage,

    // ── State ──────────────────────────────────────────────────────────────
    SaveState,
    RestoreState,
    /// Set global opacity (0.0–1.0) until the next SaveState/RestoreState.
    SetOpacity(f32),
    /// Clip all subsequent drawing to this rectangle.
    ClipRect { x: f32, y: f32, w: f32, h: f32 },

    // ── Rectangles ─────────────────────────────────────────────────────────
    FillRect   { x: f32, y: f32, w: f32, h: f32, color: Color },
    StrokeRect { x: f32, y: f32, w: f32, h: f32, color: Color, width: f32 },

    // ── Borders ────────────────────────────────────────────────────────────
    BorderLine {
        x1: f32, y1: f32, x2: f32, y2: f32,
        color: Color, width: f32,
    },

    // ── Text ───────────────────────────────────────────────────────────────
    /// A run of already-shaped glyphs from a single line.
    GlyphRun {
        x:      f32,
        y:      f32,         // baseline y
        size:   f32,         // font size in pt
        color:  Color,
        font:   String,      // font family name
        weight: u16,
        italic: bool,
        text:   String,      // the text (kept for fallback / Type1 fonts)
        glyph_ids: Vec<u16>, // glyph IDs from shaping
        advances:  Vec<f32>,
    },

    // ── Images ─────────────────────────────────────────────────────────────
    Image {
        x: f32, y: f32, w: f32, h: f32,
        /// Raw PNG/JPEG bytes.
        data: Vec<u8>,
        src:  String,
    },

    // ── Links ──────────────────────────────────────────────────────────────
    Link { x: f32, y: f32, w: f32, h: f32, uri: String },
}
