-- Device-level registry of family members. Holds only what is needed to show the
-- profile picker and authenticate; each user's finances live in their own database file.
CREATE TABLE users (
  id                    TEXT PRIMARY KEY,
  name                  TEXT NOT NULL,
  color                 TEXT NOT NULL DEFAULT '#71717a',
  role                  TEXT NOT NULL DEFAULT 'member' CHECK (role IN ('admin', 'member', 'child')),
  password_hash         TEXT,              -- Argon2id PHC string; NULL until the user sets it
  kdf_salt              TEXT,              -- salt for deriving the key that seals the TOTP secret
  invite_hash           TEXT,              -- Argon2id hash of the one-time setup code (pending users)
  invite_expires_at     INTEGER,
  totp_enabled          INTEGER NOT NULL DEFAULT 0,
  totp_secret_sealed    TEXT,              -- XChaCha20-Poly1305(nonce || ciphertext), hex
  totp_last_step        INTEGER NOT NULL DEFAULT 0,  -- RFC 6238 §5.2: accept each code once
  biometric_enabled     INTEGER NOT NULL DEFAULT 0,
  biometric_token_hash  TEXT,              -- SHA-256 of the device token kept in the OS keychain
  failed_attempts       INTEGER NOT NULL DEFAULT 0,
  locked_until          INTEGER NOT NULL DEFAULT 0,  -- unix seconds
  created_at            TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  last_login_at         TEXT
);

CREATE TABLE backup_codes (
  user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  code_hash  TEXT NOT NULL,                -- SHA-256; codes carry ~40 bits of entropy
  used_at    TEXT,
  PRIMARY KEY (user_id, code_hash)
);
