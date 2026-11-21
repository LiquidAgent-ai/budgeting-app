use crate::config::logging::{error_event, info_event};
use crate::config::Config;
use crate::db::finance;
use crate::db::rules;
use crate::db::seed;
use crate::db::sql;
use crate::db::Db;
use crate::import;
use crate::plaid;
use crate::teller;
use crate::ui;
use axum::extract::{Form, Multipart, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Db>,
    pub config: Config,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(home))
        .route("/dashboard", get(dashboard_page))
        .route("/accounts", get(accounts_page).post(create_account_form))
        .route("/accounts/rename", post(rename_account_form))
        .route("/transactions", get(transactions_page))
        .route("/transactions/edit", post(edit_transaction_form))
        .route("/rules", get(rules_page))
        .route("/rules/create", post(create_rule_form))
        .route("/rules/delete", post(delete_rule_form))
        .route("/rules/toggle", post(toggle_rule_form))
        .route("/rules/apply", post(apply_rules_form))
        .route("/import", get(import_page).post(import_form))
        .route("/budgets", get(budgets_page))
        .route("/net-worth", get(net_worth_page))
        .route("/subscriptions", get(subscriptions_page))
        .route("/api/health", get(health))
        .route("/api/dashboard", get(api_dashboard))
        .route("/api/accounts", get(api_accounts).post(api_create_account))
        .route("/api/transactions", get(api_transactions))
        .route("/api/import", post(api_import))
        .route("/api/budgets", get(api_budgets))
        .route("/api/net-worth", get(api_net_worth))
        .route("/api/subscriptions", get(api_subscriptions))
        .route("/api/seed", post(api_seed))
        .route("/api/plaid/status", get(api_plaid_status))
        .route("/api/plaid/create-link-token", post(api_plaid_link_token))
        .route("/api/plaid/exchange-token", post(api_plaid_exchange))
        .route("/api/plaid/sync", post(api_plaid_sync))
        .route("/api/teller/status", get(api_teller_status))
        .route("/api/teller/enroll", post(api_teller_enroll))
        .route("/api/teller/sync", post(api_teller_sync))
        .with_state(state)
}

async fn home(State(state): State<AppState>) -> Html<String> {
    Html(ui::landing(state.config.bank_linking_configured()))
}

async fn dashboard_page(State(state): State<AppState>) -> Response {
    match finance::dashboard(&state.db) {
        Ok(s) => Html(ui::dashboard(&s)).into_response(),
        Err(_) => {
            error_event("dashboard page failed");
            (StatusCode::INTERNAL_SERVER_ERROR, Html(ui::error_page("Could not load dashboard.")))
                .into_response()
        }
    }
}

