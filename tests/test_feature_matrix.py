"""Feature matrix — one small, structural check per CSS / PDF feature.

Every entry renders a minimal HTML snippet and asserts an observable property
of the PDF (page count, glyph positions, drawing operators, annotations...).

* Supported features are plain tests.
* Known gaps are marked ``xfail(strict=True)`` and point at their GitHub issue.
  When someone implements the feature the test XPASSes, which *fails* the run:
  remove the ``issue`` from the entry and update the README table.

The same table drives the README "CSS support" section:

    python tests/test_feature_matrix.py            # prints a Markdown table
"""

from __future__ import annotations

import base64
import struct
import sys
import warnings
import zlib
from dataclasses import dataclass
from typing import Callable

import pytest

pymupdf = pytest.importorskip("pymupdf") if __name__ != "__main__" else __import__("pymupdf")

import ferropdf  # noqa: E402

ISSUES = "https://github.com/MoncefMak/ferropdf/issues/"


def _has_dejavu() -> bool:
    """The matrix (and the README table) is defined on the reference platform:
    DejaVu fonts installed, as in the Linux CI job. Font-dependent checks are
    skipped elsewhere."""
    import glob
    import os

    dirs = ["/usr/share/fonts", "/usr/local/share/fonts", os.path.expanduser("~/.fonts"),
            os.path.expanduser("~/.local/share/fonts"), "/Library/Fonts",
            os.path.expanduser("~/Library/Fonts"), r"C:\Windows\Fonts"]
    return any(glob.glob(os.path.join(d, "**", "DejaVuSans.ttf"), recursive=True) for d in dirs if os.path.isdir(d))


HAS_DEJAVU = _has_dejavu()


# --------------------------------------------------------------------------- helpers


class Doc:
    """Rendered PDF with convenience accessors (coordinates in PDF points)."""

    def __init__(self, html: str, **options):
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            if options:
                self.pdf = ferropdf.from_html(html, options=ferropdf.Options(**options))
            else:
                self.pdf = ferropdf.from_html(html)
        self.doc = pymupdf.open(stream=self.pdf, filetype="pdf")

    @property
    def pages(self) -> int:
        return self.doc.page_count

    def text(self, page: int = 0) -> str:
        return " ".join(self.doc[page].get_text().split())

    def all_text(self) -> str:
        return " ".join(self.text(i) for i in range(self.pages))

    def words(self, page: int = 0):
        """[(x0, y0, x1, y1, word)]"""
        return [(w[0], w[1], w[2], w[3], w[4]) for w in self.doc[page].get_text("words")]

    def word(self, text: str, page: int = 0):
        for w in self.words(page):
            if w[4] == text:
                return w
        raise AssertionError(f"word {text!r} not found on page {page + 1}: {self.text(page)!r}")

    def drawings(self, page: int = 0):
        return self.doc[page].get_drawings()

    def page_width(self, page: int = 0) -> float:
        return self.doc[page].rect.width


def wrap(body: str, css: str = "") -> str:
    return (
        "<html><head><style>body{font-family:'DejaVu Sans',sans-serif}" + css + "</style></head>"
        f"<body>{body}</body></html>"
    )


def _png_data_uri(w: int = 4, h: int = 4, rgba: bytes | None = None) -> str:
    """Solid red RGB PNG, or an RGBA PNG filled with `rgba` (4 bytes)."""
    pixel, color_type = (rgba, 6) if rgba else (b"\xff\x00\x00", 2)
    raw = b"".join(b"\x00" + pixel * w for _ in range(h))

    def chunk(tag: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, color_type, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b"")
    return "data:image/png;base64," + base64.b64encode(png).decode()


SVG_URI = "data:image/svg+xml;base64," + base64.b64encode(
    b'<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40">'
    b'<rect width="40" height="40" fill="#f00"/></svg>'
).decode()

LONG_TEXT = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. " * 160


# --------------------------------------------------------------------------- checks
# Each check takes no argument and raises AssertionError when the feature is broken.


def chk_page_size():
    d = Doc(wrap("<p>x</p>", "@page{size:A6}"))
    assert round(d.page_width()) == 298


def chk_page_landscape():
    d = Doc(wrap("<p>x</p>", "@page{size:A4 landscape}"))
    assert round(d.page_width()) == 842


def chk_page_margin():
    d = Doc(wrap("<p>Start</p>", "@page{margin:30mm}"))
    assert abs(d.word("Start")[0] - 85.0) < 2


def chk_options_page_size():
    d = Doc(wrap("<p>x</p>"), page_size="Letter")
    assert round(d.page_width()) == 612


def chk_legacy_page_break():
    d = Doc(wrap('<p>A</p><p style="page-break-before:always">B</p>'))
    assert d.pages == 2


def chk_break_before():
    d = Doc(wrap('<p>A</p><p style="break-before:page">B</p>'))
    assert d.pages == 2


def chk_break_after():
    d = Doc(wrap('<p style="break-after:page">A</p><p>B</p>'))
    assert d.pages == 2


def chk_long_text_splits():
    d = Doc(wrap(f"<p>{LONG_TEXT}</p>"))
    assert d.pages >= 2 and "Lorem" in d.text(0) and "Lorem" in d.text(1)


def chk_many_blocks_paginate():
    d = Doc(wrap("".join(f"<p>Paragraph {i}</p>" for i in range(120))))
    assert d.pages >= 2 and "Paragraph 119" in d.all_text()


def chk_thead_repeat():
    rows = "".join(f"<tr><td>row{i}</td></tr>" for i in range(90))
    d = Doc(wrap(f"<table><thead><tr><th>HEADER</th></tr></thead><tbody>{rows}</tbody></table>"))
    assert d.pages >= 2 and d.text(1).startswith("HEADER")


def _long_table(extra: str = "") -> Doc:
    rows = "".join(f"<tr><td>row{i}</td></tr>" for i in range(100))
    return Doc(wrap(
        "<table><tfoot><tr><td>FOOTER</td></tr></tfoot>"
        "<thead><tr><th>HEAD1</th></tr><tr><th>HEAD2</th></tr></thead>"
        f"<tbody>{rows}</tbody></table><p>AFTER</p>"
    ))


