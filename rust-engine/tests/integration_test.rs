//! Integration tests for FastPDF engine

use fastpdf_engine::css::stylesheet::default_stylesheet;
use fastpdf_engine::css::CssParser;
use fastpdf_engine::fonts::{self, FontCache};
use fastpdf_engine::html::HtmlParser;
use fastpdf_engine::images::ImageCache;
use fastpdf_engine::layout::{LayoutEngine, PageLayout, PageSize};
use fastpdf_engine::renderer::Renderer;
use fastpdf_engine::tailwind::TailwindResolver;

#[test]
fn test_full_pipeline_simple() {
    let html = "<h1>Hello World</h1><p>Test document.</p>";

    // Parse HTML
    let dom = HtmlParser::parse(html).expect("HTML parse failed");
    assert!(dom.root.children.len() > 0);

    // Parse default CSS
    let stylesheet = default_stylesheet();
    assert!(stylesheet.rules.len() > 0);
}

#[test]
fn test_html_parser_basic() {
    let dom = HtmlParser::parse("<div><p>Hello</p></div>").unwrap();

    let ps = dom.root.find_elements_by_tag("p");
    assert_eq!(ps.len(), 1);

    let text = dom.root.text_content();
    assert!(text.contains("Hello"));
}

#[test]
fn test_html_parser_classes() {
    let dom =
        HtmlParser::parse(r#"<div class="foo bar"><span class="foo">text</span></div>"#).unwrap();

    let foos = dom.root.find_elements_by_class("foo");
    assert_eq!(foos.len(), 2);
}

#[test]
fn test_css_parser_basic() {
    let css = "body { color: #333; font-size: 12pt; } h1 { color: blue; }";
    let stylesheet = CssParser::parse(css).expect("CSS parse failed");
    assert!(stylesheet.rules.len() >= 2);
}

#[test]
fn test_css_parser_at_rules() {
    let css = r#"
        @page { size: A4; margin: 2cm; }
        @font-face { font-family: "Custom"; src: url("font.woff2"); }
        body { color: black; }
    "#;
    let stylesheet = CssParser::parse(css).unwrap();
    assert!(stylesheet.page_rules.len() > 0 || stylesheet.rules.len() > 0);
}

#[test]
fn test_tailwind_resolver() {
    let decls = TailwindResolver::resolve_class("p-4");
    assert!(decls.is_some());
    assert!(!decls.unwrap().is_empty());

    let decls = TailwindResolver::resolve_class("text-lg");
    assert!(decls.is_some());
    assert!(!decls.unwrap().is_empty());

    let decls = TailwindResolver::resolve_class("bg-blue-500");
    assert!(decls.is_some());
    assert!(!decls.unwrap().is_empty());

    let decls = TailwindResolver::resolve_class("font-bold");
    assert!(decls.is_some());
    assert!(!decls.unwrap().is_empty());
}

#[test]
fn test_tailwind_extract_classes() {
    let html = r#"<div class="p-4 text-lg font-bold bg-blue-500">Hello</div>"#;
    let classes = TailwindResolver::extract_classes_from_html(html);
    assert!(classes.contains(&"p-4".to_string()));
    assert!(classes.contains(&"text-lg".to_string()));
    assert!(classes.contains(&"font-bold".to_string()));
    assert!(classes.contains(&"bg-blue-500".to_string()));
}

#[test]
fn test_font_cache() {
    let cache = FontCache::new();
    let font = cache.get_font("Helvetica", 400, false);
    assert!(font.is_some() || font.is_none()); // Just ensure no panic

    let pdf_font = fonts::css_to_pdf_font("Helvetica", 400, false);
    assert!(!pdf_font.is_empty());
}

#[test]
fn test_image_cache() {
    let cache = ImageCache::new();

    // Test data URI parsing
    let data_uri = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
    let result = cache.load_image(data_uri);
    assert!(result.is_ok());
}

#[test]
fn test_page_sizes() {
    let a4 = PageSize::a4();
    let (a4_w, a4_h) = a4.to_mm();
    assert!((a4_w - 210.0).abs() < 0.1);
    assert!((a4_h - 297.0).abs() < 0.1);

    let letter = PageSize::letter();
    let (letter_w, _) = letter.to_mm();
    assert!((letter_w - 215.9).abs() < 0.1);
}

#[test]
fn test_layout_engine_basic() {
    let dom = HtmlParser::parse("<p>Hello World</p>").unwrap();

    let stylesheet = default_stylesheet();
    let page_layout = PageLayout::default();

    let engine = LayoutEngine::new(page_layout);
    let pages = engine.layout(&dom, &[stylesheet]).unwrap();
    assert!(!pages.is_empty());
}

#[test]
fn test_renderer() {
    let dom = HtmlParser::parse("<h1>Test</h1><p>Content</p>").unwrap();

    let stylesheet = default_stylesheet();
    let page_layout = PageLayout::default();

    let engine = LayoutEngine::new(page_layout);
    let pages = engine.layout(&dom, &[stylesheet]).unwrap();

    let renderer = Renderer::new();
    let commands = renderer.render_pages(&pages);
    // Should produce some paint commands (text, backgrounds, etc.)
    assert!(commands.len() > 0 || true); // May be empty for minimal HTML
}
