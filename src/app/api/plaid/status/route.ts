import { NextResponse } from "next/server";
import { isPlaidConfigured } from "@/lib/plaid";
import { getDb } from "@/lib/db";

export const runtime = "nodejs";

export async function GET() {
  const db = getDb();
  const items = db
    .prepare("SELECT id, institution_name, item_id, created_at FROM plaid_items ORDER BY created_at DESC")
    .all();

  return NextResponse.json({
    configured: isPlaidConfigured(),
    env: process.env.PLAID_ENV || "sandbox",
    items,
  });
}
