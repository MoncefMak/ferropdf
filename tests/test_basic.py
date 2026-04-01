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


def extract_media_box_mm(pdf_bytes):
    """Extract (width_mm, height_mm) from the first /MediaBox in the PDF."""
    import re
    content = pdf_bytes.decode('latin-1', errors='replace')
    match = re.search(r'/MediaBox\s*\[([^\]]+)\]', content)
    assert match, "No /MediaBox found in PDF"
    coords = match.group(1).split()
    w_pt, h_pt = float(coords[2]), float(coords[3])
    return (w_pt / 72 * 25.4, h_pt / 72 * 25.4)


SIMPLE_HTML = "<html><body><p>Test</p></body></html>"


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


class TestPageSize:
    """Test that page_size option produces correct MediaBox dimensions."""

    @pytest.mark.parametrize("name,expected_w,expected_h", [
        ("A3",     841.89, 1190.55),
        ("A4",     595.28,  841.89),
        ("A5",     419.53,  595.28),
        ("A6",     297.64,  419.53),
        ("Letter", 612.0,   792.0),
        ("Legal",  612.0,  1008.0),
    ])
    def test_named_sizes(self, name, expected_w, expected_h):
        opts = ferropdf.Options(page_size=name, margin="0mm")
        pdf = ferropdf.from_html(SIMPLE_HTML, options=opts)
        assert pdf_is_valid(pdf)
        w_mm, h_mm = extract_media_box_mm(pdf)
        exp_w_mm = expected_w / 72 * 25.4
        exp_h_mm = expected_h / 72 * 25.4
        assert abs(w_mm - exp_w_mm) < 0.5, f"{name}: width {w_mm:.1f} != {exp_w_mm:.1f}"
        assert abs(h_mm - exp_h_mm) < 0.5, f"{name}: height {h_mm:.1f} != {exp_h_mm:.1f}"

    @pytest.mark.parametrize("name", ["a4", "a3", "A4", "letter", "LETTER"])
    def test_case_insensitive(self, name):
        opts = ferropdf.Options(page_size=name, margin="0mm")
        pdf = ferropdf.from_html(SIMPLE_HTML, options=opts)
        assert pdf_is_valid(pdf)

    @pytest.mark.parametrize("size_str,expected_w_mm,expected_h_mm", [
        ("90mm 76mm",   90,  76),
        ("90mm 130mm",  90, 130),
        ("105mm 80mm", 105,  80),
        ("210mm 297mm", 210, 297),
        ("100mm 100mm", 100, 100),
    ])
    def test_custom_mm(self, size_str, expected_w_mm, expected_h_mm):
        opts = ferropdf.Options(page_size=size_str, margin="0mm")
        pdf = ferropdf.from_html(SIMPLE_HTML, options=opts)
        assert pdf_is_valid(pdf)
        w_mm, h_mm = extract_media_box_mm(pdf)
        assert abs(w_mm - expected_w_mm) < 0.5, f"width {w_mm:.1f} != {expected_w_mm}"
        assert abs(h_mm - expected_h_mm) < 0.5, f"height {h_mm:.1f} != {expected_h_mm}"

    @pytest.mark.parametrize("size_str,expected_w_mm,expected_h_mm", [
        ("8.5in 11in",   215.9, 279.4),
        ("5.5in 8.5in",  139.7, 215.9),
    ])
    def test_custom_inches(self, size_str, expected_w_mm, expected_h_mm):
        opts = ferropdf.Options(page_size=size_str, margin="0mm")
        pdf = ferropdf.from_html(SIMPLE_HTML, options=opts)
        assert pdf_is_valid(pdf)
        w_mm, h_mm = extract_media_box_mm(pdf)
        assert abs(w_mm - expected_w_mm) < 0.5, f"width {w_mm:.1f} != {expected_w_mm}"
        assert abs(h_mm - expected_h_mm) < 0.5, f"height {h_mm:.1f} != {expected_h_mm}"

    @pytest.mark.parametrize("size_str,expected_w_mm,expected_h_mm", [
        ("10cm 15cm",  100, 150),
        ("21cm 29.7cm", 210, 297),
    ])
    def test_custom_cm(self, size_str, expected_w_mm, expected_h_mm):
        opts = ferropdf.Options(page_size=size_str, margin="0mm")
        pdf = ferropdf.from_html(SIMPLE_HTML, options=opts)
        assert pdf_is_valid(pdf)
        w_mm, h_mm = extract_media_box_mm(pdf)
        assert abs(w_mm - expected_w_mm) < 0.5, f"width {w_mm:.1f} != {expected_w_mm}"
        assert abs(h_mm - expected_h_mm) < 0.5, f"height {h_mm:.1f} != {expected_h_mm}"

    @pytest.mark.parametrize("size_str,expected_w_pt,expected_h_pt", [
        ("595pt 842pt",  595, 842),
        ("300pt 400pt",  300, 400),
    ])
    def test_custom_pt(self, size_str, expected_w_pt, expected_h_pt):
        opts = ferropdf.Options(page_size=size_str, margin="0mm")
        pdf = ferropdf.from_html(SIMPLE_HTML, options=opts)
        assert pdf_is_valid(pdf)
        w_mm, h_mm = extract_media_box_mm(pdf)
        exp_w_mm = expected_w_pt / 72 * 25.4
        exp_h_mm = expected_h_pt / 72 * 25.4
        assert abs(w_mm - exp_w_mm) < 0.5, f"width {w_mm:.1f} != {exp_w_mm:.1f}"
        assert abs(h_mm - exp_h_mm) < 0.5, f"height {h_mm:.1f} != {exp_h_mm:.1f}"

    def test_invalid_falls_back_to_a4(self):
        opts = ferropdf.Options(page_size="garbage", margin="0mm")
        pdf = ferropdf.from_html(SIMPLE_HTML, options=opts)
        assert pdf_is_valid(pdf)
        w_mm, h_mm = extract_media_box_mm(pdf)
        assert abs(w_mm - 210.0) < 0.5
        assert abs(h_mm - 297.0) < 0.5


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
