-- Vaulty per-user data schema v1 (one database file per family member) (ported from electron/database/connection.ts, with handler/schema mismatches fixed)
CREATE TABLE IF NOT EXISTS user_settings (
      id TEXT PRIMARY KEY DEFAULT 'default',
      theme TEXT DEFAULT 'dark',
      currency TEXT DEFAULT 'USD',
      locale TEXT DEFAULT 'en-US',
      date_format TEXT DEFAULT 'MM/dd/yyyy',
      start_of_week TEXT DEFAULT 'sunday',
      fiscal_year_start TEXT DEFAULT 'january',
      negative_format TEXT DEFAULT 'minus',
      transaction_sort TEXT DEFAULT 'date_desc',
      auto_clear_clipboard BOOLEAN DEFAULT 1,
      spellcheck_enabled BOOLEAN DEFAULT 1,
      font_size TEXT DEFAULT 'medium',
      interface_font TEXT DEFAULT 'system',
      monospace_font TEXT DEFAULT 'system',
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );

CREATE TABLE IF NOT EXISTS accounts (
      id TEXT PRIMARY KEY,
      name TEXT NOT NULL,
      type TEXT NOT NULL,
      institution TEXT,
      initial_balance REAL DEFAULT 0,
      current_balance REAL DEFAULT 0,
      currency TEXT DEFAULT 'USD',
      icon TEXT,
      color TEXT,
      is_active BOOLEAN DEFAULT 1,
      include_in_total BOOLEAN DEFAULT 1,
      notes TEXT,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );

CREATE TABLE IF NOT EXISTS categories (
      id TEXT PRIMARY KEY,
      name TEXT NOT NULL,
      type TEXT NOT NULL,
      icon TEXT,
      color TEXT,
      budget_amount REAL,
      parent_id TEXT,
      is_system BOOLEAN DEFAULT 0,
      is_active BOOLEAN DEFAULT 1,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      FOREIGN KEY (parent_id) REFERENCES categories(id) ON DELETE SET NULL
    );

CREATE TABLE IF NOT EXISTS transactions (
      id TEXT PRIMARY KEY,
      account_id TEXT NOT NULL,
      category_id TEXT,
      type TEXT NOT NULL,
      amount REAL NOT NULL,
      date DATE NOT NULL,
      time TEXT,
      merchant TEXT,
      description TEXT,
      notes TEXT,
      tags TEXT,
      is_recurring BOOLEAN DEFAULT 0,
      recurring_id TEXT,
      is_pending BOOLEAN DEFAULT 0,
      is_cleared BOOLEAN DEFAULT 1,
      receipt_path TEXT,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE,
      FOREIGN KEY (category_id) REFERENCES categories(id) ON DELETE SET NULL
    );

CREATE INDEX IF NOT EXISTS idx_transactions_date ON transactions(date);
    CREATE INDEX IF NOT EXISTS idx_transactions_account ON transactions(account_id);
    CREATE INDEX IF NOT EXISTS idx_transactions_category ON transactions(category_id);
    CREATE INDEX IF NOT EXISTS idx_transactions_type ON transactions(type);

CREATE TABLE IF NOT EXISTS credit_cards (
      id TEXT PRIMARY KEY,
      account_id TEXT,
      bank_name TEXT,
      card_number_last4 TEXT,
      card_type TEXT,
      credit_limit REAL DEFAULT 0,
      current_balance REAL DEFAULT 0,
      available_credit REAL DEFAULT 0,
      apr REAL,
      annual_fee REAL DEFAULT 0,
      statement_day INTEGER,
      payment_due_day INTEGER,
      reward_type TEXT,
      reward_rate REAL,
      notes TEXT,
      is_active BOOLEAN DEFAULT 1,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE SET NULL
    );

CREATE TABLE IF NOT EXISTS recurring_payments (
      id TEXT PRIMARY KEY,
      name TEXT NOT NULL,
      amount REAL NOT NULL,
      frequency TEXT NOT NULL DEFAULT 'monthly',
      custom_days INTEGER,
      start_date DATE NOT NULL,
      end_date DATE,
      next_due_date DATE NOT NULL,
      last_paid_date DATE,
      category_id TEXT,
      account_id TEXT,
      is_active BOOLEAN DEFAULT 1,
      auto_pay BOOLEAN DEFAULT 0,
      reminder_days INTEGER DEFAULT 3,
      notes TEXT,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      FOREIGN KEY (category_id) REFERENCES categories(id) ON DELETE SET NULL,
      FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE SET NULL
    );

CREATE INDEX IF NOT EXISTS idx_recurring_next_due ON recurring_payments(next_due_date);
    CREATE INDEX IF NOT EXISTS idx_recurring_active ON recurring_payments(is_active);

CREATE TABLE IF NOT EXISTS tasks (
      id TEXT PRIMARY KEY,
      title TEXT NOT NULL,
      description TEXT,
      due_date DATE,
      priority TEXT DEFAULT 'medium',
      status TEXT DEFAULT 'pending',
      category TEXT,
      linked_type TEXT,
      linked_id TEXT,
      linked_name TEXT,
      tags TEXT,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      completed_at TIMESTAMP
    );

CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
    CREATE INDEX IF NOT EXISTS idx_tasks_due ON tasks(due_date);
    CREATE INDEX IF NOT EXISTS idx_tasks_priority ON tasks(priority);

