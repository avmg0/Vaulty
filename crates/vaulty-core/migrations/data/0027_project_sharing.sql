-- Splitting a project's costs with the people in it, and sharing a project as a file.
-- Members are the people sharing the costs, each with a share (equal by default). A payment can
-- say which member paid it; none means you.
CREATE TABLE project_members (
  id          TEXT PRIMARY KEY,
  project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  name        TEXT NOT NULL,
  share       REAL NOT NULL DEFAULT 1 CHECK (share >= 0 AND share <= 1000),
  is_me       INTEGER NOT NULL DEFAULT 0,
  position    INTEGER NOT NULL DEFAULT 0,
  created_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE (project_id, name COLLATE NOCASE)
);
ALTER TABLE project_payments ADD COLUMN paid_by TEXT REFERENCES project_members(id) ON DELETE SET NULL;
-- The same project in everyone's Vaulty has the same share id, so a file sent back updates it.
ALTER TABLE projects ADD COLUMN share_id TEXT;
CREATE UNIQUE INDEX idx_projects_share ON projects(share_id) WHERE share_id IS NOT NULL;
