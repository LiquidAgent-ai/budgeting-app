export type AccountType =
  | "checking"
  | "savings"
  | "credit"
  | "investment"
  | "loan"
  | "other";

export type Account = {
  id: string;
  name: string;
  official_name: string | null;
  type: AccountType;
  subtype: string | null;
  mask: string | null;
  balance_current: number;
  balance_available: number | null;
  currency: string;
  institution_name: string | null;
  plaid_account_id: string | null;
  plaid_item_id: string | null;
  is_manual: number;
  include_in_net_worth: number;
  created_at: string;
  updated_at: string;
};

export type Transaction = {
  id: string;
  account_id: string;
  plaid_transaction_id: string | null;
  amount: number;
  date: string;
  name: string;
  merchant_name: string | null;
  category: string;
  subcategory: string | null;
  pending: number;
  is_subscription: number;
  notes: string | null;
  created_at: string;
};

export type Budget = {
  id: string;
  category: string;
  monthly_limit: number;
  created_at: string;
};

export type PlaidItem = {
  id: string;
  item_id: string;
  access_token: string;
  institution_id: string | null;
  institution_name: string | null;
  cursor: string | null;
  created_at: string;
  updated_at: string;
};

export type DashboardSummary = {
  netWorth: number;
  cash: number;
  creditUsed: number;
  investments: number;
  monthSpend: number;
  monthIncome: number;
  budgetTotal: number;
  budgetSpent: number;
  subscriptionTotal: number;
  subscriptionCount: number;
  recentTransactions: (Transaction & { account_name: string })[];
  spendByCategory: { category: string; amount: number }[];
  accounts: Account[];
};
