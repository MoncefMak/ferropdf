# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed
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
- **`box-sizing` is now implemented and defaults to `content-box`, like browsers** (the UA sheet used to declare `* { box-sizing: border-box }`, and the property itself was ignored). Documents that relied on the old behaviour should add `* { box-sizing: border-box }` (#50).
- System fonts are loaded once per process and shared by all engines; each render uses a pooled font system.
- PyO3 0.24 → 0.29 (RUSTSEC-2026-0176, RUSTSEC-2026-0177).

### Added
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
