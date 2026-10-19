-- People you lend to, borrow from or split bills with. Each has a ledger account: a positive
-- balance means they owe you, negative that you owe them. Lending isn't spending: it moves money
-- from your account to what they owe you.
CREATE TABLE people (
  id         TEXT PRIMARY KEY,
  name       TEXT NOT NULL,
  phone      TEXT,
  notes      TEXT,
  is_active  INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

ALTER TABLE ledger_accounts ADD COLUMN person_id TEXT REFERENCES people(id) ON DELETE CASCADE;
CREATE UNIQUE INDEX ledger_accounts_person ON ledger_accounts (person_id) WHERE person_id IS NOT NULL;

-- The transactions view, now also listing money lent to or received from a person (a transfer
-- with one side on a person's ledger): `person_id`/`person_name`, and `flow` 'out' or 'in'.
DROP VIEW transactions;
CREATE VIEW transactions AS
SELECT
  e.id, e.workspace_id, e.kind AS type, e.date, e.time, e.merchant, e.description, e.notes, e.tags,
  e.is_pending, e.is_cleared, e.visibility, e.hidden, e.recurring_id, e.receipt_path, e.created_at, e.updated_at,
  sa.account_id AS account_id,
  da.account_id AS to_account_id,
  cl.category_id AS category_id,
  ABS(sp.amount_minor) AS amount_minor,
  sp.currency AS currency,
  ABS(sp.amount_minor) * 1.0 / COALESCE(cur.scale, 100) AS amount,
  pl.person_id AS person_id,
  pe.name AS person_name,
  CASE WHEN sp.amount_minor > 0 AND e.kind = 'transfer' THEN 'in' ELSE 'out' END AS flow
FROM entries e
-- The account side: money out of an account (the source of a transfer, the payer of an
-- expense), or into it (income, or a transfer received from a person).
JOIN postings sp ON sp.id = (
  SELECT p.id FROM postings p JOIN ledger_accounts l ON l.id = p.ledger_account_id JOIN entries pe2 ON pe2.id = p.entry_id
  WHERE p.entry_id = e.id AND l.account_id IS NOT NULL
    AND ((pe2.kind = 'income' AND p.amount_minor >= 0) OR (pe2.kind = 'expense' AND p.amount_minor <= 0) OR pe2.kind = 'transfer')
  ORDER BY CASE WHEN pe2.kind = 'transfer' THEN p.amount_minor ELSE 0 END, p.id LIMIT 1)
JOIN ledger_accounts sa ON sa.id = sp.ledger_account_id
LEFT JOIN postings dp ON e.kind = 'transfer' AND dp.id = (
  SELECT p.id FROM postings p JOIN ledger_accounts l ON l.id = p.ledger_account_id
  WHERE p.entry_id = e.id AND l.account_id IS NOT NULL AND p.amount_minor > 0
    -- Only when money also left an account (not for money received from a person).
    AND EXISTS (SELECT 1 FROM postings q JOIN ledger_accounts lq ON lq.id = q.ledger_account_id
                 WHERE q.entry_id = p.entry_id AND lq.account_id IS NOT NULL AND q.amount_minor < 0)
  ORDER BY p.id LIMIT 1)
LEFT JOIN ledger_accounts da ON da.id = dp.ledger_account_id
LEFT JOIN postings cp ON e.kind IN ('income', 'expense') AND cp.id = (
  SELECT p.id FROM postings p JOIN ledger_accounts l ON l.id = p.ledger_account_id
  WHERE p.entry_id = e.id AND l.kind IN ('income', 'expense')
  ORDER BY p.id LIMIT 1)
LEFT JOIN ledger_accounts cl ON cl.id = cp.ledger_account_id
LEFT JOIN postings pp ON e.kind = 'transfer' AND pp.id = (
  SELECT p.id FROM postings p JOIN ledger_accounts l ON l.id = p.ledger_account_id
  WHERE p.entry_id = e.id AND l.person_id IS NOT NULL
  ORDER BY p.id LIMIT 1)
LEFT JOIN ledger_accounts pl ON pl.id = pp.ledger_account_id
LEFT JOIN people pe ON pe.id = pl.person_id
LEFT JOIN currencies cur ON cur.code = sp.currency
WHERE e.kind IN ('income', 'expense', 'transfer');
