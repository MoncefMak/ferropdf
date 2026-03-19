"""
Django views — each view generates a PDF via ferropdf.
"""
from django.http import HttpResponse
from django.template.loader import render_to_string
import ferropdf


def _pdf_response(template_name: str, context: dict, filename: str = "document.pdf") -> HttpResponse:
    """Render a template to HTML then convert it to PDF."""
    html = render_to_string(template_name, context)
    engine = ferropdf.Engine(ferropdf.Options(margin="15mm"))
    pdf = engine.render(html)
    response = HttpResponse(content=pdf, content_type="application/pdf")
    response["Content-Disposition"] = f'inline; filename="{filename}"'
    return response


def home(request):
    """Home page with links to all PDFs."""
    html = """
    <html>
    <head><style>
      body { font-family: sans-serif; max-width: 640px; margin: 40px auto; color: #1f2937; }
      h1 { color: #1e40af; margin-bottom: 8px; }
      p.sub { color: #6b7280; margin-bottom: 24px; }
      ul { list-style: none; padding: 0; }
      li { padding: 10px 0; border-bottom: 1px solid #e5e7eb; }
      li:last-child { border: none; }
      a { color: #1e40af; text-decoration: none; font-weight: 500; }
      .desc { color: #6b7280; font-size: 13px; }
    </style></head>
    <body>
      <h1>ferropdf + Django</h1>
      <p class="sub">Server-side PDF generation examples, powered by Rust.</p>
      <ul>
        <li>
          <a href="/invoice/1/pdf/">📄 Invoice #1</a>
          <div class="desc">Table with header, rows, totals</div>
        </li>
        <li>
          <a href="/invoice/2/pdf/">📄 Invoice #2</a>
          <div class="desc">Same template, different data</div>
        </li>
        <li>
          <a href="/receipt/pdf/">🧂 Receipt</a>
          <div class="desc">Dashed borders, subtotals, VAT</div>
        </li>
        <li>
          <a href="/dashboard/pdf/">📈 Dashboard</a>
          <div class="desc">Two tables, colored badges, summary</div>
        </li>
        <li>
          <a href="/letter/pdf/">✉️ Formal letter</a>
          <div class="desc">Header, address, body, signature</div>
        </li>
      </ul>
    </body>
    </html>
    """
    return HttpResponse(html)


def invoice_pdf(request, invoice_id: int):
    """Invoice PDF with line items."""
    items = [
        {"desc": "Rust engine development (PDF module)", "qty": 12, "price": 180},
        {"desc": "Python / PyO3 bindings integration", "qty": 8, "price": 150},
        {"desc": "REST API development (FastAPI)", "qty": 6, "price": 140},
        {"desc": "CI/CD integration (GitHub Actions)", "qty": 4, "price": 120},
        {"desc": "Unit and integration testing", "qty": 5, "price": 110},
        {"desc": "OWASP Top 10 security audit", "qty": 3, "price": 200},
        {"desc": "Technical documentation (Sphinx)", "qty": 4, "price": 100},
        {"desc": "Backend team training (2 days)", "qty": 2, "price": 950},
        {"desc": "Post-delivery support (1 month)", "qty": 1, "price": 1500},
        {"desc": "FerroSuite Enterprise license (annual)", "qty": 1, "price": 2400},
    ]
    for item in items:
        item["line_total"] = item["qty"] * item["price"]
    total = sum(i["line_total"] for i in items)
    return _pdf_response("invoice.html", {
        "invoice_id": invoice_id,
        "items": items,
        "total": total,
    }, filename=f"invoice-{invoice_id}.pdf")


def receipt_pdf(request):
    """Receipt PDF with VAT."""
    items = [
        {"name": "Butter croissant", "qty": 3, "price": 1.20},
        {"name": "Long black coffee", "qty": 2, "price": 2.50},
        {"name": "Fresh orange juice", "qty": 1, "price": 3.80},
        {"name": "Chocolate croissant", "qty": 2, "price": 1.40},
        {"name": "Jam toast", "qty": 1, "price": 2.90},
        {"name": "Traditional baguette", "qty": 2, "price": 1.30},
        {"name": "Chocolate eclair", "qty": 1, "price": 3.50},
        {"name": "Apple tart (slice)", "qty": 2, "price": 4.20},
        {"name": "Quiche lorraine (slice)", "qty": 1, "price": 4.80},
        {"name": "Water bottle 50cl", "qty": 3, "price": 1.50},
        {"name": "Dark chocolate cookie", "qty": 4, "price": 1.80},
        {"name": "Macaron assortment x3", "qty": 1, "price": 5.40},
    ]
    for item in items:
        item["line_total"] = round(item["qty"] * item["price"], 2)
    subtotal = sum(i["line_total"] for i in items)
    tax = round(subtotal * 0.20, 2)
    return _pdf_response("receipt.html", {
        "shop_name": "Code Bakery",
        "shop_address": "12 Byte Street, 75011 Paris",
        "receipt_id": "2026-03-0042",
        "date": "17/03/2026 — 08:34",
        "cashier": "Marie L.",
        "payment_method": "Credit card",
        "items": items,
        "subtotal": f"{subtotal:.2f}",
        "tax": f"{tax:.2f}",
        "total": f"{subtotal + tax:.2f}",
    }, filename="receipt.pdf")


