mod display_list;
mod font_subsetter;
mod painter;
mod pdf;

use ferropdf_core::{PageConfig, PageMargins, PageSize, RenderWarning};
pub use ferropdf_layout::FontDatabase;

/// Rendering options passed from Python bindings.
pub struct RenderOptions {
    pub page_size: String,
    pub margin: String,
    pub base_url: Option<String>,
    pub title: Option<String>,
    pub author: Option<String>,
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

    // Load external stylesheets (local files only for v1)
    for stylesheet_url in &parse_result.external_stylesheets {
        if stylesheet_url.starts_with("http://") || stylesheet_url.starts_with("https://") {
            warnings.push(RenderWarning::StylesheetFailed {
                path: stylesheet_url.clone(),
                reason: "external HTTP stylesheets not supported".into(),
            });
            continue;
        }

        let path = if let Some(ref base) = opts.base_url {
            let base_dir = std::path::Path::new(base);
            let base_dir = if base_dir.is_file() {
                base_dir.parent().unwrap_or(base_dir)
            } else {
                base_dir
            };
            base_dir.join(stylesheet_url)
        } else {
            std::path::PathBuf::from(stylesheet_url)
        };

        match std::fs::read_to_string(&path) {
            Ok(css_content) => match ferropdf_parse::parse_stylesheet(&css_content) {
                Ok(sheet) => stylesheets.push(sheet),
                Err(e) => {
                    warnings.push(RenderWarning::StylesheetFailed {
                        path: path.display().to_string(),
                        reason: e.to_string(),
                    });
                }
            },
            Err(e) => {
                warnings.push(RenderWarning::StylesheetFailed {
                    path: path.display().to_string(),
                    reason: e.to_string(),
                });
            }
        }
    }

    // 3. Load @font-face custom fonts
    for sheet in &stylesheets {
        for ff in &sheet.font_faces {
            if let Some(data) = load_font_face_data(&ff.src, opts.base_url.as_deref()) {
                font_db.load_font_data(data);
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
    let pdf_bytes = pdf::write_pdf(&display_lists, &page_config, opts, Some(db_guard.db()))?;

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

/// Load font data from a @font-face src value.
/// Supports:
///   - File paths: url("path/to/font.ttf") → resolved against base_url
///   - Data URIs: data:font/ttf;base64,... or data:application/x-font-ttf;base64,...
fn load_font_face_data(src: &str, base_url: Option<&str>) -> Option<Vec<u8>> {
    let src = src.trim();

    // Data URI
    if let Some(after_data) = src.strip_prefix("data:") {
        // data:[<mediatype>][;base64],<data>
        if let Some(base64_idx) = after_data.find(";base64,") {
            let encoded = &after_data[base64_idx + 8..];
            use base64::Engine as B64Engine;
            return base64::engine::general_purpose::STANDARD
                .decode(encoded.trim())
                .ok();
        }
        return None;
    }

    // Strip file:// protocol prefix
    let src = src
        .strip_prefix("file:///")
        .map(|s| format!("/{}", s))
        .unwrap_or_else(|| src.strip_prefix("file://").unwrap_or(src).to_string());
    let src = src.trim();

    // File path — absolute paths are used directly, relative paths resolved against base_url
    let path = if std::path::Path::new(src).is_absolute() {
        std::path::PathBuf::from(src)
    } else if let Some(base) = base_url {
        let base_dir = std::path::Path::new(base);
        let base_dir = if base_dir.is_file() {
            base_dir.parent().unwrap_or(base_dir)
        } else {
            base_dir
        };
        base_dir.join(src)
    } else {
        std::path::PathBuf::from(src)
    };

    std::fs::read(&path).ok()
}
