"""PDF/A-2b / PDF/A-3b output, attachments and Factur-X (issue #42).

These tests check the structure ferropdf writes. Full conformance is checked
by veraPDF in CI (the ``pdfa`` job, files from tests/pdfa/make_corpus.py).
"""

from __future__ import annotations

import io
import os
from pathlib import Path

import pytest
from pypdf import PdfReader

import ferropdf

FIXTURES = Path(__file__).parent / "fixtures"
XML = (FIXTURES / "factur-x-en16931.xml").read_bytes()
HTML = "<html><body><h1>Facture</h1><p>Total : 1800,00 €</p></body></html>"


def render(**kw) -> bytes:
    return ferropdf.Engine(ferropdf.Options(**kw)).render(HTML)


def reader(pdf: bytes) -> PdfReader:
    return PdfReader(io.BytesIO(pdf))


def xmp(pdf: bytes) -> str:
    return reader(pdf).trailer["/Root"]["/Metadata"].get_object().get_data().decode("utf-8")


def attachment_specs(pdf: bytes) -> dict:
    root = reader(pdf).trailer["/Root"]
    return {str(s.get_object()["/UF"]): s.get_object() for s in root["/AF"]}


def test_plain_pdf_is_unchanged():
    pdf = render()
    root = reader(pdf).trailer["/Root"]
    for key in ("/Metadata", "/OutputIntents", "/AF", "/Names"):
        assert key not in root
    assert "/CreationDate" not in reader(pdf).metadata


def test_factur_x_writes_pdfa3b_with_invoice():
    pdf = render(title="Facture FA-2026-0042", author="Exemple SARL", factur_x=XML)
    r = reader(pdf)
    root = r.trailer["/Root"]

    meta = xmp(pdf)
    assert "<pdfaid:part>3</pdfaid:part>" in meta
    assert "<pdfaid:conformance>B</pdfaid:conformance>" in meta
    assert "<fx:ConformanceLevel>EN 16931</fx:ConformanceLevel>" in meta
    assert "<fx:DocumentFileName>factur-x.xml</fx:DocumentFileName>" in meta
    assert "<pdfaSchema:prefix>fx</pdfaSchema:prefix>" in meta
    assert "Facture FA-2026-0042" in meta and "Exemple SARL" in meta

    intent = root["/OutputIntents"][0].get_object()
    assert intent["/S"] == "/GTS_PDFA1"
    assert intent["/DestOutputProfile"].get_object()["/N"] == 3

    assert r.attachments == {"factur-x.xml": [XML]}
    spec = attachment_specs(pdf)["factur-x.xml"]
    assert spec["/AFRelationship"] == "/Alternative"
    stream = spec["/EF"]["/F"].get_object()
    assert stream["/Subtype"] == "/text/xml"
    assert "/ModDate" in stream["/Params"]

    assert len(r.trailer["/ID"]) == 2
    info = r.metadata
    assert info["/Producer"] == "ferropdf"
    assert info["/CreationDate"] == info["/ModDate"]


def test_pdfa_embeds_every_font():
    pdf = render(pdf_variant="pdf/a-3b")
    for page in reader(pdf).pages:
        for font in page["/Resources"]["/Font"].values():
            font = font.get_object()
            assert font["/Subtype"] == "/Type0", "non-embedded Type1 font in PDF/A"


def test_link_annotations_are_printable():
    pdf = ferropdf.from_html('<a href="https://example.com">x</a>')
    annot = reader(pdf).pages[0]["/Annots"][0].get_object()
    assert annot["/F"] & 4


def test_pdfa2b():
    meta = xmp(render(pdf_variant="PDF/A-2B"))
    assert "<pdfaid:part>2</pdfaid:part>" in meta
    assert "fx:" not in meta


@pytest.mark.parametrize(
    "profile, level, relationship",
    [
        ("MINIMUM", "MINIMUM", "/Data"),
        ("basic wl", "BASIC WL", "/Data"),
        ("BASIC", "BASIC", "/Alternative"),
        ("extended", "EXTENDED", "/Alternative"),
    ],
)
def test_factur_x_profile_override(profile, level, relationship):
    pdf = render(factur_x=XML, factur_x_profile=profile)
    assert f"<fx:ConformanceLevel>{level}</fx:ConformanceLevel>" in xmp(pdf)
    assert attachment_specs(pdf)["factur-x.xml"]["/AFRelationship"] == relationship


def test_xrechnung_uses_its_file_name():
    xml = XML.replace(
        b"urn:cen.eu:en16931:2017<",
        b"urn:cen.eu:en16931:2017#compliant#urn:xeinkauf.de:kosit:xrechnung_3.0<",
    )
    pdf = render(factur_x=xml.decode())
    assert "<fx:ConformanceLevel>XRECHNUNG</fx:ConformanceLevel>" in xmp(pdf)
    assert list(reader(pdf).attachments) == ["xrechnung.xml"]


