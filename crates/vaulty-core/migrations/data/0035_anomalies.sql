-- Unusual transactions the person has looked at and said are fine.
CREATE TABLE anomaly_dismissed (
  key TEXT PRIMARY KEY,
  at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
