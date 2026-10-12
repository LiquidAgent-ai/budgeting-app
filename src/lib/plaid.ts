import { Configuration, PlaidApi, PlaidEnvironments, Products, CountryCode } from "plaid";

export function isPlaidConfigured(): boolean {
  return Boolean(
    process.env.PLAID_CLIENT_ID &&
      process.env.PLAID_SECRET &&
      process.env.PLAID_CLIENT_ID !== "your_client_id"
  );
}

export function getPlaidClient(): PlaidApi {
  if (!isPlaidConfigured()) {
    throw new Error("Plaid is not configured. Add PLAID_CLIENT_ID and PLAID_SECRET to .env.local");
  }

  const env = (process.env.PLAID_ENV || "sandbox") as keyof typeof PlaidEnvironments;
  const configuration = new Configuration({
    basePath: PlaidEnvironments[env] ?? PlaidEnvironments.sandbox,
    baseOptions: {
      headers: {
        "PLAID-CLIENT-ID": process.env.PLAID_CLIENT_ID!,
        "PLAID-SECRET": process.env.PLAID_SECRET!,
      },
    },
  });

  return new PlaidApi(configuration);
}

export const PLAID_PRODUCTS: Products[] = [
  Products.Transactions,
  Products.Investments,
];

export const PLAID_COUNTRY_CODES: CountryCode[] = [CountryCode.Us];

/** Map Plaid account type/subtype to our AccountType */
export function mapPlaidAccountType(
  type: string | null | undefined,
  subtype: string | null | undefined
): "checking" | "savings" | "credit" | "investment" | "loan" | "other" {
  const t = (type || "").toLowerCase();
  const s = (subtype || "").toLowerCase();

  if (t === "credit" || s.includes("credit")) return "credit";
  if (t === "investment" || t === "brokerage" || s.includes("401") || s.includes("ira")) {
    return "investment";
  }
  if (t === "loan" || s.includes("mortgage") || s.includes("student")) return "loan";
  if (s.includes("checking") || t === "depository" && s === "checking") return "checking";
  if (s.includes("saving")) return "savings";
  if (t === "depository") return "checking";
  return "other";
}
