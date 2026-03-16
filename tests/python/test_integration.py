"""Tests d'intégration touchant le vrai moteur Rust."""
import pytest

try:
    from fastpdf import render_pdf, render_pdf_to_file, RenderOptions, batch_render
    HAS_ENGINE = True
except ImportError:
    HAS_ENGINE = False

pytestmark = pytest.mark.skipif(not HAS_ENGINE, reason="Rust engine not built")


class TestBasicRendering:
    def test_minimal_html(self):
        result = render_pdf("<h1>Hello</h1>")
        assert result[:4] == b"%PDF"

    def test_returns_bytes(self):
        result = render_pdf("<p>Test</p>")
        assert isinstance(result, bytes)
        assert len(result) > 200

    def test_html_with_css(self):
        html = "<p class='styled'>Text</p>"
        css = ".styled { color: #ff0000; font-size: 14pt; }"
        result = render_pdf(html, css=css)
        assert result[:4] == b"%PDF"

    def test_complete_html_document(self):
        html = """<!DOCTYPE html>
        <html><head><title>Test</title></head>
        <body><h1>Title</h1><p>Content</p></body></html>"""
        result = render_pdf(html)
        assert result[:4] == b"%PDF"

    def test_save_to_file(self, tmp_path):
        path = str(tmp_path / "out.pdf")
        render_pdf_to_file("<h1>Test</h1>", path)
        with open(path, "rb") as f:
            content = f.read()
        assert content[:4] == b"%PDF"


class TestCssFeatures:
    def test_class_selector(self):
        result = render_pdf(
            "<div class='box'>Content</div>",
            css=".box { padding: 20px; background: #f0f0f0; }"
        )
        assert result[:4] == b"%PDF"

    def test_id_selector(self):
        result = render_pdf(
            "<div id='main'>Content</div>",
            css="#main { font-size: 18px; color: #333; }"
        )
        assert result[:4] == b"%PDF"

    def test_descendant_selector(self):
        result = render_pdf(
            "<div class='parent'><p>Child</p></div>",
            css=".parent p { color: blue; margin: 10px; }"
        )
        assert result[:4] == b"%PDF"

    def test_inline_styles(self):
        html = '<p style="color: red; font-size: 16px; margin: 10px">Styled</p>'
        result = render_pdf(html)
        assert result[:4] == b"%PDF"

    def test_css_variables(self):
        html = "<p class='accent'>Variable text</p>"
        css = ":root { --primary: #0066cc; } .accent { color: var(--primary); }"
        result = render_pdf(html, css=css)
        assert result[:4] == b"%PDF"

    def test_calc_expression(self):
        html = "<div style='width: calc(100% - 40px); padding: 20px'>Content</div>"
        result = render_pdf(html)
        assert result[:4] == b"%PDF"


class TestLayoutFeatures:
    def test_flexbox_row(self):
        html = """<div style="display:flex;justify-content:space-between">
            <span>Left</span><span>Center</span><span>Right</span>
        </div>"""
        result = render_pdf(html)
        assert result[:4] == b"%PDF"

    def test_flexbox_column(self):
        html = """<div style="display:flex;flex-direction:column;gap:8px">
            <div>Item 1</div><div>Item 2</div><div>Item 3</div>
        </div>"""
        result = render_pdf(html)
        assert result[:4] == b"%PDF"

    def test_table(self):
        html = """<table border="1">
            <thead><tr><th>Name</th><th>Value</th><th>Status</th></tr></thead>
            <tbody>
                <tr><td>Item A</td><td>100</td><td>Active</td></tr>
                <tr><td>Item B</td><td>200</td><td>Inactive</td></tr>
            </tbody>
        </table>"""
        result = render_pdf(html)
        assert result[:4] == b"%PDF"

    def test_nested_layout(self):
        html = """<div style="display:flex">
            <div style="flex:1"><h2>Column 1</h2><p>Content A</p></div>
            <div style="flex:1"><h2>Column 2</h2><p>Content B</p></div>
        </div>"""
        result = render_pdf(html)
        assert result[:4] == b"%PDF"


class TestTailwind:
    def test_tailwind_spacing(self):
        html = '<div class="p-8 m-4"><h1 class="text-2xl font-bold">Title</h1></div>'
        result = render_pdf(html, options=RenderOptions(tailwind=True))
        assert result[:4] == b"%PDF"

    def test_tailwind_colors(self):
        html = '<p class="text-blue-600 bg-gray-100 p-4">Colored text</p>'
        result = render_pdf(html, options=RenderOptions(tailwind=True))
        assert result[:4] == b"%PDF"

    def test_tailwind_flex(self):
        html = """<div class="flex justify-between items-center p-4">
            <span class="font-bold">Left</span>
            <span class="text-gray-500">Right</span>
        </div>"""
        result = render_pdf(html, options=RenderOptions(tailwind=True))
        assert result[:4] == b"%PDF"


class TestEdgeCases:
    def test_empty_html(self):
        result = render_pdf("")
        assert result[:4] == b"%PDF"

    def test_invalid_html_no_crash(self):
        result = render_pdf("<<<not>valid<html>>>")
        assert result[:4] == b"%PDF"

    def test_unicode_content(self):
        html = "<p>Ünïcödé têxt wïth spécïàl chàrs: é à ü ñ</p>"
        result = render_pdf(html)
        assert result[:4] == b"%PDF"

    def test_large_document(self):
        rows = "\n".join(
            f"<tr><td>Row {i}</td><td>Value {i * 10}</td></tr>"
            for i in range(200)
        )
        html = (
            f"<table><thead><tr><th>Name</th><th>Value</th></tr></thead>"
            f"<tbody>{rows}</tbody></table>"
        )
        result = render_pdf(html)
        assert result[:4] == b"%PDF"
        assert len(result) > 5000

    def test_all_page_sizes(self):
        for size in ["A4", "A3", "Letter", "Legal"]:
            result = render_pdf("<p>Test</p>", options=RenderOptions(page_size=size))
            assert result[:4] == b"%PDF", f"Failed for page size {size}"

    def test_landscape_orientation(self):
        result = render_pdf(
            "<p>Landscape</p>",
            options=RenderOptions(orientation="landscape")
        )
        assert result[:4] == b"%PDF"
