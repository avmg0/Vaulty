-- The money journal: notes on decisions and how money felt, with mood, tags and linked
-- transactions. A month review keeps a snapshot of that month's figures next to the words.
-- The first journal table was never written to by the app, so it's rebuilt.
DROP TABLE IF EXISTS journal_entries;
CREATE TABLE journal_entries (
  id          TEXT PRIMARY KEY,
  date        TEXT NOT NULL,
  title       TEXT NOT NULL,
  content     TEXT NOT NULL DEFAULT '',
  mood        TEXT CHECK (mood IS NULL OR mood IN ('great', 'good', 'okay', 'worried', 'stressed')),
  tags        TEXT,                          -- comma-separated
  linked      TEXT NOT NULL DEFAULT '[]',    -- transaction ids, JSON
  review_of   TEXT,                          -- YYYY-MM for a month review
  facts       TEXT,                          -- the month's figures when the review was written, JSON
  created_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_journal_date ON journal_entries(date);
CREATE UNIQUE INDEX idx_journal_review ON journal_entries(review_of) WHERE review_of IS NOT NULL;
