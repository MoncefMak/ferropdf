# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
