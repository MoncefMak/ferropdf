"""Visual regression tests for the template corpus.

Every page of every template in ``tests/corpus/templates`` is rasterised with
PyMuPDF and compared against ``tests/corpus/reference/<template>-p<N>.png``.
A page fails when more than ``MAX_CHANGED_PIXELS`` pixels differ by more than
``PIXEL_TOLERANCE`` (max over channels). The threshold is absolute rather than
a ratio so that a single changed digit on an invoice is still caught.

Font rasterisation differs between platforms, so these tests are opt-in and
run in CI on Linux only (DejaVu fonts installed):

    FERROPDF_VISUAL=1 pytest tests/test_visual.py

After an intentional rendering change, refresh the references and review the
PNG diff in the pull request:

    FERROPDF_VISUAL=1 pytest tests/test_visual.py --regenerate-visual

On failure, actual renders and diff masks are written to
``tests/corpus/_output/`` (uploaded as a CI artifact).
"""

from __future__ import annotations

import os
import warnings
from pathlib import Path

import pytest

if not os.environ.get("FERROPDF_VISUAL"):
    pytest.skip("visual tests are opt-in: set FERROPDF_VISUAL=1", allow_module_level=True)

pymupdf = pytest.importorskip("pymupdf")
Image = pytest.importorskip("PIL.Image")
ImageChops = pytest.importorskip("PIL.ImageChops")

import ferropdf  # noqa: E402

CORPUS = Path(__file__).parent / "corpus"
TEMPLATES = CORPUS / "templates"
REFERENCE = CORPUS / "reference"
OUTPUT = CORPUS / "_output"

DPI = 60
PIXEL_TOLERANCE = 64  # 0-255; absorbs anti-aliasing noise
MAX_CHANGED_PIXELS = 8  # one changed digit at 60 dpi is ~12 pixels


def render_pages(template: Path) -> list:
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        pdf = ferropdf.from_html(template.read_text(encoding="utf-8"), base_url=str(TEMPLATES))
    doc = pymupdf.open(stream=pdf, filetype="pdf")
    pages = []
    for page in doc:
        pix = page.get_pixmap(dpi=DPI)
        pages.append(Image.frombytes("RGB", (pix.width, pix.height), pix.samples))
    return pages


def changed_pixels(a, b) -> tuple[int, object]:
    r, g, b_ = ImageChops.difference(a, b).split()
    diff = ImageChops.lighter(ImageChops.lighter(r, g), b_)
    mask = diff.point(lambda v: 255 if v > PIXEL_TOLERANCE else 0)
    return sum(1 for v in mask.getdata() if v), mask


@pytest.mark.parametrize("template", sorted(TEMPLATES.glob("*.html")), ids=lambda p: p.stem)
def test_visual(template: Path, regenerate_visual: bool):
    pages = render_pages(template)
    stem = template.stem

    if regenerate_visual:
        REFERENCE.mkdir(exist_ok=True)
        for old in REFERENCE.glob(f"{stem}-p*.png"):
            old.unlink()
        for i, img in enumerate(pages, 1):
            img.save(REFERENCE / f"{stem}-p{i}.png", optimize=True)
        pytest.skip(f"regenerated {len(pages)} reference page(s) for {stem}")

    refs = sorted(REFERENCE.glob(f"{stem}-p*.png"), key=lambda p: int(p.stem.rsplit("-p", 1)[1]))
    if not refs:
        pytest.fail(f"no reference for {stem}; run with --regenerate-visual")

    failures = []
    if len(pages) != len(refs):
        failures.append(f"page count {len(pages)} != reference {len(refs)}")

    for i, (img, ref_path) in enumerate(zip(pages, refs), 1):
        ref = Image.open(ref_path).convert("RGB")
        if img.size != ref.size:
            failures.append(f"page {i}: size {img.size} != reference {ref.size}")
            continue
        changed, mask = changed_pixels(img, ref)
        if changed > MAX_CHANGED_PIXELS:
            OUTPUT.mkdir(exist_ok=True)
            img.save(OUTPUT / f"{stem}-p{i}.actual.png")
            mask.save(OUTPUT / f"{stem}-p{i}.diff.png")
            failures.append(f"page {i}: {changed} pixels differ")

    assert not failures, f"{stem}: " + "; ".join(failures) + f" (see {OUTPUT})"
