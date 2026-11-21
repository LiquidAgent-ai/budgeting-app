//! Shared duplicate recognition for file import, Plaid, and Teller.
//! Prefer skipping inserts when the same spend is already on the books.

use crate::db::sql;
use rusqlite::{params, Connection};

#[derive(Debug, Clone)]
pub struct IncomingTxn<'a> {
    pub account_id: &'a str,
    pub date: &'a str,
    pub amount: f64,
    pub name: &'a str,
    pub merchant_name: Option<&'a str>,
    pub plaid_transaction_id: Option<&'a str>,
    pub teller_transaction_id: Option<&'a str>,
    pub import_fingerprint: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DuplicateDecision {
    /// Same provider id already stored — caller may update that row.
    SameProvider { id: String },
    /// Content matches an existing row — do not insert another.
    ContentMatch { id: String },
    /// No duplicate found.
    New,
}

/// Returns how to handle an incoming transaction. Never inserts.
pub fn classify(conn: &Connection, incoming: &IncomingTxn<'_>) -> Result<DuplicateDecision, String> {
    if let Some(plaid_id) = incoming.plaid_transaction_id {
        if let Some(id) = find_by_plaid_id(conn, plaid_id)? {
            return Ok(DuplicateDecision::SameProvider { id });
        }
    }
    if let Some(teller_id) = incoming.teller_transaction_id {
        if let Some(id) = find_by_teller_id(conn, teller_id)? {
            return Ok(DuplicateDecision::SameProvider { id });
        }
    }
    if let Some(fp) = incoming.import_fingerprint {
        if let Some(id) = find_by_plaid_id(conn, fp)? {
            // Import fingerprints are stored in plaid_transaction_id.
            return Ok(DuplicateDecision::SameProvider { id });
        }
    }

    if let Some(id) = find_content_match(conn, incoming)? {
        return Ok(DuplicateDecision::ContentMatch { id });
    }
    Ok(DuplicateDecision::New)
}

