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

### Fixed
- Undoing twice after editing a transaction (add, edit, undo, undo) is no longer refused.
- Statement import no longer reads a date or reference in an amount column as an amount.
- Card and account pictures would have been blocked in installed builds.

### Security
- Stricter limits on what the app window can load and do; files are only read or written after
  you choose them.
- Size limits on everything the interface sends to the app.
- Threat model (docs/THREAT_MODEL.md) and a security policy (SECURITY.md).
