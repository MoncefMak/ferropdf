# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed
- White space between flex/grid items (indented HTML) created invisible items: extra gaps, wrong `space-between`/`flex: 1` distribution, shifted grid cells.
- `gap: <row> <column>` with two values was ignored; `align-items`/`align-self: start | end` fell back to `stretch`.
- A panic inside the engine now raises `LayoutError` instead of a `PanicException` that escapes `except Exception`.
- Tables sized their columns against the page width instead of their container, overflowing padded or narrower parents.
- PNG transparency was lost (transparent areas rendered black); alpha is kept as a PDF soft mask.
- SVG element names lost their case in the HTML parser (`linearGradient`, `clipPath`).
- `@page { margin }` and `Options(margin=)` accepted a single value only: `margin: 10mm 20mm`, three/four values and unitless `0` all fell back to 20mm.
- `<img>` without CSS width/height was drawn 0×0: the `width`/`height` attributes and the intrinsic image size (and its aspect ratio, e.g. with `max-width: 100%`) are now used.
- Inline elements inside a flex/grid container were merged into one paragraph instead of being separate items (`display:flex` + `<span>`s).
- Multi-row `<thead>` overlapped on continuation pages; a header left alone at the bottom of a page now moves with the first body row; content after a multi-page table no longer overlaps its last rows.
- `<thead>`/`<tfoot>` are laid out first/last wherever they appear in the source.
- `text-align: right | center` had no effect since 0.2.7; `justify` now works (#48, #33).
- Whitespace collapsing per `white-space` (`normal`, `nowrap`, `pre`, `pre-wrap`, `pre-line`) (#13, #16).
- `<style>`, `<script>` and other non-rendered elements inside `<body>` were printed (#14).
- `break-before` / `break-after` / `break-inside` (#15); `@page { size: A4 landscape }` (#56).
- Paragraphs taller than a page were lost, and a line was dropped at page boundaries (#12).
- Table columns now respect the table width; nested tables no longer drawn off-page.
- A shared `Engine` serialised concurrent renders on a global font lock (#53).
- `from_html()`, `from_file()`, `write_pdf()` and the Django/FastAPI helpers reloaded system fonts on every call (#54).
- `@font-face` fonts leaked from one document into later renders of the same engine.
- Tag defaults (`<a>` underline, `<b>` bold, `<span>` inline…) overrode author CSS: `span { display: inline-block }`, `a { text-decoration: none }` had no effect.
- `border-radius` is painted (backgrounds, uniform borders, shadows), radii clamped so `9999px` pills work (#49).
- `dashed`, `dotted` and `double` borders (#19).
- Outer `box-shadow` was also painted under the box, showing through transparent backgrounds.
- `inline-block` elements: padding ignored around their label, full width after a block sibling, spaces next to them dropped (#17).

### Changed
- `pdf-writer` 0.9 → 0.10 (required by `svg2pdf` 0.11).
- **`box-sizing` is now implemented and defaults to `content-box`, like browsers** (the UA sheet used to declare `* { box-sizing: border-box }`, and the property itself was ignored). Documents that relied on the old behaviour should add `* { box-sizing: border-box }` (#50).
- System fonts are loaded once per process and shared by all engines; each render uses a pooled font system.
- PyO3 0.24 → 0.29 (RUSTSEC-2026-0176, RUSTSEC-2026-0177).

### Added
- 2D transforms: `transform` (`translate*`, `scale*`, `rotate`, `skew*`, `matrix`; 3D functions flattened onto the page), the individual `translate` / `rotate` / `scale` properties and `transform-origin`. Applied as a PDF transformation matrix around the box and its descendants, so text stays vector and selectable (#38).
- Gradients: `linear-gradient()`, `radial-gradient()` and their `repeating-` variants in `background` / `background-image` — angles, `to <side>` / `to <corner>`, circle / ellipse with size keywords and `at <position>`, multi-position and hard stops, translucent stops (soft mask), stacked layers, `border-radius` clipping. Encoded as native PDF axial / radial shadings (#37).
- `overflow`, `overflow-x`, `overflow-y` (`hidden`, `clip`; `auto`/`scroll` clip as in print): content is clipped to the padding box, following the inner `border-radius` (#36).
- `float: left | right` and `clear` (emulated: floats are placed on lines in source order; content beside a float stays in a column next to it rather than wrapping underneath) (#35).
- CSS Grid: `grid-template-columns/rows` (`fr`, lengths, `%`, `auto`, `min-/max-content`, `minmax()`, `fit-content()`, `repeat()` incl. `auto-fill`/`auto-fit`), `grid-column`/`grid-row`/`grid-area` (lines, `span`), `grid-auto-rows/columns/flow`, `justify-items`/`justify-self`, `place-items`/`place-self`, `grid-gap` aliases (#18).
- `calc()` in lengths: `+ - * /`, parentheses, nested `calc()`, mixed units (px, pt, pc, mm, cm, in, Q, em, rem, %), with `var()`; `cm` and `in` units in plain lengths (#52).
- SVG images — `<img src="logo.svg">`, SVG data URIs and inline `<svg>` — embedded as **vector** graphics via `svg2pdf` (gradients, paths, text) (#32).
- HTML headers/footers: `position: running(name)` + `content: element(name)` in margin boxes (#28).
- `@page { margin-top | margin-right | margin-bottom | margin-left }`.
- `<tfoot>` is repeated at the bottom of every page a table spans (#30).
- `@page` margin boxes (`@top-left|center|right`, `@bottom-left|center|right`) with `counter(page)` / `counter(pages)` and counter styles (#25, #26).
- `@page :first`, `:left`, `:right` selectors for margin boxes.
- Template corpus, feature-matrix tests (README table generated from them), visual regression tests, multi-engine benchmark (`bench/engines.py`).

## [0.2.9] - 2026-04

### Fixed
- `page_size` option now supports A6 and custom dimensions like `"210mm 297mm"` (#11).

## [0.2.8]

### Performance
- Batched glyph rendering replaces per-glyph `begin_text`/`end_text`, reducing PDF size and render time on text-heavy pages.

## [0.2.7]

### Added
- Arabic shaping with `cosmic-text` + `rustybuzz` (#4–#10).
- `direction: rtl` support.
- `box-shadow` (basic offset/blur/color).
- `position: absolute` and `position: relative`.
- `@font-face` (data: URIs and base_url-resolved file paths).

## [0.2.6]

### Style
- rustfmt cleanup to satisfy CI lint.

## [0.2.5]

### Added
- Full benchmark suite under `bench/`.
- Tests for DOM manipulation, rendering warnings, and CSS cascade resolution.

### Changed
- Various function-level performance refactors.

## [0.2.4]

Internal CI cleanup.

## [0.2.3]

### Fixed
- Clippy warnings (tuple pattern deref, manual_strip).

## [0.2.2]

### Security
- Upgraded PyO3 from 0.21 to 0.24 (RUSTSEC-2025-0020).

### Added
- Comprehensive README with API reference, CSS support tables, architecture, examples.

## [0.2.1]

### Fixed
- aarch64 cross-compile (CI: `--find-interpreter`).

## [0.2.0]

### Added
- Font subsetting + per-engine font cache.
- Pagination and table layout algorithms.
- Border-collapse, colspan, list markers, `@font-face`.

### Performance
- 8–13× faster than WeasyPrint on the bundled fixtures.

[Unreleased]: https://github.com/MoncefMak/ferropdf/compare/v0.2.9...HEAD
[0.2.9]: https://github.com/MoncefMak/ferropdf/compare/v0.2.8...v0.2.9
[0.2.8]: https://github.com/MoncefMak/ferropdf/compare/v0.2.7...v0.2.8
[0.2.7]: https://github.com/MoncefMak/ferropdf/compare/v0.2.6...v0.2.7
[0.2.6]: https://github.com/MoncefMak/ferropdf/compare/v0.2.5...v0.2.6
[0.2.5]: https://github.com/MoncefMak/ferropdf/compare/v0.2.4...v0.2.5
[0.2.4]: https://github.com/MoncefMak/ferropdf/compare/v0.2.3...v0.2.4
[0.2.3]: https://github.com/MoncefMak/ferropdf/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/MoncefMak/ferropdf/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/MoncefMak/ferropdf/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/MoncefMak/ferropdf/releases/tag/v0.2.0
