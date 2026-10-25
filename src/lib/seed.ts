import { v4 as uuid } from "uuid";
import { getDb, resetDb } from "./db";
import { categorize } from "./categories";

function daysAgo(n: number): string {
  const d = new Date();
  d.setDate(d.getDate() - n);
  return d.toISOString().slice(0, 10);
}

export function seedDatabase(fresh = false) {
  if (fresh) resetDb();
  const db = getDb();

  const existing = db.prepare("SELECT COUNT(*) as c FROM accounts").get() as { c: number };
  if (existing.c > 0 && !fresh) {
    return { seeded: false, message: "Database already has accounts" };
  }

  if (fresh) {
    // resetDb closed the connection; reopen
  }

  const checkingId = uuid();
  const savingsId = uuid();
  const creditId = uuid();
  const brokerageId = uuid();
  const iraId = uuid();

  const insertAccount = db.prepare(`
    INSERT INTO accounts (
      id, name, official_name, type, subtype, mask, balance_current, balance_available,
      currency, institution_name, is_manual, include_in_net_worth
    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'USD', ?, 1, 1)
  `);

  const tx = db.transaction(() => {
    insertAccount.run(
      checkingId,
      "Everyday Checking",
      "Chase Total Checking",
      "checking",
      "checking",
      "4821",
      4287.42,
      4287.42,
      "Chase"
    );
    insertAccount.run(
      savingsId,
      "Emergency Fund",
      "Ally Savings",
      "savings",
      "savings",
      "9033",
      18500.0,
      18500.0,
      "Ally"
    );
    insertAccount.run(
      creditId,
      "Sapphire Preferred",
      "Chase Sapphire Preferred",
      "credit",
      "credit card",
      "2219",
      -1842.67,
      8157.33,
      "Chase"
    );
    insertAccount.run(
      brokerageId,
      "Brokerage",
      "Fidelity Brokerage",
      "investment",
      "brokerage",
      "7710",
      62480.15,
      null,
      "Fidelity"
    );
    insertAccount.run(
      iraId,
      "Roth IRA",
      "Vanguard Roth IRA",
      "investment",
      "ira",
      "4412",
      41220.88,
      null,
      "Vanguard"
    );

    const samples: Array<{
      account_id: string;
      amount: number;
      date: string;
      name: string;
      merchant?: string;
    }> = [
      { account_id: checkingId, amount: -3200, date: daysAgo(28), name: "ACME CORP PAYROLL", merchant: "Payroll" },
      { account_id: checkingId, amount: 1850, date: daysAgo(27), name: "RENT PAYMENT", merchant: "Landlord" },
      { account_id: checkingId, amount: 94.32, date: daysAgo(25), name: "WHOLE FOODS MARKET", merchant: "Whole Foods" },
      { account_id: checkingId, amount: 15.99, date: daysAgo(24), name: "NETFLIX.COM", merchant: "Netflix" },
      { account_id: checkingId, amount: 10.99, date: daysAgo(24), name: "SPOTIFY USA", merchant: "Spotify" },
      { account_id: creditId, amount: 67.4, date: daysAgo(22), name: "UBER TRIP", merchant: "Uber" },
      { account_id: creditId, amount: 42.18, date: daysAgo(21), name: "CHIPOTLE 1842", merchant: "Chipotle" },
      { account_id: creditId, amount: 128.9, date: daysAgo(20), name: "AMAZON.COM", merchant: "Amazon" },
      { account_id: checkingId, amount: 89.0, date: daysAgo(19), name: "PG&E UTILITY", merchant: "PG&E" },
      { account_id: checkingId, amount: 79.99, date: daysAgo(18), name: "COMCAST CABLE", merchant: "Comcast" },
      { account_id: creditId, amount: 214.55, date: daysAgo(16), name: "NORDSTROM", merchant: "Nordstrom" },
      { account_id: checkingId, amount: 56.2, date: daysAgo(15), name: "SHELL OIL", merchant: "Shell" },
      { account_id: creditId, amount: 8.75, date: daysAgo(14), name: "STARBUCKS STORE", merchant: "Starbucks" },
      { account_id: checkingId, amount: 500, date: daysAgo(13), name: "TRANSFER TO ALLY", merchant: "Transfer" },
      { account_id: savingsId, amount: -500, date: daysAgo(13), name: "TRANSFER FROM CHASE", merchant: "Transfer" },
      { account_id: creditId, amount: 35.0, date: daysAgo(12), name: "CVS PHARMACY", merchant: "CVS" },
      { account_id: checkingId, amount: 22.0, date: daysAgo(11), name: "GITHUB INC", merchant: "GitHub" },
      { account_id: creditId, amount: 156.4, date: daysAgo(10), name: "TARGET T-8841", merchant: "Target" },
      { account_id: checkingId, amount: 48.6, date: daysAgo(9), name: "TRADER JOE'S", merchant: "Trader Joe's" },
      { account_id: creditId, amount: 62.0, date: daysAgo(8), name: "DOORDASH", merchant: "DoorDash" },
      { account_id: checkingId, amount: 20.0, date: daysAgo(7), name: "VENMO PAYMENT", merchant: "Venmo" },
      { account_id: creditId, amount: 119.0, date: daysAgo(6), name: "BEST BUY", merchant: "Best Buy" },
      { account_id: checkingId, amount: 11.99, date: daysAgo(5), name: "APPLE.COM/BILL", merchant: "Apple" },
      { account_id: creditId, amount: 28.4, date: daysAgo(4), name: "CHIPOTLE ONLINE", merchant: "Chipotle" },
      { account_id: checkingId, amount: 72.15, date: daysAgo(3), name: "SAFEWAY #122", merchant: "Safeway" },
      { account_id: creditId, amount: 45.0, date: daysAgo(2), name: "UBER EATS", merchant: "Uber Eats" },
      { account_id: checkingId, amount: -3200, date: daysAgo(1), name: "ACME CORP PAYROLL", merchant: "Payroll" },
      { account_id: brokerageId, amount: 200, date: daysAgo(20), name: "BUY VTI", merchant: "Vanguard" },
      { account_id: iraId, amount: 300, date: daysAgo(15), name: "ROTH CONTRIBUTION", merchant: "Vanguard" },
    ];

    const insertTxn = db.prepare(`
      INSERT INTO transactions (
        id, account_id, amount, date, name, merchant_name, category, is_subscription, pending
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0)
    `);

    for (const s of samples) {
      const { category, isSubscription } = categorize(s.name, s.merchant);
      insertTxn.run(
        uuid(),
        s.account_id,
        s.amount,
        s.date,
        s.name,
        s.merchant ?? null,
        category,
        isSubscription ? 1 : 0
      );
    }

    const budgets = [
      ["Groceries", 450],
      ["Dining", 250],
      ["Shopping", 300],
      ["Transport", 180],
      ["Entertainment", 100],
      ["Subscriptions", 80],
    ] as const;

    const insertBudget = db.prepare(
      `INSERT INTO budgets (id, category, monthly_limit) VALUES (?, ?, ?)`
    );
    for (const [category, limit] of budgets) {
      insertBudget.run(uuid(), category, limit);
    }
  });

  tx();

  return { seeded: true, message: "Demo household loaded" };
}
