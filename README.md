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

## Where data lives
```
<app data dir>/
  registry.db        # who's on this device: names, password hashes, sealed 2FA secrets
  users/<id>.db      # one private finance database per family member
```
Signing in opens only that member's database.

### How money is stored
- Amounts are integers in minor units (paise, cents…); each currency's decimal places come from `money.rs` and the `currencies` table (JPY 0, INR/USD 2, KWD 3…). Floats are never stored.
- Every income, expense, transfer and opening balance is a double-entry `entry` whose `postings` sum to zero per currency (`ledger.rs`, migration `0004_ledger.sql`). Account balances are sums of postings, never stored.
- The `transactions` view presents entries in the simple shape the UI uses (one row per income / expense / transfer). Admins manage who's on the device but can't open other members' data.

## Develop
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

See **ROADMAP.md** for phases, architecture decisions and feature ideas.
