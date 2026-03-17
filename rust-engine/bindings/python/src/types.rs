//! Python-exposed types: `RenderOptions`, `PdfEngine`, `PdfDocument`.

use pyo3::prelude::*;

use ferropdf_core::{Edge, EngineConfig, PageSize};

// ─── RenderOptions ────────────────────────────────────────────────────────────

/// Options controlling how an HTML document is rendered to PDF.
#[pyclass]
#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// Page size identifier: "a4", "letter", "legal", or custom e.g. "210x297".
    #[pyo3(get, set)]
    pub page_size: String,
    /// "portrait" or "landscape".
    #[pyo3(get, set)]
    pub orientation: String,
    /// Margins in mm [top, right, bottom, left].
    #[pyo3(get, set)]
    pub margins: Vec<f64>,
    /// Document title embedded in PDF metadata.
    #[pyo3(get, set)]
    pub title: String,
    /// Document author embedded in PDF metadata.
    #[pyo3(get, set)]
    pub author: String,
    /// Resolve Tailwind CSS utility classes before rendering.
    #[pyo3(get, set)]
    pub tailwind: bool,
    /// Base path for resolving relative URLs/file references.
    #[pyo3(get, set)]
    pub base_path: String,
    /// HTML snippet inserted at the top of each page.
    #[pyo3(get, set)]
    pub header_html: String,
    /// HTML snippet inserted at the bottom of each page.
    #[pyo3(get, set)]
    pub footer_html: String,
}

#[pymethods]
impl RenderOptions {
    #[new]
    #[pyo3(signature = (
        page_size   = "a4".to_string(),
        orientation = "portrait".to_string(),
        margins     = vec![20.0, 20.0, 20.0, 20.0],
        title       = String::new(),
        author      = String::new(),
        tailwind    = false,
        base_path   = String::new(),
        header_html = String::new(),
        footer_html = String::new(),
    ))]
    pub fn new(
        page_size:   String,
        orientation: String,
        margins:     Vec<f64>,
        title:       String,
        author:      String,
        tailwind:    bool,
        base_path:   String,
        header_html: String,
        footer_html: String,
    ) -> Self {
        Self {
            page_size, orientation, margins, title, author,
            tailwind, base_path, header_html, footer_html,
        }
    }

    pub fn __repr__(&self) -> String {
        format!(
            "RenderOptions(page_size={:?}, orientation={:?})",
            self.page_size, self.orientation
        )
    }
}

impl RenderOptions {
    /// Convert into the core `EngineConfig`.
    pub fn to_engine_config(&self) -> EngineConfig {
        let page_size = parse_page_size(&self.page_size, &self.orientation);

        let mm_to_px = |mm: f64| (mm * 96.0 / 25.4) as f32;
        let (mt, mr, mb, ml) = match self.margins.as_slice() {
            [t, r, b, l, ..] => (*t, *r, *b, *l),
            [t, r, b]        => (*t, *r, *b, *r),
            [t, r]           => (*t, *r, *t, *r),
            [a]              => (*a, *a, *a, *a),
            []               => (20.0, 20.0, 20.0, 20.0),
        };

        EngineConfig {
            page_size,
            margin: Edge {
                top:    mm_to_px(mt),
                right:  mm_to_px(mr),
                bottom: mm_to_px(mb),
                left:   mm_to_px(ml),
            },
            base_url:    if self.base_path.is_empty() { None } else { Some(self.base_path.clone()) },
            font_dirs:   Vec::new(),
            title:       if self.title.is_empty()  { None } else { Some(self.title.clone()) },
            author:      if self.author.is_empty() { None } else { Some(self.author.clone()) },
            tailwind:    self.tailwind,
            header_html: if self.header_html.is_empty() { None } else { Some(self.header_html.clone()) },
            footer_html: if self.footer_html.is_empty() { None } else { Some(self.footer_html.clone()) },
        }
    }
}

