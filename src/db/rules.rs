//! User category rules and tags. Built-in heuristics stay in `categories`.

use crate::db::categories::{self, CATEGORIES};
use crate::db::models::{CategoryRule, Tag};
use crate::db::sql;
use crate::db::Db;
use regex::Regex;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct CategorizeResult {
    pub category: String,
    pub is_subscription: bool,
    pub tags: Vec<String>,
    pub from_user_rule: bool,
}

pub fn list_categories() -> &'static [&'static str] {
    CATEGORIES
}

pub fn list_tags(db: &Db) -> Result<Vec<Tag>, String> {
    let conn = db.lock()?;
    list_tags_conn(&conn)
}

fn list_tags_conn(conn: &Connection) -> Result<Vec<Tag>, String> {
    let mut stmt = conn
        .prepare(sql::SELECT_TAGS)
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Tag {
                id: r.get(0)?,
                name: r.get(1)?,
                created_at: r.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

pub fn list_rules(db: &Db) -> Result<Vec<CategoryRule>, String> {
    let conn = db.lock()?;
    list_rules_conn(&conn, false)
}

fn list_rules_conn(conn: &Connection, enabled_only: bool) -> Result<Vec<CategoryRule>, String> {
    let sql = if enabled_only {
        sql::SELECT_ENABLED_CATEGORY_RULES
    } else {
        sql::SELECT_CATEGORY_RULES
    };
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], map_rule)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

fn map_rule(r: &rusqlite::Row<'_>) -> rusqlite::Result<CategoryRule> {
    Ok(CategoryRule {
        id: r.get(0)?,
        pattern: r.get(1)?,
        match_field: r.get(2)?,
        category: r.get(3)?,
        tags: r.get(4)?,
        is_subscription: r.get(5)?,
        priority: r.get(6)?,
        enabled: r.get(7)?,
        created_at: r.get(8)?,
    })
}

pub fn create_rule(
    db: &Db,
    pattern: &str,
    match_field: &str,
    category: &str,
    tags: &str,
    is_subscription: bool,
    priority: i64,
) -> Result<(), String> {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return Err("Pattern is required".into());
    }
    // Validate the pattern compiles; do not log the pattern (may contain payees).
    Regex::new(&format!("(?i){}", escape_literal_as_contains(pattern)))
        .map_err(|_| "That pattern could not be used.".to_string())?;

    let field = normalize_match_field(match_field);
    let category = normalize_category(category);
    let tags = normalize_tags_csv(tags);
    let conn = db.lock()?;
    conn.execute(
        sql::INSERT_CATEGORY_RULE,
        params![
            Uuid::new_v4().to_string(),
            pattern,
            field,
            category,
            tags,
            if is_subscription { 1 } else { 0 },
            priority.clamp(1, 10_000),
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_rule(db: &Db, id: &str) -> Result<(), String> {
    let id = id.trim();
    if id.is_empty() {
        return Err("Rule id is required".into());
    }
    let conn = db.lock()?;
    let n = conn
        .execute(sql::DELETE_CATEGORY_RULE, params![id])
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("That rule was not found.".into());
    }
    Ok(())
}

pub fn set_rule_enabled(db: &Db, id: &str, enabled: bool) -> Result<(), String> {
    let id = id.trim();
    if id.is_empty() {
        return Err("Rule id is required".into());
    }
    let conn = db.lock()?;
    let n = conn
        .execute(
            sql::UPDATE_CATEGORY_RULE_ENABLED,
            params![if enabled { 1 } else { 0 }, id],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("That rule was not found.".into());
    }
    Ok(())
}

/// Categorize using user rules first, then built-in heuristics.
pub fn categorize(conn: &Connection, name: &str, merchant: Option<&str>) -> CategorizeResult {
    if let Ok(Some(hit)) = match_user_rule(conn, name, merchant) {
        return hit;
    }
    let (category, is_subscription) = categories::categorize(name, merchant);
    CategorizeResult {
        category: category.to_string(),
        is_subscription,
        tags: Vec::new(),
        from_user_rule: false,
    }
}

fn match_user_rule(
    conn: &Connection,
    name: &str,
    merchant: Option<&str>,
) -> Result<Option<CategorizeResult>, String> {
    let rules = list_rules_conn(conn, true)?;
    for rule in rules {
        if rule_matches(&rule, name, merchant) {
            let tags = parse_tags_csv(rule.tags.as_deref().unwrap_or(""));
            return Ok(Some(CategorizeResult {
                category: rule.category,
                is_subscription: rule.is_subscription != 0,
                tags,
                from_user_rule: true,
            }));
        }
    }
    Ok(None)
}

fn rule_matches(rule: &CategoryRule, name: &str, merchant: Option<&str>) -> bool {
    let Ok(re) = Regex::new(&format!("(?i){}", escape_literal_as_contains(&rule.pattern))) else {
        return false;
    };
    let name_hit = re.is_match(name);
    let merchant_hit = merchant.map(|m| re.is_match(m)).unwrap_or(false);
    match rule.match_field.as_str() {
        "name" => name_hit,
        "merchant" => merchant_hit,
        _ => name_hit || merchant_hit,
    }
}

/// Treat user patterns as case-insensitive substring matches (not raw regex).
fn escape_literal_as_contains(pattern: &str) -> String {
    let mut out = String::from(".*");
    for ch in pattern.chars() {
        match ch {
            '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out.push_str(".*");
    out
}

pub fn rename_account(db: &Db, id: &str, name: &str) -> Result<(), String> {
    let id = id.trim();
    let name = name.trim();
    if id.is_empty() {
        return Err("Account id is required".into());
    }
    if name.is_empty() {
        return Err("Name is required".into());
    }
    let conn = db.lock()?;
    let n = conn
        .execute(sql::UPDATE_ACCOUNT_NAME, params![name, id])
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("That account was not found.".into());
    }
    Ok(())
}

pub fn update_transaction(
    db: &Db,
    id: &str,
    name: &str,
    category: &str,
    tags_csv: &str,
    is_subscription: bool,
) -> Result<(), String> {
    let id = id.trim();
    let name = name.trim();
    if id.is_empty() {
        return Err("Transaction id is required".into());
    }
    if name.is_empty() {
        return Err("Name is required".into());
    }
    let category = normalize_category(category);
    let tags = parse_tags_csv(tags_csv);
    let conn = db.lock()?;
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let n = tx
        .execute(
            sql::UPDATE_TRANSACTION_LABEL,
            params![
                name,
                category,
                if is_subscription { 1 } else { 0 },
                id
            ],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("That transaction was not found.".into());
    }
    set_transaction_tags(&*tx, id, &tags)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn set_transaction_tags(
    conn: &Connection,
    transaction_id: &str,
    tags: &[String],
) -> Result<(), String> {
    conn.execute(sql::DELETE_TRANSACTION_TAGS, params![transaction_id])
        .map_err(|e| e.to_string())?;
    for tag_name in tags {
        let tag_id = ensure_tag(conn, tag_name)?;
        conn.execute(
            sql::INSERT_TRANSACTION_TAG,
            params![transaction_id, tag_id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn ensure_tag(conn: &Connection, name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Tag name is empty".into());
    }
    if let Some(id) = conn
        .query_row(sql::SELECT_TAG_BY_NAME, params![name], |r| r.get::<_, String>(0))
        .optional()
        .map_err(|e| e.to_string())?
    {
        return Ok(id);
    }
    let id = Uuid::new_v4().to_string();
    conn.execute(sql::INSERT_TAG, params![id, name])
        .map_err(|e| e.to_string())?;
    Ok(id)
}

pub fn tags_for_transactions(
    conn: &Connection,
    ids: &[String],
) -> Result<HashMap<String, Vec<String>>, String> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    if ids.is_empty() {
        return Ok(map);
    }
    let mut stmt = conn
        .prepare(
            "SELECT tt.transaction_id, t.name
             FROM transaction_tags tt
             JOIN tags t ON t.id = tt.tag_id
             WHERE tt.transaction_id = ?
             ORDER BY t.name COLLATE NOCASE",
        )
        .map_err(|e| e.to_string())?;
    for id in ids {
        let tags = stmt
            .query_map(params![id], |r| r.get::<_, String>(1))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        if !tags.is_empty() {
            map.insert(id.clone(), tags);
        }
    }
    Ok(map)
}

/// Apply enabled user rules to existing transactions. Returns how many rows changed.
pub fn apply_rules_to_existing(db: &Db) -> Result<usize, String> {
    let conn = db.lock()?;
    let rules = list_rules_conn(&conn, true)?;
    if rules.is_empty() {
        return Ok(0);
    }
    let rows: Vec<(String, String, Option<String>)> = {
        let mut stmt = conn
            .prepare(sql::SELECT_ALL_TRANSACTION_IDS_NAMES)
            .map_err(|e| e.to_string())?;
        let mapped = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(|e| e.to_string())?;
        mapped
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
    };

    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let mut updated = 0usize;
    for (id, name, merchant) in rows {
        let Some(hit) = rules.iter().find(|r| rule_matches(r, &name, merchant.as_deref())) else {
            continue;
        };
        let tags = parse_tags_csv(hit.tags.as_deref().unwrap_or(""));
        tx.execute(
            sql::UPDATE_TRANSACTION_CATEGORY,
            params![
                hit.category,
                if hit.is_subscription != 0 { 1 } else { 0 },
                id
            ],
        )
        .map_err(|e| e.to_string())?;
        set_transaction_tags(&*tx, &id, &tags)?;
        updated += 1;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(updated)
}

fn normalize_match_field(field: &str) -> &'static str {
    match field.trim().to_ascii_lowercase().as_str() {
        "name" => "name",
        "merchant" => "merchant",
        _ => "either",
    }
}

fn normalize_category(category: &str) -> String {
    let trimmed = category.trim();
    if trimmed.is_empty() {
        return "Other".into();
    }
    for c in CATEGORIES {
        if c.eq_ignore_ascii_case(trimmed) {
            return (*c).to_string();
        }
    }
    // Allow custom category labels the user typed.
    trimmed.to_string()
}

fn normalize_tags_csv(raw: &str) -> Option<String> {
    let tags = parse_tags_csv(raw);
    if tags.is_empty() {
        None
    } else {
        Some(tags.join(", "))
    }
}

pub fn parse_tags_csv(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in raw.split(|c| c == ',' || c == ';') {
        let t = part.trim();
        if t.is_empty() {
            continue;
        }
        if out.iter().any(|e: &String| e.eq_ignore_ascii_case(t)) {
            continue;
        }
        out.push(t.to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_db() -> (Db, std::path::PathBuf) {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("anchored-rules-test-{nanos}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("anchored.db");
        let db = Db::open(path, "rules-test-key-32chars-minimum!".into()).unwrap();
        (db, dir)
    }

    #[test]
    fn user_rule_beats_builtin_and_applies_tags() {
        let (db, dir) = temp_db();
        create_rule(
            &db,
            "COSTCO GAS",
            "either",
            "Transport",
            "car, gas",
            false,
            10,
        )
        .unwrap();
        let conn = db.lock().unwrap();
        let hit = categorize(&conn, "COSTCO GAS #123", None);
        assert_eq!(hit.category, "Transport");
        assert!(hit.from_user_rule);
        assert_eq!(hit.tags, vec!["car".to_string(), "gas".to_string()]);
        // Builtin would have called COSTCO groceries; user rule wins.
        drop(conn);
        drop(db);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn substring_pattern_is_not_raw_regex() {
        let (db, dir) = temp_db();
        create_rule(&db, "a+b", "name", "Fees", "", false, 10).unwrap();
        let conn = db.lock().unwrap();
        let hit = categorize(&conn, "invoice a+b fee", None);
        assert!(hit.from_user_rule);
        let miss = categorize(&conn, "invoice aaab fee", None);
        assert!(!miss.from_user_rule);
        drop(conn);
        drop(db);
        let _ = std::fs::remove_dir_all(dir);
    }
}
