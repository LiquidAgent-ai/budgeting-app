import Link from "next/link";

export default function LandingPage() {
  return (
    <main className="noise min-h-screen overflow-hidden">
      <header className="shell flex items-center justify-between py-6">
        <span className="brand text-2xl text-ink">Anchored</span>
        <Link href="/dashboard" className="btn btn-ghost text-sm">
          Open app
        </Link>
      </header>

      <section className="shell relative grid min-h-[calc(100vh-5.5rem)] items-center gap-10 pb-16 pt-4 lg:grid-cols-[1.05fr_0.95fr]">
        <div className="relative z-10 max-w-xl">
          <p className="animate-rise brand text-5xl leading-[0.95] text-ink sm:text-6xl md:text-7xl">
            Anchored
          </p>
          <h1 className="animate-rise-delay-1 mt-5 text-2xl font-medium leading-snug text-ink-soft sm:text-3xl">
            Your full money picture — hosted on your machine.
          </h1>
          <p className="animate-rise-delay-2 mt-4 max-w-md text-base leading-relaxed text-ink-soft/80">
            Link checking, credit cards, and investments. Watch spending, catch subscriptions, and
            see net worth without shipping your life to someone else&apos;s cloud.
          </p>
          <div className="animate-rise-delay-2 mt-8 flex flex-wrap gap-3">
            <Link href="/dashboard" className="btn btn-primary">
              Launch dashboard
            </Link>
            <Link href="/accounts" className="btn btn-ghost">
              Link accounts
            </Link>
          </div>
        </div>

        <div className="animate-drift relative hidden min-h-[420px] lg:block" aria-hidden>
          <div className="absolute inset-0 rounded-[2.5rem] bg-gradient-to-br from-ink via-ink-soft to-sea opacity-95" />
          <svg
            className="absolute inset-0 h-full w-full p-10 text-foam/80"
            viewBox="0 0 400 360"
            fill="none"
          >
            <path
              className="wave-path"
              d="M20 220 C 80 160, 120 280, 180 210 S 280 140, 380 190"
              stroke="currentColor"
              strokeWidth="2.5"
              fill="none"
            />
            <path
              d="M20 260 C 90 210, 140 300, 200 240 S 300 170, 380 230"
              stroke="currentColor"
              strokeWidth="1.5"
              opacity="0.45"
              fill="none"
            />
            <circle cx="86" cy="188" r="5" fill="#d7efe9" />
            <circle cx="210" cy="205" r="5" fill="#d7efe9" />
            <circle cx="318" cy="168" r="5" fill="#d7efe9" />
            <text x="40" y="70" fill="#d7efe9" fontSize="14" opacity="0.8">
              checking · credit · brokerage
            </text>
            <text x="40" y="110" fill="#d7efe9" fontSize="42" fontFamily="Georgia, serif">
              local first
            </text>
          </svg>
        </div>
      </section>

      <section className="shell border-t border-line py-16">
        <h2 className="max-w-lg text-3xl text-ink">One place for cash, cards, and investments.</h2>
        <p className="mt-3 max-w-xl text-ink-soft/80">
          Anchored mirrors the Rocket Money workflow — account linking, budgets, subscription
          spotting, and net worth — with SQLite on disk and optional Plaid for live bank connections.
        </p>
        <div className="mt-10 grid gap-8 md:grid-cols-3">
          {[
            {
              title: "Link what you own",
              body: "Plaid sandbox or production keys for checking, credit, and investment accounts. Manual accounts when you want zero third parties.",
            },
            {
              title: "See the leaks",
              body: "Auto-categorized activity, monthly budgets, and recurring subscription detection so quiet charges do not stay quiet.",
            },
            {
              title: "Know your net worth",
              body: "Cash plus investments minus credit and loans, updated when you sync — a single number that actually means something.",
            },
          ].map((item) => (
            <div key={item.title}>
              <h3 className="text-xl text-ink">{item.title}</h3>
              <p className="mt-2 text-sm leading-relaxed text-ink-soft/75">{item.body}</p>
            </div>
          ))}
        </div>
      </section>
    </main>
  );
}
