import { NextResponse } from "next/server";
import { getDb } from "@/lib/db";
import { computeNetWorth } from "@/lib/finance";
import type { Account } from "@/lib/types";

export const runtime = "nodejs";

export async function GET() {
  const db = getDb();
  const accounts = db.prepare("SELECT * FROM accounts ORDER BY type, name").all() as Account[];
  const summary = computeNetWorth(accounts);

  const byType = accounts.reduce<Record<string, number>>((acc, a) => {
    if (!a.include_in_net_worth) return acc;
    const key = a.type;
    const value =
      a.type === "credit" || a.type === "loan"
        ? -Math.abs(a.balance_current)
        : a.balance_current;
    acc[key] = (acc[key] || 0) + value;
    return acc;
  }, {});

  return NextResponse.json({
    ...summary,
    byType,
    accounts,
  });
}
