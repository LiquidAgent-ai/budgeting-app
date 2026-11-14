use once_cell::sync::Lazy;
use regex::Regex;

#[allow(dead_code)]
pub const CATEGORIES: &[&str] = &[
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
];

struct Rule {
    re: Regex,
    category: &'static str,
    subscription: bool,
}

static RULES: Lazy<Vec<Rule>> = Lazy::new(|| {
    let raw: &[(&str, &str, bool)] = &[
        (
            r"(?i)netflix|spotify|hulu|disney\+|apple\.com/bill|youtube premium|adobe|notion|github|openai|cursor",
            "Subscriptions",
            true,
        ),
        (
            r"(?i)whole foods|trader joe|safeway|kroger|costco|aldi|grocery|wegmans",
            "Groceries",
            false,
        ),
        (
            r"(?i)starbucks|chipotle|mcdonald|uber eats|doordash|grubhub|restaurant|cafe|coffee",
            "Dining",
            false,
        ),
        (
            r"(?i)uber|lyft|shell|chevron|exxon|gas station|parking|transit",
            "Transport",
            false,
        ),
        (
            r"(?i)united|delta|american air|airbnb|marriott|hilton|hotel|expedia",
            "Travel",
            false,
        ),
        (
            r"(?i)amazon|target|walmart|best buy|ikea|nordstrom",
            "Shopping",
            false,
        ),
        (r"(?i)rent|mortgage|landlord|hoa", "Housing", false),
        (
            r"(?i)pg&e|comcast|verizon|at&t|t-mobile|electric|water bill|internet",
            "Utilities",
            true,
        ),
        (
            r"(?i)cvs|walgreens|pharmacy|dental|hospital|clinic",
            "Health",
            false,
        ),
        (
            r"(?i)payroll|direct deposit|salary|paycheck|employer",
            "Income",
            false,
        ),
        (r"(?i)transfer|venmo|zelle|paypal|wire", "Transfer", false),
        (
            r"(?i)vanguard|fidelity|schwab|robinhood|coinbase|brokerage",
            "Investments",
            false,
        ),
    ];
    raw.iter()
        .map(|(pat, cat, sub)| Rule {
            re: Regex::new(pat).expect("category regex"),
            category: cat,
            subscription: *sub,
        })
        .collect()
});

pub fn categorize(name: &str, merchant: Option<&str>) -> (&'static str, bool) {
    let haystack = format!("{} {}", name, merchant.unwrap_or(""));
    for rule in RULES.iter() {
        if rule.re.is_match(&haystack) {
            return (rule.category, rule.subscription);
        }
    }
    ("Other", false)
}
