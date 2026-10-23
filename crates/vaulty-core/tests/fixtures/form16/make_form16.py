"""Made-up Form 16s laid out like real ones: Part A as TRACES makes it (employer and employee
boxes, TAN and PANs, assessment year, TDS by quarter) and Part B as employers fill it in (salary,
exemptions, deductions under section 16 and Chapter VI-A, the regime question). Some labels
wrap over two lines with the amounts on the first, as they often do. Every name, PAN, TAN and
number is invented.

  form16-old.pdf     old regime, HRA, 80C/80D/NPS, home loan loss, Indian digit grouping
  form16-new.pdf     new regime, only salary and employer NPS, locked with password "jane01011990"
"""
import subprocess, sys
from reportlab.lib.pagesizes import A4
from reportlab.pdfgen import canvas

W, H = A4
L, R1, R2 = 40, 470, 555   # label column, gross amount, deductible / only amount

def part_a(c, quarters, total, ay, regime_note=""):
    c.setFont("Helvetica-Bold", 12); c.drawCentredString(W/2, H-40, "FORM NO. 16")
    c.setFont("Helvetica", 7); c.drawCentredString(W/2, H-52, "[See rule 31(1)(a)]")
    c.setFont("Helvetica-Bold", 9); c.drawCentredString(W/2, H-66, "PART A")
    c.setFont("Helvetica", 7)
    c.drawCentredString(W/2, H-78, "Certificate under Section 203 of the Income-tax Act, 1961 for tax deducted at source on salary paid to an employee")
    c.drawString(L, H-96, "Certificate No.  ABCDEFG          Last updated on  15-May-2027")
    c.setFont("Helvetica-Bold", 7)
    c.drawString(L, H-114, "Name and address of the Employer/Specified Bank"); c.drawString(320, H-114, "Name and address of the Employee/Specified senior citizen")
    c.setFont("Helvetica", 7)
    c.drawString(L, H-126, "EXAMPLE TECH PRIVATE LIMITED"); c.drawString(320, H-126, "JANE DOE")
    c.drawString(L, H-136, "4TH FLOOR, 12 EXAMPLE ROAD, PUNE - 411001"); c.drawString(320, H-136, "FLAT 4B, 12 EXAMPLE ROAD, PUNE - 411001")
    c.setFont("Helvetica-Bold", 7)
    c.drawString(L, H-156, "PAN of the Deductor"); c.drawString(180, H-156, "TAN of the Deductor"); c.drawString(320, H-156, "PAN of the Employee/Specified senior citizen")
    c.setFont("Helvetica", 7)
    c.drawString(L, H-166, "AAACE1234F"); c.drawString(180, H-166, "PNEE12345E"); c.drawString(320, H-166, "ABCPD1234K")
    c.setFont("Helvetica-Bold", 7)
    c.drawString(L, H-186, "CIT (TDS)"); c.drawString(320, H-186, "Assessment Year"); c.drawString(420, H-186, "Period with the Employer")
    c.setFont("Helvetica", 7)
    c.drawString(L, H-196, "The Commissioner of Income Tax (TDS), Pune")
    c.drawString(320, H-196, ay); c.drawString(420, H-196, "From  01-Apr-2026   To  31-Mar-2027")
    c.setFont("Helvetica-Bold", 7)
    c.drawString(L, H-216, "Summary of amount paid/credited and tax deducted at source thereon in respect of the employee")
    c.drawString(L, H-230, "Quarter(s)"); c.drawString(110, H-230, "Receipt Numbers of original quarterly statements of TDS")
    c.drawRightString(400, H-230, "Amount paid/credited"); c.drawRightString(480, H-230, "Amount of tax deducted (Rs.)"); c.drawRightString(R2, H-230, "Amount of tax deposited/remitted (Rs.)")
    c.setFont("Helvetica", 7)
    y = H-244
    for q, rec, paid, tds in quarters:
        c.drawString(L, y, q); c.drawString(110, y, rec); c.drawRightString(400, y, paid); c.drawRightString(480, y, tds); c.drawRightString(R2, y, tds); y -= 12
    c.setFont("Helvetica-Bold", 7)
    c.drawString(L, y, "Total (Rs.)"); c.drawRightString(400, y, total[0]); c.drawRightString(480, y, total[1]); c.drawRightString(R2, y, total[1])
    c.setFont("Helvetica", 6); c.drawString(L, 40, "Page 1 of 3")
    c.showPage()

