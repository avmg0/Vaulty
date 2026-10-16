# Vaulty roadmap

Offline-first, open-source personal finance planner for macOS, Windows, Linux, iOS and Android.
Stack: Tauri v2, Rust (`crates/vaulty-core`), React + TypeScript, shadcn/ui, Lucide icons.

---

## Decisions to settle before building further

These are cheap to get right now and expensive to change once real data exists.

### 1. Encryption at rest ✅ (done)
SHA-256 and SHA-512 are hash functions; they can't encrypt. What Vaulty does instead:

- **Each member's database and the family store are encrypted with SQLCipher (AES-256).**
- **Each member has a random 256-bit data key.** It is stored only wrapped (encrypted) under:
  - their password (Argon2id, 64 MiB, 3 passes)
  - their recovery key (24 characters, about 119 bits, shown once)
  - a biometric token kept in the OS keychain (if turned on)
  - a pending setup code (until they claim the profile)
  - for Child profiles only, each parent's X25519 public key (sealed box)
- Changing a password re-wraps the key; the data is never re-encrypted.
- **The family store key** is sealed to each member's public key.
- **Admins can't open adults' data.** Adults recover with their recovery key. Parents can reset a child's password and keep the child's data. Otherwise an admin can only erase a profile and start it over, after typing the member's name.
- Keys are wiped from memory on sign-out (zeroize, SQLCipher memory security); temp tables stay in memory; auto-lock after inactivity.
- Older plain databases are encrypted in place on first sign-in.
- Still to do:
  - [ ] bind the biometric keychain item to biometrics at the OS level (macOS/iOS access control, Android Keystore); today the prompt is enforced by the app
  - [ ] password-protected export files (exports are currently plain JSON/CSV, by the user's choice)
  - [ ] "Delete forever" in Trash, removing an item from history now rather than after 90 days
  - [ ] screenshot protection and blur in the app switcher
  - [ ] external security review before a public release

### 1b. Mistakes and accidental deletes ✅ (done)
- Every change is recorded (before and after) in the encrypted database, one batch per action.
- Undo toast after edits and deletes; Ctrl/Cmd+Z; History page with recent changes and a 30-day trash.
- Undo refuses when a later change touched the same item, rather than overwriting it.
- Encrypted safety copies: daily, before imports, app updates and restores; 7 daily and 4 weekly kept; restore from Settings.
- Removing or erasing a member requires typing their name.

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
- **Status:** format, loader and the India pack done (`crates/vaulty-core/data/tax/`). The calculator is built from generic building blocks: bands by age, standard deduction, capped deductions, rebate, surcharge, cess, marginal relief and rounding.
- **Building blocks still to add**, each the first time a country needs it, then reusable by every pack:
  - filing status (single, married jointly…) with its own bands (US)
  - tax credits that reduce the tax rather than the income (US child credit, Australian offsets)
  - layered taxes: federal plus state or provincial, chosen by where the person lives (US, Canada, Australia); extra taxes such as UK National Insurance or the Medicare levy
  - allowance taper: a tax-free amount that shrinks as income rises (UK above £100k)
  - an overall cap across several reliefs (Singapore)
  - as a last resort, a pack may name a small, tested Rust function for a rule too unusual for data
- **Updates:** packs are built into the app so tax works offline. Later, optional signed pack updates let a budget change be fixed without a full app release; nothing else leaves the device.
- **Next pack:** pick the second country first, since it shows which building blocks are missing (UK or Singapore are moderate; the US is the hardest).

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
- [x] Tax region packs: format, loader, validation and the India pack (FY 2026-27, old and new regime) with worked-example tests (decision 7)
- [x] Tax planner page: compares regimes with your numbers and recommends the lower one
- [x] Interface languages: English and Hindi (sign-in, navigation, dashboard, settings, tax planner); other screens fall back to English
- [x] Language and region per profile: any country, every ISO 4217 currency, regional number and date formats
- [x] Opt-in market ticker: regional lists (India, US, UK), crypto via CoinGecko, custom watchlist via Twelve Data with the user's own key; cached for offline
- [ ] Translate the remaining screens (Accounts, Transactions, Cards, Family, security settings) and move all dates to the shared formatter
- [ ] Tax packs translated (regime and deduction names follow the interface language)
- [ ] More tax packs: US, UK, Canada, Australia, Singapore, UAE; GST/VAT rates; capital gains at special rates
- [ ] More languages: Tamil, Bengali, Marathi, Telugu, Gujarati, Kannada, Malayalam, Punjabi, Urdu (RTL), Arabic (RTL), Spanish, French, German
- [ ] Ticker: more providers, more regional lists, and live prices for holdings on the Investments page
- [x] SQLCipher encryption at rest, recovery keys, parent recovery for children (decision 1)
- [x] Change history, undo, trash and encrypted safety copies (decision 1b)
- [x] Motion: rolling amounts, list enter/exit and undo, sliding sidebar highlight, card lift and staggered details; Motion setting (Full / Reduced / Off, following the system)
- [x] More motion: sign-in (avatar glide, code boxes, shake, recovery key reveal, lock blur, unlock moment), sliding segmented controls, change arrows and progress bars, card look cross-fade and removal, tax totals and badge, family fan-out and "new" dots, history strike-through, ticker price flash, theme cross-fade, skeleton loaders, success ticks
- [x] Animated chart kit (`ui/src/components/charts`): grouped bars grow in, donut slices draw in, sparklines draw left to right; all ease to new values and follow the Motion setting
- [x] Dashboard charts: income vs spending (6 months), spending by category (this month), net worth trend
- [x] Indian short amounts on charts (₹92.5K, ₹4.1L, ₹1.2Cr)
- [ ] Reports page built on the chart kit; goal rings and calendar motion as those pages arrive
- [ ] Settings persisted in DB: date and time format, first day of week, fiscal year start, decimals, negative format (language, country and currency done)
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
- Old vs new tax regime comparison: done as the Tax planner (enter numbers by hand); next, fill it from your actual transactions (Phase 3).
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
- Languages: Hindi ships; other Indian languages and right-to-left languages such as Arabic next (translation files in `ui/src/i18n/locales`).
- Bank holidays per country: due dates that fall on a holiday move to the next working day (Phase 2).

### Banks and institutions
- ✅ Bank / card / broker / wallet directory per country, with search, monogram badges, optional bundled logo files and per-account custom logos.
- Grow the directory and logo files through contributions (see `ui/public/institutions/README.md` and `TRADEMARKS.md`).
- Online mode, opt-in: fetch an institution's own website icon directly from its official domain (no third-party logo services), cached on the device.
- Statement import: suggest the account's institution parser automatically.

### Open-source community
- Shared bank statement parsers: a separate folder or repo where contributors add import rules for each bank (HDFC, ICICI, SBI…), so CSV and PDF import keeps improving without app changes.
- Region packs are community-maintained too: anyone can update their country's tax rules, with worked examples as tests.

## More ideas (added 16 October 2026)

### Tax planner
- [ ] Fill the planner from transactions: link categories to deductions (health insurance premium → health insurance limit, ELSS → tax-saving investments) and show progress against each limit through the year (Phase 3).
- [ ] Break-even point: how much more you'd need to invest or claim before the old regime beats the new one (Phase 3).
- [ ] Tax already paid: TDS from salary and interest, advance-tax instalments with calendar reminders, and "tax still due" or "refund expected" (Phase 3).
- [ ] Import the AIS (Annual Information Statement) to check your entries against what banks and employers reported (Phase 3).
- [ ] Capital gains harvesting: suggest selling and rebuying before 31 March to use the yearly tax-free long-term gains limit (currently ₹1.25 lakh) (Phase 3).

### Investments and prices
- [ ] Mutual fund NAVs from AMFI's free daily file: daily values for Indian funds with no account or API key (Phase 3, top pick).
- [ ] Live value of holdings from ticker prices, returns by XIRR, and price alerts checked only while the ticker is on (Phase 3).
- [ ] Exchange rates from free reference rates (e.g. the European Central Bank) to show total net worth in one currency (Phase 3).

### Language and region
- [ ] Lakh and crore: compact amounts like "₹12.3 L" and "₹1.2 Cr" on dashboards in India, full figures in tables (Phase 1).
- [ ] Amount in words ("Twelve lakh thirty-four thousand rupees") on receipts, invoices and cheque-writing, in the interface language (Phase 2).
- [ ] Natural-language and voice entry in your own language, e.g. "कल स्विगी 250" for ₹250 on Swiggy yesterday (Phase 3).

### Cards
- [ ] Benefits tracker: reward points and their expiry, lounge visits left, spend still needed for the annual fee waiver (Phase 2).
- [ ] Best card for today: the card whose billing cycle gives the longest interest-free period for a purchase made now (Phase 2).

### Family and safety
- [ ] Emergency kit: a printable, password-protected summary for a spouse or nominee with accounts, policies, nominees and who to contact (Phase 3, top pick).
- [ ] Nominee tracking on every account, policy and investment, flagging the ones without a nominee (Phase 3).

### For the project
- [ ] Demo mode with John Doe sample data, so screenshots, testing and new contributors never touch real data (Phase 1).
- [ ] Import from other apps: Money Manager, Walnut, YNAB and similar (Phase 2).
- [ ] Accessibility check: screen readers and keyboard-only use, with older family members in mind (Phase 2).

## Further ideas (added 16 October 2026, evening)

### Tax and salary (India first, via region packs)
- [ ] Rent receipts for HRA: monthly rent receipts as PDFs with the landlord's name and PAN; rent paid fills the tax planner's HRA field (Phase 3, top pick).
- [ ] Form 16 and payslip import: read salary, TDS and deductions so the tax planner fills itself in (Phase 3).
- [ ] Landlord mode: rent received, tenant details, security deposit, maintenance and property tax, with "rent not received" reminders (Phase 4).

### Planning
- [ ] What-if scenarios: prepay a loan or invest, afford a purchase by a date, salary growth; outcomes side by side (Phase 3).
- [ ] Retirement and early-retirement planner: inflation-adjusted target, progress and the monthly amount needed (Phase 3).
- [ ] Inflation-adjusted view: optionally show spending and net worth in today's money (Phase 3).
- [ ] Cash-flow diagram: salary into accounts, out to bills, investments and savings, built on the chart kit (Phase 2).

### Family life
- [ ] Senior mode: large text, fewer menus, bigger buttons; a family member can help set it up (Phase 2, top pick).
- [ ] Document expiry reminders: passport, driving licence, vehicle insurance, PUC, health policy, FD maturity; on the calendar with advance reminders (Phase 2, top pick).
- [ ] Medical expenses and insurance claims: bills, claim status, reimbursements, remaining tax-saving health cover (Phase 3).

### Security
- [ ] Shared recovery key: split a recovery key into parts held by trusted family members (e.g. any 2 of 3 rebuild it, Shamir's secret sharing); no single person can unlock the data, and losing one part isn't fatal (Phase 2, top pick).

## Keyboard shortcuts (current)
| Keys | Action |
|---|---|
| Ctrl/Cmd + 1–4 | Dashboard, Transactions, Accounts, Reports |
| Ctrl/Cmd + B | Toggle sidebar |
| Ctrl/Cmd + H | Hide or show balances |
| Ctrl/Cmd + L | Lock / switch profile |
| Ctrl/Cmd + , | Settings |
