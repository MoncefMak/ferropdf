window.BENCHMARK_DATA = {
  "lastUpdate": 1773599042043,
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
      }
    ]
  }
}