# Template corpus

Real-world style documents used to measure how ferropdf behaves on HTML that
people actually feed it — not hand-tuned fixtures. Each template deliberately
exercises a family of CSS that shows up in the wild:

| Template | What it exercises |
| --- | --- |
| `invoice_bootstrap` | Custom properties, `calc()`, flex grid, inline-block badges/buttons, zebra tables |
| `invoice_utility` | Tailwind-style utility classes, `gap`, `divide-y` sibling selectors, `text-transform`, `letter-spacing` |
| `invoice_legacy_float` | Float-based header layout, `clear`, `colspan`, `white-space: nowrap` |
| `report_long` | 10+ pages: cover page, `@page` margin boxes + counters, `break-*`, long table with repeated `<thead>`, `justify`, orphans/widows |
| `receipt_narrow` | Custom `@page` size (80 mm roll), monospace, dashed rules |
| `letter` | Serif typography, `text-indent`, `justify`, block positioning |
| `certificate` | Landscape page, `double` border, gradient background, `transform`, border-radius circle |
| `invoice_arabic_rtl` | `dir="rtl"`, Arabic shaping, mixed digits, RTL tables |
| `dashboard_grid` | CSS grid, inline `<svg>`, progress bars with `overflow: hidden` |
| `email_style_in_body` | `<style>` inside `<body>`, table-based email layout, pretty-printed whitespace |

Templates use the DejaVu font family explicitly so renders are comparable across
machines (`apt install fonts-dejavu-core`).

## Tools

* `tests/test_visual.py` — visual regression against `reference/*.png`
  (opt-in: `FERROPDF_VISUAL=1 pytest tests/test_visual.py`).
* `python tests/corpus/report.py OUT_DIR` — renders every template with
  ferropdf and, when installed, WeasyPrint and Chromium (Playwright), and
  writes a side-by-side HTML report. This is the quickest way to see what
  breaks.
