"""Made-up bills laid out like common Indian ones: a GST tax invoice with an item table and
CGST/SGST, an inter-state one with IGST, and a small shop bill without GST. Every name, GSTIN and
number is invented."""
import sys
from reportlab.lib.pagesizes import A4
from reportlab.pdfgen import canvas

W, H = A4

def gst_invoice(path, igst=False):
    c = canvas.Canvas(path, pagesize=A4)
    c.setFont("Helvetica-Bold", 14); c.drawCentredString(W/2, H-40, "TAX INVOICE")
    c.setFont("Helvetica-Bold", 11); c.drawString(40, H-70, "Jimmy Doe Electricals")
    c.setFont("Helvetica", 8)
    c.drawString(40, H-82, "Shop 4, Example Market, Pune 411001")
    c.drawString(40, H-94, "GSTIN: 27ABCDE1234F1Z5    State: Maharashtra (27)")
    c.drawRightString(W-40, H-70, "Invoice No: JDE/2026-27/0114")
    c.drawRightString(W-40, H-82, "Invoice Date: 02/10/2026")
    c.drawString(40, H-120, "Bill To: Jane Doe")
    c.drawString(40, H-132, "GSTIN: " + ("29AAAPL1234C1Z3" if igst else "Unregistered"))
    y = H-165
    c.setFont("Helvetica-Bold", 8)
    for x, t in ((40, "S.No."), (75, "Description of Goods"), (300, "HSN"), (360, "Qty"), (400, "Unit")): c.drawString(x, y, t)
    for x, t in ((470, "Rate"), (555, "Amount")): c.drawRightString(x, y, t)
    c.setFont("Helvetica", 8)
    rows = [("Copper wire 2.5 sq mm FR", "8544", "4", "Coil", "1,850.50"), ("Modular switch 6A", "8536", "24", "Pcs", "62.00"),
            ("MCB 32A double pole", "8536", "2", "Nos", "1,145.00"), ("LED panel 15W round", "9405", "12", "Pcs", "450.00")]
    taxable = 0
    for i, (d, h, q, u, r) in enumerate(rows, 1):
        y -= 14
        amt = round(float(q) * float(r.replace(",", "")), 2); taxable += amt
        c.drawString(45, y, str(i)); c.drawString(75, y, d); c.drawString(300, y, h); c.drawString(360, y, q); c.drawString(400, y, u)
        c.drawRightString(470, y, r); c.drawRightString(555, y, f"{amt:,.2f}")
    y -= 24
    def tot(label, v, bold=False):
        nonlocal y
        c.setFont("Helvetica-Bold" if bold else "Helvetica", 8); c.drawString(360, y, label); c.drawRightString(555, y, f"{v:,.2f}"); y -= 13
    tot("Taxable Value", taxable)
    if igst:
        tot("IGST @ 18%", round(taxable * 0.18, 2)); total = taxable + round(taxable * 0.18, 2)
    else:
        half = round(taxable * 0.09, 2); tot("CGST @ 9%", half); tot("SGST @ 9%", half); total = taxable + 2 * half
    tot("Grand Total", total, True)
    c.setFont("Helvetica", 7); c.drawString(40, y - 20, "Amount in words: Rupees only. E. & O.E.")
    c.save()

def shop_bill(path):
    c = canvas.Canvas(path, pagesize=(300, 420))
    c.setFont("Helvetica-Bold", 11); c.drawCentredString(150, 395, "SHARMA HARDWARE STORE")
    c.setFont("Helvetica", 7); c.drawCentredString(150, 384, "Main Road, Example Nagar")
    c.drawString(15, 365, "Bill No. 4471"); c.drawRightString(285, 365, "Date: 06-Oct-2026")
    c.setFont("Helvetica-Bold", 7)
    c.drawString(15, 345, "Item"); c.drawRightString(170, 345, "Qty"); c.drawRightString(225, 345, "Rate"); c.drawRightString(285, 345, "Amt")
    c.setFont("Helvetica", 7)
    y = 345
    total = 0
    for d, q, r in (("Wall putty 20kg", 3, 780), ("Primer 4L", 2, 1150), ("Sandpaper sheet", 20, 12.5), ("Brush 4 inch", 2, 145)):
        y -= 12; amt = q * r; total += amt
        c.drawString(15, y, d); c.drawRightString(170, y, str(q)); c.drawRightString(225, y, f"{r:.2f}"); c.drawRightString(285, y, f"{amt:.2f}")
    y -= 18
    c.setFont("Helvetica-Bold", 8); c.drawString(15, y, "TOTAL"); c.drawRightString(285, y, f"{total:.2f}")
    c.setFont("Helvetica", 7); c.drawString(15, y - 14, "Paid by: Cash. Thank you, visit again!")
    c.save()

gst_invoice(sys.argv[1] + "/gst-cgst.pdf")
gst_invoice(sys.argv[1] + "/gst-igst.pdf", igst=True)
shop_bill(sys.argv[1] + "/shop.pdf")
