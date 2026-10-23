# Changelog

What changed in each version, for people using Vaulty. Newest first.

## Unreleased

### Added
- Hidden transactions: kept in a locked section and shown as XXXXX elsewhere, amount only.
- Budgets per category, with sub-categories.
- Recurring payments and income: reminders, mark as paid, skip, optional auto-pay.
- Bank and card statement import (CSV and Excel), with duplicate detection and saved column layouts.
- Categories learned from your choices, and rules you can edit.
- Quick add (Ctrl/Cmd+J): type "250 swiggy yesterday".
- Command palette (Ctrl/Cmd+K), bulk edits, more filters, splits and tags.
- Password-protected backups and a printable recovery sheet.
- Display formats: date order, first day of the week, financial year, negative amounts, decimals.
- Demo mode with sample data.
- Updates: check from Settings, or once a week if you turn it on. Installs are signature-checked
  and admin-only.
- Auto-lock after a set idle time (enforced by the app), and a notice of wrong passwords since
  you last signed in. Easy-to-guess passwords are refused.
- Errors from the app shown in your language.
- Keyboard shortcuts you can change in Settings.
- Calendar: bills, income, card due dates, holidays (with "pay by" when a bank holiday falls on a
  due date), your own events, and .ics import and export.
- Reminders as system notifications, with an optional morning reminder while Vaulty is locked.
- Reports: spending by category and month, a cash-flow forecast, and "safe to spend".
- Lend and borrow: track money with people, including shares of a split bill.
- Subscriptions spotted in your transactions, and alerts when a regular charge goes up.
- To-do list: link a to-do to a bill, transaction, person or event; it ticks itself off when the
  bill is marked paid, and due dates show on the calendar and as reminders.
- Low-balance reminders: set a minimum on a bank or cash account and get reminded when it drops
  below, or when a bill in the next two weeks would take it there.
- Reports: save as an Excel workbook, or print or save as a PDF.
- Family helpers: choose three people (family members on this device, or printed cards for others),
  any two of whom can get you back in if you lose your password and recovery key.
- Nominee: name someone who, with their printed card and one helper after a waiting period you can
  cancel, sees a read-only "In case of emergency" summary of your money and a letter from you.
- Documents: passports, IDs, insurance policies, property papers and wills, with encrypted scans,
  where each original is kept, and reminders before they expire. Included in encrypted backups.
- Credit card statements: what each statement asks for, the minimum due, what's paid and what's
  left, with a history; enter the bank's figures when they differ. Reminders use the statement.
- Card benefits (rewards, spend milestones, perks) and "best card for today" for a purchase.
- Menu bar (tray) icon with quick add, open, lock and quit; an optional shortcut to quick add from
  any app; and an option to keep Vaulty running when its window is closed.
- Investments: mutual funds, shares, deposits and gold, with value, gain, XIRR and allocation.
  Read the CAMS or KFintech consolidated account statement (password-protected PDF) to bring in
  every folio and transaction; reading a later statement adds only what's new.
- Bank and card statements as PDF (including password-protected ones): the transaction table is read
  by column position, narrations over two lines are joined, and repeated headers and footers are left out.
- Daily mutual fund prices from AMFI's public NAV list (opt-in, in Settings or on Investments):
  fetched once a day, nothing about you is sent, and only newer prices replace older ones.
- Projects: keep a renovation, wedding, trip or business set-up together, with sections and
  budgets, bill items (qty × rate, GST, who from; every heading can be renamed), payments (advances and instalments, how paid,
  the name the bank showed), what's still owed to each person, your own columns, templates, and
  Excel or PDF copies. Payments can be added to transactions or linked to bank statement rows.
  Bills and receipts (PDFs and photos) are kept with a project, encrypted, for an item or payment.
  A PDF bill or a photo of one can be read into items (seller, GSTIN, bill no., date, each line,
  GST), checked against the bill's total, and added in one go. Photos are read by the system's
  own text recognition (Apple Vision on macOS, Windows OCR, Tesseract on Linux), offline.
  Costs can be split with the people in a project (equal or weighted shares, who paid what, the
  fewest payments to settle up), and a project can be shared as a password-protected file that
  updates the other person's copy when sent back. What each person owes can be recorded in Lend &
  borrow, with your spending adjusted to your own share; recording again adds only what changed.
  An item's warranty date adds it to Documents with a reminder before it ends, and a project that
  improves a property shows its cost by financial year, for capital gains when the property is sold.
- Tax planner keeps each year's figures. Categories can be linked to salary, other income or a
  deduction so the year's transactions (and ELSS fund purchases) fill them in. Record TDS, advance
  and self-assessment tax to see what's still to pay or coming back, with India's advance-tax
  instalments (15 Jun, 15 Sep, 15 Dec, 15 Mar) on the calendar and as reminders. The HRA
  calculator works out the exempt part of house rent allowance and prints a rent receipt for each
  month. A Form 16 PDF (password-protected too) fills in salary, exemptions, deductions, the
  regime and TDS for each quarter, next to what the plan already has, so you pick what to use.
- Settings in sections (profile, security, language, appearance, reminders, backups, family
  sharing, shortcuts, this computer, market prices), each also reachable from the command palette.

### Fixed
- Undoing twice after editing a transaction (add, edit, undo, undo) is no longer refused.
- Statement import no longer reads a date or reference in an amount column as an amount.
- Card and account pictures would have been blocked in installed builds.

### Security
- Stricter limits on what the app window can load and do; files are only read or written after
  you choose them.
- Size limits on everything the interface sends to the app.
- Threat model (docs/THREAT_MODEL.md) and a security policy (SECURITY.md).
