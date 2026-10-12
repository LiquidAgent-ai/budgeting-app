"use client";

import { useCallback, useEffect, useState } from "react";
import { ManualAccountForm } from "@/components/ManualAccountForm";
import { Money } from "@/components/Money";
import { PlaidLinkButton } from "@/components/PlaidLinkButton";
import type { Account } from "@/lib/types";
import { titleCase } from "@/lib/format";

export default function AccountsPage() {
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [syncMsg, setSyncMsg] = useState<string | null>(null);

  const load = useCallback(() => {
    fetch("/api/accounts")
      .then((r) => r.json())
      .then((d) => setAccounts(d.accounts || []));
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const sync = async () => {
    setSyncMsg("Syncing…");
    const res = await fetch("/api/plaid/sync", { method: "POST" });
    const data = await res.json();
    if (!res.ok) setSyncMsg(data.error || "Sync failed");
    else {
      setSyncMsg(`Synced ${data.synced} new transactions`);
      load();
    }
  };

  const remove = async (id: string) => {
    await fetch(`/api/accounts/${id}`, { method: "DELETE" });
    load();
  };

  const seed = async () => {
    await fetch("/api/seed", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ fresh: true }),
    });
    load();
  };

  const groups: Record<string, Account[]> = {};
  for (const a of accounts) {
    (groups[a.type] ||= []).push(a);
  }

  return (
    <div className="space-y-8">
      <div>
        <p className="text-sm uppercase tracking-[0.18em] text-ink-soft/60">Accounts</p>
        <h1 className="mt-1 text-4xl text-ink">Dock checking, credit, and investments</h1>
        <p className="mt-2 max-w-2xl text-ink-soft/75">
          Connect live institutions with Plaid, or keep everything local with manual balances. Demo
          data is available so you can explore before linking anything real.
        </p>
      </div>

      <div className="flex flex-wrap items-start gap-4">
        <PlaidLinkButton onLinked={load} />
        <ManualAccountForm onCreated={load} />
        <button type="button" className="btn btn-ghost" onClick={sync}>
          Sync linked accounts
        </button>
        <button type="button" className="btn btn-ghost" onClick={seed}>
          Load demo household
        </button>
      </div>
      {syncMsg && <p className="text-sm text-ink-soft">{syncMsg}</p>}

      <div className="space-y-6">
        {Object.keys(groups).length === 0 && (
          <div className="panel rounded-2xl p-6 text-ink-soft">
            No accounts yet. Link with Plaid or add one manually.
          </div>
        )}
        {Object.entries(groups).map(([type, list]) => (
          <section key={type}>
            <h2 className="mb-3 text-2xl text-ink">{titleCase(type)}</h2>
            <ul className="space-y-3">
              {list.map((a) => (
                <li
                  key={a.id}
                  className="panel flex flex-wrap items-center justify-between gap-3 rounded-2xl px-5 py-4"
                >
                  <div>
                    <p className="font-medium text-ink">{a.name}</p>
                    <p className="text-sm text-ink-soft/65">
                      {[a.institution_name, a.mask ? `••••${a.mask}` : null, a.is_manual ? "manual" : "plaid"]
                        .filter(Boolean)
                        .join(" · ")}
                    </p>
                  </div>
                  <div className="flex items-center gap-4">
                    <p className="text-xl">
                      <Money
                        value={
                          a.type === "credit" || a.type === "loan"
                            ? Math.abs(a.balance_current)
                            : a.balance_current
                        }
                      />
                    </p>
                    <button
                      type="button"
                      className="text-sm text-danger hover:underline"
                      onClick={() => remove(a.id)}
                    >
                      Remove
                    </button>
                  </div>
                </li>
              ))}
            </ul>
          </section>
        ))}
      </div>
    </div>
  );
}
