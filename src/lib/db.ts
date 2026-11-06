import Database from "better-sqlite3-multiple-ciphers";
import fs from "fs";
import path from "path";

type SqliteDb = InstanceType<typeof Database>;

type DbGlobal = typeof globalThis & {
  __anchoredDb?: SqliteDb;
  __anchoredDbPath?: string;
  __anchoredDbUmask?: boolean;
  __anchoredDbExitHook?: boolean;
};

const PLAINTEXT_MAGIC = Buffer.from("SQLite format 3\0", "utf8");
const KEY_PLACEHOLDER = "your_database_key";

function dbGlobal(): DbGlobal {
  return globalThis as DbGlobal;
}

function usePrivateUmask() {
  const g = dbGlobal();
  if (g.__anchoredDbUmask) return;
  g.__anchoredDbUmask = true;
  if (process.platform === "win32") return;
  if (process.env.NEXT_PHASE === "phase-production-build") return;
  process.umask(0o077);
}

function restrictPrivateFile(file: string) {
  try {
    if (fs.existsSync(file)) fs.chmodSync(file, 0o600);
  } catch {
    // A read-only filesystem or a Windows ACL we cannot change. The file still opens.
  }
}

function restrictDatabaseFiles(dbPath: string) {
  for (const suffix of ["", "-wal", "-shm"]) restrictPrivateFile(dbPath + suffix);
}

function closeStoredDb() {
  const g = dbGlobal();
  const current = g.__anchoredDb;
  g.__anchoredDb = undefined;
  g.__anchoredDbPath = undefined;
  if (current && current.open) {
    try {
      current.close();
    } catch {
      // already closed
    }
  }
}

function assignEnvFromFile(file: string) {
  if (!fs.existsSync(file)) return;
  const text = fs.readFileSync(file, "utf8");
  for (const line of text.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const eq = trimmed.indexOf("=");
    if (eq <= 0) continue;
    const key = trimmed.slice(0, eq).trim();
    if (key !== "DATABASE_KEY" && key !== "DATABASE_PATH") continue;
    if (process.env[key]?.trim()) continue;
    let value = trimmed.slice(eq + 1).trim();
    if (
      (value.startsWith('"') && value.endsWith('"')) ||
      (value.startsWith("'") && value.endsWith("'"))
    ) {
      value = value.slice(1, -1);
    }
    if (value) process.env[key] = value;
  }
}

/** Fill DATABASE_KEY and DATABASE_PATH from .env.local when the process did not set them. */
function loadLocalDatabaseEnv() {
  if (process.env.DATABASE_KEY?.trim() && process.env.DATABASE_PATH?.trim()) return;
  assignEnvFromFile(path.join(process.cwd(), ".env.local"));
  assignEnvFromFile(path.join(process.cwd(), ".env"));
}

function databaseKey(): Buffer {
  loadLocalDatabaseEnv();
  const value = process.env.DATABASE_KEY?.trim() ?? "";
  if (value.length < 16 || value === KEY_PLACEHOLDER) {
    throw new Error(
      "DATABASE_KEY is missing. Anchored will not open or create the database. Set a long random DATABASE_KEY in the environment or in .env.local."
    );
  }
  return Buffer.from(value, "utf8");
}

/** SQLite file for this process. `DATABASE_PATH` wins; otherwise `data/anchored.db`. */
export function resolveDatabasePath(): string {
  loadLocalDatabaseEnv();
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

function isPlaintextDatabase(dbPath: string): boolean {
  let fd: number | null = null;
  try {
    const size = fs.statSync(dbPath).size;
    if (size < PLAINTEXT_MAGIC.length) return false;
    fd = fs.openSync(dbPath, "r");
    const header = Buffer.alloc(PLAINTEXT_MAGIC.length);
    const read = fs.readSync(fd, header, 0, header.length, 0);
    return read === header.length && header.equals(PLAINTEXT_MAGIC);
  } catch {
    return false;
  } finally {
    if (fd != null) fs.closeSync(fd);
  }
}

function tableCounts(db: SqliteDb): Map<string, number> {
  const tables = db
    .prepare(
      `SELECT name FROM sqlite_master
       WHERE type = 'table' AND name NOT LIKE 'sqlite_%'`
    )
    .all() as { name: string }[];
  const counts = new Map<string, number>();
  for (const table of tables) {
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(table.name)) {
      throw new Error("The database has a table Anchored cannot migrate. The original file was left unchanged.");
    }
    const row = db.prepare(`SELECT count(*) AS c FROM "${table.name}"`).get() as { c: number };
    counts.set(table.name, Number(row.c));
  }
  return counts;
}

