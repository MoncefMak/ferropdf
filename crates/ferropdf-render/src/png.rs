//! PNG export (feature `png`).
//!
//! Pages are rasterized from the PDF ferropdf has just written, with
//! [hayro](https://github.com/LaurenzV/hayro), so a preview is exactly the
//! PDF: same layout, fonts, gradients, transparency and clipping.

use ferropdf_core::FerroError;
use hayro::hayro_interpret::util::TransformExt;
use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::kurbo::Affine;
use hayro::vello_cpu::color::palette::css::{TRANSPARENT, WHITE};
use hayro::vello_cpu::{Pixmap, RasterizerSettings, RenderContext, Resources, TargetInit};
use hayro::{render_into, RenderCache, RenderSettings};

/// Largest width or height of an exported image, in pixels.
pub const MAX_PNG_SIDE: u32 = 16_384;
/// Largest exported image, in pixels (64 Mpx = 256 MiB of RGBA).
pub const MAX_PNG_PIXELS: u64 = 64 * 1024 * 1024;

/// How pages are rasterized.
#[derive(Debug, Clone, PartialEq)]
pub struct PngOptions {
    /// Resolution. 96 gives one image pixel per CSS pixel.
    pub dpi: f32,
    /// Zero-based page indices, in output order; negative values count from
    /// the end (`-1` is the last page). `None` renders every page.
    pub pages: Option<Vec<i64>>,
    /// Keep the page background transparent instead of white.
    pub transparent: bool,
}

impl Default for PngOptions {
    fn default() -> Self {
        Self {
            dpi: 96.0,
            pages: None,
            transparent: false,
        }
    }
}

/// Why a PNG export failed.
#[derive(Debug, Clone, PartialEq)]
pub enum PngError {
    /// A bad `dpi`, or a page too large at that `dpi`.
    InvalidOption(String),
    /// A page index outside the document.
    PageOutOfRange(String),
    /// The PDF could not be read or the PNG encoded.
    Render(String),
}

impl std::fmt::Display for PngError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidOption(m) | Self::PageOutOfRange(m) | Self::Render(m) => {
                write!(f, "PNG export: {m}")
            }
        }
    }
}

impl std::error::Error for PngError {}

impl From<PngError> for FerroError {
    fn from(e: PngError) -> Self {
        FerroError::PdfWrite(e.to_string())
    }
}

/// Check `dpi` (the page list is checked against the page count later).
pub fn validate_png_options(opts: &PngOptions) -> Result<(), PngError> {
    if !(opts.dpi.is_finite() && (1.0..=2400.0).contains(&opts.dpi)) {
        return Err(PngError::InvalidOption(format!(
            "dpi must be between 1 and 2400, got {}",
            opts.dpi
        )));
    }
    Ok(())
}

/// Resolve `pages` against `count` (Python-style negative indices).
fn resolve_pages(pages: Option<&[i64]>, count: usize) -> Result<Vec<usize>, PngError> {
    let Some(pages) = pages else {
        return Ok((0..count).collect());
    };
    pages
        .iter()
        .map(|&i| {
            let idx = if i < 0 { count as i64 + i } else { i };
            if (0..count as i64).contains(&idx) {
                Ok(idx as usize)
            } else {
                Err(PngError::PageOutOfRange(format!(
                    "page index {i} out of range (document has {count} page{})",
                    if count == 1 { "" } else { "s" }
                )))
            }
        })
        .collect()
}

