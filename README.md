<div align="center">

# ferropdf

**Fast HTML-to-PDF for Python — Rust-powered, up to 13x faster than WeasyPrint.**

[![CI](https://github.com/MoncefMak/ferropdf/actions/workflows/ci.yml/badge.svg)](https://github.com/MoncefMak/ferropdf/actions/workflows/ci.yml)
[![PyPI](https://img.shields.io/pypi/v/ferropdf)](https://pypi.org/project/ferropdf/)
[![Python](https://img.shields.io/pypi/pyversions/ferropdf)](https://pypi.org/project/ferropdf/)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

</div>

---

## Why ferropdf?

Most Python HTML-to-PDF libraries are slow. **ferropdf** is a native Rust engine exposed as a Python package via PyO3 — it renders complex invoices, reports, and dashboards in milliseconds, not seconds.

| Feature | ferropdf | WeasyPrint |
|---|---|---|
| **Speed** (invoice) | ~25ms | ~330ms |
| **GIL** | Released during render | Held |
| **Async-safe** | Yes (FastAPI, Django) | No |
| **Font subsetting** | Automatic | Manual |
| **Install** | `pip install ferropdf` | System deps required |

> Benchmarks on Linux x86_64, median of 20 runs, reusable engine with font cache.

---

## Install

```bash
pip install ferropdf
```

Pre-built wheels for **Linux** (x86_64, aarch64), **macOS** (x86_64, ARM), and **Windows** (x86_64).  
Python 3.8 – 3.13 supported.

---

## Quick start

### One-shot rendering

```python
import ferropdf

# HTML string → PDF bytes
pdf = ferropdf.from_html("<h1>Hello, World!</h1>")

# With options
pdf = ferropdf.from_html(
    "<h1>Invoice</h1><p>Total: $1,234</p>",
    options=ferropdf.Options(page_size="Letter", margin="25mm"),
)

# Write directly to disk
ferropdf.write_pdf("<h1>Report</h1>", "report.pdf")

# From an HTML file
pdf = ferropdf.from_file("templates/invoice.html")
```

### Reusable engine (recommended for servers)

```python
from ferropdf import Engine, Options

engine = Engine(Options(page_size="A4", margin="20mm"))

# Font database is cached — subsequent renders are faster
pdf1 = engine.render("<h1>Invoice #1</h1>")
pdf2 = engine.render("<h1>Invoice #2</h1>")
```

---

## API reference

### `Options`

```python
ferropdf.Options(
    page_size="A4",         # A4, Letter, Legal, A3, Tabloid, ...
    margin="20mm",          # CSS margin (mm, pt, px)
    base_url=None,          # Resolve relative paths in CSS
    title=None,             # PDF metadata
    author=None,            # PDF metadata
)
```

### `Engine`

```python
engine = ferropdf.Engine(options=None)
engine.render(html: str) -> bytes         # Render HTML to PDF bytes
engine.render_file(path: str) -> bytes    # Render from file
```

The engine caches fonts internally — create once, render many times.

### Functions

| Function | Description |
|---|---|
| `from_html(html, base_url=None, options=None) -> bytes` | Render HTML string to PDF |
| `from_file(path, options=None) -> bytes` | Render HTML file to PDF |
| `write_pdf(html, output_path, base_url=None, options=None)` | Render and write to disk |

### Exceptions

All exceptions inherit from `ferropdf.FerroError` (itself a `RuntimeError`):

- `ParseError` — HTML/CSS parsing failure
- `LayoutError` — Layout computation failure
- `FontError` — Font loading/resolution failure
- `RenderError` — PDF generation failure

---

## Framework integration

### Django

```python
# views.py
from ferropdf.contrib.django import PdfResponse

def invoice(request, pk):
    context = {"invoice_id": pk, "items": get_items(pk)}
    return PdfResponse("invoice.html", context, request=request)
```

`PdfResponse` renders a Django template to PDF. Pass `inline=False` to force download instead of browser preview.

### FastAPI

```python
# main.py
from ferropdf.contrib.fastapi import pdf_response

@app.get("/invoice/{id}/pdf")
async def invoice_pdf(id: int):
    html = templates.get_template("invoice.html").render(invoice_id=id)
    return await pdf_response(html, filename=f"invoice-{id}.pdf")
```

`pdf_response` is async — rendering runs in a thread executor with the GIL released, so it won't block your event loop.

---

## CSS support

ferropdf uses industry-standard libraries for parsing and layout — not a hand-rolled engine.

### Layout

| Feature | Status |
|---|---|
| Block layout | Supported |
| Flexbox (`flex-direction`, `flex-wrap`, `gap`, `justify-content`, `align-items`) | Supported |
| Tables (`<table>`, `<thead>`, `<tbody>`, `<tr>`, `<td>`) | Supported |
| `width`, `height` (px, %, em) | Supported |
| `margin`, `padding` (px, mm, em, auto) | Supported |
| `box-sizing: border-box` | Supported |
| CSS Grid | Not yet |
| `float` | Not yet |

### Typography

| Feature | Status |
|---|---|
| `font-family` (system fonts, fallbacks) | Supported |
| `font-size` (px, pt, mm, em, rem) | Supported |
| `font-weight` (normal, bold, 100–900) | Supported |
| `font-style` (normal, italic) | Supported |
| `line-height` | Supported |
| `text-align` (left, center, right) | Supported |
| `@font-face` | Not yet |

### Visual

| Feature | Status |
|---|---|
| `color`, `background-color` (hex, rgb, rgba) | Supported |
| `border` (width, style, color) | Supported |
| `border-radius` | Supported |
| `opacity` | Not yet |
| `box-shadow` | Not yet |

### Page

| Feature | Status |
|---|---|
| Multi-page documents | Supported |
| Page sizes (A0–A10, Letter, Legal, Tabloid, B-series) | Supported |
| Configurable margins | Supported |
| PDF metadata (title, author) | Supported |
| `@page` rules | Planned |

---

## Architecture

ferropdf is built as a modular Rust workspace with 6 crates:

```
HTML string
  ↓  ferropdf-parse      (html5ever + cssparser)
DOM tree + Stylesheets
  ↓  ferropdf-style       (Mozilla's selectors crate — cascade, specificity, inheritance)
Style tree
  ↓  ferropdf-layout      (Taffy flexbox engine + cosmic-text shaping)
Layout tree
  ↓  ferropdf-page        (pagination into discrete pages)
Pages
  ↓  ferropdf-render      (pdf-writer — font subsetting, compression, embedding)
PDF bytes
```

| Crate | Role | Key dependency |
|---|---|---|
| `ferropdf-core` | Shared types: DOM, styles, geometry, errors | — |
| `ferropdf-parse` | HTML & CSS parsing | html5ever, cssparser |
| `ferropdf-style` | CSS cascade, specificity, inheritance | selectors (Mozilla) |
| `ferropdf-layout` | Box layout + text shaping | taffy, cosmic-text |
| `ferropdf-page` | Pagination across pages | — |
| `ferropdf-render` | PDF generation, font embedding | pdf-writer, subsetter |

Python bindings are via [PyO3](https://pyo3.rs) + [maturin](https://www.maturin.rs).

---

## Performance

Font subsetting + caching means:

- **First render**: loads system fonts + builds cache (~100ms)
- **Subsequent renders**: 15–30ms for typical documents
- **PDF sizes**: small, because only used glyphs are embedded

The `Engine` class keeps the font cache alive — ideal for web servers handling many requests.

---

## Examples

The [`examples/`](examples/) directory includes:

- **[basic.py](examples/basic.py)** — Hello world, styled report card, invoice
- **[FastAPI app](examples/fastapi_app/)** — Invoice, report, receipt, dashboard, letter endpoints
- **[Django app](examples/django_app/)** — Same templates with Django views

Run the FastAPI example:

```bash
cd examples/fastapi_app
pip install fastapi uvicorn jinja2
uvicorn main:app --reload
# Visit http://localhost:8000/invoice/42/pdf
```

---

## Development

```bash
# Clone and setup
git clone https://github.com/MoncefMak/ferropdf.git
cd ferropdf
python -m venv .venv && source .venv/bin/activate
pip install maturin pytest

# Build and install locally
maturin develop --release

# Run tests
cargo test --no-default-features   # Rust tests
pytest tests/ -v                   # Python tests

# Lint
cargo fmt --all --check
cargo clippy --workspace -- -D warnings
cargo audit
```

---

## License

MIT
