-- Credit card details, one row per credit-card account.
-- Only the network and the last 4 digits are kept; full card numbers are never stored.
CREATE TABLE card_details (
  account_id          TEXT PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
  network             TEXT CHECK (network IN ('visa', 'mastercard', 'amex', 'discover', 'rupay', 'diners', 'jcb', 'unionpay', 'maestro')),
  last4               TEXT CHECK (last4 IS NULL OR (length(last4) = 4 AND last4 NOT GLOB '*[^0-9]*')),
  credit_limit_minor  INTEGER CHECK (credit_limit_minor IS NULL OR credit_limit_minor >= 0),
  statement_day       INTEGER CHECK (statement_day IS NULL OR statement_day BETWEEN 1 AND 31),
  due_day             INTEGER CHECK (due_day IS NULL OR due_day BETWEEN 1 AND 31),
  design              TEXT NOT NULL DEFAULT 'midnight',
  updated_at          TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
