"""Render the template corpus with several engines and build a side-by-side HTML report.

    python tests/corpus/report.py OUT_DIR [--engines ferropdf,chromium,weasyprint,plutoprint]
                                          [--pages 3] [--dpi 50] [--only NAME ...]

Engines that are not installed are skipped. Requirements: ``pymupdf`` for
rasterising; optionally ``weasyprint``, ``plutoprint`` and ``playwright`` (+ ``playwright install
chromium``) for the reference renders.

The report shows the first pages of every template for every engine, the page
count, the render time and any warnings ferropdf emitted. It is the fastest way
to see *what breaks* on realistic documents.
"""

from __future__ import annotations

import argparse
import html as html_mod
import time
import warnings
from pathlib import Path

import pymupdf

HERE = Path(__file__).resolve().parent
TEMPLATES = HERE / "templates"


# --------------------------------------------------------------------------- engines


def render_ferropdf(html: str, base_url: Path):
    import ferropdf

    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        pdf = ferropdf.from_html(html, base_url=str(base_url))
    return pdf, [str(w.message) for w in caught]


def render_weasyprint(html: str, base_url: Path):
    from weasyprint import HTML

    return HTML(string=html, base_url=str(base_url)).write_pdf(), []


def render_plutoprint(html: str, base_url: Path):
    import io

    import plutoprint

    book = plutoprint.Book(plutoprint.PAGE_SIZE_A4)
    book.load_html(html, base_url=str(base_url) + "/")
    out = io.BytesIO()
    book.write_to_pdf_stream(out)
    return out.getvalue(), []


class _Chromium:
    def __init__(self):
        from playwright.sync_api import sync_playwright

        self._pw = sync_playwright().start()
        self._browser = self._pw.chromium.launch()

    def __call__(self, html: str, base_url: Path):
        page = self._browser.new_page()
        try:
            page.set_content(html, wait_until="load")
            pdf = page.pdf(prefer_css_page_size=True, print_background=True)
        finally:
            page.close()
        return pdf, []

    def close(self):
        self._browser.close()
        self._pw.stop()


def load_engines(names):
    engines = {}
    for name in names:
        try:
            if name == "ferropdf":
                import ferropdf  # noqa: F401

                engines[name] = render_ferropdf
            elif name == "weasyprint":
                import weasyprint  # noqa: F401

                engines[name] = render_weasyprint
            elif name == "plutoprint":
                import plutoprint  # noqa: F401

                engines[name] = render_plutoprint
            elif name == "chromium":
                engines[name] = _Chromium()
            else:
                print(f"unknown engine {name!r}, skipping")
        except Exception as exc:  # noqa: BLE001 — optional engines
            print(f"engine {name} unavailable ({type(exc).__name__}: {exc}), skipping")
    return engines


# --------------------------------------------------------------------------- report


def rasterise(pdf: bytes, out_stem: Path, pages: int, dpi: int) -> tuple[int, list[str]]:
    doc = pymupdf.open(stream=pdf, filetype="pdf")
    files = []
    for i in range(min(pages, doc.page_count)):
        path = out_stem.with_name(f"{out_stem.name}-p{i + 1}.png")
        doc[i].get_pixmap(dpi=dpi).save(path)
        files.append(path.name)
    return doc.page_count, files


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("out", type=Path)
    ap.add_argument("--engines", default="ferropdf,chromium,weasyprint,plutoprint")
    ap.add_argument("--pages", type=int, default=3)
    ap.add_argument("--dpi", type=int, default=50)
    ap.add_argument("--only", nargs="*", help="template stems to include")
    args = ap.parse_args()

    out: Path = args.out
    (out / "img").mkdir(parents=True, exist_ok=True)
    (out / "pdf").mkdir(parents=True, exist_ok=True)
    engines = load_engines(args.engines.split(","))

    templates = sorted(TEMPLATES.glob("*.html"))
    if args.only:
        templates = [t for t in templates if t.stem in args.only]

    rows = []
    for tpl in templates:
        source = tpl.read_text(encoding="utf-8")
        cells = []
        for name, render in engines.items():
            start = time.perf_counter()
            try:
                pdf, notes = render(source, TEMPLATES)
                ms = (time.perf_counter() - start) * 1000
                (out / "pdf" / f"{tpl.stem}.{name}.pdf").write_bytes(pdf)
                count, images = rasterise(pdf, out / "img" / f"{tpl.stem}.{name}", args.pages, args.dpi)
                cells.append((name, count, ms, images, notes, None))
            except Exception as exc:  # noqa: BLE001 — report, don't crash
                cells.append((name, 0, 0.0, [], [], f"{type(exc).__name__}: {exc}"))
            print(f"{tpl.stem:24s} {name:10s} {cells[-1][1]:3d} pages {cells[-1][2]:8.1f} ms")
        rows.append((tpl.stem, cells))

    for render in engines.values():
        if hasattr(render, "close"):
            render.close()

    esc = html_mod.escape
    parts = [
        "<!DOCTYPE html><html><head><meta charset='utf-8'><title>ferropdf corpus report</title><style>",
        "body{font-family:system-ui,sans-serif;margin:24px;background:#f6f7f9;color:#1f2328}",
        "h2{margin:32px 0 8px}.row{display:flex;gap:16px;align-items:flex-start}",
        ".cell{background:#fff;border:1px solid #d0d7de;border-radius:8px;padding:10px;flex:1;min-width:0}",
        ".cell h3{margin:0 0 6px;font-size:14px}.meta{font-size:12px;color:#57606a;margin-bottom:6px}",
        ".pages{display:flex;gap:6px;flex-wrap:wrap}.pages img{border:1px solid #d0d7de;max-width:100%}",
        ".err{color:#cf222e;font-size:12px}.warn{color:#9a6700;font-size:11px;word-break:break-all}",
        "</style></head><body><h1>ferropdf corpus report</h1>",
        f"<p>{len(rows)} templates &middot; engines: {', '.join(engines)} &middot; first {args.pages} pages at {args.dpi} dpi</p>",
    ]
    for stem, cells in rows:
        parts.append(f"<h2 id='{stem}'>{stem}</h2><div class='row'>")
        for name, count, ms, images, notes, err in cells:
            parts.append(f"<div class='cell'><h3>{name}</h3>")
            if err:
                parts.append(f"<div class='err'>{esc(err)}</div>")
            else:
                parts.append(f"<div class='meta'>{count} page(s) &middot; {ms:.0f} ms</div><div class='pages'>")
                parts += [f"<img src='img/{esc(i)}' loading='lazy'>" for i in images]
                parts.append("</div>")
                parts += [f"<div class='warn'>⚠ {esc(n[:200])}</div>" for n in notes]
            parts.append("</div>")
        parts.append("</div>")
    parts.append("</body></html>")
    (out / "index.html").write_text("".join(parts), encoding="utf-8")
    print(f"\nreport: {out / 'index.html'}")


if __name__ == "__main__":
    main()
