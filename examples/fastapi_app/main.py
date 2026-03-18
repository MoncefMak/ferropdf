"""
Exemple FastAPI — serveur PDF à la volée.

Lancer :
    pip install fastapi uvicorn jinja2
    cd examples/fastapi_app
    uvicorn main:app --reload

Endpoints :
    GET /                            → page d'accueil (liens vers tous les PDFs)
    GET /invoice/{id}/pdf            → PDF facture
    GET /report/pdf?title=...        → PDF rapport technique
    GET /receipt/pdf                  → PDF ticket de caisse
    GET /dashboard/pdf?period=...    → PDF tableau de bord commercial
    GET /letter/pdf                   → PDF lettre formelle
"""
from pathlib import Path
import asyncio

from fastapi import FastAPI
from fastapi.responses import HTMLResponse, Response
from jinja2 import Environment, FileSystemLoader
import ferropdf

app = FastAPI(title="ferropdf FastAPI Example")

templates = Environment(loader=FileSystemLoader(Path(__file__).parent / "templates"))


async def _render_pdf(html: str, filename: str, margin: str = "15mm") -> Response:
    """Helper : rend du HTML en PDF de façon non-bloquante."""
    engine = ferropdf.Engine(ferropdf.Options(margin=margin))
    loop = asyncio.get_event_loop()
    pdf_bytes = await loop.run_in_executor(None, engine.render, html)
    return Response(
        content=pdf_bytes,
        media_type="application/pdf",
        headers={"Content-Disposition": f'inline; filename="{filename}"'},
    )


# ──────────────────────────────────────────────
# Page d'accueil
# ──────────────────────────────────────────────

@app.get("/", response_class=HTMLResponse)
async def home():
    return """
    <html>
    <head><style>
      body { font-family: sans-serif; max-width: 640px; margin: 40px auto; color: #1f2937; }
      h1 { color: #1e40af; margin-bottom: 8px; }
      p.sub { color: #6b7280; margin-bottom: 24px; }
      ul { list-style: none; padding: 0; }
      li { padding: 10px 0; border-bottom: 1px solid #e5e7eb; }
      li:last-child { border: none; }
      a { color: #1e40af; text-decoration: none; font-weight: 500; }
      a:hover { text-decoration: underline; }
      .desc { color: #6b7280; font-size: 13px; }
    </style></head>
    <body>
      <h1>ferropdf + FastAPI</h1>
      <p class="sub">Exemples de génération PDF à la volée, propulsé par Rust.</p>
      <ul>
        <li>
          <a href="/invoice/1/pdf">📄 Facture #1</a>
          <div class="desc">Table avec en-tête coloré, totaux, mise en page professionnelle</div>
        </li>
        <li>
          <a href="/invoice/2/pdf">📄 Facture #2</a>
          <div class="desc">Même template, données différentes</div>
        </li>
        <li>
          <a href="/report/pdf?title=Rapport%20Technique">📊 Rapport technique</a>
          <div class="desc">Titres, paragraphes, stat-cards, architecture</div>
        </li>
        <li>
          <a href="/receipt/pdf">🧾 Ticket de caisse</a>
          <div class="desc">Bordures pointillées, sous-totaux, TVA, alignement droite</div>
        </li>
        <li>
          <a href="/dashboard/pdf?period=T1%202026">📈 Tableau de bord T1</a>
          <div class="desc">Deux tables, badges colorés, boîte résumé</div>
        </li>
        <li>
          <a href="/letter/pdf">✉️ Lettre formelle</a>
          <div class="desc">En-tête centré, adresses, corps de lettre, signature</div>
        </li>
      </ul>
    </body>
    </html>
    """


# ──────────────────────────────────────────────
# 1. Facture
# ──────────────────────────────────────────────

