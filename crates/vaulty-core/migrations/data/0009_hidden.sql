-- Hidden transactions: kept out of sight of anyone looking over the member's shoulder.
-- Outside the Hidden section their merchant, category and notes are masked (XXXXX); the amount,
-- date and account stay visible so balances and totals remain correct. Never shared with family.
ALTER TABLE entries ADD COLUMN hidden INTEGER NOT NULL DEFAULT 0 CHECK (hidden IN (0, 1));

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
