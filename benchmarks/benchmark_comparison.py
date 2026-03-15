"""
FerroPDF vs WeasyPrint vs wkhtmltopdf — Comparative Benchmark

Usage:
    pip install weasyprint ferropdf
    python benchmark_comparison.py

Optional (wkhtmltopdf):
    apt install wkhtmltopdf

Outputs:
    - Console table with timings
    - benchmark_results.json
    - benchmark_results.md  (paste-ready for README)
"""

import time, json, platform, statistics, subprocess, sys, tempfile, os
from dataclasses import dataclass, field, asdict
from typing import Optional

# ── HTML fixtures ─────────────────────────────────────────────────────────────

SIMPLE_HTML = """<!DOCTYPE html><html><head><meta charset="utf-8"></head>
<body><h1>Hello World</h1><p>Simple paragraph for benchmarking.</p></body></html>"""

STYLED_HTML = """<!DOCTYPE html><html><head><meta charset="utf-8">
<style>
  body{font-family:Arial,sans-serif;margin:20px;color:#333}
  h1{color:#1a56db;border-bottom:2px solid #1a56db;padding-bottom:8px}
  .badge{display:inline-block;padding:2px 8px;background:#dbeafe;
         color:#1e40af;border-radius:4px;font-size:12px}
</style></head>
<body>
  <h1>Styled Report <span class="badge">v1.0</span></h1>
  <p>Tests CSS styling, headings, and list rendering.</p>
  <ul><li>Item one</li><li>Item two</li><li>Item three</li></ul>
</body></html>"""


def make_complex_html(rows=50):
    tr = "".join(
        "<tr><td>#{:04d}</td><td>Product {}</td><td>Cat {}</td>"
        "<td style='text-align:right'>{:.2f} EUR</td>"
        "<td style='color:{}'>{}</td></tr>".format(
            i, i, i % 5 + 1, i * 9.99,
            "green" if i % 2 == 0 else "red",
            "Active" if i % 2 == 0 else "Inactive",
        )
        for i in range(1, rows + 1)
    )
    return """<!DOCTYPE html><html><head><meta charset="utf-8">
<style>
  body{{font-family:Arial,sans-serif;margin:24px;color:#1f2937}}
  h1{{font-size:24px}} h2{{font-size:16px;margin-top:24px}}
  table{{width:100%;border-collapse:collapse;font-size:13px}}
  th{{background:#1a56db;color:white;padding:8px 12px}}
  td{{padding:6px 12px;border-bottom:1px solid #e5e7eb}}
  tr:nth-child(even) td{{background:#f9fafb}}
  .m{{display:inline-block;width:22%;padding:12px;background:#eff6ff;
      border-radius:8px;margin-right:2%;text-align:center}}
  .m strong{{display:block;font-size:22px;color:#1a56db}}
</style></head>
<body>
  <h1>Sales Report Q4 2024</h1>
  <div>
    <div class="m"><strong>1284</strong>Orders</div>
    <div class="m"><strong>48320 EUR</strong>Revenue</div>
    <div class="m"><strong>37.6 EUR</strong>Avg Order</div>
    <div class="m"><strong>92%</strong>Satisfaction</div>
  </div>
  <h2>Products ({rows} rows)</h2>
  <table>
    <thead><tr><th>ID</th><th>Product</th><th>Category</th>
    <th>Price</th><th>Status</th></tr></thead>
    <tbody>{tr}</tbody>
  </table>
</body></html>""".format(rows=rows, tr=tr)


# ── Result dataclass ──────────────────────────────────────────────────────────

@dataclass
class Result:
    library:  str
    document: str
    runs:     int
    times_ms: list = field(default_factory=list)
    error:    Optional[str] = None

    @property
    def mean_ms(self):
        return statistics.mean(self.times_ms) if self.times_ms else None

    @property
    def stdev_ms(self):
        return statistics.stdev(self.times_ms) if len(self.times_ms) > 1 else 0.0

    @property
    def min_ms(self):
        return min(self.times_ms) if self.times_ms else None

    @property
    def max_ms(self):
        return max(self.times_ms) if self.times_ms else None

    @property
    def p95_ms(self):
        if not self.times_ms:
            return None
        s = sorted(self.times_ms)
        return s[min(int(len(s) * 0.95), len(s) - 1)]


