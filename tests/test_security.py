"""End-to-end security tests for the resource sandbox.

These tests confirm that adversarial HTML cannot reach outside the directory
named by `Options(base_url=...)`. They are deliberately end-to-end (driven
through the Python API) so they catch regressions across the whole pipeline,
not just the Rust unit tests in `crates/ferropdf-render/src/sandbox.rs`.
"""

from __future__ import annotations

import os
import tempfile
import warnings
from pathlib import Path

import pytest

import ferropdf
from ferropdf import Engine, Options


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def render_with_warnings(html: str, base_url: str | None = None) -> tuple[bytes, list[str]]:
    """Render HTML and return (pdf_bytes, warnings) — exercises the new API."""
    opts = Options(base_url=base_url) if base_url else Options()
    return Engine(opts).render_with_warnings(html)


def assert_pdf(pdf: bytes) -> None:
    assert isinstance(pdf, bytes)
    assert pdf.startswith(b"%PDF-")


# ---------------------------------------------------------------------------
# Image src sandbox
# ---------------------------------------------------------------------------


class TestImageSandbox:
    def test_absolute_path_without_base_url_blocked(self, tmp_path):
        """`<img src="/etc/passwd">` without base_url must NOT read the file."""
        secret = tmp_path / "SECRET.txt"
        secret.write_text("top-secret-marker-string")
        html = f'<html><body><img src="{secret}"></body></html>'

        pdf, warns = render_with_warnings(html)

        assert_pdf(pdf)
        # The secret bytes must not appear in the PDF output.
        assert b"top-secret-marker-string" not in pdf
        # And we should be told why.
        assert any("base_url" in w for w in warns), warns

    def test_absolute_path_with_base_url_blocked(self, tmp_path):
        """Even with base_url set, absolute paths bypass the sandbox and must be refused."""
        outside = tmp_path / "outside.txt"
        outside.write_text("outside-secret")

        sandbox = tmp_path / "sandbox"
        sandbox.mkdir()

        html = f'<html><body><img src="{outside}"></body></html>'
        pdf, warns = render_with_warnings(html, base_url=str(sandbox))

        assert b"outside-secret" not in pdf
        assert any("outside" in w.lower() or "absolute" in w.lower() for w in warns), warns

    def test_relative_path_traversal_blocked(self, tmp_path):
        """`<img src="../../../etc/passwd">` must be blocked by canonicalize check."""
        secret = tmp_path / "secret.txt"
        secret.write_text("traversal-secret")

        sandbox = tmp_path / "sandbox"
        sandbox.mkdir()

        html = '<html><body><img src="../secret.txt"></body></html>'
        pdf, warns = render_with_warnings(html, base_url=str(sandbox))

        assert_pdf(pdf)
        assert b"traversal-secret" not in pdf
        assert any("outside" in w.lower() for w in warns), warns

    def test_data_uri_always_works(self):
        """data: URIs do not touch the filesystem and must always work."""
        # 1×1 transparent PNG (standard known-valid bytes).
        png_b64 = (
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42m"
            "Nk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=="
        )
        html = f'<html><body><img src="data:image/png;base64,{png_b64}"></body></html>'
        pdf, warns = render_with_warnings(html)
        assert_pdf(pdf)
        # No image-related warnings — data URIs are not sandboxed.
        assert not any("image" in w.lower() for w in warns), warns

    def test_http_url_skipped(self):
        """http(s):// must be skipped without making any network call."""
        html = '<html><body><img src="http://example.com/x.png"></body></html>'
        pdf, warns = render_with_warnings(html)
        assert_pdf(pdf)
        assert any("http" in w.lower() for w in warns), warns

    def test_relative_path_in_sandbox_works(self, tmp_path):
        """A file genuinely inside base_url should load (smoke test for the happy path)."""
        # Tiny valid 1×1 transparent PNG.
        import base64
        png_bytes = base64.b64decode(
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42m"
            "Nk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=="
        )
        img = tmp_path / "logo.png"
        img.write_bytes(png_bytes)

        html = '<html><body><img src="logo.png"></body></html>'
        pdf, warns = render_with_warnings(html, base_url=str(tmp_path))
        assert_pdf(pdf)
        # Should not warn about this image since it's in the sandbox and decodable.
        image_warns = [w for w in warns if "logo.png" in w]
        assert image_warns == [], image_warns


# ---------------------------------------------------------------------------
# Stylesheet href sandbox
# ---------------------------------------------------------------------------


class TestStylesheetSandbox:
    def test_absolute_link_without_base_url_blocked(self, tmp_path):
        secret = tmp_path / "secret.css"
        secret.write_text("body { background: red; } /* SECRET-CSS */")
        html = f'<html><head><link rel="stylesheet" href="{secret}"></head><body><p>x</p></body></html>'

        pdf, warns = render_with_warnings(html)

        assert_pdf(pdf)
        # The CSS must not have been read; presence of comment is incidental, but the warning is the contract.
        assert any("base_url" in w or "stylesheet" in w.lower() for w in warns), warns

    def test_traversal_in_link_href_blocked(self, tmp_path):
        secret = tmp_path / "secret.css"
        secret.write_text("body { font-size: 999pt; }")
        sandbox = tmp_path / "sandbox"
        sandbox.mkdir()

        html = '<html><head><link rel="stylesheet" href="../secret.css"></head><body><p>x</p></body></html>'
        _pdf, warns = render_with_warnings(html, base_url=str(sandbox))

        assert any("stylesheet" in w.lower() and "outside" in w.lower() for w in warns), warns


# ---------------------------------------------------------------------------
# HTML size limit
# ---------------------------------------------------------------------------


class TestMaxHtmlBytes:
    def test_oversize_html_rejected(self):
        """Renders larger than max_html_bytes must raise rather than consume memory."""
        opts = Options(max_html_bytes=1024)
        engine = Engine(opts)

        # 4 KB > 1 KB cap
        html = "<p>x</p>" + ("a" * 4096)

        with pytest.raises(ferropdf.LayoutError) as excinfo:
            engine.render(html)
        assert "max_html_bytes" in str(excinfo.value)

    def test_default_cap_allows_normal_documents(self):
        """The default 10 MiB cap must not interfere with realistic invoices."""
        rows = "".join(
            f"<tr><td>{i}</td><td>Line item {i}</td><td>{i * 13}€</td></tr>"
            for i in range(200)
        )
        html = f"<table>{rows}</table>"
        pdf = ferropdf.from_html(html)
        assert_pdf(pdf)


# ---------------------------------------------------------------------------
# Backwards-compat: render() must not raise on warnings
# ---------------------------------------------------------------------------


class TestBackwardsCompat:
    def test_render_with_missing_image_does_not_raise(self):
        """The legacy `engine.render()` must keep working even when assets fail."""
        html = '<html><body><img src="/nonexistent/path.png"><p>hi</p></body></html>'

        with warnings.catch_warnings(record=True) as caught:
            warnings.simplefilter("always")
            pdf = Engine().render(html)

        assert_pdf(pdf)
        # The render should have emitted a Python warning about the missing image.
        assert any("image" in str(w.message).lower() for w in caught), [
            str(w.message) for w in caught
        ]

    def test_from_html_top_level_function_still_works(self):
        """The `ferropdf.from_html(...)` convenience must keep its legacy signature."""
        pdf = ferropdf.from_html("<h1>Hello</h1>")
        assert_pdf(pdf)
