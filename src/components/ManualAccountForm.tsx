"use client";

import { useState } from "react";

const TYPES = ["checking", "savings", "credit", "investment", "loan", "other"] as const;

export function ManualAccountForm({ onCreated }: { onCreated?: () => void }) {
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [form, setForm] = useState({
    name: "",
    type: "checking" as (typeof TYPES)[number],
    balance_current: "",
    institution_name: "",
    mask: "",
  });

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true);
    try {
      const res = await fetch("/api/accounts", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          ...form,
          balance_current: Number(form.balance_current || 0),
          institution_name: form.institution_name || null,
          mask: form.mask || null,
        }),
      });
      if (!res.ok) throw new Error("Failed");
      setForm({ name: "", type: "checking", balance_current: "", institution_name: "", mask: "" });
      setOpen(false);
      onCreated?.();
    } finally {
      setBusy(false);
    }
  };

  if (!open) {
    return (
      <button type="button" className="btn btn-ghost" onClick={() => setOpen(true)}>
        Add account manually
      </button>
    );
  }

  return (
    <form onSubmit={submit} className="panel space-y-3 rounded-2xl p-4">
      <div className="grid gap-3 sm:grid-cols-2">
        <label className="block text-sm">
          <span className="mb-1 block text-ink-soft">Name</span>
          <input
            className="input"
            required
            value={form.name}
            onChange={(e) => setForm({ ...form, name: e.target.value })}
            placeholder="Everyday Checking"
          />
        </label>
        <label className="block text-sm">
          <span className="mb-1 block text-ink-soft">Type</span>
          <select
            className="select"
            value={form.type}
            onChange={(e) => setForm({ ...form, type: e.target.value as (typeof TYPES)[number] })}
          >
            {TYPES.map((t) => (
              <option key={t} value={t}>
                {t}
              </option>
            ))}
          </select>
        </label>
        <label className="block text-sm">
          <span className="mb-1 block text-ink-soft">Balance</span>
          <input
            className="input"
            required
            type="number"
            step="0.01"
            value={form.balance_current}
            onChange={(e) => setForm({ ...form, balance_current: e.target.value })}
            placeholder="1200.00"
          />
        </label>
        <label className="block text-sm">
          <span className="mb-1 block text-ink-soft">Institution</span>
          <input
            className="input"
            value={form.institution_name}
            onChange={(e) => setForm({ ...form, institution_name: e.target.value })}
            placeholder="Chase"
          />
        </label>
      </div>
      <div className="flex gap-2">
        <button type="submit" className="btn btn-sea" disabled={busy}>
          {busy ? "Saving…" : "Save account"}
        </button>
        <button type="button" className="btn btn-ghost" onClick={() => setOpen(false)}>
          Cancel
        </button>
      </div>
    </form>
  );
}