def chk_tfoot_repeat():
    d = _long_table()
    assert d.pages >= 2
    for i in range(d.pages):
        assert "FOOTER" in d.text(i), f"footer missing on page {i + 1}"
    last = d.pages - 1
    assert d.word("AFTER", last)[1] > d.word("FOOTER", last)[1]


def chk_multi_row_thead():
    d = _long_table()
    for i in range(d.pages):
        assert d.word("HEAD2", i)[1] > d.word("HEAD1", i)[1] + 5, f"header rows overlap on page {i + 1}"
    rows = [w for i in range(d.pages) for w in d.text(i).split() if w.startswith("row")]
    assert len(rows) == len(set(rows)) == 100


def chk_page_margin_shorthand():
    d = Doc(wrap("<div>Start</div>", "@page{margin:40mm 10mm 20mm}"))
    w = d.word("Start")
    assert abs(w[0] - 28.3) < 2 and abs(w[1] - 113.4) < 4


def chk_running_element():
    css = ("@page{margin:30mm 20mm;@top-center{content:element(hdr)}}"
           ".h{position:running(hdr);display:flex;justify-content:space-between}")
    d = Doc(wrap('<div class="h"><span>LEFTH</span><span>RIGHTH</span></div>'
                 '<p>a</p><p style="break-before:page">b</p>', css))
    for i in range(2):
        left, right = d.word("LEFTH", i), d.word("RIGHTH", i)
        assert left[1] < 85 and right[2] > d.page_width() - 70
    assert d.word("a")[1] > 80, "running element is out of the flow"


def chk_margin_boxes():
    d = Doc(wrap("<p>body</p>", '@page{@bottom-center{content:"FOOTERTEXT"}}'))
    assert "FOOTERTEXT" in d.text(0)


def chk_page_counters():
    d = Doc(
        wrap(
            '<p>a</p><p style="page-break-before:always">b</p>',
            '@page{@bottom-center{content:"P" counter(page) "of" counter(pages)}}',
        )
    )
    assert "P2of2" in d.text(1).replace(" ", "")


def chk_page_selectors_margin_boxes():
    css = ('@page{@top-center{content:"ALL"}} @page :first{@top-center{content:none}}'
           '@page :left{@bottom-left{content:"EVEN" counter(page, upper-roman)}}')
    d = Doc(wrap('<p>a</p><p style="break-before:page">b</p>', css))
    assert "ALL" not in d.text(0) and "ALL" in d.text(1)
    assert "EVEN" not in d.text(0) and "EVENII" in d.text(1).replace(" ", "")


def chk_page_first_margins():
    css = "p{margin:0} @page{margin:20mm} @page :first{margin-top:80mm}"
    d = Doc(wrap('<p>COVER</p><p style="break-before:page">NEXT</p>', css))
    assert abs(d.word("COVER")[1] - 226.8) < 6, "first page starts 80mm down"
    assert abs(d.word("NEXT", 1)[1] - 56.7) < 6, "other pages keep the base margin"


def chk_page_left_right_margins():
    css = "p{margin:0} @page{margin:20mm} @page :left{margin-left:10mm} @page :right{margin-left:35mm}"
    d = Doc(wrap('<p>ONE</p><p style="break-before:page">TWO</p><p style="break-before:page">THREE</p>', css))
    one, two, three = d.word("ONE", 0), d.word("TWO", 1), d.word("THREE", 2)
    assert abs(one[0] - 99.2) < 3 and abs(three[0] - 99.2) < 3, "right pages: 35mm"
    assert abs(two[0] - 28.3) < 3, "left page: 10mm"


def chk_page_first_margin_fills_less():
    # A taller :first top margin leaves less room: the cover holds fewer lines.
    css = "@page{margin:20mm} @page :first{margin-top:150mm}"
    d = Doc(wrap("".join(f"<p style='margin:0'>L{i}</p>" for i in range(60)), css))
    first = sum(1 for w in d.words(0) if w[4].startswith("L"))
    second = sum(1 for w in d.words(1) if w[4].startswith("L"))
    assert first < second, (first, second)


def chk_custom_properties():
    d = Doc(wrap('<div style="background:var(--c);height:20px"></div>', ":root{--c:#00ff00}"))
    assert any(dr.get("fill") and tuple(round(c, 2) for c in dr["fill"]) == (0, 1, 0) for dr in d.drawings())


def chk_calc():
    d = Doc(wrap('<div style="width:calc(100px + 100px);height:10px;background:#f00"></div>'))
    widths = [dr["rect"].width for dr in d.drawings() if dr.get("fill")]
    assert any(abs(w - 150) < 1 for w in widths)  # 200px = 150pt


def chk_calc_mixed():
    d = Doc(wrap('<div style="--g:20px;width:calc(100% - 2 * var(--g));height:10px;background:#f00"></div>'))
    widths = [dr["rect"].width for dr in d.drawings() if dr.get("fill")]
    assert any(abs(w - (d.page_width() - 2 * 56.69 - 30)) < 1 for w in widths), widths


def chk_before_after_literal():
    d = Doc(wrap('<p class="x">mid</p>', '.x::before{content:"BEFORE"}.x::after{content:"AFTER"}'))
    assert "BEFOREmidAFTER" in d.text().replace(" ", "")


def chk_css_counters():
    d = Doc(wrap('<p class="n">a</p><p class="n">b</p>', '.n{counter-increment:k}.n::before{content:counter(k) "."}'))
    assert "2.b" in d.text().replace(" ", "")


def chk_section_numbering():
    css = ("body{counter-reset:s}h2{counter-reset:ss;counter-increment:s}h3{counter-increment:ss}"
           'h2::before{content:counter(s,upper-roman) ". "}h3::before{content:counter(s) "." counter(ss) " "}')
    d = Doc(wrap("<h2>Alpha</h2><h3>One</h3><h3>Two</h3><h2>Beta</h2><h3>Three</h3>", css))
    t = d.text().replace(" ", "")
    assert all(x in t for x in ("I.Alpha", "1.1One", "1.2Two", "II.Beta", "2.1Three")), t