fn parse_page_size(name: &str, orientation: &str) -> PageSize {
    let landscape = orientation.eq_ignore_ascii_case("landscape");
    let ps = match name.to_lowercase().as_str() {
        "a4"     => PageSize::a4(),
        "letter" => PageSize::letter(),
        "legal"  => PageSize::legal(),
        "a3"     => PageSize::a3(),
        "a5"     => PageSize::a5(),
        custom  => {
            // Try "WxH" in mm
            if let Some((w, h)) = custom.split_once('x') {
                let w: f32 = w.trim().parse().unwrap_or(210.0);
                let h: f32 = h.trim().parse().unwrap_or(297.0);
                if landscape {
                    return PageSize { width_mm: h, height_mm: w };
                }
                return PageSize { width_mm: w, height_mm: h };
            }
            PageSize::a4()
        }
    };
    if landscape { ps.landscape() } else { ps }
}

// ─── PdfDocument ──────────────────────────────────────────────────────────────

/// An in-memory PDF document returned by the engine.
#[pyclass]
#[derive(Debug, Clone)]
pub struct PdfDocument {
    pub bytes: Vec<u8>,
}

#[pymethods]
impl PdfDocument {
    /// Return the raw PDF bytes.
    pub fn get_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, pyo3::types::PyBytes> {
        pyo3::types::PyBytes::new(py, &self.bytes)
    }

    /// Alias for get_bytes() — matches the Python wrapper API.
    pub fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, pyo3::types::PyBytes> {
        pyo3::types::PyBytes::new(py, &self.bytes)
    }

    /// Approximate page count (counted from PDF page objects).
    #[getter]
    pub fn page_count(&self) -> usize {
        // Count occurrences of "/Type /Page\n" or "/Type/Page" as a proxy.
        let needle = b"/Type /Page";
        let needle2 = b"/Type/Page";
        let mut count = 0usize;
        let b = &self.bytes;
        for i in 0..b.len().saturating_sub(needle.len()) {
            if b[i..].starts_with(needle) || b[i..].starts_with(needle2) {
                count += 1;
            }
        }
        count.max(1)
    }

    /// Save the PDF to a file.
    pub fn save(&self, path: &str) -> PyResult<()> {
        std::fs::write(path, &self.bytes)
            .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))
    }

    /// Number of bytes.
    pub fn __len__(&self) -> usize { self.bytes.len() }
    pub fn __repr__(&self) -> String { format!("PdfDocument({} bytes)", self.bytes.len()) }
}

// ─── PdfEngine ────────────────────────────────────────────────────────────────

/// Stateful rendering engine that can be reused across multiple renders.
#[pyclass]
pub struct PdfEngine {
    pub options: RenderOptions,
}

#[pymethods]
impl PdfEngine {
    #[new]
    #[pyo3(signature = (options = None))]
    pub fn new(options: Option<RenderOptions>) -> Self {
        Self { options: options.unwrap_or_default() }
    }

    /// Render HTML source to a `PdfDocument`.
    #[pyo3(signature = (html, css = None, options = None))]
    pub fn render(&self, py: Python<'_>, html: &str, css: Option<&str>, options: Option<&RenderOptions>) -> PyResult<PdfDocument> {
        let opts = options.unwrap_or(&self.options);
        crate::engine::do_render(py, html, css.unwrap_or(""), opts)
    }

    /// Render HTML and return raw bytes.
    #[pyo3(signature = (html, css = None, options = None))]
    pub fn render_bytes<'py>(&self, py: Python<'py>, html: &str, css: Option<&str>, options: Option<&RenderOptions>) -> PyResult<Bound<'py, pyo3::types::PyBytes>> {
        let opts = options.unwrap_or(&self.options);
        let doc = crate::engine::do_render(py, html, css.unwrap_or(""), opts)?;
        Ok(doc.get_bytes(py))
    }

    pub fn __repr__(&self) -> String {
        format!("PdfEngine(page_size={:?})", self.options.page_size)
    }
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self::new(
            "a4".to_string(),
            "portrait".to_string(),
            vec![20.0, 20.0, 20.0, 20.0],
            String::new(),
            String::new(),
            false,
            String::new(),
            String::new(),
            String::new(),
        )
    }
}
