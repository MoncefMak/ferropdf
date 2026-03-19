"""
Tests fondamentaux — ces tests doivent TOUS passer avant de considérer
le projet fonctionnel.
"""
import ferropdf
import pytest

def pdf_is_valid(data: bytes) -> bool:
    return data[:4] == b"%PDF"

def count_pages(pdf: bytes) -> int:
    """Compter les pages dans un PDF de manière approximative."""
    return pdf.count(b"/Type /Page\n")


class TestModule:
    def test_import(self):
        assert hasattr(ferropdf, "Engine")
        assert hasattr(ferropdf, "Options")
        assert hasattr(ferropdf, "from_html")
        assert hasattr(ferropdf, "from_file")
        assert hasattr(ferropdf, "__version__")

    def test_from_html_minimal(self):
        pdf = ferropdf.from_html("<p>Hello</p>")
        assert pdf_is_valid(pdf)

    def test_from_html_with_styles(self):
        html = open("tests/fixtures/simple.html").read()
        pdf  = ferropdf.from_html(html)
        assert pdf_is_valid(pdf)

    def test_from_file(self):
        pdf = ferropdf.from_file("tests/fixtures/simple.html")
        assert pdf_is_valid(pdf)

    def test_write_pdf(self, tmp_path):
        out = tmp_path / "out.pdf"
        ferropdf.write_pdf("<p>Test</p>", str(out))
        assert out.exists()
        assert pdf_is_valid(out.read_bytes())

    def test_empty_html(self):
        pdf = ferropdf.from_html("")
        assert pdf_is_valid(pdf)

    def test_malformed_html_no_crash(self):
        cases = [
            "<p>Unclosed",
            "<div><p>Double unclosed",
            "Texte brut sans balises",
            "<script>alert(1)</script><p>xss</p>",
        ]
        for html in cases:
            assert pdf_is_valid(ferropdf.from_html(html)), f"Crash sur: {html!r}"


class TestOptions:
    def test_default(self):
        opts = ferropdf.Options()
        pdf  = ferropdf.from_html("<p>Test</p>", options=opts)
        assert pdf_is_valid(pdf)

    def test_a4(self):
        opts = ferropdf.Options(page_size="A4", margin="20mm")
        assert pdf_is_valid(ferropdf.from_html("<p>A4</p>", options=opts))

    def test_letter(self):
        opts = ferropdf.Options(page_size="Letter")
        assert pdf_is_valid(ferropdf.from_html("<p>Letter</p>", options=opts))

    def test_a3(self):
        opts = ferropdf.Options(page_size="A3")
        assert pdf_is_valid(ferropdf.from_html("<p>A3</p>", options=opts))


class TestLayout:
    def test_width_100_percent(self):
        html = """
        <div style="width:500px">
          <table style="width:100%">
            <tr><td>Col 1</td><td>Col 2</td><td>Col 3</td></tr>
            <tr><td>Data</td><td>Data</td><td>Data</td></tr>
          </table>
        </div>
        """
        pdf = ferropdf.from_html(html)
        assert pdf_is_valid(pdf)
        assert count_pages(pdf) <= 2

    def test_no_double_padding(self):
        html = """
        <div style="width:400px; padding:20px; background:#eee">
          <div style="width:100%; background:#ccc">
            <p style="padding:10px">Texte imbriqué</p>
          </div>
        </div>
        """
        pdf = ferropdf.from_html(html)
        assert pdf_is_valid(pdf)
        assert count_pages(pdf) == 1

    def test_flex_row(self):
        html = """
        <div style="display:flex; width:600px; gap:20px">
          <div style="flex:1; background:red; min-height:50px">A</div>
          <div style="flex:1; background:blue; min-height:50px">B</div>
          <div style="flex:1; background:green; min-height:50px">C</div>
        </div>
        """
        assert pdf_is_valid(ferropdf.from_html(html))

    def test_invoice_page_count(self):
        html = open("tests/fixtures/invoice.html").read()
        pdf  = ferropdf.from_html(html)
        assert pdf_is_valid(pdf)
        pages = count_pages(pdf)
        assert pages <= 2, f"Invoice : {pages} pages détectées (max 2)"


