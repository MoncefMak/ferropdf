"""Write PDF/A files for external validation with veraPDF.

    python tests/pdfa/make_corpus.py OUT_DIR

File names carry the veraPDF flavours each file must pass: ``-2b``, ``-3b``
(PDF/A) and ``-ua1`` (PDF/UA-1), e.g. ``letter-3b-ua1.pdf`` is checked with
both ``--flavour 3b`` and ``--flavour ua1``. See the ``pdfa`` job in
``.github/workflows/ci.yml``.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

import ferropdf

HERE = Path(__file__).resolve().parent
TESTS = HERE.parent
TEMPLATES = TESTS / "corpus" / "templates"
FACTUR_X = TESTS / "fixtures" / "factur-x-en16931.xml"

# Features most likely to break conformance: transparency, gradients, SVG,
# links, bookmarks, rounded clipping, transforms, a glyph no font has.
FEATURES_HTML = """<!DOCTYPE html>
<html lang="fr"><head><title>PDF/A features</title><style>
  body { font-family: sans-serif; }
  h1 { color: #1a237e; }
  .grad { height: 60px; background: linear-gradient(90deg, #e91e63, rgba(33,150,243,.4)); }
  .radial { height: 60px; background: radial-gradient(circle, #ffeb3b, #4caf50); border-radius: 12px; }
  .glass { opacity: .5; background: #000; color: #fff; padding: 8px; }
  .shadow { box-shadow: 4px 4px 8px rgba(0,0,0,.3); padding: 8px; }
  .clip { overflow: hidden; border-radius: 50%; width: 80px; height: 80px; background: #009688; }
  .rot { transform: rotate(8deg); width: 120px; background: #ff9800; }
</style></head><body>
<h1 id="top">Fonctionnalités PDF/A</h1>
<p>Lien externe : <a href="https://example.com/facture">example.com</a>,
   lien interne : <a href="#end">fin du document</a>.</p>
<div class="grad"></div><div class="radial"></div>
<p class="glass">Texte semi-transparent</p>
<p class="shadow">Ombre portée</p>
<div class="clip"></div>
<div class="rot">Pivoté</div>
<svg width="120" height="60" viewBox="0 0 120 60" role="img" aria-label="Un carré bleu et un cercle rose">
  <rect x="5" y="5" width="50" height="50" fill="#3f51b5" opacity="0.6"/>
  <circle cx="90" cy="30" r="25" fill="none" stroke="#e91e63" stroke-width="4"/>
</svg>
<p>Caractère sans glyphe :  (zone privée).</p>
<h2>Accents et symboles</h2>
<p>àéîõü ÆŒ ß € « » — “guillemets” ½ ±</p>
<h2 id="end">Fin</h2>
</body></html>
"""


# Lists, tables, figures, links, languages: the PDF/UA structure tree.
STRUCTURE_HTML = """<!DOCTYPE html>
<html lang="fr"><head><title>Structure accessible</title><style>
  body { font-family: sans-serif; }
  .card { background: #eef; border: 1px solid #99c; padding: 8px; border-radius: 6px; }
  th, td { border: 1px solid #999; padding: 4px; }
  @page { @bottom-center { content: "Page " counter(page) " / " counter(pages); } }
</style></head><body>
<header><p>Société Exemple — rapport annuel</p></header>
<h1>Rapport</h1>
<p>Un paragraphe avec un <a href="https://example.com/rapport">lien externe</a>, du <b>gras</b>
et un <a href="#tableau">lien interne</a>.</p>
<section class="card"><h2>Listes</h2>
  <ul><li>Premier</li><li>Second <i>élément</i></li></ul>
  <ol><li><p>Étape un</p></li><li>Étape deux</li></ol>
</section>
<h2 id="tableau">Tableau</h2>
<table><caption>Ventes par région</caption>
  <thead><tr><th>Région</th><th>Total</th></tr></thead>
  <tbody><tr><th>Nord</th><td>1 200 €</td></tr><tr><th>Sud</th><td>950 €</td></tr></tbody>
</table>
<figure><img alt="Logo : carré rouge" width="40" height="40"
  src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==">
  <figcaption>Le logo de la société</figcaption></figure>
<p><img alt="" width="200" height="4"
  src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg=="></p>
<blockquote lang="en"><p>A quotation in English.</p></blockquote>
<div style="break-before: page"><h2>Annexe</h2><p>Deuxième page.</p></div>
</body></html>
"""


def accessible(html: str) -> str:
    """The fixes an author makes for PDF/UA: a language, labels on SVGs,
    headings that start at <h1> and do not skip levels."""
    if not re.search(r"<html[^>]*\blang=", html):
        html = re.sub(r"<html\b", '<html lang="en"', html, count=1)
    html = re.sub(r"<svg\b(?![^>]*aria-label)", '<svg role="img" aria-label="Chart"', html)
    prev, current = 0, 1

    def renumber(m: re.Match) -> str:
        nonlocal prev, current
        if not m.group(1):  # opening tag
            current = min(int(m.group(2)), prev + 1)
            prev = current
        return f"<{m.group(1)}h{current}"

    return re.sub(r"<(/?)h([1-6])\b", renumber, html)


def main(out: Path) -> None:
    out.mkdir(parents=True, exist_ok=True)

    def write(name: str, html: str, base_url: Path | None = None, **kw) -> None:
        opts = ferropdf.Options(base_url=str(base_url) if base_url else None, **kw)
        (out / name).write_bytes(ferropdf.Engine(opts).render(html))
        print("wrote", name)

    xml = FACTUR_X.read_bytes()
    invoice = (TESTS / "fixtures" / "invoice.html").read_text(encoding="utf-8")
    write(
        "facturx-en16931-3b.pdf",
        invoice,
        title="Facture FA-2026-0042",
        author="Exemple SARL",
        factur_x=xml,
    )
    write(
        "attachments-3b.pdf",
        invoice,
        pdf_variant="pdf/a-3b",
        title="Rapport avec annexes",
        attachments=[
            ferropdf.Attachment("donnees.csv", "mois,total\n2026-10,1800\n",
                                description="Données source", relationship="Source"),
            ferropdf.Attachment("notes.txt", "Annexe é ü", relationship="Supplement"),
        ],
    )
    write(
        "facturx-en16931-3b-ua1.pdf",
        accessible(invoice),
        title="Facture FA-2026-0042",
        factur_x=xml,
        pdf_ua=True,
    )
    write("features-2b.pdf", FEATURES_HTML, pdf_variant="pdf/a-2b", title="PDF/A features")
    write("features-3b.pdf", FEATURES_HTML, pdf_variant="pdf/a-3b")
    write("features-ua1.pdf", FEATURES_HTML, pdf_ua=True)
    write("structure-ua1.pdf", STRUCTURE_HTML, pdf_ua=True)
    write("structure-2b-ua1.pdf", STRUCTURE_HTML, pdf_variant="pdf/a-2b", pdf_ua=True)
    for tpl in sorted(TEMPLATES.glob("*.html")):
        html = tpl.read_text(encoding="utf-8")
        write(f"{tpl.stem}-3b.pdf", html, base_url=TEMPLATES, pdf_variant="pdf/a-3b",
              title=tpl.stem)
        write(f"{tpl.stem}-3b-ua1.pdf", accessible(html), base_url=TEMPLATES,
              pdf_variant="pdf/a-3b", pdf_ua=True, title=tpl.stem)


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    main(Path(sys.argv[1]))
