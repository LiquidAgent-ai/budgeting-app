import { NextResponse } from "next/server";
import { v4 as uuid } from "uuid";
import { getDb } from "@/lib/db";
import { categorize } from "@/lib/categories";
import { getPlaidClient, isPlaidConfigured, mapPlaidAccountType } from "@/lib/plaid";

export const runtime = "nodejs";

export async function POST() {
  if (!isPlaidConfigured()) {
    return NextResponse.json({ error: "Plaid not configured" }, { status: 400 });
  }

  const db = getDb();
  const items = db
    .prepare("SELECT * FROM plaid_items")
    .all() as Array<{
    id: string;
    access_token: string;
    cursor: string | null;
  }>;

  if (items.length === 0) {
    return NextResponse.json({ synced: 0, message: "No linked institutions" });
  }

  const client = getPlaidClient();
  let added = 0;

  for (const item of items) {
    const accountsRes = await client.accountsGet({ access_token: item.access_token });
    for (const a of accountsRes.data.accounts) {
      const type = mapPlaidAccountType(a.type, a.subtype);
      let current = a.balances.current ?? 0;
      if ((type === "credit" || type === "loan") && current > 0) current = -Math.abs(current);
      db.prepare(
        `UPDATE accounts SET balance_current = ?, balance_available = ?, updated_at = datetime('now')
         WHERE plaid_account_id = ? AND plaid_item_id = ?`
      ).run(current, a.balances.available ?? null, a.account_id, item.id);
    }

    const accountMap = new Map(
      (
        db
          .prepare("SELECT id, plaid_account_id FROM accounts WHERE plaid_item_id = ?")
          .all(item.id) as { id: string; plaid_account_id: string }[]
      ).map((a) => [a.plaid_account_id, a.id])
    );

    let cursor = item.cursor || undefined;
    let hasMore = true;

    while (hasMore) {
      const res = await client.transactionsSync({
        access_token: item.access_token,
        cursor,
      });

      const insert = db.prepare(
        `INSERT OR IGNORE INTO transactions (
          id, account_id, plaid_transaction_id, amount, date, name, merchant_name,
          category, is_subscription, pending
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`
      );

      for (const t of res.data.added) {
        const accountId = accountMap.get(t.account_id);
        if (!accountId) continue;
        const existing = db
          .prepare("SELECT id FROM transactions WHERE plaid_transaction_id = ?")
          .get(t.transaction_id);
        if (existing) continue;
        const { category, isSubscription } = categorize(t.name, t.merchant_name);
        insert.run(
          uuid(),
          accountId,
          t.transaction_id,
          t.amount,
          t.date,
          t.name,
          t.merchant_name ?? null,
          category,
          isSubscription ? 1 : 0,
          t.pending ? 1 : 0
        );
        added += 1;
      }

      hasMore = res.data.has_more;
      cursor = res.data.next_cursor;
    }

    if (cursor) {
      db.prepare("UPDATE plaid_items SET cursor = ?, updated_at = datetime('now') WHERE id = ?").run(
        cursor,
        item.id
      );
    }

    try {
      const holdings = await client.investmentsHoldingsGet({ access_token: item.access_token });
      for (const a of holdings.data.accounts) {
        db.prepare(
          `UPDATE accounts SET balance_current = ?, updated_at = datetime('now')
           WHERE plaid_account_id = ? AND plaid_item_id = ?`
        ).run(a.balances.current ?? 0, a.account_id, item.id);
      }
    } catch {
      // optional
    }
  }

  return NextResponse.json({ synced: added });
}
