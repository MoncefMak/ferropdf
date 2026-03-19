"""
Basic example — generates a simple PDF in output/
"""
import ferropdf
from pathlib import Path

output_dir = Path(__file__).parent.parent / "output"
output_dir.mkdir(exist_ok=True)

# 1. PDF minimal
ferropdf.write_pdf("<h1>Hello ferropdf!</h1>", str(output_dir / "hello.pdf"))
print(f"✓ {output_dir / 'hello.pdf'}")

# 2. PDF with styles
html = """
<html>
<head><style>
  body { font-family: sans-serif; margin: 40px; }
  h1 { color: #1e40af; border-bottom: 2px solid #1e40af; padding-bottom: 10px; }
  .card { background: #f3f4f6; padding: 20px; border-radius: 8px; margin-top: 20px; }
  .card p { margin: 8px 0; }
</style></head>
<body>
  <h1>Test Report</h1>
  <div class="card">
    <p><strong>Project:</strong> ferropdf</p>
    <p><strong>Version:</strong> 0.1.0</p>
    <p><strong>Status:</strong> All tests passing</p>
  </div>
</body>
</html>
"""
ferropdf.write_pdf(html, str(output_dir / "styled.pdf"))
print(f"✓ {output_dir / 'styled.pdf'}")

# 3. Invoice
invoice = open(Path(__file__).parent.parent / "tests" / "fixtures" / "invoice.html").read()
ferropdf.write_pdf(invoice, str(output_dir / "invoice.pdf"))
print(f"✓ {output_dir / 'invoice.pdf'}")

print(f"\n📄 PDFs generated in {output_dir}/")
