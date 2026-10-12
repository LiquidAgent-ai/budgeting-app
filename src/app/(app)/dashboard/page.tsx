"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { Money } from "@/components/Money";
import { SpendChart } from "@/components/SpendChart";
import type { DashboardSummary } from "@/lib/types";
import { pct, titleCase } from "@/lib/format";

export default function DashboardPage() {
  const [data, setData] = useState<DashboardSummary | null>(null);

  useEffect(() => {
    fetch("/api/dashboard")
      .then((r) => r.json())
      .then(setData);
  }, []);

  if (!data) {
    return <p className="text-ink-soft">Loading your books…</p>;
  }

  const budgetPct = pct(data.budgetSpent, data.budgetTotal);

  return (
    <div className="space-y-8">
      <div className="animate-rise">
        <p className="text-sm uppercase tracking-[0.18em] text-ink-soft/60">Overview</p>
        <h1 className="mt-1 text-4xl text-ink">How money moved this month</h1>
      </div>

      <div className="animate-rise-delay-1 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        {[
          { label: "Net worth", value: data.netWorth },
          { label: "Cash", value: data.cash },
          { label: "Investments", value: data.investments },
          { label: "Credit used", value: data.creditUsed },
        ].map((stat) => (
          <div key={stat.label} className="panel rounded-2xl p-5">
            <p className="text-sm text-ink-soft/70">{stat.label}</p>
            <p className="mt-2 text-2xl">
              <Money value={stat.value} />
            </p>
          </div>
        ))}
      </div>

      <div className="grid gap-6 lg:grid-cols-[1.2fr_0.8fr]">
        <div className="panel animate-rise-delay-2 rounded-2xl p-5">
          <div className="mb-4 flex items-end justify-between gap-3">
            <div>
              <h2 className="text-2xl text-ink">Spending by category</h2>
              <p className="text-sm text-ink-soft/70">
                Outflows this month: <Money value={data.monthSpend} />
              </p>
            </div>
            <Link href="/transactions" className="text-sm text-sea underline-offset-2 hover:underline">
              All activity
            </Link>
          </div>
          <SpendChart data={data.spendByCategory} />
        </div>

        <div className="space-y-4">
          <div className="panel rounded-2xl p-5">
            <h2 className="text-xl text-ink">Budgets</h2>
            <p className="mt-1 text-sm text-ink-soft/70">
              <Money value={data.budgetSpent} /> of <Money value={data.budgetTotal} />
            </p>
            <div className="progress mt-4">
              <span style={{ width: `${budgetPct}%` }} />
            </div>
            <Link href="/budgets" className="mt-4 inline-block text-sm text-sea hover:underline">
              Adjust budgets
            </Link>
          </div>
          <div className="panel rounded-2xl p-5">
            <h2 className="text-xl text-ink">Subscriptions</h2>
            <p className="mt-2 text-2xl">
              <Money value={data.subscriptionTotal} />
            </p>
            <p className="text-sm text-ink-soft/70">
              {data.subscriptionCount} recurring charge
              {data.subscriptionCount === 1 ? "" : "s"} this month
            </p>
            <Link href="/subscriptions" className="mt-4 inline-block text-sm text-sea hover:underline">
              Review subscriptions
            </Link>
          </div>
        </div>
      </div>

      <div className="panel rounded-2xl p-5">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-2xl text-ink">Recent activity</h2>
          <p className="text-sm text-ink-soft/70">
            Income <Money value={data.monthIncome} accent="good" />
          </p>
        </div>
        <ul className="divide-y divide-line">
          {data.recentTransactions.map((t) => (
            <li key={t.id} className="flex items-center justify-between gap-4 py-3 text-sm">
              <div className="min-w-0">
                <p className="truncate font-medium text-ink">{t.merchant_name || t.name}</p>
                <p className="text-ink-soft/60">
                  {t.date} · {titleCase(t.category)} · {t.account_name}
                </p>
              </div>
              <Money
                value={t.category === "Income" ? -Math.abs(t.amount) : t.amount}
                accent={t.amount < 0 || t.category === "Income" ? "good" : undefined}
                className="shrink-0"
              />
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