def chk_nested_counters():
    css = 'ol{counter-reset:i;list-style:none}li{counter-increment:i}li::before{content:counters(i,".") " "}'
    d = Doc(wrap("<ol><li>a<ol><li>b</li><li>c</li></ol></li><li>d</li></ol>", css))
    t = d.text().replace(" ", "")
    assert all(x in t for x in ("1a", "1.1b", "1.2c", "2d")), t


def chk_pseudo_own_style():
    # The generated "TAG" takes its own colour and size; the element's own
    # "TAG" keeps the inherited ones. (Font size rather than weight: the
    # macOS runners have no bold fallback face.)
    d = Doc(wrap('<p class="x">TAG</p>', '.x::before{content:"TAG";color:#ff0000;font-size:20px;margin-right:4px}'))
    spans = [s for b in d.doc[0].get_text("dict")["blocks"] for l in b.get("lines", []) for s in l["spans"]]
    tags = [s for s in spans if s["text"].strip() == "TAG"]
    assert len(tags) == 2, spans
    gen, own = sorted(tags, key=lambda s: s["bbox"][0])
    assert gen["color"] == 0xFF0000 and own["color"] == 0, (gen, own)
    assert abs(gen["size"] - 15) < 0.1 and abs(own["size"] - 12) < 0.1, (gen["size"], own["size"])


def chk_content_attr_quotes():
    css = 'a::after{content:" (" attr(href) ")"}q::before{content:open-quote}q::after{content:close-quote}'
    d = Doc(wrap('<p><a href="https://x.dev">site</a> <q>hi</q></p>', css))
    t = d.text()
    assert "(https://x.dev)" in t and "\u201chi\u201d" in t, t


def chk_hidden_child_keeps_inline_flow():
    d = Doc(wrap('<p><span style="display:none">h</span><span style="display:inline-block">BTN</span> after</p>'))
    assert abs(d.word("BTN")[1] - d.word("after")[1]) < 6, "same line"


def chk_inline_block_shrink_wraps_mixed_content():
    d = Doc(wrap('<p><span style="display:inline-block;background:#00f"><b>&gt;</b> Pay online</span> x</p>'))
    assert abs(d.word("Pay")[1] - d.word("online")[1]) < 1, "no wrap inside the inline-block"


def chk_media_print():
    d = Doc(wrap("<p>x</p>", "@media print{p{color:#ff0000}}"))
    assert b"1 0 0 rg" in d.pdf


def chk_media_width():
    d = Doc(wrap('<p class="w">x</p>', "@media (min-width: 300px){.w{color:#ff0000}}"))
    assert b"1 0 0 rg" in d.pdf


def chk_lists():
    d = Doc(wrap('<ul><li>a</li></ul><ol><li>b</li></ol><ol style="list-style-type:upper-roman"><li>c</li><li>d</li></ol>'))
    t = d.text()
    assert "•" in t and "1." in t and "II." in t


def chk_text_align_right():
    d = Doc(wrap('<p style="text-align:right">R</p>'))
    assert d.word("R")[2] > d.page_width() - 80


def chk_text_align_center():
    d = Doc(wrap('<p style="text-align:center">C</p>'))
    assert abs((d.word("C")[0] + d.word("C")[2]) / 2 - d.page_width() / 2) < 10


def chk_justify():
    d = Doc(wrap(f'<p style="text-align:justify;width:300px">{LONG_TEXT[:400]}</p>'))
    lines: dict[int, list] = {}
    for w in d.words():
        lines.setdefault(round(w[1]), []).append(w)
    full = sorted(lines.values(), key=lambda ws: ws[0][1])[:-1]  # all but the last line
    rights = [max(w[2] for w in ws) for ws in full]
    assert len(rights) >= 2 and max(rights) - min(rights) < 1.5


def chk_whitespace_collapse():
    d = Doc(wrap("<p>alpha\n        beta   gamma\n\tdelta</p>"))
    ys = {round(w[1]) for w in d.words()}
    assert len(ys) == 1, "all four words should be on one line"


def chk_white_space_pre():
    d = Doc(wrap("<pre>one\ntwo</pre>"))
    assert d.word("one")[1] < d.word("two")[1]


def chk_nowrap():
    d = Doc(wrap('<p style="white-space:nowrap;width:40px">nowrap nowrap</p>'))
    assert "nowrap nowrap" in d.text() and len({round(w[1]) for w in d.words()}) == 1


def chk_style_in_body_hidden():
    d = Doc("<html><body><style>p{color:red}</style><p>hello</p></body></html>")
    assert d.text() == "hello"


def chk_letter_spacing():
    a = Doc(wrap("<p>WIDE</p>")).word("WIDE")
    b = Doc(wrap('<p style="letter-spacing:10px">WIDE</p>')).word("WIDE")
    assert (b[2] - b[0]) - (a[2] - a[0]) > 20


def chk_text_transform():
    d = Doc(wrap('<p style="text-transform:uppercase">shout</p>'))
    assert "SHOUT" in d.text()


def chk_text_indent():
    d = Doc(wrap('<p style="text-indent:50px">Indented</p><p>Normal</p>'))
    assert d.word("Indented")[0] - d.word("Normal")[0] > 30


def chk_font_weight_style():
    d = Doc(wrap("<p><b>bold</b> <i>ital</i></p>"))
    fonts = " ".join(f[3] for f in d.doc[0].get_fonts())
    assert "Bold" in fonts and ("Oblique" in fonts or "Italic" in fonts)


def chk_flex_row():
    d = Doc(wrap('<div style="display:flex;gap:40px"><div>LEFT</div><div>RIGHT</div></div>'))
    left, right = d.word("LEFT"), d.word("RIGHT")
    assert abs(left[1] - right[1]) < 1 and right[0] > left[2]


