//! PyO3 Python bindings for ferropdf.
//!
//! Exposes the pipeline as `fastpdf._engine`.

pub mod engine;
pub mod errors;
pub mod functions;
pub mod types;

use pyo3::prelude::*;

use types::{PdfDocument, PdfEngine, RenderOptions};
use functions::{batch_render, get_version, render_html_to_pdf, render_html_to_pdf_bytes};

#[pymodule]
#[pyo3(name = "_engine")]
fn _engine(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<RenderOptions>()?;
    m.add_class::<PdfEngine>()?;
    m.add_class::<PdfDocument>()?;
    m.add_function(wrap_pyfunction!(render_html_to_pdf, m)?)?;
    m.add_function(wrap_pyfunction!(render_html_to_pdf_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(batch_render, m)?)?;
    m.add_function(wrap_pyfunction!(get_version, m)?)?;
    Ok(())
}
