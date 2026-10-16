# Vaulty roadmap

Offline-first, open-source personal finance planner for macOS, Windows, Linux, iOS and Android.
Stack: Tauri v2, Rust (`crates/vaulty-core`), React + TypeScript, shadcn/ui, Lucide icons.

---

## Decisions to settle before building further

These are cheap to get right now and expensive to change once real data exists.

### 1. Encryption: use AES-256 with SQLCipher, not "SHA-256 vs SHA-512"
SHA-256 and SHA-512 are **hash functions**. They can't encrypt or decrypt data, so offering a choice between them for encryption isn't meaningful. Recommended instead:

- **Encrypt the database at rest with SQLCipher (AES-256).** In `rusqlite` this is a feature flag (`bundled-sqlcipher`).
- **Derive the key from the master password with Argon2id.** Argon2 is already used for the password check.
- Don't offer an algorithm picker. One audited default is safer than a menu of options. If a "choice" is wanted for marketing, offer an Argon2 strength level (memory and time cost): Standard or Paranoid.
- Backups are encrypted with the same key before they leave the device, including to Google Drive.

### 2. Store money as integers (minor units), not floats ✅ (done)
The schema currently uses `REAL`, which causes rounding drift (0.1 + 0.2 ≠ 0.3).

- Switch to `amount_minor INTEGER` (paise or cents) plus a `currency` column on each row.
- Use `rust_decimal` for maths in Rust.
- **Do this next, before any other feature.**

### 3. Family profiles & authentication ✅ (first version built)
Each family member has their own profile and their own database file. Signing in opens only that member's data.

**Adding someone**
1. An admin adds them (name, colour, role).
2. Vaulty shows a one-time **setup code** (8 characters, valid 7 days).
3. The new member picks their profile, enters the code and **chooses their own password**.

The code stops anyone else at the computer from claiming an unclaimed profile.

**Roles**
- **Admin:** can add, reset and remove members. There must always be at least one admin.
- **Member:** their own private finances.
- **Child:** placeholder for kids' allowance controls (phase 4).

| Layer | Status | Notes |
|---|---|---|
| Password per user (Argon2id) | ✅ | Minimum 8 characters. After 5 misses, lockout grows 30s → 60s → … → 15 min. |
| Authenticator-app 2FA (TOTP, RFC 6238) | ✅ | QR plus manual key, confirmed with a first code. Each code is accepted only once. The secret is encrypted (XChaCha20-Poly1305) with a key derived from the user's password, so the registry file alone doesn't reveal it. |
| Backup codes | ✅ | 10 one-time codes, shown once; copy, download or print. They can be regenerated. |
| Biometric unlock | ✅ desktop / ⏳ mobile | Touch ID (macOS), Windows Hello, polkit (Linux). Per user, per device. Replaces password **and** 2FA on that device, the same way phone banking apps behave. |
| Admin reset | ✅ | Clears a member's password, 2FA and biometrics and issues a new setup code. Their data is kept. |
| Security changes need the password again | ✅ | Changing the password, turning 2FA on or off, new backup codes, turning on biometrics. |
| Auto-lock timer | ⏳ phase 1 | |

**Known limits, to fix together with database encryption (decision 1):**
- **Keychain item not tied to biometrics:** the biometric token in the OS keychain can be read by Vaulty without a prompt, so today the app enforces the biometric prompt. The plan is to store the token with OS access control (macOS `kSecAccessControlBiometryCurrentSet`, Android Keystore `setUserAuthenticationRequired`).
- **Member data files aren't encrypted yet:** someone with access to the files could read them. With SQLCipher, each member's database is encrypted with a key derived from their password, and biometric unlock releases that key.
- **Admin reset vs encryption:** once databases are encrypted, an admin reset can't recover a member's data. Each member will get a printable **recovery key** at setup instead.

### 3b. Family sharing ✅ (first version built)
Members can let others in the family see chosen expenses. They never have to give up their privacy to do it.

- **Every transaction has a visibility:**
  - **Private:** only you. This is the default.
  - **Shared:** you plus the people you pick, e.g. a child picks both parents.
  - **Public:** everyone in the family who has turned Family sharing on.
