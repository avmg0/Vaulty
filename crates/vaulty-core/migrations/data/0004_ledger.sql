-- Double-entry ledger with money in integer minor units.
--
-- Every financial event is an `entry` with two or more `postings`. Postings in each currency
-- sum to zero. Signs follow the usual convention: positive = debit, negative = credit.
--   expense ₹250 from Savings:  Expense:Food +25000 / Asset:Savings -25000
--   income  ₹1000 to Savings:   Asset:Savings +100000 / Income:Salary -100000
--   transfer ₹500 to Card:      Liability:Card +50000 / Asset:Savings -50000
--   opening balance ₹10,000:    Asset:Savings +1000000 / Equity:Opening -1000000
-- An account's balance is the sum of its postings, so balances can never drift.
--
-- This migration converts data from migrations 0001–0003 (REAL amounts, single-row
-- transactions, stored balances) without changing any balance.

-- ---------------------------------------------------------------- currencies
CREATE TABLE currencies (
  code      TEXT PRIMARY KEY,
  exponent  INTEGER NOT NULL CHECK (exponent BETWEEN 0 AND 4),
  scale     INTEGER NOT NULL           -- 10^exponent
);
-- Must match money::ZERO_DECIMAL / money::THREE_DECIMAL (a test checks this). Unlisted codes use 2.
INSERT INTO currencies (code, exponent, scale) VALUES
  ('BIF',0,1),('CLP',0,1),('DJF',0,1),('GNF',0,1),('ISK',0,1),('JPY',0,1),('KMF',0,1),('KRW',0,1),('PYG',0,1),
  ('RWF',0,1),('UGX',0,1),('UYI',0,1),('VND',0,1),('VUV',0,1),('XAF',0,1),('XOF',0,1),('XPF',0,1),
  ('BHD',3,1000),('IQD',3,1000),('JOD',3,1000),('KWD',3,1000),('LYD',3,1000),('OMR',3,1000),('TND',3,1000),
  ('INR',2,100),('USD',2,100),('EUR',2,100),('GBP',2,100),('CAD',2,100),('AUD',2,100),('SGD',2,100),('AED',2,100);

