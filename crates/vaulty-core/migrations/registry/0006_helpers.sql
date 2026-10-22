-- Shared recovery: a member's data key, wrapped under a key that any 2 of 3 helper shares rebuild.
CREATE TABLE helper_sets (
  user_id     TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  set_id      TEXT NOT NULL,              -- printed on cards, so cards from an older set are recognised
  wrapped     TEXT NOT NULL,              -- the data key, XChaCha20-Poly1305 under the helper key
  created_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- One row per share. A family member's share is sealed to their public key; a printed card's
-- share is only on the card (nothing about it is kept here beyond who holds it).
CREATE TABLE helper_shares (
  user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  slot       INTEGER NOT NULL CHECK (slot BETWEEN 1 AND 3),
  kind       TEXT NOT NULL CHECK (kind IN ('member', 'card')),
  helper_id  TEXT REFERENCES users(id) ON DELETE SET NULL,
  label      TEXT,                        -- a card: who holds it, as the member wrote it
  sealed     TEXT,
  PRIMARY KEY (user_id, slot)
);

-- "Your help was used": shown to a helper the next time they sign in.
CREATE TABLE helper_notices (
  helper_id  TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  owner_name TEXT NOT NULL,
  at         INTEGER NOT NULL
);
