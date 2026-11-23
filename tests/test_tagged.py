"""Tagged PDF and PDF/UA-1 (issue #43).

Structure is checked here; conformance is checked by veraPDF (``--flavour
ua1``) in CI on the files written by tests/pdfa/make_corpus.py.
"""

from __future__ import annotations

import io
import re
import warnings

import pytest
from pypdf import PdfReader
from pypdf.generic import ArrayObject, DictionaryObject

import ferropdf

PIXEL = (
    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8"
    "z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg=="
)

DOC = f"""<!doctype html><html lang="fr"><head><title>Rapport annuel</title>
<style>@page {{ @bottom-center {{ content: "Page " counter(page) }} }}
.card {{ background: #eef; border: 1px solid #99c; }}</style></head><body>
<h1>Rapport</h1>
<p>Texte avec un <a href="https://example.com">lien</a> et du <b>gras</b>.</p>
<div class="card"><h2>Liste</h2><ul><li>Un</li><li>Deux</li></ul></div>
<table><thead><tr><th>Nom</th><th>Total</th></tr></thead>
<tbody><tr><th>A</th><td>1</td></tr></tbody></table>
<img alt="Logo rouge" src="{PIXEL}" width="20" height="20">
<img alt="" src="{PIXEL}" width="5" height="5">
<svg width="20" height="20" role="img" aria-label="Carré bleu"><rect width="20" height="20" fill="blue"/></svg>
<blockquote lang="en">Quoted.</blockquote>
</body></html>"""


def render(html: str = DOC, **kw) -> bytes:
    return ferropdf.Engine(ferropdf.Options(**kw)).render(html)


def root(pdf: bytes):
    return PdfReader(io.BytesIO(pdf)).trailer["/Root"]


def tree(pdf: bytes) -> list:
    """Structure tree as nested lists: [role, attrs, children...]."""

    def walk(e):
        e = e.get_object()
        if not isinstance(e, DictionaryObject):
            return None  # bare MCID
        t = e.get("/Type")
        if t == "/MCR":
            return "mcr"
        if t == "/OBJR":
            return "objr"
        attrs = {}
        if "/Alt" in e:
            attrs["alt"] = str(e["/Alt"])
        if "/Lang" in e:
            attrs["lang"] = str(e["/Lang"])
        if "/A" in e:
            attrs["scope"] = str(e["/A"].get_object()["/Scope"])
        kids = e.get("/K", [])
        kids = kids if isinstance(kids, ArrayObject) else [kids]
        return [str(e["/S"])[1:], attrs] + [k for k in map(walk, kids) if k is not None]

    return walk(root(pdf)["/StructTreeRoot"]["/K"])


def roles(node) -> list:
    """Element roles in document order (pre-order)."""
    if not isinstance(node, list):
        return []
    out = [node[0]]
    for k in node[2:]:
        out += roles(k)
    return out


def find(node, role):
    if not isinstance(node, list):
        return []
    hits = [node] if node[0] == role else []
    for k in node[2:]:
        hits += find(k, role)
    return hits


def test_plain_pdf_has_no_structure():
    r = root(render())
    for key in ("/StructTreeRoot", "/MarkInfo", "/Lang"):
        assert key not in r


def test_structure_follows_the_html():
    t = tree(render(tagged=True))
    assert t[0] == "Document"
    assert roles(t) == [
        "Document", "H1", "P", "Link", "Div", "H2", "L", "LI", "Lbl", "LBody",
        "LI", "Lbl", "LBody", "Table", "THead", "TR", "TH", "TH", "TBody", "TR",
        "TH", "TD", "Figure", "Figure", "BlockQuote",
    ]
    assert [th[1]["scope"] for th in find(t, "TH")] == ["/Column", "/Column", "/Row"]
    assert [f[1]["alt"] for f in find(t, "Figure")] == ["Logo rouge", "Carré bleu"]
    assert find(t, "BlockQuote")[0][1]["lang"] == "en"
    (link,) = find(t, "Link")
    assert link[2:] == ["mcr", "objr"]


def test_catalog_and_pages():
    pdf = render(tagged=True)
    r = root(pdf)
    assert r["/MarkInfo"]["/Marked"] == True  # noqa: E712 (pypdf BooleanObject)
    assert r["/Lang"] == "fr"
    assert r["/ViewerPreferences"]["/DisplayDocTitle"] == True  # noqa: E712
    reader = PdfReader(io.BytesIO(pdf))
    assert reader.metadata["/Title"] == "Rapport annuel"  # from <title>
    page = reader.pages[0]
    assert page["/StructParents"] == 0
    assert page["/Tabs"] == "/S"
    annot = page["/Annots"][0].get_object()
    assert annot["/Contents"] == "lien"
    assert "/StructParent" in annot


def test_every_content_item_is_marked_and_in_the_parent_tree():
    pdf = render(tagged=True)
    reader = PdfReader(io.BytesIO(pdf))
    nums = reader.trailer["/Root"]["/StructTreeRoot"]["/ParentTree"]["/Nums"]
    parents = dict(zip(nums[::2], nums[1::2]))
    for i, page in enumerate(reader.pages):
        data = page.get_contents().get_data()
        opened = len(re.findall(rb"\bBDC\b", data)) + len(re.findall(rb"\bBMC\b", data))
        assert opened == len(re.findall(rb"\bEMC\b", data)) > 0
        mcids = [int(m) for m in re.findall(rb"/MCID (\d+)", data)]
        assert mcids == list(range(len(mcids))), "MCIDs are 0..n per page"
        assert len(parents[i].get_object()) == len(mcids)
        # No glyph or image is drawn outside a marked-content sequence.
        depth = 0
        for tok in re.findall(rb"\b(BDC|BMC|EMC|TJ|Tj|Do)\b", data):
            if tok in (b"BDC", b"BMC"):
                depth += 1
            elif tok == b"EMC":
                depth -= 1
            else:
                assert depth == 1, f"{tok!r} outside marked content on page {i}"


