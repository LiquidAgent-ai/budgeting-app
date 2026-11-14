const ACCESS_TOKEN_RE = /access-(?:sandbox|development|production)-[A-Za-z0-9_-]+/g;
const PUBLIC_TOKEN_RE = /public-(?:sandbox|development|production)-[A-Za-z0-9_-]+/g;
const LINK_TOKEN_RE = /link-(?:sandbox|development|production)-[A-Za-z0-9_-]+/g;

/** Remove Plaid secrets, the database key, and access tokens from a string before it is shown or logged. */
export function redactSecrets(text: string): string {
  let out = text;
  for (const value of [process.env.DATABASE_KEY, process.env.PLAID_SECRET, process.env.PLAID_CLIENT_ID]) {
    if (value && value.length >= 8) {
      out = out.split(value).join("[redacted]");
      const encoded = encodeURIComponent(value);
      if (encoded !== value) out = out.split(encoded).join("[redacted]");
    }
  }
  return out.replace(ACCESS_TOKEN_RE, "[redacted]").replace(PUBLIC_TOKEN_RE, "[redacted]").replace(LINK_TOKEN_RE, "[redacted]");
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

function plaidErrorCode(err: object): string | null {
  const data = (err as { response?: { data?: unknown } }).response?.data;
  if (!data || typeof data !== "object") return null;
  const code = (data as { error_code?: unknown }).error_code;
  if (typeof code !== "string") return null;
  const trimmed = code.trim();
  if (!/^[A-Z0-9_]{1,64}$/.test(trimmed)) return null;
  return trimmed;
}

function messageIsSafe(message: string): boolean {
  if (!message || message.length > 300) return false;
  if (/[\r\n]/.test(message)) return false;
  if (/\b\d{8,}\b/.test(message)) return false;
  if (/\b(select|insert|update|delete|pragma)\b/i.test(message)) return false;
  if (/[/\\]/.test(message)) return false;
  if (message.includes("@")) return false;
  return true;
}

const SAFE_LOG_MESSAGE =
  /^(?:DATABASE_KEY is missing\b|The database\b|Could not encrypt\b|Encrypting the database\b|Encryption did not\b|The encrypted database\b|Plaid is not configured\b|Refusing Plaid product\b)/;

/**
 * A short label for a log line. Plaid failures keep the error code only.
 * Messages are included only for Anchored's own errors. Other errors keep the
 * error name so a path, SQL statement, or user value is not written.
 */
export function safeErrorName(err: unknown, fallback: string): string {
  if (!err || typeof err !== "object") return fallback;
  const code = plaidErrorCode(err);
  if (code) return code;
  if ("isAxiosError" in err || "config" in err) return "PlaidRequestFailed";
  if (err instanceof Error) {
    if (err.name && err.name !== "Error") return err.name.slice(0, 40);
    const message = redactSecrets(err.message).replace(/[\r\n\t]+/g, " ").trim();
    if (!SAFE_LOG_MESSAGE.test(message) || !messageIsSafe(message)) return fallback;
    return message.slice(0, 180);
  }
  return fallback;
}
