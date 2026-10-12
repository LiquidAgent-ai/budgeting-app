export const CATEGORIES = [
  "Housing",
  "Groceries",
  "Dining",
  "Transport",
  "Travel",
  "Shopping",
  "Entertainment",
  "Health",
  "Utilities",
  "Subscriptions",
  "Income",
  "Transfer",
  "Investments",
  "Fees",
  "Personal",
  "Other",
] as const;

export type Category = (typeof CATEGORIES)[number];

const MERCHANT_MAP: Array<{ match: RegExp; category: Category; subscription?: boolean }> = [
  { match: /netflix|spotify|hulu|disney\+|apple\.com\/bill|youtube premium|adobe|notion|github|openai|cursor/i, category: "Subscriptions", subscription: true },
  { match: /whole foods|trader joe|safeway|kroger|costco|aldi|grocery|wegmans/i, category: "Groceries" },
  { match: /starbucks|chipotle|mcdonald|uber eats|doordash|grubhub|restaurant|cafe|coffee/i, category: "Dining" },
  { match: /uber|lyft|shell|chevron|exxon|gas station|parking|transit/i, category: "Transport" },
  { match: /united|delta|american air|airbnb|marriott|hilton|hotel|expedia/i, category: "Travel" },
  { match: /amazon|target|walmart|best buy|ikea|nordstrom/i, category: "Shopping" },
  { match: /rent|mortgage|landlord|hoa/i, category: "Housing" },
  { match: /pg&e|comcast|verizon|at&t|t-mobile|electric|water bill|internet/i, category: "Utilities", subscription: true },
  { match: /cvs|walgreens|pharmacy|dental|hospital|clinic/i, category: "Health" },
  { match: /payroll|direct deposit|salary|paycheck|employer/i, category: "Income" },
  { match: /transfer|venmo|zelle|paypal|wire/i, category: "Transfer" },
  { match: /vanguard|fidelity|schwab|robinhood|coinbase|brokerage/i, category: "Investments" },
];

export function categorize(name: string, merchant?: string | null): {
  category: Category;
  isSubscription: boolean;
} {
  const haystack = `${name} ${merchant ?? ""}`;
  for (const rule of MERCHANT_MAP) {
    if (rule.match.test(haystack)) {
      return {
        category: rule.category,
        isSubscription: Boolean(rule.subscription),
      };
    }
  }
  return { category: "Other", isSubscription: false };
}
