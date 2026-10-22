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

### 1c. Security programme (planned)
Vaulty is open source and will hold business records, so security comes from the design, never from hiding the code. In order of priority:

1. **Threat model** (`docs/THREAT_MODEL.md`)
   - [ ] What's worth stealing: finances, GSTINs, client lists, saved email passwords
   - [ ] Who might try: someone with the laptop, malware on the same computer, a family member or employee, a booby-trapped import file, a compromised dependency or update
   - [ ] What Vaulty promises against each, and what it doesn't (e.g. an already-infected computer)
2. **Automatic checks on every change** (GitHub Actions)
   - [ ] Tests, clippy, TypeScript check and translation check on macOS, Windows and Linux
   - [ ] `cargo audit` and `cargo deny` (vulnerable or badly licensed crates); `npm audit`; CodeQL (enabled)
   - [ ] Fuzz testing for everything that reads outside files: statement import (CSV/XLSX/PDF), backup import, tax packs, GSTR-2B
   - [ ] Property tests: every ledger entry balances; wrapped keys unwrap only with the right secret
   - [ ] Pinned dependencies; review each new one; prefer fewer, well-known crates
3. **Harden the app shell**
   - [ ] Tighten the content security policy (drop inline styles)
   - [ ] Tauri isolation pattern: check every message from the UI before Rust acts on it
   - [ ] Never load internet content into the app window
   - [ ] Size and rate limits on every command that takes files or large input
4. **Close known gaps**
   - [ ] Password-protected export files (exports are plain JSON/CSV today)
   - [ ] Bind the biometric keychain item to biometrics at the OS level
   - [ ] Email passwords and e-invoice credentials in the OS keychain where possible
   - [ ] Revisit setup-code length (8 characters, 7-day expiry)
   - [ ] Note in the threat model: names and colours are visible before sign-in (needed for the picker); no finances are
5. **Protect the release chain**
   - [ ] GitHub account on a hardware key or passkey; signed commits
   - [ ] Actions pinned by commit, least-privilege tokens, no secrets in workflows run for outside pull requests
   - [ ] Signed releases and updates; update-signing key kept offline (ideally on a hardware key)
   - [ ] Reproducible builds and a published SBOM
6. **Pentesting**
   - [ ] Before the first release that asks people to trust it with real data, and again before each online feature (email sending, e-invoicing, sync)
   - [ ] Scope: installed app, every UI-to-Rust command, file importers, encryption and recovery (biometric, keychain), update mechanism, data left on disk or in memory; mobile against OWASP MASVS later
   - [ ] Run the OWASP ASVS checklist ourselves first, so the paid review finds deeper issues
   - [ ] Auditors to approach: OSTIF (arranges and often funds open-source audits), non-profit firms such as Radically Open Security, a CERT-In empanelled auditor (weight with Indian businesses); Trail of Bits, Cure53, NCC Group if budget allows
7. **Reports and incidents**
   - [ ] `SECURITY.md` with private reporting and response times we can keep
   - [ ] GitHub security advisories and CVEs for fixes
   - [ ] Opt-in in-app notice when a security update is available
   - [ ] Bug bounty or hall of fame once there are users