/// Rasterize pages of a ferropdf-generated PDF to PNG files (one per page).
pub fn pdf_to_png(pdf: &[u8], opts: &PngOptions) -> Result<Vec<Vec<u8>>, PngError> {
    validate_png_options(opts)?;
    let doc =
        Pdf::new(pdf.to_vec()).map_err(|e| PngError::Render(format!("cannot read PDF: {e:?}")))?;
    let pages = doc.pages();
    let indices = resolve_pages(opts.pages.as_deref(), pages.len())?;

    let scale = opts.dpi / 72.0;
    let cache = RenderCache::new();
    let background = if opts.transparent { TRANSPARENT } else { WHITE };
    // Every font ferropdf embeds is in the PDF; the standard-14 fallback
    // fonts (used only when no system font exists) have nothing to draw with.
    let interpreter = InterpreterSettings::default();

    let mut out = Vec::with_capacity(indices.len());
    for i in indices {
        let page = &pages[i];
        let (w, h) = page.render_dimensions();
        // Partial pixels are kept, as MuPDF and Poppler do (A4 at 96 dpi
        // is 794 × 1123).
        let (pw, ph) = ((w * scale).ceil() as u64, (h * scale).ceil() as u64);
        if pw == 0 || ph == 0 {
            return Err(PngError::InvalidOption(format!(
                "page {i} is empty at {} dpi",
                opts.dpi
            )));
        }
        if pw > u64::from(MAX_PNG_SIDE) || ph > u64::from(MAX_PNG_SIDE) || pw * ph > MAX_PNG_PIXELS
        {
            return Err(PngError::InvalidOption(format!(
                "page {i} would be {pw}×{ph} px at {} dpi (limit {MAX_PNG_SIDE} px per side, \
                 {} Mpx); lower the dpi",
                opts.dpi,
                MAX_PNG_PIXELS / (1024 * 1024)
            )));
        }
        // Within MAX_PNG_SIDE, so the u16 casts are exact.
        let mut ctx = RenderContext::new(pw as u16, ph as u16);
        let transform = Affine::scale(f64::from(scale)) * page.initial_transform(true).to_kurbo();
        render_into(
            page,
            &cache,
            &interpreter,
            &RenderSettings::default(),
            &mut ctx,
            transform,
        );
        ctx.flush();
        let mut pixmap = Pixmap::new(ctx.width(), ctx.height());
        ctx.render_with(
            &mut pixmap,
            &mut Resources::default(),
            RasterizerSettings {
                target_init: TargetInit::Clear(background),
                ..Default::default()
            },
        );
        out.push(
            pixmap
                .into_png()
                .map_err(|e| PngError::Render(format!("PNG encoding failed: {e}")))?,
        );
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{render, RenderOptions};

    fn png_size(png: &[u8]) -> (u32, u32) {
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        (
            u32::from_be_bytes(png[16..20].try_into().unwrap()),
            u32::from_be_bytes(png[20..24].try_into().unwrap()),
        )
    }

    fn pdf(html: &str, page_size: &str) -> Vec<u8> {
        let opts = RenderOptions {
            page_size: page_size.into(),
            ..RenderOptions::default()
        };
        render(html, &opts).unwrap()
    }

    #[test]
    fn renders_every_page_at_the_requested_dpi() {
        let html = "<p>one</p><div style='break-before: page'>two</div>";
        let doc = pdf(html, "A4");
        let pngs = pdf_to_png(&doc, &PngOptions::default()).unwrap();
        assert_eq!(pngs.len(), 2);
        // A4 = 595.28 × 841.89 pt → 793.7 × 1122.5 px at 96 dpi.
        assert_eq!(png_size(&pngs[0]), (794, 1123));

        let opts = PngOptions {
            dpi: 72.0,
            pages: Some(vec![-1]),
            ..PngOptions::default()
        };
        let pngs = pdf_to_png(&doc, &opts).unwrap();
        assert_eq!(pngs.len(), 1);
        assert_eq!(png_size(&pngs[0]), (596, 842));
    }

    #[test]
    fn rejects_bad_options() {
        let doc = pdf("<p>x</p>", "A4");
        for dpi in [0.0, -1.0, f32::NAN, 5000.0] {
            let o = PngOptions {
                dpi,
                ..PngOptions::default()
            };
            assert!(pdf_to_png(&doc, &o).is_err(), "dpi {dpi}");
        }
        for pages in [vec![1], vec![-2]] {
            let o = PngOptions {
                pages: Some(pages),
                ..PngOptions::default()
            };
            assert!(pdf_to_png(&doc, &o).is_err());
        }
        // 2400 dpi A4 is ~19 800 px tall: over the limit.
        let o = PngOptions {
            dpi: 2400.0,
            ..PngOptions::default()
        };
        assert!(pdf_to_png(&doc, &o).is_err());
    }

    #[test]
    fn resolves_page_indices() {
        assert_eq!(resolve_pages(None, 3).unwrap(), vec![0, 1, 2]);
        assert_eq!(resolve_pages(Some(&[2, 0, -1]), 3).unwrap(), vec![2, 0, 2]);
        assert!(resolve_pages(Some(&[3]), 3).is_err());
        assert!(resolve_pages(Some(&[-4]), 3).is_err());
    }
}
