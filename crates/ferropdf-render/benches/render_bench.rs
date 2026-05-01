use criterion::{criterion_group, criterion_main, Criterion};
use ferropdf_render::{render, render_with_cache, FontDatabase, RenderOptions};

const SIMPLE_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
  <style>
    body { font-family: sans-serif; margin: 20px; }
    h1   { color: #2563eb; }
    .box { background: #f3f4f6; padding: 16px; border-radius: 8px; }
  </style>
</head>
<body>
  <h1>Simple Test</h1>
  <div class="box"><p>Contenu de test</p></div>
</body>
</html>"#;

const INVOICE_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
  <style>
    body   { font-family: sans-serif; margin: 40px; font-size: 14px; }
    .header { display: flex; justify-content: space-between; margin-bottom: 40px; }
    .title  { font-size: 28px; font-weight: bold; color: #1e40af; }
    table   { width: 100%; border-collapse: collapse; margin-top: 20px; }
    th      { background: #1e40af; color: white; padding: 10px; text-align: left; }
    td      { padding: 10px; border-bottom: 1px solid #e5e7eb; }
    .total  { font-size: 18px; font-weight: bold; text-align: right; margin-top: 20px; }
  </style>
</head>
<body>
  <div class="header">
    <div class="title">FACTURE #2024-001</div>
    <div>Date : 17/03/2026</div>
  </div>
  <table>
    <thead>
      <tr><th>Description</th><th>Qté</th><th>Prix unit.</th><th>Total</th></tr>
    </thead>
    <tbody>
      <tr><td>Développement Rust</td><td>10</td><td>150€</td><td>1500€</td></tr>
      <tr><td>Intégration Python</td><td>5</td><td>120€</td><td>600€</td></tr>
      <tr><td>Tests et documentation</td><td>3</td><td>100€</td><td>300€</td></tr>
    </tbody>
  </table>
  <div class="total">Total : 2400€ HT</div>
</body>
</html>"#;

fn default_opts() -> RenderOptions {
    RenderOptions {
        page_size: "A4".to_string(),
        margin: "15mm".to_string(),
        base_url: None,
        title: None,
        author: None,
        max_html_bytes: None,
    }
}

fn bench_simple(c: &mut Criterion) {
    let opts = default_opts();
    c.bench_function("render_simple", |b| {
        b.iter(|| render(SIMPLE_HTML, &opts).unwrap());
    });
}

fn bench_invoice(c: &mut Criterion) {
    let opts = default_opts();
    c.bench_function("render_invoice", |b| {
        b.iter(|| render(INVOICE_HTML, &opts).unwrap());
    });
}

fn bench_simple_cached(c: &mut Criterion) {
    let opts = default_opts();
    let font_db = FontDatabase::new();
    c.bench_function("render_simple_cached", |b| {
        b.iter(|| render_with_cache(SIMPLE_HTML, &opts, &font_db).unwrap());
    });
}

fn bench_invoice_cached(c: &mut Criterion) {
    let opts = default_opts();
    let font_db = FontDatabase::new();
    c.bench_function("render_invoice_cached", |b| {
        b.iter(|| render_with_cache(INVOICE_HTML, &opts, &font_db).unwrap());
    });
}

criterion_group!(
    benches,
    bench_simple,
    bench_invoice,
    bench_simple_cached,
    bench_invoice_cached
);
criterion_main!(benches);
