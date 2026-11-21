//! SQL text loaded from sibling `.sql` files. Bind parameters stay in Rust.

pub const SCHEMA: &str = include_str!("schema.sql");
pub const SELECT_SQLITE_MASTER_COUNT: &str = include_str!("select_sqlite_master_count.sql");
pub const SELECT_USER_TABLES: &str = include_str!("select_user_tables.sql");
pub const PRAGMA_TABLE_INFO_PLAID_ITEMS: &str = include_str!("pragma_table_info_plaid_items.sql");
pub const PRAGMA_TABLE_INFO_ACCOUNTS: &str = include_str!("pragma_table_info_accounts.sql");
pub const PRAGMA_TABLE_INFO_TRANSACTIONS: &str =
    include_str!("pragma_table_info_transactions.sql");
pub const ALTER_PLAID_ITEMS_ADD_ACCESS_MODE: &str =
    include_str!("alter_plaid_items_add_access_mode.sql");
pub const ALTER_ACCOUNTS_ADD_TELLER_ACCOUNT_ID: &str =
    include_str!("alter_accounts_add_teller_account_id.sql");
pub const ALTER_ACCOUNTS_ADD_TELLER_ENROLLMENT_ID: &str =
    include_str!("alter_accounts_add_teller_enrollment_id.sql");
pub const ALTER_TRANSACTIONS_ADD_TELLER_TRANSACTION_ID: &str =
    include_str!("alter_transactions_add_teller_transaction_id.sql");

pub const SELECT_ACCOUNTS: &str = include_str!("select_accounts.sql");
pub const SELECT_MONTH_SPEND_INCOME: &str = include_str!("select_month_spend_income.sql");
pub const SELECT_BUDGET_TOTAL: &str = include_str!("select_budget_total.sql");
pub const SELECT_BUDGET_SPENT: &str = include_str!("select_budget_spent.sql");
pub const SELECT_SUBSCRIPTION_TOTALS: &str = include_str!("select_subscription_totals.sql");
pub const SELECT_RECENT_TRANSACTIONS: &str = include_str!("select_recent_transactions.sql");
pub const SELECT_SPEND_BY_CATEGORY: &str = include_str!("select_spend_by_category.sql");
pub const SELECT_TRANSACTIONS: &str = include_str!("select_transactions.sql");
pub const SELECT_BUDGETS: &str = include_str!("select_budgets.sql");
pub const SELECT_SUBSCRIPTIONS: &str = include_str!("select_subscriptions.sql");
pub const SELECT_BUDGET_SPEND_BY_CATEGORY: &str =
    include_str!("select_budget_spend_by_category.sql");

pub const INSERT_PLAID_ITEM: &str = include_str!("insert_plaid_item.sql");
pub const INSERT_PLAID_ACCOUNT: &str = include_str!("insert_plaid_account.sql");
pub const SELECT_PLAID_ITEMS: &str = include_str!("select_plaid_items.sql");
pub const DELETE_TRANSACTION_BY_PLAID_ID: &str =
    include_str!("delete_transaction_by_plaid_id.sql");
pub const UPDATE_PLAID_ITEM_CURSOR: &str = include_str!("update_plaid_item_cursor.sql");
pub const SELECT_ACCOUNT_BY_PLAID_IDS: &str = include_str!("select_account_by_plaid_ids.sql");
pub const SELECT_TRANSACTION_ID_BY_PLAID_ID: &str =
    include_str!("select_transaction_id_by_plaid_id.sql");
pub const UPDATE_TRANSACTION: &str = include_str!("update_transaction.sql");
pub const INSERT_PLAID_TRANSACTION: &str = include_str!("insert_plaid_transaction.sql");

pub const INSERT_TELLER_ENROLLMENT: &str = include_str!("insert_teller_enrollment.sql");
pub const INSERT_TELLER_ACCOUNT: &str = include_str!("insert_teller_account.sql");
pub const SELECT_TELLER_ENROLLMENTS: &str = include_str!("select_teller_enrollments.sql");
pub const SELECT_TELLER_ACCOUNTS: &str = include_str!("select_teller_accounts.sql");
pub const UPDATE_TELLER_ENROLLMENT_SYNC: &str = include_str!("update_teller_enrollment_sync.sql");
pub const UPDATE_ACCOUNT_BALANCES: &str = include_str!("update_account_balances.sql");
pub const SELECT_TRANSACTION_ID_BY_TELLER_ID: &str =
    include_str!("select_transaction_id_by_teller_id.sql");
pub const INSERT_TELLER_TRANSACTION: &str = include_str!("insert_teller_transaction.sql");

pub const INSERT_IMPORTED_TRANSACTION: &str = include_str!("insert_imported_transaction.sql");
pub const COUNT_ACCOUNTS_BY_ID: &str = include_str!("count_accounts_by_id.sql");
pub const SELECT_DUPLICATE_CANDIDATES: &str = include_str!("select_duplicate_candidates.sql");
pub const LINK_PLAID_TRANSACTION_ID: &str = include_str!("link_plaid_transaction_id.sql");
pub const LINK_TELLER_TRANSACTION_ID: &str = include_str!("link_teller_transaction_id.sql");

pub const INSERT_MANUAL_ACCOUNT: &str = include_str!("insert_manual_account.sql");

pub const COUNT_ACCOUNTS: &str = include_str!("count_accounts.sql");
pub const INSERT_SEED_ACCOUNT: &str = include_str!("insert_seed_account.sql");
pub const INSERT_SEED_TRANSACTION: &str = include_str!("insert_seed_transaction.sql");
pub const INSERT_BUDGET: &str = include_str!("insert_budget.sql");
