import Database from "better-sqlite3";
import fs from "fs";
import path from "path";

let dbInstance: Database.Database | null = null;
let openedPath: string | null = null;

/** SQLite file for this process. `DATABASE_PATH` wins; otherwise `data/anchored.db`. */
export function resolveDatabasePath(): string {
  const configured = process.env.DATABASE_PATH?.trim();
  if (configured) return path.resolve(configured);
  return path.join(process.cwd(), "data", "anchored.db");
}

/** True when the database file is already on disk. Does not create it. */
export function databaseFileExists(): boolean {
  return fs.existsSync(resolveDatabasePath());
}

type StartupGlobal = typeof globalThis & {
  __anchoredDatabaseExistedAtStartup?: boolean;
};

/**
 * Whether the SQLite file was already present when this server process loaded.
 * Shared across Next.js route bundles via globalThis. Opening the database
 * later in the same process does not flip this to true.
 */
export function databaseExistedAtStartup(): boolean {
  const g = globalThis as StartupGlobal;
  if (g.__anchoredDatabaseExistedAtStartup === undefined) {
    g.__anchoredDatabaseExistedAtStartup = fs.existsSync(resolveDatabasePath());
  }
  return Boolean(g.__anchoredDatabaseExistedAtStartup);
}

// Record this before any route opens the file. Later imports reuse globalThis.
databaseExistedAtStartup();

function migrate(db: Database.Database) {
  db.exec(`
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
      is_manual INTEGER NOT NULL DEFAULT 0,
      include_in_net_worth INTEGER NOT NULL DEFAULT 1,
      created_at TEXT NOT NULL DEFAULT (datetime('now')),
      updated_at TEXT NOT NULL DEFAULT (datetime('now')),
      FOREIGN KEY (plaid_item_id) REFERENCES plaid_items(id) ON DELETE SET NULL
    );

    CREATE TABLE IF NOT EXISTS transactions (
      id TEXT PRIMARY KEY,
      account_id TEXT NOT NULL,
      plaid_transaction_id TEXT,
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
    CREATE INDEX IF NOT EXISTS idx_transactions_date ON transactions(date);
    CREATE INDEX IF NOT EXISTS idx_transactions_account ON transactions(account_id);
    CREATE INDEX IF NOT EXISTS idx_transactions_category ON transactions(category);
  `);

  // Existing DBs created before access_mode existed
  const cols = db.prepare("PRAGMA table_info(plaid_items)").all() as { name: string }[];
  if (!cols.some((c) => c.name === "access_mode")) {
    db.exec(
      `ALTER TABLE plaid_items ADD COLUMN access_mode TEXT NOT NULL DEFAULT 'read_only'`
    );
  }
}

export function getDb(): Database.Database {
  databaseExistedAtStartup();
  const dbPath = resolveDatabasePath();
  if (dbInstance && openedPath === dbPath) return dbInstance;
  if (dbInstance) {
    dbInstance.close();
    dbInstance = null;
    openedPath = null;
  }
  const dir = path.dirname(dbPath);
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }
  dbInstance = new Database(dbPath);
  openedPath = dbPath;
  migrate(dbInstance);
  return dbInstance;
}

export function resetDb() {
  const dbPath = resolveDatabasePath();
  if (dbInstance) {
    dbInstance.close();
    dbInstance = null;
    openedPath = null;
  }
  for (const suffix of ["", "-wal", "-shm"]) {
    const p = dbPath + suffix;
    if (fs.existsSync(p)) fs.unlinkSync(p);
  }
}
