use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping};
use taffy::prelude::*;

/// Context attached to text leaf nodes in the Taffy tree.
/// Stores the info cosmic-text needs to measure the text on demand.
#[derive(Debug, Clone)]
pub struct TextContext {
    pub text: String,
    pub font_size: f32,
    pub line_height: f32,
    pub font_family: String,
    pub bold: bool,
    pub italic: bool,
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
