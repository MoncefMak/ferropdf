window.BENCHMARK_DATA = {
  "lastUpdate": 1775202607666,
  "repoUrl": "https://github.com/MoncefMak/ferropdf",
  "entries": {
    "FastPDF Criterion Benchmarks": [
      {
        "commit": {
          "author": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "committer": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "distinct": true,
          "id": "34dce18a315e7a9e5c11e979337edb207ad885c1",
          "message": "ci: fix git identity for gh-pages init + add workflow_dispatch to CI/Release",
          "timestamp": "2026-03-15T19:17:48+01:00",
          "tree_id": "df6fbd9ee48c0a969a690dca0f7902c6e017ce38",
          "url": "https://github.com/MoncefMak/ferropdf/commit/34dce18a315e7a9e5c11e979337edb207ad885c1"
        },
        "date": 1773599041843,
        "tool": "cargo",
        "benches": [
          {
            "name": "01_parse/html_simple",
            "value": 4152,
            "range": "± 126",
            "unit": "ns/iter"
          },
          {
            "name": "01_parse/html_complex",
            "value": 50458,
            "range": "± 379",
            "unit": "ns/iter"
          },
          {
            "name": "01_parse/css_basic",
            "value": 5380,
            "range": "± 13",
            "unit": "ns/iter"
          },
          {
            "name": "01_parse/css_complex",
            "value": 9286,
            "range": "± 64",
            "unit": "ns/iter"
          },
          {
            "name": "01_parse/default_stylesheet",
            "value": 3515,
            "range": "± 8",
            "unit": "ns/iter"
          },
          {
            "name": "02_full_pipeline/simple_html",
            "value": 113508,
            "range": "± 1272",
            "unit": "ns/iter"
          },
          {
            "name": "02_full_pipeline/styled_html",
            "value": 298124,
            "range": "± 2809",
            "unit": "ns/iter"
          },
          {
            "name": "02_full_pipeline/complex_report",
            "value": 982953,
            "range": "± 7889",
            "unit": "ns/iter"
          },
          {
            "name": "03_tables/rows/10",
            "value": 631793,
            "range": "± 3348",
            "unit": "ns/iter"
          },
          {
            "name": "03_tables/rows/25",
            "value": 1324379,
            "range": "± 5448",
            "unit": "ns/iter"
          },
          {
            "name": "03_tables/rows/50",
            "value": 2450454,
            "range": "± 14176",
            "unit": "ns/iter"
          },
          {
            "name": "03_tables/rows/100",
            "value": 4687730,
            "range": "± 30349",
            "unit": "ns/iter"
          },
          {
            "name": "04_tailwind/extract_classes",
            "value": 85315,
            "range": "± 1005",
            "unit": "ns/iter"
          },
          {
            "name": "04_tailwind/resolve_classes",
            "value": 38970,
            "range": "± 306",
            "unit": "ns/iter"
          },
          {
            "name": "05_stages/layout_complex",
            "value": 361188,
            "range": "± 3728",
            "unit": "ns/iter"
          },
          {
            "name": "05_stages/render_paint_cmds",
            "value": 36092,
            "range": "± 239",
            "unit": "ns/iter"
          },
          {
            "name": "05_stages/pdf_generate",
            "value": 439588,
            "range": "± 1724",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "committer": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "distinct": true,
          "id": "0f58dbc71c86e7cec0bcd42e5efc086120524b0b",
          "message": "Refactor PDF rendering engine and update benchmarks\n\n- Updated benchmark results for various PDF rendering endpoints, showing improved performance metrics.\n- Enhanced `RenderOptions` class to include `orientation` attribute for page layout configuration.\n- Modified `PdfEngine` methods to return `PdfDocument` objects instead of raw bytes, encapsulating PDF data and page count.\n- Implemented support for additional page sizes (A3, A5, Tabloid) in the pagination layout.\n- Improved error handling and logging in the rendering pipeline.\n- Updated Rust dependencies and removed unused ones to streamline the build process.",
          "timestamp": "2026-03-16T01:41:00+01:00",
          "tree_id": "094439801d3e169f9e6e9f9a0384cfb5de84de12",
          "url": "https://github.com/MoncefMak/ferropdf/commit/0f58dbc71c86e7cec0bcd42e5efc086120524b0b"
        },
        "date": 1773621970524,
        "tool": "cargo",
        "benches": [
          {
            "name": "01_parse/html_simple",
            "value": 4178,
            "range": "± 28",
            "unit": "ns/iter"
          },
          {
            "name": "01_parse/html_complex",
            "value": 52446,
            "range": "± 558",
            "unit": "ns/iter"
          },
          {
            "name": "01_parse/css_basic",
            "value": 5713,
            "range": "± 14",
            "unit": "ns/iter"
          },
          {
            "name": "01_parse/css_complex",
            "value": 9742,
            "range": "± 502",
            "unit": "ns/iter"
          },
          {
            "name": "01_parse/default_stylesheet",
            "value": 5211,
            "range": "± 21",
            "unit": "ns/iter"
          },
          {
            "name": "02_full_pipeline/simple_html",
            "value": 117871,
            "range": "± 558",
            "unit": "ns/iter"
          },
          {
            "name": "02_full_pipeline/styled_html",
            "value": 320966,
            "range": "± 1238",
            "unit": "ns/iter"
          },
          {
            "name": "02_full_pipeline/complex_report",
            "value": 1074235,
            "range": "± 15531",
            "unit": "ns/iter"
          },
          {
            "name": "03_tables/rows/10",
            "value": 714855,
            "range": "± 2352",
            "unit": "ns/iter"
          },
          {
            "name": "03_tables/rows/25",
            "value": 1491104,
            "range": "± 3694",
            "unit": "ns/iter"
          },
          {
            "name": "03_tables/rows/50",
            "value": 2798743,
            "range": "± 6453",
            "unit": "ns/iter"
          },
          {
            "name": "03_tables/rows/100",
            "value": 5354876,
            "range": "± 40369",
            "unit": "ns/iter"
          },
          {
            "name": "04_tailwind/extract_classes",
            "value": 7899,
            "range": "± 58",
            "unit": "ns/iter"
          },
          {
            "name": "04_tailwind/resolve_classes",
            "value": 9133,
            "range": "± 73",
            "unit": "ns/iter"
          },
          {
            "name": "05_stages/layout_complex",
            "value": 443090,
            "range": "± 1383",
            "unit": "ns/iter"
          },
          {
            "name": "05_stages/render_paint_cmds",
            "value": 35729,
            "range": "± 186",
            "unit": "ns/iter"
          },
          {
            "name": "05_stages/pdf_generate",
            "value": 438642,
            "range": "± 13154",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "72460183+MoncefMak@users.noreply.github.com",
            "name": "MoncefMak",
            "username": "MoncefMak"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "85e0bfedbf84b4a72bec260847f9c14b63105643",
          "message": "Merge pull request #1 from MoncefMak/fix/clippy-lint-and-code-quality\n\nfix: resolve all clippy warnings and rustfmt issues",
          "timestamp": "2026-03-16T07:56:09+01:00",
          "tree_id": "24d5d293c0280ad05fed3b5820f7abc786b4dd2a",
          "url": "https://github.com/MoncefMak/ferropdf/commit/85e0bfedbf84b4a72bec260847f9c14b63105643"
        },
        "date": 1773644469062,
        "tool": "cargo",
        "benches": [
          {
            "name": "01_parse/html_simple",
            "value": 4087,
            "range": "± 216",
            "unit": "ns/iter"
          },
          {
            "name": "01_parse/html_complex",
            "value": 51416,
            "range": "± 512",
            "unit": "ns/iter"
          },
          {
            "name": "01_parse/css_basic",
            "value": 5960,
            "range": "± 78",
            "unit": "ns/iter"
          },
          {
            "name": "01_parse/css_complex",
            "value": 9768,
            "range": "± 51",
            "unit": "ns/iter"
          },
          {
            "name": "01_parse/default_stylesheet",
            "value": 3918,
            "range": "± 10",
            "unit": "ns/iter"
          },
          {
            "name": "02_full_pipeline/simple_html",
            "value": 118544,
            "range": "± 913",
            "unit": "ns/iter"
          },
          {
            "name": "02_full_pipeline/styled_html",
            "value": 320010,
            "range": "± 5838",
            "unit": "ns/iter"
          },
          {
            "name": "02_full_pipeline/complex_report",
            "value": 1074576,
            "range": "± 17839",
            "unit": "ns/iter"
          },
          {
            "name": "03_tables/rows/10",
            "value": 713139,
            "range": "± 1789",
            "unit": "ns/iter"
          },
          {
            "name": "03_tables/rows/25",
            "value": 1491282,
            "range": "± 3861",
            "unit": "ns/iter"
          },
          {
            "name": "03_tables/rows/50",
            "value": 2803479,
            "range": "± 11193",
            "unit": "ns/iter"
          },
          {
            "name": "03_tables/rows/100",
            "value": 5354591,
            "range": "± 27365",
            "unit": "ns/iter"
          },
          {
            "name": "04_tailwind/extract_classes",
            "value": 7869,
            "range": "± 58",
            "unit": "ns/iter"
          },
          {
            "name": "04_tailwind/resolve_classes",
            "value": 9047,
            "range": "± 332",
            "unit": "ns/iter"
          },
          {
            "name": "05_stages/layout_complex",
            "value": 449506,
            "range": "± 7226",
            "unit": "ns/iter"
          },
          {
            "name": "05_stages/render_paint_cmds",
            "value": 35138,
            "range": "± 175",
            "unit": "ns/iter"
          },
          {
            "name": "05_stages/pdf_generate",
            "value": 439435,
            "range": "± 1789",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "72460183+MoncefMak@users.noreply.github.com",
            "name": "MoncefMak",
            "username": "MoncefMak"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "69094e90f49f81cffedfec777e92fb6f171b1d9d",
          "message": "Merge pull request #2 from MoncefMak/fix/clippy-lint-and-code-quality\n\nfix: convert batch speedup assertion to warning for CI stability",
          "timestamp": "2026-03-16T08:15:32+01:00",
          "tree_id": "281098913f0c169c2893deb7d48605844e7ebaa2",
          "url": "https://github.com/MoncefMak/ferropdf/commit/69094e90f49f81cffedfec777e92fb6f171b1d9d"
        },
        "date": 1773645624084,
        "tool": "cargo",
        "benches": [
          {
            "name": "01_parse/html_simple",
            "value": 4222,
            "range": "± 36",
            "unit": "ns/iter"
          },
          {
            "name": "01_parse/html_complex",
            "value": 52238,
            "range": "± 241",
            "unit": "ns/iter"
          },
          {
            "name": "01_parse/css_basic",
            "value": 6031,
            "range": "± 45",
            "unit": "ns/iter"
          },
          {
            "name": "01_parse/css_complex",
            "value": 10345,
            "range": "± 41",
            "unit": "ns/iter"
          },
          {
            "name": "01_parse/default_stylesheet",
            "value": 3928,
            "range": "± 157",
            "unit": "ns/iter"
          },
          {
            "name": "02_full_pipeline/simple_html",
            "value": 118018,
            "range": "± 695",
            "unit": "ns/iter"
          },
          {
            "name": "02_full_pipeline/styled_html",
            "value": 320656,
            "range": "± 1675",
            "unit": "ns/iter"
          },
          {
            "name": "02_full_pipeline/complex_report",
            "value": 1076436,
            "range": "± 10632",
            "unit": "ns/iter"
          },
          {
            "name": "03_tables/rows/10",
            "value": 713563,
            "range": "± 6765",
            "unit": "ns/iter"
          },
          {
            "name": "03_tables/rows/25",
            "value": 1500038,
            "range": "± 6328",
            "unit": "ns/iter"
          },
          {
            "name": "03_tables/rows/50",
            "value": 2812139,
            "range": "± 11855",
            "unit": "ns/iter"
          },
          {
            "name": "03_tables/rows/100",
            "value": 5367334,
            "range": "± 58083",
            "unit": "ns/iter"
          },
          {
            "name": "04_tailwind/extract_classes",
            "value": 7962,
            "range": "± 96",
            "unit": "ns/iter"
          },
          {
            "name": "04_tailwind/resolve_classes",
            "value": 9115,
            "range": "± 97",
            "unit": "ns/iter"
          },
          {
            "name": "05_stages/layout_complex",
            "value": 443519,
            "range": "± 1536",
            "unit": "ns/iter"
          },
          {
            "name": "05_stages/render_paint_cmds",
            "value": 34451,
            "range": "± 188",
            "unit": "ns/iter"
          },
          {
            "name": "05_stages/pdf_generate",
            "value": 433727,
            "range": "± 5469",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "72460183+MoncefMak@users.noreply.github.com",
            "name": "MoncefMak",
            "username": "MoncefMak"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "7bea2bb4a6039d53a4c13f4aa923aa8577bf2dab",
          "message": "Merge pull request #3 from MoncefMak/fix/benchmark-gh-pages-checkout\n\nfix: clean working directory before gh-pages branch switch",
          "timestamp": "2026-03-16T12:53:03+01:00",
          "tree_id": "014596ef15b3e43a30d37fe2acacfc65f6d2645a",
          "url": "https://github.com/MoncefMak/ferropdf/commit/7bea2bb4a6039d53a4c13f4aa923aa8577bf2dab"
        },
        "date": 1773662283350,
        "tool": "cargo",
        "benches": [
          {
            "name": "01_parse/html_simple",
            "value": 4151,
            "range": "± 50",
            "unit": "ns/iter"
          },
          {
            "name": "01_parse/html_complex",
            "value": 51291,
            "range": "± 761",
            "unit": "ns/iter"
          },
          {
            "name": "01_parse/css_basic",
            "value": 5564,
            "range": "± 26",
            "unit": "ns/iter"
          },
          {
            "name": "01_parse/css_complex",
            "value": 9563,
            "range": "± 30",
            "unit": "ns/iter"
          },
          {
            "name": "01_parse/default_stylesheet",
            "value": 4873,
            "range": "± 89",
            "unit": "ns/iter"
          },
          {
            "name": "02_full_pipeline/simple_html",
            "value": 117276,
            "range": "± 569",
            "unit": "ns/iter"
          },
          {
            "name": "02_full_pipeline/styled_html",
            "value": 319844,
            "range": "± 1014",
            "unit": "ns/iter"
          },
          {
            "name": "02_full_pipeline/complex_report",
            "value": 1071366,
            "range": "± 22052",
            "unit": "ns/iter"
          },
          {
            "name": "03_tables/rows/10",
            "value": 706190,
            "range": "± 2561",
            "unit": "ns/iter"
          },
          {
            "name": "03_tables/rows/25",
            "value": 1484078,
            "range": "± 12349",
            "unit": "ns/iter"
          },
          {
            "name": "03_tables/rows/50",
            "value": 2790758,
            "range": "± 7582",
            "unit": "ns/iter"
          },
          {
            "name": "03_tables/rows/100",
            "value": 5343842,
            "range": "± 62743",
            "unit": "ns/iter"
          },
          {
            "name": "04_tailwind/extract_classes",
            "value": 8061,
            "range": "± 21",
            "unit": "ns/iter"
          },
          {
            "name": "04_tailwind/resolve_classes",
            "value": 9152,
            "range": "± 97",
            "unit": "ns/iter"
          },
          {
            "name": "05_stages/layout_complex",
            "value": 438354,
            "range": "± 1833",
            "unit": "ns/iter"
          },
          {
            "name": "05_stages/render_paint_cmds",
            "value": 35780,
            "range": "± 148",
            "unit": "ns/iter"
          },
          {
            "name": "05_stages/pdf_generate",
            "value": 440653,
            "range": "± 2162",
            "unit": "ns/iter"
          }
        ]
      }
    ],
    "ferropdf Criterion Benchmarks": [
      {
        "commit": {
          "author": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "committer": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "distinct": true,
          "id": "e1cb20e1d037f1051efa07cbb64e0d7ef10ec962",
          "message": "perf: font subsetting + caching — 8-13x faster than WeasyPrint\n\n- Add font subsetting via subsetter crate (821KB → 5-8KB per font)\n- Use fast zlib compression (level 1 instead of 6)\n- Cache FontDatabase in Engine (OnceLock) for cross-render reuse\n- Share fontdb between cosmic-text layout and PDF writing\n- Add Criterion benchmarks and Python comparison script\n- Fix CI workflows (remove rust-engine references)\n- Bump version to 0.2.1",
          "timestamp": "2026-03-18T19:17:10+01:00",
          "tree_id": "84c1db395d7e2764c4904605fda4abc59b434079",
          "url": "https://github.com/MoncefMak/ferropdf/commit/e1cb20e1d037f1051efa07cbb64e0d7ef10ec962"
        },
        "date": 1773858047603,
        "tool": "cargo",
        "benches": [
          {
            "name": "render_simple",
            "value": 3824394,
            "range": "± 21100",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice",
            "value": 4515476,
            "range": "± 129541",
            "unit": "ns/iter"
          },
          {
            "name": "render_simple_cached",
            "value": 225259,
            "range": "± 3499",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice_cached",
            "value": 867709,
            "range": "± 6323",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "committer": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "distinct": true,
          "id": "ff135df9be48c7500db584ec12bc41befedca7da",
          "message": "fix(ci): add --find-interpreter for aarch64 cross-compile",
          "timestamp": "2026-03-18T19:32:54+01:00",
          "tree_id": "60f530304dc3cbe1163a31100d934e37d98d4231",
          "url": "https://github.com/MoncefMak/ferropdf/commit/ff135df9be48c7500db584ec12bc41befedca7da"
        },
        "date": 1773858890536,
        "tool": "cargo",
        "benches": [
          {
            "name": "render_simple",
            "value": 3920830,
            "range": "± 76996",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice",
            "value": 4611281,
            "range": "± 133838",
            "unit": "ns/iter"
          },
          {
            "name": "render_simple_cached",
            "value": 227820,
            "range": "± 1634",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice_cached",
            "value": 872917,
            "range": "± 7437",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "committer": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "distinct": true,
          "id": "176e50f95936ee1aed5f8a221e31a1a3db9b1a50",
          "message": "ci: auto-deploy on version change in pyproject.toml\n\n- Trigger release workflow on push to main when pyproject.toml changes\n- version-gate job compares pyproject.toml version to existing git tags\n- Auto-creates and pushes vX.Y.Z tag if version is new\n- All build jobs gated behind version-gate\n- workflow_dispatch still available for manual releases",
          "timestamp": "2026-03-18T19:36:35+01:00",
          "tree_id": "846961d890be2f2d049fabec60fd70eacdf58380",
          "url": "https://github.com/MoncefMak/ferropdf/commit/176e50f95936ee1aed5f8a221e31a1a3db9b1a50"
        },
        "date": 1773859123490,
        "tool": "cargo",
        "benches": [
          {
            "name": "render_simple",
            "value": 3972328,
            "range": "± 70112",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice",
            "value": 4686320,
            "range": "± 67182",
            "unit": "ns/iter"
          },
          {
            "name": "render_simple_cached",
            "value": 228337,
            "range": "± 11511",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice_cached",
            "value": 879436,
            "range": "± 11427",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "committer": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "distinct": true,
          "id": "7864677a6a33ea3baf8f21e83eb1873d1bb4635d",
          "message": "fix(ci): resolve all clippy errors, fmt violations, and PyO3 deprecations\n\n- Run cargo fmt across entire workspace\n- Fix clippy::needless_range_loop in pdf.rs glyph width loop\n- Fix clippy::manual_strip with strip_suffix() in page.rs, parser.rs, cascade.rs\n- Fix clippy::derivable_impls for Length and FontWeight defaults\n- Fix clippy::field_reassign_with_default in inherit.rs\n- Add clippy allows for too_many_arguments, large_enum_variant, dead_code\n- Remove 5 dead duplicate functions from taffy_bridge.rs\n- Replace deprecated py.get_type() with py.get_type_bound() (PyO3 0.21)\n- Pin Python interpreters to 3.8-3.12 in release.yml (PyO3 0.21 max)\n- Use Python 3.12 in CI test matrix (not 3.13)",
          "timestamp": "2026-03-18T19:57:59+01:00",
          "tree_id": "18935f7a072edd0310c99acc419db79ee38d686b",
          "url": "https://github.com/MoncefMak/ferropdf/commit/7864677a6a33ea3baf8f21e83eb1873d1bb4635d"
        },
        "date": 1773860394257,
        "tool": "cargo",
        "benches": [
          {
            "name": "render_simple",
            "value": 3834864,
            "range": "± 192897",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice",
            "value": 4510419,
            "range": "± 21681",
            "unit": "ns/iter"
          },
          {
            "name": "render_simple_cached",
            "value": 225363,
            "range": "± 1974",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice_cached",
            "value": 872194,
            "range": "± 6269",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "committer": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "distinct": true,
          "id": "7a63a27a8098da2ba3004455415346c88696f526",
          "message": "security: upgrade PyO3 0.21 → 0.24 (RUSTSEC-2025-0020)\n\n- Fix buffer overflow vulnerability in PyString::from_object\n- Replace deprecated get_type_bound() → get_type(), new_bound() → new()\n- Re-enable Python 3.13 in CI and release builds (now supported)",
          "timestamp": "2026-03-18T23:25:06+01:00",
          "tree_id": "3f29e0937655fc7f3bbc050dda40e0ebfa6614ae",
          "url": "https://github.com/MoncefMak/ferropdf/commit/7a63a27a8098da2ba3004455415346c88696f526"
        },
        "date": 1773872826001,
        "tool": "cargo",
        "benches": [
          {
            "name": "render_simple",
            "value": 3796106,
            "range": "± 28800",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice",
            "value": 4475529,
            "range": "± 54674",
            "unit": "ns/iter"
          },
          {
            "name": "render_simple_cached",
            "value": 222127,
            "range": "± 1312",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice_cached",
            "value": 867720,
            "range": "± 16524",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "committer": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "distinct": true,
          "id": "9e40bf8df33f8043a8b4a145652acc3688a57629",
          "message": "chore: bump version to 0.2.2\n\n- PyO3 0.24 security fix (RUSTSEC-2025-0020)\n- Python 3.13 support\n- All clippy/fmt/audit clean",
          "timestamp": "2026-03-18T23:37:05+01:00",
          "tree_id": "c74dda07e373b9fe37551f337bf544696bc05c95",
          "url": "https://github.com/MoncefMak/ferropdf/commit/9e40bf8df33f8043a8b4a145652acc3688a57629"
        },
        "date": 1773873545517,
        "tool": "cargo",
        "benches": [
          {
            "name": "render_simple",
            "value": 3805191,
            "range": "± 48800",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice",
            "value": 4496886,
            "range": "± 41440",
            "unit": "ns/iter"
          },
          {
            "name": "render_simple_cached",
            "value": 224336,
            "range": "± 1254",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice_cached",
            "value": 873780,
            "range": "± 24140",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "committer": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "distinct": true,
          "id": "3e86f860666bcb18d57e80cd5924b3e8ad5f1c9c",
          "message": "docs: comprehensive README with API, CSS support, architecture, examples",
          "timestamp": "2026-03-18T23:41:19+01:00",
          "tree_id": "c0a47108ed212689af55a96309941245901ab923",
          "url": "https://github.com/MoncefMak/ferropdf/commit/3e86f860666bcb18d57e80cd5924b3e8ad5f1c9c"
        },
        "date": 1773873790045,
        "tool": "cargo",
        "benches": [
          {
            "name": "render_simple",
            "value": 3824155,
            "range": "± 34946",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice",
            "value": 4505535,
            "range": "± 26960",
            "unit": "ns/iter"
          },
          {
            "name": "render_simple_cached",
            "value": 226410,
            "range": "± 4109",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice_cached",
            "value": 877704,
            "range": "± 6203",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "committer": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "distinct": true,
          "id": "1be120a4f9bf0173790d098d073e9a515bcfe4cc",
          "message": "feat: border-collapse, colspan, list markers, pagination, @font-face, text-align fix\n\nPhase 3 — border-collapse, colspan, list-style markers:\n- border-collapse: collapse skips inner cell borders using table_cell_pos\n- colspan support via CellInfo with grid_column span in Taffy\n- list-style-type (disc/circle/square/decimal/roman/alpha) with marker rendering\n\nPhase 4 — table-row-aware pagination + thead repeating:\n- TableRow boxes treated as atomic (never split across pages)\n- fragment_table() paginates row-by-row, clones thead on continuation pages\n- thead_row_count field on LayoutBox via count_thead_rows()\n- Removed dead code: fragment.rs, at_page.rs\n\nPhase 5 — @font-face custom font loading:\n- Parse @font-face rules (font-family, src, font-weight, font-style)\n- Load TTF/OTF from file paths or data: URIs with base64 decoding\n\nFix: text-align container_width in painter.rs:\n- text_content path now uses layout_box.content.width for alignment\n  instead of parent_content_width (which was wrong for table cells)\n- Removed debug eprintln from taffy_bridge.rs and table_layout.rs",
          "timestamp": "2026-03-19T08:53:46+01:00",
          "tree_id": "91f0b6e745220221092545108373062230657c52",
          "url": "https://github.com/MoncefMak/ferropdf/commit/1be120a4f9bf0173790d098d073e9a515bcfe4cc"
        },
        "date": 1773906963078,
        "tool": "cargo",
        "benches": [
          {
            "name": "render_simple",
            "value": 3846446,
            "range": "± 56061",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice",
            "value": 4560888,
            "range": "± 33146",
            "unit": "ns/iter"
          },
          {
            "name": "render_simple_cached",
            "value": 236017,
            "range": "± 4426",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice_cached",
            "value": 918513,
            "range": "± 8034",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "committer": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "distinct": true,
          "id": "1f92e586f1e89b71e59fb7a0d0f91353b2b67abf",
          "message": "chore: bump version to 0.2.3",
          "timestamp": "2026-03-19T09:13:45+01:00",
          "tree_id": "166c1da7a94e1c804cb145e403d4bcc069627571",
          "url": "https://github.com/MoncefMak/ferropdf/commit/1f92e586f1e89b71e59fb7a0d0f91353b2b67abf"
        },
        "date": 1773908167606,
        "tool": "cargo",
        "benches": [
          {
            "name": "render_simple",
            "value": 3805241,
            "range": "± 47933",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice",
            "value": 4511026,
            "range": "± 22828",
            "unit": "ns/iter"
          },
          {
            "name": "render_simple_cached",
            "value": 234324,
            "range": "± 2190",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice_cached",
            "value": 907874,
            "range": "± 3502",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "committer": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "distinct": true,
          "id": "9bf5e1e84d5c99275648bbb47c55248e1953efec",
          "message": "fix: resolve clippy lint warnings (tuple pattern deref, manual_strip)",
          "timestamp": "2026-03-19T09:18:39+01:00",
          "tree_id": "91ed97cddbfae32cea1b1c7470ab31a5efdccee1",
          "url": "https://github.com/MoncefMak/ferropdf/commit/9bf5e1e84d5c99275648bbb47c55248e1953efec"
        },
        "date": 1773908449335,
        "tool": "cargo",
        "benches": [
          {
            "name": "render_simple",
            "value": 3845113,
            "range": "± 115950",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice",
            "value": 4568959,
            "range": "± 105802",
            "unit": "ns/iter"
          },
          {
            "name": "render_simple_cached",
            "value": 235209,
            "range": "± 2466",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice_cached",
            "value": 915696,
            "range": "± 6520",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "committer": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "distinct": true,
          "id": "62f8908c3379ddfee099f573e2e248b1166f40c2",
          "message": "chore: bump version to 0.2.4",
          "timestamp": "2026-03-19T09:49:09+01:00",
          "tree_id": "46f323a6760930647501d3c066edfa020e7c1ebf",
          "url": "https://github.com/MoncefMak/ferropdf/commit/62f8908c3379ddfee099f573e2e248b1166f40c2"
        },
        "date": 1773910283159,
        "tool": "cargo",
        "benches": [
          {
            "name": "render_simple",
            "value": 2629267,
            "range": "± 36763",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice",
            "value": 3372532,
            "range": "± 92942",
            "unit": "ns/iter"
          },
          {
            "name": "render_simple_cached",
            "value": 221948,
            "range": "± 2313",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice_cached",
            "value": 887808,
            "range": "± 5990",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "committer": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "distinct": true,
          "id": "b54b8df7550e85b73ed9af40fb54f1744176231e",
          "message": "ci: trigger v0.2.4 release",
          "timestamp": "2026-03-19T12:44:12+01:00",
          "tree_id": "46f323a6760930647501d3c066edfa020e7c1ebf",
          "url": "https://github.com/MoncefMak/ferropdf/commit/b54b8df7550e85b73ed9af40fb54f1744176231e"
        },
        "date": 1773920786904,
        "tool": "cargo",
        "benches": [
          {
            "name": "render_simple",
            "value": 3909836,
            "range": "± 20033",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice",
            "value": 4644647,
            "range": "± 26508",
            "unit": "ns/iter"
          },
          {
            "name": "render_simple_cached",
            "value": 236790,
            "range": "± 1846",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice_cached",
            "value": 919656,
            "range": "± 9328",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "committer": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "distinct": true,
          "id": "3cafa380d1e5ffd983feb150d46672eda167878b",
          "message": "Remove pagination and table layout modules\n\n- Deleted `pagination.rs`, which contained the algorithm for PDF pagination and fragmentation based on CSS Fragmentation Module Level 3.\n- Deleted `table_layout.rs`, which included the algorithm for CSS table layout, including column width and row height calculations based on CSS 2.1 specifications.",
          "timestamp": "2026-03-23T08:32:36+01:00",
          "tree_id": "cf81f511505678fec979b1a465fb3b2a3fec798c",
          "url": "https://github.com/MoncefMak/ferropdf/commit/3cafa380d1e5ffd983feb150d46672eda167878b"
        },
        "date": 1774392542346,
        "tool": "cargo",
        "benches": [
          {
            "name": "render_simple",
            "value": 3837458,
            "range": "± 28589",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice",
            "value": 4549008,
            "range": "± 88676",
            "unit": "ns/iter"
          },
          {
            "name": "render_simple_cached",
            "value": 238374,
            "range": "± 2125",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice_cached",
            "value": 912752,
            "range": "± 8670",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "committer": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "distinct": true,
          "id": "382a4e1be15c17e32e1ad7f2527261b905b43f61",
          "message": "chore: bump version to 0.2.5, add full benchmark suite\n\nBump all crate versions and pyproject.toml to 0.2.5.\nAdd comprehensive benchmark (bench/benchmark_full.py) covering raw speed,\nengine reuse, concurrent rendering, PDF size, FastAPI/Django simulation,\nand head-to-head comparison with WeasyPrint (15.4x avg speedup).\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-03-28T22:33:42+01:00",
          "tree_id": "2fc98bea27ea208a0885f94074e82eba975004df",
          "url": "https://github.com/MoncefMak/ferropdf/commit/382a4e1be15c17e32e1ad7f2527261b905b43f61"
        },
        "date": 1774733853590,
        "tool": "cargo",
        "benches": [
          {
            "name": "render_simple",
            "value": 3499599,
            "range": "± 37650",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice",
            "value": 4219558,
            "range": "± 19296",
            "unit": "ns/iter"
          },
          {
            "name": "render_simple_cached",
            "value": 235910,
            "range": "± 1968",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice_cached",
            "value": 913011,
            "range": "± 3006",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "committer": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "distinct": true,
          "id": "24c7d99f5d61c872ad146c8a2118ca61f5c0180c",
          "message": "chore: bump version to 0.2.6",
          "timestamp": "2026-03-28T22:57:49+01:00",
          "tree_id": "66ff75ecc4394e953838b60c019d52f84361c004",
          "url": "https://github.com/MoncefMak/ferropdf/commit/24c7d99f5d61c872ad146c8a2118ca61f5c0180c"
        },
        "date": 1774735311297,
        "tool": "cargo",
        "benches": [
          {
            "name": "render_simple",
            "value": 3610435,
            "range": "± 129835",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice",
            "value": 4328791,
            "range": "± 17098",
            "unit": "ns/iter"
          },
          {
            "name": "render_simple_cached",
            "value": 240441,
            "range": "± 2422",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice_cached",
            "value": 926790,
            "range": "± 3596",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "committer": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "distinct": true,
          "id": "c7cb963a5a4921459c51e05df0308952ff89613e",
          "message": "feat: resolve issues #4–#10 — Arabic shaping, box-shadow, position absolute, RTL, font-face\n\n## Issue #4 — Arabic text shaping (ligatures)\n## Issue #5 — Unicode Bidi (mixed Arabic/numbers)\n\nPropagate fontdb::ID from cosmic-text shaped glyphs through the entire\nrender pipeline to the PDF writer. This ensures the exact font binary\nused for shaping is embedded in the PDF (fixes glyph ID mismatch when\ncosmic-text falls back to a different font than the one resolved by\nname). Position glyphs individually using cosmic-text x coordinates\nfor correct RTL visual rendering.\n\n- Added fontdb dep to ferropdf-core; ShapedGlyph now carries font_id\n- DrawOp::DrawText carries full ShapedGlyph vec (not just glyph IDs)\n- PDF writer groups glyphs by font_id, embeds each font independently\n- Added CSS direction property, unicode-bidi, and HTML dir attribute\n- direction is inherited (added to inherit.rs)\n\n## Issue #6 — @font-face local file paths and data URIs\n\n- Strip file:// prefix from font src paths\n- Handle absolute paths directly (no base_url resolution needed)\n- Extract only first url() from multi-source @font-face src\n- Skip format() and other CSS functions in @font-face parsing\n\n## Issue #7 — position: absolute and position: fixed\n\n- No longer forces absolute/fixed to Static in cascade\n- Added CssProperty::Top/Right/Bottom/Left/ZIndex to CSS parser\n- Maps Position::Absolute/Fixed to taffy::Position::Absolute with inset\n- Sets out_of_flow=true for absolute/fixed LayoutBoxes\n- Added z_index field to ComputedStyle\n\n## Issue #8 — box-shadow CSS property\n\n- Added BoxShadow struct (offset_x, offset_y, blur, spread, color, inset)\n- CSS parser for box-shadow shorthand (comma-separated multiple shadows)\n- DrawOp::DrawBoxShadow emitted before background in painter\n- PDF rendering via layered semi-transparent rects (blur approximation)\n- Opacity ExtGState collection updated for shadow alpha values\n\n## Issue #9 — opacity (already implemented, no changes needed)\n\n## Issue #10 — text-align: center for inline-block children\n\n- When block container with all-inline children converts to flex-row,\n  text-align now maps to justify-content (center→Center, right→FlexEnd)\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-03-29T10:05:53+01:00",
          "tree_id": "8035c109082f478555ded63a780171ca10c8598e",
          "url": "https://github.com/MoncefMak/ferropdf/commit/c7cb963a5a4921459c51e05df0308952ff89613e"
        },
        "date": 1774775578178,
        "tool": "cargo",
        "benches": [
          {
            "name": "render_simple",
            "value": 6660875,
            "range": "± 48507",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice",
            "value": 7513897,
            "range": "± 39076",
            "unit": "ns/iter"
          },
          {
            "name": "render_simple_cached",
            "value": 3251097,
            "range": "± 22110",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice_cached",
            "value": 4089328,
            "range": "± 13468",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "committer": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "distinct": true,
          "id": "a557a678496201bfe06fa01b1435eb017c52e610",
          "message": "chore: bump version to 0.2.7",
          "timestamp": "2026-03-29T10:18:31+01:00",
          "tree_id": "2a3bf0e6b9439030b309f6783ae2d19d28780554",
          "url": "https://github.com/MoncefMak/ferropdf/commit/a557a678496201bfe06fa01b1435eb017c52e610"
        },
        "date": 1774776196094,
        "tool": "cargo",
        "benches": [
          {
            "name": "render_simple",
            "value": 6827116,
            "range": "± 375213",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice",
            "value": 7684207,
            "range": "± 65614",
            "unit": "ns/iter"
          },
          {
            "name": "render_simple_cached",
            "value": 3260729,
            "range": "± 20796",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice_cached",
            "value": 4114719,
            "range": "± 31230",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "committer": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "distinct": true,
          "id": "09f3cf11fd8fc5fc7995a331d4ed1c23d44b21a5",
          "message": "perf: batch glyph rendering instead of per-glyph begin_text/end_text\n\nSort shaped glyphs by x position (visual order) and emit them in a\nsingle show() call per font run. Eliminates the per-glyph\nbegin_text/set_font/next_line/show/end_text overhead that caused\n4-13x performance regression on benchmarks.",
          "timestamp": "2026-03-29T10:36:06+01:00",
          "tree_id": "21346154d03bfc22abed306ec50c78f476e848af",
          "url": "https://github.com/MoncefMak/ferropdf/commit/09f3cf11fd8fc5fc7995a331d4ed1c23d44b21a5"
        },
        "date": 1774777190612,
        "tool": "cargo",
        "benches": [
          {
            "name": "render_simple",
            "value": 6693868,
            "range": "± 111827",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice",
            "value": 7559883,
            "range": "± 81677",
            "unit": "ns/iter"
          },
          {
            "name": "render_simple_cached",
            "value": 3286903,
            "range": "± 35325",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice_cached",
            "value": 4113170,
            "range": "± 49035",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "committer": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "distinct": true,
          "id": "44a9bd5800b7f2cca38a8d4d2bbb0fb5f98d6806",
          "message": "chore: bump version to 0.2.8",
          "timestamp": "2026-03-29T12:25:50+01:00",
          "tree_id": "f9e2d34bd31e8cd4dcf3d11feab042d10ecfc7f4",
          "url": "https://github.com/MoncefMak/ferropdf/commit/44a9bd5800b7f2cca38a8d4d2bbb0fb5f98d6806"
        },
        "date": 1774783773430,
        "tool": "cargo",
        "benches": [
          {
            "name": "render_simple",
            "value": 5542987,
            "range": "± 78516",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice",
            "value": 6402602,
            "range": "± 76234",
            "unit": "ns/iter"
          },
          {
            "name": "render_simple_cached",
            "value": 3053056,
            "range": "± 69326",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice_cached",
            "value": 3852636,
            "range": "± 29335",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "committer": {
            "email": "moncefmak@users.noreply.github.com",
            "name": "Makti Moncef",
            "username": "MoncefMak"
          },
          "distinct": true,
          "id": "b389d79a15cad79946d2b2ae660757f318c38807",
          "message": "fix: support page_size option — A6 and custom dimensions (closes #11)\n\nPageSize::from_str() only matched A3/A5/Letter/Legal and defaulted\neverything else to A4. Now parses custom \"WW UU HH UU\" strings\n(mm, cm, in, pt, px) and adds A6 support.",
          "timestamp": "2026-04-01T23:41:47+01:00",
          "tree_id": "b0b158c9fd223c35c586aa9a30a87dd50b17ff7a",
          "url": "https://github.com/MoncefMak/ferropdf/commit/b389d79a15cad79946d2b2ae660757f318c38807"
        },
        "date": 1775202607364,
        "tool": "cargo",
        "benches": [
          {
            "name": "render_simple",
            "value": 6662461,
            "range": "± 36816",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice",
            "value": 7510628,
            "range": "± 59478",
            "unit": "ns/iter"
          },
          {
            "name": "render_simple_cached",
            "value": 3261154,
            "range": "± 16852",
            "unit": "ns/iter"
          },
          {
            "name": "render_invoice_cached",
            "value": 4099810,
            "range": "± 25486",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}