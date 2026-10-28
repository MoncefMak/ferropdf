"""Reproducible HTML→PDF benchmark: ferropdf vs WeasyPrint vs PlutoPrint vs Chromium.

    python bench/engines.py                       # all installed engines, whole corpus
    python bench/engines.py --engines ferropdf,weasyprint --iterations 10
    python bench/engines.py --json results.json --markdown results.md

For every (engine, template) pair a *fresh* worker process is started so that
peak memory is measured in isolation (engine + Python interpreter, including
child processes such as Chromium). Inside the worker:

* ``cold``   — first render, including engine/font initialisation;
* ``median`` — median of ``--iterations`` warm renders (engine reused);
* ``rss``    — peak resident memory of the worker process tree.

A separate throughput run renders the invoice with 4 threads to show whether
the engine releases the GIL. Install footprint is the on-disk size of the
engine's Python distributions (and, for Chromium, the browser build); system
libraries (Pango for WeasyPrint) are not counted and are listed in the notes.

Optional dependencies: ``psutil`` (memory), ``weasyprint``, ``plutoprint``,
``playwright`` + ``playwright install chromium``.
"""

from __future__ import annotations

import argparse
import json
import platform
import statistics
import subprocess
import sys
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TEMPLATES = ROOT / "tests" / "corpus" / "templates"
ENGINES = ["ferropdf", "weasyprint", "plutoprint", "chromium"]
DISTS = {
    "ferropdf": ["ferropdf"],
    "weasyprint": ["weasyprint"],
    "plutoprint": ["plutoprint"],
    "chromium": ["playwright"],
}
NOTES = {
    "weasyprint": "plus system Pango/HarfBuzz/fontconfig",
    "chromium": "includes the headless Chromium build",
}


# --------------------------------------------------------------------------- renderers


def make_renderer(engine: str):
    """Return (render(html) -> bytes, close())."""
    base = str(TEMPLATES)
    if engine == "ferropdf":
        import ferropdf

        eng = ferropdf.Engine(ferropdf.Options(base_url=base))
        return eng.render, lambda: None
    if engine == "weasyprint":
        from weasyprint import HTML

        return (lambda html: HTML(string=html, base_url=base).write_pdf()), lambda: None
    if engine == "plutoprint":
        import io

        import plutoprint

        def render(html):
            book = plutoprint.Book(plutoprint.PAGE_SIZE_A4)
            book.load_html(html, base_url=base + "/")
            out = io.BytesIO()
            book.write_to_pdf_stream(out)
            return out.getvalue()

        return render, lambda: None
    if engine == "chromium":
        from playwright.sync_api import sync_playwright

        pw = sync_playwright().start()
        browser = pw.chromium.launch()
        lock = threading.Lock()  # sync Playwright is not thread-safe

        def render(html):
            with lock:
                page = browser.new_page()
                try:
                    page.set_content(html, wait_until="load")
                    return page.pdf(prefer_css_page_size=True, print_background=True)
                finally:
                    page.close()

        def close():
            browser.close()
            pw.stop()

        return render, close
    raise ValueError(engine)


# --------------------------------------------------------------------------- worker


class PeakRSS(threading.Thread):
    """Sample RSS of this process and its descendants every 10 ms."""

    def __init__(self):
        super().__init__(daemon=True)
        self.peak = 0
        self._stop = threading.Event()

    def run(self):
        try:
            import psutil
        except ImportError:
            return
        me = psutil.Process()
        while not self._stop.is_set():
            try:
                procs = [me] + me.children(recursive=True)
                self.peak = max(self.peak, sum(p.memory_info().rss for p in procs if p.is_running()))
            except Exception:  # noqa: BLE001 — processes come and go
                pass
            time.sleep(0.01)

    def stop(self):
        self._stop.set()
        self.join()
        return self.peak


