"""Write PDF/A files for external validation with veraPDF.

    python tests/pdfa/make_corpus.py OUT_DIR

Produces ``*-2b.pdf`` and ``*-3b.pdf`` files (the suffix is the flavour veraPDF
should check). CI runs ``verapdf --flavour 3b`` / ``--flavour 2b`` on them;
see the ``pdfa`` job in ``.github/workflows/ci.yml``.
"""

from __future__ import annotations

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
<svg width="120" height="60" viewBox="0 0 120 60">
  <rect x="5" y="5" width="50" height="50" fill="#3f51b5" opacity="0.6"/>
  <circle cx="90" cy="30" r="25" fill="none" stroke="#e91e63" stroke-width="4"/>
</svg>
<p>Caractère sans glyphe :  (zone privée).</p>
<h2>Accents et symboles</h2>
<p>àéîõü ÆŒ ß € « » — “guillemets” ½ ±</p>
<h2 id="end">Fin</h2>
</body></html>
"""


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
    write("features-2b.pdf", FEATURES_HTML, pdf_variant="pdf/a-2b", title="PDF/A features")
    write("features-3b.pdf", FEATURES_HTML, pdf_variant="pdf/a-3b")
    for tpl in sorted(TEMPLATES.glob("*.html")):
        html = tpl.read_text(encoding="utf-8")
        write(f"{tpl.stem}-3b.pdf", html, base_url=TEMPLATES, pdf_variant="pdf/a-3b",
              title=tpl.stem)


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    main(Path(sys.argv[1]))
