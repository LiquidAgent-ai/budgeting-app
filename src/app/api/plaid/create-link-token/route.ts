import { NextResponse } from "next/server";
import { getPlaidClient, isPlaidConfigured, PLAID_COUNTRY_CODES, PLAID_PRODUCTS } from "@/lib/plaid";

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
    const client = getPlaidClient();
    const response = await client.linkTokenCreate({
      user: { client_user_id: "anchored-local-user" },
      client_name: "Anchored",
      products: PLAID_PRODUCTS,
      country_codes: PLAID_COUNTRY_CODES,
      language: "en",
    });

    return NextResponse.json({ link_token: response.data.link_token });
  } catch (err) {
    const message = err instanceof Error ? err.message : "Plaid error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
