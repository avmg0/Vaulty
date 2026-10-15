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

### 2. Store money as integers (minor units), not floats
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
- [ ] Money as integer minor units (decision 2)
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
  - billing cycle and due date
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
- **Business:**
  - personal vs business tagging
  - invoices (PDF) with payment status
  - GST and VAT rates as editable data tables per country and state (don't hard-code tax law)
  - tax-filing export
  - mileage log
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

## Keyboard shortcuts (current)
| Keys | Action |
|---|---|
| Ctrl/Cmd + 1–4 | Dashboard, Transactions, Accounts, Reports |
| Ctrl/Cmd + B | Toggle sidebar |
| Ctrl/Cmd + H | Hide or show balances |
| Ctrl/Cmd + L | Lock / switch profile |
| Ctrl/Cmd + , | Settings |
