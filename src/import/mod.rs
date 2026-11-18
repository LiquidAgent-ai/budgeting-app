//! Import bank and card exports (CSV, TSV, OFX, QFX, QIF) without Plaid.

use crate::config::logging::{error_event, info_event};
use crate::db::categories::categorize;
use crate::db::sql;
use crate::db::Db;
use rusqlite::params;
use serde::Serialize;
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ParsedTxn {
    pub date: String,
    pub amount: f64,
    pub name: String,
    pub merchant_name: Option<String>,
    pub external_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ImportResult {
    pub imported: usize,
    pub skipped: usize,
    pub format: String,
    pub account_id: String,
}

pub fn import_bytes(
    db: &Db,
    account_id: &str,
    filename: &str,
    bytes: &[u8],
) -> Result<ImportResult, String> {
    let account_id = account_id.trim();
    if account_id.is_empty() {
        return Err("Choose an account for these transactions.".into());
    }
    ensure_account(db, account_id)?;

    let text = decode_text(bytes)?;
    let (format, mut rows) = parse_export(filename, &text)?;
    if rows.is_empty() {
        return Err("No transactions found in that file.".into());
    }

    // Newest first is fine; insert order does not matter.
    rows.sort_by(|a, b| a.date.cmp(&b.date));

    let mut imported = 0usize;
    let mut skipped = 0usize;
    {
        let conn = db.lock()?;
        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
        for row in &rows {
            let fingerprint = import_id(account_id, row);
            if txn_exists(&tx, &fingerprint)? {
                skipped += 1;
                continue;
            }
            let (category, is_sub) = categorize(&row.name, row.merchant_name.as_deref());
            tx.execute(
                sql::INSERT_IMPORTED_TRANSACTION,
                params![
                    Uuid::new_v4().to_string(),
                    account_id,
                    fingerprint,
                    row.amount,
                    row.date,
                    row.name,
                    row.merchant_name,
                    category,
                    if is_sub { 1 } else { 0 },
                    format!("imported:{format}"),
                ],
            )
            .map_err(|e| e.to_string())?;
            imported += 1;
        }
        tx.commit().map_err(|e| e.to_string())?;
    }

    info_event(&format!(
        "file import finished format={format} imported={imported} skipped={skipped}"
    ));
    Ok(ImportResult {
        imported,
        skipped,
        format: format.to_string(),
        account_id: account_id.to_string(),
    })
}

pub fn import_result_json(result: ImportResult) -> Value {
    json!({
        "ok": true,
        "imported": result.imported,
        "skipped": result.skipped,
        "format": result.format,
        "account_id": result.account_id,
    })
}

fn ensure_account(db: &Db, account_id: &str) -> Result<(), String> {
    let conn = db.lock()?;
    let found: i64 = conn
        .query_row(
            sql::COUNT_ACCOUNTS_BY_ID,
            params![account_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if found == 0 {
        return Err("That account was not found.".into());
    }
    Ok(())
}

fn txn_exists(conn: &rusqlite::Connection, fingerprint: &str) -> Result<bool, String> {
    let n: i64 = conn
        .query_row(
            sql::COUNT_TRANSACTIONS_BY_PLAID_ID,
            params![fingerprint],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(n > 0)
}

fn import_id(account_id: &str, row: &ParsedTxn) -> String {
    if let Some(ext) = row.external_id.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        return format!("import:{account_id}:{ext}");
    }
    // Stable fingerprint for re-imports of the same CSV row.
    let key = format!(
        "{}|{:.4}|{}|{}",
        row.date,
        row.amount,
        row.name.trim().to_lowercase(),
        row.merchant_name
            .as_deref()
            .unwrap_or("")
            .trim()
            .to_lowercase()
    );
    let digest = simple_hash(&key);
    format!("import:{account_id}:{digest}")
}

fn simple_hash(s: &str) -> String {
    // FNV-1a 64-bit — good enough for dedupe fingerprints, no extra crate.
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn decode_text(bytes: &[u8]) -> Result<String, String> {
    if bytes.is_empty() {
        return Err("The file was empty.".into());
    }
    // Strip UTF-8 BOM when present.
    let start = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        3
    } else {
        0
    };
    match std::str::from_utf8(&bytes[start..]) {
        Ok(s) => Ok(s.to_string()),
        Err(_) => {
            // Latin-1 fallback for older bank exports.
            Ok(bytes[start..].iter().map(|&b| b as char).collect())
        }
    }
}

fn parse_export(filename: &str, text: &str) -> Result<(&'static str, Vec<ParsedTxn>), String> {
    let lower = filename.to_lowercase();
    let trimmed = text.trim_start();
    let head = trimmed.chars().take(200).collect::<String>().to_uppercase();

    if lower.ends_with(".ofx")
        || lower.ends_with(".qfx")
        || head.contains("<OFX")
        || head.contains("OFXHEADER:")
    {
        return Ok(("ofx", parse_ofx(text)?));
    }
    if lower.ends_with(".qif") || trimmed.starts_with('!') {
        return Ok(("qif", parse_qif(text)?));
    }
    if lower.ends_with(".tsv") || looks_like_tsv(text) {
        return Ok(("tsv", parse_delimited(text, '\t')?));
    }
    // Default: CSV (also covers .txt bank downloads that are comma-separated).
    Ok(("csv", parse_delimited(text, ',')?))
}

fn looks_like_tsv(text: &str) -> bool {
    let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    line.contains('\t') && !line.contains(',')
}

fn parse_delimited(text: &str, sep: char) -> Result<Vec<ParsedTxn>, String> {
    let rows = read_rows(text, sep)?;
    if rows.is_empty() {
        return Err("The spreadsheet had no rows.".into());
    }

    // Skip title rows banks sometimes put above the real header.
    let header_idx = rows
        .iter()
        .position(|r| detect_columns(r).is_some())
        .ok_or_else(|| {
            "Could not find Date / Amount / Description columns. Export CSV from your bank or card and try again."
                .to_string()
        })?;
    let header = &rows[header_idx];
    let cols = detect_columns(header).expect("header checked");
    let used_split_columns = cols.amount.is_none();
    let mut out = Vec::new();
    for row in rows.iter().skip(header_idx + 1) {
        if row.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        match row_to_txn(row, &cols) {
            Ok(Some(txn)) => out.push(txn),
            Ok(None) => {}
            Err(_) => {}
        }
    }
    if out.is_empty() {
        return Err("Found a header but no usable transaction rows.".into());
    }
    // Single Amount columns from Chase-style exports often use negative for
    // spend. Debit/Credit columns already match the app. Flip when needed.
    if !used_split_columns {
        normalize_spend_positive(&mut out);
    }
    Ok(out)
}

/// App / Plaid convention: positive amount = money leaving the account.
fn normalize_spend_positive(rows: &mut [ParsedTxn]) {
    let neg = rows.iter().filter(|t| t.amount < 0.0).count();
    let pos = rows.iter().filter(|t| t.amount > 0.0).count();
    if neg > pos {
        for t in rows.iter_mut() {
            t.amount = -t.amount;
        }
    }
}

#[derive(Debug, Clone)]
struct ColumnMap {
    date: usize,
    amount: Option<usize>,
    debit: Option<usize>,
    credit: Option<usize>,
    name: usize,
    merchant: Option<usize>,
    id: Option<usize>,
}

fn detect_columns(header: &[String]) -> Option<ColumnMap> {
    let norms: Vec<String> = header.iter().map(|h| normalize_header(h)).collect();
    let find = |aliases: &[&str]| -> Option<usize> {
        norms.iter().position(|h| aliases.iter().any(|a| h == a))
    };

    let date = find(&[
        "date",
        "posted date",
        "posting date",
        "transaction date",
        "trans date",
        "post date",
        "settled date",
        "trade date",
    ])?;
    let name = find(&[
        "description",
        "desc",
        "memo",
        "name",
        "payee",
        "narrative",
        "transaction",
        "details",
        "merchant",
        "merchant name",
    ])
    .or_else(|| {
        // Some Chase exports use "Description" already covered; fall back to longest text-ish col.
        None
    })?;

    let amount = find(&[
        "amount",
        "transaction amount",
        "amt",
        "value",
        "sum",
        "transactionamount",
    ]);
    let debit = find(&["debit", "withdrawal", "withdrawals", "outflow", "charge", "charges"]);
    let credit = find(&["credit", "deposit", "deposits", "inflow", "payment", "payments"]);
    if amount.is_none() && debit.is_none() && credit.is_none() {
        return None;
    }

    let merchant = find(&["merchant", "merchant name", "payee"]).filter(|&i| i != name);
    let id = find(&[
        "fitid",
        "transaction id",
        "trans id",
        "id",
        "reference",
        "ref",
        "check number",
        "check#",
    ]);

    Some(ColumnMap {
        date,
        amount,
        debit,
        credit,
        name,
        merchant,
        id,
    })
}

fn normalize_header(h: &str) -> String {
    h.trim()
        .trim_start_matches('\u{feff}')
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c == '_' { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn row_to_txn(row: &[String], cols: &ColumnMap) -> Result<Option<ParsedTxn>, String> {
    let date_raw = row.get(cols.date).map(|s| s.trim()).unwrap_or("");
    if date_raw.is_empty() {
        return Ok(None);
    }
    let date = parse_date(date_raw).ok_or_else(|| format!("bad date: {date_raw}"))?;

    let amount = if let Some(i) = cols.amount {
        parse_amount(row.get(i).map(|s| s.as_str()).unwrap_or(""))?
    } else {
        let debit = cols
            .debit
            .and_then(|i| row.get(i))
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(parse_amount)
            .transpose()?
            .unwrap_or(0.0);
        let credit = cols
            .credit
            .and_then(|i| row.get(i))
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(parse_amount)
            .transpose()?
            .unwrap_or(0.0);
        // App convention (Plaid): positive = money out.
        debit.abs() - credit.abs()
    };

    if amount == 0.0 {
        // Keep zero rows only if they have a description — usually skip blanks.
        let name_probe = row.get(cols.name).map(|s| s.trim()).unwrap_or("");
        if name_probe.is_empty() {
            return Ok(None);
        }
    }

    let name = row
        .get(cols.name)
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("Imported transaction")
        .to_string();
    let merchant_name = cols
        .merchant
        .and_then(|i| row.get(i))
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let external_id = cols
        .id
        .and_then(|i| row.get(i))
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    Ok(Some(ParsedTxn {
        date,
        amount,
        name,
        merchant_name,
        external_id,
    }))
}

fn parse_amount(raw: &str) -> Result<f64, String> {
    let s = raw.trim();
    if s.is_empty() {
        return Ok(0.0);
    }
    let negative = s.contains('(') && s.contains(')') || s.ends_with('-');
    let cleaned: String = s
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-' || *c == '+')
        .collect();
    if cleaned.is_empty() || cleaned == "-" || cleaned == "+" || cleaned == "." {
        return Ok(0.0);
    }
    let mut n: f64 = cleaned
        .parse()
        .map_err(|_| format!("Could not read amount '{raw}'"))?;
    if negative && n > 0.0 {
        n = -n;
    }
    Ok(n)
}

fn parse_date(raw: &str) -> Option<String> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    // Already ISO.
    if let Some(iso) = take_iso_date(s) {
        return Some(iso);
    }
    // YYYYMMDD
    if s.len() >= 8 && s.as_bytes().iter().take(8).all(|b| b.is_ascii_digit()) {
        let y = &s[0..4];
        let m = &s[4..6];
        let d = &s[6..8];
        if valid_ymd(y, m, d) {
            return Some(format!("{y}-{m}-{d}"));
        }
    }
    // MM/DD/YYYY or M/D/YY and dashes.
    let sep = if s.contains('/') {
        '/'
    } else if s.contains('-') {
        '-'
    } else if s.contains('.') {
        '.'
    } else {
        return None;
    };
    let parts: Vec<&str> = s.split(sep).map(str::trim).collect();
    if parts.len() != 3 {
        return None;
    }
    let (a, b, c) = (parts[0], parts[1], parts[2]);
    // If first part is 4 digits → YYYY-MM-DD
    if a.len() == 4 {
        let y = a;
        let m = pad2(b)?;
        let d = pad2(c)?;
        if valid_ymd(y, &m, &d) {
            return Some(format!("{y}-{m}-{d}"));
        }
        return None;
    }
    // MM-DD-YYYY (US bank default)
    let m = pad2(a)?;
    let d = pad2(b)?;
    let y = normalize_year(c)?;
    if valid_ymd(&y, &m, &d) {
        return Some(format!("{y}-{m}-{d}"));
    }
    None
}

fn take_iso_date(s: &str) -> Option<String> {
    if s.len() >= 10 && s.as_bytes()[4] == b'-' && s.as_bytes()[7] == b'-' {
        let y = &s[0..4];
        let m = &s[5..7];
        let d = &s[8..10];
        if y.bytes().all(|b| b.is_ascii_digit())
            && m.bytes().all(|b| b.is_ascii_digit())
            && d.bytes().all(|b| b.is_ascii_digit())
            && valid_ymd(y, m, d)
        {
            return Some(format!("{y}-{m}-{d}"));
        }
    }
    None
}

fn pad2(s: &str) -> Option<String> {
    let n: u32 = s.parse().ok()?;
    if n == 0 || n > 31 {
        return None;
    }
    Some(format!("{n:02}"))
}

fn normalize_year(s: &str) -> Option<String> {
    if s.len() == 4 && s.bytes().all(|b| b.is_ascii_digit()) {
        return Some(s.to_string());
    }
    if s.len() == 2 && s.bytes().all(|b| b.is_ascii_digit()) {
        let n: i32 = s.parse().ok()?;
        // 00-69 → 2000s, 70-99 → 1900s (common bank heuristic).
        let full = if n >= 70 { 1900 + n } else { 2000 + n };
        return Some(full.to_string());
    }
    None
}

fn valid_ymd(y: &str, m: &str, d: &str) -> bool {
    let Ok(yi) = y.parse::<i32>() else {
        return false;
    };
    let Ok(mi) = m.parse::<u32>() else {
        return false;
    };
    let Ok(di) = d.parse::<u32>() else {
        return false;
    };
    (1970..=2100).contains(&yi) && (1..=12).contains(&mi) && (1..=31).contains(&di)
}

/// Minimal CSV/TSV reader that handles quotes.
fn read_rows(text: &str, sep: char) -> Result<Vec<Vec<String>>, String> {
    let mut rows = Vec::new();
    let mut field = String::new();
    let mut row = Vec::new();
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(c);
            }
            continue;
        }
        match c {
            '"' => in_quotes = true,
            ch if ch == sep => {
                row.push(std::mem::take(&mut field));
            }
            '\n' => {
                row.push(std::mem::take(&mut field));
                if row.iter().any(|f| !f.trim().is_empty()) {
                    rows.push(std::mem::take(&mut row));
                } else {
                    row.clear();
                }
            }
            '\r' => {}
            _ => field.push(c),
        }
    }
    if in_quotes {
        error_event("import csv had an unclosed quote");
        return Err("The CSV had an unclosed quote.".into());
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(std::mem::take(&mut field));
        if row.iter().any(|f| !f.trim().is_empty()) {
            rows.push(row);
        }
    }
    Ok(rows)
}

fn parse_ofx(text: &str) -> Result<Vec<ParsedTxn>, String> {
    // Strip OFX header before <OFX> if present.
    let body = if let Some(idx) = text.find("<OFX") {
        &text[idx..]
    } else if let Some(idx) = text.find("<ofx") {
        &text[idx..]
    } else {
        text
    };

    let mut out = Vec::new();
    // Match SGML-style tags used by bank QFX/OFX downloads.
    let mut rest = body;
    while let Some(start) = find_ci(rest, "<STMTTRN>") {
        let after = &rest[start + "<STMTTRN>".len()..];
        let end_rel = find_ci(after, "</STMTTRN>").unwrap_or(after.len());
        let block = &after[..end_rel];
        rest = if end_rel < after.len() {
            &after[end_rel + "</STMTTRN>".len()..]
        } else {
            ""
        };

        let date = ofx_tag(block, "DTPOSTED")
            .or_else(|| ofx_tag(block, "DTUSER"))
            .and_then(|d| parse_date(&d[..d.len().min(8)].to_string()).or_else(|| parse_date(&d)));
        let amount_raw = ofx_tag(block, "TRNAMT");
        let name = ofx_tag(block, "NAME")
            .or_else(|| ofx_tag(block, "MEMO"))
            .or_else(|| ofx_tag(block, "PAYEE"));
        let memo = ofx_tag(block, "MEMO");
        let fitid = ofx_tag(block, "FITID");

        let (Some(date), Some(amount_raw), Some(name)) = (date, amount_raw, name) else {
            continue;
        };
        let ofx_amount = parse_amount(&amount_raw)?;
        // OFX: positive TRNAMT increases balance. App/Plaid: positive = money out.
        let amount = -ofx_amount;
        let merchant_name = memo.filter(|m| m != &name);
        out.push(ParsedTxn {
            date,
            amount,
            name,
            merchant_name,
            external_id: fitid,
        });
    }

    if out.is_empty() {
        return Err("No <STMTTRN> transactions found in the OFX/QFX file.".into());
    }
    Ok(out)
}

fn find_ci(hay: &str, needle: &str) -> Option<usize> {
    hay.to_ascii_lowercase()
        .find(&needle.to_ascii_lowercase())
}

fn ofx_tag(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let idx = find_ci(block, &open)?;
    let after = &block[idx + open.len()..];
    // Value runs until next tag or newline.
    let end = after
        .find('<')
        .or_else(|| after.find('\n'))
        .or_else(|| after.find('\r'))
        .unwrap_or(after.len());
    let val = after[..end].trim().trim_matches('"').trim();
    if val.is_empty() {
        None
    } else {
        Some(val.to_string())
    }
}

fn parse_qif(text: &str) -> Result<Vec<ParsedTxn>, String> {
    let mut out = Vec::new();
    let mut date: Option<String> = None;
    let mut amount: Option<f64> = None;
    let mut name: Option<String> = None;
    let mut memo: Option<String> = None;

    let push = |out: &mut Vec<ParsedTxn>,
                date: &mut Option<String>,
                amount: &mut Option<f64>,
                name: &mut Option<String>,
                memo: &mut Option<String>| {
        if let (Some(d), Some(a), Some(n)) = (date.take(), amount.take(), name.take()) {
            let merchant_name = memo.take().filter(|m| m != &n);
            out.push(ParsedTxn {
                date: d,
                amount: a,
                name: n,
                merchant_name,
                external_id: None,
            });
        } else {
            date.take();
            amount.take();
            name.take();
            memo.take();
        }
    };

    for line in text.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        let code = line.chars().next().unwrap_or(' ');
        let rest = line[code.len_utf8()..].trim();
        match code {
            'D' => date = parse_date(rest),
            'T' | 'U' => amount = parse_amount(rest).ok(),
            'P' => name = Some(if rest.is_empty() {
                "Imported transaction".into()
            } else {
                rest.to_string()
            }),
            'M' => memo = Some(rest.to_string()).filter(|s| !s.is_empty()),
            '^' => push(&mut out, &mut date, &mut amount, &mut name, &mut memo),
            '!' | 'C' | 'N' | 'A' | 'L' | 'S' | 'E' | '$' | 'F' | 'B' | 'X' | 'Y' => {}
            _ => {}
        }
    }
    normalize_spend_positive(&mut out);

    if out.is_empty() {
        return Err("No transactions found in the QIF file.".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_chase_style_csv() {
        let csv = "Transaction Date,Post Date,Description,Category,Type,Amount,Memo\n\
05/01/2024,05/02/2024,WHOLE FOODS MARKET,Food,Sale,-54.20,\n\
05/02/2024,05/02/2024,UBER TRIP,Travel,Sale,-22.10,\n\
05/03/2024,05/03/2024,ACME PAYROLL,Income,Payment,3200.00,\n";
        let rows = parse_delimited(csv, ',').unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].date, "2024-05-01");
        // Majority negative in file → flipped so spend is positive.
        assert!((rows[0].amount - 54.20).abs() < 0.001);
        assert_eq!(rows[0].name, "WHOLE FOODS MARKET");
        assert!((rows[2].amount - (-3200.0)).abs() < 0.001);
    }

    #[test]
    fn parses_debit_credit_csv() {
        let csv = "Date,Description,Debit,Credit\n\
2024-06-01,RENT,1850.00,\n\
2024-06-02,PAYROLL,,3200.00\n";
        let rows = parse_delimited(csv, ',').unwrap();
        assert_eq!(rows.len(), 2);
        assert!((rows[0].amount - 1850.0).abs() < 0.001);
        assert!((rows[1].amount - (-3200.0)).abs() < 0.001);
    }

    #[test]
    fn parses_ofx_stmttrn() {
        let ofx = r#"OFXHEADER:100
DATA:OFXSGML
<OFX>
<BANKMSGSRSV1>
<STMTTRN>
<TRNTYPE>DEBIT
<DTPOSTED>20240615
<TRNAMT>-42.18
<FITID>abc123
<NAME>CHIPOTLE 1842
<MEMO>lunch
</STMTTRN>
</BANKMSGSRSV1>
</OFX>"#;
        let rows = parse_ofx(ofx).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].date, "2024-06-15");
        // OFX TRNAMT -42.18 (money out) → app +42.18 after negate? Wait:
        // OFX: negative often means debit/money out for some banks; standard says
        // positive increases balance. TRNAMT -42.18 means balance decreased.
        // We negate: -(-42.18) = +42.18 spend. Good.
        assert!((rows[0].amount - 42.18).abs() < 0.001);
        assert_eq!(rows[0].external_id.as_deref(), Some("abc123"));
    }

    #[test]
    fn parses_qif() {
        let qif = "!Type:Bank\n\
D06/01/2024\nT-54.20\nPWHOLE FOODS\n^\n\
D06/02/2024\nT-22.00\nPUBER\n^\n\
D06/03/2024\nT3200.00\nPPAYROLL\n^\n";
        let rows = parse_qif(qif).unwrap();
        assert_eq!(rows.len(), 3);
        // Majority negative in file → flip so spend is positive.
        assert!((rows[0].amount - 54.20).abs() < 0.001);
        assert!((rows[2].amount - (-3200.0)).abs() < 0.001);
    }
}
