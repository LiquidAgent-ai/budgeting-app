import { NextRequest, NextResponse } from "next/server";
import { v4 as uuid } from "uuid";
import { z } from "zod";
import { getDb } from "@/lib/db";
import { monthStart } from "@/lib/finance";
import type { Budget } from "@/lib/types";

export const runtime = "nodejs";

const upsertSchema = z.object({
  category: z.string().min(1),
  monthly_limit: z.number().positive(),
});

export async function GET() {
  const db = getDb();
  const start = monthStart();
  const budgets = db.prepare("SELECT * FROM budgets ORDER BY category").all() as Budget[];

  const spentRows = db
    .prepare(
      `SELECT category, COALESCE(SUM(amount),0) as spent
       FROM transactions
       WHERE date >= ? AND amount > 0
       GROUP BY category`
    )
    .all(start) as { category: string; spent: number }[];

  const spentMap = Object.fromEntries(spentRows.map((r) => [r.category, r.spent]));

  return NextResponse.json({
    budgets: budgets.map((b) => ({
      ...b,
      spent: spentMap[b.category] ?? 0,
    })),
  });
}

export async function POST(req: NextRequest) {
  const parsed = upsertSchema.safeParse(await req.json());
  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.flatten() }, { status: 400 });
  }

  const db = getDb();
  const existing = db
    .prepare("SELECT id FROM budgets WHERE category = ?")
    .get(parsed.data.category) as { id: string } | undefined;

  if (existing) {
    db.prepare("UPDATE budgets SET monthly_limit = ? WHERE id = ?").run(
      parsed.data.monthly_limit,
      existing.id
    );
    const budget = db.prepare("SELECT * FROM budgets WHERE id = ?").get(existing.id);
    return NextResponse.json({ budget });
  }

  const id = uuid();
  db.prepare("INSERT INTO budgets (id, category, monthly_limit) VALUES (?, ?, ?)").run(
    id,
    parsed.data.category,
    parsed.data.monthly_limit
  );
  const budget = db.prepare("SELECT * FROM budgets WHERE id = ?").get(id);
  return NextResponse.json({ budget }, { status: 201 });
}

export async function DELETE(req: NextRequest) {
  const category = req.nextUrl.searchParams.get("category");
  if (!category) return NextResponse.json({ error: "category required" }, { status: 400 });
  getDb().prepare("DELETE FROM budgets WHERE category = ?").run(category);
  return NextResponse.json({ ok: true });
}