- **Consent on both sides:** each person turns Family sharing on in Settings.
  - Until they do, nothing of theirs is published and they see nothing from others.
  - Turning it off withdraws everything they shared, immediately.
- **The owner stays in control:**
  - change visibility per transaction (click the icon on any row)
  - set a default for new transactions
  - choose who sees "Shared" items
  - Removing someone from that list hides those items from them at once.
- **Family view** (Family → Family view): tabs for **All / Private / Shared / Public**, a month picker, per-person totals and a filter by person.
  - Example: parents A and B see children C's and D's Shared and Public expenses alongside their own.
- **How it's stored:** each member's own database stays private. Shared and Public items are copied to a separate device-level `household.db`, holding only date, amount, merchant, category and account name. Notes, tags and receipts are never copied.
- **Later:**
  - the same visibility on bills, recurring payments, goals and investments
  - a per-account default (e.g. "pocket money account is Shared")
  - a shared household budget
- **With encryption (decision 1):** `household.db` gets its own key, wrapped once per member who has sharing on. Withdrawing re-keys it.

### 4. Online mode = encrypted backup first, true sync later
- **Phase 1 of online mode:** an encrypted snapshot backup to Google Drive's private `appDataFolder`, with restore and "merge newer" on restore.
- **Real multi-device sync** needs every change recorded as an event (an append-only change log or CRDT) so it can be merged safely. Design the tables with `updated_at` and soft deletes now, so sync can be added later without a rewrite.

### 5. Never store full card numbers
- Detect the card network (Visa, Mastercard, AmEx, Discover, RuPay) and run a Luhn check **on the input only**.
- Store just the network and the last 4 digits. That covers everything the spec asks for without holding sensitive data.

### 6. Screenshot protection is per-platform
| Platform | How |
|---|---|
| Windows | `SetWindowDisplayAffinity` |
| macOS | `NSWindow.sharingType = .none` |
| Android | `FLAG_SECURE` |
| iOS | Blur on background; recording can only be detected |
| Linux | No reliable equivalent. Hide the toggle there. |

### 7. Countries and taxes: region packs (data, not code)
Vaulty must work in any country. Tax rules change every year and differ by country, state and city, so they live in **region packs**: versioned data files (TOML or JSON) in the repo, maintained by the community. They are never hard-coded.

- **What a region pack contains:**
  - currency, number grouping (lakh/crore vs thousand/million), date format, first day of week
  - financial year (India Apr–Mar, US Jan–Dec, UK 6 Apr–5 Apr, Australia Jul–Jun)
  - income-tax regimes and slabs per tax year (e.g. India old vs new regime)
  - deductions and allowances (India 80C/80D/HRA, US 401(k)/IRA/standard deduction, UK personal allowance/pension/ISA, Canada RRSP/TFSA…)
  - sales tax: GST / VAT / sales tax, with rates per state, province or city where needed
  - local savings and investment instruments (PPF, EPF, NPS, SGB, 401(k), ISA, Super…) with their rules (lock-ins, limits)
  - tax deadlines and bank holidays, shown on the calendar
  - bank statement import templates for that country
  - business tax forms and categories (see decision 8)
- **Every rule records its tax year, a link to the official source and a "last checked" date.** The UI labels results "Estimate. Rules last checked on …" and never presents them as tax advice.
- **Per person, not per device:** the household has a default country; each member can override it (e.g. someone working abroad).
- **Starter packs:** India, US (federal; states over time), UK, Canada, Australia, Singapore, UAE, and a generic EU VAT pack. Without a pack, everything except tax estimates still works.
- **Tests:** each pack ships with worked examples (income X → tax Y) that run in CI, so a wrong rule fails the build.

### 8. Small business: business workspaces on a double-entry ledger (ledger ✅, workspaces next)
A member can run a small business inside Vaulty without mixing it with personal money.

