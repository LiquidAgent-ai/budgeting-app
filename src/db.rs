use crate::log::{error_event, info_event};
use rusqlite::{Connection, OpenFlags};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

const PLAINTEXT_MAGIC: &[u8] = b"SQLite format 3\0";

pub struct Db {
    path: PathBuf,
    key: String,
    conn: Mutex<Connection>,
    existed_at_startup: bool,
}

impl Db {
    pub fn open(path: PathBuf, key: String) -> Result<Self, String> {
        let existed_at_startup = path.exists();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("could not create data directory: {e}"))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));
            }
        }

        recover_interrupted_encryption(&path);
        if path.exists() && path.metadata().map(|m| m.len() == 0).unwrap_or(false) {
            let _ = fs::remove_file(&path);
        }

        let mut encrypted_existing = false;
        if path.exists() && is_plaintext_database(&path) {
            migrate_plaintext_database(&path, &key)?;
            info_event("database migrated existing file to encrypted storage");
            encrypted_existing = true;
        }

        let existed = path.exists();
        let conn = if !existed {
            let conn = Connection::open(&path).map_err(|_| {
                "The database could not be created. Anchored did not leave it open.".to_string()
            })?;
            apply_key(&conn, &key)?;
            conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
                r.get::<_, i64>(0)
            })
            .map_err(|_| {
                "The database could not be created. Anchored did not leave it open.".to_string()
            })?;
            restrict_private_file(&path);
            conn
        } else {
            open_encrypted(&path, &key).map_err(|_| {
                "The database could not be opened with DATABASE_KEY. Anchored left the file in place and did not replace it."
                    .to_string()
            })?
        };

        let schema_changed = migrate(&conn, existed)?;
        restrict_database_files(&path);
        let migrated = schema_changed || encrypted_existing;
        info_event(&format!(
            "database opened existing={} migrated={}",
            if existed { "yes" } else { "no" },
            if migrated { "yes" } else { "no" }
        ));
        if schema_changed {
            info_event("database schema migrated");
        }

        Ok(Self {
            path,
            key,
            conn: Mutex::new(conn),
            existed_at_startup,
        })
    }

    pub fn lock(&self) -> Result<MutexGuard<'_, Connection>, String> {
        self.conn
            .lock()
            .map_err(|_| "database lock poisoned".to_string())
    }

    pub fn existed_at_startup(&self) -> bool {
        self.existed_at_startup
    }

    pub fn reset(&self) -> Result<(), String> {
        {
            let conn = self.lock()?;
            drop(conn);
        }
        // Close by replacing with a temporary empty connection is messy; remove files then reopen.
        for suffix in ["", "-wal", "-shm", ".plaintext-bak", ".encrypting"] {
            let p = PathBuf::from(format!("{}{}", self.path.display(), suffix));
            let _ = fs::remove_file(&p);
        }
        remove_sidecars(&PathBuf::from(format!(
            "{}.plaintext-bak",
            self.path.display()
        )));
        remove_sidecars(&PathBuf::from(format!(
            "{}.encrypting",
            self.path.display()
        )));

        let conn = Connection::open(&self.path).map_err(|e| e.to_string())?;
        apply_key(&conn, &self.key)?;
        migrate(&conn, false)?;
        restrict_private_file(&self.path);
        let mut guard = self.lock()?;
        *guard = conn;
        Ok(())
    }
}

impl Drop for Db {
    fn drop(&mut self) {
        info_event("database closed");
    }
}

fn apply_key(conn: &Connection, key: &str) -> Result<(), String> {
    // SQLCipher 4 via rusqlite bundled-sqlcipher.
    conn.pragma_update(None, "key", key)
        .map_err(|e| format!("could not apply database key: {e}"))?;
    // Confirm the key works.
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
        r.get::<_, i64>(0)
    })
    .map_err(|_| "database key was rejected".to_string())?;
    Ok(())
}

fn open_encrypted(path: &Path, key: &str) -> Result<Connection, String> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(|e| e.to_string())?;
    apply_key(&conn, key)?;
    Ok(conn)
}

fn is_plaintext_database(path: &Path) -> bool {
    let Ok(bytes) = fs::read(path) else {
        return false;
    };
    bytes.len() >= PLAINTEXT_MAGIC.len() && &bytes[..PLAINTEXT_MAGIC.len()] == PLAINTEXT_MAGIC
}

fn remove_sidecars(path: &Path) {
    for suffix in ["-wal", "-shm"] {
        let _ = fs::remove_file(format!("{}{suffix}", path.display()));
    }
}

fn restrict_private_file(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
}

fn restrict_database_files(path: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        restrict_private_file(Path::new(&format!("{}{suffix}", path.display())));
    }
}

fn table_counts(conn: &Connection) -> Result<Vec<(String, i64)>, String> {
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'")
        .map_err(|e| e.to_string())?;
    let names: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for name in names {
        if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            || name.chars().next().is_some_and(|c| c.is_ascii_digit())
        {
            return Err(
                "The database has a table Anchored cannot migrate. The original file was left unchanged."
                    .into(),
            );
        }
        let count: i64 = conn
            .query_row(&format!("SELECT count(*) FROM \"{name}\""), [], |r| {
                r.get(0)
            })
            .map_err(|e| e.to_string())?;
        out.push((name, count));
    }
    out.sort();
    Ok(out)
}

