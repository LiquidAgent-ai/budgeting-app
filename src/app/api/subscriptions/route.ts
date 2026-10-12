import { NextResponse } from "next/server";
import { getDb } from "@/lib/db";
import { monthStart } from "@/lib/finance";

export const runtime = "nodejs";

export async function GET() {
  const db = getDb();
  const start = monthStart();

  const rows = db
    .prepare(
      `SELECT
         COALESCE(merchant_name, name) as label,
         category,
         COUNT(*) as occurrences,
         AVG(amount) as avg_amount,
         SUM(CASE WHEN date >= ? THEN amount ELSE 0 END) as month_amount,
         MAX(date) as last_seen
       FROM transactions
       WHERE is_subscription = 1 AND amount > 0
       GROUP BY COALESCE(merchant_name, name), category
       ORDER BY month_amount DESC, avg_amount DESC`
    )
    .all(start) as Array<{
    label: string;
    category: string;
    occurrences: number;
    avg_amount: number;
    month_amount: number;
    last_seen: string;
  }>;

  const monthTotal = rows.reduce((s, r) => s + (r.month_amount || 0), 0);

  return NextResponse.json({ subscriptions: rows, monthTotal });
}
