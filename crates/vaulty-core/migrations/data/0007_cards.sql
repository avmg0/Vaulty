-- Cards of every kind, replacing card_details (credit only) from 0006.
--   credit   owns a liability account (what you owe)
--   debit    draws from an existing bank account (several cards may share one account)
--   prepaid  owns an asset account holding its balance (prepaid, forex, transit…)
--   gift     owns an asset account holding the remaining value
--   other    membership, loyalty or store cards: no money attached
-- Only the network and last 4 digits are ever stored.
CREATE TABLE cards (
  id                  TEXT PRIMARY KEY,
  kind                TEXT NOT NULL CHECK (kind IN ('credit', 'debit', 'prepaid', 'gift', 'other')),
  name                TEXT NOT NULL,
  account_id          TEXT REFERENCES accounts(id) ON DELETE SET NULL,
  owns_account        INTEGER NOT NULL DEFAULT 0,
  institution_id      TEXT,
  issuer_name         TEXT,
  network             TEXT CHECK (network IN ('visa', 'mastercard', 'amex', 'discover', 'rupay', 'diners', 'jcb', 'unionpay', 'maestro')),
  last4               TEXT CHECK (last4 IS NULL OR (length(last4) = 4 AND last4 NOT GLOB '*[^0-9]*')),
  expiry_month        INTEGER CHECK (expiry_month IS NULL OR expiry_month BETWEEN 1 AND 12),
  expiry_year         INTEGER CHECK (expiry_year IS NULL OR expiry_year BETWEEN 2000 AND 2100),
  credit_limit_minor  INTEGER CHECK (credit_limit_minor IS NULL OR credit_limit_minor >= 0),
  statement_day       INTEGER CHECK (statement_day IS NULL OR statement_day BETWEEN 1 AND 31),
  due_day             INTEGER CHECK (due_day IS NULL OR due_day BETWEEN 1 AND 31),
  -- JSON: {"preset":"midnight"} or a custom fill (solid / gradient / image); validated in cards.rs.
  look                TEXT NOT NULL DEFAULT '{"preset":"midnight"}',
  notes               TEXT,
  created_at          TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at          TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_cards_account ON cards(account_id);
CREATE TRIGGER update_cards_timestamp AFTER UPDATE ON cards
BEGIN
  UPDATE cards SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

-- Existing credit-card details.
INSERT INTO cards (id, kind, name, account_id, owns_account, institution_id, network, last4,
                   credit_limit_minor, statement_day, due_day, look)
SELECT 'card_' || d.account_id, 'credit', a.name, d.account_id, 1, a.institution_id, d.network, d.last4,
       d.credit_limit_minor, d.statement_day, d.due_day, json_object('preset', d.design)
FROM card_details d JOIN accounts a ON a.id = d.account_id;

-- Every credit-card account gets a card, even without details yet.
INSERT INTO cards (id, kind, name, account_id, owns_account, institution_id)
SELECT 'card_' || a.id, 'credit', a.name, a.id, 1, a.institution_id
FROM accounts a
WHERE a.type = 'credit' AND NOT EXISTS (SELECT 1 FROM cards c WHERE c.account_id = a.id);

DROP TABLE card_details;