def part_b(c, rows, regime_answer):
    c.setFont("Helvetica-Bold", 9); c.drawCentredString(W/2, H-40, "PART B (Annexure)")
    c.setFont("Helvetica-Bold", 7); c.drawCentredString(W/2, H-52, "Details of Salary Paid and any other income and tax deducted")
    c.setFont("Helvetica", 7)
    c.drawString(L, H-70, f"Whether opting out of taxation u/s 115BAC(1A)?"); c.drawString(300, H-70, regime_answer)
    c.setFont("Helvetica-Bold", 7); c.drawRightString(R2, H-84, "Rs.")
    y = H-98
    for row in rows:
        label, *nums = row
        lines = label.split("\n")
        bold = lines[0][:3].strip().rstrip(".").isdigit()
        c.setFont("Helvetica-Bold" if bold else "Helvetica", 7)
        c.drawString(L + (0 if bold else 12), y, lines[0])
        if len(nums) == 2: c.drawRightString(R1, y, nums[0]); c.drawRightString(R2, y, nums[1])
        elif nums: c.drawRightString(R2, y, nums[0])
        for extra in lines[1:]:
            y -= 10; c.drawString(L + (0 if bold else 12), y, extra)
        y -= 13
        if y < 60:
            c.setFont("Helvetica", 6); c.drawString(L, 40, "Page 2 of 3"); c.showPage(); y = H-50
    c.showPage()

