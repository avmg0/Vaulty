"""A made-up bank statement PDF laid out like common Indian ones: account details at the top,
a table with Date, Narration (often over two lines), Ref No, Value Date, Withdrawal, Deposit and
Closing Balance, the table header repeated on each page, and a footer. Encrypted with a password,
as banks send them. Every name and number is invented."""
import sys, subprocess
from reportlab.lib.pagesizes import A4
from reportlab.pdfgen import canvas

W, H = A4
rows = [
    ("01/10/26", ["UPI-SWIGGY-swiggy@icici-ICIC0000103-", "627412345678-Payment for order"], "0000627412345678", "01/10/26", "450.00", "", "84,550.00"),
    ("01/10/26", ["SALARY OCT 2026 EXAMPLE TECH PVT LTD"], "N274000123456", "01/10/26", "", "85,000.00", "1,69,550.00"),
    ("03/10/26", ["ACH D- EXAMPLE MF SIP"], "0000000000123", "03/10/26", "5,000.00", "", "1,64,550.00"),
    ("05/10/26", ["UPI-JIMMY D-jimmyd@okaxis-UTIB0001234-", "627598765432-Wiring advance"], "0000627598765432", "05/10/26", "15,000.00", "", "1,49,550.00"),
    ("07/10/26", ["POS 412345XXXXXX1234 BIG BAZAAR PUNE"], "000012345678", "07/10/26", "2,345.50", "", "1,47,204.50"),
]
# Enough rows to run onto a second page.
for i in range(30):
    d = f"{8 + i % 20:02d}/10/26"
    rows.append((d, [f"UPI-CORNER CAFE-cafe{i}@ybl-YESB0000001-", f"62770000{i:04d}-Tea"], f"00006277000{i:05d}", d, "120.00", "", f"{147204.50 - 120 * (i + 1):,.2f}"))
rows.append(("29/10/26", ["NEFT CR-HDFC0000001-JOHN DOE-RENT SHARE"], "N302000999888", "29/10/26", "", "12,000.00", "1,55,604.50"))

c = canvas.Canvas(sys.argv[1], pagesize=A4)
page = 1
def header():
    c.setFont("Helvetica-Bold", 12); c.drawString(40, H - 40, "Example Bank Ltd")
    c.setFont("Helvetica", 8)
    c.drawString(40, H - 55, "Statement of account for the period 01/10/2026 to 31/10/2026")
    c.drawString(40, H - 67, "Account No: 50100123456789   Customer: JANE DOE   IFSC: EXMP0000001")
    c.setFont("Helvetica-Bold", 7.5)
    y = H - 95
    for x, t in ((40, "Date"), (90, "Narration")): c.drawString(x, y, t)
    c.drawString(300, y, "Chq./Ref.No."); c.drawString(385, y, "Value Dt")
    for x, t in ((480, "Withdrawal Amt."), (530, "Deposit Amt."), (580, "Closing Balance")): c.drawRightString(x, y, t)
    return y - 14
def footer():
    c.setFont("Helvetica", 7); c.drawCentredString(W / 2, 30, f"Page {page}  -  This is a computer generated statement")
y = header()
c.setFont("Helvetica", 7.5)
for d, narr, ref, vd, wd, dep, bal in rows:
    need = 11 * len(narr)
    if y - need < 60:
        footer(); c.showPage(); page += 1; y = header(); c.setFont("Helvetica", 7.5)
    c.drawString(40, y, d); c.drawString(300, y, ref); c.drawString(385, y, vd)
    if wd: c.drawRightString(480, y, wd)
    if dep: c.drawRightString(530, y, dep)
    c.drawRightString(580, y, bal)
    for k, part in enumerate(narr): c.drawString(90, y - 11 * k, part)
    y -= need + 3
c.setFont("Helvetica-Bold", 8); c.drawString(40, y - 10, "Closing balance  1,55,604.50")
footer()
c.save()
