import { NextRequest, NextResponse } from "next/server";
import { v4 as uuid } from "uuid";
import { getDb } from "@/lib/db";
import {
  getPlaidClient,
  isPlaidConfigured,
  mapPlaidAccountType,
  PLAID_ACCESS_MODE,
} from "@/lib/plaid";

export const runtime = "nodejs";

export async function POST(req: NextRequest) {
  if (!isPlaidConfigured()) {
    return NextResponse.json({ error: "Plaid not configured" }, { status: 400 });
  }

  const body = await req.json();
  const publicToken = body?.public_token as string | undefined;
  if (!publicToken) {
    return NextResponse.json({ error: "public_token required" }, { status: 400 });
  }

  try {
    const client = getPlaidClient();
    const exchange = await client.itemPublicTokenExchange({ public_token: publicToken });
    const accessToken = exchange.data.access_token;
    const itemId = exchange.data.item_id;

    const accountsRes = await client.accountsGet({ access_token: accessToken });
    const institutionName =
      body?.institution?.name ||
      accountsRes.data.item.institution_id ||
      "Linked institution";

    const db = getDb();
    const plaidItemRowId = uuid();

    const insert = db.transaction(() => {
      db.prepare(
        `INSERT INTO plaid_items (
          id, item_id, access_token, institution_id, institution_name, access_mode
        ) VALUES (?, ?, ?, ?, ?, ?)`
      ).run(
        plaidItemRowId,
        itemId,
        accessToken,
        accountsRes.data.item.institution_id ?? null,
        institutionName,
        PLAID_ACCESS_MODE
      );

      const insertAccount = db.prepare(
        `INSERT INTO accounts (
          id, name, official_name, type, subtype, mask, balance_current, balance_available,
          currency, institution_name, plaid_account_id, plaid_item_id, is_manual, include_in_net_worth
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, 1)`
      );

      for (const a of accountsRes.data.accounts) {
        const type = mapPlaidAccountType(a.type, a.subtype);
        let current = a.balances.current ?? 0;
        // Normalize credit/loan as liabilities (negative)
        if ((type === "credit" || type === "loan") && current > 0) {
          current = -Math.abs(current);
        }

        insertAccount.run(
          uuid(),
          a.name,
          a.official_name ?? null,
          type,
          a.subtype ?? null,
          a.mask ?? null,
          current,
          a.balances.available ?? null,
          a.balances.iso_currency_code || "USD",
          institutionName,
          a.account_id,
          plaidItemRowId
        );
      }
    });

    insert();

    // Best-effort transaction + investment sync
    try {
      await syncTransactions(accessToken, plaidItemRowId);
    } catch {
      // sandbox may not always have transactions immediately
    }

    try {
      await syncInvestments(accessToken, plaidItemRowId);
    } catch {
      // investments product may not be on every item
    }

    return NextResponse.json({
      ok: true,
      item_id: itemId,
      accounts: accountsRes.data.accounts.length,
      access_mode: PLAID_ACCESS_MODE,
      read_only: true,
    });
  } catch (err) {
    const message = err instanceof Error ? err.message : "Exchange failed";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

async function syncTransactions(accessToken: string, plaidItemRowId: string) {
  const { getPlaidClient } = await import("@/lib/plaid");
  const { categorize } = await import("@/lib/categories");
  const client = getPlaidClient();
  const db = getDb();

  let cursor: string | undefined = undefined;
  let hasMore = true;

  const accountMap = new Map(
    (
      db
        .prepare("SELECT id, plaid_account_id FROM accounts WHERE plaid_item_id = ?")
        .all(plaidItemRowId) as { id: string; plaid_account_id: string }[]
    ).map((a) => [a.plaid_account_id, a.id])
  );

  while (hasMore) {
    const res = await client.transactionsSync({
      access_token: accessToken,
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
      const { category, isSubscription } = categorize(t.name, t.merchant_name);
      // Plaid: positive amount = money leaving account
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
    }

    hasMore = res.data.has_more;
    cursor = res.data.next_cursor;
  }

  if (cursor) {
    db.prepare("UPDATE plaid_items SET cursor = ?, updated_at = datetime('now') WHERE id = ?").run(
      cursor,
      plaidItemRowId
    );
  }
}

async function syncInvestments(accessToken: string, plaidItemRowId: string) {
  const client = getPlaidClient();
  const db = getDb();
  const holdings = await client.investmentsHoldingsGet({ access_token: accessToken });

  for (const account of holdings.data.accounts) {
    const type = mapPlaidAccountType(account.type, account.subtype);
    if (type !== "investment") continue;
    let current = account.balances.current ?? 0;
    db.prepare(
      `UPDATE accounts SET balance_current = ?, updated_at = datetime('now')
       WHERE plaid_account_id = ? AND plaid_item_id = ?`
    ).run(current, account.account_id, plaidItemRowId);
  }
}
