-- Device-level family sharing store.
-- Each member's own database stays private; when they mark a transaction Shared or
-- Public (and have Family sharing turned on), a read-only copy of the fields needed
-- for the family view is published here. Notes, tags and receipts are never copied.

CREATE TABLE members (
  user_id             TEXT PRIMARY KEY,
  enabled             INTEGER NOT NULL DEFAULT 0,   -- consent: share mine AND see others'
  default_visibility  TEXT NOT NULL DEFAULT 'private' CHECK (default_visibility IN ('private', 'shared', 'public')),
  updated_at          TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- "Shared" audience: owner_id lets viewer_id see their Shared items.
CREATE TABLE share_targets (
  owner_id   TEXT NOT NULL,
  viewer_id  TEXT NOT NULL,
  PRIMARY KEY (owner_id, viewer_id),
  CHECK (owner_id <> viewer_id)
);

CREATE TABLE shared_items (
  owner_id        TEXT NOT NULL,
  item_id         TEXT NOT NULL,          -- transaction id in the owner's database
  kind            TEXT NOT NULL DEFAULT 'transaction',
  visibility      TEXT NOT NULL CHECK (visibility IN ('shared', 'public')),
  date            TEXT NOT NULL,
  type            TEXT NOT NULL,
  amount          REAL NOT NULL,
  currency        TEXT,
  merchant        TEXT,
  description     TEXT,
  category_name   TEXT,
  category_icon   TEXT,
  category_color  TEXT,
  account_name    TEXT,
  published_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (owner_id, item_id)
);
CREATE INDEX idx_shared_items_date ON shared_items(date);
CREATE INDEX idx_shared_items_visibility ON shared_items(visibility);
