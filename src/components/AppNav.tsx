"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

const links = [
  { href: "/dashboard", label: "Overview" },
  { href: "/accounts", label: "Accounts" },
  { href: "/transactions", label: "Activity" },
  { href: "/budgets", label: "Budgets" },
  { href: "/subscriptions", label: "Subscriptions" },
  { href: "/net-worth", label: "Net worth" },
];

export function AppNav() {
  const pathname = usePathname();

  return (
    <header className="sticky top-0 z-40 border-b border-line bg-paper/80 backdrop-blur-md">
      <div className="shell flex items-center justify-between gap-4 py-3">
        <Link href="/" className="brand text-2xl tracking-tight text-ink">
          Anchored
        </Link>
        <nav className="hidden items-center gap-1 md:flex">
          {links.map((l) => {
            const active = pathname === l.href;
            return (
              <Link
                key={l.href}
                href={l.href}
                className={`rounded-full px-3 py-1.5 text-sm transition ${
                  active
                    ? "bg-ink text-foam"
                    : "text-ink-soft/80 hover:bg-foam/70 hover:text-ink"
                }`}
              >
                {l.label}
              </Link>
            );
          })}
        </nav>
        <Link href="/accounts" className="btn btn-sea text-sm md:hidden">
          Accounts
        </Link>
      </div>
      <nav className="shell flex gap-2 overflow-x-auto pb-3 md:hidden">
        {links.map((l) => {
          const active = pathname === l.href;
          return (
            <Link
              key={l.href}
              href={l.href}
              className={`whitespace-nowrap rounded-full px-3 py-1 text-sm ${
                active ? "bg-ink text-foam" : "bg-foam/60 text-ink-soft"
              }`}
            >
              {l.label}
            </Link>
          );
        })}
      </nav>
    </header>
  );
}
