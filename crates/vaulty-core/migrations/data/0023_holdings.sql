-- Investments held: mutual fund folios (typed in, or read from a CAMS/KFintech statement),
-- shares, ETFs, bonds, deposits, gold. Units and prices are exact: multiplied by 10,000.
CREATE TABLE holdings (
  id          TEXT PRIMARY KEY,
  kind        TEXT NOT NULL DEFAULT 'mutual_fund'
              CHECK (kind IN ('mutual_fund', 'stock', 'etf', 'bond', 'deposit', 'gold', 'other')),
  name        TEXT NOT NULL,
  isin        TEXT,
  symbol      TEXT,
  folio       TEXT,
  provider    TEXT,                    -- fund house or broker
  registrar   TEXT,                    -- CAMS or KFINTECH, for funds
  currency    TEXT NOT NULL DEFAULT 'INR',
  units_e4    INTEGER NOT NULL DEFAULT 0 CHECK (units_e4 >= 0),
  price_e4    INTEGER NOT NULL DEFAULT 0 CHECK (price_e4 >= 0),  -- latest NAV or price, per unit
  price_date  TEXT,
  cost_minor  INTEGER NOT NULL DEFAULT 0 CHECK (cost_minor >= 0), -- what the units now held cost
  source      TEXT NOT NULL DEFAULT 'manual' CHECK (source IN ('manual', 'cas')),
  statement_to TEXT,                   -- the latest statement read for it
  notes       TEXT,
  created_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE (isin, folio)
);
CREATE INDEX idx_holdings_kind ON holdings(kind);

-- Each holding's transactions, as the statement printed them. `source_key` stops the same row
-- being added twice when statements overlap.
CREATE TABLE holding_txns (
  id           TEXT PRIMARY KEY,
  holding_id   TEXT NOT NULL REFERENCES holdings(id) ON DELETE CASCADE,
  date         TEXT NOT NULL,
  kind         TEXT NOT NULL,
  description  TEXT NOT NULL DEFAULT '',
  amount_minor INTEGER,
  units_e4     INTEGER,
  price_e4     INTEGER,
  balance_e4   INTEGER,
  source_key   TEXT UNIQUE,
  created_at   TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_holding_txns_holding ON holding_txns(holding_id, date);

-- Holdings kept by the earlier version of the app come across.
INSERT INTO holdings (id, kind, name, symbol, currency, units_e4, price_e4, cost_minor, price_date, notes, source)
SELECT id,
       CASE lower(type) WHEN 'stock' THEN 'stock' WHEN 'etf' THEN 'etf' WHEN 'bond' THEN 'bond'
            WHEN 'mutual_fund' THEN 'mutual_fund' WHEN 'mutualfund' THEN 'mutual_fund' WHEN 'fund' THEN 'mutual_fund'
            WHEN 'gold' THEN 'gold' WHEN 'fd' THEN 'deposit' WHEN 'deposit' THEN 'deposit' ELSE 'other' END,
       name, NULLIF(trim(symbol), ''),
       coalesce((SELECT currency FROM user_settings WHERE id = 'default'), 'INR'),
       max(0, CAST(round(quantity * 10000) AS INTEGER)),
       max(0, CAST(round((CASE WHEN current_price > 0 THEN current_price ELSE buy_price END) * 10000) AS INTEGER)),
       max(0, CAST(round(quantity * buy_price * 100) AS INTEGER)),
       NULL, notes, 'manual'
FROM investments;
