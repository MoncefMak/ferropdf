# FerroPDF

**High-performance PDF generation from HTML, CSS, and Tailwind templates.**

FerroPDF is a Rust-powered PDF rendering engine with a clean Python API. Simple documents render in under 500 µs; 100-row styled tables in under 10 ms.

## Features

- **Blazing fast** — Rust core engine, 405 µs for simple documents; **52–88× faster than WeasyPrint**, **57–1220× faster than wkhtmltopdf**
- **Full HTML/CSS support** — HTML5 parsing, CSS3 styling, flexbox, tables
- **Tailwind CSS** — Use utility classes directly, no build step required
- **Template rendering** — Jinja2 templates with context variables
- **Django integration** — Template rendering, HttpResponse helpers, CBV mixin, middleware
- **FastAPI integration** — PdfResponse, async rendering, streaming support
- **Parallel batch rendering** — Generate hundreds of PDFs concurrently via Rayon
- **Minimal memory footprint** — Efficient Rust memory management
- **Custom fonts** — Register and use TrueType/OpenType fonts
- **Page management** — Headers, footers, page numbers, page breaks

## Quick Start

### Installation

```bash
pip install ferropdf
```

### Basic Usage

```python
from fastpdf import render_pdf, render_pdf_to_file, RenderOptions

# Render HTML to PDF bytes
pdf_bytes = render_pdf("<h1>Hello World</h1>")

# Save to file
render_pdf_to_file("<h1>Hello</h1>", "output.pdf")

# With custom options
options = RenderOptions(
    page_size="A4",
    margin_top=20.0,
    title="My Document",
    author="FerroPDF",
)
pdf_bytes = render_pdf(html, options=options)
```

### With CSS

```python
html = "<h1>Styled Document</h1><p class='intro'>Hello!</p>"
css = """
h1 { color: #1a56db; border-bottom: 2px solid #1a56db; }
.intro { font-size: 14pt; color: #6b7280; }
"""
pdf_bytes = render_pdf(html, css=css)
```

### Tailwind CSS

```python
html = """
<div class="p-8">
    <h1 class="text-3xl font-bold text-blue-600 mb-4">Invoice</h1>
    <p class="text-gray-600">Generated with Tailwind CSS</p>
</div>
"""
pdf_bytes = render_pdf(html, options=RenderOptions(tailwind=True))
```

### Template Rendering

```python
from fastpdf import render_pdf_from_template

pdf_bytes = render_pdf_from_template(
    "invoice.html",
    context={
        "customer": "Acme Corp",
        "items": [{"name": "Widget", "price": 9.99}],
        "total": 9.99,
    },
    template_dir="templates/",
)
```

### Engine (Shared Configuration)

```python
from fastpdf import PdfEngine, RenderOptions

engine = PdfEngine(
    template_dir="templates/",
    default_options=RenderOptions(page_size="A4", tailwind=True),
)

# Register custom fonts
engine.register_font("CustomFont", "/path/to/font.ttf")

# Render multiple documents
doc1 = engine.render("<h1>Document 1</h1>")
doc2 = engine.render_template("report.html", context={"data": data})

doc1.save("doc1.pdf")
doc2.save("doc2.pdf")
```

### Batch Rendering (Parallel)

```python
from fastpdf import batch_render

items = [
    {"html": f"<h1>Invoice #{i}</h1>"} for i in range(100)
]
pdf_list = batch_render(items)  # Rendered in parallel via Rayon
```

## Django Integration

```python
# views.py
from fastpdf.contrib.django import render_to_pdf_response, PdfView

# Function-based view
def invoice_pdf(request, pk):
    invoice = get_object_or_404(Invoice, pk=pk)
    return render_to_pdf_response(
        request,
        "invoices/invoice.html",
        {"invoice": invoice},
        filename="invoice.pdf",
    )

# Class-based view
class ReportPdfView(PdfView, DetailView):
    model = Report
    template_name = "reports/detail.html"
    pdf_filename = "report.pdf"
```

### Django Middleware

Add automatic PDF conversion with `?format=pdf`:

```python
# settings.py
MIDDLEWARE = [
    ...
    "fastpdf.contrib.django.PdfMiddleware",
]

# settings.py (optional)
FERROPDF = {
    "DEFAULT_PAGE_SIZE": "A4",
    "DEFAULT_MARGIN": 15.0,
    "TAILWIND": True,
}
```

## FastAPI Integration