- **Business workspace:** its own accounts, categories, clients, vendors and reports, separate from personal finances. Money moving between them is recorded as "owner paid in" (capital) or "owner took out" (drawings).
- **Double-entry under the hood:** every transaction becomes balanced ledger entries. Users still just "add an expense"; they never see debits and credits. In return:
  - receivables and payables (who owes what, and for how long)
  - GST/VAT collected vs paid, correctly
  - profit & loss, balance sheet, cash flow
  - bank reconciliation an accountant can trust
- **Do it early:** switch the core to a double-entry ledger in Phase 0, together with integer money (decision 2). It's far cheaper now than after real data exists.
- **Business tax mapping comes from region packs:**
  - India: GST returns summary with input tax credit, TDS deducted by clients reconciled with Form 26AS, presumptive taxation (44AD/44ADA)
  - US: Schedule C categories, quarterly estimated tax, 1099 tracking
  - UK: self-assessment (SA103) boxes, VAT return summary
  - others as packs are added

---

## Phases

### Phase 0: Foundations (in progress)
- [x] Tauri v2 workspace, `vaulty-core` crate, migrations, tests
- [x] Master password (Argon2), lock screen, guarded commands
- [x] shadcn/ui theme: light, dark and system, accent colours, Lucide icons
- [x] App shell: collapsible sidebar, page transitions, keyboard shortcuts, hide balances
- [x] Dashboard, Accounts, Transactions (basic)
- [x] Family profiles: per-user database, setup codes, roles, admin reset and remove
- [x] Per-user 2FA (authenticator app + backup codes) and biometric unlock (desktop)
- [x] Family sharing: Private / Shared / Public visibility, two-sided consent, Family view
- [x] Money as integer minor units (decision 2)
- [x] Double-entry ledger core (decision 8): entries and balanced postings, balances derived from postings, existing data converted by migration 0004, backup format v2 (v1 still imports)
- [ ] Business workspaces on top of the ledger: create and switch workspace, owner capital and drawings (decision 8)
- [ ] Region pack format and loader, plus the India pack as the first one (decision 7)
- [ ] SQLCipher encryption at rest (decision 1)
- [ ] Settings persisted in DB: currency, date and time format, first day of week, fiscal year start, decimals, negative format
- [ ] CI: `cargo test`, `tsc`, and Tauri builds for all three desktop OSes (GitHub Actions)

### Phase 1: Core tracking (MVP you'd actually use daily)
- Transactions:
  - full edit
  - tags and notes
  - smart filters (category, merchant, tags, account, card only, recurring only)
  - multi-select bulk actions
  - pagination or virtualised list