def dashboard_pdf(request):
    """Commercial dashboard."""
    categories = [
        {"name": "Software", "revenue": 245000, "margin": 68.2, "trend": "up"},
        {"name": "Services", "revenue": 182000, "margin": 42.5, "trend": "up"},
        {"name": "Training", "revenue": 67000, "margin": 55.1, "trend": "stable"},
        {"name": "Support", "revenue": 43000, "margin": 31.8, "trend": "down"},
        {"name": "Cloud & Hosting", "revenue": 128000, "margin": 72.4, "trend": "up"},
        {"name": "Consulting", "revenue": 95000, "margin": 48.7, "trend": "stable"},
        {"name": "Third-party licenses", "revenue": 31000, "margin": 15.2, "trend": "down"},
        {"name": "Maintenance", "revenue": 52000, "margin": 61.0, "trend": "up"},
    ]
    for c in categories:
        c["revenue_fmt"] = f"{c['revenue']:,}"
        c["margin_fmt"] = f"{c['margin']:.1f}"
    top_products = [
        {"name": "FerroSuite Enterprise", "units": 342, "revenue": 171000},
        {"name": "FerroAPI Pro", "units": 580, "revenue": 58000},
        {"name": "Consulting On-Site", "units": 15, "revenue": 45000},
        {"name": "Advanced Rust Training", "units": 28, "revenue": 33600},
        {"name": "Support Premium", "units": 120, "revenue": 24000},
        {"name": "FerroCloud Starter", "units": 890, "revenue": 44500},
        {"name": "Security Audit", "units": 8, "revenue": 32000},
        {"name": "Migration Legacy", "units": 5, "revenue": 25000},
        {"name": "FerroCI Pipeline", "units": 210, "revenue": 21000},
        {"name": "Team Workshop", "units": 12, "revenue": 18000},
    ]
    for p in top_products:
        p["revenue_fmt"] = f"{p['revenue']:,}"
    total_revenue = sum(c["revenue"] for c in categories)
    avg_margin = sum(c["margin"] for c in categories) / len(categories)
    return _pdf_response("dashboard.html", {
        "title": "Dashboard — Q1 2026",
        "period": "January – March 2026",
        "date": "17/03/2026",
        "categories": categories,
        "top_products": top_products,
        "total_revenue": f"{total_revenue:,}",
        "avg_margin": f"{avg_margin:.1f}",
        "transactions": 2210,
    }, filename="dashboard.pdf")


def letter_pdf(request):
    """Formal business letter."""
    return _pdf_response("letter.html", {
        "company_name": "FerroTech SARL",
        "company_address": "42 rue du Code, 75001 Paris — contact@ferrotech.dev",
        "recipient_name": "Mr. John Smith",
        "recipient_company": "Acme Corp",
        "recipient_address": "123 Business Avenue, 69000 Lyon",
        "city": "Paris",
        "date": "17 mars 2026",
        "sender_name": "Sophie Martin",
        "sender_title": "Technical Director — FerroTech SARL",
        "subject": "Technical partnership proposal",
        "paragraphs": [
            "Following our meeting on March 10, we are pleased to "
            "submit our partnership proposal for integrating our high-performance "
            "PDF rendering engine into your document platform. This solution "
            "was specifically designed to meet the performance and typographic "
            "quality requirements of production environments.",
            "Our solution, ferropdf, is built on an optimized Rust pipeline capable of "
            "converting HTML/CSS to PDF in milliseconds. Compatible with Python "
            "via PyO3, it integrates natively with FastAPI and Django. The engine supports "
            "CSS tables, flexbox, collapsing margins, automatic pagination, "
            "system font embedding, and high-fidelity typographic rendering "
            "via cosmic-text.",
            "The pipeline consists of six independent Rust crates: ferropdf-parse (HTML5 "
            "via html5ever), ferropdf-style (CSS cascade + unit resolution), "
            "ferropdf-layout (Taffy for flexbox/grid + cosmic-text for shaping), "
            "ferropdf-page (pagination and fragmentation), ferropdf-render (display list + "
            "pdf-writer), and ferropdf-core (shared types). Each crate can evolve "
            "independently, simplifying maintenance and updates.",
            "We offer comprehensive technical support including team training, "
            "CI/CD integration, and priority support during "
            "the first six months. Our team of certified Rust consultants will be available "
            "for pair-programming sessions and weekly code reviews.",
            "In terms of performance, our internal benchmarks show an average rendering time "
            "of 12ms for a standard A4 document, compared to 800ms for headless "
            "Chromium solutions. Memory consumption is reduced by 95%, allowing "
            "hundreds of simultaneous requests to be served on a modest instance.",
            "We remain at your disposal for any additional information and hope "
            "that this collaboration will be the start of a fruitful partnership. Feel free "
            "to contact us directly at tech@ferrotech.dev or at 01 42 67 89 10 "
            "to schedule a personalized demonstration.",
        ],
    }, filename="letter.pdf")
