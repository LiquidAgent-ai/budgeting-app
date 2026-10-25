"use client";

import { useEffect, useState } from "react";
import {
  ResponsiveContainer,
  PieChart,
  Pie,
  Cell,
  Tooltip,
  Legend,
} from "recharts";
import { Money } from "@/components/Money";
import type { Account } from "@/lib/types";
import { money, titleCase } from "@/lib/format";

const COLORS = ["#0c2f2a", "#1f7a6c", "#2a9b89", "#b45309", "#9f1239", "#64748b"];

export default function NetWorthPage() {
  const [data, setData] = useState<{
    netWorth: number;
    cash: number;
    creditUsed: number;
    investments: number;
    byType: Record<string, number>;
    accounts: Account[];
  } | null>(null);

  useEffect(() => {
    fetch("/api/net-worth")
      .then((r) => r.json())
      .then(setData);
  }, []);

  if (!data) return <p className="text-ink-soft">Calculating…</p>;

  const pie = Object.entries(data.byType).map(([name, value]) => ({
    name: titleCase(name),
    value: Math.abs(value),
    raw: value,
  }));

  return (
    <div className="space-y-8">
      <div>
        <p className="text-sm uppercase tracking-[0.18em] text-ink-soft/60">Net worth</p>
        <h1 className="mt-1 text-4xl text-ink">Assets minus liabilities</h1>
      </div>

      <div className="panel rounded-2xl p-6">
        <p className="text-sm text-ink-soft/70">Total net worth</p>
        <p className="mt-2 text-5xl text-ink">
          <Money value={data.netWorth} />
        </p>
        <div className="mt-6 grid gap-4 sm:grid-cols-3">
          <div>
            <p className="text-sm text-ink-soft/60">Cash</p>
            <p className="text-xl">
              <Money value={data.cash} />
            </p>
          </div>
          <div>
            <p className="text-sm text-ink-soft/60">Investments</p>
            <p className="text-xl">
              <Money value={data.investments} />
            </p>
          </div>
          <div>
            <p className="text-sm text-ink-soft/60">Credit used</p>
            <p className="text-xl">
              <Money value={data.creditUsed} accent="danger" />
            </p>
          </div>
        </div>
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        <div className="panel rounded-2xl p-5">
          <h2 className="mb-4 text-2xl text-ink">Mix by account type</h2>
          <div className="h-72">
            <ResponsiveContainer width="100%" height="100%">
              <PieChart>
                <Pie data={pie} dataKey="value" nameKey="name" innerRadius={55} outerRadius={95}>
                  {pie.map((_, i) => (
                    <Cell key={i} fill={COLORS[i % COLORS.length]} />
                  ))}
                </Pie>
                <Tooltip formatter={(v) => money(Number(v ?? 0))} />
                <Legend />
              </PieChart>
            </ResponsiveContainer>
          </div>
        </div>

        <div className="panel rounded-2xl p-5">
          <h2 className="mb-4 text-2xl text-ink">Included accounts</h2>
          <ul className="divide-y divide-line">
            {data.accounts
              .filter((a) => a.include_in_net_worth)
              .map((a) => (
                <li key={a.id} className="flex justify-between gap-3 py-3 text-sm">
                  <span>
                    {a.name}
                    <span className="text-ink-soft/55"> · {titleCase(a.type)}</span>
                  </span>
                  <Money
                    value={
                      a.type === "credit" || a.type === "loan"
                        ? -Math.abs(a.balance_current)
                        : a.balance_current
                    }
                  />
                </li>
              ))}
          </ul>
        </div>
      </div>
    </div>
  );
}