CREATE TABLE IF NOT EXISTS investments (
      id TEXT PRIMARY KEY,
      symbol TEXT NOT NULL,
      name TEXT NOT NULL,
      type TEXT NOT NULL DEFAULT 'stock',
      quantity REAL NOT NULL DEFAULT 0,
      buy_price REAL NOT NULL DEFAULT 0,
      current_price REAL NOT NULL DEFAULT 0,
      buy_date DATE,
      account_id TEXT,
      notes TEXT,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE SET NULL
    );

CREATE INDEX IF NOT EXISTS idx_holdings_symbol ON investments(symbol);
    CREATE INDEX IF NOT EXISTS idx_holdings_type ON investments(type);

CREATE TABLE IF NOT EXISTS investment_events (
      id TEXT PRIMARY KEY,
      holding_id TEXT NOT NULL,
      type TEXT NOT NULL DEFAULT 'dividend',
      amount REAL NOT NULL DEFAULT 0,
      date DATE NOT NULL,
      notes TEXT,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      FOREIGN KEY (holding_id) REFERENCES investments(id) ON DELETE CASCADE
    );

CREATE TABLE IF NOT EXISTS journal_entries (
      id TEXT PRIMARY KEY,
      date DATE NOT NULL,
      title TEXT NOT NULL,
      content TEXT,
      mood TEXT,
      tags TEXT,
      linked_transactions TEXT,
      month_summary BOOLEAN DEFAULT 0,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );

CREATE INDEX IF NOT EXISTS idx_journal_date ON journal_entries(date);

CREATE TABLE IF NOT EXISTS goals (
      id TEXT PRIMARY KEY,
      name TEXT NOT NULL,
      type TEXT NOT NULL DEFAULT 'savings',
      target_amount REAL NOT NULL DEFAULT 0,
      current_amount REAL NOT NULL DEFAULT 0,
      monthly_contribution REAL DEFAULT 0,
      target_date DATE,
      priority TEXT DEFAULT 'medium',
      icon TEXT,
      color TEXT,
      notes TEXT,
      is_active BOOLEAN DEFAULT 1,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );

CREATE INDEX IF NOT EXISTS idx_goals_type ON goals(type);
    CREATE INDEX IF NOT EXISTS idx_goals_active ON goals(is_active);

CREATE TABLE IF NOT EXISTS notifications (
      id TEXT PRIMARY KEY,
      type TEXT NOT NULL,
      title TEXT NOT NULL,
      message TEXT,
      priority TEXT DEFAULT 'medium',
      is_read BOOLEAN DEFAULT 0,
      action_type TEXT,
      action_id TEXT,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );

CREATE INDEX IF NOT EXISTS idx_notifications_read ON notifications(is_read);

CREATE TABLE IF NOT EXISTS documents (
      id TEXT PRIMARY KEY,
      name TEXT NOT NULL,
      file_type TEXT NOT NULL DEFAULT 'pdf',
      file_data TEXT,
      file_path TEXT,
      file_size INTEGER,
      category TEXT,
      tags TEXT,
      linked_type TEXT,
      linked_id TEXT,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );

CREATE TABLE IF NOT EXISTS calendar_events (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    date DATE NOT NULL,
    time TIME,
    end_time TIME,
    type TEXT NOT NULL DEFAULT 'custom',
    amount REAL,
    color TEXT DEFAULT '#3b82f6',
    icon TEXT DEFAULT '📌',
    is_recurring BOOLEAN DEFAULT 0,
    recurring_pattern TEXT,
    reminder_minutes INTEGER,
    notes TEXT,
    linked_type TEXT,
    linked_id TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
  );

CREATE INDEX IF NOT EXISTS idx_calendar_events_date ON calendar_events(date);
  CREATE INDEX IF NOT EXISTS idx_calendar_events_type ON calendar_events(type);

CREATE TRIGGER IF NOT EXISTS update_calendar_events_timestamp
  AFTER UPDATE ON calendar_events
  BEGIN
    UPDATE calendar_events SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
  END;

CREATE TABLE IF NOT EXISTS hotkeys (
      id TEXT PRIMARY KEY,
      action TEXT NOT NULL UNIQUE,
      keys TEXT NOT NULL,
      category TEXT NOT NULL,
      is_custom BOOLEAN DEFAULT 0,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );

CREATE TRIGGER IF NOT EXISTS update_settings_timestamp
    AFTER UPDATE ON user_settings
    BEGIN
      UPDATE user_settings SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
    END;

CREATE TRIGGER IF NOT EXISTS update_accounts_timestamp
    AFTER UPDATE ON accounts
    BEGIN
      UPDATE accounts SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
    END;

CREATE TRIGGER IF NOT EXISTS update_transactions_timestamp
    AFTER UPDATE ON transactions
    BEGIN
      UPDATE transactions SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
    END;

CREATE TRIGGER IF NOT EXISTS update_recurring_timestamp
    AFTER UPDATE ON recurring_payments
    BEGIN
      UPDATE recurring_payments SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
    END;

CREATE TRIGGER IF NOT EXISTS update_tasks_timestamp
    AFTER UPDATE ON tasks
    BEGIN
      UPDATE tasks SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
    END;

CREATE TRIGGER IF NOT EXISTS update_investments_timestamp
    AFTER UPDATE ON investments
    BEGIN
      UPDATE investments SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
    END;

CREATE TRIGGER IF NOT EXISTS update_journal_timestamp
    AFTER UPDATE ON journal_entries
    BEGIN
      UPDATE journal_entries SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
    END;

CREATE TRIGGER IF NOT EXISTS update_goals_timestamp
    AFTER UPDATE ON goals
    BEGIN
      UPDATE goals SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
    END;
