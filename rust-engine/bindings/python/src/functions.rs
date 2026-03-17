//! Top-level Python functions exposed into the `fastpdf._engine` module.

use pyo3::prelude::*;
use pyo3::types::PyBytes;
use rayon::prelude::*;

use crate::types::{PdfDocument, RenderOptions};
use crate::engine::do_render;

/// Render an HTML string to a file on disk.
///
/// Parameters
/// ----------
/// html : str
/// output_path : str
/// options : RenderOptions, optional
#[pyfunction]
#[pyo3(signature = (html, output_path, options = None))]
pub fn render_html_to_pdf(
    py:          Python<'_>,
    html:        &str,
    output_path: &str,
    options:     Option<RenderOptions>,
) -> PyResult<()> {
    let opts = options.unwrap_or_default();
    let doc  = do_render(py, html, "", &opts)?;
    doc.save(output_path)
}

/// Render an HTML string and return the raw PDF bytes.
///
/// Parameters
/// ----------
/// html : str
/// options : RenderOptions, optional
#[pyfunction]
#[pyo3(signature = (html, options = None))]
pub fn render_html_to_pdf_bytes<'py>(
    py:      Python<'py>,
    html:    &str,
    options: Option<RenderOptions>,
) -> PyResult<Bound<'py, PyBytes>> {
    let opts = options.unwrap_or_default();
    let doc  = do_render(py, html, "", &opts)?;
    Ok(doc.get_bytes(py))
}

/// Render multiple HTML strings in parallel, returning one `PdfDocument` per item.
///
/// Parameters
/// ----------
/// items : list[str | (str, RenderOptions)]
#[pyfunction]
#[pyo3(signature = (items, default_options = None))]
pub fn batch_render(
    py:              Python<'_>,
    items:           Vec<(String, Option<RenderOptions>)>,
    default_options: Option<RenderOptions>,
) -> PyResult<Vec<PdfDocument>> {
    let default = default_options.unwrap_or_default();

    // Release GIL and parallelise with rayon
    let results: Vec<Result<Vec<u8>, String>> = py.allow_threads(|| {
        items.par_iter().map(|(html, opts)| {
            let config = opts.as_ref().unwrap_or(&default).to_engine_config();
            let html   = html.as_str();

            use ferropdf_layout::engine::LayoutEngine;
            use ferropdf_page::fragment::paginate;
            use ferropdf_parse::css::parser::parse_stylesheet;
            use ferropdf_parse::html::parser::parse_html;
            use ferropdf_render::{Painter, PdfRenderer};

            const UA_CSS: &str = include_str!(
                "../../../crates/ferropdf-parse/src/css/ua.css"
            );

            let doc      = parse_html(html).map_err(|e| e.to_string())?;
            let ua_sheet = parse_stylesheet(UA_CSS).map_err(|e| e.to_string())?;
            let mut sheets = vec![ua_sheet];
            for css in doc.extract_stylesheets() {
                if let Ok(s) = parse_stylesheet(&css) { sheets.push(s); }
            }

            let engine   = LayoutEngine::new(config.clone());
            let root_box = engine.layout(&doc, &sheets).map_err(|e| e.to_string())?;
            let pages    = paginate(&root_box, &config);
            let painter  = Painter::new(config.clone());
            let ops      = painter.paint_pages(&pages);
            PdfRenderer::new(config).render(&ops).map_err(|e| e.to_string())
        }).collect()
    });

    results.into_iter()
        .map(|r| r.map(|bytes| PdfDocument { bytes })
                  .map_err(|s| pyo3::exceptions::PyRuntimeError::new_err(s)))
        .collect()
}

/// Return the library version string.
#[pyfunction]
pub fn get_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
