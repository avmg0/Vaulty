-- The tax planner's figures kept per country and year, tax already paid (TDS, advance tax,
-- self-assessment), and which categories feed which figure ("health insurance premium" → the
-- health insurance deduction), so the planner can fill itself from transactions.
CREATE TABLE tax_plans (
  id          TEXT PRIMARY KEY,               -- country:year, e.g. IN:FY2026-27
  country     TEXT NOT NULL,
  year_id     TEXT NOT NULL,
  input       TEXT NOT NULL DEFAULT '{}',     -- age, salary, other income, deductions, options
  regime      TEXT,                           -- the regime chosen; none: whichever is lower
  hra         TEXT NOT NULL DEFAULT '{}',     -- rent and HRA details, for the exemption and rent receipts
  created_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE tax_payments (
  id           TEXT PRIMARY KEY,
  plan_id      TEXT NOT NULL REFERENCES tax_plans(id) ON DELETE CASCADE,
  kind         TEXT NOT NULL CHECK (kind IN ('tds', 'advance', 'self_assessment')),
  date         TEXT NOT NULL,
  amount_minor INTEGER NOT NULL CHECK (amount_minor > 0),
  note         TEXT,
  created_at   TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_tax_payments_plan ON tax_payments(plan_id, date);

CREATE TABLE tax_links (
  id           TEXT PRIMARY KEY,
  category_id  TEXT NOT NULL UNIQUE REFERENCES categories(id) ON DELETE CASCADE,
  target       TEXT NOT NULL                   -- a deduction id, or "salary" or "other_income"
);
