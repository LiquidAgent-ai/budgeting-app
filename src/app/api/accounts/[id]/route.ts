import { NextRequest, NextResponse } from "next/server";
import { z } from "zod";
import { getDb } from "@/lib/db";
import type { Account } from "@/lib/types";

export const runtime = "nodejs";

const patchSchema = z.object({
  name: z.string().min(1).max(120).optional(),
  balance_current: z.number().optional(),
  include_in_net_worth: z.boolean().optional(),
  institution_name: z.string().nullable().optional(),
});

export async function PATCH(
  req: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  const { id } = await params;
  const parsed = patchSchema.safeParse(await req.json());
  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.flatten() }, { status: 400 });
  }

  const db = getDb();
  const existing = db.prepare("SELECT * FROM accounts WHERE id = ?").get(id) as Account | undefined;
  if (!existing) return NextResponse.json({ error: "Not found" }, { status: 404 });

  const data = parsed.data;
  db.prepare(
    `UPDATE accounts SET
      name = COALESCE(?, name),
      balance_current = COALESCE(?, balance_current),
      include_in_net_worth = COALESCE(?, include_in_net_worth),
      institution_name = COALESCE(?, institution_name),
      updated_at = datetime('now')
     WHERE id = ?`
  ).run(
    data.name ?? null,
    data.balance_current ?? null,
    data.include_in_net_worth === undefined ? null : data.include_in_net_worth ? 1 : 0,
    data.institution_name === undefined ? null : data.institution_name,
    id
  );

  const account = db.prepare("SELECT * FROM accounts WHERE id = ?").get(id) as Account;
  return NextResponse.json({ account });
}

export async function DELETE(
  _req: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  const { id } = await params;
  const db = getDb();
  db.prepare("DELETE FROM accounts WHERE id = ?").run(id);
  return NextResponse.json({ ok: true });
}
