-- Encryption at rest. Each member's finance database is encrypted with a random 256-bit data key.
-- The data key is never stored in the clear: it is "wrapped" (encrypted) under each secret that
-- may unlock it. Losing every wrap means the data is unrecoverable, by design.
CREATE TABLE user_keys (
  user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  kind        TEXT NOT NULL CHECK (kind IN ('password', 'recovery', 'biometric', 'setup', 'guardian')),
  holder      TEXT NOT NULL DEFAULT '',   -- 'guardian': the parent (admin) who can recover a Child profile
  salt        TEXT,                       -- Argon2id salt for password / recovery / setup wraps
  wrapped     TEXT NOT NULL,              -- XChaCha20-Poly1305 or X25519 sealed box, hex
  created_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (user_id, kind, holder)
);

-- Each member's X25519 key pair: others seal keys to the public key (the family store key, or a
-- child's data key for parents); the secret key is sealed under the member's own data key.
ALTER TABLE users ADD COLUMN public_key TEXT;
ALTER TABLE users ADD COLUMN secret_key_sealed TEXT;

-- The family sharing store's key, sealed to each member's public key.
CREATE TABLE household_keys (
  user_id  TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  sealed   TEXT NOT NULL
);
