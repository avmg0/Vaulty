-- Column mappings remembered per account, so the next statement from the same bank imports in
-- one step. `header_signature` lets a mapping be suggested for another account with the same layout.
CREATE TABLE import_templates (
  id               TEXT PRIMARY KEY,
  account_id       TEXT UNIQUE REFERENCES accounts(id) ON DELETE CASCADE,
  header_signature TEXT NOT NULL,
  mapping          TEXT NOT NULL,
  updated_at       TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_import_templates_signature ON import_templates(header_signature);
