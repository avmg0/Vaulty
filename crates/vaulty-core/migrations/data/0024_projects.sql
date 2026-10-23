-- Projects: a home renovation, a wedding, a trip, a business set-up. Spending is kept as bill
-- items (what was bought or charged, at what rate) and payments (money that went out, often in
-- advances and instalments), so what's still owed to each person can be worked out.
CREATE TABLE projects (
  id           TEXT PRIMARY KEY,
  name         TEXT NOT NULL,
  kind         TEXT NOT NULL DEFAULT 'personal' CHECK (kind IN ('personal', 'family', 'group', 'business')),
  status       TEXT NOT NULL DEFAULT 'ongoing' CHECK (status IN ('planning', 'ongoing', 'on_hold', 'done')),
  currency     TEXT NOT NULL DEFAULT 'INR',
  budget_minor INTEGER CHECK (budget_minor IS NULL OR budget_minor >= 0),
  start_date   TEXT,
  end_date     TEXT,
  notes        TEXT,
  columns      TEXT NOT NULL DEFAULT '[]',  -- the project's own columns: [{id, name, type, options}]
  hidden       TEXT NOT NULL DEFAULT '[]',  -- standard columns not shown: ["tax", "bill_no", …]
  created_at   TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at   TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Parts of a project (electrical work, plumbing, painting), each with an optional budget.
CREATE TABLE project_sections (
  id           TEXT PRIMARY KEY,
  project_id   TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  name         TEXT NOT NULL,
  budget_minor INTEGER CHECK (budget_minor IS NULL OR budget_minor >= 0),
  position     INTEGER NOT NULL DEFAULT 0,
  created_at   TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE (project_id, name COLLATE NOCASE)
);

-- What was bought or charged: one row per line of a bill.
CREATE TABLE project_items (
  id          TEXT PRIMARY KEY,
  project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  section_id  TEXT REFERENCES project_sections(id) ON DELETE SET NULL,
  date        TEXT NOT NULL,
  item        TEXT NOT NULL,
  kind        TEXT NOT NULL DEFAULT 'material' CHECK (kind IN ('material', 'labour', 'fee', 'transport', 'tax', 'other')),
  qty_e4      INTEGER CHECK (qty_e4 IS NULL OR qty_e4 > 0),   -- quantity × 10,000
  unit        TEXT,                                            -- pcs, kg, sq ft, m, day…
  rate_minor  INTEGER CHECK (rate_minor IS NULL OR rate_minor >= 0),
  amount_minor INTEGER NOT NULL CHECK (amount_minor >= 0),     -- before tax
  tax_minor   INTEGER NOT NULL DEFAULT 0 CHECK (tax_minor >= 0),
  vendor      TEXT,                                            -- who it was bought from or who did the work
  bill_no     TEXT,
  notes       TEXT,
  custom      TEXT NOT NULL DEFAULT '{}',                      -- values of the project's own columns
  created_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_project_items_project ON project_items(project_id, date);

-- Money paid out for the project. Can be linked to the transaction it went out as.
CREATE TABLE project_payments (
  id           TEXT PRIMARY KEY,
  project_id   TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  section_id   TEXT REFERENCES project_sections(id) ON DELETE SET NULL,
  date         TEXT NOT NULL,
  amount_minor INTEGER NOT NULL CHECK (amount_minor > 0),
  vendor       TEXT,                    -- paid to
  mode         TEXT NOT NULL DEFAULT 'upi' CHECK (mode IN ('cash', 'upi', 'card', 'bank', 'cheque', 'other')),
  account_name TEXT,                    -- the name the bank shows for the person paid
  reference    TEXT,                    -- UTR, cheque or receipt number
  entry_id     TEXT REFERENCES entries(id) ON DELETE SET NULL,
  notes        TEXT,
  created_at   TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at   TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_project_payments_project ON project_payments(project_id, date);
CREATE INDEX idx_project_payments_entry ON project_payments(entry_id);
