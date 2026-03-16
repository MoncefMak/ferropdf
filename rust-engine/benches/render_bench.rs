use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use fastpdf_engine::css::stylesheet::default_stylesheet;
use fastpdf_engine::css::{CssParser, Stylesheet};
use fastpdf_engine::html::HtmlParser;
use fastpdf_engine::layout::engine::LayoutEngine;
use fastpdf_engine::layout::pagination::{PageLayout, PageSize};
use fastpdf_engine::pdf::generator::{PdfConfig, PdfGenerator};
use fastpdf_engine::renderer::paint::Renderer;
use fastpdf_engine::tailwind::TailwindResolver;

// ── Test documents ──────────────────────────────────────────────────────────

const SIMPLE_HTML: &str = "<h1>Hello World</h1><p>Simple paragraph of text.</p>";

const STYLED_HTML: &str = r#"
<div>
  <h1>Styled Document</h1>
  <p>This is a paragraph with <strong>bold</strong> and <em>italic</em> text.</p>
  <p>Another paragraph with more content to test rendering performance.</p>
  <ul>
    <li>Item one</li>
    <li>Item two</li>
    <li>Item three</li>
  </ul>
</div>
"#;

const STYLED_CSS: &str = r#"
body { font-family: Helvetica, sans-serif; color: #333; font-size: 12pt; }
h1 { color: #1a56db; border-bottom: 2px solid #1a56db; padding-bottom: 10px; }
p { line-height: 1.6; margin: 10px 0; }
ul { margin: 10px 0; padding-left: 20px; }
li { margin: 4px 0; }
"#;

fn table_html(rows: usize) -> String {
    let mut html =
        String::from("<table><tr><th>Name</th><th>Email</th><th>Role</th><th>Status</th></tr>");
    for i in 0..rows {
        html.push_str(&format!(
            "<tr><td>User {i}</td><td>user{i}@example.com</td><td>Developer</td><td>Active</td></tr>"
        ));
    }
    html.push_str("</table>");
    html
}

const TABLE_CSS: &str = r#"
table { width: 100%; border-collapse: collapse; }
th { background-color: #1a56db; color: white; padding: 12px; text-align: left; }
td { padding: 10px 12px; border-bottom: 1px solid #e5e7eb; }
"#;

const COMPLEX_HTML: &str = r#"
<div>
  <div style="background-color: #1e3a5f; color: white; padding: 20px;">
    <h1>Quarterly Report</h1>
    <p>Q4 2025 Financial Summary</p>
  </div>
  <h2>Executive Summary</h2>
  <p>This report covers the financial performance for Q4 2025. Revenue grew by 15%
     compared to the previous quarter, driven primarily by new product launches and
     expanded market reach.</p>
  <h2>Revenue Breakdown</h2>
  <table>
    <tr><th>Category</th><th>Q3 2025</th><th>Q4 2025</th><th>Growth</th></tr>
    <tr><td>Product A</td><td>$1.2M</td><td>$1.5M</td><td>+25%</td></tr>
    <tr><td>Product B</td><td>$800K</td><td>$920K</td><td>+15%</td></tr>
    <tr><td>Services</td><td>$400K</td><td>$460K</td><td>+15%</td></tr>
    <tr><td>Total</td><td>$2.4M</td><td>$2.88M</td><td>+20%</td></tr>
  </table>
  <h2>Key Metrics</h2>
  <div>
    <div style="background-color: #eff6ff; padding: 15px; margin-bottom: 10px;">
      <h3>Customer Count</h3>
      <p style="font-size: 24px; font-weight: bold;">1,247</p>
    </div>
    <div style="background-color: #f0fdf4; padding: 15px; margin-bottom: 10px;">
      <h3>Monthly Revenue</h3>
      <p style="font-size: 24px; font-weight: bold;">$960K</p>
    </div>
  </div>
  <h2>Outlook</h2>
  <p>Based on current trends and pipeline analysis, we project continued growth of
     10-15% for Q1 2026. Key initiatives include international expansion and enterprise
     tier launch.</p>
</div>
"#;

const COMPLEX_CSS: &str = r#"
body { font-family: Helvetica, sans-serif; font-size: 11pt; color: #1a1a1a; line-height: 1.5; }
h1 { font-size: 28pt; margin: 0; }
h2 { color: #1e3a5f; border-bottom: 1px solid #ddd; padding-bottom: 5px; margin-top: 20px; }
h3 { color: #555; margin: 0 0 5px 0; }
table { width: 100%; border-collapse: collapse; margin: 15px 0; }
th { background-color: #1e3a5f; color: white; padding: 10px; text-align: left; }
td { padding: 8px 10px; border-bottom: 1px solid #e5e7eb; }
"#;

// ── Helper: full render pipeline (HTML+CSS → PDF bytes) ─────────────────────

fn render_pdf(html: &str, css: &str) -> Vec<u8> {
    // 1. Parse HTML
    let dom = HtmlParser::parse_fragment(html).expect("HTML parse failed");

    // 2. Parse CSS (inline <style> tags + external CSS)
    let mut stylesheets: Vec<Stylesheet> = Vec::new();
    for style_text in dom.extract_styles() {
        if let Ok(sheet) = CssParser::parse(&style_text) {
            stylesheets.push(sheet);
        }
    }
    if !css.is_empty() {
        let sheet = CssParser::parse(css).expect("CSS parse failed");
        stylesheets.push(sheet);
    }

    // 3. Layout
    let page_layout = PageLayout::new(PageSize::a4()).with_margins(10.0, 10.0, 10.0, 10.0);
    let engine = LayoutEngine::new(page_layout);
    let pages = engine.layout(&dom, &stylesheets).expect("Layout failed");

    // 4. Render paint commands
    let renderer = Renderer::new();
    let page_commands = renderer.render_pages(&pages);

    // 5. Generate PDF
    let generator = PdfGenerator::new(PdfConfig::default());
    generator
        .generate(&pages, &page_commands)
        .expect("PDF generation failed")
}

// ═══════════════════════════════════════════════════════════════════════════
// Benchmark groups
// ═══════════════════════════════════════════════════════════════════════════

// ── 1. Parsing benchmarks ───────────────────────────────────────────────────

fn bench_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("01_parse");

    group.bench_function("html_simple", |b| {
        b.iter(|| HtmlParser::parse_fragment(black_box(SIMPLE_HTML)).unwrap())
    });

    group.bench_function("html_complex", |b| {
        b.iter(|| HtmlParser::parse_fragment(black_box(COMPLEX_HTML)).unwrap())
    });

    group.bench_function("css_basic", |b| {
        b.iter(|| CssParser::parse(black_box(STYLED_CSS)).unwrap())
    });

    group.bench_function("css_complex", |b| {
        b.iter(|| CssParser::parse(black_box(COMPLEX_CSS)).unwrap())
    });

    group.bench_function("default_stylesheet", |b| {
        b.iter(|| black_box(default_stylesheet()))
    });

    group.finish();
}

// ── 2. Full pipeline benchmarks ─────────────────────────────────────────────

fn bench_full_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("02_full_pipeline");

    group.bench_function("simple_html", |b| {
        b.iter(|| render_pdf(black_box(SIMPLE_HTML), ""))
    });

    group.bench_function("styled_html", |b| {
        b.iter(|| render_pdf(black_box(STYLED_HTML), STYLED_CSS))
    });

    group.bench_function("complex_report", |b| {
        b.iter(|| render_pdf(black_box(COMPLEX_HTML), COMPLEX_CSS))
    });

    group.finish();
}

// ── 3. Table scaling benchmarks ─────────────────────────────────────────────

fn bench_tables(c: &mut Criterion) {
    let mut group = c.benchmark_group("03_tables");
    group.sample_size(50);

    for rows in [10, 25, 50, 100] {
        let html = table_html(rows);
        group.bench_with_input(BenchmarkId::new("rows", rows), &html, |b, html| {
            b.iter(|| render_pdf(black_box(html), TABLE_CSS))
        });
    }

    group.finish();
}

// ── 4. Tailwind resolution benchmark ────────────────────────────────────────

fn bench_tailwind(c: &mut Criterion) {
    let mut group = c.benchmark_group("04_tailwind");

    let tw_html = r#"
    <div class="p-8 bg-white">
      <h1 class="text-3xl font-bold text-blue-600 mb-4">Invoice #1234</h1>
      <p class="text-gray-600 text-sm mb-8">Generated on 2026-03-15</p>
      <div class="flex gap-4 mb-6">
        <div class="flex-1 p-4 bg-blue-50 rounded">
          <p class="text-xs text-gray-500">Total</p>
          <p class="text-2xl font-bold text-blue-700">$1,234.56</p>
        </div>
        <div class="flex-1 p-4 bg-green-50 rounded">
          <p class="text-xs text-gray-500">Paid</p>
          <p class="text-2xl font-bold text-green-700">$1,234.56</p>
        </div>
      </div>
    </div>
    "#;

    group.bench_function("extract_classes", |b| {
        b.iter(|| TailwindResolver::extract_classes_from_html(black_box(tw_html)))
    });

    group.bench_function("resolve_classes", |b| {
        let classes = TailwindResolver::extract_classes_from_html(tw_html);
        b.iter(|| TailwindResolver::resolve_classes(black_box(&classes)))
    });

    group.finish();
}

// ── 5. Individual pipeline stages (on complex doc) ──────────────────────────

fn bench_stages(c: &mut Criterion) {
    let mut group = c.benchmark_group("05_stages");

    // Pre-parse for stage-level benchmarks
    let dom = HtmlParser::parse_fragment(COMPLEX_HTML).unwrap();
    let mut stylesheets: Vec<Stylesheet> = Vec::new();
    for style_text in dom.extract_styles() {
        if let Ok(sheet) = CssParser::parse(&style_text) {
            stylesheets.push(sheet);
        }
    }
    let user_sheet = CssParser::parse(COMPLEX_CSS).unwrap();
    stylesheets.push(user_sheet);

    let page_layout = PageLayout::new(PageSize::a4()).with_margins(10.0, 10.0, 10.0, 10.0);

    // Stage: Layout
    group.bench_function("layout_complex", |b| {
        let engine = LayoutEngine::new(page_layout.clone());
        b.iter(|| {
            engine
                .layout(black_box(&dom), black_box(&stylesheets))
                .unwrap()
        })
    });

    // Stage: Render paint commands
    let engine = LayoutEngine::new(page_layout.clone());
    let pages = engine.layout(&dom, &stylesheets).unwrap();
    group.bench_function("render_paint_cmds", |b| {
        let renderer = Renderer::new();
        b.iter(|| renderer.render_pages(black_box(&pages)))
    });

    // Stage: PDF generation
    let renderer = Renderer::new();
    let page_commands = renderer.render_pages(&pages);
    group.bench_function("pdf_generate", |b| {
        let generator = PdfGenerator::new(PdfConfig::default());
        b.iter(|| {
            generator
                .generate(black_box(&pages), black_box(&page_commands))
                .unwrap()
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_parse,
    bench_full_pipeline,
    bench_tables,
    bench_tailwind,
    bench_stages,
);
criterion_main!(benches);