def worker(engine: str, template: str, iterations: int, threads: int) -> dict:
    import warnings

    warnings.simplefilter("ignore")
    html = (TEMPLATES / f"{template}.html").read_text(encoding="utf-8")
    sampler = PeakRSS()
    sampler.start()

    t0 = time.perf_counter()
    render, close = make_renderer(engine)
    pdf = render(html)
    cold = time.perf_counter() - t0

    times = []
    for _ in range(iterations):
        t = time.perf_counter()
        render(html)
        times.append(time.perf_counter() - t)

    result = {
        "engine": engine,
        "template": template,
        "cold_ms": cold * 1000,
        "median_ms": statistics.median(times) * 1000,
        "p95_ms": sorted(times)[max(0, int(len(times) * 0.95) - 1)] * 1000,
        "pdf_kb": len(pdf) / 1024,
    }

    if threads > 1:
        n = threads * 5
        t = time.perf_counter()
        with ThreadPoolExecutor(threads) as pool:
            list(pool.map(lambda _: render(html), range(n)))
        result["threaded_docs_per_s"] = n / (time.perf_counter() - t)
        result["serial_docs_per_s"] = 1000 / result["median_ms"]

    close()
    result["peak_rss_mb"] = sampler.stop() / 1024 / 1024
    return result


# --------------------------------------------------------------------------- orchestration


def available(engine: str) -> bool:
    probe = {
        "ferropdf": "import ferropdf",
        "weasyprint": "import weasyprint",
        "plutoprint": "import plutoprint",
        "chromium": "from playwright.sync_api import sync_playwright as s; p=s().start(); p.chromium.launch().close(); p.stop()",
    }[engine]
    return subprocess.run([sys.executable, "-c", probe], capture_output=True).returncode == 0


def footprint_mb(engine: str) -> float | None:
    from importlib import metadata

    seen, stack, total = set(), list(DISTS[engine]), 0
    while stack:
        name = stack.pop().lower().replace("_", "-")
        if name in seen:
            continue
        seen.add(name)
        try:
            dist = metadata.distribution(name)
        except metadata.PackageNotFoundError:
            continue
        for f in dist.files or []:
            try:
                total += Path(dist.locate_file(f)).stat().st_size
            except OSError:
                pass
        for req in dist.requires or []:
            if "extra ==" in req:
                continue
            stack.append(req.split(";")[0].split("[")[0].split("(")[0].split("<")[0].split(">")[0].split("=")[0].split("!")[0].split("~")[0].strip())
    # Editable installs (`maturin develop`) record almost no files: measure the package dir.
    import importlib.util

    spec = importlib.util.find_spec(DISTS[engine][0])
    if spec and spec.submodule_search_locations:
        pkg = sum(p.stat().st_size for loc in spec.submodule_search_locations
                  for p in Path(loc).rglob("*") if p.is_file() and "__pycache__" not in p.parts)
        total = max(total, pkg)
    if engine == "chromium":
        import os

        cache = Path(os.environ.get("PLAYWRIGHT_BROWSERS_PATH") or Path.home() / ".cache" / "ms-playwright")
        total += sum(p.stat().st_size for p in cache.rglob("*")
                     if p.is_file() and "chromium" in str(p.relative_to(cache)))
    return total / 1024 / 1024 if total else None


def version(engine: str) -> str:
    from importlib import metadata

    try:
        return metadata.version(DISTS[engine][0])
    except metadata.PackageNotFoundError:
        return "?"