def test_decorations_and_page_furniture_are_artifacts():
    data = PdfReader(io.BytesIO(render(tagged=True))).pages[0].get_contents().get_data()
    assert b"/Artifact BMC" in data  # backgrounds, borders
    assert re.search(rb"/Artifact <<\s*/Type /Pagination\s*>> BDC", data)  # @bottom-center


def test_decorative_images_are_left_out():
    t = tree(render(tagged=True))
    assert len(find(t, "Figure")) == 2  # alt="" is an artifact


def test_text_and_rendering_are_unchanged():
    plain, tagged = render(), render(tagged=True)
    text = lambda pdf: [p.extract_text() for p in PdfReader(io.BytesIO(pdf)).pages]  # noqa: E731
    assert text(plain) == text(tagged)
    engine_png = lambda **kw: ferropdf.Engine(ferropdf.Options(**kw)).render_png(DOC, dpi=48)  # noqa: E731
    assert engine_png() == engine_png(tagged=True)


def test_pdf_ua_metadata():
    pdf = render(pdf_ua=True)
    meta = root(pdf)["/Metadata"].get_object().get_data().decode()
    assert "<pdfuaid:part>1</pdfuaid:part>" in meta
    assert "Rapport annuel" in meta  # dc:title
    assert "pdfaid" not in meta and "pdfaExtension" not in meta


def test_pdf_ua_with_pdf_a():
    meta = root(render(pdf_ua=True, pdf_variant="pdf/a-3b")).get("/Metadata").get_object().get_data().decode()
    assert "<pdfaid:part>3</pdfaid:part>" in meta and "<pdfuaid:part>1</pdfuaid:part>" in meta
    # PDF/A needs the pdfuaid schema described.
    assert "<pdfaSchema:prefix>pdfuaid</pdfaSchema:prefix>" in meta


def test_pdf_ua_title_option_wins():
    pdf = render(pdf_ua=True, title="Titre explicite")
    assert PdfReader(io.BytesIO(pdf)).metadata["/Title"] == "Titre explicite"


@pytest.mark.parametrize(
    "html, message",
    [
        ("<html><head><title>T</title></head><body><p>x</p></body></html>", "language"),
        ('<html lang="fr"><body><p>x</p></body></html>', "title"),
        (f'<html lang="fr"><head><title>T</title></head><body><img src="{PIXEL}"></body></html>', "alt text"),
        ('<html lang="fr"><head><title>T</title></head><body><svg width="5" height="5"></svg></body></html>', "alt text"),
        ('<html lang="fr"><head><title>T</title></head><body><h2>x</h2></body></html>', "start at <h1>"),
        ('<html lang="fr"><head><title>T</title></head><body><h1>a</h1><h3>b</h3></body></html>', "skipped"),
    ],
)
def test_pdf_ua_refuses_non_conforming_documents(html, message):
    with pytest.raises(ferropdf.RenderError, match=message):
        render(html, pdf_ua=True)


def test_tagged_warns_about_skipped_headings():
    html = '<html lang="fr"><body><h1>a</h1><h3>b</h3></body></html>'
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        render(html, tagged=True)
    assert any("<h1> is followed by <h3>" in str(w.message) for w in caught)


def test_colspan_is_written():
    html = """<html lang="fr"><body><table>
    <tr><th>A</th><th>B</th><th>C</th></tr>
    <tr><td colspan="2">Total</td><td>3</td></tr></table></body></html>"""
    reader = PdfReader(io.BytesIO(render(html, tagged=True)))

    def cells(e):
        e = e.get_object()
        if not isinstance(e, DictionaryObject) or "/S" not in e:
            return []
        out = [e] if e["/S"] == "/TD" else []
        kids = e.get("/K", [])
        for k in kids if isinstance(kids, ArrayObject) else [kids]:
            out += cells(k)
        return out
    tds = cells(reader.trailer["/Root"]["/StructTreeRoot"]["/K"])
    assert [td.get("/A", {}).get("/ColSpan") for td in tds] == [2, None]


def test_tagged_warns_about_missing_alt():
    html = f'<html lang="fr"><body><img src="{PIXEL}" width="5" height="5"><p>x</p></body></html>'
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        pdf = render(html, tagged=True)
    assert any("alt text" in str(w.message) for w in caught)
    assert find(tree(pdf), "Figure")[0][1] == {}


def test_aria_hidden_and_presentation_are_artifacts():
    html = """<html lang="fr"><body><p>Lu</p><p aria-hidden="true">Caché</p>
    <div role="presentation"><p>Aussi</p></div></body></html>"""
    t = tree(render(html, tagged=True))
    assert roles(t) == ["Document", "P"]


def test_multi_page_and_internal_links():
    html = """<html lang="fr"><head><title>T</title></head><body>
    <p><a href="#fin">Aller à la fin</a></p>
    <h1 id="fin" style="break-before: page">Fin</h1><p>Dernière page.</p></body></html>"""
    pdf = render(html, pdf_ua=True)
    reader = PdfReader(io.BytesIO(pdf))
    assert len(reader.pages) == 2
    assert [p["/StructParents"] for p in reader.pages] == [0, 1]
    assert "/Tabs" not in reader.pages[1]  # no annotations there
    assert roles(tree(pdf)) == ["Document", "P", "Link", "H1", "P"]


def test_options_repr():
    assert "pdf_ua=True" in repr(ferropdf.Options(pdf_ua=True))
    assert "tagged=True" in repr(ferropdf.Options(tagged=True))
