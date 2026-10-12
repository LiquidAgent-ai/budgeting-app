import { startOfMonth, format } from "date-fns";
import { getDb } from "./db";
import type { Account, DashboardSummary, Transaction } from "./types";

export function monthStart(): string {
  return format(startOfMonth(new Date()), "yyyy-MM-dd");
}

export function computeNetWorth(accounts: Account[]): {
  netWorth: number;
  cash: number;
  creditUsed: number;
  investments: number;
} {
  let cash = 0;
  let creditUsed = 0;
  let investments = 0;
  let loans = 0;

  for (const a of accounts) {
    if (!a.include_in_net_worth) continue;
    if (a.type === "checking" || a.type === "savings") cash += a.balance_current;
    else if (a.type === "credit") creditUsed += Math.abs(Math.min(0, a.balance_current));
    else if (a.type === "investment") investments += a.balance_current;
    else if (a.type === "loan") loans += Math.abs(a.balance_current);
    else cash += a.balance_current;
  }

  return {
    netWorth: cash + investments - creditUsed - loans,
    cash,
    creditUsed,
    investments,
  };
}

export function getDashboardSummary(): DashboardSummary {
  const db = getDb();
  const accounts = db.prepare("SELECT * FROM accounts ORDER BY type, name").all() as Account[];
  const nw = computeNetWorth(accounts);
  const start = monthStart();

  const spendRow = db
    .prepare(
      `SELECT
         COALESCE(SUM(CASE WHEN amount > 0 AND category NOT IN ('Transfer','Income','Investments') THEN amount ELSE 0 END), 0) as spend,
         COALESCE(SUM(CASE WHEN amount < 0 AND category = 'Income' THEN ABS(amount) ELSE 0 END), 0) as income
       FROM transactions WHERE date >= ?`
    )
    .get(start) as { spend: number; income: number };

  const budgets = db.prepare("SELECT COALESCE(SUM(monthly_limit),0) as total FROM budgets").get() as {
    total: number;
  };

  const budgetSpent = db
    .prepare(
      `SELECT COALESCE(SUM(t.amount),0) as spent
       FROM transactions t
       INNER JOIN budgets b ON b.category = t.category
       WHERE t.date >= ? AND t.amount > 0`
    )
    .get(start) as { spent: number };

  const subs = db
    .prepare(
      `SELECT COUNT(DISTINCT COALESCE(merchant_name, name)) as cnt,
              COALESCE(SUM(amount),0) as total
       FROM transactions
       WHERE is_subscription = 1 AND amount > 0 AND date >= ?`
    )
    .get(start) as { cnt: number; total: number };

  const recentTransactions = db
    .prepare(
      `SELECT t.*, a.name as account_name
       FROM transactions t
       JOIN accounts a ON a.id = t.account_id
       ORDER BY t.date DESC, t.created_at DESC
       LIMIT 12`
    )
    .all() as (Transaction & { account_name: string })[];

  const spendByCategory = db
    .prepare(
      `SELECT category, SUM(amount) as amount
       FROM transactions
       WHERE date >= ? AND amount > 0 AND category NOT IN ('Transfer','Income','Investments')
       GROUP BY category
       ORDER BY amount DESC
       LIMIT 8`
    )
    .all(start) as { category: string; amount: number }[];

  return {
    ...nw,
    monthSpend: spendRow.spend,
    monthIncome: spendRow.income,
    budgetTotal: budgets.total,
    budgetSpent: budgetSpent.spent,
    subscriptionTotal: subs.total,
    subscriptionCount: subs.cnt,
    recentTransactions,
    spendByCategory,
    accounts,
  };
}