def chk_flex_inline_items():
    d = Doc(wrap('<div style="display:flex;justify-content:space-between"><span>LEFTI</span><span>RIGHTI</span></div>'))
    assert d.word("RIGHTI")[2] > d.page_width() - 70, "spans in a flex container are separate items"


def chk_flex_justify_between():
    d = Doc(wrap('<div style="display:flex;justify-content:space-between"><div>L</div><div>R</div></div>'))
    assert d.word("R")[2] > d.page_width() - 80


def chk_grid_columns():
    d = Doc(wrap('<div style="display:grid;grid-template-columns:1fr 1fr"><div>G1</div><div>G2</div></div>'))
    assert abs(d.word("G1")[1] - d.word("G2")[1]) < 1


def _fill_rects(d: "Doc"):
    return [dr["rect"] for dr in d.drawings() if dr.get("fill")]


def chk_grid_placement():
    d = Doc(wrap(
        '<div style="display:grid;grid-template-columns:repeat(3,1fr);gap:5px 10px">\n'
        '  <div style="grid-column:span 2;height:10px;background:#f00"></div>\n'
        '  <div style="height:10px;background:#0f0"></div>\n'
        '  <div style="grid-area:2 / 3;height:10px;background:#00f"></div>\n</div>'
    ))
    red, green, blue = _fill_rects(d)
    assert abs(red.x0 - 56.7) < 1 and abs(green.y0 - red.y0) < 1 and green.x0 > red.x1
    assert blue.y0 > red.y1 and abs(blue.x0 - green.x0) < 1, "explicit placement in row 2, column 3"


def chk_grid_auto_fill():
    items = "".join('<div style="height:10px;background:#f00"></div>' for _ in range(6))
    d = Doc(wrap(f'<div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(120px,1fr))">{items}</div>'))
    rects = _fill_rects(d)
    assert len({round(r.y0) for r in rects}) == 2, "5 columns of ≥120px fit in the content width"


def chk_flex_whitespace_gap():
    d = Doc(wrap('<div style="display:flex;gap:20px">\n  <div>A1</div>\n  <div>B1</div>\n</div>'))
    assert abs(d.word("B1")[0] - d.word("A1")[2] - 15) < 1.5, "one 20px gap, no extra item for white space"


def chk_inline_block_shrink():
    d = Doc(wrap('<div><div>Block</div><span style="display:inline-block;background:#00f">IB</span> tail</div>'))
    assert abs(d.word("IB")[1] - d.word("tail")[1]) < 1


def chk_inline_block_padding():
    d = Doc(wrap('<p>x <span style="display:inline-block;padding:10px;background:#00f">BTN</span> y</p>'))
    box = next(dr["rect"] for dr in d.drawings() if dr.get("fill"))
    btn = d.word("BTN")
    assert btn[0] >= box.x0 + 5 and btn[2] <= box.x1 - 5, "label must sit inside the padding box"
    assert d.word("y")[0] >= box.x1


def chk_author_overrides_tag_defaults():
    d = Doc(wrap('<p><a href="#" style="text-decoration:none">link</a> <b style="font-weight:normal">plain</b>'
                 ' <span style="display:block">BLOCK</span></p>'))
    assert not any(dr.get("color") for dr in d.drawings()), "no underline"
    embedded = [f[3] for f in d.doc[0].get_fonts() if not f[3].startswith("Helvetica")]  # base-14 always declared
    assert not any("Bold" in name for name in embedded), embedded
    assert d.word("BLOCK")[1] > d.word("link")[1]


def chk_position_absolute():
    d = Doc(wrap('<div style="position:relative;height:100px"><div style="position:absolute;top:50px;left:200px">ABS</div></div>'))
    assert d.word("ABS")[0] > 150


def chk_table_layout():
    d = Doc(wrap('<table><tr><td>c1</td><td>c2</td></tr><tr><td colspan="2">wide</td></tr></table>'))
    assert d.word("c2")[0] > d.word("c1")[2] and d.word("wide")[1] > d.word("c1")[1]


def chk_box_sizing():
    d = Doc(wrap('<div style="width:200px;padding:20px;height:10px;background:#f00"></div>'
                 '<div style="box-sizing:border-box;width:200px;padding:20px;height:50px;background:#0f0"></div>'))
    widths = {tuple(round(c, 1) for c in dr["fill"]): dr["rect"].width for dr in d.drawings() if dr.get("fill")}
    assert abs(widths[(1.0, 0.0, 0.0)] - 180) < 1, "content-box: 200px + 2×20px = 240px = 180pt"
    assert abs(widths[(0.0, 1.0, 0.0)] - 150) < 1, "border-box: 200px = 150pt"


def chk_border_double():
    d = Doc(wrap('<div style="border-top:6px double #f00;height:20px"></div>'))
    assert sum(1 for dr in d.drawings() if dr.get("color")) >= 2


def chk_border_solid():
    d = Doc(wrap('<div style="border:2px solid #f00;height:20px"></div>'))
    assert any(dr.get("color") for dr in d.drawings())


def chk_border_dashed():
    d = Doc(wrap('<div style="border:2px dashed #f00;height:20px"></div>'))
    assert any(dr.get("dashes") and dr["dashes"] not in ("[] 0", "[] 0.0") for dr in d.drawings())


def chk_border_radius():
    d = Doc(wrap('<div style="width:100px;height:40px;background:#00f;border-radius:20px"></div>'))
    assert any(item[0] == "c" for dr in d.drawings() for item in dr["items"])


def chk_border_radius_percent():
    # 200x40px box at (57, 57)pt = 150x30pt: 50% makes an ellipse.
    d = Doc(wrap('<div style="width:200px;height:40px;background:#f00;border-radius:50%"></div>'))
    pix = d.doc[0].get_pixmap(dpi=72)
    assert pix.pixel(57 + 75, 57 + 15) == (255, 0, 0), "centre painted"
    assert pix.pixel(57 + 3, 57 + 3) == (255, 255, 255), "corner cut by the ellipse"
    # A 15pt pill would cover (77, 58); the 75x15pt ellipse does not.
    assert pix.pixel(57 + 20, 57 + 1) == (255, 255, 255), "elliptical, not a pill"