def _run(fn, runs):
    try:
        fn()  # warm-up
    except Exception as e:
        return [], str(e)
    times = []
    for _ in range(runs):
        t0 = time.perf_counter()
        try:
            fn()
        except Exception as e:
            return times, str(e)
        times.append((time.perf_counter() - t0) * 1000)
    return times, None


# ── Library adapters ──────────────────────────────────────────────────────────

def bench_ferropdf(html, runs, label):
    r = Result("FerroPDF", label, runs)
    try:
        from fastpdf import render_pdf
    except ImportError:
        r.error = "not installed — pip install ferropdf"
        return r
    r.times_ms, r.error = _run(lambda: render_pdf(html), runs)
    return r


def bench_weasyprint(html, runs, label):
    r = Result("WeasyPrint", label, runs)
    try:
        from weasyprint import HTML
    except ImportError:
        r.error = "not installed — pip install weasyprint"
        return r
    r.times_ms, r.error = _run(lambda: HTML(string=html).write_pdf(), runs)
    return r


def bench_wkhtmltopdf(html, runs, label):
    r = Result("wkhtmltopdf", label, runs)
    if subprocess.call(
        ["which", "wkhtmltopdf"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    ) != 0:
        r.error = "not installed — apt/brew install wkhtmltopdf"
        return r

    def _render():
        with tempfile.NamedTemporaryFile(
            suffix=".html", delete=False, mode="w", encoding="utf-8"
        ) as f:
            f.write(html)
            tmp_in = f.name
        tmp_out = tmp_in.replace(".html", ".pdf")
        try:
            subprocess.run(
                ["wkhtmltopdf", "--quiet", tmp_in, tmp_out],
                check=True, capture_output=True,
            )
        finally:
            os.unlink(tmp_in)
            if os.path.exists(tmp_out):
                os.unlink(tmp_out)

    r.times_ms, r.error = _run(_render, runs)
    return r


# ── Reporting ─────────────────────────────────────────────────────────────────

C = {
    "G": "\033[92m", "B": "\033[94m", "Y": "\033[93m", "R": "\033[91m",
    "BOLD": "\033[1m", "DIM": "\033[2m", "RST": "\033[0m",
}
LIB_COLOR = {"FerroPDF": C["G"], "WeasyPrint": C["B"], "wkhtmltopdf": C["Y"]}


def fmt(ms):
    if ms is None:  return "N/A"
    if ms < 1:      return "{:.0f} µs".format(ms * 1000)
    if ms < 1000:   return "{:.1f} ms".format(ms)
    return "{:.2f} s".format(ms / 1000)


def speedup_str(wp, lib):
    if not wp or not lib or lib == 0:
        return "—"
    ratio = wp / lib
    if ratio >= 1:
        return "{}{:.1f}x faster{}".format(C["G"], ratio, C["RST"])
    return "{}{:.1f}x slower{}".format(C["R"], 1 / ratio, C["RST"])


def print_table(results):
    libs = ["FerroPDF", "WeasyPrint", "wkhtmltopdf"]
    docs = sorted({r.document for r in results})
    idx  = {(r.library, r.document): r for r in results}
    W = 84
    print("\n{}{}{}".format(C["BOLD"], "─" * W, C["RST"]))
    print("{} {:<24} {:<14} {:>9} {:>9} {:>9} {:>16}{}".format(
        C["BOLD"], "Document", "Library", "Mean", "+/-Stdev", "P95", "vs WeasyPrint", C["RST"]))
    print("{}{}{}".format(C["BOLD"], "─" * W, C["RST"]))

    for doc in docs:
        wp = idx.get(("WeasyPrint", doc))
        wp_mean = wp.mean_ms if (wp and not wp.error) else None
        for lib in libs:
            r = idx.get((lib, doc))
            if r is None:
                continue
            col = LIB_COLOR.get(lib, "")
            if r.error:
                print(" {:<24} {}{:<14}{} ERROR: {}".format(doc, col, lib, C["RST"], r.error))
                continue
            sp = speedup_str(wp_mean, r.mean_ms) if lib != "WeasyPrint" else "             —"
            print(" {:<24} {}{:<14}{} {:>9} {:>9} {:>9}  {}".format(
                doc, col, lib, C["RST"],
                fmt(r.mean_ms), "+/-" + fmt(r.stdev_ms), fmt(r.p95_ms), sp))
        print("{}{}{}".format(C["DIM"], "─" * W, C["RST"]))


def save_markdown(results, path="benchmark_results.md"):
    libs = ["FerroPDF", "WeasyPrint", "wkhtmltopdf"]
    docs = sorted({r.document for r in results})
    idx  = {(r.library, r.document): r for r in results}
    lines = [
        "## Performance Benchmarks",
        "",
        "> Machine: `{}` — {} {}  ".format(
            platform.processor(), platform.system(), platform.release()),
        "> Python `{}` — {}".format(
            sys.version.split()[0], time.strftime("%Y-%m-%d")),
        "",
        "### Full pipeline: HTML to PDF",
        "",
        "| Document | FerroPDF | WeasyPrint | wkhtmltopdf | Speedup vs WeasyPrint |",
        "|---|---|---|---|---|",
    ]
    for doc in docs:
        cells = ["**{}**".format(doc)]
        wp = idx.get(("WeasyPrint", doc))
        wp_mean = wp.mean_ms if (wp and not wp.error) else None
        for lib in libs:
            r = idx.get((lib, doc))
            cells.append("N/A" if (r is None or r.error)
                         else "{} +/-{}".format(fmt(r.mean_ms), fmt(r.stdev_ms)))
        ferro = idx.get(("FerroPDF", doc))
        fm = ferro.mean_ms if (ferro and not ferro.error) else None
        if fm and wp_mean:
            ratio = wp_mean / fm
            cells.append("**{:.1f}x faster**".format(ratio) if ratio >= 1
                         else "{:.1f}x slower".format(1 / ratio))
        else:
            cells.append("—")
        lines.append("| " + " | ".join(cells) + " |")
    lines += [
        "",
        "> 1 warm-up run + N timed iterations. Mean +/- stdev shown.",
        "> Reproduce: `python benchmarks/benchmark_comparison.py`",
    ]
    with open(path, "w", encoding="utf-8") as f:
        f.write("\n".join(lines))
    print("  Markdown  ->  {}".format(path))


def save_json(results, path="benchmark_results.json"):
    data = {
        "meta": {
            "date":      time.strftime("%Y-%m-%dT%H:%M:%S"),
            "python":    sys.version,
            "platform":  platform.platform(),
            "processor": platform.processor(),
        },
        "results": [
            {**asdict(r), "mean_ms": r.mean_ms, "stdev_ms": r.stdev_ms,
             "min_ms": r.min_ms, "max_ms": r.max_ms, "p95_ms": r.p95_ms}
            for r in results
        ],
    }
    with open(path, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=2)
    print("  JSON      ->  {}".format(path))


# ── Suite and main ────────────────────────────────────────────────────────────

SUITE = [
    ("Simple HTML",    SIMPLE_HTML,             30),
    ("Styled HTML",    STYLED_HTML,             20),
    ("Table  10 rows", make_complex_html(10),   15),
    ("Table  50 rows", make_complex_html(50),   10),
    ("Table 100 rows", make_complex_html(100),   5),
]


def main():
    import argparse

    parser = argparse.ArgumentParser(
        description="FerroPDF vs WeasyPrint vs wkhtmltopdf — Comparative Benchmark"
    )
    parser.add_argument(
        "--runs", type=int, default=None,
        help="Override the number of runs for every fixture (default: per-fixture defaults)"
    )
    parser.add_argument(
        "--output", default="benchmark_results.md",
        help="Output path for the Markdown report (default: benchmark_results.md)"
    )
    args = parser.parse_args()

    suite = [
        (label, html, args.runs if args.runs is not None else runs)
        for label, html, runs in SUITE
    ]

    print("{b}FerroPDF Comparative Benchmark{r}".format(b=C["BOLD"], r=C["RST"]))
    print("Platform : {}".format(platform.platform()))
    print("Python   : {}".format(sys.version.split()[0]))
    print("Date     : {}\n".format(time.strftime("%Y-%m-%d %H:%M:%S")))

    all_results = []
    for label, html, runs in suite:
        print("  {} ({} runs/lib) ...".format(label, runs), end="", flush=True)
        for fn in [bench_ferropdf, bench_weasyprint, bench_wkhtmltopdf]:
            all_results.append(fn(html, runs, label))
        print(" done")

    print_table(all_results)
    print()
    save_json(all_results)
    save_markdown(all_results, path=args.output)
    print("\nAll done.\n")


if __name__ == "__main__":
    main()
