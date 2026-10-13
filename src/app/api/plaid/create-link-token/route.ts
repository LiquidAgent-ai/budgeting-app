import { NextResponse } from "next/server";
import {
  getPlaidClient,
  getReadOnlyLinkTokenProducts,
  isPlaidConfigured,
  PLAID_ACCESS_MODE,
  PLAID_COUNTRY_CODES,
} from "@/lib/plaid";

export const runtime = "nodejs";

export async function POST() {
  if (!isPlaidConfigured()) {
    return NextResponse.json(
      {
        error: "Plaid not configured",
        hint: "Copy .env.example to .env.local and add free sandbox keys from https://dashboard.plaid.com/",
      },
      { status: 400 }
    );
  }

  try {
    const { products, optional_products, access_mode } = getReadOnlyLinkTokenProducts();
    const client = getPlaidClient();

    // Read-only link: balances, transactions, holdings, liabilities only.
    // Never requests Transfer, Payment Initiation, Auth, or other write products.
    const response = await client.linkTokenCreate({
      user: { client_user_id: "anchored-local-user" },
      client_name: "Anchored (read-only)",
      products,
      optional_products,
      country_codes: PLAID_COUNTRY_CODES,
      language: "en",
    });

    return NextResponse.json({
      link_token: response.data.link_token,
      access_mode,
      products,
      optional_products,
    });
  } catch (err) {
    const message = err instanceof Error ? err.message : "Plaid error";
    return NextResponse.json(
      { error: message, access_mode: PLAID_ACCESS_MODE },
      { status: 500 }
    );
  }
}
