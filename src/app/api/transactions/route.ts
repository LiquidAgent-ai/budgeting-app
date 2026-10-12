import { NextRequest, NextResponse } from "next/server";
import { v4 as uuid } from "uuid";
import { z } from "zod";
import { getDb } from "@/lib/db";
import { categorize } from "@/lib/categories";
import type { Transaction } from "@/lib/types";

export const runtime = "nodejs";

const createSchema = z.object({
  account_id: z.string().min(1),
  amount: z.number(),
  date: z.string().min(8),
  name: z.string().min(1),
  merchant_name: z.string().optional().nullable(),
  category: z.string().optional(),
  is_subscription: z.boolean().optional(),
});

export async function GET(req: NextRequest) {
  const sp = req.nextUrl.searchParams;
  const category = sp.get("category");
  const accountId = sp.get("account_id");
  const subscriptions = sp.get("subscriptions");
  const limit = Math.min(Number(sp.get("limit") || 100), 500);

  const clauses: string[] = [];
  const params: Array<string | number> = [];

  if (category) {
    clauses.push("t.category = ?");
    params.push(category);
  }
  if (accountId) {
    clauses.push("t.account_id = ?");
    params.push(accountId);
  }
  if (subscriptions === "1") {
    clauses.push("t.is_subscription = 1");
  }

  const where = clauses.length ? `WHERE ${clauses.join(" AND ")}` : "";
  const db = getDb();
  const rows = db
    .prepare(
      `SELECT t.*, a.name as account_name
       FROM transactions t
       JOIN accounts a ON a.id = t.account_id
       ${where}
       ORDER BY t.date DESC, t.created_at DESC
       LIMIT ?`
    )
    .all(...params, limit) as (Transaction & { account_name: string })[];

  return NextResponse.json({ transactions: rows });
}

export async function POST(req: NextRequest) {
  const parsed = createSchema.safeParse(await req.json());
  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.flatten() }, { status: 400 });
  }

  const data = parsed.data;
  const auto = categorize(data.name, data.merchant_name);
  const id = uuid();
  const db = getDb();

  db.prepare(
    `INSERT INTO transactions (
      id, account_id, amount, date, name, merchant_name, category, is_subscription
    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)`
  ).run(
    id,
    data.account_id,
    data.amount,
    data.date,
    data.name,
    data.merchant_name ?? null,
    data.category ?? auto.category,
    data.is_subscription === undefined ? (auto.isSubscription ? 1 : 0) : data.is_subscription ? 1 : 0
  );

  const transaction = db.prepare("SELECT * FROM transactions WHERE id = ?").get(id);
  return NextResponse.json({ transaction }, { status: 201 });
}
