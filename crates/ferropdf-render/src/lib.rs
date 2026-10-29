mod display_list;
mod font_subsetter;
mod images;
mod margin_boxes;
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
    // One font system for the whole render, checked out of the engine's pool:
    // concurrent renders never contend, and @font-face stays private to it.
    let mut fonts = font_db.session();

    // 0. Reject oversize input early so adversarial HTML can't amplify memory.
    let max_html = opts.max_html_bytes.unwrap_or(DEFAULT_MAX_HTML_BYTES);
    if html.len() > max_html {
        return Err(ferropdf_core::FerroError::Layout(format!(
            "HTML input ({} bytes) exceeds max_html_bytes ({})",
            html.len(),
            max_html
        )));
    }

    // 1. Parse HTML — keep `parse_result` mutable so we can synthesise
    // `::before`/`::after` text nodes into the document later.
    let mut parse_result = ferropdf_parse::parse(html)?;
    // Inline <svg> elements become <img src="data:image/svg+xml…">.
    images::inline_svgs_to_img(&mut parse_result.document);

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
                Ok(Some(data)) => fonts.load_font_data(data),
                Ok(None) => {}
                Err(reason) => warnings.push(RenderWarning::StylesheetFailed {
                    path: ff.src.clone(),
                    reason: format!("@font-face: {}", reason),
                }),
            }
        }
    }

    // 4. Build page config — @page { margin / size } overrides Options
    // when present (last @page rule wins, in source order across all sheets).
    let mut page_config = PageConfig {
        size: PageSize::from_str(&opts.page_size),
        margins: PageMargins::from_css_str(&opts.margin),
        orientation: ferropdf_core::Orientation::Portrait,
    };
    // Applied in source order, so `margin` then `margin-top` behaves as in CSS.
    for sheet in &stylesheets {
        for pr in &sheet.page_rules {
            // Page size and margins are document-wide here: per-page values
            // from `:first` / `:left` / `:right` are not supported yet.
            if pr.selector != ferropdf_parse::PageSelector::All {
                continue;
            }
            if let Some(s) = &pr.size {
                page_config.size = PageSize::from_str(s);
            }
            if let Some(m) = &pr.margin {
                page_config.margins = PageMargins::from_css_str(m);
            }
            for (side, value) in pr.margin_sides.iter().enumerate() {
                if let Some(v) = value {
                    page_config.margins.set_side(side, v);
                }
            }
        }
    }

    // 4b. Materialise `::before` / `::after` content as real text
    // nodes before the cascade runs. Failures are silent — a malformed
    // selector or non-literal `content` simply means no injection.
    {
        let ua_sheet = ferropdf_parse::parse_stylesheet(ua_css)?;
        let mut all_sheets: Vec<&ferropdf_parse::Stylesheet> =
            Vec::with_capacity(1 + stylesheets.len());
        all_sheets.push(&ua_sheet);
        all_sheets.extend(stylesheets.iter());
        ferropdf_style::pseudo_elements::inject_pseudo_content(
            &mut parse_result.document,
            &all_sheets,
        );
    }

    // 5. Resolve styles (all values resolved to pt)
    let mut styles = ferropdf_style::resolve(
        &parse_result.document,
        &stylesheets,
        ua_css,
        page_config.content_width_pt(),
    )?;

    // 5b. Replaced elements: <img width/height> attributes and intrinsic size.
    apply_image_sizes(
        &parse_result.document,
        &mut styles,
        opts.base_url.as_deref(),
    );

    // 6. Layout with Taffy (all in points typographiques)
    let layout_tree = ferropdf_layout::layout_with_font_system(
        &parse_result.document,
        &styles,
        page_config.content_width_pt(),
        page_config.content_height_pt(),
        fonts.font_system(),
    )?;

    // 7. Paginate
    let mut pages = ferropdf_page::paginate(&layout_tree, &page_config)?;

    // 7b. Margin boxes (running headers/footers, page counters) — needs the
    // final page count, so it runs after pagination.
    {
        let page_rules: Vec<&ferropdf_parse::PageRule> = stylesheets
            .iter()
            .flat_map(|s| s.page_rules.iter())
            .collect();
        let root_style = root_element_style(&parse_result.document, &styles);
        let running = layout_running_elements(
            &parse_result.document,
            &styles,
            page_config.content_width_pt(),
            fonts.font_system(),
        )?;
        margin_boxes::add_margin_boxes(
            &mut pages,
            &page_rules,
            &page_config,
            &root_style,
            &running,
            fonts.font_system(),
        );
    }

    // 8. Build display lists
    let display_lists: Vec<_> = pages
        .iter()
        .map(|page| painter::paint_page(page, &page_config))
        .collect();

    // 9. Write PDF (reuse fontdb from cosmic-text — no second load_system_fonts)
    let pdf_bytes = pdf::write_pdf(
        &display_lists,
        &page_config,
        opts,
        Some(fonts.db()),
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

/// Computed style of the root (`<html>`) element, from which `@page` margin
/// boxes inherit. Falls back to defaults for fragments without `<html>`.
fn root_element_style(
    doc: &ferropdf_core::Document,
    styles: &ferropdf_style::StyleTree,
) -> ferropdf_core::ComputedStyle {
    doc.get(doc.root())
        .children
        .iter()
        .find(|&&id| doc.get(id).tag_name.as_deref() == Some("html"))
        .and_then(|id| styles.get(id))
        .cloned()
        .unwrap_or_default()
}

/// Lay out every running element (`position: running(name)`), first
/// occurrence per name, at the content width.
fn layout_running_elements(
    doc: &ferropdf_core::Document,
    styles: &ferropdf_style::StyleTree,
    width: f32,
    fs: &mut cosmic_text::FontSystem,
) -> ferropdf_core::Result<std::collections::HashMap<String, ferropdf_core::LayoutBox>> {
    let mut out = std::collections::HashMap::new();
    let mut stack = vec![doc.root()];
    while let Some(id) = stack.pop() {
        if let Some(name) = styles.get(&id).and_then(|s| s.running.clone()) {
            if let std::collections::hash_map::Entry::Vacant(slot) = out.entry(name) {
                if let Some(b) = ferropdf_layout::layout_element(doc, styles, id, width, fs)? {
                    slot.insert(b);
                }
            }
            continue; // a running element's subtree belongs to it
        }
        // Depth-first, document order.
        stack.extend(doc.get(id).children.iter().rev());
    }
    Ok(out)
}

/// Size `<img>` elements like browsers do: CSS `width`/`height` win; otherwise
/// the `width`/`height` attributes (presentational hints, in CSS px or %);
/// otherwise the image's intrinsic size. The intrinsic ratio is kept as
/// `aspect_ratio` so a single given dimension (or `max-width`) scales the
/// other. Images that cannot be read keep their style (the PDF writer warns).
fn apply_image_sizes(
    doc: &ferropdf_core::Document,
    styles: &mut ferropdf_style::StyleTree,
    base_url: Option<&str>,
) {
    use ferropdf_core::Length;
    const PX: f32 = 0.75; // CSS px → pt

    let attr_len = |v: Option<&str>| -> Option<Length> {
        let v = v?.trim();
        if let Some(p) = v.strip_suffix('%') {
            return p.trim().parse::<f32>().ok().map(Length::Percent);
        }
        let v = v.strip_suffix("px").unwrap_or(v);
        v.parse::<f32>()
            .ok()
            .filter(|n| *n >= 0.0)
            .map(|n| Length::Pt(n * PX))
    };

    let mut stack = vec![doc.root()];
    while let Some(id) = stack.pop() {
        let node = doc.get(id);
        stack.extend(node.children.iter().copied());
        if node.tag_name.as_deref() != Some("img") {
            continue;
        }
        let Some(style) = styles.get_mut(&id) else {
            continue;
        };
        let unset = |l: &Length| matches!(l, Length::Auto | Length::None);
        if unset(&style.width) {
            if let Some(w) = attr_len(node.attr("width")) {
                style.width = w;
            }
        }
        if unset(&style.height) {
            if let Some(h) = attr_len(node.attr("height")) {
                style.height = h;
            }
        }
        let Some((iw, ih)) = node
            .attr("src")
            .and_then(|src| images::intrinsic_size(src, base_url))
        else {
            continue;
        };
        if iw <= 0.0 || ih <= 0.0 {
            continue;
        }
        let ratio = iw / ih;
        style.aspect_ratio = Some(ratio);
        match (&style.width, &style.height) {
            (w, h) if unset(w) && unset(h) => {
                style.width = Length::Pt(iw * PX);
            }
            (w, Length::Pt(h)) if unset(w) => style.width = Length::Pt(h * ratio),
            _ => {}
        }
    }
}
