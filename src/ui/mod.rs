use crate::db::finance;
use crate::db::models::{
    Account, Budget, CategoryRule, DashboardSummary, NetWorthBreakdown, Tag, Transaction,
};
use crate::db::rules;
use crate::db::sql;
use crate::teller::TellerStatus;
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
    {import}
    {rules}
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
        import = nav("import", "/import", "Import"),
        rules = nav("rules", "/rules", "Rules"),
        budgets = nav("budgets", "/budgets", "Budgets"),
        networth = nav("net-worth", "/net-worth", "Net worth"),
        subscriptions = nav("subscriptions", "/subscriptions", "Subscriptions"),
        body = body,
    )
}

fn category_options(selected: &str) -> String {
    let mut out = String::new();
    let mut matched = false;
    for c in rules::list_categories() {
        let sel = if c.eq_ignore_ascii_case(selected) {
            matched = true;
            " selected"
        } else {
            ""
        };
        let _ = write!(out, r#"<option value="{}"{sel}>{}</option>"#, esc(c), esc(c));
    }
    if !selected.is_empty() && !matched {
        let _ = write!(
            out,
            r#"<option value="{}" selected>{}</option>"#,
            esc(selected),
            esc(selected)
        );
    }
    out
}

pub fn landing(bank_ready: bool) -> String {
    let bank = if bank_ready {
        "<p class=\"muted\">Bank linking is configured (read-only via Teller or Plaid).</p>"
    } else {
        "<p class=\"muted\">Add a free Teller application id (or Plaid sandbox keys) to link banks. CSV/OFX import works without them. Demo data is optional from Accounts.</p>"
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
    <p class="lede">Encrypted SQLite, CSV/OFX imports or read-only bank linking, and a clear view of spend, budgets, and net worth.</p>
    <p class="cta"><a class="btn" href="/dashboard">Open dashboard</a> <a class="btn btn-quiet" href="/import">Import a statement</a></p>
    {bank}
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

pub fn accounts_page(
    accounts: &[Account],
    teller: TellerStatus,
    plaid_ready: bool,
) -> String {
    let mut rows = String::new();
    for a in accounts {
        let _ = write!(
            rows,
            r#"<tr>
  <td>
    <form class="inline-edit" method="post" action="/accounts/rename">
      <input type="hidden" name="id" value="{id}"/>
      <input name="name" value="{name}" required aria-label="Account name"/>
      <button type="submit" class="btn btn-quiet">Rename</button>
    </form>
  </td>
  <td>{ty}</td>
  <td>{inst}</td>
  <td>{mask}</td>
  <td class="num">{bal}</td>
  <td>
    <form method="post" action="/accounts/delete" class="inline-form" onsubmit="return confirm('Remove this account and its transactions?');">
      <input type="hidden" name="id" value="{id}"/>
      <button type="submit" class="btn btn-quiet">Remove</button>
    </form>
  </td>
</tr>"#,
            id = esc(&a.id),
            name = esc(&a.name),
            ty = esc(&a.account_type),
            inst = esc(a.institution_name.as_deref().unwrap_or("—")),
            mask = esc(a.mask.as_deref().unwrap_or("")),
            bal = money(a.balance_current)
        );
    }

    let link_panel = if teller.configured {
        let app_id = esc(teller.application_id.as_deref().unwrap_or(""));
        let env = esc(&teller.env);
        let cert_note = if teller.certificate_ready {
            ""
        } else {
            "<p class=\"muted\">Development/production also needs a Teller client certificate on the server.</p>"
        };
        format!(
            r#"<section class="panel bank-link" id="teller-link">
  <h2>Link with Teller</h2>
  <p class="muted">Read-only balances and transactions. Anchored never requests payments, verify, or identity.</p>
  {cert_note}
  <p class="bank-actions">
    <button type="button" class="btn" id="teller-connect-btn">Connect a bank</button>
    <button type="button" class="btn btn-quiet" id="teller-sync-btn">Sync Teller</button>
  </p>
  <p class="muted" id="teller-status" aria-live="polite"></p>
</section>
<script src="https://cdn.teller.io/connect/connect.js"></script>
<script>
(function () {{
  var statusEl = document.getElementById("teller-status");
  function setStatus(msg) {{ if (statusEl) statusEl.textContent = msg || ""; }}
  var connectBtn = document.getElementById("teller-connect-btn");
  var syncBtn = document.getElementById("teller-sync-btn");
  if (typeof TellerConnect === "undefined") {{
    setStatus("Teller Connect could not load. Check network access to cdn.teller.io.");
    if (connectBtn) connectBtn.disabled = true;
  }} else if (connectBtn) {{
    var tellerConnect = TellerConnect.setup({{
      applicationId: "{app_id}",
      environment: "{env}",
      products: ["balance", "transactions"],
      onSuccess: function (enrollment) {{
        setStatus("Saving linked accounts…");
        fetch("/api/teller/enroll", {{
          method: "POST",
          headers: {{ "content-type": "application/json" }},
          body: JSON.stringify({{
            access_token: enrollment.accessToken,
            enrollment: enrollment.enrollment,
            user: enrollment.user
          }})
        }}).then(function (res) {{
          return res.json().then(function (body) {{
            if (!res.ok) throw new Error(body.error || "Could not save enrollment");
            setStatus("Linked. Reloading…");
            window.location.reload();
          }});
        }}).catch(function (err) {{
          setStatus(err.message || "Could not save enrollment");
        }});
      }},
      onExit: function () {{ setStatus(""); }}
    }});
    connectBtn.addEventListener("click", function () {{ tellerConnect.open(); }});
  }}
  if (syncBtn) {{
    syncBtn.addEventListener("click", function () {{
      setStatus("Syncing…");
      syncBtn.disabled = true;
      fetch("/api/teller/sync", {{ method: "POST" }})
        .then(function (res) {{
          return res.json().then(function (body) {{
            if (!res.ok) throw new Error(body.error || "Sync failed");
            setStatus("Synced " + (body.enrollments || 0) + " enrollment(s). Reloading…");
            window.location.reload();
          }});
        }})
        .catch(function (err) {{
          setStatus(err.message || "Sync failed");
          syncBtn.disabled = false;
        }});
    }});
  }}
}})();
</script>"#,
            app_id = app_id,
            env = env,
            cert_note = cert_note
        )
    } else if plaid_ready {
        r#"<section class="panel bank-link">
  <h2>Bank linking</h2>
  <p class="muted">Plaid is configured (read-only). Add <code>TELLER_APPLICATION_ID</code> for free Teller linking, or use Import for statement files.</p>
</section>"#
            .to_string()
    } else {
        r#"<section class="panel bank-link">
  <h2>Bank linking</h2>
  <p class="muted">Set <code>TELLER_APPLICATION_ID</code> (free developer tier) or Plaid keys to link banks read-only. You can still add manual accounts or use Import.</p>
</section>"#
            .to_string()
    };

    let body = format!(
        r#"<h1>Accounts</h1>
<p class="muted">Balances on this computer. Linked banks stay read-only. New installs start empty — imports never invent checking or IRA accounts.</p>
{link_panel}
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
<form class="panel" method="post" action="/accounts/seed" onsubmit="return confirm('Load the sample household? This only runs when you have no accounts yet, unless you check wipe.');">
  <h2>Optional demo data</h2>
  <p class="muted">Sample accounts such as Everyday Checking and a Vanguard Roth IRA. Off by default on first start.</p>
  <label class="check"><input type="checkbox" name="fresh" value="1"/> Wipe the database and replace everything with the demo</label>
  <button type="submit" class="btn btn-quiet">Load demo household</button>
</form>
<table>
  <thead><tr><th>Name</th><th>Type</th><th>Institution</th><th>Mask</th><th class="num">Balance</th><th></th></tr></thead>
  <tbody>{rows}</tbody>
</table>"#,
        link_panel = link_panel,
        rows = rows
    );
    layout("Accounts", "accounts", &body)
}

pub fn transactions_page(txns: &[Transaction], _known_tags: &[Tag]) -> String {
    let mut rows = String::new();
    for t in txns {
        let tags_value = t.tags.join(", ");
        let sub_checked = if t.is_subscription != 0 {
            " checked"
        } else {
            ""
        };
        let _ = write!(
            rows,
            r#"<tr class="txn-row">
  <td>{date}</td>
  <td>{account}</td>
  <td colspan="4">
    <form class="txn-edit" method="post" action="/transactions/edit">
      <input type="hidden" name="id" value="{id}"/>
      <div class="grid txn-grid">
        <label>Name <input name="name" value="{name}" required/></label>
        <label>Category <select name="category">{cats}</select></label>
        <label>Tags <input name="tags" value="{tags}" placeholder="comma-separated"/></label>
        <label class="check"><input type="checkbox" name="is_subscription" value="1"{sub}/> Subscription</label>
      </div>
      <div class="txn-edit-actions">
        <span class="num muted">{amount}</span>
        <button type="submit" class="btn btn-quiet">Save</button>
      </div>
    </form>
  </td>
</tr>"#,
            date = esc(&t.date),
            account = esc(t.account_name.as_deref().unwrap_or("")),
            id = esc(&t.id),
            name = esc(&t.name),
            cats = category_options(&t.category),
            tags = esc(&tags_value),
            sub = sub_checked,
            amount = money(t.amount),
        );
    }
    let body = format!(
        r#"<h1>Transactions</h1>
<p class="muted">Rename payees, set categories and tags, or add auto-rules on <a href="/rules">Rules</a>. Import statements on <a href="/import">Import</a>.</p>
<table>
  <thead><tr><th>Date</th><th>Account</th><th colspan="4">Details</th></tr></thead>
  <tbody>{rows}</tbody>
</table>"#,
        rows = rows
    );
    layout("Transactions", "transactions", &body)
}

pub fn rules_page(rules_list: &[CategoryRule], tags: &[Tag], notice: Option<String>) -> String {
    let notice_html = match notice {
        Some(msg) => format!(r#"<p class="notice">{}</p>"#, esc(&msg)),
        None => String::new(),
    };
    let mut rows = String::new();
    for r in rules_list {
        let enabled = r.enabled != 0;
        let toggle_value = if enabled { "0" } else { "1" };
        let toggle_label = if enabled { "Disable" } else { "Enable" };
        let _ = write!(
            rows,
            r#"<tr>
  <td><code>{pattern}</code></td>
  <td>{field}</td>
  <td>{category}</td>
  <td>{tags}</td>
  <td>{priority}</td>
  <td>{status}</td>
  <td class="rule-actions">
    <form method="post" action="/rules/toggle" class="inline-form">
      <input type="hidden" name="id" value="{id}"/>
      <input type="hidden" name="enabled" value="{toggle_value}"/>
      <button type="submit" class="btn btn-quiet">{toggle_label}</button>
    </form>
    <form method="post" action="/rules/delete" class="inline-form" onsubmit="return confirm('Delete this rule?');">
      <input type="hidden" name="id" value="{id}"/>
      <button type="submit" class="btn btn-quiet">Delete</button>
    </form>
  </td>
</tr>"#,
            pattern = esc(&r.pattern),
            field = esc(&r.match_field),
            category = esc(&r.category),
            tags = esc(r.tags.as_deref().unwrap_or("—")),
            priority = r.priority,
            status = if enabled { "On" } else { "Off" },
            id = esc(&r.id),
            toggle_value = toggle_value,
            toggle_label = toggle_label,
        );
    }
    let mut tag_chips = String::new();
    for t in tags {
        let _ = write!(tag_chips, r#"<span class="tag-chip">{}</span>"#, esc(&t.name));
    }
    if tag_chips.is_empty() {
        tag_chips = r#"<span class="muted">No tags yet.</span>"#.into();
    }
    let cats = category_options("Other");
    let body = format!(
        r#"<h1>Category rules</h1>
<p class="muted">Match a payee or merchant substring and set the category (and optional tags) automatically on import and bank sync. Lower priority runs first.</p>
{notice}
<form class="panel" method="post" action="/rules/create">
  <h2>Add rule</h2>
  <div class="grid">
    <label>Contains <input name="pattern" required placeholder="e.g. COSTCO GAS"/></label>
    <label>Match
      <select name="match_field">
        <option value="either">Name or merchant</option>
        <option value="name">Name only</option>
        <option value="merchant">Merchant only</option>
      </select>
    </label>
    <label>Category <select name="category">{cats}</select></label>
    <label>Tags <input name="tags" placeholder="comma-separated"/></label>
    <label>Priority <input name="priority" type="number" value="100" min="1" max="10000"/></label>
    <label class="check"><input type="checkbox" name="is_subscription" value="1"/> Mark as subscription</label>
  </div>
  <button type="submit" class="btn">Save rule</button>
</form>
<form class="panel" method="post" action="/rules/apply" onsubmit="return confirm('Apply enabled rules to existing transactions? Matching rows will be updated.');">
  <h2>Apply to existing</h2>
  <p class="muted">Re-run enabled rules against transactions already on this computer. Only matches change.</p>
  <button type="submit" class="btn btn-quiet">Apply rules now</button>
</form>
<section class="panel">
  <h2>Tags in use</h2>
  <div class="tag-list">{tag_chips}</div>
</section>
<table>
  <thead><tr><th>Pattern</th><th>Match</th><th>Category</th><th>Tags</th><th>Priority</th><th>Status</th><th></th></tr></thead>
  <tbody>{rows}</tbody>
</table>"#,
        notice = notice_html,
        cats = cats,
        tag_chips = tag_chips,
        rows = rows,
    );
    layout("Rules", "rules", &body)
}

pub fn import_page(accounts: &[Account], notice: Option<String>) -> String {
    let mut options = String::from(r#"<option value="">Select account</option>"#);
    for a in accounts {
        let label = match a.institution_name.as_deref() {
            Some(inst) if !inst.is_empty() => format!("{} · {} ({})", a.name, inst, a.account_type),
            _ => format!("{} ({})", a.name, a.account_type),
        };
        let _ = write!(
            options,
            r#"<option value="{}">{}</option>"#,
            esc(&a.id),
            esc(&label)
        );
    }
    let notice_html = match notice {
        Some(msg) => format!(r#"<p class="notice">{}</p>"#, esc(&msg)),
        None => String::new(),
    };
    let empty = if accounts.is_empty() {
        r#"<p class="muted">Add a manual account first, then import into it.</p>"#
    } else {
        ""
    };
    let body = format!(
        r#"<h1>Import</h1>
<p class="muted">Bring in transactions from a bank or credit-card export without connecting Teller or Plaid. Supports CSV, TSV, OFX, QFX, and QIF.</p>
{notice}
{empty}
<form class="panel" method="post" action="/import" enctype="multipart/form-data">
  <h2>Statement file</h2>
  <div class="grid">
    <label>Account
      <select name="account_id" required>{options}</select>
    </label>
    <label>File
      <input name="file" type="file" accept=".csv,.tsv,.txt,.ofx,.qfx,.qif,text/csv,application/x-ofx,application/vnd.intu.qfx" required/>
    </label>
  </div>
  <button type="submit" class="btn">Import transactions</button>
</form>
<p class="muted">Amounts follow the same sign as Plaid: spend is positive, income is negative. Duplicates are skipped — same file twice, QFX then bank sync, or matching spend already on the account.</p>"#,
        notice = notice_html,
        empty = empty,
        options = options
    );
    layout("Import", "import", &body)
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
            sql::SELECT_BUDGET_SPEND_BY_CATEGORY,
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![start], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}
