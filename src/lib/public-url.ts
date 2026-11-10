/**
 * Public address of this Anchored server.
 * APP_URL wins when it is set. In the container, the proxy headers are next.
 * On a PC, those proxy headers are ignored.
 */

function runtimeEnv(name: string): string {
  // A direct process.env.NAME read is baked in when middleware is built.
  // Computed access stays live for the container, which sets these at start.
  const env = process.env;
  return (env[name] ?? "").trim();
}

export function isContainerServer(): boolean {
  const value = runtimeEnv("ANCHORED_CONTAINER").toLowerCase();
  return value === "1" || value === "true" || value === "yes";
}

function firstForwarded(value: string | null | undefined): string | null {
  if (!value) return null;
  const first = value.split(",")[0]?.trim() ?? "";
  return first || null;
}

function schemeOf(value: string | null | undefined): "http" | "https" | null {
  const raw = firstForwarded(value)?.replace(/:$/, "").toLowerCase();
  if (raw === "http" || raw === "https") return raw;
  return null;
}

/** Origin from APP_URL, or null when it is unset or not an http(s) URL. */
export function appUrlOrigin(): string | null {
  const raw = runtimeEnv("APP_URL");
  if (!raw) return null;
  try {
    const url = new URL(raw);
    if (url.username || url.password) return null;
    if (url.protocol !== "http:" && url.protocol !== "https:") return null;
    return url.origin;
  } catch {
    return null;
  }
}

/**
 * Absolute origin for links. APP_URL when set. Otherwise the proxy's
 * X-Forwarded-Proto and X-Forwarded-Host inside the container, then the
 * request host. The dev server does not trust forwarded headers.
 */
export function publicOrigin(input?: {
  host?: string | null;
  proto?: string | null;
  forwardedHost?: string | null;
  forwardedProto?: string | null;
}): string | null {
  const configured = appUrlOrigin();
  if (configured) return configured;
  if (!input) return null;
  const trustForwarded = isContainerServer();
  const host = (trustForwarded ? firstForwarded(input.forwardedHost) : null) || input.host?.trim() || "";
  if (!host) return null;
  const proto = (trustForwarded ? schemeOf(input.forwardedProto) : null) || schemeOf(input.proto) || "http";
  try {
    return new URL(`${proto}://${host}`).origin;
  } catch {
    return null;
  }
}
