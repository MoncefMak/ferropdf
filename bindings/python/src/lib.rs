use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyList, PyTuple};
use std::sync::{Arc, OnceLock};

/// System fonts are discovered once per process and shared by every `Engine`
/// and by the one-shot functions (`from_html`, `from_file`, `write_pdf`).
/// Each render checks out its own font system from the pool inside, so
/// sharing does not serialise threads, and `@font-face` stays per document.
static SHARED_FONTS: OnceLock<Arc<ferropdf_render::FontDatabase>> = OnceLock::new();

fn shared_fonts() -> Arc<ferropdf_render::FontDatabase> {
    SHARED_FONTS
        .get_or_init(|| Arc::new(ferropdf_render::FontDatabase::new()))
        .clone()
}

pyo3::create_exception!(ferropdf, FerroError, pyo3::exceptions::PyRuntimeError);
pyo3::create_exception!(ferropdf, ParseError, FerroError);
pyo3::create_exception!(ferropdf, LayoutError, FerroError);
pyo3::create_exception!(ferropdf, FontError, FerroError);
pyo3::create_exception!(ferropdf, RenderError, FerroError);

fn value_error(msg: String) -> PyErr {
    PyErr::new::<pyo3::exceptions::PyValueError, _>(msg)
}

/// `bytes` as is, `str` as UTF-8.
fn bytes_or_str(obj: &Bound<'_, PyAny>, what: &str) -> PyResult<Vec<u8>> {
    if let Ok(s) = obj.extract::<String>() {
        return Ok(s.into_bytes());
    }
    obj.extract::<Vec<u8>>().map_err(|_| {
        PyErr::new::<pyo3::exceptions::PyTypeError, _>(format!("{what} must be bytes or str"))
    })
}

/// A file embedded in the PDF, e.g. the source data of a report.
#[pyclass(name = "Attachment", from_py_object)]
#[derive(Clone, Debug)]
pub struct PyAttachment {
    #[pyo3(get)]
    pub name: String,
    pub data: Vec<u8>,
    #[pyo3(get)]
    pub mime_type: Option<String>,
    #[pyo3(get)]
    pub description: Option<String>,
    pub relationship: ferropdf_render::AfRelationship,
}

#[pymethods]
impl PyAttachment {
    #[new]
    #[pyo3(signature = (name, data, mime_type = None, description = None, relationship = "Unspecified"))]
    fn new(
        name: String,
        data: &Bound<'_, PyAny>,
        mime_type: Option<String>,
        description: Option<String>,
        relationship: &str,
    ) -> PyResult<Self> {
        Ok(Self {
            name,
            data: bytes_or_str(data, "data")?,
            mime_type,
            description,
            relationship: ferropdf_render::AfRelationship::parse(relationship)
                .map_err(value_error)?,
        })
    }

    #[getter]
    fn data<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.data)
    }

    #[getter]
    fn relationship(&self) -> String {
        format!("{:?}", self.relationship)
    }

    fn __repr__(&self) -> String {
        format!(
            "Attachment(name={:?}, {} bytes, relationship='{:?}')",
            self.name,
            self.data.len(),
            self.relationship
        )
    }
}

impl From<PyAttachment> for ferropdf_render::Attachment {
    fn from(a: PyAttachment) -> Self {
        ferropdf_render::Attachment {
            name: a.name,
            data: a.data,
            mime_type: a.mime_type,
            description: a.description,
            relationship: a.relationship,
        }
    }
}