fn migrate_plaintext_database(path: &Path, key: &str) -> Result<(), String> {
    let plain = Connection::open(path).map_err(|e| e.to_string())?;
    let _ = plain.pragma_update(None, "wal_checkpoint", "TRUNCATE");
    let before = table_counts(&plain)?;
    drop(plain);

    let tmp = PathBuf::from(format!("{}.encrypting", path.display()));
    let bak = PathBuf::from(format!("{}.plaintext-bak", path.display()));
    let _ = fs::remove_file(&tmp);
    remove_sidecars(&tmp);
    fs::copy(path, &tmp).map_err(|e| e.to_string())?;
    restrict_private_file(&tmp);

    {
        let migrated = Connection::open(&tmp).map_err(|e| e.to_string())?;
        // Rekey plaintext → SQLCipher.
        migrated.pragma_update(None, "rekey", key).map_err(|_| {
            "Could not encrypt the existing database. The original file was left unchanged."
                .to_string()
        })?;
        let after = table_counts(&migrated).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            e
        })?;
        if before != after {
            let _ = fs::remove_file(&tmp);
            return Err(
                "Encrypting the database would have changed its data. The original file was left unchanged."
                    .into(),
            );
        }
        let _ = migrated.pragma_update(None, "wal_checkpoint", "TRUNCATE");
        drop(migrated);
    }
    remove_sidecars(&tmp);

    if is_plaintext_database(&tmp) {
        let _ = fs::remove_file(&tmp);
        return Err(
            "Encryption did not protect the database file. The original file was left unchanged."
                .into(),
        );
    }

    let _ = fs::remove_file(&bak);
    fs::rename(path, &bak).map_err(|e| e.to_string())?;
    restrict_private_file(&bak);
    remove_sidecars(path);
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::rename(&bak, path);
        let _ = fs::remove_file(&tmp);
        return Err(format!(
            "The encrypted database did not match the original. The original file was restored. ({e})"
        ));
    }
    restrict_private_file(path);
    match open_encrypted(path, key) {
        Ok(verify) => {
            let after = table_counts(&verify)?;
            drop(verify);
            if before != after {
                let _ = fs::remove_file(path);
                remove_sidecars(path);
                let _ = fs::rename(&bak, path);
                return Err(
                    "The encrypted database did not match the original. The original file was restored."
                        .into(),
                );
            }
        }
        Err(_) => {
            let _ = fs::remove_file(path);
            remove_sidecars(path);
            let _ = fs::rename(&bak, path);
            return Err(
                "The encrypted database did not match the original. The original file was restored."
                    .into(),
            );
        }
    }
    let _ = fs::remove_file(&bak);
    remove_sidecars(&bak);
    restrict_database_files(path);
    Ok(())
}

fn recover_interrupted_encryption(path: &Path) {
    let bak = PathBuf::from(format!("{}.plaintext-bak", path.display()));
    let tmp = PathBuf::from(format!("{}.encrypting", path.display()));
    let main_ready = path.exists() && path.metadata().map(|m| m.len() > 0).unwrap_or(false);
    if !main_ready && tmp.exists() && !is_plaintext_database(&tmp) {
        let _ = fs::remove_file(path);
        let _ = fs::rename(&tmp, path);
        restrict_private_file(path);
        return;
    }
    if !main_ready && bak.exists() {
        let _ = fs::remove_file(path);
        let _ = fs::rename(&bak, path);
        restrict_private_file(path);
    }
    if tmp.exists() {
        let _ = fs::remove_file(&tmp);
        remove_sidecars(&tmp);
    }
}

fn migrate(conn: &Connection, existing: bool) -> Result<bool, String> {
    let before: std::collections::HashSet<String> = if existing {
        let mut stmt = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
            )
            .map_err(|e| e.to_string())?;
        let names = stmt
            .query_map([], |r| r.get(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;
        names
    } else {
        Default::default()
    };

    conn.execute_batch(
        r#"
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
        "#,
    )
    .map_err(|e| {
        error_event("database schema migrate failed");
        e.to_string()
    })?;

    let mut changed = false;
    if existing {
        let mut stmt = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
            )
            .map_err(|e| e.to_string())?;
        let after: std::collections::HashSet<String> = stmt
            .query_map([], |r| r.get(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;
        for name in &after {
            if !before.contains(name) {
                changed = true;
            }
        }
    }

    let cols: Vec<String> = {
        let mut stmt = conn
            .prepare("PRAGMA table_info(plaid_items)")
            .map_err(|e| e.to_string())?;
        let names = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .map_err(|e| e.to_string())?
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;
        names
    };
    if !cols.iter().any(|c| c == "access_mode") {
        conn.execute(
            "ALTER TABLE plaid_items ADD COLUMN access_mode TEXT NOT NULL DEFAULT 'read_only'",
            [],
        )
        .map_err(|e| e.to_string())?;
        if existing {
            changed = true;
        }
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn wrong_key_leaves_the_encrypted_file_unreadable() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("anchored-db-test-{nanos}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("anchored.db");
        let right = "correct-key-value-32chars-minimum";
        let wrong = "different-key-value-32chars-minx";
        let opened = Db::open(path.clone(), right.to_string()).unwrap();
        drop(opened);

        let err = match Db::open(path.clone(), wrong.to_string()) {
            Err(err) => err,
            Ok(_) => panic!("wrong key opened the database"),
        };
        assert!(err.contains("left the file"));
        assert!(!err.contains(right));
        assert!(!err.contains(wrong));
        assert!(path.is_file());
        let bytes = fs::read(&path).unwrap();
        assert!(bytes.len() > 16);
        assert!(!bytes.starts_with(PLAINTEXT_MAGIC));
        let _ = fs::remove_dir_all(&dir);
    }
}
