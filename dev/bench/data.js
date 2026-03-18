window.BENCHMARK_DATA = {
  "lastUpdate": 1773858047869,
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
      }
    ]
  }
}