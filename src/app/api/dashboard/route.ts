import { NextResponse } from "next/server";
import { getDashboardSummary } from "@/lib/finance";
import { seedDatabase } from "@/lib/seed";
import { getDb } from "@/lib/db";

export const runtime = "nodejs";

export async function GET() {
  const db = getDb();
  const count = db.prepare("SELECT COUNT(*) as c FROM accounts").get() as { c: number };
  if (count.c === 0) {
    seedDatabase(false);
  }
  return NextResponse.json(getDashboardSummary());
}
