"""Render the README gallery: one PDF and one PNG thumbnail per document.

    python examples/gallery/build.py

Writes ``pdf/<name>.pdf`` and ``img/<name>.png`` next to this file. The
README links to both. Re-run it after changing a document or the renderer.
"""

from __future__ import annotations

import time
from dataclasses import dataclass, field
from pathlib import Path

import ferropdf

HERE = Path(__file__).resolve().parent
THUMB_DPI = 50  # A4 → 414 × 585 px


@dataclass
class Doc:
    name: str
    title: str
    features: str
    options: dict = field(default_factory=dict)

    @property
    def html(self) -> str:
        return (HERE / f"{self.name}.html").read_text(encoding="utf-8")


DOCS = [
    Doc(
        "invoice",
        "Invoice",
        "Bootstrap-style CSS · Factur-X · PDF/A-3b",
        {"factur_x": (HERE / "invoice.factur-x.xml").read_bytes()},
    ),
    Doc(
        "report",
        "Report",
        "Running headers · CSS grid · SVG chart · PDF/UA",
        {"pdf_ua": True},
    ),
    Doc("letter", "Letter", "Serif typography · justified text"),
    Doc("receipt", "Receipt", "80 mm roll · monospace"),
    Doc("certificate", "Certificate", "Landscape · gradient · rotated seal"),
    Doc("invoice_arabic", "Arabic invoice", "Right-to-left · Arabic shaping"),
]


def render(doc: Doc) -> tuple[bytes, bytes]:
    """PDF bytes and a PNG thumbnail of the first page."""
    engine = ferropdf.Engine(ferropdf.Options(base_url=str(HERE), **doc.options))
    pdf = engine.render(doc.html)
    (thumb,) = engine.render_png(doc.html, dpi=THUMB_DPI, pages=0)
    return pdf, thumb


def main() -> None:
    (HERE / "pdf").mkdir(exist_ok=True)
    (HERE / "img").mkdir(exist_ok=True)
    for doc in DOCS:
        t0 = time.perf_counter()
        pdf, thumb = render(doc)
        ms = (time.perf_counter() - t0) * 1000
        (HERE / "pdf" / f"{doc.name}.pdf").write_bytes(pdf)
        (HERE / "img" / f"{doc.name}.png").write_bytes(thumb)
        print(f"{doc.name:16} {len(pdf) / 1024:6.1f} KiB PDF  {ms:5.0f} ms (PDF + thumbnail)")


if __name__ == "__main__":
    main()
