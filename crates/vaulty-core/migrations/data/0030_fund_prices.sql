-- When fund prices (AMFI's daily NAVs) were last fetched, so it's done at most once a day.
CREATE TABLE fund_price_fetch (
  id          TEXT PRIMARY KEY CHECK (id = 'default'),
  fetched_on  TEXT NOT NULL,   -- the day it was fetched
  as_of       TEXT,            -- the date of the prices in it
  updated     INTEGER NOT NULL DEFAULT 0
);