```python
from fastapi import FastAPI
from fastpdf.contrib.fastapi import PdfResponse, render_pdf_async

app = FastAPI()

@app.get("/invoice/{id}")
async def get_invoice(id: int):
    html = f"<h1>Invoice #{id}</h1>"
    return PdfResponse(html, filename=f"invoice-{id}.pdf")

@app.get("/report")
async def get_report():
    # Async rendering (non-blocking)
    pdf_bytes = await render_pdf_async("<h1>Report</h1>")
    return Response(content=pdf_bytes, media_type="application/pdf")
```

## API Reference

### `render_pdf(html, *, css=None, options=None) → bytes`

Render HTML string to PDF bytes.

### `render_pdf_to_file(html, path, *, css=None, options=None) → PdfDocument`

Render HTML and save to file. Returns `PdfDocument` for inspection.

### `render_pdf_from_template(template_name, *, context=None, template_dir=None, css=None, options=None) → bytes`

Render a Jinja2 template to PDF bytes.

### `batch_render(items, *, options=None, parallel=True) → list[bytes]`

Render multiple documents in parallel. Each item is a dict with `"html"` and optional `"css"` keys.

### `RenderOptions`

| Parameter | Type | Default | Description |
|---|---|---|---|
| `page_size` | `str \| tuple` | `"A4"` | Page size name or `(width_mm, height_mm)` |
| `orientation` | `str` | `"portrait"` | `"portrait"` or `"landscape"` |
| `margin_top` | `float` | `10.0` | Top margin in mm |
| `margin_right` | `float` | `10.0` | Right margin in mm |
| `margin_bottom` | `float` | `10.0` | Bottom margin in mm |
| `margin_left` | `float` | `10.0` | Left margin in mm |
| `title` | `str \| None` | `None` | PDF title metadata |
| `author` | `str \| None` | `None` | PDF author metadata |
| `tailwind` | `bool` | `False` | Enable Tailwind CSS resolution |
| `base_path` | `str \| None` | `None` | Base path for asset resolution |
| `header_html` | `str \| None` | `None` | Running header HTML |
| `footer_html` | `str \| None` | `None` | Running footer HTML |

### `PdfDocument`

| Method/Property | Description |
|---|---|
| `.to_bytes()` | Get raw PDF bytes |
| `.save(path)` | Write to file |
| `.page_count` | Number of pages |
| `.title` | Document title |

### `PdfEngine`

| Method | Description |
|---|---|
| `.render(html, *, css, options)` | Render HTML to `PdfDocument` |
| `.render_template(name, *, context, css, options)` | Render Jinja2 template |
| `.render_to_file(html, path, *, css, options)` | Render and save |
| `.batch_render(items, *, options, parallel)` | Parallel batch render |
| `.register_font(name, path)` | Register custom font |

## Architecture

```
┌─────────────────────────────────────────────────┐
│                  Python API                      │
│  render_pdf() · PdfEngine · Django · FastAPI     │
├─────────────────────────────────────────────────┤
│              PyO3 Bindings Layer                 │
├─────────────────────────────────────────────────┤
│                Rust Core Engine                  │
│                                                  │
│  ┌──────────┐  ┌──────────┐  ┌──────────────┐  │
│  │  HTML     │  │  CSS     │  │  Tailwind    │  │
│  │  Parser   │  │  Parser  │  │  Resolver    │  │
│  │(html5ever)│  │ (custom) │  │  (custom)    │  │
│  └────┬─────┘  └────┬─────┘  └──────┬───────┘  │
│       │              │               │           │
│       ▼              ▼               ▼           │
│  ┌──────────────────────────────────────────┐   │
│  │          Style Resolution                 │   │
│  │    (selector matching, inheritance)       │   │
│  └────────────────┬─────────────────────────┘   │
│                   │                              │
│                   ▼                              │
│  ┌──────────────────────────────────────────┐   │
│  │          Layout Engine                    │   │
│  │  (block, inline, flex, table, pagination) │   │
│  └────────────────┬─────────────────────────┘   │
│                   │                              │
│                   ▼                              │
│  ┌──────────────────────────────────────────┐   │
│  │        Renderer (Paint Commands)          │   │
│  └────────────────┬─────────────────────────┘   │
│                   │                              │
│                   ▼                              │
│  ┌──────────────────────────────────────────┐   │
│  │       PDF Generator (printpdf)            │   │
│  └──────────────────────────────────────────┘   │
└─────────────────────────────────────────────────┘
```

## Performance

### Criterion.rs benchmarks (statistical)

