# Vaulty

Privacy-first, offline-first family finance planner. Tauri v2 + Rust (SQLite, Argon2id) + React/TypeScript + shadcn/ui + Lucide.

## Layout
- `crates/vaulty-core`: all data and security logic, with no Tauri dependency. `cargo test -p vaulty-core` runs anywhere.
  - `profiles.rs`: family members, passwords, setup codes, 2FA (TOTP plus backup codes), biometric tokens, lockout.
  - `accounts.rs`, `transactions.rs`, `categories.rs`, `settings.rs`, `export.rs`: finance data.
  - `migrations/registry`, `migrations/data`: SQL migrations (append-only).
- `src-tauri`: thin desktop shell. It holds the signed-in session, shows the OS biometric prompt and uses the OS keychain.
- `ui`: React + Vite + Tailwind v4 + shadcn/ui (new-york, zinc) + Lucide icons. It talks to Rust only through `ui/src/api.ts`.
  Add shadcn components with `npx shadcn@latest add <name>` from `ui/`.

## Brand
- `assets/brand/logo.svg`: master logo (brand blue `#235BAA`). `ui/public/logo.svg` is a copy used as the favicon.
- In the app the logo is drawn inline by `ui/src/components/Logo.tsx` and follows the accent colour chosen in Settings (CSS variables `--logo` / `--logo-detail` in `ui/src/index.css`). On the neutral accent it stays brand blue. If you change the master SVG, regenerate `Logo.tsx` from it.
- `assets/brand/app-icon-1024.png` (macOS/Linux tile) and `app-icon-small-1024.png` (tight crop for small sizes and Windows) are the sources for `src-tauri/icons/`.
  To regenerate after a logo change: `npx @tauri-apps/cli icon assets/brand/app-icon-1024.png -o src-tauri/icons`, then re-export the small sizes and `icon.ico` from the tight crop.

## Banks and institutions
- Directory of banks, card issuers, brokers and wallets per country: `crates/vaulty-core/data/banks/<COUNTRY>.json` (community-maintained).
- Optional logo files go in `ui/public/institutions/` (see the README there). Without one, the app shows a monogram badge; users can also attach their own image to an account.
- Names and logos belong to their owners and are not covered by the MIT licence; see `TRADEMARKS.md`.

## Languages, countries and taxes

- **Languages:** English and Hindi. Each family member picks their own; the sign-in screen has a
  language switch too. Adding a language is one JSON file: see `ui/src/i18n/README.md`.
- **Countries and currencies:** any country and every ISO 4217 currency. Numbers, money and dates
  follow the chosen region (₹12,34,567.50 in India, $1,234,567.50 in the US).
- **Tax planner:** tax rules are data files in `crates/vaulty-core/data/tax/`. India (old and new
  regime) ships first; see that folder's README to add a country. Results are estimates, not advice.

## Market ticker (optional, off by default)

Turning it on is the only thing that makes Vaulty go online. It sends the symbols you choose and
nothing about your money. Crypto prices come from CoinGecko (no key needed). Stock prices need a
free key from Twelve Data, which you add in Settings; it's stored in your profile's database and
never shown again. The last prices are kept so the ticker still shows something offline.

## Where data lives
```
<app data dir>/
  registry.db           # who's on this device: names, password hashes, wrapped keys (no finances)
  users/<id>.db         # each member's finances, encrypted with their own key (SQLCipher, AES-256)
  household.db          # what members chose to share, encrypted with the family key
  snapshots/<id>/*.db   # encrypted safety copies of each member's data
```
Signing in unwraps only that member's key and opens only their database. Every change is
recorded in their encrypted database so it can be undone; deleted items stay restorable for 30
days. See decisions 1 and 1b in `ROADMAP.md` for how keys and recovery work.

### How money is stored
- Amounts are integers in minor units (paise, cents…); each currency's decimal places come from `money.rs` and the `currencies` table (JPY 0, INR/USD 2, KWD 3…). Floats are never stored.
- Every income, expense, transfer and opening balance is a double-entry `entry` whose `postings` sum to zero per currency (`ledger.rs`, migration `0004_ledger.sql`). Account balances are sums of postings, never stored.
- The `transactions` view presents entries in the simple shape the UI uses (one row per income / expense / transfer). Admins manage who's on the device but can't open other members' data.

## Develop

SQLCipher is built with a vendored OpenSSL, so no system crypto library is needed. Building it
needs Perl and a C compiler (preinstalled on macOS and Linux; on Windows install Strawberry Perl).
```
cargo test -p vaulty-core
npm --prefix ui install
cargo install tauri-cli --version "^2"
cargo tauri dev
```
- Linux needs the Tauri system deps (webkit2gtk-4.1 etc.): https://v2.tauri.app/start/prerequisites/
- On Linux, biometric/system unlock goes through polkit. For local testing, install the policy once:
  `sudo install -Dm644 src-tauri/packaging/com.vaulty.app.policy /usr/share/polkit-1/actions/`
  (the .deb and .rpm bundles do this automatically).

### Demo mode
Run with sample data instead of your own: `VAULTY_DEMO=1 cargo tauri dev` (or pass `--demo` to a
built app). Vaulty builds a fresh household in a temporary folder on every launch: John Doe
(salaried, with a credit card and two hidden transactions), Jane Doe (freelancer) and Jenny Doe
(child), each with six months of made-up transactions. The password for all three is shown on the
sign-in screen. Your real data folder is never opened in demo mode. Use it for screenshots, bug
reports and trying changes.

### Checks
Every push and pull request runs `.github/workflows/checks.yml` on macOS, Windows and Linux. To run
the same checks locally:
```
npm --prefix ui run build && npm --prefix ui run i18n:check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo install --locked cargo-deny cargo-audit   # once
cargo deny check && cargo audit
```
`crates/vaulty-core/tests/fuzz.rs` throws random and hostile input at the importers, backups,
restores and tax estimates, and random sequences of edits and undos at the ledger. CI runs a
short pass; for a deeper search before a release:
```
PROPTEST_CASES=5000 cargo test --release -p vaulty-core --test fuzz
```

See **ROADMAP.md** for phases, architecture decisions and feature ideas.
