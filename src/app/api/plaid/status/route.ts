import { NextResponse } from "next/server";
import {
  isPlaidConfigured,
  PLAID_ACCESS_MODE,
  PLAID_READ_ONLY_OPTIONAL_PRODUCTS,
  PLAID_READ_ONLY_PRODUCTS,
  PLAID_WRITE_PRODUCTS,
} from "@/lib/plaid";
import { getDb } from "@/lib/db";

export const runtime = "nodejs";

export async function GET() {
  const db = getDb();
  const items = db
    .prepare(
      `SELECT id, institution_name, item_id, access_mode, created_at
       FROM plaid_items
       ORDER BY created_at DESC`
    )
    .all();

  return NextResponse.json({
    configured: isPlaidConfigured(),
    env: process.env.PLAID_ENV || "sandbox",
    access_mode: PLAID_ACCESS_MODE,
    read_only: true,
    products: PLAID_READ_ONLY_PRODUCTS,
    optional_products: PLAID_READ_ONLY_OPTIONAL_PRODUCTS,
    excluded_write_products: PLAID_WRITE_PRODUCTS,
    items,
  });
}
