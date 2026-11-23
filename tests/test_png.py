"""PNG export: Engine.render_png (issue #44)."""

from __future__ import annotations

import struct
import warnings

import fitz  # pymupdf
import pytest

import ferropdf

TWO_PAGES = """<style>@page { size: A4; margin: 0 }
  body { margin: 0 } .box { width: 200px; height: 100px; background: #ff0000 }</style>
<div class="box"></div>
<div style="break-before: page; width: 200px; height: 100px; background: #0000ff"></div>"""


def size(png: bytes) -> tuple[int, int]:
    assert png[:8] == b"\x89PNG\r\n\x1a\n"
    return struct.unpack(">II", png[16:24])


def pixel(png: bytes, x: int, y: int) -> tuple:
    return fitz.Pixmap(png).pixel(x, y)


def test_one_png_per_page_at_96_dpi():
    pngs = ferropdf.Engine().render_png(TWO_PAGES)
    assert isinstance(pngs, list) and len(pngs) == 2
    assert all(isinstance(p, bytes) for p in pngs)
    # A4 at 96 dpi: one pixel per CSS pixel, partial pixels kept.
    assert size(pngs[0]) == (794, 1123)
    assert pixel(pngs[0], 100, 50)[:3] == (255, 0, 0)
    assert pixel(pngs[1], 100, 50)[:3] == (0, 0, 255)
    assert pixel(pngs[0], 500, 500)[:3] == (255, 255, 255)


def test_dpi_scales_the_image():
    (png,) = ferropdf.Engine().render_png(TWO_PAGES, dpi=48, pages=0)
    assert size(png) == (397, 562)
    (png,) = ferropdf.Engine().render_png(TWO_PAGES, dpi=192, pages=[0])
    assert size(png) == (1588, 2246)


def test_page_selection():
    engine = ferropdf.Engine()
    last, first = engine.render_png(TWO_PAGES, pages=[-1, 0])
    assert pixel(last, 100, 50)[:3] == (0, 0, 255)
    assert pixel(first, 100, 50)[:3] == (255, 0, 0)
    assert len(engine.render_png(TWO_PAGES, pages=(1,))) == 1


def test_transparent_background():
    (png,) = ferropdf.Engine().render_png(TWO_PAGES, pages=0, transparent=True)
    pix = fitz.Pixmap(png)
    assert pix.alpha
    assert pix.pixel(500, 500)[3] == 0
    assert pix.pixel(100, 50) == (255, 0, 0, 255)


def test_matches_the_pdf():
    html = """<h1 style="color:#1a237e">Facture</h1>
    <div style="height:60px;background:linear-gradient(90deg,#e91e63,#2196f3);border-radius:12px"></div>
    <p>Texte <b>gras</b> et <i>italique</i>, àéîõü €.</p>
    <table border="1"><tr><th>A</th><th>B</th></tr><tr><td>1</td><td>2</td></tr></table>"""
    engine = ferropdf.Engine()
    (png,) = engine.render_png(html, dpi=72)
    ours = fitz.Pixmap(png)
    ref = fitz.open(stream=engine.render(html), filetype="pdf")[0].get_pixmap(dpi=72)
    assert (ours.width, ours.height) == (ref.width, ref.height)
    a, b = ours.samples, ref.samples
    step_a, step_b = ours.n, ref.n
    diff = sum(
        abs(a[i * step_a + c] - b[i * step_b + c])
        for i in range(0, ours.width * ours.height, 7)
        for c in range(3)
    ) / (3 * len(range(0, ours.width * ours.height, 7)))
    assert diff < 2.0, f"mean channel difference {diff:.2f} vs MuPDF"


@pytest.mark.parametrize("dpi", [0, -10, 5000, float("nan")])
def test_bad_dpi(dpi):
    with pytest.raises(ValueError, match="dpi"):
        ferropdf.Engine().render_png("<p>x</p>", dpi=dpi)


def test_too_large_raises_value_error():
    with pytest.raises(ValueError, match="lower the dpi"):
        ferropdf.Engine(ferropdf.Options(page_size="1000mm 1000mm")).render_png("<p>x</p>", dpi=600)


@pytest.mark.parametrize("pages", [2, [0, 5], -3])
def test_page_out_of_range(pages):
    with pytest.raises(IndexError, match="out of range"):
        ferropdf.Engine().render_png(TWO_PAGES, pages=pages)


def test_pages_type_error():
    with pytest.raises(TypeError):
        ferropdf.Engine().render_png(TWO_PAGES, pages="first")


def test_warnings_are_emitted():
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        pngs = ferropdf.Engine().render_png('<img src="/nonexistent.png"><p>x</p>')
    assert len(pngs) == 1
    assert any("image" in str(w.message).lower() for w in caught)


def test_works_with_pdfa_options():
    opts = ferropdf.Options(pdf_variant="pdf/a-3b", attachments=[ferropdf.Attachment("a.txt", "x")])
    assert len(ferropdf.Engine(opts).render_png("<p>x</p>")) == 1


def test_svg_keeps_its_aspect_ratio():
    # A square viewBox in a 2:1 box is scaled uniformly and centred
    # (preserveAspectRatio xMidYMid meet), as in browsers.
    html = """<style>@page { size: A4; margin: 0 } body { margin: 0 }</style>
    <svg width="10" height="10" viewBox="0 0 10 10" style="width: 200px; height: 100px">
      <rect width="10" height="10" fill="#ff0000"/></svg>"""
    (png,) = ferropdf.Engine().render_png(html, dpi=96, pages=0)
    assert pixel(png, 100, 50)[:3] == (255, 0, 0)  # centre
    assert pixel(png, 20, 50)[:3] == (255, 255, 255)  # left letterbox
    assert pixel(png, 180, 50)[:3] == (255, 255, 255)  # right letterbox
    assert pixel(png, 55, 50)[:3] == (255, 0, 0) and pixel(png, 145, 50)[:3] == (255, 0, 0)
