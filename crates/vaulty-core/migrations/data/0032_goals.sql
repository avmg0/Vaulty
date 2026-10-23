-- Goals, rebuilt: amounts in minor units, a target date, how much is saved, money set aside in
-- accounts and investments (a share of each), and the plan to get there. The first goals table
-- (REAL amounts) was never written to by the app, so it's replaced.
DROP TABLE IF EXISTS goals;

CREATE TABLE goals (
  id            TEXT PRIMARY KEY,
  name          TEXT NOT NULL,
  icon          TEXT,
  currency      TEXT NOT NULL,
  target_minor  INTEGER NOT NULL CHECK (target_minor > 0),
  target_date   TEXT,                            -- none: no deadline
  saved_minor   INTEGER NOT NULL DEFAULT 0,      -- saved elsewhere, entered by hand
  monthly_minor INTEGER NOT NULL DEFAULT 0,      -- what the person plans to put in each month
  return_rate   REAL NOT NULL DEFAULT 0,         -- expected yearly growth of the savings, %
  inflation     REAL NOT NULL DEFAULT 0,         -- if set, the target is in today's money, %
  priority      INTEGER NOT NULL DEFAULT 2,      -- 1 high, 2 normal, 3 low
  done_on       TEXT,                            -- reached or closed on
  notes         TEXT,
  created_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE goal_links (
  id          TEXT PRIMARY KEY,
  goal_id     TEXT NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
  account_id  TEXT REFERENCES accounts(id) ON DELETE CASCADE,
  holding_id  TEXT REFERENCES holdings(id) ON DELETE CASCADE,
  share       INTEGER NOT NULL DEFAULT 100 CHECK (share BETWEEN 1 AND 100),
  CHECK ((account_id IS NULL) != (holding_id IS NULL))
);
CREATE INDEX idx_goal_links_goal ON goal_links(goal_id);
