//! Core rendering pipeline: HTML → parse → layout → paginate → paint → PDF.

use pyo3::prelude::*;

use ferropdf_layout::engine::LayoutEngine;
use ferropdf_page::fragment::paginate;
use ferropdf_parse::css::parser::parse_stylesheet;
use ferropdf_parse::html::parser::parse_html;
use ferropdf_render::{Painter, PdfRenderer};

use crate::types::{PdfDocument, RenderOptions};

const UA_CSS: &str = include_str!(
    "../../../crates/ferropdf-parse/src/css/ua.css"
);

/// Execute the full rendering pipeline, releasing the GIL during heavy work.
pub fn do_render(
    py:      Python<'_>,
    html:    &str,
    css:     &str,
    options: &RenderOptions,
) -> PyResult<PdfDocument> {
    let html       = html.to_string();
    let extra_css  = css.to_string();
    let config     = options.to_engine_config();

    let bytes = py.allow_threads(|| {
        // 1) Parse HTML
        let doc = parse_html(&html).map_err(|e| e.to_string())?;

        // 2) Parse CSS (UA defaults + inline <style> tags + extra CSS)
        let ua_sheet = parse_stylesheet(UA_CSS).map_err(|e| e.to_string())?;
        let mut sheets = vec![ua_sheet];
        for css in doc.extract_stylesheets() {
            if let Ok(sheet) = parse_stylesheet(&css) {
                sheets.push(sheet);
            }
        }
        if !extra_css.is_empty() {
            if let Ok(sheet) = parse_stylesheet(&extra_css) {
                sheets.push(sheet);
            }
        }

        // 3) Layout
        let engine   = LayoutEngine::new(config.clone());
        let root_box = engine.layout(&doc, &sheets).map_err(|e| e.to_string())?;

        // 4) Paginate
        let pages = paginate(&root_box, &config);

        // 5) Paint → display list
        let painter     = Painter::new(config.clone());
        let display_ops = painter.paint_pages(&pages);

        // 6) Render → PDF bytes
        let renderer = PdfRenderer::new(config);
        renderer.render(&display_ops).map_err(|e| e.to_string())
    }).map_err(|s: String| pyo3::exceptions::PyRuntimeError::new_err(s))?;

    Ok(PdfDocument { bytes })
}