def chk_border_radius_per_corner():
    d = Doc(wrap('<div style="width:200px;height:40px;background:#f00;border-radius:0 0 20px 0 / 0 0 10px 0;'
                 'border-top-left-radius:1rem"></div>'))
    pix = d.doc[0].get_pixmap(dpi=72)
    assert pix.pixel(57 + 1, 57 + 1) == (255, 255, 255), "top-left rounded (rem longhand)"
    assert pix.pixel(57 + 148, 57 + 1) == (255, 0, 0), "top-right square"
    assert pix.pixel(57 + 149, 57 + 28) == (255, 255, 255), "bottom-right rounded"
    assert pix.pixel(57 + 1, 57 + 28) == (255, 0, 0), "bottom-left square"


def chk_box_shadow():
    d = Doc(wrap('<div style="width:100px;height:40px;box-shadow:4px 4px 0 #000"></div>'))
    assert len(d.drawings()) >= 1


def chk_box_shadow_outside_only():
    d = Doc(wrap('<div style="margin:40px;width:200px;height:100px;box-shadow:0 2px 6px rgba(0,0,0,.5)"></div>'))
    pix = d.doc[0].get_pixmap(dpi=72)
    # Centre of the (transparent) box must stay white; the shadow is outside.
    assert pix.pixel(57 + 30 + 75, 57 + 30 + 37) == (255, 255, 255)


def chk_opacity():
    d = Doc(wrap('<div style="opacity:.5;background:#f00;height:20px"></div>'))
    assert b"/ca" in d.pdf or b"/CA" in d.pdf


def _box_pixmap(style: str):
    """A 200x40px box at (57pt, 57pt) rendered at 72 dpi (1px = 1pt)."""
    d = Doc(wrap(f'<div style="width:200px;height:40px;{style}"></div>'))
    return d.doc[0].get_pixmap(dpi=72)


def _near(px, rgb, tol=40):
    return all(abs(a - b) <= tol for a, b in zip(px, rgb))


def chk_gradient():
    pix = _box_pixmap("background:linear-gradient(to right,#f00,#00f)")
    assert _near(pix.pixel(57 + 2, 57 + 15), (255, 0, 0)), "starts red"
    assert _near(pix.pixel(57 + 148, 57 + 15), (0, 0, 255)), "ends blue"
    assert _near(pix.pixel(57 + 75, 57 + 15), (128, 0, 128)), "purple halfway"
    pix = _box_pixmap("background:linear-gradient(0deg,#000,#fff)")
    assert pix.pixel(57 + 75, 57 + 28)[0] < 40 and pix.pixel(57 + 75, 57 + 2)[0] > 215, "0deg points up"


def chk_radial_gradient():
    pix = _box_pixmap("background:radial-gradient(circle closest-side,#0f0,#fff)")
    assert _near(pix.pixel(57 + 75, 57 + 15), (0, 255, 0)), "centre colour"
    assert pix.pixel(57 + 2, 57 + 2) == (255, 255, 255), "last colour outside the circle"


def chk_repeating_gradient():
    pix = _box_pixmap("background:repeating-linear-gradient(to right,#000 0 10px,#fff 10px 20px)")
    # 10px = 7.5pt per band: black, white, black, white.
    row = [pix.pixel(57 + x, 57 + 15)[0] for x in (3, 11, 18, 26)]
    assert row[0] < 60 and row[1] > 200 and row[2] < 60 and row[3] > 200, row


def chk_gradient_alpha():
    pix = _box_pixmap("background:#f00 linear-gradient(to bottom,transparent,#000)")
    assert _near(pix.pixel(57 + 75, 57 + 1), (255, 0, 0)), "transparent top shows the colour"
    assert pix.pixel(57 + 75, 57 + 28)[0] < 40, "opaque bottom"


# The 200x40px test box spans x 57..207pt, y 57..87pt; its centre is (132, 72).
RED = (255, 0, 0)
WHITE = (255, 255, 255)


def chk_transform():
    pix = _box_pixmap("background:#f00;transform:rotate(90deg)")
    assert pix.pixel(132, 72 + 60) == RED, "now extends below the box"
    assert pix.pixel(57 + 5, 72) == WHITE, "left end rotated away"


def chk_transform_translate_percent():
    pix = _box_pixmap("background:#f00;transform:translate(50%, 0)")
    assert pix.pixel(57 + 5, 72) == WHITE and pix.pixel(207 + 70, 72) == RED


def chk_transform_scale_origin():
    pix = _box_pixmap("background:#f00;transform:scale(.5);transform-origin:top left")
    assert pix.pixel(57 + 70, 57 + 5) == RED, "kept at the top-left origin"
    assert pix.pixel(57 + 100, 57 + 5) == WHITE and pix.pixel(57 + 70, 57 + 22) == WHITE, "halved"


def chk_transform_individual():
    pix = _box_pixmap("background:#f00;rotate:90deg")
    assert pix.pixel(132, 72 + 60) == RED and pix.pixel(57 + 5, 72) == WHITE


def chk_transform_text():
    d = Doc(wrap('<p style="transform:translateX(100px)">MOVED</p><p>STAY</p>'))
    assert abs(d.word("MOVED")[0] - d.word("STAY")[0] - 75) < 1, "text follows the transform"


def chk_float_header_and_clear():
    d = Doc(wrap('<div><div style="float:left">LOGOF</div>\n<div style="float:right">FIRSTR</div>\n'
                 '<div style="float:right">SECONDR</div></div><h2 style="clear:both">TITLEF</h2>'))
    logo, first, second, title = (d.word(w) for w in ("LOGOF", "FIRSTR", "SECONDR", "TITLEF"))
    assert abs(logo[1] - first[1]) < 1 and abs(first[1] - second[1]) < 1, "floats share the line"
    assert first[2] > d.page_width() - 70 and second[2] < first[0], "first right float is rightmost"
    assert title[1] > logo[3], "clear: both goes below the floats"


