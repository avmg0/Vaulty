-- Documents: passports, policies, property papers and the like. The details live here; each file
-- is encrypted separately on disk (see documents.rs), so safety copies of this database stay small.
CREATE TABLE vault_documents (
  id           TEXT PRIMARY KEY,
  title        TEXT NOT NULL,
  kind         TEXT NOT NULL DEFAULT 'other',
  holder       TEXT,              -- whose document it is
  number       TEXT,              -- policy, passport or account number
  issuer       TEXT,
  issued_on    TEXT,
  expires_on   TEXT,
  remind_days  INTEGER NOT NULL DEFAULT 30,
  kept_at      TEXT,              -- where the paper original is
  notes        TEXT,
  created_at   TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at   TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  deleted_at   TEXT               -- in the trash (undo brings it back); purged after 30 days
);
CREATE INDEX idx_vault_documents_expiry ON vault_documents(expires_on) WHERE deleted_at IS NULL;

CREATE TABLE vault_document_files (
  id           TEXT PRIMARY KEY,
  document_id  TEXT NOT NULL REFERENCES vault_documents(id) ON DELETE CASCADE,
  name         TEXT NOT NULL,
  mime         TEXT NOT NULL,
  size         INTEGER NOT NULL,
  sha256       TEXT NOT NULL,
  created_at   TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_vault_document_files_doc ON vault_document_files(document_id);
