-- Insurance: policies (life, health, motor, home, travel…), each with its cover, premium and
-- when it's next due, who's covered and the nominee; premiums paid (optionally added as a
-- transaction); and claims with what was asked for and what was paid out.
CREATE TABLE policies (
  id            TEXT PRIMARY KEY,
  kind          TEXT NOT NULL CHECK (kind IN ('term_life', 'life', 'health', 'motor', 'home', 'travel', 'accident', 'other')),
  name          TEXT NOT NULL,
  insurer       TEXT,
  policy_no     TEXT,
  holder        TEXT,
  covers        TEXT,                              -- who or what is insured
  for_parents   INTEGER NOT NULL DEFAULT 0,        -- health cover for parents (a separate tax limit in India)
  currency      TEXT NOT NULL,
  cover_minor   INTEGER CHECK (cover_minor IS NULL OR cover_minor >= 0),
  premium_minor INTEGER NOT NULL DEFAULT 0 CHECK (premium_minor >= 0),
  frequency     TEXT NOT NULL DEFAULT 'yearly' CHECK (frequency IN ('monthly', 'quarterly', 'half_yearly', 'yearly', 'single')),
  next_due      TEXT,
  start_date    TEXT,
  end_date      TEXT,                              -- maturity or expiry
  nominee       TEXT,
  contact       TEXT,
  document_id   TEXT REFERENCES vault_documents(id) ON DELETE SET NULL,
  status        TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'lapsed', 'ended')),
  notes         TEXT,
  created_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE policy_payments (
  id           TEXT PRIMARY KEY,
  policy_id    TEXT NOT NULL REFERENCES policies(id) ON DELETE CASCADE,
  date         TEXT NOT NULL,
  amount_minor INTEGER NOT NULL CHECK (amount_minor > 0),
  entry_id     TEXT,                               -- the transaction, when it was added as one
  created_at   TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_policy_payments ON policy_payments(policy_id, date);

CREATE TABLE policy_claims (
  id             TEXT PRIMARY KEY,
  policy_id      TEXT NOT NULL REFERENCES policies(id) ON DELETE CASCADE,
  date           TEXT NOT NULL,
  what           TEXT NOT NULL,
  claimed_minor  INTEGER NOT NULL CHECK (claimed_minor >= 0),
  settled_minor  INTEGER CHECK (settled_minor IS NULL OR settled_minor >= 0),
  status         TEXT NOT NULL DEFAULT 'filed' CHECK (status IN ('filed', 'in_progress', 'settled', 'partly_settled', 'rejected')),
  settled_on     TEXT,
  reference      TEXT,
  notes          TEXT,
  created_at     TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_policy_claims ON policy_claims(policy_id, date);
