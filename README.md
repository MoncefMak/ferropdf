<div align="center">

# ferropdf

**Fast HTML-to-PDF for Python — Rust-powered. Single-digit-millisecond renders for typical invoices and reports.**

[![CI](https://github.com/MoncefMak/ferropdf/actions/workflows/ci.yml/badge.svg)](https://github.com/MoncefMak/ferropdf/actions/workflows/ci.yml)
[![PyPI](https://img.shields.io/pypi/v/ferropdf)](https://pypi.org/project/ferropdf/)
[![Python](https://img.shields.io/pypi/pyversions/ferropdf)](https://pypi.org/project/ferropdf/)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

</div>

---

## Why ferropdf?

A native Rust HTML-to-PDF engine exposed as a Python package via PyO3. Releases the GIL during render so it composes cleanly with FastAPI, Django async views, and threadpools. The font cache lives on the `Engine` instance — the first render bootstraps system fonts, subsequent ones are fast.

| Feature | ferropdf | WeasyPrint |
|---|---|---|
| **GIL** | Released during render | Held |
| **Async-safe** | Yes (FastAPI, Django) | No |
| **Font subsetting** | Automatic | Manual |
| **Install** | `pip install ferropdf` | System deps required |

ferropdf intentionally targets **document rendering**, not full-fidelity browser layout. For honest performance comparisons, run [`bench/compare.py`](bench/compare.py) against your own workload — see [Performance](#performance) below.

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

## Security model

ferropdf is intended for both **trusted server-side templates** and **partially-trusted user HTML**. To stay safe with the latter, opt in to local-resource access explicitly via `base_url`:

- **`base_url=None`** *(default for v0.3+)*: `<img>`, `<link rel="stylesheet">`, and `@font-face url(...)` only resolve `data:` URIs. Local file paths and `http(s)://` URLs are ignored and produce a warning.
- **`base_url="/path/to/assets/"`**: relative paths resolve under that directory; the canonicalized result is verified to live inside it (path traversal blocked). HTTP(S) URLs are still skipped — ferropdf never makes outbound network requests.

When migrating from earlier versions, set `base_url` to your template directory to keep local images and stylesheets working.

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
| Tables (`<table>`, `<thead>`, `<tbody>`, `<tr>`, `<td>`, `colspan`, `rowspan`, `border-collapse`) | Supported |
| `position: relative`, `position: absolute` | Supported |
| `width`, `height` (px, %, em) | Supported |
| `margin`, `padding` (px, mm, em, auto) | Supported |
| `box-sizing: border-box` | Supported |
| CSS Grid | Experimental (taffy) |
| `position: fixed`, `sticky` | Not yet |
| `float` | Not yet |
| `overflow` | Not yet |

### Typography

| Feature | Status |
|---|---|
| `font-family` (system fonts, fallbacks) | Supported |
| `font-size` (px, pt, mm, em, rem) | Supported |
| `font-weight` (normal, bold, 100–900) | Supported |
| `font-style` (normal, italic) | Supported |
| `line-height` | Supported |
| `text-align` (left, center, right) | Supported |
| `@font-face` (data: URI + base_url paths) | Supported |
| Arabic shaping + `direction: rtl` | Supported (via cosmic-text/rustybuzz) |
| `text-align: justify` | Not yet |
| `letter-spacing`, `word-spacing` | Not yet |

### Visual

| Feature | Status |
|---|---|
| `color`, `background-color` (hex, rgb, rgba, named) | Supported |
| `border` (width, style, color) | Supported |
| `border-radius` | Supported |
| `box-shadow` (offset + blur + color) | Supported |
| `opacity` | Supported |
| `linear-gradient`, `radial-gradient` | Not yet |
| `transform`, `filter`, `clip-path` | Not yet |

### Selectors

| Feature | Status |
|---|---|
| Type, class, id, descendant, child, attribute | Supported |
| `:first-child`, `:last-child`, `:only-child` | Supported |
| `:nth-child(n)`, `:nth-of-type(n)` | Supported |
| `:hover`, `:focus`, `:checked` (interactive states) | Skipped silently — no DOM events in PDF |
| `::before`, `::after` (with `content:`) | Planned (v0.4) |

### At-rules

| Feature | Status |
|---|---|
| `@font-face` | Supported |
| `@page { margin / size }` | Planned (v0.4) |
| `@media print`, `@media screen` | Planned (v0.4) |
| `@import` | Not yet |
| CSS custom properties (`var(--x)`) | Planned (v0.4) |

### Page

| Feature | Status |
|---|---|
| Multi-page documents | Supported |
| Page sizes (A0–A10, Letter, Legal, Tabloid, B-series) | Supported |
| Custom page sizes (e.g. `"210mm 297mm"`) | Supported |
| Configurable margins | Supported |
| PDF metadata (title, author) | Supported |
| `@page` rules | Planned (v0.4) |

> Until `@page` and `@media print` land, page size and margins are configured via `Options(page_size=…, margin=…)`.

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

The `Engine` class amortizes font scan cost across renders — first render bootstraps the system font cache (~100 ms), subsequent renders complete in single-digit to low-double-digit milliseconds for typical invoice/report-sized documents. PDFs stay small because only used glyphs are embedded.

The repo includes [`bench/compare.py`](bench/compare.py) which times ferropdf vs WeasyPrint on a few fixtures. Re-run yourself rather than trusting numbers from a README:

```bash
pip install ferropdf weasyprint
python bench/compare.py
```

ferropdf is significantly faster on simple-layout documents because its CSS surface is much smaller than WeasyPrint's. The gap narrows as you exercise CSS features ferropdf doesn't yet implement (gradients, transforms, advanced selectors). For honest comparisons, time **your** workload on **your** hardware.

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
cargo machete   # checks for unused workspace dependencies
```

**MSRV**: Rust 1.85+ (set in `[workspace.package]`).

---

## License

MIT