def chk_overflow_hidden():
    d = Doc(wrap('<div style="width:60px;height:14px;overflow:hidden">' + "clip " * 40 + "</div>"))
    pix = d.doc[0].get_pixmap(dpi=72)
    # Below the 14px (10.5pt) box nothing is painted.
    assert all(pix.pixel(x, 57 + 20) == (255, 255, 255) for x in range(57, 105))


def chk_overflow_rounded_clip():
    d = Doc(wrap('<div style="width:60px;height:60px;border-radius:30px;overflow:hidden">'
                 '<div style="width:200px;height:200px;background:#0f0"></div></div>'))
    pix = d.doc[0].get_pixmap(dpi=72)
    assert pix.pixel(57 + 22, 57 + 22) == (0, 255, 0), "centre painted"
    assert pix.pixel(57 + 1, 57 + 1) == (255, 255, 255), "corner clipped by the radius"
    assert pix.pixel(57 + 60, 57 + 22) == (255, 255, 255), "outside the box clipped"


def chk_float():
    d = Doc(wrap('<div style="width:400px"><div style="float:right;width:100px">FLT</div><p style="margin:0">Wrap</p></div>'))
    assert abs(d.word("FLT")[1] - d.word("Wrap")[1]) < 2


def chk_png_image():
    d = Doc(wrap(f'<img src="{_png_data_uri()}" width="20" height="20">'))
    assert len(d.doc[0].get_images()) == 1


def _img_boxes(html: str):
    d = Doc(wrap(html))
    return [tuple(round(v) for v in i["bbox"]) for i in d.doc[0].get_image_info()]


def chk_img_size_attributes():
    (x0, y0, x1, y1), = _img_boxes(f'<img src="{_png_data_uri(4, 4)}" width="80" height="40">')
    assert (x1 - x0, y1 - y0) == (60, 30)  # 80×40 px


def chk_img_intrinsic_size_and_ratio():
    uri = _png_data_uri(40, 20)
    (a, b, c, e), = _img_boxes(f'<img src="{uri}">')
    assert (c - a, e - b) == (30, 15), "intrinsic 40×20 px"
    (a, b, c, e), = _img_boxes(f'<img src="{uri}" width="80">')
    assert (c - a, e - b) == (60, 30), "height from intrinsic ratio"


def chk_svg_image():
    d = Doc(wrap(f'<img src="{SVG_URI}" width="40" height="40">'))
    assert not d.doc[0].get_images() and len(d.drawings()) >= 1, "SVG must be embedded as vectors"


def chk_png_transparency():
    uri = _png_data_uri(10, 10, rgba=b"\x00\x00\x00\x00")
    d = Doc(wrap(f'<div style="background:#0000ff;padding:20px"><img src="{uri}" width="100" height="100"></div>'))
    pix = d.doc[0].get_pixmap(dpi=72)
    # Centre of the fully transparent image shows the blue background, not black.
    assert pix.pixel(57 + 15 + 37, 57 + 15 + 37) == (0, 0, 255)


def chk_inline_svg():
    d = Doc(wrap('<p>x</p><svg width="40" height="40"><rect width="40" height="40" fill="#f00"/></svg>'))
    rects = [dr["rect"] for dr in d.drawings() if dr.get("fill") == (1.0, 0.0, 0.0)]
    assert rects and abs(rects[0].width - 30) < 1, "40px inline svg drawn as a 30pt vector rect"


def chk_rtl_flex_mirrored():
    d = Doc(wrap('<div dir="rtl" style="display:flex;gap:10px"><span>FIRST</span><span>SECOND</span></div>'))
    first, second = d.word("FIRST"), d.word("SECOND")
    assert first[0] > second[0], "first item on the right"
    assert first[2] > d.page_width() - 70, "packed against the right edge"


def chk_rtl_table_mirrored():
    d = Doc(wrap('<table dir="rtl" style="width:100%"><tr><td>CELLA</td><td>CELLB</td></tr></table>'))
    assert d.word("CELLA")[0] > d.word("CELLB")[0], "first column on the right"


def chk_text_align_start_rtl():
    d = Doc(wrap('<div dir="rtl"><p>LATIN</p></div>'))
    assert d.word("LATIN")[2] > d.page_width() - 70, "start = right in RTL, even for Latin text"


def chk_rtl_arabic():
    d = Doc(wrap('<p dir="rtl">مرحبا بالعالم</p>'))
    assert d.text() != "" and d.words()[0][0] > d.page_width() / 2


FONTS = __import__("pathlib").Path(__file__).parent / "fixtures" / "fonts"


def _font_face(family: str, file: str) -> str:
    data = base64.b64encode((FONTS / file).read_bytes()).decode()
    return f"@font-face{{font-family:'{family}';src:url(data:font/otf;base64,{data})}}"


def _embedded_fonts(d: Doc):
    """[(base name without subset tag, extension)] of the embedded fonts."""
    return [(f[3].split("+")[-1], f[1]) for f in d.doc[0].get_fonts() if f[1] != "n/a"]


def chk_font_collection_face():
    # The second face of a .ttc: the right face is embedded and its text
    # extracts (macOS ships its Arabic and CJK fonts as collections).
    css = _font_face("Ferro TTC Second", "ferro-collection.ttc")
    d = Doc(wrap("<p style=\"font-family:'Ferro TTC Second'\">Collection face</p>", css))
    assert "Collection face" in d.text(), d.text()
    assert ("FerroTTCSecond-Regular", "ttf") in _embedded_fonts(d), _embedded_fonts(d)


def chk_font_cff_embedding():
    css = _font_face("Ferro CFF", "ferro-cff.otf")
    d = Doc(wrap("<p style=\"font-family:'Ferro CFF'\">Compact outlines</p>", css))
    assert "Compact outlines" in d.text(), d.text()
    assert ("FerroCFF-Regular", "otf") in _embedded_fonts(d), "embedded as OpenType/CFF (FontFile3)"


