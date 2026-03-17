//! ferropdf-core — fundamental types shared across the entire pipeline.

use thiserror::Error;

// ─── Geometry ────────────────────────────────────────────────────────────────

/// A 2-D point (x, y) in CSS pixels.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

/// A 2-D size in CSS pixels.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

/// A rectangle with top-left origin (CSS pixel space, y-down).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }
    pub fn zero() -> Self { Self::default() }

    pub fn right(&self) -> f32  { self.x + self.width  }
    pub fn bottom(&self) -> f32 { self.y + self.height }

    /// True if this rect has zero area.
    pub fn is_empty(&self) -> bool { self.width <= 0.0 || self.height <= 0.0 }

    /// Expand self to include `other` (union bounding box).
    pub fn union(&self, other: &Rect) -> Rect {
        if self.is_empty() { return *other; }
        if other.is_empty() { return *self; }
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right  = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        Rect::new(x, y, right - x, bottom - y)
    }

    /// grow rect by `pad` on every side
    pub fn inflate(&self, pad: f32) -> Rect {
        Rect::new(self.x - pad, self.y - pad, self.width + pad * 2.0, self.height + pad * 2.0)
    }
}

/// Top / right / bottom / left edge sizes (margins, padding, border).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Edge {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Edge {
    pub fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self { top, right, bottom, left }
    }
    pub fn uniform(v: f32) -> Self { Self::new(v, v, v, v) }
    pub fn zero() -> Self { Self::default() }

    pub fn horizontal(&self) -> f32 { self.left + self.right }
    pub fn vertical(&self) -> f32   { self.top  + self.bottom }
}

// ─── Color ───────────────────────────────────────────────────────────────────

/// RGBA colour with components in [0.0, 1.0].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self { Self { r, g, b, a } }
    pub const fn rgb(r: f32, g: f32, b: f32)  -> Self  { Self::rgba(r, g, b, 1.0) }

    pub const BLACK:       Color = Color::rgb(0.0, 0.0, 0.0);
    pub const WHITE:       Color = Color::rgb(1.0, 1.0, 1.0);
    pub const TRANSPARENT: Color = Color::rgba(0.0, 0.0, 0.0, 0.0);

    pub fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: a as f32 / 255.0,
        }
    }

    pub fn from_rgb8(r: u8, g: u8, b: u8) -> Self {
        Self::from_rgba8(r, g, b, 255)
    }

    pub fn with_alpha(self, a: f32) -> Self { Self { a, ..self } }

    /// Pre-multiply alpha against a white background (for formats without transparency).
    pub fn blend_over_white(self) -> Self {
        let a = self.a;
        Self::rgb(
            self.r * a + (1.0 - a),
            self.g * a + (1.0 - a),
            self.b * a + (1.0 - a),
        )
    }
}

impl Default for Color {
    fn default() -> Self { Self::BLACK }
}

// ─── Length ──────────────────────────────────────────────────────────────────

/// CSS length value (pre-resolved into pixels by the style resolver).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Length {
    Px(f32),
    Mm(f32),
    Cm(f32),
    Pt(f32),
    Em(f32),
    Rem(f32),
    Percent(f32),
    Vw(f32),
    Vh(f32),
    Zero,
    Auto,
}

impl Length {
    /// Resolve to absolute pixels.
    ///
    /// - `em_px`      : current element's `font-size` in px
    /// - `rem_px`     : root element's `font-size` in px
    /// - `parent_px`  : parent dimension (for percent) in px
    /// - `viewport`   : viewport size in px (for vw/vh)
    pub fn to_px(&self, em_px: f32, rem_px: f32, parent_px: f32, viewport: Size) -> f32 {
        match *self {
            Length::Px(v)      => v,
            Length::Mm(v)      => v * 96.0 / 25.4,
            Length::Cm(v)      => v * 96.0 / 2.54,
            Length::Pt(v)      => v * 96.0 / 72.0,
            Length::Em(v)      => v * em_px,
            Length::Rem(v)     => v * rem_px,
            Length::Percent(v) => v / 100.0 * parent_px,
            Length::Vw(v)      => v / 100.0 * viewport.width,
            Length::Vh(v)      => v / 100.0 * viewport.height,
            Length::Zero       => 0.0,
            Length::Auto       => 0.0,
        }
    }

