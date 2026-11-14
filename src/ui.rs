use crate::finance;
use crate::models::{Account, Budget, DashboardSummary, NetWorthBreakdown, Transaction};
use std::fmt::Write as _;

fn money(v: f64) -> String {
    let sign = if v < 0.0 { "-" } else { "" };
    format!("{sign}${:.2}", v.abs())
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn layout(title: &str, active: &str, body: &str) -> String {
    let nav = |id: &str, href: &str, label: &str| {
        let cls = if id == active { "active" } else { "" };
        format!(r#"<a class="{cls}" href="{href}">{label}</a>"#)
    };
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8"/>
<meta name="viewport" content="width=device-width, initial-scale=1"/>
<title>{title} · Anchored</title>
<link rel="stylesheet" href="/static/app.css"/>
</head>
<body>
<header class="top">
  <a class="brand" href="/dashboard">Anchored</a>
  <nav>
    {dashboard}
    {accounts}
    {transactions}
    {budgets}
    {networth}
    {subscriptions}
  </nav>
</header>
<main>
{body}
</main>
</body>
</html>"#,
        title = esc(title),
        dashboard = nav("dashboard", "/dashboard", "Dashboard"),
        accounts = nav("accounts", "/accounts", "Accounts"),
        transactions = nav("transactions", "/transactions", "Transactions"),
        budgets = nav("budgets", "/budgets", "Budgets"),
        networth = nav("net-worth", "/net-worth", "Net worth"),
        subscriptions = nav("subscriptions", "/subscriptions", "Subscriptions"),
        body = body,
    )
}

pub fn landing(plaid_ready: bool) -> String {
    let plaid = if plaid_ready {
        "<p class=\"muted\">Bank linking is configured (read-only).</p>"
    } else {
        "<p class=\"muted\">Add Plaid sandbox keys to link banks. Demo data works without them.</p>"
    };
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8"/>
<meta name="viewport" content="width=device-width, initial-scale=1"/>
<title>Anchored</title>
<link rel="stylesheet" href="/static/app.css"/>
</head>
<body class="landing">
  <section class="hero">
    <p class="brand-mark">Anchored</p>
    <h1>Your books stay on this machine.</h1>
    <p class="lede">Encrypted SQLite, read-only bank linking, and a clear view of spend, budgets, and net worth.</p>
    <p class="cta"><a class="btn" href="/dashboard">Open dashboard</a></p>
    {plaid}
  </section>
</body>
</html>"#
    )
}

pub fn dashboard(summary: &DashboardSummary) -> String {
    let mut cats = String::new();
    for c in &summary.spend_by_category {
        let _ = write!(
            cats,
            "<li><span>{}</span><span>{}</span></li>",
            esc(&c.category),
            money(c.amount)
        );
    }
    let mut recent = String::new();
    for t in &summary.recent_transactions {
        let _ = write!(
            recent,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td class=\"num\">{}</td></tr>",
            esc(&t.date),
            esc(t.account_name.as_deref().unwrap_or("")),
            esc(&t.name),
            money(t.amount)
        );
    }
    let body = format!(
        r#"<h1>Dashboard</h1>
<section class="metrics">
  <div><span class="label">Net worth</span><strong>{nw}</strong></div>
  <div><span class="label">Cash</span><strong>{cash}</strong></div>
  <div><span class="label">Investments</span><strong>{inv}</strong></div>
  <div><span class="label">Credit used</span><strong>{credit}</strong></div>
  <div><span class="label">Month spend</span><strong>{spend}</strong></div>
  <div><span class="label">Month income</span><strong>{income}</strong></div>
  <div><span class="label">Budget</span><strong>{bspent} / {btotal}</strong></div>
  <div><span class="label">Subscriptions</span><strong>{subs} ({scount})</strong></div>
</section>
<section class="split">
  <div>
    <h2>Spend by category</h2>
    <ul class="list">{cats}</ul>
  </div>
  <div>
    <h2>Recent transactions</h2>
    <table>
      <thead><tr><th>Date</th><th>Account</th><th>Name</th><th class="num">Amount</th></tr></thead>
      <tbody>{recent}</tbody>
    </table>
  </div>
</section>"#,
        nw = money(summary.net_worth),
        cash = money(summary.cash),
        inv = money(summary.investments),
        credit = money(summary.credit_used),
        spend = money(summary.month_spend),
        income = money(summary.month_income),
        bspent = money(summary.budget_spent),
        btotal = money(summary.budget_total),
        subs = money(summary.subscription_total),
        scount = summary.subscription_count,
        cats = cats,
        recent = recent,
    );
    layout("Dashboard", "dashboard", &body)
}