@app.get("/invoice/{invoice_id}/pdf")
async def invoice_pdf(invoice_id: int):
    """Facture PDF avec table de lignes et totaux."""
    items = [
        {"desc": "Développement moteur Rust (module PDF)", "qty": 12, "price": 180},
        {"desc": "Intégration Python / PyO3 bindings", "qty": 8, "price": 150},
        {"desc": "Développement API REST (FastAPI)", "qty": 6, "price": 140},
        {"desc": "Intégration CI/CD (GitHub Actions)", "qty": 4, "price": 120},
        {"desc": "Tests unitaires et d'intégration", "qty": 5, "price": 110},
        {"desc": "Audit sécurité OWASP Top 10", "qty": 3, "price": 200},
        {"desc": "Documentation technique (Sphinx)", "qty": 4, "price": 100},
        {"desc": "Formation équipe backend (2 jours)", "qty": 2, "price": 950},
        {"desc": "Support post-livraison (1 mois)", "qty": 1, "price": 1500},
        {"desc": "Licence FerroSuite Enterprise (annuelle)", "qty": 1, "price": 2400},
    ]
    html = templates.get_template("invoice.html").render(
        invoice_id=invoice_id,
        items=items,
        total=sum(i["qty"] * i["price"] for i in items),
    )
    return await _render_pdf(html, f"facture-{invoice_id}.pdf")


# ──────────────────────────────────────────────
# 2. Rapport technique
# ──────────────────────────────────────────────

@app.get("/report/pdf")
async def report_pdf(title: str = "Rapport Technique"):
    """Rapport avec titres, stat-cards et paragraphes."""
    html = templates.get_template("report.html").render(title=title)
    return await _render_pdf(html, "report.pdf")


# ──────────────────────────────────────────────
# 3. Ticket de caisse
# ──────────────────────────────────────────────

@app.get("/receipt/pdf")
async def receipt_pdf():
    """Ticket de caisse avec sous-totaux et TVA."""
    items = [
        {"name": "Croissant beurre", "qty": 3, "price": 1.20},
        {"name": "Café allongé", "qty": 2, "price": 2.50},
        {"name": "Jus d'orange pressé", "qty": 1, "price": 3.80},
        {"name": "Pain au chocolat", "qty": 2, "price": 1.40},
        {"name": "Tartine confiture", "qty": 1, "price": 2.90},
        {"name": "Baguette tradition", "qty": 2, "price": 1.30},
        {"name": "Éclair au chocolat", "qty": 1, "price": 3.50},
        {"name": "Tarte aux pommes (part)", "qty": 2, "price": 4.20},
        {"name": "Quiche lorraine (part)", "qty": 1, "price": 4.80},
        {"name": "Bouteille d'eau 50cl", "qty": 3, "price": 1.50},
        {"name": "Cookie chocolat noir", "qty": 4, "price": 1.80},
        {"name": "Macaron assortiment x3", "qty": 1, "price": 5.40},
    ]
    for item in items:
        item["line_total"] = round(item["qty"] * item["price"], 2)
    subtotal = round(sum(i["line_total"] for i in items), 2)
    tax = round(subtotal * 0.20, 2)
    total = round(subtotal + tax, 2)

    html = templates.get_template("receipt.html").render(
        shop_name="Boulangerie du Code",
        shop_address="12 rue des Octets, 75011 Paris",
        receipt_id="2026-03-0042",
        date="17/03/2026 — 08:34",
        cashier="Marie L.",
        payment_method="Carte bancaire",
        items=items,
        subtotal=subtotal,
        tax=tax,
        total=total,
    )
    return await _render_pdf(html, "ticket.pdf")


# ──────────────────────────────────────────────
# 4. Tableau de bord commercial
# ──────────────────────────────────────────────

