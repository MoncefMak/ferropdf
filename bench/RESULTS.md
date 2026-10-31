# Benchmark results

Generated with `python bench/engines.py --markdown bench/RESULTS.md` on the [template corpus](../tests/corpus/).
Re-run on your own hardware — numbers depend heavily on CPU, fonts and versions.

_2026-10-31 · Linux x86_64, x86_64 (2 CPUs) · Python 3.13.18 · ferropdf 0.2.9, weasyprint 70.0, plutoprint 0.23.0, chromium 1.63.0 · 20 warm iterations_

**Median warm render time (ms)** — lower is better; (×) = relative to ferropdf

| Template | ferropdf | weasyprint | plutoprint | chromium |
| --- | ---: | ---: | ---: | ---: |
| certificate | 4.0 | 525.6 (131×) | 5.3 (1.3×) | 166.5 (41×) |
| dashboard_grid | 4.0 | 449.2 (113×) | 4.3 (1.1×) | 147.5 (37×) |
| email_style_in_body | 4.5 | 424.5 (94×) | 2.6 (0.6×) | 145.2 (32×) |
| invoice_arabic_rtl | 7.4 | 470.7 (64×) | 6.0 (0.8×) | 163.6 (22×) |
| invoice_bootstrap | 7.5 | 454.4 (60×) | 6.1 (0.8×) | 161.4 (21×) |
| invoice_legacy_float | 7.6 | 411.3 (54×) | 3.1 (0.4×) | 135.6 (18×) |
| invoice_utility | 3.1 | 387.7 (123×) | 2.8 (0.9×) | 159.9 (51×) |
| letter | 2.8 | 205.5 (73×) | 4.0 (1.4×) | 138.9 (49×) |
| receipt_narrow | 2.1 | 174.8 (83×) | 2.2 (1.0×) | 134.0 (64×) |
| report_long | 113.9 | 2915.6 (26×) | 52.0 (0.5×) | 266.2 (2.3×) |

**Per engine**

| | ferropdf | weasyprint | plutoprint | chromium |
| --- | ---: | ---: | ---: | ---: |
| Cold first render, median over templates (ms) | 35 | 1561 | 55 | 1127 |
| Peak memory, max over templates (MB) | 69 | 141 | 44 | 581 |
| Invoice, 1 thread (docs/s) | 133 | 2 | 163 | — |
| Invoice, 4 threads (docs/s) | 219 | 2 | 283 | — |
| Install footprint (MB) | 9 | 50 (plus system Pango/HarfBuzz/fontconfig) | 145 | 1069 (includes the headless Chromium build) |

Speed alone is not the whole story: check the corpus report (`python tests/corpus/report.py`) — engines do not render these templates identically.
