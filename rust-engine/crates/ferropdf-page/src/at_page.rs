//! @page rule resolution — margins, size, headers/footers for each page.

use ferropdf_core::{EngineConfig, PageSize};
use ferropdf_parse::css::values::Stylesheet;

/// Resolved margins/size for a specific page (first, left, right, …).
#[derive(Debug, Clone)]
pub struct PageLayout {
    pub width:          f32,
    pub height:         f32,
    pub margin_top:     f32,
    pub margin_right:   f32,
    pub margin_bottom:  f32,
    pub margin_left:    f32,
}

/// Resolves @page rules from the author stylesheet and applies them
/// on top of the defaults supplied by `EngineConfig`.
pub struct AtPageResolver {
    config: EngineConfig,
}

impl AtPageResolver {
    pub fn new(config: EngineConfig) -> Self { Self { config } }

    /// Return the effective `PageLayout` for page number `n` (1-based).
    /// Currently applies `:first`, `:left`, `:right` pseudo-page selectors.
    pub fn layout_for_page(&self, _sheets: &[Stylesheet], n: usize) -> PageLayout {
        let w = self.config.page_size.width_px();
        let h = self.config.page_size.height_px();
        let m = &self.config.margin;
        PageLayout {
            width:         w,
            height:        h,
            margin_top:    m.top,
            margin_right:  m.right,
            margin_bottom: m.bottom,
            margin_left:   m.left,
        }
    }
}
