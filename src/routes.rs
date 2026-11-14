use crate::config::Config;
use crate::db::Db;
use crate::finance;
use crate::log::error_event;
use crate::plaid;
use crate::seed;
use crate::ui;
use axum::extract::{Form, State};
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
        .route("/transactions", get(transactions_page))
        .route("/budgets", get(budgets_page))
        .route("/net-worth", get(net_worth_page))
        .route("/subscriptions", get(subscriptions_page))
        .route("/api/health", get(health))
        .route("/api/dashboard", get(api_dashboard))
        .route("/api/accounts", get(api_accounts).post(api_create_account))
        .route("/api/transactions", get(api_transactions))
        .route("/api/budgets", get(api_budgets))
        .route("/api/net-worth", get(api_net_worth))
        .route("/api/subscriptions", get(api_subscriptions))
        .route("/api/seed", post(api_seed))
        .route("/api/plaid/status", get(api_plaid_status))
        .route("/api/plaid/create-link-token", post(api_plaid_link_token))
        .route("/api/plaid/exchange-token", post(api_plaid_exchange))
        .route("/api/plaid/sync", post(api_plaid_sync))
        .with_state(state)
}

async fn home(State(state): State<AppState>) -> Html<String> {
    Html(ui::landing(state.config.plaid_configured()))
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
        Ok(a) => Html(ui::accounts_page(&a)).into_response(),
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
        r#"INSERT INTO accounts (
            id, name, type, balance_current, balance_available, currency,
            institution_name, is_manual, include_in_net_worth
          ) VALUES (?, ?, ?, ?, ?, 'USD', ?, 1, 1)"#,
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

async fn transactions_page(State(state): State<AppState>) -> Response {
    match finance::list_transactions(&state.db, 200) {
        Ok(t) => Html(ui::transactions_page(&t)).into_response(),
        Err(_) => {
            error_event("transactions page failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(ui::error_page("Could not load transactions.")),
            )
                .into_response()
        }
    }
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