- Categories and sub-categories, with per-category budgets
- Transfers between accounts (two linked rows, so they don't count as spending)
- Auto-lock timer; theme and preferences stored per profile
- Recovery key per member (printable), required once encryption lands
- Encrypted backup file: export, restore, print recovery sheet
- Command palette (Ctrl/Cmd+K) and a hotkey editor

### Phase 2: Planning & bills
- **Recurring payments:**
  - recurrence rules (daily, weekly, monthly, yearly, custom; use RRULE)
  - generates upcoming instances
  - mark as paid creates the transaction
- **Credit cards:**
  - ✅ Cards page for credit, debit, prepaid/forex, gift and other cards: animated (tilt, glare, flip, stack), network detection, last-4-only storage, expiry
  - ✅ card backgrounds: preset finishes, solid colour, 2–3 colour gradient, or your own image (cropped, dimmable), with text colour and engraved-line options
  - ✅ limit, available credit, credit-used ring, statement and due day with countdown
  - billing cycle and due date reminders
  - monthly statement generation ("Statement for Dec 2025")
  - minimum due, paid or unpaid, late-fee warning
  - EMIs and taxes on the card
- **Calendar:**
  - month and day views of bills, recurring items, card dues and investment events
  - day totals
  - `.ics` import and export
- **To-do:** priority, status and due date; can link to a transaction, bill or investment
- **Statement import:**
  - CSV and XLSX with a column-mapping wizard
  - mappings saved per bank as templates
  - duplicate detection (same date, amount and merchant within ±1 day)
- **Exports:**
  - XLSX with sheets for Transactions, Recurring, Cards, Investments, Tasks and Summary
  - PDF monthly report (summary, charts and detail)
  - JSON
- **Reports:**
  - monthly summary and category breakdown
  - trends
  - budget vs actual
  - year-over-year comparison
  - custom date range
- **Notifications:** bill due, card due, budget threshold, low balance (OS notifications via `tauri-plugin-notification`)

### Phase 3: Wealth
- **Investments:**
  - holdings (buy price, quantity, current value)
  - events (SIP, dividend, bonus, maturity)
  - XIRR
  - allocation chart
- Goals and calculators: emergency fund, retirement, home, education, vacation, debt payoff (snowball and avalanche)
- Insurance policies: premium schedule, renewal reminders, nominees, attached policy PDF
- Net-worth timeline (monthly snapshots)
- Journal:
  - monthly financial journal
  - decision log
  - investment thesis
  - bill negotiation history
  - notes on transactions
- Documents vault with a built-in PDF viewer (pdf.js); files encrypted on disk
- Anomaly detection, rule-based first:
  - spend > 2.5σ above the category's 6-month mean
  - a new merchant with a large amount
  - a duplicate charge
- PDF statement import:
  - text-based PDFs via `pdf-extract`, plus per-bank parsers
  - "convert PDF to CSV" falls out of this

### Phase 4: Business, family, online
- **Business** (workspaces and ledger from decision 8; tax rules from region packs):
  - quotes → invoices → payments, with GST/VAT lines; recurring invoices and late-payment reminders
  - payment QR on invoices: UPI (India), EPC/SEPA QR (EU), PayNow (Singapore), Pix (Brazil)
  - client and vendor directory; receivables and payables ageing
  - profit & loss, balance sheet, cash flow; bank reconciliation
  - tax summaries per region pack (GST/VAT returns, Schedule C, SA103…) and a tax-filing export
  - exports for accountants: Tally (India), QuickBooks / Xero CSV, Excel, PDF
  - receipt capture with on-device OCR
  - time tracking for freelancers, billed straight into invoices
  - mileage log and home-office expenses
  - tax-deductible donation tracking
- **Family:** (profiles and roles are already done; these extend them)
  - a shared household budget DB that members can opt into
  - kids' allowance accounts (virtual balances that a parent tops up)
  - finer permissions on shared data (view or edit)
- **Online mode:**
  - Google Drive encrypted backup and auto-backup
  - FX rates for multi-currency
  - update checker (`tauri-plugin-updater`)
  - calendar sync (Google, Apple, Outlook) last

### Phase 5: Mobile
- iOS and Android builds from the same codebase (Tauri v2 mobile)
- Responsive layout: the sidebar becomes a bottom tab bar
- Biometric unlock and quick-add widget

---

## Suggested extra features
- **Auto-categorisation rules:** e.g. "merchant contains SWIGGY → Food". The app learns when the user re-categorises, all offline.
- **Subscription detector:** finds repeating charges and suggests turning them into recurring items.
- **Split expenses:** split one transaction across categories or people, and track who owes what.
- **Cash-flow forecast:** a projected balance for the next 30, 60 and 90 days from recurring items and card dues.
- **Receipts:** attach a photo or PDF to a transaction (OCR could come later).
- **Audit log and undo:** every change is recorded, which is also the groundwork for sync.
- **Envelope or zero-based budgeting mode** as an alternative to category budgets.
- **"Safe to spend" today:** balance minus upcoming bills before the next income.
- **Open data format:** documented JSON export so users are never locked in, which matters for an open-source app.

## Feature backlog (added October 2026)
Each idea is tagged with the phase it fits best. ⭐ marks the top picks; quick add and CAS import are the suggested next builds (small, visible, useful every day).

### Top picks
- ⭐ **Family "In case of emergency" file** (Phase 3)
  - Every bank account, policy, nominee and loan in one place, plus where the documents are kept and who to call.
  - A spouse or adult child opens it with their own login; it can also be printed as a PDF.
- ⭐ **Private on-device assistant** (Phase 4)
  - Plain-English questions about your own data: "How much did we spend on food last month?", "Which subscriptions went up this year?"
  - Runs locally through Ollama, so nothing leaves the device. The same local model can help read messy bank statements.
- ⭐ **Quick add from anywhere** (Phase 1)
  - Menu-bar / tray icon and a global shortcut open a small entry box.
  - Natural-language entry: `250 swiggy yesterday` becomes a transaction.
- ⭐ **Mutual fund import from CAS** (Phase 3)
  - Import the CAMS/KFintech consolidated account statement PDF, which covers all of a person's mutual funds in one file.
  - Automatic holdings, SIP history and returns (XIRR).
- ⭐ **Lend and borrow tracker** (Phase 2)
  - "I paid for John Smith's ticket", "Jane Doe owes me ₹2,000", with reminders and a settle-up history.

### Family and kids (Phase 4)
- Allowance with chores: a weekly allowance, plus extra earned for tasks.
- Kids' savings goals with parent matching, e.g. "we add ₹1 for every ₹2 you save".
- Money requests: a child asks for money (e.g. ₹500 for a school trip); a parent approves and it's recorded on both sides.
- Shared family wishlist with a savings target for each item.

### India-specific
- Tax-saving tracker: 80C, 80D and HRA progress against limits (Phase 3).
- Old vs new tax regime comparison using your actual numbers (Phase 3).
- EPF, PPF, NPS, sovereign gold bonds, FDs and RDs, with maturity dates on the calendar (Phase 3).
- Budgets for festivals and occasions (Diwali, weddings, school admissions), with year-on-year comparison (Phase 2).
- UPI SMS reading on Android: bank alert SMS become draft transactions on the phone itself (Phase 5).
  - Google Play restricts SMS access, so this likely ships as an opt-in or a "share to Vaulty" action.

### Insights (Phase 2–3)
- Price-rise alerts on recurring bills: "Your broadband bill went up ₹200 this month."
- Warranty tracker: attach the bill and get a reminder before the warranty ends.
- Spending heatmap calendar, so the expensive days stand out.
- Monthly and yearly summaries ("Vaulty Wrapped"): top categories, biggest change, savings rate.
- Trip mode: a trip budget, foreign-currency spending, and splitting costs with friends.

### Privacy and security (Phase 1–2)
- Duress password: a second password that opens a decoy profile with harmless data.
- Panic shortcut: instantly locks and hides the app.
- Encrypted, readable export: an export file anyone can open with the password, even if Vaulty no longer exists.

### International and tax (with region packs, decision 7)
- Tax set-aside jar: when freelance or business income arrives, suggest a percentage to put aside for tax, based on the member's region pack (Phase 3).
- Multi-country money: accounts in several countries, NRE/NRO accounts, remittance tracking, currency gains and losses (Phase 3).
- Tax residency day counter for people who split time between countries; the 183-day rule (Phase 3).
- Accountant access: a read-only profile or year-end export bundle for your CA or accountant (Phase 4).
- Languages: Hindi and other Indian languages; right-to-left languages such as Arabic (Phase 4).
- Bank holidays per country: due dates that fall on a holiday move to the next working day (Phase 2).

### Banks and institutions
- ✅ Bank / card / broker / wallet directory per country, with search, monogram badges, optional bundled logo files and per-account custom logos.
- Grow the directory and logo files through contributions (see `ui/public/institutions/README.md` and `TRADEMARKS.md`).
- Online mode, opt-in: fetch an institution's own website icon directly from its official domain (no third-party logo services), cached on the device.
- Statement import: suggest the account's institution parser automatically.

### Open-source community
- Shared bank statement parsers: a separate folder or repo where contributors add import rules for each bank (HDFC, ICICI, SBI…), so CSV and PDF import keeps improving without app changes.
- Region packs are community-maintained too: anyone can update their country's tax rules, with worked examples as tests.

## Keyboard shortcuts (current)
| Keys | Action |
|---|---|
| Ctrl/Cmd + 1–4 | Dashboard, Transactions, Accounts, Reports |
| Ctrl/Cmd + B | Toggle sidebar |
| Ctrl/Cmd + H | Hide or show balances |
| Ctrl/Cmd + L | Lock / switch profile |
| Ctrl/Cmd + , | Settings |