    pub fn is_auto(&self) -> bool    { matches!(self, Length::Auto) }
    pub fn is_percent(&self) -> bool { matches!(self, Length::Percent(_)) }
}

impl Default for Length {
    fn default() -> Self { Length::Zero }
}

// ─── PageSize ────────────────────────────────────────────────────────────────

/// Paper dimensions (width × height) in millimetres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageSize {
    pub width_mm: f32,
    pub height_mm: f32,
}

impl PageSize {
    pub fn a4()     -> Self { Self { width_mm: 210.0, height_mm: 297.0 } }
    pub fn a3()     -> Self { Self { width_mm: 297.0, height_mm: 420.0 } }
    pub fn a5()     -> Self { Self { width_mm: 148.0, height_mm: 210.0 } }
    pub fn letter() -> Self { Self { width_mm: 215.9, height_mm: 279.4 } }
    pub fn legal()  -> Self { Self { width_mm: 215.9, height_mm: 355.6 } }

    /// Landscape orientation (swap width/height).
    pub fn landscape(self) -> Self { Self { width_mm: self.height_mm, height_mm: self.width_mm } }

    /// Width in PDF user-units (= points, 72 pt/inch).
    pub fn width_pt(&self)  -> f32 { self.width_mm  * 72.0 / 25.4 }
    pub fn height_pt(&self) -> f32 { self.height_mm * 72.0 / 25.4 }

    /// Width in CSS pixels (96 dpi).
    pub fn width_px(&self)  -> f32 { self.width_mm  * 96.0 / 25.4 }
    pub fn height_px(&self) -> f32 { self.height_mm * 96.0 / 25.4 }
}

impl Default for PageSize {
    fn default() -> Self { Self::a4() }
}

// ─── EngineConfig ────────────────────────────────────────────────────────────

/// Top-level configuration passed to the rendering pipeline.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Paper size.
    pub page_size: PageSize,
    /// Page margins in CSS pixels (top/right/bottom/left).
    pub margin: Edge,
    /// Base URL for resolving relative resources.
    pub base_url: Option<String>,
    /// Extra directories to search for font files.
    pub font_dirs: Vec<String>,
    /// Document metadata.
    pub title: Option<String>,
    pub author: Option<String>,
    /// Enable Tailwind utility class resolution.
    pub tailwind: bool,
    /// Header / footer HTML templates.
    pub header_html: Option<String>,
    pub footer_html: Option<String>,
}

impl Default for EngineConfig {
    fn default() -> Self {
        // 20 mm margin on every side (≈ 75.6 px at 96 dpi)
        let margin_px = 20.0 * 96.0 / 25.4;
        Self {
            page_size: PageSize::a4(),
            margin: Edge::uniform(margin_px),
            base_url: None,
            font_dirs: Vec::new(),
            title: None,
            author: None,
            tailwind: false,
            header_html: None,
            footer_html: None,
        }
    }
}

impl EngineConfig {
    /// Usable content width in pixels.
    pub fn content_width_px(&self) -> f32 {
        self.page_size.width_px() - self.margin.horizontal()
    }
    /// Usable content height in pixels.
    pub fn content_height_px(&self) -> f32 {
        self.page_size.height_px() - self.margin.vertical()
    }
}

// ─── Errors ──────────────────────────────────────────────────────────────────

/// Main error type used throughout the pipeline.
#[derive(Debug, Error)]
pub enum FerroError {
    #[error("HTML parse error: {0}")]
    HtmlParse(String),

    #[error("CSS parse error: {0}")]
    CssParse(String),

    #[error("Layout error: {0}")]
    Layout(String),

    #[error("Font error: {0}")]
    Font(String),

    #[error("Image error: {0}")]
    Image(String),

    #[error("PDF render error: {0}")]
    Render(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, FerroError>;
