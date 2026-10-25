"use client";

import { useEffect, useState } from "react";
import { Money } from "@/components/Money";
import { CATEGORIES } from "@/lib/categories";
import { titleCase } from "@/lib/format";
import type { Account, Transaction } from "@/lib/types";

type Row = Transaction & { account_name: string };

export default function TransactionsPage() {
  const [rows, setRows] = useState<Row[]>([]);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [category, setCategory] = useState("");
  const [showForm, setShowForm] = useState(false);
  const [form, setForm] = useState({
    account_id: "",
    amount: "",
    date: new Date().toISOString().slice(0, 10),
    name: "",
    category: "Other",
  });

  const load = (cat?: string) => {
    const q = cat ? `?category=${encodeURIComponent(cat)}` : "";
    fetch(`/api/transactions${q}`)
      .then((r) => r.json())
      .then((d) => setRows(d.transactions || []));
  };

  useEffect(() => {
    load();
    fetch("/api/accounts")
      .then((r) => r.json())
      .then((d) => {
        setAccounts(d.accounts || []);
        if (d.accounts?.[0]) setForm((f) => ({ ...f, account_id: d.accounts[0].id }));
      });
  }, []);

  const filter = (cat: string) => {
    setCategory(cat);
    load(cat || undefined);
  };

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    await fetch("/api/transactions", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        ...form,
        amount: Number(form.amount),
      }),
    });
    setShowForm(false);
    load(category || undefined);
  };

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <p className="text-sm uppercase tracking-[0.18em] text-ink-soft/60">Activity</p>
          <h1 className="mt-1 text-4xl text-ink">Every charge, categorized</h1>
        </div>
        <button type="button" className="btn btn-sea" onClick={() => setShowForm((v) => !v)}>
          {showForm ? "Close" : "Add transaction"}
        </button>
      </div>

      {showForm && (
        <form onSubmit={submit} className="panel grid gap-3 rounded-2xl p-4 sm:grid-cols-2">
          <label className="text-sm">
            <span className="mb-1 block text-ink-soft">Account</span>
            <select
              className="select"
              value={form.account_id}
              onChange={(e) => setForm({ ...form, account_id: e.target.value })}
              required
            >
              {accounts.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.name}
                </option>
              ))}
            </select>
          </label>
          <label className="text-sm">
            <span className="mb-1 block text-ink-soft">Amount (positive = spend)</span>
            <input
              className="input"
              type="number"
              step="0.01"
              required
              value={form.amount}
              onChange={(e) => setForm({ ...form, amount: e.target.value })}
            />
          </label>
          <label className="text-sm">
            <span className="mb-1 block text-ink-soft">Date</span>
            <input
              className="input"
              type="date"
              required
              value={form.date}
              onChange={(e) => setForm({ ...form, date: e.target.value })}
            />
          </label>
          <label className="text-sm">
            <span className="mb-1 block text-ink-soft">Name</span>
            <input
              className="input"
              required
              value={form.name}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
            />
          </label>
          <label className="text-sm sm:col-span-2">
            <span className="mb-1 block text-ink-soft">Category</span>
            <select
              className="select"
              value={form.category}
              onChange={(e) => setForm({ ...form, category: e.target.value })}
            >
              {CATEGORIES.map((c) => (
                <option key={c} value={c}>
                  {c}
                </option>
              ))}
            </select>
          </label>
          <button type="submit" className="btn btn-primary sm:col-span-2">
            Save
          </button>
        </form>
      )}

      <div className="flex flex-wrap gap-2">
        <button
          type="button"
          onClick={() => filter("")}
          className={`rounded-full px-3 py-1 text-sm ${!category ? "bg-ink text-foam" : "bg-foam/70"}`}
        >
          All
        </button>
        {CATEGORIES.map((c) => (
          <button
            key={c}
            type="button"
            onClick={() => filter(c)}
            className={`rounded-full px-3 py-1 text-sm ${
              category === c ? "bg-ink text-foam" : "bg-foam/70"
            }`}
          >
            {c}
          </button>
        ))}
      </div>

      <ul className="panel divide-y divide-line rounded-2xl">
        {rows.map((t) => (
          <li key={t.id} className="flex items-center justify-between gap-4 px-5 py-3 text-sm">
            <div className="min-w-0">
              <p className="truncate font-medium">{t.merchant_name || t.name}</p>
              <p className="text-ink-soft/60">
                {t.date} · {titleCase(t.category)}
                {t.is_subscription ? " · subscription" : ""} · {t.account_name}
              </p>
            </div>
            <Money value={t.amount} accent={t.amount < 0 ? "good" : undefined} />
          </li>
        ))}
        {rows.length === 0 && (
          <li className="px-5 py-8 text-center text-ink-soft">No transactions yet.</li>
        )}
      </ul>
    </div>
  );
}
