import { Configuration, PlaidApi, PlaidEnvironments, Products, CountryCode } from "plaid";

/**
 * Anchored only requests read-only Plaid products.
 * These can pull balances, transactions, holdings, and liabilities —
 * they cannot initiate payments, ACH, or transfers.
 */
export const PLAID_READ_ONLY_PRODUCTS: Products[] = [
  Products.Transactions,
  Products.Investments,
];

/** Optional read-only products; skipped by institutions that do not support them. */
export const PLAID_READ_ONLY_OPTIONAL_PRODUCTS: Products[] = [
  Products.Liabilities,
  Products.RecurringTransactions,
];

/** Money-moving / write-capable products — never requested by Anchored. */
export const PLAID_WRITE_PRODUCTS: Products[] = [
  Products.Transfer,
  Products.PaymentInitiation,
  Products.ProcessorPayments,
  Products.InvestmentsAuth,
  Products.Auth, // routing/account numbers used to set up payments — excluded
];

export const PLAID_ACCESS_MODE = "read_only" as const;

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

/** @deprecated Use PLAID_READ_ONLY_PRODUCTS — kept as an alias for existing imports. */
export const PLAID_PRODUCTS = PLAID_READ_ONLY_PRODUCTS;

export const PLAID_COUNTRY_CODES: CountryCode[] = [CountryCode.Us];

const WRITE_SET = new Set<string>(PLAID_WRITE_PRODUCTS.map(String));
const READ_SET = new Set<string>([
  ...PLAID_READ_ONLY_PRODUCTS.map(String),
  ...PLAID_READ_ONLY_OPTIONAL_PRODUCTS.map(String),
]);

/** Throws if any product is not on the read-only allowlist (or is write-capable). */
export function assertReadOnlyProducts(products: Products[]): Products[] {
  for (const product of products) {
    const key = String(product);
    if (WRITE_SET.has(key)) {
      throw new Error(
        `Refusing Plaid product "${key}": Anchored only uses read-only financial access.`
      );
    }
    if (!READ_SET.has(key)) {
      throw new Error(
        `Refusing Plaid product "${key}": not on Anchored's read-only allowlist.`
      );
    }
  }
  return products;
}

/** Payload for linkTokenCreate — products are validated as read-only. */
export function getReadOnlyLinkTokenProducts(): {
  products: Products[];
  optional_products: Products[];
  access_mode: typeof PLAID_ACCESS_MODE;
} {
  return {
    products: assertReadOnlyProducts([...PLAID_READ_ONLY_PRODUCTS]),
    optional_products: assertReadOnlyProducts([...PLAID_READ_ONLY_OPTIONAL_PRODUCTS]),
    access_mode: PLAID_ACCESS_MODE,
  };
}

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
  if (s.includes("checking") || (t === "depository" && s === "checking")) return "checking";
  if (s.includes("saving")) return "savings";
  if (t === "depository") return "checking";
  return "other";
}
