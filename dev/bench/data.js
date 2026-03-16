window.BENCHMARK_DATA = {
  "lastUpdate": 1773621970783,
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
      }
    ]
  }
}