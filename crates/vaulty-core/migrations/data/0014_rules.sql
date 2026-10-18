-- Merchant → category rules. Learned when the member categorises a transaction, or added by hand.
CREATE TABLE category_rules (
  id           TEXT PRIMARY KEY,
  pattern      TEXT NOT NULL UNIQUE,            -- lower-case text the merchant must contain
  category_id  TEXT NOT NULL REFERENCES categories(id) ON DELETE CASCADE,
  source       TEXT NOT NULL DEFAULT 'learned' CHECK (source IN ('learned', 'manual')),
  created_at   TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at   TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
