mod display_list;
mod font_subsetter;
mod painter;
mod pdf;
mod sandbox;

use ferropdf_core::{PageConfig, PageMargins, PageSize, RenderWarning};
pub use ferropdf_layout::FontDatabase;

/// Default upper bound on HTML input size (10 MiB). Renders larger than this
/// are rejected early to limit memory amplification under adversarial input.
pub const DEFAULT_MAX_HTML_BYTES: usize = 10 * 1024 * 1024;

/// Rendering options passed from Python bindings.
pub struct RenderOptions {
    pub page_size: String,
    pub margin: String,
    /// Directory under which `<img src>`, `<link href>`, and `@font-face url()` resolve.
    /// When `None`, all local-filesystem reads are refused (only `data:` URIs work).
    pub base_url: Option<String>,
    pub title: Option<String>,
    pub author: Option<String>,
    /// Reject input HTML longer than this. Defaults to [`DEFAULT_MAX_HTML_BYTES`].
    pub max_html_bytes: Option<usize>,
}

/// Result of a render operation: PDF bytes + any non-fatal warnings.
pub struct RenderResult {
    pub pdf_bytes: Vec<u8>,
    pub warnings: Vec<RenderWarning>,
}

/// Main entry point: render HTML string to PDF bytes.
pub fn render(html: &str, opts: &RenderOptions) -> ferropdf_core::Result<Vec<u8>> {
    let font_db = FontDatabase::new();
    render_with_cache(html, opts, &font_db)
}

/// Render HTML to PDF with warnings, reusing a cached FontDatabase.
pub fn render_with_warnings(
    html: &str,
    opts: &RenderOptions,
    font_db: &FontDatabase,
) -> ferropdf_core::Result<RenderResult> {
    let mut warnings: Vec<RenderWarning> = Vec::new();

    // 0. Reject oversize input early so adversarial HTML can't amplify memory.
    let max_html = opts.max_html_bytes.unwrap_or(DEFAULT_MAX_HTML_BYTES);
    if html.len() > max_html {
        return Err(ferropdf_core::FerroError::Layout(format!(
            "HTML input ({} bytes) exceeds max_html_bytes ({})",
            html.len(),
            max_html
        )));
    }

    // 1. Parse HTML
    let parse_result = ferropdf_parse::parse(html)?;

    // 2. Parse stylesheets (UA + inline + external)
    let ua_css = ferropdf_parse::css::UA_CSS;
    let mut stylesheets = vec![];
    for css_text in &parse_result.inline_styles {
        if let Ok(sheet) = ferropdf_parse::parse_stylesheet(css_text) {
            stylesheets.push(sheet);
        }
    }

    // Load external stylesheets, sandboxed to base_url.
    for stylesheet_url in &parse_result.external_stylesheets {
        match sandbox::read_sandboxed(stylesheet_url, opts.base_url.as_deref()) {
            Ok(bytes) => match std::str::from_utf8(&bytes) {
                Ok(css_content) => match ferropdf_parse::parse_stylesheet(css_content) {
                    Ok(sheet) => stylesheets.push(sheet),
                    Err(e) => warnings.push(RenderWarning::StylesheetFailed {
                        path: stylesheet_url.clone(),
                        reason: e.to_string(),
                    }),
                },
                Err(e) => warnings.push(RenderWarning::StylesheetFailed {
                    path: stylesheet_url.clone(),
                    reason: format!("invalid UTF-8: {}", e),
                }),
            },
            Err(reason) => warnings.push(RenderWarning::StylesheetFailed {
                path: stylesheet_url.clone(),
                reason,
            }),
        }
    }

    // 3. Load @font-face custom fonts
    for sheet in &stylesheets {
        for ff in &sheet.font_faces {
            match load_font_face_data(&ff.src, opts.base_url.as_deref()) {
                Ok(Some(data)) => font_db.load_font_data(data),
                Ok(None) => {}
                Err(reason) => warnings.push(RenderWarning::StylesheetFailed {
                    path: ff.src.clone(),
                    reason: format!("@font-face: {}", reason),
                }),
            }
        }
    }

    // 4. Build page config
    let page_config = PageConfig {
        size: PageSize::from_str(&opts.page_size),
        margins: PageMargins::from_css_str(&opts.margin),
        orientation: ferropdf_core::Orientation::Portrait,
    };

    // 5. Resolve styles (all values resolved to pt)
    let styles = ferropdf_style::resolve(
        &parse_result.document,
        &stylesheets,
        ua_css,
        page_config.content_width_pt(),
    )?;

    // 6. Layout with Taffy (all in points typographiques)
    let layout_tree = ferropdf_layout::layout_with_fonts(
        &parse_result.document,
        &styles,
        page_config.content_width_pt(),
        page_config.content_height_pt(),
        font_db,
    )?;

    // 7. Paginate
    let pages = ferropdf_page::paginate(&layout_tree, &page_config)?;

    // 8. Build display lists
    let display_lists: Vec<_> = pages
        .iter()
        .map(|page| painter::paint_page(page, &page_config))
        .collect();

    // 9. Write PDF (reuse fontdb from cosmic-text — no second load_system_fonts)
    let db_guard = font_db.fontdb();
    let pdf_bytes = pdf::write_pdf(
        &display_lists,
        &page_config,
        opts,
        Some(db_guard.db()),
        &mut warnings,
    )?;

    Ok(RenderResult {
        pdf_bytes,
        warnings,
    })
}

/// Render HTML to PDF, reusing a cached FontDatabase for speed.
pub fn render_with_cache(
    html: &str,
    opts: &RenderOptions,
    font_db: &FontDatabase,
) -> ferropdf_core::Result<Vec<u8>> {
    let result = render_with_warnings(html, opts, font_db)?;
    Ok(result.pdf_bytes)
}

/// Load font data from a `@font-face src` value.
/// Returns:
///   - `Ok(Some(bytes))` — font loaded successfully (data URI, or sandboxed file).
///   - `Ok(None)` — `data:` URI with no parsable base64 payload.
///   - `Err(reason)` — sandboxed file path was refused (no base_url, traversal,
///     missing, or oversize). Caller turns this into a `RenderWarning`.
fn load_font_face_data(src: &str, base_url: Option<&str>) -> Result<Option<Vec<u8>>, String> {
    let src = src.trim();

    if let Some(after_data) = src.strip_prefix("data:") {
        if let Some(base64_idx) = after_data.find(";base64,") {
            let encoded = &after_data[base64_idx + 8..];
            use base64::Engine as B64Engine;
            return Ok(base64::engine::general_purpose::STANDARD
                .decode(encoded.trim())
                .ok());
        }
        return Ok(None);
    }

    sandbox::read_sandboxed(src, base_url).map(Some)
}
