-- Recurring payments, rebuilt: exact amounts, income and transfers as well as expenses, and a
-- schedule of "every N days/weeks/months/years" from a start date. Nothing linked to the old
-- table (it was never used by this app), so it is replaced; any rows in it are carried over.
CREATE TABLE recurring_payments_new (
  id             TEXT PRIMARY KEY,
  name           TEXT NOT NULL,
  type           TEXT NOT NULL DEFAULT 'expense' CHECK (type IN ('expense', 'income', 'transfer')),
  amount_minor   INTEGER NOT NULL CHECK (amount_minor > 0),
  currency       TEXT NOT NULL,
  account_id     TEXT REFERENCES accounts(id) ON DELETE SET NULL,
  to_account_id  TEXT REFERENCES accounts(id) ON DELETE SET NULL,
  category_id    TEXT REFERENCES categories(id) ON DELETE SET NULL,
  merchant       TEXT,
  notes          TEXT,
  tags           TEXT,
  frequency      TEXT NOT NULL CHECK (frequency IN ('daily', 'weekly', 'monthly', 'yearly')),
  every          INTEGER NOT NULL DEFAULT 1 CHECK (every BETWEEN 1 AND 366),
  start_date     TEXT NOT NULL,
  end_date       TEXT,
  next_due_date  TEXT,
  last_paid_date TEXT,
  reminder_days  INTEGER NOT NULL DEFAULT 3 CHECK (reminder_days BETWEEN 0 AND 60),
  auto_pay       INTEGER NOT NULL DEFAULT 0 CHECK (auto_pay IN (0, 1)),
  is_active      INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
  visibility     TEXT NOT NULL DEFAULT 'private' CHECK (visibility IN ('private', 'shared', 'public')),
  created_at     TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at     TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO recurring_payments_new (id, name, type, amount_minor, currency, account_id, category_id, notes, frequency, every,
                                    start_date, end_date, next_due_date, last_paid_date, reminder_days, auto_pay, is_active)
SELECT r.id, r.name, 'expense', CAST(ROUND(ABS(r.amount) * 100) AS INTEGER),
       COALESCE((SELECT currency FROM accounts WHERE id = r.account_id), (SELECT currency FROM user_settings WHERE id = 'default'), 'INR'),
       r.account_id, r.category_id, r.notes,
       CASE r.frequency WHEN 'daily' THEN 'daily' WHEN 'weekly' THEN 'weekly' WHEN 'biweekly' THEN 'weekly' WHEN 'yearly' THEN 'yearly'
                        WHEN 'custom' THEN 'daily' ELSE 'monthly' END,
       CASE r.frequency WHEN 'biweekly' THEN 2 WHEN 'quarterly' THEN 3 WHEN 'custom' THEN MAX(1, MIN(366, COALESCE(r.custom_days, 1))) ELSE 1 END,
       r.start_date, r.end_date, r.next_due_date, r.last_paid_date, MAX(0, MIN(60, COALESCE(r.reminder_days, 3))), COALESCE(r.auto_pay, 0), COALESCE(r.is_active, 1)
FROM recurring_payments r WHERE ABS(r.amount) > 0;

DROP TABLE recurring_payments;
ALTER TABLE recurring_payments_new RENAME TO recurring_payments;
CREATE INDEX idx_recurring_next_due ON recurring_payments(next_due_date);
CREATE INDEX idx_entries_recurring ON entries(recurring_id);
