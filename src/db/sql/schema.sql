PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS plaid_items (
  id TEXT PRIMARY KEY,
  item_id TEXT NOT NULL UNIQUE,
  access_token TEXT NOT NULL,
  institution_id TEXT,
  institution_name TEXT,
  cursor TEXT,
  access_mode TEXT NOT NULL DEFAULT 'read_only',
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS teller_enrollments (
  id TEXT PRIMARY KEY,
  enrollment_id TEXT NOT NULL UNIQUE,
  access_token TEXT NOT NULL,
  user_id TEXT,
  institution_id TEXT,
  institution_name TEXT,
  last_sync_date TEXT,
  access_mode TEXT NOT NULL DEFAULT 'read_only',
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS accounts (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  official_name TEXT,
  type TEXT NOT NULL,
  subtype TEXT,
  mask TEXT,
  balance_current REAL NOT NULL DEFAULT 0,
  balance_available REAL,
  currency TEXT NOT NULL DEFAULT 'USD',
  institution_name TEXT,
  plaid_account_id TEXT,
  plaid_item_id TEXT,
  teller_account_id TEXT,
  teller_enrollment_id TEXT,
  is_manual INTEGER NOT NULL DEFAULT 0,
  include_in_net_worth INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  FOREIGN KEY (plaid_item_id) REFERENCES plaid_items(id) ON DELETE SET NULL,
  FOREIGN KEY (teller_enrollment_id) REFERENCES teller_enrollments(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS transactions (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  plaid_transaction_id TEXT,
  teller_transaction_id TEXT,
  amount REAL NOT NULL,
  date TEXT NOT NULL,
  name TEXT NOT NULL,
  merchant_name TEXT,
  category TEXT NOT NULL DEFAULT 'Other',
  subcategory TEXT,
  pending INTEGER NOT NULL DEFAULT 0,
  is_subscription INTEGER NOT NULL DEFAULT 0,
  notes TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS budgets (
  id TEXT PRIMARY KEY,
  category TEXT NOT NULL UNIQUE,
  monthly_limit REAL NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_transactions_plaid_id
  ON transactions(plaid_transaction_id) WHERE plaid_transaction_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_transactions_teller_id
  ON transactions(teller_transaction_id) WHERE teller_transaction_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_transactions_date ON transactions(date);
CREATE INDEX IF NOT EXISTS idx_transactions_account ON transactions(account_id);
CREATE INDEX IF NOT EXISTS idx_transactions_category ON transactions(category);
CREATE INDEX IF NOT EXISTS idx_transactions_account_date_amount
  ON transactions(account_id, date, amount);
