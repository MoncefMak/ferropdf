"""Concurrency: renders on several threads must not interfere (#53, #54)."""

from __future__ import annotations

import io
from concurrent.futures import ThreadPoolExecutor

import pypdf

import ferropdf

HTML = """<html><body>
<h1 style="text-align:center">Invoice {n}</h1>
<table style="width:100%"><tr><td>Item</td><td style="text-align:right">{n}.00 EUR</td></tr></table>
</body></html>"""


def _text(pdf: bytes) -> str:
    return " ".join(p.extract_text() or "" for p in pypdf.PdfReader(io.BytesIO(pdf)).pages)


def test_shared_engine_across_threads_gives_correct_output():
    engine = ferropdf.Engine()
    docs = [HTML.format(n=i) for i in range(24)]
    with ThreadPoolExecutor(4) as pool:
        pdfs = list(pool.map(engine.render, docs))
    for i, pdf in enumerate(pdfs):
        text = _text(pdf)
        assert f"Invoice {i}" in text and f"{i}.00 EUR" in text


def test_one_shot_functions_across_threads():
    docs = [HTML.format(n=i) for i in range(12)]
    with ThreadPoolExecutor(4) as pool:
        pdfs = list(pool.map(ferropdf.from_html, docs))
    assert all(f"Invoice {i}" in _text(pdf) for i, pdf in enumerate(pdfs))
