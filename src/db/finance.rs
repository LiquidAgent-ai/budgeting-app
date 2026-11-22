use crate::db::rules;
use crate::db::sql;
use crate::db::Db;
use crate::db::models::{Account, CategorySpend, DashboardSummary, NetWorthBreakdown, Transaction};
use chrono::{Datelike, Local};
use rusqlite::params;

pub fn month_start() -> String {
    let now = Local::now().date_naive();
    format!("{:04}-{:02}-01", now.year(), now.month())
}

pub fn list_accounts(db: &Db) -> Result<Vec<Account>, String> {
    let conn = db.lock()?;
    let mut stmt = conn
        .prepare(sql::SELECT_ACCOUNTS)
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], map_account)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

fn map_account(r: &rusqlite::Row<'_>) -> rusqlite::Result<Account> {
    Ok(Account {
        id: r.get(0)?,
        name: r.get(1)?,
        official_name: r.get(2)?,
        account_type: r.get(3)?,
        subtype: r.get(4)?,
        mask: r.get(5)?,
        balance_current: r.get(6)?,
        balance_available: r.get(7)?,
        currency: r.get(8)?,
        institution_name: r.get(9)?,
        plaid_account_id: r.get(10)?,
        plaid_item_id: r.get(11)?,
        is_manual: r.get(12)?,
        include_in_net_worth: r.get(13)?,
        created_at: r.get(14)?,
        updated_at: r.get(15)?,
    })
}

pub fn compute_net_worth(accounts: &[Account]) -> (f64, f64, f64, f64, f64) {
    // Match TS: creditUsed += Math.abs(Math.min(0, balance_current))
    let mut cash = 0.0;
    let mut credit_used = 0.0;
    let mut investments = 0.0;
    let mut loans = 0.0;
    for a in accounts {
        if a.include_in_net_worth == 0 {
            continue;
        }
        match a.account_type.as_str() {
            "checking" | "savings" => cash += a.balance_current,
            "credit" => credit_used += a.balance_current.min(0.0).abs(),
            "investment" => investments += a.balance_current,
            "loan" => loans += a.balance_current.abs(),
            _ => cash += a.balance_current,
        }
    }
    (
        cash + investments - credit_used - loans,
        cash,
        credit_used,
        investments,
        loans,
    )
}

pub fn dashboard(db: &Db) -> Result<DashboardSummary, String> {
    let accounts = list_accounts(db)?;
    let (net_worth, cash, credit_used, investments, _) = compute_net_worth(&accounts);
    let start = month_start();
    let conn = db.lock()?;

    let (month_spend, month_income): (f64, f64) = conn
        .query_row(
            sql::SELECT_MONTH_SPEND_INCOME,
            params![start],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    let budget_total: f64 = conn
        .query_row(
            sql::SELECT_BUDGET_TOTAL,
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    let budget_spent: f64 = conn
        .query_row(
            sql::SELECT_BUDGET_SPENT,
            params![start],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    let (subscription_count, subscription_total): (i64, f64) = conn
        .query_row(
            sql::SELECT_SUBSCRIPTION_TOTALS,
            params![start],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            sql::SELECT_RECENT_TRANSACTIONS,
        )
        .map_err(|e| e.to_string())?;
    let mut recent_transactions = stmt
        .query_map([], map_transaction_row)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    attach_tags(&conn, &mut recent_transactions)?;

    let mut stmt = conn
        .prepare(
            sql::SELECT_SPEND_BY_CATEGORY,
        )
        .map_err(|e| e.to_string())?;
    let spend_by_category = stmt
        .query_map(params![start], |r| {
            Ok(CategorySpend {
                category: r.get(0)?,
                amount: r.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(DashboardSummary {
        net_worth,
        cash,
        credit_used,
        investments,
        month_spend,
        month_income,
        budget_total,
        budget_spent,
        subscription_total,
        subscription_count,
        recent_transactions,
        spend_by_category,
        accounts,
    })
}

fn map_transaction_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Transaction> {
    Ok(Transaction {
        id: r.get(0)?,
        account_id: r.get(1)?,
        plaid_transaction_id: r.get(2)?,
        amount: r.get(3)?,
        date: r.get(4)?,
        name: r.get(5)?,
        merchant_name: r.get(6)?,
        category: r.get(7)?,
        subcategory: r.get(8)?,
        pending: r.get(9)?,
        is_subscription: r.get(10)?,
        notes: r.get(11)?,
        created_at: r.get(12)?,
        account_name: r.get(13)?,
        tags: Vec::new(),
    })
}

fn attach_tags(conn: &rusqlite::Connection, txns: &mut [Transaction]) -> Result<(), String> {
    let ids: Vec<String> = txns.iter().map(|t| t.id.clone()).collect();
    let map = rules::tags_for_transactions(conn, &ids)?;
    for t in txns.iter_mut() {
        if let Some(tags) = map.get(&t.id) {
            t.tags = tags.clone();
        }
    }
    Ok(())
}

pub fn net_worth(db: &Db) -> Result<NetWorthBreakdown, String> {
    let accounts = list_accounts(db)?;
    let (net_worth, cash, credit_used, investments, loans) = compute_net_worth(&accounts);
    Ok(NetWorthBreakdown {
        net_worth,
        cash,
        credit_used,
        investments,
        loans,
        accounts,
    })
}

pub fn list_transactions(db: &Db, limit: i64) -> Result<Vec<Transaction>, String> {
    let conn = db.lock()?;
    let mut stmt = conn
        .prepare(
            sql::SELECT_TRANSACTIONS,
        )
        .map_err(|e| e.to_string())?;
    let mut rows = stmt
        .query_map(params![limit], map_transaction_row)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    attach_tags(&conn, &mut rows)?;
    Ok(rows)
}

pub fn list_budgets(db: &Db) -> Result<Vec<crate::db::models::Budget>, String> {
    let conn = db.lock()?;
    let mut stmt = conn
        .prepare(sql::SELECT_BUDGETS)
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(crate::db::models::Budget {
                id: r.get(0)?,
                category: r.get(1)?,
                monthly_limit: r.get(2)?,
                created_at: r.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

pub fn list_subscriptions(db: &Db) -> Result<Vec<Transaction>, String> {
    let start = month_start();
    let conn = db.lock()?;
    let mut stmt = conn
        .prepare(
            sql::SELECT_SUBSCRIPTIONS,
        )
        .map_err(|e| e.to_string())?;
    let mut rows = stmt
        .query_map(params![start], map_transaction_row)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    attach_tags(&conn, &mut rows)?;
    Ok(rows)
}
