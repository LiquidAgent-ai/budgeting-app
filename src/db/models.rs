use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub official_name: Option<String>,
    #[serde(rename = "type")]
    pub account_type: String,
    pub subtype: Option<String>,
    pub mask: Option<String>,
    pub balance_current: f64,
    pub balance_available: Option<f64>,
    pub currency: String,
    pub institution_name: Option<String>,
    pub plaid_account_id: Option<String>,
    pub plaid_item_id: Option<String>,
    pub is_manual: i64,
    pub include_in_net_worth: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub account_id: String,
    pub plaid_transaction_id: Option<String>,
    pub amount: f64,
    pub date: String,
    pub name: String,
    pub merchant_name: Option<String>,
    pub category: String,
    pub subcategory: Option<String>,
    pub pending: i64,
    pub is_subscription: i64,
    pub notes: Option<String>,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Budget {
    pub id: String,
    pub category: String,
    pub monthly_limit: f64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CategorySpend {
    pub category: String,
    pub amount: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DashboardSummary {
    pub net_worth: f64,
    pub cash: f64,
    pub credit_used: f64,
    pub investments: f64,
    pub month_spend: f64,
    pub month_income: f64,
    pub budget_total: f64,
    pub budget_spent: f64,
    pub subscription_total: f64,
    pub subscription_count: i64,
    pub recent_transactions: Vec<Transaction>,
    pub spend_by_category: Vec<CategorySpend>,
    pub accounts: Vec<Account>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetWorthBreakdown {
    pub net_worth: f64,
    pub cash: f64,
    pub credit_used: f64,
    pub investments: f64,
    pub loans: f64,
    pub accounts: Vec<Account>,
}