pub fn accounts_page(accounts: &[Account]) -> String {
    let mut rows = String::new();
    for a in accounts {
        let _ = write!(
            rows,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td class=\"num\">{}</td></tr>",
            esc(&a.name),
            esc(&a.account_type),
            esc(a.institution_name.as_deref().unwrap_or("—")),
            esc(a.mask.as_deref().unwrap_or("")),
            money(a.balance_current)
        );
    }
    let body = format!(
        r#"<h1>Accounts</h1>
<p class="muted">Balances on this computer. Linked banks stay read-only.</p>
<form class="panel" method="post" action="/accounts">
  <h2>Add manual account</h2>
  <div class="grid">
    <label>Name <input name="name" required/></label>
    <label>Type
      <select name="type">
        <option value="checking">Checking</option>
        <option value="savings">Savings</option>
        <option value="credit">Credit</option>
        <option value="investment">Investment</option>
        <option value="loan">Loan</option>
        <option value="other">Other</option>
      </select>
    </label>
    <label>Balance <input name="balance" type="number" step="0.01" value="0"/></label>
    <label>Institution <input name="institution"/></label>
  </div>
  <button type="submit" class="btn">Save account</button>
</form>
<table>
  <thead><tr><th>Name</th><th>Type</th><th>Institution</th><th>Mask</th><th class="num">Balance</th></tr></thead>
  <tbody>{rows}</tbody>
</table>"#,
        rows = rows
    );
    layout("Accounts", "accounts", &body)
}

pub fn transactions_page(txns: &[Transaction]) -> String {
    let mut rows = String::new();
    for t in txns {
        let _ = write!(
            rows,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td class=\"num\">{}</td></tr>",
            esc(&t.date),
            esc(t.account_name.as_deref().unwrap_or("")),
            esc(&t.name),
            esc(&t.category),
            money(t.amount)
        );
    }
    let body = format!(
        r#"<h1>Transactions</h1>
<table>
  <thead><tr><th>Date</th><th>Account</th><th>Name</th><th>Category</th><th class="num">Amount</th></tr></thead>
  <tbody>{rows}</tbody>
</table>"#,
        rows = rows
    );
    layout("Transactions", "transactions", &body)
}

pub fn budgets_page(budgets: &[Budget], spent: &[(String, f64)]) -> String {
    let spent_map: std::collections::HashMap<&str, f64> =
        spent.iter().map(|(c, a)| (c.as_str(), *a)).collect();
    let mut rows = String::new();
    for b in budgets {
        let used = *spent_map.get(b.category.as_str()).unwrap_or(&0.0);
        let _ = write!(
            rows,
            "<tr><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td></tr>",
            esc(&b.category),
            money(used),
            money(b.monthly_limit)
        );
    }
    let body = format!(
        r#"<h1>Budgets</h1>
<table>
  <thead><tr><th>Category</th><th class="num">Spent</th><th class="num">Limit</th></tr></thead>
  <tbody>{rows}</tbody>
</table>"#,
        rows = rows
    );
    layout("Budgets", "budgets", &body)
}

pub fn net_worth_page(nw: &NetWorthBreakdown) -> String {
    let mut rows = String::new();
    for a in &nw.accounts {
        if a.include_in_net_worth == 0 {
            continue;
        }
        let _ = write!(
            rows,
            "<tr><td>{}</td><td>{}</td><td class=\"num\">{}</td></tr>",
            esc(&a.name),
            esc(&a.account_type),
            money(a.balance_current)
        );
    }
    let body = format!(
        r#"<h1>Net worth</h1>
<section class="metrics">
  <div><span class="label">Total</span><strong>{}</strong></div>
  <div><span class="label">Cash</span><strong>{}</strong></div>
  <div><span class="label">Investments</span><strong>{}</strong></div>
  <div><span class="label">Credit used</span><strong>{}</strong></div>
  <div><span class="label">Loans</span><strong>{}</strong></div>
</section>
<table>
  <thead><tr><th>Account</th><th>Type</th><th class="num">Balance</th></tr></thead>
  <tbody>{rows}</tbody>
</table>"#,
        money(nw.net_worth),
        money(nw.cash),
        money(nw.investments),
        money(nw.credit_used),
        money(nw.loans),
        rows = rows
    );
    layout("Net worth", "net-worth", &body)
}

pub fn subscriptions_page(txns: &[Transaction]) -> String {
    let mut rows = String::new();
    for t in txns {
        let _ = write!(
            rows,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td class=\"num\">{}</td></tr>",
            esc(&t.date),
            esc(t.merchant_name.as_deref().unwrap_or(&t.name)),
            esc(t.account_name.as_deref().unwrap_or("")),
            money(t.amount)
        );
    }
    let body = format!(
        r#"<h1>Subscriptions</h1>
<p class="muted">Detected from this month's charges.</p>
<table>
  <thead><tr><th>Date</th><th>Merchant</th><th>Account</th><th class="num">Amount</th></tr></thead>
  <tbody>{rows}</tbody>
</table>"#,
        rows = rows
    );
    layout("Subscriptions", "subscriptions", &body)
}

pub fn error_page(msg: &str) -> String {
    layout(
        "Error",
        "",
        &format!("<h1>Something went wrong</h1><p class=\"muted\">{}</p>", esc(msg)),
    )
}

pub fn budget_spend_this_month(
    db: &crate::db::Db,
) -> Result<Vec<(String, f64)>, String> {
    let start = finance::month_start();
    let conn = db.lock()?;
    let mut stmt = conn
        .prepare(
            r#"SELECT t.category, COALESCE(SUM(t.amount),0)
               FROM transactions t
               INNER JOIN budgets b ON b.category = t.category
               WHERE t.date >= ? AND t.amount > 0
               GROUP BY t.category"#,
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![start], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}
