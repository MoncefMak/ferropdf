//! Image sources shared by layout sizing and the PDF writer: data URIs or
//! sandboxed files, raster formats via `image`, SVG via `usvg` (embedded as
//! vector graphics by `svg2pdf`), and inline `<svg>` elements turned into
//! `<img>` with an SVG data URI.

use base64::Engine as _;
use ferropdf_core::{Document, NodeId};
use std::sync::Arc;

/// Raw bytes of an image source.
pub(crate) fn source_bytes(src: &str, base_url: Option<&str>) -> Result<Vec<u8>, String> {
    if let Some(rest) = src.strip_prefix("data:") {
        let comma = rest
            .find(',')
            .ok_or_else(|| "invalid data URI: no comma".to_string())?;
        let (meta, payload) = (&rest[..comma], &rest[comma + 1..]);
        if meta.contains(";base64") {
            base64::engine::general_purpose::STANDARD
                .decode(payload.trim())
                .map_err(|e| format!("base64 decode error: {e}"))
        } else {
            Ok(percent_decode(payload).into_bytes())
        }
    } else {
        crate::sandbox::read_sandboxed(src, base_url)
    }
}

/// Minimal `%XX` decoding for non-base64 data URIs (`data:image/svg+xml,%3Csvg…`).
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// SVG by MIME type, extension or content sniffing.
pub(crate) fn is_svg(src: &str, data: &[u8]) -> bool {
    if src.starts_with("data:image/svg+xml") {
        return true;
    }
    let path = src.split(['?', '#']).next().unwrap_or(src);
    if path.to_ascii_lowercase().ends_with(".svg") {
        return true;
    }
    let head = String::from_utf8_lossy(&data[..data.len().min(512)]);
    let head = head.trim_start_matches('\u{feff}').trim_start();
    head.starts_with("<svg") || (head.starts_with("<?xml") && head.contains("<svg"))
}

/// System fonts for SVG `<text>`, loaded once per process and only when an
/// SVG actually contains text (usvg uses its own `fontdb` version).
fn svg_fonts() -> Arc<usvg::fontdb::Database> {
    static FONTS: std::sync::OnceLock<Arc<usvg::fontdb::Database>> = std::sync::OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = usvg::fontdb::Database::new();
            db.load_system_fonts();
            Arc::new(db)
        })
        .clone()
}

/// Parse an SVG document.
pub(crate) fn parse_svg(data: &[u8]) -> Result<usvg::Tree, String> {
    let mut opt = usvg::Options::default();
    if data.windows(5).any(|w| w == b"<text") {
        opt.fontdb = svg_fonts();
    }
    usvg::Tree::from_data(data, &opt).map_err(|e| format!("cannot parse SVG: {e}"))
}

/// Intrinsic size in CSS px (raster: pixels; SVG: its width/height or viewBox).
pub(crate) fn intrinsic_size(src: &str, base_url: Option<&str>) -> Option<(f32, f32)> {
    let data = source_bytes(src, base_url).ok()?;
    if is_svg(src, &data) {
        let tree = parse_svg(&data).ok()?;
        let s = tree.size();
        return Some((s.width(), s.height()));
    }
    let (w, h) = image::ImageReader::new(std::io::Cursor::new(data))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()?;
    Some((w as f32, h as f32))
}

/// Replace every inline `<svg>` element by an `<img>` whose `src` is the
/// serialised SVG, so it is sized, laid out and embedded like an SVG file.
/// `width`, `height`, `class`, `style` and `id` stay on the element.
pub(crate) fn inline_svgs_to_img(doc: &mut Document) {
    let mut svgs = Vec::new();
    let mut stack = vec![doc.root()];
    while let Some(id) = stack.pop() {
        let node = doc.get(id);
        if node.tag_name.as_deref() == Some("svg") {
            svgs.push(id);
            continue; // nested <svg> are part of the outer one
        }
        stack.extend(node.children.iter().copied());
    }
    for id in svgs {
        let mut xml = String::new();
        serialize(doc, id, true, &mut xml);
        let uri = format!(
            "data:image/svg+xml;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(xml.as_bytes())
        );
        let node = &mut doc.nodes[id];
        node.tag_name = Some("img".to_string());
        node.attributes
            .retain(|k, _| matches!(k.as_str(), "width" | "height" | "class" | "style" | "id"));
        node.attributes.insert("src".to_string(), uri);
        node.children.clear();
    }
}

fn serialize(doc: &Document, id: NodeId, root: bool, out: &mut String) {
    let node = doc.get(id);
    if node.is_text() {
        out.push_str(&escape(node.text.as_deref().unwrap_or(""), false));
        return;
    }
    let Some(tag) = node.tag_name.as_deref() else {
        for &c in &node.children {
            serialize(doc, c, false, out);
        }
        return;
    };
    out.push('<');
    out.push_str(tag);
    if root && !node.attributes.contains_key("xmlns") {
        out.push_str(
            r#" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink""#,
        );
    }
    let mut attrs: Vec<_> = node.attributes.iter().collect();
    attrs.sort();
    for (k, v) in attrs {
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        out.push_str(&escape(v, true));
        out.push('"');
    }
    if node.children.is_empty() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    for &c in &node.children {
        serialize(doc, c, false, out);
    }
    out.push_str("</");
    out.push_str(tag);
    out.push('>');
}

fn escape(s: &str, attr: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attr => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="60" viewBox="0 0 12 6"><rect width="12" height="6" fill="#f00"/></svg>"##;

    #[test]
    fn detects_svg() {
        assert!(is_svg("logo.SVG", b""));
        assert!(is_svg("data:image/svg+xml;base64,xx", b""));
        assert!(is_svg("x", b"\xef\xbb\xbf  <?xml version='1.0'?><svg/>"));
        assert!(!is_svg("photo.png", b"\x89PNG"));
    }

    #[test]
    fn svg_intrinsic_size() {
        let uri = format!(
            "data:image/svg+xml;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(SVG)
        );
        assert_eq!(intrinsic_size(&uri, None), Some((120.0, 60.0)));
        let utf8 = format!(
            "data:image/svg+xml,{}",
            SVG.replace('<', "%3C").replace('#', "%23")
        );
        assert_eq!(intrinsic_size(&utf8, None), Some((120.0, 60.0)));
    }

    #[test]
    fn inline_svg_becomes_img() {
        let html = format!("<html><body><p>{SVG}</p></body></html>")
            .replace(" xmlns=\"http://www.w3.org/2000/svg\"", "");
        let mut pr = ferropdf_parse::parse(&html).unwrap();
        inline_svgs_to_img(&mut pr.document);
        let (_, img) = pr
            .document
            .nodes
            .iter()
            .find(|(_, n)| n.tag_name.as_deref() == Some("img"))
            .expect("svg converted");
        assert_eq!(img.attr("width"), Some("120"));
        let data = source_bytes(img.attr("src").unwrap(), None).unwrap();
        let xml = String::from_utf8(data.clone()).unwrap();
        assert!(xml.contains("viewBox=\"0 0 12 6\""), "{xml}");
        assert!(parse_svg(&data).is_ok(), "{xml}");
    }
}
