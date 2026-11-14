use crate::categories::categorize;
use crate::db::Db;
use chrono::{Duration, Local};
use rusqlite::params;
use uuid::Uuid;

fn days_ago(n: i64) -> String {
    (Local::now().date_naive() - Duration::days(n))
        .format("%Y-%m-%d")
        .to_string()
}

pub fn seed_database(db: &Db, fresh: bool) -> Result<(bool, String), String> {
    if fresh {
        db.reset()?;
    }

    {
        let conn = db.lock()?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM accounts", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        if count > 0 && !fresh {
            return Ok((false, "Database already has accounts".into()));
        }
    }

    let checking_id = Uuid::new_v4().to_string();
    let savings_id = Uuid::new_v4().to_string();
    let credit_id = Uuid::new_v4().to_string();
    let brokerage_id = Uuid::new_v4().to_string();
    let ira_id = Uuid::new_v4().to_string();

    let conn = db.lock()?;
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

    let insert_account = |id: &str,
                          name: &str,
                          official: &str,
                          ty: &str,
                          subtype: &str,
                          mask: &str,
                          bal: f64,
                          avail: Option<f64>,
                          inst: &str|
     -> Result<(), String> {
        tx.execute(
            r#"INSERT INTO accounts (
                id, name, official_name, type, subtype, mask, balance_current, balance_available,
                currency, institution_name, is_manual, include_in_net_worth
              ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'USD', ?, 1, 1)"#,
            params![id, name, official, ty, subtype, mask, bal, avail, inst],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    };

    insert_account(
        &checking_id,
        "Everyday Checking",
        "Chase Total Checking",
        "checking",
        "checking",
        "4821",
        4287.42,
        Some(4287.42),
        "Chase",
    )?;
    insert_account(
        &savings_id,
        "Emergency Fund",
        "Ally Savings",
        "savings",
        "savings",
        "9033",
        18500.0,
        Some(18500.0),
        "Ally",
    )?;
    insert_account(
        &credit_id,
        "Sapphire Preferred",
        "Chase Sapphire Preferred",
        "credit",
        "credit card",
        "2219",
        -1842.67,
        Some(8157.33),
        "Chase",
    )?;
    insert_account(
        &brokerage_id,
        "Brokerage",
        "Fidelity Brokerage",
        "investment",
        "brokerage",
        "7710",
        62480.15,
        None,
        "Fidelity",
    )?;
    insert_account(
        &ira_id,
        "Roth IRA",
        "Vanguard Roth IRA",
        "investment",
        "ira",
        "4412",
        41220.88,
        None,
        "Vanguard",
    )?;

    let samples: Vec<(&str, f64, i64, &str, &str)> = vec![
        (&checking_id, -3200.0, 28, "ACME CORP PAYROLL", "Payroll"),
        (&checking_id, 1850.0, 27, "RENT PAYMENT", "Landlord"),
        (&checking_id, 94.32, 25, "WHOLE FOODS MARKET", "Whole Foods"),
        (&checking_id, 15.99, 24, "NETFLIX.COM", "Netflix"),
        (&checking_id, 10.99, 24, "SPOTIFY USA", "Spotify"),
        (&credit_id, 67.4, 22, "UBER TRIP", "Uber"),
        (&credit_id, 42.18, 21, "CHIPOTLE 1842", "Chipotle"),
        (&credit_id, 128.9, 20, "AMAZON.COM", "Amazon"),
        (&checking_id, 89.0, 19, "PG&E UTILITY", "PG&E"),
        (&checking_id, 79.99, 18, "COMCAST CABLE", "Comcast"),
        (&credit_id, 214.55, 16, "NORDSTROM", "Nordstrom"),
        (&checking_id, 56.2, 15, "SHELL OIL", "Shell"),
        (&credit_id, 8.75, 14, "STARBUCKS STORE", "Starbucks"),
        (&checking_id, 500.0, 13, "TRANSFER TO ALLY", "Transfer"),
        (&savings_id, -500.0, 13, "TRANSFER FROM CHASE", "Transfer"),
        (&credit_id, 35.0, 12, "CVS PHARMACY", "CVS"),
        (&checking_id, 22.0, 11, "GITHUB INC", "GitHub"),
        (&credit_id, 156.4, 10, "TARGET T-8841", "Target"),
        (&checking_id, 48.6, 9, "TRADER JOE'S", "Trader Joe's"),
        (&credit_id, 62.0, 8, "DOORDASH", "DoorDash"),
        (&checking_id, 20.0, 7, "VENMO PAYMENT", "Venmo"),
        (&credit_id, 119.0, 6, "BEST BUY", "Best Buy"),
        (&checking_id, 11.99, 5, "APPLE.COM/BILL", "Apple"),
        (&credit_id, 28.4, 4, "CHIPOTLE ONLINE", "Chipotle"),
        (&checking_id, 72.15, 3, "SAFEWAY #122", "Safeway"),
        (&credit_id, 45.0, 2, "UBER EATS", "Uber Eats"),
        (&checking_id, -3200.0, 1, "ACME CORP PAYROLL", "Payroll"),
        (&brokerage_id, 200.0, 20, "BUY VTI", "Vanguard"),
        (&ira_id, 300.0, 15, "ROTH CONTRIBUTION", "Vanguard"),
    ];

    for (account_id, amount, ago, name, merchant) in samples {
        let (category, is_sub) = categorize(name, Some(merchant));
        tx.execute(
            r#"INSERT INTO transactions (
                id, account_id, amount, date, name, merchant_name, category, is_subscription, pending
              ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0)"#,
            params![
                Uuid::new_v4().to_string(),
                account_id,
                amount,
                days_ago(ago),
                name,
                merchant,
                category,
                if is_sub { 1 } else { 0 },
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    let budgets = [
        ("Groceries", 450.0),
        ("Dining", 250.0),
        ("Shopping", 300.0),
        ("Transport", 180.0),
        ("Entertainment", 100.0),
        ("Subscriptions", 80.0),
    ];
    for (category, limit) in budgets {
        tx.execute(
            "INSERT INTO budgets (id, category, monthly_limit) VALUES (?, ?, ?)",
            params![Uuid::new_v4().to_string(), category, limit],
        )
        .map_err(|e| e.to_string())?;
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok((true, "Demo household loaded".into()))
}
