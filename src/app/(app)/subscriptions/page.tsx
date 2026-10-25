"use client";

import { useEffect, useState } from "react";
import { Money } from "@/components/Money";

type Sub = {
  label: string;
  category: string;
  occurrences: number;
  avg_amount: number;
  month_amount: number;
  last_seen: string;
};

export default function SubscriptionsPage() {
  const [subs, setSubs] = useState<Sub[]>([]);
  const [total, setTotal] = useState(0);

  useEffect(() => {
    fetch("/api/subscriptions")
      .then((r) => r.json())
      .then((d) => {
        setSubs(d.subscriptions || []);
        setTotal(d.monthTotal || 0);
      });
  }, []);

  return (
    <div className="space-y-8">
      <div>
        <p className="text-sm uppercase tracking-[0.18em] text-ink-soft/60">Subscriptions</p>
        <h1 className="mt-1 text-4xl text-ink">Recurring charges that stick around</h1>
        <p className="mt-2 text-ink-soft/75">
          This month&apos;s detected total: <Money value={total} className="font-medium text-ink" />
        </p>
      </div>

      <ul className="panel divide-y divide-line rounded-2xl">
        {subs.map((s) => (
          <li key={s.label} className="flex items-center justify-between gap-4 px-5 py-4">
            <div>
              <p className="font-medium text-ink">{s.label}</p>
              <p className="text-sm text-ink-soft/65">
                {s.category} · avg <Money value={s.avg_amount} /> · last {s.last_seen}
              </p>
            </div>
            <Money value={s.month_amount || s.avg_amount} className="text-lg" />
          </li>
        ))}
        {subs.length === 0 && (
          <li className="px-5 py-8 text-center text-ink-soft">
            No subscriptions detected yet. Link accounts or load demo data.
          </li>
        )}
      </ul>
    </div>
  );
}