def old(path):
    c = canvas.Canvas(path, pagesize=A4)
    part_a(c, [("Q1", "QVWERTYU", "3,75,000.00", "18,500.00"), ("Q2", "QVWERTYI", "3,75,000.00", "18,500.00"),
               ("Q3", "QVWERTYO", "3,75,000.00", "18,500.00"), ("Q4", "QVWERTYP", "3,75,000.00", "22,500.00")],
           ("15,00,000.00", "78,000.00"), "2027-28")
    part_b(c, [
        ("1. Gross Salary",),
        ("(a) Salary as per provisions contained in section 17(1)", "14,52,000.00"),
        ("(b) Value of perquisites under section 17(2) (as per Form No. 12BA, wherever applicable)", "48,000.00"),
        ("(c) Profits in lieu of salary under section 17(3) (as per Form No. 12BA, wherever applicable)", "0.00"),
        ("(d) Total", "15,00,000.00"),
        ("2. Less: Allowances to the extent exempt under section 10",),
        ("(a) Travel concession or assistance under section 10(5)", "0.00"),
        ("(e) House rent allowance under section 10(13A)", "2,10,000.00"),
        ("(h) Total amount of exemption claimed under section 10\n[2(a)+2(b)+2(c)+2(d)+2(e)+2(f)+2(g)]", "2,10,000.00"),
        ("3. Total amount of salary received from current employer [1(d)-2(h)]", "12,90,000.00"),
        ("4. Less: Deductions under section 16",),
        ("(a) Standard deduction under section 16(ia)", "50,000.00"),
        ("(b) Entertainment allowance under section 16(ii)", "0.00"),
        ("(c) Tax on employment under section 16(iii)", "2,500.00"),
        ("5. Total amount of deductions under section 16 [4(a)+4(b)+4(c)]", "52,500.00"),
        ("6. Income chargeable under the head \"Salaries\" [(3+1(e)-5]", "12,37,500.00"),
        ("7. Add: Any other income reported by the employee under as per section 192 (2B)",),
        ("(a) Income (or admissible loss) from house property reported by employee\noffered for TDS", "-2,00,000.00"),
        ("(b) Income under the head Other Sources offered for TDS", "12,000.00"),
        ("8. Total amount of other income reported by the employee [7(a)+7(b)]", "-1,88,000.00"),
        ("9. Gross total income (6+8)", "10,49,500.00"),
        ("10. Deductions under Chapter VI-A", "Gross Amount", "Deductible Amount"),
        ("(a) Deduction in respect of life insurance premia, contributions to provident\nfund etc. under section 80C", "1,65,000.00", "1,50,000.00"),
        ("(b) Deduction in respect of contribution to certain pension funds under section 80CCC", "0.00", "0.00"),
        ("(c) Deduction in respect of contribution by taxpayer to pension scheme under\nsection 80CCD (1)", "0.00", "0.00"),
        ("(d) Total deduction under section 80C, 80CCC and 80CCD(1)", "1,65,000.00", "1,50,000.00"),
        ("(e) Deductions in respect of amount paid/deposited to notified pension scheme\nunder section 80CCD (1B)", "50,000.00", "50,000.00"),
        ("(f) Deduction in respect of contribution by Employer to pension scheme under\nsection 80CCD (2)", "60,000.00", "60,000.00"),
        ("(g) Deduction in respect of health insurance premia under section 80D", "18,400.00", "18,400.00"),
        ("(h) Deduction in respect of interest on loan taken for higher education under section 80E", "0.00", "0.00"),
        ("(j) Total Deduction in respect of donations to certain funds, charitable\ninstitutions, etc. under section 80G", "5,000.00", "2,500.00"),
        ("(k) Amount deductible under section 80TTA in respect of interest on deposit in\nsavings account", "12,000.00", "10,000.00"),
        ("11. Aggregate of deductible amount under Chapter VI-A", "2,90,900.00"),
        ("12. Total taxable income (9-11)", "7,58,600.00"),
        ("13. Tax on total income", "64,220.00"),
        ("14. Rebate under section 87A, if applicable", "0.00"),
        ("15. Surcharge, wherever applicable", "0.00"),
        ("16. Health and education cess", "2,569.00"),
        ("17. Tax payable (13+15+16-14)", "66,789.00"),
        ("18. Less: Relief under section 89 (attach details)", "0.00"),
        ("19. Net tax payable (17-18)", "66,789.00"),
    ], "Yes")
    c.save()

def new(path):
    c = canvas.Canvas(path, pagesize=A4)
    part_a(c, [("Q1", "QWXYZABC", "450000.00", "21000.00"), ("Q2", "QWXYZABD", "450000.00", "21000.00"),
               ("Q3", "QWXYZABE", "450000.00", "21000.00"), ("Q4", "QWXYZABF", "450000.00", "24000.00")],
           ("1800000.00", "87000.00"), "2027-28")
    part_b(c, [
        ("1. Gross Salary",),
        ("(a) Salary as per provisions contained in section 17(1)", "1800000.00"),
        ("(d) Total", "1800000.00"),
        ("2. Less: Allowances to the extent exempt under section 10",),
        ("(h) Total amount of exemption claimed under section 10", "0.00"),
        ("4. Less: Deductions under section 16",),
        ("(a) Standard deduction under section 16(ia)", "75000.00"),
        ("10. Deductions under Chapter VI-A", "Gross Amount", "Deductible Amount"),
        ("(f) Deduction in respect of contribution by Employer to pension scheme under\nsection 80CCD (2)", "90000.00", "90000.00"),
        ("12. Total taxable income (9-11)", "1635000.00"),
        ("19. Net tax payable (17-18)", "107000.00"),
    ], "No")
    c.save()

if __name__ == "__main__":
    d = sys.argv[1] if len(sys.argv) > 1 else "."
    old(f"{d}/form16-old.pdf")
    new(f"{d}/form16-new-plain.pdf")
    subprocess.run(["qpdf", "--encrypt", "jane01011990", "owner-x", "256", "--", f"{d}/form16-new-plain.pdf", f"{d}/form16-new.pdf"], check=True)
    import os; os.remove(f"{d}/form16-new-plain.pdf")