function sameCounts(left: Map<string, number>, right: Map<string, number>): boolean {
  if (left.size !== right.size) return false;
  for (const [name, count] of left) {
    if (right.get(name) !== count) return false;
  }
  return true;
}

function applyKey(db: SqliteDb, key: Buffer) {
  db.pragma("cipher = 'sqlcipher'");
  db.pragma("legacy = 4");
  db.key(key);
}

function openEncrypted(dbPath: string, key: Buffer): SqliteDb {
  const db = new Database(dbPath, { fileMustExist: true });
  try {
    applyKey(db, key);
    db.prepare("SELECT count(*) AS c FROM sqlite_master").get();
    return db;
  } catch (err) {
    try {
      db.close();
    } catch {
      // already closed
    }
    throw err;
  }
}

function removeSidecars(dbPath: string) {
  for (const suffix of ["-wal", "-shm"]) fs.rmSync(dbPath + suffix, { force: true });
}

/**
 * Copy a plaintext database to a SQLCipher file, check the rows survived,
 * then replace the original. The plaintext file stays in place until that check passes.
 */
function migratePlaintextDatabase(dbPath: string, key: Buffer) {
  const plain = new Database(dbPath, { fileMustExist: true });
  let before: Map<string, number>;
  try {
    const checkpoint = plain.pragma("wal_checkpoint(TRUNCATE)") as { busy: number }[];
    if (checkpoint.some((row) => row.busy !== 0)) {
      throw new Error("The database is in use, so it was not encrypted. The original file was left unchanged.");
    }
    before = tableCounts(plain);
  } finally {
    plain.close();
  }

  const tmp = `${dbPath}.encrypting`;
  const bak = `${dbPath}.plaintext-bak`;
  fs.rmSync(tmp, { force: true });
  removeSidecars(tmp);
  fs.copyFileSync(dbPath, tmp);
  restrictPrivateFile(tmp);

  const migrated = new Database(tmp);
  try {
    migrated.pragma("cipher = 'sqlcipher'");
    migrated.pragma("legacy = 4");
    if (migrated.rekey(key) !== 0) {
      throw new Error("Could not encrypt the existing database. The original file was left unchanged.");
    }
    const after = tableCounts(migrated);
    const quick = migrated.pragma("quick_check", { simple: true });
    if (!sameCounts(before, after) || quick !== "ok") {
      throw new Error(
        "Encrypting the database would have changed its data. The original file was left unchanged."
      );
    }
    migrated.pragma("wal_checkpoint(TRUNCATE)");
  } catch (err) {
    try {
      migrated.close();
    } catch {
      // already closed
    }
    fs.rmSync(tmp, { force: true });
    removeSidecars(tmp);
    if (err instanceof Error && /left unchanged|restored/.test(err.message)) throw err;
    throw new Error("Could not encrypt the existing database. The original file was left unchanged.");
  }
  migrated.close();
  removeSidecars(tmp);

  if (isPlaintextDatabase(tmp)) {
    fs.rmSync(tmp, { force: true });
    throw new Error("Encryption did not protect the database file. The original file was left unchanged.");
  }

  if (fs.existsSync(bak)) fs.rmSync(bak, { force: true });
  fs.renameSync(dbPath, bak);
  restrictPrivateFile(bak);
  removeSidecars(dbPath);
  try {
    fs.renameSync(tmp, dbPath);
    restrictPrivateFile(dbPath);
    const verify = openEncrypted(dbPath, key);
    try {
      const after = tableCounts(verify);
      if (!sameCounts(before, after)) {
        throw new Error("The encrypted database did not match the original. The original file was restored.");
      }
    } finally {
      verify.close();
    }
  } catch (err) {
    try {
      fs.rmSync(dbPath, { force: true });
    } catch {
      // the encrypted file may already be gone
    }
    removeSidecars(dbPath);
    if (fs.existsSync(bak)) fs.renameSync(bak, dbPath);
    fs.rmSync(tmp, { force: true });
    removeSidecars(tmp);
    if (err instanceof Error && /left unchanged|restored/.test(err.message)) throw err;
    throw new Error("The encrypted database did not match the original. The original file was restored.");
  }

  fs.rmSync(bak, { force: true });
  removeSidecars(bak);
  restrictDatabaseFiles(dbPath);
}