def test_attachments_in_pdfa3b_and_plain_pdf():
    files = [
        ferropdf.Attachment("donnees.csv", "a,b\n1,2\n", description="Source", relationship="source"),
        ferropdf.Attachment("blob.bin", b"\x00\x01\x02"),
        ferropdf.Attachment("résumé.txt", "é"),
    ]
    for variant in ("pdf/a-3b", None):
        pdf = render(pdf_variant=variant, attachments=files)
        r = reader(pdf)
        assert r.attachments["donnees.csv"] == [b"a,b\n1,2\n"]
        assert r.attachments["blob.bin"] == [b"\x00\x01\x02"]
        specs = attachment_specs(pdf)
        assert specs["donnees.csv"]["/AFRelationship"] == "/Source"
        assert specs["donnees.csv"]["/Desc"] == "Source"
        assert specs["blob.bin"]["/AFRelationship"] == "/Unspecified"
        assert specs["donnees.csv"]["/EF"]["/F"].get_object()["/Subtype"] == "/text/csv"
        assert specs["blob.bin"]["/EF"]["/F"].get_object()["/Subtype"] == "/application/octet-stream"
        assert "résumé.txt" in specs
        names = r.trailer["/Root"]["/Names"]["/EmbeddedFiles"]["/Names"][::2]
        assert [bytes(n.original_bytes) for n in names] == sorted(
            bytes(n.original_bytes) for n in names
        ), "name tree keys must be sorted"
    assert "/Metadata" not in reader(render(attachments=files)).trailer["/Root"]


def test_factur_x_plus_attachments():
    pdf = render(factur_x=XML, attachments=[ferropdf.Attachment("annexe.csv", "x")])
    assert set(reader(pdf).attachments) == {"factur-x.xml", "annexe.csv"}


def test_attachment_api():
    a = ferropdf.Attachment("x.xml", "<a/>", mime_type="application/xml", relationship="Data")
    assert a.name == "x.xml" and a.data == b"<a/>" and a.mime_type == "application/xml"
    assert a.relationship == "Data"
    assert "x.xml" in repr(a)
    assert "factur_x" in repr(ferropdf.Options(factur_x=XML))


@pytest.mark.parametrize(
    "kwargs, message",
    [
        ({"pdf_variant": "pdf/a-1a"}, "pdf_variant"),
        ({"pdf_variant": "pdf/a-2b", "attachments": [ferropdf.Attachment("a.txt", "x")]}, "PDF/A-2b"),
        ({"pdf_variant": "pdf/a-2b", "factur_x": XML}, "PDF/A-3b"),
        ({"attachments": [ferropdf.Attachment("a", "x"), ferropdf.Attachment("a", "y")]}, "duplicate"),
        ({"factur_x": "<Invoice/>"}, "CrossIndustryInvoice"),
        ({"factur_x": XML.replace(b"urn:cen.eu:en16931:2017", b"urn:other")}, "profile"),
        ({"factur_x": XML, "factur_x_profile": "GOLD"}, "profile"),
        ({"factur_x_profile": "BASIC"}, "factur_x"),
        ({"factur_x": b"\xff\xfe"}, "UTF-8"),
        ({"factur_x": XML, "attachments": [ferropdf.Attachment("factur-x.xml", "x")]}, "duplicate"),
    ],
)
def test_invalid_options_raise_value_error(kwargs, message):
    with pytest.raises(ValueError, match=message):
        ferropdf.Options(**kwargs)


def test_invalid_attachment_arguments():
    with pytest.raises(ValueError, match="relationship"):
        ferropdf.Attachment("a", "x", relationship="Parent")
    with pytest.raises(TypeError):
        ferropdf.Attachment("a", 42)


def test_source_date_epoch_makes_output_reproducible(monkeypatch):
    monkeypatch.setenv("SOURCE_DATE_EPOCH", "1767225600")  # 2026-01-01T00:00:00Z
    a = render(factur_x=XML)
    b = render(factur_x=XML)
    assert a == b
    assert "<xmp:CreateDate>2026-01-01T00:00:00Z</xmp:CreateDate>" in xmp(a)
    assert reader(a).metadata["/CreationDate"].startswith("D:20260101000000")


@pytest.mark.skipif(not os.environ.get("FERROPDF_PDFA_OUT"), reason="set FERROPDF_PDFA_OUT")
def test_write_corpus_for_verapdf():
    import importlib.util

    spec = importlib.util.spec_from_file_location(
        "make_corpus", Path(__file__).parent / "pdfa" / "make_corpus.py"
    )
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    mod.main(Path(os.environ["FERROPDF_PDFA_OUT"]))
