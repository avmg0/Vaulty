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

## Where data lives
```
<app data dir>/
  registry.db        # who's on this device: names, password hashes, sealed 2FA secrets
  users/<id>.db      # one private finance database per family member
```
Signing in opens only that member's database. Admins manage who's on the device but can't open other members' data.

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
