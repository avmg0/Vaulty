# OWASP ASVS self-review

A self-assessment against the [OWASP Application Security Verification Standard
4.0.3](https://owasp.org/www-project-application-security-verification-standard/), done before
the public beta. ASVS is written for web services; Vaulty is a desktop app with no server, so
whole chapters (web sessions, cookies, server APIs) don't apply. What's left is mapped to how
Vaulty actually works. It is not a substitute for the external audit planned next
(ROADMAP decision 1c, part 6), and it should be redone before each major release.

**Reviewed:** October 2026, at commit after "Senior mode" on `dev`.
**Related:** [THREAT_MODEL.md](THREAT_MODEL.md), [BACKUP_FORMAT.md](BACKUP_FORMAT.md), [../SECURITY.md](../SECURITY.md).

Status: **Met** · **Partly** (works, with a known gap) · **Open** (not done yet) · **N/A**

## V1 Architecture

| Requirement | Status | How |
|---|---|---|
| Threat model exists and is kept current | Met | `docs/THREAT_MODEL.md`, with a checklist for new features |
| Security controls enforced in a trusted layer | Met | All checks, keys and crypto live in Rust; the interface is treated as untrusted input |
| Least privilege for components | Met | The window has only `core:default`; no file, shell, dialog, updater or network permissions |
| Sensitive data identified and classified | Met | Asset table in the threat model |

## V2 Authentication

| Requirement | Status | How |
|---|---|---|
| Passwords of at least 8 characters, no composition rules, long passphrases allowed | Met | `MIN_PASSWORD_LEN = 8`; up to 1,024 characters (`limits::PASSWORD`) |
| Passwords checked against common or breached ones | Met | New passwords are checked against the 10,000 most common (bundled, works offline), a common word with a few characters added, the person's name, and one repeated character (`passwords.rs`) |
| Brute-force protection | Met | 5 free attempts, then 30 s doubling to 15 min; applies to sign-in, re-confirmation, 2-step codes and backup codes (`check_throttle`) |
| Stored with a slow, salted hash | Met | Argon2id, 64 MiB, t=3; older hashes upgraded on next sign-in |
| Second factor available | Met | TOTP with one-time backup codes (stored as SHA-256 of high-entropy codes) |
| Recovery without weakening security | Met | Recovery key (shown once, printable sheet); admin reset for children only, which can't read their data |
| Biometric unlock bound to the OS | **Partly** | Works, but the check is app-enforced; OS-bound keys are a Phase 2 item still open |

## V3 Session management

| Requirement | Status | How |
|---|---|---|
| Session ends on lock or sign-out, keys wiped | Met | Keys held only in Rust while unlocked; `zeroize` on lock |
| Idle timeout | Met | Auto-lock (1–30 min, or never) is part of the profile. The window locks on time; Rust also drops the session and wipes the keys a minute later if the window stops reporting activity. Measured by the wall clock, so sleep counts |
| Re-authentication for sensitive actions | Met | Password re-confirmation for hidden items (re-locks after 5 min), security changes, erasing members |
| Web cookies and tokens | N/A | No server, no cookies |

## V4 Access control

| Requirement | Status | How |
|---|---|---|
| Enforced on the trusted side | Met | Roles checked in Rust (`require_admin`, `guard_hidden`, `guard_export`) |
| Members can't read each other's data | Met | One encrypted database per member, each with its own key; sharing only through the family store, only what was chosen |
| Admin limits | Met | The admin can reset a child or erase a member but can't read anyone's data; installing updates is admin-only |

## V5 Validation, sanitisation and encoding

| Requirement | Status | How |
|---|---|---|
| Input validated on the trusted side | Met | Size limits for every structured request (`limits.rs`); typed parsing of amounts, dates, settings |
| SQL injection | Met | Parameters everywhere; the few dynamic column names are checked against fixed lists or `is_identifier` |
| Output encoding / XSS | Met | React escaping; no `dangerouslySetInnerHTML` with user data; strict content security policy with no inline scripts |
| Untrusted files parsed safely | Met | Statement and backup readers capped by size, rows, columns and cell length; fuzzed (`tests/fuzz.rs`) |
| Deserialisation | Met | Restores are atomic and validated; crafted restores fuzzed to never unbalance the books |

## V6 Cryptography

| Requirement | Status | How |
|---|---|---|
| Vetted algorithms and libraries | Met | SQLCipher, Argon2id, XChaCha20-Poly1305, X25519 sealed boxes (RustCrypto and libsodium-compatible) |
| Random numbers from the OS | Met | `OsRng` for keys, salts, nonces, codes |
| Keys never hard-coded or logged | Met | No key material in code or logs; test passwords are random |
| Backups authenticated, settings fixed | Met | Header fields bound as associated data; KDF settings are fixed so a crafted file can't demand huge memory |
| Key rotation | **Partly** | Passwords and recovery keys rotate; data keys don't yet (re-encrypting a member's database is a later item) |