#[pyclass(name = "Options", from_py_object)]
#[derive(Clone, Debug)]
pub struct PyOptions {
    pub page_size: String,
    pub margin: String,
    pub base_url: Option<String>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub max_html_bytes: Option<usize>,
    pub viewport_width: Option<f32>,
    pub pdf_variant: ferropdf_render::PdfVariant,
    pub attachments: Vec<PyAttachment>,
    pub factur_x: Option<ferropdf_render::FacturX>,
    pub tagged: bool,
    pub pdf_ua: bool,
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
        viewport_width = None,
        pdf_variant = None,
        attachments = None,
        factur_x = None,
        factur_x_profile = None,
        tagged = false,
        pdf_ua = false,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        page_size: &str,
        margin: &str,
        base_url: Option<String>,
        title: Option<String>,
        author: Option<String>,
        max_html_bytes: Option<usize>,
        viewport_width: Option<f32>,
        pdf_variant: Option<&str>,
        attachments: Option<Vec<PyAttachment>>,
        factur_x: Option<&Bound<'_, PyAny>>,
        factur_x_profile: Option<&str>,
        tagged: bool,
        pdf_ua: bool,
    ) -> PyResult<Self> {
        let pdf_variant = match pdf_variant {
            Some(v) => ferropdf_render::PdfVariant::parse(v).map_err(value_error)?,
            None => ferropdf_render::PdfVariant::Standard,
        };
        let profile = factur_x_profile
            .map(ferropdf_render::FacturXProfile::parse)
            .transpose()
            .map_err(value_error)?;
        if profile.is_some() && factur_x.is_none() {
            return Err(value_error(
                "factur_x_profile is set but factur_x (the XML invoice) is not".into(),
            ));
        }
        let factur_x = match factur_x {
            Some(obj) => Some(ferropdf_render::FacturX {
                xml: bytes_or_str(obj, "factur_x")?,
                profile,
            }),
            None => None,
        };
        // Surface option conflicts now rather than at render time.
        let attachments = attachments.unwrap_or_default();
        let opts = Self {
            page_size: page_size.to_string(),
            margin: margin.to_string(),
            base_url,
            title,
            author,
            max_html_bytes,
            viewport_width,
            pdf_variant,
            attachments,
            factur_x,
            tagged,
            pdf_ua,
        };
        let render_opts: ferropdf_render::RenderOptions = opts.clone().into();
        ferropdf_render::validate_options(&render_opts).map_err(|e| value_error(e.to_string()))?;
        Ok(opts)
    }

    fn __repr__(&self) -> String {
        let mut s = format!(
            "Options(page_size='{}', margin='{}'",
            self.page_size, self.margin
        );
        let variant = match (self.pdf_variant, self.factur_x.is_some()) {
            (ferropdf_render::PdfVariant::Standard, true) => ferropdf_render::PdfVariant::PdfA3b,
            (v, _) => v,
        };
        match variant {
            ferropdf_render::PdfVariant::Standard => {}
            ferropdf_render::PdfVariant::PdfA2b => s.push_str(", pdf_variant='pdf/a-2b'"),
            ferropdf_render::PdfVariant::PdfA3b => s.push_str(", pdf_variant='pdf/a-3b'"),
        }
        if !self.attachments.is_empty() {
            s.push_str(&format!(", attachments={}", self.attachments.len()));
        }
        if self.factur_x.is_some() {
            s.push_str(", factur_x=...");
        }
        if self.pdf_ua {
            s.push_str(", pdf_ua=True");
        } else if self.tagged {
            s.push_str(", tagged=True");
        }
        s.push(')');
        s
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
            viewport_width: opts.viewport_width,
            pdf_variant: opts.pdf_variant,
            attachments: opts.attachments.into_iter().map(Into::into).collect(),
            factur_x: opts.factur_x,
            tagged: opts.tagged,
            pdf_ua: opts.pdf_ua,
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
        viewport_width: None,
        pdf_variant: ferropdf_render::PdfVariant::Standard,
        attachments: Vec::new(),
        factur_x: None,
        tagged: false,
        pdf_ua: false,
    }
}

fn warnings_to_strings(ws: &[ferropdf_core::RenderWarning]) -> Vec<String> {
    ws.iter().map(|w| w.to_string()).collect()
}

#[pyclass(name = "Engine")]
pub struct PyEngine {
    options: PyOptions,
}

#[pymethods]
impl PyEngine {
    #[new]
    #[pyo3(signature = (options = None))]
    fn new(options: Option<PyOptions>) -> Self {
        Self {
            options: options.unwrap_or_else(default_options),
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

    /// Render HTML to PNG images, one `bytes` object per page.
    ///
    /// `dpi=96` gives one image pixel per CSS pixel. `pages` selects pages by
    /// zero-based index (an int or a sequence; negative counts from the end).
    /// The images are rasterized from the PDF, so they match it exactly.
    #[pyo3(signature = (html, dpi = 96.0, pages = None, transparent = false))]
    fn render_png<'py>(
        &self,
        py: Python<'py>,
        html: &str,
        dpi: f32,
        pages: Option<&Bound<'py, PyAny>>,
        transparent: bool,
    ) -> PyResult<Bound<'py, PyList>> {
        let pages = match pages {
            None => None,
            Some(obj) => Some(match obj.extract::<i64>() {
                Ok(i) => vec![i],
                Err(_) => obj.extract::<Vec<i64>>().map_err(|_| {
                    PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                        "pages must be an int or a sequence of ints",
                    )
                })?,
            }),
        };
        let png_opts = ferropdf_render::PngOptions {
            dpi,
            pages,
            transparent,
        };
        ferropdf_render::validate_png_options(&png_opts).map_err(png_err)?;
        let (pdf, warnings) = self.render_inner(py, html)?;
        let images = py
            .detach(move || ferropdf_render::pdf_to_png(&pdf, &png_opts))
            .map_err(png_err)?;
        emit_warnings(py, &warnings);
        PyList::new(py, images.iter().map(|png| PyBytes::new(py, png)))
    }
}

fn png_err(e: ferropdf_render::PngError) -> PyErr {
    use ferropdf_render::PngError::*;
    let msg = e.to_string();
    match e {
        InvalidOption(_) => PyErr::new::<pyo3::exceptions::PyValueError, _>(msg),
        PageOutOfRange(_) => PyErr::new::<pyo3::exceptions::PyIndexError, _>(msg),
        Render(_) => PyErr::new::<RenderError, _>(msg),
    }
}

impl PyEngine {
    /// Shared rendering core. Releases the GIL during the actual work and
    /// returns the PDF bytes plus stringified warnings.
    fn render_inner(&self, py: Python<'_>, html: &str) -> PyResult<(Vec<u8>, Vec<String>)> {
        let html = html.to_string();
        let opts = self.options.clone();

        // The first call loads system fonts (~100 ms): do it with the GIL released too.
        let result = py.detach(move || {
            let font_db = shared_fonts();
            ferropdf_render::render_with_warnings(&html, &opts.into(), &font_db)
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
    m.add_class::<PyAttachment>()?;
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
