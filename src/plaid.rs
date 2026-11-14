//! Read-only Plaid access only. Never requests transfers, payments, or Auth.

use crate::categories::categorize;
use crate::config::Config;
use crate::db::Db;
use crate::log::{error_event, info_event};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

/// Products Anchored may request. Auth / Transfer / PaymentInitiation are excluded.
const READ_ONLY_PRODUCTS: &[&str] = &["transactions", "investments"];
const READ_ONLY_OPTIONAL: &[&str] = &["liabilities", "recurring_transactions"];

#[derive(Serialize)]
pub struct PlaidStatus {
    pub configured: bool,
    pub env: String,
    pub access_mode: &'static str,
    pub products: &'static [&'static str],
}

pub fn status(cfg: &Config) -> PlaidStatus {
    PlaidStatus {
        configured: cfg.plaid_configured(),
        env: cfg.plaid_env.clone(),
        access_mode: "read_only",
        products: READ_ONLY_PRODUCTS,
    }
}

async fn plaid_post(cfg: &Config, path: &str, body: Value) -> Result<Value, String> {
    let client_id = cfg
        .plaid_client_id
        .as_ref()
        .ok_or_else(|| "Plaid is not configured".to_string())?;
    let secret = cfg
        .plaid_secret
        .as_ref()
        .ok_or_else(|| "Plaid is not configured".to_string())?;

    let url = format!("{}{path}", cfg.plaid_base_url());
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;

    let mut payload = body;
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("client_id".into(), json!(client_id));
        obj.insert("secret".into(), json!(secret));
    }

    let res = client
        .post(url)
        .json(&payload)
        .send()
        .await
        .map_err(|_| {
            error_event("plaid request failed");
            "Plaid request failed".to_string()
        })?;

    if !res.status().is_success() {
        error_event("plaid request returned an error");
        return Err("Plaid request failed".into());
    }
    res.json().await.map_err(|_| {
        error_event("plaid response could not be read");
        "Plaid request failed".to_string()
    })
}

#[derive(Deserialize)]
pub struct CreateLinkTokenBody {
    pub client_user_id: Option<String>,
}

pub async fn create_link_token(cfg: &Config, body: CreateLinkTokenBody) -> Result<Value, String> {
    if !cfg.plaid_configured() {
        return Err("Plaid is not configured. Add PLAID_CLIENT_ID and PLAID_SECRET.".into());
    }
    let user_id = body
        .client_user_id
        .unwrap_or_else(|| "anchored-local".into());
    let payload = json!({
        "user": { "client_user_id": user_id },
        "client_name": "Anchored",
        "products": READ_ONLY_PRODUCTS,
        "optional_products": READ_ONLY_OPTIONAL,
        "country_codes": ["US"],
        "language": "en",
    });
    let data = plaid_post(cfg, "/link/token/create", payload).await?;
    info_event("plaid link token created");
    Ok(json!({
        "link_token": data.get("link_token"),
        "expiration": data.get("expiration"),
        "access_mode": "read_only",
    }))
}

#[derive(Deserialize)]
pub struct ExchangeTokenBody {
    pub public_token: String,
    pub institution: Option<InstitutionInfo>,
}

#[derive(Deserialize)]
pub struct InstitutionInfo {
    pub institution_id: Option<String>,
    pub name: Option<String>,
}