## V7 Errors and logging

| Requirement | Status | How |
|---|---|---|
| No sensitive data in errors or logs | Met | The app writes no logs; errors are short messages |
| Errors don't reveal internals | Met | Database, file and format failures are shown as a general message (detail goes to the console only); every other message is translated (`i18n/errors.ts`, checked in CI) |
| Security events visible to the person | Met | After signing in, the member is told how many wrong passwords or codes were entered since last time and when, with a button to change the password |
| Tamper-evident record of changes | Met | Hash-chained history with verification |

## V8 Data protection

| Requirement | Status | How |
|---|---|---|
| Sensitive data encrypted at rest | Met | Each member's database is encrypted; backups are encrypted |
| Unencrypted metadata minimised | **Partly** | The profile list (names, colours, roles, senior mode) is unencrypted so the sign-in screen can show it. Documented in the threat model |
| Sensitive data cleared from memory | **Partly** | Keys are wiped; decrypted values shown in the window live in the browser engine's memory until it frees them, which Vaulty can't control |
| Clipboard | Met | Recovery keys and codes are cleared from the clipboard after a short time |
| Hidden from shoulder-surfing | Met | Hide balances (Ctrl/Cmd+H); hidden transactions masked as XXXXX |

## V9 Communications

| Requirement | Status | How |
|---|---|---|
| TLS for all network use | Met | rustls; only opt-in features go online (market ticker, update checks) |
| Nothing about the person's money sent | Met | See the network table in the threat model |

## V10 Malicious code and supply chain

| Requirement | Status | How |
|---|---|---|
| Dependencies checked for advisories and licences | Met | `cargo deny`, `cargo audit`, `npm audit` on every push |
| Build actions pinned | Met | Third-party GitHub Actions pinned to commits |
| Updates signed and verified | Met | Signed in CI with a key stored as a GitHub secret (made and backed up offline); verified in the app before installing |
| Installers verifiable | Met | SHA256SUMS.txt and build provenance (`gh attestation verify`) |
| Code signing by the OS | **Partly** | macOS signing is ready once the Apple secrets are added; Windows isn't signed yet |

## V12 Files and resources

| Requirement | Status | How |
|---|---|---|
| Only files the person chose are touched | Met | Native dialog opened from Rust; file commands accept only those paths (`chosen`) |
| Size limits | Met | 20 MB statements, 200 MB backups, capped rows and cells |
| Path traversal | Met | No paths are built from file contents |

## V14 Configuration

| Requirement | Status | How |
|---|---|---|
| Secure defaults | Met | Everything online is off by default; private visibility by default |
| Content security policy | Met | `script-src 'self'`, no frames, objects, forms or workers; prototypes frozen |
| Debug features off in release | Met | Dev tools are not enabled in release builds; demo mode is a separate, throwaway folder |

## What to fix next, in order

Done since the first review: auto-lock enforced in Rust, common-password check, failed sign-ins
shown, translated and friendlier errors.

1. **OS-bound biometric keys** (V2; a Phase 2 item).
2. **Windows code signing** (V10; see docs/RELEASING.md).
3. **Data key rotation** (V6; later).

After these, the external audit.
