-- Recording a project's split in Lend & borrow: which person each member is there, and how much
-- of what's between you has been recorded already (positive: they owe you), so recording again
-- after more payments adds only the difference.
ALTER TABLE project_members ADD COLUMN person_id TEXT REFERENCES people(id) ON DELETE SET NULL;
ALTER TABLE project_members ADD COLUMN recorded_minor INTEGER NOT NULL DEFAULT 0;