pub async fn exchange_token(
    cfg: &Config,
    db: &Db,
    body: ExchangeTokenBody,
) -> Result<Value, String> {
    if !cfg.plaid_configured() {
        return Err("Plaid is not configured".into());
    }
    let exchanged = plaid_post(
        cfg,
        "/item/public_token/exchange",
        json!({ "public_token": body.public_token }),
    )
    .await?;
    let access_token = exchanged
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Plaid exchange failed".to_string())?
        .to_string();
    let item_id = exchanged
        .get("item_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let accounts = plaid_post(
        cfg,
        "/accounts/get",
        json!({ "access_token": access_token }),
    )
    .await?;

    let local_id = Uuid::new_v4().to_string();
    let inst_id = body
        .institution
        .as_ref()
        .and_then(|i| i.institution_id.clone());
    let inst_name = body.institution.as_ref().and_then(|i| i.name.clone());

    {
        let conn = db.lock()?;
        conn.execute(
            r#"INSERT INTO plaid_items (
                id, item_id, access_token, institution_id, institution_name, access_mode
              ) VALUES (?, ?, ?, ?, ?, 'read_only')"#,
            params![local_id, item_id, access_token, inst_id, inst_name],
        )
        .map_err(|e| e.to_string())?;

        if let Some(list) = accounts.get("accounts").and_then(|v| v.as_array()) {
            for acct in list {
                let plaid_account_id = acct.get("account_id").and_then(|v| v.as_str()).unwrap_or("");
                let name = acct
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Account");
                let official = acct.get("official_name").and_then(|v| v.as_str());
                let ty = acct.get("type").and_then(|v| v.as_str()).unwrap_or("other");
                let subtype = acct.get("subtype").and_then(|v| v.as_str());
                let mask = acct.get("mask").and_then(|v| v.as_str());
                let mapped = map_account_type(ty, subtype);
                let balances = acct.get("balances");
                let current = balances
                    .and_then(|b| b.get("current"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let available = balances
                    .and_then(|b| b.get("available"))
                    .and_then(|v| v.as_f64());
                let currency = balances
                    .and_then(|b| b.get("iso_currency_code"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("USD");
                conn.execute(
                    r#"INSERT INTO accounts (
                        id, name, official_name, type, subtype, mask, balance_current, balance_available,
                        currency, institution_name, plaid_account_id, plaid_item_id, is_manual, include_in_net_worth
                      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, 1)"#,
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
                        inst_name,
                        plaid_account_id,
                        local_id,
                    ],
                )
                .map_err(|e| e.to_string())?;
            }
        }
    }

    info_event("plaid item linked");
    // Sync transactions after link (best-effort).
    let _ = sync_all(cfg, db).await;
    Ok(json!({ "ok": true, "access_mode": "read_only" }))
}

pub async fn sync_all(cfg: &Config, db: &Db) -> Result<Value, String> {
    if !cfg.plaid_configured() {
        return Err("Plaid is not configured".into());
    }

    let items: Vec<(String, String, Option<String>)> = {
        let conn = db.lock()?;
        let mut stmt = conn
            .prepare("SELECT id, access_token, cursor FROM plaid_items")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        rows
    };

    let mut synced_items = 0i64;
    for (item_local_id, access_token, cursor) in items {
        match sync_item(cfg, db, &item_local_id, &access_token, cursor.as_deref()).await {
            Ok(()) => synced_items += 1,
            Err(_) => error_event("plaid sync failed for an item"),
        }
    }
    info_event(&format!("plaid sync finished items={synced_items}"));
    Ok(json!({ "ok": true, "items": synced_items }))
}

async fn sync_item(
    cfg: &Config,
    db: &Db,
    item_local_id: &str,
    access_token: &str,
    cursor: Option<&str>,
) -> Result<(), String> {
    let mut next_cursor = cursor.map(|s| s.to_string());
    loop {
        let mut body = json!({ "access_token": access_token, "count": 500 });
        if let Some(c) = &next_cursor {
            body.as_object_mut()
                .unwrap()
                .insert("cursor".into(), json!(c));
        }
        let data = plaid_post(cfg, "/transactions/sync", body).await?;
        let added = data
            .get("added")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let modified = data
            .get("modified")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let removed = data
            .get("removed")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        {
            let conn = db.lock()?;
            for txn in added.iter().chain(modified.iter()) {
                upsert_transaction(&conn, item_local_id, txn)?;
            }
            for rem in removed {
                if let Some(id) = rem.get("transaction_id").and_then(|v| v.as_str()) {
                    let _ = conn.execute(
                        "DELETE FROM transactions WHERE plaid_transaction_id = ?",
                        params![id],
                    );
                }
            }
            if let Some(c) = data.get("next_cursor").and_then(|v| v.as_str()) {
                next_cursor = Some(c.to_string());
                conn.execute(
                    "UPDATE plaid_items SET cursor = ?, updated_at = datetime('now') WHERE id = ?",
                    params![c, item_local_id],
                )
                .map_err(|e| e.to_string())?;
            }
        }

        let has_more = data
            .get("has_more")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !has_more {
            break;
        }
    }
    Ok(())
}

fn upsert_transaction(
    conn: &rusqlite::Connection,
    item_local_id: &str,
    txn: &Value,
) -> Result<(), String> {
    let plaid_txn_id = txn
        .get("transaction_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing transaction_id".to_string())?;
    let plaid_account_id = txn
        .get("account_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let account_id: String = conn
        .query_row(
            "SELECT id FROM accounts WHERE plaid_account_id = ? AND plaid_item_id = ?",
            params![plaid_account_id, item_local_id],
            |r| r.get(0),
        )
        .unwrap_or_default();
    if account_id.is_empty() {
        return Ok(());
    }
    let amount = txn.get("amount").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let date = txn.get("date").and_then(|v| v.as_str()).unwrap_or("");
    let name = txn
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Transaction");
    let merchant = txn.get("merchant_name").and_then(|v| v.as_str());
    let pending = txn.get("pending").and_then(|v| v.as_bool()).unwrap_or(false);
    let (category, is_sub) = categorize(name, merchant);

    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM transactions WHERE plaid_transaction_id = ?",
            params![plaid_txn_id],
            |r| r.get(0),
        )
        .ok();
    if let Some(id) = existing {
        conn.execute(
            r#"UPDATE transactions SET
                amount = ?, date = ?, name = ?, merchant_name = ?,
                category = ?, pending = ?, is_subscription = ?
              WHERE id = ?"#,
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
    } else {
        conn.execute(
            r#"INSERT INTO transactions (
                id, account_id, plaid_transaction_id, amount, date, name, merchant_name,
                category, pending, is_subscription
              ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
            params![
                Uuid::new_v4().to_string(),
                account_id,
                plaid_txn_id,
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
    Ok(())
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
    if s.contains("checking") || (t == "depository" && s == "checking") {
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
