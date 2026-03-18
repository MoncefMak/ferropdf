"""
Benchmark: ferropdf vs WeasyPrint

Renders the same HTML fixtures with both engines and compares wall-clock time.
Run:
    python bench/compare.py
"""
import statistics
import time
from pathlib import Path

import ferropdf

try:
    from weasyprint import HTML as WeasyHTML
except ImportError:
    WeasyHTML = None
    print("WeasyPrint not installed — skipping comparison.\n"
          "Install with: pip install weasyprint")

FIXTURES_DIR = Path(__file__).resolve().parent.parent / "tests" / "fixtures"

FIXTURES = {
    "simple":  FIXTURES_DIR / "simple.html",
    "invoice": FIXTURES_DIR / "invoice.html",
}

WARMUP = 2
ITERATIONS = 20


def bench_ferropdf(html: str, iterations: int) -> list[float]:
    engine = ferropdf.Engine(ferropdf.Options(margin="15mm"))
    # warmup
    for _ in range(WARMUP):
        engine.render(html)
    times = []
    for _ in range(iterations):
        start = time.perf_counter()
        engine.render(html)
        elapsed = time.perf_counter() - start
        times.append(elapsed)
    return times


def bench_weasyprint(html: str, iterations: int) -> list[float]:
    if WeasyHTML is None:
        return []
    # warmup
    for _ in range(WARMUP):
        WeasyHTML(string=html).write_pdf()
    times = []
    for _ in range(iterations):
        start = time.perf_counter()
        WeasyHTML(string=html).write_pdf()
        elapsed = time.perf_counter() - start
        times.append(elapsed)
    return times


def fmt_ms(seconds: float) -> str:
    return f"{seconds * 1000:.1f}ms"


def main():
    print("=" * 60)
    print("  ferropdf vs WeasyPrint — Benchmark")
    print("=" * 60)
    print(f"  Warmup: {WARMUP} | Iterations: {ITERATIONS}")
    print()

    for name, path in FIXTURES.items():
        html = path.read_text(encoding="utf-8")
        print(f"── {name} ({path.name}) ──")

        ferro_times = bench_ferropdf(html, ITERATIONS)
        ferro_med = statistics.median(ferro_times)
        ferro_std = statistics.stdev(ferro_times) if len(ferro_times) > 1 else 0

        print(f"  ferropdf:    {fmt_ms(ferro_med)} median  "
              f"(± {fmt_ms(ferro_std)})")

        weasy_times = bench_weasyprint(html, ITERATIONS)
        if weasy_times:
            weasy_med = statistics.median(weasy_times)
            weasy_std = statistics.stdev(weasy_times) if len(weasy_times) > 1 else 0
            speedup = weasy_med / ferro_med if ferro_med > 0 else float("inf")

            print(f"  weasyprint:  {fmt_ms(weasy_med)} median  "
                  f"(± {fmt_ms(weasy_std)})")
            print(f"  speedup:     {speedup:.1f}x faster")
        else:
            print("  weasyprint:  (not installed)")

        print()

    print("=" * 60)


if __name__ == "__main__":
    main()
