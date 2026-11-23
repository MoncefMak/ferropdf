"""Structural-fingerprint tests for full PDF outputs.

Each fixture in tests/golden/fixtures/*.html is rendered and a small
structural fingerprint (page count + extracted text per page + PDF object
count) is compared to a stored expected fingerprint in
tests/golden/expected/*.json.

A byte-level SHA-256 of the PDF is *not* used because ferropdf's output
isn't byte-deterministic across renders today — font discovery order in
the Rust pipeline depends on `HashMap` iteration which uses a randomised
hasher. Structural comparison still catches the regressions that matter
(missing pages, dropped text, lost color operators) without the
flakiness.

Refresh fingerprints after intentional changes:

    pytest tests/test_golden.py --regenerate-golden
"""

from __future__ import annotations

import io
import json
import re
from pathlib import Path

import pypdf
import pytest

import ferropdf

GOLDEN_DIR = Path(__file__).parent / "golden"
FIXTURE_DIR = GOLDEN_DIR / "fixtures"
EXPECTED_DIR = GOLDEN_DIR / "expected"


def _normalise_text(text: str) -> str:
    """Collapse whitespace and strip — the extracted text from pypdf has
    minor whitespace variations across runs that don't reflect real
    regressions."""
    return re.sub(r"\s+", " ", text).strip()


def _fingerprint(pdf: bytes) -> dict:
    reader = pypdf.PdfReader(io.BytesIO(pdf))
    pages = [_normalise_text(p.extract_text() or "") for p in reader.pages]

    # Operators are counted in the decoded page content streams only: a
    # regex over the raw file also hits random bytes inside compressed font
    # and image streams, which change with the zlib backend.
    content = b"\n".join(
        c.get_data() for p in reader.pages if (c := p.get_contents()) is not None
    )
    # Number of color operators is a coarse proxy for "did style rules
    # apply": missing colors usually mean a selector was silently dropped.
    rg_ops = len(re.findall(rb"\d(?:\.\d+)?\s+\d(?:\.\d+)?\s+\d(?:\.\d+)?\s+rg", content))
    bt_ops = len(re.findall(rb"\bBT\b", content))  # text-block markers
    image_ops = len(re.findall(rb"\bDo\b", content))  # XObject draws (images)

    return {
        "page_count": len(reader.pages),
        "pages_text": pages,
        "rg_op_count": rg_ops,
        "text_block_count": bt_ops,
        "image_draw_count": image_ops,
    }


def _fixture_paths():
    return sorted(FIXTURE_DIR.glob("*.html"))


@pytest.mark.parametrize(
    "fixture",
    _fixture_paths(),
    ids=lambda p: p.stem,
)
def test_golden_fingerprint(fixture: Path, regenerate_golden: bool):
    html = fixture.read_text(encoding="utf-8")
    pdf = ferropdf.from_html(html)
    actual = _fingerprint(pdf)

    expected_file = EXPECTED_DIR / f"{fixture.stem}.json"

    if regenerate_golden:
        EXPECTED_DIR.mkdir(parents=True, exist_ok=True)
        expected_file.write_text(
            json.dumps(actual, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )
        pytest.skip(f"regenerated {expected_file.name}")

    if not expected_file.exists():
        pytest.fail(
            f"no recorded golden for {fixture.stem}; run with "
            f"--regenerate-golden to create one"
        )

    expected = json.loads(expected_file.read_text(encoding="utf-8"))
    assert actual == expected, (
        f"PDF fingerprint drift for {fixture.stem}.\n"
        f"If the change was intentional, regenerate with:\n"
        f"  pytest tests/test_golden.py --regenerate-golden"
    )
