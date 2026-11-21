//! Read-only Teller access only. Never requests payments, verify, or identity.

use crate::config::logging::{error_event, info_event};
use crate::config::Config;
use crate::db::categories::categorize;
use crate::db::duplicates::{self, DuplicateDecision, IncomingTxn};
use crate::db::sql;
use crate::db::Db;
use chrono::{Duration, Local, NaiveDate};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

/// Products Anchored may request from Teller Connect.
const READ_ONLY_PRODUCTS: &[&str] = &["balance", "transactions"];

/// Products that would leave read-only mode (payments or sensitive account details).
const FORBIDDEN_TELLER_PRODUCTS: &[&str] = &[
    "payments",
    "verify",
    "verify.instant",
    "identity",
];

/// Returns an error if any product would leave read-only mode.
fn reject_if_not_read_only(products: &[&str]) -> Result<(), String> {
    for product in products {
        let lower = product.to_ascii_lowercase();
        if FORBIDDEN_TELLER_PRODUCTS
            .iter()
            .any(|blocked| *blocked == lower)
        {
            return Err(format!(
                "Refusing Teller product '{product}': Anchored is read-only and does not request payments, verify, or identity."
            ));
        }
    }
    Ok(())
}

#[derive(Serialize)]
pub struct TellerStatus {
    pub configured: bool,
    pub env: String,
    pub access_mode: &'static str,
    pub products: &'static [&'static str],
    pub application_id: Option<String>,
    pub certificate_ready: bool,
}

pub fn status(cfg: &Config) -> TellerStatus {
    TellerStatus {
        configured: cfg.teller_configured(),
        env: cfg.teller_env.clone(),
        access_mode: "read_only",
        products: READ_ONLY_PRODUCTS,
        application_id: cfg.teller_application_id.clone(),
        certificate_ready: cfg.teller_certificate_ready(),
    }
}

fn build_client(cfg: &Config) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none());
    if let Some(identity) = cfg.teller_identity()? {
        builder = builder.identity(identity);
    } else if cfg.teller_env != "sandbox" {
        return Err(
            "Teller development/production needs TELLER_CERT_PATH and TELLER_KEY_PATH (or PEM env vars)."
                .into(),
        );
    }
    builder.build().map_err(|e| e.to_string())
}

async fn teller_get(cfg: &Config, access_token: &str, path: &str) -> Result<Value, String> {
    let client = build_client(cfg)?;
    let url = format!("https://api.teller.io{path}");
    let res = client
        .get(url)
        .basic_auth(access_token, Some(""))
        .send()
        .await
        .map_err(|_| {
            error_event("teller request failed");
            "Teller request failed".to_string()
        })?;
    if !res.status().is_success() {
        error_event("teller request returned an error");
        return Err("Teller request failed".into());
    }
    res.json().await.map_err(|_| {
        error_event("teller response could not be read");
        "Teller request failed".to_string()
    })
}

#[derive(Deserialize)]
pub struct EnrollBody {
    pub access_token: String,
    pub enrollment: Option<EnrollmentInfo>,
    pub user: Option<UserInfo>,
}

#[derive(Deserialize)]
pub struct EnrollmentInfo {
    pub id: Option<String>,
    pub institution: Option<InstitutionInfo>,
}

#[derive(Deserialize)]
pub struct InstitutionInfo {
    pub id: Option<String>,
    pub name: Option<String>,
}

#[derive(Deserialize)]
pub struct UserInfo {
    pub id: Option<String>,
}

