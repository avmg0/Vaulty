"""A made-up CAMS+KFintech consolidated account statement, laid out like the real ones (columns
placed by position, a header on every page, a scheme that runs onto the next page). Names, PAN
and folios are invented."""
import sys
from reportlab.lib.pagesizes import A4
from reportlab.pdfgen import canvas

out = sys.argv[1]
c = canvas.Canvas(out, pagesize=A4)
W, H = A4
y = H - 40
page = 1

def header():
    global y
    c.setFont("Helvetica-Bold", 11); c.drawCentredString(W/2, H-30, "Consolidated Account Statement")
    c.setFont("Helvetica", 8); c.drawCentredString(W/2, H-42, "01-Apr-2025 To 21-Oct-2026")
    c.drawRightString(W-30, H-30, f"Page {page}")
    y = H - 70

def line(text, x=40, font="Helvetica", size=8):
    global y, page
    if y < 60:
        c.showPage(); page += 1; header()
    c.setFont(font, size); c.drawString(x, y, text); y -= 12

def row(date, desc, amount="", units="", price="", bal=""):
    global y, page
    if y < 60:
        c.showPage(); page += 1; header()
    c.setFont("Helvetica", 7.5)
    c.drawString(40, y, date); c.drawString(100, y, desc)
    for x, v in ((380, amount), (440, units), (495, price), (560, bal)):
        if v: c.drawRightString(x, y, v)
    y -= 11

header()
line("Email Id: john.doe@example.com", size=8)
line("JOHN DOE", font="Helvetica-Bold", size=9)
line("12 Example Road, Pune 411001", size=8)
y -= 6
line("PORTFOLIO SUMMARY", font="Helvetica-Bold", size=9)
line("Mutual Fund                      Cost Value        Market Value", size=8)
line("HDFC Mutual Fund                 2,15,000.00        2,65,957.33", size=8)
line("Axis Mutual Fund                   48,500.00          51,812.40", size=8)
line("Total                            2,63,500.00        3,17,769.73", size=8)
y -= 8
line("Date        Transaction                                         Amount        Units        Price     Unit Balance", font="Helvetica-Bold", size=7.5)

# --- HDFC
line("HDFC Mutual Fund", font="Helvetica-Bold", size=9)
line("Folio No: 1234567 / 89        PAN: ABCDE1234F        KYC: OK   PAN: OK", size=8)
line("JOHN DOE", size=8)
line("B205G-HDFC Flexi Cap Fund - Direct Plan - Growth (Non-Demat) - ISIN: INF179K01UT0(Advisor: DIRECT) Registrar : CAMS", size=7.5)
line("Opening Unit Balance: 1,234.567", size=7.5)
for d, amt, u, p, b in [("10-Apr-2025", "5,000.00", "3.456", "1,446.7800", "1,238.023"),
                         ("10-May-2025", "5,000.00", "3.301", "1,514.6800", "1,241.324"),
                         ("10-Jun-2025", "5,000.00", "3.212", "1,556.6600", "1,244.536")]:
    row(d, "Purchase-SIP - Instalment No 12/60 - via Internet", amt, u, p, b)
    row(d, "*** Stamp Duty ***", "0.25")
row("15-Sep-2025", "Redemption - via Internet", "(20,000.00)", "(12.345)", "1,620.1100", "1,232.191")
for i in range(60):
    m = ["Oct-2025","Nov-2025","Dec-2025","Jan-2026","Feb-2026","Mar-2026","Apr-2026","May-2026","Jun-2026"][i % 9]; d0 = 10 + i // 9
    yr_bump = "" 
    row(f"{d0:02d}-{m}", "Purchase-SIP - Instalment No %d/60" % (16 + i), "5,000.00", "2.990", "1,672.2400", f"{1235.181 + 2.99*(i+1):,.3f}")
close = 1232.191 + 60 * 2.99
line(f"Closing Unit Balance: {close:,.3f}        NAV on 21-Oct-2026: INR 2,045.6780        Total Cost Value: 3,15,000.00        Market Value on 21-Oct-2026: INR {close * 2045.678:,.2f}", size=7)
y -= 6

# --- HDFC liquid, fully redeemed
line("Folio No: 1234567 / 89        PAN: ABCDE1234F        KYC: OK   PAN: OK", size=8)
line("JOHN DOE", size=8)
line("H03-HDFC Liquid Fund - Direct Plan - Growth Option - ISIN: INF179KB1HP9(Advisor: DIRECT) Registrar : CAMS", size=7.5)
line("Opening Unit Balance: 10.000", size=7.5)
row("02-Jan-2026", "Redemption - via Internet", "(48,123.40)", "(10.000)", "4,812.3400", "0.000")
line("Closing Unit Balance: 0.000        NAV on 21-Oct-2026: INR 4,951.2200        Total Cost Value: 0.00        Market Value on 21-Oct-2026: INR 0.00", size=7)
y -= 6

# --- Axis
line("Axis Mutual Fund", font="Helvetica-Bold", size=9)
line("Folio No: 9101112131        PAN: ABCDE1234F        KYC: OK   PAN: OK", size=8)
line("JOHN DOE", size=8)
line("128EFDGG-Axis ELSS Tax Saver Fund - Direct Growth - ISIN: INF846K01EW2(Advisor: DIRECT) Registrar : KFINTECH", size=7.5)
line("Opening Unit Balance: 0.000", size=7.5)
row("05-Jul-2025", "Purchase - via Internet", "25,000.00", "253.807", "98.5000", "253.807")
row("05-Jul-2025", "*** Stamp Duty ***", "1.25")
row("05-Aug-2025", "Switch In - From Axis Liquid Fund", "23,500.00", "230.392", "102.0000", "484.199")
row("15-Mar-2026", "IDCW Paid - Rs. 2.50 per unit", "1,210.50")
line("Closing Unit Balance: 484.199        NAV on 21-Oct-2026: INR 107.0063        Total Cost Value: 48,500.00        Market Value on 21-Oct-2026: INR 51,812.40", size=7)
c.save()