def chk_ligature_text_extraction():
    css = _font_face("Ferro CFF", "ferro-cff.otf")
    d = Doc(wrap("<p style=\"font-family:'Ferro CFF'\">office affluent</p>", css))
    assert "office affluent" in d.text(), d.text()


def chk_subset_font_tag():
    d = Doc(wrap("<p>tagged</p>"))
    names = [f[3] for f in d.doc[0].get_fonts() if f[1] != "n/a"]
    assert names and all(len(n.split("+")[0]) == 6 and "+" in n for n in names), names


def chk_links():
    d = Doc(wrap('<a href="https://example.com">link</a>'))
    assert any(l.get("uri") == "https://example.com" for l in d.doc[0].get_links())


def chk_outline():
    d = Doc(wrap("<h1>One</h1><h2>Two</h2>"))
    assert len(d.doc.get_toc()) >= 2


def chk_metadata():
    d = Doc(wrap("<p>x</p>"), title="T1", author="A1")
    assert d.doc.metadata["title"] == "T1" and d.doc.metadata["author"] == "A1"


# --------------------------------------------------------------------------- matrix


@dataclass(frozen=True)
class Feature:
    category: str
    name: str
    check: Callable[[], None]
    issue: int | None = None  # open issue => expected to fail (strict xfail)
    needs_fonts: bool = False  # result depends on installed fonts (see HAS_DEJAVU)


FEATURES = [
    Feature("Page", "`@page` size", chk_page_size),
    Feature("Page", "`@page` size with `landscape`", chk_page_landscape),
    Feature("Page", "`@page` margin", chk_page_margin),
    Feature("Page", "`@page` margin shorthand (2–4 values)", chk_page_margin_shorthand),
    Feature("Page", "`Options(page_size=...)`", chk_options_page_size),
    Feature("Page", "`@page` margin boxes (headers / footers)", chk_margin_boxes),
    Feature("Page", "`counter(page)` / `counter(pages)`", chk_page_counters),
    Feature("Page", "`@page :first` margins (cover page)", chk_page_first_margins),
    Feature("Page", "`@page :left` / `:right` margins (binding)", chk_page_left_right_margins),
    Feature("Page", "Per-page margins change how much fits on a page", chk_page_first_margin_fills_less),
    Feature("Page", "HTML headers/footers: `position: running()` + `element()`", chk_running_element),
    Feature("Page", "`@page :first / :left / :right` margin boxes", chk_page_selectors_margin_boxes),
    Feature("Fragmentation", "Automatic pagination of many blocks", chk_many_blocks_paginate),
    Feature("Fragmentation", "`page-break-before: always` (legacy)", chk_legacy_page_break),
    Feature("Fragmentation", "`break-before: page`", chk_break_before),
    Feature("Fragmentation", "`break-after: page`", chk_break_after),
    Feature("Fragmentation", "Paragraph taller than a page splits across pages", chk_long_text_splits),
    Feature("Fragmentation", "`<thead>` repeated on each page", chk_thead_repeat),
    Feature("Fragmentation", "Multi-row `<thead>` repeated without overlap", chk_multi_row_thead),
    Feature("Fragmentation", "`<tfoot>` repeated at the bottom of each page", chk_tfoot_repeat),
    Feature("Cascade", "Custom properties `var()`", chk_custom_properties),
    Feature("Cascade", "`calc()`", chk_calc),
    Feature("Cascade", "`calc()` mixing `%` and lengths, with `var()`", chk_calc_mixed),
    Feature("Cascade", "`::before` / `::after` with literal `content`", chk_before_after_literal),
    Feature("Cascade", "Author CSS overrides tag defaults (`a`, `b`, `span`)", chk_author_overrides_tag_defaults),
    Feature("Cascade", "CSS counters in `content`", chk_css_counters),
    Feature("Cascade", "Section numbering (`counter-reset` scopes, counter styles)", chk_section_numbering),
    Feature("Cascade", "Nested counters with `counters()`", chk_nested_counters),
    Feature("Cascade", "`::before` / `::after` take their own styles", chk_pseudo_own_style),
    Feature("Cascade", "`attr()` and quotes in `content`", chk_content_attr_quotes),
    Feature("Cascade", "`@media print`", chk_media_print),
    Feature("Cascade", "Width media queries (`min-width`)", chk_media_width, 29),
    Feature("Cascade", "`<style>` inside `<body>` is not rendered", chk_style_in_body_hidden),
    Feature("Text", "Bold / italic", chk_font_weight_style),
    Feature("Text", "`text-align: right`", chk_text_align_right),
    Feature("Text", "`text-align: center`", chk_text_align_center),
    Feature("Text", "`text-align: justify`", chk_justify),
    Feature("Text", "`text-align: start` follows `direction`", chk_text_align_start_rtl),
    Feature("Text", "Whitespace collapsing (`white-space: normal`)", chk_whitespace_collapse),
    Feature("Text", "`white-space: pre`", chk_white_space_pre),
    Feature("Text", "`white-space: nowrap`", chk_nowrap),
    Feature("Text", "`letter-spacing`", chk_letter_spacing, 34),
    Feature("Text", "`text-transform`", chk_text_transform, 34),
    Feature("Text", "`text-indent`", chk_text_indent, 34),
    Feature("Text", "Arabic shaping + RTL alignment", chk_rtl_arabic),
    Feature("Text", "Lists (disc, decimal, roman)", chk_lists),
    Feature("Layout", "Flexbox row", chk_flex_row),
    Feature("Layout", "`justify-content: space-between`", chk_flex_justify_between),
    Feature("Layout", "`direction: rtl` mirrors flex rows", chk_rtl_flex_mirrored),
    Feature("Layout", "Inline elements as flex items", chk_flex_inline_items),
    Feature("Layout", "CSS grid columns", chk_grid_columns),
    Feature("Layout", "Grid placement (`span`, `grid-area`, row/column gaps)", chk_grid_placement),
    Feature("Layout", "Grid `repeat(auto-fill, minmax())`", chk_grid_auto_fill),
    Feature("Layout", "White space between flex items ignored", chk_flex_whitespace_gap),
    Feature("Layout", "`inline-block` shrink-to-fit next to text", chk_inline_block_shrink),
    Feature("Layout", "`inline-block` padding around its label", chk_inline_block_padding),
    Feature("Layout", "Tables with `colspan`", chk_table_layout),
    Feature("Layout", "`direction: rtl` mirrors table / grid columns", chk_rtl_table_mirrored),
    Feature("Layout", "`position: absolute`", chk_position_absolute),
    Feature("Layout", "`float` (text beside a floated box)", chk_float),
    Feature("Layout", "`float` left/right header + `clear`", chk_float_header_and_clear),
    Feature("Layout", "`overflow: hidden`", chk_overflow_hidden),
    Feature("Layout", "Hidden child keeps the inline flow", chk_hidden_child_keeps_inline_flow),
    Feature("Layout", "`inline-block` shrink-wraps mixed inline content", chk_inline_block_shrink_wraps_mixed_content),
    Feature("Visual", "`overflow: hidden` with `border-radius` (round clip)", chk_overflow_rounded_clip),
    Feature("Layout", "`box-sizing` (content-box default, border-box)", chk_box_sizing),
    Feature("Visual", "Solid borders", chk_border_solid),
    Feature("Visual", "Dashed / dotted borders", chk_border_dashed),
    Feature("Visual", "Double borders", chk_border_double),
    Feature("Visual", "`border-radius`", chk_border_radius),
    Feature("Visual", "`border-radius` percentages (circles, ellipses)", chk_border_radius_percent),
    Feature("Visual", "Per-corner and elliptical `border-radius`", chk_border_radius_per_corner),
    Feature("Visual", "`box-shadow`", chk_box_shadow),
    Feature("Visual", "`box-shadow` not painted under the box", chk_box_shadow_outside_only),
    Feature("Visual", "`opacity`", chk_opacity),
    Feature("Visual", "Linear gradients (angles, `to <side>` / `to <corner>`)", chk_gradient),
    Feature("Visual", "Radial gradients", chk_radial_gradient),
    Feature("Visual", "Repeating gradients", chk_repeating_gradient),
    Feature("Visual", "Translucent gradient stops over `background-color`", chk_gradient_alpha),
    Feature("Visual", "`transform: rotate()`", chk_transform),
    Feature("Visual", "`transform: translate()` with percentages", chk_transform_translate_percent),
    Feature("Visual", "`transform: scale()` with `transform-origin`", chk_transform_scale_origin),
    Feature("Visual", "Individual `translate` / `rotate` / `scale` properties", chk_transform_individual),
    Feature("Visual", "Transformed text stays selectable", chk_transform_text),
    Feature("Images", "PNG / JPEG `<img>`", chk_png_image),
    Feature("Images", "`<img width height>` attributes", chk_img_size_attributes),
    Feature("Images", "Intrinsic image size and aspect ratio", chk_img_intrinsic_size_and_ratio),
    Feature("Images", "PNG transparency (alpha)", chk_png_transparency),
    Feature("Images", "SVG `<img>` (vector)", chk_svg_image),
    Feature("Images", "Inline `<svg>` (vector)", chk_inline_svg),
    Feature("PDF", "Title / author metadata", chk_metadata),
    Feature("PDF", "Fonts from a `.ttc` collection (any face)", chk_font_collection_face),
    Feature("PDF", "CFF / `.otf` fonts embedded as OpenType (FontFile3)", chk_font_cff_embedding),
    Feature("PDF", "Ligatures extract as their letters", chk_ligature_text_extraction),
    Feature("PDF", "Subset fonts carry a subset tag (`ABCDEF+Name`)", chk_subset_font_tag),
    Feature("PDF", "Clickable links", chk_links, 40),
    Feature("PDF", "Outline / bookmarks", chk_outline, 41),
]


