const ACCESS_TOKEN_RE = /access-(?:sandbox|development|production)-[A-Za-z0-9_-]+/g;

/** Remove Plaid secrets and access tokens from a string before it is shown or logged. */
export function redactSecrets(text: string): string {
  let out = text;
  for (const value of [process.env.PLAID_SECRET, process.env.PLAID_CLIENT_ID]) {
    if (value && value.length >= 8) out = out.split(value).join("[redacted]");
  }
  return out.replace(ACCESS_TOKEN_RE, "[redacted]");
}

function plaidSafeField(err: object): string | null {
  const data = (err as { response?: { data?: unknown } }).response?.data;
  if (!data || typeof data !== "object") return null;
  const body = data as Record<string, unknown>;
  for (const key of ["display_message", "error_message", "error_code"]) {
    const value = body[key];
    if (typeof value === "string" && value.trim()) return value.trim();
  }
  return null;
}

/**
 * Text that is safe to put in an HTTP response.
 * Axios errors carry the request config, including the Plaid secret and access token.
 * Those objects are never copied onto the response.
 */
export function safeClientError(err: unknown, fallback: string): string {
  let message = fallback;
  if (err && typeof err === "object") {
    const fromBody = plaidSafeField(err);
    if (fromBody) {
      message = fromBody;
    } else if ("isAxiosError" in err || "config" in err) {
      const status = (err as { response?: { status?: number } }).response?.status;
      message = status ? `Plaid request failed (${status})` : fallback;
    } else if (err instanceof Error && err.message) {
      message = err.message;
    }
  }
  return redactSecrets(message).slice(0, 300);
}
