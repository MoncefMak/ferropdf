use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyList, PyTuple};
use std::sync::OnceLock;

pyo3::create_exception!(ferropdf, FerroError, pyo3::exceptions::PyRuntimeError);
pyo3::create_exception!(ferropdf, ParseError, FerroError);
pyo3::create_exception!(ferropdf, LayoutError, FerroError);
pyo3::create_exception!(ferropdf, FontError, FerroError);
pyo3::create_exception!(ferropdf, RenderError, FerroError);

#[pyclass(name = "Options")]
#[derive(Clone, Debug)]
pub struct PyOptions {
    pub page_size: String,
    pub margin: String,
    pub base_url: Option<String>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub max_html_bytes: Option<usize>,
}

#[pymethods]
impl PyOptions {
    #[new]
    #[pyo3(signature = (
        page_size = "A4",
        margin    = "20mm",
        base_url  = None,
        title     = None,
        author    = None,
        max_html_bytes = None,
    ))]
    fn new(
        page_size: &str,
        margin: &str,
        base_url: Option<String>,
        title: Option<String>,
        author: Option<String>,
        max_html_bytes: Option<usize>,
    ) -> Self {
        Self {
            page_size: page_size.to_string(),
            margin: margin.to_string(),
            base_url,
            title,
            author,
            max_html_bytes,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Options(page_size='{}', margin='{}')",
            self.page_size, self.margin
        )
    }
}

impl From<PyOptions> for ferropdf_render::RenderOptions {
    fn from(opts: PyOptions) -> Self {
        ferropdf_render::RenderOptions {
            page_size: opts.page_size,
            margin: opts.margin,
            base_url: opts.base_url,
            title: opts.title,
            author: opts.author,
            max_html_bytes: opts.max_html_bytes,
        }
    }
}

fn default_options() -> PyOptions {
    PyOptions {
        page_size: "A4".to_string(),
        margin: "20mm".to_string(),
        base_url: None,
        title: None,
        author: None,
        max_html_bytes: None,
    }
}

fn warnings_to_strings(ws: &[ferropdf_core::RenderWarning]) -> Vec<String> {
    ws.iter().map(|w| w.to_string()).collect()
}

#[pyclass(name = "Engine")]
pub struct PyEngine {
    options: PyOptions,
    font_db: OnceLock<ferropdf_render::FontDatabase>,
}

#[pymethods]
impl PyEngine {
    #[new]
    #[pyo3(signature = (options = None))]
    fn new(options: Option<PyOptions>) -> Self {
        Self {
            options: options.unwrap_or_else(default_options),
            font_db: OnceLock::new(),
        }
    }

    /// Render HTML to PDF bytes. Recoverable issues (missing image, failed
    /// stylesheet) are emitted via Python's `warnings` module and otherwise
    /// silently swallowed — call `render_with_warnings` to inspect them.
    fn render<'py>(&self, py: Python<'py>, html: &str) -> PyResult<Bound<'py, PyBytes>> {
        let (bytes, warnings) = self.render_inner(py, html)?;
        emit_warnings(py, &warnings);
        Ok(PyBytes::new(py, &bytes))
    }

    fn render_file<'py>(&self, py: Python<'py>, path: &str) -> PyResult<Bound<'py, PyBytes>> {
        let html = std::fs::read_to_string(path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(e.to_string()))?;
        self.render(py, &html)
    }

    /// Render and return a `(bytes, warnings)` tuple where `warnings` is a list
    /// of human-readable strings describing recoverable issues.
    fn render_with_warnings<'py>(
        &self,
        py: Python<'py>,
        html: &str,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let (bytes, warnings) = self.render_inner(py, html)?;
        let py_bytes = PyBytes::new(py, &bytes);
        let py_warnings = PyList::new(py, &warnings)?;
        PyTuple::new(py, [py_bytes.into_any(), py_warnings.into_any()])
    }
}

impl PyEngine {
    /// Shared rendering core. Releases the GIL during the actual work and
    /// returns the PDF bytes plus stringified warnings.
    fn render_inner(&self, py: Python<'_>, html: &str) -> PyResult<(Vec<u8>, Vec<String>)> {
        let html = html.to_string();
        let opts = self.options.clone();

        let font_db = self.font_db.get_or_init(ferropdf_render::FontDatabase::new);

        let result = py.allow_threads(move || {
            ferropdf_render::render_with_warnings(&html, &opts.into(), font_db)
        });

        match result {
            Ok(r) => Ok((r.pdf_bytes, warnings_to_strings(&r.warnings))),
            Err(e) => Err(to_py_err(e)),
        }
    }
}

fn emit_warnings(py: Python<'_>, warnings: &[String]) {
    if warnings.is_empty() {
        return;
    }
    let warnings_module = match py.import("warnings") {
        Ok(m) => m,
        Err(_) => return,
    };
    let warn_fn = match warnings_module.getattr("warn") {
        Ok(f) => f,
        Err(_) => return,
    };
    for w in warnings {
        // Best-effort — never fail the render because of a warning emission.
        let _ = warn_fn.call1((w.as_str(),));
    }
}

#[pyfunction]
#[pyo3(signature = (html, base_url = None, options = None))]
fn from_html<'py>(
    py: Python<'py>,
    html: &str,
    base_url: Option<&str>,
    options: Option<PyOptions>,
) -> PyResult<Bound<'py, PyBytes>> {
    let mut opts = options.unwrap_or_else(default_options);
    if let Some(u) = base_url {
        opts.base_url = Some(u.to_string());
    }
    PyEngine::new(Some(opts)).render(py, html)
}

#[pyfunction]
#[pyo3(signature = (path, options = None))]
fn from_file<'py>(
    py: Python<'py>,
    path: &str,
    options: Option<PyOptions>,
) -> PyResult<Bound<'py, PyBytes>> {
    let html = std::fs::read_to_string(path)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(e.to_string()))?;
    from_html(py, &html, None, options)
}

#[pyfunction]
#[pyo3(signature = (html, output_path, base_url = None, options = None))]
fn write_pdf(
    py: Python<'_>,
    html: &str,
    output_path: &str,
    base_url: Option<&str>,
    options: Option<PyOptions>,
) -> PyResult<()> {
    let bytes = from_html(py, html, base_url, options)?;
    std::fs::write(output_path, bytes.as_bytes())
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(e.to_string()))
}

#[pymodule]
fn _ferropdf(py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyOptions>()?;
    m.add_class::<PyEngine>()?;
    m.add_function(wrap_pyfunction!(from_html, m)?)?;
    m.add_function(wrap_pyfunction!(from_file, m)?)?;
    m.add_function(wrap_pyfunction!(write_pdf, m)?)?;
    m.add("FerroError", py.get_type::<FerroError>())?;
    m.add("ParseError", py.get_type::<ParseError>())?;
    m.add("LayoutError", py.get_type::<LayoutError>())?;
    m.add("FontError", py.get_type::<FontError>())?;
    m.add("RenderError", py.get_type::<RenderError>())?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}

fn to_py_err(e: ferropdf_core::FerroError) -> PyErr {
    use ferropdf_core::FerroError::*;
    match e {
        HtmlParse(m) | CssParse(m) => PyErr::new::<ParseError, _>(m),
        Layout(m) => PyErr::new::<LayoutError, _>(m),
        Font(m) => PyErr::new::<FontError, _>(m),
        PdfWrite(m) => PyErr::new::<RenderError, _>(m),
        Io(e) => PyErr::new::<pyo3::exceptions::PyIOError, _>(e.to_string()),
        other => PyErr::new::<FerroError, _>(other.to_string()),
    }
}