async fn accounts_page(State(state): State<AppState>) -> Response {
    match finance::list_accounts(&state.db) {
        Ok(a) => Html(ui::accounts_page(
            &a,
            teller::status(&state.config),
            state.config.plaid_configured(),
        ))
        .into_response(),
        Err(_) => {
            error_event("accounts page failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(ui::error_page("Could not load accounts.")),
            )
                .into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct AccountForm {
    pub name: String,
    #[serde(rename = "type")]
    pub account_type: String,
    pub balance: Option<f64>,
    pub institution: Option<String>,
}

async fn create_account_form(
    State(state): State<AppState>,
    Form(form): Form<AccountForm>,
) -> Response {
    if let Err(e) = insert_manual_account(&state.db, &form) {
        error_event("create account failed");
        return (
            StatusCode::BAD_REQUEST,
            Html(ui::error_page(&e)),
        )
            .into_response();
    }
    Redirect::to("/accounts").into_response()
}

#[derive(Deserialize)]
pub struct RenameAccountForm {
    pub id: String,
    pub name: String,
}

async fn rename_account_form(
    State(state): State<AppState>,
    Form(form): Form<RenameAccountForm>,
) -> Response {
    if let Err(e) = rules::rename_account(&state.db, &form.id, &form.name) {
        error_event("rename account failed");
        return (
            StatusCode::BAD_REQUEST,
            Html(ui::error_page(&e)),
        )
            .into_response();
    }
    Redirect::to("/accounts").into_response()
}

#[derive(Deserialize)]
pub struct EditTransactionForm {
    pub id: String,
    pub name: String,
    pub category: String,
    pub tags: Option<String>,
    pub is_subscription: Option<String>,
}

async fn edit_transaction_form(
    State(state): State<AppState>,
    Form(form): Form<EditTransactionForm>,
) -> Response {
    let is_sub = matches!(
        form.is_subscription.as_deref().map(str::trim),
        Some("1") | Some("true") | Some("on") | Some("yes")
    );
    if let Err(e) = rules::update_transaction(
        &state.db,
        &form.id,
        &form.name,
        &form.category,
        form.tags.as_deref().unwrap_or(""),
        is_sub,
    ) {
        error_event("edit transaction failed");
        return (
            StatusCode::BAD_REQUEST,
            Html(ui::error_page(&e)),
        )
            .into_response();
    }
    Redirect::to("/transactions").into_response()
}

async fn transactions_page(State(state): State<AppState>) -> Response {
    match (
        finance::list_transactions(&state.db, 200),
        rules::list_tags(&state.db),
    ) {
        (Ok(t), Ok(tags)) => Html(ui::transactions_page(&t, &tags)).into_response(),
        _ => {
            error_event("transactions page failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(ui::error_page("Could not load transactions.")),
            )
                .into_response()
        }
    }
}

async fn rules_page(State(state): State<AppState>) -> Response {
    match (rules::list_rules(&state.db), rules::list_tags(&state.db)) {
        (Ok(rules_list), Ok(tags)) => {
            Html(ui::rules_page(&rules_list, &tags, None)).into_response()
        }
        _ => {
            error_event("rules page failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(ui::error_page("Could not load category rules.")),
            )
                .into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct CreateRuleForm {
    pub pattern: String,
    pub match_field: Option<String>,
    pub category: String,
    pub tags: Option<String>,
    pub is_subscription: Option<String>,
    pub priority: Option<i64>,
}

async fn create_rule_form(
    State(state): State<AppState>,
    Form(form): Form<CreateRuleForm>,
) -> Response {
    let is_sub = matches!(
        form.is_subscription.as_deref().map(str::trim),
        Some("1") | Some("true") | Some("on") | Some("yes")
    );
    if let Err(e) = rules::create_rule(
        &state.db,
        &form.pattern,
        form.match_field.as_deref().unwrap_or("either"),
        &form.category,
        form.tags.as_deref().unwrap_or(""),
        is_sub,
        form.priority.unwrap_or(100),
    ) {
        error_event("create category rule failed");
        return (
            StatusCode::BAD_REQUEST,
            Html(ui::error_page(&e)),
        )
            .into_response();
    }
    info_event("category rule created");
    Redirect::to("/rules").into_response()
}

#[derive(Deserialize)]
pub struct RuleIdForm {
    pub id: String,
}

async fn delete_rule_form(
    State(state): State<AppState>,
    Form(form): Form<RuleIdForm>,
) -> Response {
    if let Err(e) = rules::delete_rule(&state.db, &form.id) {
        error_event("delete category rule failed");
        return (
            StatusCode::BAD_REQUEST,
            Html(ui::error_page(&e)),
        )
            .into_response();
    }
    info_event("category rule deleted");
    Redirect::to("/rules").into_response()
}

#[derive(Deserialize)]
pub struct ToggleRuleForm {
    pub id: String,
    pub enabled: Option<String>,
}

async fn toggle_rule_form(
    State(state): State<AppState>,
    Form(form): Form<ToggleRuleForm>,
) -> Response {
    let enabled = matches!(
        form.enabled.as_deref().map(str::trim),
        Some("1") | Some("true") | Some("on") | Some("yes")
    );
    if let Err(e) = rules::set_rule_enabled(&state.db, &form.id, enabled) {
        error_event("toggle category rule failed");
        return (
            StatusCode::BAD_REQUEST,
            Html(ui::error_page(&e)),
        )
            .into_response();
    }
    Redirect::to("/rules").into_response()
}

async fn apply_rules_form(State(state): State<AppState>) -> Response {
    match rules::apply_rules_to_existing(&state.db) {
        Ok(n) => {
            info_event(&format!("category rules applied count={n}"));
            match (rules::list_rules(&state.db), rules::list_tags(&state.db)) {
                (Ok(rules_list), Ok(tags)) => Html(ui::rules_page(
                    &rules_list,
                    &tags,
                    Some(format!("Updated {n} existing transaction(s).")),
                ))
                .into_response(),
                _ => Redirect::to("/rules").into_response(),
            }
        }
        Err(_) => {
            error_event("apply category rules failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(ui::error_page("Could not apply category rules.")),
            )
                .into_response()
        }
    }
}

fn insert_manual_account(db: &Db, form: &AccountForm) -> Result<(), String> {
    let name = form.name.trim();
    if name.is_empty() {
        return Err("Name is required".into());
    }
    let ty = match form.account_type.as_str() {
        "checking" | "savings" | "credit" | "investment" | "loan" | "other" => {
            form.account_type.as_str()
        }
        _ => "other",
    };
    let bal = form.balance.unwrap_or(0.0);
    let conn = db.lock()?;
    conn.execute(
        sql::INSERT_MANUAL_ACCOUNT,
        rusqlite::params![
            Uuid::new_v4().to_string(),
            name,
            ty,
            bal,
            bal,
            form.institution.as_deref().map(str::trim).filter(|s| !s.is_empty()),
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

async fn import_page(State(state): State<AppState>) -> Response {
    match finance::list_accounts(&state.db) {
        Ok(a) => Html(ui::import_page(&a, None)).into_response(),
        Err(_) => {
            error_event("import page failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(ui::error_page("Could not load accounts for import.")),
            )
                .into_response()
        }
    }
}

async fn import_form(State(state): State<AppState>, multipart: Multipart) -> Response {
    match read_import_upload(multipart).await {
        Ok((account_id, filename, bytes)) => {
            match import::import_bytes(&state.db, &account_id, &filename, &bytes) {
                Ok(result) => {
                    let msg = format!(
                        "Imported {imported} transactions from {format} ({skipped} already present).",
                        imported = result.imported,
                        format = result.format,
                        skipped = result.skipped
                    );
                    match finance::list_accounts(&state.db) {
                        Ok(a) => Html(ui::import_page(&a, Some(msg))).into_response(),
                        Err(_) => Redirect::to("/transactions").into_response(),
                    }
                }
                Err(e) => {
                    error_event("import failed");
                    match finance::list_accounts(&state.db) {
                        Ok(a) => Html(ui::import_page(&a, Some(e))).into_response(),
                        Err(_) => (
                            StatusCode::BAD_REQUEST,
                            Html(ui::error_page(&e)),
                        )
                            .into_response(),
                    }
                }
            }
        }
        Err(e) => {
            error_event("import upload failed");
            match finance::list_accounts(&state.db) {
                Ok(a) => Html(ui::import_page(&a, Some(e))).into_response(),
                Err(_) => (
                    StatusCode::BAD_REQUEST,
                    Html(ui::error_page(&e)),
                )
                    .into_response(),
            }
        }
    }
}

async fn read_import_upload(
    mut multipart: Multipart,
) -> Result<(String, String, Vec<u8>), String> {
    let mut account_id = String::new();
    let mut filename = String::from("upload.csv");
    let mut bytes: Option<Vec<u8>> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| "Could not read the uploaded file.".to_string())?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "account_id" {
            account_id = field
                .text()
                .await
                .map_err(|_| "Could not read the account field.".to_string())?;
            continue;
        }
        if name == "file" {
            if let Some(f) = field.file_name() {
                if !f.trim().is_empty() {
                    filename = f.to_string();
                }
            }
            let data = field
                .bytes()
                .await
                .map_err(|_| "Could not read the uploaded file.".to_string())?;
            if data.len() > 8 * 1024 * 1024 {
                return Err("That file is larger than 8 MB.".into());
            }
            bytes = Some(data.to_vec());
        }
    }

    let bytes = bytes.ok_or_else(|| "Choose a CSV, OFX, QFX, or QIF file.".to_string())?;
    if bytes.is_empty() {
        return Err("The uploaded file was empty.".into());
    }
    Ok((account_id, filename, bytes))
}

async fn budgets_page(State(state): State<AppState>) -> Response {
    match (finance::list_budgets(&state.db), ui::budget_spend_this_month(&state.db)) {
        (Ok(b), Ok(s)) => Html(ui::budgets_page(&b, &s)).into_response(),
        _ => {
            error_event("budgets page failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(ui::error_page("Could not load budgets.")),
            )
                .into_response()
        }
    }
}

async fn net_worth_page(State(state): State<AppState>) -> Response {
    match finance::net_worth(&state.db) {
        Ok(n) => Html(ui::net_worth_page(&n)).into_response(),
        Err(_) => {
            error_event("net worth page failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(ui::error_page("Could not load net worth.")),
            )
                .into_response()
        }
    }
}

async fn subscriptions_page(State(state): State<AppState>) -> Response {
    match finance::list_subscriptions(&state.db) {
        Ok(t) => Html(ui::subscriptions_page(&t)).into_response(),
        Err(_) => {
            error_event("subscriptions page failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(ui::error_page("Could not load subscriptions.")),
            )
                .into_response()
        }
    }
}

async fn health() -> impl IntoResponse {
    (
        [(header::CACHE_CONTROL, "no-store")],
        Json(json!({ "ok": true })),
    )
}

async fn api_dashboard(State(state): State<AppState>) -> Response {
    json_result(finance::dashboard(&state.db))
}

async fn api_accounts(State(state): State<AppState>) -> Response {
    json_result(finance::list_accounts(&state.db))
}

#[derive(Deserialize)]
pub struct ApiAccountBody {
    pub name: String,
    #[serde(rename = "type")]
    pub account_type: String,
    pub balance_current: Option<f64>,
    pub institution_name: Option<String>,
}

async fn api_create_account(
    State(state): State<AppState>,
    Json(body): Json<ApiAccountBody>,
) -> Response {
    let form = AccountForm {
        name: body.name,
        account_type: body.account_type,
        balance: body.balance_current,
        institution: body.institution_name,
    };
    match insert_manual_account(&state.db, &form) {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(e) => {
            error_event("api create account failed");
            (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response()
        }
    }
}

async fn api_transactions(State(state): State<AppState>) -> Response {
    json_result(finance::list_transactions(&state.db, 200))
}

async fn api_import(State(state): State<AppState>, multipart: Multipart) -> Response {
    match read_import_upload(multipart).await {
        Ok((account_id, filename, bytes)) => {
            match import::import_bytes(&state.db, &account_id, &filename, &bytes) {
                Ok(result) => Json(import::import_result_json(result)).into_response(),
                Err(e) => {
                    error_event("api import failed");
                    (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response()
                }
            }
        }
        Err(e) => {
            error_event("api import upload failed");
            (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response()
        }
    }
}

async fn api_budgets(State(state): State<AppState>) -> Response {
    json_result(finance::list_budgets(&state.db))
}

async fn api_net_worth(State(state): State<AppState>) -> Response {
    json_result(finance::net_worth(&state.db))
}

async fn api_subscriptions(State(state): State<AppState>) -> Response {
    json_result(finance::list_subscriptions(&state.db))
}

#[derive(Deserialize)]
pub struct SeedBody {
    pub fresh: Option<bool>,
}

async fn api_seed(State(state): State<AppState>, body: Option<Json<SeedBody>>) -> Response {
    let fresh = body.and_then(|b| b.fresh).unwrap_or(false);
    match seed::seed_database(&state.db, fresh) {
        Ok((seeded, message)) => Json(json!({ "seeded": seeded, "message": message })).into_response(),
        Err(_) => {
            error_event("seed failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Could not seed database" })),
            )
                .into_response()
        }
    }
}

async fn api_plaid_status(State(state): State<AppState>) -> Json<plaid::PlaidStatus> {
    Json(plaid::status(&state.config))
}

async fn api_plaid_link_token(
    State(state): State<AppState>,
    body: Option<Json<plaid::CreateLinkTokenBody>>,
) -> Response {
    let body = body.map(|b| b.0).unwrap_or(plaid::CreateLinkTokenBody {
        client_user_id: None,
    });
    match plaid::create_link_token(&state.config, body).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => {
            error_event("plaid link token failed");
            (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response()
        }
    }
}

async fn api_plaid_exchange(
    State(state): State<AppState>,
    Json(body): Json<plaid::ExchangeTokenBody>,
) -> Response {
    match plaid::exchange_token(&state.config, &state.db, body).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => {
            error_event("plaid exchange failed");
            (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response()
        }
    }
}

async fn api_plaid_sync(State(state): State<AppState>) -> Response {
    match plaid::sync_all(&state.config, &state.db).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => {
            error_event("plaid sync failed");
            (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response()
        }
    }
}

async fn api_teller_status(State(state): State<AppState>) -> Json<teller::TellerStatus> {
    Json(teller::status(&state.config))
}

async fn api_teller_enroll(
    State(state): State<AppState>,
    Json(body): Json<teller::EnrollBody>,
) -> Response {
    match teller::enroll(&state.config, &state.db, body).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => {
            error_event("teller enroll failed");
            (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response()
        }
    }
}

async fn api_teller_sync(State(state): State<AppState>) -> Response {
    match teller::sync_all(&state.config, &state.db).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => {
            error_event("teller sync failed");
            (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response()
        }
    }
}

fn json_result<T: serde::Serialize>(result: Result<T, String>) -> Response {
    match result {
        Ok(v) => Json(v).into_response(),
        Err(_) => {
            error_event("api request failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "request failed" })),
            )
                .into_response()
        }
    }
}

#[allow(dead_code)]
fn _value_ok(v: Value) -> Response {
    Json(v).into_response()
}
