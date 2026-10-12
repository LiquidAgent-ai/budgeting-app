import { NextRequest, NextResponse } from "next/server";
import { v4 as uuid } from "uuid";
import { z } from "zod";
import { getDb } from "@/lib/db";
import type { Account } from "@/lib/types";

export const runtime = "nodejs";

const createSchema = z.object({
  name: z.string().min(1).max(120),
  type: z.enum(["checking", "savings", "credit", "investment", "loan", "other"]),
  balance_current: z.number(),
  institution_name: z.string().optional().nullable(),
  mask: z.string().max(8).optional().nullable(),
  include_in_net_worth: z.boolean().optional(),
});

export async function GET() {
  const db = getDb();
  const accounts = db.prepare("SELECT * FROM accounts ORDER BY type, name").all() as Account[];
  return NextResponse.json({ accounts });
}

export async function POST(req: NextRequest) {
  const parsed = createSchema.safeParse(await req.json());
  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.flatten() }, { status: 400 });
  }

  const data = parsed.data;
  const id = uuid();
  const db = getDb();

  // Credit balances stored as negative liability for net worth math
  let balance = data.balance_current;
  if (data.type === "credit" && balance > 0) balance = -balance;
  if (data.type === "loan" && balance > 0) balance = -Math.abs(balance);

  db.prepare(
    `INSERT INTO accounts (
      id, name, type, balance_current, balance_available, institution_name, mask,
      is_manual, include_in_net_worth
    ) VALUES (?, ?, ?, ?, ?, ?, ?, 1, ?)`
  ).run(
    id,
    data.name,
    data.type,
    balance,
    data.type === "credit" ? null : balance,
    data.institution_name ?? null,
    data.mask ?? null,
    data.include_in_net_worth === false ? 0 : 1
  );

  const account = db.prepare("SELECT * FROM accounts WHERE id = ?").get(id) as Account;
  return NextResponse.json({ account }, { status: 201 });
}
