-- Minimum due and yearly details per credit card.
ALTER TABLE cards ADD COLUMN min_due_percent REAL;        -- share of the statement (NULL: 5%)
ALTER TABLE cards ADD COLUMN min_due_floor_minor INTEGER; -- at least this much (NULL: ₹200 for INR cards, nothing otherwise)
ALTER TABLE cards ADD COLUMN annual_fee_minor INTEGER;
ALTER TABLE cards ADD COLUMN card_year_start TEXT;        -- MM-DD: when the card year (fees, yearly benefits) starts

-- The figures on a statement as the bank printed them, when the member enters them. They replace
-- Vaulty's own estimate for that statement.
CREATE TABLE card_statements (
  id              TEXT PRIMARY KEY,
  card_id         TEXT NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
  statement_date  TEXT NOT NULL,
  total_minor     INTEGER NOT NULL CHECK (total_minor >= 0),
  min_minor       INTEGER CHECK (min_minor IS NULL OR min_minor >= 0),
  created_at      TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE (card_id, statement_date)
);

-- What a card gives back: reward rates (by category or place), spend milestones in the card
-- year (e.g. the annual fee waived), and perks counted per card year (e.g. lounge visits).
CREATE TABLE card_benefits (
  id           TEXT PRIMARY KEY,
  card_id      TEXT NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
  kind         TEXT NOT NULL CHECK (kind IN ('reward', 'milestone', 'perk')),
  title        TEXT NOT NULL,
  category_id  TEXT REFERENCES categories(id) ON DELETE SET NULL, -- reward: this category (and its sub-categories)
  merchant     TEXT,                                              -- reward: or at this place
  rate         REAL,                                              -- reward: percent back
  cap_minor    INTEGER,                                           -- reward: at most this much back per statement
  target_minor INTEGER,                                           -- milestone: spend this in the card year
  allowance    INTEGER,                                           -- perk: this many per card year
  used         INTEGER NOT NULL DEFAULT 0,                        -- perk: used so far in `used_year`
  used_year    TEXT,                                              -- perk: the card year `used` counts (its start date)
  notes        TEXT,
  created_at   TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_card_benefits_card ON card_benefits(card_id);