@pytest.mark.parametrize(
    "feature",
    [
        pytest.param(
            f,
            id=f.check.__name__.removeprefix("chk_") if hasattr(str, "removeprefix") else f.check.__name__[4:],
            marks=([pytest.mark.xfail(strict=True, reason=f"not supported yet — {ISSUES}{f.issue}")] if f.issue else [])
            + ([pytest.mark.skipif(not HAS_DEJAVU, reason="font-dependent: needs DejaVu fonts")] if f.needs_fonts else []),
        )
        for f in FEATURES
    ],
)
def test_feature(feature: Feature):
    feature.check()


def markdown_table() -> str:
    lines = ["| Category | Feature | Status |", "| --- | --- | --- |"]
    for f in FEATURES:
        try:
            f.check()
            ok = True
        except Exception:  # noqa: BLE001
            ok = False
        if ok:
            status = "✅ Supported"
        elif f.issue:
            status = f"❌ Not yet ([#{f.issue}]({ISSUES}{f.issue}))"
        else:
            status = "⚠️ Broken"
        lines.append(f"| {f.category} | {f.name} | {status} |")
    return "\n".join(lines)


@pytest.mark.skipif(not HAS_DEJAVU, reason="README table is generated on the reference platform (DejaVu fonts)")
def test_readme_table_in_sync():
    """The README "CSS support" table must match the matrix above."""
    from pathlib import Path

    readme = (Path(__file__).resolve().parent.parent / "README.md").read_text(encoding="utf-8")
    start, end = "<!-- feature-matrix:start -->", "<!-- feature-matrix:end -->"
    current = readme.split(start, 1)[1].split(end, 1)[0].strip()
    assert current == markdown_table(), (
        "README feature table is stale — regenerate it with `python tests/test_feature_matrix.py` "
        "and paste the output between the feature-matrix markers"
    )


if __name__ == "__main__":
    sys.stdout.write(markdown_table() + "\n")