pub async fn enroll(cfg: &Config, db: &Db, body: EnrollBody) -> Result<Value, String> {
    if !cfg.teller_configured() {
        return Err("Teller is not configured. Add TELLER_APPLICATION_ID.".into());
    }
    reject_if_not_read_only(READ_ONLY_PRODUCTS)?;

    let access_token = body.access_token.trim().to_string();
    if access_token.is_empty() {
        return Err("Missing Teller access token".into());
    }

    let accounts = teller_get(cfg, &access_token, "/accounts").await?;
    let list = accounts
        .as_array()
        .cloned()
        .ok_or_else(|| "Teller accounts response was not a list".to_string())?;

    let enrollment_id = body
        .enrollment
        .as_ref()
        .and_then(|e| e.id.clone())
        .or_else(|| {
            list.first()
                .and_then(|a| a.get("enrollment_id"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| format!("enr_local_{}", Uuid::new_v4()));

    let inst_id = body
        .enrollment
        .as_ref()
        .and_then(|e| e.institution.as_ref())
        .and_then(|i| i.id.clone())
        .or_else(|| {
            list.first()
                .and_then(|a| a.get("institution"))
                .and_then(|i| i.get("id"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        });
    let inst_name = body
        .enrollment
        .as_ref()
        .and_then(|e| e.institution.as_ref())
        .and_then(|i| i.name.clone())
        .or_else(|| {
            list.first()
                .and_then(|a| a.get("institution"))
                .and_then(|i| i.get("name"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        });
    let user_id = body.user.as_ref().and_then(|u| u.id.clone());
    let local_id = Uuid::new_v4().to_string();

    let mut prepared: Vec<(
        String,
        Option<String>,
        &'static str,
        Option<String>,
        Option<String>,
        f64,
        Option<f64>,
        String,
        Option<String>,
        String,
    )> = Vec::new();

    for acct in &list {
        let teller_account_id = acct.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if teller_account_id.is_empty() {
            continue;
        }
        let name = acct
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Account")
            .to_string();
        let ty = acct.get("type").and_then(|v| v.as_str()).unwrap_or("other");
        let subtype = acct
            .get("subtype")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let mapped = map_account_type(ty, subtype.as_deref());
        let mask = acct
            .get("last_four")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let currency = acct
            .get("currency")
            .and_then(|v| v.as_str())
            .unwrap_or("USD")
            .to_string();
        let acct_inst = acct
            .get("institution")
            .and_then(|i| i.get("name"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| inst_name.clone());

        let (current, available) =
            fetch_balances(cfg, &access_token, teller_account_id, mapped).await?;
        prepared.push((
            name,
            None,
            mapped,
            subtype,
            mask,
            current,
            available,
            currency,
            acct_inst,
            teller_account_id.to_string(),
        ));
    }

    {
        let conn = db.lock()?;
        conn.execute(
            sql::INSERT_TELLER_ENROLLMENT,
            params![
                local_id,
                enrollment_id,
                access_token,
                user_id,
                inst_id,
                inst_name
            ],
        )
        .map_err(|e| e.to_string())?;

        for (name, official, mapped, subtype, mask, current, available, currency, acct_inst, teller_account_id) in
            prepared
        {
            conn.execute(
                sql::INSERT_TELLER_ACCOUNT,
                params![
                    Uuid::new_v4().to_string(),
                    name,
                    official,
                    mapped,
                    subtype,
                    mask,
                    current,
                    available,
                    currency,
                    acct_inst,
                    teller_account_id,
                    local_id,
                ],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    info_event("teller enrollment linked");
    let _ = sync_all(cfg, db).await;
    Ok(json!({ "ok": true, "access_mode": "read_only" }))
}

async fn fetch_balances(
    cfg: &Config,
    access_token: &str,
    account_id: &str,
    account_type: &str,
) -> Result<(f64, Option<f64>), String> {
    let path = format!("/accounts/{account_id}/balances");
    let data = match teller_get(cfg, access_token, &path).await {
        Ok(v) => v,
        Err(_) => return Ok((0.0, None)),
    };
    let ledger = parse_amount(data.get("ledger"));
    let available = data
        .get("available")
        .and_then(|v| {
            if v.is_null() {
                None
            } else {
                Some(parse_amount(Some(v)))
            }
        });
    let current = if account_type == "credit" || account_type == "loan" {
        // Anchored stores credit/loan balances as negative when money is owed.
        if ledger > 0.0 {
            -ledger
        } else {
            ledger
        }
    } else {
        ledger
    };
    Ok((current, available))
}

pub async fn sync_all(cfg: &Config, db: &Db) -> Result<Value, String> {
    if !cfg.teller_configured() {
        return Err("Teller is not configured".into());
    }

    let enrollments: Vec<(String, String, Option<String>)> = {
        let conn = db.lock()?;
        let mut stmt = conn
            .prepare(sql::SELECT_TELLER_ENROLLMENTS)
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        rows
    };

    let mut synced = 0i64;
    for (local_id, access_token, last_sync) in enrollments {
        match sync_enrollment(cfg, db, &local_id, &access_token, last_sync.as_deref()).await {
            Ok(()) => synced += 1,
            Err(_) => error_event("teller sync failed for an enrollment"),
        }
    }
    info_event(&format!("teller sync finished enrollments={synced}"));
    Ok(json!({ "ok": true, "enrollments": synced }))
}

async fn sync_enrollment(
    cfg: &Config,
    db: &Db,
    local_id: &str,
    access_token: &str,
    last_sync: Option<&str>,
) -> Result<(), String> {
    let accounts: Vec<(String, String, String)> = {
        let conn = db.lock()?;
        let mut stmt = conn
            .prepare(sql::SELECT_TELLER_ACCOUNTS)
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![local_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        rows
    };

    let start_date = sync_start_date(last_sync);
    let today = Local::now().date_naive().format("%Y-%m-%d").to_string();
    let mut newest = start_date.clone();

    for (account_id, teller_account_id, account_type) in accounts {
        let (current, available) =
            fetch_balances(cfg, access_token, &teller_account_id, &account_type).await?;
        {
            let conn = db.lock()?;
            conn.execute(
                sql::UPDATE_ACCOUNT_BALANCES,
                params![current, available, account_id],
            )
            .map_err(|e| e.to_string())?;
        }

        let mut from_id: Option<String> = None;
        loop {
            let mut path = format!(
                "/accounts/{teller_account_id}/transactions?count=250&start_date={start_date}&end_date={today}"
            );
            if let Some(id) = &from_id {
                path.push_str(&format!("&from_id={id}"));
            }
            let data = teller_get(cfg, access_token, &path).await?;
            let list = data.as_array().cloned().unwrap_or_default();
            if list.is_empty() {
                break;
            }

            {
                let conn = db.lock()?;
                for txn in &list {
                    upsert_transaction(&conn, &account_id, &account_type, txn)?;
                    if let Some(date) = txn.get("date").and_then(|v| v.as_str()) {
                        if date > newest.as_str() {
                            newest = date.to_string();
                        }
                    }
                }
            }

            if list.len() < 250 {
                break;
            }
            from_id = list
                .last()
                .and_then(|t| t.get("id"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            if from_id.is_none() {
                break;
            }
        }
    }

    {
        let conn = db.lock()?;
        conn.execute(
            sql::UPDATE_TELLER_ENROLLMENT_SYNC,
            params![newest, local_id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn sync_start_date(last_sync: Option<&str>) -> String {
    let today = Local::now().date_naive();
    let parsed = last_sync.and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
    let start = match parsed {
        Some(d) => d - Duration::days(10),
        None => today - Duration::days(90),
    };
    start.format("%Y-%m-%d").to_string()
}

fn upsert_transaction(
    conn: &rusqlite::Connection,
    account_id: &str,
    account_type: &str,
    txn: &Value,
) -> Result<(), String> {
    let teller_txn_id = txn
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing transaction id".to_string())?;
    let raw_amount = parse_amount(txn.get("amount"));
    let amount = anchored_amount(raw_amount, account_type);
    let date = txn.get("date").and_then(|v| v.as_str()).unwrap_or("");
    let name = txn
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("Transaction");
    let merchant = txn
        .get("details")
        .and_then(|d| d.get("counterparty"))
        .and_then(|c| c.get("name"))
        .and_then(|v| v.as_str());
    let pending = txn
        .get("status")
        .and_then(|v| v.as_str())
        .map(|s| s == "pending")
        .unwrap_or(false);
    let (category, is_sub) = categorize(name, merchant);

    let incoming = IncomingTxn {
        account_id,
        date,
        amount,
        name,
        merchant_name: merchant,
        plaid_transaction_id: None,
        teller_transaction_id: Some(teller_txn_id),
        import_fingerprint: None,
    };
    match duplicates::classify(conn, &incoming)? {
        DuplicateDecision::SameProvider { id } => {
            conn.execute(
                sql::UPDATE_TRANSACTION,
                params![
                    amount,
                    date,
                    name,
                    merchant,
                    category,
                    if pending { 1 } else { 0 },
                    if is_sub { 1 } else { 0 },
                    id,
                ],
            )
            .map_err(|e| e.to_string())?;
        }
        DuplicateDecision::ContentMatch { id } => {
            // Already on the books (import/Plaid/etc.) — do not insert another row.
            let _ = duplicates::link_teller_id(conn, &id, teller_txn_id);
        }
        DuplicateDecision::New => {
            conn.execute(
                sql::INSERT_TELLER_TRANSACTION,
                params![
                    Uuid::new_v4().to_string(),
                    account_id,
                    teller_txn_id,
                    amount,
                    date,
                    name,
                    merchant,
                    category,
                    if pending { 1 } else { 0 },
                    if is_sub { 1 } else { 0 },
                ],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Convert Teller signed amounts into Anchored's Plaid-style signs (spend > 0).
fn anchored_amount(teller_amount: f64, account_type: &str) -> f64 {
    if account_type == "credit" {
        // Teller credit: purchase is positive; matches Anchored spend-positive.
        teller_amount
    } else {
        // Teller depository: debit is negative; Anchored wants spend positive.
        -teller_amount
    }
}

fn parse_amount(v: Option<&Value>) -> f64 {
    match v {
        Some(Value::String(s)) => s.parse().unwrap_or(0.0),
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        _ => 0.0,
    }
}

fn map_account_type(ty: &str, subtype: Option<&str>) -> &'static str {
    let t = ty.to_lowercase();
    let s = subtype.unwrap_or("").to_lowercase();
    if t == "credit" || s.contains("credit") {
        return "credit";
    }
    if t == "investment" || t == "brokerage" || s.contains("401") || s.contains("ira") {
        return "investment";
    }
    if t == "loan" || s.contains("mortgage") || s.contains("student") {
        return "loan";
    }
    if s.contains("checking") || (t == "depository" && s.contains("checking")) {
        return "checking";
    }
    if s.contains("saving") {
        return "savings";
    }
    if t == "depository" {
        return "checking";
    }
    "other"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_products_reject_payments_verify_identity() {
        assert!(reject_if_not_read_only(READ_ONLY_PRODUCTS).is_ok());
        assert!(reject_if_not_read_only(&["payments"]).is_err());
        assert!(reject_if_not_read_only(&["verify"]).is_err());
        assert!(reject_if_not_read_only(&["verify.instant"]).is_err());
        assert!(reject_if_not_read_only(&["identity"]).is_err());
        assert!(reject_if_not_read_only(&["balance", "payments"]).is_err());
    }

    #[test]
    fn anchored_amount_matches_spend_positive_convention() {
        assert_eq!(anchored_amount(-42.5, "checking"), 42.5);
        assert_eq!(anchored_amount(500.0, "savings"), -500.0);
        assert_eq!(anchored_amount(100.0, "credit"), 100.0);
        assert_eq!(anchored_amount(-200.0, "credit"), -200.0);
    }
}
