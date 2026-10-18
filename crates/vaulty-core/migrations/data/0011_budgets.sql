-- Monthly budgets per expense category, in exact minor units (the old REAL column on categories
-- was never used by the app; any value in it is carried over).
CREATE TABLE budgets (
  id            TEXT PRIMARY KEY,
  category_id   TEXT NOT NULL UNIQUE REFERENCES categories(id) ON DELETE CASCADE,
  amount_minor  INTEGER NOT NULL CHECK (amount_minor > 0),
  currency      TEXT NOT NULL,
  created_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO budgets (id, category_id, amount_minor, currency)
SELECT 'bud_' || c.id, c.id, CAST(ROUND(c.budget_amount * 100) AS INTEGER), COALESCE((SELECT currency FROM user_settings WHERE id = 'default'), 'INR')
FROM categories c WHERE c.type = 'expense' AND c.budget_amount > 0;