@app.get("/dashboard/pdf")
async def dashboard_pdf(period: str = "T1 2026"):
    """Dashboard avec deux tables et badges colorés."""
    categories = [
        {"name": "Logiciels", "revenue": 245000, "margin": 68.2, "trend": "up"},
        {"name": "Services", "revenue": 182000, "margin": 42.5, "trend": "up"},
        {"name": "Formation", "revenue": 67000, "margin": 55.1, "trend": "stable"},
        {"name": "Support", "revenue": 43000, "margin": 31.8, "trend": "down"},
        {"name": "Cloud & Hosting", "revenue": 128000, "margin": 72.4, "trend": "up"},
        {"name": "Consulting", "revenue": 95000, "margin": 48.7, "trend": "stable"},
        {"name": "Licences tierces", "revenue": 31000, "margin": 15.2, "trend": "down"},
        {"name": "Maintenance", "revenue": 52000, "margin": 61.0, "trend": "up"},
    ]
    top_products = [
        {"name": "FerroSuite Enterprise", "units": 342, "revenue": 171000},
        {"name": "FerroAPI Pro", "units": 580, "revenue": 58000},
        {"name": "Consulting On-Site", "units": 15, "revenue": 45000},
        {"name": "Formation Rust Avancé", "units": 28, "revenue": 33600},
        {"name": "Support Premium", "units": 120, "revenue": 24000},
        {"name": "FerroCloud Starter", "units": 890, "revenue": 44500},
        {"name": "Audit Sécurité", "units": 8, "revenue": 32000},
        {"name": "Migration Legacy", "units": 5, "revenue": 25000},
        {"name": "FerroCI Pipeline", "units": 210, "revenue": 21000},
        {"name": "Workshop Équipe", "units": 12, "revenue": 18000},
    ]
    total_revenue = sum(c["revenue"] for c in categories)
    avg_margin = sum(c["margin"] for c in categories) / len(categories)

    html = templates.get_template("dashboard.html").render(
        title=f"Tableau de Bord — {period}",
        period=period,
        date="17/03/2026",
        categories=categories,
        top_products=top_products,
        total_revenue=total_revenue,
        avg_margin=avg_margin,
        transactions=2210,
    )
    return await _render_pdf(html, f"dashboard-{period}.pdf")


# ──────────────────────────────────────────────
# 5. Lettre formelle
# ──────────────────────────────────────────────

@app.get("/letter/pdf")
async def letter_pdf():
    """Lettre d'affaires formelle."""
    html = templates.get_template("letter.html").render(
        company_name="FerroTech SARL",
        company_address="42 rue du Code, 75001 Paris — contact@ferrotech.dev",
        recipient_name="M. Jean Dupont",
        recipient_company="Acme Corp",
        recipient_address="123 Business Avenue, 69000 Lyon",
        city="Paris",
        date="17 mars 2026",
        sender_name="Sophie Martin",
        sender_title="Directrice Technique — FerroTech SARL",
        subject="Proposition de partenariat technique",
        paragraphs=[
            "Suite à notre entretien du 10 mars dernier, nous avons le plaisir de vous "
            "transmettre notre proposition de partenariat pour l'intégration de notre moteur "
            "de rendu PDF haute performance dans votre plateforme documentaire. Cette solution "
            "a été spécifiquement conçue pour répondre aux exigences de performance et de "
            "qualité typographique des environnements de production.",
            "Notre solution, ferropdf, repose sur un pipeline Rust optimisé capable de "
            "convertir du HTML/CSS en PDF en quelques millisecondes. Compatible avec Python "
            "via PyO3, elle s'intègre nativement avec FastAPI et Django. Le moteur supporte "
            "les tableaux CSS, le flexbox, les marges collapsantes, la pagination automatique, "
            "l'embarquement des polices système et le rendu typographique de haute fidélité "
            "via cosmic-text.",
            "Le pipeline se compose de six crates Rust indépendants : ferropdf-parse (HTML5 "
            "via html5ever), ferropdf-style (cascade CSS + résolution des unités), "
            "ferropdf-layout (Taffy pour flexbox/grid + cosmic-text pour le shaping), "
            "ferropdf-page (pagination et fragmentation), ferropdf-render (display list + "
            "pdf-writer), et ferropdf-core (types partagés). Chaque crate peut évoluer "
            "indépendamment, facilitant la maintenance et les mises à jour.",
            "Nous proposons un accompagnement technique complet incluant la formation de "
            "vos équipes, l'intégration dans votre CI/CD, et un support prioritaire pendant "
            "les six premiers mois. Notre équipe de consultants Rust certifiés sera disponible "
            "pour des sessions de pair-programming et des revues de code hebdomadaires.",
            "En termes de performance, nos benchmarks internes montrent un temps de rendu "
            "moyen de 12ms pour un document A4 standard, contre 800ms pour les solutions "
            "headless Chromium. La consommation mémoire est réduite de 95%, permettant de "
            "servir des centaines de requêtes simultanées sur une instance modeste.",
            "Nous restons à votre disposition pour tout complément d'information et espérons "
            "que cette collaboration sera le début d'un partenariat fructueux. N'hésitez pas "
            "à nous contacter directement à l'adresse tech@ferrotech.dev ou au 01 42 67 89 10 "
            "pour planifier une démonstration personnalisée.",
        ],
    )
    return await _render_pdf(html, "lettre.pdf", margin="20mm")
