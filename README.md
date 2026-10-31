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

A native Rust HTML-to-PDF engine exposed as a Python package via PyO3. Releases the GIL during render so it composes cleanly with FastAPI, Django async views, and threadpools. System fonts are discovered once per process and shared by every `Engine` and by `from_html()`; each render checks out its own font system from a pool, so threads sharing an `Engine` render in parallel.

| Feature | ferropdf | WeasyPrint |
|---|---|---|
| **GIL** | Released during render | Held |
| **Async-safe** | Yes (FastAPI, Django) | No |
| **Font subsetting** | Automatic | Manual |
| **Install** | `pip install ferropdf` | System deps required |

ferropdf intentionally targets **document rendering**, not full-fidelity browser layout. For honest performance comparisons, run [`bench/engines.py`](bench/engines.py) against your own workload — see [Performance](#performance) below.

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

# System fonts are loaded once per process — later renders are fast
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

## Headers, footers and page numbers

Running headers and footers use standard CSS Paged Media margin boxes:

```css
@page {
  size: A4;
  margin: 22mm 18mm;
  @top-right     { content: "Q3 2026 Business Review"; font-size: 8pt; color: #888 }
  @bottom-center { content: "Page " counter(page) " of " counter(pages) }
}
@page :first { @top-right { content: none } }          /* no header on the cover */
@page :left  { @bottom-left { content: counter(page, lower-roman) } }
```

For a header with HTML — a logo next to an address — take an element out of the flow with
`position: running(name)` and place it with `content: element(name)`:

```html
<style>
  .page-header { position: running(pageHeader); display: flex; justify-content: space-between }
  @page { margin-top: 30mm; @top-center { content: element(pageHeader) } }
</style>
<div class="page-header"><img src="logo.png" height="24"> <span>12 rue des Lilas, Paris</span></div>
```

The running element is laid out at the content width and shown on every page.

Supported boxes: `@top-left`, `@top-center`, `@top-right`, `@bottom-left`, `@bottom-center`,
`@bottom-right`; `content` accepts strings, `counter(page)` and `counter(pages)` with
`decimal`, `decimal-leading-zero`, `lower-/upper-roman` and `lower-/upper-alpha` styles; the
boxes take `font-*`, `color` and `text-align`, and inherit from the `<html>` element.
`:first`, `:left` and `:right` select pages for margin boxes (page size and margins are
document-wide).

---

## CSS support

ferropdf uses industry-standard libraries for parsing and layout — not a hand-rolled engine.
The table below is **generated from the test suite** ([`tests/test_feature_matrix.py`](tests/test_feature_matrix.py)):
every row is a test that renders a snippet and checks the PDF, and every ❌ row is a strict
`xfail` linked to its issue, so the table cannot silently drift from reality.

<!-- feature-matrix:start -->
| Category | Feature | Status |
| --- | --- | --- |
| Page | `@page` size | ✅ Supported |
| Page | `@page` size with `landscape` | ✅ Supported |
| Page | `@page` margin | ✅ Supported |
| Page | `@page` margin shorthand (2–4 values) | ✅ Supported |
| Page | `Options(page_size=...)` | ✅ Supported |
| Page | `@page` margin boxes (headers / footers) | ✅ Supported |
| Page | `counter(page)` / `counter(pages)` | ✅ Supported |
| Page | `@page :first` margins (cover page) | ✅ Supported |
| Page | `@page :left` / `:right` margins (binding) | ✅ Supported |
| Page | Per-page margins change how much fits on a page | ✅ Supported |
| Page | HTML headers/footers: `position: running()` + `element()` | ✅ Supported |
| Page | `@page :first / :left / :right` margin boxes | ✅ Supported |
| Fragmentation | Automatic pagination of many blocks | ✅ Supported |
| Fragmentation | `page-break-before: always` (legacy) | ✅ Supported |
| Fragmentation | `break-before: page` | ✅ Supported |
| Fragmentation | `break-after: page` | ✅ Supported |
| Fragmentation | Paragraph taller than a page splits across pages | ✅ Supported |
| Fragmentation | `<thead>` repeated on each page | ✅ Supported |
| Fragmentation | Multi-row `<thead>` repeated without overlap | ✅ Supported |
| Fragmentation | `<tfoot>` repeated at the bottom of each page | ✅ Supported |
| Cascade | Custom properties `var()` | ✅ Supported |
| Cascade | `calc()` | ✅ Supported |
| Cascade | `calc()` mixing `%` and lengths, with `var()` | ✅ Supported |
| Cascade | `::before` / `::after` with literal `content` | ✅ Supported |
| Cascade | Author CSS overrides tag defaults (`a`, `b`, `span`) | ✅ Supported |
| Cascade | CSS counters in `content` | ✅ Supported |
| Cascade | Section numbering (`counter-reset` scopes, counter styles) | ✅ Supported |
| Cascade | Nested counters with `counters()` | ✅ Supported |
| Cascade | `::before` / `::after` take their own styles | ✅ Supported |
| Cascade | `attr()` and quotes in `content` | ✅ Supported |
| Cascade | `@media print` | ✅ Supported |
| Cascade | Width media queries (`min-width`) | ❌ Not yet ([#29](https://github.com/MoncefMak/ferropdf/issues/29)) |
| Cascade | `<style>` inside `<body>` is not rendered | ✅ Supported |
| Text | Bold / italic | ✅ Supported |
| Text | `text-align: right` | ✅ Supported |
| Text | `text-align: center` | ✅ Supported |
| Text | `text-align: justify` | ✅ Supported |
| Text | `text-align: start` follows `direction` | ✅ Supported |
| Text | Whitespace collapsing (`white-space: normal`) | ✅ Supported |
| Text | `white-space: pre` | ✅ Supported |
| Text | `white-space: nowrap` | ✅ Supported |
| Text | `letter-spacing` | ❌ Not yet ([#34](https://github.com/MoncefMak/ferropdf/issues/34)) |
| Text | `text-transform` | ❌ Not yet ([#34](https://github.com/MoncefMak/ferropdf/issues/34)) |
| Text | `text-indent` | ❌ Not yet ([#34](https://github.com/MoncefMak/ferropdf/issues/34)) |
| Text | Arabic shaping + RTL alignment | ✅ Supported |
| Text | Lists (disc, decimal, roman) | ✅ Supported |
| Layout | Flexbox row | ✅ Supported |
| Layout | `justify-content: space-between` | ✅ Supported |
| Layout | `direction: rtl` mirrors flex rows | ✅ Supported |
| Layout | Inline elements as flex items | ✅ Supported |
| Layout | CSS grid columns | ✅ Supported |
| Layout | Grid placement (`span`, `grid-area`, row/column gaps) | ✅ Supported |
| Layout | Grid `repeat(auto-fill, minmax())` | ✅ Supported |
| Layout | White space between flex items ignored | ✅ Supported |
| Layout | `inline-block` shrink-to-fit next to text | ✅ Supported |
| Layout | `inline-block` padding around its label | ✅ Supported |
| Layout | Tables with `colspan` | ✅ Supported |
| Layout | `direction: rtl` mirrors table / grid columns | ✅ Supported |
| Layout | `position: absolute` | ✅ Supported |
| Layout | `float` (text beside a floated box) | ✅ Supported |
| Layout | `float` left/right header + `clear` | ✅ Supported |
| Layout | `overflow: hidden` | ✅ Supported |
| Layout | Hidden child keeps the inline flow | ✅ Supported |
| Layout | `inline-block` shrink-wraps mixed inline content | ✅ Supported |
| Visual | `overflow: hidden` with `border-radius` (round clip) | ✅ Supported |
| Layout | `box-sizing` (content-box default, border-box) | ✅ Supported |
| Visual | Solid borders | ✅ Supported |
| Visual | Dashed / dotted borders | ✅ Supported |
| Visual | Double borders | ✅ Supported |
| Visual | `border-radius` | ✅ Supported |
| Visual | `border-radius` percentages (circles, ellipses) | ✅ Supported |
| Visual | Per-corner and elliptical `border-radius` | ✅ Supported |
| Visual | `box-shadow` | ✅ Supported |
| Visual | `box-shadow` not painted under the box | ✅ Supported |
| Visual | `opacity` | ✅ Supported |
| Visual | Linear gradients (angles, `to <side>` / `to <corner>`) | ✅ Supported |
| Visual | Radial gradients | ✅ Supported |
| Visual | Repeating gradients | ✅ Supported |
| Visual | Translucent gradient stops over `background-color` | ✅ Supported |
| Visual | `transform: rotate()` | ✅ Supported |
| Visual | `transform: translate()` with percentages | ✅ Supported |
| Visual | `transform: scale()` with `transform-origin` | ✅ Supported |
| Visual | Individual `translate` / `rotate` / `scale` properties | ✅ Supported |
| Visual | Transformed text stays selectable | ✅ Supported |
| Images | PNG / JPEG `<img>` | ✅ Supported |
| Images | `<img width height>` attributes | ✅ Supported |
| Images | Intrinsic image size and aspect ratio | ✅ Supported |
| Images | PNG transparency (alpha) | ✅ Supported |
| Images | SVG `<img>` (vector) | ✅ Supported |
| Images | Inline `<svg>` (vector) | ✅ Supported |
| PDF | Title / author metadata | ✅ Supported |
| PDF | Fonts from a `.ttc` collection (any face) | ✅ Supported |
| PDF | CFF / `.otf` fonts embedded as OpenType (FontFile3) | ✅ Supported |
| PDF | Ligatures extract as their letters | ✅ Supported |
| PDF | Subset fonts carry a subset tag (`ABCDEF+Name`) | ✅ Supported |
| PDF | Clickable links | ❌ Not yet ([#40](https://github.com/MoncefMak/ferropdf/issues/40)) |
| PDF | Outline / bookmarks | ❌ Not yet ([#41](https://github.com/MoncefMak/ferropdf/issues/41)) |
<!-- feature-matrix:end -->

Regenerate after changing a feature: `python tests/test_feature_matrix.py`.
See the [roadmap milestones](https://github.com/MoncefMak/ferropdf/milestones) for what is planned next.

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

System fonts are scanned once per process — the first render bootstraps the font cache (~100 ms, with the GIL released), subsequent renders (from any `Engine`, any thread) complete in single-digit to low-double-digit milliseconds for typical invoice/report-sized documents. PDFs stay small because only used glyphs are embedded. `@font-face` fonts are private to the document that declares them.

[`bench/engines.py`](bench/engines.py) benchmarks ferropdf, WeasyPrint, PlutoPrint and Chromium (Playwright) on the
[template corpus](tests/corpus/) — warm and cold render time, peak memory, threaded throughput and install
footprint, each engine in an isolated process. Latest results: [`bench/RESULTS.md`](bench/RESULTS.md).
Re-run it yourself rather than trusting numbers from a README:

```bash
pip install ferropdf weasyprint plutoprint playwright psutil && playwright install chromium
python bench/engines.py --markdown my-results.md
```

To compare *output quality* side by side, render the corpus with every engine:

```bash
python tests/corpus/report.py /tmp/report && open /tmp/report/index.html
```

ferropdf is significantly faster on simple-layout documents because its CSS surface is much smaller than WeasyPrint's. The gap narrows as you exercise CSS features ferropdf doesn't yet implement (advanced selectors, some text properties). For honest comparisons, time **your** workload on **your** hardware.

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
