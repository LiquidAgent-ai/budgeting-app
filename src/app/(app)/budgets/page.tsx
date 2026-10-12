"use client";

import { useEffect, useState } from "react";
import { Money } from "@/components/Money";
import { CATEGORIES } from "@/lib/categories";
import { pct } from "@/lib/format";

type BudgetRow = {
  id: string;
  category: string;
  monthly_limit: number;
  spent: number;
};

export default function BudgetsPage() {
  const [budgets, setBudgets] = useState<BudgetRow[]>([]);
  const [category, setCategory] = useState("Groceries");
  const [limit, setLimit] = useState("400");

  const load = () => {
    fetch("/api/budgets")
      .then((r) => r.json())
      .then((d) => setBudgets(d.budgets || []));
  };

  useEffect(() => {
    load();
  }, []);

  const save = async (e: React.FormEvent) => {
    e.preventDefault();
    await fetch("/api/budgets", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ category, monthly_limit: Number(limit) }),
    });
    load();
  };

  const remove = async (cat: string) => {
    await fetch(`/api/budgets?category=${encodeURIComponent(cat)}`, { method: "DELETE" });
    load();
  };

  return (
    <div className="space-y-8">
      <div>
        <p className="text-sm uppercase tracking-[0.18em] text-ink-soft/60">Budgets</p>
        <h1 className="mt-1 text-4xl text-ink">Give every category a ceiling</h1>
      </div>

      <form onSubmit={save} className="panel flex flex-wrap items-end gap-3 rounded-2xl p-4">
        <label className="text-sm">
          <span className="mb-1 block text-ink-soft">Category</span>
          <select className="select" value={category} onChange={(e) => setCategory(e.target.value)}>
            {CATEGORIES.filter((c) => !["Income", "Transfer", "Investments"].includes(c)).map(
              (c) => (
                <option key={c} value={c}>
                  {c}
                </option>
              )
            )}
          </select>
        </label>
        <label className="text-sm">
          <span className="mb-1 block text-ink-soft">Monthly limit</span>
          <input
            className="input"
            type="number"
            min="1"
            step="1"
            value={limit}
            onChange={(e) => setLimit(e.target.value)}
          />
        </label>
        <button type="submit" className="btn btn-sea">
          Save budget
        </button>
      </form>

      <ul className="space-y-3">
        {budgets.map((b) => {
          const p = pct(b.spent, b.monthly_limit);
          const over = b.spent > b.monthly_limit;
          return (
            <li key={b.id} className="panel rounded-2xl p-5">
              <div className="flex flex-wrap items-center justify-between gap-3">
                <div>
                  <p className="text-xl text-ink">{b.category}</p>
                  <p className="text-sm text-ink-soft/70">
                    <Money value={b.spent} /> of <Money value={b.monthly_limit} />
                    {over ? " — over" : ""}
                  </p>
                </div>
                <button
                  type="button"
                  className="text-sm text-danger hover:underline"
                  onClick={() => remove(b.category)}
                >
                  Remove
                </button>
              </div>
              <div className="progress mt-4">
                <span
                  style={{
                    width: `${Math.min(p, 100)}%`,
                    background: over
                      ? "linear-gradient(90deg,#9f1239,#e11d48)"
                      : undefined,
                  }}
                />
              </div>
            </li>
          );
        })}
        {budgets.length === 0 && (
          <li className="panel rounded-2xl p-6 text-ink-soft">No budgets yet.</li>
        )}
      </ul>
    </div>
  );
}