8. **Tamper-evident history** (no blockchain: a private, offline hash chain)
   - [x] Every recorded action carries a SHA-256 fingerprint of the one before it plus everything it changed; altering, removing or reordering any action breaks every fingerprint after it
   - [x] The 90-day clean-up keeps a checkpoint so the remaining chain still verifies
   - [x] "Verify history" on the History page: checks the chain, shows the current fingerprint, and flags items changed outside Vaulty (compared with their last recorded state)
   - [x] App updates that reshape data are recorded as "Vaulty updated your data" (and can't be undone) instead of being flagged
   - [ ] Business workspaces: always on (part of the unalterable edit log) and kept for the retention period
   - [ ] Signed checkpoints: each business workspace gets its own signing key; periodic checkpoints are signed so an accountant or auditor can verify the books without trusting the owner or Vaulty
   - [ ] Fingerprints kept outside the database (someone with full control and the keys could otherwise rebuild the whole chain): in safety copies, on each exported invoice, and in a monthly fingerprint sent to the accountant
   - [ ] Invoices record their place in the chain, so edits after issue and gaps in numbering are detectable
   - [ ] Optional public timestamp with OpenTimestamps: only a fingerprint leaves the device, never data; opt-in, needs the internet
9. **Legal and compliance**
   - [ ] Never claim "GST compliant" or "audit ready"; say Vaulty helps prepare invoices and returns and that a CA should review filings (MIT "as is" plus in-app disclaimers)
   - [ ] India's DPDP Act 2023 mainly binds whoever collects and holds people's data; a local, offline app keeps the project largely outside that role. Revisit before any cloud sync or hosted service
   - [ ] Business record-keeping (unalterable edit log, 6–8 year retention) designed into business workspaces from the start (see Business below)

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

### 4. Device sync: local network first, no server (planned)
Phones, iPads and computers sync directly with each other. Data never goes to a Vaulty server; there isn't one.

- **Pairing, once per device**
  - [ ] The desktop shows a QR code; the phone scans it; both screens show the same 6-digit code and the member confirms it matches (stops a "man in the middle" on the same network)
  - [ ] Each device creates its own key pair at pairing; every sync after that is mutually authenticated and end-to-end encrypted (TLS 1.3 or the Noise protocol); no passwords travel over the network
  - [ ] Per member: each person pairs their own phone and receives only their own data plus the shared family store; a child's phone gets only the child's profile
  - [ ] Device list in Settings with "Remove this device": a lost phone stops syncing at once and keys are rotated
- **Sync the change history, not database files**
  - [ ] Devices exchange "changes since we last spoke" using the existing change batches, so edits made on both sides while apart are all kept
  - [ ] Real conflicts (the same item edited differently on two devices) are shown with both versions for the member to choose; never resolved silently
  - [ ] Each device verifies the other's tamper-evident chain before accepting changes
- **Networks**
  - [ ] Default: same Wi-Fi only, with devices finding each other on the local network (mDNS/Bonjour)
  - [ ] Optional, off by default: sync across networks with Iroh (Rust peer-to-peer; direct where possible, otherwise through a relay that only sees encrypted data; the relay can be self-hosted)
  - [ ] Optional, off by default: the member's own storage as an encrypted postbox (iCloud Drive, Google Drive, Dropbox, home NAS); changes are encrypted on the device before upload
- **Mobile realities**
  - [ ] iPhone and iPad don't allow free background syncing: sync when the app is open (fast, because only changes travel)
  - [ ] Clear explanations for the iOS "Local Network" prompt and Android's local-discovery permission
  - [ ] Data on the phone encrypted with the member's key and unlocked with Face ID or fingerprint, as on desktop
- **Build order:** pairing plus local Wi-Fi sync with the desktop as hub → change-history sync with conflict handling → device list and removal → optional cross-network sync → phone-to-phone sync without the desktop
- **Encrypted backups** (separate from sync): password-protected export files first; an optional encrypted backup to the member's own cloud drive later

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

Each phase is a release with a "done when" test. Order: daily use first, security before strangers trust it with real data, mobile and sync before wealth (quick entry happens on the phone), business last (largest and most compliance-heavy). Items from the backlog sections below are pulled into a phase when it starts.

### Already built
- Tauri v2 workspace, `vaulty-core` crate, migrations, tests
- Family profiles: per-member encrypted databases (SQLCipher), setup codes, roles, recovery keys, parent recovery for children
- Two-step verification (any authenticator app, backup codes) and biometric unlock (desktop)
- Family sharing: Private / Shared / Public, two-sided consent, Family view
- Money as integer minor units; double-entry ledger core
- Change history, undo, trash, tamper-evident chain, encrypted safety copies
- Hidden transactions
- Cards page, bank and institution directory
- Tax region packs with the India pack and the Tax planner
- English and Hindi; any country and currency; regional number formats
- Opt-in market ticker
- Motion throughout, animated chart kit, dashboard charts

### Phase 0: Finish the foundations (built; waiting for the first green run on GitHub)
- [x] Automatic checks on every change (`.github/workflows/checks.yml`) on macOS, Windows and Linux: clippy with warnings as errors, tests, TypeScript build, translation check; `cargo deny` (licences, sources, advisories), `cargo audit`, `npm audit`. Actions pinned to commits. Formatting with rustfmt is left out for now (it would reformat most files)
- [x] Demo mode (`--demo` or `VAULTY_DEMO=1`): John, Jane and Jenny Doe with six months of sample data each, in a throwaway folder
- [x] Every screen translated (English and Hindi, 658 strings); dates through the shared formatter; ordinal days per language ("20th", "20 तारीख")
- [x] Formats per profile: date order, negative amounts (minus or brackets), decimal places, first day of the week, financial-year start; each "Automatic" by default, following the profile's language and country
- [x] Error messages from the Rust side are translated (English and Hindi; CI checks every new one)
- **Done when:** every check passes on all three systems.

### Phase 1: Daily use (v0.1, family alpha) (built; now the month of real use)
- [x] Full transaction editing, tags and notes, splits across categories; transfers between accounts
- [x] Categories and sub-categories with monthly budgets (Budgets page, on-pace hints)
- [x] Recurring payments and bills: every N days/weeks/months/years, mark paid, skip, pause, auto-pay caught up at sign-in, "Coming up" on the dashboard
- [x] CSV / XLSX / XLS / ODS statement import: header detection, column mapping saved per account, Indian and international formats, duplicate detection, safety copy and one-step undo
- [x] Automatic categories: rules learned from your choices, used by import and the add dialog, applied in bulk
- [x] Quick add (Ctrl/Cmd+J) with plain-language entry ("250 swiggy yesterday", "petrol 2k from card", "कल 300 सब्ज़ी")
- [x] Password-protected backups (`.vaulty`, open format in docs/BACKUP_FORMAT.md) and a printable recovery sheet
- [x] Command palette (Ctrl/Cmd+K); period and category filters, bulk actions, lists that grow in pages
- [ ] Quick add from anywhere: tray / menu-bar icon and an OS-wide shortcut (needs the Tauri tray and global-shortcut plugins, tested on each OS)
- [x] Hotkey editor (Settings → Keyboard shortcuts; per profile, clash and system-key checks, undoable)
- **Done when:** the family uses it for a month instead of a spreadsheet.

### Phase 2: Security before public release (v0.5, public beta)
- [x] Threat model and `SECURITY.md` (decision 1c, parts 1 and 7)
- [x] Harden the app shell: strict content security policy, frozen prototypes, no window permissions beyond the core, files only after the person picks them, size limits on every request (1c, part 3). The isolation pattern was dropped: with no plugin permissions left in the window, it adds little.
- [x] Property tests for importers, backups, restores and the ledger (`tests/fuzz.rs`); found and fixed two bugs (1c, part 2)
- [x] Release workflow: signed updates, checksums, build provenance, pinned actions; opt-in, admin-only updates in the app (1c, part 5). Needs the one-time key setup in docs/RELEASING.md; Windows signing later
- [ ] Biometric item bound at the OS level; external fingerprint copies (1c, parts 4 and 8)
- [x] Accessibility pass (axe on every page, keyboard-only walk-through); senior mode
- [x] OWASP ASVS self-review (`docs/ASVS.md`)
- [x] ASVS follow-ups: auto-lock enforced in Rust, common-password check, failed sign-ins shown, errors translated (all 248 Rust messages, checked in CI)
- [ ] First external audit (1c, part 6)
- **Done when:** signed installers are on GitHub Releases and audit findings are fixed.

### Phase 3: Planning and reminders (v0.6)
- [x] Calendar: bills and income, card due and statement dates, paid bills, own events and reminders, holidays (fixed national ones built in; the rest added or imported, since festival and state bank holidays vary); due dates on bank-closed days show the day to pay by; `.ics` import and export (amounts left out unless asked)
- [ ] Calendar: document expiry (once Documents exists)
- [x] OS notifications: bills (each bill's own "remind N days before"), card payments 3 days ahead, overdue bills, budgets at 80% and 100%; amounts off by default; optional dates-only reminders while locked
- [x] Low-balance alerts: a level per bank or cash account; reminded when below it, or when a bill in the next two weeks would take it there
- [ ] Cards: statements, minimum due, benefits tracker, best card for today
- [x] Reports page on the chart kit (by month, category, place and income source; any period incl. the financial year); 60-day cash-flow forecast with and without everyday spending; "safe to spend" until the next income
- [x] Lend and borrow: people with what they owe or you owe, lent / borrowed / repaid, splitting a bill you paid (only your share is spending), settle-up, history, a copyable reminder message, reminders after 30 quiet days
- [x] Subscription detector (weekly, monthly, quarterly, yearly charges not yet tracked; track or dismiss) and price-rise alerts (detected subscriptions and recurring payments, 3%+; shown on the item and as a reminder)
- [x] To-do linked to bills, transactions, people and events (ticks itself off when the bill is paid; due dates on the calendar and as reminders)
- [ ] Shared recovery: any 2 of 3 helpers (adult members on the device or printed helper cards) restore full access, 2FA stays on
- [ ] Nominee access: a printed nominee card plus one helper, after a 7-day wait the member can cancel by signing in; read-only, with the "In case of emergency" estate summary and an optional letter
- [x] Report exports: Excel workbook (summary, by month, 90-day forecast; labels in the member's language) and print or save as PDF through the system print dialog
- **Done when:** no bill or card payment is missed.

### Phase 4: Mobile and sync (v0.8)
- [ ] iOS and Android builds; mobile layout with a bottom tab bar
- [ ] Pairing and same-Wi-Fi sync, in the build order of decision 4
- [ ] Quick-add widget; biometric unlock on the phone
- [ ] Pentest of pairing and sync
- **Done when:** something added on a phone appears on the desktop, and conflicts are handled.

### Phase 5: Wealth and tax (v1.0)
- [ ] Investments: holdings, events, XIRR, allocation; AMFI NAVs; CAS import
- [ ] Goals and calculators; insurance; net-worth timeline
- [ ] Emergency kit, nominee tracking, documents vault, journal
- [ ] Tax planner filled from transactions; TDS and advance tax; HRA rent receipts; Form 16 import
- [ ] Second country's tax pack
- [ ] PDF statement import; anomaly detection
- **Done when:** net worth is complete and the tax planner fills itself in.

### Phase 6: Business (v1.x)
- [ ] Business workspaces (decision 8), then GST invoices with PDF and email, then payments and receivables, then purchase bills and GST returns, then stock and payroll (see "Business: invoicing and beyond")
- [ ] Edit log that can't be turned off, signed checkpoints, retention (decision 1c, parts 8 and 9)
- [ ] Pentest before email sending and e-invoicing go live
- **Done when:** a small business can invoice, get paid and hand its books to an accountant.

### Ongoing
- More languages and tax packs, the private on-device assistant, family and kids features, "Vaulty Wrapped", online extras (FX rates, cloud backup).

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
- ✅ Hidden transactions: any transaction can be moved to a Hidden section. Elsewhere it shows as XXXXX with only the amount, date and account (or is left out, per device); totals and balances still count it. The details are masked in Rust, so they never reach the screen until the member re-enters their password or uses Touch ID / Windows Hello. The section locks again after 5 minutes, on leaving the tab, or when the window is hidden. Hidden transactions are never shared with family, their history entries are masked too, and exports need the section unlocked.
  - [ ] Hidden accounts and cards (same idea, a whole account at once)
  - [ ] Optional separate PIN for the Hidden section
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
- [x] Accessibility check: screen readers and keyboard-only use, with older family members in mind (Phase 2).

## Business: invoicing and beyond (added 16 October 2026)
Builds on decision 8 (business workspaces on the ledger) and the Phase 4 list. Suggested order: workspace → GST invoices with PDF and email → payments and receivables → bills and GST returns → stock, payroll and the rest.

### Invoicing in detail
- [ ] GST-ready invoices: both GSTINs, place of supply with CGST + SGST or IGST chosen from the two states, HSN/SAC codes, reverse charge
- [ ] One numbering series per financial year (e.g. INV/2026-27/0001), with no gaps or duplicates
- [ ] Document types: quote, proforma, tax invoice, bill of supply (composition dealers), export invoice under LUT, delivery challan, credit and debit notes
- [ ] Templates: logo, brand colour, signature, terms, bank details, UPI QR; PDF made on the device
- [ ] Getting paid: part payments, advances, write-offs, TDS deducted by the client recorded against the invoice; statement of account per client
- [ ] E-invoicing (IRN and QR from the government portal, mandatory above ₹5 crore turnover) and e-way bills: optional online add-on through a registered intermediary

### Sending invoices
- [ ] From your own email account (Gmail, Outlook or your domain) with an app password or sign-in; sent from your address; credentials encrypted (OS keychain where possible)
- [ ] Through your mail app: a new email opens with the PDF attached and the client filled in
- [ ] WhatsApp, or any app via the system Share sheet
- [ ] Every send recorded on the invoice (to whom, when, how)
- [ ] Overdue reminders the same way (e.g. gentle at 3 days, firmer at 15)
- [ ] Sending is opt-in and the only time Vaulty goes online for this; no "opened" tracking (needs a server and invades the client's privacy)

### More business features
- [ ] Purchases and bills: vendor bills, purchase orders, due dates; GSTR-2B matching (import the portal file to see which suppliers' invoices give input tax credit and which are missing)
- [ ] GST returns: GSTR-1 file ready to upload to the GST portal; GSTR-3B summary of tax to pay after credit
- [ ] TDS you deduct: calculated on contractor and professional payments, deposit reminder by the 7th, certificates issued
- [ ] Stock: items, stock levels, low-stock alerts, cost of goods sold; invoices reduce stock
- [ ] Payroll for small teams (up to about 20): salaries, PDF payslips, advances, PF, ESI and professional tax
- [ ] Petty cash and daily sales: cash book and quick daily-sales entry for shops
- [ ] Expense claims: staff or partners submit expenses with receipts and get reimbursed
- [ ] Partners: capital, drawings and profit share per partner
- [ ] Roles inside a business: staff can invoice but not see bank balances; accountant can view everything but change nothing
- [ ] Compliance calendar: GST (11th and 20th), TDS (7th), advance tax, PF and ESI, yearly company filings
- [ ] Cash-flow forecast from unpaid invoices, bills, salaries and taxes
- [ ] Budget vs actual per business

### Compliance built in
- [ ] Edit log that can't be turned off for business workspaces (Indian companies using accounting software must keep one since April 2023); builds on the existing tamper-evident change history (decision 1c, part 8)
- [ ] Record retention: GST records 6 years, company books 8 years; business history and safety copies kept that long instead of the personal 90 days

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
- [x] Senior mode: large text, fewer menus, bigger buttons; a family member can help set it up (Phase 2, top pick).
- [ ] Document expiry reminders: passport, driving licence, vehicle insurance, PUC, health policy, FD maturity; on the calendar with advance reminders (Phase 2, top pick).
- [ ] Medical expenses and insurance claims: bills, claim status, reimbursements, remaining tax-saving health cover (Phase 3).

### Security
- [ ] Shared recovery key: split a recovery key into parts held by trusted family members (e.g. any 2 of 3 rebuild it, Shamir's secret sharing); no single person can unlock the data, and losing one part isn't fatal (Phase 2, top pick).

## Keyboard shortcuts (defaults; each member can change them in Settings)
| Keys | Action |
|---|---|
| Ctrl/Cmd + 1–4 | Dashboard, Transactions, Accounts, Reports |
| Ctrl/Cmd + B | Toggle sidebar |
| Ctrl/Cmd + H | Hide or show balances |
| Ctrl/Cmd + L | Lock / switch profile |
| Ctrl/Cmd + Shift + L | Lock at once, no animation, balances hidden |
| Ctrl/Cmd + , | Settings |
| Ctrl/Cmd + K | Command palette: pages, actions, find a transaction |
| Ctrl/Cmd + J | Quick add |
| Ctrl/Cmd + Z | Undo the last change |