Measured with [Criterion.rs](https://github.com/bheisler/criterion.rs) — 100 samples per benchmark, 95% confidence intervals.
Machine: **Intel Core i5-10210U** (4C/8T, 1.6–4.2 GHz), 24 GB RAM, Debian 12.

#### Full pipeline (HTML+CSS → PDF)

| Document | Time (95% CI) |
|---|---|
| Simple HTML (`<h1>` + `<p>`) | **160–167 µs** |
| Styled HTML (headings, lists, CSS) | **481–543 µs** |
| Complex report (tables, metrics, multi-section) | **1.32–1.34 ms** |

#### Table scaling

| Rows | Time (95% CI) |
|---|---|
| 10 rows | **874 µs – 1.05 ms** |
| 25 rows | **1.51–1.57 ms** |
| 50 rows | **3.23–3.81 ms** |
| 100 rows | **6.14–6.51 ms** |

#### Individual pipeline stages (complex report)

| Stage | Time (95% CI) |
|---|---|
| HTML parsing | **102–113 µs** |
| CSS parsing | **8.0–8.7 µs** |
| Layout engine | **502–555 µs** |
| Paint commands | **47.7–49.3 µs** |
| PDF generation | **735–837 µs** |

#### Tailwind

| Operation | Time (95% CI) |
|---|---|
| Extract classes | **85–88 µs** |
| Resolve classes → CSS | **47–52 µs** |

Reproduce locally:

```bash
cd rust-engine && cargo bench --bench render_bench
```

The CI also runs benchmarks on every push — see the [Benchmarks workflow](../../actions/workflows/benchmark.yml).

### Python-level benchmarks — FerroPDF vs WeasyPrint

Full pipeline (HTML → PDF bytes), including PyO3 overhead. Measured with `benchmark_comparison.py` — 10 timed runs + 1 warm-up per fixture.
Machine: **Linux 6.1.0-43-amd64**, Python 3.11.2.

| Document | FerroPDF | WeasyPrint | wkhtmltopdf | vs WeasyPrint | vs wkhtmltopdf |
|---|---|---|---|---|---|
| Simple HTML | **405 µs** ±23 µs | 21.1 ms ±2.7 ms | 494.2 ms ±60.5 ms | **52× faster** | **1220× faster** |
| Styled HTML | **541 µs** ±61 µs | 28.1 ms ±2.5 ms | 482.0 ms ±42.6 ms | **52× faster** | **891× faster** |
| Table 10 rows | **1.4 ms** ±126 µs | 124.7 ms ±30.9 ms | 499.4 ms ±59.6 ms | **88× faster** | **357× faster** |
| Table 50 rows | **5.1 ms** ±434 µs | 387.8 ms ±33.9 ms | 538.1 ms ±29.8 ms | **76× faster** | **106× faster** |
| Table 100 rows | **9.5 ms** ±630 µs | 685.2 ms ±66.2 ms | 544.0 ms ±55.8 ms | **72× faster** | **57× faster** |

Reproduce:

```bash
pip install weasyprint
sudo apt install wkhtmltopdf   # optional
python benchmarks/benchmark_comparison.py --runs 10 --output benchmarks/benchmark_results.md
```

## Development

### Prerequisites

- Rust 1.70+ (for the engine)
- Python 3.8+
- maturin (`pip install maturin`)

### Building

```bash
# Development build
maturin develop

# Release build
maturin build --release

# Run Rust tests
cd rust-engine && cargo test

# Run Python tests
pytest tests/python/
```

### Project Structure

```
ferropdf/
├── rust-engine/           # Rust core engine
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs          # Module entry point
│       ├── bindings.rs     # PyO3 Python bindings
│       ├── error.rs        # Error types
│       ├── html/           # HTML5 parser (html5ever)
│       ├── css/            # CSS parser & value types
│       ├── layout/         # Box model, layout, pagination
│       ├── fonts/          # Font cache & management
│       ├── images/         # Image loading & caching
│       ├── tailwind/       # Tailwind CSS resolver
│       ├── renderer/       # Paint command generation
│       └── pdf/            # PDF file generation (printpdf)
├── python-wrapper/        # Python package
│   └── fastpdf/
│       ├── __init__.py     # Public API
│       ├── core.py         # Core wrapper classes
│       ├── utils.py        # Utility helpers
│       └── contrib/
│           ├── django.py   # Django integration
│           └── fastapi.py  # FastAPI integration
├── examples/              # Usage examples
├── benchmarks/            # Performance benchmarks
├── tests/                 # Test suites
└── pyproject.toml         # Build configuration
```

## License

MIT OR Apache-2.0