-- ---------------------------------------------------------------- workspaces
-- Personal by default; business workspaces (decision 8) get their own accounts and categories.
CREATE TABLE workspaces (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  kind        TEXT NOT NULL DEFAULT 'personal' CHECK (kind IN ('personal', 'business')),
  created_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO workspaces (id, name, kind) VALUES ('ws_personal', 'Personal', 'personal');

-- ---------------------------------------------------------------- chart of accounts
-- Each user-facing account (bank, cash, card…) and each category has exactly one ledger account.
-- System accounts (opening balances, uncategorised) are identified by `system_key`.
CREATE TABLE ledger_accounts (
  id            TEXT PRIMARY KEY,
  workspace_id  TEXT NOT NULL REFERENCES workspaces(id),
  kind          TEXT NOT NULL CHECK (kind IN ('asset', 'liability', 'equity', 'income', 'expense')),
  name          TEXT NOT NULL,
  account_id    TEXT UNIQUE REFERENCES accounts(id) ON DELETE CASCADE,
  category_id   TEXT UNIQUE REFERENCES categories(id) ON DELETE CASCADE,
  system_key    TEXT,
  created_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE (workspace_id, system_key)
);

INSERT INTO ledger_accounts (id, workspace_id, kind, name, system_key) VALUES
  ('la_sys_opening',       'ws_personal', 'equity',  'Opening balances',     'opening_balance'),
  ('la_sys_adjustment',    'ws_personal', 'equity',  'Balance adjustments',  'adjustment'),
  ('la_sys_uncat_income',  'ws_personal', 'income',  'Uncategorised income', 'uncategorized_income'),
  ('la_sys_uncat_expense', 'ws_personal', 'expense', 'Uncategorised expense','uncategorized_expense');

INSERT INTO ledger_accounts (id, workspace_id, kind, name, account_id)
SELECT 'la_acc_' || id, 'ws_personal',
       CASE WHEN type IN ('credit', 'loan', 'liability') THEN 'liability' ELSE 'asset' END,
       name, id
FROM accounts;

INSERT INTO ledger_accounts (id, workspace_id, kind, name, category_id)
SELECT 'la_cat_' || id, 'ws_personal', CASE type WHEN 'income' THEN 'income' ELSE 'expense' END, name, id
FROM categories;

-- ---------------------------------------------------------------- entries and postings
CREATE TABLE entries (
  id            TEXT PRIMARY KEY,
  workspace_id  TEXT NOT NULL DEFAULT 'ws_personal' REFERENCES workspaces(id),
  kind          TEXT NOT NULL CHECK (kind IN ('income', 'expense', 'transfer', 'opening', 'adjustment')),
  date          TEXT NOT NULL,
  time          TEXT,
  merchant      TEXT,
  description   TEXT,
  notes         TEXT,
  tags          TEXT,
  is_pending    INTEGER NOT NULL DEFAULT 0,
  is_cleared    INTEGER NOT NULL DEFAULT 1,
  visibility    TEXT NOT NULL DEFAULT 'private' CHECK (visibility IN ('private', 'shared', 'public')),
  recurring_id  TEXT,
  receipt_path  TEXT,
  created_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_entries_date ON entries(date);
CREATE INDEX idx_entries_kind ON entries(kind);
CREATE INDEX idx_entries_visibility ON entries(visibility);
CREATE TRIGGER update_entries_timestamp AFTER UPDATE ON entries
BEGIN
  UPDATE entries SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

CREATE TABLE postings (
  id                 INTEGER PRIMARY KEY,
  entry_id           TEXT NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
  ledger_account_id  TEXT NOT NULL REFERENCES ledger_accounts(id),
  amount_minor       INTEGER NOT NULL,      -- + debit, − credit, in minor units of `currency`
  currency           TEXT NOT NULL,
  memo               TEXT
);
CREATE INDEX idx_postings_entry ON postings(entry_id);
CREATE INDEX idx_postings_account ON postings(ledger_account_id);

-- ---------------------------------------------------------------- convert existing data
-- Amount in minor units for an account's currency (unlisted currencies use 2 decimals).
-- Old REAL amounts are rounded once, here, and never again.

-- Opening balances (old accounts.initial_balance).
INSERT INTO entries (id, kind, date, description, created_at)
SELECT 'open_' || a.id, 'opening', date(COALESCE(a.created_at, CURRENT_TIMESTAMP)), 'Opening balance', COALESCE(a.created_at, CURRENT_TIMESTAMP)
FROM accounts a WHERE COALESCE(a.initial_balance, 0) <> 0;

INSERT INTO postings (entry_id, ledger_account_id, amount_minor, currency)
SELECT 'open_' || a.id, 'la_acc_' || a.id,
       CAST(ROUND(a.initial_balance * COALESCE(c.scale, 100)) AS INTEGER), COALESCE(a.currency, 'USD')
FROM accounts a LEFT JOIN currencies c ON c.code = a.currency
WHERE COALESCE(a.initial_balance, 0) <> 0;

INSERT INTO postings (entry_id, ledger_account_id, amount_minor, currency)
SELECT 'open_' || a.id, 'la_sys_opening',
       -CAST(ROUND(a.initial_balance * COALESCE(c.scale, 100)) AS INTEGER), COALESCE(a.currency, 'USD')
FROM accounts a LEFT JOIN currencies c ON c.code = a.currency
WHERE COALESCE(a.initial_balance, 0) <> 0;

-- Transactions → entries.
INSERT INTO entries (id, kind, date, time, merchant, description, notes, tags, is_pending, is_cleared,
                     visibility, recurring_id, receipt_path, created_at, updated_at)
SELECT id, type, date, time, merchant, description, notes, tags, COALESCE(is_pending, 0), COALESCE(is_cleared, 1),
       visibility, recurring_id, receipt_path, COALESCE(created_at, CURRENT_TIMESTAMP), COALESCE(updated_at, CURRENT_TIMESTAMP)
FROM transactions;

-- The account side: expense and transfer take money out (credit); income puts it in (debit).
INSERT INTO postings (entry_id, ledger_account_id, amount_minor, currency)
SELECT t.id, 'la_acc_' || t.account_id,
       CASE t.type WHEN 'income' THEN 1 ELSE -1 END * CAST(ROUND(t.amount * COALESCE(c.scale, 100)) AS INTEGER),
       COALESCE(a.currency, 'USD')
FROM transactions t JOIN accounts a ON a.id = t.account_id LEFT JOIN currencies c ON c.code = a.currency;

-- The other side: the category (or "uncategorised") for income and expense.
INSERT INTO postings (entry_id, ledger_account_id, amount_minor, currency)
SELECT t.id,
       CASE WHEN t.category_id IS NOT NULL AND EXISTS (SELECT 1 FROM categories WHERE id = t.category_id)
            THEN 'la_cat_' || t.category_id
            WHEN t.type = 'income' THEN 'la_sys_uncat_income'
            ELSE 'la_sys_uncat_expense' END,
       CASE t.type WHEN 'income' THEN -1 ELSE 1 END * CAST(ROUND(t.amount * COALESCE(c.scale, 100)) AS INTEGER),
       COALESCE(a.currency, 'USD')
FROM transactions t JOIN accounts a ON a.id = t.account_id LEFT JOIN currencies c ON c.code = a.currency
WHERE t.type IN ('income', 'expense');

-- Old "transfer" rows had no destination and did not change balances; keep that by
-- balancing them against the same account.
INSERT INTO postings (entry_id, ledger_account_id, amount_minor, currency, memo)
SELECT t.id, 'la_acc_' || t.account_id, CAST(ROUND(t.amount * COALESCE(c.scale, 100)) AS INTEGER),
       COALESCE(a.currency, 'USD'), 'migrated: transfer destination unknown'
FROM transactions t JOIN accounts a ON a.id = t.account_id LEFT JOIN currencies c ON c.code = a.currency
WHERE t.type = 'transfer';

DROP TABLE transactions;

-- Stored balances are now derived from postings.
ALTER TABLE accounts DROP COLUMN current_balance;
ALTER TABLE accounts DROP COLUMN initial_balance;

-- ---------------------------------------------------------------- compatibility view
-- One row per income / expense / transfer, in the shape the app used before.
-- `amount` is a positive decimal for display; `amount_minor` is exact.
CREATE VIEW transactions AS
SELECT
  e.id, e.workspace_id, e.kind AS type, e.date, e.time, e.merchant, e.description, e.notes, e.tags,
  e.is_pending, e.is_cleared, e.visibility, e.recurring_id, e.receipt_path, e.created_at, e.updated_at,
  sa.account_id AS account_id,
  da.account_id AS to_account_id,
  cl.category_id AS category_id,
  ABS(sp.amount_minor) AS amount_minor,
  sp.currency AS currency,
  ABS(sp.amount_minor) * 1.0 / COALESCE(cur.scale, 100) AS amount
FROM entries e
JOIN postings sp ON sp.id = (
  SELECT p.id FROM postings p JOIN ledger_accounts l ON l.id = p.ledger_account_id
  WHERE p.entry_id = e.id AND l.account_id IS NOT NULL
    AND ((e.kind = 'income' AND p.amount_minor >= 0) OR (e.kind <> 'income' AND p.amount_minor <= 0))
  ORDER BY p.id LIMIT 1)
JOIN ledger_accounts sa ON sa.id = sp.ledger_account_id
LEFT JOIN postings dp ON e.kind = 'transfer' AND dp.id = (
  SELECT p.id FROM postings p JOIN ledger_accounts l ON l.id = p.ledger_account_id
  WHERE p.entry_id = e.id AND l.account_id IS NOT NULL AND p.amount_minor > 0
  ORDER BY p.id LIMIT 1)
LEFT JOIN ledger_accounts da ON da.id = dp.ledger_account_id
LEFT JOIN postings cp ON e.kind IN ('income', 'expense') AND cp.id = (
  SELECT p.id FROM postings p JOIN ledger_accounts l ON l.id = p.ledger_account_id
  WHERE p.entry_id = e.id AND l.kind IN ('income', 'expense')
  ORDER BY p.id LIMIT 1)
LEFT JOIN ledger_accounts cl ON cl.id = cp.ledger_account_id
LEFT JOIN currencies cur ON cur.code = sp.currency
WHERE e.kind IN ('income', 'expense', 'transfer');
