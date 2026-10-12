"use client";

import { useCallback, useEffect, useState } from "react";
import { usePlaidLink } from "react-plaid-link";

export function PlaidLinkButton({ onLinked }: { onLinked?: () => void }) {
  const [token, setToken] = useState<string | null>(null);
  const [configured, setConfigured] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    fetch("/api/plaid/status")
      .then((r) => r.json())
      .then((d) => setConfigured(Boolean(d.configured)))
      .catch(() => setConfigured(false));
  }, []);

  const createToken = async () => {
    setBusy(true);
    setError(null);
    try {
      const res = await fetch("/api/plaid/create-link-token", { method: "POST" });
      const data = await res.json();
      if (!res.ok) throw new Error(data.hint || data.error || "Could not create link token");
      setToken(data.link_token);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed");
    } finally {
      setBusy(false);
    }
  };

  const onSuccess = useCallback(
    async (publicToken: string | null, metadata: { institution?: { name?: string } | null }) => {
      if (!publicToken) {
        setError("No public token returned from Plaid");
        return;
      }
      setBusy(true);
      try {
        const res = await fetch("/api/plaid/exchange-token", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            public_token: publicToken,
            institution: metadata.institution,
          }),
        });
        const data = await res.json();
        if (!res.ok) throw new Error(data.error || "Exchange failed");
        onLinked?.();
      } catch (e) {
        setError(e instanceof Error ? e.message : "Link failed");
      } finally {
        setBusy(false);
        setToken(null);
      }
    },
    [onLinked]
  );

  const { open, ready } = usePlaidLink({
    token,
    onSuccess,
  });

  useEffect(() => {
    if (token && ready) open();
  }, [token, ready, open]);

  if (configured === false) {
    return (
      <div className="panel rounded-2xl p-4 text-sm text-ink-soft">
        <p className="font-medium text-ink">Bank linking via Plaid</p>
        <p className="mt-1 leading-relaxed">
          Add free sandbox keys to <code className="text-sea">.env.local</code> (
          <code>PLAID_CLIENT_ID</code>, <code>PLAID_SECRET</code>) from{" "}
          <a className="underline" href="https://dashboard.plaid.com/" target="_blank" rel="noreferrer">
            dashboard.plaid.com
          </a>
          . Until then, add accounts manually or load demo data.
        </p>
      </div>
    );
  }

  return (
    <div className="space-y-2">
      <button
        type="button"
        className="btn btn-primary"
        disabled={busy || configured === null}
        onClick={createToken}
      >
        {busy ? "Connecting…" : "Link bank with Plaid"}
      </button>
      {error && <p className="text-sm text-danger">{error}</p>}
    </div>
  );
}