def extract_text_positions(pdf_bytes):
    """Extract (x, y, font_name) tuples from PDF text operations."""
    import re
    content = pdf_bytes.decode('latin-1', errors='replace')
    positions = []
    current_font = None
    for line in content.split('\n'):
        line = line.strip()
        font_m = re.match(r'/(\w+)\s+[\d.]+\s+Tf', line)
        if font_m:
            current_font = font_m.group(1)
        td_m = re.match(r'([\d.]+)\s+([\d.]+)\s+Td', line)
        if td_m:
            positions.append((float(td_m.group(1)), float(td_m.group(2)), current_font))
    return positions


class TestInlineLayout:
    """Tests for inline element rendering (<strong>, <em>, <span>, <a>, etc.)."""

    def test_inline_bold_italic_same_line(self):
        """Inline elements should render on the same line, not stack vertically."""
        html = "<p>This is <strong>bold</strong> and <em>italic</em> text.</p>"
        pdf = ferropdf.from_html(html)
        assert pdf_is_valid(pdf)
        positions = extract_text_positions(pdf)
        assert len(positions) >= 3, f"Expected at least 3 text segments, got {len(positions)}"
        # All segments must share the same Y value (horizontal flow)
        ys = set(y for _, y, _ in positions)
        assert len(ys) == 1, f"Text should be on one line, but got Y values: {sorted(ys)}"
        # X values must be increasing (left-to-right)
        xs = [x for x, _, _ in positions]
        assert xs == sorted(xs), f"X positions should increase: {xs}"

    def test_inline_uses_correct_fonts(self):
        """Bold text should use a different font than normal text."""
        html = "<p>Normal <strong>bold</strong> normal</p>"
        pdf = ferropdf.from_html(html)
        assert pdf_is_valid(pdf)
        positions = extract_text_positions(pdf)
        fonts = [f for _, _, f in positions if f]
        assert len(set(fonts)) >= 2, f"Expected at least 2 distinct fonts, got: {fonts}"

    def test_inline_span_no_crash(self):
        """Span elements should render as inline without crashing."""
        html = '<p>Hello <span style="color:red">world</span>!</p>'
        pdf = ferropdf.from_html(html)
        assert pdf_is_valid(pdf)

    def test_inline_link(self):
        """Links should render inline."""
        html = '<p>Click <a href="#">here</a> for more.</p>'
        pdf = ferropdf.from_html(html)
        assert pdf_is_valid(pdf)
        positions = extract_text_positions(pdf)
        ys = set(y for _, y, _ in positions)
        assert len(ys) == 1, f"Link should be inline, but got Y values: {sorted(ys)}"

    def test_nested_inline(self):
        """Nested inline elements should work."""
        html = "<p>This is <strong><em>bold italic</em></strong> text.</p>"
        pdf = ferropdf.from_html(html)
        assert pdf_is_valid(pdf)
        positions = extract_text_positions(pdf)
        ys = set(y for _, y, _ in positions)
        assert len(ys) == 1, f"Nested inline should be one line, got Y: {sorted(ys)}"

    def test_inline_does_not_break_block(self):
        """Block elements after inline paragraphs should still work."""
        html = """
        <p>Paragraph with <strong>bold</strong> text.</p>
        <div style="background:blue; height:50px">Block</div>
        <p>Another <em>italic</em> paragraph.</p>
        """
        pdf = ferropdf.from_html(html)
        assert pdf_is_valid(pdf)


class TestEngine:
    def test_reusable(self):
        engine = ferropdf.Engine()
        r1 = engine.render("<p>Doc 1</p>")
        r2 = engine.render("<p>Doc 2</p>")
        assert pdf_is_valid(r1)
        assert pdf_is_valid(r2)
        assert r1 != r2

    def test_render_file(self, tmp_path):
        f = tmp_path / "page.html"
        f.write_text("<h1>From file</h1>", encoding="utf-8")
        engine = ferropdf.Engine()
        assert pdf_is_valid(engine.render_file(str(f)))


class TestErrors:
    def test_hierarchy(self):
        assert issubclass(ferropdf.ParseError,  ferropdf.FerroError)
        assert issubclass(ferropdf.LayoutError, ferropdf.FerroError)
        assert issubclass(ferropdf.FontError,   ferropdf.FerroError)
        assert issubclass(ferropdf.RenderError, ferropdf.FerroError)

    def test_ferro_error_is_exception(self):
        assert issubclass(ferropdf.FerroError, Exception)