function recoverInterruptedEncryption(dbPath: string) {
  const bak = `${dbPath}.plaintext-bak`;
  const tmp = `${dbPath}.encrypting`;
  const mainReady = fs.existsSync(dbPath) && fs.statSync(dbPath).size > 0;
  if (!mainReady && fs.existsSync(tmp) && !isPlaintextDatabase(tmp)) {
    if (fs.existsSync(dbPath)) fs.rmSync(dbPath, { force: true });
    fs.renameSync(tmp, dbPath);
    restrictPrivateFile(dbPath);
    return;
  }
  if (!mainReady && fs.existsSync(bak)) {
    if (fs.existsSync(dbPath)) fs.rmSync(dbPath, { force: true });
    fs.renameSync(bak, dbPath);
    restrictPrivateFile(dbPath);
  }
  if (fs.existsSync(tmp)) {
    fs.rmSync(tmp, { force: true });
    removeSidecars(tmp);
  }
}

function migrate(db: SqliteDb) {
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

function openDatabase(dbPath: string, key: Buffer): SqliteDb {
  recoverInterruptedEncryption(dbPath);
  if (fs.existsSync(dbPath) && fs.statSync(dbPath).size === 0) {
    fs.rmSync(dbPath, { force: true });
  }
  if (fs.existsSync(dbPath) && isPlaintextDatabase(dbPath)) {
    migratePlaintextDatabase(dbPath, key);
  }

  if (!fs.existsSync(dbPath)) {
    const created = new Database(dbPath);
    try {
      applyKey(created, key);
      created.prepare("SELECT count(*) AS c FROM sqlite_master").get();
      restrictPrivateFile(dbPath);
      return created;
    } catch (err) {
      try {
        created.close();
      } catch {
        // already closed
      }
      if (fs.existsSync(dbPath) && (fs.statSync(dbPath).size === 0 || isPlaintextDatabase(dbPath))) {
        fs.rmSync(dbPath, { force: true });
      }
      throw new Error("The database could not be created. Anchored did not leave it open.");
    }
  }

  try {
    const db = openEncrypted(dbPath, key);
    const bak = `${dbPath}.plaintext-bak`;
    if (fs.existsSync(bak)) fs.rmSync(bak, { force: true });
    return db;
  } catch {
    throw new Error(
      "The database could not be opened with DATABASE_KEY. Anchored left the file in place and did not replace it."
    );
  }
}

export function getDb(): SqliteDb {
  databaseExistedAtStartup();
  usePrivateUmask();
  const key = databaseKey();
  const g = dbGlobal();
  const dbPath = resolveDatabasePath();
  if (g.__anchoredDb && g.__anchoredDbPath === dbPath && g.__anchoredDb.open) {
    restrictDatabaseFiles(dbPath);
    return g.__anchoredDb;
  }
  closeStoredDb();

  const dir = path.dirname(dbPath);
  const createdDir = !fs.existsSync(dir);
  if (createdDir) {
    fs.mkdirSync(dir, { recursive: true, mode: 0o700 });
    if (process.platform !== "win32" && dir !== path.parse(dir).root) {
      try {
        fs.chmodSync(dir, 0o700);
      } catch {
        // directory is usable even if the mode cannot be tightened
      }
    }
  }

  if (!g.__anchoredDbExitHook) {
    g.__anchoredDbExitHook = true;
    process.on("exit", () => {
      const openDb = dbGlobal().__anchoredDb;
      if (openDb?.open) {
        try {
          openDb.close();
        } catch {
          // process is exiting
        }
      }
    });
  }

  const db = openDatabase(dbPath, key);
  g.__anchoredDb = db;
  g.__anchoredDbPath = dbPath;
  migrate(db);
  restrictDatabaseFiles(dbPath);
  return db;
}

export function resetDb() {
  const dbPath = resolveDatabasePath();
  closeStoredDb();
  for (const suffix of ["", "-wal", "-shm", ".plaintext-bak", ".encrypting"]) {
    fs.rmSync(dbPath + suffix, { force: true });
  }
  removeSidecars(`${dbPath}.plaintext-bak`);
  removeSidecars(`${dbPath}.encrypting`);
}
