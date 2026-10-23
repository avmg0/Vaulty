-- A bill item's warranty, kept as a document (with a reminder before it ends), and projects that
-- improve a property: what they cost counts towards the property's cost when it's sold.
ALTER TABLE project_items ADD COLUMN warranty_until TEXT;
ALTER TABLE project_items ADD COLUMN document_id TEXT REFERENCES vault_documents(id) ON DELETE SET NULL;
ALTER TABLE projects ADD COLUMN improvement INTEGER NOT NULL DEFAULT 0 CHECK (improvement IN (0, 1));
