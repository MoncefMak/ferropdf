# Benchmark results

Generated with `python bench/engines.py --markdown bench/RESULTS.md` on the [template corpus](../tests/corpus/).
Re-run on your own hardware — numbers depend heavily on CPU, fonts and versions.

_2026-10-28 · Linux x86_64, x86_64 (2 CPUs) · Python 3.13.18 · ferropdf 0.2.9, weasyprint 70.0, plutoprint 0.23.0, chromium 1.63.0 · 20 warm iterations_

**Median warm render time (ms)** — lower is better; (×) = relative to ferropdf

| Template | ferropdf | weasyprint | plutoprint | chromium |
| --- | ---: | ---: | ---: | ---: |
| certificate | 6.6 | 305.7 (46×) | 2.0 (0.3×) | 114.6 (17×) |
| dashboard_grid | 4.6 | 243.4 (53×) | 2.4 (0.5×) | 94.9 (21×) |
| email_style_in_body | 7.3 | 221.2 (30×) | 1.8 (0.2×) | 86.9 (12×) |
| invoice_arabic_rtl | 5.2 | 264.7 (51×) | 2.1 (0.4×) | 90.9 (17×) |
| invoice_bootstrap | 6.3 | 271.7 (43×) | 2.2 (0.4×) | 88.0 (14×) |
| invoice_legacy_float | 5.3 | 237.1 (45×) | 2.2 (0.4×) | 91.5 (17×) |
| invoice_utility | 7.1 | 255.8 (36×) | 1.9 (0.3×) | 100.4 (14×) |
| letter | 4.7 | 126.0 (27×) | 2.5 (0.5×) | 85.1 (18×) |
| receipt_narrow | 2.9 | 108.0 (37×) | 1.3 (0.5×) | 80.9 (28×) |
| report_long | 102.0 | 1580.3 (15×) | 31.0 (0.3×) | 164.8 (1.6×) |

**Per engine**

| | ferropdf | weasyprint | plutoprint | chromium |
| --- | ---: | ---: | ---: | ---: |
| Cold first render, median over templates (ms) | 20 | 937 | 31 | 657 |
| Peak memory, max over templates (MB) | 50 | 141 | 50 | 590 |
| Invoice, 1 thread (docs/s) | 159 | 4 | 446 | — |
| Invoice, 4 threads (docs/s) | 141 | 3 | 521 | — |
| Install footprint (MB) | 6 | 50 (plus system Pango/HarfBuzz/fontconfig) | 145 | 1069 (includes the headless Chromium build) |

Speed alone is not the whole story: check the corpus report (`python tests/corpus/report.py`) — engines do not render these templates identically.
