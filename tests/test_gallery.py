"""The README gallery documents render with their showcased options (#45)."""

from __future__ import annotations

import io
import sys
from pathlib import Path

import pytest
from pypdf import PdfReader

GALLERY = Path(__file__).resolve().parent.parent / "examples" / "gallery"
sys.path.insert(0, str(GALLERY))
import build  # noqa: E402


@pytest.mark.parametrize("doc", build.DOCS, ids=lambda d: d.name)
def test_gallery_document_renders(doc):
    pdf, thumb = build.render(doc)
    reader = PdfReader(io.BytesIO(pdf))
    assert len(reader.pages) >= 1
    assert thumb[:8] == b"\x89PNG\r\n\x1a\n"
    assert (GALLERY / "img" / f"{doc.name}.png").exists(), "run examples/gallery/build.py"
    assert (GALLERY / "pdf" / f"{doc.name}.pdf").exists(), "run examples/gallery/build.py"


def test_gallery_invoice_is_factur_x():
    (doc,) = [d for d in build.DOCS if d.name == "invoice"]
    pdf, _ = build.render(doc)
    assert list(PdfReader(io.BytesIO(pdf)).attachments) == ["factur-x.xml"]
