# Vaulty threat model

What Vaulty protects, from whom, how, and where its protection ends. This is a living document:
update it whenever a feature changes what is stored, who can reach it, or when Vaulty goes online.
Last reviewed: October 2026 (Phase 2).

## What's worth protecting

| Asset | Where it lives | Protection |
|---|---|---|
| Each member's finances: accounts, transactions, cards (last 4 digits only), budgets, recurring payments, notes, tags, history | `users/<id>.db` | SQLCipher (AES-256), random 256-bit key per member |
| Hidden transactions | inside the member's database | masked in Rust; details only after re-entering the password or biometrics; re-locks after 5 minutes |
| Market data API keys | member's database | encrypted with it |
| Two-step verification secret | registry | sealed (XChaCha20-Poly1305) under the member's data key |
| Shared family items (only what members chose to share) | `household.db` | SQLCipher, household key sealed to each member's X25519 public key |
| Safety copies | `snapshots/<id>/` | same key as the member's database |
| Backup files (`.vaulty`) | wherever the member saves them | Argon2id + XChaCha20-Poly1305, header authenticated |
| Change history | member's database | tamper-evident SHA-256 chain |
| Profile list: names, colours, roles, password hashes, wrapped keys, backup-code hashes, lockout counters | `registry.db` (not encrypted) | needed before sign-in; hashes are Argon2id (64 MiB, 3 passes) |
| Times of wrong sign-in attempts; senior mode | `registry.db` (not encrypted) | times only, to tell the member after signing in |
| Payment due dates, only for members who turn on "remind me while locked" | `registry.db` (not encrypted) | dates only: no names, payees or amounts; replaced each time reminders are checked |

Not stored anywhere: full card numbers, CVVs, bank passwords, online banking sessions.

## Who might try

1. **Someone with the device** (a thief, a repair shop, a curious relative) who has the files but no password.
2. **A family member or employee** who has their own profile on the same device.
3. **A booby-trapped file**: a statement, spreadsheet or backup built to crash Vaulty or exhaust memory.
4. **Malware on the same computer** while Vaulty is running.
5. **A compromised dependency or update**: a malicious crate, npm package or GitHub Action.
6. **Someone on the network**, for the few features that go online (the opt-in market ticker; later email, sync and updates).

## What Vaulty promises

**1. Device without the password.** Every financial record, safety copy and shared item is encrypted at
rest. Brute force is slowed by Argon2id per guess and by the sign-in lockout: after 5 wrong attempts
the profile locks for 30 seconds, doubling each time up to 15 minutes. Two-step verification adds a
second factor. Recovery keys (24 characters) are as strong as a random key. Admins *cannot* open
adults' data; parents hold a sealed recovery copy for children only.

*Not covered:* the registry is readable, so an attacker learns names, roles and that profiles exist,
and can attack an Argon2 hash offline (as with any password system). Weak passwords remain the
weakest point; the minimum is 8 characters.

**2. Another member of the household.** Each profile has its own key. Sharing is opt-in on both
sides and only copies the fields a member chose to share; hidden transactions are never shared.

**3. Hostile files.** Importers cap file size (20 MB statements, 200 MB backups), row and column
counts and cell length, and parse every value defensively; failures are reported, never trusted.
Backups are authenticated before being read. Imports take a safety copy first and are one undoable
action. Fuzz and property tests cover these paths (Phase 2).

**4. Mistakes.** Every change is recorded and can be undone; deletes go to a 30-day trash; daily
encrypted safety copies; the change history is a hash chain, so edits made outside Vaulty are detected.

**5. Supply chain.** Dependencies are pinned (`Cargo.lock`, `package-lock.json`) and checked on every
change: `cargo deny` (licences, sources, RustSec advisories), `cargo audit`, `npm audit`, CodeQL.
GitHub Actions are pinned to commits. Release signing is planned (see Roadmap 1c.5).

**6. The interface.** The web view loads only the bundled app (strict content security policy; no
remote scripts, frames or content). It never talks to the internet itself; the few network calls
are made by Rust. Commands validate everything they receive; the UI is never trusted with keys.
Even if a script were somehow injected, it would be boxed in:
- The policy allows only the app's own scripts, no frames, plugins or form posts, and images only
  from the app or data the person chose. Built-in prototypes are frozen.
- The web view has no file-system or shell permissions. Files are chosen in a native dialog opened
  by Rust, and file commands only accept paths chosen that way during the run.
- Every structured request is size-checked (`limits.rs`): text fields, notes, images, list lengths,
  nesting depth and passwords all have ceilings, so nothing oversized reaches the database.

Keys live in Rust memory only while a profile is unlocked and are wiped on lock (`zeroize`).

## Where protection ends (explicit non-goals)

- **An infected, unlocked computer.** Malware running as the same user while a profile is open can
  read the screen, log keys and read memory. Vaulty limits the window (auto-lock, wiping keys on
  lock, the Hidden section's 5-minute re-lock) but cannot defeat it.
- **Biometric unlock is enforced by Vaulty, not the OS.** The keychain item holding the biometric
  token is readable by Vaulty without a prompt. Binding it to biometrics at the OS level is planned
  (Phase 2, macOS and Windows first). Turn biometrics off if malware is a concern.
- **Exports you make.** CSV exports are plain text by design (for spreadsheets). Encrypted backups
  protect content but not the hint and creation date.
- **Screenshots and screen sharing.** Protection per platform is planned (see Roadmap decision 6);
  Linux can't prevent capture.
- **Lost passwords.** With no password and no recovery key, an adult's data cannot be recovered by
  anyone, including Vaulty's maintainers. This is intended.
- **Legal compliance.** Vaulty helps prepare tax and business records but is not a certified
  accounting product; filings should be reviewed by a professional.

## Network use (all opt-in)

| Feature | Talks to | Sends |
|---|---|---|
| Market ticker | CoinGecko, Twelve Data (member's own key) | symbols requested, the key |
| Planned: email invoices | the member's mail provider | the invoice, to the recipient |
| Planned: device sync | the member's own devices on the local network | encrypted changes, after pairing |
| Updates (off until turned on, or "Check now") | GitHub Releases | a request for `latest.json`; installers are verified against the signing key built into the app before anything is installed, and only the family admin can install |

No analytics, no telemetry, no crash reporting, no accounts, no Vaulty servers.

## Review checklist for new features

- Does it store something new? Add it to the asset table and keep it in the member's encrypted database.
- Does it take input from the interface? Run structured input through `limits::check`.
- Does it read or write a file? Take the path from `pick_file` and call `chosen` first.
- Does it read a file? Cap size, rows and lengths; fuzz it; snapshot before writing.
- Does it go online? Opt-in, Rust only, listed in the network table, nothing sent beyond what's needed.
- Does it change data? Record it in history so it can be undone.
- Does it show data to another member? Only through the household store, only what was chosen.
