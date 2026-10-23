-- Bills, invoices and receipts kept with a project, encrypted like documents' files (in the
-- member's documents folder as <id>.vdoc). A file can point to the bill item or payment it's for;
-- deleting that item or payment keeps the file in the project.
CREATE TABLE project_files (
  id          TEXT PRIMARY KEY,
  project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  item_id     TEXT REFERENCES project_items(id) ON DELETE SET NULL,
  payment_id  TEXT REFERENCES project_payments(id) ON DELETE SET NULL,
  name        TEXT NOT NULL,
  mime        TEXT NOT NULL,
  size        INTEGER NOT NULL,
  sha256      TEXT NOT NULL,
  created_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_project_files_project ON project_files(project_id);
CREATE INDEX idx_project_files_item ON project_files(item_id);
CREATE INDEX idx_project_files_payment ON project_files(payment_id);