def to_markdown(results: list[dict], meta: dict) -> str:
    engines = [e for e in ENGINES if any(r["engine"] == e for r in results)]
    templates = sorted({r["template"] for r in results})
    by = {(r["engine"], r["template"]): r for r in results}
    out = [
        f"_{meta['date']} · {meta['machine']} · Python {meta['python']} · "
        + ", ".join(f"{e} {meta['versions'][e]}" for e in engines)
        + f" · {meta['iterations']} warm iterations_",
        "",
        "**Median warm render time (ms)** — lower is better; (×) = relative to ferropdf",
        "",
        "| Template | " + " | ".join(engines) + " |",
        "| --- |" + " ---: |" * len(engines),
    ]
    for t in templates:
        cells = []
        base = by.get(("ferropdf", t))
        for e in engines:
            r = by.get((e, t))
            if not r:
                cells.append("—")
            elif e != "ferropdf" and base:
                ratio = r["median_ms"] / base["median_ms"]
                cells.append(f"{r['median_ms']:.1f} ({ratio:.0f}×)" if ratio >= 10 else f"{r['median_ms']:.1f} ({ratio:.1f}×)")
            else:
                cells.append(f"{r['median_ms']:.1f}")
        out.append(f"| {t} | " + " | ".join(cells) + " |")

    def agg(e, key, fn=statistics.median):
        vals = [r[key] for r in results if r["engine"] == e and key in r]
        return fn(vals) if vals else None

    out += ["", "**Per engine**", "", "| | " + " | ".join(engines) + " |", "| --- |" + " ---: |" * len(engines)]
    rows = [
        ("Cold first render, median over templates (ms)", lambda e: f"{agg(e, 'cold_ms'):.0f}"),
        ("Peak memory, max over templates (MB)", lambda e: f"{agg(e, 'peak_rss_mb', max):.0f}"),
        ("Invoice, 1 thread (docs/s)", lambda e: f"{agg(e, 'serial_docs_per_s'):.0f}" if agg(e, "serial_docs_per_s") else "—"),
        ("Invoice, 4 threads (docs/s)", lambda e: f"{agg(e, 'threaded_docs_per_s'):.0f}" if agg(e, "threaded_docs_per_s") else "—"),
        ("Install footprint (MB)", lambda e: f"{meta['footprint'][e]:.0f}" + (f" ({NOTES[e]})" if e in NOTES else "") if meta["footprint"].get(e) else "—"),
    ]
    for label, fn in rows:
        out.append(f"| {label} | " + " | ".join(fn(e) for e in engines) + " |")
    out += [
        "",
        "Speed alone is not the whole story: check the corpus report "
        "(`python tests/corpus/report.py`) — engines do not render these templates identically.",
    ]
    return "\n".join(out)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--engines", default=",".join(ENGINES))
    ap.add_argument("--templates", nargs="*", help="template stems (default: whole corpus)")
    ap.add_argument("--iterations", type=int, default=20)
    ap.add_argument("--threads", type=int, default=4)
    ap.add_argument("--json", type=Path)
    ap.add_argument("--markdown", type=Path)
    ap.add_argument("--worker", nargs=2, metavar=("ENGINE", "TEMPLATE"), help=argparse.SUPPRESS)
    args = ap.parse_args()

    if args.worker:
        engine, template = args.worker
        # Sync Playwright is bound to one greenlet/thread: no threaded run for Chromium.
        threads = args.threads if template == "invoice_bootstrap" and engine != "chromium" else 1
        print(json.dumps(worker(engine, template, args.iterations, threads)))
        return

    engines = [e for e in args.engines.split(",") if e in ENGINES]
    missing = [e for e in engines if not available(e)]
    engines = [e for e in engines if e not in missing]
    if missing:
        print(f"skipping unavailable engines: {', '.join(missing)}", file=sys.stderr)
    templates = args.templates or sorted(p.stem for p in TEMPLATES.glob("*.html"))

    results = []
    for t in templates:
        for e in engines:
            # Chromium and WeasyPrint are slow on the 15-page report: fewer iterations.
            iters = max(3, args.iterations // 4) if t == "report_long" and e != "ferropdf" else args.iterations
            cmd = [sys.executable, __file__, "--worker", e, t, "--iterations", str(iters), "--threads", str(args.threads)]
            proc = subprocess.run(cmd, capture_output=True, text=True)
            if proc.returncode != 0:
                print(f"{t:22s} {e:11s} FAILED: {proc.stderr.strip().splitlines()[-1:]}", file=sys.stderr)
                continue
            r = json.loads(proc.stdout.strip().splitlines()[-1])
            results.append(r)
            print(f"{t:22s} {e:11s} median {r['median_ms']:8.1f} ms  cold {r['cold_ms']:8.1f} ms  rss {r['peak_rss_mb']:6.0f} MB", file=sys.stderr)

    import datetime

    meta = {
        "date": datetime.date.today().isoformat(),
        "machine": f"{platform.system()} {platform.machine()}, {platform.processor() or '?'} ({_cpus()} CPUs)",
        "python": platform.python_version(),
        "iterations": args.iterations,
        "versions": {e: version(e) for e in engines},
        "footprint": {e: footprint_mb(e) for e in engines},
    }
    md = to_markdown(results, meta)
    print(md)
    if args.json:
        args.json.write_text(json.dumps({"meta": meta, "results": results}, indent=2))
    if args.markdown:
        args.markdown.write_text(md + "\n")


def _cpus():
    import os

    return os.cpu_count()


if __name__ == "__main__":
    main()
