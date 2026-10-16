-- Opt-in market ticker: preferences, provider keys and the last prices fetched (shown when offline).
CREATE TABLE market_prefs (
  id          TEXT PRIMARY KEY DEFAULT 'default' CHECK (id = 'default'),
  prefs       TEXT NOT NULL DEFAULT '{}',
  stock_key   TEXT,   -- market data API key for stocks; never sent back to the UI
  crypto_key  TEXT,   -- optional CoinGecko demo key
  updated_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO market_prefs (id) VALUES ('default');

CREATE TABLE market_quotes (
  kind        TEXT NOT NULL CHECK (kind IN ('stock', 'crypto')),
  symbol      TEXT NOT NULL,
  label       TEXT NOT NULL,
  price       REAL NOT NULL,
  change_pct  REAL,
  currency    TEXT,
  source      TEXT NOT NULL,
  as_of       TEXT NOT NULL,
  PRIMARY KEY (kind, symbol)
);