/// Link a Plaid id onto an existing row when that column is empty (no second row).
pub fn link_plaid_id(conn: &Connection, row_id: &str, plaid_id: &str) -> Result<(), String> {
    conn.execute(sql::LINK_PLAID_TRANSACTION_ID, params![plaid_id, row_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Link a Teller id onto an existing row when that column is empty (no second row).
pub fn link_teller_id(conn: &Connection, row_id: &str, teller_id: &str) -> Result<(), String> {
    conn.execute(sql::LINK_TELLER_TRANSACTION_ID, params![teller_id, row_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn find_by_plaid_id(conn: &Connection, plaid_id: &str) -> Result<Option<String>, String> {
    let id = conn
        .query_row(
            sql::SELECT_TRANSACTION_ID_BY_PLAID_ID,
            params![plaid_id],
            |r| r.get::<_, String>(0),
        )
        .ok();
    Ok(id)
}

fn find_by_teller_id(conn: &Connection, teller_id: &str) -> Result<Option<String>, String> {
    let id = conn
        .query_row(
            sql::SELECT_TRANSACTION_ID_BY_TELLER_ID,
            params![teller_id],
            |r| r.get::<_, String>(0),
        )
        .ok();
    Ok(id)
}

fn find_content_match(
    conn: &Connection,
    incoming: &IncomingTxn<'_>,
) -> Result<Option<String>, String> {
    let mut stmt = conn
        .prepare(sql::SELECT_DUPLICATE_CANDIDATES)
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(
            params![
                incoming.account_id,
                incoming.date,
                incoming.date,
                incoming.amount
            ],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .map_err(|e| e.to_string())?;

    for row in rows {
        let (id, name, merchant) = row.map_err(|e| e.to_string())?;
        if payees_match(
            incoming.name,
            incoming.merchant_name,
            &name,
            merchant.as_deref(),
        ) {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

/// Stable in-batch key so a single QFX/CSV file cannot insert the same row twice.
pub fn batch_key(account_id: &str, date: &str, amount: f64, name: &str, merchant: Option<&str>) -> String {
    format!(
        "{}|{}|{:.4}|{}",
        account_id,
        date,
        amount,
        payee_key(name, merchant)
    )
}

fn payees_match(
    a_name: &str,
    a_merchant: Option<&str>,
    b_name: &str,
    b_merchant: Option<&str>,
) -> bool {
    let a_keys = payee_variants(a_name, a_merchant);
    let b_keys = payee_variants(b_name, b_merchant);
    for a in &a_keys {
        if a.is_empty() {
            continue;
        }
        for b in &b_keys {
            if b.is_empty() {
                continue;
            }
            if a == b {
                return true;
            }
            // Bank export vs Plaid often shortens or prefixes the same payee.
            if a.len() >= 5 && b.len() >= 5 && (a.contains(b.as_str()) || b.contains(a.as_str())) {
                return true;
            }
            if token_overlap(a, b) {
                return true;
            }
        }
    }
    false
}

fn significant_tokens(s: &str) -> Vec<&str> {
    s.split_whitespace()
        .filter(|t| t.len() > 2 && !t.chars().all(|c| c.is_ascii_digit()))
        .collect()
}

/// "WHOLE FOODS #99" and "WHOLE FOODS MARKET" share the same payee tokens.
fn token_overlap(a: &str, b: &str) -> bool {
    let ta = significant_tokens(a);
    let tb = significant_tokens(b);
    if ta.is_empty() || tb.is_empty() {
        return false;
    }
    let (shorter, longer) = if ta.len() <= tb.len() {
        (&ta, &tb)
    } else {
        (&tb, &ta)
    };
    if shorter.len() >= 2 {
        return shorter.iter().all(|t| longer.iter().any(|u| u == t));
    }
    let only = shorter[0];
    longer.iter().any(|t| {
        t == &only || (only.len() >= 4 && (t.starts_with(only) || only.starts_with(t)))
    })
}

fn payee_variants(name: &str, merchant: Option<&str>) -> Vec<String> {
    let mut out = Vec::new();
    let n = normalize_payee(name);
    if !n.is_empty() {
        out.push(n);
    }
    if let Some(m) = merchant {
        let m = normalize_payee(m);
        if !m.is_empty() && !out.iter().any(|x| x == &m) {
            out.push(m);
        }
    }
    out
}

fn payee_key(name: &str, merchant: Option<&str>) -> String {
    if let Some(m) = merchant {
        let m = normalize_payee(m);
        if !m.is_empty() {
            return m;
        }
    }
    normalize_payee(name)
}

pub fn normalize_payee(raw: &str) -> String {
    let lower = raw.trim().to_ascii_lowercase();
    let mut cleaned = String::with_capacity(lower.len());
    let mut last_space = true;
    for c in lower.chars() {
        if c.is_ascii_alphanumeric() {
            cleaned.push(c);
            last_space = false;
        } else if !last_space {
            cleaned.push(' ');
            last_space = true;
        }
    }
    let mut s = cleaned.trim().to_string();
    for prefix in [
        "purchase authorized on ",
        "pos debit ",
        "pos ",
        "checkcard ",
        "visa ",
        "mastercard ",
        "debit card ",
        "ach ",
        "pending ",
    ] {
        if let Some(rest) = s.strip_prefix(prefix) {
            s = rest.trim().to_string();
        }
    }
    // Drop a leading MM DD left over from "authorized on 05/01 …".
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() >= 3
        && parts[0].chars().all(|c| c.is_ascii_digit())
        && parts[1].chars().all(|c| c.is_ascii_digit())
        && parts[0].len() <= 2
        && parts[1].len() <= 2
    {
        s = parts[2..].join(" ");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_noise() {
        assert_eq!(
            normalize_payee("POS DEBIT WHOLE FOODS #182"),
            "whole foods 182"
        );
        assert_eq!(
            normalize_payee("Purchase Authorized On 05/01 CHIPOTLE"),
            "chipotle"
        );
    }

    #[test]
    fn payees_match_across_export_and_bank() {
        assert!(payees_match(
            "WHOLE FOODS MARKET",
            None,
            "Whole Foods #182",
            Some("Whole Foods")
        ));
        assert!(payees_match(
            "UBER TRIP HELP.UBER.COM",
            Some("Uber"),
            "Uber",
            None
        ));
        assert!(payees_match(
            "WHOLE FOODS #99",
            None,
            "WHOLE FOODS MARKET",
            None
        ));
        assert!(!payees_match(
            "WHOLE FOODS",
            None,
            "CHIPOTLE",
            None
        ));
    }

    #[test]
    fn classify_skips_content_duplicates_across_sources() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE transactions (
              id TEXT PRIMARY KEY,
              account_id TEXT NOT NULL,
              plaid_transaction_id TEXT,
              teller_transaction_id TEXT,
              amount REAL NOT NULL,
              date TEXT NOT NULL,
              name TEXT NOT NULL,
              merchant_name TEXT
            );
            "#,
        )
        .unwrap();
        conn.execute(
            r#"INSERT INTO transactions
               (id, account_id, plaid_transaction_id, teller_transaction_id, amount, date, name, merchant_name)
               VALUES ('row1', 'acct', 'import:acct:fit-1', NULL, 42.18, '2024-06-15', 'CHIPOTLE 1842', 'Chipotle')"#,
            [],
        )
        .unwrap();

        let incoming = IncomingTxn {
            account_id: "acct",
            date: "2024-06-15",
            amount: 42.18,
            name: "CHIPOTLE",
            merchant_name: Some("Chipotle"),
            plaid_transaction_id: Some("plaid-xyz"),
            teller_transaction_id: None,
            import_fingerprint: None,
        };
        assert_eq!(
            classify(&conn, &incoming).unwrap(),
            DuplicateDecision::ContentMatch {
                id: "row1".into()
            }
        );

        let same_import = IncomingTxn {
            account_id: "acct",
            date: "2024-06-15",
            amount: 42.18,
            name: "CHIPOTLE 1842",
            merchant_name: None,
            plaid_transaction_id: None,
            teller_transaction_id: None,
            import_fingerprint: Some("import:acct:fit-1"),
        };
        assert_eq!(
            classify(&conn, &same_import).unwrap(),
            DuplicateDecision::SameProvider {
                id: "row1".into()
            }
        );
    }
}
