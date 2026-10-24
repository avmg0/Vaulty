-- Businesses the member runs, each kept in its own encrypted book (database). Listed in the
-- member's personal book with the business book's key, wrapped under the member's data key, so
-- only the member can open it. `capital_account_id` is the personal account that shows what the
-- member has put into the business, less what they've taken out.
CREATE TABLE businesses (
  id                 TEXT PRIMARY KEY,
  name               TEXT NOT NULL,
  kind               TEXT NOT NULL DEFAULT 'sole_proprietor'
                     CHECK (kind IN ('sole_proprietor', 'partnership', 'llp', 'company', 'other')),
  currency           TEXT NOT NULL,
  gstin              TEXT,
  pan                TEXT,
  state_code         TEXT,
  address            TEXT,
  fy_start_month     INTEGER NOT NULL DEFAULT 4 CHECK (fy_start_month BETWEEN 1 AND 12),
  wrapped_key        TEXT NOT NULL,
  capital_account_id TEXT REFERENCES accounts(id) ON DELETE SET NULL,
  created_at         TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  archived_at        TEXT
);

-- In a business's own book: what it is (name, registration details), so pages like invoices
-- can print them. One row, id 'self'. Empty in a personal book.
CREATE TABLE business_profile (
  id             TEXT PRIMARY KEY CHECK (id = 'self'),
  business_id    TEXT NOT NULL,
  name           TEXT NOT NULL,
  kind           TEXT NOT NULL,
  gstin          TEXT,
  pan            TEXT,
  state_code     TEXT,
  address        TEXT,
  fy_start_month INTEGER NOT NULL DEFAULT 4,
  updated_at     TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
